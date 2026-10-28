//! Append-only protected transport attempts on the existing workspace owner.

mod codec;
pub(super) mod containment;
pub(super) mod graph;
mod json;
pub(super) mod observation;
mod observation_operations;
mod operations;

pub(super) const TABLE_V10: &str = "CREATE TABLE chat_delivery_attempts (workspace BLOB NOT NULL, operation_id BLOB NOT NULL, recipient BLOB NOT NULL CHECK(length(recipient)=32), request_key BLOB NOT NULL CHECK(length(request_key)=32), event_id BLOB NOT NULL CHECK(length(event_id)=32), transaction_id BLOB NOT NULL CHECK(length(transaction_id)=32), part_count INTEGER NOT NULL CHECK(part_count BETWEEN 1 AND 24), byte_count INTEGER NOT NULL CHECK(byte_count BETWEEN 1 AND 393216), protected BLOB NOT NULL CHECK(length(protected)>0 AND length(protected)<=262144), PRIMARY KEY(workspace,operation_id,recipient,request_key), FOREIGN KEY(workspace,operation_id,recipient) REFERENCES chat_publication_work(workspace,operation_id,recipient)) STRICT, WITHOUT ROWID";
pub(super) const TABLE: &str = "CREATE TABLE artifact_delivery_attempts (workspace BLOB NOT NULL, operation_id BLOB NOT NULL, artifact_key BLOB NOT NULL CHECK(length(artifact_key)=32 OR (length(artifact_key)=1 AND hex(artifact_key)='00')), request_key BLOB NOT NULL CHECK(length(request_key)=32), event_id BLOB NOT NULL CHECK(length(event_id)=32), transaction_id BLOB NOT NULL CHECK(length(transaction_id)=32), part_count INTEGER NOT NULL CHECK(part_count BETWEEN 1 AND 24), byte_count INTEGER NOT NULL CHECK(byte_count BETWEEN 1 AND 393216), protected BLOB NOT NULL CHECK(length(protected)>0 AND length(protected)<=262144), PRIMARY KEY(workspace,operation_id,artifact_key,request_key), FOREIGN KEY(workspace,operation_id,artifact_key) REFERENCES artifact_evidence(workspace,operation_id,artifact_key)) STRICT, WITHOUT ROWID";
pub(super) const PARTS_V10: &str = "CREATE TABLE chat_delivery_parts (workspace BLOB NOT NULL, operation_id BLOB NOT NULL, recipient BLOB NOT NULL, request_key BLOB NOT NULL, part INTEGER NOT NULL CHECK(part BETWEEN 1 AND 24), protected BLOB NOT NULL CHECK(length(protected)>0 AND length(protected)<=262144), PRIMARY KEY(workspace,operation_id,recipient,request_key,part), FOREIGN KEY(workspace,operation_id,recipient,request_key) REFERENCES chat_delivery_attempts(workspace,operation_id,recipient,request_key)) STRICT, WITHOUT ROWID";
pub(super) const PARTS: &str = "CREATE TABLE artifact_delivery_parts (workspace BLOB NOT NULL, operation_id BLOB NOT NULL, artifact_key BLOB NOT NULL, request_key BLOB NOT NULL, part INTEGER NOT NULL CHECK(part BETWEEN 1 AND 24), protected BLOB NOT NULL CHECK(length(protected)>0 AND length(protected)<=262144), PRIMARY KEY(workspace,operation_id,artifact_key,request_key,part), FOREIGN KEY(workspace,operation_id,artifact_key,request_key) REFERENCES artifact_delivery_attempts(workspace,operation_id,artifact_key,request_key)) STRICT, WITHOUT ROWID";
pub(super) const TX_INDEX_V10: &str = "CREATE UNIQUE INDEX chat_delivery_transaction_identity ON chat_delivery_attempts(workspace,transaction_id)";
pub(super) const TX_INDEX: &str = "CREATE UNIQUE INDEX artifact_delivery_transaction_identity ON artifact_delivery_attempts(workspace,transaction_id)";

const MAX_BYTES: usize = 393216;
const MAX_PROTECTED_BYTES: usize = 2 * 1024 * 1024;
const MAX_ATTEMPTS: usize = 2 * 16 * 12;
pub(super) const OBSERVATIONS: &str = observation::TABLE;
pub(super) const OBSERVATIONS_V10: &str = observation::TABLE_V10;

/// Fixed identifier adaptation for exact retained catalogs, never user SQL.
pub(super) async fn sql(
    connection: &mut sqlx::SqliteConnection,
    legacy: &'static str,
) -> Result<sqlx::AssertSqlSafe<String>, radroots_storage::command::OperationError> {
    let version: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(connection)
        .await
        .map_err(|_| radroots_storage::command::OperationError::RecoveryRequired)?;
    match version {
        7..=10 => Ok(sqlx::AssertSqlSafe(legacy.to_owned())),
        11 => Ok(sqlx::AssertSqlSafe(
            legacy
                .replace("chat_delivery", "artifact_delivery")
                .replace("recipient", "artifact_key"),
        )),
        _ => Err(radroots_storage::command::OperationError::RecoveryRequired),
    }
}
pub(super) async fn require_target(
    connection: &mut sqlx::SqliteConnection,
    target: radroots_storage::command::SigningTarget,
) -> Result<(), radroots_storage::command::OperationError> {
    if target == radroots_storage::command::SigningTarget::PublicEvent {
        let version: i64 = sqlx::query_scalar("PRAGMA user_version")
            .fetch_one(connection)
            .await
            .map_err(|_| radroots_storage::command::OperationError::RecoveryRequired)?;
        if version != 11 {
            return Err(radroots_storage::command::OperationError::UnsupportedOperation);
        }
    }
    Ok(())
}
