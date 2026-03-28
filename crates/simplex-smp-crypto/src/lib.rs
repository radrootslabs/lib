#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod auth;
pub mod error;
pub mod ratchet;

pub mod prelude {
    pub use crate::auth::{
        RadrootsSimplexSmpQueueAuthorizationMaterial, RadrootsSimplexSmpQueueAuthorizationScope,
    };
    pub use crate::error::RadrootsSimplexSmpCryptoError;
    pub use crate::ratchet::{
        RadrootsSimplexSmpRatchetHeader, RadrootsSimplexSmpRatchetRole,
        RadrootsSimplexSmpRatchetState,
    };
}
