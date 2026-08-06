#![doc = include_str!("../README.md")]
#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;
#[cfg(not(feature = "std"))]
extern crate core as std;

#[cfg(feature = "blossom")]
pub mod blossom;

mod error;
pub mod event;
#[cfg(feature = "events")]
mod events;
pub mod filter;
pub mod key;
pub mod tag;
mod tags;
mod types;
#[cfg(feature = "events")]
mod util;

pub use error::Error;

#[cfg(test)]
mod test_fixtures;

#[cfg(feature = "events")]
mod codec_adapters;

#[cfg(feature = "events")]
mod job_adapter;

#[cfg(feature = "nip17")]
pub mod nip17;

#[cfg(feature = "signing")]
pub mod signing;

#[cfg(feature = "events")]
mod event_convert;
#[cfg(feature = "events")]
mod event_verify;
#[cfg(feature = "events")]
mod plan_signing;
