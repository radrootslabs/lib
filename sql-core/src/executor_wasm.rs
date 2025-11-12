use crate::{ExecOutcome, SqlExecutor, error::SqlError};

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
        radroots_sql_wasm_bridge::begin_tx();
        Ok(())
    }

    fn commit(&self) -> Result<(), SqlError> {
        radroots_sql_wasm_bridge::commit_tx();
        Ok(())
    }

    fn rollback(&self) -> Result<(), SqlError> {
        radroots_sql_wasm_bridge::rollback_tx();
        Ok(())
    }
}
