//! Host-neutral asynchronous client engine for Radroots.
//!
//! This checkpoint establishes the final package and module ownership
//! boundary. Client behavior is introduced through the ordered SDK refactor.

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
