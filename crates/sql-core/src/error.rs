#![cfg_attr(any(feature = "embedded", target_os = "espidf"), no_std)]

#[cfg(any(feature = "embedded", target_os = "espidf"))]
extern crate alloc;

use serde::Serialize;
use thiserror::Error;

#[cfg(any(feature = "embedded", target_os = "espidf"))]
use alloc::string::String;

#[derive(Error, Debug, Clone, Serialize)]
pub enum SqlError {
    #[error("invalid argument: {0}")]
    InvalidArgument(String),
    #[error("{0} not found")]
    NotFound(String),
    #[error("serialization error: {0}")]
    SerializationError(String),
    #[error("invalid query: {0}")]
    InvalidQuery(String),
    #[error("internal error")]
    Internal,
    #[error("unsupported on this platform")]
    UnsupportedPlatform,
}

impl SqlError {
    pub fn code(&self) -> &'static str {
        match self {
            SqlError::InvalidArgument(_) => "ERR_INVALID_ARGUMENT",
            SqlError::NotFound(_) => "ERR_NOT_FOUND",
            SqlError::SerializationError(_) => "ERR_SERIALIZATION",
            SqlError::InvalidQuery(_) => "ERR_INVALID_QUERY",
            SqlError::Internal => "ERR_INTERNAL",
            SqlError::UnsupportedPlatform => "ERR_UNSUPPORTED_PLATFORM",
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({ "code": self.code(), "message": self.to_string() })
    }
}

impl From<serde_json::Error> for SqlError {
    fn from(e: serde_json::Error) -> Self {
        SqlError::SerializationError(e.to_string())
    }
}

#[cfg(feature = "native")]
impl From<rusqlite::Error> for SqlError {
    fn from(e: rusqlite::Error) -> Self {
        SqlError::InvalidQuery(e.to_string())
    }
}
