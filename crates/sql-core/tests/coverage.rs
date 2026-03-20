use radroots_sql_core::error::SqlError;
use radroots_sql_core::migrations::{Migration, migrations_run_all_down, migrations_run_all_up};
use radroots_sql_core::utils::{
    build_insert_query_with_meta, build_select_query_with_meta, build_where_clause_eq, parse_json,
    parse_query_value, time_created_on, to_db_bind_value, to_object_map, to_params_json,
    to_partial_object_map, uuidv4, with_transaction,
};
use radroots_sql_core::{ExecOutcome, SqlExecutor};
#[cfg(feature = "native")]
use radroots_sql_core::SqliteExecutor;
use serde::ser::{SerializeMap, SerializeSeq};
use serde::{Deserialize, Serialize, Serializer};
use serde_json::{Map, Value, json};
use std::collections::BTreeSet;
use std::sync::Mutex;

#[derive(Debug, Clone, PartialEq)]
struct ExecutorSnapshot {
    exec_sql: Vec<String>,
    begin_count: usize,
    commit_count: usize,
    rollback_count: usize,
    applied: BTreeSet<String>,
}

#[derive(Debug, Clone, Default)]
struct ExecutorState {
    exec_sql: Vec<String>,
    begin_count: usize,
    commit_count: usize,
    rollback_count: usize,
    applied: BTreeSet<String>,
    fail_begin: bool,
    fail_commit: bool,
    fail_rollback: bool,
    fail_sql_contains: Option<String>,
    query_override: Option<Result<String, SqlError>>,
}

#[derive(Debug, Default)]
struct MockExecutor {
    state: Mutex<ExecutorState>,
}

impl MockExecutor {
    fn new() -> Self {
        Self::default()
    }

    fn with_fail_sql(mut self, needle: &str) -> Self {
        let state = self.state.get_mut().expect("state");
        state.fail_sql_contains = Some(needle.to_string());
        self
    }

    fn set_fail_begin(&self, value: bool) {
        let mut state = self.state.lock().expect("state");
        state.fail_begin = value;
    }

    fn set_fail_commit(&self, value: bool) {
        let mut state = self.state.lock().expect("state");
        state.fail_commit = value;
    }

    fn set_fail_rollback(&self, value: bool) {
        let mut state = self.state.lock().expect("state");
        state.fail_rollback = value;
    }

    fn set_query_override(&self, value: Option<Result<String, SqlError>>) {
        let mut state = self.state.lock().expect("state");
        state.query_override = value;
    }

    fn mark_applied(&self, name: &str) {
        let mut state = self.state.lock().expect("state");
        state.applied.insert(name.to_string());
    }

    fn snapshot(&self) -> ExecutorSnapshot {
        let state = self.state.lock().expect("state");
        ExecutorSnapshot {
            exec_sql: state.exec_sql.clone(),
            begin_count: state.begin_count,
            commit_count: state.commit_count,
            rollback_count: state.rollback_count,
            applied: state.applied.clone(),
        }
    }
}

impl SqlExecutor for MockExecutor {
    fn exec(&self, sql: &str, params_json: &str) -> Result<ExecOutcome, SqlError> {
        let mut state = self.state.lock().expect("state");
        state.exec_sql.push(sql.to_string());
        if let Some(needle) = &state.fail_sql_contains {
            if sql.contains(needle) {
                return Err(SqlError::InvalidQuery(sql.to_string()));
            }
        }

        if sql.contains("insert or ignore into __migrations(name)") {
            let params: Vec<String> =
                serde_json::from_str(params_json).map_err(|err| SqlError::from(err))?;
            if let Some(name) = params.first() {
                state.applied.insert(name.clone());
            }
        }

        if sql.contains("delete from __migrations where name = ?") {
            let params: Vec<String> =
                serde_json::from_str(params_json).map_err(|err| SqlError::from(err))?;
            if let Some(name) = params.first() {
                state.applied.remove(name);
            }
        }

        Ok(ExecOutcome {
            changes: 1,
            last_insert_id: 11,
        })
    }

    fn query_raw(&self, _sql: &str, params_json: &str) -> Result<String, SqlError> {
        let state = self.state.lock().expect("state");
        if let Some(override_value) = &state.query_override {
            return override_value.clone();
        }
        let params: Vec<String> =
            serde_json::from_str(params_json).map_err(|err| SqlError::from(err))?;
        let Some(name) = params.first() else {
            return Ok(String::new());
        };
        if state.applied.contains(name) {
            Ok(json!([{ "applied": 1 }]).to_string())
        } else {
            Ok("[]".to_string())
        }
    }

    fn begin(&self) -> Result<(), SqlError> {
        let mut state = self.state.lock().expect("state");
        state.begin_count += 1;
        if state.fail_begin {
            return Err(SqlError::Internal);
        }
        Ok(())
    }

    fn commit(&self) -> Result<(), SqlError> {
        let mut state = self.state.lock().expect("state");
        state.commit_count += 1;
        if state.fail_commit {
            return Err(SqlError::Internal);
        }
        Ok(())
    }

    fn rollback(&self) -> Result<(), SqlError> {
        let mut state = self.state.lock().expect("state");
        state.rollback_count += 1;
        if state.fail_rollback {
            return Err(SqlError::Internal);
        }
        Ok(())
    }
}

#[test]
fn sql_executor_reference_impl_forwards_all_methods() {
    let exec = MockExecutor::new();
    let exec_ref = &exec;

    let exec_result = <&MockExecutor as SqlExecutor>::exec(&exec_ref, "select 1", "[]")
        .expect("reference exec should forward");
    assert_eq!(exec_result.changes, 1);

    let query_result = <&MockExecutor as SqlExecutor>::query_raw(&exec_ref, "select 1", "[]")
        .expect("reference query should forward");
    assert_eq!(query_result, String::new());

    <&MockExecutor as SqlExecutor>::begin(&exec_ref).expect("reference begin should forward");
    <&MockExecutor as SqlExecutor>::commit(&exec_ref).expect("reference commit should forward");
    <&MockExecutor as SqlExecutor>::rollback(&exec_ref).expect("reference rollback should forward");

    let snapshot = exec.snapshot();
    assert_eq!(snapshot.begin_count, 1);
    assert_eq!(snapshot.commit_count, 1);
    assert_eq!(snapshot.rollback_count, 1);
    assert!(snapshot.exec_sql.iter().any(|sql| sql == "select 1"));
}

#[cfg(feature = "native")]
#[test]
fn sqlite_executor_exec_runs_multi_statement_batches_without_params() {
    let exec = SqliteExecutor::open_memory().expect("open sqlite memory");

    let outcome = exec
        .exec(
            "create table demo (id integer primary key, name text not null);\ncreate unique index demo_name_idx on demo(name);",
            "[]",
        )
        .expect("multi-statement batch should succeed");
    assert_eq!(outcome.changes, 0);

    let insert = exec
        .exec("insert into demo(name) values ('alpha')", "[]")
        .expect("insert should succeed");
    assert_eq!(insert.changes, 1);

    let index_rows = exec
        .query_raw(
            "select name from sqlite_master where type = 'index' and name = 'demo_name_idx'",
            "[]",
        )
        .expect("index metadata query should succeed");
    assert_eq!(index_rows, json!([{ "name": "demo_name_idx" }]).to_string());
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct Payload {
    id: String,
    amount: Option<i64>,
}

#[derive(Debug, Clone, Copy)]
enum FilterMode {
    Object,
    Array,
    Error,
}

#[derive(Debug, Clone)]
struct FilterInput {
    mode: FilterMode,
    id: Option<String>,
    amount: Option<i64>,
}

impl FilterInput {
    fn object(id: Option<&str>, amount: Option<i64>) -> Self {
        Self {
            mode: FilterMode::Object,
            id: id.map(str::to_string),
            amount,
        }
    }

    fn array() -> Self {
        Self {
            mode: FilterMode::Array,
            id: None,
            amount: None,
        }
    }

    fn error() -> Self {
        Self {
            mode: FilterMode::Error,
            id: None,
            amount: None,
        }
    }
}

impl Serialize for FilterInput {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.mode {
            FilterMode::Error => Err(serde::ser::Error::custom("serialize fail")),
            FilterMode::Array => {
                let mut seq = serializer.serialize_seq(Some(2))?;
                seq.serialize_element(&1)?;
                seq.serialize_element(&2)?;
                seq.end()
            }
            FilterMode::Object => {
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry("id", &self.id)?;
                map.serialize_entry("amount", &self.amount)?;
                map.end()
            }
        }
    }
}

#[test]
fn sql_error_code_and_to_json_cover_all_variants() {
    let errors = vec![
        SqlError::InvalidArgument("a".to_string()),
        SqlError::NotFound("b".to_string()),
        SqlError::SerializationError("c".to_string()),
        SqlError::InvalidQuery("d".to_string()),
        SqlError::Internal,
        SqlError::UnsupportedPlatform,
    ];
    let expected = vec![
        "ERR_INVALID_ARGUMENT",
        "ERR_NOT_FOUND",
        "ERR_SERIALIZATION",
        "ERR_INVALID_QUERY",
        "ERR_INTERNAL",
        "ERR_UNSUPPORTED_PLATFORM",
    ];

    for (err, code) in errors.into_iter().zip(expected.into_iter()) {
        assert_eq!(err.code(), code);
        let json_value = err.to_json();
        assert_eq!(json_value.get("code").and_then(|v| v.as_str()), Some(code));
        assert!(json_value.get("message").and_then(|v| v.as_str()).is_some());
    }
}

#[test]
fn parse_json_and_identifiers_work() {
    let parsed: Payload = parse_json(r#"{"id":"p1","amount":3}"#).expect("payload should parse");
    assert_eq!(
        parsed,
        Payload {
            id: "p1".to_string(),
            amount: Some(3),
        }
    );

    let err = parse_json::<Payload>("not-json").expect_err("invalid json should fail");
    assert!(matches!(err, SqlError::SerializationError(_)));

    let first = uuidv4();
    let second = uuidv4();
    assert_ne!(first, second);
    assert_eq!(first.len(), 36);

    let created_on = time_created_on();
    assert!(created_on.ends_with('Z'));
}

#[test]
fn object_map_helpers_cover_success_and_error_paths() {
    let payload = FilterInput::object(Some("row-1"), Some(8));
    let object = to_object_map(payload).expect("to object map");
    assert_eq!(object.get("id"), Some(&Value::String("row-1".to_string())));

    let err = to_object_map(FilterInput::array()).expect_err("array should fail");
    assert!(matches!(err, SqlError::SerializationError(_)));
    let err = to_object_map(FilterInput::error()).expect_err("serialize fail should surface");
    assert!(matches!(err, SqlError::SerializationError(_)));

    let partial =
        to_partial_object_map(FilterInput::object(Some("row-2"), None)).expect("to partial map");
    assert_eq!(partial.get("id"), Some(&Value::String("row-2".to_string())));
    assert!(!partial.contains_key("amount"));

    let err_partial = to_partial_object_map(FilterInput::array()).expect_err("array should fail");
    assert!(matches!(err_partial, SqlError::SerializationError(_)));
    let err_partial =
        to_partial_object_map(FilterInput::error()).expect_err("serialize fail should surface");
    assert!(matches!(err_partial, SqlError::SerializationError(_)));
}

#[test]
fn bind_value_helpers_cover_all_value_paths() {
    assert_eq!(to_db_bind_value(&Value::Bool(true)), Value::from(1));
    assert_eq!(to_db_bind_value(&Value::Bool(false)), Value::from(0));
    assert_eq!(to_db_bind_value(&json!(5_i64)), Value::from(5_u32));
    assert_eq!(to_db_bind_value(&json!(-5_i64)), Value::from(-5_i64));
    assert_eq!(to_db_bind_value(&json!(7.25_f64)), Value::from(7.25_f64));
    assert_eq!(
        to_db_bind_value(&json!(u32::MAX as u64)),
        Value::from(u32::MAX)
    );
    assert_eq!(
        to_db_bind_value(&json!((u32::MAX as u64) + 1)),
        Value::from((u32::MAX as u64) + 1)
    );
    assert_eq!(
        to_db_bind_value(&Value::String("x".to_string())),
        Value::String("x".to_string())
    );
    assert_eq!(to_db_bind_value(&json!({"x":1})), Value::Null);
}

#[test]
fn query_builder_helpers_cover_empty_and_non_empty_paths() {
    let empty_filter = FilterInput::object(None, None);
    let (where_empty, binds_empty) = build_where_clause_eq(&empty_filter).expect("where empty");
    assert_eq!(where_empty, "");
    assert!(binds_empty.is_empty());

    let err_filter = FilterInput::error();
    let err = build_where_clause_eq(&err_filter).expect_err("where error");
    assert!(matches!(err, SqlError::SerializationError(_)));

    let mut fields = Map::new();
    fields.insert("name".to_string(), Value::String("alpha".to_string()));
    fields.insert("weight".to_string(), Value::from(12));
    let (insert_sql, insert_binds) = build_insert_query_with_meta(
        "items",
        &[("uuid", Value::String("u-1".to_string()))],
        &fields,
    );
    assert!(insert_sql.contains("INSERT INTO items"));
    assert_eq!(insert_binds.len(), 3);

    let (select_all, select_binds_all) = build_select_query_with_meta::<FilterInput>("items", None);
    assert_eq!(select_all, "SELECT * FROM items;");
    assert!(select_binds_all.is_empty());

    let filter = FilterInput::object(Some("row-3"), Some(10));
    let (select_filtered, select_binds_filtered) =
        build_select_query_with_meta("items", Some(&filter));
    assert!(select_filtered.contains(" WHERE "));
    assert_eq!(select_binds_filtered.len(), 2);

    let array_filter = FilterInput::array();
    let (select_error_path, select_error_binds) =
        build_select_query_with_meta("items", Some(&array_filter));
    assert_eq!(select_error_path, "SELECT * FROM items;");
    assert!(select_error_binds.is_empty());
}

#[test]
fn parse_query_and_params_helpers_cover_success_and_error_paths() {
    assert_eq!(
        parse_query_value(&Value::Bool(true)).expect("bool true"),
        json!(1)
    );
    assert_eq!(
        parse_query_value(&Value::Bool(false)).expect("bool false"),
        json!(0)
    );
    assert_eq!(parse_query_value(&Value::Null).expect("null"), Value::Null);
    assert_eq!(parse_query_value(&json!(7)).expect("number"), json!(7));
    assert_eq!(
        parse_query_value(&Value::String("ok".to_string())).expect("string"),
        Value::String("ok".to_string())
    );

    let err = parse_query_value(&json!({"bad": true})).expect_err("object should fail");
    assert!(matches!(err, SqlError::InvalidArgument(_)));

    let params_json = to_params_json(FilterInput::object(Some("a"), Some(1))).expect("params json");
    let params_value: Value = serde_json::from_str(&params_json).expect("params json parse");
    assert_eq!(params_value, json!({"id":"a","amount":1}));

    let err_params =
        to_params_json(FilterInput::error()).expect_err("serialize fail should surface");
    assert!(matches!(err_params, SqlError::SerializationError(_)));
}

#[test]
fn with_transaction_covers_commit_and_rollback_paths() {
    let ok_exec = MockExecutor::new();
    let value = with_transaction(&ok_exec, || Ok::<_, SqlError>(41)).expect("tx should commit");
    assert_eq!(value, 41);
    let ok_snapshot = ok_exec.snapshot();
    assert_eq!(ok_snapshot.begin_count, 1);
    assert_eq!(ok_snapshot.commit_count, 1);
    assert_eq!(ok_snapshot.rollback_count, 0);

    let err_exec = MockExecutor::new();
    let err = with_transaction(&err_exec, || {
        Err::<i32, SqlError>(SqlError::InvalidQuery("bad".to_string()))
    })
    .expect_err("tx should rollback");
    assert!(matches!(err, SqlError::InvalidQuery(_)));
    let err_snapshot = err_exec.snapshot();
    assert_eq!(err_snapshot.begin_count, 1);
    assert_eq!(err_snapshot.commit_count, 0);
    assert_eq!(err_snapshot.rollback_count, 1);

    let rollback_err_exec = MockExecutor::new();
    rollback_err_exec.set_fail_rollback(true);
    let _ = with_transaction(&rollback_err_exec, || {
        Err::<i32, SqlError>(SqlError::InvalidQuery("err".to_string()))
    })
    .expect_err("tx should still return original error");
    let rollback_snapshot = rollback_err_exec.snapshot();
    assert_eq!(rollback_snapshot.rollback_count, 1);
}

#[test]
fn with_transaction_surfaces_begin_error() {
    let exec = MockExecutor::new();
    exec.set_fail_begin(true);
    let err = with_transaction(&exec, || Ok::<_, SqlError>(1)).expect_err("begin should fail");
    assert!(matches!(err, SqlError::Internal));
}

#[test]
fn with_transaction_surfaces_commit_error() {
    let exec = MockExecutor::new();
    exec.set_fail_commit(true);
    let err = with_transaction(&exec, || Ok::<_, SqlError>(1)).expect_err("commit should fail");
    assert!(matches!(err, SqlError::Internal));
}

fn sample_migrations() -> Vec<Migration> {
    vec![
        Migration {
            name: "001",
            up_sql: "create table m1(x integer)",
            down_sql: "drop table m1",
        },
        Migration {
            name: "002",
            up_sql: "create table m2(y integer)",
            down_sql: "drop table m2",
        },
    ]
}

#[test]
fn migrations_run_all_up_applies_pending_and_skips_existing() {
    let exec = MockExecutor::new();
    let migrations = sample_migrations();

    migrations_run_all_up(&exec, &migrations).expect("first run up");
    migrations_run_all_up(&exec, &migrations).expect("second run up");

    let snapshot = exec.snapshot();
    assert!(snapshot.applied.contains("001"));
    assert!(snapshot.applied.contains("002"));
    let up_calls = snapshot
        .exec_sql
        .iter()
        .filter(|sql| sql.starts_with("create table m"))
        .count();
    assert_eq!(up_calls, 2);
}

#[test]
fn migrations_run_all_up_surfaces_ensure_table_error() {
    let exec = MockExecutor::new().with_fail_sql("create table if not exists __migrations");
    let migrations = sample_migrations();
    let err = migrations_run_all_up(&exec, &migrations).expect_err("ensure table should fail");
    assert!(matches!(err, SqlError::InvalidQuery(_)));
}

#[test]
fn migrations_run_all_up_surfaces_begin_error() {
    let exec = MockExecutor::new();
    exec.set_fail_begin(true);
    let migrations = sample_migrations();
    let err = migrations_run_all_up(&exec, &migrations).expect_err("begin should fail");
    assert!(matches!(err, SqlError::Internal));
}

#[test]
fn migrations_run_all_up_surfaces_commit_error() {
    let exec = MockExecutor::new();
    exec.set_fail_commit(true);
    let migrations = sample_migrations();
    let err = migrations_run_all_up(&exec, &migrations).expect_err("commit should fail");
    assert!(matches!(err, SqlError::Internal));
}

#[test]
fn migrations_run_all_up_surfaces_mark_applied_error() {
    let exec = MockExecutor::new().with_fail_sql("insert or ignore into __migrations");
    let migrations = sample_migrations();
    let err = migrations_run_all_up(&exec, &migrations).expect_err("mark applied should fail");
    assert!(matches!(err, SqlError::InvalidQuery(_)));
}

#[test]
fn migrations_run_all_up_rolls_back_on_failure() {
    let exec = MockExecutor::new().with_fail_sql("create table m2");
    let migrations = sample_migrations();

    let err = migrations_run_all_up(&exec, &migrations).expect_err("second migration should fail");
    assert!(matches!(err, SqlError::InvalidQuery(_)));

    let snapshot = exec.snapshot();
    assert!(snapshot.applied.contains("001"));
    assert!(!snapshot.applied.contains("002"));
    assert!(snapshot.rollback_count >= 1);
}

#[test]
fn migrations_run_all_up_surfaces_query_parse_error() {
    let exec = MockExecutor::new();
    exec.set_query_override(Some(Ok("not-json".to_string())));
    let migrations = sample_migrations();
    let err = migrations_run_all_up(&exec, &migrations).expect_err("query parse should fail");
    assert!(matches!(err, SqlError::SerializationError(_)));
}

#[test]
fn migrations_run_all_up_surfaces_query_error() {
    let exec = MockExecutor::new();
    exec.set_query_override(Some(Err(SqlError::Internal)));
    let migrations = sample_migrations();
    let err = migrations_run_all_up(&exec, &migrations).expect_err("query should fail");
    assert!(matches!(err, SqlError::Internal));
}

#[test]
fn migrations_run_all_up_handles_empty_query_rows() {
    let exec = MockExecutor::new();
    exec.set_query_override(Some(Ok(String::new())));
    let migrations = sample_migrations();
    migrations_run_all_up(&exec, &migrations).expect("empty rows should count as not applied");
    let snapshot = exec.snapshot();
    assert!(snapshot.applied.contains("001"));
    assert!(snapshot.applied.contains("002"));
}

#[test]
fn migrations_run_all_down_reverses_and_commits() {
    let exec = MockExecutor::new();
    exec.mark_applied("001");
    exec.mark_applied("002");

    let migrations = sample_migrations();
    migrations_run_all_down(&exec, &migrations).expect("run down");

    let snapshot = exec.snapshot();
    assert!(!snapshot.applied.contains("001"));
    assert!(!snapshot.applied.contains("002"));
    assert!(snapshot.commit_count >= 1);
    let down_calls: Vec<&String> = snapshot
        .exec_sql
        .iter()
        .filter(|sql| sql.starts_with("drop table"))
        .collect();
    assert_eq!(down_calls.len(), 2);
    assert_eq!(down_calls[0].as_str(), "drop table m2");
    assert_eq!(down_calls[1].as_str(), "drop table m1");
}

#[test]
fn migrations_run_all_down_surfaces_ensure_table_error() {
    let exec = MockExecutor::new().with_fail_sql("create table if not exists __migrations");
    let migrations = sample_migrations();
    let err = migrations_run_all_down(&exec, &migrations).expect_err("ensure table should fail");
    assert!(matches!(err, SqlError::InvalidQuery(_)));
}

#[test]
fn migrations_run_all_down_surfaces_delete_error() {
    let exec = MockExecutor::new().with_fail_sql("delete from __migrations");
    let migrations = sample_migrations();
    let err = migrations_run_all_down(&exec, &migrations).expect_err("delete should fail");
    assert!(matches!(err, SqlError::InvalidQuery(_)));
}

#[test]
fn migrations_run_all_down_surfaces_down_sql_error() {
    let exec = MockExecutor::new().with_fail_sql("drop table m2");
    let migrations = sample_migrations();
    let err = migrations_run_all_down(&exec, &migrations).expect_err("down sql should fail");
    assert!(matches!(err, SqlError::InvalidQuery(_)));
}

#[test]
fn migrations_run_all_down_surfaces_begin_error() {
    let exec = MockExecutor::new();
    exec.set_fail_begin(true);
    let migrations = sample_migrations();
    let err = migrations_run_all_down(&exec, &migrations).expect_err("begin should fail");
    assert!(matches!(err, SqlError::Internal));
}

#[test]
fn migrations_run_all_down_surfaces_commit_error() {
    let exec = MockExecutor::new();
    exec.set_fail_commit(true);
    let migrations = sample_migrations();
    let err = migrations_run_all_down(&exec, &migrations).expect_err("commit should fail");
    assert!(matches!(err, SqlError::Internal));
}
