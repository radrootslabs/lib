use radroots_sql_core::{SqlExecutor, error::SqlError, utils};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::backup::{
    DATABASE_BACKUP_VERSION, MigrationBackup, REPLICA_DB_VERSION, SchemaEntry, escape_identifier,
    export_migrations, load_schema,
};

pub const REPLICA_DB_EXPORT_VERSION: &str = "1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableCount {
    pub name: String,
    pub row_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicaDbExportManifestRs {
    pub export_version: String,
    pub replica_db_version: String,
    pub backup_format_version: String,
    pub schema_hash: String,
    pub schema: Vec<SchemaEntry>,
    pub migrations: Vec<MigrationBackup>,
    pub table_counts: Vec<TableCount>,
}

pub fn export_manifest<E: SqlExecutor>(
    executor: &E,
) -> Result<ReplicaDbExportManifestRs, SqlError> {
    let schema = load_schema(executor)?;
    let migrations = export_migrations();
    let table_counts = load_table_counts(executor, &schema)?;
    let schema_hash = schema_hash(&schema)?;
    Ok(ReplicaDbExportManifestRs {
        export_version: REPLICA_DB_EXPORT_VERSION.to_string(),
        replica_db_version: REPLICA_DB_VERSION.to_string(),
        backup_format_version: DATABASE_BACKUP_VERSION.to_string(),
        schema_hash,
        schema,
        migrations,
        table_counts,
    })
}

fn load_table_counts<E: SqlExecutor>(
    executor: &E,
    schema: &[SchemaEntry],
) -> Result<Vec<TableCount>, SqlError> {
    #[derive(Deserialize)]
    struct CountRow {
        count: u64,
    }
    let mut counts = Vec::new();
    for entry in schema.iter().filter(|s| s.object_type == "table") {
        let sql = format!(
            "select count(1) as count from {}",
            escape_identifier(&entry.name)
        );
        let json = executor.query_raw(&sql, "[]")?;
        let rows: Vec<CountRow> = utils::parse_json(&json)?;
        let row_count = rows.first().map(|row| row.count).unwrap_or(0);
        counts.push(TableCount {
            name: entry.name.clone(),
            row_count,
        });
    }
    Ok(counts)
}

fn schema_hash(schema: &[SchemaEntry]) -> Result<String, SqlError> {
    let json = serde_json::to_string(schema).map_err(SqlError::from)?;
    let mut hasher = Sha256::new();
    hasher.update(json.as_bytes());
    Ok(hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use radroots_sql_core::ExecOutcome;

    struct MockExecutor {
        query_rules: Vec<(String, String)>,
        fail_query_contains: Option<String>,
    }

    impl MockExecutor {
        fn new(query_rules: Vec<(String, String)>, fail_query_contains: Option<String>) -> Self {
            Self {
                query_rules,
                fail_query_contains,
            }
        }
    }

    impl SqlExecutor for MockExecutor {
        fn exec(&self, _sql: &str, _params_json: &str) -> Result<ExecOutcome, SqlError> {
            Ok(ExecOutcome {
                changes: 1,
                last_insert_id: 1,
            })
        }

        fn query_raw(&self, sql: &str, _params_json: &str) -> Result<String, SqlError> {
            if let Some(needle) = &self.fail_query_contains {
                if sql.contains(needle) {
                    return Err(SqlError::InvalidQuery(String::from("forced query failure")));
                }
            }
            for (needle, response) in &self.query_rules {
                if sql.contains(needle) {
                    return Ok(response.clone());
                }
            }
            Ok(String::from("[]"))
        }

        fn begin(&self) -> Result<(), SqlError> {
            Ok(())
        }

        fn commit(&self) -> Result<(), SqlError> {
            Ok(())
        }

        fn rollback(&self) -> Result<(), SqlError> {
            Ok(())
        }
    }

    #[test]
    fn export_manifest_propagates_schema_query_errors() {
        let executor = MockExecutor::new(
            Vec::new(),
            Some(String::from(
                "select type, name, tbl_name as table_name, sql from sqlite_master",
            )),
        );
        let err = export_manifest(&executor).expect_err("export should fail");
        assert!(matches!(err, SqlError::InvalidQuery(_)));
    }

    #[test]
    fn export_manifest_propagates_table_count_query_errors() {
        let schema_rows = serde_json::json!([
            {
                "type": "table",
                "name": "tb_a",
                "table_name": "tb_a",
                "sql": "CREATE TABLE tb_a (id TEXT);"
            }
        ])
        .to_string();
        let executor = MockExecutor::new(
            vec![(
                String::from("select type, name, tbl_name as table_name, sql from sqlite_master"),
                schema_rows,
            )],
            Some(String::from("select count(1) as count from \"tb_a\"")),
        );
        let err = export_manifest(&executor).expect_err("export should fail");
        assert!(matches!(err, SqlError::InvalidQuery(_)));
    }

    #[test]
    fn export_manifest_defaults_missing_count_row_to_zero() {
        let schema_rows = serde_json::json!([
            {
                "type": "table",
                "name": "tb_a",
                "table_name": "tb_a",
                "sql": "CREATE TABLE tb_a (id TEXT);"
            }
        ])
        .to_string();
        let executor = MockExecutor::new(
            vec![
                (
                    String::from("select type, name, tbl_name as table_name, sql from sqlite_master"),
                    schema_rows,
                ),
                (String::from("select count(1) as count from \"tb_a\""), String::from("[]")),
            ],
            None,
        );
        let manifest = export_manifest(&executor).expect("export should succeed");
        assert_eq!(manifest.table_counts.len(), 1);
        assert_eq!(manifest.table_counts[0].name, "tb_a");
        assert_eq!(manifest.table_counts[0].row_count, 0);
    }

    #[test]
    fn mock_executor_trait_and_query_paths_are_covered() {
        let executor = MockExecutor::new(
            vec![(String::from("select 1"), String::from("[{\"count\":1}]"))],
            None,
        );
        let outcome = executor.exec("select 1", "[]").expect("exec");
        assert_eq!(outcome.changes, 1);
        assert_eq!(outcome.last_insert_id, 1);

        executor.begin().expect("begin");
        executor.commit().expect("commit");
        executor.rollback().expect("rollback");

        let matched = executor.query_raw("select 1", "[]").expect("matched query");
        assert_eq!(matched, "[{\"count\":1}]");
        let fallback = executor.query_raw("select 2", "[]").expect("fallback query");
        assert_eq!(fallback, "[]");

        let failing = MockExecutor::new(Vec::new(), Some(String::from("select fail")));
        let err = failing
            .query_raw("select fail", "[]")
            .expect_err("query should fail");
        assert!(matches!(err, SqlError::InvalidQuery(_)));
    }
}
