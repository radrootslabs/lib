#![forbid(unsafe_code)]

use radroots_sql_core::error::SqlError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LocalEventsError {
    #[error("invalid local event record: {0}")]
    InvalidRecord(String),
    #[error("sql error: {0}")]
    Sql(#[from] SqlError),
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}
