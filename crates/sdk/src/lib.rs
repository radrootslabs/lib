//! Host-neutral asynchronous client engine for Radroots.
//!
//! Advanced hosts compose capabilities through [`ClientBuilder`] and operate
//! through a cloneable [`Client`]. All fallible root operations use [`Result`]
//! and the SDK-owned [`Error`] boundary.

#![forbid(unsafe_code)]

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
