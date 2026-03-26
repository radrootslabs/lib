use crate::error::RadrootsNostrSignerError;
use crate::migrations;
use radroots_sql_core::{SqlExecutor, SqliteExecutor};
use std::path::Path;

pub struct RadrootsNostrSignerSqliteDb {
    executor: SqliteExecutor,
    file_backed: bool,
}

impl RadrootsNostrSignerSqliteDb {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, RadrootsNostrSignerError> {
        let path = path.as_ref();
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)
                .map_err(|error| RadrootsNostrSignerError::Store(error.to_string()))?;
        }
        let executor = SqliteExecutor::open(path)?;
        let db = Self {
            executor,
            file_backed: true,
        };
        db.configure()?;
        db.migrate_up()?;
        Ok(db)
    }

    pub fn open_memory() -> Result<Self, RadrootsNostrSignerError> {
        let executor = SqliteExecutor::open_memory()?;
        let db = Self {
            executor,
            file_backed: false,
        };
        db.configure()?;
        db.migrate_up()?;
        Ok(db)
    }

    pub fn executor(&self) -> &SqliteExecutor {
        &self.executor
    }

    pub fn migrate_up(&self) -> Result<(), RadrootsNostrSignerError> {
        migrations::run_all_up(&self.executor)?;
        Ok(())
    }

    pub fn migrate_down(&self) -> Result<(), RadrootsNostrSignerError> {
        migrations::run_all_down(&self.executor)?;
        Ok(())
    }

    fn configure(&self) -> Result<(), RadrootsNostrSignerError> {
        let pragma_batch = if self.file_backed {
            "PRAGMA foreign_keys = ON;
             PRAGMA synchronous = FULL;
             PRAGMA wal_autocheckpoint = 1000;
             PRAGMA busy_timeout = 5000;
             PRAGMA temp_store = MEMORY;"
        } else {
            "PRAGMA foreign_keys = ON;
             PRAGMA synchronous = NORMAL;
             PRAGMA busy_timeout = 5000;
             PRAGMA temp_store = MEMORY;"
        };
        let _ = self.executor.exec(pragma_batch, "[]")?;
        if self.file_backed {
            let _ = self.executor.query_raw("PRAGMA journal_mode = WAL", "[]")?;
        } else {
            let _ = self
                .executor
                .query_raw("PRAGMA journal_mode = MEMORY", "[]")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::RadrootsNostrSignerSqliteDb;
    use radroots_sql_core::SqlExecutor;
    use serde_json::Value;

    fn query_values(
        db: &RadrootsNostrSignerSqliteDb,
        sql: &str,
    ) -> Vec<serde_json::Map<String, Value>> {
        let raw = db.executor().query_raw(sql, "[]").expect("query");
        serde_json::from_str::<Vec<serde_json::Map<String, Value>>>(&raw).expect("rows")
    }

    fn query_single_text(db: &RadrootsNostrSignerSqliteDb, sql: &str, field: &str) -> String {
        query_values(db, sql)
            .into_iter()
            .next()
            .and_then(|row| row.get(field).cloned())
            .and_then(|value| value.as_str().map(ToOwned::to_owned))
            .expect("single text row")
    }

    fn query_single_i64(db: &RadrootsNostrSignerSqliteDb, sql: &str, field: &str) -> i64 {
        query_values(db, sql)
            .into_iter()
            .next()
            .and_then(|row| row.get(field).cloned())
            .and_then(|value| value.as_i64())
            .expect("single integer row")
    }

    #[test]
    fn open_memory_bootstraps_schema_and_migrations_idempotently() {
        let db = RadrootsNostrSignerSqliteDb::open_memory().expect("open memory db");
        db.migrate_up().expect("rerun migrations");

        let tables = query_values(
            &db,
            "SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name",
        );
        let table_names = tables
            .into_iter()
            .filter_map(|row| {
                row.get("name")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned)
            })
            .collect::<Vec<_>>();
        assert!(table_names.iter().any(|name| name == "__migrations"));
        assert!(
            table_names
                .iter()
                .any(|name| name == "signer_store_metadata")
        );
        assert!(table_names.iter().any(|name| name == "signer_connection"));
        assert!(
            table_names
                .iter()
                .any(|name| name == "signer_connection_permission_grant")
        );
        assert!(
            table_names
                .iter()
                .any(|name| name == "signer_connection_relay")
        );
        assert!(
            table_names
                .iter()
                .any(|name| name == "signer_connection_auth_challenge")
        );
        assert!(
            table_names
                .iter()
                .any(|name| name == "signer_connection_pending_request")
        );
        assert!(
            table_names
                .iter()
                .any(|name| name == "signer_request_audit")
        );
        assert!(
            table_names
                .iter()
                .any(|name| name == "signer_publish_workflow")
        );

        let migration_count = query_single_i64(
            &db,
            "SELECT COUNT(*) AS applied_count FROM __migrations",
            "applied_count",
        );
        assert_eq!(migration_count, 2);

        let store_version = query_single_i64(
            &db,
            "SELECT store_version FROM signer_store_metadata WHERE singleton_id = 1",
            "store_version",
        );
        assert_eq!(store_version, 1);
    }

    #[test]
    fn file_database_uses_wal_and_foreign_keys() {
        let temp = tempfile::tempdir().expect("tempdir");
        let db = RadrootsNostrSignerSqliteDb::open(temp.path().join("signer.sqlite"))
            .expect("open sqlite file db");

        assert_eq!(
            query_single_text(&db, "PRAGMA journal_mode", "journal_mode"),
            "wal"
        );
        assert_eq!(
            query_single_i64(&db, "PRAGMA foreign_keys", "foreign_keys"),
            1
        );
    }

    #[test]
    fn migrate_down_and_up_roundtrip_restores_schema() {
        let db = RadrootsNostrSignerSqliteDb::open_memory().expect("open memory db");
        db.migrate_down().expect("migrate down");

        let tables = query_values(
            &db,
            "SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name",
        );
        let table_names = tables
            .into_iter()
            .filter_map(|row| {
                row.get("name")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned)
            })
            .collect::<Vec<_>>();
        assert_eq!(table_names, vec!["__migrations".to_owned()]);

        db.migrate_up().expect("migrate up again");
        let migration_count = query_single_i64(
            &db,
            "SELECT COUNT(*) AS applied_count FROM __migrations",
            "applied_count",
        );
        assert_eq!(migration_count, 2);
    }
}
