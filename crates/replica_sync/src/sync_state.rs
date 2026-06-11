#[cfg(not(feature = "std"))]
use alloc::{collections::BTreeMap, string::String, string::ToString, vec::Vec};
#[cfg(feature = "std")]
use std::collections::BTreeMap;

use radroots_replica_db_schema::farm::IFarmFindMany;
use radroots_replica_db_schema::nostr_event_state::INostrEventStateFindMany;
use radroots_sql_core::SqlExecutor;

use crate::error::RadrootsReplicaEventsError;
use crate::event_state::{event_content_hash, event_state_key, tag_value};
use crate::types::{RadrootsReplicaEventDraft, RadrootsReplicaFarmSelector};

#[derive(Clone, Debug)]
pub struct RadrootsReplicaSyncStatus {
    pub expected_count: usize,
    pub pending_count: usize,
}

#[derive(Clone, Debug)]
pub struct RadrootsReplicaPendingPublishEvent {
    pub key: String,
    pub kind: u32,
    pub author: String,
    pub d_tag: String,
    pub content_hash: String,
    pub draft: RadrootsReplicaEventDraft,
}

#[derive(Clone, Debug)]
pub struct RadrootsReplicaPendingPublishBatch {
    pub expected_count: usize,
    pub pending_count: usize,
    pub pending_events: Vec<RadrootsReplicaPendingPublishEvent>,
}

pub fn radroots_replica_sync_status<E: SqlExecutor>(
    exec: &E,
) -> Result<RadrootsReplicaSyncStatus, RadrootsReplicaEventsError> {
    let batch = radroots_replica_pending_publish_batch(exec)?;
    Ok(RadrootsReplicaSyncStatus {
        expected_count: batch.expected_count,
        pending_count: batch.pending_count,
    })
}

pub fn radroots_replica_pending_publish_batch<E: SqlExecutor>(
    exec: &E,
) -> Result<RadrootsReplicaPendingPublishBatch, RadrootsReplicaEventsError> {
    let farms =
        radroots_replica_db::farm::find_many(exec, &IFarmFindMany { filter: None })?.results;
    let mut expected: BTreeMap<String, RadrootsReplicaPendingPublishEvent> = BTreeMap::new();

    for farm in farms {
        let selector = RadrootsReplicaFarmSelector {
            id: Some(farm.id),
            d_tag: None,
            pubkey: None,
        };
        let bundle = crate::emit::radroots_replica_sync_all_with_options(exec, &selector, None)?;
        for event in bundle.events {
            let d_tag = tag_value(&event.tags, "d").unwrap_or("");
            let key = event_state_key(event.kind, &event.author, d_tag);
            let content_hash = draft_content_hash(&event)?;
            expected
                .entry(key.clone())
                .or_insert(RadrootsReplicaPendingPublishEvent {
                    key,
                    kind: event.kind,
                    author: event.author.clone(),
                    d_tag: d_tag.to_string(),
                    content_hash,
                    draft: event,
                });
        }
    }

    let states_query = radroots_replica_db::nostr_event_state::find_many(
        exec,
        &INostrEventStateFindMany { filter: None },
    );
    let states_result = states_query?;
    let states = states_result.results;

    let mut state_map: BTreeMap<String, String> = BTreeMap::new();
    for state in states {
        state_map.insert(state.key, state.content_hash);
    }

    let mut pending_events = Vec::new();
    for (key, event) in expected.iter() {
        match state_map.get(key) {
            Some(existing) if existing == &event.content_hash => {}
            _ => pending_events.push(event.clone()),
        }
    }

    Ok(RadrootsReplicaPendingPublishBatch {
        expected_count: expected.len(),
        pending_count: pending_events.len(),
        pending_events,
    })
}

fn draft_content_hash(
    event: &RadrootsReplicaEventDraft,
) -> Result<String, RadrootsReplicaEventsError> {
    #[cfg(test)]
    {
        event_content_hash(&event.content, &event.tags)
    }
    #[cfg(not(test))]
    {
        Ok(event_content_hash(&event.content, &event.tags))
    }
}

#[cfg(test)]
mod tests {
    use super::{radroots_replica_pending_publish_batch, radroots_replica_sync_status};
    use crate::emit::radroots_replica_sync_all_with_options;
    use crate::event_state::{
        event_content_hash, event_content_hash_fail_next, event_state_key, tag_value,
    };
    use crate::types::RadrootsReplicaFarmSelector;
    use radroots_replica_db::{farm, migrations, nostr_event_state};
    use radroots_replica_db_schema::farm::IFarmFields;
    use radroots_replica_db_schema::nostr_event_state::INostrEventStateFields;
    use radroots_sql_core::{SqlExecutor, SqliteExecutor};

    #[test]
    fn sync_status_empty_db_is_zero() {
        let exec = SqliteExecutor::open_memory().expect("db");
        migrations::run_all_up(&exec).expect("migrations");
        let status = radroots_replica_sync_status(&exec).expect("status");
        assert_eq!(status.expected_count, 0);
        assert_eq!(status.pending_count, 0);
    }

    #[test]
    fn sync_status_tracks_expected_and_pending() {
        let exec = SqliteExecutor::open_memory().expect("db");
        migrations::run_all_up(&exec).expect("migrations");

        let farm_row = farm::create(
            &exec,
            &IFarmFields {
                d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_string(),
                pubkey: "f".repeat(64),
                name: "farm".to_string(),
                about: None,
                website: None,
                picture: None,
                banner: None,
                location_primary: None,
                location_city: None,
                location_region: None,
                location_country: None,
            },
        )
        .expect("farm")
        .result;

        let selector = RadrootsReplicaFarmSelector {
            id: Some(farm_row.id.clone()),
            d_tag: None,
            pubkey: None,
        };
        let bundle =
            radroots_replica_sync_all_with_options(&exec, &selector, None).expect("bundle");
        let expected_count = bundle.events.len();
        let first = bundle.events.first().expect("event");
        let d_tag = tag_value(&first.tags, "d").unwrap_or("");
        let key = event_state_key(first.kind, &first.author, d_tag);
        let content_hash = event_content_hash(&first.content, &first.tags).expect("hash");
        let fields = INostrEventStateFields {
            key,
            kind: first.kind,
            pubkey: first.author.clone(),
            d_tag: d_tag.to_string(),
            last_event_id: format!("{:064x}", 1u64),
            last_created_at: 1,
            content_hash,
        };
        let _ = nostr_event_state::create(&exec, &fields).expect("state");

        let status = radroots_replica_sync_status(&exec).expect("status");
        assert_eq!(status.expected_count, expected_count);
        assert_eq!(status.pending_count, expected_count.saturating_sub(1));
    }

    #[test]
    fn pending_publish_batch_lists_only_missing_or_changed_expected_events() {
        let exec = SqliteExecutor::open_memory().expect("db");
        migrations::run_all_up(&exec).expect("migrations");

        let farm_row = farm::create(
            &exec,
            &IFarmFields {
                d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_string(),
                pubkey: "a".repeat(64),
                name: "farm".to_string(),
                about: None,
                website: None,
                picture: None,
                banner: None,
                location_primary: None,
                location_city: None,
                location_region: None,
                location_country: None,
            },
        )
        .expect("farm")
        .result;

        let selector = RadrootsReplicaFarmSelector {
            id: Some(farm_row.id.clone()),
            d_tag: None,
            pubkey: None,
        };
        let bundle =
            radroots_replica_sync_all_with_options(&exec, &selector, None).expect("bundle");
        let first = bundle.events.first().expect("event");
        let d_tag = tag_value(&first.tags, "d").unwrap_or("");
        let key = event_state_key(first.kind, &first.author, d_tag);
        let content_hash = event_content_hash(&first.content, &first.tags).expect("hash");
        let fields = INostrEventStateFields {
            key: key.clone(),
            kind: first.kind,
            pubkey: first.author.clone(),
            d_tag: d_tag.to_string(),
            last_event_id: format!("{:064x}", 1u64),
            last_created_at: 1,
            content_hash,
        };
        let _ = nostr_event_state::create(&exec, &fields).expect("state");

        let batch = radroots_replica_pending_publish_batch(&exec).expect("batch");

        assert_eq!(batch.expected_count, bundle.events.len());
        assert_eq!(batch.pending_count, bundle.events.len().saturating_sub(1));
        assert!(
            batch
                .pending_events
                .iter()
                .all(|event| event.key != key && event.content_hash.len() == 64)
        );
    }

    #[test]
    fn sync_status_reports_farm_query_errors() {
        let exec = SqliteExecutor::open_memory().expect("db");
        let err = radroots_replica_sync_status(&exec).expect_err("farm query error");
        assert!(err.to_string().contains("invalid query"));
    }

    #[test]
    fn sync_status_reports_emit_errors() {
        let exec = SqliteExecutor::open_memory().expect("db");
        migrations::run_all_up(&exec).expect("migrations");
        let _ = farm::create(
            &exec,
            &IFarmFields {
                d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_string(),
                pubkey: "b".repeat(64),
                name: "farm".to_string(),
                about: None,
                website: None,
                picture: None,
                banner: None,
                location_primary: None,
                location_city: None,
                location_region: None,
                location_country: None,
            },
        )
        .expect("farm");
        let _ = exec
            .exec("DROP TABLE farm_tag;", "[]")
            .expect("drop farm_tag");
        let err = radroots_replica_sync_status(&exec).expect_err("emit error");
        assert!(err.to_string().contains("invalid query"));
    }

    #[test]
    fn sync_status_reports_content_hash_errors() {
        let exec = SqliteExecutor::open_memory().expect("db");
        migrations::run_all_up(&exec).expect("migrations");
        let _ = farm::create(
            &exec,
            &IFarmFields {
                d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_string(),
                pubkey: "c".repeat(64),
                name: "farm".to_string(),
                about: None,
                website: None,
                picture: None,
                banner: None,
                location_primary: None,
                location_city: None,
                location_region: None,
                location_country: None,
            },
        )
        .expect("farm");
        event_content_hash_fail_next();
        let err = radroots_replica_sync_status(&exec).expect_err("content hash error");
        assert!(err.to_string().contains("content_hash"));
    }

    #[test]
    fn sync_status_reports_state_query_errors() {
        let exec = SqliteExecutor::open_memory().expect("db");
        migrations::run_all_up(&exec).expect("migrations");
        let _ = exec
            .exec("DROP TABLE nostr_event_state;", "[]")
            .expect("drop nostr_event_state");
        let err = radroots_replica_sync_status(&exec).expect_err("state query error");
        assert!(err.to_string().contains("invalid query"));
    }
}
