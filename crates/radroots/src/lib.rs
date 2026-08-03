#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! Curated ordinary-user entry point for Radroots.
//!
//! The root exposes only the ordinary client boundary. Deliberately selected
//! domain types live in the named modules below.

pub mod client;
pub mod event;
pub mod farm;
pub mod identity;
pub mod knowledge;
pub mod listing;
pub mod signing;
pub mod storage;
pub mod sync;
pub mod trade;
pub mod transport;

pub use radroots_sdk::{Client, ClientBuilder, Error, Result};
