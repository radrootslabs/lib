#![forbid(unsafe_code)]

#[cfg(feature = "sqlite")]
mod error;
#[cfg(feature = "sqlite")]
mod migrations;
#[cfg(feature = "sqlite")]
mod model;
#[cfg(feature = "sqlite")]
mod store;

#[cfg(feature = "sqlite")]
pub use error::RadrootsEventStoreError;
#[cfg(feature = "sqlite")]
pub use migrations::{EVENT_STORE_MIGRATION_DOWN, EVENT_STORE_MIGRATION_UP};
#[cfg(feature = "sqlite")]
pub use model::{
    RadrootsEventContractStatus, RadrootsEventHeadStoreDecision, RadrootsEventIngest,
    RadrootsEventIngestReceipt, RadrootsEventVerificationStatus, RadrootsProjectionCursor,
    RadrootsRelayObservation, RadrootsRelayObservationType, RadrootsStoredEvent,
    RadrootsStoredEventHead, RadrootsStoredEventTag, StoredEventClass,
};
#[cfg(feature = "sqlite")]
pub use store::RadrootsEventStore;
