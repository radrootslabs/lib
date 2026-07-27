#![forbid(unsafe_code)]

use thiserror::Error;

#[derive(Debug, Error)]
pub enum RadrootsRelayTransportError {
    #[error("Relay URL parse failed for `{url}`: {reason}")]
    RelayUrlParse { url: String, reason: String },

    #[error("Relay URL `{url}` uses ws outside localhost relay policy")]
    WsRequiresLocalhostPolicy { url: String },

    #[error("Relay URL `{url}` has unsupported scheme `{scheme}`")]
    UnsupportedRelayScheme { url: String, scheme: String },

    #[error("Relay URL `{url}` must include a host")]
    EmptyRelayHost { url: String },

    #[error("Relay URL `{url}` must not include userinfo")]
    RelayUrlUserinfo { url: String },

    #[error("Relay URL `{url}` must not include query or fragment")]
    RelayUrlQueryOrFragment { url: String },

    #[error("Relay URL `{url}` targets forbidden destination: {reason}")]
    RelayUrlForbiddenDestination { url: String, reason: String },

    #[error("Relay URL `{url}` resolved to forbidden address `{address}`: {reason}")]
    RelayUrlResolvedForbiddenDestination {
        url: String,
        address: String,
        reason: String,
    },

    #[error("Relay URL `{url}` did not resolve to any addresses")]
    RelayUrlResolvedNoAddresses { url: String },

    #[error("Relay target set must not be empty")]
    EmptyTargetSet,

    #[error("Relay target set contains duplicate URL `{url}`")]
    DuplicateRelayUrl { url: String },

    #[error("Relay fetch item contains invalid relay URL `{url}`: {reason}")]
    InvalidFetchItemRelayUrl { url: String, reason: String },

    #[error("Relay fetch item came from unrequested relay URL `{url}`")]
    UnexpectedFetchItemRelayUrl { url: String },

    #[error("Relay fetch adapter returned duplicate terminal outcome for relay URL `{url}`")]
    DuplicateFetchTerminalRelayUrl { url: String },

    #[error(
        "Relay fetch adapter returned conflicting terminal outcomes for relay URL `{url}`: first={first}, next={next}"
    )]
    ConflictingFetchTerminalRelayUrl {
        url: String,
        first: &'static str,
        next: &'static str,
    },

    #[error("Relay publish receipt contains invalid relay URL `{url}`: {reason}")]
    InvalidPublishReceiptRelayUrl { url: String, reason: String },

    #[error("Relay publish receipt came from unrequested relay URL `{url}`")]
    UnexpectedPublishReceiptRelayUrl { url: String },

    #[error("Relay publish adapter returned duplicate receipts for relay URL `{url}`")]
    DuplicatePublishReceiptRelayUrl { url: String },

    #[error("Relay publish adapter returned incoherent attempt state for relay URL `{url}`")]
    InvalidPublishReceiptAttemptState { url: String },

    #[error("Transport returned conflicting publish receipts for relay URL `{url}`")]
    ConflictingTransportReceiptRelayUrl { url: String },

    #[error("Expected transport kind `{expected}`, received `{actual}`")]
    UnexpectedTransportKind {
        expected: &'static str,
        actual: String,
    },

    #[error("Relay fetch filters must not be empty")]
    EmptyFetchFilters,

    #[error("Relay fetch {field} must be greater than zero")]
    InvalidFetchLimit { field: &'static str },

    #[error("Relay fetch {field} {actual} exceeds maximum {max}")]
    FetchLimitTooLarge {
        field: &'static str,
        max: usize,
        actual: usize,
    },

    #[error("Relay transport {field} uses {actual} UTF-8 bytes; maximum is {max}")]
    DiagnosticLimitExceeded {
        field: &'static str,
        max: usize,
        actual: usize,
    },

    #[error("Relay transport {field} cannot be negative: {value}")]
    InvalidTimestamp { field: &'static str, value: i64 },

    #[error(
        "Relay publish idempotency key is invalid: {reason}; bytes={actual_bytes}, max={max_bytes}"
    )]
    InvalidIdempotencyKey {
        reason: &'static str,
        actual_bytes: usize,
        max_bytes: usize,
    },

    #[error("Relay publish required target `{fingerprint}` is not in the requested relay set")]
    RequiredTargetNotRequested { fingerprint: String },

    #[error("Transport contract error: {0}")]
    TransportContract(String),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Nostr event JSON error: {0}")]
    NostrEventJson(String),

    #[cfg(feature = "storage")]
    #[error("Event store error: {0}")]
    EventStore(#[from] radroots_event_store::RadrootsEventStoreError),

    #[cfg(feature = "storage")]
    #[error("Event store returned no current visibility for persisted event `{event_id}`")]
    MissingStoredEventVisibility { event_id: String },

    #[cfg(feature = "storage")]
    #[error("Persisted relay fetch event receipt is missing its event id")]
    MissingPersistedFetchReceiptEventId,

    #[cfg(feature = "storage")]
    #[error("Event store returned an unsupported current visibility for event `{event_id}`")]
    UnsupportedStoredEventVisibility { event_id: String },

    #[cfg(feature = "storage")]
    #[error("Outbox error: {0}")]
    Outbox(#[from] radroots_outbox::RadrootsOutboxError),

    #[cfg(feature = "storage")]
    #[error("Outbox claim {0} does not contain a signed event")]
    MissingSignedOutboxEvent(i64),

    #[error("Relay transport error: {0}")]
    Transport(String),
}

pub(crate) fn ensure_nonnegative_timestamp(
    field: &'static str,
    value: i64,
) -> Result<(), RadrootsRelayTransportError> {
    if value < 0 {
        return Err(RadrootsRelayTransportError::InvalidTimestamp { field, value });
    }
    Ok(())
}

impl From<radroots_transport::RadrootsTransportError> for RadrootsRelayTransportError {
    fn from(value: radroots_transport::RadrootsTransportError) -> Self {
        Self::TransportContract(value.to_string())
    }
}
