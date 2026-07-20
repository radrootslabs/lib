use crate::RadrootsEventStoreError;
use crate::model::{
    RadrootsEventAdmissionStatus, RadrootsEventIngest, RadrootsEventIngestReceipt,
    RadrootsEventPersistence, RadrootsEventStoreStatusSummary, RadrootsEventVisibility,
    RadrootsProjectionCursor, RadrootsRawHeadDecision, RadrootsStoredEventTag,
    RadrootsStoredRawEvent, RadrootsStoredRawEventHead, RadrootsStoredSellerReservation,
    RadrootsStoredSellerReservationLine, RadrootsStoredTradeMissingParent,
    RadrootsStoredTradeMutation, RadrootsStoredTradeMutationParent,
    RadrootsStoredTradeTransportEnvelope, RadrootsStoredValidEvent, RadrootsStoredVisibleEvent,
    RadrootsStoredVisibleEventHead, RadrootsTradeProjectionCheckpoint,
    RadrootsTransportObservation, RadrootsTransportObservationType, StoredEventClass,
    tag_semantic_name, tag_value_type_name,
};
#[cfg(test)]
use crate::schema::destroy_event_store_schema_for_test;
use crate::schema::{
    RadrootsEventStoreSchemaStatus, inspect_event_store_schema_status, migrate_event_store_schema,
    rollback_event_store_schema_offline,
};
use radroots_event::contract::{RadrootsContractMatchError, RadrootsEventContract};
use radroots_event::event_head::{
    RadrootsCurrentEventHead, RadrootsEventHeadCandidate, RadrootsEventHeadCandidateResult,
    RadrootsEventHeadCoordinate, RadrootsEventHeadDecision, event_head_candidate_for_nip01_event,
    select_event_head,
};
use radroots_event::ids::{
    RadrootsDTag, RadrootsEventId, RadrootsTradeCandidateId, RadrootsTradeId,
    RadrootsTradeMutationId,
};
use radroots_event::trade::{
    RADROOTS_TRADE_MUTATION_CONTRACT_IDS, RadrootsSellerReservationAssertionV1,
    RadrootsTradeDecisionV1, RadrootsTradeMutationBodyV1, RadrootsTradeMutationEnvelopeV1,
    RadrootsTradeMutationKindV1, trade_mutation_from_canonical_content,
};
use radroots_event::{RadrootsEventEnvelope, RadrootsEventKind, RadrootsEventKindClass};
use radroots_event_codec::admission::{
    RadrootsAdmittedEvent, RadrootsEventAdmissionError, admit_verified_event,
};
use radroots_transport::{
    RadrootsTransportKind, RadrootsTransportTargetFingerprint, RadrootsTransportTargetUri,
};
use sha2::{Digest, Sha256};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

pub const RADROOTS_EVENT_STORE_QUERY_LIMIT_MAX: u32 = 1_000;
pub const RADROOTS_EVENT_STORE_CONTRACT_QUERY_LIMIT_MAX: usize = 16;

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
        query_string(&self.pool, "PRAGMA journal_mode").await
    }

    pub async fn status_summary(
        &self,
    ) -> Result<RadrootsEventStoreStatusSummary, RadrootsEventStoreError> {
        inspect_event_store_status(&self.pool).await
    }

    pub async fn ingest_event(
        &self,
        ingest: RadrootsEventIngest,
    ) -> Result<RadrootsEventIngestReceipt, RadrootsEventStoreError> {
        let mut tx = self.pool.begin().await?;
        let receipt = ingest_event_in_transaction(&mut tx, ingest).await?;
        tx.commit().await?;
        Ok(receipt)
    }

    pub async fn ingest_event_in_transaction(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        ingest: RadrootsEventIngest,
    ) -> Result<RadrootsEventIngestReceipt, RadrootsEventStoreError> {
        ingest_event_in_transaction(tx, ingest).await
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

    pub async fn observations_for_endpoint(
        &self,
        transport_kind: RadrootsTransportKind,
        endpoint_uri: impl AsRef<str>,
    ) -> Result<Vec<RadrootsTransportObservationRow>, RadrootsEventStoreError> {
        let endpoint_uri = RadrootsTransportTargetUri::parse(endpoint_uri)?;
        let endpoint_fingerprint =
            RadrootsTransportTargetFingerprint::from_target(&transport_kind, &endpoint_uri, None);
        let rows = sqlx::query(
            "SELECT event_id, transport_kind, endpoint_uri, endpoint_fingerprint, observation_type, first_observed_at_ms, last_observed_at_ms, observation_count, redacted_message FROM event_transport_observation WHERE transport_kind = ? AND endpoint_fingerprint = ? ORDER BY last_observed_at_ms, event_id, observation_type",
        )
        .bind(transport_kind.canonical_label())
        .bind(endpoint_fingerprint.as_str())
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
        let Some(snapshot) = visible_event_snapshot(&mut tx, event_id).await? else {
            tx.commit().await?;
            return Ok(None);
        };
        let visibility = visibility_from_snapshot(&snapshot);
        tx.commit().await?;
        Ok(Some(visibility?))
    }

    pub async fn visible_event(
        &self,
        event_id: &str,
    ) -> Result<Option<RadrootsStoredVisibleEvent>, RadrootsEventStoreError> {
        let mut tx = self.pool.begin().await?;
        let Some(snapshot) = visible_event_snapshot(&mut tx, event_id).await? else {
            tx.commit().await?;
            return Ok(None);
        };
        if visibility_from_snapshot(&snapshot)? != RadrootsEventVisibility::Visible {
            tx.commit().await?;
            return Ok(None);
        }
        let valid_event = RadrootsStoredValidEvent::try_from_raw(snapshot.raw_event)?;
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
        let RawHeadSnapshot {
            raw_head,
            raw_event,
        } = snapshot;
        if raw_event.admission_status != RadrootsEventAdmissionStatus::Admitted {
            tx.commit().await?;
            return Ok(None);
        }
        let valid_event = RadrootsStoredValidEvent::try_from_raw(raw_event)?;
        let event = RadrootsStoredVisibleEvent::new(valid_event);
        tx.commit().await?;
        Ok(Some(RadrootsStoredVisibleEventHead::new(raw_head, event)))
    }

    pub async fn projection_cursor(
        &self,
        projection_id: &str,
        expected_projection_version: u32,
    ) -> Result<Option<RadrootsProjectionCursor>, RadrootsEventStoreError> {
        let row = sqlx::query(
            "SELECT projection_id, projection_version, last_event_seq, updated_at_ms FROM projection_cursor WHERE projection_id = ?",
        )
        .bind(projection_id)
        .fetch_optional(&self.pool)
        .await?;
        let cursor = row.map(projection_cursor_from_row).transpose()?;
        if let Some(cursor) = cursor.as_ref()
            && cursor.projection_version != expected_projection_version
        {
            return Err(RadrootsEventStoreError::ProjectionVersionMismatch {
                projection_id: projection_id.to_owned(),
                expected: expected_projection_version,
                actual: cursor.projection_version,
            });
        }
        Ok(cursor)
    }

    pub async fn compare_and_swap_projection_cursor(
        &self,
        cursor: &RadrootsProjectionCursor,
        expected_prior_sequence: Option<i64>,
    ) -> Result<(), RadrootsEventStoreError> {
        if cursor.last_event_seq < 0 {
            return Err(RadrootsEventStoreError::InvalidProjectionCursor {
                projection_id: cursor.projection_id.clone(),
                value: cursor.last_event_seq,
            });
        }
        match expected_prior_sequence {
            None => {
                let inserted = sqlx::query(
                    "INSERT OR IGNORE INTO projection_cursor(projection_id, projection_version, last_event_seq, updated_at_ms) VALUES (?, ?, ?, ?)",
                )
                .bind(cursor.projection_id.as_str())
                .bind(i64::from(cursor.projection_version))
                .bind(cursor.last_event_seq)
                .bind(cursor.updated_at_ms)
                .execute(&self.pool)
                .await?;
                if inserted.rows_affected() == 1 {
                    return Ok(());
                }
            }
            Some(expected) => {
                if cursor.last_event_seq < expected {
                    return Err(RadrootsEventStoreError::ProjectionCursorRegression {
                        projection_id: cursor.projection_id.clone(),
                        current: expected,
                        proposed: cursor.last_event_seq,
                    });
                }
                let updated = sqlx::query(
                    "UPDATE projection_cursor SET last_event_seq = ?, updated_at_ms = ? WHERE projection_id = ? AND projection_version = ? AND last_event_seq = ?",
                )
                .bind(cursor.last_event_seq)
                .bind(cursor.updated_at_ms)
                .bind(cursor.projection_id.as_str())
                .bind(i64::from(cursor.projection_version))
                .bind(expected)
                .execute(&self.pool)
                .await?;
                if updated.rows_affected() == 1 {
                    return Ok(());
                }
            }
        }

        let actual = projection_cursor_unchecked(&self.pool, cursor.projection_id.as_str()).await?;
        if let Some(actual) = actual.as_ref() {
            if actual.projection_version != cursor.projection_version {
                return Err(RadrootsEventStoreError::ProjectionVersionMismatch {
                    projection_id: cursor.projection_id.clone(),
                    expected: cursor.projection_version,
                    actual: actual.projection_version,
                });
            }
            if cursor.last_event_seq < actual.last_event_seq {
                return Err(RadrootsEventStoreError::ProjectionCursorRegression {
                    projection_id: cursor.projection_id.clone(),
                    current: actual.last_event_seq,
                    proposed: cursor.last_event_seq,
                });
            }
        }
        Err(RadrootsEventStoreError::ProjectionCursorConflict {
            projection_id: cursor.projection_id.clone(),
            expected: expected_prior_sequence,
            actual: actual.map(|cursor| cursor.last_event_seq),
        })
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

/// Inspects an existing event-store pool without configuring or migrating it.
///
/// The inspection uses one read transaction and applies the same fail-closed
/// classification checks as [`RadrootsEventStore::status_summary`]. Callers
/// must supply a pool whose event-store schema has already been initialized.
pub async fn inspect_event_store_status(
    pool: &SqlitePool,
) -> Result<RadrootsEventStoreStatusSummary, RadrootsEventStoreError> {
    let mut tx = pool.begin().await?;
    let inconsistent_event_id: Option<String> = sqlx::query_scalar(
        "SELECT event_id FROM event_envelopes WHERE contract_status NOT IN ('supported', 'unsupported_kind', 'unsupported_shape', 'ambiguous_shape') AND (verification_status != 'verified' OR contract_status NOT IN ('admitted', 'unsupported', 'invalid') OR kind < 0 OR kind > 65535 OR kind BETWEEN 20000 AND 29999 OR event_class IS NULL OR event_class != CASE WHEN kind = 0 OR kind = 3 OR kind BETWEEN 10000 AND 19999 THEN 'replaceable' WHEN kind BETWEEN 30000 AND 39999 THEN 'addressable' ELSE 'regular' END OR projection_eligible NOT IN (0, 1) OR projection_eligible != CASE WHEN contract_status = 'admitted' THEN 1 ELSE 0 END OR (contract_status = 'admitted') != (contract_id IS NOT NULL)) LIMIT 1",
    )
    .fetch_optional(&mut *tx)
    .await?;
    if let Some(event_id) = inconsistent_event_id {
        return Err(RadrootsEventStoreError::StoredRawEventClassificationInconsistent { event_id });
    }
    let row = sqlx::query(
        "SELECT COUNT(*) AS total_events, COALESCE(SUM(CASE WHEN verification_status = 'verified' AND contract_status = 'admitted' AND contract_id IS NOT NULL AND projection_eligible = 1 AND kind BETWEEN 0 AND 65535 AND NOT (kind BETWEEN 20000 AND 29999) AND event_class = CASE WHEN kind = 0 OR kind = 3 OR kind BETWEEN 10000 AND 19999 THEN 'replaceable' WHEN kind BETWEEN 30000 AND 39999 THEN 'addressable' ELSE 'regular' END THEN 1 ELSE 0 END), 0) AS valid_stream_events, MAX(seq) AS last_event_seq, MAX(updated_at_ms) AS last_event_updated_at_ms FROM event_envelopes",
    )
    .fetch_one(&mut *tx)
    .await?;
    let transport_observations: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM event_transport_observation")
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
    pub redacted_message: Option<String>,
}

struct EventAdmission {
    status: RadrootsEventAdmissionStatus,
    code: Option<String>,
    contract: Option<&'static RadrootsEventContract>,
}

impl EventAdmission {
    fn from_result(result: &Result<RadrootsAdmittedEvent, RadrootsEventAdmissionError>) -> Self {
        match result {
            Ok(event) => Self {
                status: RadrootsEventAdmissionStatus::Admitted,
                code: None,
                contract: Some(event.contract()),
            },
            Err(error) => {
                let status = if matches!(
                    error,
                    RadrootsEventAdmissionError::ContractMatch(
                        RadrootsContractMatchError::UnsupportedKind(_)
                            | RadrootsContractMatchError::UnsupportedShape(_)
                    )
                ) {
                    RadrootsEventAdmissionStatus::Unsupported
                } else {
                    RadrootsEventAdmissionStatus::Invalid
                };
                Self {
                    status,
                    code: Some(error.code().to_owned()),
                    contract: None,
                }
            }
        }
    }

    fn valid_stream_eligible(&self, kind_class: RadrootsEventKindClass) -> bool {
        self.status == RadrootsEventAdmissionStatus::Admitted
            && kind_class != RadrootsEventKindClass::Ephemeral
    }
}

struct AppliedHead {
    decision: RadrootsRawHeadDecision,
}

struct InsertRawEventResult {
    inserted: bool,
    seq: i64,
    admission_status: RadrootsEventAdmissionStatus,
    contract_id: Option<String>,
    valid_stream_eligible: bool,
}

struct RawHeadSnapshot {
    raw_head: RadrootsStoredRawEventHead,
    raw_event: RadrootsStoredRawEvent,
}

struct VisibleEventSnapshot {
    raw_event: RadrootsStoredRawEvent,
    raw_head_event_id: Option<String>,
}

async fn configure_pool(
    pool: &SqlitePool,
    file_backed: bool,
) -> Result<(), RadrootsEventStoreError> {
    let max_connections = pool.options().get_max_connections();
    let existing_options = pool.connect_options();
    let main_filename: String =
        sqlx::query_scalar("SELECT file FROM pragma_database_list WHERE name = 'main'")
            .fetch_one(pool)
            .await?;
    let database_is_memory = main_filename.is_empty();
    if file_backed == database_is_memory {
        return Err(RadrootsEventStoreError::SqlitePoolBackingMismatch {
            file_backed,
            filename: main_filename,
        });
    }
    if !file_backed && max_connections != 1 {
        return Err(RadrootsEventStoreError::UnsafeInMemoryPoolConnectionCount {
            actual: max_connections,
        });
    }

    let mut connect_options = existing_options
        .as_ref()
        .clone()
        .foreign_keys(true)
        .busy_timeout(Duration::from_millis(5_000));
    if file_backed {
        connect_options = connect_options.journal_mode(SqliteJournalMode::Wal);
    }
    pool.set_connect_options(connect_options);

    let mut connections = Vec::with_capacity(max_connections as usize);
    for _ in 0..max_connections {
        connections.push(pool.acquire().await?);
    }
    for connection in &mut connections {
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&mut **connection)
            .await?;
        sqlx::query("PRAGMA busy_timeout = 5000")
            .execute(&mut **connection)
            .await?;
        if file_backed {
            sqlx::query("PRAGMA journal_mode = WAL")
                .execute(&mut **connection)
                .await?;
        }
    }
    Ok(())
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

fn is_trade_mutation_contract_id(contract_id: &str) -> bool {
    RADROOTS_TRADE_MUTATION_CONTRACT_IDS.contains(&contract_id)
}

async fn store_trade_mutation_event(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    ingest: &RadrootsEventIngest,
    event_seq: i64,
) -> Result<(), RadrootsEventStoreError> {
    let event = ingest.event();
    let payload_sha256 = sha256_hex(event.content().as_bytes());
    let parsed = match trade_mutation_from_canonical_content(event.content()) {
        Ok(envelope) => envelope,
        Err(error) => {
            insert_trade_quarantine(
                tx,
                None,
                None,
                Some(event.id_str()),
                format!("{error}").as_str(),
                ingest.observed_at_ms(),
            )
            .await?;
            return Ok(());
        }
    };
    let Some(mutation_id) = parsed.mutation_id.clone() else {
        insert_trade_quarantine(
            tx,
            Some(parsed.trade_id.as_str()),
            None,
            Some(event.id_str()),
            "canonical trade mutation content is missing mutation_id",
            ingest.observed_at_ms(),
        )
        .await?;
        return Ok(());
    };
    if parsed.author_pubkey.as_str() != event.author_str() {
        insert_trade_quarantine(
            tx,
            Some(parsed.trade_id.as_str()),
            Some(mutation_id.as_str()),
            Some(event.id_str()),
            "trade mutation author_pubkey does not match transport event pubkey",
            ingest.observed_at_ms(),
        )
        .await?;
        return Ok(());
    }
    let mutation_kind = parsed.mutation_kind();
    let candidate_id = candidate_id_for_mutation(&parsed);
    let proposal_mutation_id = proposal_mutation_id_for_mutation(&parsed);
    let target_claim_mutation_id = target_claim_mutation_id_for_mutation(&parsed);
    sqlx::query(
        "INSERT OR IGNORE INTO trade_mutation(mutation_id, trade_id, root_mutation_id, contract_id, mutation_kind, schema_version, candidate_id, proposal_mutation_id, target_claim_mutation_id, author_pubkey, counterparty_pubkey, buyer_pubkey, seller_pubkey, farm_id, authored_at_unix_s, canonical_payload_bytes, payload_sha256, first_event_seq, first_transport_event_id, inserted_at_ms) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(mutation_id.as_str())
    .bind(parsed.trade_id.as_str())
    .bind(parsed.root_mutation_id.as_ref().map(RadrootsTradeMutationId::as_str))
    .bind(parsed.contract_id.as_str())
    .bind(trade_mutation_kind_storage_value(mutation_kind))
    .bind(i64::from(parsed.schema_version))
    .bind(candidate_id.as_ref().map(RadrootsTradeCandidateId::as_str))
    .bind(proposal_mutation_id.as_ref().map(RadrootsTradeMutationId::as_str))
    .bind(target_claim_mutation_id.as_ref().map(RadrootsTradeMutationId::as_str))
    .bind(parsed.author_pubkey.as_str())
    .bind(parsed.counterparty_pubkey.as_str())
    .bind(parsed.buyer_pubkey.as_str())
    .bind(parsed.seller_pubkey.as_str())
    .bind(parsed.farm_id.as_str())
    .bind(i64_from_u64("authored_at_unix_s", parsed.authored_at_unix_s)?)
    .bind(event.content().as_bytes())
    .bind(payload_sha256.as_str())
    .bind(event_seq)
    .bind(event.id_str())
    .bind(ingest.observed_at_ms())
    .execute(&mut **tx)
    .await?;
    insert_trade_mutation_parents(tx, &mutation_id, &parsed.parent_mutation_ids).await?;
    insert_trade_transport_envelope(
        tx,
        event,
        event_seq,
        &parsed,
        &mutation_id,
        &payload_sha256,
        ingest.observed_at_ms(),
    )
    .await?;
    insert_missing_parent_records(
        tx,
        &parsed,
        &mutation_id,
        event.id_str(),
        ingest.observed_at_ms(),
    )
    .await?;
    delete_resolved_missing_parent_records(tx, &mutation_id).await?;
    if let Some(reservation) = seller_reservation_for_mutation(&parsed) {
        insert_seller_reservation(
            tx,
            &parsed,
            &mutation_id,
            reservation,
            ingest.observed_at_ms(),
        )
        .await?;
    }
    Ok(())
}

async fn insert_trade_quarantine(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    trade_id: Option<&str>,
    mutation_id: Option<&str>,
    transport_event_id: Option<&str>,
    reason: &str,
    observed_at_ms: i64,
) -> Result<(), RadrootsEventStoreError> {
    sqlx::query(
        "INSERT INTO trade_projection_quarantine(trade_id, mutation_id, transport_event_id, reason, observed_at_ms) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(trade_id)
    .bind(mutation_id)
    .bind(transport_event_id)
    .bind(reason)
    .bind(observed_at_ms)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn insert_trade_mutation_parents(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    mutation_id: &RadrootsTradeMutationId,
    parents: &[RadrootsTradeMutationId],
) -> Result<(), RadrootsEventStoreError> {
    for (index, parent) in parents.iter().enumerate() {
        sqlx::query(
            "INSERT OR IGNORE INTO trade_mutation_parent(mutation_id, parent_mutation_id, parent_index) VALUES (?, ?, ?)",
        )
        .bind(mutation_id.as_str())
        .bind(parent.as_str())
        .bind(i64_from_usize("parent_index", index)?)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

async fn insert_trade_transport_envelope(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    event: &RadrootsEventEnvelope,
    event_seq: i64,
    mutation: &RadrootsTradeMutationEnvelopeV1,
    mutation_id: &RadrootsTradeMutationId,
    payload_sha256: &str,
    observed_at_ms: i64,
) -> Result<(), RadrootsEventStoreError> {
    sqlx::query(
        "INSERT OR IGNORE INTO trade_transport_envelope(transport_event_id, mutation_id, trade_id, transport_kind, pubkey, created_at, event_seq, payload_sha256, observed_at_ms) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(event.id_str())
    .bind(mutation_id.as_str())
    .bind(mutation.trade_id.as_str())
    .bind(RadrootsTransportKind::Nostr.canonical_label())
    .bind(event.author_str())
    .bind(i64_from_u64("created_at", event.created_at_u64())?)
    .bind(event_seq)
    .bind(payload_sha256)
    .bind(observed_at_ms)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn insert_missing_parent_records(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    mutation: &RadrootsTradeMutationEnvelopeV1,
    mutation_id: &RadrootsTradeMutationId,
    transport_event_id: &str,
    observed_at_ms: i64,
) -> Result<(), RadrootsEventStoreError> {
    for parent in &mutation.parent_mutation_ids {
        let exists: Option<i64> =
            sqlx::query_scalar("SELECT 1 FROM trade_mutation WHERE mutation_id = ? LIMIT 1")
                .bind(parent.as_str())
                .fetch_optional(&mut **tx)
                .await?;
        if exists.is_none() {
            sqlx::query(
                "INSERT OR IGNORE INTO trade_missing_parent(trade_id, mutation_id, missing_parent_mutation_id, first_transport_event_id, first_seen_at_ms) VALUES (?, ?, ?, ?, ?)",
            )
            .bind(mutation.trade_id.as_str())
            .bind(mutation_id.as_str())
            .bind(parent.as_str())
            .bind(transport_event_id)
            .bind(observed_at_ms)
            .execute(&mut **tx)
            .await?;
        }
    }
    Ok(())
}

async fn delete_resolved_missing_parent_records(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    mutation_id: &RadrootsTradeMutationId,
) -> Result<(), RadrootsEventStoreError> {
    sqlx::query("DELETE FROM trade_missing_parent WHERE missing_parent_mutation_id = ?")
        .bind(mutation_id.as_str())
        .execute(&mut **tx)
        .await?;
    Ok(())
}

async fn insert_seller_reservation(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    mutation: &RadrootsTradeMutationEnvelopeV1,
    claim_mutation_id: &RadrootsTradeMutationId,
    reservation: &RadrootsSellerReservationAssertionV1,
    inserted_at_ms: i64,
) -> Result<(), RadrootsEventStoreError> {
    let reservation_json = serde_json::to_string(reservation)?;
    sqlx::query(
        "INSERT OR IGNORE INTO seller_inventory_reservation(reservation_id, trade_id, candidate_id, claim_mutation_id, inventory_authority_pubkey, inventory_epoch, assertion_commitment, reservation_expires_at_unix_s, reservation_json, inserted_at_ms) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(reservation.reservation_id.as_str())
    .bind(mutation.trade_id.as_str())
    .bind(reservation.candidate_id.as_str())
    .bind(claim_mutation_id.as_str())
    .bind(reservation.inventory_authority_id.as_str())
    .bind(i64_from_u64("inventory_epoch", reservation.inventory_epoch)?)
    .bind(reservation.assertion_commitment.as_str())
    .bind(i64_from_u64(
        "reservation_expires_at_unix_s",
        reservation.reservation_expires_at_unix_s,
    )?)
    .bind(reservation_json.as_str())
    .bind(inserted_at_ms)
    .execute(&mut **tx)
    .await?;
    for (index, line) in reservation.commitments.iter().enumerate() {
        sqlx::query(
            "INSERT OR IGNORE INTO seller_inventory_reservation_line(reservation_id, line_id, bin_id, quantity_mantissa, quantity_scale, unit_code, line_index) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(reservation.reservation_id.as_str())
        .bind(line.line_id.as_str())
        .bind(line.bin_id.as_str())
        .bind(line.quantity_mantissa.as_str())
        .bind(i64::from(line.quantity_scale))
        .bind(line.unit_code.as_str())
        .bind(i64_from_usize("reservation.line_index", index)?)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

fn candidate_id_for_mutation(
    mutation: &RadrootsTradeMutationEnvelopeV1,
) -> Option<RadrootsTradeCandidateId> {
    match &mutation.body {
        RadrootsTradeMutationBodyV1::Proposal { candidate }
        | RadrootsTradeMutationBodyV1::RevisionProposal { candidate } => {
            candidate.candidate_id.clone()
        }
        RadrootsTradeMutationBodyV1::Decision { candidate_id, .. }
        | RadrootsTradeMutationBodyV1::RevisionDecision { candidate_id, .. } => {
            Some(candidate_id.clone())
        }
        RadrootsTradeMutationBodyV1::Cancellation {
            target_candidate_id,
            ..
        } => target_candidate_id.clone(),
    }
}

fn proposal_mutation_id_for_mutation(
    mutation: &RadrootsTradeMutationEnvelopeV1,
) -> Option<RadrootsTradeMutationId> {
    match &mutation.body {
        RadrootsTradeMutationBodyV1::Decision {
            proposal_mutation_id,
            ..
        }
        | RadrootsTradeMutationBodyV1::RevisionDecision {
            proposal_mutation_id,
            ..
        } => Some(proposal_mutation_id.clone()),
        _ => None,
    }
}

fn target_claim_mutation_id_for_mutation(
    mutation: &RadrootsTradeMutationEnvelopeV1,
) -> Option<RadrootsTradeMutationId> {
    match &mutation.body {
        RadrootsTradeMutationBodyV1::Cancellation {
            target_claim_mutation_id,
            ..
        } => target_claim_mutation_id.clone(),
        _ => None,
    }
}

fn seller_reservation_for_mutation(
    mutation: &RadrootsTradeMutationEnvelopeV1,
) -> Option<&RadrootsSellerReservationAssertionV1> {
    match &mutation.body {
        RadrootsTradeMutationBodyV1::Decision {
            decision:
                RadrootsTradeDecisionV1::Accepted {
                    reservation_assertion: Some(reservation),
                },
            ..
        }
        | RadrootsTradeMutationBodyV1::RevisionDecision {
            decision:
                RadrootsTradeDecisionV1::Accepted {
                    reservation_assertion: Some(reservation),
                },
            ..
        } => Some(reservation),
        _ => None,
    }
}

fn trade_mutation_kind_storage_value(kind: RadrootsTradeMutationKindV1) -> &'static str {
    match kind {
        RadrootsTradeMutationKindV1::Proposal => "proposal",
        RadrootsTradeMutationKindV1::Decision => "decision",
        RadrootsTradeMutationKindV1::RevisionProposal => "revision_proposal",
        RadrootsTradeMutationKindV1::RevisionDecision => "revision_decision",
        RadrootsTradeMutationKindV1::Cancellation => "cancellation",
    }
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

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

async fn ingest_event_in_transaction(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    ingest: RadrootsEventIngest,
) -> Result<RadrootsEventIngestReceipt, RadrootsEventStoreError> {
    let event = ingest.event();
    let admission_result = admit_verified_event(ingest.verified_event().clone());
    let admission = EventAdmission::from_result(&admission_result);
    let kind_class = event.kind_class();
    let valid_stream_eligible = admission.valid_stream_eligible(kind_class);
    if kind_class == RadrootsEventKindClass::Ephemeral {
        return Ok(RadrootsEventIngestReceipt {
            persistence: RadrootsEventPersistence::NotPersisted,
            event_id: event.id_str().to_owned(),
            admission_status: admission.status,
            admission_code: admission.code,
            contract_id: admission.contract.map(|contract| contract.id.to_owned()),
            valid_stream_eligible: false,
            raw_head_decision: RadrootsRawHeadDecision::NotPersisted,
        });
    }
    let tags = event.tags_as_vec();
    let tags_json = serde_json::to_string(&tags)?;
    let event_id = event.id_str().to_owned();
    let insert = insert_raw_event(
        tx,
        &ingest,
        &admission,
        valid_stream_eligible,
        ingest.raw_json(),
        tags_json.as_str(),
    )
    .await?;
    let inserted = insert.inserted;
    if inserted {
        insert_tags(tx, event, admission.contract).await?;
        if let Some(contract) = admission.contract
            && insert.valid_stream_eligible
            && is_trade_mutation_contract_id(contract.id)
        {
            store_trade_mutation_event(tx, &ingest, insert.seq).await?;
        }
    }
    let raw_head_decision = apply_raw_event_head(tx, event, ingest.observed_at_ms())
        .await?
        .decision;

    if let Some(observation) = ingest.transport_observation() {
        upsert_observation(tx, event_id.as_str(), observation).await?;
    }

    Ok(RadrootsEventIngestReceipt {
        persistence: if inserted {
            RadrootsEventPersistence::Inserted { seq: insert.seq }
        } else {
            RadrootsEventPersistence::Duplicate { seq: insert.seq }
        },
        event_id,
        admission_status: insert.admission_status,
        admission_code: inserted.then_some(admission.code).flatten(),
        contract_id: insert.contract_id,
        valid_stream_eligible: insert.valid_stream_eligible,
        raw_head_decision,
    })
}

async fn insert_raw_event(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    ingest: &RadrootsEventIngest,
    admission: &EventAdmission,
    valid_stream_eligible: bool,
    raw_json: &str,
    tags_json: &str,
) -> Result<InsertRawEventResult, RadrootsEventStoreError> {
    let event = ingest.event();
    let contract_id = admission.contract.map(|contract| contract.id);
    let event_class = StoredEventClass::from_event_kind_class(event.kind_class()).as_str();
    let result = sqlx::query(
        "INSERT OR IGNORE INTO event_envelopes(event_id, pubkey, created_at, kind, tags_json, content, sig, raw_json, verification_status, contract_status, contract_id, event_class, projection_eligible, inserted_at_ms, updated_at_ms) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(event.id_str())
    .bind(event.author_str())
    .bind(i64_from_u64("created_at", event.created_at_u64())?)
    .bind(i64::from(event.kind_u32()))
    .bind(tags_json)
    .bind(event.content())
    .bind(event.sig_str())
    .bind(raw_json)
    .bind("verified")
    .bind(admission.status.as_str())
    .bind(contract_id)
    .bind(event_class)
    .bind(bool_i64(valid_stream_eligible))
    .bind(ingest.observed_at_ms())
    .bind(ingest.observed_at_ms())
    .execute(&mut **tx)
    .await?;
    let inserted = result.rows_affected() > 0;
    let seq = event_seq(tx, event.id_str()).await?;
    if inserted {
        return Ok(InsertRawEventResult {
            inserted: true,
            seq,
            admission_status: admission.status,
            contract_id: contract_id.map(str::to_owned),
            valid_stream_eligible,
        });
    }

    let existing = stored_raw_event_row_in_transaction(tx, event.id_str()).await?;
    let stored = stored_raw_event_from_row(existing)?;
    Ok(InsertRawEventResult {
        inserted: false,
        seq: stored.seq,
        admission_status: stored.admission_status,
        contract_id: stored.contract_id,
        valid_stream_eligible: stored.valid_stream_eligible,
    })
}

async fn stored_raw_event_row_in_transaction(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    event_id: &str,
) -> Result<sqlx::sqlite::SqliteRow, RadrootsEventStoreError> {
    sqlx::query(
        "SELECT seq, event_id, pubkey, created_at, kind, tags_json, content, sig, raw_json, verification_status, contract_status, contract_id, event_class, projection_eligible, inserted_at_ms, updated_at_ms FROM event_envelopes WHERE event_id = ?",
    )
    .bind(event_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(Into::into)
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn event_seq(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    event_id: &str,
) -> Result<i64, RadrootsEventStoreError> {
    let row = sqlx::query("SELECT seq FROM event_envelopes WHERE event_id = ?")
        .bind(event_id)
        .fetch_one(&mut **tx)
        .await?;
    row.try_get("seq").map_err(Into::into)
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn insert_tags(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    event: &RadrootsEventEnvelope,
    contract: Option<&'static RadrootsEventContract>,
) -> Result<(), RadrootsEventStoreError> {
    for (index, tag) in event.tag_slices().iter().enumerate() {
        let tag_values = tag.as_slice();
        let tag_name = tag_values.first().map(String::as_str).unwrap_or("");
        let tag_value = tag_values.get(1).map(String::as_str);
        let tag_json = serde_json::to_string(tag_values)?;
        let tag_contract = contract.and_then(|contract| {
            contract
                .tags
                .iter()
                .find(|candidate| candidate.name == tag_name)
        });
        let contract_semantic = tag_contract.map(|tag| tag_semantic_name(tag.semantic));
        let contract_value_type = tag_contract.map(|tag| tag_value_type_name(tag.value_type));
        let relay_indexed = tag_contract.map(|tag| tag.relay_indexed).unwrap_or(false);
        sqlx::query(
            "INSERT INTO event_envelope_tags(event_id, tag_index, tag_name, tag_value, tag_json, contract_semantic, contract_value_type, relay_indexed) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(event.id_str())
        .bind(i64::try_from(index).map_err(|_| RadrootsEventStoreError::IntegerRange {
            field: "tag_index",
            value: i64::MAX,
        })?)
        .bind(tag_name)
        .bind(tag_value)
        .bind(tag_json.as_str())
        .bind(contract_semantic)
        .bind(contract_value_type)
        .bind(bool_i64(relay_indexed))
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn upsert_observation(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    event_id: &str,
    observation: &RadrootsTransportObservation,
) -> Result<(), RadrootsEventStoreError> {
    sqlx::query(
        "INSERT INTO event_transport_observation(event_id, transport_kind, endpoint_uri, endpoint_fingerprint, observation_type, first_observed_at_ms, last_observed_at_ms, observation_count, redacted_message) VALUES (?, ?, ?, ?, ?, ?, ?, 1, ?) ON CONFLICT(event_id, transport_kind, endpoint_fingerprint, observation_type) DO UPDATE SET endpoint_uri = CASE WHEN excluded.last_observed_at_ms >= event_transport_observation.last_observed_at_ms THEN excluded.endpoint_uri ELSE event_transport_observation.endpoint_uri END, first_observed_at_ms = min(event_transport_observation.first_observed_at_ms, excluded.first_observed_at_ms), last_observed_at_ms = max(event_transport_observation.last_observed_at_ms, excluded.last_observed_at_ms), observation_count = event_transport_observation.observation_count + 1, redacted_message = CASE WHEN excluded.last_observed_at_ms >= event_transport_observation.last_observed_at_ms AND excluded.redacted_message IS NOT NULL THEN excluded.redacted_message ELSE event_transport_observation.redacted_message END",
    )
    .bind(event_id)
    .bind(observation.transport_kind.canonical_label())
    .bind(observation.endpoint_uri.as_str())
    .bind(observation.endpoint_fingerprint.as_str())
    .bind(observation.observation_type.as_str())
    .bind(observation.observed_at_ms)
    .bind(observation.observed_at_ms)
    .bind(observation.redacted_message.as_deref())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn apply_raw_event_head(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    event: &RadrootsEventEnvelope,
    updated_at_ms: i64,
) -> Result<AppliedHead, RadrootsEventStoreError> {
    let candidate = match event_head_candidate_for_nip01_event(event) {
        RadrootsEventHeadCandidateResult::Candidate(candidate) => candidate,
        RadrootsEventHeadCandidateResult::NotHeadSelected => {
            return Ok(AppliedHead {
                decision: RadrootsRawHeadDecision::NotHeadSelected,
            });
        }
        RadrootsEventHeadCandidateResult::NotPersisted => {
            return Ok(AppliedHead {
                decision: RadrootsRawHeadDecision::NotPersisted,
            });
        }
        RadrootsEventHeadCandidateResult::Malformed(_) => {
            return Ok(AppliedHead {
                decision: RadrootsRawHeadDecision::MalformedCoordinate,
            });
        }
    };
    let current = current_event_head(tx, &candidate.coordinate).await?;
    let protocol_decision = select_event_head(candidate.clone(), current.as_ref());
    if let RadrootsEventHeadDecision::Applied(head) = &protocol_decision {
        upsert_head(tx, &candidate, head, updated_at_ms).await?;
    }
    Ok(AppliedHead {
        decision: RadrootsRawHeadDecision::from_protocol(&protocol_decision),
    })
}

async fn current_event_head(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    coordinate: &RadrootsEventHeadCoordinate,
) -> Result<Option<RadrootsCurrentEventHead>, RadrootsEventStoreError> {
    let snapshot = raw_head_snapshot_in_transaction(tx, coordinate).await?;
    snapshot
        .map(|snapshot| {
            Ok(RadrootsCurrentEventHead {
                coordinate: coordinate.clone(),
                event_id: RadrootsEventId::parse(snapshot.raw_head.event_id)?,
                created_at: snapshot.raw_head.created_at,
            })
        })
        .transpose()
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn upsert_head(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    candidate: &RadrootsEventHeadCandidate,
    head: &RadrootsCurrentEventHead,
    updated_at_ms: i64,
) -> Result<(), RadrootsEventStoreError> {
    match &head.coordinate {
        RadrootsEventHeadCoordinate::Replaceable { kind, pubkey } => {
            sqlx::query(
                "DELETE FROM event_envelope_head WHERE coordinate_type = 'replaceable' AND kind = ? AND pubkey = ? AND d_tag IS NULL",
            )
            .bind(i64::from(*kind))
            .bind(pubkey.as_str())
            .execute(&mut **tx)
            .await?;
            sqlx::query(
                "INSERT INTO event_envelope_head(coordinate_type, kind, pubkey, d_tag, event_id, created_at, updated_at_ms) VALUES ('replaceable', ?, ?, NULL, ?, ?, ?)",
            )
            .bind(i64::from(*kind))
            .bind(pubkey.as_str())
            .bind(candidate.event_id.as_str())
            .bind(i64_from_u64("created_at", candidate.created_at)?)
            .bind(updated_at_ms)
            .execute(&mut **tx)
            .await?;
        }
        RadrootsEventHeadCoordinate::Addressable {
            kind,
            pubkey,
            d_tag,
        } => {
            sqlx::query(
                "DELETE FROM event_envelope_head WHERE coordinate_type = 'addressable' AND kind = ? AND pubkey = ? AND d_tag = ?",
            )
            .bind(i64::from(*kind))
            .bind(pubkey.as_str())
            .bind(d_tag.as_str())
            .execute(&mut **tx)
            .await?;
            sqlx::query(
                "INSERT INTO event_envelope_head(coordinate_type, kind, pubkey, d_tag, event_id, created_at, updated_at_ms) VALUES ('addressable', ?, ?, ?, ?, ?, ?)",
            )
            .bind(i64::from(*kind))
            .bind(pubkey.as_str())
            .bind(d_tag.as_str())
            .bind(candidate.event_id.as_str())
            .bind(i64_from_u64("created_at", candidate.created_at)?)
            .bind(updated_at_ms)
            .execute(&mut **tx)
            .await?;
        }
    }
    Ok(())
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn stored_raw_event_from_row(
    row: sqlx::sqlite::SqliteRow,
) -> Result<RadrootsStoredRawEvent, RadrootsEventStoreError> {
    let kind = u32_from_i64("kind", row.try_get("kind")?)?;
    let created_at = u64_from_i64("created_at", row.try_get("created_at")?)?;
    let event_id: String = row.try_get("event_id")?;
    let verification_status: String = row.try_get("verification_status")?;
    if verification_status != "verified" {
        return Err(RadrootsEventStoreError::StoredRawEventNotVerified {
            event_id,
            status: verification_status,
        });
    }
    let contract_status: String = row.try_get("contract_status")?;
    if is_legacy_contract_status(contract_status.as_str()) {
        return Err(
            RadrootsEventStoreError::StoredRawEventRequiresReconciliation {
                event_id,
                contract_status,
            },
        );
    }
    let admission_status = RadrootsEventAdmissionStatus::parse(contract_status.as_str())?;
    let event_class = row
        .try_get::<Option<String>, _>("event_class")?
        .ok_or_else(|| RadrootsEventStoreError::StoredRawEventMissingClass {
            event_id: event_id.clone(),
        })
        .and_then(|value| StoredEventClass::parse(value.as_str()))?;
    let projection_eligible: i64 = row.try_get("projection_eligible")?;
    let valid_stream_eligible = match projection_eligible {
        0 => false,
        1 => true,
        _ => {
            return Err(
                RadrootsEventStoreError::StoredRawEventClassificationInconsistent { event_id },
            );
        }
    };
    let contract_id: Option<String> = row.try_get("contract_id")?;
    if kind > u32::from(u16::MAX) {
        return Err(RadrootsEventStoreError::StoredRawEventClassificationInconsistent { event_id });
    }
    let expected_class =
        StoredEventClass::from_event_kind_class(RadrootsEventKind::new(kind).class());
    if expected_class == StoredEventClass::Ephemeral {
        return Err(RadrootsEventStoreError::StoredRawEventClassificationInconsistent { event_id });
    }
    let expected_eligible = admission_status == RadrootsEventAdmissionStatus::Admitted
        && expected_class != StoredEventClass::Ephemeral;
    let contract_id_is_consistent =
        (admission_status == RadrootsEventAdmissionStatus::Admitted) == contract_id.is_some();
    if event_class != expected_class
        || valid_stream_eligible != expected_eligible
        || !contract_id_is_consistent
    {
        return Err(RadrootsEventStoreError::StoredRawEventClassificationInconsistent { event_id });
    }
    Ok(RadrootsStoredRawEvent {
        seq: row.try_get("seq")?,
        event_id,
        pubkey: row.try_get("pubkey")?,
        created_at,
        kind,
        tags_json: row.try_get("tags_json")?,
        content: row.try_get("content")?,
        sig: row.try_get("sig")?,
        raw_json: row.try_get("raw_json")?,
        admission_status,
        contract_id,
        event_class,
        valid_stream_eligible,
        inserted_at_ms: row.try_get("inserted_at_ms")?,
        updated_at_ms: row.try_get("updated_at_ms")?,
    })
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

fn stored_raw_head_from_joined_row(
    row: &sqlx::sqlite::SqliteRow,
) -> Result<RadrootsStoredRawEventHead, RadrootsEventStoreError> {
    Ok(RadrootsStoredRawEventHead {
        coordinate_type: StoredEventClass::parse(
            row.try_get::<String, _>("raw_head_coordinate_type")?
                .as_str(),
        )?,
        kind: u32_from_i64("kind", row.try_get("raw_head_kind")?)?,
        pubkey: row.try_get("raw_head_pubkey")?,
        d_tag: row.try_get("raw_head_d_tag")?,
        event_id: row.try_get("raw_head_event_id")?,
        created_at: u64_from_i64("created_at", row.try_get("raw_head_created_at")?)?,
        updated_at_ms: row.try_get("raw_head_updated_at_ms")?,
    })
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn projection_cursor_from_row(
    row: sqlx::sqlite::SqliteRow,
) -> Result<RadrootsProjectionCursor, RadrootsEventStoreError> {
    let projection_id: String = row.try_get("projection_id")?;
    let last_event_seq: i64 = row.try_get("last_event_seq")?;
    if last_event_seq < 0 {
        return Err(RadrootsEventStoreError::InvalidProjectionCursor {
            projection_id,
            value: last_event_seq,
        });
    }
    Ok(RadrootsProjectionCursor {
        projection_id,
        projection_version: u32_from_i64("projection_version", row.try_get("projection_version")?)?,
        last_event_seq,
        updated_at_ms: row.try_get("updated_at_ms")?,
    })
}

async fn projection_cursor_unchecked(
    pool: &SqlitePool,
    projection_id: &str,
) -> Result<Option<RadrootsProjectionCursor>, RadrootsEventStoreError> {
    let row = sqlx::query(
        "SELECT projection_id, projection_version, last_event_seq, updated_at_ms FROM projection_cursor WHERE projection_id = ?",
    )
    .bind(projection_id)
    .fetch_optional(pool)
    .await?;
    row.map(projection_cursor_from_row).transpose()
}

async fn raw_head_snapshot_in_transaction(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    coordinate: &RadrootsEventHeadCoordinate,
) -> Result<Option<RawHeadSnapshot>, RadrootsEventStoreError> {
    let row = match coordinate {
        RadrootsEventHeadCoordinate::Replaceable { kind, pubkey } => {
            sqlx::query(
                "SELECT event.seq, event.event_id, event.pubkey, event.created_at, event.kind, event.tags_json, event.content, event.sig, event.raw_json, event.verification_status, event.contract_status, event.contract_id, event.event_class, event.projection_eligible, event.inserted_at_ms, event.updated_at_ms, head.coordinate_type AS raw_head_coordinate_type, head.kind AS raw_head_kind, head.pubkey AS raw_head_pubkey, head.d_tag AS raw_head_d_tag, head.event_id AS raw_head_event_id, head.created_at AS raw_head_created_at, head.updated_at_ms AS raw_head_updated_at_ms FROM event_envelope_head AS head LEFT JOIN event_envelopes AS event ON event.event_id = head.event_id WHERE head.coordinate_type = 'replaceable' AND head.kind = ? AND head.pubkey = ? AND head.d_tag IS NULL",
            )
            .bind(i64::from(*kind))
            .bind(pubkey.as_str())
            .fetch_optional(&mut **tx)
            .await?
        }
        RadrootsEventHeadCoordinate::Addressable {
            kind,
            pubkey,
            d_tag,
        } => {
            sqlx::query(
                "SELECT event.seq, event.event_id, event.pubkey, event.created_at, event.kind, event.tags_json, event.content, event.sig, event.raw_json, event.verification_status, event.contract_status, event.contract_id, event.event_class, event.projection_eligible, event.inserted_at_ms, event.updated_at_ms, head.coordinate_type AS raw_head_coordinate_type, head.kind AS raw_head_kind, head.pubkey AS raw_head_pubkey, head.d_tag AS raw_head_d_tag, head.event_id AS raw_head_event_id, head.created_at AS raw_head_created_at, head.updated_at_ms AS raw_head_updated_at_ms FROM event_envelope_head AS head LEFT JOIN event_envelopes AS event ON event.event_id = head.event_id WHERE head.coordinate_type = 'addressable' AND head.kind = ? AND head.pubkey = ? AND head.d_tag = ?",
            )
            .bind(i64::from(*kind))
            .bind(pubkey.as_str())
            .bind(d_tag.as_str())
            .fetch_optional(&mut **tx)
            .await?
        }
    };
    row.map(|row| {
        let raw_head = stored_raw_head_from_joined_row(&row)?;
        if row.try_get::<Option<String>, _>("event_id")?.is_none() {
            return Err(RadrootsEventStoreError::StoredHeadInconsistent {
                event_id: raw_head.event_id,
            });
        }
        let raw_event = stored_raw_event_from_row(row)?;
        validate_raw_head_snapshot(coordinate, &raw_head, &raw_event)?;
        Ok(RawHeadSnapshot {
            raw_head,
            raw_event,
        })
    })
    .transpose()
}

async fn visible_event_snapshot(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    event_id: &str,
) -> Result<Option<VisibleEventSnapshot>, RadrootsEventStoreError> {
    let row = sqlx::query(
        "SELECT seq, event_id, pubkey, created_at, kind, tags_json, content, sig, raw_json, verification_status, contract_status, contract_id, event_class, projection_eligible, inserted_at_ms, updated_at_ms FROM event_envelopes WHERE event_id = ?",
    )
    .bind(event_id)
    .fetch_optional(&mut **tx)
    .await?;
    let Some(row) = row else {
        return Ok(None);
    };
    let raw_event = stored_raw_event_from_row(row)?;
    let raw_head_event_id = match raw_event.event_class {
        StoredEventClass::Regular | StoredEventClass::Ephemeral => None,
        StoredEventClass::Replaceable | StoredEventClass::Addressable => {
            let coordinate = raw_head_coordinate_for_stored_event(&raw_event)?;
            raw_head_snapshot_in_transaction(tx, &coordinate)
                .await?
                .map(|snapshot| snapshot.raw_head.event_id)
        }
    };
    Ok(Some(VisibleEventSnapshot {
        raw_event,
        raw_head_event_id,
    }))
}

fn raw_head_coordinate_for_stored_event(
    event: &RadrootsStoredRawEvent,
) -> Result<RadrootsEventHeadCoordinate, RadrootsEventStoreError> {
    let inconsistent = || RadrootsEventStoreError::StoredHeadInconsistent {
        event_id: event.event_id.clone(),
    };
    let pubkey = radroots_event::ids::RadrootsPublicKey::parse(event.pubkey.clone())
        .map_err(|_| inconsistent())?;
    match event.event_class {
        StoredEventClass::Replaceable => Ok(RadrootsEventHeadCoordinate::Replaceable {
            kind: event.kind,
            pubkey,
        }),
        StoredEventClass::Addressable => {
            let tags: Vec<Vec<String>> =
                serde_json::from_str(event.tags_json.as_str()).map_err(|_| inconsistent())?;
            let d_tag = tags
                .iter()
                .find(|tag| tag.first().map(String::as_str) == Some("d"))
                .and_then(|tag| tag.get(1))
                .cloned()
                .unwrap_or_default();
            Ok(RadrootsEventHeadCoordinate::Addressable {
                kind: event.kind,
                pubkey,
                d_tag,
            })
        }
        StoredEventClass::Regular | StoredEventClass::Ephemeral => Err(inconsistent()),
    }
}

fn validate_raw_head_snapshot(
    requested_coordinate: &RadrootsEventHeadCoordinate,
    raw_head: &RadrootsStoredRawEventHead,
    raw_event: &RadrootsStoredRawEvent,
) -> Result<(), RadrootsEventStoreError> {
    let expected_coordinate = raw_head_coordinate_for_stored_event(raw_event)?;
    let stored_coordinate = match raw_head.coordinate_type {
        StoredEventClass::Replaceable if raw_head.d_tag.is_none() => {
            RadrootsEventHeadCoordinate::Replaceable {
                kind: raw_head.kind,
                pubkey: radroots_event::ids::RadrootsPublicKey::parse(raw_head.pubkey.clone())
                    .map_err(|_| RadrootsEventStoreError::StoredHeadInconsistent {
                        event_id: raw_head.event_id.clone(),
                    })?,
            }
        }
        StoredEventClass::Addressable => RadrootsEventHeadCoordinate::Addressable {
            kind: raw_head.kind,
            pubkey: radroots_event::ids::RadrootsPublicKey::parse(raw_head.pubkey.clone())
                .map_err(|_| RadrootsEventStoreError::StoredHeadInconsistent {
                    event_id: raw_head.event_id.clone(),
                })?,
            d_tag: raw_head.d_tag.clone().ok_or_else(|| {
                RadrootsEventStoreError::StoredHeadInconsistent {
                    event_id: raw_head.event_id.clone(),
                }
            })?,
        },
        _ => {
            return Err(RadrootsEventStoreError::StoredHeadInconsistent {
                event_id: raw_head.event_id.clone(),
            });
        }
    };
    if &stored_coordinate != requested_coordinate
        || stored_coordinate != expected_coordinate
        || raw_head.event_id != raw_event.event_id
        || raw_head.created_at != raw_event.created_at
    {
        return Err(RadrootsEventStoreError::StoredHeadInconsistent {
            event_id: raw_head.event_id.clone(),
        });
    }
    Ok(())
}

fn visibility_from_snapshot(
    snapshot: &VisibleEventSnapshot,
) -> Result<RadrootsEventVisibility, RadrootsEventStoreError> {
    let event = &snapshot.raw_event;
    match event.event_class {
        StoredEventClass::Ephemeral => Err(
            RadrootsEventStoreError::StoredRawEventClassificationInconsistent {
                event_id: event.event_id.clone(),
            },
        ),
        StoredEventClass::Regular
            if event.admission_status != RadrootsEventAdmissionStatus::Admitted =>
        {
            Ok(RadrootsEventVisibility::NotAdmitted)
        }
        StoredEventClass::Regular => Ok(RadrootsEventVisibility::Visible),
        StoredEventClass::Replaceable | StoredEventClass::Addressable => {
            if event.admission_status != RadrootsEventAdmissionStatus::Admitted {
                return Ok(RadrootsEventVisibility::NotAdmitted);
            }
            let raw_head_event_id = snapshot.raw_head_event_id.as_ref().ok_or_else(|| {
                RadrootsEventStoreError::StoredHeadCoordinateUnavailable {
                    event_id: event.event_id.clone(),
                }
            })?;
            if raw_head_event_id == &event.event_id {
                Ok(RadrootsEventVisibility::Visible)
            } else {
                Ok(RadrootsEventVisibility::NotCurrent {
                    raw_head_event_id: raw_head_event_id.clone(),
                })
            }
        }
    }
}

fn is_legacy_contract_status(value: &str) -> bool {
    matches!(
        value,
        "supported" | "unsupported_kind" | "unsupported_shape" | "ambiguous_shape"
    )
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
    let transport_kind = RadrootsTransportKind::parse(&transport_kind_label)?;
    let endpoint_uri = RadrootsTransportTargetUri::parse(&endpoint_uri_raw)?;
    let endpoint_fingerprint =
        RadrootsTransportTargetFingerprint::parse(&endpoint_fingerprint_raw)?;
    let expected_fingerprint =
        RadrootsTransportTargetFingerprint::from_target(&transport_kind, &endpoint_uri, None);
    if endpoint_fingerprint != expected_fingerprint {
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
    if observation_count <= 0 || first_observed_at_ms > last_observed_at_ms {
        return Err(RadrootsEventStoreError::InvalidStoredTransportObservation {
            event_id,
            first_observed_at_ms,
            last_observed_at_ms,
            observation_count,
        });
    }
    Ok(RadrootsTransportObservationRow {
        event_id,
        transport_kind,
        endpoint_uri,
        endpoint_fingerprint,
        observation_type: RadrootsTransportObservationType::parse(
            row.try_get("observation_type")?,
        )?,
        first_observed_at_ms,
        last_observed_at_ms,
        observation_count,
        redacted_message: row.try_get("redacted_message")?,
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

#[cfg_attr(coverage_nightly, coverage(off))]
fn i64_from_u64(field: &'static str, value: u64) -> Result<i64, RadrootsEventStoreError> {
    i64::try_from(value).map_err(|_| RadrootsEventStoreError::UnsignedIntegerRange { field, value })
}

fn i64_from_usize(field: &'static str, value: usize) -> Result<i64, RadrootsEventStoreError> {
    i64::try_from(value).map_err(|_| RadrootsEventStoreError::UnsignedIntegerRange {
        field,
        value: value as u64,
    })
}

fn bool_i64(value: bool) -> i64 {
    if value { 1 } else { 0 }
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
    use radroots_event::ids::{
        RadrootsClassifiedListingAddress, RadrootsInventoryBinId, RadrootsPublicKey,
    };
    use radroots_event::kinds::{
        KIND_CLASSIFIED_LISTING, KIND_GEOCHAT, KIND_POST, KIND_PROFILE, KIND_RELAY_AUTH,
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
    use radroots_event::wire::{RadrootsNip01EventWire, compute_canonical_nip01_event_id};

    const FIXTURE_ALICE_SECRET_KEY_HEX: &str =
        "10c5304d6c9ae3a1a16f7860f1cc8f5e3a76225a2663b3a989a0d775919b7df5";
    const FIXTURE_ALICE_PUBLIC_KEY_HEX: &str =
        "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df";

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
        let raw_event = test_event_builder(kind, content, tags)
            .custom_created_at(RadrootsNostrTimestamp::from_secs(u64::from(created_at)))
            .sign_with_keys(&fixture_keys())
            .expect("signed event");
        signed_event_from_raw_json(serde_json::to_string(&raw_event).expect("raw json"))
    }

    fn signed_event_from_raw_json(raw_json: String) -> RadrootsSignedEvent {
        let wire = RadrootsNip01EventWire::parse_json(raw_json.as_str()).expect("wire");
        RadrootsSignedEvent::from_wire_verified_id(wire, raw_json).expect("signed event")
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

    fn head_coordinate_for_event(event: &RadrootsSignedEvent) -> RadrootsEventHeadCoordinate {
        let RadrootsEventHeadCandidateResult::Candidate(candidate) =
            event_head_candidate_for_nip01_event(event.envelope())
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

    async fn assert_raw_head_inconsistent(
        store: &RadrootsEventStore,
        coordinate: &RadrootsEventHeadCoordinate,
    ) {
        assert!(matches!(
            store.raw_event_head(coordinate).await,
            Err(RadrootsEventStoreError::StoredHeadInconsistent { .. })
        ));
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
            let orphan = sqlx::query(
                "INSERT INTO event_envelope_tags(event_id, tag_index, tag_name, tag_value, tag_json, contract_semantic, contract_value_type, relay_indexed) VALUES ('missing', 0, 'd', 'value', '[\"d\",\"value\"]', NULL, NULL, 0)",
            )
            .execute(&mut **connection)
            .await;
            assert!(orphan.is_err());
        }
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
    async fn new_format_corruption_fails_closed_in_raw_valid_and_status_reads() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let event = signed_event(KIND_POST, 9, Vec::new(), "corruption target");
        store
            .ingest_event(RadrootsEventIngest::new(event.clone(), 900))
            .await
            .expect("ingest");

        sqlx::query("UPDATE event_envelopes SET projection_eligible = 2 WHERE event_id = ?")
            .bind(event.id_str())
            .execute(store.pool())
            .await
            .expect("corrupt eligibility");
        assert!(matches!(
            store.raw_event(event.id_str()).await,
            Err(RadrootsEventStoreError::StoredRawEventClassificationInconsistent { .. })
        ));
        assert!(matches!(
            store.valid_event(event.id_str()).await,
            Err(RadrootsEventStoreError::StoredRawEventClassificationInconsistent { .. })
        ));
        assert!(matches!(
            store.status_summary().await,
            Err(RadrootsEventStoreError::StoredRawEventClassificationInconsistent { .. })
        ));
        assert!(matches!(
            inspect_event_store_status(store.pool()).await,
            Err(RadrootsEventStoreError::StoredRawEventClassificationInconsistent { .. })
        ));

        sqlx::query(
            "UPDATE event_envelopes SET projection_eligible = 1, verification_status = 'signature_invalid' WHERE event_id = ?",
        )
        .bind(event.id_str())
        .execute(store.pool())
        .await
        .expect("corrupt verification");
        assert!(matches!(
            store.raw_event(event.id_str()).await,
            Err(RadrootsEventStoreError::StoredRawEventNotVerified { .. })
        ));
        assert!(matches!(
            store.status_summary().await,
            Err(RadrootsEventStoreError::StoredRawEventClassificationInconsistent { .. })
        ));

        sqlx::query(
            "UPDATE event_envelopes SET verification_status = 'verified', contract_id = NULL WHERE event_id = ?",
        )
        .bind(event.id_str())
        .execute(store.pool())
        .await
        .expect("corrupt contract id");
        assert!(matches!(
            store.raw_event(event.id_str()).await,
            Err(RadrootsEventStoreError::StoredRawEventClassificationInconsistent { .. })
        ));

        sqlx::query(
            "UPDATE event_envelopes SET contract_id = 'radroots.social.post.v1', event_class = 'replaceable' WHERE event_id = ?",
        )
        .bind(event.id_str())
        .execute(store.pool())
        .await
        .expect("corrupt class");
        assert!(matches!(
            store.raw_event(event.id_str()).await,
            Err(RadrootsEventStoreError::StoredRawEventClassificationInconsistent { .. })
        ));

        sqlx::query(
            "UPDATE event_envelopes SET event_class = 'regular', kind = 20001, projection_eligible = 0 WHERE event_id = ?",
        )
        .bind(event.id_str())
        .execute(store.pool())
        .await
        .expect("persist impossible ephemeral");
        assert!(matches!(
            store.raw_event(event.id_str()).await,
            Err(RadrootsEventStoreError::StoredRawEventClassificationInconsistent { .. })
        ));
        assert!(matches!(
            store.status_summary().await,
            Err(RadrootsEventStoreError::StoredRawEventClassificationInconsistent { .. })
        ));
    }

    #[tokio::test]
    async fn file_store_reopens_existing_schema() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let path = tempdir.path().join("event_store.sqlite");

        let first = RadrootsEventStore::open_file(&path).await.expect("first");
        assert_eq!(first.pragma_foreign_keys().await.expect("foreign_keys"), 1);
        drop(first);

        let second = RadrootsEventStore::open_file(&path).await.expect("second");
        assert_eq!(second.pragma_foreign_keys().await.expect("foreign_keys"), 1);
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
            RadrootsEventStoreSchemaStatus::Managed { version: 1 }
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
            RadrootsEventStoreSchemaStatus::Managed { version: 1 }
        );
    }

    #[tokio::test]
    async fn rollback_is_terminal_for_every_clone_of_the_store_pool() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let clone = store.clone();

        store
            .rollback_to_schema_version_and_close(1)
            .await
            .expect("terminal rollback");

        assert!(clone.pool().is_closed());
        assert!(matches!(
            clone.schema_status().await,
            Err(RadrootsEventStoreError::Sqlx(sqlx::Error::PoolClosed))
        ));
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
        let second = store.ingest_event(ingest).await.expect("second ingest");
        let stored = store
            .raw_event(event.id_str())
            .await
            .expect("get")
            .expect("stored");

        assert!(first.persistence.is_inserted());
        assert!(second.persistence.is_duplicate());
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
    async fn duplicate_uses_persisted_classification_and_preserves_first_raw_bytes() {
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
        .expect("persist alternate classification");
        let before: (String, String, String, i64) = sqlx::query_as(
            "SELECT sig, raw_json, tags_json, updated_at_ms FROM event_envelopes WHERE event_id = ?",
        )
        .bind(first_event.id_str())
        .fetch_one(store.pool())
        .await
        .expect("before");

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

        assert!(receipt.persistence.is_duplicate());
        assert_eq!(
            receipt.admission_status,
            RadrootsEventAdmissionStatus::Unsupported
        );
        assert_eq!(receipt.admission_code, None);
        assert_eq!(receipt.contract_id, None);
        assert!(!receipt.valid_stream_eligible);
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
            RadrootsEventAdmissionStatus::Unsupported
        );
        assert!(
            store
                .valid_event(first_event.id_str())
                .await
                .expect("valid event")
                .is_none()
        );
    }

    #[tokio::test]
    async fn legacy_duplicate_requires_reconciliation_without_mutating_any_raw_data() {
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
        .expect("legacy row");
        let before: (String, String, String, String, Option<String>, i64) = sqlx::query_as(
            "SELECT sig, raw_json, tags_json, contract_status, event_class, updated_at_ms FROM event_envelopes WHERE event_id = ?",
        )
        .bind(first_event.id_str())
        .fetch_one(store.pool())
        .await
        .expect("before");

        let error = store
            .ingest_event(RadrootsEventIngest::new(second_event, 1_400))
            .await
            .expect_err("legacy duplicate");
        let after: (String, String, String, String, Option<String>, i64) = sqlx::query_as(
            "SELECT sig, raw_json, tags_json, contract_status, event_class, updated_at_ms FROM event_envelopes WHERE event_id = ?",
        )
        .bind(first_event.id_str())
        .fetch_one(store.pool())
        .await
        .expect("after");

        assert!(matches!(
            error,
            RadrootsEventStoreError::StoredRawEventRequiresReconciliation { .. }
        ));
        assert_eq!(after, before);
        assert_eq!(after.0, first_event.sig_str());
        assert_eq!(after.1, first_event.raw_json());
        assert!(matches!(
            store.raw_event(first_event.id_str()).await,
            Err(RadrootsEventStoreError::StoredRawEventRequiresReconciliation { .. })
        ));
        assert!(matches!(
            store.valid_event(first_event.id_str()).await,
            Err(RadrootsEventStoreError::StoredRawEventRequiresReconciliation { .. })
        ));
        assert!(matches!(
            store.event_visibility(first_event.id_str()).await,
            Err(RadrootsEventStoreError::StoredRawEventRequiresReconciliation { .. })
        ));
        let status = store.status_summary().await.expect("legacy status");
        let inspected = inspect_event_store_status(store.pool())
            .await
            .expect("legacy pool status");
        assert_eq!(inspected, status);
        assert_eq!(status.total_events, 1);
        assert_eq!(status.valid_stream_events, 0);
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
        let proposal = canonical_trade_mutation_content(proposal_envelope()).expect("proposal");
        let decision =
            canonical_trade_mutation_content(decision_envelope(&proposal)).expect("decision");
        let RadrootsTradeMutationBodyV1::Decision {
            decision:
                RadrootsTradeDecisionV1::Accepted {
                    reservation_assertion: Some(reservation),
                },
            ..
        } = &decision.envelope.body
        else {
            unreachable!("accepted decision fixture");
        };
        let mut tx = store.pool().begin().await.expect("transaction");

        let mut invalid_epoch = reservation.clone();
        invalid_epoch.inventory_epoch = u64::MAX;
        assert!(matches!(
            insert_seller_reservation(
                &mut tx,
                &decision.envelope,
                &decision.mutation_id,
                &invalid_epoch,
                1,
            )
            .await,
            Err(RadrootsEventStoreError::UnsignedIntegerRange {
                field: "inventory_epoch",
                ..
            })
        ));

        let mut invalid_expiry = reservation.clone();
        invalid_expiry.reservation_expires_at_unix_s = u64::MAX;
        assert!(matches!(
            insert_seller_reservation(
                &mut tx,
                &decision.envelope,
                &decision.mutation_id,
                &invalid_expiry,
                1,
            )
            .await,
            Err(RadrootsEventStoreError::UnsignedIntegerRange {
                field: "reservation_expires_at_unix_s",
                ..
            })
        ));
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
        let receipt = store
            .ingest_event(RadrootsEventIngest::new(event.clone(), 2_000))
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
        assert_eq!(
            stored.admission_status,
            RadrootsEventAdmissionStatus::Unsupported
        );
        assert!(!stored.valid_stream_eligible);
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
        assert!(duplicate.persistence.is_duplicate());
        assert_eq!(
            duplicate.raw_head_decision,
            RadrootsRawHeadDecision::NotHeadSelected
        );
        assert_eq!(
            duplicate.admission_status,
            RadrootsEventAdmissionStatus::Unsupported
        );
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
            admitted_receipt.raw_head_decision,
            RadrootsRawHeadDecision::NotPersisted
        );
    }

    #[tokio::test]
    async fn event_head_helper_maps_not_persisted_candidates() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let event = signed_event(KIND_GEOCHAT, 17, Vec::new(), "hello");
        let mut tx = store.pool.begin().await.expect("tx");

        let head = apply_raw_event_head(&mut tx, event.envelope(), 2_280)
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
    async fn raw_and_visible_head_reads_reject_every_head_event_mismatch() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let fixture_pubkey =
            RadrootsPublicKey::parse(FIXTURE_ALICE_PUBLIC_KEY_HEX).expect("fixture pubkey");

        let kind_event = signed_event(10_001, 40, Vec::new(), "kind");
        store
            .ingest_event(RadrootsEventIngest::new(kind_event.clone(), 3_100))
            .await
            .expect("kind ingest");
        sqlx::query("UPDATE event_envelope_head SET kind = 10002 WHERE event_id = ?")
            .bind(kind_event.id_str())
            .execute(store.pool())
            .await
            .expect("corrupt kind");
        assert_raw_head_inconsistent(
            &store,
            &RadrootsEventHeadCoordinate::Replaceable {
                kind: 10_002,
                pubkey: fixture_pubkey.clone(),
            },
        )
        .await;

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
            .expect("corrupt author");
        assert_raw_head_inconsistent(
            &store,
            &RadrootsEventHeadCoordinate::Replaceable {
                kind: 10_003,
                pubkey: RadrootsPublicKey::parse(other_pubkey).expect("other pubkey"),
            },
        )
        .await;

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
        .expect("corrupt class");
        assert_raw_head_inconsistent(
            &store,
            &RadrootsEventHeadCoordinate::Addressable {
                kind: 10_004,
                pubkey: fixture_pubkey.clone(),
                d_tag: "wrong-class".to_owned(),
            },
        )
        .await;

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
            .expect("corrupt d");
        assert_raw_head_inconsistent(
            &store,
            &RadrootsEventHeadCoordinate::Addressable {
                kind: 39_980,
                pubkey: fixture_pubkey.clone(),
                d_tag: "wrong-d".to_owned(),
            },
        )
        .await;

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
        .expect("corrupt created at");
        assert_raw_head_inconsistent(&store, &created_coordinate).await;
        assert!(matches!(
            store.event_visibility(created_event.id_str()).await,
            Err(RadrootsEventStoreError::StoredHeadInconsistent { .. })
        ));
        assert!(matches!(
            store.visible_event(created_event.id_str()).await,
            Err(RadrootsEventStoreError::StoredHeadInconsistent { .. })
        ));
        assert!(matches!(
            store.visible_event_head(&created_coordinate).await,
            Err(RadrootsEventStoreError::StoredHeadInconsistent { .. })
        ));

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
            .expect("corrupt reference");
        assert_raw_head_inconsistent(&store, &reference_coordinate).await;

        let missing_event = signed_event(10_007, 47, Vec::new(), "missing reference");
        let missing_coordinate = head_coordinate_for_event(&missing_event);
        store
            .ingest_event(RadrootsEventIngest::new(missing_event.clone(), 3_107))
            .await
            .expect("missing ingest");
        sqlx::query("PRAGMA foreign_keys = OFF")
            .execute(store.pool())
            .await
            .expect("disable foreign keys");
        sqlx::query("UPDATE event_envelope_head SET event_id = ? WHERE event_id = ?")
            .bind(event_id('f'))
            .bind(missing_event.id_str())
            .execute(store.pool())
            .await
            .expect("remove reference");
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(store.pool())
            .await
            .expect("enable foreign keys");
        assert_raw_head_inconsistent(&store, &missing_coordinate).await;
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
    async fn tag_reads_reject_non_boolean_relay_indexed_values() {
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
        sqlx::query(
            "UPDATE event_envelope_tags SET relay_indexed = 2 WHERE event_id = ? AND tag_index = 0",
        )
        .bind(event.id_str())
        .execute(store.pool())
        .await
        .expect("corrupt relay_indexed");

        assert!(matches!(
            store.tags_for_event(event.id_str()).await,
            Err(RadrootsEventStoreError::InvalidStoredBoolean {
                field: "relay_indexed",
                value: 2,
            })
        ));
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
        .with_redacted_message("duplicate accepted");
        let ingest = RadrootsEventIngest::new(event.clone(), 4_100).with_observation(observation);
        store.ingest_event(ingest).await.expect("second");
        let observation = RadrootsTransportObservation::new(
            RadrootsTransportKind::Nostr,
            "wss://relay.local",
            crate::RadrootsTransportObservationType::Subscription,
            4_050,
        )
        .expect("observation")
        .with_redacted_message("stale duplicate");
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
            observations[0].redacted_message.as_deref(),
            Some("duplicate accepted")
        );

        let observation = RadrootsTransportObservation::new(
            RadrootsTransportKind::Nostr,
            "wss://relay.local",
            crate::RadrootsTransportObservationType::Subscription,
            4_100,
        )
        .expect("observation")
        .with_redacted_message("tie duplicate accepted");
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
            observations[0].redacted_message.as_deref(),
            Some("tie duplicate accepted")
        );

        let endpoint_observations = store
            .observations_for_endpoint(RadrootsTransportKind::Nostr, "WSS://RELAY.LOCAL")
            .await
            .expect("endpoint observations");
        assert_eq!(endpoint_observations, observations);
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
        .expect("remove head");
        let restored = store
            .ingest_event(RadrootsEventIngest::new(newer.clone(), 4_900))
            .await
            .expect("restore duplicate head");
        assert!(restored.persistence.is_duplicate());
        assert_eq!(restored.raw_head_decision, RadrootsRawHeadDecision::Applied);
        assert_eq!(
            store
                .raw_event_head(&coordinate)
                .await
                .expect("raw head")
                .expect("restored head")
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
    async fn projection_cursor_reads_reject_negative_persisted_sequences() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        sqlx::query(
            "INSERT INTO projection_cursor(projection_id, projection_version, last_event_seq, updated_at_ms) VALUES ('corrupt', 1, -1, 1)",
        )
        .execute(store.pool())
        .await
        .expect("insert corrupt cursor");

        assert!(matches!(
            store.projection_cursor("corrupt", 1).await,
            Err(RadrootsEventStoreError::InvalidProjectionCursor {
                projection_id,
                value: -1,
            }) if projection_id == "corrupt"
        ));
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
