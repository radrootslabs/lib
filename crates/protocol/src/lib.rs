//! Versioned wire contracts for Radroots.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

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
