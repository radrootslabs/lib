#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod error;
pub mod net;

#[cfg(feature = "std")]
pub mod logging;

pub mod builder;
pub mod config;

#[cfg(feature = "nostr-client")]
pub mod keys;

#[cfg(feature = "nostr-client")]
pub mod nostr_client;

pub use net::{Net, NetHandle, NetInfo};
