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
    RadrootsOutboxReticulumPreviewBehavior, RadrootsOutboxReticulumPreviewEventRecord,
    RadrootsOutboxSignedOperationInput, RadrootsOutboxStatusSummary,
};
use radroots_event::draft::{
    RadrootsEventDraft, RadrootsSignedEvent, validate_signed_nostr_event_matches_draft,
};
use radroots_event::wire::RadrootsNip01EventWire;
use radroots_event_store::{
    RadrootsEventIngest, RadrootsEventStore, RadrootsTransportObservation,
    RadrootsTransportObservationType,
};
use radroots_transport::{
    RADROOTS_RETICULUM_PREVIEW_ENDPOINT_URI, RadrootsTransportError, RadrootsTransportKind,
    RadrootsTransportMeshScopeId, RadrootsTransportOutcomeKind, RadrootsTransportSatisfactionClass,
    RadrootsTransportSatisfactionPolicy, RadrootsTransportTarget,
    RadrootsTransportTargetFingerprint, RadrootsTransportTargetLabel, RadrootsTransportTargetUri,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions, SqliteQueryResult};
use sqlx::{Row, SqlitePool};
use std::collections::BTreeSet;
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
            "SELECT COUNT(*) AS total_events, COALESCE(SUM(CASE WHEN state IN ('draft_queued', 'signing', 'signed', 'publishing') THEN 1 ELSE 0 END), 0) AS pending_events, COALESCE(SUM(CASE WHEN state IN ('sign_retryable', 'publish_retryable') THEN 1 ELSE 0 END), 0) AS retryable_events, COALESCE(SUM(CASE WHEN state IN ('published', 'failed_terminal', 'cancelled') THEN 1 ELSE 0 END), 0) AS terminal_events, COALESCE(SUM(CASE WHEN state = 'failed_terminal' THEN 1 ELSE 0 END), 0) AS failed_terminal_events, COALESCE(SUM(CASE WHEN state = 'preview_unavailable' THEN 1 ELSE 0 END), 0) AS preview_unavailable_events, COALESCE(SUM(CASE WHEN state = 'deferred_until_implemented' THEN 1 ELSE 0 END), 0) AS deferred_until_implemented_events, COALESCE(SUM(CASE WHEN state = 'publishing' THEN 1 ELSE 0 END), 0) AS publishing_events FROM outbox_event",
        )
        .fetch_one(&self.pool)
        .await?;
        let ready_signed_events = sqlx::query(
            "SELECT COUNT(*) FROM outbox_event AS event WHERE event.state IN ('signed', 'publish_retryable') AND event.signed_event_json IS NOT NULL AND event.next_attempt_after_ms <= ? AND (event.claim_token IS NULL OR event.claim_expires_at_ms <= ?) AND EXISTS (SELECT 1 FROM outbox_delivery_plan AS plan JOIN outbox_delivery_target AS target ON target.delivery_plan_id = plan.delivery_plan_id WHERE plan.outbox_event_id = event.outbox_event_id AND plan.status = 'queued' AND target.status IN ('pending', 'failed_retryable'))",
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
            preview_unavailable_events: row.try_get("preview_unavailable_events")?,
            deferred_until_implemented_events: row.try_get("deferred_until_implemented_events")?,
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
        let prepared =
            prepare_delivery_plan(input.draft.expected_event_id_str(), &input.delivery_plan)?;
        let operation_digest = operation_idempotency_digest(
            input.operation_kind.as_str(),
            input.draft.expected_pubkey_str(),
            &input.draft,
        );

        if let Some(idempotency_key) = input.idempotency_key.as_deref()
            && let Some(existing) = existing_idempotent_operation_for_pool(
                &self.pool,
                input.operation_kind.as_str(),
                input.draft.expected_pubkey_str(),
                idempotency_key,
            )
            .await?
            && existing.operation_idempotency_digest != operation_digest
        {
            return Err(RadrootsOutboxError::IdempotencyConflict {
                operation_kind: input.operation_kind.clone(),
                expected_pubkey: input.draft.expected_pubkey_str().to_owned(),
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
        let prepared =
            prepare_delivery_plan(input.draft.expected_event_id_str(), &input.delivery_plan)?;
        let operation_digest = operation_idempotency_digest(
            input.operation_kind.as_str(),
            input.draft.expected_pubkey_str(),
            &input.draft,
        );
        let mut tx = self.pool.begin().await?;

        if let Some(idempotency_key) = input.idempotency_key.as_deref()
            && let Some(existing) = existing_idempotent_operation(
                &mut tx,
                input.operation_kind.as_str(),
                input.draft.expected_pubkey_str(),
                idempotency_key,
            )
            .await?
        {
            if existing.operation_idempotency_digest != operation_digest {
                return Err(RadrootsOutboxError::IdempotencyConflict {
                    operation_kind: input.operation_kind,
                    expected_pubkey: input.draft.expected_pubkey_str().to_owned(),
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
                sync_signed_event_lifecycle(&mut tx, existing.outbox_event_id, input.created_at_ms)
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
        .bind(input.draft.expected_pubkey_str())
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
        .bind(input.draft.expected_event_id_str())
        .bind(input.draft.expected_pubkey_str())
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
            expected_event_id: input.draft.expected_event_id_str().to_owned(),
            operation_idempotency_digest: operation_digest,
            delivery_plan_idempotency_digest: prepared.delivery_plan_idempotency_digest,
        })
    }

    pub async fn enqueue_signed_operation(
        &self,
        input: RadrootsOutboxSignedOperationInput,
    ) -> Result<RadrootsOutboxEnqueueReceipt, RadrootsOutboxError> {
        validate_signed_nostr_event_matches_draft(&input.signed_event, &input.draft)?;
        let prepared =
            prepare_delivery_plan(input.draft.expected_event_id_str(), &input.delivery_plan)?;
        let operation_digest = operation_idempotency_digest(
            input.operation_kind.as_str(),
            input.draft.expected_pubkey_str(),
            &input.draft,
        );
        let mut tx = self.pool.begin().await?;

        if let Some(idempotency_key) = input.idempotency_key.as_deref()
            && let Some(existing) = existing_idempotent_operation(
                &mut tx,
                input.operation_kind.as_str(),
                input.draft.expected_pubkey_str(),
                idempotency_key,
            )
            .await?
        {
            if existing.operation_idempotency_digest != operation_digest {
                return Err(RadrootsOutboxError::IdempotencyConflict {
                    operation_kind: input.operation_kind,
                    expected_pubkey: input.draft.expected_pubkey_str().to_owned(),
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
            sync_signed_event_lifecycle(&mut tx, existing.outbox_event_id, input.created_at_ms)
                .await?;
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
        .bind(input.draft.expected_pubkey_str())
        .bind(input.idempotency_key.as_deref())
        .bind(operation_digest.as_str())
        .bind(RadrootsOutboxOperationStatus::Queued.as_str())
        .bind(input.created_at_ms)
        .bind(input.created_at_ms)
        .execute(&mut *tx)
        .await?;
        let operation_id = operation.last_insert_rowid();
        let draft_json = serde_json::to_string(&input.draft)?;
        let signed_event_json = signed_event_wire_json(&input.signed_event)?;
        let event = sqlx::query(
            "INSERT INTO outbox_event(operation_id, event_id, expected_pubkey, draft_json, signed_event_json, raw_event_json, state, attempt_count, next_attempt_after_ms, event_store_ingested, event_store_inserted, event_store_ingested_at_ms, created_at_ms, updated_at_ms) VALUES (?, ?, ?, ?, ?, ?, ?, 0, ?, 1, ?, ?, ?, ?)",
        )
        .bind(operation_id)
        .bind(input.draft.expected_event_id_str())
        .bind(input.draft.expected_pubkey_str())
        .bind(draft_json.as_str())
        .bind(signed_event_json.as_str())
        .bind(input.signed_event.raw_json())
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
        sync_signed_event_lifecycle(&mut tx, outbox_event_id, input.created_at_ms).await?;
        tx.commit().await?;
        Ok(RadrootsOutboxEnqueueReceipt {
            status: RadrootsOutboxEnqueueStatus::Inserted,
            operation_id,
            outbox_event_id,
            delivery_plan_id: plan.delivery_plan_id,
            expected_event_id: input.draft.expected_event_id_str().to_owned(),
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
            "SELECT outbox_event_id, operation_id, event_id, expected_pubkey, draft_json, signed_event_json, raw_event_json, state, attempt_count, claim_token, claim_owner, claim_expires_at_ms, active_delivery_plan_id, next_attempt_after_ms, last_error, event_store_ingested, event_store_inserted, event_store_ingested_at_ms, created_at_ms, updated_at_ms FROM outbox_event WHERE outbox_event_id = ?",
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

    pub async fn reticulum_preview_events(
        &self,
        outbox_event_id: Option<i64>,
        limit: usize,
    ) -> Result<Vec<RadrootsOutboxReticulumPreviewEventRecord>, RadrootsOutboxError> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let outbox_event_ids =
            reticulum_preview_event_ids_pool(&self.pool, outbox_event_id, limit).await?;
        let mut records = Vec::with_capacity(outbox_event_ids.len());
        for outbox_event_id in outbox_event_ids {
            let Some(event) = self.get_event(outbox_event_id).await? else {
                continue;
            };
            let targets =
                reticulum_preview_targets_for_event_pool(&self.pool, outbox_event_id).await?;
            if !targets.is_empty() {
                records.push(RadrootsOutboxReticulumPreviewEventRecord { event, targets });
            }
        }
        Ok(records)
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
            "SELECT outbox_event_id, state, signed_event_json FROM outbox_event AS event WHERE ((event.state IN ('draft_queued', 'sign_retryable')) OR (event.state IN ('signed', 'publish_retryable') AND event.signed_event_json IS NOT NULL AND EXISTS (SELECT 1 FROM outbox_delivery_plan AS plan JOIN outbox_delivery_target AS target ON target.delivery_plan_id = plan.delivery_plan_id WHERE plan.outbox_event_id = event.outbox_event_id AND plan.status = 'queued' AND target.status IN ('pending', 'failed_retryable')))) AND event.next_attempt_after_ms <= ? AND (event.claim_token IS NULL OR event.claim_expires_at_ms <= ?) ORDER BY event.created_at_ms, event.outbox_event_id LIMIT 1",
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
        let active_delivery_plan_id = if claimed_state == RadrootsOutboxEventState::Publishing {
            Some(
                ready_delivery_plan_id_for_event_tx(&mut tx, outbox_event_id)
                    .await?
                    .ok_or(RadrootsOutboxError::MissingActiveDeliveryPlan { outbox_event_id })?,
            )
        } else {
            None
        };
        let changed = if let Some(delivery_plan_id) = active_delivery_plan_id {
            claim_publish_event_for_plan(
                &mut tx,
                outbox_event_id,
                delivery_plan_id,
                claim_owner.as_ref(),
                claim_token.as_ref(),
                claim_expires_at_ms,
                now_ms,
            )
            .await?
        } else {
            claim_event(
                &mut tx,
                ClaimEventUpdate {
                    outbox_event_id,
                    claimed_state,
                    active_delivery_plan_id: None,
                    claim_owner: claim_owner.as_ref(),
                    claim_token: claim_token.as_ref(),
                    claim_expires_at_ms,
                    now_ms,
                    suffix: "AND (claim_token IS NULL OR claim_expires_at_ms <= ?)",
                },
            )
            .await?
        };
        if changed.rows_affected() == 0 {
            tx.commit().await?;
            return Ok(None);
        }
        let claimed = claimed_event_from_tx(
            &mut tx,
            outbox_event_id,
            claimed_state,
            active_delivery_plan_id,
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
            "SELECT event.outbox_event_id, plan.delivery_plan_id FROM outbox_event AS event JOIN outbox_delivery_plan AS plan ON plan.outbox_event_id = event.outbox_event_id JOIN outbox_delivery_target AS target ON target.delivery_plan_id = plan.delivery_plan_id WHERE event.state IN ('signed', 'publish_retryable') AND event.signed_event_json IS NOT NULL AND event.next_attempt_after_ms <= ? AND (event.claim_token IS NULL OR event.claim_expires_at_ms <= ?) AND plan.status = 'queued' AND target.status IN ('pending', 'failed_retryable') GROUP BY event.outbox_event_id, plan.delivery_plan_id ORDER BY event.created_at_ms, event.outbox_event_id, plan.delivery_plan_id LIMIT 1",
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
        let active_delivery_plan_id: i64 = row.try_get("delivery_plan_id")?;
        let changed = claim_publish_event_for_plan(
            &mut tx,
            outbox_event_id,
            active_delivery_plan_id,
            claim_owner.as_ref(),
            claim_token.as_ref(),
            claim_expires_at_ms,
            now_ms,
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
            Some(active_delivery_plan_id),
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
        let Some(active_delivery_plan_id) =
            ready_delivery_plan_id_for_event_tx(&mut tx, outbox_event_id).await?
        else {
            tx.commit().await?;
            return Ok(None);
        };
        let changed = claim_publish_event_for_plan(
            &mut tx,
            outbox_event_id,
            active_delivery_plan_id,
            claim_owner.as_ref(),
            claim_token.as_ref(),
            claim_expires_at_ms,
            now_ms,
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
            Some(active_delivery_plan_id),
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
        signed_event: RadrootsSignedEvent,
        now_ms: i64,
    ) -> Result<RadrootsSignedEvent, RadrootsOutboxError> {
        let mut tx = self.pool.begin().await?;
        let record = event_by_id_tx(&mut tx, outbox_event_id).await?;
        let stored = record.claim_token.as_deref();
        if stored != Some(claim_token) {
            return Err(RadrootsOutboxError::ClaimTokenMismatch { outbox_event_id });
        }
        if signed_event.id_str() != record.event_id {
            return Err(RadrootsOutboxError::SignedEventIdMismatch {
                expected_event_id: record.event_id,
                actual_event_id: signed_event.id_str().to_owned(),
            });
        }
        let signed_event_json = signed_event_wire_json(&signed_event)?;
        let changed = sqlx::query(
            "UPDATE outbox_event SET signed_event_json = ?, raw_event_json = ?, state = ?, claim_token = NULL, claim_owner = NULL, claim_expires_at_ms = NULL, active_delivery_plan_id = NULL, last_error = NULL, updated_at_ms = ? WHERE outbox_event_id = ? AND claim_token = ?",
        )
        .bind(signed_event_json.as_str())
        .bind(signed_event.raw_json())
        .bind(RadrootsOutboxEventState::Signed.as_str())
        .bind(now_ms)
        .bind(outbox_event_id)
        .bind(claim_token)
        .execute(&mut *tx)
        .await?;
        if changed.rows_affected() == 0 {
            return Err(RadrootsOutboxError::ClaimTokenMismatch { outbox_event_id });
        }
        sync_signed_event_lifecycle(&mut tx, outbox_event_id, now_ms).await?;
        tx.commit().await?;
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
            "UPDATE outbox_event SET state = ?, claim_token = NULL, claim_owner = NULL, claim_expires_at_ms = NULL, active_delivery_plan_id = NULL, last_error = ?, next_attempt_after_ms = ?, updated_at_ms = ? WHERE outbox_event_id = ? AND claim_token = ?",
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
            "UPDATE outbox_event SET state = ?, claim_token = NULL, claim_owner = NULL, claim_expires_at_ms = NULL, active_delivery_plan_id = NULL, last_error = ?, next_attempt_after_ms = ?, updated_at_ms = ? WHERE outbox_event_id = ? AND claim_token = ?",
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
            "UPDATE outbox_event SET state = CASE WHEN state = 'signing' AND signed_event_json IS NULL THEN 'sign_retryable' WHEN state = 'signing' AND signed_event_json IS NOT NULL THEN 'signed' WHEN state = 'publishing' THEN 'publish_retryable' ELSE state END, claim_token = NULL, claim_owner = NULL, claim_expires_at_ms = NULL, active_delivery_plan_id = NULL, updated_at_ms = ? WHERE claim_token IS NOT NULL AND claim_expires_at_ms <= ? AND state IN ('signing', 'signed', 'publishing', 'preview_unavailable', 'deferred_until_implemented')",
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
        let observation = RadrootsTransportObservation::new(
            RadrootsTransportKind::Local,
            "local:outbox",
            RadrootsTransportObservationType::LocalImport,
            observed_at_ms,
        )?;
        let ingest = RadrootsEventIngest::new(signed_event.clone(), observed_at_ms)
            .with_observation(observation);
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

    pub async fn mark_delivery_target_delivered(
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
            RadrootsOutboxDeliveryTargetStatus::Delivered,
            None,
            attempted_at_ms,
        )
        .await
    }

    pub async fn mark_delivery_target_forwarded(
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
            RadrootsOutboxDeliveryTargetStatus::Forwarded,
            None,
            attempted_at_ms,
        )
        .await
    }

    pub async fn mark_delivery_target_stored_by_gateway(
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
            RadrootsOutboxDeliveryTargetStatus::StoredByGateway,
            None,
            attempted_at_ms,
        )
        .await
    }

    pub async fn mark_delivery_target_seen(
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
            RadrootsOutboxDeliveryTargetStatus::Seen,
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

    pub async fn mark_delivery_target_deferred_until_implemented(
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
            RadrootsOutboxDeliveryTargetStatus::DeferredUntilImplemented,
            Some(message),
            attempted_at_ms,
        )
        .await
    }

    pub async fn mark_delivery_target_preview_unavailable(
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
            RadrootsOutboxDeliveryTargetStatus::PreviewUnavailable,
            Some(message),
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
        if row.active_delivery_plan_id.is_none() {
            return Err(RadrootsOutboxError::MissingActiveDeliveryPlan { outbox_event_id });
        }
        let evaluation = evaluate_delivery_plans(&mut tx, outbox_event_id, now_ms).await?;
        let (event_state, operation_status, last_error, next_attempt_after_ms) =
            publish_lifecycle_from_plan_evaluation(
                &evaluation,
                retryable_error.as_ref(),
                terminal_error.as_ref(),
                next_attempt_after_ms,
                now_ms,
            );

        let changed = sqlx::query(
            "UPDATE outbox_event SET state = ?, claim_token = NULL, claim_owner = NULL, claim_expires_at_ms = NULL, active_delivery_plan_id = NULL, last_error = ?, next_attempt_after_ms = ?, updated_at_ms = ? WHERE outbox_event_id = ? AND claim_token = ?",
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

    pub async fn mark_active_delivery_plan_failed_terminal(
        &self,
        outbox_event_id: i64,
        claim_token: &str,
        error: impl AsRef<str>,
        now_ms: i64,
    ) -> Result<RadrootsOutboxEventState, RadrootsOutboxError> {
        let mut tx = self.pool.begin().await?;
        let row = claimed_event_identity_tx(&mut tx, outbox_event_id, claim_token).await?;
        let Some(active_delivery_plan_id) = row.active_delivery_plan_id else {
            return Err(RadrootsOutboxError::MissingActiveDeliveryPlan { outbox_event_id });
        };
        let targets = delivery_targets_for_plan_tx(&mut tx, active_delivery_plan_id).await?;
        let terminal_outcome_kind =
            outcome_kind_for_status(RadrootsOutboxDeliveryTargetStatus::FailedTerminal);
        for target in targets
            .iter()
            .filter(|target| target.status.is_ready_for_attempt())
        {
            sqlx::query(
                "UPDATE outbox_delivery_target SET status = ?, last_outcome_kind = ?, attempt_count = attempt_count + 1, last_attempt_at_ms = ?, completed_at_ms = ?, last_error = ? WHERE delivery_target_id = ? AND delivery_plan_id = ?",
            )
            .bind(RadrootsOutboxDeliveryTargetStatus::FailedTerminal.as_str())
            .bind(terminal_outcome_kind.as_str())
            .bind(now_ms)
            .bind(now_ms)
            .bind(error.as_ref())
            .bind(target.delivery_target_id)
            .bind(active_delivery_plan_id)
            .execute(&mut *tx)
            .await?;
            sqlx::query(
                "INSERT INTO outbox_delivery_attempt(delivery_plan_id, delivery_target_id, status, outcome_kind, attempted_at_ms, message) VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(active_delivery_plan_id)
            .bind(target.delivery_target_id)
            .bind(RadrootsOutboxDeliveryTargetStatus::FailedTerminal.as_str())
            .bind(terminal_outcome_kind.as_str())
            .bind(now_ms)
            .bind(error.as_ref())
            .execute(&mut *tx)
            .await?;
        }
        let evaluation = evaluate_delivery_plans(&mut tx, outbox_event_id, now_ms).await?;
        let (event_state, operation_status, last_error, next_attempt_after_ms) =
            publish_lifecycle_from_plan_evaluation(
                &evaluation,
                error.as_ref(),
                error.as_ref(),
                now_ms,
                now_ms,
            );
        let changed = sqlx::query(
            "UPDATE outbox_event SET state = ?, claim_token = NULL, claim_owner = NULL, claim_expires_at_ms = NULL, active_delivery_plan_id = NULL, last_error = ?, next_attempt_after_ms = ?, updated_at_ms = ? WHERE outbox_event_id = ? AND claim_token = ?",
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

    pub async fn cancel_claimed_event(
        &self,
        outbox_event_id: i64,
        claim_token: &str,
        now_ms: i64,
    ) -> Result<(), RadrootsOutboxError> {
        let mut tx = self.pool.begin().await?;
        let row = claimed_event_identity_tx(&mut tx, outbox_event_id, claim_token).await?;
        sqlx::query(
            "UPDATE outbox_delivery_plan SET status = ?, updated_at_ms = ? WHERE outbox_event_id = ?",
        )
        .bind(RadrootsOutboxDeliveryPlanStatus::Cancelled.as_str())
        .bind(now_ms)
        .bind(outbox_event_id)
        .execute(&mut *tx)
        .await?;
        let changed = sqlx::query(
            "UPDATE outbox_event SET state = ?, claim_token = NULL, claim_owner = NULL, claim_expires_at_ms = NULL, active_delivery_plan_id = NULL, last_error = ?, next_attempt_after_ms = ?, updated_at_ms = ? WHERE outbox_event_id = ? AND claim_token = ?",
        )
        .bind(RadrootsOutboxEventState::Cancelled.as_str())
        .bind(Option::<&str>::None)
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
        .bind(RadrootsOutboxOperationStatus::Cancelled.as_str())
        .bind(now_ms)
        .bind(row.operation_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
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
        let identity = claimed_event_identity_tx(&mut tx, outbox_event_id, claim_token).await?;
        let Some(active_delivery_plan_id) = identity.active_delivery_plan_id else {
            return Err(RadrootsOutboxError::MissingActiveDeliveryPlan { outbox_event_id });
        };
        let target_row = sqlx::query(
            "SELECT delivery_plan_id, status FROM outbox_delivery_target WHERE delivery_target_id = ? AND delivery_plan_id = ? AND delivery_plan_id IN (SELECT delivery_plan_id FROM outbox_delivery_plan WHERE outbox_event_id = ?)",
        )
        .bind(delivery_target_id)
        .bind(active_delivery_plan_id)
        .bind(outbox_event_id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(target_row) = target_row else {
            return Err(RadrootsOutboxError::DeliveryTargetNotFound(
                delivery_target_id,
            ));
        };
        let delivery_plan_id: i64 = target_row.try_get("delivery_plan_id")?;
        let current_status = RadrootsOutboxDeliveryTargetStatus::parse(
            target_row.try_get::<String, _>("status")?.as_str(),
        )?;
        if current_status.is_completed() {
            if current_status == status {
                tx.commit().await?;
                return Ok(());
            }
            if !delivery_target_status_can_advance(current_status, status) {
                return Err(RadrootsOutboxError::DeliveryTargetStatusConflict {
                    delivery_target_id,
                    current_status: current_status.as_str(),
                    requested_status: status.as_str(),
                });
            }
        }
        let completed_at_ms = status.is_completed().then_some(attempted_at_ms);
        let outcome_kind = outcome_kind_for_status(status);
        let changed = sqlx::query(
            "UPDATE outbox_delivery_target SET status = ?, last_outcome_kind = ?, attempt_count = attempt_count + 1, last_attempt_at_ms = ?, completed_at_ms = ?, last_error = ? WHERE delivery_target_id = ? AND delivery_plan_id = ?",
        )
        .bind(status.as_str())
        .bind(outcome_kind.as_str())
        .bind(attempted_at_ms)
        .bind(completed_at_ms)
        .bind(message)
        .bind(delivery_target_id)
        .bind(active_delivery_plan_id)
        .execute(&mut *tx)
        .await?;
        if changed.rows_affected() == 0 {
            return Err(RadrootsOutboxError::DeliveryTargetNotFound(
                delivery_target_id,
            ));
        }
        sqlx::query(
            "INSERT INTO outbox_delivery_attempt(delivery_plan_id, delivery_target_id, status, outcome_kind, attempted_at_ms, message) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(delivery_plan_id)
        .bind(delivery_target_id)
        .bind(status.as_str())
        .bind(outcome_kind.as_str())
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
    active_delivery_plan_id: Option<i64>,
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
    any_preview_unavailable: bool,
    any_deferred_until_implemented: bool,
}

fn publish_lifecycle_from_plan_evaluation<'a>(
    evaluation: &PlanEvaluation,
    retryable_error: &'a str,
    terminal_error: &'a str,
    next_attempt_after_ms: i64,
    now_ms: i64,
) -> (
    RadrootsOutboxEventState,
    Option<RadrootsOutboxOperationStatus>,
    Option<&'a str>,
    i64,
) {
    if evaluation.all_complete {
        (
            RadrootsOutboxEventState::Published,
            Some(RadrootsOutboxOperationStatus::Complete),
            None,
            now_ms,
        )
    } else if evaluation.any_ready {
        (
            RadrootsOutboxEventState::PublishRetryable,
            None,
            Some(retryable_error),
            next_attempt_after_ms,
        )
    } else if evaluation.any_failed_terminal {
        (
            RadrootsOutboxEventState::FailedTerminal,
            Some(RadrootsOutboxOperationStatus::FailedTerminal),
            Some(terminal_error),
            now_ms,
        )
    } else if evaluation.any_preview_unavailable {
        (
            RadrootsOutboxEventState::PreviewUnavailable,
            Some(RadrootsOutboxOperationStatus::PreviewUnavailable),
            None,
            now_ms,
        )
    } else if evaluation.any_deferred_until_implemented {
        (
            RadrootsOutboxEventState::DeferredUntilImplemented,
            Some(RadrootsOutboxOperationStatus::DeferredUntilImplemented),
            None,
            now_ms,
        )
    } else {
        (
            RadrootsOutboxEventState::FailedTerminal,
            Some(RadrootsOutboxOperationStatus::FailedTerminal),
            Some(terminal_error),
            now_ms,
        )
    }
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
async fn query_i64(pool: &SqlitePool, sql: &'static str) -> Result<i64, RadrootsOutboxError> {
    let row = sqlx::query(sql).fetch_one(pool).await?;
    Ok(row.try_get(0)?)
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn query_string(pool: &SqlitePool, sql: &'static str) -> Result<String, RadrootsOutboxError> {
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
    let targets = input.targets.clone();
    let is_no_wait = input.satisfaction_policy == RadrootsTransportSatisfactionPolicy::no_wait();
    if targets.is_empty() && !is_no_wait {
        return Err(RadrootsOutboxError::EmptyDeliveryTargets);
    }
    validate_unique_targets(&targets)?;
    if targets
        .iter()
        .any(|target| target.kind == RadrootsTransportKind::Proxy)
        && targets.len() != 1
    {
        return Err(RadrootsOutboxError::Transport(
            RadrootsTransportError::InvalidTransportKind,
        ));
    }
    let total_target_count = targets.len();
    let prepared_targets = targets
        .into_iter()
        .map(|target| {
            validate_delivery_target(&target)?;
            let initial_status =
                initial_delivery_target_status(&target, input.reticulum_preview_behavior);
            Ok(PreparedDeliveryTarget {
                target,
                initial_status,
            })
        })
        .collect::<Result<Vec<_>, RadrootsOutboxError>>()?;
    let satisfaction_policy = canonical_satisfaction_policy(&input.satisfaction_policy)?;
    validate_required_targets_belong_to_plan(&satisfaction_policy, &prepared_targets)?;
    let required_success_count = satisfaction_policy.required_target_count(
        satisfaction_target_count(&prepared_targets, total_target_count),
    )? as i64;
    let initial_status = initial_delivery_plan_status(&satisfaction_policy, &prepared_targets);
    let target_policy_fingerprint = target_policy_fingerprint(
        &satisfaction_policy,
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
        satisfaction_policy,
        required_success_count,
        delivery_plan_idempotency_digest,
        initial_status,
        targets: prepared_targets,
    })
}

fn canonical_satisfaction_policy(
    policy: &RadrootsTransportSatisfactionPolicy,
) -> Result<RadrootsTransportSatisfactionPolicy, RadrootsOutboxError> {
    match policy {
        RadrootsTransportSatisfactionPolicy::RequiredTargets { class, targets } => Ok(
            RadrootsTransportSatisfactionPolicy::required_targets(*class, targets.clone())?,
        ),
        RadrootsTransportSatisfactionPolicy::NoWait
        | RadrootsTransportSatisfactionPolicy::Any { .. }
        | RadrootsTransportSatisfactionPolicy::All { .. }
        | RadrootsTransportSatisfactionPolicy::Quorum { .. } => Ok(policy.clone()),
    }
}

fn validate_required_targets_belong_to_plan(
    policy: &RadrootsTransportSatisfactionPolicy,
    prepared_targets: &[PreparedDeliveryTarget],
) -> Result<(), RadrootsOutboxError> {
    let RadrootsTransportSatisfactionPolicy::RequiredTargets { targets, .. } = policy else {
        return Ok(());
    };
    if targets.iter().all(|required| {
        prepared_targets
            .iter()
            .any(|target| target.target.fingerprint == *required)
    }) {
        Ok(())
    } else {
        Err(RadrootsOutboxError::Transport(
            RadrootsTransportError::InvalidSatisfactionPolicy,
        ))
    }
}

fn satisfaction_target_count(
    prepared_targets: &[PreparedDeliveryTarget],
    total_target_count: usize,
) -> usize {
    let delivery_capable_target_count = prepared_targets
        .iter()
        .filter(|target| !target.initial_status.is_deferred_preview())
        .count();
    if delivery_capable_target_count == 0 {
        return total_target_count;
    }
    delivery_capable_target_count
}

fn validate_delivery_target(target: &RadrootsTransportTarget) -> Result<(), RadrootsOutboxError> {
    if target.kind == RadrootsTransportKind::Reticulum
        && target.uri.as_str() != RADROOTS_RETICULUM_PREVIEW_ENDPOINT_URI
    {
        return Err(RadrootsOutboxError::Transport(
            radroots_transport::RadrootsTransportError::InvalidTargetUri,
        ));
    }
    Ok(())
}

fn validate_unique_targets(targets: &[RadrootsTransportTarget]) -> Result<(), RadrootsOutboxError> {
    let mut fingerprints = BTreeSet::new();
    for target in targets {
        if !fingerprints.insert(target.fingerprint.as_str()) {
            return Err(RadrootsOutboxError::Transport(
                RadrootsTransportError::DuplicateTargetFingerprint,
            ));
        }
    }
    Ok(())
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
    satisfaction_policy: &RadrootsTransportSatisfactionPolicy,
    prepared_targets: &[PreparedDeliveryTarget],
) -> RadrootsOutboxDeliveryPlanStatus {
    if *satisfaction_policy == RadrootsTransportSatisfactionPolicy::no_wait() {
        return RadrootsOutboxDeliveryPlanStatus::Complete;
    }
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

    let satisfied_at_ms = (plan.initial_status == RadrootsOutboxDeliveryPlanStatus::Complete)
        .then_some(created_at_ms);
    let inserted = sqlx::query(
        "INSERT INTO outbox_delivery_plan(outbox_event_id, transport_profile_id, target_policy_fingerprint, target_policy_version, satisfaction_policy, required_success_count, delivery_plan_idempotency_digest, status, satisfied_at_ms, created_at_ms, updated_at_ms) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(outbox_event_id)
    .bind(plan.transport_profile_id.as_str())
    .bind(plan.target_policy_fingerprint.as_str())
    .bind(i64::from(plan.target_policy_version))
    .bind(satisfaction_policy_storage_value(&plan.satisfaction_policy))
    .bind(plan.required_success_count)
    .bind(plan.delivery_plan_idempotency_digest.as_str())
    .bind(plan.initial_status.as_str())
    .bind(satisfied_at_ms)
    .bind(created_at_ms)
    .bind(created_at_ms)
    .execute(&mut **tx)
    .await?;
    let delivery_plan_id = inserted.last_insert_rowid();
    for prepared_target in &plan.targets {
        let last_outcome_kind = prepared_target
            .initial_status
            .is_completed()
            .then(|| outcome_kind_for_status(prepared_target.initial_status));
        sqlx::query(
            "INSERT INTO outbox_delivery_target(delivery_plan_id, transport_kind, endpoint_uri, target_scope, target_label, endpoint_fingerprint, status, last_outcome_kind, attempt_count) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 0)",
        )
        .bind(delivery_plan_id)
        .bind(prepared_target.target.kind.canonical_label())
        .bind(prepared_target.target.uri.as_str())
        .bind(
            prepared_target
                .target
                .scope
                .as_ref()
                .map(RadrootsTransportMeshScopeId::as_str),
        )
        .bind(
            prepared_target
                .target
                .label
                .as_ref()
                .map(RadrootsTransportTargetLabel::as_str),
        )
        .bind(prepared_target.target.fingerprint.as_str())
        .bind(prepared_target.initial_status.as_str())
        .bind(last_outcome_kind.map(RadrootsTransportOutcomeKind::as_str))
        .execute(&mut **tx)
        .await?;
    }
    Ok(PlanInsertResult {
        status: RadrootsOutboxEnqueueStatus::Inserted,
        delivery_plan_id,
    })
}

async fn sync_signed_event_lifecycle(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    outbox_event_id: i64,
    now_ms: i64,
) -> Result<(), RadrootsOutboxError> {
    let row = sqlx::query(
        "SELECT operation_id, signed_event_json FROM outbox_event WHERE outbox_event_id = ?",
    )
    .bind(outbox_event_id)
    .fetch_one(&mut **tx)
    .await?;
    let signed_event_json: Option<String> = row.try_get("signed_event_json")?;
    if signed_event_json.is_none() {
        return Ok(());
    }
    let (event_state, operation_status) =
        signed_event_lifecycle_for_plans(tx, outbox_event_id).await?;
    sqlx::query(
        "UPDATE outbox_event SET state = ?, last_error = NULL, next_attempt_after_ms = ?, updated_at_ms = ? WHERE outbox_event_id = ?",
    )
    .bind(event_state.as_str())
    .bind(now_ms)
    .bind(now_ms)
    .bind(outbox_event_id)
    .execute(&mut **tx)
    .await?;
    sqlx::query(
        "UPDATE outbox_operations SET status = ?, updated_at_ms = ? WHERE operation_id = ?",
    )
    .bind(operation_status.as_str())
    .bind(now_ms)
    .bind(row.try_get::<i64, _>("operation_id")?)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn signed_event_lifecycle_for_plans(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    outbox_event_id: i64,
) -> Result<(RadrootsOutboxEventState, RadrootsOutboxOperationStatus), RadrootsOutboxError> {
    let plans = delivery_plans_for_tx(tx, outbox_event_id).await?;
    if plans.is_empty()
        || plans
            .iter()
            .any(|plan| plan.status == RadrootsOutboxDeliveryPlanStatus::Queued)
    {
        return Ok((
            RadrootsOutboxEventState::Signed,
            RadrootsOutboxOperationStatus::Queued,
        ));
    }
    if plans
        .iter()
        .all(|plan| plan.status == RadrootsOutboxDeliveryPlanStatus::Complete)
    {
        return Ok((
            RadrootsOutboxEventState::Published,
            RadrootsOutboxOperationStatus::Complete,
        ));
    }
    if plans
        .iter()
        .any(|plan| plan.status == RadrootsOutboxDeliveryPlanStatus::FailedTerminal)
    {
        return Ok((
            RadrootsOutboxEventState::FailedTerminal,
            RadrootsOutboxOperationStatus::FailedTerminal,
        ));
    }
    if plans
        .iter()
        .any(|plan| plan.status == RadrootsOutboxDeliveryPlanStatus::PreviewUnavailable)
    {
        return Ok((
            RadrootsOutboxEventState::PreviewUnavailable,
            RadrootsOutboxOperationStatus::PreviewUnavailable,
        ));
    }
    if plans
        .iter()
        .any(|plan| plan.status == RadrootsOutboxDeliveryPlanStatus::DeferredUntilImplemented)
    {
        return Ok((
            RadrootsOutboxEventState::DeferredUntilImplemented,
            RadrootsOutboxOperationStatus::DeferredUntilImplemented,
        ));
    }
    if plans
        .iter()
        .all(|plan| plan.status == RadrootsOutboxDeliveryPlanStatus::Cancelled)
    {
        return Ok((
            RadrootsOutboxEventState::Cancelled,
            RadrootsOutboxOperationStatus::Cancelled,
        ));
    }
    Ok((
        RadrootsOutboxEventState::Signed,
        RadrootsOutboxOperationStatus::Queued,
    ))
}

async fn ensure_event_signed(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    outbox_event_id: i64,
    signed_event: &RadrootsSignedEvent,
    event_store_inserted: bool,
    event_store_ingested_at_ms: i64,
) -> Result<(), RadrootsOutboxError> {
    let signed_event_json = signed_event_wire_json(signed_event)?;
    sqlx::query(
        "UPDATE outbox_event SET signed_event_json = ?, raw_event_json = ?, state = CASE WHEN state IN ('draft_queued', 'sign_retryable', 'signing') THEN ? ELSE state END, event_store_ingested = 1, event_store_inserted = ?, event_store_ingested_at_ms = ? WHERE outbox_event_id = ? AND signed_event_json IS NULL",
    )
    .bind(signed_event_json.as_str())
    .bind(signed_event.raw_json())
    .bind(RadrootsOutboxEventState::Signed.as_str())
    .bind(bool_i64(event_store_inserted))
    .bind(event_store_ingested_at_ms)
    .bind(outbox_event_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

struct ClaimEventUpdate<'a> {
    outbox_event_id: i64,
    claimed_state: RadrootsOutboxEventState,
    active_delivery_plan_id: Option<i64>,
    claim_owner: &'a str,
    claim_token: &'a str,
    claim_expires_at_ms: i64,
    now_ms: i64,
    suffix: &'a str,
}

async fn claim_event(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    update: ClaimEventUpdate<'_>,
) -> Result<SqliteQueryResult, RadrootsOutboxError> {
    let ClaimEventUpdate {
        outbox_event_id,
        claimed_state,
        active_delivery_plan_id,
        claim_owner,
        claim_token,
        claim_expires_at_ms,
        now_ms,
        suffix,
    } = update;
    let sql = format!(
        "UPDATE outbox_event SET state = ?, claim_token = ?, claim_owner = ?, claim_expires_at_ms = ?, active_delivery_plan_id = ?, attempt_count = attempt_count + 1, updated_at_ms = ? WHERE outbox_event_id = ? {suffix}"
    );
    let changed = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(claimed_state.as_str())
        .bind(claim_token)
        .bind(claim_owner)
        .bind(claim_expires_at_ms)
        .bind(active_delivery_plan_id)
        .bind(now_ms)
        .bind(outbox_event_id)
        .bind(now_ms)
        .execute(&mut **tx)
        .await?;
    Ok(changed)
}

async fn claim_publish_event_for_plan(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    outbox_event_id: i64,
    delivery_plan_id: i64,
    claim_owner: &str,
    claim_token: &str,
    claim_expires_at_ms: i64,
    now_ms: i64,
) -> Result<SqliteQueryResult, RadrootsOutboxError> {
    let changed = sqlx::query(
        "UPDATE outbox_event SET state = ?, claim_token = ?, claim_owner = ?, claim_expires_at_ms = ?, active_delivery_plan_id = ?, attempt_count = attempt_count + 1, updated_at_ms = ? WHERE outbox_event_id = ? AND state IN ('signed', 'publish_retryable') AND signed_event_json IS NOT NULL AND next_attempt_after_ms <= ? AND (claim_token IS NULL OR claim_expires_at_ms <= ?) AND EXISTS (SELECT 1 FROM outbox_delivery_plan AS plan JOIN outbox_delivery_target AS target ON target.delivery_plan_id = plan.delivery_plan_id WHERE plan.outbox_event_id = outbox_event.outbox_event_id AND plan.delivery_plan_id = ? AND plan.status = 'queued' AND target.status IN ('pending', 'failed_retryable'))",
    )
    .bind(RadrootsOutboxEventState::Publishing.as_str())
    .bind(claim_token)
    .bind(claim_owner)
    .bind(claim_expires_at_ms)
    .bind(delivery_plan_id)
    .bind(now_ms)
    .bind(outbox_event_id)
    .bind(now_ms)
    .bind(now_ms)
    .bind(delivery_plan_id)
    .execute(&mut **tx)
    .await?;
    Ok(changed)
}

async fn claimed_event_from_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    outbox_event_id: i64,
    claimed_state: RadrootsOutboxEventState,
    active_delivery_plan_id: Option<i64>,
    claim_token: &str,
) -> Result<RadrootsOutboxClaimedEvent, RadrootsOutboxError> {
    let record = event_by_id_tx(tx, outbox_event_id).await?;
    let delivery_targets = match (claimed_state, active_delivery_plan_id) {
        (RadrootsOutboxEventState::Publishing, Some(delivery_plan_id)) => {
            ready_delivery_targets_for_plan_tx(tx, delivery_plan_id).await?
        }
        (RadrootsOutboxEventState::Publishing, None) => {
            return Err(RadrootsOutboxError::MissingActiveDeliveryPlan { outbox_event_id });
        }
        _ => delivery_targets_for_event_tx(tx, outbox_event_id).await?,
    };
    Ok(RadrootsOutboxClaimedEvent {
        outbox_event_id: record.outbox_event_id,
        operation_id: record.operation_id,
        expected_event_id: record.event_id,
        attempt_count: record.attempt_count,
        state: claimed_state,
        claim_token: claim_token.to_owned(),
        active_delivery_plan_id,
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
        "SELECT outbox_event_id, operation_id, event_id, expected_pubkey, draft_json, signed_event_json, raw_event_json, state, attempt_count, claim_token, claim_owner, claim_expires_at_ms, active_delivery_plan_id, next_attempt_after_ms, last_error, event_store_ingested, event_store_inserted, event_store_ingested_at_ms, created_at_ms, updated_at_ms FROM outbox_event WHERE outbox_event_id = ?",
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
        sqlx::query("SELECT operation_id, claim_token, active_delivery_plan_id FROM outbox_event WHERE outbox_event_id = ?")
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
        active_delivery_plan_id: row.try_get("active_delivery_plan_id")?,
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
        "SELECT target.delivery_target_id, target.delivery_plan_id, target.transport_kind, target.endpoint_uri, target.target_scope, target.target_label, target.endpoint_fingerprint, target.status, target.last_outcome_kind, target.attempt_count, target.last_attempt_at_ms, target.completed_at_ms, target.last_error FROM outbox_delivery_target AS target JOIN outbox_delivery_plan AS plan ON plan.delivery_plan_id = target.delivery_plan_id WHERE plan.outbox_event_id = ? ORDER BY target.delivery_plan_id, target.delivery_target_id",
    )
    .bind(outbox_event_id)
    .fetch_all(pool)
    .await?;
    rows.into_iter().map(delivery_target_from_row).collect()
}

async fn reticulum_preview_event_ids_pool(
    pool: &SqlitePool,
    outbox_event_id: Option<i64>,
    limit: usize,
) -> Result<Vec<i64>, RadrootsOutboxError> {
    let limit = i64::try_from(limit).map_err(|_| RadrootsOutboxError::IntegerRange {
        field: "reticulum_preview_events.limit",
        value: i64::MAX,
    })?;
    let mut query = String::from(
        "SELECT event.outbox_event_id FROM outbox_event AS event WHERE event.signed_event_json IS NOT NULL AND EXISTS (SELECT 1 FROM outbox_delivery_plan AS plan JOIN outbox_delivery_target AS target ON target.delivery_plan_id = plan.delivery_plan_id WHERE plan.outbox_event_id = event.outbox_event_id AND plan.status IN ('queued', 'preview_unavailable', 'deferred_until_implemented') AND target.transport_kind = 'reticulum' AND target.status IN ('pending', 'failed_retryable', 'preview_unavailable', 'deferred_until_implemented'))",
    );
    if outbox_event_id.is_some() {
        query.push_str(" AND event.outbox_event_id = ?");
    }
    query.push_str(" ORDER BY event.created_at_ms, event.outbox_event_id LIMIT ?");
    let mut query = sqlx::query(sqlx::AssertSqlSafe(query));
    if let Some(outbox_event_id) = outbox_event_id {
        query = query.bind(outbox_event_id);
    }
    let rows = query.bind(limit).fetch_all(pool).await?;
    rows.into_iter()
        .map(|row| row.try_get("outbox_event_id"))
        .collect::<Result<Vec<_>, sqlx::Error>>()
        .map_err(Into::into)
}

async fn reticulum_preview_targets_for_event_pool(
    pool: &SqlitePool,
    outbox_event_id: i64,
) -> Result<Vec<RadrootsOutboxDeliveryTargetRecord>, RadrootsOutboxError> {
    let rows = sqlx::query(
        "SELECT target.delivery_target_id, target.delivery_plan_id, target.transport_kind, target.endpoint_uri, target.target_scope, target.target_label, target.endpoint_fingerprint, target.status, target.last_outcome_kind, target.attempt_count, target.last_attempt_at_ms, target.completed_at_ms, target.last_error FROM outbox_delivery_target AS target JOIN outbox_delivery_plan AS plan ON plan.delivery_plan_id = target.delivery_plan_id WHERE plan.outbox_event_id = ? AND plan.status IN ('queued', 'preview_unavailable', 'deferred_until_implemented') AND target.transport_kind = 'reticulum' AND target.status IN ('pending', 'failed_retryable', 'preview_unavailable', 'deferred_until_implemented') ORDER BY target.delivery_plan_id, target.delivery_target_id",
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
        "SELECT target.delivery_target_id, target.delivery_plan_id, target.transport_kind, target.endpoint_uri, target.target_scope, target.target_label, target.endpoint_fingerprint, target.status, target.last_outcome_kind, target.attempt_count, target.last_attempt_at_ms, target.completed_at_ms, target.last_error FROM outbox_delivery_target AS target JOIN outbox_delivery_plan AS plan ON plan.delivery_plan_id = target.delivery_plan_id WHERE plan.outbox_event_id = ? ORDER BY target.delivery_plan_id, target.delivery_target_id",
    )
    .bind(outbox_event_id)
    .fetch_all(&mut **tx)
    .await?;
    rows.into_iter().map(delivery_target_from_row).collect()
}

async fn ready_delivery_plan_id_for_event_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    outbox_event_id: i64,
) -> Result<Option<i64>, RadrootsOutboxError> {
    let row = sqlx::query(
        "SELECT plan.delivery_plan_id FROM outbox_delivery_plan AS plan JOIN outbox_delivery_target AS target ON target.delivery_plan_id = plan.delivery_plan_id WHERE plan.outbox_event_id = ? AND plan.status = 'queued' AND target.status IN ('pending', 'failed_retryable') GROUP BY plan.delivery_plan_id ORDER BY plan.delivery_plan_id LIMIT 1",
    )
    .bind(outbox_event_id)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(row.map(|row| row.try_get("delivery_plan_id")).transpose()?)
}

async fn ready_delivery_targets_for_plan_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    delivery_plan_id: i64,
) -> Result<Vec<RadrootsOutboxDeliveryTargetRecord>, RadrootsOutboxError> {
    let rows = sqlx::query(
        "SELECT delivery_target_id, delivery_plan_id, transport_kind, endpoint_uri, target_scope, target_label, endpoint_fingerprint, status, last_outcome_kind, attempt_count, last_attempt_at_ms, completed_at_ms, last_error FROM outbox_delivery_target WHERE delivery_plan_id = ? AND status IN ('pending', 'failed_retryable') ORDER BY delivery_target_id",
    )
    .bind(delivery_plan_id)
    .fetch_all(&mut **tx)
    .await?;
    rows.into_iter().map(delivery_target_from_row).collect()
}

async fn delivery_targets_for_plan_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    delivery_plan_id: i64,
) -> Result<Vec<RadrootsOutboxDeliveryTargetRecord>, RadrootsOutboxError> {
    let rows = sqlx::query(
        "SELECT delivery_target_id, delivery_plan_id, transport_kind, endpoint_uri, target_scope, target_label, endpoint_fingerprint, status, last_outcome_kind, attempt_count, last_attempt_at_ms, completed_at_ms, last_error FROM outbox_delivery_target WHERE delivery_plan_id = ? ORDER BY delivery_target_id",
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
        "SELECT delivery_attempt_id, delivery_plan_id, delivery_target_id, status, outcome_kind, attempted_at_ms, message FROM outbox_delivery_attempt WHERE delivery_target_id = ? ORDER BY delivery_attempt_id",
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
    let mut any_preview_unavailable = false;
    let mut any_deferred_until_implemented = false;
    for plan in plans {
        let targets = delivery_targets_for_plan_tx(tx, plan.delivery_plan_id).await?;
        let satisfied_count = outbox_satisfied_target_count(&plan.satisfaction_policy, &targets);
        let status_targets = delivery_plan_status_targets(&plan.satisfaction_policy, &targets);
        let ready_count = status_targets
            .iter()
            .filter(|target| target.status.is_ready_for_attempt())
            .count();
        let deferred_count = status_targets
            .iter()
            .filter(|target| target.status.is_deferred_preview())
            .count();
        let preview_unavailable_count = status_targets
            .iter()
            .filter(|target| {
                target.status == RadrootsOutboxDeliveryTargetStatus::PreviewUnavailable
            })
            .count();
        let terminal_failure_count = status_targets
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
        if plan_status == RadrootsOutboxDeliveryPlanStatus::PreviewUnavailable {
            any_preview_unavailable = true;
        }
        if plan_status == RadrootsOutboxDeliveryPlanStatus::DeferredUntilImplemented {
            any_deferred_until_implemented = true;
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
        any_preview_unavailable,
        any_deferred_until_implemented,
    })
}

fn delivery_plan_status_targets<'a>(
    policy: &RadrootsTransportSatisfactionPolicy,
    targets: &'a [RadrootsOutboxDeliveryTargetRecord],
) -> Vec<&'a RadrootsOutboxDeliveryTargetRecord> {
    match policy {
        RadrootsTransportSatisfactionPolicy::RequiredTargets {
            targets: required_targets,
            ..
        } => targets
            .iter()
            .filter(|target| required_targets.contains(&target.endpoint_fingerprint))
            .collect(),
        RadrootsTransportSatisfactionPolicy::NoWait
        | RadrootsTransportSatisfactionPolicy::Any { .. }
        | RadrootsTransportSatisfactionPolicy::All { .. }
        | RadrootsTransportSatisfactionPolicy::Quorum { .. } => targets.iter().collect(),
    }
}

fn outbox_satisfied_target_count(
    policy: &RadrootsTransportSatisfactionPolicy,
    targets: &[RadrootsOutboxDeliveryTargetRecord],
) -> i64 {
    match policy {
        RadrootsTransportSatisfactionPolicy::NoWait => 0,
        RadrootsTransportSatisfactionPolicy::Any { class }
        | RadrootsTransportSatisfactionPolicy::All { class }
        | RadrootsTransportSatisfactionPolicy::Quorum { class, .. } => targets
            .iter()
            .filter(|target| target.status.counts_as_transport_satisfaction(*class))
            .count() as i64,
        RadrootsTransportSatisfactionPolicy::RequiredTargets {
            class,
            targets: required_targets,
        } => required_targets
            .iter()
            .filter(|required| {
                targets.iter().any(|target| {
                    target.endpoint_fingerprint == **required
                        && target.status.counts_as_transport_satisfaction(*class)
                })
            })
            .count() as i64,
    }
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
    let outbox_event_id = row.try_get("outbox_event_id")?;
    let event_id = row.try_get::<String, _>("event_id")?;
    let draft: RadrootsEventDraft =
        serde_json::from_str(row.try_get::<String, _>("draft_json")?.as_str())?;
    let signed_event = signed_event_from_storage(
        outbox_event_id,
        row.try_get("signed_event_json")?,
        row.try_get("raw_event_json")?,
    )?;
    if let Some(signed_event) = signed_event.as_ref()
        && signed_event.id_str() != event_id
    {
        return Err(RadrootsOutboxError::SignedEventIdMismatch {
            expected_event_id: event_id,
            actual_event_id: signed_event.id_str().to_owned(),
        });
    }
    let state = RadrootsOutboxEventState::parse(row.try_get::<String, _>("state")?.as_str())?;
    Ok(RadrootsOutboxEventRecord {
        outbox_event_id,
        operation_id: row.try_get("operation_id")?,
        event_id,
        expected_pubkey: row.try_get("expected_pubkey")?,
        draft,
        signed_event,
        raw_event_json: row.try_get("raw_event_json")?,
        state,
        attempt_count: row.try_get("attempt_count")?,
        claim_token: row.try_get("claim_token")?,
        claim_owner: row.try_get("claim_owner")?,
        claim_expires_at_ms: row.try_get("claim_expires_at_ms")?,
        active_delivery_plan_id: row.try_get("active_delivery_plan_id")?,
        next_attempt_after_ms: row.try_get("next_attempt_after_ms")?,
        last_error: row.try_get("last_error")?,
        event_store_ingested: row.try_get::<i64, _>("event_store_ingested")? != 0,
        event_store_inserted: row.try_get::<i64, _>("event_store_inserted")? != 0,
        event_store_ingested_at_ms: row.try_get("event_store_ingested_at_ms")?,
        created_at_ms: row.try_get("created_at_ms")?,
        updated_at_ms: row.try_get("updated_at_ms")?,
    })
}

fn signed_event_from_storage(
    outbox_event_id: i64,
    signed_event_json: Option<String>,
    raw_event_json: Option<String>,
) -> Result<Option<RadrootsSignedEvent>, RadrootsOutboxError> {
    match (signed_event_json, raw_event_json) {
        (None, None) => Ok(None),
        (Some(_), None) => Err(RadrootsOutboxError::StoredSignedEventMissingRawJson(
            outbox_event_id,
        )),
        (None, Some(_)) => Err(RadrootsOutboxError::StoredRawEventMissingSignedEvent(
            outbox_event_id,
        )),
        (Some(signed_json), Some(raw_json)) => {
            let wire = RadrootsNip01EventWire::parse_json(signed_json.as_str())?;
            RadrootsSignedEvent::from_wire_verified_id(wire, raw_json)
                .map(Some)
                .map_err(Into::into)
        }
    }
}

fn signed_event_wire_json(
    signed_event: &RadrootsSignedEvent,
) -> Result<String, RadrootsOutboxError> {
    serde_json::to_string(signed_event.wire()).map_err(Into::into)
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
    let target_scope = row
        .try_get::<Option<String>, _>("target_scope")?
        .map(RadrootsTransportMeshScopeId::parse)
        .transpose()?;
    let target_label = row
        .try_get::<Option<String>, _>("target_label")?
        .map(RadrootsTransportTargetLabel::parse)
        .transpose()?;
    let endpoint_fingerprint = RadrootsTransportTargetFingerprint::parse(
        row.try_get::<String, _>("endpoint_fingerprint")?,
    )?;
    let status =
        RadrootsOutboxDeliveryTargetStatus::parse(row.try_get::<String, _>("status")?.as_str())?;
    let last_outcome_kind = row
        .try_get::<Option<String>, _>("last_outcome_kind")?
        .map(|value| {
            parse_transport_outcome_kind(value.as_str(), "outbox_delivery_target.last_outcome_kind")
        })
        .transpose()?;
    Ok(RadrootsOutboxDeliveryTargetRecord {
        delivery_target_id: row.try_get("delivery_target_id")?,
        delivery_plan_id: row.try_get("delivery_plan_id")?,
        transport_kind,
        endpoint_uri,
        target_scope,
        target_label,
        endpoint_fingerprint,
        status,
        last_outcome_kind,
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
    let outcome_kind = parse_transport_outcome_kind(
        row.try_get::<String, _>("outcome_kind")?.as_str(),
        "outbox_delivery_attempt.outcome_kind",
    )?;
    Ok(RadrootsOutboxDeliveryAttemptRecord {
        delivery_attempt_id: row.try_get("delivery_attempt_id")?,
        delivery_plan_id: row.try_get("delivery_plan_id")?,
        delivery_target_id: row.try_get("delivery_target_id")?,
        status,
        outcome_kind,
        attempted_at_ms: row.try_get("attempted_at_ms")?,
        message: row.try_get("message")?,
    })
}

fn outcome_kind_for_status(
    status: RadrootsOutboxDeliveryTargetStatus,
) -> RadrootsTransportOutcomeKind {
    match status {
        RadrootsOutboxDeliveryTargetStatus::Pending => {
            RadrootsTransportOutcomeKind::TransportUnavailable
        }
        RadrootsOutboxDeliveryTargetStatus::Accepted => RadrootsTransportOutcomeKind::Accepted,
        RadrootsOutboxDeliveryTargetStatus::Delivered => RadrootsTransportOutcomeKind::Delivered,
        RadrootsOutboxDeliveryTargetStatus::Forwarded => RadrootsTransportOutcomeKind::Forwarded,
        RadrootsOutboxDeliveryTargetStatus::StoredByGateway => {
            RadrootsTransportOutcomeKind::StoredByGateway
        }
        RadrootsOutboxDeliveryTargetStatus::Seen => RadrootsTransportOutcomeKind::Seen,
        RadrootsOutboxDeliveryTargetStatus::DeferredUntilImplemented => {
            RadrootsTransportOutcomeKind::DeferredUntilImplemented
        }
        RadrootsOutboxDeliveryTargetStatus::PreviewUnavailable => {
            RadrootsTransportOutcomeKind::TransportUnavailable
        }
        RadrootsOutboxDeliveryTargetStatus::SkippedPolicyDenied => {
            RadrootsTransportOutcomeKind::PolicyDenied
        }
        RadrootsOutboxDeliveryTargetStatus::FailedRetryable => {
            RadrootsTransportOutcomeKind::TransportUnavailable
        }
        RadrootsOutboxDeliveryTargetStatus::FailedTerminal => {
            RadrootsTransportOutcomeKind::Rejected
        }
    }
}

fn delivery_target_status_can_advance(
    current: RadrootsOutboxDeliveryTargetStatus,
    requested: RadrootsOutboxDeliveryTargetStatus,
) -> bool {
    matches!(
        (current, requested),
        (
            RadrootsOutboxDeliveryTargetStatus::Accepted,
            RadrootsOutboxDeliveryTargetStatus::Forwarded
                | RadrootsOutboxDeliveryTargetStatus::StoredByGateway
                | RadrootsOutboxDeliveryTargetStatus::Seen
                | RadrootsOutboxDeliveryTargetStatus::Delivered
        ) | (
            RadrootsOutboxDeliveryTargetStatus::Forwarded
                | RadrootsOutboxDeliveryTargetStatus::StoredByGateway
                | RadrootsOutboxDeliveryTargetStatus::Seen,
            RadrootsOutboxDeliveryTargetStatus::Delivered
        )
    )
}

fn parse_transport_outcome_kind(
    value: &str,
    field: &'static str,
) -> Result<RadrootsTransportOutcomeKind, RadrootsOutboxError> {
    match value {
        "accepted" => Ok(RadrootsTransportOutcomeKind::Accepted),
        "duplicate_accepted" => Ok(RadrootsTransportOutcomeKind::DuplicateAccepted),
        "delivered" => Ok(RadrootsTransportOutcomeKind::Delivered),
        "forwarded" => Ok(RadrootsTransportOutcomeKind::Forwarded),
        "stored_by_gateway" => Ok(RadrootsTransportOutcomeKind::StoredByGateway),
        "seen" => Ok(RadrootsTransportOutcomeKind::Seen),
        "deferred_until_implemented" => Ok(RadrootsTransportOutcomeKind::DeferredUntilImplemented),
        "rejected" => Ok(RadrootsTransportOutcomeKind::Rejected),
        "route_unavailable" => Ok(RadrootsTransportOutcomeKind::RouteUnavailable),
        "payload_too_large" => Ok(RadrootsTransportOutcomeKind::PayloadTooLarge),
        "policy_denied" => Ok(RadrootsTransportOutcomeKind::PolicyDenied),
        "timeout" => Ok(RadrootsTransportOutcomeKind::Timeout),
        "connection_failed" => Ok(RadrootsTransportOutcomeKind::ConnectionFailed),
        "transport_unavailable" => Ok(RadrootsTransportOutcomeKind::TransportUnavailable),
        _ => Err(RadrootsOutboxError::InvalidStoredEnum {
            field,
            value: value.to_owned(),
        }),
    }
}

#[derive(Serialize)]
struct OperationDigestInput<'a> {
    operation_kind: &'a str,
    expected_pubkey: &'a str,
    draft: &'a RadrootsEventDraft,
}

fn operation_idempotency_digest(
    operation_kind: &str,
    expected_pubkey: &str,
    draft: &RadrootsEventDraft,
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
        RadrootsTransportSatisfactionPolicy::NoWait => "no_wait".to_owned(),
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
        RadrootsTransportSatisfactionPolicy::RequiredTargets { class, targets } => {
            let mut fingerprints = targets
                .iter()
                .map(RadrootsTransportTargetFingerprint::as_str)
                .collect::<Vec<_>>();
            fingerprints.sort();
            let fingerprints = fingerprints.join(",");
            format!(
                "required_{}:{fingerprints}",
                satisfaction_class_storage_value(*class)
            )
        }
    }
}

fn satisfaction_class_storage_value(class: RadrootsTransportSatisfactionClass) -> &'static str {
    match class {
        RadrootsTransportSatisfactionClass::Accepted => "accepted",
        RadrootsTransportSatisfactionClass::Forwarded => "forwarded",
        RadrootsTransportSatisfactionClass::Stored => "stored",
        RadrootsTransportSatisfactionClass::Seen => "seen",
        RadrootsTransportSatisfactionClass::Delivered => "delivered",
        RadrootsTransportSatisfactionClass::DurableOrObserved => "durable_or_observed",
    }
}

fn parse_satisfaction_class_storage_value(
    value: &str,
) -> Option<RadrootsTransportSatisfactionClass> {
    match value {
        "accepted" => Some(RadrootsTransportSatisfactionClass::Accepted),
        "forwarded" => Some(RadrootsTransportSatisfactionClass::Forwarded),
        "stored" => Some(RadrootsTransportSatisfactionClass::Stored),
        "seen" => Some(RadrootsTransportSatisfactionClass::Seen),
        "delivered" => Some(RadrootsTransportSatisfactionClass::Delivered),
        "durable_or_observed" => Some(RadrootsTransportSatisfactionClass::DurableOrObserved),
        _ => None,
    }
}

fn parse_satisfaction_policy(
    value: &str,
    required_success_count: i64,
) -> Result<RadrootsTransportSatisfactionPolicy, RadrootsOutboxError> {
    if value == "no_wait" && required_success_count == 0 {
        return Ok(RadrootsTransportSatisfactionPolicy::no_wait());
    }
    if let Some(class) = value
        .strip_prefix("all_")
        .and_then(parse_satisfaction_class_storage_value)
    {
        return Ok(RadrootsTransportSatisfactionPolicy::All { class });
    }
    if let Some(class) = value
        .strip_prefix("any_")
        .and_then(parse_satisfaction_class_storage_value)
    {
        return Ok(RadrootsTransportSatisfactionPolicy::Any { class });
    }
    if let Some((class_label, threshold)) = value
        .strip_prefix("quorum_")
        .and_then(|stored| stored.split_once(':'))
        && threshold == required_success_count.to_string()
        && let Some(class) = parse_satisfaction_class_storage_value(class_label)
    {
        return Ok(RadrootsTransportSatisfactionPolicy::Quorum {
            class,
            threshold: required_count_u16(required_success_count)?,
        });
    }
    if let Some((class_label, fingerprints)) = value
        .strip_prefix("required_")
        .and_then(|stored| stored.split_once(':'))
        && let Some(class) = parse_satisfaction_class_storage_value(class_label)
    {
        return parse_required_target_policy(value, fingerprints, class, required_success_count);
    }
    Err(RadrootsOutboxError::InvalidStoredEnum {
        field: "outbox_delivery_plan.satisfaction_policy",
        value: value.to_owned(),
    })
}

fn parse_required_target_policy(
    stored: &str,
    fingerprints: &str,
    class: RadrootsTransportSatisfactionClass,
    required_success_count: i64,
) -> Result<RadrootsTransportSatisfactionPolicy, RadrootsOutboxError> {
    let targets = fingerprints
        .split(',')
        .map(RadrootsTransportTargetFingerprint::parse)
        .collect::<Result<Vec<_>, _>>()?;
    if required_success_count
        != i64::try_from(targets.len()).map_err(|_| RadrootsOutboxError::IntegerRange {
            field: "required_success_count",
            value: required_success_count,
        })?
    {
        return Err(RadrootsOutboxError::InvalidStoredEnum {
            field: "outbox_delivery_plan.satisfaction_policy",
            value: stored.to_owned(),
        });
    }
    Ok(RadrootsTransportSatisfactionPolicy::required_targets(
        class, targets,
    )?)
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
    use radroots_event::kinds::KIND_POST;
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

    fn post_draft(expected_pubkey: &str, content: &str) -> RadrootsEventDraft {
        RadrootsEventDraft::new(
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
        RadrootsTransportTarget::nostr_relay(uri).expect("nostr target")
    }

    fn scoped_nostr_target(uri: &str, scope: &str, label: &str) -> RadrootsTransportTarget {
        RadrootsTransportTarget::nostr_relay_with_metadata(
            uri,
            Some(RadrootsTransportMeshScopeId::parse(scope).expect("target scope")),
            Some(RadrootsTransportTargetLabel::parse(label).expect("target label")),
        )
        .expect("scoped nostr target")
    }

    fn scoped_reticulum_target(scope: &str, label: &str) -> RadrootsTransportTarget {
        RadrootsTransportTarget::reticulum_preview_with_metadata(
            Some(RadrootsTransportMeshScopeId::parse(scope).expect("target scope")),
            Some(RadrootsTransportTargetLabel::parse(label).expect("target label")),
        )
        .expect("scoped reticulum target")
    }

    fn reticulum_target(uri: &str) -> RadrootsTransportTarget {
        assert_eq!(uri, RADROOTS_RETICULUM_PREVIEW_ENDPOINT_URI);
        RadrootsTransportTarget::reticulum_preview().expect("reticulum target")
    }

    fn proxy_target(uri: &str) -> RadrootsTransportTarget {
        RadrootsTransportTarget::proxy(uri).expect("proxy target")
    }

    #[test]
    fn required_target_satisfaction_policy_storage_round_trips() {
        let first = nostr_target("wss://required-one.example");
        let second = nostr_target("wss://required-two.example");
        let policy = RadrootsTransportSatisfactionPolicy::required_targets(
            RadrootsTransportSatisfactionClass::Delivered,
            vec![first.fingerprint.clone(), second.fingerprint.clone()],
        )
        .expect("required targets policy");

        let stored = satisfaction_policy_storage_value(&policy);
        let mut fingerprints = [first.fingerprint.as_str(), second.fingerprint.as_str()];
        fingerprints.sort();
        assert_eq!(
            stored,
            format!("required_delivered:{},{}", fingerprints[0], fingerprints[1])
        );
        assert_eq!(
            parse_satisfaction_policy(stored.as_str(), 2).expect("parse required targets"),
            policy
        );
        assert!(matches!(
            parse_satisfaction_policy(stored.as_str(), 1),
            Err(RadrootsOutboxError::InvalidStoredEnum { .. })
        ));
    }

    #[test]
    fn satisfaction_policy_storage_round_trips_all_transport_classes() {
        for (policy, stored, required_count) in [
            (
                RadrootsTransportSatisfactionPolicy::any_forwarded(),
                "any_forwarded",
                1,
            ),
            (
                RadrootsTransportSatisfactionPolicy::all_stored(),
                "all_stored",
                2,
            ),
            (
                RadrootsTransportSatisfactionPolicy::quorum_seen(2),
                "quorum_seen:2",
                2,
            ),
            (
                RadrootsTransportSatisfactionPolicy::any_durable_or_observed(),
                "any_durable_or_observed",
                1,
            ),
            (
                RadrootsTransportSatisfactionPolicy::quorum_delivered(2),
                "quorum_delivered:2",
                2,
            ),
        ] {
            assert_eq!(satisfaction_policy_storage_value(&policy), stored);
            assert_eq!(
                parse_satisfaction_policy(stored, required_count).expect("parse policy"),
                policy
            );
        }
    }

    #[test]
    fn required_target_policy_idempotency_is_order_independent() {
        let first = nostr_target("wss://required-one.example");
        let second = nostr_target("wss://required-two.example");
        let first_policy = RadrootsTransportSatisfactionPolicy::RequiredTargets {
            class: RadrootsTransportSatisfactionClass::Accepted,
            targets: vec![second.fingerprint.clone(), first.fingerprint.clone()],
        };
        let second_policy = RadrootsTransportSatisfactionPolicy::RequiredTargets {
            class: RadrootsTransportSatisfactionClass::Accepted,
            targets: vec![first.fingerprint.clone(), second.fingerprint.clone()],
        };
        let targets = vec![first, second];

        let first_prepared = prepare_delivery_plan(
            "event-required-target-order",
            &RadrootsOutboxDeliveryPlanInput::new(
                "transport.nostr.local",
                1,
                first_policy,
                targets.clone(),
            ),
        )
        .expect("first delivery plan");
        let second_prepared = prepare_delivery_plan(
            "event-required-target-order",
            &RadrootsOutboxDeliveryPlanInput::new(
                "transport.nostr.local",
                1,
                second_policy,
                targets,
            ),
        )
        .expect("second delivery plan");

        assert_eq!(
            first_prepared.satisfaction_policy,
            second_prepared.satisfaction_policy
        );
        assert_eq!(first_prepared.required_success_count, 2);
        assert_eq!(
            first_prepared.target_policy_fingerprint,
            second_prepared.target_policy_fingerprint
        );
        assert_eq!(
            first_prepared.delivery_plan_idempotency_digest,
            second_prepared.delivery_plan_idempotency_digest
        );
        assert_eq!(
            satisfaction_policy_storage_value(&first_prepared.satisfaction_policy),
            satisfaction_policy_storage_value(&second_prepared.satisfaction_policy)
        );
    }

    fn malformed_reticulum_target(uri: &str) -> RadrootsTransportTarget {
        let endpoint_uri = RadrootsTransportTargetUri::parse(uri).expect("target uri");
        let endpoint_fingerprint = RadrootsTransportTargetFingerprint::from_target(
            &RadrootsTransportKind::Reticulum,
            &endpoint_uri,
            None,
        );
        RadrootsTransportTarget {
            kind: RadrootsTransportKind::Reticulum,
            uri: endpoint_uri,
            scope: None,
            label: None,
            fingerprint: endpoint_fingerprint,
        }
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
        draft: RadrootsEventDraft,
        created_at_ms: i64,
    ) -> RadrootsOutboxOperationInput {
        RadrootsOutboxOperationInput::new(
            "publish_post",
            draft,
            delivery_plan(vec![
                nostr_target(NOSTR_PRIMARY_WSS),
                nostr_target(NOSTR_SECONDARY_WSS),
            ]),
            created_at_ms,
        )
    }

    fn signed_operation_input(
        draft: RadrootsEventDraft,
        signed_event: RadrootsSignedEvent,
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
        sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
            .fetch_one(outbox.pool())
            .await
            .expect("table count")
    }

    async fn table_columns(outbox: &RadrootsOutbox, table_name: &str) -> Vec<String> {
        let sql = format!("PRAGMA table_info({table_name})");
        sqlx::query(sqlx::AssertSqlSafe(sql))
            .fetch_all(outbox.pool())
            .await
            .expect("table columns")
            .into_iter()
            .map(|row| row.try_get("name").expect("column name"))
            .collect()
    }

    async fn mark_target_status_for_test(
        outbox: &RadrootsOutbox,
        outbox_event_id: i64,
        claim_token: &str,
        delivery_target_id: i64,
        status: RadrootsOutboxDeliveryTargetStatus,
        attempted_at_ms: i64,
    ) -> Result<(), RadrootsOutboxError> {
        match status {
            RadrootsOutboxDeliveryTargetStatus::Accepted => {
                outbox
                    .mark_delivery_target_accepted(
                        outbox_event_id,
                        claim_token,
                        delivery_target_id,
                        attempted_at_ms,
                    )
                    .await
            }
            RadrootsOutboxDeliveryTargetStatus::DeferredUntilImplemented => {
                outbox
                    .mark_delivery_target_deferred_until_implemented(
                        outbox_event_id,
                        claim_token,
                        delivery_target_id,
                        "transport deferred",
                        attempted_at_ms,
                    )
                    .await
            }
            RadrootsOutboxDeliveryTargetStatus::PreviewUnavailable => {
                outbox
                    .mark_delivery_target_preview_unavailable(
                        outbox_event_id,
                        claim_token,
                        delivery_target_id,
                        "transport preview unavailable",
                        attempted_at_ms,
                    )
                    .await
            }
            RadrootsOutboxDeliveryTargetStatus::SkippedPolicyDenied => {
                outbox
                    .mark_delivery_target_skipped_policy_denied(
                        outbox_event_id,
                        claim_token,
                        delivery_target_id,
                        "policy denied",
                        attempted_at_ms,
                    )
                    .await
            }
            RadrootsOutboxDeliveryTargetStatus::FailedTerminal => {
                outbox
                    .mark_delivery_target_failed_terminal(
                        outbox_event_id,
                        claim_token,
                        delivery_target_id,
                        "terminal failure",
                        attempted_at_ms,
                    )
                    .await
            }
            RadrootsOutboxDeliveryTargetStatus::FailedRetryable => {
                outbox
                    .mark_delivery_target_failed_retryable(
                        outbox_event_id,
                        claim_token,
                        delivery_target_id,
                        "retryable failure",
                        attempted_at_ms,
                    )
                    .await
            }
            RadrootsOutboxDeliveryTargetStatus::Pending
            | RadrootsOutboxDeliveryTargetStatus::Delivered
            | RadrootsOutboxDeliveryTargetStatus::Forwarded
            | RadrootsOutboxDeliveryTargetStatus::StoredByGateway
            | RadrootsOutboxDeliveryTargetStatus::Seen => unreachable!(),
        }
    }

    #[test]
    fn event_wide_publish_failure_helper_names_stay_removed() {
        let source = include_str!("store.rs");
        for removed in [
            ["mark_publish", "_failed_terminal"].concat(),
            ["finish_claimed", "_event"].concat(),
        ] {
            assert!(!source.contains(removed.as_str()), "{removed}");
        }
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
        let target_columns = table_columns(&outbox, "outbox_delivery_target").await;
        for column in ["target_scope", "target_label", "last_outcome_kind"] {
            assert!(target_columns.iter().any(|name| name == column), "{column}");
        }
        let attempt_columns = table_columns(&outbox, "outbox_delivery_attempt").await;
        assert!(
            attempt_columns.iter().any(|name| name == "outcome_kind"),
            "outcome_kind"
        );

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
    async fn no_wait_delivery_plan_persists_directly_and_completes_without_target_satisfaction() {
        let outbox = RadrootsOutbox::open_memory().await.expect("open");
        let draft = post_draft(FIXTURE_ALICE_PUBLIC_KEY_HEX, "no wait");
        let signed_event =
            radroots_nostr_sign_frozen_draft(&fixture_keys(), &draft).expect("signed event");
        let receipt = outbox
            .enqueue_signed_operation(RadrootsOutboxSignedOperationInput::new(
                "publish_post",
                draft,
                signed_event,
                RadrootsOutboxDeliveryPlanInput::new(
                    "transport.no_wait.local",
                    11,
                    RadrootsTransportSatisfactionPolicy::no_wait(),
                    vec![
                        nostr_target(NOSTR_PRIMARY_WSS),
                        reticulum_target(RADROOTS_RETICULUM_PREVIEW_ENDPOINT_URI),
                    ],
                ),
                true,
                1_007,
                1_000,
            ))
            .await
            .expect("enqueue");

        let stored_policy: String = sqlx::query_scalar(
            "SELECT satisfaction_policy FROM outbox_delivery_plan WHERE delivery_plan_id = ?",
        )
        .bind(receipt.delivery_plan_id)
        .fetch_one(outbox.pool())
        .await
        .expect("stored satisfaction policy");
        let event = outbox
            .get_event(receipt.outbox_event_id)
            .await
            .expect("event")
            .expect("event");
        let operation = outbox
            .get_operation(receipt.operation_id)
            .await
            .expect("operation")
            .expect("operation");
        let plans = outbox
            .delivery_plans(receipt.outbox_event_id)
            .await
            .expect("plans");
        let targets = outbox
            .delivery_targets(receipt.outbox_event_id)
            .await
            .expect("targets");

        assert_eq!(stored_policy, "no_wait");
        assert_eq!(event.state, RadrootsOutboxEventState::Published);
        assert_eq!(operation.status, RadrootsOutboxOperationStatus::Complete);
        assert_eq!(plans.len(), 1);
        assert_eq!(
            plans[0].satisfaction_policy,
            RadrootsTransportSatisfactionPolicy::no_wait()
        );
        assert_eq!(plans[0].required_success_count, 0);
        assert_eq!(plans[0].status, RadrootsOutboxDeliveryPlanStatus::Complete);
        assert_eq!(plans[0].satisfied_at_ms, Some(1_000));
        assert_eq!(targets.len(), 2);
        assert!(targets.iter().all(|target| {
            !target
                .status
                .counts_as_transport_satisfaction(RadrootsTransportSatisfactionClass::Accepted)
        }));
        assert!(
            outbox
                .claim_next_ready_signed_event("publisher", "claim-a", 2_000, 1_000)
                .await
                .expect("claim")
                .is_none()
        );
        assert!(
            outbox
                .reticulum_preview_events(None, 10)
                .await
                .expect("reticulum preview events")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn delivery_targets_persist_scope_label_and_identity_rules() {
        let outbox = RadrootsOutbox::open_memory().await.expect("open");
        let draft = post_draft(FIXTURE_ALICE_PUBLIC_KEY_HEX, "target metadata");
        let scoped = scoped_nostr_target(NOSTR_PRIMARY_WSS, "farm.alpha", "North greenhouse");
        let relabeled = scoped_nostr_target(NOSTR_PRIMARY_WSS, "farm.alpha", "Renamed greenhouse");
        let rescaled = scoped_nostr_target(NOSTR_PRIMARY_WSS, "farm.beta", "North greenhouse");
        assert_eq!(scoped.fingerprint, relabeled.fingerprint);
        assert_ne!(scoped.fingerprint, rescaled.fingerprint);

        let first_prepared = prepare_delivery_plan(
            draft.expected_event_id_str(),
            &RadrootsOutboxDeliveryPlanInput::new(
                "transport.nostr.local",
                1,
                RadrootsTransportSatisfactionPolicy::all_accepted(),
                vec![scoped.clone()],
            ),
        )
        .expect("first plan");
        let relabeled_prepared = prepare_delivery_plan(
            draft.expected_event_id_str(),
            &RadrootsOutboxDeliveryPlanInput::new(
                "transport.nostr.local",
                1,
                RadrootsTransportSatisfactionPolicy::all_accepted(),
                vec![relabeled],
            ),
        )
        .expect("relabeled plan");
        let rescaled_prepared = prepare_delivery_plan(
            draft.expected_event_id_str(),
            &RadrootsOutboxDeliveryPlanInput::new(
                "transport.nostr.local",
                1,
                RadrootsTransportSatisfactionPolicy::all_accepted(),
                vec![rescaled],
            ),
        )
        .expect("rescaled plan");
        assert_eq!(
            first_prepared.target_policy_fingerprint,
            relabeled_prepared.target_policy_fingerprint
        );
        assert_ne!(
            first_prepared.target_policy_fingerprint,
            rescaled_prepared.target_policy_fingerprint
        );

        let receipt = outbox
            .enqueue_operation(RadrootsOutboxOperationInput::new(
                "publish_post",
                draft,
                RadrootsOutboxDeliveryPlanInput::new(
                    "transport.nostr.local",
                    1,
                    RadrootsTransportSatisfactionPolicy::all_accepted(),
                    vec![scoped.clone()],
                ),
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
            targets[0].target_scope.as_ref().map(|scope| scope.as_str()),
            Some("farm.alpha")
        );
        assert_eq!(
            targets[0].target_label.as_ref().map(|label| label.as_str()),
            Some("North greenhouse")
        );
        assert_eq!(targets[0].endpoint_fingerprint, scoped.fingerprint);
        assert_eq!(targets[0].last_outcome_kind, None);
    }

    #[tokio::test]
    async fn enqueue_rejects_duplicate_delivery_targets_before_persistence() {
        let outbox = RadrootsOutbox::open_memory().await.expect("open");
        let draft = post_draft(hex_64('a').as_str(), "duplicate targets");

        let err = outbox
            .enqueue_operation(RadrootsOutboxOperationInput::new(
                "publish_post",
                draft,
                delivery_plan(vec![
                    nostr_target(NOSTR_PRIMARY_WSS),
                    nostr_target(NOSTR_PRIMARY_WSS),
                ]),
                1_000,
            ))
            .await
            .expect_err("duplicate targets");

        assert!(matches!(
            err,
            RadrootsOutboxError::Transport(RadrootsTransportError::DuplicateTargetFingerprint)
        ));
        assert_eq!(table_count(&outbox, "outbox_operations").await, 0);
        assert_eq!(table_count(&outbox, "outbox_event").await, 0);
        assert_eq!(table_count(&outbox, "outbox_delivery_plan").await, 0);
        assert_eq!(table_count(&outbox, "outbox_delivery_target").await, 0);
    }

    #[tokio::test]
    async fn enqueue_rejects_invalid_reticulum_targets_before_persistence() {
        let outbox = RadrootsOutbox::open_memory().await.expect("open");
        let draft = post_draft(hex_64('a').as_str(), "invalid reticulum");

        let err = outbox
            .enqueue_operation(RadrootsOutboxOperationInput::new(
                "publish_post",
                draft,
                delivery_plan(vec![malformed_reticulum_target(
                    "reticulum:preview-unavailable-alt",
                )]),
                1_000,
            ))
            .await
            .expect_err("invalid Reticulum target");

        assert!(matches!(
            err,
            RadrootsOutboxError::Transport(
                radroots_transport::RadrootsTransportError::InvalidTargetUri
            )
        ));
        assert_eq!(table_count(&outbox, "outbox_operations").await, 0);
        assert_eq!(table_count(&outbox, "outbox_event").await, 0);
        assert_eq!(table_count(&outbox, "outbox_delivery_plan").await, 0);
        assert_eq!(table_count(&outbox, "outbox_delivery_target").await, 0);
    }

    #[tokio::test]
    async fn enqueue_rejects_mixed_proxy_delegate_targets_before_persistence() {
        let outbox = RadrootsOutbox::open_memory().await.expect("open");
        let draft = post_draft(hex_64('a').as_str(), "mixed proxy");

        let err = outbox
            .enqueue_operation(RadrootsOutboxOperationInput::new(
                "publish_post",
                draft,
                delivery_plan(vec![
                    proxy_target("radrootsd-proxy:publish"),
                    nostr_target(NOSTR_PRIMARY_WSS),
                ]),
                1_000,
            ))
            .await
            .expect_err("mixed proxy targets");

        assert!(matches!(
            err,
            RadrootsOutboxError::Transport(RadrootsTransportError::InvalidTransportKind)
        ));
        assert_eq!(table_count(&outbox, "outbox_operations").await, 0);
        assert_eq!(table_count(&outbox, "outbox_event").await, 0);
        assert_eq!(table_count(&outbox, "outbox_delivery_plan").await, 0);
        assert_eq!(table_count(&outbox, "outbox_delivery_target").await, 0);
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
        assert_eq!(
            claimed.active_delivery_plan_id,
            Some(receipt.delivery_plan_id)
        );
        assert_eq!(claimed.delivery_targets.len(), 2);
        assert!(
            claimed
                .delivery_targets
                .iter()
                .all(|target| target.delivery_plan_id == receipt.delivery_plan_id)
        );

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
        assert_eq!(
            attempts[0].outcome_kind,
            RadrootsTransportOutcomeKind::Accepted
        );
        let targets = outbox
            .delivery_targets(receipt.outbox_event_id)
            .await
            .expect("targets");
        assert_eq!(
            targets
                .iter()
                .find(|target| {
                    target.delivery_target_id == claimed.delivery_targets[0].delivery_target_id
                })
                .expect("accepted target")
                .last_outcome_kind,
            Some(RadrootsTransportOutcomeKind::Accepted)
        );
    }

    #[tokio::test]
    async fn required_target_plan_evaluation_uses_persisted_fingerprints() {
        let outbox = RadrootsOutbox::open_memory().await.expect("open");
        let draft = post_draft(FIXTURE_ALICE_PUBLIC_KEY_HEX, "required target");
        let signed_event =
            radroots_nostr_sign_frozen_draft(&fixture_keys(), &draft).expect("signed event");
        let optional = nostr_target(NOSTR_PRIMARY_WSS);
        let required = nostr_target(NOSTR_SECONDARY_WSS);
        let receipt = outbox
            .enqueue_signed_operation(RadrootsOutboxSignedOperationInput::new(
                "publish_post",
                draft,
                signed_event,
                RadrootsOutboxDeliveryPlanInput::new(
                    "transport.nostr.local",
                    1,
                    RadrootsTransportSatisfactionPolicy::required_targets(
                        RadrootsTransportSatisfactionClass::Accepted,
                        vec![required.fingerprint.clone()],
                    )
                    .expect("required target policy"),
                    vec![optional.clone(), required.clone()],
                ),
                true,
                1_007,
                1_000,
            ))
            .await
            .expect("enqueue");
        let claimed = outbox
            .claim_next_ready_signed_event("publisher", "claim-a", 2_000, 1_000)
            .await
            .expect("claim")
            .expect("claimed");
        let optional_claim = claimed
            .delivery_targets
            .iter()
            .find(|target| target.endpoint_fingerprint == optional.fingerprint)
            .expect("optional target");
        outbox
            .mark_delivery_target_accepted(
                receipt.outbox_event_id,
                "claim-a",
                optional_claim.delivery_target_id,
                1_100,
            )
            .await
            .expect("optional accepted");

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

        assert_eq!(state, RadrootsOutboxEventState::PublishRetryable);
        let plans = outbox
            .delivery_plans(receipt.outbox_event_id)
            .await
            .expect("plans");
        assert_eq!(plans[0].status, RadrootsOutboxDeliveryPlanStatus::Queued);
        assert_eq!(plans[0].required_success_count, 1);
        let event = outbox
            .get_event(receipt.outbox_event_id)
            .await
            .expect("event")
            .expect("event");
        assert_eq!(event.state, RadrootsOutboxEventState::PublishRetryable);
        assert_eq!(event.claim_token, None);
        assert_eq!(event.next_attempt_after_ms, 2_500);
        let targets = outbox
            .delivery_targets(receipt.outbox_event_id)
            .await
            .expect("targets");
        assert!(targets.iter().any(|target| {
            target.endpoint_fingerprint == optional.fingerprint
                && target.status == RadrootsOutboxDeliveryTargetStatus::Accepted
        }));
        assert!(targets.iter().any(|target| {
            target.endpoint_fingerprint == required.fingerprint
                && target.status == RadrootsOutboxDeliveryTargetStatus::Pending
        }));
        assert_eq!(
            outbox
                .status_summary(3_000)
                .await
                .expect("summary")
                .ready_signed_events,
            1
        );
        let retry_claim = outbox
            .claim_next_ready_signed_event("publisher", "claim-b", 4_000, 3_000)
            .await
            .expect("retry claim")
            .expect("retry claimed");
        assert_eq!(retry_claim.delivery_targets.len(), 1);
        assert_eq!(
            retry_claim.delivery_targets[0].endpoint_fingerprint,
            required.fingerprint
        );
    }

    #[tokio::test]
    async fn required_target_plan_evaluation_ignores_optional_retryable_failure() {
        let outbox = RadrootsOutbox::open_memory().await.expect("open");
        let draft = post_draft(
            FIXTURE_ALICE_PUBLIC_KEY_HEX,
            "required target optional failure",
        );
        let signed_event =
            radroots_nostr_sign_frozen_draft(&fixture_keys(), &draft).expect("signed event");
        let optional = nostr_target(NOSTR_PRIMARY_WSS);
        let required = nostr_target(NOSTR_SECONDARY_WSS);
        let receipt = outbox
            .enqueue_signed_operation(RadrootsOutboxSignedOperationInput::new(
                "publish_post",
                draft,
                signed_event,
                RadrootsOutboxDeliveryPlanInput::new(
                    "transport.nostr.local",
                    1,
                    RadrootsTransportSatisfactionPolicy::required_targets(
                        RadrootsTransportSatisfactionClass::Accepted,
                        vec![required.fingerprint.clone()],
                    )
                    .expect("required target policy"),
                    vec![optional.clone(), required.clone()],
                ),
                true,
                1_007,
                1_000,
            ))
            .await
            .expect("enqueue");
        let claimed = outbox
            .claim_next_ready_signed_event("publisher", "claim-a", 2_000, 1_000)
            .await
            .expect("claim")
            .expect("claimed");
        let optional_claim = claimed
            .delivery_targets
            .iter()
            .find(|target| target.endpoint_fingerprint == optional.fingerprint)
            .expect("optional target");
        let required_claim = claimed
            .delivery_targets
            .iter()
            .find(|target| target.endpoint_fingerprint == required.fingerprint)
            .expect("required target");
        outbox
            .mark_delivery_target_failed_retryable(
                receipt.outbox_event_id,
                "claim-a",
                optional_claim.delivery_target_id,
                "optional relay timeout",
                1_100,
            )
            .await
            .expect("optional retryable");
        outbox
            .mark_delivery_target_accepted(
                receipt.outbox_event_id,
                "claim-a",
                required_claim.delivery_target_id,
                1_110,
            )
            .await
            .expect("required accepted");

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
        let targets = outbox
            .delivery_targets(receipt.outbox_event_id)
            .await
            .expect("targets");
        assert!(targets.iter().any(|target| {
            target.endpoint_fingerprint == optional.fingerprint
                && target.status == RadrootsOutboxDeliveryTargetStatus::FailedRetryable
        }));
        assert!(targets.iter().any(|target| {
            target.endpoint_fingerprint == required.fingerprint
                && target.status == RadrootsOutboxDeliveryTargetStatus::Accepted
        }));
    }

    #[tokio::test]
    async fn enqueue_rejects_required_targets_outside_delivery_plan_before_persistence() {
        let outbox = RadrootsOutbox::open_memory().await.expect("open");
        let draft = post_draft(FIXTURE_ALICE_PUBLIC_KEY_HEX, "stale required target");
        let missing = nostr_target(NOSTR_SECONDARY_WSS);

        let err = outbox
            .enqueue_operation(RadrootsOutboxOperationInput::new(
                "publish_post",
                draft,
                RadrootsOutboxDeliveryPlanInput::new(
                    "transport.nostr.local",
                    1,
                    RadrootsTransportSatisfactionPolicy::required_targets(
                        RadrootsTransportSatisfactionClass::Accepted,
                        vec![missing.fingerprint],
                    )
                    .expect("required target policy"),
                    vec![nostr_target(NOSTR_PRIMARY_WSS)],
                ),
                1_000,
            ))
            .await
            .expect_err("missing required target");

        assert!(matches!(
            err,
            RadrootsOutboxError::Transport(RadrootsTransportError::InvalidSatisfactionPolicy)
        ));
        assert_eq!(table_count(&outbox, "outbox_operations").await, 0);
        assert_eq!(table_count(&outbox, "outbox_event").await, 0);
        assert_eq!(table_count(&outbox, "outbox_delivery_plan").await, 0);
        assert_eq!(table_count(&outbox, "outbox_delivery_target").await, 0);
    }

    #[tokio::test]
    async fn terminal_delivery_target_updates_are_idempotent_and_conflicting_rewrites_fail() {
        for terminal_status in [
            RadrootsOutboxDeliveryTargetStatus::Accepted,
            RadrootsOutboxDeliveryTargetStatus::DeferredUntilImplemented,
            RadrootsOutboxDeliveryTargetStatus::PreviewUnavailable,
            RadrootsOutboxDeliveryTargetStatus::SkippedPolicyDenied,
            RadrootsOutboxDeliveryTargetStatus::FailedTerminal,
        ] {
            let outbox = RadrootsOutbox::open_memory().await.expect("open");
            let draft = post_draft(FIXTURE_ALICE_PUBLIC_KEY_HEX, terminal_status.as_str());
            let signed_event =
                radroots_nostr_sign_frozen_draft(&fixture_keys(), &draft).expect("signed event");
            let receipt = outbox
                .enqueue_signed_operation(RadrootsOutboxSignedOperationInput::new(
                    "publish_post",
                    draft,
                    signed_event,
                    RadrootsOutboxDeliveryPlanInput::new(
                        "transport.nostr.local",
                        1,
                        RadrootsTransportSatisfactionPolicy::all_accepted(),
                        vec![nostr_target(NOSTR_PRIMARY_WSS)],
                    ),
                    true,
                    1_007,
                    1_000,
                ))
                .await
                .expect("enqueue");
            let claimed = outbox
                .claim_next_ready_signed_event("publisher", "claim-a", 2_000, 1_000)
                .await
                .expect("claim")
                .expect("claimed");
            let target_id = claimed.delivery_targets[0].delivery_target_id;

            mark_target_status_for_test(
                &outbox,
                receipt.outbox_event_id,
                "claim-a",
                target_id,
                terminal_status,
                1_100,
            )
            .await
            .expect("first terminal update");
            mark_target_status_for_test(
                &outbox,
                receipt.outbox_event_id,
                "claim-a",
                target_id,
                terminal_status,
                1_200,
            )
            .await
            .expect("idempotent terminal update");

            let targets = outbox
                .delivery_targets(receipt.outbox_event_id)
                .await
                .expect("targets");
            assert_eq!(targets.len(), 1);
            assert_eq!(targets[0].status, terminal_status);
            assert_eq!(targets[0].attempt_count, 1);
            assert_eq!(targets[0].last_attempt_at_ms, Some(1_100));
            assert_eq!(targets[0].completed_at_ms, Some(1_100));
            let attempts = outbox.delivery_attempts(target_id).await.expect("attempts");
            assert_eq!(attempts.len(), 1);
            assert_eq!(attempts[0].status, terminal_status);
            assert_eq!(attempts[0].attempted_at_ms, 1_100);

            let requested_status =
                if terminal_status == RadrootsOutboxDeliveryTargetStatus::Accepted {
                    RadrootsOutboxDeliveryTargetStatus::FailedTerminal
                } else {
                    RadrootsOutboxDeliveryTargetStatus::Accepted
                };
            let err = mark_target_status_for_test(
                &outbox,
                receipt.outbox_event_id,
                "claim-a",
                target_id,
                requested_status,
                1_300,
            )
            .await
            .expect_err("conflicting terminal rewrite");
            assert!(matches!(
                err,
                RadrootsOutboxError::DeliveryTargetStatusConflict {
                    delivery_target_id,
                    current_status,
                    requested_status: observed_requested_status,
                } if delivery_target_id == target_id
                    && current_status == terminal_status.as_str()
                    && observed_requested_status == requested_status.as_str()
            ));
            let targets = outbox
                .delivery_targets(receipt.outbox_event_id)
                .await
                .expect("targets after conflict");
            assert_eq!(targets[0].status, terminal_status);
            assert_eq!(targets[0].attempt_count, 1);
            assert_eq!(targets[0].completed_at_ms, Some(1_100));
            let attempts = outbox
                .delivery_attempts(target_id)
                .await
                .expect("attempts after conflict");
            assert_eq!(attempts.len(), 1);
        }
    }

    #[tokio::test]
    async fn publish_claims_one_delivery_plan_and_rejects_sibling_target_outcomes() {
        let outbox = RadrootsOutbox::open_memory().await.expect("open");
        let draft = post_draft(FIXTURE_ALICE_PUBLIC_KEY_HEX, "duplicate endpoint plans");
        let signed_event =
            radroots_nostr_sign_frozen_draft(&fixture_keys(), &draft).expect("signed event");
        let first = outbox
            .enqueue_signed_operation(
                RadrootsOutboxSignedOperationInput::new(
                    "publish_post",
                    draft.clone(),
                    signed_event.clone(),
                    RadrootsOutboxDeliveryPlanInput::new(
                        "transport.nostr.primary-plan",
                        1,
                        RadrootsTransportSatisfactionPolicy::all_accepted(),
                        vec![nostr_target(NOSTR_PRIMARY_WSS)],
                    ),
                    true,
                    1_007,
                    1_000,
                )
                .with_idempotency_key("idem-duplicate-endpoint"),
            )
            .await
            .expect("first plan");
        let second = outbox
            .enqueue_signed_operation(
                RadrootsOutboxSignedOperationInput::new(
                    "publish_post",
                    draft,
                    signed_event,
                    RadrootsOutboxDeliveryPlanInput::new(
                        "transport.nostr.secondary-plan",
                        1,
                        RadrootsTransportSatisfactionPolicy::all_accepted(),
                        vec![nostr_target(NOSTR_PRIMARY_WSS)],
                    ),
                    true,
                    1_017,
                    1_010,
                )
                .with_idempotency_key("idem-duplicate-endpoint"),
            )
            .await
            .expect("second plan");

        assert_eq!(first.outbox_event_id, second.outbox_event_id);
        assert_ne!(first.delivery_plan_id, second.delivery_plan_id);
        let initial_targets = outbox
            .delivery_targets(first.outbox_event_id)
            .await
            .expect("targets");
        assert_eq!(initial_targets.len(), 2);
        assert_eq!(
            initial_targets[0].endpoint_fingerprint,
            initial_targets[1].endpoint_fingerprint
        );

        let first_claim = outbox
            .claim_next_ready_signed_event("publisher", "claim-a", 2_000, 1_020)
            .await
            .expect("claim")
            .expect("claimed");
        let active_plan_id = first_claim.active_delivery_plan_id.expect("active plan");
        assert_eq!(first_claim.delivery_targets.len(), 1);
        assert_eq!(
            first_claim.delivery_targets[0].delivery_plan_id,
            active_plan_id
        );
        let sibling_target = initial_targets
            .iter()
            .find(|target| target.delivery_plan_id != active_plan_id)
            .expect("sibling target");
        let sibling_err = outbox
            .mark_delivery_target_accepted(
                first.outbox_event_id,
                "claim-a",
                sibling_target.delivery_target_id,
                1_100,
            )
            .await
            .expect_err("sibling target is not in active plan");
        assert!(matches!(
            sibling_err,
            RadrootsOutboxError::DeliveryTargetNotFound(_)
        ));

        outbox
            .mark_delivery_target_accepted(
                first.outbox_event_id,
                "claim-a",
                first_claim.delivery_targets[0].delivery_target_id,
                1_110,
            )
            .await
            .expect("active target accepted");
        let retry_state = outbox
            .complete_publish_attempt(
                first.outbox_event_id,
                "claim-a",
                "retryable",
                "terminal",
                2_500,
                1_200,
            )
            .await
            .expect("complete first plan");
        assert_eq!(retry_state, RadrootsOutboxEventState::PublishRetryable);

        let retry_claim = outbox
            .claim_next_ready_signed_event("publisher", "claim-b", 3_000, 2_500)
            .await
            .expect("retry claim")
            .expect("claimed");
        assert_eq!(retry_claim.delivery_targets.len(), 1);
        assert_ne!(
            retry_claim.active_delivery_plan_id,
            first_claim.active_delivery_plan_id
        );
        outbox
            .mark_delivery_target_accepted(
                first.outbox_event_id,
                "claim-b",
                retry_claim.delivery_targets[0].delivery_target_id,
                2_600,
            )
            .await
            .expect("second active target accepted");
        let final_state = outbox
            .complete_publish_attempt(
                first.outbox_event_id,
                "claim-b",
                "retryable",
                "terminal",
                3_500,
                2_700,
            )
            .await
            .expect("complete second plan");
        assert_eq!(final_state, RadrootsOutboxEventState::Published);
    }

    #[tokio::test]
    async fn active_plan_terminal_failure_leaves_sibling_claimable() {
        let outbox = RadrootsOutbox::open_memory().await.expect("open");
        let draft = post_draft(FIXTURE_ALICE_PUBLIC_KEY_HEX, "plan scoped terminal");
        let signed_event =
            radroots_nostr_sign_frozen_draft(&fixture_keys(), &draft).expect("signed event");
        let first = outbox
            .enqueue_signed_operation(
                RadrootsOutboxSignedOperationInput::new(
                    "publish_post",
                    draft.clone(),
                    signed_event.clone(),
                    RadrootsOutboxDeliveryPlanInput::new(
                        "transport.proxy.invalid",
                        1,
                        RadrootsTransportSatisfactionPolicy::all_accepted(),
                        vec![nostr_target(NOSTR_PRIMARY_WSS)],
                    ),
                    true,
                    1_007,
                    1_000,
                )
                .with_idempotency_key("idem-plan-terminal"),
            )
            .await
            .expect("first plan");
        let second = outbox
            .enqueue_signed_operation(
                RadrootsOutboxSignedOperationInput::new(
                    "publish_post",
                    draft,
                    signed_event,
                    RadrootsOutboxDeliveryPlanInput::new(
                        "transport.nostr.sibling",
                        1,
                        RadrootsTransportSatisfactionPolicy::all_accepted(),
                        vec![nostr_target(NOSTR_SECONDARY_WSS)],
                    ),
                    true,
                    1_017,
                    1_010,
                )
                .with_idempotency_key("idem-plan-terminal"),
            )
            .await
            .expect("second plan");
        assert_eq!(first.outbox_event_id, second.outbox_event_id);

        let first_claim = outbox
            .claim_next_ready_signed_event("publisher", "claim-a", 2_000, 1_020)
            .await
            .expect("claim")
            .expect("claimed");
        let failed_plan_id = first_claim.active_delivery_plan_id.expect("active plan");
        let failed_target_id = first_claim.delivery_targets[0].delivery_target_id;
        let first_state = outbox
            .mark_active_delivery_plan_failed_terminal(
                first.outbox_event_id,
                "claim-a",
                "local proxy validation failed",
                1_100,
            )
            .await
            .expect("active plan terminal");
        assert_eq!(first_state, RadrootsOutboxEventState::PublishRetryable);
        let event_after_first = outbox
            .get_event(first.outbox_event_id)
            .await
            .expect("event")
            .expect("event");
        assert_eq!(
            event_after_first.state,
            RadrootsOutboxEventState::PublishRetryable
        );
        assert_eq!(event_after_first.claim_token, None);

        let targets_after_first = outbox
            .delivery_targets(first.outbox_event_id)
            .await
            .expect("targets");
        assert_eq!(
            targets_after_first
                .iter()
                .find(|target| target.delivery_target_id == failed_target_id)
                .expect("failed target")
                .status,
            RadrootsOutboxDeliveryTargetStatus::FailedTerminal
        );
        assert!(
            targets_after_first
                .iter()
                .filter(|target| target.delivery_plan_id != failed_plan_id)
                .all(|target| target.status == RadrootsOutboxDeliveryTargetStatus::Pending)
        );
        let plans_after_first = outbox
            .delivery_plans(first.outbox_event_id)
            .await
            .expect("plans");
        assert_eq!(
            plans_after_first
                .iter()
                .find(|plan| plan.delivery_plan_id == failed_plan_id)
                .expect("failed plan")
                .status,
            RadrootsOutboxDeliveryPlanStatus::FailedTerminal
        );
        assert!(
            plans_after_first
                .iter()
                .filter(|plan| plan.delivery_plan_id != failed_plan_id)
                .all(|plan| plan.status == RadrootsOutboxDeliveryPlanStatus::Queued)
        );

        let sibling_claim = outbox
            .claim_next_ready_signed_event("publisher", "claim-b", 3_000, 1_100)
            .await
            .expect("sibling claim")
            .expect("sibling claim");
        assert_ne!(
            sibling_claim.active_delivery_plan_id,
            first_claim.active_delivery_plan_id
        );
        let final_state = outbox
            .mark_active_delivery_plan_failed_terminal(
                first.outbox_event_id,
                "claim-b",
                "sibling terminal",
                1_200,
            )
            .await
            .expect("sibling terminal");
        assert_eq!(final_state, RadrootsOutboxEventState::FailedTerminal);
        let operation = outbox
            .get_operation(first.operation_id)
            .await
            .expect("operation")
            .expect("operation");
        assert_eq!(
            operation.status,
            RadrootsOutboxOperationStatus::FailedTerminal
        );
    }

    #[tokio::test]
    async fn cancel_claimed_event_marks_every_delivery_plan_cancelled() {
        let outbox = RadrootsOutbox::open_memory().await.expect("open");
        let draft = post_draft(FIXTURE_ALICE_PUBLIC_KEY_HEX, "cancel all plans");
        let signed_event =
            radroots_nostr_sign_frozen_draft(&fixture_keys(), &draft).expect("signed event");
        let first = outbox
            .enqueue_signed_operation(
                RadrootsOutboxSignedOperationInput::new(
                    "publish_post",
                    draft.clone(),
                    signed_event.clone(),
                    RadrootsOutboxDeliveryPlanInput::new(
                        "transport.nostr.cancel",
                        1,
                        RadrootsTransportSatisfactionPolicy::all_accepted(),
                        vec![nostr_target(NOSTR_PRIMARY_WSS)],
                    ),
                    true,
                    1_007,
                    1_000,
                )
                .with_idempotency_key("idem-cancel-all-plans"),
            )
            .await
            .expect("first plan");
        let second = outbox
            .enqueue_signed_operation(
                RadrootsOutboxSignedOperationInput::new(
                    "publish_post",
                    draft,
                    signed_event,
                    RadrootsOutboxDeliveryPlanInput::new(
                        "transport.nostr.cancel.sibling",
                        1,
                        RadrootsTransportSatisfactionPolicy::all_accepted(),
                        vec![nostr_target(NOSTR_SECONDARY_WSS)],
                    ),
                    true,
                    1_017,
                    1_010,
                )
                .with_idempotency_key("idem-cancel-all-plans"),
            )
            .await
            .expect("second plan");
        assert_eq!(first.outbox_event_id, second.outbox_event_id);

        outbox
            .claim_next_ready_signed_event("publisher", "claim-a", 2_000, 1_020)
            .await
            .expect("claim")
            .expect("claimed");
        outbox
            .cancel_claimed_event(first.outbox_event_id, "claim-a", 1_100)
            .await
            .expect("cancel");

        let event = outbox
            .get_event(first.outbox_event_id)
            .await
            .expect("event")
            .expect("event");
        assert_eq!(event.state, RadrootsOutboxEventState::Cancelled);
        assert_eq!(event.claim_token, None);
        assert_eq!(event.active_delivery_plan_id, None);
        let operation = outbox
            .get_operation(first.operation_id)
            .await
            .expect("operation")
            .expect("operation");
        assert_eq!(operation.status, RadrootsOutboxOperationStatus::Cancelled);
        let plans = outbox
            .delivery_plans(first.outbox_event_id)
            .await
            .expect("plans");
        assert_eq!(plans.len(), 2);
        assert!(
            plans
                .iter()
                .all(|plan| plan.status == RadrootsOutboxDeliveryPlanStatus::Cancelled)
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
        let event = outbox
            .get_event(receipt.outbox_event_id)
            .await
            .expect("event")
            .expect("event");
        assert_eq!(event.state, RadrootsOutboxEventState::PreviewUnavailable);
        let operation = outbox
            .get_operation(receipt.operation_id)
            .await
            .expect("operation")
            .expect("operation");
        assert_eq!(
            operation.status,
            RadrootsOutboxOperationStatus::PreviewUnavailable
        );
        let summary = outbox.status_summary(1_000).await.expect("summary");
        assert_eq!(summary.pending_events, 0);
        assert_eq!(summary.ready_signed_events, 0);
        assert_eq!(summary.preview_unavailable_events, 1);
        assert_eq!(summary.deferred_until_implemented_events, 0);
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
        let event = outbox
            .get_event(receipt.outbox_event_id)
            .await
            .expect("event")
            .expect("event");
        assert_eq!(
            event.state,
            RadrootsOutboxEventState::DeferredUntilImplemented
        );
        let operation = outbox
            .get_operation(receipt.operation_id)
            .await
            .expect("operation")
            .expect("operation");
        assert_eq!(
            operation.status,
            RadrootsOutboxOperationStatus::DeferredUntilImplemented
        );
        let summary = outbox.status_summary(1_000).await.expect("summary");
        assert_eq!(summary.pending_events, 0);
        assert_eq!(summary.ready_signed_events, 0);
        assert_eq!(summary.preview_unavailable_events, 0);
        assert_eq!(summary.deferred_until_implemented_events, 1);
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
    async fn reticulum_preview_events_report_preview_and_deferred_records() {
        let outbox = RadrootsOutbox::open_memory().await.expect("open");
        let preview_draft = post_draft(FIXTURE_ALICE_PUBLIC_KEY_HEX, "reticulum preview record");
        let preview_signed_event =
            radroots_nostr_sign_frozen_draft(&fixture_keys(), &preview_draft)
                .expect("preview signed event");
        let preview = outbox
            .enqueue_signed_operation(RadrootsOutboxSignedOperationInput::new(
                "publish_post",
                preview_draft,
                preview_signed_event,
                RadrootsOutboxDeliveryPlanInput::new(
                    "transport.reticulum.preview",
                    1,
                    RadrootsTransportSatisfactionPolicy::all_accepted(),
                    vec![scoped_reticulum_target("farm.preview", "Preview mesh")],
                ),
                true,
                1_007,
                1_000,
            ))
            .await
            .expect("preview enqueue");
        let deferred_draft = post_draft(FIXTURE_ALICE_PUBLIC_KEY_HEX, "reticulum deferred record");
        let deferred_signed_event =
            radroots_nostr_sign_frozen_draft(&fixture_keys(), &deferred_draft)
                .expect("deferred signed event");
        let deferred = outbox
            .enqueue_signed_operation(RadrootsOutboxSignedOperationInput::new(
                "publish_post",
                deferred_draft,
                deferred_signed_event,
                RadrootsOutboxDeliveryPlanInput::new(
                    "transport.reticulum.preview",
                    1,
                    RadrootsTransportSatisfactionPolicy::all_accepted(),
                    vec![scoped_reticulum_target("farm.deferred", "Deferred mesh")],
                )
                .with_reticulum_preview_behavior(
                    RadrootsOutboxReticulumPreviewBehavior::DeferDeliveryPlans,
                ),
                true,
                1_008,
                1_001,
            ))
            .await
            .expect("deferred enqueue");

        let records = outbox
            .reticulum_preview_events(None, 10)
            .await
            .expect("records");
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].event.outbox_event_id, preview.outbox_event_id);
        assert_eq!(
            records[0].event.state,
            RadrootsOutboxEventState::PreviewUnavailable
        );
        assert_eq!(records[0].targets.len(), 1);
        assert_eq!(
            records[0].targets[0].status,
            RadrootsOutboxDeliveryTargetStatus::PreviewUnavailable
        );
        assert_eq!(
            records[0].targets[0]
                .target_scope
                .as_ref()
                .map(|scope| scope.as_str()),
            Some("farm.preview")
        );
        assert_eq!(
            records[0].targets[0]
                .target_label
                .as_ref()
                .map(|label| label.as_str()),
            Some("Preview mesh")
        );
        assert_eq!(
            records[0].targets[0].last_outcome_kind,
            Some(RadrootsTransportOutcomeKind::TransportUnavailable)
        );
        assert_eq!(records[0].targets[0].attempt_count, 0);
        assert_eq!(records[1].event.outbox_event_id, deferred.outbox_event_id);
        assert_eq!(
            records[1].event.state,
            RadrootsOutboxEventState::DeferredUntilImplemented
        );
        assert_eq!(records[1].targets.len(), 1);
        assert_eq!(
            records[1].targets[0].status,
            RadrootsOutboxDeliveryTargetStatus::DeferredUntilImplemented
        );
        assert_eq!(
            records[1].targets[0]
                .target_scope
                .as_ref()
                .map(|scope| scope.as_str()),
            Some("farm.deferred")
        );
        assert_eq!(
            records[1].targets[0]
                .target_label
                .as_ref()
                .map(|label| label.as_str()),
            Some("Deferred mesh")
        );
        assert_eq!(
            records[1].targets[0].last_outcome_kind,
            Some(RadrootsTransportOutcomeKind::DeferredUntilImplemented)
        );
        assert_eq!(records[1].targets[0].attempt_count, 0);

        let limited = outbox
            .reticulum_preview_events(None, 1)
            .await
            .expect("limited");
        assert_eq!(limited.len(), 1);
        assert_eq!(limited[0].event.outbox_event_id, preview.outbox_event_id);

        let specific = outbox
            .reticulum_preview_events(Some(deferred.outbox_event_id), 10)
            .await
            .expect("specific");
        assert_eq!(specific.len(), 1);
        assert_eq!(specific[0].event.outbox_event_id, deferred.outbox_event_id);
        assert!(
            outbox
                .reticulum_preview_events(Some(999), 10)
                .await
                .expect("missing")
                .is_empty()
        );
        assert!(
            outbox
                .reticulum_preview_events(None, 0)
                .await
                .expect("zero")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn complete_signing_sets_preview_unavailable_lifecycle_for_reticulum_only_plans() {
        let outbox = RadrootsOutbox::open_memory().await.expect("open");
        let draft = post_draft(FIXTURE_ALICE_PUBLIC_KEY_HEX, "reticulum after signing");
        let receipt = outbox
            .enqueue_operation(RadrootsOutboxOperationInput::new(
                "publish_post",
                draft,
                RadrootsOutboxDeliveryPlanInput::new(
                    "transport.reticulum.preview",
                    1,
                    RadrootsTransportSatisfactionPolicy::all_accepted(),
                    vec![reticulum_target("reticulum:preview-unavailable")],
                ),
                1_000,
            ))
            .await
            .expect("enqueue");
        let claimed = outbox
            .claim_next_ready_event("signer", "claim-a", 2_000, 1_000)
            .await
            .expect("claim")
            .expect("claimed");
        assert_eq!(claimed.active_delivery_plan_id, None);
        let signed =
            radroots_nostr_sign_frozen_draft(&fixture_keys(), &claimed.draft).expect("signed");

        outbox
            .complete_signing(receipt.outbox_event_id, "claim-a", signed, 1_100)
            .await
            .expect("complete signing");

        let event = outbox
            .get_event(receipt.outbox_event_id)
            .await
            .expect("event")
            .expect("event");
        assert_eq!(event.state, RadrootsOutboxEventState::PreviewUnavailable);
        assert_eq!(event.claim_token, None);
        let operation = outbox
            .get_operation(receipt.operation_id)
            .await
            .expect("operation")
            .expect("operation");
        assert_eq!(
            operation.status,
            RadrootsOutboxOperationStatus::PreviewUnavailable
        );
        assert!(
            outbox
                .claim_next_ready_signed_event("publisher", "claim-b", 3_000, 1_100)
                .await
                .expect("claim")
                .is_none()
        );
    }

    #[tokio::test]
    async fn hybrid_all_targets_completes_after_nostr_success_with_reticulum_preview_preserved() {
        let outbox = RadrootsOutbox::open_memory().await.expect("open");
        let draft = post_draft(FIXTURE_ALICE_PUBLIC_KEY_HEX, "hybrid all targets");
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
                    RadrootsTransportSatisfactionPolicy::all_accepted(),
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
        let plans = outbox
            .delivery_plans(receipt.outbox_event_id)
            .await
            .expect("plans");
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].required_success_count, 1);
        assert_eq!(plans[0].status, RadrootsOutboxDeliveryPlanStatus::Queued);
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
        let claimed = outbox
            .claim_next_ready_signed_event("publisher", "claim-a", 2_000, 1_000)
            .await
            .expect("claim")
            .expect("claimed");
        assert_eq!(claimed.delivery_targets.len(), 1);
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
        let operation = outbox
            .get_operation(receipt.operation_id)
            .await
            .expect("operation")
            .expect("operation");
        assert_eq!(operation.status, RadrootsOutboxOperationStatus::Complete);
        let plans = outbox
            .delivery_plans(receipt.outbox_event_id)
            .await
            .expect("plans");
        assert_eq!(plans[0].status, RadrootsOutboxDeliveryPlanStatus::Complete);
        assert_eq!(plans[0].satisfied_at_ms, Some(1_200));
        let targets = outbox
            .delivery_targets(receipt.outbox_event_id)
            .await
            .expect("targets");
        assert!(targets.iter().any(|target| {
            target.status == RadrootsOutboxDeliveryTargetStatus::Accepted
                && target.attempt_count == 1
        }));
        assert!(targets.iter().any(|target| {
            target.status == RadrootsOutboxDeliveryTargetStatus::PreviewUnavailable
                && target.attempt_count == 0
                && target.completed_at_ms.is_none()
        }));
    }

    #[tokio::test]
    async fn hybrid_quorum_rejects_preview_targets_as_required_success_capacity() {
        let outbox = RadrootsOutbox::open_memory().await.expect("open");
        let draft = post_draft(FIXTURE_ALICE_PUBLIC_KEY_HEX, "hybrid quorum");
        let signed_event =
            radroots_nostr_sign_frozen_draft(&fixture_keys(), &draft).expect("signed event");
        let err = outbox
            .enqueue_signed_operation(RadrootsOutboxSignedOperationInput::new(
                "publish_post",
                draft,
                signed_event,
                RadrootsOutboxDeliveryPlanInput::new(
                    "transport.hybrid",
                    1,
                    RadrootsTransportSatisfactionPolicy::quorum_accepted(2),
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
            .expect_err("quorum exceeds delivery-capable targets");

        assert!(matches!(
            err,
            RadrootsOutboxError::Transport(RadrootsTransportError::InvalidSatisfactionPolicy)
        ));
        assert_eq!(table_count(&outbox, "outbox_operations").await, 0);
        assert_eq!(table_count(&outbox, "outbox_event").await, 0);
        assert_eq!(table_count(&outbox, "outbox_delivery_plan").await, 0);
        assert_eq!(table_count(&outbox, "outbox_delivery_target").await, 0);
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
        let summary = outbox.status_summary(1_000).await.expect("summary");
        assert_eq!(summary.pending_events, 1);
        assert_eq!(summary.preview_unavailable_events, 0);
        assert_eq!(summary.deferred_until_implemented_events, 0);
    }

    #[tokio::test]
    async fn accepted_delivery_target_can_advance_to_strict_success_status() {
        let outbox = RadrootsOutbox::open_memory().await.expect("open");
        let draft = post_draft(FIXTURE_ALICE_PUBLIC_KEY_HEX, "accepted then delivered");
        let signed_event =
            radroots_nostr_sign_frozen_draft(&fixture_keys(), &draft).expect("signed event");
        let receipt = outbox
            .enqueue_signed_operation(RadrootsOutboxSignedOperationInput::new(
                "publish_post",
                draft,
                signed_event,
                RadrootsOutboxDeliveryPlanInput::new(
                    "transport.nostr.local",
                    1,
                    RadrootsTransportSatisfactionPolicy::all_delivered(),
                    vec![nostr_target(NOSTR_PRIMARY_WSS)],
                ),
                true,
                1_007,
                1_000,
            ))
            .await
            .expect("enqueue");
        let claimed = outbox
            .claim_next_ready_signed_event("publisher", "claim-a", 2_000, 1_000)
            .await
            .expect("claim")
            .expect("claimed");
        let target_id = claimed.delivery_targets[0].delivery_target_id;

        outbox
            .mark_delivery_target_accepted(receipt.outbox_event_id, "claim-a", target_id, 1_100)
            .await
            .expect("accepted");
        outbox
            .mark_delivery_target_delivered(receipt.outbox_event_id, "claim-a", target_id, 1_110)
            .await
            .expect("delivered");

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
        let targets = outbox
            .delivery_targets(receipt.outbox_event_id)
            .await
            .expect("targets");
        assert_eq!(
            targets[0].status,
            RadrootsOutboxDeliveryTargetStatus::Delivered
        );
        assert_eq!(targets[0].attempt_count, 2);
        assert_eq!(targets[0].completed_at_ms, Some(1_110));
        assert_eq!(
            targets[0].last_outcome_kind,
            Some(RadrootsTransportOutcomeKind::Delivered)
        );
        let attempts = outbox.delivery_attempts(target_id).await.expect("attempts");
        assert_eq!(attempts.len(), 2);
        assert_eq!(
            attempts[0].outcome_kind,
            RadrootsTransportOutcomeKind::Accepted
        );
        assert_eq!(
            attempts[1].outcome_kind,
            RadrootsTransportOutcomeKind::Delivered
        );
        let plans = outbox
            .delivery_plans(receipt.outbox_event_id)
            .await
            .expect("plans");
        assert_eq!(plans[0].status, RadrootsOutboxDeliveryPlanStatus::Complete);
    }

    #[tokio::test]
    async fn claimed_delivery_targets_can_complete_as_strict_transport_successes() {
        for (content, mark, expected_status, satisfaction_policy) in [
            (
                "transport delivered",
                "delivered",
                RadrootsOutboxDeliveryTargetStatus::Delivered,
                RadrootsTransportSatisfactionPolicy::all_delivered(),
            ),
            (
                "transport forwarded",
                "forwarded",
                RadrootsOutboxDeliveryTargetStatus::Forwarded,
                RadrootsTransportSatisfactionPolicy::all_forwarded(),
            ),
            (
                "transport stored",
                "stored",
                RadrootsOutboxDeliveryTargetStatus::StoredByGateway,
                RadrootsTransportSatisfactionPolicy::all_stored(),
            ),
            (
                "transport seen",
                "seen",
                RadrootsOutboxDeliveryTargetStatus::Seen,
                RadrootsTransportSatisfactionPolicy::all_seen(),
            ),
        ] {
            let outbox = RadrootsOutbox::open_memory().await.expect("open");
            let draft = post_draft(FIXTURE_ALICE_PUBLIC_KEY_HEX, content);
            let signed_event =
                radroots_nostr_sign_frozen_draft(&fixture_keys(), &draft).expect("signed event");
            let receipt = outbox
                .enqueue_signed_operation(RadrootsOutboxSignedOperationInput::new(
                    "publish_post",
                    draft,
                    signed_event,
                    RadrootsOutboxDeliveryPlanInput::new(
                        "transport.nostr.local",
                        1,
                        satisfaction_policy,
                        vec![nostr_target(NOSTR_PRIMARY_WSS)],
                    ),
                    true,
                    1_007,
                    1_000,
                ))
                .await
                .expect("enqueue");
            let claimed = outbox
                .claim_next_ready_signed_event("publisher", "claim-a", 2_000, 1_000)
                .await
                .expect("claim")
                .expect("claimed");
            let target_id = claimed.delivery_targets[0].delivery_target_id;

            match mark {
                "delivered" => {
                    outbox
                        .mark_delivery_target_delivered(
                            receipt.outbox_event_id,
                            "claim-a",
                            target_id,
                            1_100,
                        )
                        .await
                        .expect("delivered");
                }
                "forwarded" => {
                    outbox
                        .mark_delivery_target_forwarded(
                            receipt.outbox_event_id,
                            "claim-a",
                            target_id,
                            1_100,
                        )
                        .await
                        .expect("forwarded");
                }
                "stored" => {
                    outbox
                        .mark_delivery_target_stored_by_gateway(
                            receipt.outbox_event_id,
                            "claim-a",
                            target_id,
                            1_100,
                        )
                        .await
                        .expect("stored");
                }
                "seen" => {
                    outbox
                        .mark_delivery_target_seen(
                            receipt.outbox_event_id,
                            "claim-a",
                            target_id,
                            1_100,
                        )
                        .await
                        .expect("seen");
                }
                _ => unreachable!(),
            }

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
            let targets = outbox
                .delivery_targets(receipt.outbox_event_id)
                .await
                .expect("targets");
            assert_eq!(targets[0].status, expected_status);
            assert_eq!(
                targets[0].last_outcome_kind,
                Some(outcome_kind_for_status(expected_status))
            );
            let attempts = outbox.delivery_attempts(target_id).await.expect("attempts");
            assert_eq!(attempts.len(), 1);
            assert_eq!(
                attempts[0].outcome_kind,
                outcome_kind_for_status(expected_status)
            );
            let plans = outbox
                .delivery_plans(receipt.outbox_event_id)
                .await
                .expect("plans");
            assert_eq!(plans[0].status, RadrootsOutboxDeliveryPlanStatus::Complete);
        }
    }

    #[tokio::test]
    async fn claimed_delivery_targets_can_complete_as_preview_outcomes() {
        for (
            content,
            mark,
            expected_status,
            expected_plan_status,
            expected_event_state,
            expected_operation_status,
        ) in [
            (
                "proxy deferred",
                "deferred",
                RadrootsOutboxDeliveryTargetStatus::DeferredUntilImplemented,
                RadrootsOutboxDeliveryPlanStatus::DeferredUntilImplemented,
                RadrootsOutboxEventState::DeferredUntilImplemented,
                RadrootsOutboxOperationStatus::DeferredUntilImplemented,
            ),
            (
                "proxy preview unavailable",
                "preview",
                RadrootsOutboxDeliveryTargetStatus::PreviewUnavailable,
                RadrootsOutboxDeliveryPlanStatus::PreviewUnavailable,
                RadrootsOutboxEventState::PreviewUnavailable,
                RadrootsOutboxOperationStatus::PreviewUnavailable,
            ),
        ] {
            let outbox = RadrootsOutbox::open_memory().await.expect("open");
            let draft = post_draft(FIXTURE_ALICE_PUBLIC_KEY_HEX, content);
            let signed_event =
                radroots_nostr_sign_frozen_draft(&fixture_keys(), &draft).expect("signed event");
            let receipt = outbox
                .enqueue_signed_operation(RadrootsOutboxSignedOperationInput::new(
                    "publish_post",
                    draft,
                    signed_event,
                    RadrootsOutboxDeliveryPlanInput::new(
                        "transport.nostr.local",
                        1,
                        RadrootsTransportSatisfactionPolicy::all_accepted(),
                        vec![nostr_target(NOSTR_PRIMARY_WSS)],
                    ),
                    true,
                    1_007,
                    1_000,
                ))
                .await
                .expect("enqueue");
            let claimed = outbox
                .claim_next_ready_signed_event("publisher", "claim-a", 2_000, 1_000)
                .await
                .expect("claim")
                .expect("claimed");
            let target_id = claimed.delivery_targets[0].delivery_target_id;

            match mark {
                "deferred" => {
                    outbox
                        .mark_delivery_target_deferred_until_implemented(
                            receipt.outbox_event_id,
                            "claim-a",
                            target_id,
                            "transport deferred",
                            1_100,
                        )
                        .await
                        .expect("deferred");
                }
                "preview" => {
                    outbox
                        .mark_delivery_target_preview_unavailable(
                            receipt.outbox_event_id,
                            "claim-a",
                            target_id,
                            "transport preview unavailable",
                            1_100,
                        )
                        .await
                        .expect("preview unavailable");
                }
                _ => unreachable!(),
            }

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
            assert_eq!(state, expected_event_state);
            let operation = outbox
                .get_operation(receipt.operation_id)
                .await
                .expect("operation")
                .expect("operation");
            assert_eq!(operation.status, expected_operation_status);
            let targets = outbox
                .delivery_targets(receipt.outbox_event_id)
                .await
                .expect("targets");
            assert_eq!(targets[0].status, expected_status);
            assert_eq!(
                targets[0].last_outcome_kind,
                Some(outcome_kind_for_status(expected_status))
            );
            assert_eq!(targets[0].completed_at_ms, Some(1_100));
            assert_eq!(targets[0].attempt_count, 1);
            let attempts = outbox.delivery_attempts(target_id).await.expect("attempts");
            assert_eq!(attempts.len(), 1);
            assert_eq!(
                attempts[0].outcome_kind,
                outcome_kind_for_status(expected_status)
            );
            let plans = outbox
                .delivery_plans(receipt.outbox_event_id)
                .await
                .expect("plans");
            assert_eq!(plans[0].status, expected_plan_status);
            assert_eq!(
                outbox.status_summary(1_200).await.expect("summary"),
                RadrootsOutboxStatusSummary {
                    total_events: 1,
                    pending_events: 0,
                    retryable_events: 0,
                    terminal_events: 0,
                    failed_terminal_events: 0,
                    preview_unavailable_events: if expected_status
                        == RadrootsOutboxDeliveryTargetStatus::PreviewUnavailable
                    {
                        1
                    } else {
                        0
                    },
                    deferred_until_implemented_events: if expected_status
                        == RadrootsOutboxDeliveryTargetStatus::DeferredUntilImplemented
                    {
                        1
                    } else {
                        0
                    },
                    ready_signed_events: 0,
                    publishing_events: 0,
                    last_attempt_at_ms: Some(1_100),
                    last_error: None,
                }
            );
        }
    }

    #[tokio::test]
    async fn local_signed_event_ingest_records_one_idempotent_local_import_observation() {
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
        outbox
            .claim_next_ready_event("publisher", "claim-b", 3_000, 1_100)
            .await
            .expect("claim")
            .expect("publish claim");

        let first = outbox
            .ingest_signed_event_local(&event_store, receipt.outbox_event_id, "claim-b", 2_200)
            .await
            .expect("first ingest");
        assert_eq!(first.event_id, signed.id_str());
        assert!(!first.already_ingested);

        let second = outbox
            .ingest_signed_event_local(&event_store, receipt.outbox_event_id, "claim-b", 2_300)
            .await
            .expect("second ingest");
        assert!(second.already_ingested);
        let observations = event_store
            .observations_for_event(signed.id_str())
            .await
            .expect("observations");
        assert_eq!(observations.len(), 1);
        assert_eq!(observations[0].transport_kind, RadrootsTransportKind::Local);
        assert_eq!(observations[0].endpoint_uri.as_str(), "local:outbox");
        assert_eq!(
            observations[0].observation_type,
            RadrootsTransportObservationType::LocalImport
        );
        assert_eq!(observations[0].observation_count, 1);
        assert_eq!(observations[0].first_observed_at_ms, 2_200);
        assert_eq!(observations[0].last_observed_at_ms, 2_200);
    }
}
