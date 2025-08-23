use thiserror::Error;

#[derive(Debug, Error)]
pub enum NostrUtilsError {
    #[cfg(feature = "sdk")]
    #[error("Client error: {0}")]
    ClientError(#[from] nostr_sdk::client::Error),

    #[cfg(feature = "sdk")]
    #[error("Database error: {0}")]
    DatabaseError(#[from] nostr_sdk::prelude::DatabaseError),

    #[error("Event error: {0}")]
    EventError(#[from] nostr::event::Error),

    #[error("Event not found: {0}")]
    EventNotFound(String),

    #[error("Event builder failure: {0}")]
    EventBuildError(#[from] nostr::event::builder::Error),
}

#[derive(Debug, Error)]
pub enum NostrTagsResolveError {
    #[error("Missing public key 'p' tag in encrypted event: {0:?}")]
    MissingPTag(nostr::Event),

    #[error("Encrypted event recipient mismatch")]
    NotRecipient,

    #[error("Decryption error: {0}")]
    DecryptionError(String),

    #[error("Failed to parse decrypted tag JSON: {0}")]
    ParseError(#[from] serde_json::Error),
}
