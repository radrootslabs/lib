#![doc = include_str!("../README.md")]
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod account;
mod error;
pub mod key;
pub mod profile;
pub mod username;

pub use account::AccountId;
pub use error::Error;
pub use key::{IdentityId, PublicKey};
pub use profile::{Profile, PublicIdentity};
pub use username::Username;
