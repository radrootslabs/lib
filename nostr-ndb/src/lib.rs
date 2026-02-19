#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
compile_error!("radroots-nostr-ndb requires the std feature");

extern crate alloc;

#[cfg(feature = "ndb")]
pub mod config;
pub mod error;
#[cfg(feature = "ndb")]
pub mod ingest;
#[cfg(feature = "ndb")]
pub mod ndb;

pub mod prelude {
    #[cfg(feature = "ndb")]
    pub use crate::config::RadrootsNostrNdbConfig;
    pub use crate::error::RadrootsNostrNdbError;
    #[cfg(feature = "ndb")]
    pub use crate::ingest::RadrootsNostrNdbIngestSource;
    #[cfg(feature = "ndb")]
    pub use crate::ndb::RadrootsNostrNdb;
}
