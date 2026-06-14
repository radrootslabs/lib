#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
extern crate alloc;

pub mod actor;
pub mod error;
pub mod signer;

pub use actor::{
    RadrootsActorContext, RadrootsActorResolutionRequest, RadrootsActorSelector,
    RadrootsActorSource, role_satisfies,
};
pub use error::RadrootsAuthorityError;
pub use signer::RadrootsSignerIdentity;
