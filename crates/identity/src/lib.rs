#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod account;
pub mod error;
pub mod key;
pub mod profile;
#[cfg(feature = "json-file")]
pub mod storage;
pub mod username;

pub use account::AccountId;
pub use error::Error;
#[cfg(feature = "std")]
pub use error::IdentityError;
pub use key::{IdentityId, PublicKey};
pub use profile::{Profile, PublicIdentity};
#[cfg(feature = "json-file")]
pub use storage::{load_identity_profile, store_identity_profile};
pub use username::Username;
