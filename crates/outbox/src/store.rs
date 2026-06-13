#![forbid(unsafe_code)]

use crate::RadrootsOutboxError;
use crate::migrations::{OUTBOX_MIGRATION_DOWN, OUTBOX_MIGRATION_UP};
use crate::model::{
    RadrootsOutboxClaimedEvent, RadrootsOutboxEnqueueReceipt, RadrootsOutboxEnqueueStatus,
    RadrootsOutboxEventRecord, RadrootsOutboxEventState, RadrootsOutboxEventStoreIngestReceipt,
    RadrootsOutboxOperationInput, RadrootsOutboxOperationRecord, RadrootsOutboxOperationStatus,
    RadrootsOutboxRelayStatus, RadrootsOutboxRelayStatusRecord,
};
use radroots_event_store::{RadrootsEventIngest, RadrootsEventStore};
use radroots_events::RadrootsNostrEvent;
use radroots_events::draft::{RadrootsFrozenEventDraft, RadrootsSignedNostrEvent};
use radroots_nostr::prelude::{RadrootsNostrKeys, radroots_nostr_sign_frozen_draft};
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use std::path::Path;
use std::str::FromStr;

#[derive(Clone)]
pub struct RadrootsOutbox {
    pool: SqlitePool,
}

impl RadrootsOutbox {
    pub async fn open_memory() -> Result<Self, RadrootsOutboxError> {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")?;
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        configure_connection(&pool, false).await?;
        apply_up(&pool).await?;
        Ok(Self { pool })
    }

    pub async fn open_file(path: impl AsRef<Path>) -> Result<Self, RadrootsOutboxError> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        configure_connection(&pool, true).await?;
        apply_up(&pool).await?;
        Ok(Self { pool })
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn migrate_down(&self) -> Result<(), RadrootsOutboxError> {
        apply_down(&self.pool).await
    }

    pub async fn pragma_foreign_keys(&self) -> Result<i64, RadrootsOutboxError> {
        query_i64(&self.pool, "PRAGMA foreign_keys").await
    }

    pub async fn pragma_busy_timeout(&self) -> Result<i64, RadrootsOutboxError> {
        query_i64(&self.pool, "PRAGMA busy_timeout").await
    }

    pub async fn pragma_journal_mode(&self) -> Result<String, RadrootsOutboxError> {
        query_string(&self.pool, "PRAGMA journal_mode").await
    }

    pub async fn enqueue_operation(
        &self,
        input: RadrootsOutboxOperationInput,
    ) -> Result<RadrootsOutboxEnqueueReceipt, RadrootsOutboxError> {
        let target_relays = canonical_relays(input.target_relays);
        let digest = idempotency_digest(
            input.operation_kind.as_str(),
            input.draft.expected_pubkey.as_str(),
            &input.draft,
            &target_relays,
        )?;
        let accepted_quorum = target_relays.len() as i64;
        let mut tx = self.pool.begin().await?;

        if let Some(idempotency_key) = input.idempotency_key.as_deref() {
            if let Some(existing) = existing_idempotent_operation(
                &mut tx,
                input.operation_kind.as_str(),
                input.draft.expected_pubkey.as_str(),
                idempotency_key,
            )
            .await?
            {
                if existing.idempotency_digest != digest {
                    return Err(RadrootsOutboxError::IdempotencyConflict {
                        operation_kind: input.operation_kind,
                        expected_pubkey: input.draft.expected_pubkey,
                        idempotency_key: idempotency_key.to_owned(),
                        existing_digest: existing.idempotency_digest,
                        new_digest: digest,
                    });
                }
                tx.commit().await?;
                return Ok(RadrootsOutboxEnqueueReceipt {
                    status: RadrootsOutboxEnqueueStatus::Existing,
                    operation_id: existing.operation_id,
                    outbox_event_id: existing.outbox_event_id,
                    expected_event_id: existing.event_id,
                    idempotency_digest: digest,
                });
            }
        }

        let operation = sqlx::query(
            "INSERT INTO outbox_operation(operation_kind, expected_pubkey, idempotency_key, idempotency_digest, status, created_at_ms, updated_at_ms) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(input.operation_kind.as_str())
        .bind(input.draft.expected_pubkey.as_str())
        .bind(input.idempotency_key.as_deref())
        .bind(digest.as_str())
        .bind(RadrootsOutboxOperationStatus::Queued.as_str())
        .bind(input.created_at_ms)
        .bind(input.created_at_ms)
        .execute(&mut *tx)
        .await?;
        let operation_id = operation.last_insert_rowid();
        let draft_json = serde_json::to_string(&input.draft)?;
        let event = sqlx::query(
            "INSERT INTO outbox_event(operation_id, event_id, expected_pubkey, draft_json, state, accepted_quorum, attempt_count, next_attempt_after_ms, event_store_ingested, event_store_inserted, created_at_ms, updated_at_ms) VALUES (?, ?, ?, ?, ?, ?, 0, ?, 0, 0, ?, ?)",
        )
        .bind(operation_id)
        .bind(input.draft.expected_event_id.as_str())
        .bind(input.draft.expected_pubkey.as_str())
        .bind(draft_json.as_str())
        .bind(RadrootsOutboxEventState::DraftQueued.as_str())
        .bind(accepted_quorum)
        .bind(input.created_at_ms)
        .bind(input.created_at_ms)
        .bind(input.created_at_ms)
        .execute(&mut *tx)
        .await?;
        let outbox_event_id = event.last_insert_rowid();

        for relay_url in target_relays {
            sqlx::query(
                "INSERT INTO outbox_event_relay_status(outbox_event_id, relay_url, status, attempt_count) VALUES (?, ?, ?, 0)",
            )
            .bind(outbox_event_id)
            .bind(relay_url.as_str())
            .bind(RadrootsOutboxRelayStatus::Pending.as_str())
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(RadrootsOutboxEnqueueReceipt {
            status: RadrootsOutboxEnqueueStatus::Inserted,
            operation_id,
            outbox_event_id,
            expected_event_id: input.draft.expected_event_id,
            idempotency_digest: digest,
        })
    }

    pub async fn get_operation(
        &self,
        operation_id: i64,
    ) -> Result<Option<RadrootsOutboxOperationRecord>, RadrootsOutboxError> {
        let row = sqlx::query(
            "SELECT operation_id, operation_kind, expected_pubkey, idempotency_key, idempotency_digest, status, created_at_ms, updated_at_ms FROM outbox_operation WHERE operation_id = ?",
        )
        .bind(operation_id)
        .fetch_optional(&self.pool)
        .await?;
        row.map(operation_from_row).transpose()
    }

    pub async fn get_event(
        &self,
        outbox_event_id: i64,
    ) -> Result<Option<RadrootsOutboxEventRecord>, RadrootsOutboxError> {
        let row = sqlx::query(
            "SELECT outbox_event_id, operation_id, event_id, expected_pubkey, draft_json, signed_event_json, raw_event_json, state, accepted_quorum, attempt_count, claim_token, claim_owner, claim_expires_at_ms, next_attempt_after_ms, last_error, event_store_ingested, event_store_inserted, event_store_ingested_at_ms, created_at_ms, updated_at_ms FROM outbox_event WHERE outbox_event_id = ?",
        )
        .bind(outbox_event_id)
        .fetch_optional(&self.pool)
        .await?;
        row.map(event_from_row).transpose()
    }

    pub async fn relay_statuses(
        &self,
        outbox_event_id: i64,
    ) -> Result<Vec<RadrootsOutboxRelayStatusRecord>, RadrootsOutboxError> {
        relay_statuses_for(&self.pool, outbox_event_id).await
    }

    pub async fn claim_next_ready_event(
        &self,
        claim_owner: impl AsRef<str>,
        claim_token: impl AsRef<str>,
        claim_expires_at_ms: i64,
        now_ms: i64,
    ) -> Result<Option<RadrootsOutboxClaimedEvent>, RadrootsOutboxError> {
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query(
            "SELECT outbox_event_id, state, signed_event_json FROM outbox_event WHERE state IN ('draft_queued', 'sign_retryable', 'signed', 'publish_retryable') AND next_attempt_after_ms <= ? AND (claim_token IS NULL OR claim_expires_at_ms <= ?) ORDER BY created_at_ms, outbox_event_id LIMIT 1",
        )
        .bind(now_ms)
        .bind(now_ms)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(row) = row else {
            tx.commit().await?;
            return Ok(None);
        };
        let outbox_event_id: i64 = row.try_get("outbox_event_id")?;
        let state = RadrootsOutboxEventState::parse(row.try_get::<String, _>("state")?.as_str())?;
        let signed_event_json: Option<String> = row.try_get("signed_event_json")?;
        let claimed_state = match (state, signed_event_json.as_ref()) {
            (
                RadrootsOutboxEventState::DraftQueued | RadrootsOutboxEventState::SignRetryable,
                None,
            ) => RadrootsOutboxEventState::Signing,
            _ => RadrootsOutboxEventState::Publishing,
        };
        let changed = sqlx::query(
            "UPDATE outbox_event SET state = ?, claim_token = ?, claim_owner = ?, claim_expires_at_ms = ?, attempt_count = attempt_count + 1, updated_at_ms = ? WHERE outbox_event_id = ? AND (claim_token IS NULL OR claim_expires_at_ms <= ?)",
        )
        .bind(claimed_state.as_str())
        .bind(claim_token.as_ref())
        .bind(claim_owner.as_ref())
        .bind(claim_expires_at_ms)
        .bind(now_ms)
        .bind(outbox_event_id)
        .bind(now_ms)
        .execute(&mut *tx)
        .await?;
        if changed.rows_affected() == 0 {
            tx.commit().await?;
            return Ok(None);
        }
        let record = event_by_id_tx(&mut tx, outbox_event_id).await?;
        let target_relays = relay_urls_for_tx(&mut tx, outbox_event_id).await?;
        tx.commit().await?;
        Ok(Some(RadrootsOutboxClaimedEvent {
            outbox_event_id: record.outbox_event_id,
            operation_id: record.operation_id,
            expected_event_id: record.event_id,
            state: claimed_state,
            claim_token: claim_token.as_ref().to_owned(),
            draft: record.draft,
            signed_event: record.signed_event,
            target_relays,
        }))
    }

    pub async fn complete_signing(
        &self,
        outbox_event_id: i64,
        claim_token: &str,
        signed_event: RadrootsSignedNostrEvent,
        now_ms: i64,
    ) -> Result<RadrootsSignedNostrEvent, RadrootsOutboxError> {
        let record = self.claimed_event(outbox_event_id, claim_token).await?;
        if signed_event.id != record.event_id {
            return Err(RadrootsOutboxError::SignedEventIdMismatch {
                expected_event_id: record.event_id,
                actual_event_id: signed_event.id,
            });
        }
        let signed_event_json = serde_json::to_string(&signed_event)?;
        sqlx::query(
            "UPDATE outbox_event SET signed_event_json = ?, raw_event_json = ?, state = ?, last_error = NULL, updated_at_ms = ? WHERE outbox_event_id = ? AND claim_token = ?",
        )
        .bind(signed_event_json.as_str())
        .bind(signed_event.raw_json.as_str())
        .bind(RadrootsOutboxEventState::Signed.as_str())
        .bind(now_ms)
        .bind(outbox_event_id)
        .bind(claim_token)
        .execute(&self.pool)
        .await?;
        Ok(signed_event)
    }

    pub async fn sign_claimed_event(
        &self,
        claimed: &RadrootsOutboxClaimedEvent,
        keys: &RadrootsNostrKeys,
        now_ms: i64,
    ) -> Result<RadrootsSignedNostrEvent, RadrootsOutboxError> {
        if let Some(signed_event) = claimed.signed_event.clone() {
            return Ok(signed_event);
        }
        let signed_event = radroots_nostr_sign_frozen_draft(keys, &claimed.draft)?;
        self.complete_signing(
            claimed.outbox_event_id,
            claimed.claim_token.as_str(),
            signed_event,
            now_ms,
        )
        .await
    }

    pub async fn mark_sign_retryable(
        &self,
        outbox_event_id: i64,
        claim_token: &str,
        error: impl AsRef<str>,
        next_attempt_after_ms: i64,
        now_ms: i64,
    ) -> Result<(), RadrootsOutboxError> {
        self.ensure_claim_token(outbox_event_id, claim_token)
            .await?;
        sqlx::query(
            "UPDATE outbox_event SET state = ?, claim_token = NULL, claim_owner = NULL, claim_expires_at_ms = NULL, last_error = ?, next_attempt_after_ms = ?, updated_at_ms = ? WHERE outbox_event_id = ?",
        )
        .bind(RadrootsOutboxEventState::SignRetryable.as_str())
        .bind(error.as_ref())
        .bind(next_attempt_after_ms)
        .bind(now_ms)
        .bind(outbox_event_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn mark_publish_retryable(
        &self,
        outbox_event_id: i64,
        claim_token: &str,
        error: impl AsRef<str>,
        next_attempt_after_ms: i64,
        now_ms: i64,
    ) -> Result<(), RadrootsOutboxError> {
        self.ensure_claim_token(outbox_event_id, claim_token)
            .await?;
        sqlx::query(
            "UPDATE outbox_event SET state = ?, claim_token = NULL, claim_owner = NULL, claim_expires_at_ms = NULL, last_error = ?, next_attempt_after_ms = ?, updated_at_ms = ? WHERE outbox_event_id = ?",
        )
        .bind(RadrootsOutboxEventState::PublishRetryable.as_str())
        .bind(error.as_ref())
        .bind(next_attempt_after_ms)
        .bind(now_ms)
        .bind(outbox_event_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn recover_expired_claims(&self, now_ms: i64) -> Result<u64, RadrootsOutboxError> {
        let changed = sqlx::query(
            "UPDATE outbox_event SET state = CASE WHEN state = 'signing' AND signed_event_json IS NULL THEN 'sign_retryable' WHEN state = 'signing' AND signed_event_json IS NOT NULL THEN 'signed' WHEN state = 'publishing' THEN 'publish_retryable' ELSE state END, claim_token = NULL, claim_owner = NULL, claim_expires_at_ms = NULL, updated_at_ms = ? WHERE claim_token IS NOT NULL AND claim_expires_at_ms <= ? AND state IN ('signing', 'signed', 'publishing')",
        )
        .bind(now_ms)
        .bind(now_ms)
        .execute(&self.pool)
        .await?;
        Ok(changed.rows_affected())
    }

    pub async fn ingest_signed_event_local(
        &self,
        event_store: &RadrootsEventStore,
        outbox_event_id: i64,
        claim_token: &str,
        observed_at_ms: i64,
    ) -> Result<RadrootsOutboxEventStoreIngestReceipt, RadrootsOutboxError> {
        let record = self.claimed_event(outbox_event_id, claim_token).await?;
        if record.event_store_ingested {
            return Ok(RadrootsOutboxEventStoreIngestReceipt {
                outbox_event_id,
                event_id: record.event_id,
                already_ingested: true,
                event_store_inserted: false,
            });
        }
        let signed_event = record
            .signed_event
            .ok_or(RadrootsOutboxError::MissingSignedEvent(outbox_event_id))?;
        let event = event_from_signed(&signed_event);
        let ingest = RadrootsEventIngest::verified(event, observed_at_ms)
            .with_raw_json(signed_event.raw_json.clone());
        let receipt = event_store.ingest_event(ingest).await?;
        sqlx::query(
            "UPDATE outbox_event SET event_store_ingested = 1, event_store_inserted = ?, event_store_ingested_at_ms = ?, state = ?, updated_at_ms = ? WHERE outbox_event_id = ? AND claim_token = ?",
        )
        .bind(bool_i64(receipt.inserted))
        .bind(observed_at_ms)
        .bind(RadrootsOutboxEventState::Publishing.as_str())
        .bind(observed_at_ms)
        .bind(outbox_event_id)
        .bind(claim_token)
        .execute(&self.pool)
        .await?;
        Ok(RadrootsOutboxEventStoreIngestReceipt {
            outbox_event_id,
            event_id: receipt.event_id,
            already_ingested: false,
            event_store_inserted: receipt.inserted,
        })
    }

    pub async fn mark_relay_accepted(
        &self,
        outbox_event_id: i64,
        claim_token: &str,
        relay_url: &str,
        acknowledged_at_ms: i64,
    ) -> Result<(), RadrootsOutboxError> {
        self.ensure_claim_token(outbox_event_id, claim_token)
            .await?;
        sqlx::query(
            "UPDATE outbox_event_relay_status SET status = ?, attempt_count = attempt_count + 1, last_attempt_at_ms = ?, acknowledged_at_ms = ?, last_error = NULL WHERE outbox_event_id = ? AND relay_url = ?",
        )
        .bind(RadrootsOutboxRelayStatus::Accepted.as_str())
        .bind(acknowledged_at_ms)
        .bind(acknowledged_at_ms)
        .bind(outbox_event_id)
        .bind(relay_url)
        .execute(&self.pool)
        .await?;
        let remaining: i64 = sqlx::query(
            "SELECT COUNT(*) FROM outbox_event_relay_status WHERE outbox_event_id = ? AND status != ?",
        )
        .bind(outbox_event_id)
        .bind(RadrootsOutboxRelayStatus::Accepted.as_str())
        .fetch_one(&self.pool)
        .await?
        .try_get(0)?;
        if remaining == 0 {
            sqlx::query(
                "UPDATE outbox_event SET state = ?, claim_token = NULL, claim_owner = NULL, claim_expires_at_ms = NULL, updated_at_ms = ? WHERE outbox_event_id = ? AND claim_token = ?",
            )
            .bind(RadrootsOutboxEventState::Published.as_str())
            .bind(acknowledged_at_ms)
            .bind(outbox_event_id)
            .bind(claim_token)
            .execute(&self.pool)
            .await?;
            let operation_id: i64 =
                sqlx::query("SELECT operation_id FROM outbox_event WHERE outbox_event_id = ?")
                    .bind(outbox_event_id)
                    .fetch_one(&self.pool)
                    .await?
                    .try_get("operation_id")?;
            sqlx::query(
                "UPDATE outbox_operation SET status = ?, updated_at_ms = ? WHERE operation_id = ?",
            )
            .bind(RadrootsOutboxOperationStatus::Complete.as_str())
            .bind(acknowledged_at_ms)
            .bind(operation_id)
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    pub async fn set_publish_quorum(
        &self,
        outbox_event_id: i64,
        claim_token: &str,
        accepted_quorum: i64,
        now_ms: i64,
    ) -> Result<(), RadrootsOutboxError> {
        self.ensure_claim_token(outbox_event_id, claim_token)
            .await?;
        sqlx::query(
            "UPDATE outbox_event SET accepted_quorum = ?, updated_at_ms = ? WHERE outbox_event_id = ? AND claim_token = ?",
        )
        .bind(accepted_quorum)
        .bind(now_ms)
        .bind(outbox_event_id)
        .bind(claim_token)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn mark_relay_failed_retryable(
        &self,
        outbox_event_id: i64,
        claim_token: &str,
        relay_url: &str,
        error: &str,
        attempted_at_ms: i64,
    ) -> Result<(), RadrootsOutboxError> {
        self.mark_relay_failed(
            outbox_event_id,
            claim_token,
            relay_url,
            RadrootsOutboxRelayStatus::FailedRetryable,
            error,
            attempted_at_ms,
        )
        .await
    }

    pub async fn mark_relay_failed_terminal(
        &self,
        outbox_event_id: i64,
        claim_token: &str,
        relay_url: &str,
        error: &str,
        attempted_at_ms: i64,
    ) -> Result<(), RadrootsOutboxError> {
        self.mark_relay_failed(
            outbox_event_id,
            claim_token,
            relay_url,
            RadrootsOutboxRelayStatus::FailedTerminal,
            error,
            attempted_at_ms,
        )
        .await
    }

    async fn claimed_event(
        &self,
        outbox_event_id: i64,
        claim_token: &str,
    ) -> Result<RadrootsOutboxEventRecord, RadrootsOutboxError> {
        self.ensure_claim_token(outbox_event_id, claim_token)
            .await?;
        self.get_event(outbox_event_id)
            .await?
            .ok_or(RadrootsOutboxError::EventNotFound(outbox_event_id))
    }

    async fn ensure_claim_token(
        &self,
        outbox_event_id: i64,
        claim_token: &str,
    ) -> Result<(), RadrootsOutboxError> {
        let row = sqlx::query("SELECT claim_token FROM outbox_event WHERE outbox_event_id = ?")
            .bind(outbox_event_id)
            .fetch_optional(&self.pool)
            .await?;
        let Some(row) = row else {
            return Err(RadrootsOutboxError::EventNotFound(outbox_event_id));
        };
        let stored: Option<String> = row.try_get("claim_token")?;
        if stored.as_deref() != Some(claim_token) {
            return Err(RadrootsOutboxError::ClaimTokenMismatch { outbox_event_id });
        }
        Ok(())
    }

    async fn mark_relay_failed(
        &self,
        outbox_event_id: i64,
        claim_token: &str,
        relay_url: &str,
        status: RadrootsOutboxRelayStatus,
        error: &str,
        attempted_at_ms: i64,
    ) -> Result<(), RadrootsOutboxError> {
        self.ensure_claim_token(outbox_event_id, claim_token)
            .await?;
        sqlx::query(
            "UPDATE outbox_event_relay_status SET status = ?, attempt_count = attempt_count + 1, last_attempt_at_ms = ?, acknowledged_at_ms = NULL, last_error = ? WHERE outbox_event_id = ? AND relay_url = ?",
        )
        .bind(status.as_str())
        .bind(attempted_at_ms)
        .bind(error)
        .bind(outbox_event_id)
        .bind(relay_url)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

struct ExistingOperation {
    operation_id: i64,
    outbox_event_id: i64,
    event_id: String,
    idempotency_digest: String,
}

async fn configure_connection(
    pool: &SqlitePool,
    file_backed: bool,
) -> Result<(), RadrootsOutboxError> {
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(pool)
        .await?;
    sqlx::query("PRAGMA busy_timeout = 5000")
        .execute(pool)
        .await?;
    if file_backed {
        sqlx::query("PRAGMA journal_mode = WAL")
            .execute(pool)
            .await?;
    }
    Ok(())
}

async fn apply_up(pool: &SqlitePool) -> Result<(), RadrootsOutboxError> {
    sqlx::raw_sql(OUTBOX_MIGRATION_UP).execute(pool).await?;
    Ok(())
}

async fn apply_down(pool: &SqlitePool) -> Result<(), RadrootsOutboxError> {
    sqlx::raw_sql(OUTBOX_MIGRATION_DOWN).execute(pool).await?;
    Ok(())
}

async fn query_i64(pool: &SqlitePool, sql: &str) -> Result<i64, RadrootsOutboxError> {
    let row = sqlx::query(sql).fetch_one(pool).await?;
    Ok(row.try_get(0)?)
}

async fn query_string(pool: &SqlitePool, sql: &str) -> Result<String, RadrootsOutboxError> {
    let row = sqlx::query(sql).fetch_one(pool).await?;
    Ok(row.try_get(0)?)
}

async fn existing_idempotent_operation(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    operation_kind: &str,
    expected_pubkey: &str,
    idempotency_key: &str,
) -> Result<Option<ExistingOperation>, RadrootsOutboxError> {
    let row = sqlx::query(
        "SELECT o.operation_id, o.idempotency_digest, e.outbox_event_id, e.event_id FROM outbox_operation o JOIN outbox_event e ON e.operation_id = o.operation_id WHERE o.operation_kind = ? AND o.expected_pubkey = ? AND o.idempotency_key = ? ORDER BY e.outbox_event_id LIMIT 1",
    )
    .bind(operation_kind)
    .bind(expected_pubkey)
    .bind(idempotency_key)
    .fetch_optional(&mut **tx)
    .await?;
    row.map(|row| {
        Ok(ExistingOperation {
            operation_id: row.try_get("operation_id")?,
            outbox_event_id: row.try_get("outbox_event_id")?,
            event_id: row.try_get("event_id")?,
            idempotency_digest: row.try_get("idempotency_digest")?,
        })
    })
    .transpose()
}

async fn event_by_id_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    outbox_event_id: i64,
) -> Result<RadrootsOutboxEventRecord, RadrootsOutboxError> {
    let row = sqlx::query(
        "SELECT outbox_event_id, operation_id, event_id, expected_pubkey, draft_json, signed_event_json, raw_event_json, state, accepted_quorum, attempt_count, claim_token, claim_owner, claim_expires_at_ms, next_attempt_after_ms, last_error, event_store_ingested, event_store_inserted, event_store_ingested_at_ms, created_at_ms, updated_at_ms FROM outbox_event WHERE outbox_event_id = ?",
    )
    .bind(outbox_event_id)
    .fetch_one(&mut **tx)
    .await?;
    event_from_row(row)
}

async fn relay_urls_for_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    outbox_event_id: i64,
) -> Result<Vec<String>, RadrootsOutboxError> {
    let rows = sqlx::query(
        "SELECT relay_url FROM outbox_event_relay_status WHERE outbox_event_id = ? ORDER BY relay_url",
    )
    .bind(outbox_event_id)
    .fetch_all(&mut **tx)
    .await?;
    rows.into_iter()
        .map(|row| row.try_get("relay_url").map_err(Into::into))
        .collect()
}

async fn relay_statuses_for(
    pool: &SqlitePool,
    outbox_event_id: i64,
) -> Result<Vec<RadrootsOutboxRelayStatusRecord>, RadrootsOutboxError> {
    let rows = sqlx::query(
        "SELECT outbox_event_id, relay_url, status, attempt_count, last_attempt_at_ms, acknowledged_at_ms, last_error FROM outbox_event_relay_status WHERE outbox_event_id = ? ORDER BY relay_url",
    )
    .bind(outbox_event_id)
    .fetch_all(pool)
    .await?;
    rows.into_iter().map(relay_status_from_row).collect()
}

fn operation_from_row(
    row: sqlx::sqlite::SqliteRow,
) -> Result<RadrootsOutboxOperationRecord, RadrootsOutboxError> {
    let status =
        RadrootsOutboxOperationStatus::parse(row.try_get::<String, _>("status")?.as_str())?;
    Ok(RadrootsOutboxOperationRecord {
        operation_id: row.try_get("operation_id")?,
        operation_kind: row.try_get("operation_kind")?,
        expected_pubkey: row.try_get("expected_pubkey")?,
        idempotency_key: row.try_get("idempotency_key")?,
        idempotency_digest: row.try_get("idempotency_digest")?,
        status,
        created_at_ms: row.try_get("created_at_ms")?,
        updated_at_ms: row.try_get("updated_at_ms")?,
    })
}

fn event_from_row(
    row: sqlx::sqlite::SqliteRow,
) -> Result<RadrootsOutboxEventRecord, RadrootsOutboxError> {
    let draft: RadrootsFrozenEventDraft =
        serde_json::from_str(row.try_get::<String, _>("draft_json")?.as_str())?;
    let signed_event = row
        .try_get::<Option<String>, _>("signed_event_json")?
        .map(|json| serde_json::from_str(json.as_str()))
        .transpose()?;
    let state = RadrootsOutboxEventState::parse(row.try_get::<String, _>("state")?.as_str())?;
    Ok(RadrootsOutboxEventRecord {
        outbox_event_id: row.try_get("outbox_event_id")?,
        operation_id: row.try_get("operation_id")?,
        event_id: row.try_get("event_id")?,
        expected_pubkey: row.try_get("expected_pubkey")?,
        draft,
        signed_event,
        raw_event_json: row.try_get("raw_event_json")?,
        state,
        accepted_quorum: row.try_get("accepted_quorum")?,
        attempt_count: row.try_get("attempt_count")?,
        claim_token: row.try_get("claim_token")?,
        claim_owner: row.try_get("claim_owner")?,
        claim_expires_at_ms: row.try_get("claim_expires_at_ms")?,
        next_attempt_after_ms: row.try_get("next_attempt_after_ms")?,
        last_error: row.try_get("last_error")?,
        event_store_ingested: row.try_get::<i64, _>("event_store_ingested")? != 0,
        event_store_inserted: row.try_get::<i64, _>("event_store_inserted")? != 0,
        event_store_ingested_at_ms: row.try_get("event_store_ingested_at_ms")?,
        created_at_ms: row.try_get("created_at_ms")?,
        updated_at_ms: row.try_get("updated_at_ms")?,
    })
}

fn relay_status_from_row(
    row: sqlx::sqlite::SqliteRow,
) -> Result<RadrootsOutboxRelayStatusRecord, RadrootsOutboxError> {
    let status = RadrootsOutboxRelayStatus::parse(row.try_get::<String, _>("status")?.as_str())?;
    Ok(RadrootsOutboxRelayStatusRecord {
        outbox_event_id: row.try_get("outbox_event_id")?,
        relay_url: row.try_get("relay_url")?,
        status,
        attempt_count: row.try_get("attempt_count")?,
        last_attempt_at_ms: row.try_get("last_attempt_at_ms")?,
        acknowledged_at_ms: row.try_get("acknowledged_at_ms")?,
        last_error: row.try_get("last_error")?,
    })
}

fn event_from_signed(signed_event: &RadrootsSignedNostrEvent) -> RadrootsNostrEvent {
    RadrootsNostrEvent {
        id: signed_event.id.clone(),
        author: signed_event.pubkey.clone(),
        created_at: signed_event.created_at,
        kind: signed_event.kind,
        tags: signed_event.tags.clone(),
        content: signed_event.content.clone(),
        sig: signed_event.sig.clone(),
    }
}

fn canonical_relays(relays: Vec<String>) -> Vec<String> {
    let mut out = Vec::new();
    for relay in relays {
        if !out.iter().any(|existing| existing == &relay) {
            out.push(relay);
        }
    }
    out
}

#[derive(Serialize)]
struct DigestInput<'a> {
    operation_kind: &'a str,
    expected_pubkey: &'a str,
    draft: &'a RadrootsFrozenEventDraft,
    target_relays: &'a [String],
}

fn idempotency_digest(
    operation_kind: &str,
    expected_pubkey: &str,
    draft: &RadrootsFrozenEventDraft,
    target_relays: &[String],
) -> Result<String, RadrootsOutboxError> {
    let input = DigestInput {
        operation_kind,
        expected_pubkey,
        draft,
        target_relays,
    };
    let bytes = serde_json::to_vec(&input)?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

fn bool_i64(value: bool) -> i64 {
    if value { 1 } else { 0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use radroots_events::kinds::KIND_POST;
    use radroots_nostr::prelude::RadrootsNostrSecretKey;

    const FIXTURE_ALICE_SECRET_KEY_HEX: &str =
        "10c5304d6c9ae3a1a16f7860f1cc8f5e3a76225a2663b3a989a0d775919b7df5";
    const FIXTURE_ALICE_PUBLIC_KEY_HEX: &str =
        "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df";
    const RELAY_PRIMARY_WSS: &str = "wss://relay.example.com";
    const RELAY_SECONDARY_WSS: &str = "wss://relay-2.example.com";

    fn hex_64(character: char) -> String {
        std::iter::repeat_n(character, 64).collect()
    }

    fn post_draft(expected_pubkey: &str, content: &str) -> RadrootsFrozenEventDraft {
        RadrootsFrozenEventDraft::new(
            "radroots.social.post.v1",
            KIND_POST,
            1_700_000_000,
            vec![vec!["t".to_owned(), "soil".to_owned()]],
            content,
            expected_pubkey,
        )
        .expect("post draft")
    }

    fn operation_input(
        draft: RadrootsFrozenEventDraft,
        created_at_ms: i64,
    ) -> RadrootsOutboxOperationInput {
        RadrootsOutboxOperationInput::new(
            "publish_post",
            draft,
            vec![
                RELAY_PRIMARY_WSS.to_owned(),
                RELAY_SECONDARY_WSS.to_owned(),
                RELAY_PRIMARY_WSS.to_owned(),
            ],
            created_at_ms,
        )
    }

    fn fixture_keys() -> RadrootsNostrKeys {
        let secret_key =
            RadrootsNostrSecretKey::from_hex(FIXTURE_ALICE_SECRET_KEY_HEX).expect("secret key");
        RadrootsNostrKeys::new(secret_key)
    }

    async fn enqueue_signed_fixture(
        outbox: &RadrootsOutbox,
    ) -> (RadrootsOutboxEnqueueReceipt, RadrootsOutboxClaimedEvent) {
        let draft = post_draft(FIXTURE_ALICE_PUBLIC_KEY_HEX, "hello");
        let receipt = outbox
            .enqueue_operation(operation_input(draft, 1_000))
            .await
            .expect("enqueue");
        let claimed = outbox
            .claim_next_ready_event("worker-a", "claim-a", 2_000, 1_000)
            .await
            .expect("claim")
            .expect("claimed event");
        (receipt, claimed)
    }

    #[tokio::test]
    async fn migration_applies_pragmas_and_migrates_down() {
        let outbox = RadrootsOutbox::open_memory().await.expect("open");

        assert_eq!(outbox.pragma_foreign_keys().await.expect("foreign keys"), 1);
        assert_eq!(
            outbox.pragma_busy_timeout().await.expect("busy timeout"),
            5_000
        );

        let row = sqlx::query(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'outbox_event'",
        )
        .fetch_optional(outbox.pool())
        .await
        .expect("table query");
        assert!(row.is_some());

        outbox.migrate_down().await.expect("migrate down");
        let row = sqlx::query(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'outbox_event'",
        )
        .fetch_optional(outbox.pool())
        .await
        .expect("table query");
        assert!(row.is_none());
    }

    #[tokio::test]
    async fn enqueue_idempotency_is_scoped_by_kind_pubkey_and_digest() {
        let outbox = RadrootsOutbox::open_memory().await.expect("open");
        let first_draft = post_draft(hex_64('a').as_str(), "hello");

        let first = outbox
            .enqueue_operation(operation_input(first_draft.clone(), 1_000))
            .await
            .expect("first enqueue");
        let second = outbox
            .enqueue_operation(operation_input(first_draft.clone(), 1_001))
            .await
            .expect("second enqueue");

        assert_eq!(first.status, RadrootsOutboxEnqueueStatus::Inserted);
        assert_eq!(second.status, RadrootsOutboxEnqueueStatus::Inserted);
        assert_ne!(first.operation_id, second.operation_id);
        assert_ne!(first.outbox_event_id, second.outbox_event_id);

        let keyed_first = outbox
            .enqueue_operation(
                operation_input(first_draft.clone(), 1_002).with_idempotency_key("idem-a"),
            )
            .await
            .expect("keyed first");
        let keyed_second = outbox
            .enqueue_operation(
                operation_input(first_draft.clone(), 1_003).with_idempotency_key("idem-a"),
            )
            .await
            .expect("keyed second");

        assert_eq!(keyed_first.status, RadrootsOutboxEnqueueStatus::Inserted);
        assert_eq!(keyed_second.status, RadrootsOutboxEnqueueStatus::Existing);
        assert_eq!(keyed_first.operation_id, keyed_second.operation_id);
        assert_eq!(keyed_first.outbox_event_id, keyed_second.outbox_event_id);
        assert_eq!(
            keyed_first.idempotency_digest,
            keyed_second.idempotency_digest
        );

        let conflict = outbox
            .enqueue_operation(
                operation_input(post_draft(hex_64('a').as_str(), "changed"), 1_004)
                    .with_idempotency_key("idem-a"),
            )
            .await
            .expect_err("conflict");
        assert!(matches!(
            conflict,
            RadrootsOutboxError::IdempotencyConflict { .. }
        ));

        let other_kind = outbox
            .enqueue_operation(
                RadrootsOutboxOperationInput::new(
                    "publish_post_reply",
                    first_draft.clone(),
                    vec![RELAY_PRIMARY_WSS.to_owned()],
                    1_005,
                )
                .with_idempotency_key("idem-a"),
            )
            .await
            .expect("other kind");
        assert_eq!(other_kind.status, RadrootsOutboxEnqueueStatus::Inserted);

        let other_pubkey = outbox
            .enqueue_operation(
                operation_input(post_draft(hex_64('b').as_str(), "hello"), 1_006)
                    .with_idempotency_key("idem-a"),
            )
            .await
            .expect("other pubkey");
        assert_eq!(other_pubkey.status, RadrootsOutboxEnqueueStatus::Inserted);
    }

    #[tokio::test]
    async fn claim_token_guards_updates_and_expired_signing_claim_recovers() {
        let outbox = RadrootsOutbox::open_memory().await.expect("open");
        let draft = post_draft(hex_64('a').as_str(), "hello");
        let receipt = outbox
            .enqueue_operation(operation_input(draft, 1_000))
            .await
            .expect("enqueue");

        let claimed = outbox
            .claim_next_ready_event("worker-a", "claim-a", 1_100, 1_000)
            .await
            .expect("claim")
            .expect("claimed event");
        assert_eq!(claimed.state, RadrootsOutboxEventState::Signing);
        assert_eq!(
            claimed.target_relays,
            vec![RELAY_SECONDARY_WSS.to_owned(), RELAY_PRIMARY_WSS.to_owned()]
        );

        let unavailable = outbox
            .claim_next_ready_event("worker-b", "claim-b", 1_100, 1_050)
            .await
            .expect("claim");
        assert!(unavailable.is_none());

        let wrong_token = outbox
            .mark_sign_retryable(
                receipt.outbox_event_id,
                "claim-b",
                "sign failed",
                1_200,
                1_100,
            )
            .await
            .expect_err("wrong token");
        assert!(matches!(
            wrong_token,
            RadrootsOutboxError::ClaimTokenMismatch { .. }
        ));

        let recovered = outbox.recover_expired_claims(1_101).await.expect("recover");
        assert_eq!(recovered, 1);

        let event = outbox
            .get_event(receipt.outbox_event_id)
            .await
            .expect("event")
            .expect("event");
        assert_eq!(event.state, RadrootsOutboxEventState::SignRetryable);
        assert_eq!(event.attempt_count, 1);
        assert!(event.claim_token.is_none());

        let reclaimed = outbox
            .claim_next_ready_event("worker-b", "claim-b", 1_400, 1_200)
            .await
            .expect("claim")
            .expect("reclaimed");
        assert_eq!(reclaimed.state, RadrootsOutboxEventState::Signing);
    }

    #[tokio::test]
    async fn signed_events_are_reused_after_claim_recovery() {
        let outbox = RadrootsOutbox::open_memory().await.expect("open");
        let (receipt, claimed) = enqueue_signed_fixture(&outbox).await;
        let keys = fixture_keys();

        let signed = outbox
            .sign_claimed_event(&claimed, &keys, 1_100)
            .await
            .expect("sign");
        assert_eq!(signed.id, receipt.expected_event_id);

        let recovered = outbox.recover_expired_claims(2_001).await.expect("recover");
        assert_eq!(recovered, 1);

        let publish_claim = outbox
            .claim_next_ready_event("publisher-a", "claim-b", 3_000, 2_100)
            .await
            .expect("claim")
            .expect("publish claim");
        assert_eq!(publish_claim.state, RadrootsOutboxEventState::Publishing);
        assert_eq!(publish_claim.signed_event.as_ref(), Some(&signed));

        let reused = outbox
            .sign_claimed_event(&publish_claim, &keys, 2_200)
            .await
            .expect("reuse signed event");
        assert_eq!(reused, signed);

        let event = outbox
            .get_event(receipt.outbox_event_id)
            .await
            .expect("event")
            .expect("event");
        assert_eq!(event.state, RadrootsOutboxEventState::Publishing);
        assert_eq!(event.signed_event.as_ref(), Some(&signed));
    }

    #[tokio::test]
    async fn local_signed_event_ingest_is_idempotent_without_relay_observation() {
        let outbox = RadrootsOutbox::open_memory().await.expect("open");
        let event_store = RadrootsEventStore::open_memory()
            .await
            .expect("event store");
        let (receipt, claimed) = enqueue_signed_fixture(&outbox).await;
        let keys = fixture_keys();
        let signed = outbox
            .sign_claimed_event(&claimed, &keys, 1_100)
            .await
            .expect("sign");

        let first = outbox
            .ingest_signed_event_local(&event_store, receipt.outbox_event_id, "claim-a", 1_200)
            .await
            .expect("first ingest");
        assert_eq!(first.outbox_event_id, receipt.outbox_event_id);
        assert_eq!(first.event_id, signed.id);
        assert!(!first.already_ingested);
        assert!(first.event_store_inserted);

        let stored = event_store
            .get_event(signed.id.as_str())
            .await
            .expect("stored event");
        assert!(stored.is_some());

        let observations = event_store
            .observations_for_event(signed.id.as_str())
            .await
            .expect("observations");
        assert!(observations.is_empty());

        let second = outbox
            .ingest_signed_event_local(&event_store, receipt.outbox_event_id, "claim-a", 1_300)
            .await
            .expect("second ingest");
        assert!(second.already_ingested);
        assert!(!second.event_store_inserted);

        let event = outbox
            .get_event(receipt.outbox_event_id)
            .await
            .expect("event")
            .expect("event");
        assert_eq!(event.state, RadrootsOutboxEventState::Publishing);
        assert!(event.event_store_ingested);
        assert!(event.event_store_inserted);
        assert_eq!(event.event_store_ingested_at_ms, Some(1_200));

        let recovered = outbox.recover_expired_claims(2_001).await.expect("recover");
        assert_eq!(recovered, 1);

        let event = outbox
            .get_event(receipt.outbox_event_id)
            .await
            .expect("event")
            .expect("event");
        assert_eq!(event.state, RadrootsOutboxEventState::PublishRetryable);
        assert!(event.claim_token.is_none());

        let reclaimed = outbox
            .claim_next_ready_event("publisher-a", "claim-b", 3_000, 2_100)
            .await
            .expect("claim")
            .expect("publish claim");
        assert_eq!(reclaimed.state, RadrootsOutboxEventState::Publishing);
        assert_eq!(reclaimed.signed_event.as_ref(), Some(&signed));
    }
}
