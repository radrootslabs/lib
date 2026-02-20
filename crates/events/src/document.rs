#![forbid(unsafe_code)]

use crate::RadrootsNostrEvent;
#[cfg(feature = "ts-rs")]
use ts_rs::TS;

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsDocumentEventIndex {
    pub event: RadrootsNostrEvent,
    pub metadata: RadrootsDocumentEventMetadata,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsDocumentEventMetadata {
    pub id: String,
    pub author: String,
    pub published_at: u32,
    pub kind: u32,
    pub document: RadrootsDocument,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsDocumentSubject {
    pub pubkey: String,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub address: Option<String>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsDocument {
    pub d_tag: String,
    pub doc_type: String,
    pub title: String,
    pub version: String,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub summary: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "number | null"))]
    pub effective_at: Option<u32>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub body_markdown: Option<String>,
    pub subject: RadrootsDocumentSubject,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string[] | null"))]
    pub tags: Option<Vec<String>>,
}
