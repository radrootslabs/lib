#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![forbid(unsafe_code)]

#[cfg(feature = "client")]
mod connector;
#[cfg(feature = "client")]
mod diagnostic;
mod error;
#[cfg(feature = "storage")]
mod fetch;
#[cfg(feature = "storage")]
mod outbox;
mod outcome;
mod publish;
mod relay;

pub use error::RadrootsRelayTransportError;
#[cfg(all(feature = "storage", feature = "runtime-tokio"))]
pub use fetch::fetch_relay_events_blocking;
#[cfg(feature = "storage")]
pub use fetch::{
    RADROOTS_RELAY_FETCH_EVENT_LIMIT_MAX, RADROOTS_RELAY_FETCH_FILTER_JSON_BYTE_LIMIT_MAX,
    RADROOTS_RELAY_FETCH_FILTER_LIMIT_MAX, RADROOTS_RELAY_FETCH_FILTER_SET_JSON_BYTE_LIMIT_MAX,
    RADROOTS_RELAY_FETCH_RAW_EVENT_LIMIT_MAX, RADROOTS_RELAY_FETCH_RAW_JSON_BYTE_LIMIT_MAX,
    RADROOTS_RELAY_FETCH_TIMEOUT_MS_MAX, RadrootsMockRelayFetchAdapter,
    RadrootsNostrClientFetchAdapter, RadrootsRelayFetchAdapter, RadrootsRelayFetchEmitter,
    RadrootsRelayFetchEventAdmission, RadrootsRelayFetchEventReceipt,
    RadrootsRelayFetchEventValidStream, RadrootsRelayFetchEventVerification,
    RadrootsRelayFetchEventVisibility, RadrootsRelayFetchFailure, RadrootsRelayFetchFilters,
    RadrootsRelayFetchItem, RadrootsRelayFetchMode, RadrootsRelayFetchOutcomeKind,
    RadrootsRelayFetchReceipt, RadrootsRelayFetchRelayOutcome, RadrootsRelayFetchRequest,
    RadrootsRelayFetchedEvent, RadrootsRelayFetchedEventsReceipt, fetch_and_ingest_relay_events,
    fetch_relay_events,
};
#[cfg(feature = "storage")]
pub use outbox::{
    RadrootsOutboxPublishEmptyTargetState, RadrootsOutboxPublishPolicy,
    RadrootsOutboxPublishReceipt, RadrootsOutboxPublishTargetReceipt,
    execute_claimed_phase1_publication_target_with_transport, phase1_publication_delivery_request,
    publish_claimed_outbox_event, publish_claimed_outbox_event_with_transport,
    publish_claimed_phase1_publication_target_with_transport,
    repair_phase1_publication_observation,
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
pub use relay::{RadrootsRelayTargetSet, RadrootsRelayUrl, RadrootsRelayUrlPolicy};
