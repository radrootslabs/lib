use thiserror::Error;

#[derive(Debug, Error)]
pub enum RadrootsNostrError {
    #[error("Nostr event kind {kind} exceeds {max}")]
    KindOutOfRange { kind: u32, max: u16 },

    #[cfg(feature = "client")]
    #[error("Client error: {0}")]
    ClientError(#[from] nostr_sdk::client::Error),

    #[cfg(feature = "client")]
    #[error("Database error: {0}")]
    DatabaseError(#[from] nostr_sdk::prelude::DatabaseError),

    #[cfg(feature = "client")]
    #[error("Client configuration error: {0}")]
    ClientConfigError(String),

    #[error("Nostr event kind {kind} requires typed authoring")]
    TypedAuthoringRequired { kind: u16 },

    #[error("External signing author mismatch: expected {expected}, got {actual}")]
    ExternalSigningAuthorMismatch {
        expected: nostr::PublicKey,
        actual: nostr::PublicKey,
    },

    #[error("External signing event ID mismatch: expected {expected}, got {actual}")]
    ExternalSigningEventIdMismatch {
        expected: nostr::EventId,
        actual: nostr::EventId,
    },

    #[error("External signing event is invalid: {0}")]
    ExternalSigningEventInvalid(#[source] nostr::event::Error),

    #[error("Event error: {0}")]
    EventError(#[from] nostr::event::Error),

    #[error("Event not found: {0}")]
    EventNotFound(String),

    #[error("Event builder failure: {0}")]
    EventBuildError(#[from] nostr::event::builder::Error),

    #[cfg(feature = "events")]
    #[error("Draft error: {0}")]
    DraftError(#[from] radroots_event::draft::DraftError),

    #[cfg(feature = "events")]
    #[error("Event wire error: {0}")]
    EventWire(#[from] radroots_event::wire::EventWireError),

    #[cfg(feature = "events")]
    #[error("FoodAvailability encoding error: {0}")]
    FoodAvailabilityEncode(
        #[from]
        radroots_event_codec::food_availability::authored::RadrootsFoodAvailabilityEncodeError,
    ),

    #[cfg(feature = "events")]
    #[error("Profile encoding error: {0}")]
    ProfileEncode(
        #[from] radroots_event_codec::profile::authored::RadrootsAuthoredProfileEncodeError,
    ),

    #[cfg(feature = "events")]
    #[error("Signed event error: {0}")]
    SignedEvent(#[from] radroots_event::draft::SignedEventError),

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
}

#[derive(Debug, Error)]
pub enum RadrootsNostrTagsResolveError {
    #[error("Missing public key 'p' tag in encrypted event: {0:?}")]
    MissingPTag(Box<nostr::Event>),

    #[error("Encrypted event recipient mismatch")]
    NotRecipient,

    #[error("Decryption error: {0}")]
    DecryptionError(String),

    #[error("Failed to parse decrypted tag JSON: {0}")]
    ParseError(#[from] serde_json::Error),
}
