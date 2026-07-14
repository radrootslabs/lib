#![cfg_attr(all(not(feature = "std"), not(test)), no_std)]
#![forbid(unsafe_code)]
#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

pub const RADROOTS_EVENT_VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod account;
pub mod app_data;
pub mod article;
pub mod calendar;
pub mod comment;
pub mod contract;
pub mod coop;
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
pub mod gcs;
pub mod geochat;
pub mod gift_wrap;
pub mod group;
pub mod http_auth;
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
pub mod message;
pub mod message_file;
pub mod order;
pub mod order_economics;
pub mod plot;
pub mod post;
pub mod profile;
pub mod reaction;
pub mod relay_auth;
pub mod relay_document;
pub mod report;
pub mod repost;
pub mod resource_area;
pub mod resource_cap;
pub mod seal;
pub mod social;
pub mod tags;
pub mod trade_validation;
pub mod wire;

pub use envelope::{
    RadrootsEventEnvelope, RadrootsEventEnvelopeError, RadrootsEventEnvelopeLimits,
    RadrootsEventEnvelopeParts, RadrootsEventKind, RadrootsEventKindClass, RadrootsEventTag,
    RadrootsEventTags, RadrootsEventTimestamp,
};
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
