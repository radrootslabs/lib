#![forbid(unsafe_code)]

use crate::RadrootsOutboxError;
use crate::migrations::{OUTBOX_MIGRATION_DOWN, OUTBOX_MIGRATION_UP};
use crate::model::{
    RadrootsOutboxClaimedEvent, RadrootsOutboxDeliveryAttemptRecord,
    RadrootsOutboxDeliveryPlanInput, RadrootsOutboxDeliveryPlanRecord,
    RadrootsOutboxDeliveryPlanStatus, RadrootsOutboxDeliveryTargetRecord,
    RadrootsOutboxDeliveryTargetStatus, RadrootsOutboxEnqueueReceipt, RadrootsOutboxEnqueueStatus,
    RadrootsOutboxEventRecord, RadrootsOutboxEventState, RadrootsOutboxEventStoreIngestReceipt,
    RadrootsOutboxIdempotencyPreflight, RadrootsOutboxOperationInput,
    RadrootsOutboxOperationRecord, RadrootsOutboxOperationStatus,
    RadrootsOutboxReticulumPreviewBehavior, RadrootsOutboxSignedOperationInput,
    RadrootsOutboxStatusSummary,
};
use radroots_event_store::{RadrootsEventIngest, RadrootsEventStore};
use radroots_events::RadrootsNostrEvent;
use radroots_events::draft::{
    RadrootsFrozenEventDraft, RadrootsSignedNostrEvent, validate_signed_nostr_event_matches_draft,
};
use radroots_transport::{
    RadrootsTransportKind, RadrootsTransportSatisfactionClass, RadrootsTransportSatisfactionPolicy,
    RadrootsTransportTarget, RadrootsTransportTargetFingerprint, RadrootsTransportTargetUri,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions, SqliteQueryResult};
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

    pub async fn status_summary(
        &self,
        now_ms: i64,
    ) -> Result<RadrootsOutboxStatusSummary, RadrootsOutboxError> {
        let row = sqlx::query(
            "SELECT COUNT(*) AS total_events, COALESCE(SUM(CASE WHEN state IN ('draft_queued', 'signing', 'signed', 'publishing') THEN 1 ELSE 0 END), 0) AS pending_events, COALESCE(SUM(CASE WHEN state IN ('sign_retryable', 'publish_retryable') THEN 1 ELSE 0 END), 0) AS retryable_events, COALESCE(SUM(CASE WHEN state IN ('published', 'failed_terminal', 'cancelled') THEN 1 ELSE 0 END), 0) AS terminal_events, COALESCE(SUM(CASE WHEN state = 'failed_terminal' THEN 1 ELSE 0 END), 0) AS failed_terminal_events, COALESCE(SUM(CASE WHEN state = 'publishing' THEN 1 ELSE 0 END), 0) AS publishing_events FROM outbox_event",
        )
        .fetch_one(&self.pool)
        .await?;
        let ready_signed_events = sqlx::query(
            "SELECT COUNT(*) FROM outbox_event AS event WHERE event.state IN ('signed', 'publish_retryable') AND event.signed_event_json IS NOT NULL AND event.next_attempt_after_ms <= ? AND (event.claim_token IS NULL OR event.claim_expires_at_ms <= ?) AND EXISTS (SELECT 1 FROM outbox_delivery_plan AS plan JOIN outbox_delivery_target AS target ON target.delivery_plan_id = plan.delivery_plan_id WHERE plan.outbox_event_id = event.outbox_event_id AND target.status IN ('pending', 'failed_retryable'))",
        )
        .bind(now_ms)
        .bind(now_ms)
        .fetch_one(&self.pool)
        .await?
        .try_get(0)?;
        let last_attempt_at_ms =
            sqlx::query("SELECT MAX(attempted_at_ms) FROM outbox_delivery_attempt")
                .fetch_one(&self.pool)
                .await?
                .try_get(0)?;
        let last_error = sqlx::query(
            "SELECT last_error FROM outbox_event WHERE last_error IS NOT NULL ORDER BY updated_at_ms DESC, outbox_event_id DESC LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await?
        .map(|row| row.try_get("last_error"))
        .transpose()?;
        Ok(RadrootsOutboxStatusSummary {
            total_events: row.try_get("total_events")?,
            pending_events: row.try_get("pending_events")?,
            retryable_events: row.try_get("retryable_events")?,
            terminal_events: row.try_get("terminal_events")?,
            failed_terminal_events: row.try_get("failed_terminal_events")?,
            ready_signed_events,
            publishing_events: row.try_get("publishing_events")?,
            last_attempt_at_ms,
            last_error,
        })
    }

    pub async fn preflight_signed_operation_idempotency(
        &self,
        input: &RadrootsOutboxSignedOperationInput,
    ) -> Result<RadrootsOutboxIdempotencyPreflight, RadrootsOutboxError> {
        validate_signed_nostr_event_matches_draft(&input.signed_event, &input.draft)?;
        let prepared = prepare_delivery_plan(&input.draft.expected_event_id, &input.delivery_plan)?;
        let operation_digest = operation_idempotency_digest(
            input.operation_kind.as_str(),
            input.draft.expected_pubkey.as_str(),
            &input.draft,
        );

        if let Some(idempotency_key) = input.idempotency_key.as_deref()
            && let Some(existing) = existing_idempotent_operation_for_pool(
                &self.pool,
                input.operation_kind.as_str(),
                input.draft.expected_pubkey.as_str(),
                idempotency_key,
            )
            .await?
            && existing.operation_idempotency_digest != operation_digest
        {
            return Err(RadrootsOutboxError::IdempotencyConflict {
                operation_kind: input.operation_kind.clone(),
                expected_pubkey: input.draft.expected_pubkey.clone(),
                idempotency_key: idempotency_key.to_owned(),
                existing_digest: existing.operation_idempotency_digest,
                new_digest: operation_digest,
            });
        }

        Ok(RadrootsOutboxIdempotencyPreflight {
            operation_idempotency_digest: operation_digest,
            delivery_plan_idempotency_digest: prepared.delivery_plan_idempotency_digest,
        })
    }

    pub async fn enqueue_operation(
        &self,
        input: RadrootsOutboxOperationInput,
    ) -> Result<RadrootsOutboxEnqueueReceipt, RadrootsOutboxError> {
        let prepared = prepare_delivery_plan(&input.draft.expected_event_id, &input.delivery_plan)?;
        let operation_digest = operation_idempotency_digest(
            input.operation_kind.as_str(),
            input.draft.expected_pubkey.as_str(),
            &input.draft,
        );
        let mut tx = self.pool.begin().await?;

        if let Some(idempotency_key) = input.idempotency_key.as_deref()
            && let Some(existing) = existing_idempotent_operation(
                &mut tx,
                input.operation_kind.as_str(),
                input.draft.expected_pubkey.as_str(),
                idempotency_key,
            )
            .await?
        {
            if existing.operation_idempotency_digest != operation_digest {
                return Err(RadrootsOutboxError::IdempotencyConflict {
                    operation_kind: input.operation_kind,
                    expected_pubkey: input.draft.expected_pubkey,
                    idempotency_key: idempotency_key.to_owned(),
                    existing_digest: existing.operation_idempotency_digest,
                    new_digest: operation_digest,
                });
            }
            let plan = insert_or_get_delivery_plan(
                &mut tx,
                existing.outbox_event_id,
                &prepared,
                input.created_at_ms,
            )
            .await?;
            if plan.status == RadrootsOutboxEnqueueStatus::Inserted {
                reactivate_event_for_new_plan(
                    &mut tx,
                    existing.outbox_event_id,
                    input.created_at_ms,
                )
                .await?;
            }
            tx.commit().await?;
            return Ok(RadrootsOutboxEnqueueReceipt {
                status: plan.status,
                operation_id: existing.operation_id,
                outbox_event_id: existing.outbox_event_id,
                delivery_plan_id: plan.delivery_plan_id,
                expected_event_id: existing.event_id,
                operation_idempotency_digest: operation_digest,
                delivery_plan_idempotency_digest: prepared.delivery_plan_idempotency_digest,
            });
        }

        let operation = sqlx::query(
            "INSERT INTO outbox_operations(operation_kind, expected_pubkey, idempotency_key, operation_idempotency_digest, status, created_at_ms, updated_at_ms) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(input.operation_kind.as_str())
        .bind(input.draft.expected_pubkey.as_str())
        .bind(input.idempotency_key.as_deref())
        .bind(operation_digest.as_str())
        .bind(RadrootsOutboxOperationStatus::Queued.as_str())
        .bind(input.created_at_ms)
        .bind(input.created_at_ms)
        .execute(&mut *tx)
        .await?;
        let operation_id = operation.last_insert_rowid();
        let draft_json = serde_json::to_string(&input.draft)?;
        let event = sqlx::query(
            "INSERT INTO outbox_event(operation_id, event_id, expected_pubkey, draft_json, state, attempt_count, next_attempt_after_ms, event_store_ingested, event_store_inserted, created_at_ms, updated_at_ms) VALUES (?, ?, ?, ?, ?, 0, ?, 0, 0, ?, ?)",
        )
        .bind(operation_id)
        .bind(input.draft.expected_event_id.as_str())
        .bind(input.draft.expected_pubkey.as_str())
        .bind(draft_json.as_str())
        .bind(RadrootsOutboxEventState::DraftQueued.as_str())
        .bind(input.created_at_ms)
        .bind(input.created_at_ms)
        .bind(input.created_at_ms)
        .execute(&mut *tx)
        .await?;
        let outbox_event_id = event.last_insert_rowid();
        let plan =
            insert_or_get_delivery_plan(&mut tx, outbox_event_id, &prepared, input.created_at_ms)
                .await?;
        tx.commit().await?;
        Ok(RadrootsOutboxEnqueueReceipt {
            status: RadrootsOutboxEnqueueStatus::Inserted,
            operation_id,
            outbox_event_id,
            delivery_plan_id: plan.delivery_plan_id,
            expected_event_id: input.draft.expected_event_id,
            operation_idempotency_digest: operation_digest,
            delivery_plan_idempotency_digest: prepared.delivery_plan_idempotency_digest,
        })
    }

    pub async fn enqueue_signed_operation(
        &self,
        input: RadrootsOutboxSignedOperationInput,
    ) -> Result<RadrootsOutboxEnqueueReceipt, RadrootsOutboxError> {
        validate_signed_nostr_event_matches_draft(&input.signed_event, &input.draft)?;
        let prepared = prepare_delivery_plan(&input.draft.expected_event_id, &input.delivery_plan)?;
        let operation_digest = operation_idempotency_digest(
            input.operation_kind.as_str(),
            input.draft.expected_pubkey.as_str(),
            &input.draft,
        );
        let mut tx = self.pool.begin().await?;

        if let Some(idempotency_key) = input.idempotency_key.as_deref()
            && let Some(existing) = existing_idempotent_operation(
                &mut tx,
                input.operation_kind.as_str(),
                input.draft.expected_pubkey.as_str(),
                idempotency_key,
            )
            .await?
        {
            if existing.operation_idempotency_digest != operation_digest {
                return Err(RadrootsOutboxError::IdempotencyConflict {
                    operation_kind: input.operation_kind,
                    expected_pubkey: input.draft.expected_pubkey,
                    idempotency_key: idempotency_key.to_owned(),
                    existing_digest: existing.operation_idempotency_digest,
                    new_digest: operation_digest,
                });
            }
            ensure_event_signed(
                &mut tx,
                existing.outbox_event_id,
                &input.signed_event,
                input.event_store_inserted,
                input.event_store_ingested_at_ms,
            )
            .await?;
            let plan = insert_or_get_delivery_plan(
                &mut tx,
                existing.outbox_event_id,
                &prepared,
                input.created_at_ms,
            )
            .await?;
            if plan.status == RadrootsOutboxEnqueueStatus::Inserted {
                reactivate_event_for_new_plan(
                    &mut tx,
                    existing.outbox_event_id,
                    input.created_at_ms,
                )
                .await?;
            }
            tx.commit().await?;
            return Ok(RadrootsOutboxEnqueueReceipt {
                status: plan.status,
                operation_id: existing.operation_id,
                outbox_event_id: existing.outbox_event_id,
                delivery_plan_id: plan.delivery_plan_id,
                expected_event_id: existing.event_id,
                operation_idempotency_digest: operation_digest,
                delivery_plan_idempotency_digest: prepared.delivery_plan_idempotency_digest,
            });
        }

        let operation = sqlx::query(
            "INSERT INTO outbox_operations(operation_kind, expected_pubkey, idempotency_key, operation_idempotency_digest, status, created_at_ms, updated_at_ms) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(input.operation_kind.as_str())
        .bind(input.draft.expected_pubkey.as_str())
        .bind(input.idempotency_key.as_deref())
        .bind(operation_digest.as_str())
        .bind(RadrootsOutboxOperationStatus::Queued.as_str())
        .bind(input.created_at_ms)
        .bind(input.created_at_ms)
        .execute(&mut *tx)
        .await?;
        let operation_id = operation.last_insert_rowid();
        let draft_json = serde_json::to_string(&input.draft)?;
        let signed_event_json = serde_json::to_string(&input.signed_event)?;
        let event = sqlx::query(
            "INSERT INTO outbox_event(operation_id, event_id, expected_pubkey, draft_json, signed_event_json, raw_event_json, state, attempt_count, next_attempt_after_ms, event_store_ingested, event_store_inserted, event_store_ingested_at_ms, created_at_ms, updated_at_ms) VALUES (?, ?, ?, ?, ?, ?, ?, 0, ?, 1, ?, ?, ?, ?)",
        )
        .bind(operation_id)
        .bind(input.draft.expected_event_id.as_str())
        .bind(input.draft.expected_pubkey.as_str())
        .bind(draft_json.as_str())
        .bind(signed_event_json.as_str())
        .bind(input.signed_event.raw_json.as_str())
        .bind(RadrootsOutboxEventState::Signed.as_str())
        .bind(input.created_at_ms)
        .bind(bool_i64(input.event_store_inserted))
        .bind(input.event_store_ingested_at_ms)
        .bind(input.created_at_ms)
        .bind(input.created_at_ms)
        .execute(&mut *tx)
        .await?;
        let outbox_event_id = event.last_insert_rowid();
        let plan =
            insert_or_get_delivery_plan(&mut tx, outbox_event_id, &prepared, input.created_at_ms)
                .await?;
        tx.commit().await?;
        Ok(RadrootsOutboxEnqueueReceipt {
            status: RadrootsOutboxEnqueueStatus::Inserted,
            operation_id,
            outbox_event_id,
            delivery_plan_id: plan.delivery_plan_id,
            expected_event_id: input.draft.expected_event_id,
            operation_idempotency_digest: operation_digest,
            delivery_plan_idempotency_digest: prepared.delivery_plan_idempotency_digest,
        })
    }

    pub async fn get_operation(
        &self,
        operation_id: i64,
    ) -> Result<Option<RadrootsOutboxOperationRecord>, RadrootsOutboxError> {
        let row = sqlx::query(
            "SELECT operation_id, operation_kind, expected_pubkey, idempotency_key, operation_idempotency_digest, status, created_at_ms, updated_at_ms FROM outbox_operations WHERE operation_id = ?",
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
            "SELECT outbox_event_id, operation_id, event_id, expected_pubkey, draft_json, signed_event_json, raw_event_json, state, attempt_count, claim_token, claim_owner, claim_expires_at_ms, next_attempt_after_ms, last_error, event_store_ingested, event_store_inserted, event_store_ingested_at_ms, created_at_ms, updated_at_ms FROM outbox_event WHERE outbox_event_id = ?",
        )
        .bind(outbox_event_id)
        .fetch_optional(&self.pool)
        .await?;
        row.map(event_from_row).transpose()
    }

    pub async fn delivery_plans(
        &self,
        outbox_event_id: i64,
    ) -> Result<Vec<RadrootsOutboxDeliveryPlanRecord>, RadrootsOutboxError> {
        delivery_plans_for_pool(&self.pool, outbox_event_id).await
    }

    pub async fn delivery_targets(
        &self,
        outbox_event_id: i64,
    ) -> Result<Vec<RadrootsOutboxDeliveryTargetRecord>, RadrootsOutboxError> {
        delivery_targets_for_event_pool(&self.pool, outbox_event_id).await
    }

    pub async fn delivery_attempts(
        &self,
        delivery_target_id: i64,
    ) -> Result<Vec<RadrootsOutboxDeliveryAttemptRecord>, RadrootsOutboxError> {
        delivery_attempts_for_pool(&self.pool, delivery_target_id).await
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
            "SELECT outbox_event_id, state, signed_event_json FROM outbox_event AS event WHERE ((event.state IN ('draft_queued', 'sign_retryable')) OR (event.state IN ('signed', 'publish_retryable') AND event.signed_event_json IS NOT NULL AND EXISTS (SELECT 1 FROM outbox_delivery_plan AS plan JOIN outbox_delivery_target AS target ON target.delivery_plan_id = plan.delivery_plan_id WHERE plan.outbox_event_id = event.outbox_event_id AND target.status IN ('pending', 'failed_retryable')))) AND event.next_attempt_after_ms <= ? AND (event.claim_token IS NULL OR event.claim_expires_at_ms <= ?) ORDER BY event.created_at_ms, event.outbox_event_id LIMIT 1",
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
        let changed = claim_event(
            &mut tx,
            outbox_event_id,
            claimed_state,
            claim_owner.as_ref(),
            claim_token.as_ref(),
            claim_expires_at_ms,
            now_ms,
            "AND (claim_token IS NULL OR claim_expires_at_ms <= ?)",
        )
        .await?;
        if changed.rows_affected() == 0 {
            tx.commit().await?;
            return Ok(None);
        }
        let claimed = claimed_event_from_tx(
            &mut tx,
            outbox_event_id,
            claimed_state,
            claim_token.as_ref(),
        )
        .await?;
        tx.commit().await?;
        Ok(Some(claimed))
    }

    pub async fn claim_next_ready_signed_event(
        &self,
        claim_owner: impl AsRef<str>,
        claim_token: impl AsRef<str>,
        claim_expires_at_ms: i64,
        now_ms: i64,
    ) -> Result<Option<RadrootsOutboxClaimedEvent>, RadrootsOutboxError> {
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query(
            "SELECT outbox_event_id FROM outbox_event AS event WHERE event.state IN ('signed', 'publish_retryable') AND event.signed_event_json IS NOT NULL AND event.next_attempt_after_ms <= ? AND (event.claim_token IS NULL OR event.claim_expires_at_ms <= ?) AND EXISTS (SELECT 1 FROM outbox_delivery_plan AS plan JOIN outbox_delivery_target AS target ON target.delivery_plan_id = plan.delivery_plan_id WHERE plan.outbox_event_id = event.outbox_event_id AND target.status IN ('pending', 'failed_retryable')) ORDER BY event.created_at_ms, event.outbox_event_id LIMIT 1",
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
        let changed = claim_event(
            &mut tx,
            outbox_event_id,
            RadrootsOutboxEventState::Publishing,
            claim_owner.as_ref(),
            claim_token.as_ref(),
            claim_expires_at_ms,
            now_ms,
            "AND state IN ('signed', 'publish_retryable') AND signed_event_json IS NOT NULL AND (claim_token IS NULL OR claim_expires_at_ms <= ?)",
        )
        .await?;
        if changed.rows_affected() == 0 {
            tx.commit().await?;
            return Ok(None);
        }
        let claimed = claimed_event_from_tx(
            &mut tx,
            outbox_event_id,
            RadrootsOutboxEventState::Publishing,
            claim_token.as_ref(),
        )
        .await?;
        tx.commit().await?;
        Ok(Some(claimed))
    }

    pub async fn claim_ready_signed_event(
        &self,
        outbox_event_id: i64,
        claim_owner: impl AsRef<str>,
        claim_token: impl AsRef<str>,
        claim_expires_at_ms: i64,
        now_ms: i64,
    ) -> Result<Option<RadrootsOutboxClaimedEvent>, RadrootsOutboxError> {
        let mut tx = self.pool.begin().await?;
        let changed = sqlx::query(
            "UPDATE outbox_event SET state = ?, claim_token = ?, claim_owner = ?, claim_expires_at_ms = ?, attempt_count = attempt_count + 1, updated_at_ms = ? WHERE outbox_event_id = ? AND state IN ('signed', 'publish_retryable') AND signed_event_json IS NOT NULL AND next_attempt_after_ms <= ? AND (claim_token IS NULL OR claim_expires_at_ms <= ?) AND EXISTS (SELECT 1 FROM outbox_delivery_plan AS plan JOIN outbox_delivery_target AS target ON target.delivery_plan_id = plan.delivery_plan_id WHERE plan.outbox_event_id = outbox_event.outbox_event_id AND target.status IN ('pending', 'failed_retryable'))",
        )
        .bind(RadrootsOutboxEventState::Publishing.as_str())
        .bind(claim_token.as_ref())
        .bind(claim_owner.as_ref())
        .bind(claim_expires_at_ms)
        .bind(now_ms)
        .bind(outbox_event_id)
        .bind(now_ms)
        .bind(now_ms)
        .execute(&mut *tx)
        .await?;
        if changed.rows_affected() == 0 {
            tx.commit().await?;
            return Ok(None);
        }
        let claimed = claimed_event_from_tx(
            &mut tx,
            outbox_event_id,
            RadrootsOutboxEventState::Publishing,
            claim_token.as_ref(),
        )
        .await?;
        tx.commit().await?;
        Ok(Some(claimed))
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
        let changed = sqlx::query(
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
        self.ensure_claimed_update(outbox_event_id, claim_token, changed)
            .await?;
        Ok(signed_event)
    }

    pub async fn mark_sign_retryable(
        &self,
        outbox_event_id: i64,
        claim_token: &str,
        error: impl AsRef<str>,
        next_attempt_after_ms: i64,
        now_ms: i64,
    ) -> Result<(), RadrootsOutboxError> {
        let changed = sqlx::query(
            "UPDATE outbox_event SET state = ?, claim_token = NULL, claim_owner = NULL, claim_expires_at_ms = NULL, last_error = ?, next_attempt_after_ms = ?, updated_at_ms = ? WHERE outbox_event_id = ? AND claim_token = ?",
        )
        .bind(RadrootsOutboxEventState::SignRetryable.as_str())
        .bind(error.as_ref())
        .bind(next_attempt_after_ms)
        .bind(now_ms)
        .bind(outbox_event_id)
        .bind(claim_token)
        .execute(&self.pool)
        .await?;
        self.ensure_claimed_update(outbox_event_id, claim_token, changed)
            .await
    }

    pub async fn mark_publish_retryable(
        &self,
        outbox_event_id: i64,
        claim_token: &str,
        error: impl AsRef<str>,
        next_attempt_after_ms: i64,
        now_ms: i64,
    ) -> Result<(), RadrootsOutboxError> {
        let changed = sqlx::query(
            "UPDATE outbox_event SET state = ?, claim_token = NULL, claim_owner = NULL, claim_expires_at_ms = NULL, last_error = ?, next_attempt_after_ms = ?, updated_at_ms = ? WHERE outbox_event_id = ? AND claim_token = ?",
        )
        .bind(RadrootsOutboxEventState::PublishRetryable.as_str())
        .bind(error.as_ref())
        .bind(next_attempt_after_ms)
        .bind(now_ms)
        .bind(outbox_event_id)
        .bind(claim_token)
        .execute(&self.pool)
        .await?;
        self.ensure_claimed_update(outbox_event_id, claim_token, changed)
            .await
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
        let ingest = RadrootsEventIngest::new(event, observed_at_ms)
            .with_raw_json(signed_event.raw_json.clone());
        let receipt = event_store.ingest_event(ingest).await?;
        let changed = sqlx::query(
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
        self.ensure_claimed_update(outbox_event_id, claim_token, changed)
            .await?;
        Ok(RadrootsOutboxEventStoreIngestReceipt {
            outbox_event_id,
            event_id: receipt.event_id,
            already_ingested: false,
            event_store_inserted: receipt.inserted,
        })
    }

    pub async fn mark_delivery_target_accepted(
        &self,
        outbox_event_id: i64,
        claim_token: &str,
        delivery_target_id: i64,
        attempted_at_ms: i64,
    ) -> Result<(), RadrootsOutboxError> {
        self.mark_delivery_target_status(
            outbox_event_id,
            claim_token,
            delivery_target_id,
            RadrootsOutboxDeliveryTargetStatus::Accepted,
            None,
            attempted_at_ms,
        )
        .await
    }

    pub async fn mark_delivery_target_failed_retryable(
        &self,
        outbox_event_id: i64,
        claim_token: &str,
        delivery_target_id: i64,
        error: &str,
        attempted_at_ms: i64,
    ) -> Result<(), RadrootsOutboxError> {
        self.mark_delivery_target_status(
            outbox_event_id,
            claim_token,
            delivery_target_id,
            RadrootsOutboxDeliveryTargetStatus::FailedRetryable,
            Some(error),
            attempted_at_ms,
        )
        .await
    }

    pub async fn mark_delivery_target_failed_terminal(
        &self,
        outbox_event_id: i64,
        claim_token: &str,
        delivery_target_id: i64,
        error: &str,
        attempted_at_ms: i64,
    ) -> Result<(), RadrootsOutboxError> {
        self.mark_delivery_target_status(
            outbox_event_id,
            claim_token,
            delivery_target_id,
            RadrootsOutboxDeliveryTargetStatus::FailedTerminal,
            Some(error),
            attempted_at_ms,
        )
        .await
    }

    pub async fn mark_delivery_target_skipped_policy_denied(
        &self,
        outbox_event_id: i64,
        claim_token: &str,
        delivery_target_id: i64,
        message: &str,
        attempted_at_ms: i64,
    ) -> Result<(), RadrootsOutboxError> {
        self.mark_delivery_target_status(
            outbox_event_id,
            claim_token,
            delivery_target_id,
            RadrootsOutboxDeliveryTargetStatus::SkippedPolicyDenied,
            Some(message),
            attempted_at_ms,
        )
        .await
    }

    pub async fn complete_publish_attempt(
        &self,
        outbox_event_id: i64,
        claim_token: &str,
        retryable_error: impl AsRef<str>,
        terminal_error: impl AsRef<str>,
        next_attempt_after_ms: i64,
        now_ms: i64,
    ) -> Result<RadrootsOutboxEventState, RadrootsOutboxError> {
        let mut tx = self.pool.begin().await?;
        let row = claimed_event_identity_tx(&mut tx, outbox_event_id, claim_token).await?;
        let evaluation = evaluate_delivery_plans(&mut tx, outbox_event_id, now_ms).await?;
        let (event_state, operation_status, last_error, next_attempt_after_ms) =
            if evaluation.all_complete {
                (
                    RadrootsOutboxEventState::Published,
                    Some(RadrootsOutboxOperationStatus::Complete),
                    None,
                    now_ms,
                )
            } else if evaluation.any_failed_terminal {
                (
                    RadrootsOutboxEventState::FailedTerminal,
                    Some(RadrootsOutboxOperationStatus::FailedTerminal),
                    Some(terminal_error.as_ref()),
                    now_ms,
                )
            } else if evaluation.any_ready {
                (
                    RadrootsOutboxEventState::PublishRetryable,
                    None,
                    Some(retryable_error.as_ref()),
                    next_attempt_after_ms,
                )
            } else {
                (
                    RadrootsOutboxEventState::Signed,
                    None,
                    Some("delivery deferred until implemented"),
                    now_ms,
                )
            };

        let changed = sqlx::query(
            "UPDATE outbox_event SET state = ?, claim_token = NULL, claim_owner = NULL, claim_expires_at_ms = NULL, last_error = ?, next_attempt_after_ms = ?, updated_at_ms = ? WHERE outbox_event_id = ? AND claim_token = ?",
        )
        .bind(event_state.as_str())
        .bind(last_error)
        .bind(next_attempt_after_ms)
        .bind(now_ms)
        .bind(outbox_event_id)
        .bind(claim_token)
        .execute(&mut *tx)
        .await?;
        if changed.rows_affected() == 0 {
            return Err(RadrootsOutboxError::ClaimTokenMismatch { outbox_event_id });
        }

        if let Some(operation_status) = operation_status {
            sqlx::query(
                "UPDATE outbox_operations SET status = ?, updated_at_ms = ? WHERE operation_id = ?",
            )
            .bind(operation_status.as_str())
            .bind(now_ms)
            .bind(row.operation_id)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(event_state)
    }

    pub async fn mark_publish_failed_terminal(
        &self,
        outbox_event_id: i64,
        claim_token: &str,
        error: impl AsRef<str>,
        now_ms: i64,
    ) -> Result<(), RadrootsOutboxError> {
        self.finish_claimed_event(
            outbox_event_id,
            claim_token,
            RadrootsOutboxEventState::FailedTerminal,
            RadrootsOutboxOperationStatus::FailedTerminal,
            Some(error.as_ref()),
            now_ms,
        )
        .await
    }

    pub async fn cancel_claimed_event(
        &self,
        outbox_event_id: i64,
        claim_token: &str,
        now_ms: i64,
    ) -> Result<(), RadrootsOutboxError> {
        self.finish_claimed_event(
            outbox_event_id,
            claim_token,
            RadrootsOutboxEventState::Cancelled,
            RadrootsOutboxOperationStatus::Cancelled,
            None,
            now_ms,
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

    async fn ensure_claimed_update(
        &self,
        outbox_event_id: i64,
        claim_token: &str,
        changed: SqliteQueryResult,
    ) -> Result<(), RadrootsOutboxError> {
        if changed.rows_affected() > 0 {
            return Ok(());
        }
        self.ensure_claim_token(outbox_event_id, claim_token)
            .await?;
        Err(RadrootsOutboxError::ClaimTokenMismatch { outbox_event_id })
    }

    async fn finish_claimed_event(
        &self,
        outbox_event_id: i64,
        claim_token: &str,
        event_state: RadrootsOutboxEventState,
        operation_status: RadrootsOutboxOperationStatus,
        last_error: Option<&str>,
        now_ms: i64,
    ) -> Result<(), RadrootsOutboxError> {
        let mut tx = self.pool.begin().await?;
        let row = claimed_event_identity_tx(&mut tx, outbox_event_id, claim_token).await?;
        let plan_status = match event_state {
            RadrootsOutboxEventState::Cancelled => {
                Some(RadrootsOutboxDeliveryPlanStatus::Cancelled)
            }
            RadrootsOutboxEventState::FailedTerminal => {
                Some(RadrootsOutboxDeliveryPlanStatus::FailedTerminal)
            }
            _ => None,
        };
        if let Some(plan_status) = plan_status {
            sqlx::query(
                "UPDATE outbox_delivery_plan SET status = ?, updated_at_ms = ? WHERE outbox_event_id = ?",
            )
            .bind(plan_status.as_str())
            .bind(now_ms)
            .bind(outbox_event_id)
            .execute(&mut *tx)
            .await?;
        }
        let changed = sqlx::query(
            "UPDATE outbox_event SET state = ?, claim_token = NULL, claim_owner = NULL, claim_expires_at_ms = NULL, last_error = ?, next_attempt_after_ms = ?, updated_at_ms = ? WHERE outbox_event_id = ? AND claim_token = ?",
        )
        .bind(event_state.as_str())
        .bind(last_error)
        .bind(now_ms)
        .bind(now_ms)
        .bind(outbox_event_id)
        .bind(claim_token)
        .execute(&mut *tx)
        .await?;
        if changed.rows_affected() == 0 {
            return Err(RadrootsOutboxError::ClaimTokenMismatch { outbox_event_id });
        }

        sqlx::query(
            "UPDATE outbox_operations SET status = ?, updated_at_ms = ? WHERE operation_id = ?",
        )
        .bind(operation_status.as_str())
        .bind(now_ms)
        .bind(row.operation_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    async fn mark_delivery_target_status(
        &self,
        outbox_event_id: i64,
        claim_token: &str,
        delivery_target_id: i64,
        status: RadrootsOutboxDeliveryTargetStatus,
        message: Option<&str>,
        attempted_at_ms: i64,
    ) -> Result<(), RadrootsOutboxError> {
        let mut tx = self.pool.begin().await?;
        claimed_event_identity_tx(&mut tx, outbox_event_id, claim_token).await?;
        let completed_at_ms = status.is_completed().then_some(attempted_at_ms);
        let changed = sqlx::query(
            "UPDATE outbox_delivery_target SET status = ?, attempt_count = attempt_count + 1, last_attempt_at_ms = ?, completed_at_ms = ?, last_error = ? WHERE delivery_target_id = ? AND delivery_plan_id IN (SELECT delivery_plan_id FROM outbox_delivery_plan WHERE outbox_event_id = ?)",
        )
        .bind(status.as_str())
        .bind(attempted_at_ms)
        .bind(completed_at_ms)
        .bind(message)
        .bind(delivery_target_id)
        .bind(outbox_event_id)
        .execute(&mut *tx)
        .await?;
        if changed.rows_affected() == 0 {
            return Err(RadrootsOutboxError::DeliveryTargetNotFound(
                delivery_target_id,
            ));
        }
        let delivery_plan_id: i64 = sqlx::query(
            "SELECT delivery_plan_id FROM outbox_delivery_target WHERE delivery_target_id = ?",
        )
        .bind(delivery_target_id)
        .fetch_one(&mut *tx)
        .await?
        .try_get("delivery_plan_id")?;
        sqlx::query(
            "INSERT INTO outbox_delivery_attempt(delivery_plan_id, delivery_target_id, status, attempted_at_ms, message) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(delivery_plan_id)
        .bind(delivery_target_id)
        .bind(status.as_str())
        .bind(attempted_at_ms)
        .bind(message)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }
}

struct ExistingOperation {
    operation_id: i64,
    outbox_event_id: i64,
    event_id: String,
    operation_idempotency_digest: String,
}

struct ClaimedEventIdentity {
    operation_id: i64,
}

struct PreparedDeliveryPlan {
    transport_profile_id: String,
    target_policy_fingerprint: String,
    target_policy_version: u32,
    satisfaction_policy: RadrootsTransportSatisfactionPolicy,
    required_success_count: i64,
    delivery_plan_idempotency_digest: String,
    initial_status: RadrootsOutboxDeliveryPlanStatus,
    targets: Vec<PreparedDeliveryTarget>,
}

struct PreparedDeliveryTarget {
    target: RadrootsTransportTarget,
    initial_status: RadrootsOutboxDeliveryTargetStatus,
}

struct PlanInsertResult {
    status: RadrootsOutboxEnqueueStatus,
    delivery_plan_id: i64,
}

struct PlanEvaluation {
    all_complete: bool,
    any_failed_terminal: bool,
    any_ready: bool,
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

#[cfg_attr(coverage_nightly, coverage(off))]
async fn apply_up(pool: &SqlitePool) -> Result<(), RadrootsOutboxError> {
    sqlx::raw_sql(OUTBOX_MIGRATION_UP).execute(pool).await?;
    Ok(())
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn apply_down(pool: &SqlitePool) -> Result<(), RadrootsOutboxError> {
    sqlx::raw_sql(OUTBOX_MIGRATION_DOWN).execute(pool).await?;
    Ok(())
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn query_i64(pool: &SqlitePool, sql: &str) -> Result<i64, RadrootsOutboxError> {
    let row = sqlx::query(sql).fetch_one(pool).await?;
    Ok(row.try_get(0)?)
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn query_string(pool: &SqlitePool, sql: &str) -> Result<String, RadrootsOutboxError> {
    let row = sqlx::query(sql).fetch_one(pool).await?;
    Ok(row.try_get(0)?)
}

fn prepare_delivery_plan(
    event_id: &str,
    input: &RadrootsOutboxDeliveryPlanInput,
) -> Result<PreparedDeliveryPlan, RadrootsOutboxError> {
    if input.transport_profile_id.trim().is_empty() {
        return Err(RadrootsOutboxError::EmptyTransportProfileId);
    }
    let targets = ordered_unique_targets(input.targets.clone());
    if targets.is_empty() {
        return Err(RadrootsOutboxError::EmptyDeliveryTargets);
    }
    let required_success_count = input
        .satisfaction_policy
        .required_target_count(targets.len())? as i64;
    let prepared_targets = targets
        .into_iter()
        .map(|target| {
            let initial_status =
                initial_delivery_target_status(&target, input.reticulum_preview_behavior);
            PreparedDeliveryTarget {
                target,
                initial_status,
            }
        })
        .collect::<Vec<_>>();
    let initial_status = initial_delivery_plan_status(&prepared_targets);
    let target_policy_fingerprint = target_policy_fingerprint(
        &input.satisfaction_policy,
        input.reticulum_preview_behavior,
        &prepared_targets,
    );
    let delivery_plan_idempotency_digest = delivery_plan_idempotency_digest(
        event_id,
        input.transport_profile_id.as_str(),
        target_policy_fingerprint.as_str(),
        input.target_policy_version,
    );
    Ok(PreparedDeliveryPlan {
        transport_profile_id: input.transport_profile_id.trim().to_owned(),
        target_policy_fingerprint,
        target_policy_version: input.target_policy_version,
        satisfaction_policy: input.satisfaction_policy.clone(),
        required_success_count,
        delivery_plan_idempotency_digest,
        initial_status,
        targets: prepared_targets,
    })
}

fn initial_delivery_target_status(
    target: &RadrootsTransportTarget,
    reticulum_preview_behavior: RadrootsOutboxReticulumPreviewBehavior,
) -> RadrootsOutboxDeliveryTargetStatus {
    if target.kind != RadrootsTransportKind::Reticulum {
        return RadrootsOutboxDeliveryTargetStatus::Pending;
    }
    match reticulum_preview_behavior {
        RadrootsOutboxReticulumPreviewBehavior::RejectDeliveryAttempts => {
            RadrootsOutboxDeliveryTargetStatus::PreviewUnavailable
        }
        RadrootsOutboxReticulumPreviewBehavior::DeferDeliveryPlans => {
            RadrootsOutboxDeliveryTargetStatus::DeferredUntilImplemented
        }
    }
}

fn initial_delivery_plan_status(
    prepared_targets: &[PreparedDeliveryTarget],
) -> RadrootsOutboxDeliveryPlanStatus {
    if prepared_targets
        .iter()
        .any(|target| target.initial_status.is_ready_for_attempt())
    {
        return RadrootsOutboxDeliveryPlanStatus::Queued;
    }
    if prepared_targets.iter().all(|target| {
        target.initial_status == RadrootsOutboxDeliveryTargetStatus::PreviewUnavailable
    }) {
        return RadrootsOutboxDeliveryPlanStatus::PreviewUnavailable;
    }
    if prepared_targets.iter().all(|target| {
        target.initial_status == RadrootsOutboxDeliveryTargetStatus::DeferredUntilImplemented
    }) {
        return RadrootsOutboxDeliveryPlanStatus::DeferredUntilImplemented;
    }
    RadrootsOutboxDeliveryPlanStatus::Queued
}

async fn existing_idempotent_operation(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    operation_kind: &str,
    expected_pubkey: &str,
    idempotency_key: &str,
) -> Result<Option<ExistingOperation>, RadrootsOutboxError> {
    let row = sqlx::query(
        "SELECT o.operation_id, o.operation_idempotency_digest, e.outbox_event_id, e.event_id FROM outbox_operations o JOIN outbox_event e ON e.operation_id = o.operation_id WHERE o.operation_kind = ? AND o.expected_pubkey = ? AND o.idempotency_key = ? ORDER BY e.outbox_event_id LIMIT 1",
    )
    .bind(operation_kind)
    .bind(expected_pubkey)
    .bind(idempotency_key)
    .fetch_optional(&mut **tx)
    .await?;
    row.map(existing_operation_from_row).transpose()
}

async fn existing_idempotent_operation_for_pool(
    pool: &SqlitePool,
    operation_kind: &str,
    expected_pubkey: &str,
    idempotency_key: &str,
) -> Result<Option<ExistingOperation>, RadrootsOutboxError> {
    let row = sqlx::query(
        "SELECT o.operation_id, o.operation_idempotency_digest, e.outbox_event_id, e.event_id FROM outbox_operations o JOIN outbox_event e ON e.operation_id = o.operation_id WHERE o.operation_kind = ? AND o.expected_pubkey = ? AND o.idempotency_key = ? ORDER BY e.outbox_event_id LIMIT 1",
    )
    .bind(operation_kind)
    .bind(expected_pubkey)
    .bind(idempotency_key)
    .fetch_optional(pool)
    .await?;
    row.map(existing_operation_from_row).transpose()
}

fn existing_operation_from_row(
    row: sqlx::sqlite::SqliteRow,
) -> Result<ExistingOperation, RadrootsOutboxError> {
    Ok(ExistingOperation {
        operation_id: row.try_get("operation_id")?,
        outbox_event_id: row.try_get("outbox_event_id")?,
        event_id: row.try_get("event_id")?,
        operation_idempotency_digest: row.try_get("operation_idempotency_digest")?,
    })
}

async fn insert_or_get_delivery_plan(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    outbox_event_id: i64,
    plan: &PreparedDeliveryPlan,
    created_at_ms: i64,
) -> Result<PlanInsertResult, RadrootsOutboxError> {
    if let Some(row) = sqlx::query(
        "SELECT delivery_plan_id FROM outbox_delivery_plan WHERE outbox_event_id = ? AND delivery_plan_idempotency_digest = ?",
    )
    .bind(outbox_event_id)
    .bind(plan.delivery_plan_idempotency_digest.as_str())
    .fetch_optional(&mut **tx)
    .await?
    {
        return Ok(PlanInsertResult {
            status: RadrootsOutboxEnqueueStatus::Existing,
            delivery_plan_id: row.try_get("delivery_plan_id")?,
        });
    }

    let inserted = sqlx::query(
        "INSERT INTO outbox_delivery_plan(outbox_event_id, transport_profile_id, target_policy_fingerprint, target_policy_version, satisfaction_policy, required_success_count, delivery_plan_idempotency_digest, status, created_at_ms, updated_at_ms) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(outbox_event_id)
    .bind(plan.transport_profile_id.as_str())
    .bind(plan.target_policy_fingerprint.as_str())
    .bind(i64::from(plan.target_policy_version))
    .bind(satisfaction_policy_storage_value(&plan.satisfaction_policy))
    .bind(plan.required_success_count)
    .bind(plan.delivery_plan_idempotency_digest.as_str())
    .bind(plan.initial_status.as_str())
    .bind(created_at_ms)
    .bind(created_at_ms)
    .execute(&mut **tx)
    .await?;
    let delivery_plan_id = inserted.last_insert_rowid();
    for prepared_target in &plan.targets {
        sqlx::query(
            "INSERT INTO outbox_delivery_target(delivery_plan_id, transport_kind, endpoint_uri, endpoint_fingerprint, status, attempt_count) VALUES (?, ?, ?, ?, ?, 0)",
        )
        .bind(delivery_plan_id)
        .bind(prepared_target.target.kind.canonical_label())
        .bind(prepared_target.target.uri.as_str())
        .bind(prepared_target.target.fingerprint.as_str())
        .bind(prepared_target.initial_status.as_str())
        .execute(&mut **tx)
        .await?;
    }
    Ok(PlanInsertResult {
        status: RadrootsOutboxEnqueueStatus::Inserted,
        delivery_plan_id,
    })
}

async fn reactivate_event_for_new_plan(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    outbox_event_id: i64,
    now_ms: i64,
) -> Result<(), RadrootsOutboxError> {
    sqlx::query(
        "UPDATE outbox_operations SET status = ?, updated_at_ms = ? WHERE operation_id = (SELECT operation_id FROM outbox_event WHERE outbox_event_id = ?)",
    )
    .bind(RadrootsOutboxOperationStatus::Queued.as_str())
    .bind(now_ms)
    .bind(outbox_event_id)
    .execute(&mut **tx)
    .await?;
    sqlx::query(
        "UPDATE outbox_event SET state = CASE WHEN signed_event_json IS NULL THEN ? ELSE ? END, last_error = NULL, next_attempt_after_ms = ?, updated_at_ms = ? WHERE outbox_event_id = ? AND state IN ('published', 'failed_terminal')",
    )
    .bind(RadrootsOutboxEventState::DraftQueued.as_str())
    .bind(RadrootsOutboxEventState::Signed.as_str())
    .bind(now_ms)
    .bind(now_ms)
    .bind(outbox_event_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn ensure_event_signed(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    outbox_event_id: i64,
    signed_event: &RadrootsSignedNostrEvent,
    event_store_inserted: bool,
    event_store_ingested_at_ms: i64,
) -> Result<(), RadrootsOutboxError> {
    let signed_event_json = serde_json::to_string(signed_event)?;
    sqlx::query(
        "UPDATE outbox_event SET signed_event_json = ?, raw_event_json = ?, state = CASE WHEN state IN ('draft_queued', 'sign_retryable', 'signing') THEN ? ELSE state END, event_store_ingested = 1, event_store_inserted = ?, event_store_ingested_at_ms = ? WHERE outbox_event_id = ? AND signed_event_json IS NULL",
    )
    .bind(signed_event_json.as_str())
    .bind(signed_event.raw_json.as_str())
    .bind(RadrootsOutboxEventState::Signed.as_str())
    .bind(bool_i64(event_store_inserted))
    .bind(event_store_ingested_at_ms)
    .bind(outbox_event_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn claim_event(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    outbox_event_id: i64,
    claimed_state: RadrootsOutboxEventState,
    claim_owner: &str,
    claim_token: &str,
    claim_expires_at_ms: i64,
    now_ms: i64,
    suffix: &str,
) -> Result<SqliteQueryResult, RadrootsOutboxError> {
    let sql = format!(
        "UPDATE outbox_event SET state = ?, claim_token = ?, claim_owner = ?, claim_expires_at_ms = ?, attempt_count = attempt_count + 1, updated_at_ms = ? WHERE outbox_event_id = ? {suffix}"
    );
    let changed = sqlx::query(sql.as_str())
        .bind(claimed_state.as_str())
        .bind(claim_token)
        .bind(claim_owner)
        .bind(claim_expires_at_ms)
        .bind(now_ms)
        .bind(outbox_event_id)
        .bind(now_ms)
        .execute(&mut **tx)
        .await?;
    Ok(changed)
}

async fn claimed_event_from_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    outbox_event_id: i64,
    claimed_state: RadrootsOutboxEventState,
    claim_token: &str,
) -> Result<RadrootsOutboxClaimedEvent, RadrootsOutboxError> {
    let record = event_by_id_tx(tx, outbox_event_id).await?;
    let delivery_targets = if claimed_state == RadrootsOutboxEventState::Publishing {
        ready_delivery_targets_for_event_tx(tx, outbox_event_id).await?
    } else {
        delivery_targets_for_event_tx(tx, outbox_event_id).await?
    };
    Ok(RadrootsOutboxClaimedEvent {
        outbox_event_id: record.outbox_event_id,
        operation_id: record.operation_id,
        expected_event_id: record.event_id,
        attempt_count: record.attempt_count,
        state: claimed_state,
        claim_token: claim_token.to_owned(),
        draft: record.draft,
        signed_event: record.signed_event,
        delivery_targets,
    })
}

async fn event_by_id_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    outbox_event_id: i64,
) -> Result<RadrootsOutboxEventRecord, RadrootsOutboxError> {
    let row = sqlx::query(
        "SELECT outbox_event_id, operation_id, event_id, expected_pubkey, draft_json, signed_event_json, raw_event_json, state, attempt_count, claim_token, claim_owner, claim_expires_at_ms, next_attempt_after_ms, last_error, event_store_ingested, event_store_inserted, event_store_ingested_at_ms, created_at_ms, updated_at_ms FROM outbox_event WHERE outbox_event_id = ?",
    )
    .bind(outbox_event_id)
    .fetch_one(&mut **tx)
    .await?;
    event_from_row(row)
}

async fn claimed_event_identity_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    outbox_event_id: i64,
    claim_token: &str,
) -> Result<ClaimedEventIdentity, RadrootsOutboxError> {
    let row =
        sqlx::query("SELECT operation_id, claim_token FROM outbox_event WHERE outbox_event_id = ?")
            .bind(outbox_event_id)
            .fetch_optional(&mut **tx)
            .await?;
    let Some(row) = row else {
        return Err(RadrootsOutboxError::EventNotFound(outbox_event_id));
    };
    let stored: Option<String> = row.try_get("claim_token")?;
    if stored.as_deref() != Some(claim_token) {
        return Err(RadrootsOutboxError::ClaimTokenMismatch { outbox_event_id });
    }
    Ok(ClaimedEventIdentity {
        operation_id: row.try_get("operation_id")?,
    })
}

async fn delivery_plans_for_pool(
    pool: &SqlitePool,
    outbox_event_id: i64,
) -> Result<Vec<RadrootsOutboxDeliveryPlanRecord>, RadrootsOutboxError> {
    let rows = sqlx::query(
        "SELECT delivery_plan_id, outbox_event_id, transport_profile_id, target_policy_fingerprint, target_policy_version, satisfaction_policy, required_success_count, delivery_plan_idempotency_digest, status, satisfied_at_ms, created_at_ms, updated_at_ms FROM outbox_delivery_plan WHERE outbox_event_id = ? ORDER BY delivery_plan_id",
    )
    .bind(outbox_event_id)
    .fetch_all(pool)
    .await?;
    rows.into_iter().map(delivery_plan_from_row).collect()
}

async fn delivery_plans_for_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    outbox_event_id: i64,
) -> Result<Vec<RadrootsOutboxDeliveryPlanRecord>, RadrootsOutboxError> {
    let rows = sqlx::query(
        "SELECT delivery_plan_id, outbox_event_id, transport_profile_id, target_policy_fingerprint, target_policy_version, satisfaction_policy, required_success_count, delivery_plan_idempotency_digest, status, satisfied_at_ms, created_at_ms, updated_at_ms FROM outbox_delivery_plan WHERE outbox_event_id = ? ORDER BY delivery_plan_id",
    )
    .bind(outbox_event_id)
    .fetch_all(&mut **tx)
    .await?;
    rows.into_iter().map(delivery_plan_from_row).collect()
}

async fn delivery_targets_for_event_pool(
    pool: &SqlitePool,
    outbox_event_id: i64,
) -> Result<Vec<RadrootsOutboxDeliveryTargetRecord>, RadrootsOutboxError> {
    let rows = sqlx::query(
        "SELECT target.delivery_target_id, target.delivery_plan_id, target.transport_kind, target.endpoint_uri, target.endpoint_fingerprint, target.status, target.attempt_count, target.last_attempt_at_ms, target.completed_at_ms, target.last_error FROM outbox_delivery_target AS target JOIN outbox_delivery_plan AS plan ON plan.delivery_plan_id = target.delivery_plan_id WHERE plan.outbox_event_id = ? ORDER BY target.delivery_plan_id, target.delivery_target_id",
    )
    .bind(outbox_event_id)
    .fetch_all(pool)
    .await?;
    rows.into_iter().map(delivery_target_from_row).collect()
}

async fn delivery_targets_for_event_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    outbox_event_id: i64,
) -> Result<Vec<RadrootsOutboxDeliveryTargetRecord>, RadrootsOutboxError> {
    let rows = sqlx::query(
        "SELECT target.delivery_target_id, target.delivery_plan_id, target.transport_kind, target.endpoint_uri, target.endpoint_fingerprint, target.status, target.attempt_count, target.last_attempt_at_ms, target.completed_at_ms, target.last_error FROM outbox_delivery_target AS target JOIN outbox_delivery_plan AS plan ON plan.delivery_plan_id = target.delivery_plan_id WHERE plan.outbox_event_id = ? ORDER BY target.delivery_plan_id, target.delivery_target_id",
    )
    .bind(outbox_event_id)
    .fetch_all(&mut **tx)
    .await?;
    rows.into_iter().map(delivery_target_from_row).collect()
}

async fn ready_delivery_targets_for_event_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    outbox_event_id: i64,
) -> Result<Vec<RadrootsOutboxDeliveryTargetRecord>, RadrootsOutboxError> {
    let rows = sqlx::query(
        "SELECT target.delivery_target_id, target.delivery_plan_id, target.transport_kind, target.endpoint_uri, target.endpoint_fingerprint, target.status, target.attempt_count, target.last_attempt_at_ms, target.completed_at_ms, target.last_error FROM outbox_delivery_target AS target JOIN outbox_delivery_plan AS plan ON plan.delivery_plan_id = target.delivery_plan_id WHERE plan.outbox_event_id = ? AND target.status IN ('pending', 'failed_retryable') ORDER BY target.delivery_plan_id, target.delivery_target_id",
    )
    .bind(outbox_event_id)
    .fetch_all(&mut **tx)
    .await?;
    rows.into_iter().map(delivery_target_from_row).collect()
}

async fn delivery_targets_for_plan_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    delivery_plan_id: i64,
) -> Result<Vec<RadrootsOutboxDeliveryTargetRecord>, RadrootsOutboxError> {
    let rows = sqlx::query(
        "SELECT delivery_target_id, delivery_plan_id, transport_kind, endpoint_uri, endpoint_fingerprint, status, attempt_count, last_attempt_at_ms, completed_at_ms, last_error FROM outbox_delivery_target WHERE delivery_plan_id = ? ORDER BY delivery_target_id",
    )
    .bind(delivery_plan_id)
    .fetch_all(&mut **tx)
    .await?;
    rows.into_iter().map(delivery_target_from_row).collect()
}

async fn delivery_attempts_for_pool(
    pool: &SqlitePool,
    delivery_target_id: i64,
) -> Result<Vec<RadrootsOutboxDeliveryAttemptRecord>, RadrootsOutboxError> {
    let rows = sqlx::query(
        "SELECT delivery_attempt_id, delivery_plan_id, delivery_target_id, status, attempted_at_ms, message FROM outbox_delivery_attempt WHERE delivery_target_id = ? ORDER BY delivery_attempt_id",
    )
    .bind(delivery_target_id)
    .fetch_all(pool)
    .await?;
    rows.into_iter().map(delivery_attempt_from_row).collect()
}

async fn evaluate_delivery_plans(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    outbox_event_id: i64,
    now_ms: i64,
) -> Result<PlanEvaluation, RadrootsOutboxError> {
    let plans = delivery_plans_for_tx(tx, outbox_event_id).await?;
    let mut all_complete = !plans.is_empty();
    let mut any_failed_terminal = false;
    let mut any_ready = false;
    for plan in plans {
        let targets = delivery_targets_for_plan_tx(tx, plan.delivery_plan_id).await?;
        let satisfied_count = targets
            .iter()
            .filter(|target| {
                target
                    .status
                    .counts_as_transport_satisfaction(plan.satisfaction_policy.class())
            })
            .count() as i64;
        let ready_count = targets
            .iter()
            .filter(|target| target.status.is_ready_for_attempt())
            .count();
        let deferred_count = targets
            .iter()
            .filter(|target| target.status.is_deferred_preview())
            .count();
        let preview_unavailable_count = targets
            .iter()
            .filter(|target| {
                target.status == RadrootsOutboxDeliveryTargetStatus::PreviewUnavailable
            })
            .count();
        let terminal_failure_count = targets
            .iter()
            .filter(|target| target.status.is_terminal_failure())
            .count();
        let plan_status = if satisfied_count >= plan.required_success_count {
            RadrootsOutboxDeliveryPlanStatus::Complete
        } else if ready_count > 0 {
            RadrootsOutboxDeliveryPlanStatus::Queued
        } else if terminal_failure_count > 0 {
            RadrootsOutboxDeliveryPlanStatus::FailedTerminal
        } else if preview_unavailable_count > 0 {
            RadrootsOutboxDeliveryPlanStatus::PreviewUnavailable
        } else if deferred_count > 0 {
            RadrootsOutboxDeliveryPlanStatus::DeferredUntilImplemented
        } else {
            RadrootsOutboxDeliveryPlanStatus::FailedTerminal
        };
        if plan_status != RadrootsOutboxDeliveryPlanStatus::Complete {
            all_complete = false;
        }
        if plan_status == RadrootsOutboxDeliveryPlanStatus::FailedTerminal {
            any_failed_terminal = true;
        }
        if ready_count > 0 {
            any_ready = true;
        }
        sqlx::query(
            "UPDATE outbox_delivery_plan SET status = ?, satisfied_at_ms = CASE WHEN ? = 'complete' THEN ? ELSE satisfied_at_ms END, updated_at_ms = ? WHERE delivery_plan_id = ?",
        )
        .bind(plan_status.as_str())
        .bind(plan_status.as_str())
        .bind(now_ms)
        .bind(now_ms)
        .bind(plan.delivery_plan_id)
        .execute(&mut **tx)
        .await?;
    }
    Ok(PlanEvaluation {
        all_complete,
        any_failed_terminal,
        any_ready,
    })
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
        operation_idempotency_digest: row.try_get("operation_idempotency_digest")?,
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

fn delivery_plan_from_row(
    row: sqlx::sqlite::SqliteRow,
) -> Result<RadrootsOutboxDeliveryPlanRecord, RadrootsOutboxError> {
    let required_success_count: i64 = row.try_get("required_success_count")?;
    let satisfaction_policy = parse_satisfaction_policy(
        row.try_get::<String, _>("satisfaction_policy")?.as_str(),
        required_success_count,
    )?;
    let status =
        RadrootsOutboxDeliveryPlanStatus::parse(row.try_get::<String, _>("status")?.as_str())?;
    Ok(RadrootsOutboxDeliveryPlanRecord {
        delivery_plan_id: row.try_get("delivery_plan_id")?,
        outbox_event_id: row.try_get("outbox_event_id")?,
        transport_profile_id: row.try_get("transport_profile_id")?,
        target_policy_fingerprint: row.try_get("target_policy_fingerprint")?,
        target_policy_version: u32_from_i64(
            "target_policy_version",
            row.try_get("target_policy_version")?,
        )?,
        satisfaction_policy,
        required_success_count,
        delivery_plan_idempotency_digest: row.try_get("delivery_plan_idempotency_digest")?,
        status,
        satisfied_at_ms: row.try_get("satisfied_at_ms")?,
        created_at_ms: row.try_get("created_at_ms")?,
        updated_at_ms: row.try_get("updated_at_ms")?,
    })
}

fn delivery_target_from_row(
    row: sqlx::sqlite::SqliteRow,
) -> Result<RadrootsOutboxDeliveryTargetRecord, RadrootsOutboxError> {
    let transport_kind = RadrootsTransportKind::parse(row.try_get::<String, _>("transport_kind")?)?;
    let endpoint_uri =
        RadrootsTransportTargetUri::parse(row.try_get::<String, _>("endpoint_uri")?)?;
    let endpoint_fingerprint = RadrootsTransportTargetFingerprint::parse(
        row.try_get::<String, _>("endpoint_fingerprint")?,
    )?;
    let status =
        RadrootsOutboxDeliveryTargetStatus::parse(row.try_get::<String, _>("status")?.as_str())?;
    Ok(RadrootsOutboxDeliveryTargetRecord {
        delivery_target_id: row.try_get("delivery_target_id")?,
        delivery_plan_id: row.try_get("delivery_plan_id")?,
        transport_kind,
        endpoint_uri,
        endpoint_fingerprint,
        status,
        attempt_count: row.try_get("attempt_count")?,
        last_attempt_at_ms: row.try_get("last_attempt_at_ms")?,
        completed_at_ms: row.try_get("completed_at_ms")?,
        last_error: row.try_get("last_error")?,
    })
}

fn delivery_attempt_from_row(
    row: sqlx::sqlite::SqliteRow,
) -> Result<RadrootsOutboxDeliveryAttemptRecord, RadrootsOutboxError> {
    let status =
        RadrootsOutboxDeliveryTargetStatus::parse(row.try_get::<String, _>("status")?.as_str())?;
    Ok(RadrootsOutboxDeliveryAttemptRecord {
        delivery_attempt_id: row.try_get("delivery_attempt_id")?,
        delivery_plan_id: row.try_get("delivery_plan_id")?,
        delivery_target_id: row.try_get("delivery_target_id")?,
        status,
        attempted_at_ms: row.try_get("attempted_at_ms")?,
        message: row.try_get("message")?,
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

fn ordered_unique_targets(targets: Vec<RadrootsTransportTarget>) -> Vec<RadrootsTransportTarget> {
    let mut out = Vec::new();
    for target in targets {
        if !out
            .iter()
            .any(|existing: &RadrootsTransportTarget| existing.fingerprint == target.fingerprint)
        {
            out.push(target);
        }
    }
    out
}

#[derive(Serialize)]
struct OperationDigestInput<'a> {
    operation_kind: &'a str,
    expected_pubkey: &'a str,
    draft: &'a RadrootsFrozenEventDraft,
}

fn operation_idempotency_digest(
    operation_kind: &str,
    expected_pubkey: &str,
    draft: &RadrootsFrozenEventDraft,
) -> String {
    let input = OperationDigestInput {
        operation_kind,
        expected_pubkey,
        draft,
    };
    sha256_json(&input)
}

#[derive(Serialize)]
struct TargetPolicyDigestInput<'a> {
    satisfaction_policy: String,
    reticulum_preview_behavior: &'a str,
    targets: Vec<TargetPolicyDigestTarget<'a>>,
}

#[derive(Serialize)]
struct TargetPolicyDigestTarget<'a> {
    transport_kind: String,
    endpoint_uri: &'a str,
    endpoint_fingerprint: &'a str,
}

fn target_policy_fingerprint(
    satisfaction_policy: &RadrootsTransportSatisfactionPolicy,
    reticulum_preview_behavior: RadrootsOutboxReticulumPreviewBehavior,
    targets: &[PreparedDeliveryTarget],
) -> String {
    let mut target_inputs = targets
        .iter()
        .map(|target| TargetPolicyDigestTarget {
            transport_kind: target.target.kind.canonical_label(),
            endpoint_uri: target.target.uri.as_str(),
            endpoint_fingerprint: target.target.fingerprint.as_str(),
        })
        .collect::<Vec<_>>();
    target_inputs.sort_by(|left, right| {
        left.endpoint_fingerprint
            .cmp(right.endpoint_fingerprint)
            .then_with(|| left.transport_kind.cmp(&right.transport_kind))
            .then_with(|| left.endpoint_uri.cmp(right.endpoint_uri))
    });
    sha256_json(&TargetPolicyDigestInput {
        satisfaction_policy: satisfaction_policy_storage_value(satisfaction_policy),
        reticulum_preview_behavior: reticulum_preview_behavior.as_str(),
        targets: target_inputs,
    })
}

#[derive(Serialize)]
struct DeliveryPlanDigestInput<'a> {
    event_id: &'a str,
    transport_profile_id: &'a str,
    target_policy_fingerprint: &'a str,
    target_policy_version: u32,
}

fn delivery_plan_idempotency_digest(
    event_id: &str,
    transport_profile_id: &str,
    target_policy_fingerprint: &str,
    target_policy_version: u32,
) -> String {
    sha256_json(&DeliveryPlanDigestInput {
        event_id,
        transport_profile_id,
        target_policy_fingerprint,
        target_policy_version,
    })
}

fn sha256_json<T: Serialize>(value: &T) -> String {
    let bytes = serde_json::to_vec(value).expect("outbox digest input is serializable");
    hex::encode(Sha256::digest(bytes))
}

fn satisfaction_policy_storage_value(policy: &RadrootsTransportSatisfactionPolicy) -> String {
    match policy {
        RadrootsTransportSatisfactionPolicy::All { class } => {
            format!("all_{}", satisfaction_class_storage_value(*class))
        }
        RadrootsTransportSatisfactionPolicy::Any { class } => {
            format!("any_{}", satisfaction_class_storage_value(*class))
        }
        RadrootsTransportSatisfactionPolicy::Quorum { class, threshold } => {
            format!(
                "quorum_{}:{threshold}",
                satisfaction_class_storage_value(*class)
            )
        }
    }
}

fn satisfaction_class_storage_value(class: RadrootsTransportSatisfactionClass) -> &'static str {
    match class {
        RadrootsTransportSatisfactionClass::Accepted => "accepted",
        RadrootsTransportSatisfactionClass::Delivered => "delivered",
    }
}

fn parse_satisfaction_policy(
    value: &str,
    required_success_count: i64,
) -> Result<RadrootsTransportSatisfactionPolicy, RadrootsOutboxError> {
    match value {
        "all_accepted" => Ok(RadrootsTransportSatisfactionPolicy::all_accepted()),
        "any_accepted" => Ok(RadrootsTransportSatisfactionPolicy::any_accepted()),
        "all_delivered" => Ok(RadrootsTransportSatisfactionPolicy::all_delivered()),
        "any_delivered" => Ok(RadrootsTransportSatisfactionPolicy::any_delivered()),
        stored if stored == format!("quorum_accepted:{required_success_count}") => {
            Ok(RadrootsTransportSatisfactionPolicy::quorum_accepted(
                required_count_u16(required_success_count)?,
            ))
        }
        stored if stored == format!("quorum_delivered:{required_success_count}") => {
            Ok(RadrootsTransportSatisfactionPolicy::quorum_delivered(
                required_count_u16(required_success_count)?,
            ))
        }
        _ => Err(RadrootsOutboxError::InvalidStoredEnum {
            field: "outbox_delivery_plan.satisfaction_policy",
            value: value.to_owned(),
        }),
    }
}

fn required_count_u16(required_success_count: i64) -> Result<u16, RadrootsOutboxError> {
    u16::try_from(required_success_count).map_err(|_| RadrootsOutboxError::IntegerRange {
        field: "required_success_count",
        value: required_success_count,
    })
}

fn bool_i64(value: bool) -> i64 {
    if value { 1 } else { 0 }
}

fn u32_from_i64(field: &'static str, value: i64) -> Result<u32, RadrootsOutboxError> {
    u32::try_from(value).map_err(|_| RadrootsOutboxError::IntegerRange { field, value })
}

#[cfg(test)]
mod tests {
    use super::*;
    use radroots_events::kinds::KIND_POST;
    use radroots_nostr::prelude::{
        RadrootsNostrKeys, RadrootsNostrSecretKey, radroots_nostr_sign_frozen_draft,
    };

    const FIXTURE_ALICE_SECRET_KEY_HEX: &str =
        "10c5304d6c9ae3a1a16f7860f1cc8f5e3a76225a2663b3a989a0d775919b7df5";
    const FIXTURE_ALICE_PUBLIC_KEY_HEX: &str =
        "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df";
    const NOSTR_PRIMARY_WSS: &str = "wss://relay.example.com";
    const NOSTR_SECONDARY_WSS: &str = "wss://relay-2.example.com";

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

    fn nostr_target(uri: &str) -> RadrootsTransportTarget {
        RadrootsTransportTarget::new(RadrootsTransportKind::Nostr, uri).expect("nostr target")
    }

    fn reticulum_target(uri: &str) -> RadrootsTransportTarget {
        RadrootsTransportTarget::new(RadrootsTransportKind::Reticulum, uri)
            .expect("reticulum target")
    }

    fn delivery_plan(targets: Vec<RadrootsTransportTarget>) -> RadrootsOutboxDeliveryPlanInput {
        RadrootsOutboxDeliveryPlanInput::new(
            "transport.nostr.local",
            1,
            RadrootsTransportSatisfactionPolicy::all_accepted(),
            targets,
        )
    }

    fn operation_input(
        draft: RadrootsFrozenEventDraft,
        created_at_ms: i64,
    ) -> RadrootsOutboxOperationInput {
        RadrootsOutboxOperationInput::new(
            "publish_post",
            draft,
            delivery_plan(vec![
                nostr_target(NOSTR_PRIMARY_WSS),
                nostr_target(NOSTR_SECONDARY_WSS),
                nostr_target(NOSTR_PRIMARY_WSS),
            ]),
            created_at_ms,
        )
    }

    fn signed_operation_input(
        draft: RadrootsFrozenEventDraft,
        signed_event: RadrootsSignedNostrEvent,
        created_at_ms: i64,
    ) -> RadrootsOutboxSignedOperationInput {
        RadrootsOutboxSignedOperationInput::new(
            "publish_post",
            draft,
            signed_event,
            delivery_plan(vec![
                nostr_target(NOSTR_PRIMARY_WSS),
                nostr_target(NOSTR_SECONDARY_WSS),
            ]),
            true,
            created_at_ms + 7,
            created_at_ms,
        )
    }

    fn fixture_keys() -> RadrootsNostrKeys {
        let secret_key =
            RadrootsNostrSecretKey::from_hex(FIXTURE_ALICE_SECRET_KEY_HEX).expect("secret key");
        RadrootsNostrKeys::new(secret_key)
    }

    async fn table_count(outbox: &RadrootsOutbox, table_name: &str) -> i64 {
        let sql = format!("SELECT COUNT(*) FROM {table_name}");
        sqlx::query_scalar(sql.as_str())
            .fetch_one(outbox.pool())
            .await
            .expect("table count")
    }

    #[tokio::test]
    async fn migration_applies_delivery_plan_schema_and_migrates_down() {
        let outbox = RadrootsOutbox::open_memory().await.expect("open");

        assert_eq!(outbox.pragma_foreign_keys().await.expect("foreign keys"), 1);
        assert_eq!(
            outbox.pragma_busy_timeout().await.expect("busy timeout"),
            5_000
        );
        assert_eq!(
            outbox.pragma_journal_mode().await.expect("journal mode"),
            "memory"
        );

        for table in [
            "outbox_operations",
            "outbox_event",
            "outbox_delivery_plan",
            "outbox_delivery_target",
            "outbox_delivery_attempt",
        ] {
            let row =
                sqlx::query("SELECT name FROM sqlite_master WHERE type = 'table' AND name = ?")
                    .bind(table)
                    .fetch_optional(outbox.pool())
                    .await
                    .expect("table query");
            assert!(row.is_some(), "{table}");
        }
        let old = sqlx::query(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'outbox_event_relay_status'",
        )
        .fetch_optional(outbox.pool())
        .await
        .expect("old table query");
        assert!(old.is_none());

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
    async fn operation_and_delivery_plan_idempotency_are_split() {
        let outbox = RadrootsOutbox::open_memory().await.expect("open");
        let draft = post_draft(hex_64('a').as_str(), "hello");
        let first = outbox
            .enqueue_operation(operation_input(draft.clone(), 1_000).with_idempotency_key("idem-a"))
            .await
            .expect("first");
        let same_plan = outbox
            .enqueue_operation(operation_input(draft.clone(), 1_100).with_idempotency_key("idem-a"))
            .await
            .expect("same");
        let new_plan = outbox
            .enqueue_operation(
                RadrootsOutboxOperationInput::new(
                    "publish_post",
                    draft.clone(),
                    delivery_plan(vec![nostr_target("wss://relay-3.example.com")]),
                    1_200,
                )
                .with_idempotency_key("idem-a"),
            )
            .await
            .expect("new plan");

        assert_eq!(first.status, RadrootsOutboxEnqueueStatus::Inserted);
        assert_eq!(same_plan.status, RadrootsOutboxEnqueueStatus::Existing);
        assert_eq!(new_plan.status, RadrootsOutboxEnqueueStatus::Inserted);
        assert_eq!(first.operation_id, same_plan.operation_id);
        assert_eq!(first.operation_id, new_plan.operation_id);
        assert_eq!(first.outbox_event_id, new_plan.outbox_event_id);
        assert_eq!(
            first.operation_idempotency_digest,
            new_plan.operation_idempotency_digest
        );
        assert_ne!(
            first.delivery_plan_idempotency_digest,
            new_plan.delivery_plan_idempotency_digest
        );
        assert_eq!(table_count(&outbox, "outbox_operations").await, 1);
        assert_eq!(table_count(&outbox, "outbox_event").await, 1);
        assert_eq!(table_count(&outbox, "outbox_delivery_plan").await, 2);

        let conflict = outbox
            .enqueue_operation(
                operation_input(post_draft(hex_64('a').as_str(), "changed"), 1_300)
                    .with_idempotency_key("idem-a"),
            )
            .await
            .expect_err("conflict");
        assert!(matches!(
            conflict,
            RadrootsOutboxError::IdempotencyConflict { .. }
        ));
    }

    #[tokio::test]
    async fn enqueue_rejects_empty_delivery_targets_before_persistence() {
        let outbox = RadrootsOutbox::open_memory().await.expect("open");
        let draft = post_draft(hex_64('a').as_str(), "hello");

        let err = outbox
            .enqueue_operation(RadrootsOutboxOperationInput::new(
                "publish_post",
                draft,
                delivery_plan(Vec::new()),
                1_000,
            ))
            .await
            .expect_err("empty targets");

        assert!(matches!(err, RadrootsOutboxError::EmptyDeliveryTargets));
        assert_eq!(table_count(&outbox, "outbox_operations").await, 0);
        assert_eq!(table_count(&outbox, "outbox_event").await, 0);
        assert_eq!(table_count(&outbox, "outbox_delivery_plan").await, 0);
    }

    #[tokio::test]
    async fn signed_enqueue_claims_ready_delivery_targets_and_records_attempts() {
        let outbox = RadrootsOutbox::open_memory().await.expect("open");
        let draft = post_draft(FIXTURE_ALICE_PUBLIC_KEY_HEX, "signed");
        let signed_event =
            radroots_nostr_sign_frozen_draft(&fixture_keys(), &draft).expect("signed event");
        let receipt = outbox
            .enqueue_signed_operation(signed_operation_input(draft, signed_event.clone(), 1_000))
            .await
            .expect("enqueue");

        let event = outbox
            .get_event(receipt.outbox_event_id)
            .await
            .expect("event")
            .expect("event");
        assert_eq!(event.state, RadrootsOutboxEventState::Signed);
        assert_eq!(event.signed_event, Some(signed_event));

        let claimed = outbox
            .claim_next_ready_signed_event("publisher", "claim-a", 2_000, 1_000)
            .await
            .expect("claim")
            .expect("claimed");
        assert_eq!(claimed.state, RadrootsOutboxEventState::Publishing);
        assert_eq!(claimed.delivery_targets.len(), 2);

        outbox
            .mark_delivery_target_accepted(
                receipt.outbox_event_id,
                "claim-a",
                claimed.delivery_targets[0].delivery_target_id,
                1_100,
            )
            .await
            .expect("first accepted");
        outbox
            .mark_delivery_target_accepted(
                receipt.outbox_event_id,
                "claim-a",
                claimed.delivery_targets[1].delivery_target_id,
                1_110,
            )
            .await
            .expect("second accepted");

        let state = outbox
            .complete_publish_attempt(
                receipt.outbox_event_id,
                "claim-a",
                "retryable",
                "terminal",
                2_500,
                1_200,
            )
            .await
            .expect("complete");
        assert_eq!(state, RadrootsOutboxEventState::Published);
        let operation = outbox
            .get_operation(receipt.operation_id)
            .await
            .expect("operation")
            .expect("operation");
        assert_eq!(operation.status, RadrootsOutboxOperationStatus::Complete);
        let attempts = outbox
            .delivery_attempts(claimed.delivery_targets[0].delivery_target_id)
            .await
            .expect("attempts");
        assert_eq!(attempts.len(), 1);
        assert_eq!(
            attempts[0].status,
            RadrootsOutboxDeliveryTargetStatus::Accepted
        );
    }

    #[tokio::test]
    async fn quorum_accepted_delivery_plan_round_trips_and_completes_after_required_target() {
        let outbox = RadrootsOutbox::open_memory().await.expect("open");
        let draft = post_draft(FIXTURE_ALICE_PUBLIC_KEY_HEX, "at least");
        let signed_event =
            radroots_nostr_sign_frozen_draft(&fixture_keys(), &draft).expect("signed event");
        let receipt = outbox
            .enqueue_signed_operation(RadrootsOutboxSignedOperationInput::new(
                "publish_post",
                draft,
                signed_event,
                RadrootsOutboxDeliveryPlanInput::new(
                    "transport.nostr.local",
                    7,
                    RadrootsTransportSatisfactionPolicy::quorum_accepted(1),
                    vec![
                        nostr_target(NOSTR_PRIMARY_WSS),
                        nostr_target(NOSTR_SECONDARY_WSS),
                    ],
                ),
                true,
                1_007,
                1_000,
            ))
            .await
            .expect("enqueue");
        let plans = outbox
            .delivery_plans(receipt.outbox_event_id)
            .await
            .expect("plans");

        assert_eq!(plans.len(), 1);
        assert_eq!(
            plans[0].satisfaction_policy,
            RadrootsTransportSatisfactionPolicy::quorum_accepted(1)
        );
        assert_eq!(plans[0].required_success_count, 1);
        assert_eq!(plans[0].target_policy_version, 7);

        let claimed = outbox
            .claim_next_ready_signed_event("publisher", "claim-a", 2_000, 1_000)
            .await
            .expect("claim")
            .expect("claimed");
        outbox
            .mark_delivery_target_accepted(
                receipt.outbox_event_id,
                "claim-a",
                claimed.delivery_targets[0].delivery_target_id,
                1_100,
            )
            .await
            .expect("accepted");
        let state = outbox
            .complete_publish_attempt(
                receipt.outbox_event_id,
                "claim-a",
                "retryable",
                "terminal",
                2_500,
                1_200,
            )
            .await
            .expect("complete");

        assert_eq!(state, RadrootsOutboxEventState::Published);
    }

    #[tokio::test]
    async fn publish_attempt_stays_retryable_while_delivery_targets_remain_ready() {
        let outbox = RadrootsOutbox::open_memory().await.expect("open");
        let draft = post_draft(FIXTURE_ALICE_PUBLIC_KEY_HEX, "retryable");
        let signed_event =
            radroots_nostr_sign_frozen_draft(&fixture_keys(), &draft).expect("signed event");
        let receipt = outbox
            .enqueue_signed_operation(signed_operation_input(draft, signed_event, 1_000))
            .await
            .expect("enqueue");
        let claimed = outbox
            .claim_next_ready_signed_event("publisher", "claim-a", 2_000, 1_000)
            .await
            .expect("claim")
            .expect("claimed");
        outbox
            .mark_delivery_target_failed_retryable(
                receipt.outbox_event_id,
                "claim-a",
                claimed.delivery_targets[0].delivery_target_id,
                "timeout",
                1_100,
            )
            .await
            .expect("failed retryable");
        let state = outbox
            .complete_publish_attempt(
                receipt.outbox_event_id,
                "claim-a",
                "retryable error",
                "terminal",
                2_500,
                1_200,
            )
            .await
            .expect("complete");
        assert_eq!(state, RadrootsOutboxEventState::PublishRetryable);
        let summary = outbox.status_summary(2_500).await.expect("summary");
        assert_eq!(summary.ready_signed_events, 1);
        assert_eq!(summary.last_attempt_at_ms, Some(1_100));
    }

    #[tokio::test]
    async fn reticulum_reject_targets_are_preview_unavailable_and_not_ready() {
        let outbox = RadrootsOutbox::open_memory().await.expect("open");
        let draft = post_draft(FIXTURE_ALICE_PUBLIC_KEY_HEX, "reticulum rejected");
        let signed_event =
            radroots_nostr_sign_frozen_draft(&fixture_keys(), &draft).expect("signed event");
        let receipt = outbox
            .enqueue_signed_operation(RadrootsOutboxSignedOperationInput::new(
                "publish_post",
                draft,
                signed_event,
                RadrootsOutboxDeliveryPlanInput::new(
                    "transport.reticulum.preview",
                    1,
                    RadrootsTransportSatisfactionPolicy::all_accepted(),
                    vec![reticulum_target("reticulum:preview-unavailable")],
                ),
                true,
                1_007,
                1_000,
            ))
            .await
            .expect("enqueue");
        let targets = outbox
            .delivery_targets(receipt.outbox_event_id)
            .await
            .expect("targets");
        assert_eq!(targets.len(), 1);
        assert_eq!(
            targets[0].status,
            RadrootsOutboxDeliveryTargetStatus::PreviewUnavailable
        );
        let plans = outbox
            .delivery_plans(receipt.outbox_event_id)
            .await
            .expect("plans");
        assert_eq!(
            plans[0].status,
            RadrootsOutboxDeliveryPlanStatus::PreviewUnavailable
        );
        let summary = outbox.status_summary(1_000).await.expect("summary");
        assert_eq!(summary.ready_signed_events, 0);
        assert!(
            outbox
                .claim_next_ready_signed_event("publisher", "claim-a", 2_000, 1_000)
                .await
                .expect("claim")
                .is_none()
        );
    }

    #[tokio::test]
    async fn reticulum_deferred_targets_do_not_retry_or_satisfy_delivery() {
        let outbox = RadrootsOutbox::open_memory().await.expect("open");
        let draft = post_draft(FIXTURE_ALICE_PUBLIC_KEY_HEX, "reticulum deferred");
        let signed_event =
            radroots_nostr_sign_frozen_draft(&fixture_keys(), &draft).expect("signed event");
        let receipt = outbox
            .enqueue_signed_operation(RadrootsOutboxSignedOperationInput::new(
                "publish_post",
                draft,
                signed_event,
                RadrootsOutboxDeliveryPlanInput::new(
                    "transport.reticulum.preview",
                    1,
                    RadrootsTransportSatisfactionPolicy::all_accepted(),
                    vec![reticulum_target("reticulum:preview-unavailable")],
                )
                .with_reticulum_preview_behavior(
                    RadrootsOutboxReticulumPreviewBehavior::DeferDeliveryPlans,
                ),
                true,
                1_007,
                1_000,
            ))
            .await
            .expect("enqueue");
        let targets = outbox
            .delivery_targets(receipt.outbox_event_id)
            .await
            .expect("targets");
        assert_eq!(targets.len(), 1);
        assert_eq!(
            targets[0].status,
            RadrootsOutboxDeliveryTargetStatus::DeferredUntilImplemented
        );
        let plans = outbox
            .delivery_plans(receipt.outbox_event_id)
            .await
            .expect("plans");
        assert_eq!(
            plans[0].status,
            RadrootsOutboxDeliveryPlanStatus::DeferredUntilImplemented
        );
        let summary = outbox.status_summary(1_000).await.expect("summary");
        assert_eq!(summary.ready_signed_events, 0);
        assert!(
            outbox
                .claim_next_ready_signed_event("publisher", "claim-a", 2_000, 1_000)
                .await
                .expect("claim")
                .is_none()
        );
        let default_behavior_draft = post_draft(FIXTURE_ALICE_PUBLIC_KEY_HEX, "reticulum deferred");
        let default_behavior_signed_event =
            radroots_nostr_sign_frozen_draft(&fixture_keys(), &default_behavior_draft)
                .expect("signed event");
        let default_behavior_preflight = outbox
            .preflight_signed_operation_idempotency(&RadrootsOutboxSignedOperationInput::new(
                "publish_post",
                default_behavior_draft,
                default_behavior_signed_event,
                RadrootsOutboxDeliveryPlanInput::new(
                    "transport.reticulum.preview",
                    1,
                    RadrootsTransportSatisfactionPolicy::all_accepted(),
                    vec![reticulum_target("reticulum:preview-unavailable")],
                ),
                true,
                1_007,
                1_000,
            ))
            .await
            .expect("preflight");
        assert_ne!(
            receipt.delivery_plan_idempotency_digest,
            default_behavior_preflight.delivery_plan_idempotency_digest
        );
    }

    #[tokio::test]
    async fn hybrid_nostr_and_reticulum_preview_preserves_ready_nostr_work() {
        let outbox = RadrootsOutbox::open_memory().await.expect("open");
        let draft = post_draft(FIXTURE_ALICE_PUBLIC_KEY_HEX, "hybrid preview");
        let signed_event =
            radroots_nostr_sign_frozen_draft(&fixture_keys(), &draft).expect("signed event");
        let receipt = outbox
            .enqueue_signed_operation(RadrootsOutboxSignedOperationInput::new(
                "publish_post",
                draft,
                signed_event,
                RadrootsOutboxDeliveryPlanInput::new(
                    "transport.hybrid",
                    1,
                    RadrootsTransportSatisfactionPolicy::any_accepted(),
                    vec![
                        nostr_target(NOSTR_PRIMARY_WSS),
                        reticulum_target("reticulum:preview-unavailable"),
                    ],
                ),
                true,
                1_007,
                1_000,
            ))
            .await
            .expect("enqueue");
        let targets = outbox
            .delivery_targets(receipt.outbox_event_id)
            .await
            .expect("targets");
        assert_eq!(targets.len(), 2);
        assert!(
            targets
                .iter()
                .any(|target| target.status == RadrootsOutboxDeliveryTargetStatus::Pending)
        );
        assert!(targets.iter().any(|target| {
            target.status == RadrootsOutboxDeliveryTargetStatus::PreviewUnavailable
        }));
        let plans = outbox
            .delivery_plans(receipt.outbox_event_id)
            .await
            .expect("plans");
        assert_eq!(plans[0].status, RadrootsOutboxDeliveryPlanStatus::Queued);
        assert_eq!(
            outbox
                .status_summary(1_000)
                .await
                .expect("summary")
                .ready_signed_events,
            1
        );
    }

    #[tokio::test]
    async fn local_signed_event_ingest_remains_idempotent_without_transport_observation() {
        let outbox = RadrootsOutbox::open_memory().await.expect("open");
        let event_store = RadrootsEventStore::open_memory()
            .await
            .expect("event store");
        let draft = post_draft(FIXTURE_ALICE_PUBLIC_KEY_HEX, "local ingest");
        let receipt = outbox
            .enqueue_operation(operation_input(draft, 1_000))
            .await
            .expect("enqueue");
        let claimed = outbox
            .claim_next_ready_event("signer", "claim-a", 2_000, 1_000)
            .await
            .expect("claim")
            .expect("claimed");
        let signed =
            radroots_nostr_sign_frozen_draft(&fixture_keys(), &claimed.draft).expect("signed");
        outbox
            .complete_signing(
                receipt.outbox_event_id,
                claimed.claim_token.as_str(),
                signed.clone(),
                1_100,
            )
            .await
            .expect("complete signing");
        outbox.recover_expired_claims(2_001).await.expect("recover");
        outbox
            .claim_next_ready_event("publisher", "claim-b", 3_000, 2_100)
            .await
            .expect("claim")
            .expect("publish claim");

        let first = outbox
            .ingest_signed_event_local(&event_store, receipt.outbox_event_id, "claim-b", 2_200)
            .await
            .expect("first ingest");
        assert_eq!(first.event_id, signed.id);
        assert!(!first.already_ingested);

        let second = outbox
            .ingest_signed_event_local(&event_store, receipt.outbox_event_id, "claim-b", 2_300)
            .await
            .expect("second ingest");
        assert!(second.already_ingested);
        let observations = event_store
            .observations_for_event(signed.id.as_str())
            .await
            .expect("observations");
        assert!(observations.is_empty());
    }
}
