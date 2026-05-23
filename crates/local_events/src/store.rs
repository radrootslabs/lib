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
                encode_json(input.local_work_json.as_ref())?,
                input.event_id,
                input.event_kind,
                input.event_pubkey,
                input.event_created_at,
                encode_json(input.event_tags_json.as_ref())?,
                input.event_content,
                input.event_sig,
                encode_json(input.raw_event_json.as_ref())?,
                input.outbox_status.as_str(),
                input.relay_set_fingerprint,
                encode_json(input.relay_delivery_json.as_ref())?
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
                encode_json(update.relay_delivery_json.as_ref())?,
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

fn encode_json(value: Option<&Value>) -> Result<Option<String>, LocalEventsError> {
    value
        .map(serde_json::to_string)
        .transpose()
        .map_err(Into::into)
}

fn decode_json(value: Option<String>) -> Result<Option<Value>, LocalEventsError> {
    value
        .map(|value| serde_json::from_str(&value))
        .transpose()
        .map_err(Into::into)
}
