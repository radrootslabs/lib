#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! Curated ordinary-user entry point for Radroots.
//!
//! This non-publishable scaffold reserves the final module and feature
//! vocabulary without exposing private migration packages.

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
