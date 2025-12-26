#![cfg(feature = "serde_json")]

use radroots_events::list::RadrootsListEntry;
use radroots_events_codec::list::decode::list_private_entries_from_json;
use radroots_events_codec::list::encode::list_private_entries_json;
use radroots_events_codec::list_set::decode::list_set_private_entries_from_json;
use radroots_events_codec::list_set::encode::list_set_private_entries_json;

#[test]
fn list_private_entries_roundtrip() {
    let entries = vec![
        RadrootsListEntry {
            tag: "p".to_string(),
            values: vec!["pubkey".to_string()],
        },
        RadrootsListEntry {
            tag: "a".to_string(),
            values: vec!["30340:pubkey:farm-1".to_string()],
        },
    ];

    let json = list_private_entries_json(&entries).expect("json");
    let parsed = list_private_entries_from_json(&json).expect("parsed");
    assert_eq!(parsed.len(), entries.len());
    assert_eq!(parsed[0].tag, "p");
    assert_eq!(parsed[1].values[0], "30340:pubkey:farm-1");
}

#[test]
fn list_set_private_entries_roundtrip() {
    let entries = vec![
        RadrootsListEntry {
            tag: "p".to_string(),
            values: vec!["member".to_string()],
        },
        RadrootsListEntry {
            tag: "t".to_string(),
            values: vec!["orchard".to_string()],
        },
    ];

    let json = list_set_private_entries_json(&entries).expect("json");
    let parsed = list_set_private_entries_from_json(&json).expect("parsed");
    assert_eq!(parsed.len(), entries.len());
    assert_eq!(parsed[0].tag, "p");
    assert_eq!(parsed[1].values[0], "orchard");
}
