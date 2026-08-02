//! Publish-frozen compatibility implementation. Use `radroots_storage` and
//! `radroots_storage_sqlite` for new integrations. Step 313 removes this package.
#![doc(hidden)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![forbid(unsafe_code)]

mod error;
mod migrations;
mod model;
mod store;

pub use error::RadrootsOutboxError;
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
pub use store::RadrootsOutbox;
