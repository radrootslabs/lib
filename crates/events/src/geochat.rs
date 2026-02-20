#![forbid(unsafe_code)]

#[cfg(feature = "ts-rs")]
use ts_rs::TS;

use crate::RadrootsNostrEvent;

#[cfg(not(feature = "std"))]
use alloc::string::String;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsGeoChatEventIndex {
    pub event: RadrootsNostrEvent,
    pub metadata: RadrootsGeoChatEventMetadata,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsGeoChatEventMetadata {
    pub id: String,
    pub author: String,
    pub published_at: u32,
    pub kind: u32,
    pub geochat: RadrootsGeoChat,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsGeoChat {
    pub geohash: String,
    pub content: String,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub nickname: Option<String>,
    pub teleported: bool,
}
