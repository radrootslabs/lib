//! Owner-private pool creation on the existing connection owner.
use super::*;
use radroots_domain::{
    CheckpointDigest, CommandDigest, OperationId, PoolId, UnixSeconds,
    stock::{PhysicalBalances, PoolBalances, PoolSnapshot},
};
use radroots_storage::{
    command::{CommitAttempt, OperationError, protected::same_binding},
    inventory::{CreatePoolRequest, PoolCreationOutcome, PoolCreationReceipt},
};
use serde_json::value::RawValue;
use sha2::{Digest, Sha256};
use sqlx::Row;

pub(super) async fn check_head(
    connection: &mut SqliteConnection,
    checkpoint: InventoryCheckpoint,
) -> Result<(), WorkspaceError> {
    let row=sqlx::query("SELECT count(*),coalesce(max(revision),0),coalesce(sum(CASE WHEN revision=? AND chain_head=? THEN 1 ELSE 0 END),0) FROM stock_operations")
        .bind(checkpoint.revision.get() as i64).bind(checkpoint.head.map(|id|id.as_bytes().to_vec())).fetch_one(&mut *connection).await.map_err(|_|WorkspaceError::RecoveryRequired)?;
    let count: i64 = row
        .try_get(0)
        .map_err(|_| WorkspaceError::RecoveryRequired)?;
    let max: i64 = row
        .try_get(1)
        .map_err(|_| WorkspaceError::RecoveryRequired)?;
    let matched: i64 = row
        .try_get(2)
        .map_err(|_| WorkspaceError::RecoveryRequired)?;
    let pools: i64 = sqlx::query_scalar("SELECT count(*) FROM stock_pools")
        .fetch_one(&mut *connection)
        .await
        .map_err(|_| WorkspaceError::RecoveryRequired)?;
    let movements: i64 = sqlx::query_scalar("SELECT count(*) FROM stock_movements")
        .fetch_one(&mut *connection)
        .await
        .map_err(|_| WorkspaceError::RecoveryRequired)?;
    if pools != checkpoint.pools.get() as i64 || movements != checkpoint.movements.get() as i64 {
        return Err(WorkspaceError::RecoveryRequired);
    }
    if count != checkpoint.revision.get() as i64
        || max != count
        || matched != i64::from(checkpoint.head.is_some())
    {
        return Err(WorkspaceError::RecoveryRequired);
    }
    Ok(())
}
struct Access {
    authority: Authority,
    record: GrantRecord,
    root: Vec<u8>,
    grant: Vec<u8>,
    current: WorkspaceBinding,
    minimum_proof: MinimumProof,
}
pub(in crate::workspace) struct Turn<'a> {
    pub connection: &'a mut SqliteConnection,
    pub namespace: &'a Namespace,
    pub selected: &'a AttachmentInput,
    pub crypto: &'a WorkspaceCrypto,
    pub clock: &'a dyn WorkspaceClock,
    pub uncertain: &'a mut bool,
}
impl Turn<'_> {
    async fn access(
        &mut self,
        binding: &WorkspaceBinding,
        minimum: radroots_storage::inventory::InventoryCheckpointRef,
        proof: &AttachmentProof,
    ) -> Result<Access, OperationError> {
        if *self.uncertain {
            return Err(OperationError::RecoveryRequired);
        }
        self.namespace.validate()?;
        let (authority, record, root, grant) =
            super::read(self.connection, self.selected, self.crypto).await?;
        let current = record.authenticate(binding.input(), proof, self.clock.now()?)?;
        if current.scope() != WorkspaceScope::SellerStockOwner {
            return Err(OperationError::Unauthorized);
        }
        same_binding(binding, &current)?;
        if authority.inventory.is_none() {
            return Err(OperationError::UnsupportedOperation);
        }

        let minimum_proof = authenticate_minimum(
            self.connection,
            self.selected,
            self.crypto,
            &current,
            authority
                .inventory
                .ok_or(OperationError::UnsupportedOperation)?,
            minimum,
        )
        .await?;
        self.namespace.validate()?;
        Ok(Access {
            authority,
            record,
            root,
            grant,
            current,
            minimum_proof,
        })
    }
    pub async fn create(
        &mut self,
        request: CreatePoolRequest,
        proof: AttachmentProof,
    ) -> Result<PoolCreationOutcome, OperationError> {
        let mut access = self
            .access(request.binding(), request.minimum_checkpoint(), &proof)
            .await?;
        let context = request.command().context();
        let operation = context.operation_id();
        let digest = request.command().digest();
        // Retained identity precedes every fresh deadline or pool-existence check.
        if let Some(retained) = read_operation(
            self.connection,
            self.selected,
            self.crypto,
            request.binding(),
            operation,
        )
        .await?
        {
            if retained.digest != digest {
                return Err(OperationError::IdempotencyConflict);
            }
            self.confirm(&access, &proof, Some(&retained), None, None, None)
                .await?;
            return Ok(PoolCreationOutcome::Committed(Box::new(retained.receipt)));
        }
        if access.current.observed_at() < request.binding().observed_at() {
            return Err(WorkspaceError::Unavailable.into());
        }
        if access.current.observed_at() >= context.not_after() {
            return Err(OperationError::DeadlineExceeded);
        }
        let input = request.command().input();
        if read_pool(self.connection, self.selected, self.crypto, input.id())
            .await?
            .is_some()
        {
            return Err(OperationError::StaleState);
        }
        let zero =
            radroots_domain::Sequence::new(1).map_err(|_| OperationError::RecoveryRequired)?;
        let pool = PoolSnapshot::new(
            input.id(),
            input.unit(),
            PoolBalances::new(
                input.initial_ticks(),
                PhysicalBalances {
                    available: input.initial_ticks(),
                    ..PhysicalBalances::ZERO
                },
            )
            .map_err(|_| OperationError::InvalidCommand)?,
            zero,
        );
        let transaction = TransactionId::from_bytes(super::super::random()?);
        let movement = if input.initial_ticks() == radroots_domain::stock::StockTicks::ZERO {
            None
        } else {
            Some(super::super::random()?)
        };
        let checkpoint = access
            .authority
            .inventory
            .ok_or(OperationError::UnsupportedOperation)?;
        let revision = Sequence::new(
            checkpoint
                .revision
                .get()
                .checked_add(1)
                .ok_or(WorkspaceError::Capacity)?,
        )
        .map_err(|_| WorkspaceError::Capacity)?;
        let head = chain_head(ChainInput {
            previous: checkpoint.head,
            revision,
            binding: request.binding(),
            operation,
            digest,
            transaction,
            pool,
            movement,
        })?;
        let attempt = CommitAttempt::new(
            self.selected.workspace_id(),
            operation,
            digest,
            Some(transaction),
        );
        let receipt = PoolCreationReceipt::from_committed_store(
            attempt,
            pool,
            radroots_storage::inventory::InventoryCheckpointRef::from_host_metadata(
                self.selected.authority(),
                revision,
                Some(head),
            )?,
        )?;
        let protected_pool = self
            .crypto
            .seal_stock_record(
                self.selected,
                "pool",
                pool.id().as_bytes(),
                &encode(&PoolWire {
                    version: 1,
                    label: input.label(),
                    pool,
                })?,
            )
            .await?;
        let protected_movement = if let Some(id) = movement {
            Some(
                self.crypto
                    .seal_stock_record(
                        self.selected,
                        "movement",
                        &id,
                        &encode(&Movement {
                            version: 1,
                            operation,
                            digest,
                            pool: pool.id(),
                            quantity: input.initial_ticks(),
                            kind: MovementKind::Introduced,
                        })?,
                    )
                    .await?,
            )
        } else {
            None
        };
        let command: &RawValue = serde_json::from_slice(request.command().protected_wire())
            .map_err(|_| OperationError::InvalidCommand)?;
        let protected_operation = self
            .crypto
            .seal_stock_record(
                self.selected,
                "operation",
                operation.as_bytes(),
                &encode(&OperationWire {
                    version: 1,
                    selection: request.binding().input(),
                    scope: request.binding().scope(),
                    principal: request.binding().principal_id(),
                    grant_revision: request.binding().grant_revision(),
                    grant_expires_at: request.binding().expires_at(),
                    observed_at: request.binding().observed_at(),
                    minimum_revision: request.minimum_checkpoint().revision(),
                    minimum_head: request.minimum_checkpoint().head(),
                    transaction,
                    revision,
                    previous_head: checkpoint.head,
                    head,
                    command,
                    pool,
                    movement,
                })?,
            )
            .await?;
        access.authority.inventory = Some(InventoryCheckpoint {
            revision,
            head: Some(head),
            pools: Sequence::new(
                checkpoint
                    .pools
                    .get()
                    .checked_add(1)
                    .ok_or(WorkspaceError::Capacity)?,
            )
            .map_err(|_| WorkspaceError::Capacity)?,
            movements: Sequence::new(
                checkpoint
                    .movements
                    .get()
                    .checked_add(u64::from(movement.is_some()))
                    .ok_or(WorkspaceError::Capacity)?,
            )
            .map_err(|_| WorkspaceError::Capacity)?,
        });
        let root = self
            .crypto
            .seal_stock_root(self.selected, 2, &access.authority.encode()?)
            .await?;
        self.namespace.validate()?;
        let mut tx = self
            .connection
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(|_| OperationError::StorageFailure)?;
        let changed:Result<(),OperationError>=async {
            super::unchanged(&mut tx,self.selected,&access.root,&access.grant).await?;
            check_head(&mut tx,checkpoint).await?;
            access.minimum_proof.confirm(&mut tx).await?;
            let current=access.record.authenticate(request.binding().input(),&proof,self.clock.now()?)?;
            same_binding(request.binding(),&current)?;
            if current.observed_at()<access.current.observed_at() {return Err(WorkspaceError::Unavailable.into());}
            if current.observed_at()>=context.not_after() {return Err(OperationError::DeadlineExceeded);}
            self.namespace.validate()?;
            let absent:i64=sqlx::query_scalar("SELECT count(*) FROM stock_operations WHERE actor=? AND grant_id=? AND operation_id=?").bind(request.binding().actor().participant().as_bytes().as_slice()).bind(request.binding().input().grant_id().as_bytes().as_slice()).bind(operation.as_bytes().as_slice()).fetch_one(&mut *tx).await.map_err(|_|OperationError::StorageFailure)?;
            if absent!=0 {return Err(OperationError::StaleState);}
            let existing:i64=sqlx::query_scalar("SELECT count(*) FROM stock_pools WHERE pool_id=?").bind(pool.id().as_bytes().as_slice()).fetch_one(&mut *tx).await.map_err(|_|OperationError::StorageFailure)?;
            if existing!=0 {return Err(OperationError::StaleState);}
            sqlx::query("INSERT INTO stock_pools(pool_id,revision,protected) VALUES(?,1,?)").bind(pool.id().as_bytes().as_slice()).bind(protected_pool).execute(&mut *tx).await.map_err(|_|OperationError::StorageFailure)?;
            sqlx::query("INSERT INTO stock_operations(actor,grant_id,operation_id,command_digest,transaction_id,pool_id,revision,chain_head,protected) VALUES(?,?,?,?,?,?,?,?,?)")
                .bind(request.binding().actor().participant().as_bytes().as_slice()).bind(request.binding().input().grant_id().as_bytes().as_slice()).bind(operation.as_bytes().as_slice()).bind(digest.as_bytes().as_slice()).bind(transaction.as_bytes().as_slice()).bind(pool.id().as_bytes().as_slice()).bind(revision.get() as i64).bind(head.as_bytes().as_slice()).bind(protected_operation).execute(&mut *tx).await.map_err(|_|OperationError::StorageFailure)?;
            if let (Some(id),Some(protected))=(movement,protected_movement) {
            sqlx::query("INSERT INTO stock_movements(movement_id,actor,grant_id,operation_id,pool_id,protected) VALUES(?,?,?,?,?,?)").bind(id.as_slice()).bind(request.binding().actor().participant().as_bytes().as_slice()).bind(request.binding().input().grant_id().as_bytes().as_slice()).bind(operation.as_bytes().as_slice()).bind(pool.id().as_bytes().as_slice()).bind(protected).execute(&mut *tx).await.map_err(|_|OperationError::StorageFailure)?;
            }
            let changed=sqlx::query("UPDATE stock_authority SET protected=? WHERE singleton=1 AND protected=?").bind(root).bind(&access.root).execute(&mut *tx).await.map_err(|_|OperationError::StorageFailure)?;
            if changed.rows_affected()!=1 {return Err(OperationError::RecoveryRequired);}self.namespace.validate()?;Ok(())
        }.await;
        if let Err(error) = changed {
            if tx.rollback().await.is_err() {
                *self.uncertain = true;
                return Ok(PoolCreationOutcome::Unknown(attempt));
            }
            return Err(error);
        }
        if tx.commit().await.is_err() {
            *self.uncertain = true;
            return Ok(PoolCreationOutcome::Unknown(attempt));
        }
        Ok(PoolCreationOutcome::Committed(Box::new(receipt)))
    }
    async fn confirm(
        &mut self,
        access: &Access,
        proof: &AttachmentProof,
        operation: Option<&Retained>,
        pool: Option<&RetainedPool>,
        absent_operation: Option<OperationId>,
        absent_pool: Option<PoolId>,
    ) -> Result<(), OperationError> {
        let mut tx = self
            .connection
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(|_| OperationError::StorageFailure)?;
        let checked:Result<(),OperationError>=async {
            self.namespace.validate()?;
            super::unchanged(&mut tx,self.selected,&access.root,&access.grant).await?;
            check_head(&mut tx,access.authority.inventory.ok_or(OperationError::UnsupportedOperation)?).await?;
            access.minimum_proof.confirm(&mut tx).await?;
            let current=access.record.authenticate(access.current.input(),proof,self.clock.now()?)?;
            same_binding(&access.current,&current)?;
            if let Some(operation)=absent_operation {let count:i64=sqlx::query_scalar("SELECT count(*) FROM stock_operations WHERE actor=? AND grant_id=? AND operation_id=?").bind(current.actor().participant().as_bytes().as_slice()).bind(current.input().grant_id().as_bytes().as_slice()).bind(operation.as_bytes().as_slice()).fetch_one(&mut *tx).await.map_err(|_|OperationError::StorageFailure)?;if count!=0 {return Err(OperationError::RecoveryRequired);}}
            if let Some(id)=absent_pool {let count:i64=sqlx::query_scalar("SELECT count(*) FROM stock_pools WHERE pool_id=?").bind(id.as_bytes().as_slice()).fetch_one(&mut *tx).await.map_err(|_|OperationError::StorageFailure)?;if count!=0 {return Err(OperationError::RecoveryRequired);}}
            if let Some(retained)=operation {
                let count:i64=sqlx::query_scalar("SELECT count(*) FROM stock_operations WHERE actor=? AND grant_id=? AND operation_id=? AND protected=?").bind(current.actor().participant().as_bytes().as_slice()).bind(current.input().grant_id().as_bytes().as_slice()).bind(retained.receipt.attempt().operation_id().as_bytes().as_slice()).bind(&retained.protected).fetch_one(&mut *tx).await.map_err(|_|OperationError::StorageFailure)?;
                if count!=1 {return Err(OperationError::RecoveryRequired);}
                let movement_count:i64=sqlx::query_scalar("SELECT count(*) FROM stock_movements WHERE actor=? AND grant_id=? AND operation_id=?").bind(current.actor().participant().as_bytes().as_slice()).bind(current.input().grant_id().as_bytes().as_slice()).bind(retained.receipt.attempt().operation_id().as_bytes().as_slice()).fetch_one(&mut *tx).await.map_err(|_|OperationError::StorageFailure)?;
                if movement_count!=i64::from(retained.movement.is_some()) {return Err(OperationError::RecoveryRequired);}
                if let (Some(id),Some(protected))=(retained.movement,retained.movement_protected.as_ref()) {
                    let count:i64=sqlx::query_scalar("SELECT count(*) FROM stock_movements WHERE movement_id=? AND protected=?").bind(id.as_slice()).bind(protected).fetch_one(&mut *tx).await.map_err(|_|OperationError::StorageFailure)?;
                    if count!=1 {return Err(OperationError::RecoveryRequired);}
                }

            }
            if let Some(pool)=pool {let count:i64=sqlx::query_scalar("SELECT count(*) FROM stock_pools WHERE pool_id=? AND revision=? AND protected=?").bind(pool.pool.id().as_bytes().as_slice()).bind(pool.pool.version().get() as i64).bind(&pool.protected).fetch_one(&mut *tx).await.map_err(|_|OperationError::StorageFailure)?;if count!=1 {return Err(OperationError::RecoveryRequired);}}
            Ok(())
        }.await;
        if tx.rollback().await.is_err() {
            *self.uncertain = true;
            return Err(OperationError::RecoveryRequired);
        }
        checked
    }
    pub async fn creation(
        &mut self,
        binding: WorkspaceBinding,
        minimum: radroots_storage::inventory::InventoryCheckpointRef,
        operation: OperationId,
        digest: Option<CommandDigest>,
        proof: AttachmentProof,
    ) -> Result<Option<PoolCreationReceipt>, OperationError> {
        let access = self.access(&binding, minimum, &proof).await?;
        let result = read_operation(
            self.connection,
            self.selected,
            self.crypto,
            &binding,
            operation,
        )
        .await?;
        if let Some(retained) = &result
            && digest.is_some_and(|d| d != retained.digest)
        {
            return Err(OperationError::IdempotencyConflict);
        }
        self.confirm(
            &access,
            &proof,
            result.as_ref(),
            None,
            result.is_none().then_some(operation),
            None,
        )
        .await?;
        Ok(result.map(|v| v.receipt))
    }
    pub async fn pool(
        &mut self,
        binding: WorkspaceBinding,
        minimum: radroots_storage::inventory::InventoryCheckpointRef,
        id: PoolId,
        proof: AttachmentProof,
    ) -> Result<Option<radroots_domain::stock::PoolView>, OperationError> {
        let access = self.access(&binding, minimum, &proof).await?;
        let result = read_pool(self.connection, self.selected, self.crypto, id).await?;
        self.confirm(
            &access,
            &proof,
            None,
            result.as_ref(),
            None,
            result.is_none().then_some(id),
        )
        .await?;
        Ok(result.map(|v| v.view))
    }
}
struct RetainedPool {
    view: radroots_domain::stock::PoolView,
    pool: PoolSnapshot,
    protected: Vec<u8>,
}
#[derive(Serialize)]
struct PoolWire<'a> {
    version: u8,
    label: &'a str,
    pool: PoolSnapshot,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PoolDecode {
    version: u8,
    label: String,
    pool: PoolSnapshot,
}
async fn read_pool(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    id: PoolId,
) -> Result<Option<RetainedPool>, OperationError> {
    let row=sqlx::query("SELECT revision,CASE WHEN length(protected) BETWEEN 1 AND 262144 THEN protected END FROM stock_pools WHERE pool_id=?").bind(id.as_bytes().as_slice()).fetch_optional(connection).await.map_err(|_|OperationError::RecoveryRequired)?;
    let Some(row) = row else { return Ok(None) };
    let revision: i64 = row
        .try_get(0)
        .map_err(|_| OperationError::RecoveryRequired)?;
    let protected: Vec<u8> = row
        .try_get::<Option<Vec<u8>>, _>(1)
        .map_err(|_| OperationError::RecoveryRequired)?
        .ok_or(OperationError::RecoveryRequired)?;
    let clear = crypto
        .open_stock_record(input, "pool", id.as_bytes(), &protected)
        .await?;
    let decoded: PoolDecode = clear
        .expose_secret(|bytes| super::closed_record(bytes))
        .map_err(|_| OperationError::RecoveryRequired)?;
    if decoded.version != 1
        || decoded.label.len() > 160
        || decoded.label.chars().any(char::is_control)
        || decoded.pool.id() != id
        || revision <= 0
        || decoded.pool.version().get() != revision as u64
    {
        return Err(OperationError::RecoveryRequired);
    }
    let view = radroots_domain::stock::PoolView::new(&decoded.label, decoded.pool)
        .map_err(|_| OperationError::RecoveryRequired)?;
    Ok(Some(RetainedPool {
        view,
        pool: decoded.pool,
        protected,
    }))
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum MovementKind {
    Introduced,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Movement {
    version: u8,
    operation: OperationId,
    digest: CommandDigest,
    pool: PoolId,
    quantity: radroots_domain::stock::StockTicks,
    kind: MovementKind,
}
#[derive(Serialize)]
struct OperationWire<'a> {
    version: u8,
    selection: &'a AttachmentInput,
    scope: WorkspaceScope,
    principal: &'a str,
    grant_revision: Sequence,
    grant_expires_at: UnixSeconds,
    observed_at: UnixSeconds,
    minimum_revision: Sequence,
    minimum_head: Option<CheckpointDigest>,
    transaction: TransactionId,
    revision: Sequence,
    previous_head: Option<CheckpointDigest>,
    head: CheckpointDigest,
    command: &'a RawValue,
    pool: PoolSnapshot,
    movement: Option<[u8; 32]>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OperationDecode<'a> {
    version: u8,
    selection: AttachmentInput,
    scope: WorkspaceScope,
    principal: String,
    grant_revision: Sequence,
    grant_expires_at: UnixSeconds,
    observed_at: UnixSeconds,
    minimum_revision: Sequence,
    #[serde(deserialize_with = "super::required_nullable")]
    minimum_head: Option<CheckpointDigest>,
    transaction: TransactionId,
    revision: Sequence,
    #[serde(deserialize_with = "super::required_nullable")]
    previous_head: Option<CheckpointDigest>,
    head: CheckpointDigest,
    #[serde(borrow)]
    command: &'a RawValue,
    pool: PoolSnapshot,
    #[serde(deserialize_with = "super::required_nullable")]
    movement: Option<[u8; 32]>,
}
struct OperationData {
    version: u8,
    transaction: TransactionId,
    revision: Sequence,
    previous_head: Option<CheckpointDigest>,
    head: CheckpointDigest,
    pool: PoolSnapshot,
    movement: Option<[u8; 32]>,
}
struct Retained {
    previous_head: Option<CheckpointDigest>,
    movement: Option<[u8; 32]>,
    movement_protected: Option<Vec<u8>>,
    receipt: PoolCreationReceipt,
    digest: CommandDigest,
    protected: Vec<u8>,
}
async fn read_operation(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    binding: &WorkspaceBinding,
    operation: OperationId,
) -> Result<Option<Retained>, OperationError> {
    let row=sqlx::query("SELECT CASE WHEN length(command_digest)=32 THEN command_digest END,CASE WHEN length(transaction_id)=32 THEN transaction_id END,CASE WHEN length(pool_id)=32 THEN pool_id END,revision,CASE WHEN length(chain_head)=32 THEN chain_head END,CASE WHEN length(protected) BETWEEN 1 AND 262144 THEN protected END FROM stock_operations WHERE actor=? AND grant_id=? AND operation_id=?")
        .bind(binding.actor().participant().as_bytes().as_slice()).bind(binding.input().grant_id().as_bytes().as_slice()).bind(operation.as_bytes().as_slice()).fetch_optional(&mut *connection).await.map_err(|_|OperationError::RecoveryRequired)?;
    let Some(row) = row else { return Ok(None) };
    let protected: Vec<u8> = row
        .try_get::<Option<Vec<u8>>, _>(5)
        .map_err(|_| OperationError::RecoveryRequired)?
        .ok_or(OperationError::RecoveryRequired)?;
    let clear = crypto
        .open_stock_record(input, "operation", operation.as_bytes(), &protected)
        .await?;
    let (request, decoded) = clear.expose_secret(|bytes| -> Result<_, OperationError> {
        let decoded: OperationDecode<'_> =
            super::closed_record(bytes).map_err(|_| OperationError::RecoveryRequired)?;
        let captured = WorkspaceBinding::from_authenticated_host(
            decoded.selection,
            decoded.scope,
            binding.actor(),
            decoded.principal,
            decoded.grant_revision,
            decoded.grant_expires_at,
            decoded.observed_at,
        )?;
        let minimum = radroots_storage::inventory::InventoryCheckpointRef::from_host_metadata(
            captured.input().authority(),
            decoded.minimum_revision,
            decoded.minimum_head,
        )?;
        let request = CreatePoolRequest::from_protected_wire(
            captured,
            minimum,
            decoded.command.get().as_bytes(),
        )?;
        Ok((
            request,
            OperationData {
                version: decoded.version,
                transaction: decoded.transaction,
                revision: decoded.revision,
                previous_head: decoded.previous_head,
                head: decoded.head,
                pool: decoded.pool,
                movement: decoded.movement,
            },
        ))
    })?;
    same_binding(request.binding(), binding)?;
    let digest = request.command().digest();
    let pool_input = request.command().input();
    let physical = decoded.pool.balances().physical();
    if decoded.version != 1
        || request.command().context().operation_id() != operation
        || decoded.revision.get() == 0
        || (decoded.revision.get() == 1) != decoded.previous_head.is_none()
        || decoded.pool.version().get() != 1
        || decoded.pool.id() != pool_input.id()
        || decoded.pool.unit() != pool_input.unit()
        || decoded.pool.balances().introduced() != pool_input.initial_ticks()
        || physical.available != pool_input.initial_ticks()
        || physical
            != (PhysicalBalances {
                available: pool_input.initial_ticks(),
                ..PhysicalBalances::ZERO
            })
    {
        return Err(OperationError::RecoveryRequired);
    }
    let head = chain_head(ChainInput {
        previous: decoded.previous_head,
        revision: decoded.revision,
        binding: request.binding(),
        operation,
        digest,
        transaction: decoded.transaction,
        pool: decoded.pool,
        movement: decoded.movement,
    })?;
    let pool_id = decoded.pool.id();
    for (column, expected) in [
        (0, digest.as_bytes()),
        (1, decoded.transaction.as_bytes()),
        (2, pool_id.as_bytes()),
        (4, head.as_bytes()),
    ] {
        let observed: Vec<u8> = row
            .try_get(column)
            .map_err(|_| OperationError::RecoveryRequired)?;
        if observed != expected {
            return Err(OperationError::RecoveryRequired);
        }
    }
    let revision: i64 = row
        .try_get(3)
        .map_err(|_| OperationError::RecoveryRequired)?;
    if revision <= 0 || revision as u64 != decoded.revision.get() || head != decoded.head {
        return Err(OperationError::RecoveryRequired);
    }
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM stock_movements WHERE actor=? AND grant_id=? AND operation_id=?",
    )
    .bind(binding.actor().participant().as_bytes().as_slice())
    .bind(binding.input().grant_id().as_bytes().as_slice())
    .bind(operation.as_bytes().as_slice())
    .fetch_one(&mut *connection)
    .await
    .map_err(|_| OperationError::RecoveryRequired)?;
    if count != i64::from(decoded.movement.is_some()) {
        return Err(OperationError::RecoveryRequired);
    }
    let movement_protected = if let Some(id) = decoded.movement {
        let protected:Vec<u8>=sqlx::query_scalar("SELECT protected FROM stock_movements WHERE movement_id=? AND actor=? AND grant_id=? AND operation_id=? AND pool_id=? AND length(protected) BETWEEN 1 AND 262144").bind(id.as_slice()).bind(binding.actor().participant().as_bytes().as_slice()).bind(binding.input().grant_id().as_bytes().as_slice()).bind(operation.as_bytes().as_slice()).bind(decoded.pool.id().as_bytes().as_slice()).fetch_optional(connection).await.map_err(|_|OperationError::RecoveryRequired)?.ok_or(OperationError::RecoveryRequired)?;
        let clear = crypto
            .open_stock_record(input, "movement", &id, &protected)
            .await?;
        let movement: Movement = clear
            .expose_secret(|bytes| super::closed_record(bytes))
            .map_err(|_| OperationError::RecoveryRequired)?;
        if movement.version != 1
            || movement.operation != operation
            || movement.digest != digest
            || movement.pool != decoded.pool.id()
            || movement.quantity != pool_input.initial_ticks()
            || movement.quantity == radroots_domain::stock::StockTicks::ZERO
        {
            return Err(OperationError::RecoveryRequired);
        }
        Some(protected)
    } else {
        if pool_input.initial_ticks() != radroots_domain::stock::StockTicks::ZERO {
            return Err(OperationError::RecoveryRequired);
        }
        None
    };
    let receipt = PoolCreationReceipt::from_committed_store(
        CommitAttempt::new(
            input.workspace_id(),
            operation,
            digest,
            Some(decoded.transaction),
        ),
        decoded.pool,
        radroots_storage::inventory::InventoryCheckpointRef::from_host_metadata(
            request.command().context().authority(),
            decoded.revision,
            Some(decoded.head),
        )?,
    )?;
    Ok(Some(Retained {
        previous_head: decoded.previous_head,
        movement: decoded.movement,
        movement_protected,
        receipt,
        digest,
        protected,
    }))
}
fn encode(value: &impl Serialize) -> Result<zeroize::Zeroizing<Vec<u8>>, OperationError> {
    let bytes = zeroize::Zeroizing::new(
        serde_json::to_vec(value).map_err(|_| OperationError::StorageFailure)?,
    );
    if bytes.len() > 16 * 1024 {
        return Err(OperationError::InvalidCommand);
    }
    Ok(bytes)
}
struct ChainInput<'a> {
    previous: Option<CheckpointDigest>,
    revision: Sequence,
    binding: &'a WorkspaceBinding,
    operation: OperationId,
    digest: CommandDigest,
    transaction: TransactionId,
    pool: PoolSnapshot,
    movement: Option<[u8; 32]>,
}
fn chain_head(value: ChainInput<'_>) -> Result<CheckpointDigest, OperationError> {
    let ChainInput {
        previous,
        revision,
        binding,
        operation,
        digest,
        transaction,
        pool,
        movement,
    } = value;
    let mut h = Sha256::new();
    h.update(b"radroots.seller_stock.inventory_chain.v1\0");
    h.update([u8::from(previous.is_some())]);
    h.update(previous.map(|v| *v.as_bytes()).unwrap_or([0; 32]));
    h.update(revision.get().to_be_bytes());
    h.update(binding.input().workspace_id().as_bytes());
    h.update(binding.actor().participant().as_bytes());
    h.update(binding.input().grant_id().as_bytes());
    h.update(operation.as_bytes());
    h.update(digest.as_bytes());
    h.update(transaction.as_bytes());
    h.update(pool.id().as_bytes());
    h.update(encode(&pool)?.as_slice());
    h.update([u8::from(movement.is_some())]);
    h.update(movement.unwrap_or([0; 32]));
    Ok(CheckpointDigest::from_bytes(h.finalize().into()))
}

/// Constant-memory witness of the exact protected ancestry checked before IO
/// enters the write transaction. The transaction rereads every bound SQL row;
/// no key-provider callback or decryption runs while it owns the SQL lock.
struct MinimumProof {
    lower: u64,
    upper: u64,
    digest: [u8; 32],
}
impl MinimumProof {
    async fn confirm(&self, connection: &mut SqliteConnection) -> Result<(), OperationError> {
        let mut hash = history_hash();
        for revision in (self.lower..=self.upper).rev() {
            let row=sqlx::query("SELECT actor,grant_id,operation_id,command_digest,transaction_id,pool_id,chain_head,CASE WHEN length(protected) BETWEEN 1 AND 262144 THEN protected END FROM stock_operations WHERE revision=?")
                .bind(revision as i64).fetch_optional(&mut *connection).await.map_err(|_|OperationError::RecoveryRequired)?.ok_or(OperationError::RecoveryRequired)?;
            hash.update(revision.to_be_bytes());
            for column in 0..7 {
                let bytes: Vec<u8> = row
                    .try_get(column)
                    .map_err(|_| OperationError::RecoveryRequired)?;
                if bytes.len() != 32 {
                    return Err(OperationError::RecoveryRequired);
                }
                hash.update(bytes);
            }
            let protected: Vec<u8> = row
                .try_get::<Option<Vec<u8>>, _>(7)
                .map_err(|_| OperationError::RecoveryRequired)?
                .ok_or(OperationError::RecoveryRequired)?;
            hash.update((protected.len() as u64).to_be_bytes());
            hash.update(protected);
        }
        let actual: [u8; 32] = hash.finalize().into();
        if actual != self.digest {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(())
    }
}
fn history_hash() -> Sha256 {
    let mut hash = Sha256::new();
    hash.update(b"radroots.seller_stock.ancestry_snapshot.v1\0");
    hash
}
async fn authenticate_minimum(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    binding: &WorkspaceBinding,
    current: InventoryCheckpoint,
    minimum: radroots_storage::inventory::InventoryCheckpointRef,
) -> Result<MinimumProof, OperationError> {
    if minimum.authority() != input.authority() {
        return Err(WorkspaceError::StaleAuthority.into());
    }
    if minimum.revision().get() > current.revision.get() {
        return Err(WorkspaceError::StaleCheckpoint.into());
    }
    let lower = minimum.revision().get().max(1);
    let upper = current.revision.get();
    let mut hash = history_hash();
    let mut expected = current.head;
    for revision in (lower..=upper).rev() {
        let id:Vec<u8>=sqlx::query_scalar("SELECT operation_id FROM stock_operations WHERE revision=? AND length(operation_id)=32")
            .bind(revision as i64).fetch_optional(&mut *connection).await.map_err(|_|OperationError::RecoveryRequired)?.ok_or(OperationError::RecoveryRequired)?;
        let operation = OperationId::from_bytes(
            id.try_into()
                .map_err(|_| OperationError::RecoveryRequired)?,
        );
        let retained = read_operation(connection, input, crypto, binding, operation)
            .await?
            .ok_or(OperationError::RecoveryRequired)?;
        let receipt = &retained.receipt;
        if receipt.checkpoint().revision().get() != revision
            || receipt.checkpoint().head() != expected
        {
            return Err(WorkspaceError::StaleCheckpoint.into());
        }
        if revision == minimum.revision().get() && expected != minimum.head() {
            return Err(WorkspaceError::StaleCheckpoint.into());
        }
        hash.update(revision.to_be_bytes());
        hash.update(binding.actor().participant().as_bytes());
        hash.update(binding.input().grant_id().as_bytes());
        hash.update(operation.as_bytes());
        hash.update(retained.digest.as_bytes());
        hash.update(
            receipt
                .transaction_id()
                .ok_or(OperationError::RecoveryRequired)?
                .as_bytes(),
        );
        hash.update(receipt.pool().id().as_bytes());
        hash.update(expected.ok_or(OperationError::RecoveryRequired)?.as_bytes());
        hash.update((retained.protected.len() as u64).to_be_bytes());
        hash.update(&retained.protected);
        expected = retained.previous_head;
    }
    if minimum.revision().get() == 0 && expected.is_some() {
        return Err(WorkspaceError::StaleCheckpoint.into());
    }
    Ok(MinimumProof {
        lower,
        upper,
        digest: hash.finalize().into(),
    })
}
