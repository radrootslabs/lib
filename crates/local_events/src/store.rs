#![forbid(unsafe_code)]

use radroots_sql_core::SqlExecutor;
use radroots_sql_core::error::SqlError;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::migrations;
use crate::models::validate_non_empty;
use crate::{
    LocalEventRecord, LocalEventRecordInput, LocalEventRecordUpdate, LocalEventsCursor,
    LocalEventsError, LocalRecordFamily, LocalRecordStatus, PublishOutboxStatus, SourceRuntime,
};

pub struct LocalEventsStore<E: SqlExecutor> {
    executor: E,
}

impl<E: SqlExecutor> LocalEventsStore<E> {
    pub fn new(executor: E) -> Self {
        Self { executor }
    }

    pub fn executor(&self) -> &E {
        &self.executor
    }

    pub fn migrate_up(&self) -> Result<(), SqlError> {
        migrations::run_all_up(self.executor())
    }

    pub fn migrate_down(&self) -> Result<(), SqlError> {
        migrations::run_all_down(self.executor())
    }

    pub fn append_record(
        &self,
        input: &LocalEventRecordInput,
    ) -> Result<LocalEventRecord, LocalEventsError> {
        input.validate()?;
        self.executor.begin()?;
        let result = (|| -> Result<(), LocalEventsError> {
            let change_seq = self.next_change_seq()?;
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
                encode_json(input.local_work_json.as_ref()),
                input.event_id,
                input.event_kind,
                input.event_pubkey,
                input.event_created_at,
                encode_json(input.event_tags_json.as_ref()),
                input.event_content,
                input.event_sig,
                encode_json(input.raw_event_json.as_ref()),
                input.outbox_status.as_str(),
                input.relay_set_fingerprint,
                encode_json(input.relay_delivery_json.as_ref())
            ])
            .to_string();
            let sql = "insert or ignore into local_event_record(
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
            ) values(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)";
            let _ = self.executor.exec(sql, &params)?;
            Ok(())
        })();
        match result {
            Ok(()) => self.executor.commit()?,
            Err(err) => {
                let _ = self.executor.rollback();
                return Err(err);
            }
        }
        self.get_record(&input.record_id)?
            .ok_or_else(|| LocalEventsError::InvalidRecord("record append failed".to_owned()))
    }

    pub fn get_record(
        &self,
        record_id: &str,
    ) -> Result<Option<LocalEventRecord>, LocalEventsError> {
        validate_non_empty("record_id", record_id)?;
        let params = json!([record_id]).to_string();
        let rows = self.query_records(
            "select * from local_event_record where record_id = ? limit 1",
            &params,
        )?;
        Ok(rows.into_iter().next())
    }

    pub fn list_records_after_seq(
        &self,
        after_seq: i64,
        limit: u32,
    ) -> Result<Vec<LocalEventRecord>, LocalEventsError> {
        let params = json!([after_seq, i64::from(limit)]).to_string();
        self.query_records(
            "select * from local_event_record where seq > ? order by seq asc limit ?",
            &params,
        )
    }

    pub fn list_records_changed_after(
        &self,
        after_change_seq: i64,
        limit: u32,
    ) -> Result<Vec<LocalEventRecord>, LocalEventsError> {
        let params = json!([after_change_seq, i64::from(limit)]).to_string();
        self.query_records(
            "select * from local_event_record where change_seq > ? order by change_seq asc, seq asc limit ?",
            &params,
        )
    }

    pub fn list_records_changed_latest(
        &self,
        limit: u32,
    ) -> Result<Vec<LocalEventRecord>, LocalEventsError> {
        let params = json!([i64::from(limit)]).to_string();
        self.query_records(
            "select * from local_event_record order by change_seq desc, seq desc, record_id asc limit ?",
            &params,
        )
    }

    pub fn list_records_changed_before(
        &self,
        before_change_seq: i64,
        before_seq: i64,
        limit: u32,
    ) -> Result<Vec<LocalEventRecord>, LocalEventsError> {
        let params = json!([
            before_change_seq,
            before_change_seq,
            before_seq,
            i64::from(limit)
        ])
        .to_string();
        self.query_records(
            "select * from local_event_record
             where change_seq < ? or (change_seq = ? and seq < ?)
             order by change_seq desc, seq desc, record_id asc
             limit ?",
            &params,
        )
    }

    pub fn update_outbox(
        &self,
        update: &LocalEventRecordUpdate,
    ) -> Result<LocalEventRecord, LocalEventsError> {
        validate_non_empty("record_id", &update.record_id)?;
        self.executor.begin()?;
        let result = (|| -> Result<i64, LocalEventsError> {
            let change_seq = self.next_change_seq()?;
            let params = json!([
                change_seq,
                update.status.as_str(),
                update.outbox_status.as_str(),
                update.relay_set_fingerprint,
                encode_json(update.relay_delivery_json.as_ref()),
                update.updated_at_ms,
                update.record_id
            ])
            .to_string();
            let outcome = self.executor.exec(
                "update local_event_record
                 set change_seq = ?,
                     status = ?,
                     outbox_status = ?,
                     relay_set_fingerprint = ?,
                     relay_delivery_json = ?,
                     updated_at_ms = ?
                 where record_id = ?",
                &params,
            )?;
            Ok(outcome.changes)
        })();
        let changes = match result {
            Ok(changes) => {
                self.executor.commit()?;
                changes
            }
            Err(err) => {
                let _ = self.executor.rollback();
                return Err(err);
            }
        };
        if changes == 0 {
            return Err(LocalEventsError::Sql(SqlError::NotFound(
                update.record_id.clone(),
            )));
        }
        self.get_record(&update.record_id)?
            .ok_or_else(|| LocalEventsError::Sql(SqlError::NotFound(update.record_id.clone())))
    }

    pub fn get_cursor(
        &self,
        consumer_id: &str,
    ) -> Result<Option<LocalEventsCursor>, LocalEventsError> {
        validate_non_empty("consumer_id", consumer_id)?;
        let params = json!([consumer_id]).to_string();
        let raw = self.executor.query_raw(
            "select consumer_id, last_change_seq, updated_at_ms from local_event_projection_cursor where consumer_id = ? limit 1",
            &params,
        )?;
        let rows: Vec<CursorRow> = serde_json::from_str(&raw)?;
        Ok(rows.into_iter().next().map(Into::into))
    }

    pub fn advance_cursor(
        &self,
        consumer_id: &str,
        last_change_seq: i64,
        updated_at_ms: i64,
    ) -> Result<LocalEventsCursor, LocalEventsError> {
        validate_non_empty("consumer_id", consumer_id)?;
        let params = json!([consumer_id, last_change_seq, updated_at_ms]).to_string();
        self.executor.exec(
            "insert into local_event_projection_cursor(consumer_id, last_change_seq, updated_at_ms)
             values(?,?,?)
             on conflict(consumer_id) do update set
                 last_change_seq = max(local_event_projection_cursor.last_change_seq, excluded.last_change_seq),
                 updated_at_ms = excluded.updated_at_ms",
            &params,
        )?;
        self.get_cursor(consumer_id)?
            .ok_or_else(|| LocalEventsError::InvalidRecord("cursor advance failed".to_owned()))
    }

    fn query_records(
        &self,
        sql: &str,
        params: &str,
    ) -> Result<Vec<LocalEventRecord>, LocalEventsError> {
        let raw = self.executor.query_raw(sql, params)?;
        let rows: Vec<RecordRow> = serde_json::from_str(&raw)?;
        rows.into_iter().map(TryInto::try_into).collect()
    }

    fn next_change_seq(&self) -> Result<i64, LocalEventsError> {
        let raw = self.executor.query_raw(
            "select coalesce(max(change_seq), 0) + 1 as change_seq from local_event_record",
            "[]",
        )?;
        let rows: Vec<ChangeSeqRow> = serde_json::from_str(&raw)?;
        rows.into_iter()
            .next()
            .map(|row| row.change_seq)
            .ok_or_else(|| {
                LocalEventsError::InvalidRecord("change sequence unavailable".to_owned())
            })
    }
}

#[derive(Debug, Deserialize)]
struct RecordRow {
    seq: i64,
    change_seq: i64,
    record_id: String,
    family: String,
    status: String,
    source_runtime: String,
    created_at_ms: i64,
    inserted_at_ms: i64,
    updated_at_ms: i64,
    owner_account_id: Option<String>,
    owner_pubkey: Option<String>,
    farm_id: Option<String>,
    listing_addr: Option<String>,
    local_work_json: Option<String>,
    event_id: Option<String>,
    event_kind: Option<i64>,
    event_pubkey: Option<String>,
    event_created_at: Option<i64>,
    event_tags_json: Option<String>,
    event_content: Option<String>,
    event_sig: Option<String>,
    raw_event_json: Option<String>,
    outbox_status: String,
    relay_set_fingerprint: Option<String>,
    relay_delivery_json: Option<String>,
}

impl TryFrom<RecordRow> for LocalEventRecord {
    type Error = LocalEventsError;

    fn try_from(row: RecordRow) -> Result<Self, Self::Error> {
        Ok(Self {
            seq: row.seq,
            change_seq: row.change_seq,
            record_id: row.record_id,
            family: LocalRecordFamily::parse(&row.family)?,
            status: LocalRecordStatus::parse(&row.status)?,
            source_runtime: SourceRuntime::parse(&row.source_runtime)?,
            created_at_ms: row.created_at_ms,
            inserted_at_ms: row.inserted_at_ms,
            updated_at_ms: row.updated_at_ms,
            owner_account_id: row.owner_account_id,
            owner_pubkey: row.owner_pubkey,
            farm_id: row.farm_id,
            listing_addr: row.listing_addr,
            local_work_json: decode_json(row.local_work_json)?,
            event_id: row.event_id,
            event_kind: row.event_kind,
            event_pubkey: row.event_pubkey,
            event_created_at: row.event_created_at,
            event_tags_json: decode_json(row.event_tags_json)?,
            event_content: row.event_content,
            event_sig: row.event_sig,
            raw_event_json: decode_json(row.raw_event_json)?,
            outbox_status: PublishOutboxStatus::parse(&row.outbox_status)?,
            relay_set_fingerprint: row.relay_set_fingerprint,
            relay_delivery_json: decode_json(row.relay_delivery_json)?,
        })
    }
}

#[derive(Debug, Deserialize)]
struct CursorRow {
    consumer_id: String,
    last_change_seq: i64,
    updated_at_ms: i64,
}

impl From<CursorRow> for LocalEventsCursor {
    fn from(row: CursorRow) -> Self {
        Self {
            consumer_id: row.consumer_id,
            last_change_seq: row.last_change_seq,
            updated_at_ms: row.updated_at_ms,
        }
    }
}

#[derive(Debug, Deserialize)]
struct ChangeSeqRow {
    change_seq: i64,
}

fn encode_json(value: Option<&Value>) -> Option<String> {
    value.map(Value::to_string)
}

fn decode_json(value: Option<String>) -> Result<Option<Value>, LocalEventsError> {
    value
        .map(|value| serde_json::from_str(&value))
        .transpose()
        .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use radroots_sql_core::{ExecOutcome, SqlExecutor, SqliteExecutor};
    use serde_json::json;

    use super::*;

    fn store() -> LocalEventsStore<SqliteExecutor> {
        let executor = SqliteExecutor::open_memory().expect("open memory sqlite");
        let store = LocalEventsStore::new(executor);
        store.migrate_up().expect("migrate up");
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
            event_id: Some(record_id.to_owned()),
            event_kind: Some(3421),
            event_pubkey: Some("seller-pubkey".to_owned()),
            event_created_at: Some(2000),
            event_tags_json: Some(json!([["d", "listing-a"]])),
            event_content: Some("{\"title\":\"Eggs\"}".to_owned()),
            event_sig: Some("sig-a".to_owned()),
            raw_event_json: Some(json!({"id":record_id,"kind":3421})),
            outbox_status: PublishOutboxStatus::Pending,
            relay_set_fingerprint: Some("relay-set-a".to_owned()),
            relay_delivery_json: Some(json!({
                "state": "pending",
                "target_relays": ["ws://127.0.0.1:8080"],
                "connected_relays": [],
                "acknowledged_relays": [],
                "failed_relays": []
            })),
        }
    }

    #[derive(Debug)]
    struct ScriptedExecutor {
        begin_result: Mutex<Result<(), SqlError>>,
        commit_result: Mutex<Result<(), SqlError>>,
        exec_results: Mutex<VecDeque<Result<ExecOutcome, SqlError>>>,
        query_results: Mutex<VecDeque<Result<String, SqlError>>>,
        rollbacks: AtomicUsize,
    }

    impl ScriptedExecutor {
        fn new(
            exec_results: Vec<Result<ExecOutcome, SqlError>>,
            query_results: Vec<Result<String, SqlError>>,
        ) -> Self {
            Self {
                begin_result: Mutex::new(Ok(())),
                commit_result: Mutex::new(Ok(())),
                exec_results: Mutex::new(exec_results.into()),
                query_results: Mutex::new(query_results.into()),
                rollbacks: AtomicUsize::new(0),
            }
        }

        fn with_begin_error(error: SqlError) -> Self {
            let executor = Self::new(Vec::new(), Vec::new());
            *executor.begin_result.lock().expect("begin result") = Err(error);
            executor
        }

        fn with_commit_error(error: SqlError) -> Self {
            let executor = Self::new(
                vec![Ok(ExecOutcome {
                    changes: 1,
                    last_insert_id: 0,
                })],
                vec![Ok(r#"[{"change_seq":1}]"#.to_owned())],
            );
            *executor.commit_result.lock().expect("commit result") = Err(error);
            executor
        }
    }

    impl SqlExecutor for ScriptedExecutor {
        fn exec(&self, _sql: &str, _params_json: &str) -> Result<ExecOutcome, SqlError> {
            self.exec_results
                .lock()
                .expect("exec results")
                .pop_front()
                .unwrap_or(Ok(ExecOutcome {
                    changes: 1,
                    last_insert_id: 0,
                }))
        }

        fn query_raw(&self, _sql: &str, _params_json: &str) -> Result<String, SqlError> {
            self.query_results
                .lock()
                .expect("query results")
                .pop_front()
                .unwrap_or_else(|| Ok("[]".to_owned()))
        }

        fn begin(&self) -> Result<(), SqlError> {
            self.begin_result.lock().expect("begin result").clone()
        }

        fn commit(&self) -> Result<(), SqlError> {
            self.commit_result.lock().expect("commit result").clone()
        }

        fn rollback(&self) -> Result<(), SqlError> {
            self.rollbacks.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    fn record_row_with(field: &str, value: serde_json::Value) -> String {
        let mut row = json!({
            "seq": 1,
            "change_seq": 1,
            "record_id": "record-a",
            "family": "signed_event",
            "status": "pending_publish",
            "source_runtime": "cli",
            "created_at_ms": 1000,
            "inserted_at_ms": 1001,
            "updated_at_ms": 1001,
            "owner_account_id": "seller-account",
            "owner_pubkey": "seller-pubkey",
            "farm_id": "farm-a",
            "listing_addr": "listing-a",
            "local_work_json": null,
            "event_id": "event-a",
            "event_kind": 3421,
            "event_pubkey": "seller-pubkey",
            "event_created_at": 1000,
            "event_tags_json": "[[\"d\",\"listing-a\"]]",
            "event_content": "{}",
            "event_sig": "sig-a",
            "raw_event_json": "{\"id\":\"event-a\",\"kind\":3421}",
            "outbox_status": "pending",
            "relay_set_fingerprint": "relay-set-a",
            "relay_delivery_json": "{\"state\":\"pending\",\"target_relays\":[\"ws://127.0.0.1:8080\"],\"connected_relays\":[],\"acknowledged_relays\":[],\"failed_relays\":[]}"
        });
        row[field] = value;
        json!([row]).to_string()
    }

    #[test]
    fn store_methods_round_trip_records_and_cursors() {
        let store = store();

        assert!(
            store
                .executor()
                .query_raw("select 1 as value", "[]")
                .is_ok()
        );
        assert!(store.get_record("missing").expect("get missing").is_none());
        assert!(store.get_cursor("app").expect("cursor missing").is_none());

        let local = store
            .append_record(&local_work("local-a"))
            .expect("append local work");
        let event = store
            .append_record(&signed_event("event-a"))
            .expect("append signed event");

        assert_eq!(
            store
                .get_record("local-a")
                .expect("get local")
                .expect("local record")
                .record_id,
            local.record_id
        );
        assert_eq!(
            store
                .list_records_after_seq(0, 10)
                .expect("list after seq")
                .len(),
            2
        );
        assert_eq!(
            store
                .list_records_changed_after(local.change_seq, 10)
                .expect("list changed after")[0]
                .record_id,
            event.record_id
        );
        assert_eq!(
            store.list_records_changed_latest(1).expect("list latest")[0].record_id,
            event.record_id
        );
        assert_eq!(
            store
                .list_records_changed_before(event.change_seq, event.seq, 10)
                .expect("list before")[0]
                .record_id,
            local.record_id
        );

        let cursor = store
            .advance_cursor("app", event.change_seq, 3000)
            .expect("advance cursor");
        assert_eq!(cursor.consumer_id, "app");
        assert_eq!(
            store
                .get_cursor("app")
                .expect("get cursor")
                .expect("cursor")
                .last_change_seq,
            event.change_seq
        );

        let updated = store
            .update_outbox(&LocalEventRecordUpdate {
                record_id: "event-a".to_owned(),
                status: LocalRecordStatus::Published,
                outbox_status: PublishOutboxStatus::Acknowledged,
                relay_set_fingerprint: Some("relay-set-a".to_owned()),
                relay_delivery_json: Some(json!({
                    "state": "acknowledged",
                    "target_relays": ["ws://127.0.0.1:8080"],
                    "connected_relays": ["ws://127.0.0.1:8080"],
                    "acknowledged_relays": ["ws://127.0.0.1:8080"],
                    "failed_relays": []
                })),
                updated_at_ms: 4000,
            })
            .expect("update outbox");

        assert_eq!(updated.status, LocalRecordStatus::Published);
        assert_eq!(updated.outbox_status, PublishOutboxStatus::Acknowledged);
        store.migrate_down().expect("migrate down");
    }

    #[test]
    fn store_reports_missing_updates_and_decode_errors() {
        let store = store();
        assert!(
            store
                .get_record(" ")
                .expect_err("empty record id")
                .to_string()
                .contains("record_id")
        );
        assert!(
            store
                .get_cursor(" ")
                .expect_err("empty consumer id")
                .to_string()
                .contains("consumer_id")
        );
        assert!(
            store
                .advance_cursor(" ", 1, 1000)
                .expect_err("empty cursor consumer")
                .to_string()
                .contains("consumer_id")
        );
        assert!(
            store
                .update_outbox(&LocalEventRecordUpdate {
                    record_id: " ".to_owned(),
                    status: LocalRecordStatus::Published,
                    outbox_status: PublishOutboxStatus::Acknowledged,
                    relay_set_fingerprint: None,
                    relay_delivery_json: None,
                    updated_at_ms: 4000,
                })
                .expect_err("empty update record id")
                .to_string()
                .contains("record_id")
        );

        let missing_update = store
            .update_outbox(&LocalEventRecordUpdate {
                record_id: "missing-event".to_owned(),
                status: LocalRecordStatus::Published,
                outbox_status: PublishOutboxStatus::Acknowledged,
                relay_set_fingerprint: None,
                relay_delivery_json: None,
                updated_at_ms: 4000,
            })
            .expect_err("missing record update");

        assert!(missing_update.to_string().contains("missing-event"));

        store
            .append_record(&local_work("local-a"))
            .expect("append local");
        let params = json!(["{", "local-a"]).to_string();
        store
            .executor()
            .exec(
                "update local_event_record set local_work_json = ? where record_id = ?",
                &params,
            )
            .expect("corrupt local work json");
        let decode_error = store.get_record("local-a").expect_err("decode error");

        assert!(decode_error.to_string().contains("EOF"));
    }

    #[test]
    fn store_rolls_back_when_change_sequence_is_unavailable() {
        let append_store =
            LocalEventsStore::new(ScriptedExecutor::new(Vec::new(), vec![Ok("[]".to_owned())]));
        let append_error = append_store
            .append_record(&local_work("local-a"))
            .expect_err("append error");

        assert!(append_error.to_string().contains("change sequence"));
        assert_eq!(append_store.executor().rollbacks.load(Ordering::SeqCst), 1);

        let update_store =
            LocalEventsStore::new(ScriptedExecutor::new(Vec::new(), vec![Ok("[]".to_owned())]));
        let update_error = update_store
            .update_outbox(&LocalEventRecordUpdate {
                record_id: "event-a".to_owned(),
                status: LocalRecordStatus::Published,
                outbox_status: PublishOutboxStatus::Acknowledged,
                relay_set_fingerprint: None,
                relay_delivery_json: None,
                updated_at_ms: 4000,
            })
            .expect_err("update error");

        assert!(update_error.to_string().contains("change sequence"));
        assert_eq!(update_store.executor().rollbacks.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn store_reports_cursor_advance_without_returned_cursor() {
        let store = LocalEventsStore::new(ScriptedExecutor::new(Vec::new(), Vec::new()));

        assert!(store.get_cursor("app").expect("missing cursor").is_none());
        let cursor_error = store
            .advance_cursor("app", 1, 1000)
            .expect_err("cursor advance error");

        assert!(cursor_error.to_string().contains("cursor advance failed"));
    }

    #[test]
    fn store_reports_executor_and_decode_failures() {
        let begin_store = LocalEventsStore::new(ScriptedExecutor::with_begin_error(
            SqlError::InvalidQuery("begin failed".to_owned()),
        ));
        assert!(
            begin_store
                .append_record(&local_work("local-a"))
                .expect_err("begin failure")
                .to_string()
                .contains("begin failed")
        );

        let exec_store = LocalEventsStore::new(ScriptedExecutor::new(
            vec![Err(SqlError::InvalidQuery("insert failed".to_owned()))],
            vec![Ok(r#"[{"change_seq":1}]"#.to_owned())],
        ));
        assert!(
            exec_store
                .append_record(&local_work("local-a"))
                .expect_err("exec failure")
                .to_string()
                .contains("insert failed")
        );
        assert_eq!(exec_store.executor().rollbacks.load(Ordering::SeqCst), 1);

        let commit_store = LocalEventsStore::new(ScriptedExecutor::with_commit_error(
            SqlError::InvalidQuery("commit failed".to_owned()),
        ));
        assert!(
            commit_store
                .append_record(&local_work("local-a"))
                .expect_err("commit failure")
                .to_string()
                .contains("commit failed")
        );

        let query_error_store = LocalEventsStore::new(ScriptedExecutor::new(
            Vec::new(),
            vec![Err(SqlError::InvalidQuery("query failed".to_owned()))],
        ));
        assert!(
            query_error_store
                .get_record("record-a")
                .expect_err("query failure")
                .to_string()
                .contains("query failed")
        );

        let invalid_rows_store =
            LocalEventsStore::new(ScriptedExecutor::new(Vec::new(), vec![Ok("{".to_owned())]));
        let _ = invalid_rows_store
            .get_record("record-a")
            .expect_err("invalid rows");

        let cursor_rows_store =
            LocalEventsStore::new(ScriptedExecutor::new(Vec::new(), vec![Ok("{".to_owned())]));
        let _ = cursor_rows_store
            .get_cursor("app")
            .expect_err("invalid cursor rows");

        let change_rows_store =
            LocalEventsStore::new(ScriptedExecutor::new(Vec::new(), vec![Ok("{".to_owned())]));
        let _ = change_rows_store
            .append_record(&local_work("local-a"))
            .expect_err("invalid change rows");

        let cursor_exec_store = LocalEventsStore::new(ScriptedExecutor::new(
            vec![Err(SqlError::InvalidQuery("cursor failed".to_owned()))],
            Vec::new(),
        ));
        assert!(
            cursor_exec_store
                .advance_cursor("app", 1, 1000)
                .expect_err("cursor exec failure")
                .to_string()
                .contains("cursor failed")
        );

        let append_lookup_store = LocalEventsStore::new(ScriptedExecutor::new(
            vec![Ok(ExecOutcome {
                changes: 1,
                last_insert_id: 0,
            })],
            vec![Ok(r#"[{"change_seq":1}]"#.to_owned()), Ok("[]".to_owned())],
        ));
        assert!(
            append_lookup_store
                .append_record(&local_work("local-a"))
                .expect_err("append lookup failure")
                .to_string()
                .contains("record append failed")
        );

        let update_lookup_store = LocalEventsStore::new(ScriptedExecutor::new(
            vec![Ok(ExecOutcome {
                changes: 1,
                last_insert_id: 0,
            })],
            vec![Ok(r#"[{"change_seq":1}]"#.to_owned()), Ok("[]".to_owned())],
        ));
        assert!(
            update_lookup_store
                .update_outbox(&LocalEventRecordUpdate {
                    record_id: "event-a".to_owned(),
                    status: LocalRecordStatus::Published,
                    outbox_status: PublishOutboxStatus::Acknowledged,
                    relay_set_fingerprint: None,
                    relay_delivery_json: None,
                    updated_at_ms: 4000,
                })
                .expect_err("update lookup failure")
                .to_string()
                .contains("event-a")
        );

        let cursor_query_store = LocalEventsStore::new(ScriptedExecutor::new(
            Vec::new(),
            vec![Err(SqlError::InvalidQuery(
                "cursor query failed".to_owned(),
            ))],
        ));
        assert!(
            cursor_query_store
                .get_cursor("app")
                .expect_err("cursor query failure")
                .to_string()
                .contains("cursor query failed")
        );

        let advance_cursor_query_store = LocalEventsStore::new(ScriptedExecutor::new(
            vec![Ok(ExecOutcome {
                changes: 1,
                last_insert_id: 0,
            })],
            vec![Err(SqlError::InvalidQuery(
                "advanced cursor query failed".to_owned(),
            ))],
        ));
        assert!(
            advance_cursor_query_store
                .advance_cursor("app", 1, 1000)
                .expect_err("advance cursor query failure")
                .to_string()
                .contains("advanced cursor query failed")
        );

        let change_query_store = LocalEventsStore::new(ScriptedExecutor::new(
            Vec::new(),
            vec![Err(SqlError::InvalidQuery(
                "change query failed".to_owned(),
            ))],
        ));
        assert!(
            change_query_store
                .append_record(&local_work("local-a"))
                .expect_err("change query failure")
                .to_string()
                .contains("change query failed")
        );
    }

    #[test]
    fn store_reports_record_row_conversion_failures() {
        for (field, value, expected) in [
            ("family", json!("bad_family"), "family"),
            ("status", json!("bad_status"), "status"),
            ("source_runtime", json!("bad_runtime"), "runtime"),
            ("event_tags_json", json!("{"), "EOF"),
            ("raw_event_json", json!("{"), "EOF"),
            ("outbox_status", json!("bad_outbox"), "outbox"),
            ("relay_delivery_json", json!("{"), "EOF"),
        ] {
            let store = LocalEventsStore::new(ScriptedExecutor::new(
                Vec::new(),
                vec![Ok(record_row_with(field, value))],
            ));
            let error = store.get_record("record-a").expect_err("conversion error");

            assert!(
                error.to_string().contains(expected),
                "expected error to contain {expected}, got {error}"
            );
        }
    }
}
