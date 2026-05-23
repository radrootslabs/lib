use radroots_local_events::{
    LocalEventRecordInput, LocalEventRecordUpdate, LocalEventsStore, LocalRecordFamily,
    LocalRecordStatus, MIGRATIONS, PublishOutboxStatus, SourceRuntime,
};
use radroots_sql_core::migrations::migrations_run_all_up;
use radroots_sql_core::{SqlExecutor, SqliteExecutor};
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
    let rows = store.list_records_after_seq(0, 10).expect("list records");

    assert_eq!(first.seq, second.seq);
    assert_eq!(first.change_seq, second.change_seq);
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

    assert_eq!(first.last_change_seq, 10);
    assert_eq!(second.last_change_seq, 10);
    assert_eq!(third.last_change_seq, 12);
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

#[test]
fn changed_after_uses_change_seq_for_appends_and_outbox_updates() {
    let store = store();
    let input = signed_event("event-a");
    let appended = store.append_record(&input).expect("append signed event");
    let initial_rows = store
        .list_records_changed_after(0, 10)
        .expect("list initial changes");

    assert_eq!(initial_rows.len(), 1);
    assert_eq!(initial_rows[0].record_id, "event-a");
    assert_eq!(initial_rows[0].seq, appended.seq);
    assert_eq!(initial_rows[0].change_seq, appended.change_seq);

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
    let changed_rows = store
        .list_records_changed_after(appended.change_seq, 10)
        .expect("list changed rows");

    assert_eq!(updated.seq, appended.seq);
    assert!(updated.change_seq > appended.change_seq);
    assert_eq!(changed_rows.len(), 1);
    assert_eq!(changed_rows[0].record_id, "event-a");
    assert_eq!(changed_rows[0].change_seq, updated.change_seq);
}

#[test]
fn migration_assigns_existing_records_change_seq_from_insert_order() {
    let executor = SqliteExecutor::open_memory().expect("open memory sqlite");
    migrations_run_all_up(&executor, &MIGRATIONS[..1]).expect("apply initial migration");
    let first = insert_pre_change_tracking_record(&executor, "local-a");
    let second = insert_pre_change_tracking_record(&executor, "local-b");
    let store = LocalEventsStore::new(executor);

    store.migrate_up().expect("apply change tracking migration");
    let rows = store
        .list_records_changed_after(0, 10)
        .expect("list changed rows after migration");

    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].seq, first);
    assert_eq!(rows[0].change_seq, first);
    assert_eq!(rows[1].seq, second);
    assert_eq!(rows[1].change_seq, second);
}

fn insert_pre_change_tracking_record(executor: &SqliteExecutor, record_id: &str) -> i64 {
    let input = local_work(record_id);
    let params = json!([
        input.record_id,
        input.family.as_str(),
        input.status.as_str(),
        input.source_runtime.as_str(),
        input.created_at_ms,
        input.inserted_at_ms,
        input.inserted_at_ms,
        input.owner_account_id,
        input.owner_pubkey,
        input.farm_id,
        input.listing_addr,
        serde_json::to_string(&input.local_work_json).expect("encode local work"),
        input.event_id,
        input.event_kind,
        input.event_pubkey,
        input.event_created_at,
        input
            .event_tags_json
            .map(|value| serde_json::to_string(&value).expect("encode tags")),
        input.event_content,
        input.event_sig,
        input
            .raw_event_json
            .map(|value| serde_json::to_string(&value).expect("encode raw event")),
        input.outbox_status.as_str(),
        input.relay_set_fingerprint,
        input
            .relay_delivery_json
            .map(|value| serde_json::to_string(&value).expect("encode relay delivery")),
    ])
    .to_string();
    let outcome = executor
        .exec(
            "insert into local_event_record(
                record_id,
                family,
                status,
                source_runtime,
                created_at_ms,
                inserted_at_ms,
                updated_at_ms,
                owner_account_id,
                owner_pubkey,
                farm_id,
                listing_addr,
                local_work_json,
                event_id,
                event_kind,
                event_pubkey,
                event_created_at,
                event_tags_json,
                event_content,
                event_sig,
                raw_event_json,
                outbox_status,
                relay_set_fingerprint,
                relay_delivery_json
            ) values(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
            &params,
        )
        .expect("insert old local event record");
    outcome.last_insert_id
}
