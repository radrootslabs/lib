#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![cfg_attr(all(not(feature = "std"), not(test)), no_std)]
#![forbid(unsafe_code)]
#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

pub const RADROOTS_EVENT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
/// Returns deterministic 64-character fixtures that are also valid secp256k1
/// x-only public keys; labels without a curve point are remapped.
pub(crate) fn test_valid_hex_64(character: char) -> String {
    if matches!(character, 'b' | 'B') {
        let value = "2f8bde4d1a07209355b4a7250a5c5128e88b84bddc619ab7cba8d569b240efe4";
        return if character.is_ascii_uppercase() {
            value.to_ascii_uppercase()
        } else {
            value.to_owned()
        };
    }
    let character = match character {
        '0' => '7',
        '1' => '8',
        '5' => 'd',
        '6' => 'e',
        '9' => 'a',
        'c' | 'C' => '3',
        'f' | 'F' => '4',
        other => other,
    };
    core::iter::repeat_n(character, 64).collect()
}

pub mod account;
pub mod admission;
pub mod app_data;
pub mod article;
pub mod calendar;
pub mod classified_listing;
pub mod comment;
pub mod contract;
pub mod coop;
pub mod deletion;
pub mod document;
pub mod draft;
#[cfg(feature = "dto-bindgen")]
pub mod dto;
pub mod envelope;
pub mod event_head;
pub mod farm;
pub mod farm_crdt;
pub mod farm_file;
pub mod farm_workspace;
pub mod file_metadata;
pub mod follow;
pub mod food;
pub mod food_availability;
pub mod gcs;
pub mod geochat;
pub mod gift_wrap;
pub mod group;
pub mod http_auth;
pub mod id;
pub mod ids;
pub mod job;
pub mod job_feedback;
pub mod job_request;
pub mod job_result;
pub mod kinds;
#[cfg(feature = "knowledge")]
pub mod knowledge;
pub mod list;
pub mod list_set;
pub mod listing;
pub mod location;
pub mod media;
pub mod message;
pub mod message_file;
pub mod operational_listing;
pub mod order;
pub mod order_economics;
pub mod plot;
pub mod post;
pub mod profile;
pub mod reaction;
pub mod relay_auth;
pub mod relay_document;
pub mod relay_hint;
pub mod reply;
pub mod report;
pub mod repost;
pub mod resource_area;
pub mod resource_cap;
pub mod seal;
pub mod social;
pub mod tag;
pub mod tags;
pub mod trade;
pub mod trade_validation;
pub mod wire;

pub use envelope::{
    RadrootsEventEnvelope, RadrootsEventEnvelopeError, RadrootsEventEnvelopeLimits,
    RadrootsEventEnvelopeParts, RadrootsEventKind, RadrootsEventKindClass, RadrootsEventTag,
    RadrootsEventTags, RadrootsEventTimestamp,
};
pub use media::{RadrootsAuthoredImage, RadrootsAuthoredImageError};
pub use wire::{
    RadrootsCanonicalEventIdError, RadrootsEventWireError, RadrootsEventWireLimits,
    RadrootsNip01EventWire, canonical_nip01_event_id_preimage, compute_canonical_nip01_event_id,
};

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsEventRef {
    pub id: String,
    pub author: String,
    pub kind: u32,
    pub d_tag: Option<String>,
    pub relays: Option<Vec<String>>,
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsEventPtr {
    pub id: String,
    pub relays: Option<String>,
}
