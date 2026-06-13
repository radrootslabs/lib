#![forbid(unsafe_code)]

use thiserror::Error;

#[derive(Debug, Error)]
pub enum RadrootsRelayTransportError {
    #[error("Relay URL parse failed for `{url}`: {reason}")]
    RelayUrlParse { url: String, reason: String },

    #[error("Relay URL `{url}` uses ws outside local-dev policy")]
    WsRequiresLocalPolicy { url: String },

    #[error("Relay URL `{url}` has unsupported scheme `{scheme}`")]
    UnsupportedRelayScheme { url: String, scheme: String },

    #[error("Relay URL `{url}` must include a host")]
    EmptyRelayHost { url: String },

    #[error("Relay URL `{url}` must not include userinfo")]
    RelayUrlUserinfo { url: String },

    #[error("Relay URL `{url}` must not include query or fragment")]
    RelayUrlQueryOrFragment { url: String },

    #[error("Relay target set must not be empty")]
    EmptyTargetSet,

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Nostr event JSON error: {0}")]
    NostrEventJson(String),

    #[error("Event store error: {0}")]
    EventStore(#[from] radroots_event_store::RadrootsEventStoreError),

    #[error("Outbox error: {0}")]
    Outbox(#[from] radroots_outbox::RadrootsOutboxError),

    #[error("Outbox claim {0} does not contain a signed event")]
    MissingSignedOutboxEvent(i64),

    #[error("Relay transport error: {0}")]
    Transport(String),
}
