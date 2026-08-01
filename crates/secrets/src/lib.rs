//! Secret material and protected-storage abstractions for Radroots.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod envelope;
pub mod error;
#[cfg(feature = "file")]
pub mod file;
pub mod id;
#[cfg(feature = "keyring")]
pub mod keyring;
#[cfg(feature = "memory")]
pub mod memory;
pub mod provider;
pub mod wrapping;

pub use error::Error;
pub use id::{SecretId, SecretRef};
