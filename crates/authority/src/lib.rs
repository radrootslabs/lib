#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
extern crate alloc;

pub mod actor;
pub mod authorization;
pub mod error;
#[cfg(feature = "local_signer")]
pub mod local_signer;
pub mod signer;

pub use actor::{
    RadrootsActorContext, RadrootsActorResolutionRequest, RadrootsActorSelector,
    RadrootsActorSource, role_satisfies,
};
pub use authorization::{
    authorize_actor_for_contract, authorize_actor_for_draft, authorize_signer_for_draft,
    sign_authorized_draft, validate_signed_event_matches_draft,
};
pub use error::{RadrootsAuthorityError, RadrootsSignerError};
#[cfg(feature = "local_signer")]
pub use local_signer::RadrootsLocalEventSigner;
pub use signer::{RadrootsEventSigner, RadrootsSignerIdentity};
