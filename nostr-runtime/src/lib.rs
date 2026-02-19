#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod error;
pub mod types;

#[cfg(all(feature = "nostr-client", feature = "rt"))]
pub mod runtime;

pub mod prelude {
    pub use crate::error::RadrootsNostrRuntimeError;
    #[cfg(all(feature = "nostr-client", feature = "rt"))]
    pub use crate::runtime::{RadrootsNostrRuntime, RadrootsNostrRuntimeBuilder};
    pub use crate::types::{
        RadrootsNostrConnectionSnapshot, RadrootsNostrRuntimeEvent,
        RadrootsNostrSubscriptionHandle, RadrootsNostrSubscriptionPolicy,
        RadrootsNostrSubscriptionSpec, RadrootsNostrTrafficLight,
    };
}
