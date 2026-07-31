//! Normalized Nostr Connect errors.

use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RadrootsNostrConnectError {
    #[error("NIP-46 request encryption failed: {reason}")]
    Encrypt { reason: String },
    #[error("NIP-46 response decryption failed: {reason}")]
    Decrypt { reason: String },
    #[error("NIP-46 event signing failed: {reason}")]
    Sign { reason: String },
    #[error("NIP-46 transport failed: {reason}")]
    Transport { reason: String },
    #[error("NIP-46 request timed out")]
    RequestTimedOut,
    #[error("invalid NIP-46 client key")]
    InvalidClientKey,
    #[error("invalid NIP-46 client target: {reason}")]
    InvalidClientTarget { reason: &'static str },
    #[error("invalid NIP-46 client event")]
    InvalidClientEvent,
    #[error("invalid NIP-46 client state: {reason}")]
    InvalidClientState { reason: &'static str },
    #[error("invalid NIP-46 server request: {reason}")]
    InvalidServerRequest { reason: &'static str },
    #[error("invalid NIP-46 server state: {reason}")]
    InvalidServerState { reason: &'static str },
    #[error("unsupported NIP-46 method `{0}`")]
    UnsupportedMethod(crate::method::Method),
    #[error("replayed NIP-46 request")]
    ReplayedRequest,
    #[error("invalid NIP-46 request id: {reason}")]
    InvalidRequestId { reason: &'static str },
    #[error("NIP-46 response id does not match the request")]
    WrongRequestId,
    #[error("NIP-46 response signer does not match the expected signer")]
    WrongResponseSigner,
    #[error("replayed NIP-46 response")]
    ReplayedResponse,
    #[error("invalid NIP-46 response envelope: {reason}")]
    InvalidResponseEnvelope { reason: &'static str },
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
    #[error("invalid NIP-46 client metadata field `{field}`: {reason}")]
    InvalidClientMetadata { field: &'static str, reason: String },
    #[error("NIP-46 client metadata exceeds {max} bytes (received {received})")]
    ClientMetadataTooLarge { max: usize, received: usize },
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
