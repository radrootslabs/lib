use thiserror::Error;

#[derive(Debug, Error)]
pub enum RadrootsNostrNdbError {
    #[error("database path must be utf-8")]
    NonUtf8Path,

    #[error("invalid hex for {field}: {reason}")]
    InvalidHex { field: &'static str, reason: String },

    #[error("invalid hex length for {field}: expected {expected} bytes, got {actual}")]
    InvalidHexLength {
        field: &'static str,
        expected: usize,
        actual: usize,
    },

    #[error("event json encode failed: {0}")]
    EventJsonEncode(String),

    #[error("nostrdb error: {0}")]
    Ndb(String),
}

#[cfg(feature = "ndb")]
impl From<nostrdb::Error> for RadrootsNostrNdbError {
    fn from(value: nostrdb::Error) -> Self {
        Self::Ndb(value.to_string())
    }
}
