use std::collections::VecDeque;
use std::sync::Mutex;

use radroots_replica_db::backup::{
    DATABASE_BACKUP_VERSION, DatabaseBackup, REPLICA_DB_VERSION, SchemaEntry, TableData,
    export_database_backup, export_database_backup_json, restore_database_backup,
};
use radroots_replica_db::export::export_manifest;
use radroots_replica_db::{ExecOutcome, SqlError, SqlExecutor};
use serde_json::{Map, Value, json};

struct PatternExecutor {
    query_rules: Vec<(String, String)>,
    fail_query_contains: Option<String>,
    fail_exec_contains: Option<String>,
    fail_begin: bool,
    fail_commit: bool,
    begin_count: Mutex<usize>,
    commit_count: Mutex<usize>,
    rollback_count: Mutex<usize>,
    query_queue: Mutex<VecDeque<Result<String, SqlError>>>,
    exec_queue: Mutex<VecDeque<Result<ExecOutcome, SqlError>>>,
}

impl PatternExecutor {
    fn new() -> Self {
        Self {
            query_rules: Vec::new(),
            fail_query_contains: None,
            fail_exec_contains: None,
            fail_begin: false,
            fail_commit: false,
            begin_count: Mutex::new(0),
            commit_count: Mutex::new(0),
            rollback_count: Mutex::new(0),
            query_queue: Mutex::new(VecDeque::new()),
            exec_queue: Mutex::new(VecDeque::new()),
        }
    }

    fn with_query_rule(mut self, needle: &str, response: &str) -> Self {
        self.query_rules
            .push((needle.to_string(), response.to_string()));
        self
    }

    fn with_query_failure(mut self, needle: &str) -> Self {
        self.fail_query_contains = Some(needle.to_string());
        self
    }

    fn with_exec_failure(mut self, needle: &str) -> Self {
        self.fail_exec_contains = Some(needle.to_string());
        self
    }

    fn with_begin_failure(mut self) -> Self {
        self.fail_begin = true;
        self
    }

    fn with_commit_failure(mut self) -> Self {
        self.fail_commit = true;
        self
    }
}

impl SqlExecutor for PatternExecutor {
    fn exec(&self, sql: &str, _params_json: &str) -> Result<ExecOutcome, SqlError> {
        if let Some(result) = self.exec_queue.lock().expect("exec queue lock").pop_front() {
            return result;
        }
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
        if let Some(result) = self
            .query_queue
            .lock()
            .expect("query queue lock")
            .pop_front()
        {
            return result;
        }
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
        *self.begin_count.lock().expect("begin count lock") += 1;
        if self.fail_begin {
            return Err(SqlError::InvalidQuery(String::from("forced begin failure")));
        }
        Ok(())
    }

    fn commit(&self) -> Result<(), SqlError> {
        *self.commit_count.lock().expect("commit count lock") += 1;
        if self.fail_commit {
            return Err(SqlError::InvalidQuery(String::from(
                "forced commit failure",
            )));
        }
        Ok(())
    }

    fn rollback(&self) -> Result<(), SqlError> {
        *self.rollback_count.lock().expect("rollback count lock") += 1;
        Ok(())
    }
}

fn assert_sql_error_code<T: core::fmt::Debug>(result: Result<T, SqlError>, code: &str) {
    let err = result.unwrap_err();
    assert_eq!(err.code(), code);
}

fn backup_with_versions(format_version: &str, replica_db_version: &str) -> DatabaseBackup {
    DatabaseBackup {
        format_version: format_version.to_string(),
        replica_db_version: replica_db_version.to_string(),
        schema: Vec::new(),
        migrations: Vec::new(),
        data: Vec::new(),
    }
}

#[test]
fn backup_public_api_error_paths_cover_library_instantiations() {
    let schema_query = "select type, name, tbl_name as table_name, sql from sqlite_master";

    let executor = PatternExecutor::new().with_query_failure(schema_query);
    assert_sql_error_code(export_database_backup(&executor), "ERR_INVALID_QUERY");

    let schema_rows = json!([
        {
            "type": "table",
            "name": "tb_a",
            "table_name": "tb_a",
            "sql": "CREATE TABLE tb_a (id TEXT);"
        }
    ])
    .to_string();
    let executor = PatternExecutor::new()
        .with_query_rule(schema_query, &schema_rows)
        .with_query_failure("SELECT * FROM \"tb_a\";");
    assert_sql_error_code(export_database_backup(&executor), "ERR_INVALID_QUERY");

    let executor = PatternExecutor::new().with_query_failure(schema_query);
    assert_sql_error_code(export_database_backup_json(&executor), "ERR_INVALID_QUERY");

    let executor = PatternExecutor::new().with_query_rule(schema_query, "[]");
    let backup_json = export_database_backup_json(&executor).expect("backup json success");
    assert!(backup_json.contains("\"schema\":[]"));

    let executor = PatternExecutor::new();
    let backup = backup_with_versions("0.0.1", REPLICA_DB_VERSION);
    assert_sql_error_code(
        restore_database_backup(&executor, &backup),
        "ERR_INVALID_ARGUMENT",
    );

    let backup = backup_with_versions(DATABASE_BACKUP_VERSION, REPLICA_DB_VERSION);
    let executor = PatternExecutor::new().with_exec_failure("PRAGMA foreign_keys = OFF;");
    assert_sql_error_code(
        restore_database_backup(&executor, &backup),
        "ERR_INVALID_QUERY",
    );

    let executor = PatternExecutor::new().with_begin_failure();
    assert_sql_error_code(
        restore_database_backup(&executor, &backup),
        "ERR_INVALID_QUERY",
    );

    let executor =
        PatternExecutor::new().with_query_failure("select type, name from sqlite_master");
    assert_sql_error_code(
        restore_database_backup(&executor, &backup),
        "ERR_INVALID_QUERY",
    );

    let executor = PatternExecutor::new().with_commit_failure();
    assert_sql_error_code(
        restore_database_backup(&executor, &backup),
        "ERR_INVALID_QUERY",
    );

    let executor = PatternExecutor::new().with_exec_failure("PRAGMA foreign_keys = ON;");
    assert_sql_error_code(
        restore_database_backup(&executor, &backup),
        "ERR_INVALID_QUERY",
    );
}

#[test]
fn backup_public_api_insert_and_parse_failures_cover_library_instantiations() {
    let backup = DatabaseBackup {
        format_version: DATABASE_BACKUP_VERSION.to_string(),
        replica_db_version: REPLICA_DB_VERSION.to_string(),
        schema: vec![SchemaEntry {
            object_type: String::from("table"),
            name: String::from("tb_a"),
            table_name: Some(String::from("tb_a")),
            sql: Some(String::from("CREATE TABLE tb_a (id TEXT);")),
        }],
        migrations: Vec::new(),
        data: vec![TableData {
            name: String::from("tb_a"),
            rows: vec![{
                let mut row = Map::new();
                row.insert(String::from("id"), Value::from("1"));
                row
            }],
        }],
    };

    let executor = PatternExecutor::new().with_exec_failure("INSERT INTO \"tb_a\"");
    assert_sql_error_code(
        restore_database_backup(&executor, &backup),
        "ERR_INVALID_QUERY",
    );

    let schema_query = "select type, name, tbl_name as table_name, sql from sqlite_master";
    let executor = PatternExecutor::new().with_query_rule(schema_query, "{");
    assert_sql_error_code(export_database_backup(&executor), "ERR_SERIALIZATION");

    let schema_rows = json!([
        {
            "type": "table",
            "name": "tb_a",
            "table_name": "tb_a",
            "sql": "CREATE TABLE tb_a (id TEXT);"
        }
    ])
    .to_string();
    let executor = PatternExecutor::new()
        .with_query_rule(schema_query, &schema_rows)
        .with_query_rule("SELECT * FROM \"tb_a\";", "{");
    assert_sql_error_code(export_database_backup(&executor), "ERR_SERIALIZATION");

    let null_name_rows = json!([
        {
            "type": "table",
            "name": null,
            "table_name": "tb_a",
            "sql": "CREATE TABLE tb_a (id TEXT);"
        }
    ])
    .to_string();
    let executor = PatternExecutor::new().with_query_rule(schema_query, &null_name_rows);
    let backup = export_database_backup(&executor).expect("backup with null-name schema rows");
    assert!(backup.schema.is_empty());
}

#[test]
fn export_manifest_public_api_error_paths_cover_library_instantiations() {
    let schema_query = "select type, name, tbl_name as table_name, sql from sqlite_master";

    let executor = PatternExecutor::new().with_query_failure(schema_query);
    assert_sql_error_code(export_manifest(&executor), "ERR_INVALID_QUERY");

    let schema_rows = json!([
        {
            "type": "table",
            "name": "tb_a",
            "table_name": "tb_a",
            "sql": "CREATE TABLE tb_a (id TEXT);"
        }
    ])
    .to_string();

    let executor = PatternExecutor::new()
        .with_query_rule(schema_query, &schema_rows)
        .with_query_failure("select count(1) as count from \"tb_a\"");
    assert_sql_error_code(export_manifest(&executor), "ERR_INVALID_QUERY");

    let executor = PatternExecutor::new()
        .with_query_rule(schema_query, &schema_rows)
        .with_query_rule("select count(1) as count from \"tb_a\"", "{");
    assert_sql_error_code(export_manifest(&executor), "ERR_SERIALIZATION");

    let executor = PatternExecutor::new()
        .with_query_rule(schema_query, &schema_rows)
        .with_query_rule("select count(1) as count from \"tb_a\"", "[]");
    let manifest = export_manifest(&executor).expect("manifest success");
    assert_eq!(manifest.table_counts.len(), 1);
}
