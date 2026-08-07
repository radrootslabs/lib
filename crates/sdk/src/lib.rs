//! Host-neutral asynchronous client engine for Radroots.
//!
//! Advanced hosts compose capabilities through [`ClientBuilder`] and operate
//! through a cloneable [`Client`]. All fallible root operations use [`Result`]
//! and the SDK-owned [`Error`] boundary.
//!
//! Constructing an empty builder performs no I/O and makes missing composition
//! explicit:
//!
//! ```
//! use radroots_sdk::ClientBuilder;
//!
//! let result = ClientBuilder::new().build();
//! assert!(result.is_err());
//! ```
//!
//! Native structs are constructor-led and representation-private. Hosts must
//! not depend on field layout:
//!
//! ```compile_fail
//! use radroots_sdk::ClientBuilder;
//!
//! let _ = ClientBuilder { storage: None };
//! ```

#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

mod adapters;

pub mod capability;
pub mod client;
pub mod diagnostics;
pub mod error;
pub mod farm;
pub mod listing;
pub mod signing;
pub mod storage;
pub mod sync;
pub mod trade;
pub mod transport;

pub use crate::client::{Client, ClientBuilder};
pub use crate::error::{Error, Result};
