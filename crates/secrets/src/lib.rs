#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![deny(rustdoc::broken_intra_doc_links)]
#![doc = include_str!("../README.md")]

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

pub use envelope::EncryptedEnvelope;
pub use error::Error;
pub use id::{SecretId, SecretRef};
pub use provider::SecretProvider;
pub use wrapping::KeyWrapping;
