use thiserror::Error;

#[derive(Debug, Error)]
pub enum RadrootsNostrError {
    #[cfg(feature = "client")]
    #[error("Client error: {0}")]
    ClientError(#[from] nostr_sdk::client::Error),

    #[cfg(feature = "client")]
    #[error("Database error: {0}")]
    DatabaseError(#[from] nostr_sdk::prelude::DatabaseError),

    #[cfg(feature = "client")]
    #[error("Client configuration error: {0}")]
    ClientConfigError(String),

    #[error("Event error: {0}")]
    EventError(#[from] nostr::event::Error),

    #[error("Event not found: {0}")]
    EventNotFound(String),

    #[error("Event builder failure: {0}")]
    EventBuildError(#[from] nostr::event::builder::Error),

    #[cfg(feature = "events")]
    #[error("Draft error: {0}")]
    DraftError(#[from] radroots_events::draft::RadrootsDraftError),

    #[cfg(feature = "events")]
    #[error(
        "Frozen draft signer public key mismatch: expected {expected_pubkey}, got {actual_pubkey}"
    )]
    FrozenDraftPubkeyMismatch {
        expected_pubkey: String,
        actual_pubkey: String,
    },

    #[cfg(feature = "events")]
    #[error("Frozen draft event ID mismatch: expected {expected_event_id}, got {actual_event_id}")]
    FrozenDraftEventIdMismatch {
        expected_event_id: String,
        actual_event_id: String,
    },

    #[error("Key error: {0}")]
    KeyError(#[from] nostr::key::Error),

    #[error("Filter tag error: {0}")]
    FilterTagError(String),

    #[cfg(feature = "codec")]
    #[error("Profile encode error: {0}")]
    ProfileEncodeError(#[from] radroots_events_codec::profile::error::ProfileEncodeError),
}

#[derive(Debug, Error)]
pub enum RadrootsNostrTagsResolveError {
    #[error("Missing public key 'p' tag in encrypted event: {0:?}")]
    MissingPTag(nostr::Event),

    #[error("Encrypted event recipient mismatch")]
    NotRecipient,

    #[error("Decryption error: {0}")]
    DecryptionError(String),

    #[error("Failed to parse decrypted tag JSON: {0}")]
    ParseError(#[from] serde_json::Error),
}
