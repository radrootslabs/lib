#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]
#[cfg(not(feature = "std"))]
extern crate alloc;

pub mod canonical;
mod codec;
pub mod decode;
pub mod encode;
mod field_helpers;
#[cfg(feature = "manifests")]
pub mod manifest;
mod social_helpers;
pub mod verify;

// TEMPORARY COMPATIBILITY QUARANTINE (publish = false): these legacy domain
// paths remain source-visible only for first-party consumers scheduled for
// migration in Steps 288-294. They are hidden from the Release V1 API and
// must be removed at the final compatibility checkpoint, Step 313.
#[doc(hidden)]
pub mod app_data;
#[doc(hidden)]
pub mod article;
#[doc(hidden)]
pub mod calendar;
#[doc(hidden)]
pub mod comment;
#[doc(hidden)]
pub mod coop;
#[doc(hidden)]
pub mod d_tag;
#[doc(hidden)]
pub mod deletion;
#[doc(hidden)]
pub mod document;
#[doc(hidden)]
pub mod error;
#[doc(hidden)]
pub mod event_ref;
#[doc(hidden)]
pub mod farm;
#[doc(hidden)]
pub mod farm_crdt;
#[doc(hidden)]
pub mod farm_file;
#[doc(hidden)]
pub mod farm_workspace;
#[doc(hidden)]
pub mod file_metadata;
#[doc(hidden)]
pub mod follow;
#[doc(hidden)]
pub mod food_availability;
#[doc(hidden)]
pub mod geochat;
#[doc(hidden)]
pub mod gift_wrap;
#[doc(hidden)]
pub mod group;
#[doc(hidden)]
pub mod http_auth;
#[doc(hidden)]
pub mod job;
#[cfg(feature = "knowledge")]
#[doc(hidden)]
pub mod knowledge;
#[doc(hidden)]
pub mod list;
#[doc(hidden)]
pub mod list_set;
#[doc(hidden)]
pub mod message;
#[doc(hidden)]
pub mod message_file;
#[doc(hidden)]
pub mod operational_listing;
#[doc(hidden)]
pub mod order;
#[doc(hidden)]
pub mod parsed;
#[doc(hidden)]
pub mod plot;
#[doc(hidden)]
pub mod post;
#[doc(hidden)]
pub mod profile;
#[doc(hidden)]
pub mod reaction;
#[doc(hidden)]
pub mod relay_auth;
#[doc(hidden)]
pub mod reply;
#[doc(hidden)]
pub mod report;
#[doc(hidden)]
pub mod repost;
#[doc(hidden)]
pub mod resource_area;
#[doc(hidden)]
pub mod resource_cap;
#[doc(hidden)]
pub mod seal;
#[doc(hidden)]
pub mod tag_builders;
#[doc(hidden)]
pub mod trade;
#[doc(hidden)]
pub mod verification;
#[doc(hidden)]
pub mod wire;

pub use codec::Codec;
pub use decode::DecodeError;
pub use encode::EncodeError;
pub use verify::VerificationError;

#[cfg(feature = "json")]
pub mod admission;

#[cfg(test)]
mod test_fixtures;

#[cfg(feature = "json")]
#[doc(hidden)]
pub mod relay_document;
