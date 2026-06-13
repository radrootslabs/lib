#![forbid(unsafe_code)]

mod error;
mod fetch;
mod outbox;
mod outcome;
mod publish;
mod relay;

pub use error::RadrootsRelayTransportError;
pub use fetch::{
    RadrootsMockRelayFetchAdapter, RadrootsRelayFetchAdapter, RadrootsRelayFetchEventReceipt,
    RadrootsRelayFetchItem, RadrootsRelayFetchMode, RadrootsRelayFetchReceipt,
    RadrootsRelayFetchRequest, fetch_and_ingest_relay_events,
};
pub use outbox::{
    RadrootsOutboxPublishPolicy, RadrootsOutboxPublishReceipt, publish_claimed_outbox_event,
};
pub use outcome::{RadrootsRelayOutcome, RadrootsRelayOutcomeKind};
pub use publish::{
    RadrootsMockRelayPublishAdapter, RadrootsNostrClientPublishAdapter,
    RadrootsRelayPublishAdapter, RadrootsRelayPublishReceipt, RadrootsRelayPublishRelayReceipt,
    RadrootsRelayPublishRequest, publish_signed_event,
};
pub use relay::{RadrootsRelayTargetSet, RadrootsRelayUrl, RadrootsRelayUrlPolicy};
