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
    radroots_tangle_farm_event, radroots_tangle_list_set_events,
    radroots_tangle_membership_claim_events, radroots_tangle_plot_events,
    radroots_tangle_profile_events, radroots_tangle_sync_all,
    radroots_tangle_sync_all_with_options,
};
pub use error::RadrootsTangleEventsError;
pub use ingest::{
    RadrootsTangleIdFactory, RadrootsTangleIngestOutcome, radroots_tangle_ingest_event_state,
    radroots_tangle_ingest_event_with_factory,
};
pub use sync_state::{RadrootsTangleSyncStatus, radroots_tangle_sync_status};
pub use types::{
    RADROOTS_TANGLE_TRANSFER_VERSION, RadrootsTangleEventDraft, RadrootsTangleFarmSelector,
    RadrootsTangleSyncBundle, RadrootsTangleSyncOptions, RadrootsTangleSyncRequest,
};

#[cfg(feature = "std")]
pub use ingest::{RadrootsTangleDefaultIdFactory, radroots_tangle_ingest_event};

#[cfg(test)]
mod tests;
