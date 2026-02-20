use crate::{RadrootsNostrEvent, RadrootsNostrEventRef};
#[cfg(feature = "ts-rs")]
use ts_rs::TS;

#[cfg(not(feature = "std"))]
use alloc::string::String;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsReactionEventIndex {
    pub event: RadrootsNostrEvent,
    pub metadata: RadrootsReactionEventMetadata,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsReactionEventMetadata {
    pub id: String,
    pub author: String,
    pub published_at: u32,
    pub kind: u32,
    pub reaction: RadrootsReaction,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsReaction {
    pub root: RadrootsNostrEventRef,
    pub content: String,
}
