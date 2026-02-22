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

impl From<serde_json::Error> for RadrootsNostrNdbError {
    fn from(value: serde_json::Error) -> Self {
        Self::EventJsonEncode(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_nostrdb_error() {
        let converted: RadrootsNostrNdbError = nostrdb::Error::NotFound.into();
        assert!(matches!(converted, RadrootsNostrNdbError::Ndb(_)));
    }

    #[test]
    fn converts_serde_json_error() {
        let source = serde_json::from_str::<serde_json::Value>("not json").expect_err("json error");
        let converted: RadrootsNostrNdbError = source.into();
        assert!(matches!(
            converted,
            RadrootsNostrNdbError::EventJsonEncode(_)
        ));
    }
}
