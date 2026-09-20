//! Bound result columns before SQLx materializes a stored authored revision.
//! Invalid nullable metadata uses a noncanonical one-byte sentinel, never NULL:
//! corruption must not turn into an accepted missing operation or scope.

use radroots_storage::{Error, authored_draft::AuthoredDraftRevision};
use sqlx::{Executor, Sqlite, sqlite::SqliteRow};

macro_rules! select {
    ($suffix:literal) => {
        concat!(
            "SELECT
             CASE WHEN typeof(draft_id) = 'blob' AND octet_length(draft_id) = 16 THEN draft_id END AS draft_id,
             CASE WHEN typeof(revision) = 'integer' THEN revision END AS revision,
             CASE WHEN typeof(author) = 'blob' AND octet_length(author) = 32 THEN author END AS author,
             CASE WHEN typeof(stage) = 'integer' THEN stage END AS stage,
             CASE WHEN operation_id IS NULL OR (typeof(operation_id) = 'blob' AND octet_length(operation_id) = 16)
                  THEN operation_id ELSE X'00' END AS operation_id,
             CASE WHEN typeof(payload_sha256) = 'blob' AND octet_length(payload_sha256) = 32 THEN payload_sha256 END AS payload_sha256,
             CASE WHEN typeof(created_at_unix_ms) = 'integer' THEN created_at_unix_ms END AS created_at_unix_ms,
             CASE WHEN typeof(updated_at_unix_ms) = 'integer' THEN updated_at_unix_ms END AS updated_at_unix_ms,
             CASE WHEN typeof(snapshot) = 'blob' AND octet_length(snapshot) BETWEEN 1 AND 16777216 THEN snapshot END AS snapshot,
             CASE WHEN typeof(payload_schema) = 'text' AND octet_length(payload_schema) <= 128 THEN payload_schema END AS payload_schema,
             CASE WHEN payload_scope IS NULL OR (typeof(payload_scope) = 'blob' AND octet_length(payload_scope) = 32)
                  THEN payload_scope ELSE X'00' END AS payload_scope
             FROM radroots_runtime_authored_draft_revisions ",
            $suffix
        )
    };
}

const HEAD: &str = select!(
    "WHERE draft_id = ? AND ? IS NULL ORDER BY radroots_runtime_authored_draft_revisions.revision DESC LIMIT 1"
);
const REVISION: &str = select!("WHERE draft_id = ? AND revision = ?");

pub(super) async fn load<'e>(
    executor: impl Executor<'e, Database = Sqlite>,
    key: &[u8; 16],
    revision: Option<AuthoredDraftRevision>,
) -> Result<Option<SqliteRow>, Error> {
    let selected = if revision.is_some() { REVISION } else { HEAD };
    let revision = revision
        .map(|value| super::i64_from_u64(value.get()))
        .transpose()?;
    // Both alternatives are closed compile-time SQL above. Every data value is
    // bound separately; this private helper accepts no caller-supplied SQL.
    sqlx::query(sqlx::AssertSqlSafe(selected))
        .bind(key.as_slice())
        .bind(revision)
        .fetch_optional(executor)
        .await
        .map_err(super::map_backend)
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
#[path = "authored_draft_row_tests.rs"]
mod tests;
