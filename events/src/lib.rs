#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(feature = "ts-rs")]
use ts_rs::TS;

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

pub mod comment;
pub mod follow;
pub mod job;
pub mod job_feedback;
pub mod job_request;
pub mod job_result;
pub mod kinds;
pub mod listing;
pub mod post;
pub mod profile;
pub mod reaction;
pub mod relay_document;
pub mod tags;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsNostrEvent {
    pub id: String,
    pub author: String,
    pub created_at: u32,
    pub kind: u32,
    pub tags: Vec<Vec<String>>,
    pub content: String,
    pub sig: String,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsNostrEventRef {
    pub id: String,
    pub author: String,
    pub kind: u32,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub d_tag: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string[] | null"))]
    pub relays: Option<Vec<String>>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsNostrEventPtr {
    pub id: String,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub relays: Option<String>,
}
