use radroots_local_events::{
    LocalEventRecordInput, LocalEventRecordUpdate, LocalEventsStore, LocalRecordFamily,
    LocalRecordStatus, PublishOutboxStatus, SourceRuntime,
};
use radroots_sql_core::SqliteExecutor;
use serde_json::json;

fn store() -> LocalEventsStore<SqliteExecutor> {
    let executor = SqliteExecutor::open_memory().expect("open memory sqlite");
    let store = LocalEventsStore::new(executor);
    store.migrate_up().expect("migrate local events");
    store
}

fn local_work(record_id: &str) -> LocalEventRecordInput {
    LocalEventRecordInput {
        record_id: record_id.to_owned(),
        family: LocalRecordFamily::LocalWork,
        status: LocalRecordStatus::LocalSaved,
        source_runtime: SourceRuntime::Cli,
        created_at_ms: 1000,
        inserted_at_ms: 1001,
        owner_account_id: Some("seller-account".to_owned()),
        owner_pubkey: Some("seller-pubkey".to_owned()),
        farm_id: Some("farm-a".to_owned()),
        listing_addr: Some("listing-a".to_owned()),
        local_work_json: Some(json!({"kind":"listing","title":"Eggs"})),
        event_id: None,
        event_kind: None,
        event_pubkey: None,
        event_created_at: None,
        event_tags_json: None,
        event_content: None,
        event_sig: None,
        raw_event_json: None,
        outbox_status: PublishOutboxStatus::None,
        relay_set_fingerprint: None,
        relay_delivery_json: None,
    }
}

fn signed_event(record_id: &str) -> LocalEventRecordInput {
    LocalEventRecordInput {
        record_id: record_id.to_owned(),
        family: LocalRecordFamily::SignedEvent,
        status: LocalRecordStatus::PendingPublish,
        source_runtime: SourceRuntime::Cli,
        created_at_ms: 2000,
        inserted_at_ms: 2001,
        owner_account_id: Some("seller-account".to_owned()),
        owner_pubkey: Some("seller-pubkey".to_owned()),
        farm_id: Some("farm-a".to_owned()),
        listing_addr: Some("listing-a".to_owned()),
        local_work_json: None,
        event_id: Some("event-a".to_owned()),
        event_kind: Some(3421),
        event_pubkey: Some("seller-pubkey".to_owned()),
        event_created_at: Some(2000),
        event_tags_json: Some(json!([["d", "listing-a"]])),
        event_content: Some("{\"title\":\"Eggs\"}".to_owned()),
        event_sig: Some("sig-a".to_owned()),
        raw_event_json: Some(json!({"id":"event-a","kind":3421})),
        outbox_status: PublishOutboxStatus::Pending,
        relay_set_fingerprint: Some("relay-set-a".to_owned()),
        relay_delivery_json: Some(json!({"pending":["ws://127.0.0.1:8080"]})),
    }
}

#[test]
fn append_rejects_malformed_local_work_records() {
    let store = store();
    let mut input = local_work("local-a");
    input.local_work_json = None;

    let err = store.append_record(&input).expect_err("invalid record");

    assert!(err.to_string().contains("local_work_json"));
}

#[test]
fn append_is_idempotent_by_record_id() {
    let store = store();
    let input = local_work("local-a");

    let first = store.append_record(&input).expect("append first");
    let second = store.append_record(&input).expect("append second");
    let rows = store.list_records_after(0, 10).expect("list records");

    assert_eq!(first.seq, second.seq);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].record_id, "local-a");
    assert_eq!(
        rows[0].local_work_json,
        Some(json!({"kind":"listing","title":"Eggs"}))
    );
}

#[test]
fn projection_cursor_advances_without_rewinding() {
    let store = store();

    let first = store
        .advance_cursor("app", 10, 100)
        .expect("advance cursor");
    let second = store.advance_cursor("app", 5, 200).expect("ignore rewind");
    let third = store.advance_cursor("app", 12, 300).expect("advance again");

    assert_eq!(first.last_seq, 10);
    assert_eq!(second.last_seq, 10);
    assert_eq!(third.last_seq, 12);
}

#[test]
fn outbox_status_updates_signed_event_records() {
    let store = store();
    let input = signed_event("event-a");
    store.append_record(&input).expect("append signed event");

    let updated = store
        .update_outbox(&LocalEventRecordUpdate {
            record_id: "event-a".to_owned(),
            status: LocalRecordStatus::Published,
            outbox_status: PublishOutboxStatus::Acknowledged,
            relay_set_fingerprint: Some("relay-set-a".to_owned()),
            relay_delivery_json: Some(json!({"acked":["ws://127.0.0.1:8080"]})),
            updated_at_ms: 3000,
        })
        .expect("update outbox");

    assert_eq!(updated.status, LocalRecordStatus::Published);
    assert_eq!(updated.outbox_status, PublishOutboxStatus::Acknowledged);
    assert_eq!(
        updated.relay_delivery_json,
        Some(json!({"acked":["ws://127.0.0.1:8080"]}))
    );
}
