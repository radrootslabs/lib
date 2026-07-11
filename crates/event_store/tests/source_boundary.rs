use std::fs;
use std::path::Path;

#[test]
fn event_store_schema_does_not_reintroduce_old_nostr_table_names() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = [
        fs::read_to_string(root.join("src/store.rs")).expect("read store source"),
        fs::read_to_string(root.join("migrations/0001_event_store.up.sql"))
            .expect("read up migration"),
        fs::read_to_string(root.join("migrations/0001_event_store.down.sql"))
            .expect("read down migration"),
    ]
    .join("\n");

    for forbidden in ["nostr_events", "nostr_event_tags", "nostr_event_head"] {
        assert!(
            !source.contains(forbidden),
            "radroots_event_store baseline must not reintroduce old table name `{forbidden}`"
        );
    }

    for required in [
        "event_envelopes",
        "event_envelope_tags",
        "event_envelope_head",
    ] {
        assert!(
            source.contains(required),
            "radroots_event_store baseline must retain neutral table name `{required}`"
        );
    }
}
