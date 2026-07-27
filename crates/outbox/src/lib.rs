#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![forbid(unsafe_code)]

mod error;
#[cfg(feature = "sqlite")]
mod generated;
#[cfg(feature = "sqlite")]
mod migrations;
mod model;
#[cfg(feature = "sqlite")]
mod phase1_publication;
#[cfg(feature = "sqlite")]
mod schema;
#[cfg(feature = "sqlite")]
mod sqlite_lifecycle;
#[cfg(feature = "sqlite")]
mod store;

pub use error::RadrootsOutboxError;
#[cfg(feature = "sqlite")]
pub use migrations::{RADROOTS_OUTBOX_SCHEMA_VERSION_CURRENT, RADROOTS_OUTBOX_SCHEMA_VERSION_MIN};
pub use model::{
    RadrootsOutboxClaimedEvent, RadrootsOutboxDeliveryAttemptRecord,
    RadrootsOutboxDeliveryPlanInput, RadrootsOutboxDeliveryPlanRecord,
    RadrootsOutboxDeliveryPlanStatus, RadrootsOutboxDeliveryTargetRecord,
    RadrootsOutboxDeliveryTargetStatus, RadrootsOutboxEnqueueReceipt, RadrootsOutboxEnqueueStatus,
    RadrootsOutboxEventRecord, RadrootsOutboxEventState, RadrootsOutboxEventStoreIngestReceipt,
    RadrootsOutboxIdempotencyPreflight, RadrootsOutboxOperationInput,
    RadrootsOutboxOperationRecord, RadrootsOutboxOperationStatus, RadrootsOutboxReticulumBehavior,
    RadrootsOutboxReticulumEventRecord, RadrootsOutboxSignedOperationInput,
    RadrootsOutboxSignedTradeMutationInput, RadrootsOutboxStatusSummary,
    RadrootsOutboxTradeMutationInput,
};
#[cfg(feature = "sqlite")]
pub use phase1_publication::{
    RADROOTS_PHASE1_PUBLICATION_CLAIM_LEASE_MAX_MILLIS,
    RADROOTS_PHASE1_PUBLICATION_DIAGNOSTIC_MAX_BYTES, RADROOTS_PHASE1_PUBLICATION_ERROR_CODES,
    RADROOTS_PHASE1_PUBLICATION_TARGET_MAX_COUNT, RADROOTS_PHASE1_PUBLICATION_TARGET_URI_MAX_BYTES,
    RADROOTS_PHASE1_PUBLICATION_TRANSITIONS, RadrootsPhase1PublicationClaim,
    RadrootsPhase1PublicationEnqueueReceipt, RadrootsPhase1PublicationEnqueueStatus,
    RadrootsPhase1PublicationError, RadrootsPhase1PublicationEventState,
    RadrootsPhase1PublicationRecord, RadrootsPhase1PublicationSigningPreflight,
    RadrootsPhase1PublicationTarget, RadrootsPhase1PublicationTargetClaim,
    RadrootsPhase1PublicationTargetPolicy, RadrootsPhase1PublicationTargetState,
    RadrootsPhase1PublicationTransition, RadrootsPhase1PublicationTransitionRetryClass,
    RadrootsPhase1PublicationTransitionScope,
};
#[cfg(feature = "sqlite")]
pub use schema::{RadrootsOutboxSchemaStatus, inspect_outbox_schema_status};
#[cfg(feature = "sqlite")]
pub use sqlite_lifecycle::{
    RADROOTS_OUTBOX_DIAGNOSTIC_BYTES_MAX, RADROOTS_OUTBOX_FILE_CONNECTION_LIMIT,
    RADROOTS_OUTBOX_FILE_PATH_BYTES_MAX, RADROOTS_OUTBOX_OPEN_DEADLINE_MILLIS,
    RadrootsOutboxRollbackConfirmation,
};
#[cfg(feature = "sqlite")]
pub use store::RadrootsOutbox;
