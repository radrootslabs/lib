#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![forbid(unsafe_code)]

mod error;
#[cfg(feature = "sqlite")]
mod migrations;
mod model;
#[cfg(feature = "sqlite")]
mod store;

pub use error::RadrootsOutboxError;
#[cfg(feature = "sqlite")]
pub use migrations::{OUTBOX_MIGRATION_DOWN, OUTBOX_MIGRATION_UP};
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
pub use store::RadrootsOutbox;
