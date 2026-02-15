#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#[cfg(not(feature = "std"))]
extern crate alloc;

pub mod d_tag;
pub mod error;
pub mod event_ref;
pub mod job;
pub mod profile;
pub mod tag_builders;
pub mod wire;

pub mod app_data;
pub mod comment;
pub mod coop;
pub mod document;
pub mod farm;
pub mod follow;
pub mod geochat;
pub mod gift_wrap;
pub mod message;
pub mod message_file;
pub mod plot;
pub mod post;
pub mod reaction;
pub mod resource_area;
pub mod resource_cap;
pub mod seal;

pub mod list;
pub mod list_set;
pub mod listing;

#[cfg(feature = "serde_json")]
pub mod relay_document;

pub use tag_builders::RadrootsEventTagBuilder;
