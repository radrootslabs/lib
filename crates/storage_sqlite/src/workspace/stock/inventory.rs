//! Owner-private inventory effects on the existing connection owner.
pub(in crate::workspace) mod acceptance;
pub(super) mod checkpoint;
mod codec;
mod mapping;
mod plan;
use super::*;
use checkpoint::{MinimumProof, authenticate_minimum, authenticate_projection};
pub(super) async fn grant_minimum(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    binding: &WorkspaceBinding,
    authority: &Authority,
    minimum: radroots_storage::inventory::InventoryCheckpointRef,
) -> Result<MinimumProof, WorkspaceError> {
    authenticate_minimum(connection, input, crypto, binding, authority, minimum)
        .await
        .map_err(|_| WorkspaceError::RecoveryRequired)
}
use codec::{Movement, MovementKind, OperationWire, decode_operation, encode_operation};
use plan::{plan_effect, record_version};
use radroots_domain::{
    CheckpointDigest, CommandDigest, OperationId, PoolId, UnixSeconds,
    stock::{PhysicalBalances, PoolBalances, PoolSnapshot},
};
use radroots_storage::{
    command::{CommitAttempt, OperationError, protected::same_binding},
    inventory::{InventoryOutcome, InventoryReceipt, InventoryRequest},
};
use serde_json::value::RawValue;
use sha2::{Digest, Sha256};
use sqlx::Row;

pub(super) async fn check_head(
    connection: &mut SqliteConnection,
    checkpoint: InventoryCheckpoint,
) -> Result<(), WorkspaceError> {
    if let Some(expected) = checkpoint.orders {
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM stock_orders")
            .fetch_one(&mut *connection)
            .await
            .map_err(|_| WorkspaceError::RecoveryRequired)?;
        if count != expected.get() as i64 {
            return Err(WorkspaceError::RecoveryRequired);
        }
    }
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
    if let Some(expected) = checkpoint.mappings {
        let actual: i64 = sqlx::query_scalar("SELECT count(*) FROM stock_mappings")
            .fetch_one(&mut *connection)
            .await
            .map_err(|_| WorkspaceError::RecoveryRequired)?;
        if actual != expected.get() as i64 {
            return Err(WorkspaceError::RecoveryRequired);
        }
    }
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
pub(super) struct Access {
    pub(super) authority: Authority,
    pub(super) record: StockRecord,
    pub(super) root: Vec<u8>,
    pub(super) grant: Vec<u8>,
    pub(super) current: WorkspaceBinding,
    pub(super) minimum_proof: MinimumProof,
    identity_proof: Option<MinimumProof>,
    projection_proof: Option<MinimumProof>,
    mapping_read: Option<mapping::MappingRead>,
}
pub(in crate::workspace) struct Turn<'a> {
    pub connection: &'a mut SqliteConnection,
    pub namespace: &'a Namespace,
    pub selected: &'a AttachmentInput,
    pub crypto: &'a WorkspaceCrypto,
    pub clock: &'a dyn WorkspaceClock,
    pub uncertain: &'a mut bool,
}
impl Access {
    /// Rechecks authenticated store state inside the caller's actual SQL
    /// transaction. Family-specific permissions and deadlines remain separate.
    pub(super) async fn confirm(
        &self,
        connection: &mut SqliteConnection,
        namespace: &Namespace,
        clock: &dyn WorkspaceClock,
        proof: &AttachmentProof,
    ) -> Result<WorkspaceBinding, OperationError> {
        namespace.validate()?;
        super::unchanged(
            connection,
            self.current.input(),
            self.authority.catalog,
            &self.root,
            &self.grant,
        )
        .await?;
        if let Some(state) = self.authority.authorization {
            grants::check_state(connection, state).await?;
        }
        check_head(
            connection,
            self.authority
                .inventory
                .ok_or(OperationError::UnsupportedOperation)?,
        )
        .await?;
        self.minimum_proof.confirm(connection).await?;
        if let Some(identity) = &self.identity_proof {
            identity.confirm(connection).await?;
        }
        let current = self
            .record
            .authenticate(self.current.input(), proof, clock.now()?)?;
        same_binding(&self.current, &current)?;
        if current.observed_at() < self.current.observed_at() {
            return Err(WorkspaceError::Unavailable.into());
        }
        namespace.validate()?;
        Ok(current)
    }
}
impl Turn<'_> {
    pub(super) async fn access(
        &mut self,
        binding: &WorkspaceBinding,
        minimum: radroots_storage::inventory::InventoryCheckpointRef,
        proof: &AttachmentProof,
    ) -> Result<Access, OperationError> {
        if *self.uncertain {
            return Err(OperationError::RecoveryRequired);
        }
        self.namespace.validate()?;
        if !SellerAuthoritySelection::from_attachment(self.selected)
            .matches_attachment(binding.input())
        {
            return Err(OperationError::Unauthorized);
        }
        let (authority, record, root, grant) =
            super::read(self.connection, binding.input(), self.crypto).await?;
        let current = record.authenticate(binding.input(), proof, self.clock.now()?)?;
        if current.scope() != WorkspaceScope::SellerStockOwner {
            return Err(OperationError::Unauthorized);
        }
        same_binding(binding, &current)?;
        if current.observed_at() < binding.observed_at() {
            return Err(WorkspaceError::Unavailable.into());
        }
        if authority.authorization != minimum.authorization() {
            return Err(WorkspaceError::StaleCheckpoint.into());
        }
        let current = if let Some(state) = authority.authorization {
            current.with_stock_authorization(state)?
        } else {
            current
        };
        if authority.inventory.is_none() {
            return Err(OperationError::UnsupportedOperation);
        }

        let minimum_proof = authenticate_minimum(
            self.connection,
            self.selected,
            self.crypto,
            &current,
            &authority,
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
            identity_proof: None,
            projection_proof: None,
            mapping_read: None,
        })
    }
    pub async fn create(
        &mut self,
        request: InventoryRequest,
        proof: AttachmentProof,
    ) -> Result<InventoryOutcome, OperationError> {
        let mut access = self
            .access(request.binding(), request.minimum_checkpoint(), &proof)
            .await?;
        access
            .record
            .require(StockAction::ReadOperations, request.command().pool_id())?;
        if let Some(input) = request.command().mapping_input() {
            mapping::require(&access, input.key())?;
        }
        let context = request.command().context();
        let operation = context.operation_id();
        let digest = request.command().digest();
        // Retained identity precedes every fresh deadline or pool-existence check.
        let retained = read_operation(
            self.connection,
            self.selected,
            self.crypto,
            request.binding(),
            operation,
        )
        .await?;
        access.identity_proof = Some(
            checkpoint::authenticate_identity(
                self.connection,
                self.selected,
                self.crypto,
                &access.current,
                &access.authority,
                checkpoint::IdentityTarget::Operation(
                    operation,
                    retained.as_ref().map(Retained::identity).transpose()?,
                ),
            )
            .await?,
        );
        if let Some(retained) = retained {
            if retained.digest != digest {
                return Err(OperationError::IdempotencyConflict);
            }
            self.confirm(&access, &proof, Some(&retained), None, None, None)
                .await?;
            return Ok(InventoryOutcome::Committed(Box::new(retained.receipt)));
        }
        if request.replay_only() {
            return Err(OperationError::RecoveryRequired);
        }
        if request.command().mapping_input().is_none() {
            access.record.require(
                match request.command().kind() {
                    radroots_storage::inventory::InventoryCommandKind::MapOption => {
                        return Err(OperationError::RecoveryRequired);
                    }
                    radroots_storage::inventory::InventoryCommandKind::CreatePool => {
                        StockAction::CreatePool
                    }
                    radroots_storage::inventory::InventoryCommandKind::AddStock => {
                        StockAction::AddStock
                    }
                    radroots_storage::inventory::InventoryCommandKind::RemoveStock => {
                        StockAction::RemoveStock
                    }
                    radroots_storage::inventory::InventoryCommandKind::SpoilStock => {
                        StockAction::SpoilStock
                    }
                },
                request.command().pool_id(),
            )?;
        }
        // An original captured command may be replayed under a replacement grant,
        // but a new effect must have been captured under this current grant.
        if request.command().context().actor() != access.current.actor() {
            return Err(OperationError::Unauthorized);
        }
        if access.current.observed_at() < request.binding().observed_at() {
            return Err(WorkspaceError::Unavailable.into());
        }
        if access.current.observed_at() >= context.not_after() {
            return Err(OperationError::DeadlineExceeded);
        }
        let catalog = access.authority.version();
        if catalog < 3 && request.command().correction_input().is_some() {
            return Err(OperationError::UnsupportedOperation);
        }
        let existing = read_pool(
            self.connection,
            self.selected,
            self.crypto,
            request.command().pool_id(),
        )
        .await?;
        let before = existing.as_ref().map(|v| v.pool);
        access.projection_proof = Some(
            authenticate_projection(
                self.connection,
                self.selected,
                self.crypto,
                &access.current,
                &access.authority,
                request.command().pool_id(),
                before,
            )
            .await?,
        );
        let (pool, quantity, movement_kind) = plan_effect(request.command(), before)?;
        let mapping = mapping::prepare(self, &access, &request, pool).await?;
        let label = if let Some(input) = request.command().creation_input() {
            input.label()
        } else {
            existing
                .as_ref()
                .ok_or(OperationError::StaleState)?
                .view
                .label()
        };
        let transaction = TransactionId::from_bytes(super::super::random()?);
        let movement = if quantity == radroots_domain::stock::StockTicks::ZERO {
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
        let receipt_checkpoint =
            radroots_storage::inventory::InventoryCheckpointRef::from_host_metadata(
                self.selected.authority(),
                revision,
                Some(head),
            )?
            .with_optional_authorization(access.authority.authorization);
        let receipt = if let Some(mapping) = &mapping {
            InventoryReceipt::from_committed_mapping(
                attempt,
                pool,
                mapping.effect.after,
                receipt_checkpoint,
            )?
        } else {
            InventoryReceipt::from_committed_store(
                attempt,
                request.command().kind(),
                pool,
                receipt_checkpoint,
            )?
        };
        let protected_pool = if mapping.is_some() {
            None
        } else {
            Some(
                self.crypto
                    .seal_stock_record(
                        self.selected,
                        "pool",
                        pool.id().as_bytes(),
                        &encode(&PoolWire {
                            version: 1,
                            label,
                            pool,
                        })?,
                    )
                    .await?,
            )
        };
        let protected_movement = if let Some(id) = movement {
            Some(
                self.crypto
                    .seal_stock_record_for_catalog(
                        self.selected,
                        "movement",
                        &id,
                        catalog,
                        &encode(&Movement {
                            version: record_version(catalog)?,
                            operation,
                            digest,
                            pool: pool.id(),
                            quantity,
                            kind: movement_kind,
                        })?,
                    )
                    .await?,
            )
        } else {
            None
        };
        let command: &RawValue = serde_json::from_slice(request.command().protected_wire())
            .map_err(|_| OperationError::InvalidCommand)?;
        let operation_body = encode_operation(
            &OperationWire {
                version: record_version(catalog)?,
                before: if catalog >= 3 { Some(before) } else { None },
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
            },
            catalog,
            access.authority.authorization,
            mapping.as_ref().map(|value| value.effect),
        )?;
        let framed = if matches!(catalog, 7..=10) {
            Some(
                super::ledger::prepare(
                    self.crypto,
                    self.selected,
                    super::ledger::Header {
                        version: 5,
                        family: super::ledger::Family::Inventory,
                        namespace: SellerAuthoritySelection::from_attachment(self.selected),
                        actor: request.binding().actor(),
                        operation,
                        digest,
                        transaction,
                        revision,
                        previous: checkpoint.head,
                        head,
                        pool: Some(pool.id()),
                        order: None,
                        related_operation: None,
                        subject_grant: None,
                        bytes: 0,
                        parts: 0,
                        body_digest: [0; 32],
                    },
                    &operation_body,
                )
                .await?,
            )
        } else {
            None
        };
        let protected_operation = if let Some(framed) = &framed {
            framed.protected.clone()
        } else {
            self.crypto
                .seal_stock_record_for_catalog(
                    self.selected,
                    "operation",
                    operation.as_bytes(),
                    catalog,
                    &operation_body,
                )
                .await?
        };
        access.authority.inventory = Some(InventoryCheckpoint {
            orders: checkpoint.orders,
            mappings: checkpoint
                .mappings
                .map(|count| {
                    Sequence::new(
                        count
                            .get()
                            .checked_add(u64::from(
                                mapping
                                    .as_ref()
                                    .is_some_and(|value| value.effect.before.is_none()),
                            ))
                            .ok_or(WorkspaceError::Capacity)?,
                    )
                    .map_err(|_| WorkspaceError::Capacity)
                })
                .transpose()?,
            revision,
            head: Some(head),
            pools: Sequence::new(
                checkpoint
                    .pools
                    .get()
                    .checked_add(u64::from(before.is_none()))
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
            .seal_stock_root(self.selected, catalog, &access.authority.encode()?)
            .await?;
        self.namespace.validate()?;
        let mut tx = self
            .connection
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(|_| OperationError::StorageFailure)?;
        let changed:Result<(),OperationError>=async {
            super::unchanged(&mut tx,access.current.input(),access.authority.version(),&access.root,&access.grant).await?;
            if let Some(state)=access.authority.authorization { grants::check_state(&mut tx,state).await?; }
            check_head(&mut tx,checkpoint).await?;
            access.minimum_proof.confirm(&mut tx).await?;
            if let Some(identity)=&access.identity_proof {identity.confirm(&mut tx).await?;}
            if let Some(proof)=&access.projection_proof {proof.confirm(&mut tx).await?;}
            let current=access.record.authenticate(request.binding().input(),&proof,self.clock.now()?)?;
            same_binding(request.binding(),&current)?;
            if current.observed_at()<access.current.observed_at() {return Err(WorkspaceError::Unavailable.into());}
            if current.observed_at()>=context.not_after() {return Err(OperationError::DeadlineExceeded);}
            self.namespace.validate()?;
            let absent:i64=sqlx::query_scalar("SELECT count(*) FROM stock_operations WHERE actor=? AND operation_id=?").bind(request.binding().actor().participant().as_bytes().as_slice()).bind(operation.as_bytes().as_slice()).fetch_one(&mut *tx).await.map_err(|_|OperationError::StorageFailure)?;
            if absent!=0 {return Err(OperationError::StaleState);}
            if let Some(mapping)=&mapping {
                let original=existing.as_ref().ok_or(OperationError::RecoveryRequired)?;
                let count:i64=sqlx::query_scalar("SELECT count(*) FROM stock_pools WHERE pool_id=? AND revision=? AND protected=?").bind(pool.id().as_bytes().as_slice()).bind(pool.version().get() as i64).bind(&original.protected).fetch_one(&mut *tx).await.map_err(|_|OperationError::RecoveryRequired)?;
                if count!=1 {return Err(OperationError::RecoveryRequired);}
                mapping.apply(&mut tx).await?;
            } else if let Some(before)=before {
                let original=existing.as_ref().ok_or(OperationError::RecoveryRequired)?;
                let updated=sqlx::query("UPDATE stock_pools SET revision=?,protected=? WHERE pool_id=? AND revision=? AND protected=?")
                    .bind(pool.version().get() as i64).bind(protected_pool).bind(pool.id().as_bytes().as_slice()).bind(before.version().get() as i64).bind(&original.protected).execute(&mut *tx).await.map_err(|_|OperationError::StorageFailure)?;
                if updated.rows_affected()!=1 {return Err(OperationError::StaleState);}
            } else {
                let count:i64=sqlx::query_scalar("SELECT count(*) FROM stock_pools WHERE pool_id=?").bind(pool.id().as_bytes().as_slice()).fetch_one(&mut *tx).await.map_err(|_|OperationError::StorageFailure)?;
                if count!=0 {return Err(OperationError::StaleState);}
                sqlx::query("INSERT INTO stock_pools(pool_id,revision,protected) VALUES(?,?,?)").bind(pool.id().as_bytes().as_slice()).bind(pool.version().get() as i64).bind(protected_pool).execute(&mut *tx).await.map_err(|_|OperationError::StorageFailure)?;
            }
            sqlx::query("INSERT INTO stock_operations(actor,grant_id,operation_id,command_digest,transaction_id,pool_id,revision,chain_head,protected) VALUES(?,?,?,?,?,?,?,?,?)")
                .bind(request.binding().actor().participant().as_bytes().as_slice()).bind(request.binding().input().grant_id().as_bytes().as_slice()).bind(operation.as_bytes().as_slice()).bind(digest.as_bytes().as_slice()).bind(transaction.as_bytes().as_slice()).bind(pool.id().as_bytes().as_slice()).bind(revision.get() as i64).bind(head.as_bytes().as_slice()).bind(protected_operation).execute(&mut *tx).await.map_err(|_|OperationError::StorageFailure)?;
            if let Some(framed)=&framed {framed.insert_parts(&mut tx,request.binding().actor(),operation).await?;}
            if let (Some(id),Some(protected))=(movement,protected_movement) {
            sqlx::query("INSERT INTO stock_movements(movement_id,actor,grant_id,operation_id,pool_id,protected) VALUES(?,?,?,?,?,?)").bind(id.as_slice()).bind(request.binding().actor().participant().as_bytes().as_slice()).bind(request.binding().input().grant_id().as_bytes().as_slice()).bind(operation.as_bytes().as_slice()).bind(pool.id().as_bytes().as_slice()).bind(protected).execute(&mut *tx).await.map_err(|_|OperationError::StorageFailure)?;
            }
            let changed=sqlx::query("UPDATE stock_authority SET protected=? WHERE singleton=1 AND protected=?").bind(root).bind(&access.root).execute(&mut *tx).await.map_err(|_|OperationError::StorageFailure)?;
            if changed.rows_affected()!=1 {return Err(OperationError::RecoveryRequired);}self.namespace.validate()?;Ok(())
        }.await;
        if let Err(error) = changed {
            if tx.rollback().await.is_err() {
                *self.uncertain = true;
                return Ok(InventoryOutcome::Unknown(attempt));
            }
            return Err(error);
        }
        if tx.commit().await.is_err() {
            *self.uncertain = true;
            return Ok(InventoryOutcome::Unknown(attempt));
        }
        Ok(InventoryOutcome::Committed(Box::new(receipt)))
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
            super::unchanged(&mut tx,access.current.input(),access.authority.version(),&access.root,&access.grant).await?;
            if let Some(state)=access.authority.authorization { grants::check_state(&mut tx,state).await?; }
            check_head(&mut tx,access.authority.inventory.ok_or(OperationError::UnsupportedOperation)?).await?;
            access.minimum_proof.confirm(&mut tx).await?;
            if let Some(identity)=&access.identity_proof {identity.confirm(&mut tx).await?;}
            if let Some(proof)=&access.projection_proof {proof.confirm(&mut tx).await?;}
            if let Some(mapping)=&access.mapping_read {mapping.confirm(&mut tx).await?;}
            let current=access.record.authenticate(access.current.input(),proof,self.clock.now()?)?;
            same_binding(&access.current,&current)?;
            if current.observed_at()<access.current.observed_at() {return Err(WorkspaceError::Unavailable.into());}
            if let Some(operation)=absent_operation {let count:i64=sqlx::query_scalar("SELECT count(*) FROM stock_operations WHERE actor=? AND operation_id=?").bind(current.actor().participant().as_bytes().as_slice()).bind(operation.as_bytes().as_slice()).fetch_one(&mut *tx).await.map_err(|_|OperationError::StorageFailure)?;if count!=0 {return Err(OperationError::RecoveryRequired);}}
            if let Some(id)=absent_pool {let count:i64=sqlx::query_scalar("SELECT count(*) FROM stock_pools WHERE pool_id=?").bind(id.as_bytes().as_slice()).fetch_one(&mut *tx).await.map_err(|_|OperationError::StorageFailure)?;if count!=0 {return Err(OperationError::RecoveryRequired);}}
            if let Some(retained)=operation {
                let count:i64=sqlx::query_scalar("SELECT count(*) FROM stock_operations WHERE actor=? AND operation_id=? AND protected=?").bind(current.actor().participant().as_bytes().as_slice()).bind(retained.receipt.attempt().operation_id().as_bytes().as_slice()).bind(&retained.protected).fetch_one(&mut *tx).await.map_err(|_|OperationError::StorageFailure)?;
                if count!=1 {return Err(OperationError::RecoveryRequired);}
                if let Some((header,parts))=&retained.frame {
                    super::ledger::confirm_header(&mut tx,header,&retained.protected).await?;
                    super::ledger::confirm_parts(&mut tx,header.actor,header.operation,*parts,header.parts).await?;
                }
                let movement_count:i64=sqlx::query_scalar("SELECT count(*) FROM stock_movements WHERE actor=? AND operation_id=?").bind(current.actor().participant().as_bytes().as_slice()).bind(retained.receipt.attempt().operation_id().as_bytes().as_slice()).fetch_one(&mut *tx).await.map_err(|_|OperationError::StorageFailure)?;
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
    ) -> Result<Option<InventoryReceipt>, OperationError> {
        let mut access = self.access(&binding, minimum, &proof).await?;
        let result = read_operation(
            self.connection,
            self.selected,
            self.crypto,
            &binding,
            operation,
        )
        .await?;
        access.identity_proof = Some(
            checkpoint::authenticate_identity(
                self.connection,
                self.selected,
                self.crypto,
                &access.current,
                &access.authority,
                checkpoint::IdentityTarget::Operation(
                    operation,
                    result.as_ref().map(Retained::identity).transpose()?,
                ),
            )
            .await?,
        );
        if !access.record.allows_action(StockAction::ReadOperations) {
            return Err(OperationError::Unauthorized);
        }
        if let Some(retained) = &result {
            access
                .record
                .require(StockAction::ReadOperations, retained.receipt.pool().id())?;
            if let Some(mapping) = retained.receipt.mapping() {
                mapping::require(&access, mapping.key())?;
            }
        }
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
    pub async fn option_mapping(
        &mut self,
        binding: WorkspaceBinding,
        minimum: radroots_storage::inventory::InventoryCheckpointRef,
        key: radroots_domain::stock::OptionMappingKey,
        proof: AttachmentProof,
    ) -> Result<Option<radroots_domain::stock::OptionPoolMapping>, OperationError> {
        let mut access = self.access(&binding, minimum, &proof).await?;
        mapping::require(&access, key)?;
        let retained = mapping::read(self.connection, self.selected, self.crypto, key).await?;
        let result = retained.as_ref().map(|value| value.value);
        let witness = checkpoint::authenticate_mapping_projection(
            self.connection,
            self.selected,
            self.crypto,
            &access.current,
            &access.authority,
            key,
            result,
        )
        .await?;
        access.mapping_read = Some(mapping::MappingRead {
            key,
            retained,
            proof: witness,
        });
        self.confirm(&access, &proof, None, None, None, None)
            .await?;
        Ok(result)
    }
    pub async fn pool(
        &mut self,
        binding: WorkspaceBinding,
        minimum: radroots_storage::inventory::InventoryCheckpointRef,
        id: PoolId,
        proof: AttachmentProof,
    ) -> Result<Option<radroots_domain::stock::PoolView>, OperationError> {
        let mut access = self.access(&binding, minimum, &proof).await?;
        access.record.require(StockAction::ReadPools, id)?;
        let result = read_pool(self.connection, self.selected, self.crypto, id).await?;
        access.projection_proof = Some(
            authenticate_projection(
                self.connection,
                self.selected,
                self.crypto,
                &access.current,
                &access.authority,
                id,
                result.as_ref().map(|v| v.pool),
            )
            .await?,
        );
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
pub(super) struct Retained {
    pub(super) frame: Option<(super::ledger::Header, [u8; 32])>,
    pub(super) original_grant: GrantId,
    pub(super) previous_head: Option<CheckpointDigest>,
    pub(super) movement: Option<[u8; 32]>,
    pub(super) movement_protected: Option<Vec<u8>>,
    pub(super) receipt: InventoryReceipt,
    pub(super) digest: CommandDigest,
    pub(super) protected: Vec<u8>,
}
impl Retained {
    fn identity(&self) -> Result<checkpoint::EntryIdentity, OperationError> {
        Ok(checkpoint::EntryIdentity {
            operation: self.receipt.attempt().operation_id(),
            revision: self.receipt.checkpoint().revision(),
            head: self
                .receipt
                .checkpoint()
                .head()
                .ok_or(OperationError::RecoveryRequired)?,
            protected_digest: Sha256::digest(&self.protected).into(),
        })
    }
}
pub(super) async fn read_operation(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    binding: &WorkspaceBinding,
    operation: OperationId,
) -> Result<Option<Retained>, OperationError> {
    let row=sqlx::query("SELECT CASE WHEN length(command_digest)=32 THEN command_digest END,CASE WHEN length(transaction_id)=32 THEN transaction_id END,CASE WHEN length(pool_id)=32 THEN pool_id END,revision,CASE WHEN length(chain_head)=32 THEN chain_head END,CASE WHEN length(protected) BETWEEN 1 AND 262144 THEN protected END,grant_id FROM stock_operations WHERE actor=? AND operation_id=?")
        .bind(binding.actor().participant().as_bytes().as_slice()).bind(operation.as_bytes().as_slice()).fetch_optional(&mut *connection).await.map_err(|_|OperationError::RecoveryRequired)?;
    let Some(row) = row else { return Ok(None) };
    let protected: Vec<u8> = row
        .try_get::<Option<Vec<u8>>, _>(5)
        .map_err(|_| OperationError::RecoveryRequired)?
        .ok_or(OperationError::RecoveryRequired)?;
    let catalog: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(&mut *connection)
        .await
        .map_err(|_| OperationError::RecoveryRequired)?;
    let (frame, request, decoded) = if matches!(catalog, 7..=10) {
        let retained = super::ledger::read(connection, input, crypto, operation)
            .await?
            .ok_or(OperationError::RecoveryRequired)?;
        if retained.protected != protected {
            return Err(OperationError::RecoveryRequired);
        }
        if retained.header.family != super::ledger::Family::Inventory {
            return Err(OperationError::IdempotencyConflict);
        }
        let (request, decoded) = decode_operation(retained.body.as_slice(), catalog)?;
        if retained.header.actor != request.binding().actor()
            || retained.header.previous != decoded.previous_head
        {
            return Err(OperationError::RecoveryRequired);
        }
        (
            Some((retained.header, retained.parts_witness)),
            request,
            decoded,
        )
    } else {
        let clear = crypto
            .open_stock_record_for_catalog(
                input,
                "operation",
                operation.as_bytes(),
                catalog,
                &protected,
            )
            .await?;
        let (request, decoded) = clear.expose_secret(|bytes| decode_operation(bytes, catalog))?;
        (None, request, decoded)
    };
    if !SellerAuthoritySelection::from_attachment(request.binding().input())
        .matches_attachment(binding.input())
        || request.binding().actor().participant() != binding.actor().participant()
    {
        return Err(OperationError::RecoveryRequired);
    }
    let digest = request.command().digest();
    let original_grant: Vec<u8> = row
        .try_get(6)
        .map_err(|_| OperationError::RecoveryRequired)?;
    if original_grant != request.binding().input().grant_id().as_bytes() {
        return Err(OperationError::RecoveryRequired);
    }
    let (planned, quantity, movement_kind) = plan_effect(request.command(), decoded.before)
        .map_err(|_| OperationError::RecoveryRequired)?;
    let expected_mapping = mapping::effect(
        request.command(),
        decoded.pool,
        decoded.mapping.and_then(|value| value.before),
    )
    .map_err(|_| OperationError::RecoveryRequired)?;
    if expected_mapping != decoded.mapping || (catalog < 5 && decoded.mapping.is_some()) {
        return Err(OperationError::RecoveryRequired);
    }
    if decoded.version != record_version(catalog)?
        || request.command().context().operation_id() != operation
        || decoded.revision.get() == 0
        || (decoded.revision.get() == 1) != decoded.previous_head.is_none()
        || decoded.pool != planned
        || (catalog == 2 && decoded.before.is_some())
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
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM stock_movements WHERE actor=? AND operation_id=?")
            .bind(binding.actor().participant().as_bytes().as_slice())
            .bind(operation.as_bytes().as_slice())
            .fetch_one(&mut *connection)
            .await
            .map_err(|_| OperationError::RecoveryRequired)?;
    if count != i64::from(decoded.movement.is_some()) {
        return Err(OperationError::RecoveryRequired);
    }
    let movement_protected = if let Some(id) = decoded.movement {
        let protected:Vec<u8>=sqlx::query_scalar("SELECT protected FROM stock_movements WHERE movement_id=? AND actor=? AND grant_id=? AND operation_id=? AND pool_id=? AND length(protected) BETWEEN 1 AND 262144").bind(id.as_slice()).bind(binding.actor().participant().as_bytes().as_slice()).bind(request.binding().input().grant_id().as_bytes().as_slice()).bind(operation.as_bytes().as_slice()).bind(decoded.pool.id().as_bytes().as_slice()).fetch_optional(connection).await.map_err(|_|OperationError::RecoveryRequired)?.ok_or(OperationError::RecoveryRequired)?;
        let clear = crypto
            .open_stock_record_for_catalog(input, "movement", &id, catalog, &protected)
            .await?;
        let movement: Movement = clear
            .expose_secret(|bytes| super::closed_record(bytes))
            .map_err(|_| OperationError::RecoveryRequired)?;
        if movement.version != record_version(catalog)?
            || movement.operation != operation
            || movement.digest != digest
            || movement.pool != decoded.pool.id()
            || movement.quantity != quantity
            || movement.kind != movement_kind
            || movement.quantity == radroots_domain::stock::StockTicks::ZERO
        {
            return Err(OperationError::RecoveryRequired);
        }
        Some(protected)
    } else {
        if quantity != radroots_domain::stock::StockTicks::ZERO {
            return Err(OperationError::RecoveryRequired);
        }
        None
    };
    let attempt = CommitAttempt::new(
        input.workspace_id(),
        operation,
        digest,
        Some(decoded.transaction),
    );
    let checkpoint = radroots_storage::inventory::InventoryCheckpointRef::from_host_metadata(
        request.command().context().authority(),
        decoded.revision,
        Some(decoded.head),
    )?
    .with_optional_authorization(decoded.authorization);
    let receipt = if let Some(mapping) = decoded.mapping {
        InventoryReceipt::from_committed_mapping(attempt, decoded.pool, mapping.after, checkpoint)?
    } else {
        InventoryReceipt::from_committed_store(
            attempt,
            request.command().kind(),
            decoded.pool,
            checkpoint,
        )?
    };
    Ok(Some(Retained {
        frame,
        original_grant: request.binding().input().grant_id(),
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
