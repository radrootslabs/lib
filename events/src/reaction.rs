use crate::{RadrootsNostrEvent, RadrootsNostrEventRef};
use serde::{Deserialize, Serialize};
#[cfg(feature = "ts-rs")]
use ts_rs::TS;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsReactionEventIndex {
    pub event: RadrootsNostrEvent,
    pub metadata: RadrootsReactionEventMetadata,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsReactionEventMetadata {
    pub id: String,
    pub author: String,
    pub published_at: u32,
    pub kind: u32,
    pub reaction: RadrootsReaction,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsReaction {
    pub root: RadrootsNostrEventRef,
    pub content: String,
}
