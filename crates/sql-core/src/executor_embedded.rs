#![cfg(any(feature = "embedded", target_os = "espidf"))]
#![no_std]

extern crate alloc;

use alloc::string::String;

use crate::{ExecOutcome, SqlExecutor, error::SqlError};

#[derive(Clone, Debug)]
pub struct EmbeddedSqlExecutor;

impl EmbeddedSqlExecutor {
    pub fn new() -> Self {
        Self
    }
}

impl SqlExecutor for EmbeddedSqlExecutor {
    fn exec(&self, _sql: &str, _params_json: &str) -> Result<ExecOutcome, SqlError> {
        Ok(ExecOutcome {
            changes: 0,
            last_insert_id: 0,
        })
    }

    fn query_raw(&self, _sql: &str, _params_json: &str) -> Result<String, SqlError> {
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
