#![doc = include_str!("../README.md")]
#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
extern crate alloc;

pub mod actor;
pub mod authorization;
pub mod capability;
pub mod error;
pub mod identity;
pub mod receipt;
pub mod recovery;
pub mod request;
pub mod signer;
pub mod status;

pub use actor::Actor;
pub use authorization::{CurrentAuthoringAuthority, CurrentAuthoringDecision};
pub use error::Error;
pub use identity::{AuthoredArtifactId, SignerRequestId, SigningIntentId, SigningOperationId};
pub use receipt::SignReceipt;
pub use request::SignRequest;
pub use signer::Signer;
pub use status::SignerStatus;
