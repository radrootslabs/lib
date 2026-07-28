mod addressable_transition_feed_v1;
mod current_visibility_v1;
pub(crate) mod food_availability_projection_v1;
mod post_core_extension_capabilities;
mod post_core_extension_dispatcher;
mod post_core_extensions_v1;
mod post_core_extensions_v2;
mod post_core_storage_v1;
mod post_core_storage_v2;
mod protocol_reconciliation_v1;
mod protocol_storage_v1;
#[cfg(test)]
mod raw_source_rebuild_v1_tests;

use self::current_visibility_v1::current_visibility_in_transaction;
use self::post_core_extension_capabilities::PostCoreExtensionCapabilities;
use self::post_core_extension_dispatcher::dispatch_post_core_extensions;
#[cfg(test)]
use self::post_core_extensions_v1::{
    candidate_id_for_mutation_for_test as candidate_id_for_mutation,
    proposal_mutation_id_for_mutation_for_test as proposal_mutation_id_for_mutation,
    seller_reservation_for_mutation_for_test as seller_reservation_for_mutation,
    sha256_hex_for_test as sha256_hex,
    target_claim_mutation_id_for_mutation_for_test as target_claim_mutation_id_for_mutation,
};
#[cfg(test)]
use self::post_core_storage_v1::{
    PostCoreStorageV1, TradeProjectionWrite, i64_from_usize_for_test as i64_from_usize,
    register_protocol_post_extension_raw_authority_forge,
    register_protocol_post_extension_schema_forge,
    trade_mutation_kind_storage_value_for_test as trade_mutation_kind_storage_value,
};
#[cfg(test)]
use self::protocol_reconciliation_v1::apply_raw_event_head;
use self::protocol_reconciliation_v1::{
    ingest_event_protocol_reconciliation_v1, validate_protocol_post_extensions,
};
use self::protocol_storage_v1::{raw_head_snapshot_in_transaction, stored_raw_event_from_row};
use crate::RadrootsEventStoreError;
use crate::model::{
    RadrootsCurrentVisibilityDecisionV1, RadrootsEventIngest, RadrootsEventIngestReceipt,
    RadrootsEventStoreRawSourceRebuildReportV1, RadrootsEventStoreSourceGeneration,
    RadrootsEventStoreStatusSummary, RadrootsEventVisibility, RadrootsProjectionCursor,
    RadrootsProjectionRebuildPrior, RadrootsProjectionRebuildTicket, RadrootsStoredEventTag,
    RadrootsStoredRawEvent, RadrootsStoredRawEventHead, RadrootsStoredSellerReservation,
    RadrootsStoredSellerReservationLine, RadrootsStoredTradeMissingParent,
    RadrootsStoredTradeMutation, RadrootsStoredTradeMutationParent,
    RadrootsStoredTradeTransportEnvelope, RadrootsStoredValidEvent, RadrootsStoredVisibleEvent,
    RadrootsStoredVisibleEventHead, RadrootsTradeProjectionCheckpoint,
    RadrootsTransportObservationType,
};
#[cfg(test)]
use crate::model::{
    RadrootsEventAdmissionStatus, RadrootsEventPersistence, RadrootsRawHeadDecision,
    RadrootsTransportObservation,
};
#[cfg(test)]
use crate::nip09::reconciliation_v1::ReconciliationProfile;
use crate::nip09::reconciliation_v1::{
    active_source_generation, generation_from_blob, preflight_projection_cursor_insert_v1,
};
use crate::schema::{
    RadrootsEventStoreSchemaStatus, inspect_event_store_schema_status, migrate_event_store_schema,
    rollback_event_store_schema_offline,
};
#[cfg(test)]
use crate::schema::{
    destroy_event_store_schema_for_test,
    rollback_event_store_schema_offline_destructive_for_migration_test,
};
use radroots_event::event_head::v1::RadrootsEventHeadCoordinate;
#[cfg(test)]
use radroots_event::event_head::v1::{
    RadrootsEventHeadCandidateResult, event_head_candidate_for_nip01_event_v1,
};
use radroots_event::ids::{
    RadrootsDTag, RadrootsEventId, RadrootsTradeId, RadrootsTradeMutationId,
};
use radroots_event::trade::RadrootsTradeMutationKindV1;
use radroots_transport::{
    RadrootsTransportKind, RadrootsTransportTarget, RadrootsTransportTargetFingerprint,
    RadrootsTransportTargetUri,
};
#[cfg(test)]
use sha2::{Digest, Sha256};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::{Connection, Row, SqliteConnection, SqlitePool};
use std::collections::BTreeMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Duration;

pub const RADROOTS_EVENT_STORE_QUERY_LIMIT_MAX: u32 = 1_000;
pub const RADROOTS_EVENT_STORE_CONTRACT_QUERY_LIMIT_MAX: usize = 16;
const FILE_JOURNAL_MODE_BUSY_RETRY_LIMIT: usize = 3;

#[derive(Clone)]
pub struct RadrootsEventStore {
    pool: SqlitePool,
}

impl RadrootsEventStore {
    pub async fn open_memory() -> Result<Self, RadrootsEventStoreError> {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")?;
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        configure_pool(&pool, false).await?;
        migrate_event_store_schema(&pool).await?;
        Ok(Self { pool })
    }

    pub async fn open_file(path: impl AsRef<Path>) -> Result<Self, RadrootsEventStoreError> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        configure_pool(&pool, true).await?;
        migrate_event_store_schema(&pool).await?;
        Ok(Self { pool })
    }

    pub async fn open_pool(
        pool: SqlitePool,
        file_backed: bool,
    ) -> Result<Self, RadrootsEventStoreError> {
        configure_pool(&pool, file_backed).await?;
        migrate_event_store_schema(&pool).await?;
        Ok(Self { pool })
    }

    /// Returns the fully trusted database-authority escape hatch.
    ///
    /// Arbitrary SQL, including reproduction of internal maintenance
    /// protocols, is outside the supported event-store mutation contract.
    /// Use the typed store methods for integrity-enforced writes.
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn schema_status(
        &self,
    ) -> Result<RadrootsEventStoreSchemaStatus, RadrootsEventStoreError> {
        inspect_event_store_schema_status(&self.pool).await
    }

    pub async fn migrate_to_current_schema(&self) -> Result<(), RadrootsEventStoreError> {
        migrate_event_store_schema(&self.pool).await
    }

    /// Terminates every clone of this store after an exclusive rollback attempt.
    ///
    /// Independent SQLite pools for the same file must be quiesced by the
    /// caller before invoking this maintenance operation.
    pub async fn rollback_to_schema_version_and_close(
        self,
        target: u32,
    ) -> Result<(), RadrootsEventStoreError> {
        let result = rollback_event_store_schema_offline(&self.pool, target).await;
        self.pool.close().await;
        result
    }

    #[cfg(test)]
    async fn destroy_schema_for_test(&self) -> Result<(), RadrootsEventStoreError> {
        destroy_event_store_schema_for_test(&self.pool).await
    }

    pub async fn pragma_foreign_keys(&self) -> Result<i64, RadrootsEventStoreError> {
        query_i64(&self.pool, "PRAGMA foreign_keys").await
    }

    pub async fn pragma_busy_timeout(&self) -> Result<i64, RadrootsEventStoreError> {
        query_i64(&self.pool, "PRAGMA busy_timeout").await
    }

    pub async fn pragma_journal_mode(&self) -> Result<String, RadrootsEventStoreError> {
        query_string(&self.pool, "PRAGMA main.journal_mode").await
    }

    pub async fn status_summary(
        &self,
    ) -> Result<RadrootsEventStoreStatusSummary, RadrootsEventStoreError> {
        inspect_event_store_status(&self.pool).await
    }

    pub async fn source_generation(
        &self,
    ) -> Result<RadrootsEventStoreSourceGeneration, RadrootsEventStoreError> {
        let mut tx = self.pool.begin().await?;
        let generation = active_source_generation(&mut tx).await?;
        tx.commit().await?;
        Ok(generation)
    }

    /// Returns the fast persisted raw-source capacity seal for one snapshot.
    ///
    /// This validates the seal against active source and generation metadata
    /// without rescanning raw rows. Migration and database reopen perform the
    /// full raw-source recount.
    pub async fn source_capacity_v1(
        &self,
    ) -> Result<crate::RadrootsEventStoreSourceCapacityV1, RadrootsEventStoreError> {
        let mut tx = self.pool.begin().await?;
        let capacity =
            crate::source_maintenance_v1::validate_source_capacity_authority_fast_v1(&mut tx)
                .await?;
        tx.commit().await?;
        Ok(capacity)
    }

    /// Rebuilds active product state solely from retained immutable raw rows.
    ///
    /// Every successful call appends one irreversible retained source
    /// generation, up to the governed history limit, and invalidates generic
    /// projection cursors by rotating the active generation. Calls at the
    /// history limit return
    /// [`RadrootsEventStoreError::SourceGenerationHistoryLimitReached`].
    pub async fn rebuild_from_raw_v1(
        &self,
    ) -> Result<RadrootsEventStoreRawSourceRebuildReportV1, RadrootsEventStoreError> {
        crate::nip09::reconciliation_v1::rebuild_from_raw_v1_on_pool(&self.pool).await
    }

    /// Repairs an exact managed-v4 file without exposing its invalid state.
    ///
    /// The database file must already exist, and the caller must quiesce every
    /// store alias, independent pool, and direct SQL user of that file for the
    /// duration of repair. The canonical path, symlink targets, and file
    /// replacement or rename operations must also remain quiesced.
    /// Caller-provided pools are intentionally unavailable;
    /// their callbacks and session state cannot be sealed. This path creates a
    /// fresh governed pool, never creates or migrates a schema, requires the
    /// existing file to use WAL journal mode, and proves its canonical path
    /// shares the reserved SQLite writer-lock domain before rebuilding in the
    /// same validated transaction. It returns a usable store only after rebuild
    /// commit. Every successful call appends one irreversible retained source
    /// generation, up to the governed history limit, and invalidates generic
    /// projection cursors by rotating the active generation. Calls at the
    /// history limit return
    /// [`RadrootsEventStoreError::SourceGenerationHistoryLimitReached`].
    pub async fn repair_file_from_raw_v1(
        path: impl AsRef<Path>,
    ) -> Result<(Self, RadrootsEventStoreRawSourceRebuildReportV1), RadrootsEventStoreError> {
        let canonical_path = canonical_raw_source_repair_main_path_v1(path.as_ref())?;
        let options = SqliteConnectOptions::new()
            .filename(&canonical_path)
            .create_if_missing(false);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        pool.set_connect_options(raw_source_repair_connect_options_v1(&canonical_path));
        let mut connection = pool.acquire().await?;
        prepare_raw_source_repair_connection_v1(&mut connection, &canonical_path).await?;
        let transaction = connection.begin_with("BEGIN IMMEDIATE").await?;
        if let Err(primary) =
            validate_raw_source_repair_canonical_lock_domain_v1(&canonical_path).await
        {
            return preserve_raw_source_repair_probe_failure(primary, transaction.rollback().await);
        }
        let report = crate::nip09::reconciliation_v1::rebuild_from_raw_v1_in_existing_transaction(
            transaction,
        )
        .await?;
        drop(connection);
        Ok((Self { pool }, report))
    }

    /// Begins a serialized write transaction suitable for composed event-store writes.
    ///
    /// Call this before performing any reads that will precede
    /// [`Self::ingest_event_in_transaction`]. A deferred SQLite transaction
    /// cannot be upgraded after another writer invalidates its snapshot.
    pub async fn begin_write_transaction(
        &self,
    ) -> Result<sqlx::Transaction<'static, sqlx::Sqlite>, RadrootsEventStoreError> {
        Ok(self.pool.begin_with("BEGIN IMMEDIATE").await?)
    }

    pub async fn ingest_event(
        &self,
        ingest: RadrootsEventIngest,
    ) -> Result<RadrootsEventIngestReceipt, RadrootsEventStoreError> {
        let mut tx = self.begin_write_transaction().await?;
        match ingest_event_in_transaction(&mut tx, ingest).await {
            Ok(receipt) => {
                tx.commit().await?;
                Ok(receipt)
            }
            Err(error) => {
                let rollback = tx.rollback().await;
                preserve_ingest_primary_failure(error, rollback)
            }
        }
    }

    /// Ingests inside a caller-owned transaction.
    ///
    /// Transactions that include preceding reads should be created with
    /// [`Self::begin_write_transaction`] so they hold the writer reservation
    /// before observing event-store state. Each call runs inside a nested
    /// savepoint, so an ingest error rolls back that call without discarding
    /// successful work already present in the caller's transaction.
    pub async fn ingest_event_in_transaction(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        ingest: RadrootsEventIngest,
    ) -> Result<RadrootsEventIngestReceipt, RadrootsEventStoreError> {
        let mut savepoint = sqlx::Acquire::begin(&mut *tx).await?;
        match ingest_event_in_transaction(&mut savepoint, ingest).await {
            Ok(receipt) => {
                savepoint.commit().await?;
                Ok(receipt)
            }
            Err(error) => {
                let rollback = savepoint.rollback().await;
                preserve_ingest_primary_failure(error, rollback)
            }
        }
    }

    pub async fn raw_event(
        &self,
        event_id: &str,
    ) -> Result<Option<RadrootsStoredRawEvent>, RadrootsEventStoreError> {
        let row = sqlx::query(
            "SELECT seq, event_id, pubkey, created_at, kind, tags_json, content, sig, raw_json, verification_status, contract_status, contract_id, event_class, projection_eligible, inserted_at_ms, updated_at_ms FROM event_envelopes WHERE event_id = ?",
        )
        .bind(event_id)
        .fetch_optional(&self.pool)
        .await?;
        row.map(stored_raw_event_from_row).transpose()
    }

    pub async fn valid_event(
        &self,
        event_id: &str,
    ) -> Result<Option<RadrootsStoredValidEvent>, RadrootsEventStoreError> {
        let Some(raw_event) = self.raw_event(event_id).await? else {
            return Ok(None);
        };
        if !raw_event.valid_stream_eligible {
            return Ok(None);
        }
        Ok(Some(RadrootsStoredValidEvent::try_from_raw(raw_event)?))
    }

    pub async fn tags_for_event(
        &self,
        event_id: &str,
    ) -> Result<Vec<RadrootsStoredEventTag>, RadrootsEventStoreError> {
        let rows = sqlx::query(
            "SELECT event_id, tag_index, tag_name, tag_value, tag_json, contract_semantic, contract_value_type, relay_indexed FROM event_envelope_tags WHERE event_id = ? ORDER BY tag_index",
        )
        .bind(event_id)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(stored_tag_from_row).collect()
    }

    pub async fn observations_for_event(
        &self,
        event_id: &str,
    ) -> Result<Vec<RadrootsTransportObservationRow>, RadrootsEventStoreError> {
        let rows = sqlx::query(
            "SELECT event_id, transport_kind, endpoint_uri, endpoint_fingerprint, observation_type, first_observed_at_ms, last_observed_at_ms, observation_count, redacted_message FROM event_transport_observation WHERE event_id = ? ORDER BY transport_kind, endpoint_uri, observation_type",
        )
        .bind(event_id)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(transport_observation_from_row)
            .collect()
    }

    /// Queries the endpoint-level v1 observation identity.
    ///
    /// This does not distinguish logical target scope or label. Scoped
    /// delivery evidence remains available from transport delivery receipts.
    pub async fn observations_for_endpoint(
        &self,
        transport_kind: RadrootsTransportKind,
        endpoint_uri: impl AsRef<str>,
    ) -> Result<Vec<RadrootsTransportObservationRow>, RadrootsEventStoreError> {
        let target = RadrootsTransportTarget::new(transport_kind, endpoint_uri)?;
        let rows = sqlx::query(
            "SELECT event_id, transport_kind, endpoint_uri, endpoint_fingerprint, observation_type, first_observed_at_ms, last_observed_at_ms, observation_count, redacted_message FROM event_transport_observation WHERE transport_kind = ? AND endpoint_fingerprint = ? ORDER BY last_observed_at_ms, event_id, observation_type",
        )
        .bind(target.kind().canonical_label())
        .bind(target.fingerprint().as_str())
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(transport_observation_from_row)
            .collect()
    }

    pub async fn raw_event_head(
        &self,
        coordinate: &RadrootsEventHeadCoordinate,
    ) -> Result<Option<RadrootsStoredRawEventHead>, RadrootsEventStoreError> {
        let mut tx = self.pool.begin().await?;
        let snapshot = raw_head_snapshot_in_transaction(&mut tx, coordinate).await?;
        tx.commit().await?;
        Ok(snapshot.map(|snapshot| snapshot.raw_head))
    }

    pub async fn event_visibility(
        &self,
        event_id: &str,
    ) -> Result<Option<RadrootsEventVisibility>, RadrootsEventStoreError> {
        let mut tx = self.pool.begin().await?;
        let visibility = event_visibility_in_transaction(&mut tx, event_id).await?;
        tx.commit().await?;
        Ok(visibility)
    }

    /// Evaluates all requested event ids against one coherent database snapshot.
    ///
    /// Results preserve input order and cardinality, including duplicate or
    /// missing ids, while each distinct id is evaluated only once. The request
    /// is bounded by [`RADROOTS_EVENT_STORE_QUERY_LIMIT_MAX`], and every id must
    /// be canonical lowercase 32-byte hex. This is the batch authority for
    /// callers that must not mix current-visibility decisions from different
    /// SQLite snapshots.
    pub async fn event_visibilities<I, S>(
        &self,
        event_ids: I,
    ) -> Result<Vec<Option<RadrootsEventVisibility>>, RadrootsEventStoreError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.event_visibilities_with_probe(event_ids, |_| async {
            Ok::<(), RadrootsEventStoreError>(())
        })
        .await
    }

    async fn event_visibilities_with_probe<I, S, F, Fut>(
        &self,
        event_ids: I,
        mut after_evaluation: F,
    ) -> Result<Vec<Option<RadrootsEventVisibility>>, RadrootsEventStoreError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
        F: FnMut(usize) -> Fut,
        Fut: Future<Output = Result<(), RadrootsEventStoreError>>,
    {
        let max = RADROOTS_EVENT_STORE_QUERY_LIMIT_MAX as usize;
        let event_ids = event_ids
            .into_iter()
            .take(max.saturating_add(1))
            .collect::<Vec<_>>();
        if event_ids.len() > max {
            return Err(RadrootsEventStoreError::EventVisibilityBatchTooLarge { max });
        }
        let event_ids = event_ids
            .into_iter()
            .map(|event_id| RadrootsEventId::parse(event_id.as_ref()))
            .collect::<Result<Vec<_>, _>>()?;
        let mut unique_event_ids = Vec::new();
        let mut seen_event_ids = BTreeMap::new();
        for event_id in &event_ids {
            if seen_event_ids.insert(event_id.clone(), ()).is_none() {
                unique_event_ids.push(event_id.clone());
            }
        }
        if unique_event_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut tx = self.pool.begin().await?;
        let mut evaluated = BTreeMap::new();
        for (index, event_id) in unique_event_ids.into_iter().enumerate() {
            let visibility = event_visibility_in_transaction(&mut tx, event_id.as_str()).await?;
            evaluated.insert(event_id, visibility);
            after_evaluation(index + 1).await?;
        }
        tx.commit().await?;

        collect_event_visibilities(event_ids, &evaluated)
    }

    pub async fn visible_event(
        &self,
        event_id: &str,
    ) -> Result<Option<RadrootsStoredVisibleEvent>, RadrootsEventStoreError> {
        let mut tx = self.pool.begin().await?;
        let Some(current) = current_visibility_in_transaction(&mut tx, event_id).await? else {
            tx.commit().await?;
            return Ok(None);
        };
        if current.decision() != RadrootsCurrentVisibilityDecisionV1::Visible {
            tx.commit().await?;
            return Ok(None);
        }
        let valid_event = RadrootsStoredValidEvent::try_from_raw(current.event)?;
        tx.commit().await?;
        Ok(Some(RadrootsStoredVisibleEvent::new(valid_event)))
    }

    pub async fn visible_event_head(
        &self,
        coordinate: &RadrootsEventHeadCoordinate,
    ) -> Result<Option<RadrootsStoredVisibleEventHead>, RadrootsEventStoreError> {
        let mut tx = self.pool.begin().await?;
        let Some(snapshot) = raw_head_snapshot_in_transaction(&mut tx, coordinate).await? else {
            tx.commit().await?;
            return Ok(None);
        };
        let raw_head = snapshot.raw_head;
        let current = require_raw_head_visibility(
            &raw_head,
            current_visibility_in_transaction(&mut tx, raw_head.event_id.as_str()).await?,
        )?;
        if current.decision() != RadrootsCurrentVisibilityDecisionV1::Visible {
            tx.commit().await?;
            return Ok(None);
        }
        let valid_event = RadrootsStoredValidEvent::try_from_raw(current.event().clone())?;
        let event = RadrootsStoredVisibleEvent::new(valid_event);
        tx.commit().await?;
        Ok(Some(RadrootsStoredVisibleEventHead::new(raw_head, event)))
    }

    pub async fn projection_cursor(
        &self,
        projection_id: &str,
        expected_projection_version: u32,
    ) -> Result<Option<RadrootsProjectionCursor>, RadrootsEventStoreError> {
        validate_projection_identity(projection_id, expected_projection_version)?;
        let mut tx = self.pool.begin().await?;
        let active_generation = active_source_generation(&mut tx).await?;
        let row = sqlx::query(
            "SELECT cursor.projection_id, cursor.projection_version, cursor.last_event_seq, cursor.updated_at_ms, source.source_generation, source.source_revision FROM projection_cursor AS cursor LEFT JOIN radroots_event_store_projection_cursor_source AS source ON source.projection_id = cursor.projection_id WHERE cursor.projection_id = ?",
        )
        .bind(projection_id)
        .fetch_optional(&mut *tx)
        .await?;
        let cursor = row
            .map(|row| projection_cursor_from_row(row, active_generation))
            .transpose()?;
        if let Some(cursor) = cursor.as_ref() {
            validate_projection_cursor_high_water(
                &mut tx,
                cursor.projection_id(),
                cursor.last_event_seq(),
            )
            .await?;
        }
        if let Some(cursor) = cursor.as_ref()
            && cursor.projection_version != expected_projection_version
        {
            return Err(RadrootsEventStoreError::ProjectionVersionMismatch {
                projection_id: projection_id.to_owned(),
                expected: expected_projection_version,
                actual: cursor.projection_version,
            });
        }
        tx.commit().await?;
        Ok(cursor)
    }

    pub async fn compare_and_swap_projection_cursor(
        &self,
        cursor: &RadrootsProjectionCursor,
        expected_prior_sequence: Option<i64>,
    ) -> Result<(), RadrootsEventStoreError> {
        validate_projection_identity(cursor.projection_id(), cursor.projection_version())?;
        validate_projection_sequence(cursor.projection_id(), cursor.last_event_seq())?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let active_generation = active_source_generation(&mut tx).await?;
        validate_projection_cursor_high_water(
            &mut tx,
            cursor.projection_id(),
            cursor.last_event_seq(),
        )
        .await?;
        if cursor.source_generation() != active_generation {
            return Err(
                RadrootsEventStoreError::ProjectionSourceGenerationMismatch {
                    projection_id: cursor.projection_id().to_owned(),
                },
            );
        }
        let existing =
            projection_cursor_unchecked(&mut tx, cursor.projection_id(), active_generation).await?;
        match expected_prior_sequence {
            None => {
                if existing.is_none() {
                    preflight_projection_cursor_insert_v1(&mut tx).await?;
                }
                let inserted = sqlx::query(
                    "INSERT OR IGNORE INTO projection_cursor(projection_id, projection_version, last_event_seq, updated_at_ms) VALUES (?, ?, ?, ?)",
                )
                .bind(cursor.projection_id())
                .bind(i64::from(cursor.projection_version()))
                .bind(cursor.last_event_seq())
                .bind(cursor.updated_at_ms())
                .execute(&mut *tx)
                .await?;
                if inserted.rows_affected() == 1 {
                    tx.commit().await?;
                    return Ok(());
                }
            }
            Some(expected) => {
                if cursor.last_event_seq() < expected {
                    return Err(RadrootsEventStoreError::ProjectionCursorRegression {
                        projection_id: cursor.projection_id().to_owned(),
                        current: expected,
                        proposed: cursor.last_event_seq(),
                    });
                }
                let updated = sqlx::query(
                    "UPDATE projection_cursor SET last_event_seq = ?, updated_at_ms = ? WHERE projection_id = ? AND projection_version = ? AND last_event_seq = ? AND EXISTS (SELECT 1 FROM radroots_event_store_projection_cursor_source AS source WHERE source.projection_id = projection_cursor.projection_id AND source.source_generation = ?)",
                )
                .bind(cursor.last_event_seq())
                .bind(cursor.updated_at_ms())
                .bind(cursor.projection_id())
                .bind(i64::from(cursor.projection_version()))
                .bind(expected)
                .bind(active_generation.as_bytes().as_slice())
                .execute(&mut *tx)
                .await?;
                if updated.rows_affected() == 1 {
                    tx.commit().await?;
                    return Ok(());
                }
            }
        }

        let actual =
            projection_cursor_unchecked(&mut tx, cursor.projection_id(), active_generation).await?;
        if let Some(actual) = actual.as_ref() {
            if actual.projection_version() != cursor.projection_version() {
                return Err(RadrootsEventStoreError::ProjectionVersionMismatch {
                    projection_id: cursor.projection_id().to_owned(),
                    expected: cursor.projection_version(),
                    actual: actual.projection_version(),
                });
            }
            if cursor.last_event_seq() < actual.last_event_seq() {
                return Err(RadrootsEventStoreError::ProjectionCursorRegression {
                    projection_id: cursor.projection_id().to_owned(),
                    current: actual.last_event_seq(),
                    proposed: cursor.last_event_seq(),
                });
            }
        }
        Err(RadrootsEventStoreError::ProjectionCursorConflict {
            projection_id: cursor.projection_id().to_owned(),
            expected: expected_prior_sequence,
            actual: actual.map(|cursor| cursor.last_event_seq()),
        })
    }

    pub async fn prepare_projection_cursor_rebuild(
        &self,
        projection_id: impl Into<String>,
        target_projection_version: u32,
    ) -> Result<RadrootsProjectionRebuildTicket, RadrootsEventStoreError> {
        let projection_id = projection_id.into();
        validate_projection_identity(projection_id.as_str(), target_projection_version)?;
        let mut tx = self.pool.begin().await?;
        let target_source_generation = active_source_generation(&mut tx).await?;
        let target_raw_high_water_seq: i64 = sqlx::query_scalar(
            "SELECT raw_high_water_seq FROM radroots_event_store_source_state WHERE singleton = 1",
        )
        .fetch_one(&mut *tx)
        .await?;
        let prior = sqlx::query(
            "SELECT cursor.projection_version, cursor.last_event_seq, cursor.updated_at_ms, source.source_generation, source.source_revision FROM projection_cursor AS cursor LEFT JOIN radroots_event_store_projection_cursor_source AS source ON source.projection_id = cursor.projection_id WHERE cursor.projection_id = ?",
        )
        .bind(projection_id.as_str())
        .fetch_optional(&mut *tx)
        .await?;
        let prior = if let Some(prior) = prior {
            let projection_version = projection_version_from_i64(
                projection_id.as_str(),
                prior.try_get("projection_version")?,
            )?;
            let last_event_seq: i64 = prior.try_get("last_event_seq")?;
            validate_projection_sequence(projection_id.as_str(), last_event_seq)?;
            if last_event_seq > target_raw_high_water_seq {
                return Err(RadrootsEventStoreError::ProjectionCursorAheadOfSource {
                    projection_id,
                    proposed: last_event_seq,
                    high_water: target_raw_high_water_seq,
                });
            }
            let source_generation = prior
                .try_get::<Option<Vec<u8>>, _>("source_generation")?
                .map(generation_from_blob)
                .transpose()?;
            let source_revision = projection_source_revision_from_i64(
                projection_id.as_str(),
                prior.try_get("source_revision")?,
            )?;
            if source_generation == Some(target_source_generation)
                && projection_version == target_projection_version
            {
                return Err(RadrootsEventStoreError::ProjectionRebuildNotRequired {
                    projection_id,
                    projection_version,
                });
            }
            RadrootsProjectionRebuildPrior::Cursor {
                source_generation,
                source_revision,
                projection_version,
                last_event_seq,
                updated_at_ms: prior.try_get("updated_at_ms")?,
            }
        } else {
            RadrootsProjectionRebuildPrior::Missing
        };
        tx.commit().await?;
        Ok(RadrootsProjectionRebuildTicket {
            projection_id,
            target_projection_version,
            target_source_generation,
            target_raw_high_water_seq,
            prior,
        })
    }

    pub async fn reset_projection_cursor_after_rebuild(
        &self,
        ticket: RadrootsProjectionRebuildTicket,
        updated_at_ms: i64,
    ) -> Result<RadrootsProjectionCursor, RadrootsEventStoreError> {
        let RadrootsProjectionRebuildTicket {
            projection_id,
            target_projection_version,
            target_source_generation,
            target_raw_high_water_seq,
            prior: expected_prior,
        } = ticket;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let source_generation = active_source_generation(&mut tx).await?;
        if source_generation != target_source_generation {
            return Err(
                RadrootsEventStoreError::ProjectionSourceGenerationMismatch { projection_id },
            );
        }
        let current_high_water: i64 = sqlx::query_scalar(
            "SELECT raw_high_water_seq FROM radroots_event_store_source_state WHERE singleton = 1",
        )
        .fetch_one(&mut *tx)
        .await?;
        if target_raw_high_water_seq > current_high_water {
            return Err(RadrootsEventStoreError::ProjectionCursorAheadOfSource {
                projection_id,
                proposed: target_raw_high_water_seq,
                high_water: current_high_water,
            });
        }
        let actual_prior = sqlx::query(
            "SELECT cursor.projection_version, cursor.last_event_seq, cursor.updated_at_ms, source.source_generation, source.source_revision FROM projection_cursor AS cursor LEFT JOIN radroots_event_store_projection_cursor_source AS source ON source.projection_id = cursor.projection_id WHERE cursor.projection_id = ?",
        )
        .bind(projection_id.as_str())
        .fetch_optional(&mut *tx)
        .await?;
        match (expected_prior, actual_prior) {
            (RadrootsProjectionRebuildPrior::Missing, None) => {
                preflight_projection_cursor_insert_v1(&mut tx).await?;
                let inserted = sqlx::query(
                    "INSERT OR IGNORE INTO projection_cursor(projection_id, projection_version, last_event_seq, updated_at_ms) VALUES (?, ?, ?, ?)",
                )
                .bind(projection_id.as_str())
                .bind(i64::from(target_projection_version))
                .bind(target_raw_high_water_seq)
                .bind(updated_at_ms)
                .execute(&mut *tx)
                .await?;
                ensure_projection_rebuild_row_changed(
                    projection_id.as_str(),
                    inserted.rows_affected(),
                )?;
            }
            (
                RadrootsProjectionRebuildPrior::Cursor {
                    source_generation: expected_generation,
                    source_revision: expected_revision,
                    projection_version: expected_version,
                    last_event_seq: expected_sequence,
                    updated_at_ms: expected_updated_at_ms,
                },
                Some(actual),
            ) => {
                let actual_generation = actual
                    .try_get::<Option<Vec<u8>>, _>("source_generation")?
                    .map(generation_from_blob)
                    .transpose()?;
                let actual_revision = projection_source_revision_from_i64(
                    projection_id.as_str(),
                    actual.try_get("source_revision")?,
                )?;
                let actual_version = projection_version_from_i64(
                    projection_id.as_str(),
                    actual.try_get("projection_version")?,
                )?;
                let actual_sequence: i64 = actual.try_get("last_event_seq")?;
                let actual_updated_at_ms: i64 = actual.try_get("updated_at_ms")?;
                if actual_generation != expected_generation
                    || actual_revision != expected_revision
                    || actual_version != expected_version
                    || actual_sequence != expected_sequence
                    || actual_updated_at_ms != expected_updated_at_ms
                {
                    return Err(RadrootsEventStoreError::ProjectionRebuildTicketConflict {
                        projection_id,
                    });
                }
                let expected_revision_i64 =
                    projection_source_revision_to_i64(projection_id.as_str(), expected_revision)?;
                let updated = sqlx::query(
                    "UPDATE projection_cursor SET projection_version = ?, last_event_seq = ?, updated_at_ms = ? WHERE projection_id = ? AND projection_version = ? AND last_event_seq = ? AND updated_at_ms = ? AND EXISTS (SELECT 1 FROM radroots_event_store_projection_cursor_source AS source WHERE source.projection_id = projection_cursor.projection_id AND source.source_generation IS ? AND source.source_revision = ?)",
                )
                .bind(i64::from(target_projection_version))
                .bind(target_raw_high_water_seq)
                .bind(updated_at_ms)
                .bind(projection_id.as_str())
                .bind(i64::from(expected_version))
                .bind(expected_sequence)
                .bind(expected_updated_at_ms)
                .bind(
                    expected_generation
                        .as_ref()
                        .map(|generation| generation.as_bytes().as_slice()),
                )
                .bind(expected_revision_i64)
                .execute(&mut *tx)
                .await?;
                ensure_projection_rebuild_row_changed(
                    projection_id.as_str(),
                    updated.rows_affected(),
                )?;
            }
            _ => {
                return Err(RadrootsEventStoreError::ProjectionRebuildTicketConflict {
                    projection_id,
                });
            }
        }
        tx.commit().await?;
        RadrootsProjectionCursor::new(
            projection_id,
            target_projection_version,
            source_generation,
            target_raw_high_water_seq,
            updated_at_ms,
        )
    }

    pub async fn valid_stream_after(
        &self,
        after_sequence: i64,
        limit: u32,
    ) -> Result<Vec<RadrootsStoredValidEvent>, RadrootsEventStoreError> {
        validate_event_query_limit(limit)?;
        let rows = sqlx::query(
            "SELECT seq, event_id, pubkey, created_at, kind, tags_json, content, sig, raw_json, verification_status, contract_status, contract_id, event_class, projection_eligible, inserted_at_ms, updated_at_ms FROM event_envelopes WHERE verification_status = 'verified' AND contract_status = 'admitted' AND projection_eligible = 1 AND seq > ? ORDER BY seq ASC LIMIT ?",
        )
        .bind(after_sequence)
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(stored_raw_event_from_row)
            .map(|event| event.and_then(RadrootsStoredValidEvent::try_from_raw))
            .collect()
    }

    pub async fn raw_events_after(
        &self,
        after_sequence: i64,
        limit: u32,
    ) -> Result<Vec<RadrootsStoredRawEvent>, RadrootsEventStoreError> {
        validate_event_query_limit(limit)?;
        let rows = sqlx::query(
            "SELECT seq, event_id, pubkey, created_at, kind, tags_json, content, sig, raw_json, verification_status, contract_status, contract_id, event_class, projection_eligible, inserted_at_ms, updated_at_ms FROM event_envelopes WHERE seq > ? ORDER BY seq ASC LIMIT ?",
        )
        .bind(after_sequence)
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(stored_raw_event_from_row).collect()
    }

    pub async fn raw_events_by_tag(
        &self,
        tag_name: &str,
        tag_value: &str,
        limit: u32,
    ) -> Result<Vec<RadrootsStoredRawEvent>, RadrootsEventStoreError> {
        validate_tag_query(tag_name, limit)?;
        let rows = sqlx::query(
            "SELECT seq, event_id, pubkey, created_at, kind, tags_json, content, sig, raw_json, verification_status, contract_status, contract_id, event_class, projection_eligible, inserted_at_ms, updated_at_ms FROM event_envelopes AS event WHERE EXISTS (SELECT 1 FROM event_envelope_tags AS tag WHERE tag.event_id = event.event_id AND tag.tag_name = ? AND tag.tag_value = ?) ORDER BY event.seq ASC LIMIT ?",
        )
        .bind(tag_name)
        .bind(tag_value)
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(stored_raw_event_from_row).collect()
    }

    pub async fn valid_stream_by_tag(
        &self,
        tag_name: &str,
        tag_value: &str,
        limit: u32,
    ) -> Result<Vec<RadrootsStoredValidEvent>, RadrootsEventStoreError> {
        validate_tag_query(tag_name, limit)?;
        let rows = sqlx::query(
            "SELECT seq, event_id, pubkey, created_at, kind, tags_json, content, sig, raw_json, verification_status, contract_status, contract_id, event_class, projection_eligible, inserted_at_ms, updated_at_ms FROM event_envelopes AS event WHERE verification_status = 'verified' AND contract_status = 'admitted' AND projection_eligible = 1 AND EXISTS (SELECT 1 FROM event_envelope_tags AS tag WHERE tag.event_id = event.event_id AND tag.tag_name = ? AND tag.tag_value = ?) ORDER BY event.seq ASC LIMIT ?",
        )
        .bind(tag_name)
        .bind(tag_value)
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(stored_raw_event_from_row)
            .map(|event| event.and_then(RadrootsStoredValidEvent::try_from_raw))
            .collect()
    }

    pub async fn valid_stream_by_contract_and_tag<S>(
        &self,
        contract_ids: &[S],
        tag_name: &str,
        tag_value: &str,
        limit: u32,
    ) -> Result<Vec<RadrootsStoredValidEvent>, RadrootsEventStoreError>
    where
        S: AsRef<str>,
    {
        validate_contract_tag_query(contract_ids, tag_name, limit)?;
        let placeholders = core::iter::repeat_n("?", contract_ids.len())
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT seq, event_id, pubkey, created_at, kind, tags_json, content, sig, raw_json, verification_status, contract_status, contract_id, event_class, projection_eligible, inserted_at_ms, updated_at_ms FROM event_envelopes AS event WHERE verification_status = 'verified' AND contract_status = 'admitted' AND projection_eligible = 1 AND contract_id IN ({placeholders}) AND EXISTS (SELECT 1 FROM event_envelope_tags AS tag WHERE tag.event_id = event.event_id AND tag.tag_name = ? AND tag.tag_value = ?) ORDER BY event.seq ASC LIMIT ?"
        );
        let mut query = sqlx::query(sqlx::AssertSqlSafe(sql));
        for contract_id in contract_ids {
            query = query.bind(contract_id.as_ref());
        }
        let rows = query
            .bind(tag_name)
            .bind(tag_value)
            .bind(i64::from(limit))
            .fetch_all(&self.pool)
            .await?;
        rows.into_iter()
            .map(stored_raw_event_from_row)
            .map(|event| event.and_then(RadrootsStoredValidEvent::try_from_raw))
            .collect()
    }

    pub async fn get_trade_mutation(
        &self,
        mutation_id: &RadrootsTradeMutationId,
    ) -> Result<Option<RadrootsStoredTradeMutation>, RadrootsEventStoreError> {
        let row = sqlx::query(
            "SELECT mutation_id, trade_id, root_mutation_id, contract_id, mutation_kind, schema_version, candidate_id, proposal_mutation_id, target_claim_mutation_id, author_pubkey, counterparty_pubkey, buyer_pubkey, seller_pubkey, farm_id, authored_at_unix_s, canonical_payload_bytes, payload_sha256, first_event_seq, first_transport_event_id, inserted_at_ms FROM trade_mutation WHERE mutation_id = ?",
        )
        .bind(mutation_id.as_str())
        .fetch_optional(&self.pool)
        .await?;
        row.map(trade_mutation_from_row).transpose()
    }

    pub async fn trade_mutations_for_trade(
        &self,
        trade_id: &RadrootsTradeId,
        limit: u32,
    ) -> Result<Vec<RadrootsStoredTradeMutation>, RadrootsEventStoreError> {
        validate_trade_query_limit(limit)?;
        let rows = sqlx::query(
            "SELECT mutation_id, trade_id, root_mutation_id, contract_id, mutation_kind, schema_version, candidate_id, proposal_mutation_id, target_claim_mutation_id, author_pubkey, counterparty_pubkey, buyer_pubkey, seller_pubkey, farm_id, authored_at_unix_s, canonical_payload_bytes, payload_sha256, first_event_seq, first_transport_event_id, inserted_at_ms FROM trade_mutation WHERE trade_id = ? ORDER BY authored_at_unix_s, mutation_id LIMIT ?",
        )
        .bind(trade_id.as_str())
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(trade_mutation_from_row).collect()
    }

    pub async fn trade_mutation_parents(
        &self,
        mutation_id: &RadrootsTradeMutationId,
    ) -> Result<Vec<RadrootsStoredTradeMutationParent>, RadrootsEventStoreError> {
        let rows = sqlx::query(
            "SELECT mutation_id, parent_mutation_id, parent_index FROM trade_mutation_parent WHERE mutation_id = ? ORDER BY parent_index",
        )
        .bind(mutation_id.as_str())
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(trade_mutation_parent_from_row)
            .collect()
    }

    pub async fn trade_transport_envelopes_for_mutation(
        &self,
        mutation_id: &RadrootsTradeMutationId,
    ) -> Result<Vec<RadrootsStoredTradeTransportEnvelope>, RadrootsEventStoreError> {
        let rows = sqlx::query(
            "SELECT transport_event_id, mutation_id, trade_id, transport_kind, pubkey, created_at, event_seq, payload_sha256, observed_at_ms FROM trade_transport_envelope WHERE mutation_id = ? ORDER BY observed_at_ms, transport_event_id",
        )
        .bind(mutation_id.as_str())
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(trade_transport_envelope_from_row)
            .collect()
    }

    pub async fn missing_trade_parents(
        &self,
        trade_id: &RadrootsTradeId,
    ) -> Result<Vec<RadrootsStoredTradeMissingParent>, RadrootsEventStoreError> {
        let rows = sqlx::query(
            "SELECT trade_id, mutation_id, missing_parent_mutation_id, first_transport_event_id, first_seen_at_ms FROM trade_missing_parent WHERE trade_id = ? ORDER BY first_seen_at_ms, mutation_id, missing_parent_mutation_id",
        )
        .bind(trade_id.as_str())
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(trade_missing_parent_from_row)
            .collect()
    }

    pub async fn seller_reservation(
        &self,
        reservation_id: &RadrootsDTag,
    ) -> Result<Option<RadrootsStoredSellerReservation>, RadrootsEventStoreError> {
        let row = sqlx::query(
            "SELECT reservation_id, trade_id, candidate_id, claim_mutation_id, inventory_authority_pubkey, inventory_epoch, assertion_commitment, reservation_expires_at_unix_s, reservation_json, inserted_at_ms FROM seller_inventory_reservation WHERE reservation_id = ?",
        )
        .bind(reservation_id.as_str())
        .fetch_optional(&self.pool)
        .await?;
        row.map(seller_reservation_from_row).transpose()
    }

    pub async fn seller_reservation_lines(
        &self,
        reservation_id: &RadrootsDTag,
    ) -> Result<Vec<RadrootsStoredSellerReservationLine>, RadrootsEventStoreError> {
        let rows = sqlx::query(
            "SELECT reservation_id, line_id, bin_id, quantity_mantissa, quantity_scale, unit_code, line_index FROM seller_inventory_reservation_line WHERE reservation_id = ? ORDER BY line_index",
        )
        .bind(reservation_id.as_str())
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(seller_reservation_line_from_row)
            .collect()
    }

    pub async fn update_trade_projection_checkpoint(
        &self,
        checkpoint: &RadrootsTradeProjectionCheckpoint,
    ) -> Result<(), RadrootsEventStoreError> {
        sqlx::query(
            "INSERT INTO trade_projection_checkpoint(trade_id, reducer_contract_id, reducer_version, projection_digest, root_mutation_id, negotiation_state, agreement_state, evidence_state, conflict_state, private_terms_state, attestation_state, fulfillment_state, payment_state, projection_json, last_mutation_id, last_transport_event_seq, updated_at_ms) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(trade_id) DO UPDATE SET reducer_contract_id = excluded.reducer_contract_id, reducer_version = excluded.reducer_version, projection_digest = excluded.projection_digest, root_mutation_id = excluded.root_mutation_id, negotiation_state = excluded.negotiation_state, agreement_state = excluded.agreement_state, evidence_state = excluded.evidence_state, conflict_state = excluded.conflict_state, private_terms_state = excluded.private_terms_state, attestation_state = excluded.attestation_state, fulfillment_state = excluded.fulfillment_state, payment_state = excluded.payment_state, projection_json = excluded.projection_json, last_mutation_id = excluded.last_mutation_id, last_transport_event_seq = excluded.last_transport_event_seq, updated_at_ms = excluded.updated_at_ms",
        )
        .bind(checkpoint.trade_id.as_str())
        .bind(checkpoint.reducer_contract_id.as_str())
        .bind(i64::from(checkpoint.reducer_version))
        .bind(checkpoint.projection_digest.as_str())
        .bind(checkpoint.root_mutation_id.as_ref().map(RadrootsTradeMutationId::as_str))
        .bind(checkpoint.negotiation_state.as_str())
        .bind(checkpoint.agreement_state.as_str())
        .bind(checkpoint.evidence_state.as_str())
        .bind(checkpoint.conflict_state.as_str())
        .bind(checkpoint.private_terms_state.as_str())
        .bind(checkpoint.attestation_state.as_str())
        .bind(checkpoint.fulfillment_state.as_str())
        .bind(checkpoint.payment_state.as_str())
        .bind(checkpoint.projection_json.as_str())
        .bind(checkpoint.last_mutation_id.as_ref().map(RadrootsTradeMutationId::as_str))
        .bind(checkpoint.last_transport_event_seq)
        .bind(checkpoint.updated_at_ms)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn trade_projection_checkpoint(
        &self,
        trade_id: &RadrootsTradeId,
    ) -> Result<Option<RadrootsTradeProjectionCheckpoint>, RadrootsEventStoreError> {
        let row = sqlx::query(
            "SELECT trade_id, reducer_contract_id, reducer_version, projection_digest, root_mutation_id, negotiation_state, agreement_state, evidence_state, conflict_state, private_terms_state, attestation_state, fulfillment_state, payment_state, projection_json, last_mutation_id, last_transport_event_seq, updated_at_ms FROM trade_projection_checkpoint WHERE trade_id = ?",
        )
        .bind(trade_id.as_str())
        .fetch_optional(&self.pool)
        .await?;
        row.map(trade_projection_checkpoint_from_row).transpose()
    }
}

fn collect_event_visibilities(
    event_ids: Vec<RadrootsEventId>,
    evaluated: &BTreeMap<RadrootsEventId, Option<RadrootsEventVisibility>>,
) -> Result<Vec<Option<RadrootsEventVisibility>>, RadrootsEventStoreError> {
    event_ids
        .into_iter()
        .map(|event_id| {
            evaluated.get(&event_id).cloned().ok_or_else(|| {
                RadrootsEventStoreError::CurrentVisibilityDrift {
                    reason: format!("event visibility batch lost evaluated id `{event_id}`"),
                }
            })
        })
        .collect()
}

fn require_raw_head_visibility(
    raw_head: &RadrootsStoredRawEventHead,
    current: Option<crate::model::RadrootsCurrentEventVisibilityV1>,
) -> Result<crate::model::RadrootsCurrentEventVisibilityV1, RadrootsEventStoreError> {
    current.ok_or_else(|| RadrootsEventStoreError::StoredHeadInconsistent {
        event_id: raw_head.event_id.clone(),
    })
}

async fn event_visibility_in_transaction(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    event_id: &str,
) -> Result<Option<RadrootsEventVisibility>, RadrootsEventStoreError> {
    let Some(current) = current_visibility_in_transaction(tx, event_id).await? else {
        return Ok(None);
    };
    Ok(Some(event_visibility_from_current(event_id, &current)?))
}

fn event_visibility_from_current(
    event_id: &str,
    current: &crate::model::RadrootsCurrentEventVisibilityV1,
) -> Result<RadrootsEventVisibility, RadrootsEventStoreError> {
    let visibility = match current.decision() {
        RadrootsCurrentVisibilityDecisionV1::Visible => RadrootsEventVisibility::Visible,
        RadrootsCurrentVisibilityDecisionV1::NotAdmitted => RadrootsEventVisibility::NotAdmitted,
        RadrootsCurrentVisibilityDecisionV1::NotCurrent => RadrootsEventVisibility::NotCurrent {
            raw_head_event_id: current
                .raw_head_event_id()
                .ok_or_else(
                    || RadrootsEventStoreError::StoredHeadCoordinateUnavailable {
                        event_id: event_id.to_owned(),
                    },
                )?
                .as_str()
                .to_owned(),
        },
        RadrootsCurrentVisibilityDecisionV1::Suppressed => {
            let evidence = current.suppression().ok_or_else(|| {
                RadrootsEventStoreError::CurrentVisibilityDrift {
                    reason: format!(
                        "suppressed current visibility is missing evidence for `{event_id}`"
                    ),
                }
            })?;
            RadrootsEventVisibility::Suppressed {
                reason: evidence.reason,
                event_reference_request_id: evidence.event_reference_request_id.clone(),
                address_reference_request_id: evidence.address_reference_request_id.clone(),
                address_reference_cutoff: evidence.address_reference_cutoff,
            }
        }
    };
    Ok(visibility)
}

/// Inspects an existing event-store pool without configuring or migrating it.
///
/// The inspection uses one read transaction and applies the same fail-closed
/// classification checks as [`RadrootsEventStore::status_summary`]. Callers
/// must supply a pool whose event-store schema has already been initialized.
pub async fn inspect_event_store_status(
    pool: &SqlitePool,
) -> Result<RadrootsEventStoreStatusSummary, RadrootsEventStoreError> {
    let mut tx = pool.begin().await?;
    crate::schema::validate_event_store_temp_schema(&mut tx).await?;
    let inconsistent_event_id: Option<String> = sqlx::query_scalar(
        "SELECT event_id FROM main.event_envelopes WHERE contract_status NOT IN ('supported', 'unsupported_kind', 'unsupported_shape', 'ambiguous_shape') AND (verification_status != 'verified' OR contract_status NOT IN ('admitted', 'unsupported', 'invalid') OR kind < 0 OR kind > 65535 OR kind BETWEEN 20000 AND 29999 OR event_class IS NULL OR event_class != CASE WHEN kind = 0 OR kind = 3 OR kind BETWEEN 10000 AND 19999 THEN 'replaceable' WHEN kind BETWEEN 30000 AND 39999 THEN 'addressable' ELSE 'regular' END OR projection_eligible NOT IN (0, 1) OR projection_eligible != CASE WHEN contract_status = 'admitted' THEN 1 ELSE 0 END OR (contract_status = 'admitted') != (contract_id IS NOT NULL)) LIMIT 1",
    )
    .fetch_optional(&mut *tx)
    .await?;
    if let Some(event_id) = inconsistent_event_id {
        return Err(RadrootsEventStoreError::StoredRawEventClassificationInconsistent { event_id });
    }
    let row = sqlx::query(
        "SELECT COUNT(*) AS total_events, COALESCE(SUM(CASE WHEN verification_status = 'verified' AND contract_status = 'admitted' AND contract_id IS NOT NULL AND projection_eligible = 1 AND kind BETWEEN 0 AND 65535 AND NOT (kind BETWEEN 20000 AND 29999) AND event_class = CASE WHEN kind = 0 OR kind = 3 OR kind BETWEEN 10000 AND 19999 THEN 'replaceable' WHEN kind BETWEEN 30000 AND 39999 THEN 'addressable' ELSE 'regular' END THEN 1 ELSE 0 END), 0) AS valid_stream_events, MAX(seq) AS last_event_seq, MAX(updated_at_ms) AS last_event_updated_at_ms FROM main.event_envelopes",
    )
    .fetch_one(&mut *tx)
    .await?;
    let transport_observations: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM main.event_transport_observation")
            .fetch_one(&mut *tx)
            .await?;
    let summary = RadrootsEventStoreStatusSummary {
        total_events: row.try_get("total_events")?,
        valid_stream_events: row.try_get("valid_stream_events")?,
        transport_observations,
        last_event_seq: row.try_get("last_event_seq")?,
        last_event_updated_at_ms: row.try_get("last_event_updated_at_ms")?,
    };
    tx.commit().await?;
    Ok(summary)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTransportObservationRow {
    pub event_id: String,
    pub transport_kind: RadrootsTransportKind,
    pub endpoint_uri: RadrootsTransportTargetUri,
    pub endpoint_fingerprint: RadrootsTransportTargetFingerprint,
    pub observation_type: RadrootsTransportObservationType,
    pub first_observed_at_ms: i64,
    pub last_observed_at_ms: i64,
    pub observation_count: i64,
    pub caller_redacted_message: Option<crate::model::RadrootsTransportObservationMessage>,
}

async fn configure_pool(
    pool: &SqlitePool,
    file_backed: bool,
) -> Result<(), RadrootsEventStoreError> {
    let max_connections = pool.options().get_max_connections();
    if !file_backed && max_connections != 1 {
        return Err(RadrootsEventStoreError::UnsafeInMemoryPoolConnectionCount {
            actual: max_connections,
        });
    }

    let mut connections = Vec::with_capacity(max_connections as usize);
    for _ in 0..max_connections {
        connections.push(pool.acquire().await?);
    }
    for connection in &mut connections {
        let main_filename = main_database_filename(connection).await?;
        let database_is_memory = main_filename.is_empty();
        if file_backed == database_is_memory {
            return Err(RadrootsEventStoreError::SqlitePoolBackingMismatch {
                file_backed,
                filename: main_filename,
            });
        }
        validate_main_database_encoding(connection).await?;
        crate::schema::validate_event_store_temp_schema(connection).await?;
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&mut **connection)
            .await?;
        sqlx::query("PRAGMA busy_timeout = 5000")
            .execute(&mut **connection)
            .await?;
        if file_backed {
            configure_file_journal_mode(connection).await?;
        }
    }
    let existing_options = pool.connect_options();
    let connect_options = existing_options
        .as_ref()
        .clone()
        .foreign_keys(true)
        .busy_timeout(Duration::from_millis(5_000));
    let connect_options = if file_backed {
        connect_options.journal_mode(SqliteJournalMode::Wal)
    } else {
        connect_options
    };
    pool.set_connect_options(connect_options);
    Ok(())
}

async fn prepare_raw_source_repair_connection_v1(
    connection: &mut SqliteConnection,
    canonical_path: &Path,
) -> Result<(), RadrootsEventStoreError> {
    let main_filename = main_database_filename(connection).await?;
    let actual = canonical_raw_source_repair_main_path_v1(Path::new(&main_filename))?;
    if actual != canonical_path {
        return Err(
            RadrootsEventStoreError::RawSourceRepairDatabaseIdentityMismatch {
                expected: canonical_path.display().to_string(),
                actual: actual.display().to_string(),
            },
        );
    }
    validate_main_database_encoding(connection).await?;
    crate::schema::validate_exact_managed_v4_for_raw_source_rebuild_v1(connection).await?;
    validate_file_journal_mode_is_wal(connection).await?;
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&mut *connection)
        .await?;
    sqlx::query("PRAGMA busy_timeout = 5000")
        .execute(&mut *connection)
        .await?;
    Ok(())
}

fn raw_source_repair_connect_options_v1(canonical_path: &Path) -> SqliteConnectOptions {
    SqliteConnectOptions::new()
        .filename(canonical_path)
        .create_if_missing(false)
        .journal_mode(SqliteJournalMode::Wal)
        .foreign_keys(true)
        .busy_timeout(Duration::from_millis(5_000))
}

async fn validate_raw_source_repair_canonical_lock_domain_v1(
    canonical_path: &Path,
) -> Result<(), RadrootsEventStoreError> {
    let mut candidate = SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(canonical_path)
            .create_if_missing(false)
            .foreign_keys(true)
            .busy_timeout(Duration::ZERO),
    )
    .await?;
    let candidate_filename = main_database_filename(&mut candidate).await?;
    let candidate_path = canonical_raw_source_repair_main_path_v1(Path::new(&candidate_filename))?;
    if candidate_path != canonical_path {
        return Err(
            RadrootsEventStoreError::RawSourceRepairDatabaseIdentityMismatch {
                expected: canonical_path.display().to_string(),
                actual: candidate_path.display().to_string(),
            },
        );
    }
    validate_main_database_encoding(&mut candidate).await?;
    crate::schema::validate_exact_managed_v4_for_raw_source_rebuild_v1(&mut candidate).await?;
    validate_file_journal_mode_is_wal(&mut candidate).await?;

    let mut probe = candidate.begin().await?;
    let write = sqlx::query(
        "UPDATE main.radroots_event_store_write_lock SET lock_version = lock_version WHERE singleton = 1",
    )
    .execute(&mut *probe)
    .await;
    let rollback = probe.rollback().await;
    match write {
        Ok(_) => preserve_raw_source_repair_probe_failure(
            RadrootsEventStoreError::RawSourceRepairCanonicalPathLockDomainMismatch {
                canonical_path: canonical_path.display().to_string(),
            },
            rollback,
        ),
        Err(error) => {
            if sqlite_error_is_busy_or_locked(&error) {
                rollback?;
                Ok(())
            } else {
                preserve_raw_source_repair_probe_failure(error.into(), rollback)
            }
        }
    }
}

fn preserve_raw_source_repair_probe_failure<T>(
    primary: RadrootsEventStoreError,
    rollback: Result<(), sqlx::Error>,
) -> Result<T, RadrootsEventStoreError> {
    match rollback {
        Ok(()) => Err(primary),
        Err(rollback) => Err(
            RadrootsEventStoreError::RawSourceRebuildTransactionRollbackFailed {
                primary: Box::new(primary),
                rollback,
            },
        ),
    }
}

fn canonical_raw_source_repair_main_path_v1(
    path: &Path,
) -> Result<PathBuf, RadrootsEventStoreError> {
    let filename = path.display().to_string();
    std::fs::canonicalize(path).map_err(|source| {
        RadrootsEventStoreError::RawSourceRepairMainDatabaseCanonicalizationFailed {
            filename,
            source,
        }
    })
}

async fn validate_main_database_encoding(
    connection: &mut SqliteConnection,
) -> Result<(), RadrootsEventStoreError> {
    let actual: String = sqlx::query_scalar("PRAGMA main.encoding")
        .fetch_one(&mut *connection)
        .await?;
    if actual == "UTF-8" {
        return Ok(());
    }
    Err(RadrootsEventStoreError::SqliteMainDatabaseEncodingNotUtf8 { actual })
}

async fn configure_file_journal_mode(
    connection: &mut SqliteConnection,
) -> Result<(), RadrootsEventStoreError> {
    let mut busy_retries = 0;
    loop {
        match sqlx::query_scalar::<_, String>("PRAGMA main.journal_mode = WAL")
            .fetch_one(&mut *connection)
            .await
        {
            Ok(actual) if actual == "wal" => return Ok(()),
            Ok(actual) => {
                return Err(RadrootsEventStoreError::SqliteFileJournalModeNotWal { actual });
            }
            Err(error)
                if sqlite_error_is_busy(&error)
                    && busy_retries < FILE_JOURNAL_MODE_BUSY_RETRY_LIMIT =>
            {
                busy_retries += 1;
                let transaction = connection.begin_with("BEGIN EXCLUSIVE").await?;
                transaction.rollback().await?;
            }
            Err(error) => return Err(error.into()),
        }
    }
}

async fn validate_file_journal_mode_is_wal(
    connection: &mut SqliteConnection,
) -> Result<(), RadrootsEventStoreError> {
    let actual: String = sqlx::query_scalar("PRAGMA main.journal_mode")
        .fetch_one(&mut *connection)
        .await?;
    if actual == "wal" {
        return Ok(());
    }
    Err(RadrootsEventStoreError::SqliteFileJournalModeNotWal { actual })
}

fn sqlite_error_is_busy(error: &sqlx::Error) -> bool {
    let sqlx::Error::Database(error) = error else {
        return false;
    };
    error
        .code()
        .and_then(|code| code.parse::<i32>().ok())
        .is_some_and(|code| code & 0xff == 5)
}

fn sqlite_error_is_busy_or_locked(error: &sqlx::Error) -> bool {
    let sqlx::Error::Database(error) = error else {
        return false;
    };
    error
        .code()
        .and_then(|code| code.parse::<i32>().ok())
        .is_some_and(|code| code & 0xff == 5 || code & 0xff == 6)
}

async fn main_database_filename(
    connection: &mut SqliteConnection,
) -> Result<String, RadrootsEventStoreError> {
    let rows = sqlx::query("PRAGMA database_list")
        .fetch_all(&mut *connection)
        .await?;
    for row in rows {
        if row.try_get::<String, _>("name")? == "main" {
            return Ok(row.try_get("file")?);
        }
    }
    Err(RadrootsEventStoreError::SqliteMainDatabaseUnavailable)
}

fn preserve_ingest_primary_failure<T>(
    primary: RadrootsEventStoreError,
    rollback: Result<(), sqlx::Error>,
) -> Result<T, RadrootsEventStoreError> {
    match rollback {
        Ok(()) => Err(primary),
        Err(rollback) => Err(RadrootsEventStoreError::IngestTransactionRollbackFailed {
            primary: Box::new(primary),
            rollback,
        }),
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn query_i64(pool: &SqlitePool, sql: &'static str) -> Result<i64, RadrootsEventStoreError> {
    let row = sqlx::query(sql).fetch_one(pool).await?;
    Ok(row.try_get(0)?)
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn query_string(
    pool: &SqlitePool,
    sql: &'static str,
) -> Result<String, RadrootsEventStoreError> {
    let row = sqlx::query(sql).fetch_one(pool).await?;
    Ok(row.try_get(0)?)
}

fn parse_trade_mutation_kind(
    value: &str,
) -> Result<RadrootsTradeMutationKindV1, RadrootsEventStoreError> {
    match value {
        "proposal" => Ok(RadrootsTradeMutationKindV1::Proposal),
        "decision" => Ok(RadrootsTradeMutationKindV1::Decision),
        "revision_proposal" => Ok(RadrootsTradeMutationKindV1::RevisionProposal),
        "revision_decision" => Ok(RadrootsTradeMutationKindV1::RevisionDecision),
        "cancellation" => Ok(RadrootsTradeMutationKindV1::Cancellation),
        _ => Err(RadrootsEventStoreError::InvalidStoredEnum {
            field: "trade_mutation.mutation_kind",
            value: value.to_owned(),
        }),
    }
}

async fn ingest_event_in_transaction(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    ingest: RadrootsEventIngest,
) -> Result<RadrootsEventIngestReceipt, RadrootsEventStoreError> {
    crate::schema::validate_event_store_temp_schema(tx).await?;
    let result = ingest_event_protocol_reconciliation_v1(tx, &ingest).await?;
    {
        let mut capabilities = PostCoreExtensionCapabilities::new(tx);
        dispatch_post_core_extensions(&mut capabilities, &ingest, &result).await?;
    }
    validate_protocol_post_extensions(tx, &result).await?;
    Ok(result.receipt)
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn stored_tag_from_row(
    row: sqlx::sqlite::SqliteRow,
) -> Result<RadrootsStoredEventTag, RadrootsEventStoreError> {
    Ok(RadrootsStoredEventTag {
        event_id: row.try_get("event_id")?,
        tag_index: u32_from_i64("tag_index", row.try_get("tag_index")?)?,
        tag_name: row.try_get("tag_name")?,
        tag_value: row.try_get("tag_value")?,
        tag_json: row.try_get("tag_json")?,
        contract_semantic: row.try_get("contract_semantic")?,
        contract_value_type: row.try_get("contract_value_type")?,
        relay_indexed: bool_from_i64("relay_indexed", row.try_get("relay_indexed")?)?,
    })
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn projection_cursor_from_row(
    row: sqlx::sqlite::SqliteRow,
    active_generation: RadrootsEventStoreSourceGeneration,
) -> Result<RadrootsProjectionCursor, RadrootsEventStoreError> {
    let projection_id: String = row.try_get("projection_id")?;
    validate_projection_id(projection_id.as_str())?;
    let last_event_seq: i64 = row.try_get("last_event_seq")?;
    validate_projection_sequence(projection_id.as_str(), last_event_seq)?;
    let projection_version =
        projection_version_from_i64(projection_id.as_str(), row.try_get("projection_version")?)?;
    projection_source_revision_from_i64(projection_id.as_str(), row.try_get("source_revision")?)?;
    let source_generation = row
        .try_get::<Option<Vec<u8>>, _>("source_generation")?
        .ok_or_else(
            || RadrootsEventStoreError::ProjectionCursorRebuildRequired {
                projection_id: projection_id.clone(),
            },
        )
        .and_then(generation_from_blob)?;
    if source_generation != active_generation {
        return Err(RadrootsEventStoreError::ProjectionSourceGenerationMismatch { projection_id });
    }
    RadrootsProjectionCursor::new(
        projection_id,
        projection_version,
        source_generation,
        last_event_seq,
        row.try_get("updated_at_ms")?,
    )
}

async fn projection_cursor_unchecked(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    projection_id: &str,
    active_generation: RadrootsEventStoreSourceGeneration,
) -> Result<Option<RadrootsProjectionCursor>, RadrootsEventStoreError> {
    let row = sqlx::query(
        "SELECT cursor.projection_id, cursor.projection_version, cursor.last_event_seq, cursor.updated_at_ms, source.source_generation, source.source_revision FROM projection_cursor AS cursor LEFT JOIN radroots_event_store_projection_cursor_source AS source ON source.projection_id = cursor.projection_id WHERE cursor.projection_id = ?",
    )
    .bind(projection_id)
    .fetch_optional(&mut **tx)
    .await?;
    row.map(|row| projection_cursor_from_row(row, active_generation))
        .transpose()
}

fn validate_projection_identity(
    projection_id: &str,
    projection_version: u32,
) -> Result<(), RadrootsEventStoreError> {
    validate_projection_id(projection_id)?;
    if projection_version == 0 {
        return Err(RadrootsEventStoreError::InvalidProjectionVersion {
            projection_id: projection_id.to_owned(),
            value: 0,
        });
    }
    Ok(())
}

fn validate_projection_id(projection_id: &str) -> Result<(), RadrootsEventStoreError> {
    if projection_id.is_empty() {
        Err(RadrootsEventStoreError::InvalidProjectionId)
    } else {
        Ok(())
    }
}

fn validate_projection_sequence(
    projection_id: &str,
    value: i64,
) -> Result<(), RadrootsEventStoreError> {
    if value < 0 {
        Err(RadrootsEventStoreError::InvalidProjectionCursor {
            projection_id: projection_id.to_owned(),
            value,
        })
    } else {
        Ok(())
    }
}

fn projection_version_from_i64(
    projection_id: &str,
    value: i64,
) -> Result<u32, RadrootsEventStoreError> {
    let version =
        u32::try_from(value).map_err(|_| RadrootsEventStoreError::InvalidProjectionVersion {
            projection_id: projection_id.to_owned(),
            value,
        })?;
    if version == 0 {
        return Err(RadrootsEventStoreError::InvalidProjectionVersion {
            projection_id: projection_id.to_owned(),
            value,
        });
    }
    Ok(version)
}

fn projection_source_revision_from_i64(
    projection_id: &str,
    value: Option<i64>,
) -> Result<u64, RadrootsEventStoreError> {
    let Some(value) = value else {
        return Err(RadrootsEventStoreError::InvalidProjectionSourceRevision {
            projection_id: projection_id.to_owned(),
            value: None,
        });
    };
    if value <= 0 || value == i64::MAX {
        return Err(RadrootsEventStoreError::InvalidProjectionSourceRevision {
            projection_id: projection_id.to_owned(),
            value: Some(value),
        });
    }
    Ok(value as u64)
}

fn projection_source_revision_to_i64(
    projection_id: &str,
    value: u64,
) -> Result<i64, RadrootsEventStoreError> {
    i64::try_from(value).map_err(
        |_| RadrootsEventStoreError::InvalidProjectionSourceRevision {
            projection_id: projection_id.to_owned(),
            value: None,
        },
    )
}

fn ensure_projection_rebuild_row_changed(
    projection_id: &str,
    rows_affected: u64,
) -> Result<(), RadrootsEventStoreError> {
    if rows_affected == 1 {
        Ok(())
    } else {
        Err(RadrootsEventStoreError::ProjectionRebuildTicketConflict {
            projection_id: projection_id.to_owned(),
        })
    }
}

async fn validate_projection_cursor_high_water(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    projection_id: &str,
    proposed: i64,
) -> Result<(), RadrootsEventStoreError> {
    let high_water: i64 = sqlx::query_scalar(
        "SELECT raw_high_water_seq FROM radroots_event_store_source_state WHERE singleton = 1",
    )
    .fetch_one(&mut **tx)
    .await?;
    if proposed > high_water {
        return Err(RadrootsEventStoreError::ProjectionCursorAheadOfSource {
            projection_id: projection_id.to_owned(),
            proposed,
            high_water,
        });
    }
    Ok(())
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn trade_mutation_from_row(
    row: sqlx::sqlite::SqliteRow,
) -> Result<RadrootsStoredTradeMutation, RadrootsEventStoreError> {
    Ok(RadrootsStoredTradeMutation {
        mutation_id: parse_id(row.try_get::<String, _>("mutation_id")?)?,
        trade_id: parse_id(row.try_get::<String, _>("trade_id")?)?,
        root_mutation_id: parse_optional_id(row.try_get("root_mutation_id")?)?,
        contract_id: row.try_get("contract_id")?,
        mutation_kind: parse_trade_mutation_kind(
            row.try_get::<String, _>("mutation_kind")?.as_str(),
        )?,
        schema_version: u16_from_i64("schema_version", row.try_get("schema_version")?)?,
        candidate_id: parse_optional_id(row.try_get("candidate_id")?)?,
        proposal_mutation_id: parse_optional_id(row.try_get("proposal_mutation_id")?)?,
        target_claim_mutation_id: parse_optional_id(row.try_get("target_claim_mutation_id")?)?,
        author_pubkey: parse_id(row.try_get::<String, _>("author_pubkey")?)?,
        counterparty_pubkey: parse_id(row.try_get::<String, _>("counterparty_pubkey")?)?,
        buyer_pubkey: parse_id(row.try_get::<String, _>("buyer_pubkey")?)?,
        seller_pubkey: parse_id(row.try_get::<String, _>("seller_pubkey")?)?,
        farm_id: parse_id(row.try_get::<String, _>("farm_id")?)?,
        authored_at_unix_s: u64_from_i64("authored_at_unix_s", row.try_get("authored_at_unix_s")?)?,
        canonical_payload_bytes: row.try_get("canonical_payload_bytes")?,
        payload_sha256: row.try_get("payload_sha256")?,
        first_event_seq: row.try_get("first_event_seq")?,
        first_transport_event_id: parse_id(row.try_get::<String, _>("first_transport_event_id")?)?,
        inserted_at_ms: row.try_get("inserted_at_ms")?,
    })
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn trade_mutation_parent_from_row(
    row: sqlx::sqlite::SqliteRow,
) -> Result<RadrootsStoredTradeMutationParent, RadrootsEventStoreError> {
    Ok(RadrootsStoredTradeMutationParent {
        mutation_id: parse_id(row.try_get::<String, _>("mutation_id")?)?,
        parent_mutation_id: parse_id(row.try_get::<String, _>("parent_mutation_id")?)?,
        parent_index: u32_from_i64("parent_index", row.try_get("parent_index")?)?,
    })
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn trade_missing_parent_from_row(
    row: sqlx::sqlite::SqliteRow,
) -> Result<RadrootsStoredTradeMissingParent, RadrootsEventStoreError> {
    Ok(RadrootsStoredTradeMissingParent {
        trade_id: parse_id(row.try_get::<String, _>("trade_id")?)?,
        mutation_id: parse_id(row.try_get::<String, _>("mutation_id")?)?,
        missing_parent_mutation_id: parse_id(
            row.try_get::<String, _>("missing_parent_mutation_id")?,
        )?,
        first_transport_event_id: parse_id(row.try_get::<String, _>("first_transport_event_id")?)?,
        first_seen_at_ms: row.try_get("first_seen_at_ms")?,
    })
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn trade_transport_envelope_from_row(
    row: sqlx::sqlite::SqliteRow,
) -> Result<RadrootsStoredTradeTransportEnvelope, RadrootsEventStoreError> {
    Ok(RadrootsStoredTradeTransportEnvelope {
        transport_event_id: parse_id(row.try_get::<String, _>("transport_event_id")?)?,
        mutation_id: parse_id(row.try_get::<String, _>("mutation_id")?)?,
        trade_id: parse_id(row.try_get::<String, _>("trade_id")?)?,
        transport_kind: row.try_get("transport_kind")?,
        pubkey: parse_id(row.try_get::<String, _>("pubkey")?)?,
        created_at: u64_from_i64("created_at", row.try_get("created_at")?)?,
        event_seq: row.try_get("event_seq")?,
        payload_sha256: row.try_get("payload_sha256")?,
        observed_at_ms: row.try_get("observed_at_ms")?,
    })
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn seller_reservation_from_row(
    row: sqlx::sqlite::SqliteRow,
) -> Result<RadrootsStoredSellerReservation, RadrootsEventStoreError> {
    Ok(RadrootsStoredSellerReservation {
        reservation_id: parse_id(row.try_get::<String, _>("reservation_id")?)?,
        trade_id: parse_id(row.try_get::<String, _>("trade_id")?)?,
        candidate_id: parse_id(row.try_get::<String, _>("candidate_id")?)?,
        claim_mutation_id: parse_id(row.try_get::<String, _>("claim_mutation_id")?)?,
        inventory_authority_pubkey: parse_id(
            row.try_get::<String, _>("inventory_authority_pubkey")?,
        )?,
        inventory_epoch: u64_from_i64("inventory_epoch", row.try_get("inventory_epoch")?)?,
        assertion_commitment: row.try_get("assertion_commitment")?,
        reservation_expires_at_unix_s: u64_from_i64(
            "reservation_expires_at_unix_s",
            row.try_get("reservation_expires_at_unix_s")?,
        )?,
        reservation_json: row.try_get("reservation_json")?,
        inserted_at_ms: row.try_get("inserted_at_ms")?,
    })
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn seller_reservation_line_from_row(
    row: sqlx::sqlite::SqliteRow,
) -> Result<RadrootsStoredSellerReservationLine, RadrootsEventStoreError> {
    Ok(RadrootsStoredSellerReservationLine {
        reservation_id: parse_id(row.try_get::<String, _>("reservation_id")?)?,
        line_id: parse_id(row.try_get::<String, _>("line_id")?)?,
        bin_id: parse_id(row.try_get::<String, _>("bin_id")?)?,
        quantity_mantissa: row.try_get("quantity_mantissa")?,
        quantity_scale: u8_from_i64("quantity_scale", row.try_get("quantity_scale")?)?,
        unit_code: row.try_get("unit_code")?,
        line_index: u32_from_i64("line_index", row.try_get("line_index")?)?,
    })
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn trade_projection_checkpoint_from_row(
    row: sqlx::sqlite::SqliteRow,
) -> Result<RadrootsTradeProjectionCheckpoint, RadrootsEventStoreError> {
    Ok(RadrootsTradeProjectionCheckpoint {
        trade_id: parse_id(row.try_get::<String, _>("trade_id")?)?,
        reducer_contract_id: row.try_get("reducer_contract_id")?,
        reducer_version: u16_from_i64("reducer_version", row.try_get("reducer_version")?)?,
        projection_digest: row.try_get("projection_digest")?,
        root_mutation_id: parse_optional_id(row.try_get("root_mutation_id")?)?,
        negotiation_state: row.try_get("negotiation_state")?,
        agreement_state: row.try_get("agreement_state")?,
        evidence_state: row.try_get("evidence_state")?,
        conflict_state: row.try_get("conflict_state")?,
        private_terms_state: row.try_get("private_terms_state")?,
        attestation_state: row.try_get("attestation_state")?,
        fulfillment_state: row.try_get("fulfillment_state")?,
        payment_state: row.try_get("payment_state")?,
        projection_json: row.try_get("projection_json")?,
        last_mutation_id: parse_optional_id(row.try_get("last_mutation_id")?)?,
        last_transport_event_seq: row.try_get("last_transport_event_seq")?,
        updated_at_ms: row.try_get("updated_at_ms")?,
    })
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn transport_observation_from_row(
    row: sqlx::sqlite::SqliteRow,
) -> Result<RadrootsTransportObservationRow, RadrootsEventStoreError> {
    let event_id: String = row.try_get("event_id")?;
    let transport_kind_label: String = row.try_get("transport_kind")?;
    let endpoint_uri_raw: String = row.try_get("endpoint_uri")?;
    let endpoint_fingerprint_raw: String = row.try_get("endpoint_fingerprint")?;
    let transport_kind = RadrootsTransportKind::parse_canonical(&transport_kind_label)?;
    let endpoint_fingerprint =
        RadrootsTransportTargetFingerprint::parse(&endpoint_fingerprint_raw)?;
    let target = RadrootsTransportTarget::new(transport_kind, &endpoint_uri_raw)?;
    if target.uri().as_str() != endpoint_uri_raw
        || endpoint_fingerprint.as_str() != endpoint_fingerprint_raw
        || &endpoint_fingerprint != target.fingerprint()
    {
        return Err(
            RadrootsEventStoreError::InvalidStoredTransportEndpointFingerprint {
                event_id,
                transport_kind: transport_kind_label,
                endpoint_uri: endpoint_uri_raw,
                endpoint_fingerprint: endpoint_fingerprint_raw,
            },
        );
    }
    let first_observed_at_ms = row.try_get("first_observed_at_ms")?;
    let last_observed_at_ms = row.try_get("last_observed_at_ms")?;
    let observation_count = row.try_get("observation_count")?;
    if observation_count <= 0
        || first_observed_at_ms < 0
        || last_observed_at_ms < 0
        || first_observed_at_ms > last_observed_at_ms
    {
        return Err(RadrootsEventStoreError::InvalidStoredTransportObservation {
            event_id,
            first_observed_at_ms,
            last_observed_at_ms,
            observation_count,
        });
    }
    let caller_redacted_message = row
        .try_get::<Option<String>, _>("redacted_message")?
        .map(|message| {
            crate::model::RadrootsTransportObservationMessage::parse_stored(
                event_id.as_str(),
                message,
            )
        })
        .transpose()?;
    Ok(RadrootsTransportObservationRow {
        event_id,
        transport_kind: target.kind().clone(),
        endpoint_uri: target.uri().clone(),
        endpoint_fingerprint: target.fingerprint().clone(),
        observation_type: RadrootsTransportObservationType::parse(
            row.try_get("observation_type")?,
        )?,
        first_observed_at_ms,
        last_observed_at_ms,
        observation_count,
        caller_redacted_message,
    })
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn u32_from_i64(field: &'static str, value: i64) -> Result<u32, RadrootsEventStoreError> {
    u32::try_from(value).map_err(|_| RadrootsEventStoreError::IntegerRange { field, value })
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn u16_from_i64(field: &'static str, value: i64) -> Result<u16, RadrootsEventStoreError> {
    u16::try_from(value).map_err(|_| RadrootsEventStoreError::IntegerRange { field, value })
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn u8_from_i64(field: &'static str, value: i64) -> Result<u8, RadrootsEventStoreError> {
    u8::try_from(value).map_err(|_| RadrootsEventStoreError::IntegerRange { field, value })
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn u64_from_i64(field: &'static str, value: i64) -> Result<u64, RadrootsEventStoreError> {
    u64::try_from(value).map_err(|_| RadrootsEventStoreError::IntegerRange { field, value })
}

fn bool_from_i64(field: &'static str, value: i64) -> Result<bool, RadrootsEventStoreError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(RadrootsEventStoreError::InvalidStoredBoolean { field, value }),
    }
}

fn parse_id<T>(value: String) -> Result<T, RadrootsEventStoreError>
where
    T: TryFrom<String, Error = radroots_event::ids::RadrootsIdParseError>,
{
    T::try_from(value).map_err(Into::into)
}

fn parse_optional_id<T>(value: Option<String>) -> Result<Option<T>, RadrootsEventStoreError>
where
    T: TryFrom<String, Error = radroots_event::ids::RadrootsIdParseError>,
{
    value.map(parse_id).transpose()
}

fn validate_event_query_limit(limit: u32) -> Result<(), RadrootsEventStoreError> {
    if !(1..=RADROOTS_EVENT_STORE_QUERY_LIMIT_MAX).contains(&limit) {
        return Err(RadrootsEventStoreError::QueryLimitOutOfRange {
            min: 1,
            max: RADROOTS_EVENT_STORE_QUERY_LIMIT_MAX,
            actual: limit,
        });
    }
    Ok(())
}

fn validate_tag_query(tag_name: &str, limit: u32) -> Result<(), RadrootsEventStoreError> {
    if tag_name.is_empty() {
        return Err(RadrootsEventStoreError::EmptyTagName);
    }
    validate_event_query_limit(limit)
}

fn validate_contract_tag_query<S>(
    contract_ids: &[S],
    tag_name: &str,
    limit: u32,
) -> Result<(), RadrootsEventStoreError>
where
    S: AsRef<str>,
{
    if contract_ids.is_empty() {
        return Err(RadrootsEventStoreError::EmptyContractList);
    }
    if contract_ids.len() > RADROOTS_EVENT_STORE_CONTRACT_QUERY_LIMIT_MAX {
        return Err(RadrootsEventStoreError::ContractListTooLarge {
            max: RADROOTS_EVENT_STORE_CONTRACT_QUERY_LIMIT_MAX,
            actual: contract_ids.len(),
        });
    }
    validate_tag_query(tag_name, limit)
}

fn validate_trade_query_limit(limit: u32) -> Result<(), RadrootsEventStoreError> {
    validate_event_query_limit(limit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nostr::{
        EventBuilder, Keys as RadrootsNostrKeys, Kind as RadrootsNostrKind,
        SecretKey as RadrootsNostrSecretKey, Tag as RadrootsNostrTag,
        TagKind as RadrootsNostrTagKind, Timestamp as RadrootsNostrTimestamp,
    };
    use radroots_event::draft::RadrootsSignedEvent;
    use radroots_event::food_availability::{
        RadrootsFoodAvailabilityStatus, RadrootsFoodIdentifier,
    };
    use radroots_event::ids::{
        RadrootsClassifiedListingAddress, RadrootsInventoryBinId, RadrootsPublicKey,
    };
    use radroots_event::kinds::{
        KIND_CALENDAR_DATE_EVENT, KIND_CLASSIFIED_LISTING, KIND_DELETION_REQUEST, KIND_FARM,
        KIND_GEOCHAT, KIND_LIST_SET_RELAY, KIND_POST, KIND_PROFILE, KIND_RELAY_AUTH,
    };
    use radroots_event::trade::{
        RADROOTS_TRADE_DECISION_CONTRACT_ID, RADROOTS_TRADE_PROPOSAL_CONTRACT_ID,
        RADROOTS_TRADE_SCHEMA_VERSION, RadrootsFulfillmentProfileV1,
        RadrootsSellerReservationAssertionV1, RadrootsSellerReservationLineV1,
        RadrootsTradeCancellationProfileV1, RadrootsTradeCandidateLineV1,
        RadrootsTradeCandidateTermsV1, RadrootsTradeCanonicalMutationV1, RadrootsTradeDecisionV1,
        RadrootsTradeEconomicAdjustmentV1, RadrootsTradeEconomicsProfileV1,
        RadrootsTradeMutationBodyV1, RadrootsTradeMutationEnvelopeV1, canonical_jcs_value,
        canonical_trade_mutation_content,
    };
    use radroots_event::wire::{
        DEFAULT_CONTENT_MAX_BYTES, DEFAULT_RAW_JSON_MAX_BYTES, RadrootsNip01EventWire,
        compute_canonical_nip01_event_id,
    };
    use radroots_event_codec::food_availability::inbound::RadrootsFoodAvailabilityImageDiagnostic;
    use std::sync::Arc;

    const FIXTURE_ALICE_SECRET_KEY_HEX: &str =
        "10c5304d6c9ae3a1a16f7860f1cc8f5e3a76225a2663b3a989a0d775919b7df5";
    const FIXTURE_ALICE_PUBLIC_KEY_HEX: &str =
        "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df";

    struct FixedGeneration([u8; 32]);

    impl crate::nip09::reconciliation_v1::SourceGenerationProvider for FixedGeneration {
        fn fill_generation(
            &self,
            generation: &mut [u8; 32],
        ) -> Result<(), RadrootsEventStoreError> {
            generation.copy_from_slice(&self.0);
            Ok(())
        }
    }

    struct FailingGeneration;

    impl crate::nip09::reconciliation_v1::SourceGenerationProvider for FailingGeneration {
        fn fill_generation(
            &self,
            _generation: &mut [u8; 32],
        ) -> Result<(), RadrootsEventStoreError> {
            Err(RadrootsEventStoreError::SourceGenerationEntropyUnavailable)
        }
    }

    struct PanickingGeneration;

    impl crate::nip09::reconciliation_v1::SourceGenerationProvider for PanickingGeneration {
        fn fill_generation(
            &self,
            _generation: &mut [u8; 32],
        ) -> Result<(), RadrootsEventStoreError> {
            panic!("generation entropy was requested after the retained-history preflight")
        }
    }

    fn fixture_keys() -> RadrootsNostrKeys {
        let secret_key =
            RadrootsNostrSecretKey::from_hex(FIXTURE_ALICE_SECRET_KEY_HEX).expect("secret key");
        RadrootsNostrKeys::new(secret_key)
    }

    fn alternate_keys() -> RadrootsNostrKeys {
        let secret_key = RadrootsNostrSecretKey::from_hex(
            "0000000000000000000000000000000000000000000000000000000000000001",
        )
        .expect("alternate secret key");
        RadrootsNostrKeys::new(secret_key)
    }

    fn test_event_builder(
        kind: u32,
        content: impl Into<String>,
        tags: Vec<Vec<String>>,
    ) -> EventBuilder {
        let tags: Vec<_> = tags
            .into_iter()
            .filter(|tag| !tag.is_empty())
            .map(|mut tag| {
                let key = tag.remove(0);
                RadrootsNostrTag::custom(RadrootsNostrTagKind::Custom(key.into()), tag)
            })
            .collect();
        EventBuilder::new(
            RadrootsNostrKind::Custom(u16::try_from(kind).expect("test kind must fit NIP-01")),
            content.into(),
        )
        .tags(tags)
        .allow_self_tagging()
    }

    fn event_id(character: char) -> String {
        core::iter::repeat_n(character, 64).collect()
    }

    fn trade_id() -> RadrootsTradeId {
        RadrootsTradeId::parse("1".repeat(32)).expect("trade id")
    }

    fn public_key(character: char) -> RadrootsPublicKey {
        if character == 'a' {
            return RadrootsPublicKey::parse(FIXTURE_ALICE_PUBLIC_KEY_HEX).expect("alice pubkey");
        }
        RadrootsPublicKey::parse(event_id(character)).expect("pubkey")
    }

    fn candidate_terms() -> RadrootsTradeCandidateTermsV1 {
        RadrootsTradeCandidateTermsV1 {
            candidate_id: None,
            schema_version: RADROOTS_TRADE_SCHEMA_VERSION,
            base_candidate_id: None,
            supersession_intent: None,
            buyer_pubkey: public_key('a'),
            seller_pubkey: public_key('a'),
            farm_id: RadrootsDTag::parse("farm-1").expect("farm id"),
            lines: vec![RadrootsTradeCandidateLineV1 {
                line_id: RadrootsDTag::parse("line-1").expect("line id"),
                listing_addr: RadrootsClassifiedListingAddress::parse(format!(
                    "{KIND_CLASSIFIED_LISTING}:{}:listing-1",
                    FIXTURE_ALICE_PUBLIC_KEY_HEX
                ))
                .expect("listing address"),
                listing_event_id: RadrootsEventId::parse(event_id('c')).expect("listing event id"),
                listing_snapshot_sha256: event_id('d'),
                product_id: "carrots".to_owned(),
                option_id: None,
                bin_id: RadrootsInventoryBinId::parse("bin-1").expect("bin id"),
                quantity_mantissa: "2".to_owned(),
                quantity_scale: 0,
                unit_code: "count".to_owned(),
                unit_profile: "mvp-count".to_owned(),
                unit_price_mantissa: "300".to_owned(),
                currency_code: "USD".to_owned(),
                line_subtotal_mantissa: "600".to_owned(),
                replaces_line_id: None,
            }],
            line_tombstones: Vec::new(),
            economics: RadrootsTradeEconomicsProfileV1 {
                profile_id: "mvp-no-payment".to_owned(),
                currency_code: "USD".to_owned(),
                currency_exponent: 2,
                rounding_profile: "half-up".to_owned(),
                subtotal_mantissa: "600".to_owned(),
                discount_total_mantissa: "0".to_owned(),
                adjustment_total_mantissa: "0".to_owned(),
                total_mantissa: "600".to_owned(),
                adjustments: Vec::<RadrootsTradeEconomicAdjustmentV1>::new(),
            },
            fulfillment: RadrootsFulfillmentProfileV1 {
                profile_id: "market-pickup".to_owned(),
                method: "pickup".to_owned(),
                starts_at_unix_s: 1_800_000_000,
                ends_at_unix_s: 1_800_003_600,
                timezone: "America/New_York".to_owned(),
                utc_offset_seconds: -18_000,
                fold: 0,
                location_class: "private_after_agreement".to_owned(),
                requires_private_terms: false,
            },
            cancellation: RadrootsTradeCancellationProfileV1 {
                profile_id: "mvp".to_owned(),
                buyer_pre_agreement: true,
                post_agreement_cutoff_unix_s: Some(1_799_999_000),
            },
            private_terms: None,
            proposal_expires_at_unix_s: 1_799_999_000,
        }
    }

    fn proposal_envelope() -> RadrootsTradeMutationEnvelopeV1 {
        RadrootsTradeMutationEnvelopeV1 {
            mutation_id: None,
            contract_id: RADROOTS_TRADE_PROPOSAL_CONTRACT_ID.to_owned(),
            schema_version: RADROOTS_TRADE_SCHEMA_VERSION,
            trade_id: trade_id(),
            root_mutation_id: None,
            buyer_pubkey: public_key('a'),
            seller_pubkey: public_key('a'),
            farm_id: RadrootsDTag::parse("farm-1").expect("farm id"),
            parent_mutation_ids: Vec::new(),
            author_pubkey: public_key('a'),
            counterparty_pubkey: public_key('a'),
            authored_at_unix_s: 1_799_000_000,
            body: RadrootsTradeMutationBodyV1::Proposal {
                candidate: candidate_terms(),
            },
        }
    }

    fn decision_envelope(
        proposal: &RadrootsTradeCanonicalMutationV1,
    ) -> RadrootsTradeMutationEnvelopeV1 {
        let RadrootsTradeMutationBodyV1::Proposal { candidate } = &proposal.envelope.body else {
            panic!("proposal");
        };
        let candidate_id = candidate.candidate_id.clone().expect("candidate id");
        let line = candidate.lines.first().expect("candidate line");
        RadrootsTradeMutationEnvelopeV1 {
            mutation_id: None,
            contract_id: RADROOTS_TRADE_DECISION_CONTRACT_ID.to_owned(),
            schema_version: RADROOTS_TRADE_SCHEMA_VERSION,
            trade_id: proposal.envelope.trade_id.clone(),
            root_mutation_id: Some(proposal.mutation_id.clone()),
            buyer_pubkey: public_key('a'),
            seller_pubkey: public_key('a'),
            farm_id: RadrootsDTag::parse("farm-1").expect("farm id"),
            parent_mutation_ids: vec![proposal.mutation_id.clone()],
            author_pubkey: public_key('a'),
            counterparty_pubkey: public_key('a'),
            authored_at_unix_s: 1_799_000_060,
            body: RadrootsTradeMutationBodyV1::Decision {
                proposal_mutation_id: proposal.mutation_id.clone(),
                candidate_id: candidate_id.clone(),
                decision: RadrootsTradeDecisionV1::Accepted {
                    reservation_assertion: Some(RadrootsSellerReservationAssertionV1 {
                        reservation_id: RadrootsDTag::parse("reservation-1")
                            .expect("reservation id"),
                        inventory_authority_id: public_key('a'),
                        inventory_epoch: 7,
                        candidate_id,
                        commitments: vec![RadrootsSellerReservationLineV1 {
                            line_id: line.line_id.clone(),
                            bin_id: line.bin_id.clone(),
                            quantity_mantissa: line.quantity_mantissa.clone(),
                            quantity_scale: line.quantity_scale,
                            unit_code: line.unit_code.clone(),
                        }],
                        reservation_expires_at_unix_s: 1_799_000_600,
                        assertion_commitment: event_id('e'),
                    }),
                },
            },
        }
    }

    fn signed_trade_mutation(canonical: &RadrootsTradeCanonicalMutationV1) -> RadrootsSignedEvent {
        signed_trade_content_with_keys(canonical, canonical.content.clone(), &fixture_keys())
    }

    fn signed_trade_content_with_keys(
        canonical: &RadrootsTradeCanonicalMutationV1,
        content: String,
        keys: &RadrootsNostrKeys,
    ) -> RadrootsSignedEvent {
        let counterparty = canonical.envelope.counterparty_pubkey.as_str().to_owned();
        let mut tags = vec![
            vec![
                "contract".to_owned(),
                canonical.envelope.contract_id.clone(),
            ],
            vec!["d".to_owned(), canonical.mutation_id.to_string()],
            vec!["p".to_owned(), counterparty],
        ];
        for parent in &canonical.envelope.parent_mutation_ids {
            tags.push(vec!["e".to_owned(), parent.to_string()]);
        }
        let raw_event = test_event_builder(
            canonical.envelope.mutation_kind().nostr_kind(),
            content,
            tags,
        )
        .custom_created_at(RadrootsNostrTimestamp::from_secs(
            canonical.envelope.authored_at_unix_s,
        ))
        .sign_with_keys(keys)
        .expect("signed trade event");
        signed_event_from_raw_json(serde_json::to_string(&raw_event).expect("trade raw json"))
    }

    fn signed_event(
        kind: u32,
        created_at: u32,
        tags: Vec<Vec<String>>,
        content: &str,
    ) -> RadrootsSignedEvent {
        signed_event_with_keys(&fixture_keys(), kind, created_at, tags, content)
    }

    fn signed_event_with_keys(
        keys: &RadrootsNostrKeys,
        kind: u32,
        created_at: u32,
        tags: Vec<Vec<String>>,
        content: &str,
    ) -> RadrootsSignedEvent {
        let raw_event = test_event_builder(kind, content, tags)
            .custom_created_at(RadrootsNostrTimestamp::from_secs(u64::from(created_at)))
            .sign_with_keys(keys)
            .expect("signed event");
        signed_event_from_raw_json(serde_json::to_string(&raw_event).expect("raw json"))
    }

    fn signed_event_from_raw_json(raw_json: String) -> RadrootsSignedEvent {
        let wire = RadrootsNip01EventWire::parse_json(raw_json.as_str()).expect("wire");
        RadrootsSignedEvent::from_wire_verified_id(wire, raw_json).expect("signed event")
    }

    fn raw_source_text_bytes(event: &RadrootsSignedEvent) -> (u64, u64) {
        let tags = event.tags_as_vec();
        let tags_json = serde_json::to_string(&tags).expect("tags JSON");
        let event_bytes = [
            event.id_str(),
            event.pubkey_str(),
            tags_json.as_str(),
            event.content(),
            event.sig_str(),
            event.raw_json(),
        ]
        .into_iter()
        .map(str::len)
        .sum::<usize>();
        let tag_bytes = tags
            .iter()
            .map(|tag| {
                let tag_json = serde_json::to_string(tag).expect("tag JSON");
                event.id_str().len()
                    + tag.first().map_or(0, String::len)
                    + tag.get(1).map_or(0, String::len)
                    + tag_json.len()
            })
            .sum::<usize>();
        (
            u64::try_from(event_bytes).expect("event byte count fits u64"),
            u64::try_from(tag_bytes).expect("tag byte count fits u64"),
        )
    }

    async fn initialize_utf16le_database(path: &Path) {
        let mut connection = SqliteConnection::connect_with(
            &SqliteConnectOptions::new()
                .filename(path)
                .create_if_missing(true),
        )
        .await
        .expect("UTF-16 fixture connection");
        sqlx::query("PRAGMA main.encoding = 'UTF-16le'")
            .execute(&mut connection)
            .await
            .expect("set UTF-16LE before schema creation");
        sqlx::query("CREATE TABLE encoding_anchor (value TEXT NOT NULL)")
            .execute(&mut connection)
            .await
            .expect("materialize UTF-16LE database");
        sqlx::query("DROP TABLE encoding_anchor")
            .execute(&mut connection)
            .await
            .expect("return UTF-16LE database to empty catalog");
        let actual: String = sqlx::query_scalar("PRAGMA main.encoding")
            .fetch_one(&mut connection)
            .await
            .expect("read fixture encoding");
        assert_eq!(actual, "UTF-16le");
        connection.close().await.expect("close UTF-16 fixture");
    }

    async fn assert_utf16le_database_was_not_mutated(path: &Path) {
        let mut connection =
            SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(path))
                .await
                .expect("UTF-16 verification connection");
        let encoding: String = sqlx::query_scalar("PRAGMA main.encoding")
            .fetch_one(&mut connection)
            .await
            .expect("verification encoding");
        let journal_mode: String = sqlx::query_scalar("PRAGMA main.journal_mode")
            .fetch_one(&mut connection)
            .await
            .expect("verification journal mode");
        let event_store_objects: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM main.sqlite_schema WHERE name = 'radroots_event_store_schema_migrations' OR name = 'event_envelopes' OR name LIKE 'radroots_event_store_%'",
        )
        .fetch_one(&mut connection)
        .await
        .expect("verification event-store catalog");
        assert_eq!(encoding, "UTF-16le");
        assert_eq!(journal_mode, "delete");
        assert_eq!(event_store_objects, 0);
        connection.close().await.expect("close UTF-16 verifier");
    }

    fn synthetic_signed_event(
        kind: u32,
        created_at: u64,
        tags: Vec<Vec<String>>,
        content: &str,
    ) -> RadrootsSignedEvent {
        let pubkey = FIXTURE_ALICE_PUBLIC_KEY_HEX.to_owned();
        let content = content.to_owned();
        let id = compute_canonical_nip01_event_id(
            pubkey.as_str(),
            created_at,
            kind,
            &tags,
            content.as_str(),
        )
        .expect("event id")
        .into_string();
        let wire = RadrootsNip01EventWire {
            id,
            pubkey,
            created_at,
            kind,
            tags,
            content,
            sig: event_id('f').repeat(2),
            extra: Default::default(),
        };
        let raw_json = serde_json::to_string(&wire).expect("raw json");
        RadrootsSignedEvent::from_wire_verified_id(wire, raw_json).expect("signed event")
    }

    fn tamper_signature(event: &RadrootsSignedEvent) -> RadrootsSignedEvent {
        let mut wire = event.wire().clone();
        let replacement = if wire.sig.starts_with('0') { "1" } else { "0" };
        wire.sig.replace_range(0..1, replacement);
        let raw_json = serde_json::to_string(&wire).expect("raw json");
        RadrootsSignedEvent::from_wire_verified_id(wire, raw_json).expect("signed event")
    }

    fn tampered_content_raw_json(event: &RadrootsSignedEvent, content: &str) -> String {
        let mut wire = event.wire().clone();
        wire.content = content.to_owned();
        serde_json::to_string(&wire).expect("raw json")
    }

    fn operational_listing_tags(d_tag: &str) -> Vec<Vec<String>> {
        vec![
            vec!["d".to_owned(), d_tag.to_owned()],
            vec!["radroots:primary_bin".to_owned(), "bin-1".to_owned()],
        ]
    }

    fn admitted_operational_listing_tags(d_tag: &str, published_at: u64) -> Vec<Vec<String>> {
        vec![
            vec!["d".to_owned(), d_tag.to_owned()],
            vec!["p".to_owned(), FIXTURE_ALICE_PUBLIC_KEY_HEX.to_owned()],
            vec![
                "a".to_owned(),
                format!("{KIND_FARM}:{FIXTURE_ALICE_PUBLIC_KEY_HEX}:AAAAAAAAAAAAAAAAAAAAAA"),
            ],
            vec!["key".to_owned(), "carrot-nantes".to_owned()],
            vec!["title".to_owned(), "Nantes Carrots".to_owned()],
            vec!["category".to_owned(), "produce".to_owned()],
            vec![
                "summary".to_owned(),
                "Fresh bunches harvested in Saanich".to_owned(),
            ],
            vec!["published_at".to_owned(), published_at.to_string()],
            vec!["radroots:primary_bin".to_owned(), "bunch".to_owned()],
            vec![
                "radroots:bin".to_owned(),
                "bunch".to_owned(),
                "1".to_owned(),
                "each".to_owned(),
            ],
            vec![
                "radroots:price".to_owned(),
                "bunch".to_owned(),
                "4".to_owned(),
                "CAD".to_owned(),
                "1".to_owned(),
                "each".to_owned(),
            ],
            vec!["price".to_owned(), "4".to_owned(), "CAD".to_owned()],
            vec!["inventory".to_owned(), "24".to_owned()],
            vec!["status".to_owned(), "active".to_owned()],
            vec!["delivery".to_owned(), "pickup".to_owned()],
            vec![
                "location".to_owned(),
                "Saanich Peninsula".to_owned(),
                "Victoria".to_owned(),
                "BC".to_owned(),
                "CA".to_owned(),
            ],
            vec!["g".to_owned(), "c28hr".to_owned()],
        ]
    }

    fn head_coordinate_for_event(event: &RadrootsSignedEvent) -> RadrootsEventHeadCoordinate {
        let RadrootsEventHeadCandidateResult::Candidate(candidate) =
            event_head_candidate_for_nip01_event_v1(event.envelope())
        else {
            panic!("event should select a head");
        };
        candidate.coordinate
    }

    fn profile_coordinate() -> RadrootsEventHeadCoordinate {
        RadrootsEventHeadCoordinate::Replaceable {
            kind: KIND_PROFILE,
            pubkey: RadrootsPublicKey::parse(FIXTURE_ALICE_PUBLIC_KEY_HEX).expect("pubkey"),
        }
    }

    fn addressable_event(
        keys: &RadrootsNostrKeys,
        created_at: u32,
        d_tags: Vec<Vec<String>>,
        content: &str,
    ) -> RadrootsSignedEvent {
        signed_event_with_keys(keys, KIND_LIST_SET_RELAY, created_at, d_tags, content)
    }

    fn deletion_event(
        keys: &RadrootsNostrKeys,
        created_at: u32,
        tags: Vec<Vec<String>>,
    ) -> RadrootsSignedEvent {
        signed_event_with_keys(keys, KIND_DELETION_REQUEST, created_at, tags, "")
    }

    fn food_availability_event(
        created_at: u32,
        d_tag: &str,
        title: &str,
        summary: &str,
        status: &str,
        mut images: Vec<Vec<String>>,
    ) -> RadrootsSignedEvent {
        let mut tags = vec![
            vec!["d".to_owned(), d_tag.to_owned()],
            vec!["title".to_owned(), title.to_owned()],
            vec!["summary".to_owned(), summary.to_owned()],
            vec!["published_at".to_owned(), "100".to_owned()],
            vec!["location".to_owned(), "Central Saanich, BC".to_owned()],
            vec!["price".to_owned(), "3".to_owned(), "CAD".to_owned()],
            vec!["radroots:price_unit".to_owned(), "lb".to_owned()],
            vec![
                "radroots:quantity".to_owned(),
                "10".to_owned(),
                "lb".to_owned(),
            ],
            vec!["status".to_owned(), status.to_owned()],
        ];
        tags.append(&mut images);
        signed_event(
            KIND_CLASSIFIED_LISTING,
            created_at,
            tags,
            format!("{summary} Available in Victoria this week.").as_str(),
        )
    }

    async fn food_availability_audit_corruption_store() -> RadrootsEventStore {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let event = food_availability_event(
            200,
            "audit-corruption-carrots",
            "Audit Corruption Carrots",
            "Fresh audit harvest",
            "active",
            vec![vec![
                "image".to_owned(),
                "https://media.example/2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824.webp"
                    .to_owned(),
                "800x600".to_owned(),
            ]],
        );
        store
            .ingest_event(RadrootsEventIngest::new(event, 19_050))
            .await
            .expect("FoodAvailability audit fixture ingest");
        store
    }

    async fn suppressed_food_visibility_store() -> (RadrootsEventStore, String) {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let food = food_availability_event(
            220,
            "visibility-drift-carrots",
            "Visibility Drift Carrots",
            "Suppressed harvest",
            "active",
            Vec::new(),
        );
        let food_id = food.id_str().to_owned();
        let deletion = deletion_event(
            &fixture_keys(),
            230,
            vec![vec![
                "a".to_owned(),
                food_availability_coordinate("visibility-drift-carrots"),
            ]],
        );
        for (observed_at_ms, event) in [(19_100, food), (19_101, deletion)] {
            store
                .ingest_event(RadrootsEventIngest::new(event, observed_at_ms))
                .await
                .expect("suppressed visibility fixture ingest");
        }
        (store, food_id)
    }

    async fn transition_cause_corruption_store() -> RadrootsEventStore {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let regular = signed_event(KIND_POST, 240, Vec::new(), "transition cause fixture");
        let food = food_availability_event(
            250,
            "transition-cause-carrots",
            "Transition Cause Carrots",
            "Transition cause harvest",
            "active",
            Vec::new(),
        );
        for (observed_at_ms, event) in [(19_300, regular), (19_301, food)] {
            store
                .ingest_event(RadrootsEventIngest::new(event, observed_at_ms))
                .await
                .expect("transition cause fixture ingest");
        }
        store
    }

    async fn replacement_transition_corruption_store() -> RadrootsEventStore {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let first = food_availability_event(
            260,
            "transition-lineage-carrots",
            "Transition Lineage Carrots",
            "First lineage harvest",
            "active",
            Vec::new(),
        );
        let second = food_availability_event(
            270,
            "transition-lineage-carrots",
            "Transition Lineage Carrots",
            "Second lineage harvest",
            "sold",
            Vec::new(),
        );
        for (observed_at_ms, event) in [(19_400, first), (19_401, second)] {
            store
                .ingest_event(RadrootsEventIngest::new(event, observed_at_ms))
                .await
                .expect("replacement transition fixture ingest");
        }
        store
    }

    async fn non_admitted_retraction_corruption_store() -> RadrootsEventStore {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let malformed = signed_event(
            30_402,
            280,
            vec![vec!["d".to_owned(), "non-admitted-retraction".to_owned()]],
            "malformed FoodAvailability",
        );
        let admitted = food_availability_event(
            290,
            "non-admitted-retraction",
            "Admitted Retraction Carrots",
            "Admitted harvest",
            "active",
            Vec::new(),
        );
        for (observed_at_ms, event) in [(19_500, malformed), (19_501, admitted)] {
            store
                .ingest_event(RadrootsEventIngest::new(event, observed_at_ms))
                .await
                .expect("non-admitted retraction fixture ingest");
        }
        store
    }

    async fn transition_feed_error_after_trusted_corruption(
        store: &RadrootsEventStore,
        additional_guards: &[&'static str],
        mutations: &[&'static str],
    ) -> RadrootsEventStoreError {
        let mut connection = store.pool().acquire().await.expect("trusted connection");
        sqlx::query("DROP TRIGGER radroots_event_store_addressable_transition_update_guard")
            .execute(&mut *connection)
            .await
            .expect("trusted transition guard removal");
        for guard in additional_guards {
            sqlx::query(*guard)
                .execute(&mut *connection)
                .await
                .expect("trusted transition dependency guard removal");
        }
        sqlx::query("PRAGMA foreign_keys = OFF")
            .execute(&mut *connection)
            .await
            .expect("disable trusted foreign-key enforcement");
        sqlx::query("PRAGMA ignore_check_constraints = ON")
            .execute(&mut *connection)
            .await
            .expect("enable trusted check-constraint bypass");
        for mutation in mutations {
            sqlx::query(*mutation)
                .execute(&mut *connection)
                .await
                .expect("trusted transition lineage corruption");
        }
        sqlx::query("PRAGMA ignore_check_constraints = OFF")
            .execute(&mut *connection)
            .await
            .expect("restore check-constraint enforcement");
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&mut *connection)
            .await
            .expect("restore foreign-key enforcement");
        drop(connection);

        let scope = crate::RadrootsAddressableTransitionScopeV1::food_availability();
        store
            .addressable_transition_page_v1(&scope, None, 64)
            .await
            .expect_err("corrupt transition lineage must fail public feed read")
    }

    fn calendar_date_event(
        created_at: u32,
        d_tag: &str,
        content: impl Into<String>,
    ) -> RadrootsSignedEvent {
        let content = content.into();
        signed_event(
            KIND_CALENDAR_DATE_EVENT,
            created_at,
            vec![
                vec!["d".to_owned(), d_tag.to_owned()],
                vec!["title".to_owned(), "Victoria Market Day".to_owned()],
                vec!["start".to_owned(), "2026-07-20".to_owned()],
                vec!["end".to_owned(), "2026-07-21".to_owned()],
                vec!["location".to_owned(), "Victoria, BC".to_owned()],
            ],
            content.as_str(),
        )
    }

    fn food_availability_coordinate(d_tag: &str) -> String {
        format!("{KIND_CLASSIFIED_LISTING}:{FIXTURE_ALICE_PUBLIC_KEY_HEX}:{d_tag}")
    }

    fn addressable_coordinate(d_tag: &str) -> String {
        format!("{KIND_LIST_SET_RELAY}:{FIXTURE_ALICE_PUBLIC_KEY_HEX}:{d_tag}")
    }

    async fn rollback_store_to_v1(store: &RadrootsEventStore) {
        rollback_event_store_schema_offline_destructive_for_migration_test(store.pool(), 1)
            .await
            .expect("test-only destructive rollback to v1");
        assert_eq!(
            inspect_event_store_schema_status(store.pool())
                .await
                .expect("v1 status"),
            RadrootsEventStoreSchemaStatus::Managed { version: 1 }
        );
    }

    async fn rollback_store_to_v2(store: &RadrootsEventStore) {
        rollback_event_store_schema_offline(store.pool(), 2)
            .await
            .expect("rollback to v2");
        assert_eq!(
            inspect_event_store_schema_status(store.pool())
                .await
                .expect("v2 status"),
            RadrootsEventStoreSchemaStatus::Managed { version: 2 }
        );
    }

    async fn migrate_store_with_generation(
        store: &RadrootsEventStore,
        generation: [u8; 32],
    ) -> Result<(), RadrootsEventStoreError> {
        crate::schema::migrate_event_store_schema_with_generation_provider(
            store.pool(),
            &FixedGeneration(generation),
        )
        .await
    }

    async fn migrate_store_with_generation_and_limits(
        store: &RadrootsEventStore,
        generation: [u8; 32],
        limits: crate::nip09::reconciliation_v1::ReconciliationCapacityLimits,
    ) -> Result<(), RadrootsEventStoreError> {
        crate::schema::migrate_event_store_schema_with_generation_provider_and_limits(
            store.pool(),
            &FixedGeneration(generation),
            limits,
        )
        .await
    }

    async fn raw_authority_digest(store: &RadrootsEventStore) -> String {
        let envelope_rows = sqlx::query(
            "SELECT seq, event_id, pubkey, created_at, kind, tags_json, content, sig, raw_json, inserted_at_ms FROM event_envelopes ORDER BY seq",
        )
        .fetch_all(store.pool())
        .await
        .expect("raw envelopes");
        let tag_rows = sqlx::query(
            "SELECT event_id, tag_index, tag_name, tag_value, tag_json FROM event_envelope_tags ORDER BY event_id, tag_index",
        )
        .fetch_all(store.pool())
        .await
        .expect("raw tags");
        let mut digest = Sha256::new();
        for row in envelope_rows {
            for field in [
                row.try_get::<i64, _>("seq").expect("seq").to_string(),
                row.try_get::<String, _>("event_id").expect("event_id"),
                row.try_get::<String, _>("pubkey").expect("pubkey"),
                row.try_get::<i64, _>("created_at")
                    .expect("created_at")
                    .to_string(),
                row.try_get::<i64, _>("kind").expect("kind").to_string(),
                row.try_get::<String, _>("tags_json").expect("tags_json"),
                row.try_get::<String, _>("content").expect("content"),
                row.try_get::<String, _>("sig").expect("sig"),
                row.try_get::<String, _>("raw_json").expect("raw_json"),
                row.try_get::<i64, _>("inserted_at_ms")
                    .expect("inserted_at_ms")
                    .to_string(),
            ] {
                digest.update(field.len().to_le_bytes());
                digest.update(field.as_bytes());
            }
        }
        for row in tag_rows {
            let fields = [
                Some(row.try_get::<String, _>("event_id").expect("event_id")),
                Some(
                    row.try_get::<i64, _>("tag_index")
                        .expect("tag_index")
                        .to_string(),
                ),
                Some(row.try_get::<String, _>("tag_name").expect("tag_name")),
                row.try_get::<Option<String>, _>("tag_value")
                    .expect("tag_value"),
                Some(row.try_get::<String, _>("tag_json").expect("tag_json")),
            ];
            for field in fields {
                match field {
                    Some(field) => {
                        digest.update([1]);
                        digest.update(field.len().to_le_bytes());
                        digest.update(field.as_bytes());
                    }
                    None => digest.update([0]),
                }
            }
        }
        hex::encode(digest.finalize())
    }

    type SourceAuthoritySnapshot = (Vec<u8>, i64, i64, i64, i64);

    async fn source_authority_snapshot(store: &RadrootsEventStore) -> SourceAuthoritySnapshot {
        sqlx::query_as(
            "SELECT active_generation, raw_event_count, raw_tag_count, raw_high_water_seq, last_transition_seq FROM radroots_event_store_source_state WHERE singleton = 1",
        )
        .fetch_one(store.pool())
        .await
        .expect("source authority snapshot")
    }

    async fn assert_no_event_or_trade_residue(
        store: &RadrootsEventStore,
        expected_source_authority: &SourceAuthoritySnapshot,
    ) {
        let actual_source_authority = source_authority_snapshot(store).await;
        assert_eq!(&actual_source_authority, expected_source_authority);

        let rows = sqlx::query(
            "SELECT 'event_envelopes' AS relation, COUNT(*) AS row_count FROM event_envelopes
             UNION ALL SELECT 'event_envelope_tags', COUNT(*) FROM event_envelope_tags
             UNION ALL SELECT 'event_transport_observation', COUNT(*) FROM event_transport_observation
             UNION ALL SELECT 'event_envelope_head', COUNT(*) FROM event_envelope_head
             UNION ALL SELECT 'radroots_event_store_event_coordinate', COUNT(*) FROM radroots_event_store_event_coordinate
             UNION ALL SELECT 'radroots_event_store_nip09_request', COUNT(*) FROM radroots_event_store_nip09_request
             UNION ALL SELECT 'radroots_event_store_nip09_event_target', COUNT(*) FROM radroots_event_store_nip09_event_target
             UNION ALL SELECT 'radroots_event_store_nip09_address_target', COUNT(*) FROM radroots_event_store_nip09_address_target
             UNION ALL SELECT 'radroots_event_store_addressable_head_state', COUNT(*) FROM radroots_event_store_addressable_head_state
             UNION ALL SELECT 'radroots_event_store_addressable_head_transition', COUNT(*) FROM radroots_event_store_addressable_head_transition
             UNION ALL SELECT 'trade_mutation', COUNT(*) FROM trade_mutation
             UNION ALL SELECT 'trade_mutation_parent', COUNT(*) FROM trade_mutation_parent
             UNION ALL SELECT 'trade_missing_parent', COUNT(*) FROM trade_missing_parent
             UNION ALL SELECT 'trade_transport_envelope', COUNT(*) FROM trade_transport_envelope
             UNION ALL SELECT 'seller_inventory_reservation', COUNT(*) FROM seller_inventory_reservation
             UNION ALL SELECT 'seller_inventory_reservation_line', COUNT(*) FROM seller_inventory_reservation_line
             UNION ALL SELECT 'trade_projection_checkpoint', COUNT(*) FROM trade_projection_checkpoint
             UNION ALL SELECT 'trade_projection_quarantine', COUNT(*) FROM trade_projection_quarantine",
        )
        .fetch_all(store.pool())
        .await
        .expect("event and trade residue counts");
        for row in rows {
            let relation: String = row.try_get("relation").expect("relation");
            let row_count: i64 = row.try_get("row_count").expect("row count");
            assert_eq!(
                row_count, 0,
                "{relation} retained rows after failed owned ingest"
            );
        }
    }

    async fn nip09_v2_object_count(store: &RadrootsEventStore) -> i64 {
        sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_schema WHERE name LIKE 'radroots_event_store_%' AND name != 'radroots_event_store_schema_migrations'",
        )
        .fetch_one(store.pool())
        .await
        .expect("v2 object count")
    }

    async fn validate_nip09_authority(store: &RadrootsEventStore) {
        let mut connection = store.pool().acquire().await.expect("connection");
        crate::nip09::reconciliation_v1::validate_applied_hook_state(&mut connection)
            .await
            .expect("deep NIP-09 authority");
    }

    async fn normalized_nip09_snapshot(store: &RadrootsEventStore) -> serde_json::Value {
        let coordinate_rows = sqlx::query(
            "SELECT event_id, coordinate_type, kind, pubkey, raw_d_tag, nip09_matchable, nip09_d_tag, admission_status, admission_code, contract_id FROM radroots_event_store_event_coordinate ORDER BY event_id",
        )
        .fetch_all(store.pool())
        .await
        .expect("coordinate rows")
        .into_iter()
        .map(|row| {
            serde_json::json!([
                row.try_get::<String, _>("event_id").expect("event_id"),
                row.try_get::<String, _>("coordinate_type")
                    .expect("coordinate_type"),
                row.try_get::<i64, _>("kind").expect("kind"),
                row.try_get::<String, _>("pubkey").expect("pubkey"),
                row.try_get::<String, _>("raw_d_tag").expect("raw_d_tag"),
                row.try_get::<i64, _>("nip09_matchable")
                    .expect("nip09_matchable"),
                row.try_get::<Option<String>, _>("nip09_d_tag")
                    .expect("nip09_d_tag"),
                row.try_get::<String, _>("admission_status")
                    .expect("admission_status"),
                row.try_get::<Option<String>, _>("admission_code")
                    .expect("admission_code"),
                row.try_get::<Option<String>, _>("contract_id")
                    .expect("contract_id"),
            ])
        })
        .collect::<Vec<_>>();
        let request_rows = sqlx::query(
            "SELECT request_event_id, request_pubkey, request_created_at FROM radroots_event_store_nip09_request ORDER BY request_event_id",
        )
        .fetch_all(store.pool())
        .await
        .expect("request rows")
        .into_iter()
        .map(|row| {
            serde_json::json!([
                row.try_get::<String, _>("request_event_id")
                    .expect("request_event_id"),
                row.try_get::<String, _>("request_pubkey")
                    .expect("request_pubkey"),
                row.try_get::<i64, _>("request_created_at")
                    .expect("request_created_at"),
            ])
        })
        .collect::<Vec<_>>();
        let event_target_rows = sqlx::query(
            "SELECT request_event_id, target_event_id, source_tag_index, source_tag_value FROM radroots_event_store_nip09_event_target ORDER BY request_event_id, target_event_id",
        )
        .fetch_all(store.pool())
        .await
        .expect("event target rows")
        .into_iter()
        .map(|row| {
            serde_json::json!([
                row.try_get::<String, _>("request_event_id")
                    .expect("request_event_id"),
                row.try_get::<String, _>("target_event_id")
                    .expect("target_event_id"),
                row.try_get::<i64, _>("source_tag_index")
                    .expect("source_tag_index"),
                row.try_get::<String, _>("source_tag_value")
                    .expect("source_tag_value"),
            ])
        })
        .collect::<Vec<_>>();
        let address_target_rows = sqlx::query(
            "SELECT request_event_id, target_kind, target_pubkey, target_d_tag, inclusive_cutoff, source_tag_index, source_tag_value FROM radroots_event_store_nip09_address_target ORDER BY request_event_id, target_kind, target_pubkey, target_d_tag",
        )
        .fetch_all(store.pool())
        .await
        .expect("address target rows")
        .into_iter()
        .map(|row| {
            serde_json::json!([
                row.try_get::<String, _>("request_event_id")
                    .expect("request_event_id"),
                row.try_get::<i64, _>("target_kind").expect("target_kind"),
                row.try_get::<String, _>("target_pubkey")
                    .expect("target_pubkey"),
                row.try_get::<String, _>("target_d_tag")
                    .expect("target_d_tag"),
                row.try_get::<i64, _>("inclusive_cutoff")
                    .expect("inclusive_cutoff"),
                row.try_get::<i64, _>("source_tag_index")
                    .expect("source_tag_index"),
                row.try_get::<String, _>("source_tag_value")
                    .expect("source_tag_value"),
            ])
        })
        .collect::<Vec<_>>();
        let state_rows = sqlx::query(
            "SELECT kind, pubkey, d_tag, raw_head_event_id, raw_head_created_at, admission_status, admission_code, contract_id, visibility, nip09_outcome, nip09_reason, event_reference_request_id, address_reference_request_id, address_reference_cutoff FROM radroots_event_store_addressable_head_state ORDER BY kind, pubkey, d_tag",
        )
        .fetch_all(store.pool())
        .await
        .expect("state rows")
        .into_iter()
        .map(|row| {
            serde_json::json!([
                row.try_get::<i64, _>("kind").expect("kind"),
                row.try_get::<String, _>("pubkey").expect("pubkey"),
                row.try_get::<String, _>("d_tag").expect("d_tag"),
                row.try_get::<String, _>("raw_head_event_id")
                    .expect("raw_head_event_id"),
                row.try_get::<i64, _>("raw_head_created_at")
                    .expect("raw_head_created_at"),
                row.try_get::<String, _>("admission_status")
                    .expect("admission_status"),
                row.try_get::<Option<String>, _>("admission_code")
                    .expect("admission_code"),
                row.try_get::<Option<String>, _>("contract_id")
                    .expect("contract_id"),
                row.try_get::<String, _>("visibility")
                    .expect("visibility"),
                row.try_get::<Option<String>, _>("nip09_outcome")
                    .expect("nip09_outcome"),
                row.try_get::<Option<String>, _>("nip09_reason")
                    .expect("nip09_reason"),
                row.try_get::<Option<String>, _>("event_reference_request_id")
                    .expect("event_reference_request_id"),
                row.try_get::<Option<String>, _>("address_reference_request_id")
                    .expect("address_reference_request_id"),
                row.try_get::<Option<i64>, _>("address_reference_cutoff")
                    .expect("address_reference_cutoff"),
            ])
        })
        .collect::<Vec<_>>();
        serde_json::json!({
            "coordinates": coordinate_rows,
            "requests": request_rows,
            "event_targets": event_target_rows,
            "address_targets": address_target_rows,
            "states": state_rows,
        })
    }

    async fn assert_contiguous_transition_sequence(store: &RadrootsEventStore) {
        let sequences = sqlx::query_scalar::<_, i64>(
            "SELECT transition_seq FROM radroots_event_store_addressable_head_transition ORDER BY transition_seq",
        )
        .fetch_all(store.pool())
        .await
        .expect("transition sequence");
        for (index, sequence) in sequences.iter().enumerate() {
            assert_eq!(*sequence, i64::try_from(index + 1).expect("sequence"));
        }
        let last_transition: i64 = sqlx::query_scalar(
            "SELECT last_transition_seq FROM radroots_event_store_source_state WHERE singleton = 1",
        )
        .fetch_one(store.pool())
        .await
        .expect("last transition");
        assert_eq!(
            last_transition,
            sequences.last().copied().unwrap_or_default()
        );
    }

    async fn assert_sql_rejected(store: &RadrootsEventStore, statement: &'static str) {
        let error = sqlx::query(statement)
            .execute(store.pool())
            .await
            .expect_err(statement);
        assert!(
            matches!(error, sqlx::Error::Database(_)),
            "statement was rejected outside SQLite authority: {statement}: {error}"
        );
    }

    async fn insert_pending_raw_envelope(
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        signed_event: RadrootsSignedEvent,
        observed_at_ms: i64,
    ) {
        let ingest = RadrootsEventIngest::new(signed_event, observed_at_ms);
        let event = ingest.event();
        let tags_json = serde_json::to_string(&event.tags_as_vec()).expect("tags json");
        sqlx::query(
            "INSERT INTO event_envelopes(event_id, pubkey, created_at, kind, tags_json, content, sig, raw_json, verification_status, contract_status, contract_id, event_class, projection_eligible, inserted_at_ms, updated_at_ms) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'verified', 'admitted', 'radroots.post.v1', 'regular', 1, ?, ?)",
        )
        .bind(event.id_str())
        .bind(event.author_str())
        .bind(i64::try_from(event.created_at_u64()).expect("created_at"))
        .bind(i64::from(event.kind_u32()))
        .bind(tags_json)
        .bind(event.content())
        .bind(event.sig_str())
        .bind(ingest.raw_json())
        .bind(observed_at_ms)
        .bind(observed_at_ms)
        .execute(&mut **tx)
        .await
        .expect("pending raw envelope");
        for (tag_index, tag) in event.tags_as_vec().into_iter().enumerate() {
            let tag_name = tag.first().map(String::as_str).unwrap_or("");
            let tag_value = tag.get(1).map(String::as_str);
            let tag_json = serde_json::to_string(&tag).expect("tag JSON");
            sqlx::query(
                "INSERT INTO event_envelope_tags(event_id, tag_index, tag_name, tag_value, tag_json, contract_semantic, contract_value_type, relay_indexed) VALUES (?, ?, ?, ?, ?, NULL, NULL, 0)",
            )
            .bind(event.id_str())
            .bind(i64::try_from(tag_index).expect("tag index"))
            .bind(tag_name)
            .bind(tag_value)
            .bind(tag_json)
            .execute(&mut **tx)
            .await
            .expect("pending raw tag");
        }
    }

    #[tokio::test]
    async fn event_ingest_rejects_negative_timestamp_before_storage() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let signed = signed_event(KIND_POST, 10, Vec::new(), "negative ingest time");

        assert!(matches!(
            RadrootsEventIngest::from_signed_event(signed.clone(), -1),
            Err(RadrootsEventStoreError::InvalidEventIngestTimestamp { value: -1 })
        ));
        assert!(matches!(
            RadrootsEventIngest::from_raw_json(signed.raw_json(), -2),
            Err(RadrootsEventStoreError::InvalidEventIngestTimestamp { value: -2 })
        ));
        assert_eq!(
            store
                .status_summary()
                .await
                .expect("event-store summary")
                .total_events,
            0
        );
    }

    #[tokio::test]
    async fn nip09_migration_round_trip_preserves_v1_authority_and_rotates_generation() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let target = addressable_event(
            &fixture_keys(),
            20,
            vec![vec!["d".to_owned(), "round-trip".to_owned()]],
            "{}",
        );
        let deletion = deletion_event(
            &fixture_keys(),
            30,
            vec![vec!["e".to_owned(), target.id_str().to_owned()]],
        );
        let target_receipt = store
            .ingest_event(RadrootsEventIngest::new(target.clone(), 2_000))
            .await
            .expect("target");
        assert_eq!(
            target_receipt.admission_status,
            RadrootsEventAdmissionStatus::Admitted
        );
        store
            .ingest_event(RadrootsEventIngest::new(deletion.clone(), 3_000))
            .await
            .expect("deletion");
        let first_generation = store.source_generation().await.expect("first generation");
        let expected_raw_digest = raw_authority_digest(&store).await;

        rollback_store_to_v1(&store).await;
        sqlx::query(
            "UPDATE event_envelopes SET verification_status = 'legacy', contract_status = 'supported', contract_id = NULL, event_class = NULL, projection_eligible = 0, updated_at_ms = -1",
        )
        .execute(store.pool())
        .await
        .expect("stale derived envelope fields");
        sqlx::query(
            "UPDATE event_envelope_tags SET contract_semantic = 'legacy', contract_value_type = 'legacy', relay_indexed = 7",
        )
        .execute(store.pool())
        .await
        .expect("stale derived tag fields");
        sqlx::query("DELETE FROM event_envelope_head")
            .execute(store.pool())
            .await
            .expect("stale raw heads");
        sqlx::query(
            "INSERT INTO projection_cursor(projection_id, projection_version, last_event_seq, updated_at_ms) VALUES ('legacy', 1, 1, 10)",
        )
        .execute(store.pool())
        .await
        .expect("legacy cursor");
        sqlx::query("CREATE TABLE unrelated_owner_state(id INTEGER PRIMARY KEY, value TEXT)")
            .execute(store.pool())
            .await
            .expect("unrelated table");
        sqlx::query("INSERT INTO unrelated_owner_state(id, value) VALUES (1, 'preserve')")
            .execute(store.pool())
            .await
            .expect("unrelated row");
        assert_eq!(raw_authority_digest(&store).await, expected_raw_digest);

        let second_generation_bytes = [0x22; 32];
        migrate_store_with_generation(&store, second_generation_bytes)
            .await
            .expect("re-upgrade");
        let second_generation = store.source_generation().await.expect("second generation");
        assert_eq!(second_generation.as_bytes(), &second_generation_bytes);
        assert_ne!(second_generation, first_generation);
        assert_eq!(raw_authority_digest(&store).await, expected_raw_digest);
        let reconciled = store
            .raw_event(target.id_str())
            .await
            .expect("target read")
            .expect("target row");
        assert_eq!(
            reconciled.admission_status,
            RadrootsEventAdmissionStatus::Admitted
        );
        assert_eq!(
            reconciled.contract_id.as_deref(),
            Some("radroots.list_set.relay.v1")
        );
        assert!(reconciled.valid_stream_eligible);
        let state: (String, String, String) = sqlx::query_as(
            "SELECT visibility, nip09_outcome, nip09_reason FROM radroots_event_store_addressable_head_state WHERE d_tag = 'round-trip'",
        )
        .fetch_one(store.pool())
        .await
        .expect("suppressed state");
        assert_eq!(
            state,
            (
                "suppressed".to_owned(),
                "suppressed".to_owned(),
                "deletion_event_id_reference".to_owned(),
            )
        );
        assert!(matches!(
            store.projection_cursor("legacy", 1).await,
            Err(RadrootsEventStoreError::ProjectionCursorRebuildRequired {
                projection_id
            }) if projection_id == "legacy"
        ));
        let unrelated: String =
            sqlx::query_scalar("SELECT value FROM unrelated_owner_state WHERE id = 1")
                .fetch_one(store.pool())
                .await
                .expect("unrelated row");
        assert_eq!(unrelated, "preserve");
        validate_nip09_authority(&store).await;

        rollback_store_to_v1(&store).await;
        assert_eq!(nip09_v2_object_count(&store).await, 0);
        assert_eq!(raw_authority_digest(&store).await, expected_raw_digest);
        let cursor_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM projection_cursor WHERE projection_id = 'legacy'",
        )
        .fetch_one(store.pool())
        .await
        .expect("legacy cursor count");
        assert_eq!(cursor_count, 1);
        let unrelated: String =
            sqlx::query_scalar("SELECT value FROM unrelated_owner_state WHERE id = 1")
                .fetch_one(store.pool())
                .await
                .expect("unrelated row after rollback");
        assert_eq!(unrelated, "preserve");

        let third_generation_bytes = [0x33; 32];
        migrate_store_with_generation(&store, third_generation_bytes)
            .await
            .expect("second re-upgrade");
        assert_eq!(
            store
                .source_generation()
                .await
                .expect("third generation")
                .as_bytes(),
            &third_generation_bytes
        );
        assert_eq!(raw_authority_digest(&store).await, expected_raw_digest);
        validate_nip09_authority(&store).await;
    }

    #[tokio::test]
    async fn current_v4_rebuild_rotates_capacity_and_food_authority_end_to_end() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let food = food_availability_event(
            210,
            "v4-rebuild-carrots",
            "Nantes Carrots",
            "Fresh bunches",
            "active",
            Vec::new(),
        );
        store
            .ingest_event(RadrootsEventIngest::new(food.clone(), 2_100))
            .await
            .expect("FoodAvailability ingest");
        let before = store
            .source_capacity_v1()
            .await
            .expect("capacity before rebuild");
        let raw_digest = raw_authority_digest(&store).await;
        let target_generation = [0x44; 32];

        let mut transaction = store
            .begin_write_transaction()
            .await
            .expect("rebuild transaction");
        crate::nip09::reconciliation_v1::apply_reconciliation_hook(
            &mut transaction,
            &FixedGeneration(target_generation),
            crate::nip09::reconciliation_v1::ReconciliationCapacityLimits::production(),
        )
        .await
        .expect("current-v4 rebuild");
        transaction
            .commit()
            .await
            .expect("commit current-v4 rebuild");

        let after = store
            .source_capacity_v1()
            .await
            .expect("capacity after rebuild");
        assert_eq!(after.source_generation().as_bytes(), &target_generation);
        assert_eq!(after.raw_event_count(), before.raw_event_count());
        assert_eq!(after.raw_tag_count(), before.raw_tag_count());
        assert_eq!(after.raw_event_text_bytes(), before.raw_event_text_bytes());
        assert_eq!(after.raw_tag_text_bytes(), before.raw_tag_text_bytes());
        assert_eq!(after.raw_high_water_seq(), before.raw_high_water_seq());
        assert_eq!(
            after.retained_generation_count(),
            before.retained_generation_count() + 1
        );
        assert_eq!(
            after.retained_generation_limit(),
            before.retained_generation_limit()
        );
        assert_eq!(raw_authority_digest(&store).await, raw_digest);
        let marker_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM radroots_event_store_source_rebuild_marker")
                .fetch_one(store.pool())
                .await
                .expect("rebuild marker count");
        assert_eq!(marker_count, 0);
        let cursor_generation: Vec<u8> = sqlx::query_scalar(
            "SELECT source_generation FROM radroots_event_store_food_availability_cursor WHERE singleton = 1",
        )
        .fetch_one(store.pool())
        .await
        .expect("FoodAvailability cursor generation");
        assert_eq!(cursor_generation, target_generation);
        let projected = store
            .food_availability_v1(
                &RadrootsPublicKey::parse(FIXTURE_ALICE_PUBLIC_KEY_HEX).expect("author"),
                &RadrootsFoodIdentifier::parse("v4-rebuild-carrots").expect("identifier"),
            )
            .await
            .expect("projection lookup")
            .expect("projection after rebuild");
        assert_eq!(projected.event_id().as_str(), food.id_str());
        validate_nip09_authority(&store).await;
        store
            .audit_food_availability_projection_v1()
            .await
            .expect("FoodAvailability authority after rebuild");
    }

    #[tokio::test]
    async fn ninth_current_v4_rebuild_is_typed_and_preflight_atomic() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        for ordinal in 2_u8..=8 {
            let mut transaction = store
                .begin_write_transaction()
                .await
                .expect("rebuild transaction");
            crate::nip09::reconciliation_v1::apply_reconciliation_hook(
                &mut transaction,
                &FixedGeneration([ordinal; 32]),
                crate::nip09::reconciliation_v1::ReconciliationCapacityLimits::production(),
            )
            .await
            .expect("rebuild through retained generation eight");
            transaction.commit().await.expect("commit rebuild");
        }

        let capacity_before = store
            .source_capacity_v1()
            .await
            .expect("capacity at generation limit");
        assert_eq!(capacity_before.retained_generation_count(), 8);
        let source_before = source_authority_snapshot(&store).await;
        let raw_before = raw_authority_digest(&store).await;
        let nip09_before = normalized_nip09_snapshot(&store).await;
        let food_before: (Vec<u8>, i64, i64) = sqlx::query_as(
            "SELECT source_generation, last_transition_seq, projected_row_count FROM radroots_event_store_food_availability_cursor WHERE singleton = 1",
        )
        .fetch_one(store.pool())
        .await
        .expect("FoodAvailability cursor at generation limit");
        let derived_before: (i64, i64, i64) = sqlx::query_as(
            "SELECT (SELECT COUNT(*) FROM radroots_event_store_source_generation), (SELECT COUNT(*) FROM radroots_event_store_addressable_head_transition), (SELECT COUNT(*) FROM radroots_event_store_addressable_feed_integrity_v1)",
        )
        .fetch_one(store.pool())
        .await
        .expect("derived authority counts at generation limit");
        assert_eq!(derived_before.0, 8);

        let mut transaction = store
            .begin_write_transaction()
            .await
            .expect("ninth rebuild transaction");
        let error = crate::nip09::reconciliation_v1::apply_reconciliation_hook(
            &mut transaction,
            &PanickingGeneration,
            crate::nip09::reconciliation_v1::ReconciliationCapacityLimits::production(),
        )
        .await
        .expect_err("ninth rebuild must fail before entropy or mutation");
        assert!(matches!(
            error,
            RadrootsEventStoreError::SourceGenerationHistoryLimitReached {
                current: 8,
                limit: 8,
            }
        ));
        transaction
            .commit()
            .await
            .expect("commit mutation-free rejected rebuild transaction");

        assert_eq!(
            store
                .source_capacity_v1()
                .await
                .expect("capacity after rejected rebuild"),
            capacity_before
        );
        assert_eq!(source_authority_snapshot(&store).await, source_before);
        assert_eq!(raw_authority_digest(&store).await, raw_before);
        assert_eq!(normalized_nip09_snapshot(&store).await, nip09_before);
        let food_after: (Vec<u8>, i64, i64) = sqlx::query_as(
            "SELECT source_generation, last_transition_seq, projected_row_count FROM radroots_event_store_food_availability_cursor WHERE singleton = 1",
        )
        .fetch_one(store.pool())
        .await
        .expect("FoodAvailability cursor after rejected rebuild");
        assert_eq!(food_after, food_before);
        let derived_after: (i64, i64, i64, i64) = sqlx::query_as(
            "SELECT (SELECT COUNT(*) FROM radroots_event_store_source_generation), (SELECT COUNT(*) FROM radroots_event_store_addressable_head_transition), (SELECT COUNT(*) FROM radroots_event_store_addressable_feed_integrity_v1), (SELECT COUNT(*) FROM radroots_event_store_source_rebuild_marker)",
        )
        .fetch_one(store.pool())
        .await
        .expect("derived authority counts after rejected rebuild");
        assert_eq!(
            derived_after,
            (derived_before.0, derived_before.1, derived_before.2, 0)
        );
    }

    #[tokio::test]
    async fn food_projection_migration_backfills_v2_and_survives_rollback_reupgrade() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let food = food_availability_event(
            200,
            "migration-carrots",
            "Nantes Carrots",
            "Fresh bunches",
            "active",
            vec![vec![
                "image".to_owned(),
                "https://media.example/2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824.webp"
                    .to_owned(),
                "800x600".to_owned(),
            ]],
        );
        store
            .ingest_event(RadrootsEventIngest::new(food.clone(), 2_000))
            .await
            .expect("FoodAvailability ingest");
        let generation = store.source_generation().await.expect("source generation");
        let raw_digest = raw_authority_digest(&store).await;
        let projected = store
            .food_availability_v1(
                &RadrootsPublicKey::parse(FIXTURE_ALICE_PUBLIC_KEY_HEX).expect("author"),
                &RadrootsFoodIdentifier::parse("migration-carrots").expect("identifier"),
            )
            .await
            .expect("projection lookup")
            .expect("projection");
        assert_eq!(projected.event_id().as_str(), food.id_str());
        assert_eq!(projected.images().len(), 1);
        assert!(projected.images()[0].qualifies());
        assert_eq!(
            projected.images()[0].blossom_sha256(),
            Some(
                radroots_blossom::RadrootsBlossomSha256::from_hex(
                    "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824",
                )
                .expect("Blossom digest"),
            )
        );
        let search = crate::RadrootsFoodAvailabilitySearchQueryV1::parse("Nantes Carrots")
            .expect("search query");

        for cycle in 0..2 {
            rollback_store_to_v2(&store).await;
            let successor_objects: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM sqlite_schema WHERE name LIKE 'radroots_event_store_food_availability_%' OR name IN ('radroots_event_store_addressable_feed_generation_insert', 'radroots_event_store_addressable_feed_integrity_v1', 'radroots_event_store_addressable_feed_transition_insert', 'radroots_event_store_addressable_transition_coordinate_idx', 'radroots_event_store_current_visibility_head_lookup_idx', 'radroots_event_store_current_visibility_v1', 'radroots_event_store_nip09_address_target_visibility_lookup_idx')",
            )
            .fetch_one(store.pool())
            .await
            .expect("successor object count");
            assert_eq!(successor_objects, 0);
            assert_eq!(raw_authority_digest(&store).await, raw_digest);
            let transitions: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM radroots_event_store_addressable_head_transition WHERE kind = 30402",
            )
            .fetch_one(store.pool())
            .await
            .expect("v2 FoodAvailability transitions");
            assert_eq!(transitions, 1);

            migrate_store_with_generation(
                &store,
                [0x70 + u8::try_from(cycle).expect("bounded cycle"); 32],
            )
            .await
            .expect("v2 to v3 re-upgrade");
            assert_eq!(
                store.source_generation().await.expect("generation"),
                generation
            );
            assert_eq!(raw_authority_digest(&store).await, raw_digest);
            let rebuilt = store
                .food_availability_v1(
                    &RadrootsPublicKey::parse(FIXTURE_ALICE_PUBLIC_KEY_HEX).expect("author"),
                    &RadrootsFoodIdentifier::parse("migration-carrots").expect("identifier"),
                )
                .await
                .expect("rebuilt projection lookup")
                .expect("rebuilt projection");
            assert_eq!(rebuilt, projected);
            let matches = store
                .search_food_availability_v1(
                    &search,
                    crate::RadrootsFoodAvailabilityStatusFilterV1::Active,
                    10,
                )
                .await
                .expect("rebuilt FTS search");
            assert_eq!(matches, vec![projected.clone()]);
            let cursor_and_high_water: (i64, i64) = sqlx::query_as(
                "SELECT cursor.last_transition_seq, source.last_transition_seq FROM radroots_event_store_food_availability_cursor AS cursor JOIN radroots_event_store_source_state AS source ON source.singleton = 1 WHERE cursor.singleton = 1",
            )
            .fetch_one(store.pool())
            .await
            .expect("projection cursor high-water");
            assert_eq!(cursor_and_high_water.0, cursor_and_high_water.1);
        }
    }

    #[tokio::test]
    async fn nip09_migration_entropy_and_legacy_source_failures_are_atomic() {
        let entropy_store = RadrootsEventStore::open_memory().await.expect("open");
        let event = signed_event(KIND_POST, 20, Vec::new(), "entropy");
        entropy_store
            .ingest_event(RadrootsEventIngest::new(event, 2_000))
            .await
            .expect("seed");
        rollback_store_to_v1(&entropy_store).await;
        let before = raw_authority_digest(&entropy_store).await;
        assert!(matches!(
            crate::schema::migrate_event_store_schema_with_generation_provider(
                entropy_store.pool(),
                &FailingGeneration,
            )
            .await,
            Err(RadrootsEventStoreError::SourceGenerationEntropyUnavailable)
        ));
        assert_eq!(
            inspect_event_store_schema_status(entropy_store.pool())
                .await
                .expect("status"),
            RadrootsEventStoreSchemaStatus::Managed { version: 1 }
        );
        assert_eq!(nip09_v2_object_count(&entropy_store).await, 0);
        assert_eq!(raw_authority_digest(&entropy_store).await, before);

        for invalid_seq in [0_i64, -1, i64::MAX] {
            let store = RadrootsEventStore::open_memory().await.expect("open");
            let event = signed_event(KIND_POST, 21, Vec::new(), "invalid sequence");
            store
                .ingest_event(RadrootsEventIngest::new(event.clone(), 2_100))
                .await
                .expect("seed");
            rollback_store_to_v1(&store).await;
            sqlx::query("UPDATE event_envelopes SET seq = ? WHERE event_id = ?")
                .bind(invalid_seq)
                .bind(event.id_str())
                .execute(store.pool())
                .await
                .expect("install invalid legacy sequence");
            let before = raw_authority_digest(&store).await;
            assert!(matches!(
                migrate_store_with_generation(&store, [0x44; 32]).await,
                Err(RadrootsEventStoreError::MigrationHookStateDrift { .. })
            ));
            assert_eq!(
                inspect_event_store_schema_status(store.pool())
                    .await
                    .expect("status"),
                RadrootsEventStoreSchemaStatus::Managed { version: 1 }
            );
            assert_eq!(nip09_v2_object_count(&store).await, 0);
            assert_eq!(raw_authority_digest(&store).await, before);
        }

        let mismatch_store = RadrootsEventStore::open_memory().await.expect("open");
        let mismatch = signed_event(KIND_POST, 22, Vec::new(), "signed content");
        mismatch_store
            .ingest_event(RadrootsEventIngest::new(mismatch.clone(), 2_200))
            .await
            .expect("seed");
        rollback_store_to_v1(&mismatch_store).await;
        sqlx::query("UPDATE event_envelopes SET content = 'forged' WHERE event_id = ?")
            .bind(mismatch.id_str())
            .execute(mismatch_store.pool())
            .await
            .expect("forge immutable content");
        let before = raw_authority_digest(&mismatch_store).await;
        assert!(matches!(
            migrate_store_with_generation(&mismatch_store, [0x55; 32]).await,
            Err(RadrootsEventStoreError::RawEventReconciliationMismatch {
                field: "content",
                ..
            })
        ));
        assert_eq!(nip09_v2_object_count(&mismatch_store).await, 0);
        assert_eq!(raw_authority_digest(&mismatch_store).await, before);

        let cursor_store = RadrootsEventStore::open_memory().await.expect("open");
        rollback_store_to_v1(&cursor_store).await;
        sqlx::query(
            "INSERT INTO projection_cursor(projection_id, projection_version, last_event_seq, updated_at_ms) VALUES ('ahead', 1, 1, 1)",
        )
        .execute(cursor_store.pool())
        .await
        .expect("invalid legacy cursor");
        assert!(matches!(
            migrate_store_with_generation(&cursor_store, [0x66; 32]).await,
            Err(RadrootsEventStoreError::MigrationHookStateDrift { .. })
        ));
        assert_eq!(nip09_v2_object_count(&cursor_store).await, 0);
    }

    #[tokio::test]
    async fn nip09_migration_capacity_limits_are_exact_and_atomic() {
        assert_eq!(
            crate::nip09::reconciliation_v1::ReconciliationCapacityLimits::production(),
            crate::nip09::reconciliation_v1::ReconciliationCapacityLimits {
                raw_events: 25_000,
                raw_tags: 250_000,
                raw_event_bytes: 64 * 1024 * 1024,
                raw_tag_bytes: 32 * 1024 * 1024,
            }
        );
        assert_eq!(
            crate::RadrootsEventStoreSourceCapacityResourceV1::RawTagBytes.as_str(),
            "total retained raw-source tag row text bytes"
        );
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let event = signed_event(
            KIND_POST,
            23,
            vec![vec!["t".to_owned(), "capacity".to_owned()]],
            "bounded migration",
        );
        store
            .ingest_event(RadrootsEventIngest::new(event, 2_300))
            .await
            .expect("seed");
        rollback_store_to_v1(&store).await;

        let raw_event_bytes: i64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(length(CAST(event_id AS BLOB)) + length(CAST(pubkey AS BLOB)) + length(CAST(tags_json AS BLOB)) + length(CAST(content AS BLOB)) + length(CAST(sig AS BLOB)) + length(CAST(raw_json AS BLOB))), 0) FROM event_envelopes",
        )
        .fetch_one(store.pool())
        .await
        .expect("raw event bytes");
        let raw_tag_bytes: i64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(length(CAST(event_id AS BLOB)) + length(CAST(tag_name AS BLOB)) + COALESCE(length(CAST(tag_value AS BLOB)), 0) + length(CAST(tag_json AS BLOB))), 0) FROM event_envelope_tags",
        )
        .fetch_one(store.pool())
        .await
        .expect("raw tag bytes");
        let exact_limits = crate::nip09::reconciliation_v1::ReconciliationCapacityLimits {
            raw_events: 1,
            raw_tags: 1,
            raw_event_bytes: u64::try_from(raw_event_bytes).expect("raw event bytes"),
            raw_tag_bytes: u64::try_from(raw_tag_bytes).expect("raw tag bytes"),
        };
        let before = raw_authority_digest(&store).await;

        let below_limit_cases = [
            (
                crate::RadrootsEventStoreSourceCapacityResourceV1::RawEvents,
                crate::nip09::reconciliation_v1::ReconciliationCapacityLimits {
                    raw_events: 0,
                    ..exact_limits
                },
                0,
                1,
                0,
            ),
            (
                crate::RadrootsEventStoreSourceCapacityResourceV1::RawTags,
                crate::nip09::reconciliation_v1::ReconciliationCapacityLimits {
                    raw_tags: 0,
                    ..exact_limits
                },
                0,
                1,
                0,
            ),
            (
                crate::RadrootsEventStoreSourceCapacityResourceV1::RawEventBytes,
                crate::nip09::reconciliation_v1::ReconciliationCapacityLimits {
                    raw_event_bytes: exact_limits.raw_event_bytes - 1,
                    ..exact_limits
                },
                0,
                exact_limits.raw_event_bytes,
                exact_limits.raw_event_bytes - 1,
            ),
            (
                crate::RadrootsEventStoreSourceCapacityResourceV1::RawTagBytes,
                crate::nip09::reconciliation_v1::ReconciliationCapacityLimits {
                    raw_tag_bytes: exact_limits.raw_tag_bytes - 1,
                    ..exact_limits
                },
                0,
                exact_limits.raw_tag_bytes,
                exact_limits.raw_tag_bytes - 1,
            ),
        ];
        for (resource, limits, expected_current, expected_requested, expected_limit) in
            below_limit_cases
        {
            assert!(matches!(
                migrate_store_with_generation_and_limits(&store, [0x67; 32], limits).await,
                Err(RadrootsEventStoreError::SourceCapacityExceeded {
                    resource: actual_resource,
                    current,
                    requested,
                    limit,
                }) if actual_resource == resource
                    && current == expected_current
                    && requested == expected_requested
                    && limit == expected_limit
            ));
            assert_eq!(
                inspect_event_store_schema_status(store.pool())
                    .await
                    .expect("status"),
                RadrootsEventStoreSchemaStatus::Managed { version: 1 }
            );
            assert_eq!(nip09_v2_object_count(&store).await, 0);
            assert_eq!(raw_authority_digest(&store).await, before);
        }

        migrate_store_with_generation_and_limits(&store, [0x68; 32], exact_limits)
            .await
            .expect("exact capacity boundary");
        assert_eq!(raw_authority_digest(&store).await, before);
        validate_nip09_authority(&store).await;
    }

    #[tokio::test]
    async fn nip09_migration_capacity_rejects_oversized_legacy_duplicate_columns() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let event = signed_event(KIND_POST, 24, Vec::new(), "signed content");
        store
            .ingest_event(RadrootsEventIngest::new(event.clone(), 2_400))
            .await
            .expect("seed");
        rollback_store_to_v1(&store).await;
        let prior_raw_event_bytes: i64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(length(CAST(event_id AS BLOB)) + length(CAST(pubkey AS BLOB)) + length(CAST(tags_json AS BLOB)) + length(CAST(content AS BLOB)) + length(CAST(sig AS BLOB)) + length(CAST(raw_json AS BLOB))), 0) FROM event_envelopes",
        )
        .fetch_one(store.pool())
        .await
        .expect("prior raw event bytes");
        sqlx::query("UPDATE event_envelopes SET pubkey = ? WHERE event_id = ?")
            .bind("f".repeat(4_096))
            .bind(event.id_str())
            .execute(store.pool())
            .await
            .expect("oversized legacy pubkey");
        let oversized_raw_event_bytes: i64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(length(CAST(event_id AS BLOB)) + length(CAST(pubkey AS BLOB)) + length(CAST(tags_json AS BLOB)) + length(CAST(content AS BLOB)) + length(CAST(sig AS BLOB)) + length(CAST(raw_json AS BLOB))), 0) FROM event_envelopes",
        )
        .fetch_one(store.pool())
        .await
        .expect("oversized raw event bytes");
        let oversized_raw_event_bytes =
            u64::try_from(oversized_raw_event_bytes).expect("oversized raw event bytes");
        let limits = crate::nip09::reconciliation_v1::ReconciliationCapacityLimits {
            raw_event_bytes: u64::try_from(prior_raw_event_bytes).expect("prior raw event bytes"),
            ..crate::nip09::reconciliation_v1::ReconciliationCapacityLimits::production()
        };
        let before = raw_authority_digest(&store).await;

        let error = migrate_store_with_generation_and_limits(&store, [0x69; 32], limits)
            .await
            .expect_err("oversized duplicate column must exceed capacity");
        assert!(
            error
                .to_string()
                .contains("total retained raw-source event row text bytes")
        );
        assert!(matches!(
            error,
            RadrootsEventStoreError::SourceCapacityExceeded {
                resource: crate::RadrootsEventStoreSourceCapacityResourceV1::RawEventBytes,
                current,
                requested,
                limit,
            } if current == 0
                && requested == oversized_raw_event_bytes
                && requested > limit
                && limit == limits.raw_event_bytes
        ));
        assert_eq!(
            inspect_event_store_schema_status(store.pool())
                .await
                .expect("status"),
            RadrootsEventStoreSchemaStatus::Managed { version: 1 }
        );
        assert_eq!(nip09_v2_object_count(&store).await, 0);
        assert_eq!(raw_authority_digest(&store).await, before);
    }

    #[tokio::test]
    async fn nip09_migration_reconciliation_crosses_snapshot_page_boundaries() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        rollback_store_to_v1(&store).await;
        let mut transaction = store
            .pool()
            .begin_with("BEGIN IMMEDIATE")
            .await
            .expect("legacy seed transaction");
        for index in 0..513_u32 {
            insert_pending_raw_envelope(
                &mut transaction,
                signed_event(
                    KIND_POST,
                    100 + index,
                    vec![vec!["t".to_owned(), format!("page-{index}")]],
                    format!("paged event {index}").as_str(),
                ),
                10_000 + i64::from(index),
            )
            .await;
        }
        transaction.commit().await.expect("legacy seed commit");

        migrate_store_with_generation(&store, [0x6a; 32])
            .await
            .expect("paged migration");
        let source_counts: (i64, i64) = sqlx::query_as(
            "SELECT raw_event_count, raw_tag_count FROM radroots_event_store_source_state WHERE singleton = 1",
        )
        .fetch_one(store.pool())
        .await
        .expect("source counts");
        assert_eq!(source_counts, (513, 513));
        validate_nip09_authority(&store).await;
    }

    #[tokio::test]
    async fn nip09_migration_baseline_and_incremental_reconciliation_are_equivalent() {
        let exact_target = addressable_event(
            &fixture_keys(),
            20,
            vec![vec!["d".to_owned(), "exact".to_owned()]],
            "{}",
        );
        let exact_coordinate = addressable_coordinate("exact");
        let mixed_deletion = deletion_event(
            &fixture_keys(),
            10,
            vec![
                vec!["e".to_owned(), exact_target.id_str().to_owned()],
                vec!["e".to_owned(), exact_target.id_str().to_owned()],
                vec!["a".to_owned(), exact_coordinate.clone()],
                vec!["a".to_owned(), exact_coordinate],
                vec!["k".to_owned(), KIND_LIST_SET_RELAY.to_string()],
            ],
        );
        let cutoff_v1 = addressable_event(
            &fixture_keys(),
            20,
            vec![vec!["d".to_owned(), "cutoff".to_owned()]],
            "{}",
        );
        let cutoff_deletion = deletion_event(
            &fixture_keys(),
            30,
            vec![vec!["a".to_owned(), addressable_coordinate("cutoff")]],
        );
        let cutoff_v2 = addressable_event(
            &fixture_keys(),
            40,
            vec![vec!["d".to_owned(), "cutoff".to_owned()]],
            "{}",
        );
        let max_target = addressable_event(
            &fixture_keys(),
            35,
            vec![vec!["d".to_owned(), "max-cutoff".to_owned()]],
            "{}",
        );
        let max_deletion_30 = deletion_event(
            &fixture_keys(),
            30,
            vec![vec!["a".to_owned(), addressable_coordinate("max-cutoff")]],
        );
        let max_deletion_40 = deletion_event(
            &fixture_keys(),
            40,
            vec![vec!["a".to_owned(), addressable_coordinate("max-cutoff")]],
        );
        let wrong_target = addressable_event(
            &fixture_keys(),
            25,
            vec![vec!["d".to_owned(), "wrong-author".to_owned()]],
            "{}",
        );
        let wrong_deletion = deletion_event(
            &alternate_keys(),
            50,
            vec![vec!["e".to_owned(), wrong_target.id_str().to_owned()]],
        );
        let delete_deletion = deletion_event(
            &fixture_keys(),
            60,
            vec![vec!["e".to_owned(), mixed_deletion.id_str().to_owned()]],
        );
        let missing_d = addressable_event(&fixture_keys(), 1, Vec::new(), "{}");
        let valueless_d = addressable_event(&fixture_keys(), 2, vec![vec!["d".to_owned()]], "{}");
        let empty_d = addressable_event(
            &fixture_keys(),
            3,
            vec![vec!["d".to_owned(), String::new()]],
            "{}",
        );
        let first_d = addressable_event(
            &fixture_keys(),
            4,
            vec![
                vec!["d".to_owned(), "first".to_owned()],
                vec!["d".to_owned(), "later".to_owned()],
            ],
            "{}",
        );
        let events = vec![
            exact_target.clone(),
            mixed_deletion.clone(),
            cutoff_v1,
            cutoff_deletion,
            cutoff_v2.clone(),
            max_target,
            max_deletion_30,
            max_deletion_40.clone(),
            wrong_target,
            wrong_deletion,
            delete_deletion.clone(),
            missing_d.clone(),
            valueless_d.clone(),
            empty_d.clone(),
            first_d.clone(),
        ];

        let baseline = RadrootsEventStore::open_memory()
            .await
            .expect("baseline open");
        for (index, event) in events.iter().enumerate() {
            baseline
                .ingest_event(RadrootsEventIngest::new(
                    event.clone(),
                    10_000 + i64::try_from(index).expect("index"),
                ))
                .await
                .expect("baseline seed");
        }
        rollback_store_to_v1(&baseline).await;
        migrate_store_with_generation(&baseline, [0x71; 32])
            .await
            .expect("baseline migration");

        let incremental_forward = RadrootsEventStore::open_memory()
            .await
            .expect("incremental forward open");
        for (index, event) in events.iter().enumerate() {
            incremental_forward
                .ingest_event(RadrootsEventIngest::new(
                    event.clone(),
                    20_000 + i64::try_from(index).expect("index"),
                ))
                .await
                .expect("incremental forward");
        }

        let incremental_reverse = RadrootsEventStore::open_memory()
            .await
            .expect("incremental reverse open");
        for (index, event) in events.iter().rev().enumerate() {
            incremental_reverse
                .ingest_event(RadrootsEventIngest::new(
                    event.clone(),
                    30_000 + i64::try_from(index).expect("index"),
                ))
                .await
                .expect("incremental reverse");
        }

        let expected = normalized_nip09_snapshot(&baseline).await;
        assert_eq!(
            normalized_nip09_snapshot(&incremental_forward).await,
            expected
        );
        assert_eq!(
            normalized_nip09_snapshot(&incremental_reverse).await,
            expected
        );

        for store in [&baseline, &incremental_forward, &incremental_reverse] {
            validate_nip09_authority(store).await;
            assert_contiguous_transition_sequence(store).await;
            let before_duplicate: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM radroots_event_store_addressable_head_transition",
            )
            .fetch_one(store.pool())
            .await
            .expect("transition count");
            let receipt = store
                .ingest_event(RadrootsEventIngest::new(max_deletion_40.clone(), 40_000))
                .await
                .expect("duplicate deletion");
            assert!(receipt.persistence.is_duplicate());
            let after_duplicate: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM radroots_event_store_addressable_head_transition",
            )
            .fetch_one(store.pool())
            .await
            .expect("transition count");
            assert_eq!(after_duplicate, before_duplicate);
            assert!(
                store
                    .valid_event(delete_deletion.id_str())
                    .await
                    .expect("kind-5 query")
                    .is_some()
            );
        }

        let mixed_event_targets: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM radroots_event_store_nip09_event_target WHERE request_event_id = ?",
        )
        .bind(mixed_deletion.id_str())
        .fetch_one(baseline.pool())
        .await
        .expect("mixed event targets");
        let mixed_address_targets: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM radroots_event_store_nip09_address_target WHERE request_event_id = ?",
        )
        .bind(mixed_deletion.id_str())
        .fetch_one(baseline.pool())
        .await
        .expect("mixed address targets");
        assert_eq!((mixed_event_targets, mixed_address_targets), (1, 1));

        for (event, expected_raw_d, expected_matchable, expected_nip09_d) in [
            (&missing_d, "", 0_i64, None),
            (&valueless_d, "", 0_i64, None),
            (&empty_d, "", 1_i64, Some("")),
            (&first_d, "first", 1_i64, Some("first")),
        ] {
            let row = sqlx::query(
                "SELECT raw_d_tag, nip09_matchable, nip09_d_tag FROM radroots_event_store_event_coordinate WHERE event_id = ?",
            )
            .bind(event.id_str())
            .fetch_one(baseline.pool())
            .await
            .expect("coordinate fact");
            assert_eq!(
                row.try_get::<String, _>("raw_d_tag").expect("raw_d_tag"),
                expected_raw_d
            );
            assert_eq!(
                row.try_get::<i64, _>("nip09_matchable")
                    .expect("nip09_matchable"),
                expected_matchable
            );
            assert_eq!(
                row.try_get::<Option<String>, _>("nip09_d_tag")
                    .expect("nip09_d_tag")
                    .as_deref(),
                expected_nip09_d
            );
        }

        let state_rows = sqlx::query(
            "SELECT d_tag, raw_head_event_id, visibility, nip09_reason, address_reference_cutoff FROM radroots_event_store_addressable_head_state WHERE d_tag IN ('exact', 'cutoff', 'max-cutoff', 'wrong-author') ORDER BY d_tag",
        )
        .fetch_all(baseline.pool())
        .await
        .expect("scenario state");
        let state = state_rows
            .into_iter()
            .map(|row| {
                (
                    row.try_get::<String, _>("d_tag").expect("d_tag"),
                    (
                        row.try_get::<String, _>("raw_head_event_id")
                            .expect("raw_head_event_id"),
                        row.try_get::<String, _>("visibility").expect("visibility"),
                        row.try_get::<Option<String>, _>("nip09_reason")
                            .expect("nip09_reason"),
                        row.try_get::<Option<i64>, _>("address_reference_cutoff")
                            .expect("address_reference_cutoff"),
                    ),
                )
            })
            .collect::<BTreeMap<_, _>>();
        assert_eq!(
            state["exact"].1, "suppressed",
            "exact-id deletion is timestamp-independent"
        );
        assert_eq!(
            state["exact"].2.as_deref(),
            Some("deletion_event_id_reference")
        );
        assert_eq!(state["cutoff"].0, cutoff_v2.id_str());
        assert_eq!(state["cutoff"].1, "visible");
        assert_eq!(
            state["cutoff"].2.as_deref(),
            Some("deletion_address_cutoff_precedes_target")
        );
        assert_eq!(state["max-cutoff"].1, "suppressed");
        assert_eq!(state["max-cutoff"].3, Some(40));
        assert_eq!(state["wrong-author"].1, "visible");
        assert_eq!(
            state["wrong-author"].2.as_deref(),
            Some("deletion_request_author_mismatch")
        );
    }

    #[tokio::test]
    async fn nip09_migration_projection_rebuild_tickets_are_identity_bound() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        store
            .ingest_event(RadrootsEventIngest::new(
                signed_event(KIND_POST, 10, Vec::new(), "cursor source"),
                1_000,
            ))
            .await
            .expect("seed source");
        rollback_store_to_v1(&store).await;
        sqlx::query(
            "INSERT INTO projection_cursor(projection_id, projection_version, last_event_seq, updated_at_ms) VALUES ('legacy', 1, 0, 10)",
        )
        .execute(store.pool())
        .await
        .expect("legacy cursor");
        migrate_store_with_generation(&store, [0x81; 32])
            .await
            .expect("re-upgrade");

        assert!(matches!(
            store.projection_cursor("legacy", 1).await,
            Err(RadrootsEventStoreError::ProjectionCursorRebuildRequired {
                projection_id
            }) if projection_id == "legacy"
        ));
        let legacy_ticket = store
            .prepare_projection_cursor_rebuild("legacy", 1)
            .await
            .expect("legacy ticket");
        assert!(matches!(
            legacy_ticket.prior(),
            RadrootsProjectionRebuildPrior::Cursor {
                source_generation: None,
                source_revision: 1,
                projection_version: 1,
                last_event_seq: 0,
                updated_at_ms: 10,
            }
        ));
        let replay_ticket = legacy_ticket.clone();
        let rebuilt = store
            .reset_projection_cursor_after_rebuild(legacy_ticket, 20)
            .await
            .expect("legacy rebuild");
        assert_eq!(rebuilt.last_event_seq(), 1);
        assert_eq!(rebuilt.projection_version(), 1);
        assert!(matches!(
            store
                .reset_projection_cursor_after_rebuild(replay_ticket, 21)
                .await,
            Err(RadrootsEventStoreError::ProjectionRebuildTicketConflict {
                projection_id
            }) if projection_id == "legacy"
        ));
        assert!(matches!(
            store.projection_cursor("legacy", 2).await,
            Err(RadrootsEventStoreError::ProjectionVersionMismatch {
                projection_id,
                expected: 2,
                actual: 1,
            }) if projection_id == "legacy"
        ));
        assert!(matches!(
            store.prepare_projection_cursor_rebuild("legacy", 1).await,
            Err(RadrootsEventStoreError::ProjectionRebuildNotRequired {
                projection_id,
                projection_version: 1,
            }) if projection_id == "legacy"
        ));

        let generation = store.source_generation().await.expect("generation");

        let missing_success_ticket = store
            .prepare_projection_cursor_rebuild("missing-success", 1)
            .await
            .expect("missing-success ticket");
        assert!(matches!(
            missing_success_ticket.prior(),
            RadrootsProjectionRebuildPrior::Missing
        ));
        let missing_success = store
            .reset_projection_cursor_after_rebuild(missing_success_ticket, 29)
            .await
            .expect("missing-success rebuild");
        assert_eq!(missing_success.last_event_seq(), 1);
        assert_eq!(
            store
                .projection_cursor("missing-success", 1)
                .await
                .expect("missing-success cursor read")
                .expect("missing-success cursor"),
            missing_success
        );

        let mut high_water_ticket = store
            .prepare_projection_cursor_rebuild("ahead-ticket", 1)
            .await
            .expect("ahead ticket");
        high_water_ticket.target_raw_high_water_seq = 2;
        assert!(matches!(
            store
                .reset_projection_cursor_after_rebuild(high_water_ticket, 29)
                .await,
            Err(RadrootsEventStoreError::ProjectionCursorAheadOfSource {
                projection_id,
                proposed: 2,
                high_water: 1,
            }) if projection_id == "ahead-ticket"
        ));

        let ahead = RadrootsProjectionCursor::new("ahead", 1, generation, 2, 30).expect("ahead");
        assert!(matches!(
            store
                .compare_and_swap_projection_cursor(&ahead, None)
                .await,
            Err(RadrootsEventStoreError::ProjectionCursorAheadOfSource {
                projection_id,
                proposed: 2,
                high_water: 1,
            }) if projection_id == "ahead"
        ));
        let regression =
            RadrootsProjectionCursor::new("legacy", 1, generation, 0, 30).expect("regression");
        assert!(matches!(
            store
                .compare_and_swap_projection_cursor(&regression, Some(1))
                .await,
            Err(RadrootsEventStoreError::ProjectionCursorRegression {
                projection_id,
                current: 1,
                proposed: 0,
            }) if projection_id == "legacy"
        ));

        let missing_ticket = store
            .prepare_projection_cursor_rebuild("missing-race", 1)
            .await
            .expect("missing ticket");
        let missing_cursor =
            RadrootsProjectionCursor::new("missing-race", 1, generation, 1, 31).expect("cursor");
        store
            .compare_and_swap_projection_cursor(&missing_cursor, None)
            .await
            .expect("racing cursor insert");
        assert!(matches!(
            store
                .reset_projection_cursor_after_rebuild(missing_ticket, 32)
                .await,
            Err(RadrootsEventStoreError::ProjectionRebuildTicketConflict {
                projection_id
            }) if projection_id == "missing-race"
        ));

        let aba_ticket = store
            .prepare_projection_cursor_rebuild("legacy", 2)
            .await
            .expect("ABA ticket");
        let revision_before: i64 = sqlx::query_scalar(
            "SELECT source_revision FROM radroots_event_store_projection_cursor_source WHERE projection_id = 'legacy'",
        )
        .fetch_one(store.pool())
        .await
        .expect("revision before");
        let same_sequence =
            RadrootsProjectionCursor::new("legacy", 1, generation, 1, 33).expect("same sequence");
        store
            .compare_and_swap_projection_cursor(&same_sequence, Some(1))
            .await
            .expect("same-sequence CAS");
        let revision_after: i64 = sqlx::query_scalar(
            "SELECT source_revision FROM radroots_event_store_projection_cursor_source WHERE projection_id = 'legacy'",
        )
        .fetch_one(store.pool())
        .await
        .expect("revision after");
        assert_eq!(revision_after, revision_before + 1);
        assert!(matches!(
            store
                .reset_projection_cursor_after_rebuild(aba_ticket, 34)
                .await,
            Err(RadrootsEventStoreError::ProjectionRebuildTicketConflict {
                projection_id
            }) if projection_id == "legacy"
        ));

        let old_generation_ticket = store
            .prepare_projection_cursor_rebuild("legacy", 2)
            .await
            .expect("old-generation ticket");
        rollback_store_to_v1(&store).await;
        migrate_store_with_generation(&store, [0x82; 32])
            .await
            .expect("generation rotation");
        assert!(matches!(
            store
                .reset_projection_cursor_after_rebuild(old_generation_ticket, 35)
                .await,
            Err(RadrootsEventStoreError::ProjectionSourceGenerationMismatch {
                projection_id
            }) if projection_id == "legacy"
        ));
        validate_nip09_authority(&store).await;

        let ahead_store = RadrootsEventStore::open_memory()
            .await
            .expect("ahead store");
        ahead_store
            .ingest_event(RadrootsEventIngest::new(
                signed_event(KIND_POST, 11, Vec::new(), "ahead cursor source"),
                2_000,
            ))
            .await
            .expect("ahead cursor source");
        sqlx::query("DROP TRIGGER radroots_event_store_projection_cursor_insert_guard")
            .execute(ahead_store.pool())
            .await
            .expect("remove cursor high-water guard");
        sqlx::query(
            "INSERT INTO projection_cursor(projection_id, projection_version, last_event_seq, updated_at_ms) VALUES ('ahead-existing', 1, 2, 10)",
        )
        .execute(ahead_store.pool())
        .await
        .expect("install ahead cursor fixture");
        assert!(matches!(
            ahead_store
                .prepare_projection_cursor_rebuild("ahead-existing", 2)
                .await,
            Err(RadrootsEventStoreError::ProjectionCursorAheadOfSource {
                projection_id,
                proposed: 2,
                high_water: 1,
            }) if projection_id == "ahead-existing"
        ));
    }

    #[tokio::test]
    async fn nip09_migration_projection_rebuild_can_commit_at_captured_high_water() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        store
            .ingest_event(RadrootsEventIngest::new(
                signed_event(KIND_POST, 10, Vec::new(), "first source event"),
                1_000,
            ))
            .await
            .expect("first source event");
        rollback_store_to_v1(&store).await;
        sqlx::query(
            "INSERT INTO projection_cursor(projection_id, projection_version, last_event_seq, updated_at_ms) VALUES ('lagging-rebuild', 1, 0, 10)",
        )
        .execute(store.pool())
        .await
        .expect("legacy cursor");
        migrate_store_with_generation(&store, [0x83; 32])
            .await
            .expect("re-upgrade");

        let ticket = store
            .prepare_projection_cursor_rebuild("lagging-rebuild", 1)
            .await
            .expect("rebuild ticket");
        assert_eq!(ticket.target_raw_high_water_seq(), 1);

        store
            .ingest_event(RadrootsEventIngest::new(
                signed_event(KIND_POST, 11, Vec::new(), "concurrent source event"),
                2_000,
            ))
            .await
            .expect("concurrent source event");

        let rebuilt = store
            .reset_projection_cursor_after_rebuild(ticket, 20)
            .await
            .expect("commit rebuild at captured high-water");
        assert_eq!(rebuilt.last_event_seq(), 1);
        let stored = store
            .projection_cursor("lagging-rebuild", 1)
            .await
            .expect("projection cursor")
            .expect("stored projection cursor");
        assert_eq!(stored.last_event_seq(), 1);

        let caught_up =
            RadrootsProjectionCursor::new("lagging-rebuild", 1, stored.source_generation(), 2, 21)
                .expect("caught-up cursor");
        store
            .compare_and_swap_projection_cursor(&caught_up, Some(1))
            .await
            .expect("advance after rebuild");
        validate_nip09_authority(&store).await;
    }

    #[tokio::test]
    async fn nip09_migration_sql_authority_guards_reject_forgery_and_count_drift() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let target = addressable_event(
            &fixture_keys(),
            20,
            vec![vec!["d".to_owned(), "guard".to_owned()]],
            "{}",
        );
        let deletion = deletion_event(
            &fixture_keys(),
            30,
            vec![
                vec!["e".to_owned(), target.id_str().to_owned()],
                vec!["a".to_owned(), addressable_coordinate("guard")],
            ],
        );
        store
            .ingest_event(RadrootsEventIngest::new(target, 2_000))
            .await
            .expect("target");
        store
            .ingest_event(RadrootsEventIngest::new(deletion, 3_000))
            .await
            .expect("deletion");

        for statement in [
            "UPDATE event_envelopes SET content = content || '-forged' WHERE seq = (SELECT MIN(seq) FROM event_envelopes)",
            "DELETE FROM event_envelopes WHERE seq = (SELECT MIN(seq) FROM event_envelopes)",
            "UPDATE event_envelope_tags SET tag_value = tag_value || '-forged' WHERE rowid = (SELECT MIN(rowid) FROM event_envelope_tags)",
            "DELETE FROM event_envelope_tags WHERE rowid = (SELECT MIN(rowid) FROM event_envelope_tags)",
            "UPDATE event_envelope_head SET updated_at_ms = updated_at_ms + 1 WHERE coordinate_type = 'addressable'",
            "DELETE FROM event_envelope_head WHERE coordinate_type = 'addressable'",
            "UPDATE radroots_event_store_event_coordinate SET inserted_at_ms = inserted_at_ms + 1",
            "DELETE FROM radroots_event_store_event_coordinate",
            "UPDATE radroots_event_store_nip09_request SET request_created_at = request_created_at + 1",
            "DELETE FROM radroots_event_store_nip09_request",
            "UPDATE radroots_event_store_nip09_event_target SET source_tag_value = source_tag_value || '-forged'",
            "DELETE FROM radroots_event_store_nip09_event_target",
            "UPDATE radroots_event_store_nip09_address_target SET inclusive_cutoff = inclusive_cutoff + 1",
            "DELETE FROM radroots_event_store_nip09_address_target",
            "UPDATE radroots_event_store_addressable_head_state SET visibility = 'visible', nip09_outcome = 'visible', nip09_reason = 'deletion_no_authorized_reference', event_reference_request_id = NULL, address_reference_request_id = NULL, address_reference_cutoff = NULL WHERE d_tag = 'guard'",
            "INSERT INTO radroots_event_store_addressable_head_transition(source_generation, origin, kind, pubkey, d_tag, raw_head_event_id, raw_head_event_seq, raw_head_created_at, visible_event_id, visible_event_seq, retracted_event_id, retracted_event_seq, admission_status, admission_code, contract_id, visibility, nip09_outcome, nip09_reason, event_reference_request_id, address_reference_request_id, address_reference_cutoff, cause_event_seq, cause_event_id, raw_head_decision) SELECT source_generation, origin, kind, pubkey, d_tag, raw_head_event_id, raw_head_event_seq, raw_head_created_at, visible_event_id, visible_event_seq, retracted_event_id, retracted_event_seq, admission_status, admission_code, contract_id, visibility, nip09_outcome, nip09_reason, event_reference_request_id, address_reference_request_id, address_reference_cutoff, cause_event_seq, cause_event_id, raw_head_decision FROM radroots_event_store_addressable_head_transition ORDER BY transition_seq DESC LIMIT 1",
            "INSERT INTO radroots_event_store_addressable_head_transition SELECT * FROM radroots_event_store_addressable_head_transition ORDER BY transition_seq DESC LIMIT 1",
            "UPDATE radroots_event_store_addressable_head_transition SET raw_head_decision = 'not_head_selected' WHERE transition_seq = (SELECT MAX(transition_seq) FROM radroots_event_store_addressable_head_transition)",
            "DELETE FROM radroots_event_store_addressable_head_transition WHERE transition_seq = (SELECT MAX(transition_seq) FROM radroots_event_store_addressable_head_transition)",
            "DELETE FROM radroots_event_store_source_state",
            "DELETE FROM radroots_event_store_source_generation",
        ] {
            assert_sql_rejected(&store, statement).await;
        }

        let mut pending = store
            .pool()
            .begin_with("BEGIN IMMEDIATE")
            .await
            .expect("pending transaction");
        insert_pending_raw_envelope(
            &mut pending,
            signed_event(KIND_POST, 40, Vec::new(), "pending one"),
            4_000,
        )
        .await;
        insert_pending_raw_envelope(
            &mut pending,
            signed_event(KIND_POST, 41, Vec::new(), "pending two"),
            4_100,
        )
        .await;
        let count_drift = sqlx::query(
            "UPDATE radroots_event_store_source_state SET raw_event_count = raw_event_count + 1, raw_high_water_seq = (SELECT MAX(seq) FROM event_envelopes) WHERE singleton = 1",
        )
        .execute(&mut *pending)
        .await
        .expect_err("two pending raw envelopes must not advance one source row");
        assert!(matches!(count_drift, sqlx::Error::Database(_)));
        pending.rollback().await.expect("pending rollback");

        let legacy = RadrootsEventStore::open_memory()
            .await
            .expect("legacy open");
        rollback_store_to_v1(&legacy).await;
        sqlx::query(
            "INSERT INTO projection_cursor(projection_id, projection_version, last_event_seq, updated_at_ms) VALUES ('legacy-guard', 1, 0, 1)",
        )
        .execute(legacy.pool())
        .await
        .expect("legacy cursor");
        migrate_store_with_generation(&legacy, [0x91; 32])
            .await
            .expect("legacy migration");
        let legacy_source: Option<Vec<u8>> = sqlx::query_scalar(
            "SELECT source_generation FROM radroots_event_store_projection_cursor_source WHERE projection_id = 'legacy-guard'",
        )
        .fetch_one(legacy.pool())
        .await
        .expect("legacy source");
        assert!(legacy_source.is_none());
        for statement in [
            "UPDATE radroots_event_store_projection_cursor_source SET source_generation = (SELECT active_generation FROM radroots_event_store_source_state WHERE singleton = 1) WHERE projection_id = 'legacy-guard'",
            "DELETE FROM radroots_event_store_projection_cursor_source WHERE projection_id = 'legacy-guard'",
            "DELETE FROM projection_cursor WHERE projection_id = 'legacy-guard'",
        ] {
            assert_sql_rejected(&legacy, statement).await;
        }

        validate_nip09_authority(&store).await;
        validate_nip09_authority(&legacy).await;
    }

    #[tokio::test]
    async fn nip09_migration_file_writers_serialize_and_stale_deferred_snapshot_fails_closed() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let path = tempdir.path().join("nip09-concurrent.sqlite");
        let pool = SqlitePoolOptions::new()
            .max_connections(2)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(path)
                    .create_if_missing(true),
            )
            .await
            .expect("file pool");
        let store = RadrootsEventStore::open_pool(pool, true)
            .await
            .expect("file store");

        let first_event = signed_event(KIND_POST, 10, Vec::new(), "first writer");
        let second_event = signed_event(KIND_POST, 11, Vec::new(), "second writer");
        let mut first_tx = store
            .begin_write_transaction()
            .await
            .expect("first writer transaction");
        store
            .ingest_event_in_transaction(
                &mut first_tx,
                RadrootsEventIngest::new(first_event, 1_000),
            )
            .await
            .expect("first writer ingest");
        let concurrent_store = store.clone();
        let second_writer = tokio::spawn(async move {
            concurrent_store
                .ingest_event(RadrootsEventIngest::new(second_event, 1_100))
                .await
        });
        tokio::task::yield_now().await;
        assert!(!second_writer.is_finished());
        first_tx.commit().await.expect("first writer commit");
        second_writer
            .await
            .expect("second writer task")
            .expect("second writer ingest");
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM event_envelopes")
                .fetch_one(store.pool())
                .await
                .expect("serialized raw count"),
            2
        );

        let mut stale_tx = store.pool().begin().await.expect("deferred transaction");
        let stale_snapshot_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM event_envelopes")
            .fetch_one(&mut *stale_tx)
            .await
            .expect("deferred snapshot");
        assert_eq!(stale_snapshot_count, 2);
        store
            .ingest_event(RadrootsEventIngest::new(
                signed_event(KIND_POST, 12, Vec::new(), "snapshot invalidator"),
                1_200,
            ))
            .await
            .expect("snapshot invalidator");
        let stale_event = signed_event(KIND_POST, 13, Vec::new(), "stale writer");
        let stale_event_id = stale_event.id_str().to_owned();
        let stale_error = store
            .ingest_event_in_transaction(
                &mut stale_tx,
                RadrootsEventIngest::new(stale_event, 1_300),
            )
            .await
            .expect_err("a stale deferred snapshot must not upgrade to a writer");
        match stale_error {
            RadrootsEventStoreError::Sqlx(sqlx::Error::Database(error)) => {
                assert_eq!(error.code().as_deref(), Some("517"));
            }
            other => panic!("unexpected stale snapshot error: {other}"),
        }
        stale_tx.rollback().await.expect("stale rollback");
        assert!(
            store
                .raw_event(&stale_event_id)
                .await
                .expect("stale event query")
                .is_none()
        );
        validate_nip09_authority(&store).await;
    }

    async fn explain_query_plan(store: &RadrootsEventStore, sql: &str, bind: &str) -> String {
        let rows = sqlx::query(sqlx::AssertSqlSafe(sql.to_owned()))
            .bind(bind)
            .fetch_all(store.pool())
            .await
            .expect("query plan");
        rows.into_iter()
            .map(|row| row.try_get::<String, _>("detail").expect("detail"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[tokio::test]
    async fn constructor_enforces_sqlite_pragmas() {
        let store = RadrootsEventStore::open_memory().await.expect("open");

        assert_eq!(store.pragma_foreign_keys().await.expect("foreign_keys"), 1);
        assert_eq!(
            store.pragma_busy_timeout().await.expect("busy_timeout"),
            5000
        );
        assert_eq!(
            store.pragma_journal_mode().await.expect("journal"),
            "memory"
        );
    }

    #[tokio::test]
    async fn file_journal_mode_configuration_rejects_successful_non_wal_result() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let path = tempdir.path().join("immutable-delete.sqlite");
        let mut writer = SqliteConnection::connect_with(
            &SqliteConnectOptions::new()
                .filename(&path)
                .create_if_missing(true),
        )
        .await
        .expect("writer connection");
        let initial_mode: String = sqlx::query_scalar("PRAGMA main.journal_mode = DELETE")
            .fetch_one(&mut writer)
            .await
            .expect("delete journal mode");
        assert_eq!(initial_mode, "delete");
        writer.close().await.expect("close writer");

        let mut connection = SqliteConnection::connect_with(
            &SqliteConnectOptions::new().filename(&path).immutable(true),
        )
        .await
        .expect("immutable connection");

        assert!(matches!(
            configure_file_journal_mode(&mut connection).await,
            Err(RadrootsEventStoreError::SqliteFileJournalModeNotWal { actual })
                if actual == "delete"
        ));
    }

    #[tokio::test]
    async fn sqlite_busy_classification_is_exact() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let path = tempdir.path().join("busy-classification.sqlite");
        let mut holder = SqliteConnection::connect_with(
            &SqliteConnectOptions::new()
                .filename(&path)
                .create_if_missing(true),
        )
        .await
        .expect("holder connection");
        sqlx::query("CREATE TABLE fixture(value INTEGER NOT NULL)")
            .execute(&mut holder)
            .await
            .expect("fixture table");
        let mut contender =
            SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(&path))
                .await
                .expect("contender connection");
        sqlx::query("PRAGMA busy_timeout = 0")
            .execute(&mut contender)
            .await
            .expect("disable busy wait");
        sqlx::query("BEGIN EXCLUSIVE")
            .execute(&mut holder)
            .await
            .expect("exclusive holder");

        let busy = sqlx::query("BEGIN IMMEDIATE")
            .execute(&mut contender)
            .await
            .expect_err("contender must observe SQLITE_BUSY");
        assert!(sqlite_error_is_busy(&busy));
        assert!(sqlite_error_is_busy_or_locked(&busy));
        assert!(!sqlite_error_is_busy(&sqlx::Error::RowNotFound));
        assert!(!sqlite_error_is_busy_or_locked(&sqlx::Error::RowNotFound));

        sqlx::query("ROLLBACK")
            .execute(&mut holder)
            .await
            .expect("release holder");
    }

    #[tokio::test]
    async fn sqlite_busy_journal_mode_probe_fails_closed_until_reader_releases() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let path = tempdir.path().join("busy-journal-mode.sqlite");
        let mut initializer = SqliteConnection::connect_with(
            &SqliteConnectOptions::new()
                .filename(&path)
                .create_if_missing(true),
        )
        .await
        .expect("initializer connection");
        sqlx::query("CREATE TABLE fixture(value INTEGER NOT NULL)")
            .execute(&mut initializer)
            .await
            .expect("fixture table");
        initializer.close().await.expect("close initializer");

        let mut reader =
            SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(&path))
                .await
                .expect("reader connection");
        sqlx::query("BEGIN")
            .execute(&mut reader)
            .await
            .expect("reader transaction");
        let _: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM fixture")
            .fetch_one(&mut reader)
            .await
            .expect("establish read lock");

        let mut contender = SqliteConnection::connect_with(
            &SqliteConnectOptions::new()
                .filename(&path)
                .busy_timeout(std::time::Duration::ZERO),
        )
        .await
        .expect("contender connection");

        let error = configure_file_journal_mode(&mut contender)
            .await
            .expect_err("exclusive journal-mode probe must respect the read lock");
        assert!(matches!(
            error,
            RadrootsEventStoreError::Sqlx(ref source) if sqlite_error_is_busy_or_locked(source)
        ));

        sqlx::query("ROLLBACK")
            .execute(&mut reader)
            .await
            .expect("release read lock");
        configure_file_journal_mode(&mut contender)
            .await
            .expect("journal mode after reader release");
        assert_eq!(
            sqlx::query_scalar::<_, String>("PRAGMA main.journal_mode")
                .fetch_one(&mut contender)
                .await
                .expect("journal mode"),
            "wal"
        );
    }

    #[tokio::test]
    async fn open_file_rejects_utf16_main_database_before_schema_or_journal_mutation() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let path = tempdir.path().join("open-file-utf16.sqlite");
        initialize_utf16le_database(&path).await;

        let error = match RadrootsEventStore::open_file(&path).await {
            Ok(_) => panic!("UTF-16 main database must be rejected"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            RadrootsEventStoreError::SqliteMainDatabaseEncodingNotUtf8 { actual }
                if actual == "UTF-16le"
        ));
        assert_utf16le_database_was_not_mutated(&path).await;
    }

    #[tokio::test]
    async fn open_pool_rejects_utf16_main_database_before_schema_or_journal_mutation() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let path = tempdir.path().join("open-pool-utf16.sqlite");
        initialize_utf16le_database(&path).await;
        let pool = SqlitePoolOptions::new()
            .max_connections(2)
            .connect_with(SqliteConnectOptions::new().filename(&path))
            .await
            .expect("UTF-16 pool");

        let error = match RadrootsEventStore::open_pool(pool, true).await {
            Ok(_) => panic!("UTF-16 supplied pool must be rejected"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            RadrootsEventStoreError::SqliteMainDatabaseEncodingNotUtf8 { actual }
                if actual == "UTF-16le"
        ));
        assert_utf16le_database_was_not_mutated(&path).await;
    }

    #[tokio::test]
    async fn open_pool_configures_every_file_connection_and_rejects_multi_connection_memory() {
        let memory_options = SqliteConnectOptions::from_str("sqlite::memory:")
            .expect("memory options")
            .foreign_keys(false);
        let memory_pool = SqlitePoolOptions::new()
            .max_connections(2)
            .connect_with(memory_options)
            .await
            .expect("memory pool");
        assert!(matches!(
            RadrootsEventStore::open_pool(memory_pool, false).await,
            Err(RadrootsEventStoreError::UnsafeInMemoryPoolConnectionCount { actual: 2 })
        ));
        let mislabeled_memory_pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::from_str("sqlite::memory:")
                    .expect("memory options")
                    .foreign_keys(false),
            )
            .await
            .expect("mislabeled memory pool");
        assert!(matches!(
            RadrootsEventStore::open_pool(mislabeled_memory_pool, true).await,
            Err(RadrootsEventStoreError::SqlitePoolBackingMismatch {
                file_backed: true,
                ..
            })
        ));
        for memory_url in ["sqlite://?mode=memory", "sqlite://named?mode=memory"] {
            let mode_memory_pool = SqlitePoolOptions::new()
                .max_connections(2)
                .connect_with(
                    SqliteConnectOptions::from_str(memory_url)
                        .expect("mode-memory options")
                        .foreign_keys(false),
                )
                .await
                .expect("mode-memory pool");
            assert!(matches!(
                RadrootsEventStore::open_pool(mode_memory_pool, false).await,
                Err(RadrootsEventStoreError::UnsafeInMemoryPoolConnectionCount { actual: 2 })
            ));

            let mislabeled_mode_memory_pool = SqlitePoolOptions::new()
                .max_connections(1)
                .connect_with(
                    SqliteConnectOptions::from_str(memory_url)
                        .expect("mode-memory options")
                        .foreign_keys(false),
                )
                .await
                .expect("mislabeled mode-memory pool");
            assert!(matches!(
                RadrootsEventStore::open_pool(mislabeled_mode_memory_pool, true).await,
                Err(RadrootsEventStoreError::SqlitePoolBackingMismatch {
                    file_backed: true,
                    ..
                })
            ));
        }

        let tempdir = tempfile::tempdir().expect("tempdir");
        let path = tempdir.path().join("multi.sqlite");
        let file_options = SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true)
            .foreign_keys(false);
        let file_pool = SqlitePoolOptions::new()
            .max_connections(3)
            .connect_with(file_options)
            .await
            .expect("file pool");
        let store = RadrootsEventStore::open_pool(file_pool, true)
            .await
            .expect("store");
        let mut connections = Vec::new();
        for _ in 0..3 {
            connections.push(store.pool().acquire().await.expect("connection"));
        }
        for connection in &mut connections {
            let foreign_keys: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
                .fetch_one(&mut **connection)
                .await
                .expect("foreign keys");
            assert_eq!(foreign_keys, 1);
            let journal_mode: String = sqlx::query_scalar("PRAGMA main.journal_mode")
                .fetch_one(&mut **connection)
                .await
                .expect("journal mode");
            assert_eq!(journal_mode, "wal");
            let orphan = sqlx::query(
                "INSERT INTO event_envelope_tags(event_id, tag_index, tag_name, tag_value, tag_json, contract_semantic, contract_value_type, relay_indexed) VALUES ('missing', 0, 'd', 'value', '[\"d\",\"value\"]', NULL, NULL, 0)",
            )
            .execute(&mut **connection)
            .await;
            assert!(orphan.is_err());
        }
    }

    #[tokio::test]
    async fn open_pool_rejects_governed_temp_collision_on_any_file_connection() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let path = tempdir.path().join("temp-collision.sqlite");
        let pool = SqlitePoolOptions::new()
            .max_connections(2)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(&path)
                    .create_if_missing(true),
            )
            .await
            .expect("file pool");
        let first = pool.acquire().await.expect("first connection");
        let mut second = pool.acquire().await.expect("second connection");
        sqlx::query(
            "CREATE TEMP TABLE \"RaDrOoTs_EvEnT_StOrE_ScHeMa_MiGrAtIoNs\" (version INTEGER)",
        )
        .execute(&mut *second)
        .await
        .expect("collision on non-first connection");
        drop(second);
        drop(first);

        assert!(matches!(
            RadrootsEventStore::open_pool(pool, true).await,
            Err(RadrootsEventStoreError::TemporarySchemaCollision {
                name,
                ..
            }) if name == "RaDrOoTs_EvEnT_StOrE_ScHeMa_MiGrAtIoNs"
        ));
        let verifier = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(SqliteConnectOptions::new().filename(&path))
            .await
            .expect("verification pool");
        let main_object_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM main.sqlite_schema WHERE NOT (substr(name, 1, 7) = 'sqlite_' COLLATE NOCASE)",
        )
        .fetch_one(&verifier)
        .await
        .expect("main catalog");
        assert_eq!(main_object_count, 0);
    }

    #[tokio::test]
    async fn open_pool_allows_unrelated_temp_state_and_uses_pragma_database_list() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::from_str("sqlite::memory:").expect("memory options"),
            )
            .await
            .expect("memory pool");
        let mut connection = pool.acquire().await.expect("connection");
        sqlx::raw_sql(
            "CREATE TEMP TABLE caller_cache (value TEXT NOT NULL);
INSERT INTO caller_cache(value) VALUES ('preserved');
CREATE TEMP TABLE pragma_database_list (name TEXT NOT NULL, file TEXT NOT NULL);
INSERT INTO pragma_database_list(name, file) VALUES ('main', 'counterfeit.sqlite');",
        )
        .execute(&mut *connection)
        .await
        .expect("unrelated temporary state");
        drop(connection);

        let store = RadrootsEventStore::open_pool(pool, false)
            .await
            .expect("real PRAGMA database_list determines memory backing");
        let value: String = sqlx::query_scalar("SELECT value FROM temp.caller_cache")
            .fetch_one(store.pool())
            .await
            .expect("preserved temporary row");
        assert_eq!(value, "preserved");
        let fake_filename: String =
            sqlx::query_scalar("SELECT file FROM temp.pragma_database_list WHERE name = 'main'")
                .fetch_one(store.pool())
                .await
                .expect("preserved fake pragma table");
        assert_eq!(fake_filename, "counterfeit.sqlite");
    }

    #[tokio::test]
    async fn pool_status_inspection_does_not_initialize_an_unmigrated_pool() {
        let options = SqliteConnectOptions::from_str("sqlite::memory:").expect("options");
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .expect("pool");

        assert!(matches!(
            inspect_event_store_status(&pool).await,
            Err(RadrootsEventStoreError::Sqlx(_))
        ));
        let event_table_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'event_envelopes'",
        )
        .fetch_one(&pool)
        .await
        .expect("schema inspection");
        assert_eq!(event_table_count, 0);
    }

    #[tokio::test]
    async fn pool_status_inspection_never_resolves_attached_event_store_tables() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::from_str("sqlite::memory:").expect("memory options"),
            )
            .await
            .expect("memory pool");
        sqlx::raw_sql(
            "ATTACH DATABASE ':memory:' AS aux;
CREATE TABLE aux.event_envelopes (
  event_id TEXT,
  contract_status TEXT,
  verification_status TEXT,
  kind INTEGER,
  event_class TEXT,
  projection_eligible INTEGER,
  contract_id TEXT,
  seq INTEGER,
  updated_at_ms INTEGER
);
CREATE TABLE aux.event_transport_observation (event_id TEXT);",
        )
        .execute(&pool)
        .await
        .expect("attached lookalike schema");

        assert!(matches!(
            inspect_event_store_status(&pool).await,
            Err(RadrootsEventStoreError::Sqlx(_))
        ));
        let main_event_table_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM main.sqlite_schema WHERE name = 'event_envelopes'",
        )
        .fetch_one(&pool)
        .await
        .expect("main catalog");
        assert_eq!(main_event_table_count, 0);
    }

    #[tokio::test]
    async fn pool_status_inspection_rejects_governed_temp_schema() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::from_str("sqlite::memory:").expect("memory options"),
            )
            .await
            .expect("memory pool");
        sqlx::query("CREATE TEMP TABLE event_envelopes (event_id TEXT)")
            .execute(&pool)
            .await
            .expect("temporary collision");

        assert!(matches!(
            inspect_event_store_status(&pool).await,
            Err(RadrootsEventStoreError::TemporarySchemaCollision {
                name,
                ..
            }) if name == "event_envelopes"
        ));
    }

    #[tokio::test]
    async fn status_summary_counts_events_projections_and_transport_observations() {
        let store = RadrootsEventStore::open_memory().await.expect("open");

        let empty = store.status_summary().await.expect("empty status");
        assert_eq!(
            inspect_event_store_status(store.pool())
                .await
                .expect("empty pool status"),
            empty
        );
        assert_eq!(empty.total_events, 0);
        assert_eq!(empty.valid_stream_events, 0);
        assert_eq!(empty.transport_observations, 0);
        assert_eq!(empty.last_event_seq, None);
        assert_eq!(empty.last_event_updated_at_ms, None);

        let event = signed_event(
            KIND_POST,
            10,
            vec![vec!["t".to_owned(), "soil".to_owned()]],
            "hello",
        );
        let observation = RadrootsTransportObservation::new(
            RadrootsTransportKind::Nostr,
            "wss://relay.example.com",
            crate::RadrootsTransportObservationType::PublishAck,
            1_100,
        )
        .expect("observation");
        store
            .ingest_event(RadrootsEventIngest::new(event.clone(), 1_000))
            .await
            .expect("event ingest");
        store
            .ingest_event(RadrootsEventIngest::new(event, 1_100).with_observation(observation))
            .await
            .expect("observation ingest");

        let status = store.status_summary().await.expect("status");
        sqlx::query("PRAGMA query_only = ON")
            .execute(store.pool())
            .await
            .expect("read-only connection");
        let inspected = inspect_event_store_status(store.pool())
            .await
            .expect("read-only pool status");
        assert_eq!(inspected, status);
        assert_eq!(status.total_events, 1);
        assert_eq!(status.valid_stream_events, 1);
        assert_eq!(status.transport_observations, 1);
        assert_eq!(status.last_event_seq, Some(1));
        assert_eq!(status.last_event_updated_at_ms, Some(1_000));
    }

    #[tokio::test]
    async fn database_guards_reject_derived_envelope_mutation() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let event = signed_event(KIND_POST, 9, Vec::new(), "corruption target");
        store
            .ingest_event(RadrootsEventIngest::new(event.clone(), 900))
            .await
            .expect("ingest");

        for statement in [
            "UPDATE event_envelopes SET projection_eligible = 2 WHERE event_id = ?",
            "UPDATE event_envelopes SET verification_status = 'signature_invalid' WHERE event_id = ?",
            "UPDATE event_envelopes SET contract_id = NULL WHERE event_id = ?",
            "UPDATE event_envelopes SET event_class = 'replaceable' WHERE event_id = ?",
            "UPDATE event_envelopes SET kind = 20001 WHERE event_id = ?",
        ] {
            sqlx::query(statement)
                .bind(event.id_str())
                .execute(store.pool())
                .await
                .expect_err("envelope mutation must be rejected");
        }

        let raw = store
            .raw_event(event.id_str())
            .await
            .expect("raw read")
            .expect("stored event");
        assert_eq!(raw.admission_status, RadrootsEventAdmissionStatus::Admitted);
        assert!(raw.valid_stream_eligible);
        assert_eq!(
            store.status_summary().await.expect("status").total_events,
            1
        );
    }

    #[tokio::test]
    async fn file_store_reopens_existing_schema() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let path = tempdir.path().join("event_store.sqlite");

        let first = RadrootsEventStore::open_file(&path).await.expect("first");
        assert_eq!(first.pragma_foreign_keys().await.expect("foreign_keys"), 1);
        assert_eq!(first.pragma_journal_mode().await.expect("journal"), "wal");
        drop(first);

        let second = RadrootsEventStore::open_file(&path).await.expect("second");
        assert_eq!(second.pragma_foreign_keys().await.expect("foreign_keys"), 1);
        assert_eq!(second.pragma_journal_mode().await.expect("journal"), "wal");
    }

    #[tokio::test]
    async fn migration_installs_canonical_projection_tables() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let rows = sqlx::query(
            "SELECT name FROM sqlite_master WHERE type IN ('table', 'virtual table') ORDER BY name",
        )
        .fetch_all(store.pool())
        .await
        .expect("tables");
        let names = rows
            .into_iter()
            .map(|row| row.try_get::<String, _>("name").expect("name"))
            .collect::<Vec<_>>();

        assert!(names.iter().any(|name| name == "listing_projection"));
        for table in [
            "trade_mutation",
            "trade_mutation_parent",
            "trade_missing_parent",
            "trade_transport_envelope",
            "seller_inventory_reservation",
            "seller_inventory_reservation_line",
            "trade_projection_checkpoint",
            "trade_projection_quarantine",
        ] {
            assert!(names.iter().any(|name| name == table), "{table}");
        }
        assert!(!names.iter().any(|name| name == "trade_projection"));
        assert!(names.iter().any(|name| name == "listing_search_fts"));
    }

    #[tokio::test]
    async fn migration_installs_semantic_trade_mutation_keys() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let rows = sqlx::query("PRAGMA table_info(trade_mutation)")
            .fetch_all(store.pool())
            .await
            .expect("table info");
        let columns = rows
            .iter()
            .map(|row| {
                (
                    row.try_get::<String, _>("name").expect("name"),
                    row.try_get::<i64, _>("notnull").expect("notnull"),
                    row.try_get::<i64, _>("pk").expect("pk"),
                )
            })
            .collect::<Vec<_>>();
        let mut primary_key = columns
            .iter()
            .filter_map(|(name, _, pk)| (*pk > 0).then_some((name.as_str(), *pk)))
            .collect::<Vec<_>>();
        primary_key.sort_by_key(|(_, pk)| *pk);

        assert_eq!(primary_key, vec![("mutation_id", 1)]);
        assert!(
            columns
                .iter()
                .any(|(name, notnull, _)| name == "canonical_payload_bytes" && *notnull == 1)
        );
        assert!(
            columns
                .iter()
                .any(|(name, notnull, _)| name == "payload_sha256" && *notnull == 1)
        );
    }

    #[tokio::test]
    async fn managed_schema_can_be_destroyed_and_recreated_for_tests() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        assert_eq!(
            store.schema_status().await.expect("managed status"),
            RadrootsEventStoreSchemaStatus::Managed {
                version: crate::RADROOTS_EVENT_STORE_SCHEMA_VERSION_CURRENT,
            }
        );

        store.destroy_schema_for_test().await.expect("destroy");
        assert_eq!(
            inspect_event_store_schema_status(store.pool())
                .await
                .expect("uninitialized status"),
            RadrootsEventStoreSchemaStatus::Uninitialized
        );
        store
            .migrate_to_current_schema()
            .await
            .expect("recreate schema");
        assert_eq!(
            store.schema_status().await.expect("recreated status"),
            RadrootsEventStoreSchemaStatus::Managed {
                version: crate::RADROOTS_EVENT_STORE_SCHEMA_VERSION_CURRENT,
            }
        );
    }

    #[tokio::test]
    async fn rollback_is_terminal_for_every_clone_of_the_store_pool() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let clone = store.clone();

        store
            .rollback_to_schema_version_and_close(3)
            .await
            .expect("terminal rollback");

        assert!(clone.pool().is_closed());
        assert!(matches!(
            clone.schema_status().await,
            Err(RadrootsEventStoreError::Sqlx(sqlx::Error::PoolClosed))
        ));
    }

    #[tokio::test]
    async fn utf8_file_reopen_preserves_non_ascii_and_nul_capacity_accounting() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let path = tempdir.path().join("utf8-capacity-reopen.sqlite");
        let store = RadrootsEventStore::open_file(&path)
            .await
            .expect("UTF-8 file store");
        let event = signed_event(
            KIND_POST,
            10,
            vec![
                vec!["t".to_owned(), "Victoria vegetables 野菜\0".to_owned()],
                vec!["location".to_owned(), "Victoria, B.C., Canada".to_owned()],
            ],
            "Café-grown carrots 🥕 in Victoria\0",
        );
        let event_id = event.id_str().to_owned();
        let expected_tag_count =
            u64::try_from(event.tags_as_vec().len()).expect("tag count fits u64");
        let (expected_event_bytes, expected_tag_bytes) = raw_source_text_bytes(&event);

        store
            .ingest_event(RadrootsEventIngest::new(event, 1_000))
            .await
            .expect("non-ASCII and NUL ingest");
        let before_reopen = store
            .source_capacity_v1()
            .await
            .expect("capacity before reopen");
        assert_eq!(before_reopen.raw_event_count(), 1);
        assert_eq!(before_reopen.raw_tag_count(), expected_tag_count);
        assert_eq!(before_reopen.raw_event_text_bytes(), expected_event_bytes);
        assert_eq!(before_reopen.raw_tag_text_bytes(), expected_tag_bytes);
        store.pool().close().await;

        let reopened = RadrootsEventStore::open_file(&path)
            .await
            .expect("reopen UTF-8 file store");
        assert_eq!(
            reopened
                .source_capacity_v1()
                .await
                .expect("capacity after reopen"),
            before_reopen
        );
        let stored = reopened
            .raw_event(&event_id)
            .await
            .expect("raw event after reopen")
            .expect("stored raw event after reopen");
        assert_eq!(stored.content, "Café-grown carrots 🥕 in Victoria\0");
        let tags = reopened
            .tags_for_event(&event_id)
            .await
            .expect("stored tags after reopen");
        assert_eq!(tags.len(), 2);
        assert_eq!(
            tags[0].tag_value.as_deref(),
            Some("Victoria vegetables 野菜\0")
        );
    }

    #[tokio::test]
    async fn ingest_retains_raw_event_and_ignores_duplicate_rows() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let event = signed_event(
            KIND_POST,
            10,
            vec![vec!["t".to_owned(), "soil".to_owned()]],
            "hello",
        );
        let ingest = RadrootsEventIngest::new(event.clone(), 1_000);

        let first = store
            .ingest_event(ingest.clone())
            .await
            .expect("first ingest");
        let capacity_after_first = store
            .source_capacity_v1()
            .await
            .expect("capacity after first ingest");
        let second = store.ingest_event(ingest).await.expect("second ingest");
        let capacity_after_duplicate = store
            .source_capacity_v1()
            .await
            .expect("capacity after duplicate ingest");
        let stored = store
            .raw_event(event.id_str())
            .await
            .expect("get")
            .expect("stored");

        assert!(first.persistence.is_inserted());
        assert!(second.persistence.is_duplicate());
        assert_eq!(capacity_after_duplicate, capacity_after_first);
        assert_eq!(first.persistence.sequence(), second.persistence.sequence());
        assert_eq!(first.persistence.sequence(), Some(stored.seq));
        assert_eq!(
            second.raw_head_decision,
            RadrootsRawHeadDecision::NotHeadSelected
        );
        assert_eq!(
            first.admission_status,
            RadrootsEventAdmissionStatus::Admitted
        );
        assert_eq!(stored.raw_json, event.raw_json());
        assert_eq!(stored.content, "hello");
        assert_eq!(stored.tags_json, "[[\"t\",\"soil\"]]");
        assert_eq!(
            stored.admission_status,
            RadrootsEventAdmissionStatus::Admitted
        );
        assert!(stored.valid_stream_eligible);
        assert_eq!(
            store
                .tags_for_event(event.id_str())
                .await
                .expect("tags")
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn independent_file_pools_serialize_the_last_raw_event_byte_capacity_slot() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let path = tempdir.path().join("capacity-last-slot-race.sqlite");
        let contender_a = signed_event(KIND_POST, 101, Vec::new(), "race-a");
        let contender_b = signed_event(KIND_POST, 102, Vec::new(), "race-b");
        let contender_bytes = raw_source_text_bytes(&contender_a).0;
        assert_eq!(raw_source_text_bytes(&contender_b).0, contender_bytes);

        const FILLER_CREATED_AT_BASE: u32 = 1_000_000;
        let filler_base = signed_event(KIND_POST, FILLER_CREATED_AT_BASE, Vec::new(), "");
        let filler_base_bytes = raw_source_text_bytes(&filler_base).0;
        let filler_target = crate::RADROOTS_EVENT_STORE_RAW_EVENT_TEXT_BYTES_LIMIT_V1
            .checked_sub(contender_bytes)
            .expect("one contender fits the production byte limit");
        let full_content_len = DEFAULT_CONTENT_MAX_BYTES - 4_096;
        let full_content = "v".repeat(full_content_len);
        let full_filler = signed_event(
            KIND_POST,
            FILLER_CREATED_AT_BASE,
            Vec::new(),
            full_content.as_str(),
        );
        assert!(full_filler.raw_json().len() <= DEFAULT_RAW_JSON_MAX_BYTES);
        let full_filler_bytes = raw_source_text_bytes(&full_filler).0;
        let content_shape_for_target = |target: u64| {
            let adjustment = target.checked_sub(filler_base_bytes)?;
            let (ascii_len, append_nul) = if adjustment % 2 == 0 {
                (adjustment / 2, false)
            } else {
                (adjustment.checked_sub(7)? / 2, true)
            };
            (ascii_len <= u64::try_from(full_content_len).ok()?)
                .then_some((usize::try_from(ascii_len).ok()?, append_nul))
        };

        let mut full_filler_count = filler_target / full_filler_bytes;
        let mut tail_total = filler_target % full_filler_bytes;
        let tail_targets = if tail_total == 0 {
            Vec::new()
        } else if content_shape_for_target(tail_total).is_some() {
            vec![tail_total]
        } else {
            full_filler_count = full_filler_count
                .checked_sub(1)
                .expect("filler target admits a two-event exact tail");
            tail_total += full_filler_bytes;
            let lower = filler_base_bytes.max(tail_total - full_filler_bytes);
            let upper = full_filler_bytes.min(tail_total - filler_base_bytes);
            let first = (lower..=upper)
                .take(16)
                .find(|candidate| {
                    content_shape_for_target(*candidate).is_some()
                        && content_shape_for_target(tail_total - *candidate).is_some()
                })
                .expect("two bounded filler events can represent the exact tail");
            vec![first, tail_total - first]
        };

        let filler_store = RadrootsEventStore::open_file(&path)
            .await
            .expect("filler store");
        let mut filler_transaction = filler_store
            .begin_write_transaction()
            .await
            .expect("filler transaction");
        let mut filler_count = 0_u64;
        for index in 0..full_filler_count {
            let created_at = FILLER_CREATED_AT_BASE
                + u32::try_from(index).expect("bounded full filler index fits u32");
            let filler = signed_event(KIND_POST, created_at, Vec::new(), full_content.as_str());
            assert_eq!(raw_source_text_bytes(&filler).0, full_filler_bytes);
            filler_store
                .ingest_event_in_transaction(
                    &mut filler_transaction,
                    RadrootsEventIngest::new(filler, 1_000 + i64::from(created_at)),
                )
                .await
                .expect("coherent full filler ingest");
            filler_count += 1;
        }
        for target in tail_targets {
            let (ascii_len, append_nul) =
                content_shape_for_target(target).expect("validated exact tail shape");
            let mut content = "v".repeat(ascii_len);
            if append_nul {
                content.push('\0');
            }
            let created_at = FILLER_CREATED_AT_BASE
                + u32::try_from(filler_count).expect("bounded tail filler index fits u32");
            let filler = signed_event(KIND_POST, created_at, Vec::new(), content.as_str());
            assert!(filler.raw_json().len() <= DEFAULT_RAW_JSON_MAX_BYTES);
            assert_eq!(raw_source_text_bytes(&filler).0, target);
            filler_store
                .ingest_event_in_transaction(
                    &mut filler_transaction,
                    RadrootsEventIngest::new(filler, 1_000 + i64::from(created_at)),
                )
                .await
                .expect("coherent exact-tail filler ingest");
            filler_count += 1;
        }
        filler_transaction
            .commit()
            .await
            .expect("commit coherent filler source");
        let before_race = filler_store
            .source_capacity_v1()
            .await
            .expect("capacity before last-slot race");
        assert_eq!(before_race.raw_event_count(), filler_count);
        assert_eq!(before_race.raw_event_text_bytes(), filler_target);
        filler_store.pool().close().await;

        let contender_a_id = contender_a.id_str().to_owned();
        let contender_b_id = contender_b.id_str().to_owned();
        let store_a = RadrootsEventStore::open_file(&path)
            .await
            .expect("first independent store");
        let store_b = RadrootsEventStore::open_file(&path)
            .await
            .expect("second independent store");
        let barrier = Arc::new(tokio::sync::Barrier::new(3));
        let barrier_a = barrier.clone();
        let first = tokio::spawn(async move {
            barrier_a.wait().await;
            let result = store_a
                .ingest_event(RadrootsEventIngest::new(contender_a, 1_100))
                .await;
            (store_a, result)
        });
        let barrier_b = barrier.clone();
        let second = tokio::spawn(async move {
            barrier_b.wait().await;
            let result = store_b
                .ingest_event(RadrootsEventIngest::new(contender_b, 1_200))
                .await;
            (store_b, result)
        });
        barrier.wait().await;
        let (first, second) = tokio::join!(first, second);
        let (store_a, result_a) = first.expect("first contender task");
        let (store_b, result_b) = second.expect("second contender task");
        let (accepted, rejected) = match (result_a, result_b) {
            (Ok(accepted), Err(rejected)) | (Err(rejected), Ok(accepted)) => (accepted, rejected),
            _ => panic!("exactly one contender must consume the last capacity slot"),
        };
        assert!(accepted.persistence.is_inserted());
        assert!(matches!(
            rejected,
            RadrootsEventStoreError::SourceCapacityExceeded {
                resource: crate::RadrootsEventStoreSourceCapacityResourceV1::RawEventBytes,
                current: crate::RADROOTS_EVENT_STORE_RAW_EVENT_TEXT_BYTES_LIMIT_V1,
                requested,
                limit: crate::RADROOTS_EVENT_STORE_RAW_EVENT_TEXT_BYTES_LIMIT_V1,
            } if requested == contender_bytes
        ));
        store_a.pool().close().await;
        store_b.pool().close().await;

        let reopened = RadrootsEventStore::open_file(&path)
            .await
            .expect("clean full reopen after last-slot race");
        let after_race = reopened
            .source_capacity_v1()
            .await
            .expect("capacity after last-slot race");
        assert_eq!(after_race.raw_event_count(), filler_count + 1);
        assert_eq!(
            after_race.raw_event_text_bytes(),
            crate::RADROOTS_EVENT_STORE_RAW_EVENT_TEXT_BYTES_LIMIT_V1
        );
        let retained_contenders: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM event_envelopes WHERE event_id = ? OR event_id = ?",
        )
        .bind(contender_a_id)
        .bind(contender_b_id)
        .fetch_one(reopened.pool())
        .await
        .expect("retained contender count");
        assert_eq!(retained_contenders, 1);
    }

    #[tokio::test]
    async fn exact_capacity_boundary_allows_duplicate_observation_and_ephemeral_noop() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let retained = signed_event(KIND_POST, 20, Vec::new(), "retained at boundary");
        store
            .ingest_event(RadrootsEventIngest::new(retained.clone(), 1_000))
            .await
            .expect("initial durable ingest");

        let source_guard: String = sqlx::query_scalar(
            "SELECT sql FROM main.sqlite_schema WHERE type = 'trigger' AND name = 'radroots_event_store_source_state_authority_update_guard'",
        )
        .fetch_one(store.pool())
        .await
        .expect("source-state update guard SQL");
        let capacity_guard: String = sqlx::query_scalar(
            "SELECT sql FROM main.sqlite_schema WHERE type = 'trigger' AND name = 'radroots_event_store_source_capacity_update_guard'",
        )
        .fetch_one(store.pool())
        .await
        .expect("capacity update guard SQL");
        let mut transaction = store
            .begin_write_transaction()
            .await
            .expect("boundary fixture transaction");
        sqlx::query("DROP TRIGGER radroots_event_store_source_state_authority_update_guard")
            .execute(&mut *transaction)
            .await
            .expect("remove source-state guard in rolled-back fixture");
        sqlx::query("DROP TRIGGER radroots_event_store_source_capacity_update_guard")
            .execute(&mut *transaction)
            .await
            .expect("remove capacity guard in rolled-back fixture");
        sqlx::query(
            "UPDATE radroots_event_store_source_state SET raw_event_count = ? WHERE singleton = 1",
        )
        .bind(
            i64::try_from(crate::RADROOTS_EVENT_STORE_RAW_EVENT_COUNT_LIMIT_V1)
                .expect("production event-count limit fits SQLite"),
        )
        .execute(&mut *transaction)
        .await
        .expect("place source state at event-count boundary");
        sqlx::query(
            "UPDATE radroots_event_store_source_capacity_v1 SET raw_event_count = ? WHERE singleton = 1",
        )
        .bind(
            i64::try_from(crate::RADROOTS_EVENT_STORE_RAW_EVENT_COUNT_LIMIT_V1)
                .expect("production event-count limit fits SQLite"),
        )
        .execute(&mut *transaction)
        .await
        .expect("place capacity seal at event-count boundary");
        sqlx::raw_sql(sqlx::AssertSqlSafe(source_guard))
            .execute(&mut *transaction)
            .await
            .expect("restore exact source-state guard");
        sqlx::raw_sql(sqlx::AssertSqlSafe(capacity_guard))
            .execute(&mut *transaction)
            .await
            .expect("restore exact capacity guard");

        let at_limit = crate::source_maintenance_v1::validate_source_capacity_authority_fast_v1(
            &mut transaction,
        )
        .await
        .expect("fast capacity seal at exact limit");
        assert_eq!(
            at_limit.raw_event_count(),
            crate::RADROOTS_EVENT_STORE_RAW_EVENT_COUNT_LIMIT_V1
        );
        let duplicate_observation = RadrootsTransportObservation::new(
            RadrootsTransportKind::Nostr,
            "wss://capacity-boundary.example.test",
            RadrootsTransportObservationType::Subscription,
            1_100,
        )
        .expect("duplicate observation");
        let duplicate = store
            .ingest_event_in_transaction(
                &mut transaction,
                RadrootsEventIngest::new(retained.clone(), 1_100)
                    .with_observation(duplicate_observation),
            )
            .await
            .expect("duplicate does not charge capacity at the exact boundary");
        assert!(duplicate.persistence.is_duplicate());
        let observation_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM event_transport_observation WHERE event_id = ?",
        )
        .bind(retained.id_str())
        .fetch_one(&mut *transaction)
        .await
        .expect("duplicate observation count");
        assert_eq!(observation_count, 1);
        assert_eq!(
            crate::source_maintenance_v1::validate_source_capacity_authority_fast_v1(
                &mut transaction,
            )
            .await
            .expect("capacity after duplicate"),
            at_limit
        );

        let unique = signed_event(KIND_POST, 21, Vec::new(), "one event over boundary");
        assert!(matches!(
            store
                .ingest_event_in_transaction(
                    &mut transaction,
                    RadrootsEventIngest::new(unique.clone(), 1_200),
                )
                .await,
            Err(RadrootsEventStoreError::SourceCapacityExceeded {
                resource: crate::RadrootsEventStoreSourceCapacityResourceV1::RawEvents,
                current: crate::RADROOTS_EVENT_STORE_RAW_EVENT_COUNT_LIMIT_V1,
                requested: 1,
                limit: crate::RADROOTS_EVENT_STORE_RAW_EVENT_COUNT_LIMIT_V1,
            })
        ));
        let ephemeral = signed_event(KIND_GEOCHAT, 22, Vec::new(), "live-only boundary event");
        let ephemeral_observation = RadrootsTransportObservation::new(
            RadrootsTransportKind::Nostr,
            "wss://ephemeral-boundary.example.test",
            RadrootsTransportObservationType::Subscription,
            1_300,
        )
        .expect("ephemeral observation");
        let ephemeral_receipt = store
            .ingest_event_in_transaction(
                &mut transaction,
                RadrootsEventIngest::new(ephemeral.clone(), 1_300)
                    .with_observation(ephemeral_observation),
            )
            .await
            .expect("ephemeral event is not charged at the exact boundary");
        assert_eq!(
            ephemeral_receipt.persistence,
            RadrootsEventPersistence::NotPersisted
        );
        let ephemeral_observation_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM event_transport_observation WHERE event_id = ?",
        )
        .bind(ephemeral.id_str())
        .fetch_one(&mut *transaction)
        .await
        .expect("ephemeral observation count");
        assert_eq!(ephemeral_observation_count, 0);
        assert_eq!(
            crate::source_maintenance_v1::validate_source_capacity_authority_fast_v1(
                &mut transaction,
            )
            .await
            .expect("capacity after ephemeral event"),
            at_limit
        );
        transaction
            .rollback()
            .await
            .expect("roll back exact-bound fixture");
        assert!(
            store
                .raw_event(unique.id_str())
                .await
                .expect("unique raw event after rollback")
                .is_none()
        );
        assert!(
            store
                .raw_event(ephemeral.id_str())
                .await
                .expect("ephemeral raw event after rollback")
                .is_none()
        );
    }

    #[tokio::test]
    async fn duplicate_preserves_immutable_classification_and_first_raw_bytes() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let first_event = signed_event(
            KIND_POST,
            11,
            vec![vec!["t".to_owned(), "soil".to_owned()]],
            "same event",
        );
        let second_event = signed_event(
            KIND_POST,
            11,
            vec![vec!["t".to_owned(), "soil".to_owned()]],
            "same event",
        );
        assert_eq!(first_event.id_str(), second_event.id_str());
        assert_ne!(first_event.sig_str(), second_event.sig_str());
        assert_ne!(first_event.raw_json(), second_event.raw_json());

        store
            .ingest_event(RadrootsEventIngest::new(first_event.clone(), 1_100))
            .await
            .expect("first ingest");
        sqlx::query(
            "UPDATE event_envelopes SET contract_status = 'unsupported', contract_id = NULL, projection_eligible = 0 WHERE event_id = ?",
        )
        .bind(first_event.id_str())
        .execute(store.pool())
        .await
        .expect_err("derived classification mutation must be rejected");
        let before: (String, String, String, i64) = sqlx::query_as(
            "SELECT sig, raw_json, tags_json, updated_at_ms FROM event_envelopes WHERE event_id = ?",
        )
        .bind(first_event.id_str())
        .fetch_one(store.pool())
        .await
        .expect("before");
        let capacity_before_duplicate = store
            .source_capacity_v1()
            .await
            .expect("capacity before alternate-encoding duplicate");

        let receipt = store
            .ingest_event(RadrootsEventIngest::new(second_event, 1_200))
            .await
            .expect("duplicate ingest");
        let after: (String, String, String, i64) = sqlx::query_as(
            "SELECT sig, raw_json, tags_json, updated_at_ms FROM event_envelopes WHERE event_id = ?",
        )
        .bind(first_event.id_str())
        .fetch_one(store.pool())
        .await
        .expect("after");
        let capacity_after_duplicate = store
            .source_capacity_v1()
            .await
            .expect("capacity after alternate-encoding duplicate");

        assert!(receipt.persistence.is_duplicate());
        assert_eq!(capacity_after_duplicate, capacity_before_duplicate);
        assert_eq!(
            receipt.admission_status,
            RadrootsEventAdmissionStatus::Admitted
        );
        assert_eq!(receipt.admission_code, None);
        assert_eq!(
            receipt.contract_id.as_deref(),
            Some("radroots.social.update.v1")
        );
        assert!(receipt.valid_stream_eligible);
        assert_eq!(
            receipt.raw_head_decision,
            RadrootsRawHeadDecision::NotHeadSelected
        );
        assert_eq!(after, before);
        assert_eq!(after.0, first_event.sig_str());
        assert_eq!(after.1, first_event.raw_json());
        assert_eq!(
            store
                .raw_event(first_event.id_str())
                .await
                .expect("raw event")
                .expect("stored")
                .admission_status,
            RadrootsEventAdmissionStatus::Admitted
        );
        assert!(
            store
                .valid_event(first_event.id_str())
                .await
                .expect("valid event")
                .is_some()
        );
    }

    #[tokio::test]
    async fn database_guards_reject_reintroducing_legacy_classification() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let first_event = signed_event(KIND_POST, 12, Vec::new(), "legacy");
        let second_event = signed_event(KIND_POST, 12, Vec::new(), "legacy");
        assert_eq!(first_event.id_str(), second_event.id_str());
        assert_ne!(first_event.sig_str(), second_event.sig_str());

        store
            .ingest_event(RadrootsEventIngest::new(first_event.clone(), 1_300))
            .await
            .expect("first ingest");
        sqlx::query(
            "UPDATE event_envelopes SET contract_status = 'supported', event_class = NULL WHERE event_id = ?",
        )
        .bind(first_event.id_str())
        .execute(store.pool())
        .await
        .expect_err("legacy classification mutation must be rejected");
        let before: (String, String, String, String, Option<String>, i64) = sqlx::query_as(
            "SELECT sig, raw_json, tags_json, contract_status, event_class, updated_at_ms FROM event_envelopes WHERE event_id = ?",
        )
        .bind(first_event.id_str())
        .fetch_one(store.pool())
        .await
        .expect("before");

        let receipt = store
            .ingest_event(RadrootsEventIngest::new(second_event, 1_400))
            .await
            .expect("duplicate");
        let after: (String, String, String, String, Option<String>, i64) = sqlx::query_as(
            "SELECT sig, raw_json, tags_json, contract_status, event_class, updated_at_ms FROM event_envelopes WHERE event_id = ?",
        )
        .bind(first_event.id_str())
        .fetch_one(store.pool())
        .await
        .expect("after");

        assert!(receipt.persistence.is_duplicate());
        assert_eq!(
            receipt.admission_status,
            RadrootsEventAdmissionStatus::Admitted
        );
        assert_eq!(after, before);
        assert_eq!(after.0, first_event.sig_str());
        assert_eq!(after.1, first_event.raw_json());
        assert!(
            store
                .raw_event(first_event.id_str())
                .await
                .expect("raw event")
                .is_some()
        );
        assert!(
            store
                .valid_event(first_event.id_str())
                .await
                .expect("valid event")
                .is_some()
        );
        assert_eq!(
            store
                .event_visibility(first_event.id_str())
                .await
                .expect("visibility"),
            Some(RadrootsEventVisibility::Visible)
        );
        let status = store.status_summary().await.expect("legacy status");
        let inspected = inspect_event_store_status(store.pool())
            .await
            .expect("legacy pool status");
        assert_eq!(inspected, status);
        assert_eq!(status.total_events, 1);
        assert_eq!(status.valid_stream_events, 1);
    }

    #[tokio::test]
    async fn trade_mutation_ingest_stores_semantic_rows_missing_parents_and_reservations() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let proposal = canonical_trade_mutation_content(proposal_envelope()).expect("proposal");
        let decision =
            canonical_trade_mutation_content(decision_envelope(&proposal)).expect("decision");
        let decision_event = signed_trade_mutation(&decision);

        let decision_receipt = store
            .ingest_event(RadrootsEventIngest::new(decision_event.clone(), 2_000))
            .await
            .expect("decision ingest");
        assert!(decision_receipt.valid_stream_eligible);

        let stored_decision = store
            .get_trade_mutation(&decision.mutation_id)
            .await
            .expect("decision query")
            .expect("decision mutation");
        assert_eq!(stored_decision.mutation_id, decision.mutation_id);
        assert_eq!(stored_decision.trade_id, trade_id());
        assert_eq!(
            stored_decision.canonical_payload_bytes,
            decision.content.as_bytes()
        );
        assert_eq!(
            stored_decision.payload_sha256,
            sha256_hex(decision.content.as_bytes())
        );
        assert_eq!(
            stored_decision.first_transport_event_id.as_str(),
            decision_event.id_str()
        );
        assert_eq!(
            stored_decision.mutation_kind,
            RadrootsTradeMutationKindV1::Decision
        );

        let missing = store
            .missing_trade_parents(&trade_id())
            .await
            .expect("missing parents");
        assert_eq!(missing.len(), 1);
        assert_eq!(missing[0].mutation_id, decision.mutation_id);
        assert_eq!(missing[0].missing_parent_mutation_id, proposal.mutation_id);

        let reservation_id = RadrootsDTag::parse("reservation-1").expect("reservation id");
        let reservation = store
            .seller_reservation(&reservation_id)
            .await
            .expect("reservation query")
            .expect("reservation");
        assert_eq!(reservation.claim_mutation_id, decision.mutation_id);
        assert_eq!(reservation.trade_id, trade_id());
        assert_eq!(reservation.assertion_commitment, event_id('e'));
        let lines = store
            .seller_reservation_lines(&reservation_id)
            .await
            .expect("reservation lines");
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].bin_id.as_str(), "bin-1");

        let proposal_event = signed_trade_mutation(&proposal);
        store
            .ingest_event(RadrootsEventIngest::new(proposal_event, 2_100))
            .await
            .expect("proposal ingest");
        let missing = store
            .missing_trade_parents(&trade_id())
            .await
            .expect("missing parents resolved");
        assert!(missing.is_empty());
        let parents = store
            .trade_mutation_parents(&decision.mutation_id)
            .await
            .expect("parents");
        assert_eq!(parents.len(), 1);
        assert_eq!(parents[0].parent_mutation_id, proposal.mutation_id);
        let transport_envelopes = store
            .trade_transport_envelopes_for_mutation(&decision.mutation_id)
            .await
            .expect("transport envelopes");
        assert_eq!(transport_envelopes.len(), 1);
        assert_eq!(transport_envelopes[0].transport_kind, "nostr");
    }

    #[tokio::test]
    async fn borrowed_ingest_savepoint_rolls_back_post_core_authority_forge() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let prior_event = signed_event(
            KIND_POST,
            1_799_123_455,
            Vec::new(),
            "caller transaction prior event",
        );
        let mut proposal_envelope = proposal_envelope();
        proposal_envelope.authored_at_unix_s = 1_799_123_456;
        let proposal =
            canonical_trade_mutation_content(proposal_envelope).expect("unique proposal");
        let trigger_event = signed_trade_mutation(&proposal);
        let forged_event = signed_event(
            KIND_POST,
            1_799_123_457,
            Vec::new(),
            "post-core forged raw authority",
        );
        register_protocol_post_extension_raw_authority_forge(
            trigger_event.id_str().to_owned(),
            RadrootsEventIngest::new(forged_event.clone(), 2_251),
        );

        let mut tx = store
            .begin_write_transaction()
            .await
            .expect("caller transaction");
        store
            .ingest_event_in_transaction(
                &mut tx,
                RadrootsEventIngest::new(prior_event.clone(), 2_249),
            )
            .await
            .expect("prior caller work");
        let capacity_after_prior =
            crate::source_maintenance_v1::validate_source_capacity_authority_fast_v1(&mut tx)
                .await
                .expect("capacity after prior caller work");
        let error = store
            .ingest_event_in_transaction(
                &mut tx,
                RadrootsEventIngest::new(trigger_event.clone(), 2_250),
            )
            .await
            .expect_err("post-core raw authority mutation must fail");
        assert!(matches!(
            error,
            RadrootsEventStoreError::MigrationHookStateDrift { ref reason, .. }
                if reason.contains("post-core extensions changed protocol-owned authority")
        ));
        let capacity_after_rollback =
            crate::source_maintenance_v1::validate_source_capacity_authority_fast_v1(&mut tx)
                .await
                .expect("capacity after failed nested ingest");
        assert_eq!(capacity_after_rollback, capacity_after_prior);
        tx.commit()
            .await
            .expect("caller may commit prior work after failed ingest");

        assert!(
            store
                .raw_event(prior_event.id_str())
                .await
                .expect("prior raw event")
                .is_some()
        );
        assert!(
            store
                .raw_event(trigger_event.id_str())
                .await
                .expect("trigger raw event")
                .is_none()
        );
        assert!(
            store
                .raw_event(forged_event.id_str())
                .await
                .expect("forged raw event")
                .is_none()
        );
        let source_authority =
            sqlx::query("SELECT raw_event_count, raw_tag_count, raw_high_water_seq, last_transition_seq FROM radroots_event_store_source_state WHERE singleton = 1")
                .fetch_one(store.pool())
                .await
                .expect("source authority after rollback");
        assert_eq!(
            source_authority
                .try_get::<i64, _>("raw_event_count")
                .unwrap(),
            1
        );
        assert_eq!(
            source_authority.try_get::<i64, _>("raw_tag_count").unwrap(),
            0
        );
        assert_eq!(
            source_authority
                .try_get::<i64, _>("raw_high_water_seq")
                .unwrap(),
            1
        );
        assert_eq!(
            source_authority
                .try_get::<i64, _>("last_transition_seq")
                .unwrap(),
            0
        );
        let trade_mutation_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM trade_mutation")
            .fetch_one(store.pool())
            .await
            .expect("trade mutation count");
        let transition_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM radroots_event_store_addressable_head_transition",
        )
        .fetch_one(store.pool())
        .await
        .expect("transition count");
        assert_eq!(trade_mutation_count, 0);
        assert_eq!(transition_count, 0);
        assert_eq!(
            store
                .source_capacity_v1()
                .await
                .expect("committed capacity after rollback"),
            capacity_after_prior
        );
    }

    #[tokio::test]
    async fn trade_projection_rejects_authored_timestamps_outside_sqlite_range() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let canonical =
            canonical_trade_mutation_content(proposal_envelope()).expect("canonical proposal");
        let signed = signed_trade_mutation(&canonical);
        let ingest = RadrootsEventIngest::new(signed, 2_250);
        let mut mutation = canonical.envelope.clone();
        mutation.authored_at_unix_s = u64::MAX;
        let mut transaction = store
            .begin_write_transaction()
            .await
            .expect("write transaction");
        let mut storage = PostCoreStorageV1::new(&mut transaction);
        let candidate_id = candidate_id_for_mutation(&mutation);
        let proposal_mutation_id = proposal_mutation_id_for_mutation(&mutation);
        let target_claim_mutation_id = target_claim_mutation_id_for_mutation(&mutation);
        let write = TradeProjectionWrite::new(
            ingest.event(),
            1,
            &mutation,
            &canonical.mutation_id,
            candidate_id.as_ref(),
            proposal_mutation_id.as_ref(),
            target_claim_mutation_id.as_ref(),
            "fixture-sha256",
            ingest.observed_at_ms(),
            seller_reservation_for_mutation(&mutation),
        );

        assert!(matches!(
            storage.persist_trade_projection(write).await,
            Err(RadrootsEventStoreError::UnsignedIntegerRange {
                field: "authored_at_unix_s",
                value: u64::MAX,
            })
        ));
    }

    #[tokio::test]
    async fn ingest_rejects_governed_temp_shadow_before_owned_or_borrowed_mutation() {
        let owned = RadrootsEventStore::open_memory()
            .await
            .expect("owned store");
        let mut connection = owned.pool().acquire().await.expect("connection");
        sqlx::query("CREATE TEMP TABLE \"EVENT_ENVELOPES\" (event_id TEXT)")
            .execute(&mut *connection)
            .await
            .expect("owned temporary collision");
        drop(connection);
        assert!(matches!(
            owned
                .ingest_event(RadrootsEventIngest::new(
                    signed_event(KIND_POST, 50, Vec::new(), "owned"),
                    1_000,
                ))
                .await,
            Err(RadrootsEventStoreError::TemporarySchemaCollision {
                name,
                ..
            }) if name == "EVENT_ENVELOPES"
        ));
        let owned_main_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM main.event_envelopes")
            .fetch_one(owned.pool())
            .await
            .expect("owned main count");
        assert_eq!(owned_main_count, 0);

        let borrowed = RadrootsEventStore::open_memory()
            .await
            .expect("borrowed store");
        let mut transaction = borrowed
            .begin_write_transaction()
            .await
            .expect("borrowed transaction");
        sqlx::query("CREATE TEMP TABLE \"EvEnT_EnVeLoPe_TaGs\" (event_id TEXT)")
            .execute(&mut *transaction)
            .await
            .expect("borrowed temporary collision");
        assert!(matches!(
            borrowed
                .ingest_event_in_transaction(
                    &mut transaction,
                    RadrootsEventIngest::new(
                        signed_event(KIND_POST, 51, Vec::new(), "borrowed"),
                        1_001,
                    ),
                )
                .await,
            Err(RadrootsEventStoreError::TemporarySchemaCollision {
                name,
                ..
            }) if name == "EvEnT_EnVeLoPe_TaGs"
        ));
        let borrowed_main_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM main.event_envelopes")
                .fetch_one(&mut *transaction)
                .await
                .expect("borrowed main count");
        assert_eq!(borrowed_main_count, 0);
        transaction.rollback().await.expect("borrowed rollback");
    }

    #[tokio::test]
    async fn post_core_schema_change_is_detected_and_rolled_back() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let mut proposal_envelope = proposal_envelope();
        proposal_envelope.authored_at_unix_s = 1_799_123_458;
        let proposal =
            canonical_trade_mutation_content(proposal_envelope).expect("unique proposal");
        let trigger_event = signed_trade_mutation(&proposal);
        register_protocol_post_extension_schema_forge(trigger_event.id_str().to_owned());

        let error = store
            .ingest_event(RadrootsEventIngest::new(trigger_event.clone(), 2_252))
            .await
            .expect_err("post-core schema mutation must fail");
        assert!(matches!(
            error,
            RadrootsEventStoreError::MigrationHookStateDrift { ref reason, .. }
                if reason.contains("post-core extensions changed protocol-owned authority")
        ));
        assert!(
            store
                .raw_event(trigger_event.id_str())
                .await
                .expect("trigger raw event")
                .is_none()
        );
        let forged_table_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_schema WHERE name = 'radroots_event_store_post_extension_schema_forge'",
        )
        .fetch_one(store.pool())
        .await
        .expect("forged table count");
        assert_eq!(forged_table_count, 0);
    }

    #[tokio::test]
    async fn public_pool_transaction_and_trade_query_apis_roundtrip() {
        let options = SqliteConnectOptions::from_str("sqlite::memory:").expect("options");
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .expect("pool");
        let store = RadrootsEventStore::open_pool(pool, false)
            .await
            .expect("store");
        let proposal = canonical_trade_mutation_content(proposal_envelope()).expect("proposal");
        let proposal_event = signed_trade_mutation(&proposal);
        let ingest =
            RadrootsEventIngest::from_raw_json(proposal_event.raw_json().to_owned(), 2_200)
                .expect("raw ingest");
        let mut tx = store.pool().begin().await.expect("transaction");
        let receipt = store
            .ingest_event_in_transaction(&mut tx, ingest)
            .await
            .expect("transactional ingest");
        tx.commit().await.expect("commit");
        assert!(receipt.valid_stream_eligible);

        assert!(matches!(
            store.trade_mutations_for_trade(&trade_id(), 0).await,
            Err(RadrootsEventStoreError::QueryLimitOutOfRange { .. })
        ));
        assert!(matches!(
            store
                .trade_mutations_for_trade(&trade_id(), RADROOTS_EVENT_STORE_QUERY_LIMIT_MAX + 1,)
                .await,
            Err(RadrootsEventStoreError::QueryLimitOutOfRange { .. })
        ));
        let mutations = store
            .trade_mutations_for_trade(&trade_id(), 10)
            .await
            .expect("mutations");
        assert_eq!(mutations.len(), 1);
        assert_eq!(mutations[0].mutation_id, proposal.mutation_id);

        let decision =
            canonical_trade_mutation_content(decision_envelope(&proposal)).expect("decision");
        store
            .ingest_event(RadrootsEventIngest::new(
                signed_trade_mutation(&decision),
                2_300,
            ))
            .await
            .expect("decision ingest");
        assert!(
            store
                .missing_trade_parents(&trade_id())
                .await
                .expect("missing parents")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn contract_admitted_but_malformed_trade_mutations_are_quarantined() {
        let store = RadrootsEventStore::open_memory().await.expect("store");
        let proposal = canonical_trade_mutation_content(proposal_envelope()).expect("proposal");

        let malformed = signed_trade_content_with_keys(
            &proposal,
            format!("{} ", proposal.content),
            &fixture_keys(),
        );
        let malformed_id = malformed.id_str().to_owned();
        let malformed_receipt = store
            .ingest_event(RadrootsEventIngest::new(malformed, 2_400))
            .await
            .expect("malformed ingest");
        assert_eq!(
            malformed_receipt.admission_status,
            RadrootsEventAdmissionStatus::Admitted
        );
        assert!(malformed_receipt.valid_stream_eligible);
        assert_eq!(
            malformed_receipt.raw_head_decision,
            RadrootsRawHeadDecision::NotHeadSelected
        );
        assert!(store.raw_event(&malformed_id).await.expect("raw").is_some());
        assert!(
            store
                .valid_event(&malformed_id)
                .await
                .expect("valid")
                .is_some()
        );

        let mut missing_id_value: serde_json::Value =
            serde_json::from_str(&proposal.content).expect("proposal json");
        missing_id_value
            .as_object_mut()
            .expect("proposal object")
            .remove("mutation_id");
        let missing_id_content = canonical_jcs_value(&missing_id_value).expect("canonical json");
        let missing_id =
            signed_trade_content_with_keys(&proposal, missing_id_content, &fixture_keys());
        let missing_id_event_id = missing_id.id_str().to_owned();
        let missing_id_receipt = store
            .ingest_event(RadrootsEventIngest::new(missing_id, 2_500))
            .await
            .expect("missing id ingest");
        assert_eq!(
            missing_id_receipt.admission_status,
            RadrootsEventAdmissionStatus::Admitted
        );
        assert!(missing_id_receipt.valid_stream_eligible);
        assert!(
            store
                .valid_event(&missing_id_event_id)
                .await
                .expect("valid")
                .is_some()
        );

        let mismatched_author =
            signed_trade_content_with_keys(&proposal, proposal.content.clone(), &alternate_keys());
        let mismatched_author_id = mismatched_author.id_str().to_owned();
        let mismatched_author_receipt = store
            .ingest_event(RadrootsEventIngest::new(mismatched_author, 2_600))
            .await
            .expect("mismatched author ingest");
        assert_eq!(
            mismatched_author_receipt.admission_status,
            RadrootsEventAdmissionStatus::Admitted
        );
        assert!(mismatched_author_receipt.valid_stream_eligible);
        assert!(
            store
                .valid_event(&mismatched_author_id)
                .await
                .expect("valid")
                .is_some()
        );

        let quarantined: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM trade_projection_quarantine")
                .fetch_one(store.pool())
                .await
                .expect("quarantine count");
        assert_eq!(quarantined, 3);
        let projected: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM trade_mutation")
            .fetch_one(store.pool())
            .await
            .expect("projected count");
        assert_eq!(projected, 0);
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn trade_storage_helpers_cover_every_mutation_variant() {
        let proposal = canonical_trade_mutation_content(proposal_envelope()).expect("proposal");
        let decision =
            canonical_trade_mutation_content(decision_envelope(&proposal)).expect("decision");
        let RadrootsTradeMutationBodyV1::Proposal { candidate } = &proposal.envelope.body else {
            unreachable!("proposal fixture");
        };
        let RadrootsTradeMutationBodyV1::Decision {
            proposal_mutation_id,
            candidate_id,
            decision: decision_value,
        } = &decision.envelope.body
        else {
            unreachable!("decision fixture");
        };

        let mut revision_proposal = proposal.envelope.clone();
        revision_proposal.body = RadrootsTradeMutationBodyV1::RevisionProposal {
            candidate: candidate.clone(),
        };
        let mut revision_decision = decision.envelope.clone();
        revision_decision.body = RadrootsTradeMutationBodyV1::RevisionDecision {
            proposal_mutation_id: proposal_mutation_id.clone(),
            candidate_id: candidate_id.clone(),
            decision: decision_value.clone(),
        };
        let mut cancellation = proposal.envelope.clone();
        cancellation.body = RadrootsTradeMutationBodyV1::Cancellation {
            target_candidate_id: Some(candidate_id.clone()),
            target_claim_mutation_id: Some(decision.mutation_id.clone()),
            reason: "fixture".to_owned(),
        };
        let mut claim_only_cancellation = cancellation.clone();
        claim_only_cancellation.body = RadrootsTradeMutationBodyV1::Cancellation {
            target_candidate_id: None,
            target_claim_mutation_id: Some(decision.mutation_id.clone()),
            reason: "fixture".to_owned(),
        };

        assert_eq!(
            candidate_id_for_mutation(&revision_proposal),
            Some(candidate_id.clone())
        );
        assert_eq!(
            candidate_id_for_mutation(&revision_decision),
            Some(candidate_id.clone())
        );
        assert_eq!(
            candidate_id_for_mutation(&cancellation),
            Some(candidate_id.clone())
        );
        assert_eq!(candidate_id_for_mutation(&claim_only_cancellation), None);
        assert_eq!(
            proposal_mutation_id_for_mutation(&revision_decision),
            Some(proposal_mutation_id.clone())
        );
        assert_eq!(
            target_claim_mutation_id_for_mutation(&cancellation),
            Some(decision.mutation_id.clone())
        );
        assert!(seller_reservation_for_mutation(&revision_decision).is_some());
        assert!(seller_reservation_for_mutation(&cancellation).is_none());
        assert_eq!(public_key('b').as_str(), event_id('b'));

        for kind in [
            RadrootsTradeMutationKindV1::Proposal,
            RadrootsTradeMutationKindV1::Decision,
            RadrootsTradeMutationKindV1::RevisionProposal,
            RadrootsTradeMutationKindV1::RevisionDecision,
            RadrootsTradeMutationKindV1::Cancellation,
        ] {
            let stored = trade_mutation_kind_storage_value(kind);
            assert_eq!(
                parse_trade_mutation_kind(stored).expect("stored kind"),
                kind
            );
        }
        assert!(parse_trade_mutation_kind("bad").is_err());
    }

    #[tokio::test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    async fn seller_reservation_rejects_unrepresentable_storage_times() {
        let store = RadrootsEventStore::open_memory().await.expect("store");
        let source_authority = source_authority_snapshot(&store).await;
        let proposal = canonical_trade_mutation_content(proposal_envelope()).expect("proposal");

        let mut invalid_epoch_envelope = decision_envelope(&proposal);
        let RadrootsTradeMutationBodyV1::Decision {
            decision:
                RadrootsTradeDecisionV1::Accepted {
                    reservation_assertion: Some(invalid_epoch),
                },
            ..
        } = &mut invalid_epoch_envelope.body
        else {
            unreachable!("accepted decision fixture");
        };
        invalid_epoch.inventory_epoch = u64::MAX;
        let invalid_epoch =
            canonical_trade_mutation_content(invalid_epoch_envelope).expect("invalid epoch");
        assert!(matches!(
            store
                .ingest_event(RadrootsEventIngest::new(
                    signed_trade_mutation(&invalid_epoch),
                    1,
                ))
                .await,
            Err(RadrootsEventStoreError::UnsignedIntegerRange {
                field: "inventory_epoch",
                ..
            })
        ));
        assert_no_event_or_trade_residue(&store, &source_authority).await;

        let mut invalid_expiry_envelope = decision_envelope(&proposal);
        let RadrootsTradeMutationBodyV1::Decision {
            decision:
                RadrootsTradeDecisionV1::Accepted {
                    reservation_assertion: Some(invalid_expiry),
                },
            ..
        } = &mut invalid_expiry_envelope.body
        else {
            unreachable!("accepted decision fixture");
        };
        invalid_expiry.reservation_expires_at_unix_s = u64::MAX;
        let invalid_expiry =
            canonical_trade_mutation_content(invalid_expiry_envelope).expect("invalid expiry");
        assert!(matches!(
            store
                .ingest_event(RadrootsEventIngest::new(
                    signed_trade_mutation(&invalid_expiry),
                    2,
                ))
                .await,
            Err(RadrootsEventStoreError::UnsignedIntegerRange {
                field: "reservation_expires_at_unix_s",
                ..
            })
        ));
        assert_no_event_or_trade_residue(&store, &source_authority).await;
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn usize_storage_conversion_covers_success_and_overflow() {
        assert_eq!(i64_from_usize("index", 1).expect("index"), 1);
        assert!(matches!(
            i64_from_usize("index", usize::MAX),
            Err(RadrootsEventStoreError::UnsignedIntegerRange { field: "index", .. })
        ));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn projection_scalar_helpers_reject_every_unrepresentable_boundary() {
        assert!(matches!(
            validate_projection_identity("", 1),
            Err(RadrootsEventStoreError::InvalidProjectionId)
        ));
        assert!(matches!(
            validate_projection_identity("projection", 0),
            Err(RadrootsEventStoreError::InvalidProjectionVersion {
                projection_id,
                value: 0,
            }) if projection_id == "projection"
        ));

        assert_eq!(
            projection_version_from_i64("projection", 1).expect("version"),
            1
        );
        for value in [-1, i64::from(u32::MAX) + 1] {
            assert!(matches!(
                projection_version_from_i64("projection", value),
                Err(RadrootsEventStoreError::InvalidProjectionVersion {
                    projection_id,
                    value: actual,
                }) if projection_id == "projection" && actual == value
            ));
        }
        assert!(matches!(
            projection_version_from_i64("projection", 0),
            Err(RadrootsEventStoreError::InvalidProjectionVersion {
                projection_id,
                value: 0,
            }) if projection_id == "projection"
        ));

        assert_eq!(
            projection_source_revision_from_i64("projection", Some(1)).expect("revision"),
            1
        );
        for value in [None, Some(-1), Some(0), Some(i64::MAX)] {
            assert!(matches!(
                projection_source_revision_from_i64("projection", value),
                Err(RadrootsEventStoreError::InvalidProjectionSourceRevision {
                    projection_id,
                    value: actual,
                }) if projection_id == "projection" && actual == value
            ));
        }
        assert_eq!(
            projection_source_revision_to_i64("projection", 1).expect("stored revision"),
            1
        );
        assert!(matches!(
            projection_source_revision_to_i64("projection", u64::MAX),
            Err(RadrootsEventStoreError::InvalidProjectionSourceRevision {
                projection_id,
                value: None,
            }) if projection_id == "projection"
        ));

        ensure_projection_rebuild_row_changed("projection", 1).expect("one changed row");
        for rows_affected in [0, 2] {
            assert!(matches!(
                ensure_projection_rebuild_row_changed("projection", rows_affected),
                Err(RadrootsEventStoreError::ProjectionRebuildTicketConflict {
                    projection_id,
                }) if projection_id == "projection"
            ));
        }

        assert!(!bool_from_i64("flag", 0).expect("false"));
        assert!(bool_from_i64("flag", 1).expect("true"));
        assert!(matches!(
            bool_from_i64("flag", 2),
            Err(RadrootsEventStoreError::InvalidStoredBoolean {
                field: "flag",
                value: 2,
            })
        ));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn visibility_collection_helpers_preserve_typed_integrity_failures() {
        let missing_id = RadrootsEventId::parse(event_id('a')).expect("event id");
        let error = collect_event_visibilities(
            vec![missing_id.clone()],
            &BTreeMap::<RadrootsEventId, Option<RadrootsEventVisibility>>::new(),
        )
        .expect_err("missing evaluated visibility");
        assert!(matches!(
            error,
            RadrootsEventStoreError::CurrentVisibilityDrift { reason }
                if reason.contains(missing_id.as_str())
        ));

        let raw_head = RadrootsStoredRawEventHead {
            coordinate_type: crate::model::StoredEventClass::Replaceable,
            kind: KIND_PROFILE,
            pubkey: FIXTURE_ALICE_PUBLIC_KEY_HEX.to_owned(),
            d_tag: None,
            event_id: missing_id.to_string(),
            created_at: 1,
            updated_at_ms: 2,
        };
        assert!(matches!(
            require_raw_head_visibility(&raw_head, None),
            Err(RadrootsEventStoreError::StoredHeadInconsistent { event_id })
                if event_id == raw_head.event_id
        ));
    }

    #[test]
    fn ingest_rollback_failure_preserves_the_primary_error() {
        let result: Result<(), _> = preserve_ingest_primary_failure(
            RadrootsEventStoreError::MissingEvent("primary".to_owned()),
            Err(sqlx::Error::PoolClosed),
        );
        assert!(matches!(
            result,
            Err(RadrootsEventStoreError::IngestTransactionRollbackFailed {
                primary,
                rollback: sqlx::Error::PoolClosed,
            }) if matches!(
                primary.as_ref(),
                RadrootsEventStoreError::MissingEvent(event_id) if event_id == "primary"
            )
        ));
    }

    #[test]
    #[should_panic(expected = "proposal")]
    fn decision_fixture_rejects_a_non_proposal() {
        let proposal = canonical_trade_mutation_content(proposal_envelope()).expect("proposal");
        let decision =
            canonical_trade_mutation_content(decision_envelope(&proposal)).expect("decision");
        let _ = decision_envelope(&decision);
    }

    #[tokio::test]
    async fn wrapper_json_is_rejected_as_event_authority() {
        let event = signed_event(KIND_POST, 10, Vec::new(), "hello");
        let wrapper_json = serde_json::to_string(&event).expect("wrapper json");

        let error = RadrootsEventIngest::from_raw_json(wrapper_json, 1_000)
            .expect_err("wrapper json should not parse as event wire");

        assert!(matches!(error, RadrootsEventStoreError::EventWire(_)));
    }

    #[tokio::test]
    async fn unsupported_verified_events_are_stored_but_not_projected() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let event = signed_event(999, 11, Vec::new(), "unsupported");
        let capacity_before = store
            .source_capacity_v1()
            .await
            .expect("capacity before unsupported ingest");
        let receipt = store
            .ingest_event(RadrootsEventIngest::new(event.clone(), 2_000))
            .await
            .expect("ingest");
        let stored = store
            .raw_event(event.id_str())
            .await
            .expect("get")
            .expect("stored");
        let capacity_after_insert = store
            .source_capacity_v1()
            .await
            .expect("capacity after unsupported ingest");

        assert_eq!(
            receipt.admission_status,
            RadrootsEventAdmissionStatus::Unsupported
        );
        assert_eq!(receipt.admission_code.as_deref(), Some("unsupported_kind"));
        assert_eq!(
            stored.admission_status,
            RadrootsEventAdmissionStatus::Unsupported
        );
        assert!(!stored.valid_stream_eligible);
        assert_eq!(
            capacity_after_insert.raw_event_count(),
            capacity_before.raw_event_count() + 1
        );
        assert_eq!(
            capacity_after_insert.raw_tag_count(),
            capacity_before.raw_tag_count()
        );
        assert!(
            capacity_after_insert.raw_event_text_bytes() > capacity_before.raw_event_text_bytes()
        );
        assert_eq!(
            capacity_after_insert.raw_tag_text_bytes(),
            capacity_before.raw_tag_text_bytes()
        );
        assert!(
            store
                .valid_event(event.id_str())
                .await
                .expect("valid event")
                .is_none()
        );

        let duplicate = store
            .ingest_event(RadrootsEventIngest::new(event, 2_100))
            .await
            .expect("duplicate");
        let capacity_after_duplicate = store
            .source_capacity_v1()
            .await
            .expect("capacity after unsupported duplicate");
        assert!(duplicate.persistence.is_duplicate());
        assert_eq!(capacity_after_duplicate, capacity_after_insert);
        assert_eq!(
            duplicate.raw_head_decision,
            RadrootsRawHeadDecision::NotHeadSelected
        );
        assert_eq!(
            duplicate.admission_status,
            RadrootsEventAdmissionStatus::Unsupported
        );
        assert_eq!(duplicate.admission_code, receipt.admission_code);
    }

    #[test]
    fn test_helpers_cover_signature_and_non_head_branches() {
        let zero_sig = synthetic_signed_event(KIND_POST, 12, Vec::new(), "zero");
        let zero_sig = tamper_signature(&zero_sig);
        assert!(zero_sig.sig_str().starts_with('0'));

        let nonzero_sig = tamper_signature(&signed_event(KIND_POST, 12, Vec::new(), "nonzero"));
        assert_ne!(
            nonzero_sig.sig_str(),
            signed_event(KIND_POST, 12, Vec::new(), "nonzero").sig_str()
        );
    }

    #[test]
    #[should_panic(expected = "event should select a head")]
    fn head_coordinate_helper_panics_for_regular_events() {
        let event = signed_event(KIND_POST, 12, Vec::new(), "regular");
        let _ = head_coordinate_for_event(&event);
    }

    #[tokio::test]
    async fn id_mismatch_raw_json_is_rejected_before_storage() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let event = signed_event(KIND_POST, 12, Vec::new(), "hello");
        let raw_json = tampered_content_raw_json(&event, "tampered");

        let error = RadrootsEventIngest::from_raw_json(raw_json, 2_100).expect_err("id mismatch");

        assert!(matches!(error, RadrootsEventStoreError::EventWire(_)));
        assert!(
            store
                .valid_stream_after(0, 10)
                .await
                .expect("events")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn signature_invalid_events_are_rejected_before_storage() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let event = tamper_signature(&signed_event(KIND_POST, 13, Vec::new(), "hello"));
        let error = RadrootsEventIngest::from_signed_event(event.clone(), 2_200)
            .expect_err("invalid signature");

        assert!(matches!(
            error,
            RadrootsEventStoreError::Nip01Verification(
                radroots_event_codec::verification::RadrootsNip01VerificationError::SignatureInvalid
            )
        ));
        assert!(
            store
                .raw_event(event.id_str())
                .await
                .expect("raw event")
                .is_none()
        );
        assert!(
            store
                .raw_events_after(0, 10)
                .await
                .expect("events")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn out_of_range_kind_events_are_rejected_before_storage() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let event = synthetic_signed_event(u32::from(u16::MAX) + 1, 13, Vec::new(), "hello");

        let error = RadrootsEventIngest::from_signed_event(event.clone(), 2_250)
            .expect_err("kind out of range");

        assert!(matches!(
            error,
            RadrootsEventStoreError::Nip01Verification(
                radroots_event_codec::verification::RadrootsNip01VerificationError::KindOutOfRange {
                    ..
                }
            )
        ));
        assert!(
            store
                .raw_event(event.id_str())
                .await
                .expect("raw event")
                .is_none()
        );
    }

    #[tokio::test]
    async fn ephemeral_admission_outcomes_are_never_persisted() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let capacity_before = store
            .source_capacity_v1()
            .await
            .expect("capacity before ephemeral ingests");
        let admitted = signed_event(KIND_GEOCHAT, 15, Vec::new(), "hello");
        let unsupported = signed_event(29_999, 16, Vec::new(), "unsupported");
        let invalid = signed_event(KIND_RELAY_AUTH, 17, Vec::new(), "not-json");
        let observation = RadrootsTransportObservation::new(
            RadrootsTransportKind::Nostr,
            "wss://relay.example.test",
            RadrootsTransportObservationType::Subscription,
            2_260,
        )
        .expect("observation");

        let admitted_receipt = store
            .ingest_event(
                RadrootsEventIngest::new(admitted.clone(), 2_260).with_observation(observation),
            )
            .await
            .expect("ingest");
        let unsupported_receipt = store
            .ingest_event(RadrootsEventIngest::new(unsupported.clone(), 2_261))
            .await
            .expect("unsupported");
        let invalid_receipt = store
            .ingest_event(RadrootsEventIngest::new(invalid.clone(), 2_262))
            .await
            .expect("invalid");

        assert_eq!(
            admitted_receipt.persistence,
            RadrootsEventPersistence::NotPersisted
        );
        assert_eq!(
            admitted_receipt.admission_status,
            RadrootsEventAdmissionStatus::Admitted
        );
        assert_eq!(admitted_receipt.admission_code, None);
        assert_eq!(
            unsupported_receipt.admission_status,
            RadrootsEventAdmissionStatus::Unsupported
        );
        assert_eq!(
            unsupported_receipt.admission_code.as_deref(),
            Some("unsupported_kind")
        );
        assert_eq!(
            invalid_receipt.admission_status,
            RadrootsEventAdmissionStatus::Invalid
        );
        assert!(invalid_receipt.admission_code.is_some());
        for receipt in [&admitted_receipt, &unsupported_receipt, &invalid_receipt] {
            assert_eq!(receipt.persistence, RadrootsEventPersistence::NotPersisted);
            assert!(!receipt.valid_stream_eligible);
            assert_eq!(
                receipt.raw_head_decision,
                RadrootsRawHeadDecision::NotPersisted
            );
        }
        for event in [&admitted, &unsupported, &invalid] {
            assert!(
                store
                    .raw_event(event.id_str())
                    .await
                    .expect("raw event")
                    .is_none()
            );
            assert!(
                store
                    .valid_event(event.id_str())
                    .await
                    .expect("valid event")
                    .is_none()
            );
            assert_eq!(
                store
                    .event_visibility(event.id_str())
                    .await
                    .expect("visibility"),
                None
            );
            assert!(
                store
                    .tags_for_event(event.id_str())
                    .await
                    .expect("tags")
                    .is_empty()
            );
        }
        assert!(
            store
                .observations_for_event(admitted.id_str())
                .await
                .expect("observations")
                .is_empty()
        );
        let status = store.status_summary().await.expect("status");
        assert_eq!(status.total_events, 0);
        assert_eq!(status.valid_stream_events, 0);
        assert_eq!(status.transport_observations, 0);
        assert_eq!(
            store
                .source_capacity_v1()
                .await
                .expect("capacity after ephemeral ingests"),
            capacity_before
        );
        assert_eq!(
            admitted_receipt.raw_head_decision,
            RadrootsRawHeadDecision::NotPersisted
        );
    }

    #[tokio::test]
    async fn event_head_helper_maps_not_persisted_candidates() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let event = signed_event(KIND_GEOCHAT, 17, Vec::new(), "hello");
        let mut tx = store.pool.begin().await.expect("tx");

        let head = apply_raw_event_head(
            &mut tx,
            ReconciliationProfile::Nip09V1RegistryV7,
            event.envelope(),
            2_280,
        )
        .await
        .expect("head");

        assert_eq!(head.decision, RadrootsRawHeadDecision::NotPersisted);
    }

    #[tokio::test]
    async fn marker_free_classified_listings_are_unsupported() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let event = signed_event(KIND_CLASSIFIED_LISTING, 16, Vec::new(), "{}");

        let receipt = store
            .ingest_event(RadrootsEventIngest::new(event.clone(), 2_270))
            .await
            .expect("ingest");
        let stored = store
            .raw_event(event.id_str())
            .await
            .expect("get")
            .expect("stored");

        assert_eq!(
            receipt.admission_status,
            RadrootsEventAdmissionStatus::Unsupported
        );
        assert_eq!(receipt.raw_head_decision, RadrootsRawHeadDecision::Applied);
        assert!(!receipt.valid_stream_eligible);
        assert!(!stored.valid_stream_eligible);
        let coordinate = head_coordinate_for_event(&event);
        assert!(matches!(
            &coordinate,
            RadrootsEventHeadCoordinate::Addressable { d_tag, .. } if d_tag.is_empty()
        ));
        assert_eq!(
            store
                .raw_event_head(&coordinate)
                .await
                .expect("raw head")
                .expect("stored raw head")
                .event_id,
            event.id_str()
        );
        assert_eq!(
            store
                .event_visibility(event.id_str())
                .await
                .expect("visibility"),
            Some(RadrootsEventVisibility::NotAdmitted)
        );
        assert!(
            store
                .visible_event_head(&coordinate)
                .await
                .expect("visible head")
                .is_none()
        );
    }

    #[tokio::test]
    async fn ambiguous_classified_listing_shape_is_invalid_but_still_updates_the_raw_head() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let capacity_before = store
            .source_capacity_v1()
            .await
            .expect("capacity before invalid durable ingest");
        let event = signed_event(
            KIND_CLASSIFIED_LISTING,
            17,
            vec![
                vec!["d".to_owned(), "mixed-listing".to_owned()],
                vec!["radroots:primary_bin".to_owned(), "bin-1".to_owned()],
                vec!["radroots:price_unit".to_owned(), "kg".to_owned()],
            ],
            "{}",
        );
        let coordinate = head_coordinate_for_event(&event);

        let receipt = store
            .ingest_event(RadrootsEventIngest::new(event.clone(), 2_275))
            .await
            .expect("ingest");
        let capacity_after = store
            .source_capacity_v1()
            .await
            .expect("capacity after invalid durable ingest");

        assert_eq!(
            receipt.admission_status,
            RadrootsEventAdmissionStatus::Invalid
        );
        assert_eq!(
            receipt.admission_code.as_deref(),
            Some("food_profile_ambiguous")
        );
        assert!(!receipt.valid_stream_eligible);
        assert_eq!(receipt.raw_head_decision, RadrootsRawHeadDecision::Applied);
        assert_eq!(
            capacity_after.raw_event_count(),
            capacity_before.raw_event_count() + 1
        );
        assert_eq!(
            capacity_after.raw_tag_count(),
            capacity_before.raw_tag_count() + 3
        );
        assert!(capacity_after.raw_event_text_bytes() > capacity_before.raw_event_text_bytes());
        assert!(capacity_after.raw_tag_text_bytes() > capacity_before.raw_tag_text_bytes());
        assert!(
            store
                .raw_event(event.id_str())
                .await
                .expect("raw event")
                .is_some()
        );
        assert!(
            store
                .valid_event(event.id_str())
                .await
                .expect("valid event")
                .is_none()
        );
        assert_eq!(
            store
                .raw_event_head(&coordinate)
                .await
                .expect("raw head")
                .expect("head")
                .event_id,
            event.id_str()
        );
        assert_eq!(
            store
                .event_visibility(event.id_str())
                .await
                .expect("visibility"),
            Some(RadrootsEventVisibility::NotAdmitted)
        );
        assert!(
            store
                .visible_event_head(&coordinate)
                .await
                .expect("visible head")
                .is_none()
        );
    }

    #[tokio::test]
    async fn food_availability_projection_replaces_and_queries_real_ingest_events() {
        const BLOSSOM_CARROTS: &str = "https://media.example/2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824.webp";
        const BLOSSOM_DETAIL: &str = "https://media.example/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.jpg";

        let store = RadrootsEventStore::open_memory().await.expect("open");
        let carrots = food_availability_event(
            200,
            "nantes-carrots",
            "Nantes Carrots",
            "Fresh bunches",
            "active",
            vec![
                vec![
                    "image".to_owned(),
                    BLOSSOM_CARROTS.to_owned(),
                    "800x600".to_owned(),
                ],
                vec!["image".to_owned(), BLOSSOM_DETAIL.to_owned()],
            ],
        );
        let kale = food_availability_event(
            201,
            "lacinato-kale",
            "Lacinato Kale",
            "Tender greens",
            "active",
            Vec::new(),
        );

        for (observed_at_ms, event) in [(10_000, &carrots), (10_001, &kale)] {
            let receipt = store
                .ingest_event(RadrootsEventIngest::new(event.clone(), observed_at_ms))
                .await
                .expect("food ingest");
            assert_eq!(
                receipt.admission_status,
                RadrootsEventAdmissionStatus::Admitted
            );
            assert_eq!(
                receipt.contract_id.as_deref(),
                Some("radroots.food.availability.v1")
            );
        }
        let initial_projection_count: i64 = sqlx::query_scalar(
            "SELECT projected_row_count FROM radroots_event_store_food_availability_cursor WHERE singleton = 1",
        )
        .fetch_one(store.pool())
        .await
        .expect("sealed projection count");
        assert_eq!(initial_projection_count, 2);

        let author = RadrootsPublicKey::parse(FIXTURE_ALICE_PUBLIC_KEY_HEX).expect("author");
        let carrot_id = RadrootsFoodIdentifier::parse("nantes-carrots").expect("identifier");
        let projected = store
            .food_availability_v1(&author, &carrot_id)
            .await
            .expect("food lookup")
            .expect("projected carrots");
        assert_eq!(projected.event_id().as_str(), carrots.id_str());
        assert_eq!(projected.d_tag(), &carrot_id);
        assert_eq!(projected.title().as_str(), "Nantes Carrots");
        assert_eq!(projected.summary().as_str(), "Fresh bunches");
        assert_eq!(projected.price().amount(), "3");
        assert_eq!(projected.price().currency().as_str(), "CAD");
        assert_eq!(projected.price().unit().as_str(), "lb");
        assert_eq!(projected.quantity().expect("quantity").amount(), "10");
        assert_eq!(projected.status(), RadrootsFoodAvailabilityStatus::Active);
        assert_eq!(
            projected.diagnostics(),
            &[
                RadrootsFoodAvailabilityImageDiagnostic::ShapeInvalid,
                RadrootsFoodAvailabilityImageDiagnostic::DimensionsMissing,
            ]
        );
        assert_eq!(projected.images().len(), 2);
        assert!(projected.images()[0].qualifies());
        assert_eq!(
            projected.images()[0].blossom_sha256(),
            Some(
                radroots_blossom::RadrootsBlossomSha256::from_hex(
                    "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824",
                )
                .expect("Blossom digest"),
            )
        );
        assert!(!projected.images()[1].qualifies());
        assert_eq!(
            projected.images()[1].blossom_sha256(),
            Some(
                radroots_blossom::RadrootsBlossomSha256::from_hex(
                    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                )
                .expect("Blossom digest"),
            )
        );

        let active = store
            .recent_food_availability_v1(crate::RadrootsFoodAvailabilityStatusFilterV1::Active, 10)
            .await
            .expect("active food");
        assert_eq!(active.len(), 2);
        let carrots_query =
            crate::RadrootsFoodAvailabilitySearchQueryV1::parse("Nantes Central Saanich")
                .expect("search query");
        let matches = store
            .search_food_availability_v1(
                &carrots_query,
                crate::RadrootsFoodAvailabilityStatusFilterV1::Any,
                10,
            )
            .await
            .expect("search");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].event_id().as_str(), carrots.id_str());
        let summary_query = crate::RadrootsFoodAvailabilitySearchQueryV1::parse("Fresh")
            .expect("summary search query");
        let summary_matches = store
            .search_food_availability_v1(
                &summary_query,
                crate::RadrootsFoodAvailabilityStatusFilterV1::Any,
                10,
            )
            .await
            .expect("summary search");
        assert_eq!(summary_matches.len(), 1);
        assert_eq!(summary_matches[0].event_id().as_str(), carrots.id_str());

        let sold = food_availability_event(
            220,
            "nantes-carrots",
            "Nantes Carrots Sold",
            "Farm stand sold out",
            "sold",
            Vec::new(),
        );
        store
            .ingest_event(RadrootsEventIngest::new(sold.clone(), 10_002))
            .await
            .expect("sold replacement");

        let replacement = store
            .food_availability_v1(&author, &carrot_id)
            .await
            .expect("replacement lookup")
            .expect("sold projection");
        assert_eq!(replacement.event_id().as_str(), sold.id_str());
        assert_eq!(replacement.status(), RadrootsFoodAvailabilityStatus::Sold);
        assert_eq!(replacement.published_at().as_u64(), 100);
        assert!(replacement.images().is_empty());
        let replacement_projection_count: i64 = sqlx::query_scalar(
            "SELECT projected_row_count FROM radroots_event_store_food_availability_cursor WHERE singleton = 1",
        )
        .fetch_one(store.pool())
        .await
        .expect("sealed replacement projection count");
        assert_eq!(replacement_projection_count, 2);
        assert_eq!(
            store
                .current_event_visibility_v1(carrots.id_str())
                .await
                .expect("old visibility")
                .expect("stored old revision")
                .decision(),
            crate::RadrootsCurrentVisibilityDecisionV1::NotCurrent
        );
        assert_eq!(
            store
                .current_event_visibility_v1(sold.id_str())
                .await
                .expect("sold visibility")
                .expect("stored sold revision")
                .decision(),
            crate::RadrootsCurrentVisibilityDecisionV1::Visible
        );

        let active = store
            .recent_food_availability_v1(crate::RadrootsFoodAvailabilityStatusFilterV1::Active, 10)
            .await
            .expect("active after replacement");
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].event_id().as_str(), kale.id_str());
        let sold_rows = store
            .recent_food_availability_v1(crate::RadrootsFoodAvailabilityStatusFilterV1::Sold, 10)
            .await
            .expect("sold food");
        assert_eq!(sold_rows.len(), 1);
        assert_eq!(sold_rows[0].event_id().as_str(), sold.id_str());

        let stale_query = crate::RadrootsFoodAvailabilitySearchQueryV1::parse("Fresh bunches")
            .expect("stale query");
        assert!(
            store
                .search_food_availability_v1(
                    &stale_query,
                    crate::RadrootsFoodAvailabilityStatusFilterV1::Any,
                    10,
                )
                .await
                .expect("stale search")
                .is_empty()
        );
        let replacement_query = crate::RadrootsFoodAvailabilitySearchQueryV1::parse("Farm sold")
            .expect("replacement query");
        let replacement_matches = store
            .search_food_availability_v1(
                &replacement_query,
                crate::RadrootsFoodAvailabilityStatusFilterV1::Sold,
                10,
            )
            .await
            .expect("replacement search");
        assert_eq!(replacement_matches.len(), 1);
        assert_eq!(replacement_matches[0].event_id().as_str(), sold.id_str());
    }

    #[test]
    fn stored_food_projection_rejects_unrepresentable_internal_boundaries() {
        let event = food_availability_event(
            200,
            "bounded-model-carrots",
            "Bounded Model Carrots",
            "Fresh model harvest",
            "active",
            vec![vec![
                "image".to_owned(),
                "https://media.example/2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824.webp"
                    .to_owned(),
                "800x600".to_owned(),
            ]],
        );
        let ingest = RadrootsEventIngest::new(event.clone(), 10_000);
        let projection = match radroots_event_codec::food_availability::inbound::registry_v7::project_verified_food_availability_event_registry_v7(
            ingest.verified_event(),
        )
        .expect("food projection")
        {
            radroots_event_codec::food_availability::inbound::registry_v7::RadrootsFoodAvailabilityProjectionOutcome::Focused(projection) => projection,
            _ => panic!("unexpected food projection outcome"),
        };
        let image = projection.images().first().expect("projected image");
        assert!(matches!(
            crate::RadrootsStoredFoodAvailabilityImageV1::from_projection_for_test(
                usize::MAX,
                image,
            ),
            Err(RadrootsEventStoreError::FoodAvailabilityProjectionDrift { ref reason })
                if reason.contains("image index exceeds")
        ));

        let author = RadrootsPublicKey::parse(event.pubkey_str()).expect("author");
        let event_id = RadrootsEventId::parse(event.id_str()).expect("event id");
        for (event_seq, source_transition_seq, expected_reason) in [
            (0, 1, "event sequence must be positive"),
            (1, 0, "source transition sequence must be positive"),
        ] {
            assert!(matches!(
                crate::RadrootsStoredFoodAvailabilityV1::from_projection(
                    RadrootsEventStoreSourceGeneration::from_bytes([0x55; 32]),
                    author.clone(),
                    event_id.clone(),
                    event_seq,
                    200,
                    source_transition_seq,
                    &projection,
                ),
                Err(RadrootsEventStoreError::FoodAvailabilityProjectionDrift { ref reason })
                    if reason.contains(expected_reason)
            ));
        }
        assert!(matches!(
            crate::RadrootsStoredFoodAvailabilityV1::from_projection(
                RadrootsEventStoreSourceGeneration::from_bytes([0x55; 32]),
                author.clone(),
                event_id.clone(),
                1,
                99,
                1,
                &projection,
            ),
            Err(RadrootsEventStoreError::FoodAvailabilityProjectionDrift { .. })
        ));

        let stored = crate::RadrootsStoredFoodAvailabilityV1::from_projection(
            RadrootsEventStoreSourceGeneration::from_bytes([0x55; 32]),
            author,
            event_id,
            1,
            200,
            1,
            &projection,
        )
        .expect("bounded stored projection");
        assert_eq!(stored.d_tag().as_str(), "bounded-model-carrots");
    }

    #[tokio::test]
    async fn food_availability_projection_guards_images_and_exhaustive_audit_detects_fts_drift() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let with_image = food_availability_event(
            200,
            "guarded-carrots",
            "Guarded Carrots",
            "Fresh harvest",
            "active",
            vec![vec![
                "image".to_owned(),
                "https://media.example/2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824.webp"
                    .to_owned(),
                "800x600".to_owned(),
            ]],
        );
        store
            .ingest_event(RadrootsEventIngest::new(with_image, 19_000))
            .await
            .expect("FoodAvailability ingest");

        let image_delete = sqlx::query(
            "DELETE FROM radroots_event_store_food_availability_image WHERE d_tag = 'guarded-carrots'",
        )
        .execute(store.pool())
        .await
        .expect_err("direct image delete must be guarded");
        assert!(
            image_delete
                .to_string()
                .contains("image delete is not backed by a pending retraction")
        );
        let cursor_update = sqlx::query(
            "UPDATE radroots_event_store_food_availability_cursor SET last_transition_seq = last_transition_seq WHERE singleton = 1",
        )
        .execute(store.pool())
        .await
        .expect_err("projection cursor must reject direct writes");
        assert!(
            cursor_update
                .to_string()
                .contains("FoodAvailability cursor update is invalid")
        );

        let replacement = food_availability_event(
            210,
            "guarded-carrots",
            "Guarded Carrots",
            "Sold at market",
            "sold",
            Vec::new(),
        );
        store
            .ingest_event(RadrootsEventIngest::new(replacement.clone(), 19_001))
            .await
            .expect("authorized replacement cascade");
        let image_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM radroots_event_store_food_availability_image")
                .fetch_one(store.pool())
                .await
                .expect("image count");
        assert_eq!(image_count, 0);

        let event_seq: i64 = sqlx::query_scalar(
            "SELECT event_seq FROM radroots_event_store_food_availability_projection WHERE event_id = ?",
        )
        .bind(replacement.id_str())
        .fetch_one(store.pool())
        .await
        .expect("replacement sequence");
        sqlx::query(
            "DELETE FROM radroots_event_store_food_availability_search_fts WHERE rowid = ?",
        )
        .bind(event_seq)
        .execute(store.pool())
        .await
        .expect("test-only FTS corruption");
        assert!(matches!(
            store.audit_food_availability_projection_v1().await,
            Err(RadrootsEventStoreError::FoodAvailabilityProjectionDrift { .. })
        ));
    }

    #[tokio::test]
    async fn food_availability_exhaustive_audit_rejects_projection_and_image_row_drift() {
        for (label, guard, bypass_checks, mutation, expected_reason) in [
            (
                "projection identity",
                Some("DROP TRIGGER radroots_event_store_food_availability_projection_update_guard"),
                false,
                "UPDATE radroots_event_store_food_availability_projection SET created_at = created_at + 1",
                "projection identity disagrees",
            ),
            (
                "projection column",
                Some("DROP TRIGGER radroots_event_store_food_availability_projection_update_guard"),
                false,
                "UPDATE radroots_event_store_food_availability_projection SET title = 'corrupt title'",
                "columns differ",
            ),
            (
                "image count",
                Some("DROP TRIGGER radroots_event_store_food_availability_image_delete_guard"),
                false,
                "DELETE FROM radroots_event_store_food_availability_image",
                "image count differs",
            ),
            (
                "Blossom digest",
                Some("DROP TRIGGER radroots_event_store_food_availability_image_update_guard"),
                true,
                "UPDATE radroots_event_store_food_availability_image SET blossom_sha256 = 'invalid'",
                "stored Blossom digest is invalid",
            ),
            (
                "image dimensions",
                Some("DROP TRIGGER radroots_event_store_food_availability_image_update_guard"),
                false,
                "UPDATE radroots_event_store_food_availability_image SET width = width + 1",
                "stored FoodAvailability image differs",
            ),
            (
                "sealed projection row count",
                Some("DROP TRIGGER radroots_event_store_food_availability_cursor_update_guard"),
                false,
                "UPDATE radroots_event_store_food_availability_cursor SET projected_row_count = projected_row_count + 1",
                "projection row count",
            ),
            (
                "FTS content",
                None,
                false,
                "UPDATE radroots_event_store_food_availability_search_fts SET title = 'corrupt title'",
                "FTS row differs",
            ),
        ] {
            let store = food_availability_audit_corruption_store().await;
            let mut connection = store.pool().acquire().await.expect("trusted connection");
            if let Some(guard) = guard {
                sqlx::query(guard)
                    .execute(&mut *connection)
                    .await
                    .expect("trusted FoodAvailability guard removal");
            }
            if bypass_checks {
                sqlx::query("PRAGMA ignore_check_constraints = ON")
                    .execute(&mut *connection)
                    .await
                    .expect("enable trusted check-constraint bypass");
            }
            sqlx::query(mutation)
                .execute(&mut *connection)
                .await
                .expect("trusted FoodAvailability corruption");
            if bypass_checks {
                sqlx::query("PRAGMA ignore_check_constraints = OFF")
                    .execute(&mut *connection)
                    .await
                    .expect("restore check-constraint enforcement");
            }
            drop(connection);

            let error = store
                .audit_food_availability_projection_v1()
                .await
                .expect_err("corrupt FoodAvailability authority must fail audit");
            assert!(
                matches!(
                    error,
                    RadrootsEventStoreError::FoodAvailabilityProjectionDrift { ref reason }
                        if reason.contains(expected_reason)
                ),
                "{label}: {error}",
            );
        }
    }

    #[tokio::test]
    async fn food_availability_exhaustive_audit_rejects_seal_and_fts_count_drift() {
        for (label, guard, mutation, expected_reason) in [
            (
                "missing projection cursor",
                Some("DROP TRIGGER radroots_event_store_food_availability_cursor_delete_guard"),
                "DELETE FROM radroots_event_store_food_availability_cursor",
                "active source, feed, or projection seal is missing",
            ),
            (
                "feed integrity seal",
                None,
                "UPDATE radroots_event_store_addressable_feed_integrity_v1 SET last_transition_seq = last_transition_seq + 1, transition_count = transition_count + 1",
                "active addressable feed integrity seal is inconsistent",
            ),
            (
                "cursor high-water",
                Some("DROP TRIGGER radroots_event_store_food_availability_cursor_update_guard"),
                "UPDATE radroots_event_store_food_availability_cursor SET last_transition_seq = last_transition_seq - 1",
                "projection cursor is not at the source high-water",
            ),
            (
                "FTS row count",
                None,
                "INSERT INTO radroots_event_store_food_availability_search_fts(rowid, event_id, pubkey, d_tag, title, summary, content, location) VALUES (999, 'extra', 'extra', 'extra', 'extra', 'extra', 'extra', 'extra')",
                "FoodAvailability FTS row count",
            ),
        ] {
            let store = food_availability_audit_corruption_store().await;
            let mut connection = store.pool().acquire().await.expect("trusted connection");
            if let Some(guard) = guard {
                sqlx::query(guard)
                    .execute(&mut *connection)
                    .await
                    .expect("trusted FoodAvailability seal guard removal");
            }
            sqlx::query(mutation)
                .execute(&mut *connection)
                .await
                .expect("trusted FoodAvailability seal corruption");
            drop(connection);

            let error = store
                .audit_food_availability_projection_v1()
                .await
                .expect_err("corrupt FoodAvailability seal must fail audit");
            assert!(
                matches!(
                    error,
                    RadrootsEventStoreError::FoodAvailabilityProjectionDrift { ref reason }
                        if reason.contains(expected_reason)
                ),
                "{label}: {error}",
            );
        }
    }

    #[tokio::test]
    async fn food_availability_exhaustive_audit_rejects_wrong_source_transition_authority() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        for event in [
            food_availability_event(
                200,
                "transition-carrots",
                "Transition Carrots",
                "First harvest",
                "active",
                Vec::new(),
            ),
            food_availability_event(
                201,
                "transition-kale",
                "Transition Kale",
                "Second harvest",
                "active",
                Vec::new(),
            ),
        ] {
            store
                .ingest_event(RadrootsEventIngest::new(event, 19_100))
                .await
                .expect("FoodAvailability ingest");
        }

        let wrong_transition_seq: i64 = sqlx::query_scalar(
            "SELECT source_transition_seq FROM radroots_event_store_food_availability_projection WHERE d_tag = 'transition-kale'",
        )
        .fetch_one(store.pool())
        .await
        .expect("wrong-coordinate transition");
        sqlx::query("DROP TRIGGER radroots_event_store_food_availability_projection_update_guard")
            .execute(store.pool())
            .await
            .expect("trusted projection guard removal");
        sqlx::query(
            "UPDATE radroots_event_store_food_availability_projection SET source_transition_seq = ? WHERE d_tag = 'transition-carrots'",
        )
        .bind(wrong_transition_seq)
        .execute(store.pool())
        .await
        .expect("trusted source-transition corruption");

        let error = store
            .audit_food_availability_projection_v1()
            .await
            .expect_err("wrong source transition must fail exhaustive audit");
        assert!(
            matches!(
                error,
                RadrootsEventStoreError::FoodAvailabilityProjectionDrift { ref reason }
                    if reason.contains("source transition")
            ),
            "{error}"
        );
    }

    #[tokio::test]
    async fn food_availability_exhaustive_audit_compares_exact_head_coordinates() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        store
            .ingest_event(RadrootsEventIngest::new(
                food_availability_event(
                    200,
                    "coordinate-carrots",
                    "Coordinate Carrots",
                    "Coordinate harvest",
                    "active",
                    Vec::new(),
                ),
                19_200,
            ))
            .await
            .expect("FoodAvailability ingest");

        let mut connection = store.pool().acquire().await.expect("trusted connection");
        for statement in [
            "DROP TRIGGER radroots_event_store_addressable_state_identity_update_guard",
            "DROP TRIGGER radroots_event_store_addressable_state_old_update_guard",
        ] {
            sqlx::query(statement)
                .execute(&mut *connection)
                .await
                .expect("trusted head-state guard removal");
        }
        sqlx::query("PRAGMA foreign_keys = OFF")
            .execute(&mut *connection)
            .await
            .expect("disable trusted foreign-key enforcement");
        sqlx::query(
            "UPDATE radroots_event_store_addressable_head_state SET d_tag = 'retargeted-carrots' WHERE d_tag = 'coordinate-carrots'",
        )
        .execute(&mut *connection)
        .await
        .expect("trusted head-coordinate corruption");
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&mut *connection)
            .await
            .expect("restore foreign-key enforcement");
        drop(connection);

        let error = store
            .audit_food_availability_projection_v1()
            .await
            .expect_err("retargeted head coordinate must fail exhaustive audit");
        assert!(
            matches!(
                error,
                RadrootsEventStoreError::FoodAvailabilityProjectionDrift { ref reason }
                    if reason.contains("coordinate witnesses")
            ),
            "{error}"
        );
    }

    #[tokio::test]
    async fn food_availability_exhaustive_audit_reserves_wal_writer_before_snapshot_reads() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let path = tempdir.path().join("food-audit-wal.sqlite");
        let audit_store = RadrootsEventStore::open_file(&path)
            .await
            .expect("audit store");
        let writer_store = RadrootsEventStore::open_file(&path)
            .await
            .expect("writer store");
        audit_store
            .ingest_event(RadrootsEventIngest::new(
                food_availability_event(
                    200,
                    "wal-carrots",
                    "WAL Carrots",
                    "Serialized audit harvest",
                    "active",
                    Vec::new(),
                ),
                19_300,
            ))
            .await
            .expect("FoodAvailability ingest");
        assert_eq!(
            audit_store
                .pragma_journal_mode()
                .await
                .expect("journal mode"),
            "wal"
        );

        let checkpoint_reached = std::sync::Arc::new(tokio::sync::Notify::new());
        let checkpoint_release = std::sync::Arc::new(tokio::sync::Notify::new());
        let audit_task = tokio::spawn(
            super::food_availability_projection_v1::FOOD_AVAILABILITY_AUDIT_FTS_CHECKPOINT.scope(
                (
                    std::sync::Arc::clone(&checkpoint_reached),
                    std::sync::Arc::clone(&checkpoint_release),
                ),
                async move { audit_store.audit_food_availability_projection_v1().await },
            ),
        );
        checkpoint_reached.notified().await;

        let writer_started = std::sync::Arc::new(tokio::sync::Notify::new());
        let writer_started_task = std::sync::Arc::clone(&writer_started);
        let writer_task = tokio::spawn(async move {
            writer_started_task.notify_one();
            let transaction = writer_store
                .begin_write_transaction()
                .await
                .expect("competing writer transaction");
            transaction.commit().await.expect("competing writer commit");
        });
        writer_started.notified().await;
        tokio::task::yield_now().await;
        assert!(
            !writer_task.is_finished(),
            "competing writer acquired while the audit was paused before FTS integrity-check"
        );

        checkpoint_release.notify_one();
        audit_task
            .await
            .expect("audit task")
            .expect("serialized exhaustive audit");
        writer_task.await.expect("competing writer task");
    }

    #[tokio::test]
    async fn food_availability_queries_enforce_limits_order_ties_and_execute_literal_fts_input() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let events = [
            food_availability_event(
                200,
                "query-carrots",
                "Query Carrots",
                "Shared harvest",
                "active",
                Vec::new(),
            ),
            food_availability_event(
                201,
                "query-kale",
                "Query Kale",
                "Shared harvest",
                "active",
                Vec::new(),
            ),
            food_availability_event(
                202,
                "query-beets",
                "Query Beets",
                "Shared harvest",
                "active",
                Vec::new(),
            ),
        ];
        for (index, event) in events.iter().enumerate() {
            store
                .ingest_event(RadrootsEventIngest::new(
                    event.clone(),
                    19_100 + i64::try_from(index).expect("index"),
                ))
                .await
                .expect("query fixture ingest");
        }

        let mut expected_ids = events
            .iter()
            .map(|event| event.id_str().to_owned())
            .collect::<Vec<_>>();
        expected_ids.sort();
        let recent = store
            .recent_food_availability_v1(
                crate::RadrootsFoodAvailabilityStatusFilterV1::Any,
                RADROOTS_EVENT_STORE_QUERY_LIMIT_MAX,
            )
            .await
            .expect("maximum-limit recent query");
        assert_eq!(
            recent
                .iter()
                .map(|projection| projection.event_id().as_str())
                .collect::<Vec<_>>(),
            expected_ids.iter().map(String::as_str).collect::<Vec<_>>(),
        );
        assert_eq!(
            store
                .recent_food_availability_v1(crate::RadrootsFoodAvailabilityStatusFilterV1::Any, 1,)
                .await
                .expect("minimum-limit recent query")[0]
                .event_id()
                .as_str(),
            expected_ids[0],
        );
        for invalid_limit in [0, RADROOTS_EVENT_STORE_QUERY_LIMIT_MAX + 1] {
            assert!(matches!(
                store
                    .recent_food_availability_v1(
                        crate::RadrootsFoodAvailabilityStatusFilterV1::Any,
                        invalid_limit,
                    )
                    .await,
                Err(RadrootsEventStoreError::QueryLimitOutOfRange { .. })
            ));
        }

        let shared = crate::RadrootsFoodAvailabilitySearchQueryV1::parse("Shared")
            .expect("shared search query");
        let search = store
            .search_food_availability_v1(
                &shared,
                crate::RadrootsFoodAvailabilityStatusFilterV1::Any,
                RADROOTS_EVENT_STORE_QUERY_LIMIT_MAX,
            )
            .await
            .expect("maximum-limit search query");
        assert_eq!(
            search
                .iter()
                .map(|projection| projection.event_id().as_str())
                .collect::<Vec<_>>(),
            expected_ids.iter().map(String::as_str).collect::<Vec<_>>(),
        );
        for hostile in ["\"", "()", "title:", "*", "---", "OR title:beets*"] {
            let query = crate::RadrootsFoodAvailabilitySearchQueryV1::parse(hostile)
                .expect("literal hostile query");
            store
                .search_food_availability_v1(
                    &query,
                    crate::RadrootsFoodAvailabilityStatusFilterV1::Any,
                    1,
                )
                .await
                .expect("hostile input remains a valid literal FTS query");
        }
        assert!(matches!(
            store
                .search_food_availability_v1(
                    &shared,
                    crate::RadrootsFoodAvailabilityStatusFilterV1::Any,
                    0,
                )
                .await,
            Err(RadrootsEventStoreError::QueryLimitOutOfRange { .. })
        ));
    }

    #[tokio::test]
    async fn current_visibility_and_food_reads_use_bounded_authority_indexes() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let visibility_plan = explain_query_plan(
            &store,
            "EXPLAIN QUERY PLAN SELECT suppression_outcome, suppression_reason, event_reference_request_id, address_reference_request_id, address_reference_cutoff, current_visibility FROM radroots_event_store_current_visibility_v1 WHERE event_id = ?",
            event_id('a').as_str(),
        )
        .await;
        assert!(
            visibility_plan.contains("radroots_event_store_nip09_event_target_lookup_idx"),
            "{visibility_plan}"
        );
        assert!(
            visibility_plan
                .contains("radroots_event_store_nip09_address_target_visibility_lookup_idx"),
            "{visibility_plan}"
        );
        assert!(
            !visibility_plan.contains("USE TEMP B-TREE"),
            "{visibility_plan}"
        );

        let food_sql = format!(
            "EXPLAIN QUERY PLAN {}",
            super::food_availability_projection_v1::FOOD_AVAILABILITY_RECENT_QUERY_V1
        );
        let food_plan = explain_query_plan(&store, food_sql.as_str(), "1000").await;
        assert!(
            food_plan.contains("radroots_event_store_food_availability_recent_idx"),
            "{food_plan}"
        );
        assert!(
            food_plan.contains("SEARCH head USING PRIMARY KEY"),
            "{food_plan}"
        );
        assert!(
            !food_plan.contains("radroots_event_store_nip09_event_target")
                && !food_plan.contains("radroots_event_store_nip09_address_target"),
            "{food_plan}"
        );
        assert!(!food_plan.contains("USE TEMP B-TREE"), "{food_plan}");
    }

    #[tokio::test]
    async fn food_availability_retraction_never_resurrects_an_older_revision() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let active = food_availability_event(
            200,
            "nantes-carrots",
            "Nantes Carrots",
            "Fresh bunches",
            "active",
            Vec::new(),
        );
        let sold = food_availability_event(
            220,
            "nantes-carrots",
            "Nantes Carrots Sold",
            "Sold at market",
            "sold",
            Vec::new(),
        );
        let deletion = deletion_event(
            &fixture_keys(),
            230,
            vec![vec![
                "a".to_owned(),
                food_availability_coordinate("nantes-carrots"),
            ]],
        );
        let older = food_availability_event(
            210,
            "nantes-carrots",
            "Nantes Carrots Older",
            "Older active revision",
            "active",
            Vec::new(),
        );
        let recovered = food_availability_event(
            240,
            "nantes-carrots",
            "Nantes Carrots Restocked",
            "Fresh restock",
            "active",
            Vec::new(),
        );
        let author = RadrootsPublicKey::parse(FIXTURE_ALICE_PUBLIC_KEY_HEX).expect("author");
        let identifier = RadrootsFoodIdentifier::parse("nantes-carrots").expect("identifier");

        for (observed_at_ms, event) in [(20_000, &active), (20_001, &sold)] {
            store
                .ingest_event(RadrootsEventIngest::new(event.clone(), observed_at_ms))
                .await
                .expect("food revision");
        }
        store
            .ingest_event(RadrootsEventIngest::new(deletion.clone(), 20_002))
            .await
            .expect("address deletion");
        assert!(
            store
                .food_availability_v1(&author, &identifier)
                .await
                .expect("retracted lookup")
                .is_none()
        );
        let retracted_projection_count: i64 = sqlx::query_scalar(
            "SELECT projected_row_count FROM radroots_event_store_food_availability_cursor WHERE singleton = 1",
        )
        .fetch_one(store.pool())
        .await
        .expect("sealed retracted projection count");
        assert_eq!(retracted_projection_count, 0);
        let suppressed = store
            .current_event_visibility_v1(sold.id_str())
            .await
            .expect("suppressed visibility")
            .expect("stored sold revision");
        assert_eq!(
            suppressed.decision(),
            crate::RadrootsCurrentVisibilityDecisionV1::Suppressed
        );
        let evidence = suppressed.suppression().expect("suppression evidence");
        assert_eq!(
            evidence.reason(),
            crate::RadrootsNip09SuppressionReason::AddressReferenceAtOrBeforeCutoff
        );
        assert_eq!(
            evidence
                .address_reference_request_id()
                .expect("address deletion id")
                .as_str(),
            deletion.id_str()
        );
        assert_eq!(evidence.address_reference_cutoff(), Some(230));

        let older_receipt = store
            .ingest_event(RadrootsEventIngest::new(older.clone(), 20_003))
            .await
            .expect("older late arrival");
        assert_eq!(
            older_receipt.raw_head_decision,
            RadrootsRawHeadDecision::SkippedOlder
        );
        assert!(
            store
                .food_availability_v1(&author, &identifier)
                .await
                .expect("no resurrection lookup")
                .is_none()
        );
        assert_eq!(
            store
                .current_event_visibility_v1(older.id_str())
                .await
                .expect("older visibility")
                .expect("stored older revision")
                .decision(),
            crate::RadrootsCurrentVisibilityDecisionV1::NotCurrent
        );

        store
            .ingest_event(RadrootsEventIngest::new(recovered.clone(), 20_004))
            .await
            .expect("post-cutoff replacement");
        let projection = store
            .food_availability_v1(&author, &identifier)
            .await
            .expect("recovered lookup")
            .expect("recovered projection");
        assert_eq!(projection.event_id().as_str(), recovered.id_str());
        assert_eq!(projection.status(), RadrootsFoodAvailabilityStatus::Active);
        let recovered_projection_count: i64 = sqlx::query_scalar(
            "SELECT projected_row_count FROM radroots_event_store_food_availability_cursor WHERE singleton = 1",
        )
        .fetch_one(store.pool())
        .await
        .expect("sealed recovered projection count");
        assert_eq!(recovered_projection_count, 1);

        let scope = crate::RadrootsAddressableTransitionScopeV1::food_availability();
        let page = store
            .addressable_transition_page_v1(&scope, None, 64)
            .await
            .expect("transition page");
        assert_eq!(page.transitions().len(), 4);
        assert_eq!(page.source_high_water(), 4);
        assert_eq!(page.next_cursor().last_transition_seq(), 4);
        assert!(!page.has_more());
        let deletion_cause = page.transitions()[2]
            .cause_event()
            .expect("deletion cause metadata");
        assert_eq!(
            deletion_cause.event().event_id().as_str(),
            deletion.id_str()
        );
        assert_eq!(
            deletion_cause.pubkey().as_str(),
            FIXTURE_ALICE_PUBLIC_KEY_HEX
        );
        assert_eq!(deletion_cause.created_at(), 230);
        assert_eq!(deletion_cause.kind(), KIND_DELETION_REQUEST);
        assert_eq!(
            deletion_cause.admission_status(),
            RadrootsEventAdmissionStatus::Admitted
        );
        assert!(deletion_cause.admission_code().is_none());
        assert!(deletion_cause.contract_id().is_some());
        assert_eq!(
            page.transitions()[2]
                .retracted_event()
                .expect("sold retraction")
                .event_id()
                .as_str(),
            sold.id_str()
        );
        assert!(page.transitions()[2].visible_event().is_none());
        assert_eq!(
            page.transitions()[2]
                .suppression()
                .expect("feed suppression")
                .reason(),
            crate::RadrootsNip09SuppressionReason::AddressReferenceAtOrBeforeCutoff
        );
        assert_eq!(
            page.transitions()[3]
                .visible_event()
                .expect("recovered canonical event")
                .event_id()
                .as_str(),
            recovered.id_str()
        );
        assert!(page.transitions()[3].retracted_event().is_none());
        assert_eq!(projection.source_transition_seq(), 4);
        let cursor_state: (i64, i64) = sqlx::query_as(
            "SELECT last_transition_seq, projected_row_count FROM radroots_event_store_food_availability_cursor WHERE singleton = 1",
        )
        .fetch_one(store.pool())
        .await
        .expect("projection cursor");
        assert_eq!(cursor_state, (page.source_high_water(), 1));
    }

    #[tokio::test]
    async fn admitted_operational_listing_retracts_food_until_a_later_food_revision() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let identifier =
            RadrootsFoodIdentifier::parse("AAAAAAAAAAAAAAAAAAAAAg").expect("identifier");
        let author = RadrootsPublicKey::parse(FIXTURE_ALICE_PUBLIC_KEY_HEX).expect("author");
        let food = food_availability_event(
            200,
            identifier.as_str(),
            "Partition Carrots",
            "Focused contract",
            "active",
            Vec::new(),
        );
        store
            .ingest_event(RadrootsEventIngest::new(food, 29_000))
            .await
            .expect("focused FoodAvailability ingest");

        let operational = signed_event(
            KIND_CLASSIFIED_LISTING,
            210,
            admitted_operational_listing_tags(identifier.as_str(), 210),
            "# Nantes Carrots\n\nFresh bunches harvested in Saanich",
        );
        let receipt = store
            .ingest_event(RadrootsEventIngest::new(operational.clone(), 29_001))
            .await
            .expect("operational listing ingest");
        assert_eq!(
            receipt.admission_status,
            RadrootsEventAdmissionStatus::Admitted
        );
        assert_eq!(
            receipt.contract_id.as_deref(),
            Some("radroots.operational_listing.published.v1"),
        );
        assert!(
            store
                .food_availability_v1(&author, &identifier)
                .await
                .expect("projection after operational head")
                .is_none()
        );
        let retracted_count: i64 = sqlx::query_scalar(
            "SELECT projected_row_count FROM radroots_event_store_food_availability_cursor WHERE singleton = 1",
        )
        .fetch_one(store.pool())
        .await
        .expect("sealed operational-head count");
        assert_eq!(retracted_count, 0);

        let restored = food_availability_event(
            220,
            identifier.as_str(),
            "Partition Carrots Restored",
            "Focused contract restored",
            "active",
            Vec::new(),
        );
        store
            .ingest_event(RadrootsEventIngest::new(restored.clone(), 29_002))
            .await
            .expect("restored FoodAvailability ingest");
        assert_eq!(
            store
                .food_availability_v1(&author, &identifier)
                .await
                .expect("restored lookup")
                .expect("restored projection")
                .event_id()
                .as_str(),
            restored.id_str(),
        );
        let page = store
            .addressable_transition_page_v1(
                &crate::RadrootsAddressableTransitionScopeV1::food_availability(),
                None,
                64,
            )
            .await
            .expect("partition transition page");
        assert_eq!(page.transitions().len(), 3);
        assert_eq!(
            page.transitions()[1].contract_id(),
            receipt.contract_id.as_deref()
        );
        assert_eq!(
            page.transitions()[1]
                .visible_event()
                .expect("operational transition payload")
                .event_id()
                .as_str(),
            operational.id_str(),
        );
        assert_eq!(
            page.transitions()[1]
                .retracted_event()
                .expect("focused projection retraction")
                .event_id()
                .as_str(),
            page.transitions()[0]
                .visible_event()
                .expect("initial focused payload")
                .event_id()
                .as_str(),
        );
    }

    #[tokio::test]
    async fn food_availability_projection_and_feed_rollback_atomically() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let scope = crate::RadrootsAddressableTransitionScopeV1::food_availability();
        let initial_page = store
            .addressable_transition_page_v1(&scope, None, 64)
            .await
            .expect("initial feed");
        assert!(initial_page.transitions().is_empty());
        assert_eq!(initial_page.next_cursor().last_transition_seq(), 0);
        let initial_cursor = initial_page.next_cursor().clone();
        let event = food_availability_event(
            200,
            "rollback-carrots",
            "Rollback Carrots",
            "Transaction fixture",
            "active",
            Vec::new(),
        );
        let author = RadrootsPublicKey::parse(FIXTURE_ALICE_PUBLIC_KEY_HEX).expect("author");
        let identifier = RadrootsFoodIdentifier::parse("rollback-carrots").expect("identifier");

        let mut transaction = store.begin_write_transaction().await.expect("transaction");
        store
            .ingest_event_in_transaction(
                &mut transaction,
                RadrootsEventIngest::new(event.clone(), 30_000),
            )
            .await
            .expect("transactional ingest");
        let in_transaction: (i64, i64, i64) = sqlx::query_as(
            "SELECT source.last_transition_seq, cursor.last_transition_seq, (SELECT COUNT(*) FROM radroots_event_store_food_availability_projection) FROM radroots_event_store_source_state AS source JOIN radroots_event_store_food_availability_cursor AS cursor ON cursor.singleton = 1 WHERE source.singleton = 1",
        )
        .fetch_one(&mut *transaction)
        .await
        .expect("transactional projection state");
        assert_eq!(in_transaction, (1, 1, 1));
        transaction.rollback().await.expect("rollback");

        assert!(
            store
                .raw_event(event.id_str())
                .await
                .expect("raw event after rollback")
                .is_none()
        );
        assert!(
            store
                .food_availability_v1(&author, &identifier)
                .await
                .expect("projection after rollback")
                .is_none()
        );
        let after_rollback = store
            .addressable_transition_page_v1(&scope, Some(&initial_cursor), 64)
            .await
            .expect("feed after rollback");
        assert!(after_rollback.transitions().is_empty());
        assert_eq!(after_rollback.source_high_water(), 0);
        assert_eq!(after_rollback.next_cursor().last_transition_seq(), 0);
        let cursor_after_rollback: i64 = sqlx::query_scalar(
            "SELECT last_transition_seq FROM radroots_event_store_food_availability_cursor WHERE singleton = 1",
        )
        .fetch_one(store.pool())
        .await
        .expect("cursor after rollback");
        assert_eq!(cursor_after_rollback, 0);

        store
            .ingest_event(RadrootsEventIngest::new(event.clone(), 30_001))
            .await
            .expect("committed ingest");
        let committed = store
            .food_availability_v1(&author, &identifier)
            .await
            .expect("committed projection")
            .expect("projected event");
        assert_eq!(committed.event_id().as_str(), event.id_str());
        let committed_page = store
            .addressable_transition_page_v1(&scope, Some(&initial_cursor), 64)
            .await
            .expect("committed feed");
        assert_eq!(committed_page.transitions().len(), 1);
        assert_eq!(committed_page.source_high_water(), 1);
        assert_eq!(committed.source_transition_seq(), 1);
    }

    #[tokio::test]
    async fn food_availability_wrong_author_deletion_preserves_projection() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let food = food_availability_event(
            200,
            "protected-carrots",
            "Protected Carrots",
            "Still available",
            "active",
            Vec::new(),
        );
        store
            .ingest_event(RadrootsEventIngest::new(food.clone(), 35_000))
            .await
            .expect("food ingest");
        let wrong_author_deletion = deletion_event(
            &alternate_keys(),
            210,
            vec![vec!["e".to_owned(), food.id_str().to_owned()]],
        );
        store
            .ingest_event(RadrootsEventIngest::new(
                wrong_author_deletion.clone(),
                35_001,
            ))
            .await
            .expect("wrong-author deletion remains a valid event");

        let author = RadrootsPublicKey::parse(FIXTURE_ALICE_PUBLIC_KEY_HEX).expect("author");
        let identifier = RadrootsFoodIdentifier::parse("protected-carrots").expect("identifier");
        let projection = store
            .food_availability_v1(&author, &identifier)
            .await
            .expect("projection lookup")
            .expect("projection remains visible");
        assert_eq!(projection.event_id().as_str(), food.id_str());
        assert_eq!(projection.source_transition_seq(), 1);
        let visibility = store
            .current_event_visibility_v1(food.id_str())
            .await
            .expect("current visibility")
            .expect("stored event");
        assert_eq!(
            visibility.decision(),
            crate::RadrootsCurrentVisibilityDecisionV1::Visible
        );
        assert_eq!(
            visibility
                .suppression()
                .expect("visibility evidence")
                .reason(),
            crate::RadrootsNip09SuppressionReason::RequestAuthorMismatch
        );

        let scope = crate::RadrootsAddressableTransitionScopeV1::food_availability();
        let page = store
            .addressable_transition_page_v1(&scope, None, 64)
            .await
            .expect("transition page");
        assert_eq!(page.transitions().len(), 2);
        let unchanged = &page.transitions()[1];
        assert_eq!(
            unchanged
                .visible_event()
                .expect("same visible event")
                .event_id()
                .as_str(),
            food.id_str()
        );
        assert!(unchanged.retracted_event().is_none());
        assert_eq!(
            unchanged
                .cause_event()
                .expect("deletion cause")
                .event()
                .event_id()
                .as_str(),
            wrong_author_deletion.id_str()
        );
    }

    #[tokio::test]
    async fn event_visibility_batches_validate_before_read_and_preserve_duplicate_order() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};

        let store = RadrootsEventStore::open_memory().await.expect("open");
        let event = signed_event(KIND_POST, 100, Vec::new(), "Victoria harvest update");
        store
            .ingest_event(RadrootsEventIngest::new(event.clone(), 36_000))
            .await
            .expect("event ingest");
        let missing_event_id = "f".repeat(64);
        let evaluation_count = Arc::new(AtomicUsize::new(0));
        let probe_count = Arc::clone(&evaluation_count);
        let visibilities = store
            .event_visibilities_with_probe(
                [
                    event.id_str().to_owned(),
                    missing_event_id.clone(),
                    event.id_str().to_owned(),
                ],
                move |_| {
                    let probe_count = Arc::clone(&probe_count);
                    async move {
                        probe_count.fetch_add(1, Ordering::SeqCst);
                        Ok(())
                    }
                },
            )
            .await
            .expect("visibility batch");
        assert_eq!(
            visibilities,
            vec![
                Some(RadrootsEventVisibility::Visible),
                None,
                Some(RadrootsEventVisibility::Visible),
            ]
        );
        assert_eq!(evaluation_count.load(Ordering::SeqCst), 2);

        let max = RADROOTS_EVENT_STORE_QUERY_LIMIT_MAX as usize;
        let exact_max = store
            .event_visibilities(vec![event.id_str().to_owned(); max])
            .await
            .expect("maximum visibility batch");
        assert_eq!(exact_max.len(), max);
        assert!(
            exact_max
                .iter()
                .all(|visibility| *visibility == Some(RadrootsEventVisibility::Visible))
        );

        let closed = RadrootsEventStore::open_memory()
            .await
            .expect("closed fixture");
        closed.pool().close().await;
        assert!(
            closed
                .event_visibilities(Vec::<String>::new())
                .await
                .expect("empty batch does not open a transaction")
                .is_empty()
        );
        assert!(matches!(
            closed
                .event_visibilities([event.id_str().to_owned(), "not-an-event-id".to_owned(),])
                .await,
            Err(RadrootsEventStoreError::IdParse(_))
        ));
        assert!(matches!(
            closed
                .event_visibilities(vec![event.id_str().to_owned(); max + 1])
                .await,
            Err(RadrootsEventStoreError::EventVisibilityBatchTooLarge {
                max: actual_max,
            }) if actual_max == max
        ));
    }

    #[tokio::test]
    async fn event_visibility_batch_holds_one_snapshot_across_concurrent_head_commits() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let path = tempdir.path().join("visibility-batch-snapshot.sqlite");
        let pool = SqlitePoolOptions::new()
            .max_connections(2)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(path)
                    .create_if_missing(true),
            )
            .await
            .expect("file pool");
        let store = RadrootsEventStore::open_pool(pool, true)
            .await
            .expect("file store");

        let old_alice = signed_event_with_keys(
            &fixture_keys(),
            KIND_PROFILE,
            100,
            Vec::new(),
            r#"{"name":"Alice"}"#,
        );
        let new_alice = signed_event_with_keys(
            &fixture_keys(),
            KIND_PROFILE,
            200,
            Vec::new(),
            r#"{"name":"Alice Farm"}"#,
        );
        let old_bob = signed_event_with_keys(
            &alternate_keys(),
            KIND_PROFILE,
            100,
            Vec::new(),
            r#"{"name":"Bob"}"#,
        );
        let new_bob = signed_event_with_keys(
            &alternate_keys(),
            KIND_PROFILE,
            200,
            Vec::new(),
            r#"{"name":"Bob Farm"}"#,
        );
        for (observed_at_ms, event) in [(36_100, &old_alice), (36_101, &old_bob)] {
            store
                .ingest_event(RadrootsEventIngest::new(event.clone(), observed_at_ms))
                .await
                .expect("old profile ingest");
        }

        let concurrent_store = store.clone();
        let committed_new_alice = new_alice.clone();
        let committed_new_bob = new_bob.clone();
        let snapshot = store
            .event_visibilities_with_probe(
                [old_alice.id_str(), old_bob.id_str()],
                move |evaluated| {
                    let concurrent_store = concurrent_store.clone();
                    let new_alice = committed_new_alice.clone();
                    let new_bob = committed_new_bob.clone();
                    async move {
                        if evaluated == 1 {
                            concurrent_store
                                .ingest_event(RadrootsEventIngest::new(new_alice, 36_200))
                                .await?;
                            concurrent_store
                                .ingest_event(RadrootsEventIngest::new(new_bob, 36_201))
                                .await?;
                        }
                        Ok(())
                    }
                },
            )
            .await
            .expect("coherent visibility snapshot");
        assert_eq!(
            snapshot,
            vec![
                Some(RadrootsEventVisibility::Visible),
                Some(RadrootsEventVisibility::Visible),
            ]
        );
        for (old, new) in [(&old_alice, &new_alice), (&old_bob, &new_bob)] {
            assert_eq!(
                store
                    .event_visibility(old.id_str())
                    .await
                    .expect("post-commit visibility"),
                Some(RadrootsEventVisibility::NotCurrent {
                    raw_head_event_id: new.id_str().to_owned(),
                })
            );
        }
    }

    #[tokio::test]
    async fn current_visibility_agrees_for_regular_replaceable_and_addressable_reads() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        assert!(
            store
                .visible_event(event_id('f').as_str())
                .await
                .expect("missing visible event")
                .is_none()
        );
        assert!(
            store
                .visible_event_head(&profile_coordinate())
                .await
                .expect("missing visible head")
                .is_none()
        );
        let regular = signed_event(KIND_POST, 100, Vec::new(), "Victoria harvest update");
        let older_profile = signed_event(KIND_PROFILE, 110, Vec::new(), "{\"name\":\"Alice\"}");
        let newer_profile =
            signed_event(KIND_PROFILE, 120, Vec::new(), "{\"name\":\"Alice Farm\"}");
        let older_food = food_availability_event(
            130,
            "visibility-carrots",
            "Visibility Carrots",
            "First harvest",
            "active",
            Vec::new(),
        );
        let newer_food = food_availability_event(
            140,
            "visibility-carrots",
            "Visibility Carrots",
            "Second harvest",
            "sold",
            Vec::new(),
        );
        for (index, event) in [
            &regular,
            &older_profile,
            &newer_profile,
            &older_food,
            &newer_food,
        ]
        .into_iter()
        .enumerate()
        {
            store
                .ingest_event(RadrootsEventIngest::new(
                    event.clone(),
                    37_000 + i64::try_from(index).expect("index"),
                ))
                .await
                .expect("visibility fixture ingest");
        }

        let regular_current = store
            .current_event_visibility_v1(regular.id_str())
            .await
            .expect("regular current visibility")
            .expect("regular event");
        assert_eq!(
            regular_current.decision(),
            crate::RadrootsCurrentVisibilityDecisionV1::Visible
        );
        assert!(regular_current.is_raw_head());
        assert!(regular_current.raw_head_event_id().is_none());

        let mut incomplete_not_current = regular_current.clone();
        incomplete_not_current.decision = RadrootsCurrentVisibilityDecisionV1::NotCurrent;
        assert!(matches!(
            event_visibility_from_current(regular.id_str(), &incomplete_not_current),
            Err(RadrootsEventStoreError::StoredHeadCoordinateUnavailable { event_id })
                if event_id == regular.id_str()
        ));
        let mut incomplete_suppression = regular_current.clone();
        incomplete_suppression.decision = RadrootsCurrentVisibilityDecisionV1::Suppressed;
        incomplete_suppression.suppression = None;
        assert!(matches!(
            event_visibility_from_current(regular.id_str(), &incomplete_suppression),
            Err(RadrootsEventStoreError::CurrentVisibilityDrift { reason })
                if reason.contains("missing evidence")
        ));
        assert_eq!(
            store
                .event_visibility(regular.id_str())
                .await
                .expect("regular compatibility visibility"),
            Some(RadrootsEventVisibility::Visible)
        );
        assert!(
            store
                .visible_event(regular.id_str())
                .await
                .expect("regular visible event")
                .is_some()
        );

        for (event, expected_head, expected_decision) in [
            (
                &older_profile,
                newer_profile.id_str(),
                crate::RadrootsCurrentVisibilityDecisionV1::NotCurrent,
            ),
            (
                &newer_profile,
                newer_profile.id_str(),
                crate::RadrootsCurrentVisibilityDecisionV1::Visible,
            ),
            (
                &older_food,
                newer_food.id_str(),
                crate::RadrootsCurrentVisibilityDecisionV1::NotCurrent,
            ),
            (
                &newer_food,
                newer_food.id_str(),
                crate::RadrootsCurrentVisibilityDecisionV1::Visible,
            ),
        ] {
            let current = store
                .current_event_visibility_v1(event.id_str())
                .await
                .expect("coordinate current visibility")
                .expect("coordinate event");
            assert_eq!(current.decision(), expected_decision);
            assert_eq!(
                current
                    .raw_head_event_id()
                    .expect("coordinate raw head")
                    .as_str(),
                expected_head
            );
            assert_eq!(
                store
                    .visible_event(event.id_str())
                    .await
                    .expect("coordinate visible event")
                    .is_some(),
                expected_decision == crate::RadrootsCurrentVisibilityDecisionV1::Visible
            );
        }
        assert!(
            store
                .visible_event_head(&profile_coordinate())
                .await
                .expect("profile visible head")
                .is_some_and(|head| head.raw_head().event_id == newer_profile.id_str())
        );
        assert!(
            store
                .visible_event_head(&head_coordinate_for_event(&newer_food))
                .await
                .expect("food visible head")
                .is_some_and(|head| head.raw_head().event_id == newer_food.id_str())
        );

        let regular_deletion = deletion_event(
            &fixture_keys(),
            150,
            vec![vec!["e".to_owned(), regular.id_str().to_owned()]],
        );
        store
            .ingest_event(RadrootsEventIngest::new(regular_deletion.clone(), 37_005))
            .await
            .expect("regular deletion");
        assert_eq!(
            store
                .current_event_visibility_v1(regular.id_str())
                .await
                .expect("suppressed regular visibility")
                .expect("stored regular event")
                .decision(),
            crate::RadrootsCurrentVisibilityDecisionV1::Suppressed
        );
        assert!(matches!(
            store
                .event_visibility(regular.id_str())
                .await
                .expect("regular compatibility suppression"),
            Some(RadrootsEventVisibility::Suppressed {
                reason: crate::RadrootsNip09SuppressionReason::EventIdReference,
                event_reference_request_id: Some(request_id),
                address_reference_request_id: None,
                address_reference_cutoff: None,
            }) if request_id.as_str() == regular_deletion.id_str()
        ));
        assert!(
            store
                .visible_event(regular.id_str())
                .await
                .expect("suppressed regular event")
                .is_none()
        );

        let deletion_of_deletion = deletion_event(
            &fixture_keys(),
            160,
            vec![vec!["e".to_owned(), regular_deletion.id_str().to_owned()]],
        );
        store
            .ingest_event(RadrootsEventIngest::new(deletion_of_deletion, 37_006))
            .await
            .expect("deletion-of-deletion ingest");
        let immune = store
            .current_event_visibility_v1(regular_deletion.id_str())
            .await
            .expect("deletion request visibility")
            .expect("stored deletion request");
        assert_eq!(
            immune.decision(),
            crate::RadrootsCurrentVisibilityDecisionV1::Visible
        );
        let immune_evidence = immune.suppression().expect("kind-5 immunity evidence");
        assert_eq!(
            immune_evidence.reason(),
            crate::RadrootsNip09SuppressionReason::DeletionRequestImmune
        );
        assert!(immune_evidence.event_reference_request_id().is_none());
        assert!(immune_evidence.address_reference_request_id().is_none());
        assert!(immune_evidence.address_reference_cutoff().is_none());
        assert_eq!(
            store
                .event_visibility(regular_deletion.id_str())
                .await
                .expect("compatibility deletion-request visibility"),
            Some(RadrootsEventVisibility::Visible)
        );
    }

    #[tokio::test]
    async fn current_visibility_rejects_missing_active_authority() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let regular = signed_event(KIND_POST, 100, Vec::new(), "missing visibility authority");
        store
            .ingest_event(RadrootsEventIngest::new(regular.clone(), 37_100))
            .await
            .expect("regular ingest");

        let mut connection = store.pool().acquire().await.expect("trusted connection");
        sqlx::query("DROP TRIGGER radroots_event_store_source_state_delete_guard")
            .execute(&mut *connection)
            .await
            .expect("trusted source-state guard removal");
        sqlx::query("PRAGMA foreign_keys = OFF")
            .execute(&mut *connection)
            .await
            .expect("disable trusted foreign-key enforcement");
        sqlx::query("DELETE FROM radroots_event_store_source_state")
            .execute(&mut *connection)
            .await
            .expect("trusted active-authority corruption");
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&mut *connection)
            .await
            .expect("restore foreign-key enforcement");
        drop(connection);

        assert!(matches!(
            store.current_event_visibility_v1(regular.id_str()).await,
            Err(RadrootsEventStoreError::CurrentVisibilityDrift { reason })
                if reason.contains("has no current-visibility authority")
        ));
    }

    #[tokio::test]
    async fn current_visibility_rejects_addressable_head_projection_drift() {
        for (label, bypass_checks, mutation, expected_reason) in [
            (
                "negative stored cutoff",
                true,
                "UPDATE radroots_event_store_addressable_head_state SET address_reference_cutoff = -1",
                "stored address deletion cutoff is invalid",
            ),
            (
                "negative stored created-at",
                true,
                "UPDATE radroots_event_store_addressable_head_state SET raw_head_created_at = -1",
                "stored raw-head created-at is invalid",
            ),
            (
                "contract disagreement",
                false,
                "UPDATE radroots_event_store_addressable_head_state SET contract_id = 'radroots.event.invalid.v1'",
                "central visibility disagrees with addressable head state",
            ),
        ] {
            let (store, food_id) = suppressed_food_visibility_store().await;
            let mut connection = store.pool().acquire().await.expect("trusted connection");
            sqlx::query("DROP TRIGGER radroots_event_store_addressable_state_old_update_guard")
                .execute(&mut *connection)
                .await
                .expect("trusted addressable-state guard removal");
            if bypass_checks {
                sqlx::query("PRAGMA foreign_keys = OFF")
                    .execute(&mut *connection)
                    .await
                    .expect("disable trusted foreign-key enforcement");
                sqlx::query("PRAGMA ignore_check_constraints = ON")
                    .execute(&mut *connection)
                    .await
                    .expect("enable trusted check-constraint bypass");
            }
            sqlx::query(mutation)
                .execute(&mut *connection)
                .await
                .expect("trusted addressable-state corruption");
            if bypass_checks {
                sqlx::query("PRAGMA ignore_check_constraints = OFF")
                    .execute(&mut *connection)
                    .await
                    .expect("restore check-constraint enforcement");
                sqlx::query("PRAGMA foreign_keys = ON")
                    .execute(&mut *connection)
                    .await
                    .expect("restore foreign-key enforcement");
            }
            drop(connection);

            let error = store
                .current_event_visibility_v1(food_id.as_str())
                .await
                .expect_err("corrupt addressable head projection must fail visibility read");
            assert!(
                matches!(
                    error,
                    RadrootsEventStoreError::CurrentVisibilityDrift { ref reason }
                        if reason.contains(expected_reason)
                ),
                "{label}: {error}",
            );
        }
    }

    #[tokio::test]
    async fn addressable_transition_feed_advances_across_unrelated_kinds() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let first = food_availability_event(
            200,
            "first-food",
            "First Food",
            "First harvest",
            "active",
            Vec::new(),
        );
        let unrelated_first = signed_event(
            30_340,
            201,
            vec![vec!["d".to_owned(), "unrelated".to_owned()]],
            "unrelated addressable state",
        );
        let unrelated_second = signed_event(
            30_340,
            202,
            vec![vec!["d".to_owned(), "unrelated".to_owned()]],
            "new unrelated addressable state",
        );
        let second = food_availability_event(
            203,
            "second-food",
            "Second Food",
            "Second harvest",
            "active",
            Vec::new(),
        );
        for (index, event) in [&first, &unrelated_first, &unrelated_second, &second]
            .into_iter()
            .enumerate()
        {
            store
                .ingest_event(RadrootsEventIngest::new(
                    event.clone(),
                    40_000 + i64::try_from(index).expect("index"),
                ))
                .await
                .expect("addressable ingest");
        }

        let scope = crate::RadrootsAddressableTransitionScopeV1::food_availability();
        let first_page = store
            .addressable_transition_page_v1(&scope, None, 1)
            .await
            .expect("first scoped page");
        assert_eq!(first_page.transitions().len(), 1);
        assert_eq!(
            first_page.transitions()[0]
                .visible_event()
                .expect("first visible event")
                .event_id()
                .as_str(),
            first.id_str()
        );
        assert_eq!(first_page.source_high_water(), 4);
        assert_eq!(first_page.next_cursor().last_transition_seq(), 3);
        assert!(first_page.has_more());

        let second_page = store
            .addressable_transition_page_v1(&scope, Some(first_page.next_cursor()), 1)
            .await
            .expect("second scoped page");
        assert_eq!(second_page.transitions().len(), 1);
        assert_eq!(
            second_page.transitions()[0]
                .visible_event()
                .expect("second visible event")
                .event_id()
                .as_str(),
            second.id_str()
        );
        assert_eq!(second_page.next_cursor().last_transition_seq(), 4);
        assert!(!second_page.has_more());

        let unrelated_scope =
            crate::RadrootsAddressableTransitionScopeV1::new([30_340]).expect("scope");
        assert!(matches!(
            store
                .addressable_transition_page_v1(
                    &unrelated_scope,
                    Some(first_page.next_cursor()),
                    1,
                )
                .await,
            Err(RadrootsEventStoreError::AddressableTransitionScopeMismatch)
        ));
        let generation_mismatch = crate::RadrootsAddressableTransitionCursorV1::new(
            crate::RadrootsEventStoreSourceGeneration::from_bytes([0xff; 32]),
            scope.fingerprint(),
            3,
        )
        .expect("generation-mismatched cursor");
        assert!(matches!(
            store
                .addressable_transition_page_v1(&scope, Some(&generation_mismatch), 1)
                .await,
            Err(RadrootsEventStoreError::AddressableTransitionSourceGenerationMismatch)
        ));
        let ahead = crate::RadrootsAddressableTransitionCursorV1::new(
            first_page.next_cursor().source_generation(),
            scope.fingerprint(),
            5,
        )
        .expect("ahead cursor");
        assert!(matches!(
            store
                .addressable_transition_page_v1(&scope, Some(&ahead), 1)
                .await,
            Err(RadrootsEventStoreError::AddressableTransitionCursorAhead {
                cursor: 5,
                high_water: 4,
            })
        ));
    }

    #[tokio::test]
    async fn addressable_transition_feed_pages_at_the_exact_transition_limit() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let mut expected_ids = Vec::new();
        let mut transaction = store.begin_write_transaction().await.expect("transaction");
        for index in 0..=crate::RADROOTS_ADDRESSABLE_TRANSITION_PAGE_LIMIT_MAX_V1 {
            let event = signed_event(
                30_340,
                500 + index,
                vec![vec!["d".to_owned(), format!("page-limit-{index:02}")]],
                "page-limit fixture",
            );
            expected_ids.push(event.id_str().to_owned());
            store
                .ingest_event_in_transaction(
                    &mut transaction,
                    RadrootsEventIngest::new(event, 45_100 + i64::from(index)),
                )
                .await
                .expect("scoped transition ingest");
        }
        transaction.commit().await.expect("commit fixtures");

        let scope = crate::RadrootsAddressableTransitionScopeV1::new([30_340]).expect("scope");
        let first = store
            .addressable_transition_page_v1(
                &scope,
                None,
                crate::RADROOTS_ADDRESSABLE_TRANSITION_PAGE_LIMIT_MAX_V1,
            )
            .await
            .expect("full first page");
        assert_eq!(
            first.transitions().len(),
            usize::try_from(crate::RADROOTS_ADDRESSABLE_TRANSITION_PAGE_LIMIT_MAX_V1)
                .expect("page limit"),
        );
        assert!(first.has_more());
        assert_eq!(
            first
                .transitions()
                .iter()
                .map(|transition| transition.raw_head().event_id().as_str())
                .collect::<Vec<_>>(),
            expected_ids[..usize::try_from(
                crate::RADROOTS_ADDRESSABLE_TRANSITION_PAGE_LIMIT_MAX_V1
            )
            .expect("page limit")],
        );

        let second = store
            .addressable_transition_page_v1(
                &scope,
                Some(first.next_cursor()),
                crate::RADROOTS_ADDRESSABLE_TRANSITION_PAGE_LIMIT_MAX_V1,
            )
            .await
            .expect("continued page");
        assert_eq!(second.transitions().len(), 1);
        assert_eq!(
            second.transitions()[0].raw_head().event_id().as_str(),
            expected_ids.last().expect("last expected event"),
        );
        assert!(!second.has_more());
    }

    #[tokio::test]
    async fn addressable_transition_feed_preserves_the_maximum_opaque_d_tag() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let d_tag = "d".repeat(crate::RADROOTS_ADDRESSABLE_TRANSITION_D_TAG_MAX_BYTES_V1);
        let event = signed_event(
            30_340,
            204,
            vec![vec!["d".to_owned(), d_tag.clone()]],
            "maximum d-tag boundary",
        );
        store
            .ingest_event(RadrootsEventIngest::new(event.clone(), 45_000))
            .await
            .expect("maximum d-tag ingest");

        let scope = crate::RadrootsAddressableTransitionScopeV1::new([30_340]).expect("scope");
        let page = store
            .addressable_transition_page_v1(&scope, None, 1)
            .await
            .expect("maximum d-tag feed");
        assert_eq!(page.transitions().len(), 1);
        let transition = &page.transitions()[0];
        assert_eq!(transition.coordinate().kind(), 30_340);
        assert_eq!(transition.coordinate().d_tag(), d_tag);
        assert_eq!(transition.raw_head().event_id().as_str(), event.id_str());
        assert_eq!(transition.raw_head_created_at(), 204);
        assert!(!page.has_more());
    }

    #[tokio::test]
    async fn addressable_transition_feed_scan_boundary_does_not_skip_scoped_events() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let mut transaction = store.begin_write_transaction().await.expect("transaction");
        for index in 0..crate::RADROOTS_ADDRESSABLE_TRANSITION_PAGE_SCAN_MAX_V1 {
            let unrelated = signed_event(
                30_340,
                1_000 + index,
                vec![vec!["d".to_owned(), "scan-boundary".to_owned()]],
                "unrelated addressable transition",
            );
            store
                .ingest_event_in_transaction(
                    &mut transaction,
                    RadrootsEventIngest::new(unrelated, 46_000 + i64::from(index)),
                )
                .await
                .expect("unrelated transition ingest");
        }
        let food = food_availability_event(
            3_000,
            "scan-boundary-food",
            "Scan Boundary Carrots",
            "Scoped transition after unrelated traffic",
            "active",
            Vec::new(),
        );
        store
            .ingest_event_in_transaction(
                &mut transaction,
                RadrootsEventIngest::new(food.clone(), 47_025),
            )
            .await
            .expect("scoped transition ingest");
        transaction.commit().await.expect("commit fixtures");

        let scope = crate::RadrootsAddressableTransitionScopeV1::food_availability();
        let first = store
            .addressable_transition_page_v1(&scope, None, 64)
            .await
            .expect("first scan page");
        assert!(first.transitions().is_empty());
        assert_eq!(
            first.next_cursor().last_transition_seq(),
            i64::from(crate::RADROOTS_ADDRESSABLE_TRANSITION_PAGE_SCAN_MAX_V1)
        );
        assert_eq!(
            first.source_high_water(),
            i64::from(crate::RADROOTS_ADDRESSABLE_TRANSITION_PAGE_SCAN_MAX_V1) + 1
        );
        assert!(first.has_more());

        let second = store
            .addressable_transition_page_v1(&scope, Some(first.next_cursor()), 64)
            .await
            .expect("second scan page");
        assert_eq!(second.transitions().len(), 1);
        assert_eq!(
            second.transitions()[0]
                .visible_event()
                .expect("FoodAvailability canonical event")
                .event_id()
                .as_str(),
            food.id_str()
        );
        assert_eq!(second.next_cursor().last_transition_seq(), 1_025);
        assert!(!second.has_more());
    }

    #[tokio::test]
    async fn addressable_transition_feed_payload_cap_continues_without_loss() {
        const EVENT_COUNT: u32 = 33;

        let store = RadrootsEventStore::open_memory().await.expect("open");
        let content = "x".repeat(radroots_event::wire::v1::DEFAULT_CONTENT_MAX_BYTES);
        let mut expected_ids = Vec::new();
        let mut transaction = store.begin_write_transaction().await.expect("transaction");
        for index in 0..EVENT_COUNT {
            let event = calendar_date_event(
                4_000 + index,
                format!("payload-cap-{index:02}").as_str(),
                content.clone(),
            );
            expected_ids.push(event.id_str().to_owned());
            store
                .ingest_event_in_transaction(
                    &mut transaction,
                    RadrootsEventIngest::new(event, 48_000 + i64::from(index)),
                )
                .await
                .expect("calendar transition ingest");
        }
        transaction.commit().await.expect("commit fixtures");

        let scope = crate::RadrootsAddressableTransitionScopeV1::new([KIND_CALENDAR_DATE_EVENT])
            .expect("calendar scope");
        let mut cursor = None;
        let mut actual_ids = Vec::new();
        let mut page_count = 0_u32;
        loop {
            let page = store
                .addressable_transition_page_v1(&scope, cursor.as_ref(), 64)
                .await
                .expect("payload-bounded page");
            page_count += 1;
            assert!(!page.transitions().is_empty());
            for transition in page.transitions() {
                actual_ids.push(
                    transition
                        .visible_event()
                        .expect("admitted calendar canonical event")
                        .event_id()
                        .as_str()
                        .to_owned(),
                );
            }
            cursor = Some(page.next_cursor().clone());
            if !page.has_more() {
                break;
            }
        }
        assert!(page_count > 1, "payload cap must force continuation");
        assert_eq!(actual_ids, expected_ids);
        assert_eq!(
            cursor.expect("terminal cursor").last_transition_seq(),
            i64::from(EVENT_COUNT)
        );
    }

    #[tokio::test]
    async fn addressable_transition_feed_distinguishes_gaps_from_corruption() {
        let gap_store = RadrootsEventStore::open_memory().await.expect("gap store");
        let first = food_availability_event(
            200,
            "gap-first",
            "Gap First",
            "First row",
            "active",
            Vec::new(),
        );
        let unrelated = signed_event(
            30_340,
            201,
            vec![vec!["d".to_owned(), "gap-middle".to_owned()]],
            "middle row",
        );
        let second = food_availability_event(
            202,
            "gap-second",
            "Gap Second",
            "Last row",
            "active",
            Vec::new(),
        );
        for event in [&first, &unrelated, &second] {
            gap_store
                .ingest_event(RadrootsEventIngest::new(event.clone(), 50_000))
                .await
                .expect("gap fixture ingest");
        }
        sqlx::query("DROP TRIGGER radroots_event_store_addressable_transition_delete_guard")
            .execute(gap_store.pool())
            .await
            .expect("drop test-only delete guard");
        sqlx::query(
            "DELETE FROM radroots_event_store_addressable_head_transition WHERE transition_seq = 2",
        )
        .execute(gap_store.pool())
        .await
        .expect("remove middle transition");
        let scope = crate::RadrootsAddressableTransitionScopeV1::food_availability();
        assert!(matches!(
            gap_store
                .addressable_transition_page_v1(&scope, None, 64)
                .await,
            Err(RadrootsEventStoreError::AddressableTransitionSequenceGap { .. })
        ));

        let corrupt_store = RadrootsEventStore::open_memory()
            .await
            .expect("corrupt store");
        corrupt_store
            .ingest_event(RadrootsEventIngest::new(first, 50_001))
            .await
            .expect("corruption fixture ingest");
        sqlx::query("DROP TRIGGER radroots_event_store_addressable_transition_update_guard")
            .execute(corrupt_store.pool())
            .await
            .expect("drop test-only update guard");
        sqlx::query(
            "UPDATE radroots_event_store_addressable_head_transition SET raw_head_created_at = raw_head_created_at + 1 WHERE transition_seq = 1",
        )
        .execute(corrupt_store.pool())
        .await
        .expect("corrupt transition metadata");
        assert!(matches!(
            corrupt_store
                .addressable_transition_page_v1(&scope, None, 64)
                .await,
            Err(RadrootsEventStoreError::AddressableTransitionCorruption { .. })
        ));
    }

    #[tokio::test]
    async fn addressable_transition_feed_rejects_row_authority_drift() {
        for (label, mutation) in [
            (
                "origin enum",
                "UPDATE radroots_event_store_addressable_head_transition SET origin = 'invalid'",
            ),
            (
                "public key",
                "UPDATE radroots_event_store_addressable_head_transition SET pubkey = 'invalid'",
            ),
            (
                "coordinate bounds",
                "UPDATE radroots_event_store_addressable_head_transition SET d_tag = replace(hex(zeroblob(513)), '00', 'x')",
            ),
            (
                "negative kind",
                "UPDATE radroots_event_store_addressable_head_transition SET kind = -1",
            ),
            (
                "kind above u32",
                "UPDATE radroots_event_store_addressable_head_transition SET kind = 4294967296",
            ),
            (
                "raw-head sequence",
                "UPDATE radroots_event_store_addressable_head_transition SET raw_head_event_seq = 0",
            ),
            (
                "partial visible identity",
                "UPDATE radroots_event_store_addressable_head_transition SET visible_event_seq = NULL",
            ),
            (
                "partial retracted identity",
                "UPDATE radroots_event_store_addressable_head_transition SET retracted_event_id = raw_head_event_id",
            ),
            (
                "partial cause identity",
                "UPDATE radroots_event_store_addressable_head_transition SET cause_event_seq = NULL",
            ),
            (
                "admission enum",
                "UPDATE radroots_event_store_addressable_head_transition SET admission_status = 'invalid'",
            ),
            (
                "visibility enum",
                "UPDATE radroots_event_store_addressable_head_transition SET visibility = 'invalid'",
            ),
            (
                "suppression evidence",
                "UPDATE radroots_event_store_addressable_head_transition SET nip09_reason = NULL",
            ),
            (
                "event suppression request id",
                "UPDATE radroots_event_store_addressable_head_transition SET event_reference_request_id = 'invalid'",
            ),
            (
                "address suppression request id",
                "UPDATE radroots_event_store_addressable_head_transition SET address_reference_request_id = 'invalid'",
            ),
            (
                "raw-head decision enum",
                "UPDATE radroots_event_store_addressable_head_transition SET raw_head_decision = 'invalid'",
            ),
            (
                "admission identity",
                "UPDATE radroots_event_store_addressable_head_transition SET contract_id = 'radroots.event.invalid.v1'",
            ),
            (
                "visible identity",
                "UPDATE radroots_event_store_addressable_head_transition SET visible_event_id = '0000000000000000000000000000000000000000000000000000000000000000'",
            ),
            (
                "missing raw event",
                "UPDATE radroots_event_store_addressable_head_transition SET raw_head_event_id = '0000000000000000000000000000000000000000000000000000000000000000', visible_event_id = '0000000000000000000000000000000000000000000000000000000000000000'",
            ),
        ] {
            let store = food_availability_audit_corruption_store().await;
            let mut connection = store.pool().acquire().await.expect("trusted connection");
            sqlx::query("DROP TRIGGER radroots_event_store_addressable_transition_update_guard")
                .execute(&mut *connection)
                .await
                .expect("trusted transition guard removal");
            sqlx::query("PRAGMA foreign_keys = OFF")
                .execute(&mut *connection)
                .await
                .expect("disable trusted foreign-key enforcement");
            sqlx::query("PRAGMA ignore_check_constraints = ON")
                .execute(&mut *connection)
                .await
                .expect("enable trusted check-constraint bypass");
            sqlx::query(mutation)
                .execute(&mut *connection)
                .await
                .expect("trusted transition corruption");
            sqlx::query("PRAGMA ignore_check_constraints = OFF")
                .execute(&mut *connection)
                .await
                .expect("restore check-constraint enforcement");
            sqlx::query("PRAGMA foreign_keys = ON")
                .execute(&mut *connection)
                .await
                .expect("restore foreign-key enforcement");
            drop(connection);

            let scope = crate::RadrootsAddressableTransitionScopeV1::food_availability();
            let error = store
                .addressable_transition_page_v1(&scope, None, 64)
                .await
                .expect_err("corrupt transition row must fail public feed read");
            assert!(
                matches!(
                    error,
                    RadrootsEventStoreError::AddressableTransitionCorruption { .. }
                ),
                "{label}: {error}",
            );
        }
    }

    #[tokio::test]
    async fn addressable_transition_feed_rejects_valid_but_wrong_coordinate_authority() {
        let store = food_availability_audit_corruption_store().await;
        let mut connection = store.pool().acquire().await.expect("trusted connection");
        sqlx::query("DROP TRIGGER radroots_event_store_addressable_transition_update_guard")
            .execute(&mut *connection)
            .await
            .expect("trusted transition guard removal");
        sqlx::query("PRAGMA foreign_keys = OFF")
            .execute(&mut *connection)
            .await
            .expect("disable trusted foreign-key enforcement");
        let other_pubkey = alternate_keys().public_key().to_hex();
        sqlx::query(
            "UPDATE radroots_event_store_addressable_head_transition SET pubkey = ? WHERE transition_seq = 1",
        )
        .bind(other_pubkey)
        .execute(&mut *connection)
        .await
        .expect("trusted coordinate-authority corruption");
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&mut *connection)
            .await
            .expect("restore foreign-key enforcement");
        drop(connection);

        let scope = crate::RadrootsAddressableTransitionScopeV1::food_availability();
        let error = store
            .addressable_transition_page_v1(&scope, None, 64)
            .await
            .expect_err("wrong coordinate authority must fail public feed read");
        assert!(matches!(
            error,
            RadrootsEventStoreError::AddressableTransitionCorruption { ref reason }
                if reason.contains("does not match transition coordinate")
        ));
    }

    #[tokio::test]
    async fn addressable_transition_feed_rejects_source_authority_drift() {
        for (label, mutations, expected_class) in [
            (
                "missing source state",
                &["DELETE FROM radroots_event_store_source_state"][..],
                "corruption",
            ),
            (
                "invalid generation",
                &[
                    "UPDATE radroots_event_store_source_generation SET source_generation = zeroblob(31)",
                    "UPDATE radroots_event_store_addressable_feed_integrity_v1 SET source_generation = zeroblob(31)",
                    "UPDATE radroots_event_store_source_state SET active_generation = zeroblob(31)",
                ][..],
                "corruption",
            ),
            (
                "unsupported feed version",
                &["UPDATE radroots_event_store_source_generation SET addressable_feed_version = 2"]
                    [..],
                "version",
            ),
            (
                "negative transition floor",
                &["UPDATE radroots_event_store_source_generation SET transition_floor_seq = -1"][..],
                "corruption",
            ),
            (
                "high-water before floor",
                &["UPDATE radroots_event_store_source_state SET last_transition_seq = -1"][..],
                "corruption",
            ),
            (
                "integrity seal mismatch",
                &[
                    "UPDATE radroots_event_store_addressable_feed_integrity_v1 SET last_transition_seq = last_transition_seq + 1, transition_count = transition_count + 1",
                ][..],
                "gap",
            ),
            (
                "missing boundary transition",
                &["DELETE FROM radroots_event_store_addressable_head_transition"][..],
                "gap",
            ),
        ] {
            let store = food_availability_audit_corruption_store().await;
            let mut connection = store.pool().acquire().await.expect("trusted connection");
            for guard in [
                "DROP TRIGGER radroots_event_store_source_state_active_generation_guard",
                "DROP TRIGGER radroots_event_store_source_state_authority_update_guard",
                "DROP TRIGGER radroots_event_store_source_state_delete_guard",
                "DROP TRIGGER radroots_event_store_source_generation_update_guard",
                "DROP TRIGGER radroots_event_store_addressable_transition_delete_guard",
            ] {
                sqlx::query(guard)
                    .execute(&mut *connection)
                    .await
                    .expect("trusted feed-authority guard removal");
            }
            sqlx::query("PRAGMA foreign_keys = OFF")
                .execute(&mut *connection)
                .await
                .expect("disable trusted foreign-key enforcement");
            sqlx::query("PRAGMA ignore_check_constraints = ON")
                .execute(&mut *connection)
                .await
                .expect("enable trusted check-constraint bypass");
            for mutation in mutations {
                sqlx::query(*mutation)
                    .execute(&mut *connection)
                    .await
                    .expect("trusted feed-authority corruption");
            }
            sqlx::query("PRAGMA ignore_check_constraints = OFF")
                .execute(&mut *connection)
                .await
                .expect("restore check-constraint enforcement");
            sqlx::query("PRAGMA foreign_keys = ON")
                .execute(&mut *connection)
                .await
                .expect("restore foreign-key enforcement");
            drop(connection);

            let scope = crate::RadrootsAddressableTransitionScopeV1::food_availability();
            let error = store
                .addressable_transition_page_v1(&scope, None, 64)
                .await
                .expect_err("corrupt source authority must fail public feed read");
            let actual_class = match &error {
                RadrootsEventStoreError::AddressableTransitionCorruption { .. } => "corruption",
                RadrootsEventStoreError::AddressableTransitionFeedVersionMismatch { .. } => {
                    "version"
                }
                RadrootsEventStoreError::AddressableTransitionSequenceGap { .. } => "gap",
                _ => "unexpected",
            };
            assert_eq!(actual_class, expected_class, "{label}: {error}");
        }
    }

    #[tokio::test]
    async fn addressable_transition_feed_rejects_expired_and_missing_cursors() {
        let expired_store = food_availability_audit_corruption_store().await;
        let scope = crate::RadrootsAddressableTransitionScopeV1::food_availability();
        let current = expired_store
            .addressable_transition_page_v1(&scope, None, 64)
            .await
            .expect("current transition page");
        let expired = crate::RadrootsAddressableTransitionCursorV1::new(
            current.next_cursor().source_generation(),
            scope.fingerprint(),
            0,
        )
        .expect("expired cursor fixture");
        let mut connection = expired_store
            .pool()
            .acquire()
            .await
            .expect("trusted connection");
        sqlx::query("DROP TRIGGER radroots_event_store_source_generation_update_guard")
            .execute(&mut *connection)
            .await
            .expect("trusted generation guard removal");
        sqlx::query("PRAGMA ignore_check_constraints = ON")
            .execute(&mut *connection)
            .await
            .expect("enable trusted check-constraint bypass");
        sqlx::query("UPDATE radroots_event_store_source_generation SET transition_floor_seq = 1")
            .execute(&mut *connection)
            .await
            .expect("advance trusted generation floor");
        sqlx::query(
            "UPDATE radroots_event_store_addressable_feed_integrity_v1 SET transition_floor_seq = 1, last_transition_seq = 1, transition_count = 0",
        )
        .execute(&mut *connection)
        .await
        .expect("advance trusted integrity floor");
        sqlx::query("PRAGMA ignore_check_constraints = OFF")
            .execute(&mut *connection)
            .await
            .expect("restore check-constraint enforcement");
        drop(connection);
        assert!(matches!(
            expired_store
                .addressable_transition_page_v1(&scope, Some(&expired), 64)
                .await,
            Err(
                RadrootsEventStoreError::AddressableTransitionCursorExpired {
                    cursor: 0,
                    floor: 1,
                }
            )
        ));

        let missing_store = RadrootsEventStore::open_memory().await.expect("open");
        let first = food_availability_event(
            300,
            "missing-cursor-first",
            "Missing Cursor Carrots",
            "First cursor authority fixture",
            "active",
            Vec::new(),
        );
        let unrelated = signed_event(
            30_340,
            301,
            vec![vec!["d".to_owned(), "missing-cursor-middle".to_owned()]],
            "unrelated cursor authority fixture",
        );
        let third = food_availability_event(
            302,
            "missing-cursor-third",
            "Missing Cursor Carrots",
            "Third cursor authority fixture",
            "active",
            Vec::new(),
        );
        for (index, event) in [first, unrelated, third].into_iter().enumerate() {
            missing_store
                .ingest_event(RadrootsEventIngest::new(
                    event,
                    19_200 + i64::try_from(index).expect("fixture index"),
                ))
                .await
                .expect("missing-cursor fixture ingest");
        }
        let current = missing_store
            .addressable_transition_page_v1(&scope, None, 2)
            .await
            .expect("current cursor page");
        let middle = crate::RadrootsAddressableTransitionCursorV1::new(
            current.next_cursor().source_generation(),
            scope.fingerprint(),
            2,
        )
        .expect("middle cursor");
        sqlx::query("DROP TRIGGER radroots_event_store_addressable_transition_delete_guard")
            .execute(missing_store.pool())
            .await
            .expect("trusted transition delete guard removal");
        sqlx::query(
            "DELETE FROM radroots_event_store_addressable_head_transition WHERE transition_seq = 2",
        )
        .execute(missing_store.pool())
        .await
        .expect("trusted cursor transition deletion");
        assert!(matches!(
            missing_store
                .addressable_transition_page_v1(&scope, Some(&middle), 64)
                .await,
            Err(RadrootsEventStoreError::AddressableTransitionSequenceGap { reason })
                if reason.contains("cursor sequence 2 is absent")
        ));
    }

    #[tokio::test]
    async fn addressable_transition_feed_rejects_incremental_cause_and_lineage_drift() {
        for (label, mutation) in [
            (
                "applied cause identity",
                "UPDATE radroots_event_store_addressable_head_transition SET cause_event_id = (SELECT event_id FROM event_envelopes WHERE kind = 1), cause_event_seq = (SELECT seq FROM event_envelopes WHERE kind = 1)",
            ),
            (
                "non-deletion non-head cause",
                "UPDATE radroots_event_store_addressable_head_transition SET raw_head_decision = 'not_head_selected'",
            ),
            (
                "illegal incremental decision",
                "UPDATE radroots_event_store_addressable_head_transition SET raw_head_decision = 'skipped_older'",
            ),
        ] {
            let store = transition_cause_corruption_store().await;
            let error =
                transition_feed_error_after_trusted_corruption(&store, &[], &[mutation]).await;
            assert!(
                matches!(
                    error,
                    RadrootsEventStoreError::AddressableTransitionCorruption { .. }
                ),
                "{label}: {error}",
            );
        }

        let (evidence_store, _) = suppressed_food_visibility_store().await;
        let evidence_error = transition_feed_error_after_trusted_corruption(
            &evidence_store,
            &[],
            &[
                "UPDATE radroots_event_store_addressable_head_transition SET visibility = 'visible', visible_event_id = raw_head_event_id, visible_event_seq = raw_head_event_seq, retracted_event_id = NULL, retracted_event_seq = NULL, nip09_outcome = 'visible', nip09_reason = 'deletion_request_author_mismatch', event_reference_request_id = NULL, address_reference_request_id = NULL, address_reference_cutoff = NULL WHERE transition_seq = 2",
            ],
        )
        .await;
        assert!(matches!(
            evidence_error,
            RadrootsEventStoreError::AddressableTransitionCorruption { ref reason }
                if reason.contains("author does not agree")
        ));

        let (target_store, _) = suppressed_food_visibility_store().await;
        let target_error = transition_feed_error_after_trusted_corruption(
            &target_store,
            &["DROP TRIGGER radroots_event_store_nip09_address_target_delete_guard"],
            &["DELETE FROM radroots_event_store_nip09_address_target"],
        )
        .await;
        assert!(matches!(
            target_error,
            RadrootsEventStoreError::AddressableTransitionCorruption { ref reason }
                if reason.contains("does not target the transitioned coordinate")
        ));

        for (label, mutation, expected_reason) in [
            (
                "missing retraction",
                "UPDATE radroots_event_store_addressable_head_transition SET retracted_event_id = NULL, retracted_event_seq = NULL WHERE transition_seq = 2",
                "retraction does not match",
            ),
            (
                "repeated prior state",
                "UPDATE radroots_event_store_addressable_head_transition SET raw_head_event_id = (SELECT raw_head_event_id FROM radroots_event_store_addressable_head_transition WHERE transition_seq = 1), raw_head_event_seq = (SELECT raw_head_event_seq FROM radroots_event_store_addressable_head_transition WHERE transition_seq = 1), raw_head_created_at = (SELECT raw_head_created_at FROM radroots_event_store_addressable_head_transition WHERE transition_seq = 1), visible_event_id = (SELECT visible_event_id FROM radroots_event_store_addressable_head_transition WHERE transition_seq = 1), visible_event_seq = (SELECT visible_event_seq FROM radroots_event_store_addressable_head_transition WHERE transition_seq = 1), retracted_event_id = NULL, retracted_event_seq = NULL, cause_event_id = (SELECT raw_head_event_id FROM radroots_event_store_addressable_head_transition WHERE transition_seq = 1), cause_event_seq = (SELECT raw_head_event_seq FROM radroots_event_store_addressable_head_transition WHERE transition_seq = 1) WHERE transition_seq = 2",
                "repeats the complete prior state",
            ),
            (
                "baseline after prior state",
                "UPDATE radroots_event_store_addressable_head_transition SET origin = 'baseline', retracted_event_id = NULL, retracted_event_seq = NULL, cause_event_id = NULL, cause_event_seq = NULL, raw_head_decision = 'baseline_rebuild' WHERE transition_seq = 2",
                "baseline transition follows existing coordinate state",
            ),
        ] {
            let store = replacement_transition_corruption_store().await;
            let error =
                transition_feed_error_after_trusted_corruption(&store, &[], &[mutation]).await;
            assert!(
                matches!(
                    error,
                    RadrootsEventStoreError::AddressableTransitionCorruption { ref reason }
                        if reason.contains(expected_reason)
                ),
                "{label}: {error}",
            );
        }
    }

    #[tokio::test]
    async fn addressable_transition_feed_rejects_corrupt_prior_reference_after_cursor() {
        let store = replacement_transition_corruption_store().await;
        let scope = crate::RadrootsAddressableTransitionScopeV1::food_availability();
        let first = store
            .addressable_transition_page_v1(&scope, None, 1)
            .await
            .expect("first transition page");
        let cursor = first.next_cursor().clone();
        assert_eq!(cursor.last_transition_seq(), 1);

        let mut connection = store.pool().acquire().await.expect("trusted connection");
        sqlx::query("DROP TRIGGER radroots_event_store_addressable_transition_update_guard")
            .execute(&mut *connection)
            .await
            .expect("trusted transition guard removal");
        sqlx::query("PRAGMA foreign_keys = OFF")
            .execute(&mut *connection)
            .await
            .expect("disable trusted foreign-key enforcement");
        sqlx::query("PRAGMA ignore_check_constraints = ON")
            .execute(&mut *connection)
            .await
            .expect("enable trusted check-constraint bypass");
        sqlx::query(
            "UPDATE radroots_event_store_addressable_head_transition SET raw_head_event_seq = 0 WHERE transition_seq = 1",
        )
        .execute(&mut *connection)
        .await
        .expect("trusted prior-reference corruption");
        sqlx::query("PRAGMA ignore_check_constraints = OFF")
            .execute(&mut *connection)
            .await
            .expect("restore check-constraint enforcement");
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&mut *connection)
            .await
            .expect("restore foreign-key enforcement");
        drop(connection);

        let error = store
            .addressable_transition_page_v1(&scope, Some(&cursor), 64)
            .await
            .expect_err("corrupt prior reference must fail the resumed public feed");
        assert!(matches!(
            error,
            RadrootsEventStoreError::AddressableTransitionCorruption { ref reason }
                if reason.contains("prior_raw_head sequence is not positive")
        ));
    }

    #[tokio::test]
    async fn addressable_transition_feed_rejects_stored_event_authority_drift() {
        for (label, store, guards, mutations, expected_reason) in [
            (
                "coordinate wire bound",
                food_availability_audit_corruption_store().await,
                &[][..],
                &[
                    "UPDATE radroots_event_store_addressable_head_transition SET d_tag = replace(hex(zeroblob(4097)), '00', 'x')",
                ][..],
                "coordinate is outside wire bounds",
            ),
            (
                "negative suppression cutoff",
                suppressed_food_visibility_store().await.0,
                &[][..],
                &[
                    "UPDATE radroots_event_store_addressable_head_transition SET address_reference_cutoff = -1 WHERE transition_seq = 2",
                ][..],
                "address_reference_cutoff",
            ),
            (
                "signed event fields",
                food_availability_audit_corruption_store().await,
                &["DROP TRIGGER radroots_event_store_event_envelopes_raw_update_guard"][..],
                &["UPDATE event_envelopes SET content = 'corrupt' WHERE kind = 30402"][..],
                "disagrees with its signed raw JSON",
            ),
            (
                "registry admission",
                food_availability_audit_corruption_store().await,
                &["DROP TRIGGER radroots_event_store_event_envelopes_derived_update_guard"][..],
                &[
                    "UPDATE event_envelopes SET contract_id = 'radroots.event.invalid.v1' WHERE kind = 30402",
                ][..],
                "disagrees with registry-v7 admission",
            ),
            (
                "coordinate authority",
                food_availability_audit_corruption_store().await,
                &["DROP TRIGGER radroots_event_store_event_coordinate_delete_guard"][..],
                &["DELETE FROM radroots_event_store_event_coordinate"][..],
                "has no matching addressable coordinate authority",
            ),
            (
                "non-admitted retraction",
                non_admitted_retraction_corruption_store().await,
                &[][..],
                &[
                    "UPDATE radroots_event_store_addressable_head_transition SET retracted_event_id = (SELECT raw_head_event_id FROM radroots_event_store_addressable_head_transition WHERE transition_seq = 1), retracted_event_seq = (SELECT raw_head_event_seq FROM radroots_event_store_addressable_head_transition WHERE transition_seq = 1) WHERE transition_seq = 2",
                ][..],
                "retracts an event that is not admitted",
            ),
        ] {
            let error =
                transition_feed_error_after_trusted_corruption(&store, guards, mutations).await;
            assert!(
                matches!(
                    error,
                    RadrootsEventStoreError::AddressableTransitionCorruption { ref reason }
                        if reason.contains(expected_reason)
                ),
                "{label}: {error}",
            );
        }
    }

    #[tokio::test]
    async fn raw_addressable_heads_use_the_first_opaque_d_value_or_empty() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let missing = signed_event(39_990, 30, Vec::new(), "missing");
        let missing_value = signed_event(
            39_990,
            31,
            vec![
                vec!["d".to_owned()],
                vec!["d".to_owned(), "ignored".to_owned()],
            ],
            "missing value",
        );
        let opaque = signed_event(
            39_991,
            32,
            vec![vec!["d".to_owned(), "  opaque/value  ".to_owned()]],
            "opaque",
        );
        let control = signed_event(
            39_992,
            33,
            vec![vec!["d".to_owned(), "line\nbreak".to_owned()]],
            "control",
        );

        for event in [&missing, &missing_value, &opaque, &control] {
            let receipt = store
                .ingest_event(RadrootsEventIngest::new(event.clone(), 3_000))
                .await
                .expect("ingest");
            assert_eq!(
                receipt.admission_status,
                RadrootsEventAdmissionStatus::Unsupported
            );
            assert_eq!(receipt.raw_head_decision, RadrootsRawHeadDecision::Applied);
        }

        let missing_coordinate = head_coordinate_for_event(&missing);
        assert_eq!(
            missing_coordinate,
            head_coordinate_for_event(&missing_value)
        );
        assert!(matches!(
            &missing_coordinate,
            RadrootsEventHeadCoordinate::Addressable { d_tag, .. } if d_tag.is_empty()
        ));
        assert_eq!(
            store
                .raw_event_head(&missing_coordinate)
                .await
                .expect("missing raw head")
                .expect("missing head")
                .event_id,
            missing_value.id_str()
        );
        for (event, expected_d) in [(&opaque, "  opaque/value  "), (&control, "line\nbreak")] {
            let coordinate = head_coordinate_for_event(event);
            assert!(matches!(
                &coordinate,
                RadrootsEventHeadCoordinate::Addressable { d_tag, .. } if d_tag == expected_d
            ));
            let head = store
                .raw_event_head(&coordinate)
                .await
                .expect("raw head")
                .expect("head");
            assert_eq!(head.event_id, event.id_str());
            assert_eq!(head.d_tag.as_deref(), Some(expected_d));
        }
        let missing_tags = store
            .tags_for_event(missing_value.id_str())
            .await
            .expect("tags");
        assert_eq!(missing_tags[0].tag_name, "d");
        assert_eq!(missing_tags[0].tag_value, None);
    }

    #[tokio::test]
    async fn database_guards_reject_raw_head_mutation() {
        let store = RadrootsEventStore::open_memory().await.expect("open");

        let kind_event = signed_event(10_001, 40, Vec::new(), "kind");
        store
            .ingest_event(RadrootsEventIngest::new(kind_event.clone(), 3_100))
            .await
            .expect("kind ingest");
        sqlx::query("UPDATE event_envelope_head SET kind = 10002 WHERE event_id = ?")
            .bind(kind_event.id_str())
            .execute(store.pool())
            .await
            .expect_err("kind mutation must be rejected");

        let author_event = signed_event(10_003, 41, Vec::new(), "author");
        store
            .ingest_event(RadrootsEventIngest::new(author_event.clone(), 3_101))
            .await
            .expect("author ingest");
        let other_pubkey = alternate_keys().public_key().to_hex();
        sqlx::query("UPDATE event_envelope_head SET pubkey = ? WHERE event_id = ?")
            .bind(other_pubkey.as_str())
            .bind(author_event.id_str())
            .execute(store.pool())
            .await
            .expect_err("author mutation must be rejected");

        let class_event = signed_event(10_004, 42, Vec::new(), "class");
        store
            .ingest_event(RadrootsEventIngest::new(class_event.clone(), 3_102))
            .await
            .expect("class ingest");
        sqlx::query(
            "UPDATE event_envelope_head SET coordinate_type = 'addressable', d_tag = 'wrong-class' WHERE event_id = ?",
        )
        .bind(class_event.id_str())
        .execute(store.pool())
        .await
        .expect_err("coordinate-class mutation must be rejected");

        let d_event = signed_event(
            39_980,
            43,
            vec![vec!["d".to_owned(), "actual".to_owned()]],
            "d",
        );
        store
            .ingest_event(RadrootsEventIngest::new(d_event.clone(), 3_103))
            .await
            .expect("d ingest");
        sqlx::query("UPDATE event_envelope_head SET d_tag = 'wrong-d' WHERE event_id = ?")
            .bind(d_event.id_str())
            .execute(store.pool())
            .await
            .expect_err("d-tag mutation must be rejected");

        let created_event = signed_event(10_005, 44, Vec::new(), "created");
        let created_coordinate = head_coordinate_for_event(&created_event);
        store
            .ingest_event(RadrootsEventIngest::new(created_event.clone(), 3_104))
            .await
            .expect("created ingest");
        sqlx::query(
            "UPDATE event_envelope_head SET created_at = created_at + 1 WHERE event_id = ?",
        )
        .bind(created_event.id_str())
        .execute(store.pool())
        .await
        .expect_err("created-at mutation must be rejected");
        assert_eq!(
            store
                .raw_event_head(&created_coordinate)
                .await
                .expect("raw head")
                .expect("stored head")
                .event_id,
            created_event.id_str()
        );

        let reference_event = signed_event(10_006, 45, Vec::new(), "reference");
        let reference_coordinate = head_coordinate_for_event(&reference_event);
        let unrelated_event = signed_event(998, 46, Vec::new(), "unrelated");
        store
            .ingest_event(RadrootsEventIngest::new(reference_event.clone(), 3_105))
            .await
            .expect("reference ingest");
        store
            .ingest_event(RadrootsEventIngest::new(unrelated_event.clone(), 3_106))
            .await
            .expect("unrelated ingest");
        sqlx::query("UPDATE event_envelope_head SET event_id = ? WHERE event_id = ?")
            .bind(unrelated_event.id_str())
            .bind(reference_event.id_str())
            .execute(store.pool())
            .await
            .expect_err("event reference mutation must be rejected");
        assert!(
            store
                .raw_event_head(&reference_coordinate)
                .await
                .expect("raw head")
                .is_some()
        );

        let missing_event = signed_event(10_007, 47, Vec::new(), "missing reference");
        let missing_coordinate = head_coordinate_for_event(&missing_event);
        store
            .ingest_event(RadrootsEventIngest::new(missing_event.clone(), 3_107))
            .await
            .expect("missing ingest");
        sqlx::query("UPDATE event_envelope_head SET event_id = ? WHERE event_id = ?")
            .bind(event_id('f'))
            .bind(missing_event.id_str())
            .execute(store.pool())
            .await
            .expect_err("missing event reference mutation must be rejected");
        assert!(
            store
                .raw_event_head(&missing_coordinate)
                .await
                .expect("raw head")
                .is_some()
        );
    }

    #[tokio::test]
    async fn id_mismatch_addressable_raw_json_does_not_update_heads() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let original = signed_event(
            KIND_CLASSIFIED_LISTING,
            17,
            operational_listing_tags("listing-1"),
            "{}",
        );
        let first = store
            .ingest_event(RadrootsEventIngest::new(original.clone(), 2_300))
            .await
            .expect("first");
        let coordinate = head_coordinate_for_event(&original);
        let invalid = signed_event(
            KIND_CLASSIFIED_LISTING,
            18,
            operational_listing_tags("listing-1"),
            "{}",
        );
        let raw_json = tampered_content_raw_json(&invalid, "{\"tampered\":true}");
        let error = RadrootsEventIngest::from_raw_json(raw_json, 2_400).expect_err("id mismatch");
        let head = store
            .raw_event_head(&coordinate)
            .await
            .expect("head")
            .expect("stored head");

        assert_eq!(first.raw_head_decision, RadrootsRawHeadDecision::Applied);
        assert!(matches!(error, RadrootsEventStoreError::EventWire(_)));
        assert_eq!(head.event_id, original.id_str());
    }

    #[tokio::test]
    async fn signature_invalid_addressable_events_do_not_update_heads() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let original = signed_event(
            KIND_CLASSIFIED_LISTING,
            19,
            operational_listing_tags("listing-2"),
            "{}",
        );
        store
            .ingest_event(RadrootsEventIngest::new(original.clone(), 2_500))
            .await
            .expect("first");
        let coordinate = head_coordinate_for_event(&original);
        let invalid = tamper_signature(&signed_event(
            KIND_CLASSIFIED_LISTING,
            20,
            operational_listing_tags("listing-2"),
            "{}",
        ));

        let error = RadrootsEventIngest::from_signed_event(invalid.clone(), 2_600)
            .expect_err("invalid signature");
        let head = store
            .raw_event_head(&coordinate)
            .await
            .expect("head")
            .expect("stored head");

        assert!(matches!(
            error,
            RadrootsEventStoreError::Nip01Verification(
                radroots_event_codec::verification::RadrootsNip01VerificationError::SignatureInvalid
            )
        ));
        assert!(
            store
                .raw_event(invalid.id_str())
                .await
                .expect("raw event")
                .is_none()
        );
        assert_eq!(head.event_id, original.id_str());
    }

    #[tokio::test]
    async fn duplicate_contract_invalid_addressable_events_preserve_raw_heads() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let invalid = signed_event(
            KIND_CLASSIFIED_LISTING,
            22,
            operational_listing_tags("listing-3"),
            "{}",
        );
        let coordinate = head_coordinate_for_event(&invalid);

        let first_invalid = store
            .ingest_event(RadrootsEventIngest::new(invalid.clone(), 2_800))
            .await
            .expect("first invalid");
        let second_invalid = store
            .ingest_event(RadrootsEventIngest::new(invalid.clone(), 2_900))
            .await
            .expect("second invalid");
        let head = store
            .raw_event_head(&coordinate)
            .await
            .expect("head")
            .expect("stored head");

        assert!(first_invalid.persistence.is_inserted());
        assert!(second_invalid.persistence.is_duplicate());
        assert_eq!(
            first_invalid.persistence.sequence(),
            second_invalid.persistence.sequence()
        );
        assert_eq!(
            first_invalid.admission_status,
            RadrootsEventAdmissionStatus::Invalid
        );
        assert_eq!(
            second_invalid.admission_status,
            RadrootsEventAdmissionStatus::Invalid
        );
        assert!(first_invalid.admission_code.is_some());
        assert_eq!(second_invalid.admission_code, first_invalid.admission_code);
        assert_eq!(
            first_invalid.raw_head_decision,
            RadrootsRawHeadDecision::Applied
        );
        assert_eq!(
            second_invalid.raw_head_decision,
            RadrootsRawHeadDecision::SkippedDuplicate
        );
        assert_eq!(head.event_id, invalid.id_str());
    }

    #[tokio::test]
    async fn duplicate_verified_addressable_events_preserve_heads() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let event = signed_event(
            KIND_CLASSIFIED_LISTING,
            23,
            operational_listing_tags("listing-4"),
            "{}",
        );
        let coordinate = head_coordinate_for_event(&event);

        let first = store
            .ingest_event(RadrootsEventIngest::new(event.clone(), 3_000))
            .await
            .expect("first");
        let second = store
            .ingest_event(RadrootsEventIngest::new(event.clone(), 3_100))
            .await
            .expect("second");
        let head = store
            .raw_event_head(&coordinate)
            .await
            .expect("head")
            .expect("stored head");

        assert!(first.persistence.is_inserted());
        assert!(second.persistence.is_duplicate());
        assert_eq!(first.persistence.sequence(), second.persistence.sequence());
        assert_eq!(first.raw_head_decision, RadrootsRawHeadDecision::Applied);
        assert_eq!(
            second.raw_head_decision,
            RadrootsRawHeadDecision::SkippedDuplicate
        );
        assert_eq!(head.event_id, event.id_str());
    }

    #[tokio::test]
    async fn verified_regular_events_remain_projection_eligible_without_head_selection() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let event = signed_event(KIND_POST, 24, Vec::new(), "hello");

        let receipt = store
            .ingest_event(RadrootsEventIngest::new(event.clone(), 3_200))
            .await
            .expect("ingest");
        let stored = store
            .raw_event(event.id_str())
            .await
            .expect("get")
            .expect("stored");

        assert_eq!(
            receipt.admission_status,
            RadrootsEventAdmissionStatus::Admitted
        );
        assert_eq!(
            receipt.raw_head_decision,
            RadrootsRawHeadDecision::NotHeadSelected
        );
        assert!(receipt.valid_stream_eligible);
        assert!(stored.valid_stream_eligible);
        assert!(
            store
                .valid_event(event.id_str())
                .await
                .expect("valid event")
                .is_some()
        );
        assert_eq!(
            store
                .event_visibility(event.id_str())
                .await
                .expect("visibility"),
            Some(RadrootsEventVisibility::Visible)
        );
        assert!(
            store
                .visible_event(event.id_str())
                .await
                .expect("visible event")
                .is_some()
        );
    }

    #[tokio::test]
    async fn tag_reads_separate_raw_events_from_the_valid_stream() {
        let store = RadrootsEventStore::open_memory().await.expect("store");

        assert!(matches!(
            store.valid_stream_by_tag("", "soil", 1).await,
            Err(RadrootsEventStoreError::EmptyTagName)
        ));
        assert!(matches!(
            store.valid_stream_by_tag("t", "soil", 0).await,
            Err(RadrootsEventStoreError::QueryLimitOutOfRange { .. })
        ));
        assert!(matches!(
            store
                .valid_stream_by_tag("t", "soil", RADROOTS_EVENT_STORE_QUERY_LIMIT_MAX + 1)
                .await,
            Err(RadrootsEventStoreError::QueryLimitOutOfRange { .. })
        ));

        let unsupported = signed_event(
            999,
            40,
            vec![vec!["t".to_owned(), "soil".to_owned()]],
            "unsupported",
        );
        let high_created_at = signed_event(
            KIND_POST,
            60,
            vec![
                vec!["t".to_owned(), "soil".to_owned()],
                vec!["t".to_owned(), "soil".to_owned()],
            ],
            "high-created-at",
        );
        let low_created_at = signed_event(
            KIND_POST,
            50,
            vec![vec!["t".to_owned(), "soil".to_owned()]],
            "low-created-at",
        );

        store
            .ingest_event(RadrootsEventIngest::new(unsupported.clone(), 3_300))
            .await
            .expect("unsupported ingest");
        store
            .ingest_event(RadrootsEventIngest::new(high_created_at.clone(), 3_400))
            .await
            .expect("high ingest");
        store
            .ingest_event(RadrootsEventIngest::new(low_created_at.clone(), 3_500))
            .await
            .expect("low ingest");

        let events = store
            .valid_stream_by_tag("t", "soil", 10)
            .await
            .expect("tag query");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].raw_event().event_id, high_created_at.id_str());
        assert_eq!(events[1].raw_event().event_id, low_created_at.id_str());
        assert!(
            events
                .iter()
                .all(|event| event.raw_event().valid_stream_eligible)
        );

        let raw_events = store
            .raw_events_by_tag("t", "soil", 10)
            .await
            .expect("raw tag query");
        assert_eq!(raw_events.len(), 3);
        assert_eq!(raw_events[0].event_id, unsupported.id_str());

        let limited = store
            .valid_stream_by_tag("t", "soil", 1)
            .await
            .expect("limited tag query");
        assert_eq!(limited.len(), 1);
        assert_eq!(limited[0].raw_event().event_id, high_created_at.id_str());
    }

    #[tokio::test]
    async fn valid_stream_by_contract_and_tag_enforces_contract_and_tag_filters() {
        let store = RadrootsEventStore::open_memory().await.expect("store");

        assert!(matches!(
            store
                .valid_stream_by_contract_and_tag::<&str>(
                    &[],
                    "p",
                    FIXTURE_ALICE_PUBLIC_KEY_HEX,
                    1,
                )
                .await,
            Err(RadrootsEventStoreError::EmptyContractList)
        ));
        let too_many_contracts = vec![
            RADROOTS_TRADE_PROPOSAL_CONTRACT_ID;
            RADROOTS_EVENT_STORE_CONTRACT_QUERY_LIMIT_MAX + 1
        ];
        assert!(matches!(
            store
                .valid_stream_by_contract_and_tag(
                    too_many_contracts.as_slice(),
                    "p",
                    FIXTURE_ALICE_PUBLIC_KEY_HEX,
                    1,
                )
                .await,
            Err(RadrootsEventStoreError::ContractListTooLarge { .. })
        ));

        let proposal = canonical_trade_mutation_content(proposal_envelope()).expect("proposal");
        let matching_trade = signed_trade_mutation(&proposal);
        let same_tag_wrong_contract = signed_event(
            KIND_POST,
            72,
            vec![vec![
                "p".to_owned(),
                FIXTURE_ALICE_PUBLIC_KEY_HEX.to_owned(),
            ]],
            "hello",
        );
        let unsupported_same_tag = signed_event(
            999,
            73,
            vec![vec![
                "p".to_owned(),
                FIXTURE_ALICE_PUBLIC_KEY_HEX.to_owned(),
            ]],
            "unsupported",
        );

        for (event, observed_at_ms) in [
            (matching_trade.clone(), 3_600),
            (same_tag_wrong_contract, 3_800),
            (unsupported_same_tag, 3_900),
        ] {
            store
                .ingest_event(RadrootsEventIngest::new(event, observed_at_ms))
                .await
                .expect("ingest");
        }

        let events = store
            .valid_stream_by_contract_and_tag(
                &[RADROOTS_TRADE_PROPOSAL_CONTRACT_ID],
                "p",
                FIXTURE_ALICE_PUBLIC_KEY_HEX,
                10,
            )
            .await
            .expect("contract tag query");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].raw_event().event_id, matching_trade.id_str());
        assert_eq!(
            events[0].raw_event().contract_id.as_deref(),
            Some(RADROOTS_TRADE_PROPOSAL_CONTRACT_ID)
        );
        assert!(events[0].raw_event().valid_stream_eligible);
    }

    #[tokio::test]
    async fn tag_rows_preserve_order_and_contract_metadata() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let event = signed_event(
            KIND_PROFILE,
            14,
            vec![
                vec!["p".to_owned(), FIXTURE_ALICE_PUBLIC_KEY_HEX.to_owned()],
                vec!["t".to_owned(), "harvest".to_owned()],
            ],
            "{}",
        );

        store
            .ingest_event(RadrootsEventIngest::new(event.clone(), 3_000))
            .await
            .expect("ingest");
        let tags = store.tags_for_event(event.id_str()).await.expect("tags");

        assert_eq!(tags[0].tag_index, 0);
        assert_eq!(tags[0].tag_name, "p");
        assert_eq!(tags[0].contract_value_type.as_deref(), Some("public_key"));
        assert!(tags[0].relay_indexed);
        assert_eq!(tags[1].tag_index, 1);
        assert_eq!(tags[1].tag_json, "[\"t\",\"harvest\"]");
    }

    #[tokio::test]
    async fn database_guards_reject_non_boolean_relay_indexed_values() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let event = signed_event(
            KIND_POST,
            14,
            vec![vec!["t".to_owned(), "harvest".to_owned()]],
            "hello",
        );

        store
            .ingest_event(RadrootsEventIngest::new(event.clone(), 3_000))
            .await
            .expect("ingest");
        let before = store.tags_for_event(event.id_str()).await.expect("tags");
        sqlx::query(
            "UPDATE event_envelope_tags SET relay_indexed = 2 WHERE event_id = ? AND tag_index = 0",
        )
        .bind(event.id_str())
        .execute(store.pool())
        .await
        .expect_err("non-boolean relay_indexed mutation must be rejected");

        let tags = store.tags_for_event(event.id_str()).await.expect("tags");
        assert_eq!(tags, before);
    }

    #[tokio::test]
    async fn trade_mutation_tags_persist_contract_and_semantic_metadata() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let proposal = canonical_trade_mutation_content(proposal_envelope()).expect("proposal");
        let event = signed_trade_mutation(&proposal);

        store
            .ingest_event(RadrootsEventIngest::new(event.clone(), 3_100))
            .await
            .expect("ingest");
        let tags = store.tags_for_event(event.id_str()).await.expect("tags");
        let contract_tag = tags
            .iter()
            .find(|tag| tag.tag_name == "contract")
            .expect("contract tag");
        let mutation_tag = tags
            .iter()
            .find(|tag| tag.tag_name == "d")
            .expect("mutation d tag");

        assert_eq!(
            contract_tag.tag_value.as_deref(),
            Some(RADROOTS_TRADE_PROPOSAL_CONTRACT_ID)
        );
        assert_eq!(contract_tag.contract_semantic.as_deref(), Some("contract"));
        assert_eq!(
            contract_tag.contract_value_type.as_deref(),
            Some("contract_id")
        );
        assert!(!contract_tag.relay_indexed);
        assert_eq!(
            mutation_tag.tag_value.as_deref(),
            Some(proposal.mutation_id.as_str())
        );
        assert_eq!(
            mutation_tag.contract_semantic.as_deref(),
            Some("identifier")
        );
        assert_eq!(mutation_tag.contract_value_type.as_deref(), Some("d_tag"));
    }

    #[tokio::test]
    async fn transport_observations_upsert_and_query_by_endpoint() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let event = signed_event(KIND_POST, 15, Vec::new(), "hello");
        let observation = RadrootsTransportObservation::new(
            RadrootsTransportKind::Nostr,
            "wss://relay.local",
            crate::RadrootsTransportObservationType::Subscription,
            4_000,
        )
        .expect("observation");
        let ingest = RadrootsEventIngest::new(event.clone(), 4_000).with_observation(observation);
        store.ingest_event(ingest).await.expect("first");
        let observation = RadrootsTransportObservation::new(
            RadrootsTransportKind::Nostr,
            "wss://relay.local",
            crate::RadrootsTransportObservationType::Subscription,
            4_100,
        )
        .expect("observation")
        .try_with_caller_redacted_message("duplicate accepted")
        .expect("caller-redacted message");
        let ingest = RadrootsEventIngest::new(event.clone(), 4_100).with_observation(observation);
        store.ingest_event(ingest).await.expect("second");
        let observation = RadrootsTransportObservation::new(
            RadrootsTransportKind::Nostr,
            "wss://relay.local",
            crate::RadrootsTransportObservationType::Subscription,
            4_050,
        )
        .expect("observation")
        .try_with_caller_redacted_message("stale duplicate")
        .expect("caller-redacted message");
        let ingest = RadrootsEventIngest::new(event.clone(), 4_050).with_observation(observation);
        store.ingest_event(ingest).await.expect("older duplicate");

        let observations = store
            .observations_for_event(event.id_str())
            .await
            .expect("stale duplicate observations");
        assert_eq!(observations.len(), 1);
        assert_eq!(observations[0].observation_count, 3);
        assert_eq!(observations[0].first_observed_at_ms, 4_000);
        assert_eq!(observations[0].last_observed_at_ms, 4_100);
        assert_eq!(
            observations[0].caller_redacted_message.as_deref(),
            Some("duplicate accepted")
        );

        let observation = RadrootsTransportObservation::new(
            RadrootsTransportKind::Nostr,
            "wss://relay.local",
            crate::RadrootsTransportObservationType::Subscription,
            4_100,
        )
        .expect("observation")
        .try_with_caller_redacted_message("tie duplicate accepted")
        .expect("caller-redacted message");
        let ingest = RadrootsEventIngest::new(event.clone(), 4_100).with_observation(observation);
        store.ingest_event(ingest).await.expect("tie duplicate");
        let observation = RadrootsTransportObservation::new(
            RadrootsTransportKind::Nostr,
            "wss://relay.local",
            crate::RadrootsTransportObservationType::Subscription,
            4_100,
        )
        .expect("observation");
        let ingest = RadrootsEventIngest::new(event.clone(), 4_100).with_observation(observation);
        store
            .ingest_event(ingest)
            .await
            .expect("tie duplicate without message");
        let observation = RadrootsTransportObservation::new(
            RadrootsTransportKind::Nostr,
            "wss://relay.local",
            crate::RadrootsTransportObservationType::Subscription,
            4_200,
        )
        .expect("observation");
        let ingest = RadrootsEventIngest::new(event.clone(), 4_200).with_observation(observation);
        store
            .ingest_event(ingest)
            .await
            .expect("newer duplicate without message");

        let observations = store
            .observations_for_event(event.id_str())
            .await
            .expect("observations");
        assert_eq!(observations.len(), 1);
        assert_eq!(observations[0].transport_kind, RadrootsTransportKind::Nostr);
        assert_eq!(observations[0].endpoint_uri.as_str(), "wss://relay.local");
        assert_eq!(
            observations[0].observation_type,
            crate::RadrootsTransportObservationType::Subscription
        );
        assert_eq!(observations[0].observation_count, 6);
        assert_eq!(observations[0].first_observed_at_ms, 4_000);
        assert_eq!(observations[0].last_observed_at_ms, 4_200);
        assert_eq!(
            observations[0].caller_redacted_message.as_deref(),
            Some("tie duplicate accepted")
        );

        let endpoint_observations = store
            .observations_for_endpoint(RadrootsTransportKind::Nostr, "WSS://RELAY.LOCAL/")
            .await
            .expect("endpoint observations");
        assert_eq!(endpoint_observations, observations);

        let reticulum_observation = RadrootsTransportObservation::new(
            RadrootsTransportKind::Reticulum,
            "reticulum:local",
            crate::RadrootsTransportObservationType::MeshHeard,
            4_300,
        )
        .expect("Reticulum observation");
        store
            .ingest_event(
                RadrootsEventIngest::new(event, 4_300).with_observation(reticulum_observation),
            )
            .await
            .expect("Reticulum observation ingest");
        let reticulum_observations = store
            .observations_for_endpoint(RadrootsTransportKind::Reticulum, "reticulum:local")
            .await
            .expect("Reticulum endpoint observations");
        assert_eq!(reticulum_observations.len(), 1);
        let expected_reticulum =
            RadrootsTransportTarget::reticulum().expect("canonical Reticulum target");
        assert_eq!(
            &reticulum_observations[0].endpoint_fingerprint,
            expected_reticulum.fingerprint()
        );
    }

    #[tokio::test]
    async fn transport_observation_ingest_rejects_forged_endpoint_identity_atomically() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let event = signed_event(KIND_POST, 16, Vec::new(), "forged observation");
        let endpoint_uri =
            RadrootsTransportTargetUri::parse("wss://relay-a.local").expect("endpoint A");
        let endpoint_b =
            RadrootsTransportTarget::nostr_relay("wss://relay-b.local").expect("endpoint B");
        let observation = RadrootsTransportObservation::from_unchecked_parts_for_test(
            RadrootsTransportKind::Nostr,
            endpoint_uri,
            endpoint_b.fingerprint().clone(),
            crate::RadrootsTransportObservationType::Subscription,
            4_000,
        );

        assert!(matches!(
            store
                .ingest_event(
                    RadrootsEventIngest::new(event.clone(), 4_000)
                        .with_observation(observation),
                )
                .await,
            Err(RadrootsEventStoreError::InvalidStoredTransportEndpointFingerprint {
                event_id,
                ..
            }) if event_id == event.id_str()
        ));
        assert!(
            store
                .raw_event(event.id_str())
                .await
                .expect("raw event")
                .is_none()
        );
        let observation_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM event_transport_observation")
                .fetch_one(store.pool())
                .await
                .expect("observation count");
        assert_eq!(observation_count, 0);
    }

    #[tokio::test]
    async fn transport_observation_reads_reject_invalid_counts_and_time_order() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let event = signed_event(KIND_POST, 16, Vec::new(), "observation corruption");
        let observation = RadrootsTransportObservation::new(
            RadrootsTransportKind::Nostr,
            "wss://relay.local",
            crate::RadrootsTransportObservationType::Subscription,
            4_000,
        )
        .expect("observation");
        store
            .ingest_event(
                RadrootsEventIngest::new(event.clone(), 4_000).with_observation(observation),
            )
            .await
            .expect("ingest");

        sqlx::query(
            "UPDATE event_transport_observation SET observation_count = 0 WHERE event_id = ?",
        )
        .bind(event.id_str())
        .execute(store.pool())
        .await
        .expect("corrupt observation count");
        assert!(matches!(
            store.observations_for_event(event.id_str()).await,
            Err(RadrootsEventStoreError::InvalidStoredTransportObservation {
                observation_count: 0,
                ..
            })
        ));

        sqlx::query(
            "UPDATE event_transport_observation SET observation_count = 1, first_observed_at_ms = 4_100, last_observed_at_ms = 4_000 WHERE event_id = ?",
        )
        .bind(event.id_str())
        .execute(store.pool())
        .await
        .expect("corrupt observation time order");
        assert!(matches!(
            store
                .observations_for_endpoint(RadrootsTransportKind::Nostr, "wss://relay.local")
                .await,
            Err(RadrootsEventStoreError::InvalidStoredTransportObservation {
                first_observed_at_ms: 4_100,
                last_observed_at_ms: 4_000,
                ..
            })
        ));

        sqlx::query(
            "UPDATE event_transport_observation SET first_observed_at_ms = -1, last_observed_at_ms = 4_000 WHERE event_id = ?",
        )
        .bind(event.id_str())
        .execute(store.pool())
        .await
        .expect("corrupt observation timestamp");
        assert!(matches!(
            store.observations_for_event(event.id_str()).await,
            Err(RadrootsEventStoreError::InvalidStoredTransportObservation {
                first_observed_at_ms: -1,
                ..
            })
        ));

        sqlx::query(
            "UPDATE event_transport_observation SET first_observed_at_ms = 4_000, redacted_message = ? WHERE event_id = ?",
        )
        .bind("line\nbreak")
        .bind(event.id_str())
        .execute(store.pool())
        .await
        .expect("corrupt observation message");
        assert!(matches!(
            store.observations_for_event(event.id_str()).await,
            Err(
                RadrootsEventStoreError::InvalidStoredTransportObservationMessage {
                    ref event_id,
                    ..
                }
            ) if event_id == event.id_str()
        ));

        sqlx::query(
            "UPDATE event_transport_observation SET observation_count = 1, first_observed_at_ms = 4_000, last_observed_at_ms = 4_000, redacted_message = NULL, transport_kind = 'NOSTR' WHERE event_id = ?",
        )
        .bind(event.id_str())
        .execute(store.pool())
        .await
        .expect("corrupt transport kind canonical form");
        assert!(matches!(
            store.observations_for_event(event.id_str()).await,
            Err(RadrootsEventStoreError::Transport(
                radroots_transport::RadrootsTransportError::InvalidTransportKind
            ))
        ));

        sqlx::query(
            "UPDATE event_transport_observation SET transport_kind = 'nostr', endpoint_fingerprint = upper(endpoint_fingerprint) WHERE event_id = ?",
        )
        .bind(event.id_str())
        .execute(store.pool())
        .await
        .expect("corrupt endpoint fingerprint canonical form");
        assert!(matches!(
            store.observations_for_event(event.id_str()).await,
            Err(RadrootsEventStoreError::InvalidStoredTransportEndpointFingerprint { .. })
        ));
    }

    #[tokio::test]
    async fn invalid_raw_head_never_falls_back_to_an_older_valid_revision() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let older = signed_event(KIND_PROFILE, 18, Vec::new(), "{\"name\":\"valid\"}");
        let newer = signed_event(KIND_PROFILE, 19, Vec::new(), "not-json");
        let coordinate = profile_coordinate();

        let older_receipt = store
            .ingest_event(RadrootsEventIngest::new(older.clone(), 4_500))
            .await
            .expect("older");
        let newer_receipt = store
            .ingest_event(RadrootsEventIngest::new(newer.clone(), 4_600))
            .await
            .expect("newer");
        assert_eq!(
            older_receipt.admission_status,
            RadrootsEventAdmissionStatus::Admitted
        );
        assert_eq!(
            newer_receipt.admission_status,
            RadrootsEventAdmissionStatus::Invalid
        );
        assert_eq!(
            store
                .raw_event_head(&coordinate)
                .await
                .expect("raw head")
                .expect("head")
                .event_id,
            newer.id_str()
        );
        assert!(
            store
                .valid_event(older.id_str())
                .await
                .expect("valid")
                .is_some()
        );
        assert!(
            store
                .valid_event(newer.id_str())
                .await
                .expect("invalid")
                .is_none()
        );
        assert_eq!(
            store
                .event_visibility(older.id_str())
                .await
                .expect("older visibility"),
            Some(RadrootsEventVisibility::NotCurrent {
                raw_head_event_id: newer.id_str().to_owned(),
            })
        );
        assert_eq!(
            store
                .event_visibility(newer.id_str())
                .await
                .expect("newer visibility"),
            Some(RadrootsEventVisibility::NotAdmitted)
        );
        assert!(
            store
                .visible_event(older.id_str())
                .await
                .expect("older visible")
                .is_none()
        );
        assert!(
            store
                .visible_event(newer.id_str())
                .await
                .expect("newer visible")
                .is_none()
        );
        assert!(
            store
                .visible_event_head(&coordinate)
                .await
                .expect("visible head")
                .is_none()
        );
        let valid_stream = store.valid_stream_after(0, 10).await.expect("valid stream");
        assert_eq!(valid_stream.len(), 1);
        assert_eq!(valid_stream[0].raw_event().event_id, older.id_str());

        let older_duplicate = store
            .ingest_event(RadrootsEventIngest::new(older, 4_700))
            .await
            .expect("older duplicate");
        assert!(older_duplicate.persistence.is_duplicate());
        assert_eq!(
            older_duplicate.raw_head_decision,
            RadrootsRawHeadDecision::SkippedOlder
        );
        let newer_duplicate = store
            .ingest_event(RadrootsEventIngest::new(newer.clone(), 4_800))
            .await
            .expect("newer duplicate");
        assert!(newer_duplicate.persistence.is_duplicate());
        assert_eq!(
            newer_duplicate.raw_head_decision,
            RadrootsRawHeadDecision::SkippedDuplicate
        );

        sqlx::query(
            "DELETE FROM event_envelope_head WHERE coordinate_type = 'replaceable' AND kind = ? AND pubkey = ?",
        )
        .bind(i64::from(KIND_PROFILE))
        .bind(FIXTURE_ALICE_PUBLIC_KEY_HEX)
        .execute(store.pool())
        .await
        .expect_err("raw-head deletion must be rejected");
        let duplicate = store
            .ingest_event(RadrootsEventIngest::new(newer.clone(), 4_900))
            .await
            .expect("duplicate after rejected deletion");
        assert!(duplicate.persistence.is_duplicate());
        assert_eq!(
            duplicate.raw_head_decision,
            RadrootsRawHeadDecision::SkippedDuplicate
        );
        assert_eq!(
            store
                .raw_event_head(&coordinate)
                .await
                .expect("raw head")
                .expect("preserved head")
                .event_id,
            newer.id_str()
        );
    }

    #[tokio::test]
    async fn event_heads_use_protocol_tie_breaks() {
        let mut events = [
            signed_event(KIND_PROFILE, 20, Vec::new(), "{\"name\":\"a\"}"),
            signed_event(KIND_PROFILE, 20, Vec::new(), "{\"name\":\"b\"}"),
        ];
        events.sort_by(|left, right| left.id_str().cmp(right.id_str()));
        let lower = events[0].clone();
        let higher = events[1].clone();

        let store = RadrootsEventStore::open_memory().await.expect("open");
        let first = store
            .ingest_event(RadrootsEventIngest::new(higher.clone(), 5_000))
            .await
            .expect("first");
        let second = store
            .ingest_event(RadrootsEventIngest::new(lower.clone(), 5_100))
            .await
            .expect("second");
        let head = store
            .raw_event_head(&profile_coordinate())
            .await
            .expect("head")
            .expect("stored head");

        assert_eq!(first.raw_head_decision, RadrootsRawHeadDecision::Applied);
        assert_eq!(second.raw_head_decision, RadrootsRawHeadDecision::Applied);
        assert_eq!(head.event_id, lower.id_str());

        let store = RadrootsEventStore::open_memory().await.expect("open");
        store
            .ingest_event(RadrootsEventIngest::new(lower.clone(), 5_200))
            .await
            .expect("first");
        let second = store
            .ingest_event(RadrootsEventIngest::new(higher, 5_300))
            .await
            .expect("second");
        let head = store
            .raw_event_head(&profile_coordinate())
            .await
            .expect("head")
            .expect("stored head");

        assert_eq!(
            second.raw_head_decision,
            RadrootsRawHeadDecision::SkippedSameTimestampHigherEventId
        );
        assert_eq!(head.event_id, lower.id_str());
    }

    #[tokio::test]
    async fn projection_cursors_replay_by_store_sequence() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let source_generation = store.source_generation().await.expect("source generation");
        let first = signed_event(KIND_POST, 30, Vec::new(), "one");
        let second = signed_event(KIND_POST, 30, Vec::new(), "two");
        let first_receipt = store
            .ingest_event(RadrootsEventIngest::new(first.clone(), 6_000))
            .await
            .expect("first");
        let second_receipt = store
            .ingest_event(RadrootsEventIngest::new(second.clone(), 6_100))
            .await
            .expect("second");
        let first_seq = first_receipt
            .persistence
            .sequence()
            .expect("first sequence");
        let second_seq = second_receipt
            .persistence
            .sequence()
            .expect("second sequence");
        assert!(first_seq < second_seq);

        let replay = store
            .valid_stream_after(0, 10)
            .await
            .expect("initial replay");
        assert_eq!(replay.len(), 2);
        assert_eq!(replay[0].raw_event().event_id, first.id_str());
        assert_eq!(replay[1].raw_event().event_id, second.id_str());
        let first_cursor = RadrootsProjectionCursor {
            projection_id: "social".to_owned(),
            projection_version: 1,
            source_generation,
            last_event_seq: first_seq,
            updated_at_ms: 6_200,
        };
        store
            .compare_and_swap_projection_cursor(&first_cursor, None)
            .await
            .expect("cursor");
        assert_eq!(
            store
                .projection_cursor("social", 1)
                .await
                .expect("cursor")
                .expect("stored cursor"),
            first_cursor
        );

        let mismatched_generation = RadrootsProjectionCursor {
            source_generation: RadrootsEventStoreSourceGeneration::from_bytes([0xff; 32]),
            ..first_cursor.clone()
        };
        assert!(matches!(
            store
                .compare_and_swap_projection_cursor(&mismatched_generation, None)
                .await,
            Err(RadrootsEventStoreError::ProjectionSourceGenerationMismatch { .. })
        ));
        let duplicate_insert = RadrootsProjectionCursor {
            updated_at_ms: first_cursor.updated_at_ms() + 1,
            ..first_cursor.clone()
        };
        assert!(matches!(
            store
                .compare_and_swap_projection_cursor(&duplicate_insert, None)
                .await,
            Err(RadrootsEventStoreError::ProjectionCursorConflict { .. })
        ));
        let mismatched_version = RadrootsProjectionCursor {
            projection_version: 2,
            ..first_cursor.clone()
        };
        assert!(matches!(
            store
                .compare_and_swap_projection_cursor(&mismatched_version, None)
                .await,
            Err(RadrootsEventStoreError::ProjectionVersionMismatch {
                expected: 2,
                actual: 1,
                ..
            })
        ));
        let stale_insert = RadrootsProjectionCursor {
            last_event_seq: 0,
            ..first_cursor.clone()
        };
        assert!(matches!(
            store
                .compare_and_swap_projection_cursor(&stale_insert, None)
                .await,
            Err(RadrootsEventStoreError::ProjectionCursorRegression {
                current,
                proposed: 0,
                ..
            }) if current == first_seq
        ));
        assert!(matches!(
            store.projection_cursor("social", 2).await,
            Err(RadrootsEventStoreError::ProjectionVersionMismatch { .. })
        ));
        let replay = store
            .valid_stream_after(first_seq, 10)
            .await
            .expect("next replay");
        assert_eq!(replay.len(), 1);
        assert_eq!(replay[0].raw_event().event_id, second.id_str());

        let second_cursor = RadrootsProjectionCursor {
            projection_id: "social".to_owned(),
            projection_version: 1,
            source_generation,
            last_event_seq: second_seq,
            updated_at_ms: 6_300,
        };
        assert!(matches!(
            store
                .compare_and_swap_projection_cursor(&second_cursor, Some(0))
                .await,
            Err(RadrootsEventStoreError::ProjectionCursorConflict { .. })
        ));
        store
            .compare_and_swap_projection_cursor(&second_cursor, Some(first_seq))
            .await
            .expect("advance cursor");
        assert!(matches!(
            store
                .compare_and_swap_projection_cursor(&first_cursor, Some(second_seq))
                .await,
            Err(RadrootsEventStoreError::ProjectionCursorRegression { .. })
        ));
        assert!(matches!(
            store
                .compare_and_swap_projection_cursor(
                    &RadrootsProjectionCursor {
                        projection_id: "negative".to_owned(),
                        projection_version: 1,
                        source_generation,
                        last_event_seq: -1,
                        updated_at_ms: 6_400,
                    },
                    None,
                )
                .await,
            Err(RadrootsEventStoreError::InvalidProjectionCursor { .. })
        ));
    }

    #[tokio::test]
    async fn database_guards_reject_negative_projection_cursor_sequences() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        sqlx::query(
            "INSERT INTO projection_cursor(projection_id, projection_version, last_event_seq, updated_at_ms) VALUES ('corrupt', 1, -1, 1)",
        )
        .execute(store.pool())
        .await
        .expect_err("negative cursor sequence must be rejected");

        assert!(
            store
                .projection_cursor("corrupt", 1)
                .await
                .expect("cursor read")
                .is_none()
        );
    }

    #[tokio::test]
    async fn trade_projection_checkpoint_and_list_queries_use_semantic_indexes() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let proposal = canonical_trade_mutation_content(proposal_envelope()).expect("proposal");
        let proposal_event = signed_trade_mutation(&proposal);
        let proposal_receipt = store
            .ingest_event(RadrootsEventIngest::new(proposal_event, 7_000))
            .await
            .expect("proposal ingest");
        store
            .update_trade_projection_checkpoint(&RadrootsTradeProjectionCheckpoint {
                trade_id: trade_id(),
                reducer_contract_id: "radroots.trade.reducer.v1".to_owned(),
                reducer_version: 1,
                projection_digest: event_id('f'),
                root_mutation_id: Some(proposal.mutation_id.clone()),
                negotiation_state: "open".to_owned(),
                agreement_state: "none".to_owned(),
                evidence_state: "complete".to_owned(),
                conflict_state: "none".to_owned(),
                private_terms_state: "not_required".to_owned(),
                attestation_state: "none".to_owned(),
                fulfillment_state: "not_started".to_owned(),
                payment_state: "not_tracked".to_owned(),
                projection_json: "{\"trade_id\":\"fixture\"}".to_owned(),
                last_mutation_id: Some(proposal.mutation_id.clone()),
                last_transport_event_seq: proposal_receipt.persistence.sequence(),
                updated_at_ms: 7_100,
            })
            .await
            .expect("checkpoint");
        let checkpoint = store
            .trade_projection_checkpoint(&trade_id())
            .await
            .expect("checkpoint query")
            .expect("checkpoint");
        assert_eq!(
            checkpoint.root_mutation_id,
            Some(proposal.mutation_id.clone())
        );
        assert_eq!(checkpoint.agreement_state, "none");

        let mutation_plan = explain_query_plan(
            &store,
            "EXPLAIN QUERY PLAN SELECT mutation_id FROM trade_mutation WHERE trade_id = ? ORDER BY authored_at_unix_s, mutation_id LIMIT 10",
            trade_id().as_str(),
        )
        .await;
        assert!(
            mutation_plan.contains("trade_mutation_trade_idx"),
            "{mutation_plan}"
        );
        let checkpoint_plan = explain_query_plan(
            &store,
            "EXPLAIN QUERY PLAN SELECT trade_id FROM trade_projection_checkpoint WHERE agreement_state = ? ORDER BY updated_at_ms, trade_id LIMIT 10",
            "none",
        )
        .await;
        assert!(
            checkpoint_plan.contains("trade_projection_checkpoint_agreement_idx"),
            "{checkpoint_plan}"
        );
    }

    #[tokio::test]
    async fn smoke_event_store_ingests_and_replays_ten_thousand_events() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let source_generation = store.source_generation().await.expect("source generation");
        for index in 0..10_000u32 {
            let event = signed_event(
                KIND_POST,
                10_000 + index,
                vec![vec!["t".to_owned(), "smoke".to_owned()]],
                format!("smoke-{index}").as_str(),
            );
            let receipt = store
                .ingest_event(RadrootsEventIngest::new(event, 10_000 + i64::from(index)))
                .await
                .expect("ingest");
            assert!(receipt.persistence.is_inserted());
        }

        let mut replay = Vec::with_capacity(10_000);
        let mut after_sequence = 0;
        loop {
            let page = store
                .valid_stream_after(after_sequence, RADROOTS_EVENT_STORE_QUERY_LIMIT_MAX)
                .await
                .expect("replay");
            if page.is_empty() {
                break;
            }
            after_sequence = page.last().expect("page").raw_event().seq;
            replay.extend(page);
        }
        assert_eq!(replay.len(), 10_000);
        assert_eq!(replay[0].raw_event().seq, 1);
        assert_eq!(replay[9_999].raw_event().seq, 10_000);

        store
            .compare_and_swap_projection_cursor(
                &RadrootsProjectionCursor {
                    projection_id: "smoke".to_owned(),
                    projection_version: 1,
                    source_generation,
                    last_event_seq: replay[4_999].raw_event().seq,
                    updated_at_ms: 25_000,
                },
                None,
            )
            .await
            .expect("cursor");
        let replay = store
            .valid_stream_after(5_000, RADROOTS_EVENT_STORE_QUERY_LIMIT_MAX)
            .await
            .expect("replay after cursor");
        assert_eq!(replay.len(), 1_000);
        assert_eq!(replay[0].raw_event().seq, 5_001);
        assert_eq!(replay[999].raw_event().seq, 6_000);
    }
}
