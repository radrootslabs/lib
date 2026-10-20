//! Retained initial-coordinate decisions. Updates have separate expected-head
//! commands; another initial publish never overwrites this creation decision.
use super::{
    AttachmentInput,
    crypto::{ListingScope, WorkspaceCrypto},
};
use radroots_domain::{
    CommandDigest, OperationId, PublicKey, TransactionId, WorkspaceId, listing::ListingId,
};
use radroots_storage::command::{AdmissionRequest, OperationError, PreparedOperation};
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqliteConnection};
use zeroize::Zeroizing;

pub(super) const CREATIONS: &str = "CREATE TABLE listing_creations (workspace BLOB NOT NULL, operation_id BLOB NOT NULL, author BLOB NOT NULL CHECK(length(author)=32), listing_id BLOB NOT NULL CHECK(length(listing_id)=16), transaction_id BLOB NOT NULL CHECK(length(transaction_id)=32), protected BLOB NOT NULL CHECK(length(protected)>0 AND length(protected)<=262144), PRIMARY KEY(workspace,operation_id), FOREIGN KEY(workspace,operation_id) REFERENCES operations(workspace,operation_id)) STRICT, WITHOUT ROWID";
pub(super) const COORDINATE_INDEX: &str = "CREATE UNIQUE INDEX listing_coordinate_identity ON listing_creations(workspace,author,listing_id)";
#[derive(Serialize, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct DecisionRecord {
    version: u8,
    workspace: WorkspaceId,
    operation: OperationId,
    command: CommandDigest,
    author: PublicKey,
    listing: ListingId,
    transaction: TransactionId,
}
impl DecisionRecord {
    fn new(request: &AdmissionRequest, transaction: TransactionId) -> Result<Self, OperationError> {
        let command = request
            .command()
            .listing()
            .ok_or(OperationError::UnsupportedOperation)?;
        let context = request.command().context();
        Ok(Self {
            version: 1,
            workspace: context.workspace_id(),
            operation: context.operation_id(),
            command: request.command().digest(),
            author: context.actor().participant(),
            listing: command.input().id(),
            transaction,
        })
    }
    fn scope(&self) -> ListingScope {
        ListingScope::Decision {
            workspace: self.workspace,
            operation: self.operation,
        }
    }
}
pub(super) struct Creation {
    record: DecisionRecord,
    protected: Vec<u8>,
}
impl Creation {
    pub(super) async fn protect(
        request: &AdmissionRequest,
        transaction: TransactionId,
        crypto: &WorkspaceCrypto,
    ) -> Result<Option<Self>, OperationError> {
        if request.command().listing().is_none() {
            return Ok(None);
        }
        let record = DecisionRecord::new(request, transaction)?;
        let raw = Zeroizing::new(
            serde_json::to_vec(&record).map_err(|_| OperationError::InvalidCommand)?,
        );
        let protected = crypto.seal_listing(record.scope(), &raw).await?;
        Ok(Some(Self { record, protected }))
    }
    pub(super) async fn read(
        connection: &mut SqliteConnection,
        prepared: &PreparedOperation,
        crypto: &WorkspaceCrypto,
    ) -> Result<Option<Self>, OperationError> {
        if !current(connection).await? {
            return Ok(None);
        }
        let request = prepared.request();
        let context = request.command().context();
        let row=sqlx::query("SELECT CASE WHEN length(author)=32 THEN author END,CASE WHEN length(listing_id)=16 THEN listing_id END,CASE WHEN length(transaction_id)=32 THEN transaction_id END,CASE WHEN length(protected)>0 AND length(protected)<=262144 THEN protected END FROM listing_creations WHERE workspace=? AND operation_id=?")
            .bind(context.workspace_id().as_bytes().as_slice()).bind(context.operation_id().as_bytes().as_slice()).fetch_optional(&mut *connection).await.map_err(|_|OperationError::RecoveryRequired)?;
        let Some(row) = row else {
            return if request.command().listing().is_some() {
                Err(OperationError::RecoveryRequired)
            } else {
                Ok(None)
            };
        };
        let bytes = |i| -> Result<Vec<u8>, OperationError> {
            row.try_get::<Option<Vec<u8>>, _>(i)
                .map_err(|_| OperationError::RecoveryRequired)?
                .ok_or(OperationError::RecoveryRequired)
        };
        let expected = DecisionRecord::new(request, prepared.transaction_id())
            .map_err(|_| OperationError::RecoveryRequired)?;
        let protected = bytes(3)?;
        let raw = crypto.open_listing(expected.scope(), &protected).await?;
        let record: DecisionRecord = raw
            .expose_secret(|raw| serde_json::from_slice(raw))
            .map_err(|_| OperationError::RecoveryRequired)?;
        if record != expected
            || bytes(0)? != record.author.as_bytes()
            || bytes(1)? != record.listing.as_bytes()
            || bytes(2)? != record.transaction.as_bytes()
        {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(Some(Self { record, protected }))
    }
    pub(super) async fn insert(
        &self,
        connection: &mut SqliteConnection,
    ) -> Result<(), OperationError> {
        let r = &self.record;
        // This check is inside the same BEGIN IMMEDIATE as T0 insertion. A
        // competing operation cannot reach its first signing claim.
        let exists:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM listing_creations WHERE workspace=? AND author=? AND listing_id=?)")
            .bind(r.workspace.as_bytes().as_slice()).bind(r.author.as_bytes().as_slice()).bind(r.listing.as_bytes().as_slice()).fetch_one(&mut *connection).await.map_err(|_|OperationError::RecoveryRequired)?;
        if exists {
            return Err(OperationError::StaleState);
        }
        sqlx::query("INSERT INTO listing_creations(workspace,operation_id,author,listing_id,transaction_id,protected) VALUES(?,?,?,?,?,?)")
            .bind(r.workspace.as_bytes().as_slice()).bind(r.operation.as_bytes().as_slice()).bind(r.author.as_bytes().as_slice()).bind(r.listing.as_bytes().as_slice()).bind(r.transaction.as_bytes().as_slice()).bind(&self.protected).execute(connection).await.map_err(|_|OperationError::StorageFailure)?;
        Ok(())
    }
    pub(super) async fn unchanged(
        &self,
        connection: &mut SqliteConnection,
    ) -> Result<(), OperationError> {
        let r = &self.record;
        let count:i64=sqlx::query_scalar("SELECT count(*) FROM listing_creations WHERE workspace=? AND operation_id=? AND author=? AND listing_id=? AND transaction_id=? AND protected=?")
            .bind(r.workspace.as_bytes().as_slice()).bind(r.operation.as_bytes().as_slice()).bind(r.author.as_bytes().as_slice()).bind(r.listing.as_bytes().as_slice()).bind(r.transaction.as_bytes().as_slice()).bind(&self.protected).fetch_one(connection).await.map_err(|_|OperationError::RecoveryRequired)?;
        if count != 1 {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(())
    }
}
pub(super) async fn current(connection: &mut SqliteConnection) -> Result<bool, OperationError> {
    let version: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(connection)
        .await
        .map_err(|_| OperationError::RecoveryRequired)?;
    match version {
        7..=9 => Ok(false),
        10 => Ok(true),
        _ => Err(OperationError::RecoveryRequired),
    }
}
pub(super) async fn absent(
    connection: &mut SqliteConnection,
    selected: &AttachmentInput,
    operation: OperationId,
) -> Result<(), OperationError> {
    if !current(connection).await? {
        return Ok(());
    }
    let found: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM listing_creations WHERE workspace=? AND operation_id=?)",
    )
    .bind(selected.workspace_id().as_bytes().as_slice())
    .bind(operation.as_bytes().as_slice())
    .fetch_one(connection)
    .await
    .map_err(|_| OperationError::RecoveryRequired)?;
    if found {
        return Err(OperationError::RecoveryRequired);
    }
    Ok(())
}
