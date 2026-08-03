#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]

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
