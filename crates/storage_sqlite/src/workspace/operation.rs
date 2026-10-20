//! Retained non-stock command preparation. No signing or delivery occurs here.

use super::{AttachmentInput, crypto::WorkspaceCrypto, listing};
use radroots_domain::{IssuanceWindow, OperationId, TransactionId};
use radroots_storage::command::{AdmissionRequest, OperationError, PreparedOperation};
use sqlx::{Row, SqliteConnection};

pub(super) const TABLE_V7: &str = "CREATE TABLE operations (workspace BLOB NOT NULL CHECK(length(workspace)=32), operation_id BLOB NOT NULL CHECK(length(operation_id)=32), actor BLOB NOT NULL CHECK(length(actor)=32), grant_id BLOB NOT NULL CHECK(length(grant_id)=32), command_type TEXT NOT NULL CHECK(command_type IN ('inbox.inquire','inbox.reply')), command_digest BLOB NOT NULL CHECK(length(command_digest)=32), transaction_id BLOB NOT NULL CHECK(length(transaction_id)=32), phase TEXT NOT NULL CHECK(phase='prepared'), revision INTEGER NOT NULL CHECK(revision>0), protected BLOB NOT NULL CHECK(length(protected)>0 AND length(protected)<=262144), grant_singleton INTEGER NOT NULL CHECK(grant_singleton=1) REFERENCES workspace_grant(singleton), PRIMARY KEY(workspace,operation_id)) STRICT, WITHOUT ROWID";
pub(super) const TABLE: &str = "CREATE TABLE operations (workspace BLOB NOT NULL CHECK(length(workspace)=32), operation_id BLOB NOT NULL CHECK(length(operation_id)=32), actor BLOB NOT NULL CHECK(length(actor)=32), grant_id BLOB NOT NULL CHECK(length(grant_id)=32), command_type TEXT NOT NULL CHECK(command_type IN ('inbox.inquire','inbox.reply','listing.publish')), command_digest BLOB NOT NULL CHECK(length(command_digest)=32), transaction_id BLOB NOT NULL CHECK(length(transaction_id)=32), phase TEXT NOT NULL CHECK(phase='prepared'), revision INTEGER NOT NULL CHECK(revision>0), protected BLOB NOT NULL CHECK(length(protected)>0 AND length(protected)<=262144), grant_singleton INTEGER NOT NULL CHECK(grant_singleton=1) REFERENCES workspace_grant(singleton), PRIMARY KEY(workspace,operation_id)) STRICT, WITHOUT ROWID";
pub(super) const INDEX: &str =
    "CREATE UNIQUE INDEX operation_transaction_identity ON operations(workspace,transaction_id)";

pub(super) struct ProtectedOperation {
    pub bytes: Vec<u8>,
    pub creation: Option<listing::Creation>,
}

pub(super) async fn protect(
    request: &AdmissionRequest,
    transaction_id: TransactionId,
    window: IssuanceWindow,
    crypto: &WorkspaceCrypto,
) -> Result<Vec<u8>, OperationError> {
    let plaintext = radroots_storage::command::protected::encode(request, transaction_id, window)?;
    Ok(crypto
        .seal_operation(
            request.binding().input().workspace_id(),
            request.command().context().operation_id(),
            &plaintext,
        )
        .await?)
}

pub(super) async fn read(
    connection: &mut SqliteConnection,
    selected: &AttachmentInput,
    operation_id: OperationId,
    crypto: &WorkspaceCrypto,
) -> Result<Option<(PreparedOperation, ProtectedOperation)>, OperationError> {
    let row = sqlx::query("SELECT CASE WHEN length(actor)=32 THEN actor END,CASE WHEN length(grant_id)=32 THEN grant_id END,CASE WHEN length(command_type)<=80 THEN command_type END,CASE WHEN length(command_digest)=32 THEN command_digest END,CASE WHEN length(transaction_id)=32 THEN transaction_id END,CASE WHEN length(phase)<=32 THEN phase END,revision,CASE WHEN length(protected)>0 AND length(protected)<=262144 THEN protected END FROM operations WHERE workspace=? AND operation_id=?")
        .bind(selected.workspace_id().as_bytes().as_slice()).bind(operation_id.as_bytes().as_slice())
        .fetch_optional(&mut *connection).await.map_err(|_| OperationError::RecoveryRequired)?;
    let Some(row) = row else {
        absent(connection, selected, operation_id).await?;
        return Ok(None);
    };
    let bytes = |index| -> Result<Vec<u8>, OperationError> {
        row.try_get::<Option<Vec<u8>>, _>(index)
            .map_err(|_| OperationError::RecoveryRequired)?
            .ok_or(OperationError::RecoveryRequired)
    };
    let actor = bytes(0)?;
    let grant = bytes(1)?;
    let kind: String = row
        .try_get::<Option<String>, _>(2)
        .map_err(|_| OperationError::RecoveryRequired)?
        .ok_or(OperationError::RecoveryRequired)?;
    let digest = bytes(3)?;
    let transaction = bytes(4)?;
    let phase: String = row
        .try_get::<Option<String>, _>(5)
        .map_err(|_| OperationError::RecoveryRequired)?
        .ok_or(OperationError::RecoveryRequired)?;
    let revision: i64 = row
        .try_get(6)
        .map_err(|_| OperationError::RecoveryRequired)?;
    let protected = bytes(7)?;
    let plaintext = crypto
        .open_operation(selected.workspace_id(), operation_id, &protected)
        .await?;
    let snapshot = plaintext.expose_secret(|bytes| {
        radroots_storage::command::protected::decode(bytes, selected, operation_id)
    })?;
    let request = snapshot.request();
    if phase != "prepared"
        || revision != 1
        || actor != request.command().context().actor().participant().as_bytes()
        || grant != request.binding().input().grant_id().as_bytes()
        || digest != request.command().digest().as_bytes()
        || transaction != snapshot.transaction_id().as_bytes()
        || kind != request.command().kind().as_str()
    {
        return Err(OperationError::RecoveryRequired);
    }
    let creation = listing::Creation::read(connection, &snapshot, crypto).await?;
    Ok(Some((
        snapshot,
        ProtectedOperation {
            bytes: protected,
            creation,
        },
    )))
}

pub(super) async fn insert(
    connection: &mut SqliteConnection,
    request: &AdmissionRequest,
    transaction_id: TransactionId,
    protected: &[u8],
) -> Result<(), OperationError> {
    let context = request.command().context();
    sqlx::query("INSERT INTO operations(workspace,operation_id,actor,grant_id,command_type,command_digest,transaction_id,phase,revision,protected,grant_singleton) VALUES(?,?,?,?,?,?,?,'prepared',1,?,1)")
        .bind(context.workspace_id().as_bytes().as_slice()).bind(context.operation_id().as_bytes().as_slice())
        .bind(context.actor().participant().as_bytes().as_slice()).bind(request.binding().input().grant_id().as_bytes().as_slice())
        .bind(request.command().kind().as_str()).bind(request.command().digest().as_bytes().as_slice())
        .bind(transaction_id.as_bytes().as_slice()).bind(protected)
        .execute(connection).await.map_err(|_| OperationError::StorageFailure)?;
    Ok(())
}

/// Rechecks the retained ciphertext and every indexed fact in one transaction,
/// without awaiting a provider, signer or network capability under it.
pub(super) async fn unchanged(
    connection: &mut SqliteConnection,
    snapshot: &PreparedOperation,
    expected: &ProtectedOperation,
) -> Result<(), OperationError> {
    let request = snapshot.request();
    let context = request.command().context();
    let matched: i64 = sqlx::query_scalar("SELECT count(*) FROM operations WHERE workspace=? AND operation_id=? AND actor=? AND grant_id=? AND command_type=? AND command_digest=? AND transaction_id=? AND phase='prepared' AND revision=? AND protected=?")
        .bind(context.workspace_id().as_bytes().as_slice()).bind(context.operation_id().as_bytes().as_slice())
        .bind(context.actor().participant().as_bytes().as_slice()).bind(request.binding().input().grant_id().as_bytes().as_slice())
        .bind(request.command().kind().as_str()).bind(request.command().digest().as_bytes().as_slice())
        .bind(snapshot.transaction_id().as_bytes().as_slice())
        // Sequence construction enforces the signed 64-bit storage bound.
        .bind(snapshot.revision().get() as i64).bind(&expected.bytes)
        .fetch_one(&mut *connection).await.map_err(|_| OperationError::RecoveryRequired)?;
    if matched != 1 {
        return Err(OperationError::RecoveryRequired);
    }
    if let Some(creation) = &expected.creation {
        creation.unchanged(connection).await?;
    } else if listing::current(connection).await? {
        if snapshot.request().command().listing().is_some() {
            return Err(OperationError::RecoveryRequired);
        }
        listing::absent(
            connection,
            snapshot.request().binding().input(),
            context.operation_id(),
        )
        .await?;
    }
    Ok(())
}

pub(super) use radroots_storage::command::protected::check_digest;

/// Confirms complete original-ID absence before a new preparation or absent
/// lookup. Every current operation-owned table has an indexed workspace/ID
/// prefix. EXISTS never materializes an unbounded retained record set.
pub(super) async fn absent(
    connection: &mut SqliteConnection,
    selected: &AttachmentInput,
    operation: OperationId,
) -> Result<(), OperationError> {
    let retained: bool = sqlx::query_scalar(super::attempt::sql(connection, "SELECT EXISTS(SELECT 1 FROM operations WHERE workspace=?1 AND operation_id=?2) OR EXISTS(SELECT 1 FROM signing_plans WHERE workspace=?1 AND operation_id=?2) OR EXISTS(SELECT 1 FROM signing_plan_parts WHERE workspace=?1 AND operation_id=?2) OR EXISTS(SELECT 1 FROM signing_attempts WHERE workspace=?1 AND operation_id=?2) OR EXISTS(SELECT 1 FROM seal_evidence WHERE workspace=?1 AND operation_id=?2) OR EXISTS(SELECT 1 FROM seal_evidence_parts WHERE workspace=?1 AND operation_id=?2) OR EXISTS(SELECT 1 FROM chat_messages WHERE workspace=?1 AND operation_id=?2) OR EXISTS(SELECT 1 FROM chat_publication_work WHERE workspace=?1 AND operation_id=?2) OR EXISTS(SELECT 1 FROM chat_delivery_attempts WHERE workspace=?1 AND operation_id=?2) OR EXISTS(SELECT 1 FROM chat_delivery_parts WHERE workspace=?1 AND operation_id=?2) OR EXISTS(SELECT 1 FROM chat_delivery_observations WHERE workspace=?1 AND operation_id=?2)").await?)
        .bind(selected.workspace_id().as_bytes().as_slice())
        .bind(operation.as_bytes().as_slice())
        .fetch_one(&mut *connection).await.map_err(|_|OperationError::RecoveryRequired)?;
    listing::absent(connection, selected, operation).await?;
    super::listing_installation::absent(connection, selected, operation).await?;
    if retained {
        return Err(OperationError::RecoveryRequired);
    }
    Ok(())
}
