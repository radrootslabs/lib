#![forbid(unsafe_code)]
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc;

pub mod error;
mod canonical;
mod event_state;
mod geo;
pub mod emit;
pub mod ingest;
pub mod sync_state;
pub mod types;

pub use error::RadrootsTangleEventsError;
pub use emit::{
    radroots_tangle_sync_all,
    radroots_tangle_sync_all_with_options,
    radroots_tangle_farm_event,
    radroots_tangle_list_set_events,
    radroots_tangle_membership_claim_events,
    radroots_tangle_plot_events,
    radroots_tangle_profile_events,
};
pub use ingest::{
    radroots_tangle_ingest_event_with_factory,
    radroots_tangle_ingest_event_state,
    RadrootsTangleIngestOutcome,
    RadrootsTangleIdFactory,
};
pub use sync_state::{radroots_tangle_sync_status, RadrootsTangleSyncStatus};
pub use types::{
    RADROOTS_TANGLE_TRANSFER_VERSION,
    RadrootsTangleEventDraft,
    RadrootsTangleFarmSelector,
    RadrootsTangleSyncBundle,
    RadrootsTangleSyncOptions,
    RadrootsTangleSyncRequest,
};

#[cfg(feature = "std")]
pub use ingest::{radroots_tangle_ingest_event, RadrootsTangleDefaultIdFactory};

#[cfg(test)]
mod tests;
