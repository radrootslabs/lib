#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![forbid(unsafe_code)]

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
    RadrootsMockRelayFetchAdapter, RadrootsNostrClientFetchAdapter, RadrootsRelayFetchAdapter,
    RadrootsRelayFetchEventReceipt, RadrootsRelayFetchFailure, RadrootsRelayFetchFilters,
    RadrootsRelayFetchItem, RadrootsRelayFetchMode, RadrootsRelayFetchOutcomeKind,
    RadrootsRelayFetchReceipt, RadrootsRelayFetchRelayOutcome, RadrootsRelayFetchRequest,
    RadrootsRelayFetchedEvent, RadrootsRelayFetchedEventsReceipt, fetch_and_ingest_relay_events,
    fetch_relay_events,
};
#[cfg(feature = "storage")]
pub use outbox::{
    RadrootsOutboxPublishPolicy, RadrootsOutboxPublishReceipt, publish_claimed_outbox_event,
};
pub use outcome::{RadrootsRelayOutcome, RadrootsRelayOutcomeKind};
#[cfg(feature = "client")]
pub use publish::RadrootsNostrClientPublishAdapter;
pub use publish::{
    RadrootsMockRelayPublishAdapter, RadrootsRelayPublishAdapter, RadrootsRelayPublishReceipt,
    RadrootsRelayPublishRelayReceipt, RadrootsRelayPublishRequest, publish_signed_event,
};
pub use relay::{RadrootsRelayTargetSet, RadrootsRelayUrl, RadrootsRelayUrlPolicy};
