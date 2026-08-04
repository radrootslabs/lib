#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

mod auth;
mod client;
mod error;
mod relay;
mod sink;
mod source;
mod status;

pub use client::{Config, NostrTransport};
pub use error::Error;
pub use relay::{RelayUrl, RelayUrlPolicy};
