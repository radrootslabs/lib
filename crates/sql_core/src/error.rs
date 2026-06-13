use alloc::string::{String, ToString};
use core::fmt::{Display, Formatter};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub enum SqlError {
    InvalidArgument(String),
    NotFound(String),
    SerializationError(String),
    InvalidQuery(String),
    Internal,
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

impl Display for SqlError {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            SqlError::InvalidArgument(value) => write!(f, "invalid argument: {value}"),
            SqlError::NotFound(value) => write!(f, "{value} not found"),
            SqlError::SerializationError(value) => write!(f, "serialization error: {value}"),
            SqlError::InvalidQuery(value) => write!(f, "invalid query: {value}"),
            SqlError::Internal => f.write_str("internal error"),
            SqlError::UnsupportedPlatform => f.write_str("unsupported on this platform"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for SqlError {}

impl From<serde_json::Error> for SqlError {
    fn from(e: serde_json::Error) -> Self {
        SqlError::SerializationError(e.to_string())
    }
}

#[cfg(all(feature = "native", feature = "std"))]
impl From<rusqlite::Error> for SqlError {
    fn from(e: rusqlite::Error) -> Self {
        SqlError::InvalidQuery(e.to_string())
    }
}
