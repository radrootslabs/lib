use crate::RadrootsNostrEvent;
use serde::{Deserialize, Serialize};
#[cfg(feature = "ts-rs")]
use ts_rs::TS;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsPostEventIndex {
    pub event: RadrootsNostrEvent,
    pub metadata: RadrootsPostEventMetadata,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsPostEventMetadata {
    pub id: String,
    pub author: String,
    pub published_at: u64,
    pub kind: u32,
    pub post: RadrootsPost,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsPost {
    pub content: String,
}
