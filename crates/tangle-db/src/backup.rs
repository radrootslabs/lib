use radroots_sql_core::{SqlExecutor, error::SqlError, utils};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, HashMap};

pub const DATABASE_BACKUP_VERSION: &str = "1.0.0";
pub const TANGLE_DB_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaEntry {
    pub object_type: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub table_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sql: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableData {
    pub name: String,
    pub rows: Vec<Map<String, Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationBackup {
    pub name: String,
    pub up_sql: String,
    pub down_sql: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseBackup {
    pub format_version: String,
    pub tangle_db_version: String,
    pub schema: Vec<SchemaEntry>,
    pub migrations: Vec<MigrationBackup>,
    pub data: Vec<TableData>,
}

pub fn export_database_backup<E: SqlExecutor>(executor: &E) -> Result<DatabaseBackup, SqlError> {
    let schema = load_schema(executor)?;
    let data = read_tables_for_backup(executor, &schema)?;
    let migrations = export_migrations();
    Ok(DatabaseBackup {
        format_version: DATABASE_BACKUP_VERSION.to_string(),
        tangle_db_version: TANGLE_DB_VERSION.to_string(),
        schema,
        migrations,
        data,
    })
}

pub fn export_database_backup_json<E: SqlExecutor>(executor: &E) -> Result<String, SqlError> {
    let backup = export_database_backup(executor)?;
    serde_json::to_string(&backup).map_err(SqlError::from)
}

pub fn restore_database_backup<E: SqlExecutor>(
    executor: &E,
    backup: &DatabaseBackup,
) -> Result<(), SqlError> {
    validate_backup_version(backup)?;
    executor.exec("PRAGMA foreign_keys = OFF;", "[]")?;
    executor.begin()?;
    let result = (|| {
        drop_existing_objects(executor)?;
        create_schema_from_backup(executor, &backup.schema)?;
        insert_rows_from_backup(executor, backup)?;
        Ok(())
    })();

    match result {
        Ok(()) => {
            executor.commit()?;
            let _ = executor.exec("PRAGMA foreign_keys = ON;", "[]")?;
            Ok(())
        }
        Err(err) => {
            let _ = executor.rollback();
            let _ = executor.exec("PRAGMA foreign_keys = ON;", "[]");
            Err(err)
        }
    }
}

pub fn restore_database_backup_json<E: SqlExecutor>(
    executor: &E,
    backup_json: &str,
) -> Result<(), SqlError> {
    let backup: DatabaseBackup = serde_json::from_str(backup_json).map_err(SqlError::from)?;
    restore_database_backup(executor, &backup)
}

fn drop_existing_objects<E: SqlExecutor>(executor: &E) -> Result<(), SqlError> {
    #[derive(Deserialize)]
    struct MasterRow {
        #[serde(rename = "type")]
        object_type: Option<String>,
        name: Option<String>,
    }
    let query = "select type, name from sqlite_master where name not like 'sqlite_%'";
    let json = executor.query_raw(query, "[]")?;
    let rows: Vec<MasterRow> = utils::parse_json(&json)?;

    let mut groups: HashMap<String, Vec<String>> = HashMap::new();
    for row in rows.into_iter() {
        let obj_type = row.object_type.unwrap_or_default();
        let name = match row.name {
            Some(n) => n,
            None => continue,
        };
        groups.entry(obj_type).or_default().push(name);
    }

    for object_type in ["trigger", "view", "index", "table"] {
        if let Some(names) = groups.get(object_type) {
            for name in names {
                let stmt = match object_type {
                    "trigger" => format!("DROP TRIGGER IF EXISTS {};", escape_identifier(name)),
                    "view" => format!("DROP VIEW IF EXISTS {};", escape_identifier(name)),
                    "index" => format!("DROP INDEX IF EXISTS {};", escape_identifier(name)),
                    _ => format!("DROP TABLE IF EXISTS {};", escape_identifier(name)),
                };
                let _ = executor.exec(&stmt, "[]")?;
            }
        }
    }
    Ok(())
}

fn create_schema_from_backup<E: SqlExecutor>(
    executor: &E,
    schema: &[SchemaEntry],
) -> Result<(), SqlError> {
    for entry in schema.iter().filter(|s| s.object_type == "table") {
        if let Some(sql) = &entry.sql {
            executor.exec(sql, "[]")?;
        }
    }
    for entry in schema.iter().filter(|s| s.object_type != "table") {
        if let Some(sql) = &entry.sql {
            executor.exec(sql, "[]")?;
        }
    }
    Ok(())
}

fn insert_rows_from_backup<E: SqlExecutor>(
    executor: &E,
    backup: &DatabaseBackup,
) -> Result<(), SqlError> {
    let mut row_sources: HashMap<&str, &Vec<Map<String, Value>>> = HashMap::new();
    for table in &backup.data {
        row_sources.insert(table.name.as_str(), &table.rows);
    }
    for entry in backup.schema.iter().filter(|s| s.object_type == "table") {
        let rows = match row_sources.get(entry.name.as_str()) {
            Some(r) => *r,
            None => continue,
        };
        for row in rows {
            insert_row(executor, &entry.name, row)?;
        }
    }
    Ok(())
}

fn insert_row<E: SqlExecutor>(
    executor: &E,
    table: &str,
    row: &Map<String, Value>,
) -> Result<(), SqlError> {
    if row.is_empty() {
        return Ok(());
    }

    let mut cols: BTreeMap<String, &Value> = BTreeMap::new();
    for (k, v) in row {
        cols.insert(k.clone(), v);
    }

    let column_names: Vec<String> = cols.keys().cloned().collect();
    let placeholders = (0..column_names.len())
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "INSERT INTO {} ({}) VALUES ({});",
        escape_identifier(table),
        column_names
            .iter()
            .map(|c| escape_identifier(c))
            .collect::<Vec<_>>()
            .join(","),
        placeholders
    );

    let binds: Vec<Value> = cols.values().map(|v| utils::to_db_bind_value(*v)).collect();
    let params_json = serde_json::to_string(&binds).map_err(SqlError::from)?;
    executor.exec(&sql, &params_json)?;
    Ok(())
}

pub(crate) fn load_schema<E: SqlExecutor>(executor: &E) -> Result<Vec<SchemaEntry>, SqlError> {
    let query = "select type, name, tbl_name as table_name, sql from sqlite_master where name not like 'sqlite_%' order by type, name";
    let json = executor.query_raw(query, "[]")?;
    #[derive(Deserialize)]
    struct RawSchema {
        #[serde(rename = "type")]
        object_type: Option<String>,
        name: Option<String>,
        table_name: Option<String>,
        sql: Option<String>,
    }
    let rows: Vec<RawSchema> = utils::parse_json(&json)?;
    Ok(rows
        .into_iter()
        .filter_map(|row| {
            let name = row.name?;
            let object_type = row.object_type.unwrap_or_default();
            Some(SchemaEntry {
                object_type,
                name,
                table_name: row.table_name,
                sql: row.sql,
            })
        })
        .collect())
}

pub(crate) fn export_migrations() -> Vec<MigrationBackup> {
    crate::migrations::MIGRATIONS
        .iter()
        .map(|m| MigrationBackup {
            name: m.name.to_string(),
            up_sql: m.up_sql.to_string(),
            down_sql: m.down_sql.to_string(),
        })
        .collect()
}

fn read_tables_for_backup<E: SqlExecutor>(
    executor: &E,
    schema: &[SchemaEntry],
) -> Result<Vec<TableData>, SqlError> {
    let mut data = Vec::new();
    for entry in schema.iter().filter(|s| s.object_type == "table") {
        let select_sql = format!("SELECT * FROM {};", escape_identifier(&entry.name));
        let json = executor.query_raw(&select_sql, "[]")?;
        let rows: Vec<Map<String, Value>> = utils::parse_json(&json)?;
        data.push(TableData {
            name: entry.name.clone(),
            rows,
        });
    }
    Ok(data)
}

pub(crate) fn escape_identifier(name: &str) -> String {
    let mut escaped = String::with_capacity(name.len() + 2);
    escaped.push('"');
    for c in name.chars() {
        if c == '"' {
            escaped.push('"');
        }
        escaped.push(c);
    }
    escaped.push('"');
    escaped
}

fn validate_backup_version(backup: &DatabaseBackup) -> Result<(), SqlError> {
    if backup.format_version != DATABASE_BACKUP_VERSION {
        return Err(SqlError::InvalidArgument(format!(
            "unsupported backup format {}, expected {}",
            backup.format_version, DATABASE_BACKUP_VERSION
        )));
    }
    if backup.tangle_db_version != TANGLE_DB_VERSION {
        return Err(SqlError::InvalidArgument(format!(
            "unsupported tangle-db version {}, expected {}",
            backup.tangle_db_version, TANGLE_DB_VERSION
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use radroots_sql_core::ExecOutcome;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct MockExecutor {
        query_rules: Vec<(String, String)>,
        fail_exec_contains: Option<String>,
        exec_calls: Mutex<Vec<String>>,
        begin_calls: AtomicUsize,
        commit_calls: AtomicUsize,
        rollback_calls: AtomicUsize,
    }

    impl MockExecutor {
        fn new(query_rules: Vec<(String, String)>, fail_exec_contains: Option<String>) -> Self {
            Self {
                query_rules,
                fail_exec_contains,
                exec_calls: Mutex::new(Vec::new()),
                begin_calls: AtomicUsize::new(0),
                commit_calls: AtomicUsize::new(0),
                rollback_calls: AtomicUsize::new(0),
            }
        }

        fn exec_calls(&self) -> Vec<String> {
            self.exec_calls.lock().expect("exec calls lock").clone()
        }

        fn begin_count(&self) -> usize {
            self.begin_calls.load(Ordering::SeqCst)
        }

        fn commit_count(&self) -> usize {
            self.commit_calls.load(Ordering::SeqCst)
        }

        fn rollback_count(&self) -> usize {
            self.rollback_calls.load(Ordering::SeqCst)
        }
    }

    impl SqlExecutor for MockExecutor {
        fn exec(&self, sql: &str, _params_json: &str) -> Result<ExecOutcome, SqlError> {
            self.exec_calls
                .lock()
                .expect("exec calls lock")
                .push(sql.to_string());
            if let Some(needle) = &self.fail_exec_contains {
                if sql.contains(needle) {
                    return Err(SqlError::InvalidQuery(String::from("forced exec failure")));
                }
            }
            Ok(ExecOutcome {
                changes: 1,
                last_insert_id: 1,
            })
        }

        fn query_raw(&self, sql: &str, _params_json: &str) -> Result<String, SqlError> {
            for (needle, response) in &self.query_rules {
                if sql.contains(needle) {
                    return Ok(response.clone());
                }
            }
            Ok(String::from("[]"))
        }

        fn begin(&self) -> Result<(), SqlError> {
            self.begin_calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        fn commit(&self) -> Result<(), SqlError> {
            self.commit_calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        fn rollback(&self) -> Result<(), SqlError> {
            self.rollback_calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    fn backup_with_versions(format_version: &str, tangle_db_version: &str) -> DatabaseBackup {
        DatabaseBackup {
            format_version: format_version.to_string(),
            tangle_db_version: tangle_db_version.to_string(),
            schema: Vec::new(),
            migrations: Vec::new(),
            data: Vec::new(),
        }
    }

    #[test]
    fn restore_database_backup_rolls_back_when_exec_fails() {
        let executor = MockExecutor::new(
            vec![(
                String::from("select type, name from sqlite_master"),
                String::from("[]"),
            )],
            Some(String::from("CREATE TABLE fail_table")),
        );
        let backup = DatabaseBackup {
            format_version: DATABASE_BACKUP_VERSION.to_string(),
            tangle_db_version: TANGLE_DB_VERSION.to_string(),
            schema: vec![SchemaEntry {
                object_type: String::from("table"),
                name: String::from("fail_table"),
                table_name: Some(String::from("fail_table")),
                sql: Some(String::from("CREATE TABLE fail_table (id TEXT);")),
            }],
            migrations: Vec::new(),
            data: Vec::new(),
        };

        let err = restore_database_backup(&executor, &backup).expect_err("restore should fail");
        assert!(matches!(err, SqlError::InvalidQuery(_)));
        assert_eq!(executor.begin_count(), 1);
        assert_eq!(executor.commit_count(), 0);
        assert_eq!(executor.rollback_count(), 1);
        let calls = executor.exec_calls();
        assert!(
            calls
                .iter()
                .any(|sql| sql.contains("PRAGMA foreign_keys = OFF"))
        );
        assert!(
            calls
                .iter()
                .any(|sql| sql.contains("PRAGMA foreign_keys = ON"))
        );
    }

    #[test]
    fn drop_existing_objects_skips_rows_without_name() {
        let master_rows = serde_json::json!([
            { "type": "trigger", "name": "tg_a" },
            { "type": "view", "name": "vw_a" },
            { "type": "index", "name": "ix_a" },
            { "type": "table", "name": "tb_a" },
            { "type": "table", "name": null }
        ])
        .to_string();
        let executor = MockExecutor::new(
            vec![(
                String::from("select type, name from sqlite_master"),
                master_rows,
            )],
            None,
        );

        drop_existing_objects(&executor).expect("drop existing objects");
        let calls = executor.exec_calls();
        assert!(
            calls
                .iter()
                .any(|sql| sql.contains("DROP TRIGGER IF EXISTS \"tg_a\";"))
        );
        assert!(
            calls
                .iter()
                .any(|sql| sql.contains("DROP VIEW IF EXISTS \"vw_a\";"))
        );
        assert!(
            calls
                .iter()
                .any(|sql| sql.contains("DROP INDEX IF EXISTS \"ix_a\";"))
        );
        assert!(
            calls
                .iter()
                .any(|sql| sql.contains("DROP TABLE IF EXISTS \"tb_a\";"))
        );
    }

    #[test]
    fn create_schema_from_backup_executes_table_and_non_table_sql() {
        let executor = MockExecutor::new(Vec::new(), None);
        let schema = vec![
            SchemaEntry {
                object_type: String::from("table"),
                name: String::from("tb_a"),
                table_name: Some(String::from("tb_a")),
                sql: Some(String::from("CREATE TABLE tb_a (id TEXT);")),
            },
            SchemaEntry {
                object_type: String::from("table"),
                name: String::from("tb_b"),
                table_name: Some(String::from("tb_b")),
                sql: None,
            },
            SchemaEntry {
                object_type: String::from("view"),
                name: String::from("vw_a"),
                table_name: Some(String::from("vw_a")),
                sql: Some(String::from("CREATE VIEW vw_a AS SELECT 1;")),
            },
            SchemaEntry {
                object_type: String::from("index"),
                name: String::from("ix_a"),
                table_name: Some(String::from("ix_a")),
                sql: None,
            },
        ];

        create_schema_from_backup(&executor, &schema).expect("create schema from backup");
        let calls = executor.exec_calls();
        assert!(
            calls
                .iter()
                .any(|sql| sql == "CREATE TABLE tb_a (id TEXT);")
        );
        assert!(
            calls
                .iter()
                .any(|sql| sql == "CREATE VIEW vw_a AS SELECT 1;")
        );
        assert_eq!(calls.len(), 2);
    }

    #[test]
    fn insert_rows_from_backup_skips_missing_data_and_empty_rows() {
        let executor = MockExecutor::new(Vec::new(), None);
        let mut row = Map::new();
        row.insert(String::from("co\"l"), Value::from(7));
        let backup = DatabaseBackup {
            format_version: DATABASE_BACKUP_VERSION.to_string(),
            tangle_db_version: TANGLE_DB_VERSION.to_string(),
            schema: vec![
                SchemaEntry {
                    object_type: String::from("table"),
                    name: String::from("tb_a"),
                    table_name: Some(String::from("tb_a")),
                    sql: Some(String::from("CREATE TABLE tb_a (id TEXT);")),
                },
                SchemaEntry {
                    object_type: String::from("table"),
                    name: String::from("tb_b"),
                    table_name: Some(String::from("tb_b")),
                    sql: Some(String::from("CREATE TABLE tb_b (id TEXT);")),
                },
            ],
            migrations: Vec::new(),
            data: vec![TableData {
                name: String::from("tb_a"),
                rows: vec![row],
            }],
        };

        insert_rows_from_backup(&executor, &backup).expect("insert rows from backup");
        let calls_after_insert = executor.exec_calls();
        assert!(
            calls_after_insert
                .iter()
                .any(|sql| sql.contains("INSERT INTO \"tb_a\" (\"co\"\"l\") VALUES (?);"))
        );
        assert!(
            !calls_after_insert
                .iter()
                .any(|sql| sql.contains("\"tb_b\""))
        );

        let empty_row = Map::new();
        insert_row(&executor, "tb_a", &empty_row).expect("insert empty row");
        assert_eq!(executor.exec_calls().len(), calls_after_insert.len());
        assert_eq!(escape_identifier("a\"b"), "\"a\"\"b\"");
    }

    #[test]
    fn load_schema_filters_rows_without_name() {
        let schema_rows = serde_json::json!([
            { "type": "table", "name": null, "table_name": "tb_a", "sql": "CREATE TABLE tb_a (id TEXT);" },
            { "type": "view", "name": "vw_a", "table_name": "vw_a", "sql": "CREATE VIEW vw_a AS SELECT 1;" }
        ])
        .to_string();
        let executor = MockExecutor::new(
            vec![(
                String::from("select type, name, tbl_name as table_name, sql from sqlite_master"),
                schema_rows,
            )],
            None,
        );

        let rows = load_schema(&executor).expect("load schema");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "vw_a");
        assert_eq!(rows[0].object_type, "view");
    }

    #[test]
    fn validate_backup_version_rejects_invalid_versions() {
        let wrong_format = backup_with_versions("0.0.1", TANGLE_DB_VERSION);
        let err = validate_backup_version(&wrong_format).expect_err("format version must fail");
        assert!(matches!(err, SqlError::InvalidArgument(_)));

        let wrong_db_version = backup_with_versions(DATABASE_BACKUP_VERSION, "0.0.0");
        let err = validate_backup_version(&wrong_db_version).expect_err("db version must fail");
        assert!(matches!(err, SqlError::InvalidArgument(_)));
    }

    #[test]
    fn restore_database_backup_commits_on_success_and_query_fallback_works() {
        let executor = MockExecutor::new(
            vec![(
                String::from("select type, name from sqlite_master"),
                String::from("[]"),
            )],
            None,
        );
        let backup = backup_with_versions(DATABASE_BACKUP_VERSION, TANGLE_DB_VERSION);

        let matched = executor
            .query_raw("select type, name from sqlite_master", "[]")
            .expect("query match");
        assert_eq!(matched, "[]");

        let fallback = executor
            .query_raw("select 1", "[]")
            .expect("query fallback");
        assert_eq!(fallback, "[]");

        restore_database_backup(&executor, &backup).expect("restore should succeed");
        assert_eq!(executor.begin_count(), 1);
        assert_eq!(executor.commit_count(), 1);
        assert_eq!(executor.rollback_count(), 0);
    }

    #[test]
    fn restore_database_backup_json_rejects_invalid_json() {
        let executor = MockExecutor::new(Vec::new(), None);
        let err = restore_database_backup_json(&executor, "{")
            .expect_err("invalid backup json should fail");
        assert!(matches!(err, SqlError::SerializationError(_)));
    }
}
