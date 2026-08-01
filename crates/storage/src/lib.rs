//! Backend-neutral persistence abstractions for Radroots.

#![forbid(unsafe_code)]

pub mod atomic;
pub mod backup;
pub mod event;
pub mod journal;
#[cfg(feature = "memory")]
pub mod memory;
pub mod outbox;
pub mod private_artifact;
pub mod projection;
pub mod status;

pub use event::{Error, EventStore};
