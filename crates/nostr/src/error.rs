//! Normalized Nostr adapter failures.
//!
//! Errors describe stable failure categories without retaining secret keys,
//! passwords, plaintext messages, or caller-supplied credential strings.

#[cfg(feature = "events")]
use alloc::boxed::Box;
use alloc::string::String;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid Nostr public key")]
    InvalidPublicKey,

    #[error("invalid NIP-19 npub public key")]
    InvalidNpub,

    #[cfg(feature = "signing")]
    #[error("invalid Nostr secret key")]
    InvalidSecretKey,

    #[cfg(feature = "signing")]
    #[error("invalid NIP-49 encrypted secret key")]
    InvalidEncryptedSecretKey,

    #[cfg(feature = "signing")]
    #[error("NIP-49 secret-key encryption failed")]
    SecretKeyEncryption,

    #[cfg(feature = "signing")]
    #[error("NIP-49 secret-key decryption failed")]
    SecretKeyDecryption,

    #[error("Nostr event kind {kind} exceeds {max}")]
    KindOutOfRange { kind: u32, max: u16 },

    #[error("Radroots event field {field} cannot be represented by Nostr")]
    EventConversion { field: &'static str },

    #[error("Radroots event coordinate cannot be represented by Nostr")]
    CoordinateConversion,

    #[error("Radroots event tag cannot be represented by Nostr")]
    TagConversion,

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
    ExternalSigningEventInvalid(#[cfg_attr(feature = "std", source)] nostr::event::Error),

    #[error("External signing result does not match authored plan field `{field}`")]
    ExternalSigningPlanMismatch { field: &'static str },

    #[error("Event error: {0}")]
    EventError(#[cfg_attr(feature = "std", source)] nostr::event::Error),

    #[error("Event builder failure: {0}")]
    EventBuildError(#[cfg_attr(feature = "std", source)] nostr::event::builder::Error),

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
        radroots_event_codec::encode::food_availability::RadrootsFoodAvailabilityEncodeError,
    ),

    #[cfg(feature = "events")]
    #[error("Profile encoding error: {0}")]
    ProfileEncode(
        #[from] radroots_event_codec::encode::profile::RadrootsAuthoredProfileEncodeError,
    ),

    #[cfg(feature = "events")]
    #[error("Authored plan error: {0}")]
    AuthoredPlan(#[from] radroots_event_codec::authoring::AuthoredPlanError),

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
    KeyError(#[cfg_attr(feature = "std", source)] nostr::key::Error),

    #[error("Filter tag error: {0}")]
    FilterTagError(String),
}

impl From<nostr::event::Error> for Error {
    fn from(error: nostr::event::Error) -> Self {
        Self::EventError(error)
    }
}

impl From<nostr::event::builder::Error> for Error {
    fn from(error: nostr::event::builder::Error) -> Self {
        Self::EventBuildError(error)
    }
}

impl From<nostr::key::Error> for Error {
    fn from(error: nostr::key::Error) -> Self {
        Self::KeyError(error)
    }
}

#[cfg(feature = "events")]
#[derive(Debug, Error)]
pub enum ResolveError {
    #[error("Missing public key 'p' tag in encrypted event: {0:?}")]
    MissingPTag(Box<nostr::Event>),

    #[error("Encrypted event recipient mismatch")]
    NotRecipient,

    #[error("Decryption error: {0}")]
    DecryptionError(String),

    #[error("Failed to parse decrypted tag JSON: {0}")]
    ParseError(#[from] serde_json::Error),
}
