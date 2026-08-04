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

mod app_data;
mod article;
mod calendar;
mod comment;
mod coop;
mod d_tag;
mod deletion;
mod document;
mod error;
mod event_ref;
mod farm;
mod farm_crdt;
mod farm_file;
mod farm_workspace;
mod file_metadata;
mod follow;
mod food_availability;
mod geochat;
mod gift_wrap;
mod group;
mod http_auth;
mod job;
#[cfg(feature = "knowledge")]
mod knowledge;
mod list;
mod list_set;
mod message;
mod message_file;
mod operational_listing;
mod order;
mod parsed;
mod plot;
mod post;
mod profile;
mod reaction;
mod relay_auth;
mod reply;
mod report;
mod repost;
mod resource_area;
mod resource_cap;
mod seal;
mod tag_builders;
mod trade;
mod verification;
mod wire;

pub use codec::Codec;
pub use decode::DecodeError;
pub use encode::EncodeError;
pub use verify::VerificationError;

#[cfg(feature = "json")]
pub mod admission;

#[cfg(test)]
mod test_fixtures;

#[cfg(feature = "json")]
mod relay_document;
