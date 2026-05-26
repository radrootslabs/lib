use radroots_local_events::{
    LocalEventRecordInput, LocalEventRecordUpdate, LocalEventsStore, LocalRecordFamily,
    LocalRecordStatus, MIGRATIONS, PublishOutboxStatus, RelayDeliveryEvidence, SourceRuntime,
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
        relay_delivery_json: Some(
            RelayDeliveryEvidence::pending(["ws://127.0.0.1:8080"])
                .expect("pending delivery")
                .to_json_value()
                .expect("pending delivery json"),
        ),
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
fn source_runtime_network_round_trips() {
    let store = store();
    let mut input = signed_event("event-network-a");
    input.source_runtime = SourceRuntime::Network;

    let inserted = store.append_record(&input).expect("append network event");
    let rows = store
        .list_records_after_seq(0, 10)
        .expect("list network event");

    assert_eq!(SourceRuntime::Network.as_str(), "network");
    assert_eq!(
        SourceRuntime::parse("network").expect("parse network runtime"),
        SourceRuntime::Network
    );
    assert_eq!(inserted.source_runtime, SourceRuntime::Network);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].source_runtime, SourceRuntime::Network);
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
            relay_delivery_json: Some(
                RelayDeliveryEvidence::acknowledged(
                    ["ws://127.0.0.1:8080"],
                    ["ws://127.0.0.1:8080"],
                    ["ws://127.0.0.1:8080"],
                    Vec::new(),
                )
                .expect("acknowledged delivery")
                .to_json_value()
                .expect("acknowledged delivery json"),
            ),
            updated_at_ms: 3000,
        })
        .expect("update outbox");

    assert_eq!(updated.status, LocalRecordStatus::Published);
    assert_eq!(updated.outbox_status, PublishOutboxStatus::Acknowledged);
    assert_eq!(
        updated.relay_delivery_json,
        Some(json!({
            "state": "acknowledged",
            "target_relays": ["ws://127.0.0.1:8080"],
            "connected_relays": ["ws://127.0.0.1:8080"],
            "acknowledged_relays": ["ws://127.0.0.1:8080"],
            "failed_relays": []
        }))
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
            relay_delivery_json: Some(
                RelayDeliveryEvidence::acknowledged(
                    ["ws://127.0.0.1:8080"],
                    ["ws://127.0.0.1:8080"],
                    ["ws://127.0.0.1:8080"],
                    Vec::new(),
                )
                .expect("acknowledged delivery")
                .to_json_value()
                .expect("acknowledged delivery json"),
            ),
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
fn changed_latest_lists_newest_records_first() {
    let store = store();
    let first = store
        .append_record(&local_work("local-a"))
        .expect("append first");
    let second = store
        .append_record(&local_work("local-b"))
        .expect("append second");
    let third = store
        .append_record(&local_work("local-c"))
        .expect("append third");

    let rows = store
        .list_records_changed_latest(2)
        .expect("list latest changed rows");

    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].record_id, "local-c");
    assert_eq!(rows[0].change_seq, third.change_seq);
    assert_eq!(rows[1].record_id, "local-b");
    assert_eq!(rows[1].change_seq, second.change_seq);
    assert!(rows[1].change_seq > first.change_seq);
}

#[test]
fn changed_before_pages_newest_first_by_cursor() {
    let store = store();
    let _first = store
        .append_record(&local_work("local-a"))
        .expect("append first");
    let second = store
        .append_record(&local_work("local-b"))
        .expect("append second");
    let third = store
        .append_record(&local_work("local-c"))
        .expect("append third");
    let fourth = store
        .append_record(&local_work("local-d"))
        .expect("append fourth");

    let first_page = store
        .list_records_changed_latest(2)
        .expect("list first page");
    let cursor = first_page.last().expect("last first page");
    let second_page = store
        .list_records_changed_before(cursor.change_seq, cursor.seq, 2)
        .expect("list second page");

    assert_eq!(first_page.len(), 2);
    assert_eq!(first_page[0].record_id, "local-d");
    assert_eq!(first_page[0].change_seq, fourth.change_seq);
    assert_eq!(first_page[1].record_id, "local-c");
    assert_eq!(first_page[1].change_seq, third.change_seq);
    assert_eq!(second_page.len(), 2);
    assert_eq!(second_page[0].record_id, "local-b");
    assert_eq!(second_page[0].change_seq, second.change_seq);
    assert_eq!(second_page[1].record_id, "local-a");
}

#[test]
fn changed_latest_is_not_blocked_by_older_record_volume() {
    let store = store();
    for index in 0..505 {
        store
            .append_record(&local_work(&format!("older-{index:03}")))
            .expect("append older record");
    }
    let current = store
        .append_record(&local_work("current-record"))
        .expect("append current record");

    let rows = store
        .list_records_changed_latest(1)
        .expect("list latest record");

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].record_id, "current-record");
    assert_eq!(rows[0].change_seq, current.change_seq);
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

#[test]
fn migration_repairs_pre_network_source_runtime_constraint() {
    let executor = SqliteExecutor::open_memory().expect("open memory sqlite");
    create_pre_network_change_tracking_schema(&executor);
    let legacy_seq = insert_pre_network_change_tracking_record(&executor, "legacy-cli", 1);
    let store = LocalEventsStore::new(executor);

    store
        .migrate_up()
        .expect("apply network source repair migration");
    let mut input = signed_event("event-network-repaired");
    input.source_runtime = SourceRuntime::Network;
    input.event_id = Some("event-network-repaired".to_owned());
    input.raw_event_json = Some(json!({"id":"event-network-repaired","kind":3421}));
    let inserted = store
        .append_record(&input)
        .expect("append repaired network event");
    let rows = store
        .list_records_changed_after(0, 10)
        .expect("list changed rows after repair");

    assert_eq!(legacy_seq, 1);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].record_id, "legacy-cli");
    assert_eq!(rows[0].change_seq, 1);
    assert_eq!(rows[0].source_runtime, SourceRuntime::Cli);
    assert_eq!(rows[1].record_id, "event-network-repaired");
    assert_eq!(rows[1].seq, inserted.seq);
    assert_eq!(rows[1].source_runtime, SourceRuntime::Network);
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

fn create_pre_network_change_tracking_schema(executor: &SqliteExecutor) {
    let schema = [
        "create table __migrations(id integer primary key, name text not null unique, applied_at text not null default (datetime('now')))",
        "create table local_event_record (
            seq integer primary key autoincrement,
            change_seq integer not null unique,
            record_id text not null unique,
            family text not null check (family in ('local_work', 'signed_event')),
            status text not null check (status in ('local_draft', 'local_saved', 'pending_publish', 'published', 'failed', 'conflict')),
            source_runtime text not null check (source_runtime in ('cli', 'app', 'service', 'worker', 'test')),
            created_at_ms integer not null,
            inserted_at_ms integer not null,
            updated_at_ms integer not null,
            owner_account_id text,
            owner_pubkey text,
            farm_id text,
            listing_addr text,
            local_work_json text,
            event_id text,
            event_kind integer,
            event_pubkey text,
            event_created_at integer,
            event_tags_json text,
            event_content text,
            event_sig text,
            raw_event_json text,
            outbox_status text not null check (outbox_status in ('none', 'pending', 'acknowledged', 'failed')),
            relay_set_fingerprint text,
            relay_delivery_json text,
            check (change_seq >= 1),
            check (trim(record_id) <> ''),
            check (family <> 'local_work' or local_work_json is not null),
            check (family <> 'local_work' or outbox_status = 'none'),
            check (family <> 'signed_event' or (event_id is not null and event_kind is not null and event_pubkey is not null and event_sig is not null and raw_event_json is not null))
        )",
        "create index local_event_record_change_seq_idx on local_event_record(change_seq)",
        "create index local_event_record_event_id_idx on local_event_record(event_id)",
        "create index local_event_record_listing_addr_idx on local_event_record(listing_addr)",
        "create index local_event_record_owner_pubkey_idx on local_event_record(owner_pubkey)",
        "create index local_event_record_status_idx on local_event_record(status)",
        "create table local_event_projection_cursor (
            consumer_id text primary key,
            last_change_seq integer not null,
            updated_at_ms integer not null,
            check (trim(consumer_id) <> ''),
            check (last_change_seq >= 0)
        )",
    ];
    for sql in schema {
        executor.exec(sql, "[]").expect("schema statement");
    }
    for name in ["0000_local_events", "0001_change_tracking"] {
        let params = json!([name]).to_string();
        executor
            .exec("insert into __migrations(name) values(?)", &params)
            .expect("migration marker");
    }
}

fn insert_pre_network_change_tracking_record(
    executor: &SqliteExecutor,
    record_id: &str,
    change_seq: i64,
) -> i64 {
    let input = local_work(record_id);
    let params = json!([
        change_seq,
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
                change_seq,
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
            ) values(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
            &params,
        )
        .expect("insert pre-network local event record");
    outcome.last_insert_id
}
