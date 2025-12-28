use crate::{ExecOutcome, SqlExecutor, error::SqlError};
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, Ordering};

const EXPORT_LOCK_ERR: &str = "tangle db export in progress";

static EXPORT_LOCK_ACTIVE: AtomicBool = AtomicBool::new(false);

thread_local! {
    static EXPORT_LOCK_BYPASS: Cell<bool> = Cell::new(false);
}

pub fn export_lock_begin() -> Result<(), SqlError> {
    let was_active = EXPORT_LOCK_ACTIVE.swap(true, Ordering::SeqCst);
    if was_active {
        return Err(SqlError::InvalidArgument(EXPORT_LOCK_ERR.to_string()));
    }
    Ok(())
}

pub fn export_lock_end() {
    EXPORT_LOCK_ACTIVE.store(false, Ordering::SeqCst);
}

pub fn export_lock_active() -> bool {
    EXPORT_LOCK_ACTIVE.load(Ordering::SeqCst)
}

pub fn with_export_lock_bypass<T>(f: impl FnOnce() -> T) -> T {
    EXPORT_LOCK_BYPASS.with(|flag| {
        let prev = flag.replace(true);
        let out = f();
        flag.set(prev);
        out
    })
}

fn export_lock_blocked() -> bool {
    if !EXPORT_LOCK_ACTIVE.load(Ordering::SeqCst) {
        return false;
    }
    EXPORT_LOCK_BYPASS.with(|flag| !flag.get())
}

pub struct WasmSqlExecutor;

impl WasmSqlExecutor {
    pub fn new() -> Self {
        Self
    }
}

impl Default for WasmSqlExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl SqlExecutor for WasmSqlExecutor {
    fn exec(&self, sql: &str, params_json: &str) -> Result<ExecOutcome, SqlError> {
        if export_lock_blocked() {
            return Err(SqlError::InvalidArgument(EXPORT_LOCK_ERR.to_string()));
        }
        let js = radroots_sql_wasm_bridge::exec(sql, params_json);
        let v: serde_json::Value = serde_wasm_bindgen::from_value(js)
            .map_err(|e| SqlError::SerializationError(e.to_string()))?;
        let changes = v.get("changes").and_then(|x| x.as_i64()).unwrap_or(0);
        let last_insert_id = v
            .get("last_insert_id")
            .or_else(|| v.get("lastInsertRowid"))
            .and_then(|x| x.as_i64())
            .unwrap_or(0);
        Ok(ExecOutcome {
            changes,
            last_insert_id,
        })
    }

    fn query_raw(&self, sql: &str, params_json: &str) -> Result<String, SqlError> {
        let js = radroots_sql_wasm_bridge::query(sql, params_json);
        let v: serde_json::Value = serde_wasm_bindgen::from_value(js)
            .map_err(|e| SqlError::SerializationError(e.to_string()))?;
        Ok(v.to_string())
    }

    fn begin(&self) -> Result<(), SqlError> {
        if export_lock_blocked() {
            return Err(SqlError::InvalidArgument(EXPORT_LOCK_ERR.to_string()));
        }
        radroots_sql_wasm_bridge::begin_tx();
        Ok(())
    }

    fn commit(&self) -> Result<(), SqlError> {
        if export_lock_blocked() {
            return Err(SqlError::InvalidArgument(EXPORT_LOCK_ERR.to_string()));
        }
        radroots_sql_wasm_bridge::commit_tx();
        Ok(())
    }

    fn rollback(&self) -> Result<(), SqlError> {
        if export_lock_blocked() {
            return Err(SqlError::InvalidArgument(EXPORT_LOCK_ERR.to_string()));
        }
        radroots_sql_wasm_bridge::rollback_tx();
        Ok(())
    }
}
