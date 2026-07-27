#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![forbid(unsafe_code)]

mod error;
#[cfg(feature = "sqlite")]
mod generated;
#[cfg(feature = "sqlite")]
mod migrations;
mod model;
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
pub use schema::{RadrootsOutboxSchemaStatus, inspect_outbox_schema_status};
#[cfg(feature = "sqlite")]
pub use sqlite_lifecycle::{
    RADROOTS_OUTBOX_DIAGNOSTIC_BYTES_MAX, RADROOTS_OUTBOX_FILE_CONNECTION_LIMIT,
    RADROOTS_OUTBOX_FILE_PATH_BYTES_MAX, RADROOTS_OUTBOX_OPEN_DEADLINE_MILLIS,
    RadrootsOutboxRollbackConfirmation,
};
#[cfg(feature = "sqlite")]
pub use store::RadrootsOutbox;
