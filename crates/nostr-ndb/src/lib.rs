#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
compile_error!("radroots-nostr-ndb requires the std feature");

extern crate alloc;

#[cfg(feature = "ndb")]
pub mod config;
pub mod error;
#[cfg(feature = "ndb")]
pub mod filter;
#[cfg(feature = "ndb")]
pub mod ingest;
#[cfg(feature = "ndb")]
pub mod ndb;
#[cfg(feature = "ndb")]
pub mod query;
#[cfg(all(feature = "ndb", feature = "runtime-adapter"))]
pub mod runtime_adapter;
#[cfg(feature = "ndb")]
pub mod subscription;

pub mod prelude {
    #[cfg(feature = "ndb")]
    pub use crate::config::RadrootsNostrNdbConfig;
    pub use crate::error::RadrootsNostrNdbError;
    #[cfg(feature = "ndb")]
    pub use crate::filter::RadrootsNostrNdbFilterSpec;
    #[cfg(feature = "ndb")]
    pub use crate::ingest::RadrootsNostrNdbIngestSource;
    #[cfg(feature = "ndb")]
    pub use crate::ndb::RadrootsNostrNdb;
    #[cfg(feature = "ndb")]
    pub use crate::query::{
        RadrootsNostrNdbNote, RadrootsNostrNdbProfile, RadrootsNostrNdbQuerySpec,
    };
    #[cfg(all(feature = "ndb", feature = "runtime-adapter"))]
    pub use crate::runtime_adapter::RadrootsNostrNdbEventStoreAdapter;
    #[cfg(all(feature = "ndb", feature = "rt"))]
    pub use crate::subscription::RadrootsNostrNdbSubscriptionStream;
    #[cfg(feature = "ndb")]
    pub use crate::subscription::{
        RadrootsNostrNdbNoteKey, RadrootsNostrNdbSubscriptionHandle,
        RadrootsNostrNdbSubscriptionSpec,
    };
}
