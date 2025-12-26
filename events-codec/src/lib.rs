#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#[cfg(not(feature = "std"))]
extern crate alloc;

pub mod error;
pub mod event_ref;
pub mod job;
pub mod profile;
pub mod tag_builders;
pub mod wire;

pub mod comment;
pub mod follow;
pub mod message;
pub mod post;
pub mod reaction;

pub mod listing;

#[cfg(feature = "serde_json")]
pub mod relay_document;

pub use tag_builders::RadrootsEventTagBuilder;
