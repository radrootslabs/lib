#![forbid(unsafe_code)]

use radroots_sql_core::error::SqlError;

#[derive(Clone, Copy, Debug, Default)]
pub struct EmbeddedSqlEngine;

impl EmbeddedSqlEngine {
    pub fn new() -> Result<Self, SqlError> {
        Err(SqlError::UnsupportedPlatform)
    }
}
