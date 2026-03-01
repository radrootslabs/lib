#![forbid(unsafe_code)]
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc;

mod canonical;
pub mod emit;
pub mod error;
mod event_state;
mod geo;
pub mod ingest;
pub mod sync_state;
pub mod types;

pub use emit::{
    radroots_replica_farm_event, radroots_replica_list_set_events,
    radroots_replica_membership_claim_events, radroots_replica_plot_events,
    radroots_replica_profile_events, radroots_replica_sync_all,
    radroots_replica_sync_all_with_options,
};
pub use error::RadrootsReplicaEventsError;
pub use ingest::{
    RadrootsReplicaIdFactory, RadrootsReplicaIngestOutcome, radroots_replica_ingest_event_state,
    radroots_replica_ingest_event_with_factory,
};
pub use sync_state::{RadrootsReplicaSyncStatus, radroots_replica_sync_status};
pub use types::{
    RADROOTS_REPLICA_TRANSFER_VERSION, RadrootsReplicaEventDraft, RadrootsReplicaFarmSelector,
    RadrootsReplicaSyncBundle, RadrootsReplicaSyncOptions, RadrootsReplicaSyncRequest,
};

#[cfg(feature = "std")]
pub use ingest::{RadrootsReplicaDefaultIdFactory, radroots_replica_ingest_event};

#[cfg(test)]
mod tests;
