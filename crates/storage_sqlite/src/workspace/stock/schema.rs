//! Exact stock catalogs. Enrollment-only version1 is retained without upgrade.
use super::WorkspaceError;
use sqlx::SqliteConnection;
pub(super) const CURRENT: i64 = 5;
const AUTHORITY: &str = "CREATE TABLE stock_authority (singleton INTEGER PRIMARY KEY CHECK(singleton=1), protected BLOB NOT NULL CHECK(length(protected)>0 AND length(protected)<=262144)) STRICT";
const GRANTS: &str = "CREATE TABLE stock_grants (grant_id BLOB PRIMARY KEY CHECK(length(grant_id)=32), protected BLOB NOT NULL CHECK(length(protected)>0 AND length(protected)<=262144)) STRICT, WITHOUT ROWID";
const POOLS: &str = "CREATE TABLE stock_pools (pool_id BLOB PRIMARY KEY CHECK(length(pool_id)=32), revision INTEGER NOT NULL CHECK(revision>0), protected BLOB NOT NULL CHECK(length(protected)>0 AND length(protected)<=262144)) STRICT, WITHOUT ROWID";
const OPERATIONS: &str = "CREATE TABLE stock_operations (actor BLOB NOT NULL CHECK(length(actor)=32), grant_id BLOB NOT NULL REFERENCES stock_grants(grant_id), operation_id BLOB NOT NULL CHECK(length(operation_id)=32), command_digest BLOB NOT NULL CHECK(length(command_digest)=32), transaction_id BLOB NOT NULL CHECK(length(transaction_id)=32), pool_id BLOB NOT NULL REFERENCES stock_pools(pool_id) DEFERRABLE INITIALLY DEFERRED, revision INTEGER NOT NULL CHECK(revision>0), chain_head BLOB NOT NULL CHECK(length(chain_head)=32), protected BLOB NOT NULL CHECK(length(protected)>0 AND length(protected)<=262144), PRIMARY KEY(actor,grant_id,operation_id)) STRICT, WITHOUT ROWID";
const TX: &str =
    "CREATE UNIQUE INDEX stock_operation_transaction_identity ON stock_operations(transaction_id)";
const REVISION: &str =
    "CREATE UNIQUE INDEX stock_operation_revision_identity ON stock_operations(revision)";
const HEAD: &str =
    "CREATE UNIQUE INDEX stock_operation_head_identity ON stock_operations(chain_head)";
const MOVEMENTS: &str = "CREATE TABLE stock_movements (movement_id BLOB PRIMARY KEY CHECK(length(movement_id)=32), actor BLOB NOT NULL CHECK(length(actor)=32), grant_id BLOB NOT NULL CHECK(length(grant_id)=32), operation_id BLOB NOT NULL CHECK(length(operation_id)=32), pool_id BLOB NOT NULL REFERENCES stock_pools(pool_id), protected BLOB NOT NULL CHECK(length(protected)>0 AND length(protected)<=262144), FOREIGN KEY(actor,grant_id,operation_id) REFERENCES stock_operations(actor,grant_id,operation_id)) STRICT, WITHOUT ROWID";
pub(super) fn objects(
    version: i64,
) -> Result<Vec<(&'static str, &'static str, &'static str, &'static str)>, WorkspaceError> {
    let mut objects = vec![
        ("table", "stock_authority", "stock_authority", AUTHORITY),
        ("table", "stock_grants", "stock_grants", GRANTS),
    ];
    match version {
        1 => {}
        2..=5 => objects.extend([
            ("table", "stock_pools", "stock_pools", POOLS),
            ("table", "stock_operations", "stock_operations", OPERATIONS),
            (
                "index",
                "stock_operation_transaction_identity",
                "stock_operations",
                TX,
            ),
            (
                "index",
                "stock_operation_revision_identity",
                "stock_operations",
                REVISION,
            ),
            (
                "index",
                "stock_operation_head_identity",
                "stock_operations",
                HEAD,
            ),
            ("table", "stock_movements", "stock_movements", MOVEMENTS),
        ]),
        _ => return Err(WorkspaceError::RecoveryRequired),
    }
    if version >= 4 {
        objects.push(("table", "stock_grant_changes", "stock_grant_changes", "CREATE TABLE stock_grant_changes (transaction_id BLOB PRIMARY KEY CHECK(length(transaction_id)=32), revision INTEGER NOT NULL CHECK(revision>1), protected BLOB NOT NULL CHECK(length(protected)>0 AND length(protected)<=262144)) STRICT, WITHOUT ROWID"));
        objects.push((
            "index",
            "stock_grant_change_revision",
            "stock_grant_changes",
            "CREATE UNIQUE INDEX stock_grant_change_revision ON stock_grant_changes(revision)",
        ));
        objects.push(("index", "stock_actor_operation_identity", "stock_operations", "CREATE UNIQUE INDEX stock_actor_operation_identity ON stock_operations(actor,operation_id)"));
    }
    if version == 5 {
        objects.push(("table","stock_mappings","stock_mappings","CREATE TABLE stock_mappings (listing_author BLOB NOT NULL CHECK(length(listing_author)=32), listing_d BLOB NOT NULL CHECK(length(listing_d)=16), option_id BLOB NOT NULL CHECK(length(option_id)=32), pool_id BLOB NOT NULL REFERENCES stock_pools(pool_id), revision INTEGER NOT NULL CHECK(revision>0), protected BLOB NOT NULL CHECK(length(protected)>0 AND length(protected)<=262144), PRIMARY KEY(listing_author,listing_d,option_id)) STRICT, WITHOUT ROWID"));
    }
    Ok(objects)
}
pub(super) async fn admit(connection: &mut SqliteConnection) -> Result<i64, WorkspaceError> {
    let version: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(&mut *connection)
        .await
        .map_err(|_| WorkspaceError::RecoveryRequired)?;
    let objects = objects(version)?;
    let mut sql = String::from("SELECT CASE WHEN count(*)=? AND sum(CASE");
    for _ in &objects {
        sql.push_str(" WHEN type=? AND name=? AND tbl_name=? AND sql=? THEN 1");
    }
    sql.push_str(" ELSE 0 END)=? THEN 1 ELSE 0 END FROM sqlite_schema");
    let mut query = sqlx::query_scalar(sqlx::AssertSqlSafe(sql)).bind(objects.len() as i64);
    for (kind, name, owner, definition) in &objects {
        query = query.bind(*kind).bind(*name).bind(*owner).bind(*definition);
    }
    let valid: i64 = query
        .bind(objects.len() as i64)
        .fetch_one(connection)
        .await
        .map_err(|_| WorkspaceError::RecoveryRequired)?;
    if valid != 1 {
        return Err(WorkspaceError::RecoveryRequired);
    }
    Ok(version)
}
pub(super) async fn integrity(connection: &mut SqliteConnection) -> Result<(), WorkspaceError> {
    let check: String = sqlx::query_scalar("PRAGMA quick_check(1)")
        .fetch_one(connection)
        .await
        .map_err(|_| WorkspaceError::RecoveryRequired)?;
    if check != "ok" {
        return Err(WorkspaceError::RecoveryRequired);
    }
    Ok(())
}
