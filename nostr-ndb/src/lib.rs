#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
compile_error!("radroots-nostr-ndb requires the std feature");

extern crate alloc;

pub mod error;

pub mod prelude {
    pub use crate::error::RadrootsNostrNdbError;
}
