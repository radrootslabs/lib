#[cfg(not(feature = "std"))]
use alloc::{collections::BTreeMap, string::{String, ToString}};
#[cfg(feature = "std")]
use std::collections::BTreeMap;

use radroots_events::kinds::is_nip51_list_set_kind;
use radroots_sql_core::SqlExecutor;
use radroots_tangle_db_schema::farm::IFarmFindMany;
use radroots_tangle_db_schema::nostr_event_state::INostrEventStateFindMany;

use crate::error::RadrootsTangleEventsError;
use crate::event_state::{event_content_hash, event_state_key, tag_value};
use crate::types::RadrootsTangleFarmSelector;

#[derive(Clone, Debug)]
pub struct RadrootsTangleSyncStatus {
    pub expected_count: usize,
    pub pending_count: usize,
}

pub fn radroots_tangle_sync_status<E: SqlExecutor>(
    exec: &E,
) -> Result<RadrootsTangleSyncStatus, RadrootsTangleEventsError> {
    let farms = radroots_tangle_db::farm::find_many(exec, &IFarmFindMany { filter: None })?
        .results;
    let mut expected: BTreeMap<String, String> = BTreeMap::new();

    for farm in farms {
        let selector = RadrootsTangleFarmSelector {
            id: Some(farm.id),
            d_tag: None,
            pubkey: None,
        };
        let bundle = crate::emit::radroots_tangle_sync_all_with_options(exec, &selector, None)?;
        for event in bundle.events {
            let d_tag = tag_value(&event.tags, "d").unwrap_or("");
            if is_nip51_list_set_kind(event.kind) && d_tag.is_empty() {
                return Err(RadrootsTangleEventsError::InvalidData(
                    "list set d tag missing".to_string(),
                ));
            }
            let key = event_state_key(event.kind, &event.author, d_tag);
            let content_hash = event_content_hash(&event.content, &event.tags)?;
            expected.entry(key).or_insert(content_hash);
        }
    }

    let states = radroots_tangle_db::nostr_event_state::find_many(
        exec,
        &INostrEventStateFindMany { filter: None },
    )?
    .results;

    let mut state_map: BTreeMap<String, String> = BTreeMap::new();
    for state in states {
        state_map.insert(state.key, state.content_hash);
    }

    let mut pending = 0;
    for (key, content_hash) in expected.iter() {
        match state_map.get(key) {
            Some(existing) if existing == content_hash => {}
            _ => pending += 1,
        }
    }

    Ok(RadrootsTangleSyncStatus {
        expected_count: expected.len(),
        pending_count: pending,
    })
}
