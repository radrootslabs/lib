#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
compile_error!("radroots_nostrdb requires the std feature");

extern crate alloc;

#[cfg(test)]
mod test_fixtures;

#[cfg(feature = "nostrdb")]
pub mod config;
pub mod error;
#[cfg(feature = "nostrdb")]
pub mod filter;
#[cfg(feature = "nostrdb")]
pub mod ingest;
#[cfg(feature = "nostrdb")]
pub mod nostrdb;
#[cfg(feature = "nostrdb")]
pub mod query;
#[cfg(all(feature = "nostrdb", feature = "runtime-adapter"))]
pub mod runtime_adapter;
#[cfg(feature = "nostrdb")]
pub mod subscription;

pub mod prelude {
    #[cfg(feature = "nostrdb")]
    pub use crate::config::RadrootsNostrdbConfig;
    pub use crate::error::RadrootsNostrdbError;
    #[cfg(feature = "nostrdb")]
    pub use crate::filter::RadrootsNostrdbFilterSpec;
    #[cfg(feature = "nostrdb")]
    pub use crate::ingest::RadrootsNostrdbIngestSource;
    #[cfg(feature = "nostrdb")]
    pub use crate::nostrdb::RadrootsNostrdb;
    #[cfg(feature = "nostrdb")]
    pub use crate::query::{RadrootsNostrdbNote, RadrootsNostrdbProfile, RadrootsNostrdbQuerySpec};
    #[cfg(all(feature = "nostrdb", feature = "runtime-adapter"))]
    pub use crate::runtime_adapter::RadrootsNostrdbEventSinkAdapter;
    #[cfg(all(feature = "nostrdb", feature = "rt"))]
    pub use crate::subscription::RadrootsNostrdbSubscriptionStream;
    #[cfg(feature = "nostrdb")]
    pub use crate::subscription::{
        RadrootsNostrdbNoteKey, RadrootsNostrdbSubscriptionHandle, RadrootsNostrdbSubscriptionSpec,
    };
}
