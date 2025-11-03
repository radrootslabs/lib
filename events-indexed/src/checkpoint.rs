#![allow(clippy::module_name_repetitions)]
#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use crate::types::RadrootsEventsIndexedShardId;

#[typeshare::typeshare]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsEventsIndexedShardCheckpoint {
    pub shard_id: RadrootsEventsIndexedShardId,
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "crate::serde_ext::epoch_seconds::de")
    )]
    pub last_created_at: u32,
    pub last_event_id: Option<String>,
    pub cursor: Option<String>,
}

#[typeshare::typeshare]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsEventsIndexedIndexCheckpoint {
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "crate::serde_ext::epoch_seconds::de")
    )]
    pub generated_at: u32,
    pub shards: Vec<RadrootsEventsIndexedShardCheckpoint>,
}

impl RadrootsEventsIndexedIndexCheckpoint {
    pub fn get(
        &self,
        id: &RadrootsEventsIndexedShardId,
    ) -> Option<&RadrootsEventsIndexedShardCheckpoint> {
        self.shards.iter().find(|s| &s.shard_id == id)
    }
    pub fn upsert(&mut self, cp: RadrootsEventsIndexedShardCheckpoint) {
        if let Some(slot) = self.shards.iter_mut().find(|s| s.shard_id == cp.shard_id) {
            *slot = cp;
        } else {
            self.shards.push(cp);
        }
    }
}
