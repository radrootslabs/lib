#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;
#[cfg(not(feature = "std"))]
extern crate core as std;

#[cfg(feature = "blossom")]
pub mod blossom;

pub mod error;
pub mod event;
pub mod events;
pub mod filter;
pub mod key;
pub mod tag;
mod tags;
pub mod types;
pub mod util;

pub use error::RadrootsNostrError as Error;

#[cfg(test)]
mod test_fixtures;

#[cfg(feature = "codec")]
pub mod codec_adapters;

#[cfg(feature = "codec")]
pub mod job_adapter;

#[cfg(feature = "nip17")]
pub mod nip17;

#[cfg(feature = "signing")]
pub mod signing;

#[cfg(feature = "events")]
pub mod event_adapters;

#[cfg(feature = "events")]
pub mod draft_signing;
#[cfg(feature = "events")]
mod event_convert;
#[cfg(feature = "events")]
pub mod event_verify;
