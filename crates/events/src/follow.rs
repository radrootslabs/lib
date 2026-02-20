use crate::RadrootsNostrEvent;
#[cfg(feature = "ts-rs")]
use ts_rs::TS;

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsFollowEventIndex {
    pub event: RadrootsNostrEvent,
    pub metadata: RadrootsFollowEventMetadata,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsFollowEventMetadata {
    pub id: String,
    pub author: String,
    pub published_at: u32,
    pub kind: u32,
    pub follow: RadrootsFollow,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsFollow {
    pub list: Vec<RadrootsFollowProfile>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsFollowProfile {
    pub published_at: u32,
    pub public_key: String,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub relay_url: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub contact_name: Option<String>,
}
