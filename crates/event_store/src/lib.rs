#![doc(hidden)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![forbid(unsafe_code)]

#[cfg(feature = "sqlite")]
mod error;
#[cfg(feature = "sqlite")]
mod generated;
#[cfg(feature = "sqlite")]
mod migrations;
#[cfg(feature = "sqlite")]
mod model;
#[cfg(feature = "sqlite")]
mod nip09;
#[cfg(feature = "sqlite")]
mod schema;
#[cfg(feature = "sqlite")]
mod source_maintenance_v1;
#[cfg(feature = "sqlite")]
mod store;

#[cfg(feature = "sqlite")]
pub use error::{
    RADROOTS_EVENT_STORE_RAW_EVENT_COUNT_LIMIT_V1,
    RADROOTS_EVENT_STORE_RAW_EVENT_TEXT_BYTES_LIMIT_V1,
    RADROOTS_EVENT_STORE_RAW_TAG_COUNT_LIMIT_V1, RADROOTS_EVENT_STORE_RAW_TAG_TEXT_BYTES_LIMIT_V1,
    RADROOTS_EVENT_STORE_RETAINED_SOURCE_GENERATION_LIMIT_V1, RadrootsEventStoreError,
    RadrootsEventStoreSourceCapacityResourceV1,
};
#[cfg(feature = "sqlite")]
pub use migrations::{
    RADROOTS_EVENT_STORE_SCHEMA_VERSION_CURRENT, RADROOTS_EVENT_STORE_SCHEMA_VERSION_MIN,
};
#[cfg(feature = "sqlite")]
pub use model::{
    RADROOTS_ADDRESSABLE_TRANSITION_CURSOR_JSON_MAX_BYTES_V1,
    RADROOTS_ADDRESSABLE_TRANSITION_D_TAG_MAX_BYTES_V1,
    RADROOTS_ADDRESSABLE_TRANSITION_FEED_VERSION_V1,
    RADROOTS_ADDRESSABLE_TRANSITION_PAGE_LIMIT_MAX_V1,
    RADROOTS_ADDRESSABLE_TRANSITION_PAGE_RAW_JSON_MAX_BYTES_V1,
    RADROOTS_ADDRESSABLE_TRANSITION_PAGE_SCAN_MAX_V1,
    RADROOTS_ADDRESSABLE_TRANSITION_SCOPE_KIND_MAX_V1,
    RADROOTS_FOOD_AVAILABILITY_PROJECTION_APPLY_PAGE_LIMIT_V1,
    RADROOTS_FOOD_AVAILABILITY_PROJECTION_VERSION_V1,
    RADROOTS_FOOD_AVAILABILITY_SEARCH_QUERY_MAX_BYTES_V1,
    RADROOTS_FOOD_AVAILABILITY_SEARCH_QUERY_MAX_TERMS_V1,
    RADROOTS_TRANSPORT_OBSERVATION_MESSAGE_MAX_BYTES, RadrootsAddressableTransitionCauseV1,
    RadrootsAddressableTransitionCoordinateV1, RadrootsAddressableTransitionCursorV1,
    RadrootsAddressableTransitionEventReferenceV1, RadrootsAddressableTransitionOriginV1,
    RadrootsAddressableTransitionPageV1, RadrootsAddressableTransitionRawHeadDecisionV1,
    RadrootsAddressableTransitionScopeFingerprintV1, RadrootsAddressableTransitionScopeV1,
    RadrootsAddressableTransitionV1, RadrootsAddressableTransitionVisibilityV1,
    RadrootsCurrentEventVisibilityV1, RadrootsCurrentVisibilityDecisionV1,
    RadrootsEventAdmissionStatus, RadrootsEventIngest, RadrootsEventIngestReceipt,
    RadrootsEventPersistence, RadrootsEventStoreSourceGeneration, RadrootsEventStoreStatusSummary,
    RadrootsEventVisibility, RadrootsFoodAvailabilitySearchQueryV1,
    RadrootsFoodAvailabilityStatusFilterV1, RadrootsNip09SuppressionEvidenceV1,
    RadrootsNip09SuppressionOutcome, RadrootsNip09SuppressionReason, RadrootsProjectionCursor,
    RadrootsProjectionRebuildPrior, RadrootsProjectionRebuildTicket, RadrootsRawHeadDecision,
    RadrootsStoreProducedCanonicalEventV1, RadrootsStoredEventTag,
    RadrootsStoredFoodAvailabilityImageV1, RadrootsStoredFoodAvailabilityV1,
    RadrootsStoredRawEvent, RadrootsStoredRawEventHead, RadrootsStoredSellerReservation,
    RadrootsStoredSellerReservationLine, RadrootsStoredTradeMissingParent,
    RadrootsStoredTradeMutation, RadrootsStoredTradeMutationParent,
    RadrootsStoredTradeTransportEnvelope, RadrootsStoredValidEvent, RadrootsStoredVisibleEvent,
    RadrootsStoredVisibleEventHead, RadrootsTradeProjectionCheckpoint,
    RadrootsTransportObservation, RadrootsTransportObservationMessage,
    RadrootsTransportObservationType, StoredEventClass,
};
#[cfg(feature = "sqlite")]
pub use schema::{RadrootsEventStoreSchemaStatus, inspect_event_store_schema_status};
#[cfg(feature = "sqlite")]
pub use source_maintenance_v1::RadrootsEventStoreSourceCapacityV1;
#[cfg(feature = "sqlite")]
pub use store::{
    RADROOTS_EVENT_STORE_CONTRACT_QUERY_LIMIT_MAX, RADROOTS_EVENT_STORE_QUERY_LIMIT_MAX,
    RadrootsEventStore, RadrootsTransportObservationRow, inspect_event_store_status,
};
