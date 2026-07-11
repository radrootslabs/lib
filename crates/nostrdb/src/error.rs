use thiserror::Error;

#[derive(Debug, Error)]
pub enum RadrootsNostrdbError {
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
    Nostrdb(String),
}

#[cfg(feature = "nostrdb")]
impl From<nostrdb::Error> for RadrootsNostrdbError {
    fn from(value: nostrdb::Error) -> Self {
        Self::Nostrdb(value.to_string())
    }
}

impl From<serde_json::Error> for RadrootsNostrdbError {
    fn from(value: serde_json::Error) -> Self {
        Self::EventJsonEncode(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_nostrdb_error() {
        let converted: RadrootsNostrdbError = nostrdb::Error::NotFound.into();
        assert!(converted.to_string().starts_with("nostrdb error:"));
    }

    #[test]
    fn converts_serde_json_error() {
        let source = serde_json::from_str::<serde_json::Value>("not json").expect_err("json error");
        let converted: RadrootsNostrdbError = source.into();
        assert!(
            converted
                .to_string()
                .starts_with("event json encode failed:")
        );
    }
}
