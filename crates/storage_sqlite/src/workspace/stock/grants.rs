//! Protected grant-set updates on the existing stock connection owner.
use super::*;
use radroots_domain::{CheckpointDigest, PoolId};
use radroots_storage::command::OperationError;
use sha2::{Digest, Sha256};
use sqlx::Row;

pub(super) enum StockRecord {
    Legacy(GrantRecord),
    Scoped(StockGrantRecord),
}
impl StockRecord {
    pub(super) fn authenticate(
        &self,
        input: &AttachmentInput,
        proof: &AttachmentProof,
        now: radroots_domain::UnixSeconds,
    ) -> Result<WorkspaceBinding, WorkspaceError> {
        match self {
            Self::Legacy(v) => v.authenticate(input, proof, now),
            Self::Scoped(v) => v.authenticate(input, proof, now),
        }
    }
    pub(super) fn authenticate_retained_capture(
        &self,
        input: &AttachmentInput,
        proof: &AttachmentProof,
    ) -> Result<(), WorkspaceError> {
        match self {
            Self::Legacy(v) => v.authenticate_retained_capture(input, proof),
            Self::Scoped(v) => v.authenticate_retained_capture(input, proof),
        }
    }
    pub(super) fn revoke(&mut self) -> Result<(), WorkspaceError> {
        match self {
            Self::Legacy(v) => v.revoke(),
            Self::Scoped(v) => v.revoke(),
        }
    }
    pub(super) fn encode(&self) -> Result<zeroize::Zeroizing<Vec<u8>>, WorkspaceError> {
        match self {
            Self::Legacy(v) => v.encode(),
            Self::Scoped(v) => v.encode(),
        }
    }
    pub(super) fn allows_action(&self, action: StockAction) -> bool {
        match self {
            Self::Legacy(_) => true,
            Self::Scoped(v) => v.permissions().allows_action(action),
        }
    }
    pub(super) fn require(&self, action: StockAction, pool: PoolId) -> Result<(), OperationError> {
        if match self {
            Self::Legacy(_) => true,
            Self::Scoped(v) => v.permissions().allows(action, pool),
        } {
            Ok(())
        } else {
            Err(OperationError::Unauthorized)
        }
    }
    pub(super) fn administrator(&self) -> bool {
        matches!(self, Self::Scoped(v) if v.administrator())
    }
}

fn state_hash() -> Sha256 {
    let mut h = Sha256::new();
    h.update(b"radroots.seller_stock.grant_set.v1\0");
    h
}
fn hash_row(hash: &mut Sha256, id: &[u8], protected: &[u8]) {
    hash.update(id);
    hash.update((protected.len() as u64).to_be_bytes());
    hash.update(protected);
}
pub(super) fn single_state(
    id: GrantId,
    protected: &[u8],
) -> Result<StockAuthorizationRef, WorkspaceError> {
    let mut hash = state_hash();
    hash_row(&mut hash, id.as_bytes(), protected);
    StockAuthorizationRef::from_host_metadata(
        Sequence::new(1).map_err(|_| WorkspaceError::Capacity)?,
        CheckpointDigest::from_bytes(hash.finalize().into()),
    )
}
async fn snapshot(
    connection: &mut SqliteConnection,
) -> Result<Vec<(GrantId, Vec<u8>)>, WorkspaceError> {
    // The count is checked before materializing bounded rows. Revoked grants
    // remain retained; exhaustion requires an explicit successor contract.
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM stock_grants")
        .fetch_one(&mut *connection)
        .await
        .map_err(|_| WorkspaceError::RecoveryRequired)?;
    if !(1..=64).contains(&count) {
        return Err(WorkspaceError::Capacity);
    }
    let rows=sqlx::query("SELECT grant_id, CASE WHEN length(protected) BETWEEN 1 AND 262144 THEN protected END FROM stock_grants ORDER BY grant_id LIMIT 65").fetch_all(connection).await.map_err(|_|WorkspaceError::RecoveryRequired)?;
    if rows.len() != count as usize {
        return Err(WorkspaceError::RecoveryRequired);
    }
    rows.into_iter()
        .map(|row| {
            let id: Vec<u8> = row
                .try_get(0)
                .map_err(|_| WorkspaceError::RecoveryRequired)?;
            let protected: Vec<u8> = row
                .try_get::<Option<Vec<u8>>, _>(1)
                .map_err(|_| WorkspaceError::RecoveryRequired)?
                .ok_or(WorkspaceError::RecoveryRequired)?;
            Ok((
                GrantId::from_bytes(
                    id.try_into()
                        .map_err(|_| WorkspaceError::RecoveryRequired)?,
                ),
                protected,
            ))
        })
        .collect()
}
fn state(
    generation: Sequence,
    rows: &[(GrantId, Vec<u8>)],
) -> Result<StockAuthorizationRef, WorkspaceError> {
    let mut hash = state_hash();
    for (id, protected) in rows {
        hash_row(&mut hash, id.as_bytes(), protected);
    }
    StockAuthorizationRef::from_host_metadata(
        generation,
        CheckpointDigest::from_bytes(hash.finalize().into()),
    )
}
pub(super) async fn check_state(
    connection: &mut SqliteConnection,
    expected: StockAuthorizationRef,
) -> Result<(), WorkspaceError> {
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM stock_grant_changes")
        .fetch_one(&mut *connection)
        .await
        .map_err(|_| WorkspaceError::RecoveryRequired)?;
    let maximum: i64 =
        sqlx::query_scalar("SELECT coalesce(max(revision),1) FROM stock_grant_changes")
            .fetch_one(&mut *connection)
            .await
            .map_err(|_| WorkspaceError::RecoveryRequired)?;
    if count != expected.generation().get() as i64 - 1
        || maximum != expected.generation().get() as i64
    {
        return Err(WorkspaceError::RecoveryRequired);
    }
    if state(expected.generation(), &snapshot(connection).await?)? != expected {
        return Err(WorkspaceError::RecoveryRequired);
    }
    Ok(())
}

/// Prepared before IO; the original credential and transaction ID survive a
/// lost observer. This value creates no grant until an administrator commits it.
pub struct StockGrantEnrollment {
    input: AttachmentInput,
    proof: AttachmentProof,
    record: StockGrantRecord,
    transaction: TransactionId,
}
impl StockGrantEnrollment {
    pub fn prepare(
        namespace: &SellerAuthoritySelection,
        principal: String,
        expires_at: radroots_domain::UnixSeconds,
        permissions: StockGrantScope,
    ) -> Result<Self, WorkspaceError> {
        let input = namespace.attachment(GrantId::from_bytes(super::super::random()?))?;
        let secret = zeroize::Zeroizing::new(super::super::random()?);
        let proof = AttachmentProof::from_protected_input(secret.as_ref())?;
        let record =
            StockGrantRecord::new(input.clone(), principal, expires_at, permissions, &proof)?;
        Ok(Self {
            input,
            proof,
            record,
            transaction: TransactionId::from_bytes(super::super::random()?),
        })
    }
    pub fn input(&self) -> &AttachmentInput {
        &self.input
    }
    pub fn proof(&self) -> &AttachmentProof {
        &self.proof
    }
    pub const fn transaction_id(&self) -> TransactionId {
        self.transaction
    }
}
impl std::fmt::Debug for StockGrantEnrollment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("StockGrantEnrollment(<redacted>)")
    }
}

/// An actual confirmed grant-set mutation, suitable for independent retention.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StockGrantReceipt {
    pub(super) transaction: TransactionId,
    pub(super) grant: GrantId,
    pub(super) authorization: StockAuthorizationRef,
}
impl StockGrantReceipt {
    pub const fn transaction_id(&self) -> TransactionId {
        self.transaction
    }
    pub const fn grant_id(&self) -> GrantId {
        self.grant
    }
    pub const fn authorization(&self) -> StockAuthorizationRef {
        self.authorization
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StockGrantOutcome {
    Committed(StockGrantReceipt),
    Unknown(TransactionId),
}

pub(in crate::workspace) struct Change {
    pub caller: WorkspaceBinding,
    pub minimum: radroots_storage::inventory::InventoryCheckpointRef,
    pub transaction: TransactionId,
    pub target: GrantId,
    pub addition: Option<zeroize::Zeroizing<Vec<u8>>>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChangeWire {
    version: u8,
    transaction: TransactionId,
    target: GrantId,
    caller: GrantId,
    digest: CheckpointDigest,
    prior_authorization: StockAuthorizationRef,
    authorization: StockAuthorizationRef,
}
fn change_digest(change: &Change) -> Result<CheckpointDigest, WorkspaceError> {
    let mut hash = Sha256::new();
    hash.update(b"radroots.seller_stock.grant_change.v1\0");
    let namespace = SellerAuthoritySelection::from_attachment(change.caller.input());
    let clear = zeroize::Zeroizing::new(
        serde_json::to_vec(&namespace).map_err(|_| WorkspaceError::InvalidInput)?,
    );
    hash.update(clear.as_slice());
    hash.update(change.caller.input().grant_id().as_bytes());
    hash.update(change.transaction.as_bytes());
    hash.update(change.target.as_bytes());
    hash.update([u8::from(change.addition.is_some())]);
    if let Some(clear) = &change.addition {
        hash.update(clear.as_slice());
    }
    Ok(CheckpointDigest::from_bytes(hash.finalize().into()))
}

impl NativeWorkspace {
    pub async fn enroll_stock_grant(
        &self,
        caller: WorkspaceBinding,
        minimum: radroots_storage::inventory::InventoryCheckpointRef,
        proof: &AttachmentProof,
        enrollment: &StockGrantEnrollment,
    ) -> Result<StockGrantOutcome, WorkspaceError> {
        let addition = enrollment.record.encode()?;
        let proof = proof.with_secret(|bytes| AttachmentProof::from_protected_input(bytes))?;
        self.worker
            .change_stock_grant(
                Change {
                    caller,
                    minimum,
                    transaction: enrollment.transaction,
                    target: enrollment.input.grant_id(),
                    addition: Some(addition),
                },
                proof,
            )
            .await
    }
    /// The caller prepares and retains this transaction ID before submission.
    pub async fn revoke_stock_grant(
        &self,
        caller: WorkspaceBinding,
        minimum: radroots_storage::inventory::InventoryCheckpointRef,
        proof: &AttachmentProof,
        transaction: TransactionId,
        target: GrantId,
    ) -> Result<StockGrantOutcome, WorkspaceError> {
        let proof = proof.with_secret(|bytes| AttachmentProof::from_protected_input(bytes))?;
        self.worker
            .change_stock_grant(
                Change {
                    caller,
                    minimum,
                    transaction,
                    target,
                    addition: None,
                },
                proof,
            )
            .await
    }
}

pub(in crate::workspace) async fn change(
    turn: &mut inventory::Turn<'_>,
    change: Change,
    proof: AttachmentProof,
) -> Result<StockGrantOutcome, WorkspaceError> {
    if *turn.uncertain {
        return Err(WorkspaceError::RecoveryRequired);
    }
    turn.namespace.validate()?;
    if !SellerAuthoritySelection::from_attachment(turn.selected)
        .matches_attachment(change.caller.input())
    {
        return Err(WorkspaceError::WrongNamespace);
    }
    let (mut authority, record, old_root, old_grant) =
        super::read(turn.connection, change.caller.input(), turn.crypto).await?;
    if !(4..=15).contains(&authority.catalog) {
        return Err(WorkspaceError::Unauthorized);
    }
    let catalog = authority.catalog;
    let current = record.authenticate(change.caller.input(), &proof, turn.clock.now()?)?;
    if current.observed_at() < change.caller.observed_at() {
        return Err(WorkspaceError::Unavailable);
    }
    radroots_storage::command::protected::same_binding(&change.caller, &current)
        .map_err(|_| WorkspaceError::Unauthorized)?;
    if !record.administrator() {
        return Err(WorkspaceError::Unauthorized);
    }
    let authorization = authority
        .authorization
        .ok_or(WorkspaceError::RecoveryRequired)?;
    // Existing protected ancestry is checked outside the SQL lock and its exact
    // row witness is rechecked inside it, alongside the grant-set fingerprint.
    let ancestry = inventory::grant_minimum(
        turn.connection,
        turn.selected,
        turn.crypto,
        &current,
        &authority,
        change.minimum,
    )
    .await?;
    let digest = change_digest(&change)?;
    let existing:Option<Vec<u8>>=sqlx::query_scalar("SELECT protected FROM stock_grant_changes WHERE transaction_id=? AND length(protected) BETWEEN 1 AND 262144").bind(change.transaction.as_bytes().as_slice()).fetch_optional(&mut *turn.connection).await.map_err(|_|WorkspaceError::RecoveryRequired)?;
    if let Some(protected) = existing {
        let clear = turn
            .crypto
            .open_stock_record(
                turn.selected,
                "grant_change",
                change.transaction.as_bytes(),
                &protected,
            )
            .await?;
        let retained: ChangeWire = clear.expose_secret(|bytes| super::closed_record(bytes))?;
        if retained.version != 1
            || retained.transaction != change.transaction
            || retained.target != change.target
            || retained.caller != current.input().grant_id()
            || retained.digest != digest
            || (change.minimum.authorization() != Some(authorization)
                && change.minimum.authorization() != Some(retained.prior_authorization))
            || retained.authorization.generation().get() > authorization.generation().get()
        {
            return Err(WorkspaceError::InvalidInput);
        }
        let mut tx = turn
            .connection
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(|_| WorkspaceError::StorageFailure)?;
        let checked: Result<(), WorkspaceError> = async {
            super::unchanged(&mut tx, current.input(), catalog, &old_root, &old_grant).await?;
            check_state(&mut tx, authorization).await?;
            ancestry
                .confirm(&mut tx)
                .await
                .map_err(|_| WorkspaceError::RecoveryRequired)?;
            let checked = record.authenticate(current.input(), &proof, turn.clock.now()?)?;
            if checked.observed_at() < current.observed_at() {
                return Err(WorkspaceError::Unavailable);
            }
            let matched: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM stock_grant_changes WHERE transaction_id=? AND protected=?",
            )
            .bind(change.transaction.as_bytes().as_slice())
            .bind(&protected)
            .fetch_one(&mut *tx)
            .await
            .map_err(|_| WorkspaceError::StorageFailure)?;
            if matched != 1 {
                return Err(WorkspaceError::RecoveryRequired);
            }
            Ok(())
        }
        .await;
        if tx.rollback().await.is_err() {
            *turn.uncertain = true;
            return Ok(StockGrantOutcome::Unknown(change.transaction));
        }
        checked?;
        return Ok(StockGrantOutcome::Committed(StockGrantReceipt {
            transaction: retained.transaction,
            grant: retained.target,
            authorization: retained.authorization,
        }));
    }
    if change.minimum.authorization() != Some(authorization) {
        return Err(WorkspaceError::StaleCheckpoint);
    }
    let mut rows = snapshot(turn.connection).await?;
    let position = rows.binary_search_by_key(&change.target, |v| v.0);
    let protected = if let Some(clear) = &change.addition {
        if position.is_ok() {
            return Err(WorkspaceError::InvalidInput);
        }
        if rows.len() >= 64 {
            return Err(WorkspaceError::Capacity);
        }
        let new = StockGrantRecord::decode(clear)?;
        if new.input().grant_id() != change.target || new.expires_at() <= turn.clock.now()? {
            return Err(WorkspaceError::InvalidInput);
        }
        if new.administrator()
            || !new.belongs_to(&SellerAuthoritySelection::from_attachment(turn.selected))?
        {
            return Err(WorkspaceError::Unauthorized);
        }
        turn.crypto
            .seal_stock_record(turn.selected, "grant", change.target.as_bytes(), clear)
            .await?
    } else {
        let index = position.map_err(|_| WorkspaceError::WrongNamespace)?;
        let input =
            SellerAuthoritySelection::from_attachment(turn.selected).attachment(change.target)?;
        let clear = turn
            .crypto
            .open_stock_record(&input, "grant", change.target.as_bytes(), &rows[index].1)
            .await?;
        let mut target = clear.expose_secret(StockGrantRecord::decode)?;
        target.revoke()?;
        turn.crypto
            .seal_stock_record(&input, "grant", change.target.as_bytes(), &target.encode()?)
            .await?
    };
    match position {
        Ok(index) => rows[index].1 = protected.clone(),
        Err(index) => rows.insert(index, (change.target, protected.clone())),
    }
    authority.revocation = Sequence::new(
        authority
            .revocation
            .get()
            .checked_add(1)
            .ok_or(WorkspaceError::Capacity)?,
    )
    .map_err(|_| WorkspaceError::Capacity)?;
    let next = state(authority.revocation, &rows)?;
    authority.authorization = Some(next);
    let root = turn
        .crypto
        .seal_stock_root(turn.selected, catalog, &authority.encode()?)
        .await?;
    let retained = ChangeWire {
        version: 1,
        transaction: change.transaction,
        target: change.target,
        caller: current.input().grant_id(),
        digest,
        prior_authorization: authorization,
        authorization: next,
    };
    let clear = zeroize::Zeroizing::new(
        serde_json::to_vec(&retained).map_err(|_| WorkspaceError::StorageFailure)?,
    );
    let receipt = turn
        .crypto
        .seal_stock_record(
            turn.selected,
            "grant_change",
            change.transaction.as_bytes(),
            &clear,
        )
        .await?;
    let mut tx = turn
        .connection
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|_| WorkspaceError::StorageFailure)?;
    let changed: Result<(), WorkspaceError> = async {
        turn.namespace.validate()?;
        super::unchanged(&mut tx, current.input(), catalog, &old_root, &old_grant).await?;
        check_state(&mut tx, authorization).await?;
        ancestry
            .confirm(&mut tx)
            .await
            .map_err(|_| WorkspaceError::RecoveryRequired)?;
        let now = turn.clock.now()?;
        record.authenticate(current.input(), &proof, now)?;
        if now < current.observed_at() {
            return Err(WorkspaceError::Unavailable);
        }
        if let Some(clear) = &change.addition
            && StockGrantRecord::decode(clear)?.expires_at() <= now
        {
            return Err(WorkspaceError::GrantExpired);
        }
        if position.is_ok() {
            let result = sqlx::query("UPDATE stock_grants SET protected=? WHERE grant_id=?")
                .bind(&protected)
                .bind(change.target.as_bytes().as_slice())
                .execute(&mut *tx)
                .await
                .map_err(|_| WorkspaceError::StorageFailure)?;
            if result.rows_affected() != 1 {
                return Err(WorkspaceError::RecoveryRequired);
            }
        } else {
            sqlx::query("INSERT INTO stock_grants(grant_id,protected) VALUES(?,?)")
                .bind(change.target.as_bytes().as_slice())
                .bind(&protected)
                .execute(&mut *tx)
                .await
                .map_err(|_| WorkspaceError::StorageFailure)?;
        }
        sqlx::query(
            "INSERT INTO stock_grant_changes(transaction_id,revision,protected) VALUES(?,?,?)",
        )
        .bind(change.transaction.as_bytes().as_slice())
        .bind(next.generation().get() as i64)
        .bind(receipt)
        .execute(&mut *tx)
        .await
        .map_err(|_| WorkspaceError::StorageFailure)?;
        let result =
            sqlx::query("UPDATE stock_authority SET protected=? WHERE singleton=1 AND protected=?")
                .bind(root)
                .bind(&old_root)
                .execute(&mut *tx)
                .await
                .map_err(|_| WorkspaceError::StorageFailure)?;
        if result.rows_affected() != 1 {
            return Err(WorkspaceError::RecoveryRequired);
        }
        turn.namespace.validate()?;
        Ok(())
    }
    .await;
    if let Err(error) = changed {
        if tx.rollback().await.is_err() {
            *turn.uncertain = true;
            return Ok(StockGrantOutcome::Unknown(change.transaction));
        }
        return Err(error);
    }
    if tx.commit().await.is_err() {
        *turn.uncertain = true;
        return Ok(StockGrantOutcome::Unknown(change.transaction));
    }
    Ok(StockGrantOutcome::Committed(StockGrantReceipt {
        transaction: change.transaction,
        grant: change.target,
        authorization: next,
    }))
}
