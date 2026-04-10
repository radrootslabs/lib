#![allow(clippy::module_name_repetitions)]
#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use crate::types::RadrootsEventsIndexedShardId;

#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
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

#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
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

#[cfg(test)]
mod tests {
    use super::{RadrootsEventsIndexedIndexCheckpoint, RadrootsEventsIndexedShardCheckpoint};
    use crate::types::RadrootsEventsIndexedShardId;
    #[cfg(not(feature = "std"))]
    use alloc::{string::String, vec, vec::Vec};
    #[cfg(feature = "std")]
    use std::{string::String, vec::Vec};

    fn checkpoint(
        shard_id: &str,
        last_created_at: u32,
        last_event_id: Option<&str>,
    ) -> RadrootsEventsIndexedShardCheckpoint {
        RadrootsEventsIndexedShardCheckpoint {
            shard_id: RadrootsEventsIndexedShardId(String::from(shard_id)),
            last_created_at,
            last_event_id: last_event_id.map(String::from),
            cursor: None,
        }
    }

    #[test]
    fn get_returns_none_for_unknown_shard() {
        let cp = RadrootsEventsIndexedIndexCheckpoint {
            generated_at: 1,
            shards: vec![checkpoint("us-1", 10, Some("a"))],
        };
        let missing = cp.get(&RadrootsEventsIndexedShardId(String::from("us-2")));
        assert!(missing.is_none());
    }

    #[test]
    fn upsert_inserts_and_updates_shards() {
        let mut cp = RadrootsEventsIndexedIndexCheckpoint {
            generated_at: 2,
            shards: Vec::new(),
        };

        cp.upsert(checkpoint("us-1", 10, Some("a")));
        assert_eq!(cp.shards.len(), 1);
        assert_eq!(
            cp.get(&RadrootsEventsIndexedShardId(String::from("us-1")))
                .expect("inserted shard")
                .last_created_at,
            10
        );

        cp.upsert(checkpoint("us-1", 11, Some("b")));
        assert_eq!(cp.shards.len(), 1);
        let updated = cp
            .get(&RadrootsEventsIndexedShardId(String::from("us-1")))
            .expect("updated shard");
        assert_eq!(updated.last_created_at, 11);
        assert_eq!(updated.last_event_id.as_deref(), Some("b"));
    }
}
