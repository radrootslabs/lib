#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#[cfg(not(feature = "std"))]
extern crate alloc;

pub mod error;
pub mod d_tag;
pub mod event_ref;
pub mod job;
pub mod profile;
pub mod tag_builders;
pub mod wire;

pub mod comment;
pub mod follow;
pub mod app_data;
pub mod document;
pub mod coop;
pub mod farm;
pub mod resource_area;
pub mod resource_cap;
pub mod gift_wrap;
pub mod message;
pub mod message_file;
pub mod post;
pub mod plot;
pub mod reaction;
pub mod seal;

pub mod listing;
pub mod list;
pub mod list_set;

#[cfg(feature = "serde_json")]
pub mod relay_document;

pub use tag_builders::RadrootsEventTagBuilder;
