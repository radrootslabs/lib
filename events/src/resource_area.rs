#![forbid(unsafe_code)]

use crate::RadrootsNostrEvent;
use crate::farm::RadrootsGcsLocation;
#[cfg(feature = "ts-rs")]
use ts_rs::TS;

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsResourceAreaEventIndex {
    pub event: RadrootsNostrEvent,
    pub metadata: RadrootsResourceAreaEventMetadata,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsResourceAreaEventMetadata {
    pub id: String,
    pub author: String,
    pub published_at: u32,
    pub kind: u32,
    pub area: RadrootsResourceArea,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsResourceArea {
    pub d_tag: String,
    pub name: String,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub about: Option<String>,
    pub location: RadrootsResourceAreaLocation,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string[] | null"))]
    pub tags: Option<Vec<String>>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsResourceAreaRef {
    pub pubkey: String,
    pub d_tag: String,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsResourceAreaLocation {
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub primary: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub city: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub region: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub country: Option<String>,
    pub gcs: RadrootsGcsLocation,
}
