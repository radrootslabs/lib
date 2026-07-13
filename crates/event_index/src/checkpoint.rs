#![allow(clippy::module_name_repetitions)]
#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use crate::types::RadrootsEventIndexShardId;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsEventIndexShardCheckpoint {
    pub shard_id: RadrootsEventIndexShardId,
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "crate::serde_ext::epoch_seconds::de")
    )]
    pub last_created_at: u32,
    pub last_event_id: Option<String>,
    pub cursor: Option<String>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsEventIndexCheckpoint {
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "crate::serde_ext::epoch_seconds::de")
    )]
    pub generated_at: u32,
    pub shards: Vec<RadrootsEventIndexShardCheckpoint>,
}

impl RadrootsEventIndexCheckpoint {
    pub fn get(
        &self,
        id: &RadrootsEventIndexShardId,
    ) -> Option<&RadrootsEventIndexShardCheckpoint> {
        self.shards.iter().find(|s| &s.shard_id == id)
    }
    pub fn upsert(&mut self, cp: RadrootsEventIndexShardCheckpoint) {
        if let Some(slot) = self.shards.iter_mut().find(|s| s.shard_id == cp.shard_id) {
            *slot = cp;
        } else {
            self.shards.push(cp);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{RadrootsEventIndexCheckpoint, RadrootsEventIndexShardCheckpoint};
    use crate::types::RadrootsEventIndexShardId;
    #[cfg(not(feature = "std"))]
    use alloc::{string::String, vec, vec::Vec};
    #[cfg(feature = "std")]
    use std::{string::String, vec::Vec};

    fn checkpoint(
        shard_id: &str,
        last_created_at: u32,
        last_event_id: Option<&str>,
    ) -> RadrootsEventIndexShardCheckpoint {
        RadrootsEventIndexShardCheckpoint {
            shard_id: RadrootsEventIndexShardId(String::from(shard_id)),
            last_created_at,
            last_event_id: last_event_id.map(String::from),
            cursor: None,
        }
    }

    #[test]
    fn get_returns_none_for_unknown_shard() {
        let cp = RadrootsEventIndexCheckpoint {
            generated_at: 1,
            shards: vec![checkpoint("us-1", 10, Some("a"))],
        };
        let missing = cp.get(&RadrootsEventIndexShardId(String::from("us-2")));
        assert!(missing.is_none());
    }

    #[test]
    fn upsert_inserts_and_updates_shards() {
        let mut cp = RadrootsEventIndexCheckpoint {
            generated_at: 2,
            shards: Vec::new(),
        };

        cp.upsert(checkpoint("us-1", 10, Some("a")));
        assert_eq!(cp.shards.len(), 1);
        assert_eq!(
            cp.get(&RadrootsEventIndexShardId(String::from("us-1")))
                .expect("inserted shard")
                .last_created_at,
            10
        );

        cp.upsert(checkpoint("us-1", 11, Some("b")));
        assert_eq!(cp.shards.len(), 1);
        let updated = cp
            .get(&RadrootsEventIndexShardId(String::from("us-1")))
            .expect("updated shard");
        assert_eq!(updated.last_created_at, 11);
        assert_eq!(updated.last_event_id.as_deref(), Some("b"));
    }
}
