use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RadrootsNostrConnectError {
    #[error("invalid NIP-46 method `{0}`")]
    InvalidMethod(String),
    #[error("invalid NIP-46 permission `{0}`")]
    InvalidPermission(String),
    #[error("invalid public key `{value}`: {reason}")]
    InvalidPublicKey { value: String, reason: String },
    #[error("invalid relay url `{value}`: {reason}")]
    InvalidRelayUrl { value: String, reason: String },
    #[error("invalid url `{value}`: {reason}")]
    InvalidUrl { value: String, reason: String },
    #[error("invalid URI scheme `{0}`")]
    InvalidUriScheme(String),
    #[error("invalid NIP-46 uri")]
    InvalidUri,
    #[error("missing public key in URI authority")]
    MissingPublicKey,
    #[error("missing relay in URI")]
    MissingRelay,
    #[error("missing secret in nostrconnect uri")]
    MissingSecret,
    #[error("missing response result")]
    MissingResult,
    #[error("invalid parameter count for method `{method}`: expected {expected}, got {received}")]
    InvalidParams {
        method: String,
        expected: &'static str,
        received: usize,
    },
    #[error("invalid request payload for method `{method}`: {reason}")]
    InvalidRequestPayload { method: String, reason: String },
    #[error("invalid response payload for method `{method}`: {reason}")]
    InvalidResponsePayload { method: String, reason: String },
    #[error("JSON error: {0}")]
    Json(String),
}

impl From<serde_json::Error> for RadrootsNostrConnectError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value.to_string())
    }
}
