#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![forbid(unsafe_code)]

#[cfg(feature = "client")]
mod client;
mod error;
#[cfg(feature = "storage")]
mod fetch;
#[cfg(feature = "nip11")]
mod nip11;
#[cfg(feature = "storage")]
mod outbox;
mod outcome;
mod publish;
mod relay;
#[cfg(feature = "client")]
mod relays;

#[cfg(feature = "client")]
pub use client::{
    RadrootsNostrClient, RadrootsNostrClientKey, RadrootsNostrClientOptions,
    RadrootsNostrEventStream, RadrootsNostrMonitor, RadrootsNostrMonitorNotification,
    RadrootsNostrMonitorReceiveError, RadrootsNostrMonitorReceiver, RadrootsNostrOutput,
    RadrootsNostrRelay, RadrootsNostrRelayStatus, RadrootsNostrSubscribeAutoCloseOptions,
    RadrootsNostrSubscriptionId, radroots_nostr_fetch_event_by_id,
};
pub use error::RadrootsRelayTransportError;
#[cfg(all(feature = "storage", feature = "runtime-tokio"))]
pub use fetch::fetch_relay_events_blocking;
#[cfg(feature = "storage")]
pub use fetch::{
    RADROOTS_RELAY_FETCH_EVENT_LIMIT_MAX, RADROOTS_RELAY_FETCH_RAW_EVENT_LIMIT_MAX,
    RADROOTS_RELAY_FETCH_RAW_JSON_BYTE_LIMIT_MAX, RadrootsMockRelayFetchAdapter,
    RadrootsNostrClientFetchAdapter, RadrootsRelayFetchAdapter, RadrootsRelayFetchEventAdmission,
    RadrootsRelayFetchEventReceipt, RadrootsRelayFetchEventValidStream,
    RadrootsRelayFetchEventVerification, RadrootsRelayFetchEventVisibility,
    RadrootsRelayFetchFailure, RadrootsRelayFetchFilters, RadrootsRelayFetchItem,
    RadrootsRelayFetchMode, RadrootsRelayFetchOutcomeKind, RadrootsRelayFetchReceipt,
    RadrootsRelayFetchRelayOutcome, RadrootsRelayFetchRequest, RadrootsRelayFetchedEvent,
    RadrootsRelayFetchedEventsReceipt, fetch_and_ingest_relay_events, fetch_relay_events,
};
#[cfg(feature = "nip11")]
pub use nip11::fetch_nip11;
#[cfg(feature = "storage")]
pub use outbox::{
    RadrootsOutboxPublishPolicy, RadrootsOutboxPublishReceipt, RadrootsOutboxPublishTargetReceipt,
    publish_claimed_outbox_event, publish_claimed_outbox_event_with_transport,
};
pub use outcome::{RadrootsRelayOutcome, RadrootsRelayOutcomeKind};
#[cfg(feature = "client")]
pub use publish::RadrootsNostrClientPublishAdapter;
pub use publish::{
    RADROOTS_RELAY_PUBLISH_IDEMPOTENCY_KEY_MAX_BYTES, RadrootsMockRelayPublishAdapter,
    RadrootsNostrTransport, RadrootsRelayPublishAdapter, RadrootsRelayPublishReceipt,
    RadrootsRelayPublishRelayReceipt, RadrootsRelayPublishRequest, publish_signed_event,
    verified_signed_event_payload,
};
pub use relay::{RadrootsRelayTargetSet, RadrootsRelayUrlPolicy, RelayUrl};
#[cfg(feature = "client")]
pub use relays::{radroots_nostr_add_relay, radroots_nostr_connect, radroots_nostr_remove_relay};
