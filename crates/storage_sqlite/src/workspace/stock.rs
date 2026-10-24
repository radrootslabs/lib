//! Fresh native stock authority. No allocation or issuance is implied by enrollment.
use super::{
    AttachmentInput, AttachmentProof, NativeWorkspace, WorkspaceBinding, WorkspaceClock,
    WorkspaceCrypto, WorkspaceError, WorkspaceScope, fs::Namespace, record::GrantRecord,
};
use radroots_domain::{ExecutorId, GrantId, Sequence, TransactionId, WorkspaceId};
pub use radroots_storage::workspace::seller_stock::SellerStockConfig;
use serde::{Deserialize, Serialize};
use sqlx::{Connection, SqliteConnection};
use std::{
    path::Path,
    sync::{Arc, Mutex},
};

/// One-shot protected enrollment, retained before starting durable creation. A lost
/// observer must reopen its original selection, never silently create a replacement.
pub struct StockEnrollment {
    input: AttachmentInput,
    proof: AttachmentProof,
    record: Mutex<Option<GrantRecord>>,
    transaction: TransactionId,
}
impl StockEnrollment {
    pub fn prepare(
        config: SellerStockConfig,
        clock: &dyn WorkspaceClock,
    ) -> Result<Self, WorkspaceError> {
        let now = clock.now()?;
        config.validate_at(now)?;
        let secret = zeroize::Zeroizing::new(super::random()?);
        let proof = AttachmentProof::from_protected_input(secret.as_ref())?;
        let (input, proof, record) = config.enroll(
            now,
            WorkspaceId::from_bytes(super::random()?),
            ExecutorId::from_bytes(super::random()?),
            GrantId::from_bytes(super::random()?),
            proof,
        )?;
        Ok(Self {
            input,
            proof,
            record: Mutex::new(Some(record)),
            transaction: TransactionId::from_bytes(super::random()?),
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
impl std::fmt::Debug for StockEnrollment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("StockEnrollment(<redacted>)")
    }
}
/// A confirmed creation identity read from protected durable state. This is
/// enrollment evidence, not proof of any stock quantity or business operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StockAuthorityReceipt {
    pub(super) input: AttachmentInput,
    pub(super) transaction: TransactionId,
    pub(super) revocation: Sequence,
}
impl StockAuthorityReceipt {
    pub fn input(&self) -> &AttachmentInput {
        &self.input
    }
    pub const fn transaction_id(&self) -> TransactionId {
        self.transaction
    }
    pub const fn revocation_generation(&self) -> Sequence {
        self.revocation
    }
}
impl NativeWorkspace {
    pub async fn initialize_seller_stock(
        path: &Path,
        enrollment: &StockEnrollment,
        crypto: WorkspaceCrypto,
        clock: Arc<dyn WorkspaceClock>,
    ) -> Result<Self, WorkspaceError> {
        // Taking the sole prepared grant is irreversible, even if this observer
        // disappears. Retained selectors/proof permit reopening, never cloning authority.
        let record = {
            enrollment
                .record
                .lock()
                .map_err(|_| WorkspaceError::Unavailable)?
                .take()
                .ok_or(WorkspaceError::RecoveryRequired)?
        };
        let binding = record.authenticate(&enrollment.input, &enrollment.proof, clock.now()?)?;
        let worker = super::worker::Handle::launch_stock(
            path.to_path_buf(),
            enrollment.input.clone(),
            Some((record, enrollment.transaction, binding.expires_at())),
            crypto,
            clock,
        )
        .await?;
        Ok(Self {
            worker: Arc::new(worker),
        })
    }
    pub async fn open_seller_stock(
        path: &Path,
        expected: AttachmentInput,
        crypto: WorkspaceCrypto,
        clock: Arc<dyn WorkspaceClock>,
    ) -> Result<Self, WorkspaceError> {
        let worker =
            super::worker::Handle::launch_stock(path.to_path_buf(), expected, None, crypto, clock)
                .await?;
        Ok(Self {
            worker: Arc::new(worker),
        })
    }
    /// Query after an interrupted creation using the retained original selection.
    pub async fn stock_authority(
        &self,
        input: AttachmentInput,
        proof: &AttachmentProof,
    ) -> Result<StockAuthorityReceipt, WorkspaceError> {
        let proof = proof.with_secret(|bytes| AttachmentProof::from_protected_input(bytes))?;
        self.worker.stock_authority(input, proof).await
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Authority {
    version: u8,
    input: AttachmentInput,
    transaction: TransactionId,
    revocation: Sequence,
}
pub(super) struct Initial {
    pub grant: GrantRecord,
    pub transaction: TransactionId,
    pub expires_at: radroots_domain::UnixSeconds,
    pub clock: Arc<dyn WorkspaceClock>,
}
const AUTHORITY: &str = "CREATE TABLE stock_authority (singleton INTEGER PRIMARY KEY CHECK(singleton=1), protected BLOB NOT NULL CHECK(length(protected)>0 AND length(protected)<=262144)) STRICT";
const GRANTS: &str = "CREATE TABLE stock_grants (grant_id BLOB PRIMARY KEY CHECK(length(grant_id)=32), protected BLOB NOT NULL CHECK(length(protected)>0 AND length(protected)<=262144)) STRICT, WITHOUT ROWID";
const CATALOG: [&str; 2] = [AUTHORITY, GRANTS];
async fn catalog(connection: &mut SqliteConnection) -> Result<(), WorkspaceError> {
    let valid: i64 = sqlx::query_scalar("SELECT CASE WHEN count(*)=2 AND sum(CASE WHEN type='table' AND name='stock_authority' AND tbl_name='stock_authority' AND sql=? THEN 1 WHEN type='table' AND name='stock_grants' AND tbl_name='stock_grants' AND sql=? THEN 1 ELSE 0 END)=2 THEN 1 ELSE 0 END FROM sqlite_schema").bind(AUTHORITY).bind(GRANTS).fetch_one(&mut *connection).await.map_err(|_| WorkspaceError::RecoveryRequired)?;
    let version: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(&mut *connection)
        .await
        .map_err(|_| WorkspaceError::RecoveryRequired)?;
    if valid != 1 || version != 1 {
        return Err(WorkspaceError::RecoveryRequired);
    }
    let integrity: String = sqlx::query_scalar("PRAGMA quick_check(1)")
        .fetch_one(connection)
        .await
        .map_err(|_| WorkspaceError::RecoveryRequired)?;
    if integrity != "ok" {
        return Err(WorkspaceError::RecoveryRequired);
    }
    Ok(())
}
pub(super) async fn initialize(
    connection: &mut SqliteConnection,
    namespace: &Namespace,
    input: &AttachmentInput,
    initial: Option<Initial>,
    crypto: &WorkspaceCrypto,
) -> Result<(), WorkspaceError> {
    namespace.validate()?;
    if let Some(initial) = initial {
        let authority = Authority {
            version: 1,
            input: input.clone(),
            transaction: initial.transaction,
            revocation: Sequence::new(1).map_err(|_| WorkspaceError::InvalidInput)?,
        };
        let clear = zeroize::Zeroizing::new(
            serde_json::to_vec(&authority).map_err(|_| WorkspaceError::StorageFailure)?,
        );
        let root = crypto.seal_stock(input, true, &clear).await?;
        let grant = crypto
            .seal_stock(input, false, &initial.grant.encode()?)
            .await?;
        let now = initial.clock.now()?;
        if now.get() == 0 {
            return Err(WorkspaceError::Unavailable);
        }
        if now >= initial.expires_at {
            return Err(WorkspaceError::GrantExpired);
        }
        let mut tx = connection
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(|_| WorkspaceError::StorageFailure)?;
        let changed: Result<(), WorkspaceError> = async {
            namespace.validate()?;
            let now = initial.clock.now()?;
            if now.get() == 0 {
                return Err(WorkspaceError::Unavailable);
            }
            if now >= initial.expires_at {
                return Err(WorkspaceError::GrantExpired);
            }
            for sql in CATALOG {
                sqlx::raw_sql(sql)
                    .execute(&mut *tx)
                    .await
                    .map_err(|_| WorkspaceError::StorageFailure)?;
            }
            sqlx::query("PRAGMA user_version=1")
                .execute(&mut *tx)
                .await
                .map_err(|_| WorkspaceError::StorageFailure)?;
            sqlx::query("INSERT INTO stock_authority(singleton,protected) VALUES(1,?)")
                .bind(root)
                .execute(&mut *tx)
                .await
                .map_err(|_| WorkspaceError::StorageFailure)?;
            sqlx::query("INSERT INTO stock_grants(grant_id,protected) VALUES(?,?)")
                .bind(input.grant_id().as_bytes().as_slice())
                .bind(grant)
                .execute(&mut *tx)
                .await
                .map_err(|_| WorkspaceError::StorageFailure)?;
            Ok(())
        }
        .await;
        if let Err(error) = changed {
            tx.rollback()
                .await
                .map_err(|_| WorkspaceError::CommitUnknown)?;
            return Err(error);
        }
        tx.commit()
            .await
            .map_err(|_| WorkspaceError::CommitUnknown)?;
    }
    catalog(connection).await?;
    read(connection, input, crypto).await?;
    namespace.validate()
}
async fn read(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
) -> Result<(Authority, GrantRecord, Vec<u8>, Vec<u8>), WorkspaceError> {
    // Bound in SQL before materializing attacker-controlled protected payloads.
    let root: Vec<u8> = sqlx::query_scalar("SELECT protected FROM stock_authority WHERE singleton=1 AND length(protected) BETWEEN 1 AND 262144").fetch_optional(&mut *connection).await.map_err(|_| WorkspaceError::RecoveryRequired)?.ok_or(WorkspaceError::RecoveryRequired)?;
    let grant: Vec<u8> = sqlx::query_scalar("SELECT protected FROM stock_grants WHERE grant_id=? AND length(protected) BETWEEN 1 AND 262144").bind(input.grant_id().as_bytes().as_slice()).fetch_optional(&mut *connection).await.map_err(|_| WorkspaceError::RecoveryRequired)?.ok_or(WorkspaceError::WrongNamespace)?;
    let clear = crypto.open_stock(input, true, &root).await?;
    let authority: Authority = clear
        .expose_secret(|bytes| serde_json::from_slice(bytes))
        .map_err(|_| WorkspaceError::RecoveryRequired)?;
    if authority.version != 1 || authority.input != *input || authority.revocation.get() == 0 {
        return Err(WorkspaceError::WrongNamespace);
    }
    let clear = crypto.open_stock(input, false, &grant).await?;
    let record = clear.expose_secret(GrantRecord::decode)?;
    // Validate the protected scope without inventing possession of its credential.
    if record.scope() != WorkspaceScope::SellerStockOwner {
        return Err(WorkspaceError::RecoveryRequired);
    }
    Ok((authority, record, root, grant))
}
pub(super) async fn authenticate(
    connection: &mut SqliteConnection,
    namespace: &Namespace,
    expected: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    clock: &dyn WorkspaceClock,
    selected: &AttachmentInput,
    proof: &AttachmentProof,
) -> Result<(StockAuthorityReceipt, WorkspaceBinding), WorkspaceError> {
    namespace.validate()?;
    let (authority, record, root, grant) = read(connection, expected, crypto).await?;
    record.authenticate(selected, proof, clock.now()?)?;
    let mut tx = connection
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|_| WorkspaceError::StorageFailure)?;
    let checked = async {
        namespace.validate()?;
        unchanged(&mut tx, expected, &root, &grant).await?;
        record.authenticate(selected, proof, clock.now()?)
    }
    .await;
    tx.rollback()
        .await
        .map_err(|_| WorkspaceError::CommitUnknown)?;
    let binding = checked?;
    if binding.scope() != WorkspaceScope::SellerStockOwner {
        return Err(WorkspaceError::RecoveryRequired);
    }
    namespace.validate()?;
    Ok((
        StockAuthorityReceipt {
            input: authority.input,
            transaction: authority.transaction,
            revocation: authority.revocation,
        },
        binding,
    ))
}
pub(super) async fn revoke(
    connection: &mut SqliteConnection,
    namespace: &Namespace,
    expected: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    clock: &dyn WorkspaceClock,
    selected: &AttachmentInput,
    proof: &AttachmentProof,
) -> Result<(), WorkspaceError> {
    namespace.validate()?;
    let (mut authority, mut record, old_root, old_grant) =
        read(connection, expected, crypto).await?;
    let expiry = record
        .authenticate(selected, proof, clock.now()?)?
        .expires_at();
    record.revoke()?;
    authority.revocation = Sequence::new(
        authority
            .revocation
            .get()
            .checked_add(1)
            .ok_or(WorkspaceError::Capacity)?,
    )
    .map_err(|_| WorkspaceError::Capacity)?;
    let clear = zeroize::Zeroizing::new(
        serde_json::to_vec(&authority).map_err(|_| WorkspaceError::StorageFailure)?,
    );
    let root = crypto.seal_stock(expected, true, &clear).await?;
    let grant = crypto
        .seal_stock(expected, false, &record.encode()?)
        .await?;
    namespace.validate()?;
    let mut tx = connection
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|_| WorkspaceError::StorageFailure)?;
    let changed: Result<(), WorkspaceError> = async {
        // Current clock and both exact protected snapshots fence the write after provider IO.
        record.authenticate_retained_capture(selected, proof)?;
        let now = clock.now()?;
        if now.get() == 0 {
            return Err(WorkspaceError::Unavailable);
        }
        if now >= expiry {
            return Err(WorkspaceError::GrantExpired);
        }
        let a =
            sqlx::query("UPDATE stock_authority SET protected=? WHERE singleton=1 AND protected=?")
                .bind(root)
                .bind(old_root)
                .execute(&mut *tx)
                .await
                .map_err(|_| WorkspaceError::StorageFailure)?;
        let g = sqlx::query("UPDATE stock_grants SET protected=? WHERE grant_id=? AND protected=?")
            .bind(grant)
            .bind(expected.grant_id().as_bytes().as_slice())
            .bind(old_grant)
            .execute(&mut *tx)
            .await
            .map_err(|_| WorkspaceError::StorageFailure)?;
        if a.rows_affected() != 1 || g.rows_affected() != 1 {
            return Err(WorkspaceError::RecoveryRequired);
        }
        namespace.validate()?;
        Ok(())
    }
    .await;
    if let Err(error) = changed {
        tx.rollback()
            .await
            .map_err(|_| WorkspaceError::CommitUnknown)?;
        return Err(error);
    }
    tx.commit().await.map_err(|_| WorkspaceError::CommitUnknown)
}

async fn unchanged(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    root: &[u8],
    grant: &[u8],
) -> Result<(), WorkspaceError> {
    let matched: i64 = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM stock_authority WHERE singleton=1 AND protected=?) AND EXISTS(SELECT 1 FROM stock_grants WHERE grant_id=? AND protected=?)")
        .bind(root).bind(input.grant_id().as_bytes().as_slice()).bind(grant).fetch_one(connection).await.map_err(|_| WorkspaceError::StorageFailure)?;
    if matched != 1 {
        return Err(WorkspaceError::RecoveryRequired);
    }
    Ok(())
}
