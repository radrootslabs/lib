#![forbid(unsafe_code)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(feature = "std")]
mod canonical;
#[cfg(feature = "std")]
pub mod emit;
pub mod error;
#[cfg(feature = "std")]
mod event_head;
#[cfg(feature = "std")]
mod geo;
#[cfg(feature = "legacy-ingest")]
pub mod ingest;
#[cfg(feature = "std")]
pub mod sync_state;
pub mod types;

#[cfg(feature = "std")]
pub use emit::{
    radroots_replica_farm_event, radroots_replica_list_set_events,
    radroots_replica_membership_claim_events, radroots_replica_plot_events,
    radroots_replica_sync_all, radroots_replica_sync_all_with_options,
};
pub use error::RadrootsReplicaEventsError;
#[cfg(feature = "legacy-ingest")]
pub use ingest::{
    RadrootsReplicaIdFactory, RadrootsReplicaIngestOutcome, radroots_replica_ingest_event_head,
    radroots_replica_ingest_event_with_factory,
};
#[cfg(feature = "std")]
pub use sync_state::{
    RadrootsReplicaPendingPublishBatch, RadrootsReplicaPendingPublishEvent,
    RadrootsReplicaSyncStatus, radroots_replica_pending_publish_batch,
    radroots_replica_sync_status,
};
pub use types::{
    RADROOTS_REPLICA_TRANSFER_VERSION, RadrootsReplicaEventDraft, RadrootsReplicaFarmSelector,
    RadrootsReplicaSyncBundle, RadrootsReplicaSyncOptions, RadrootsReplicaSyncRequest,
};

#[cfg(feature = "legacy-ingest")]
pub use ingest::{RadrootsReplicaDefaultIdFactory, radroots_replica_ingest_event};

#[cfg(all(test, feature = "std"))]
mod tests;
