#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![forbid(unsafe_code)]
//! Portable public identity and account values.
//!
//! Host filesystem and runtime-path APIs are intentionally absent:
//!
//! ```compile_fail
//! use radroots_identity::{storage, IdentityError};
//! ```
//!
//! Secret-bearing and upstream Nostr event APIs are intentionally absent:
//!
//! ```compile_fail
//! use radroots_identity::{NostrEvent, SecretKey};
//! ```

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
