#![doc = include_str!("../README.md")]
#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

/// Closed known-kind chat command data and canonical digest input. These
/// values authenticate no principal and perform no command admission.
#[cfg(feature = "commands")]
pub mod command;

/// Exact canonical NIP-01 bytes and complete serialization budgets.
#[cfg(feature = "nip01")]
pub mod nip01;

/// Bounded known-kind private-chat data; no admission or route authority.
#[cfg(feature = "chat")]
pub mod chat;

#[cfg(all(test, not(feature = "std")))]
extern crate std;

/// Versioned capability catalog contracts.
pub mod capability;

/// Versioned stable error-report contracts.
pub mod error;

/// Versioned event wire contracts.
pub mod event;

/// Versioned daemon protocol contracts.
pub mod radrootsd;

/// Versioned runtime operation contracts.
pub mod runtime;

/// Schema identity and structural validation contracts.
pub mod schema;
