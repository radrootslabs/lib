#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![forbid(unsafe_code)]

#[cfg(feature = "sqlite")]
mod error;
#[cfg(feature = "sqlite")]
mod migrations;
#[cfg(feature = "sqlite")]
mod model;
#[cfg(feature = "sqlite")]
mod schema;
#[cfg(feature = "sqlite")]
mod store;

#[cfg(feature = "sqlite")]
pub use error::RadrootsEventStoreError;
#[cfg(feature = "sqlite")]
pub use migrations::{
    RADROOTS_EVENT_STORE_SCHEMA_VERSION_CURRENT, RADROOTS_EVENT_STORE_SCHEMA_VERSION_MIN,
};
#[cfg(feature = "sqlite")]
pub use model::{
    RadrootsEventAdmissionStatus, RadrootsEventIngest, RadrootsEventIngestReceipt,
    RadrootsEventPersistence, RadrootsEventStoreStatusSummary, RadrootsEventVisibility,
    RadrootsProjectionCursor, RadrootsRawHeadDecision, RadrootsStoredEventTag,
    RadrootsStoredRawEvent, RadrootsStoredRawEventHead, RadrootsStoredSellerReservation,
    RadrootsStoredSellerReservationLine, RadrootsStoredTradeMissingParent,
    RadrootsStoredTradeMutation, RadrootsStoredTradeMutationParent,
    RadrootsStoredTradeTransportEnvelope, RadrootsStoredValidEvent, RadrootsStoredVisibleEvent,
    RadrootsStoredVisibleEventHead, RadrootsTradeProjectionCheckpoint,
    RadrootsTransportObservation, RadrootsTransportObservationType, StoredEventClass,
};
#[cfg(feature = "sqlite")]
pub use schema::{RadrootsEventStoreSchemaStatus, inspect_event_store_schema_status};
#[cfg(feature = "sqlite")]
pub use store::{
    RADROOTS_EVENT_STORE_CONTRACT_QUERY_LIMIT_MAX, RADROOTS_EVENT_STORE_QUERY_LIMIT_MAX,
    RadrootsEventStore, RadrootsTransportObservationRow, inspect_event_store_status,
};
