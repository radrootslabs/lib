use crate::RadrootsEventStoreError;
use crate::migrations::{EVENT_STORE_MIGRATION_DOWN, EVENT_STORE_MIGRATION_UP};
use crate::model::{
    RadrootsEventContractStatus, RadrootsEventHeadStoreDecision, RadrootsEventIngest,
    RadrootsEventIngestReceipt, RadrootsEventStoreStatusSummary, RadrootsEventVerificationStatus,
    RadrootsProjectionCursor, RadrootsStoredEvent, RadrootsStoredEventHead, RadrootsStoredEventTag,
    RadrootsStoredSellerReservation, RadrootsStoredSellerReservationLine,
    RadrootsStoredTradeMissingParent, RadrootsStoredTradeMutation,
    RadrootsStoredTradeMutationParent, RadrootsStoredTradeTransportEnvelope,
    RadrootsTradeProjectionCheckpoint, RadrootsTransportObservation,
    RadrootsTransportObservationType, StoredEventClass, tag_semantic_name, tag_value_type_name,
};
use radroots_event::RadrootsEventEnvelope;
use radroots_event::contract::{
    RadrootsEventClass, RadrootsEventContract, identify_event_contract,
};
use radroots_event::event_head::{
    RadrootsCurrentEventHead, RadrootsEventHeadCandidate, RadrootsEventHeadCandidateResult,
    RadrootsEventHeadCoordinate, RadrootsEventHeadDecision, event_head_candidate_for_contract,
    select_event_head,
};
use radroots_event::ids::{
    RadrootsDTag, RadrootsEventId, RadrootsPublicKey, RadrootsTradeCandidateId, RadrootsTradeId,
    RadrootsTradeMutationId,
};
use radroots_event::trade::{
    RADROOTS_TRADE_MUTATION_CONTRACT_IDS, RadrootsSellerReservationAssertionV1,
    RadrootsTradeDecisionV1, RadrootsTradeMutationBodyV1, RadrootsTradeMutationEnvelopeV1,
    RadrootsTradeMutationKindV1, trade_mutation_from_canonical_content,
};
use radroots_nostr::prelude::{RadrootsNostrEventVerification, radroots_nostr_verify_event};
use radroots_transport::{
    RadrootsTransportKind, RadrootsTransportTargetFingerprint, RadrootsTransportTargetUri,
};
use sha2::{Digest, Sha256};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use std::path::Path;
use std::str::FromStr;

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
        configure_connection(&pool, false).await?;
        apply_up(&pool).await?;
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
        configure_connection(&pool, true).await?;
        apply_up(&pool).await?;
        Ok(Self { pool })
    }

    pub async fn open_pool(
        pool: SqlitePool,
        file_backed: bool,
    ) -> Result<Self, RadrootsEventStoreError> {
        configure_connection(&pool, file_backed).await?;
        apply_up(&pool).await?;
        Ok(Self { pool })
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn migrate_down(&self) -> Result<(), RadrootsEventStoreError> {
        apply_down(&self.pool).await
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
        let row = sqlx::query(
            "SELECT COUNT(*) AS total_events, COALESCE(SUM(CASE WHEN projection_eligible = 1 THEN 1 ELSE 0 END), 0) AS projection_eligible_events, MAX(seq) AS last_event_seq, MAX(updated_at_ms) AS last_event_updated_at_ms FROM event_envelopes",
        )
        .fetch_one(&self.pool)
        .await?;
        let transport_observations = query_i64(
            &self.pool,
            "SELECT COUNT(*) FROM event_transport_observation",
        )
        .await?;
        Ok(RadrootsEventStoreStatusSummary {
            total_events: row.try_get("total_events")?,
            projection_eligible_events: row.try_get("projection_eligible_events")?,
            transport_observations,
            last_event_seq: row.try_get("last_event_seq")?,
            last_event_updated_at_ms: row.try_get("last_event_updated_at_ms")?,
        })
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

    pub async fn get_event(
        &self,
        event_id: &str,
    ) -> Result<Option<RadrootsStoredEvent>, RadrootsEventStoreError> {
        let row = sqlx::query(
            "SELECT seq, event_id, pubkey, created_at, kind, tags_json, content, sig, raw_json, verification_status, contract_status, contract_id, event_class, projection_eligible, inserted_at_ms, updated_at_ms FROM event_envelopes WHERE event_id = ?",
        )
        .bind(event_id)
        .fetch_optional(&self.pool)
        .await?;
        row.map(stored_event_from_row).transpose()
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

    pub async fn event_head(
        &self,
        coordinate: &RadrootsEventHeadCoordinate,
    ) -> Result<Option<RadrootsStoredEventHead>, RadrootsEventStoreError> {
        let row = match coordinate {
            RadrootsEventHeadCoordinate::Replaceable { kind, pubkey } => {
                sqlx::query(
                    "SELECT coordinate_type, kind, pubkey, d_tag, event_id, created_at, updated_at_ms FROM event_envelope_head WHERE coordinate_type = 'replaceable' AND kind = ? AND pubkey = ? AND d_tag IS NULL",
                )
                .bind(i64::from(*kind))
                .bind(pubkey.as_str())
                .fetch_optional(&self.pool)
                .await?
            }
            RadrootsEventHeadCoordinate::Addressable {
                kind,
                pubkey,
                d_tag,
            } => {
                sqlx::query(
                    "SELECT coordinate_type, kind, pubkey, d_tag, event_id, created_at, updated_at_ms FROM event_envelope_head WHERE coordinate_type = 'addressable' AND kind = ? AND pubkey = ? AND d_tag = ?",
                )
                .bind(i64::from(*kind))
                .bind(pubkey.as_str())
                .bind(d_tag.as_str())
                .fetch_optional(&self.pool)
                .await?
            }
        };
        row.map(stored_head_from_row).transpose()
    }

    pub async fn get_projection_cursor(
        &self,
        projection_id: &str,
    ) -> Result<Option<RadrootsProjectionCursor>, RadrootsEventStoreError> {
        let row = sqlx::query(
            "SELECT projection_id, projection_version, last_event_seq, updated_at_ms FROM projection_cursor WHERE projection_id = ?",
        )
        .bind(projection_id)
        .fetch_optional(&self.pool)
        .await?;
        row.map(projection_cursor_from_row).transpose()
    }

    pub async fn update_projection_cursor(
        &self,
        cursor: &RadrootsProjectionCursor,
    ) -> Result<(), RadrootsEventStoreError> {
        sqlx::query(
            "INSERT INTO projection_cursor(projection_id, projection_version, last_event_seq, updated_at_ms) VALUES (?, ?, ?, ?) ON CONFLICT(projection_id) DO UPDATE SET projection_version = excluded.projection_version, last_event_seq = excluded.last_event_seq, updated_at_ms = excluded.updated_at_ms",
        )
        .bind(cursor.projection_id.as_str())
        .bind(i64::from(cursor.projection_version))
        .bind(cursor.last_event_seq)
        .bind(cursor.updated_at_ms)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn events_since_cursor(
        &self,
        projection_id: &str,
        limit: u32,
    ) -> Result<Vec<RadrootsStoredEvent>, RadrootsEventStoreError> {
        let cursor = self.get_projection_cursor(projection_id).await?;
        let last_event_seq = cursor
            .as_ref()
            .map(|cursor| cursor.last_event_seq)
            .unwrap_or(0);
        let rows = sqlx::query(
            "SELECT seq, event_id, pubkey, created_at, kind, tags_json, content, sig, raw_json, verification_status, contract_status, contract_id, event_class, projection_eligible, inserted_at_ms, updated_at_ms FROM event_envelopes WHERE projection_eligible = 1 AND seq > ? ORDER BY seq ASC LIMIT ?",
        )
        .bind(last_event_seq)
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(stored_event_from_row).collect()
    }

    pub async fn events_by_tag(
        &self,
        tag_name: &str,
        tag_value: &str,
        limit: u32,
    ) -> Result<Vec<RadrootsStoredEvent>, RadrootsEventStoreError> {
        validate_tag_query(tag_name, limit)?;
        let rows = sqlx::query(
            "SELECT seq, event_id, pubkey, created_at, kind, tags_json, content, sig, raw_json, verification_status, contract_status, contract_id, event_class, projection_eligible, inserted_at_ms, updated_at_ms FROM event_envelopes AS event WHERE projection_eligible = 1 AND EXISTS (SELECT 1 FROM event_envelope_tags AS tag WHERE tag.event_id = event.event_id AND tag.tag_name = ? AND tag.tag_value = ?) ORDER BY event.seq ASC LIMIT ?",
        )
        .bind(tag_name)
        .bind(tag_value)
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(stored_event_from_row).collect()
    }

    pub async fn events_by_contract_and_tag<S>(
        &self,
        contract_ids: &[S],
        tag_name: &str,
        tag_value: &str,
        limit: u32,
    ) -> Result<Vec<RadrootsStoredEvent>, RadrootsEventStoreError>
    where
        S: AsRef<str>,
    {
        validate_contract_tag_query(contract_ids, tag_name, limit)?;
        let placeholders = core::iter::repeat_n("?", contract_ids.len())
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT seq, event_id, pubkey, created_at, kind, tags_json, content, sig, raw_json, verification_status, contract_status, contract_id, event_class, projection_eligible, inserted_at_ms, updated_at_ms FROM event_envelopes AS event WHERE projection_eligible = 1 AND contract_id IN ({placeholders}) AND EXISTS (SELECT 1 FROM event_envelope_tags AS tag WHERE tag.event_id = event.event_id AND tag.tag_name = ? AND tag.tag_value = ?) ORDER BY event.seq ASC LIMIT ?"
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
        rows.into_iter().map(stored_event_from_row).collect()
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

struct EventClassification {
    contract_status: RadrootsEventContractStatus,
    contract: Option<&'static RadrootsEventContract>,
}

impl EventClassification {
    fn base_projection_eligible(&self, verification: RadrootsEventVerificationStatus) -> bool {
        verification == RadrootsEventVerificationStatus::Verified
            && self
                .contract
                .map(|contract| contract.class != RadrootsEventClass::Ephemeral)
                .unwrap_or(false)
    }
}

struct AppliedHead {
    decision: RadrootsEventHeadStoreDecision,
    projection_eligible: bool,
}

struct InsertRawEventResult {
    inserted: bool,
    seq: i64,
}

async fn configure_connection(
    pool: &SqlitePool,
    file_backed: bool,
) -> Result<(), RadrootsEventStoreError> {
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
async fn apply_up(pool: &SqlitePool) -> Result<(), RadrootsEventStoreError> {
    sqlx::raw_sql(EVENT_STORE_MIGRATION_UP)
        .execute(pool)
        .await?;
    Ok(())
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn apply_down(pool: &SqlitePool) -> Result<(), RadrootsEventStoreError> {
    sqlx::raw_sql(EVENT_STORE_MIGRATION_DOWN)
        .execute(pool)
        .await?;
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

fn validate_event_identity(event: &RadrootsEventEnvelope) -> Result<(), RadrootsEventStoreError> {
    RadrootsEventId::parse(event.id_str())?;
    RadrootsPublicKey::parse(event.author_str())?;
    Ok(())
}

fn classify_event(event: &RadrootsEventEnvelope) -> EventClassification {
    let tags = event.tags_as_vec();
    match identify_event_contract(event.kind_u32(), &tags, event.content()) {
        Ok(contract) => EventClassification {
            contract_status: RadrootsEventContractStatus::Supported,
            contract: Some(contract),
        },
        Err(error) => EventClassification {
            contract_status: RadrootsEventContractStatus::from_match_error(error),
            contract: None,
        },
    }
}

fn is_trade_mutation_contract_id(contract_id: &str) -> bool {
    RADROOTS_TRADE_MUTATION_CONTRACT_IDS.contains(&contract_id)
}

async fn store_trade_mutation_event(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    ingest: &RadrootsEventIngest,
    contract: &RadrootsEventContract,
    event_seq: i64,
) -> Result<bool, RadrootsEventStoreError> {
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
                ingest.observed_at_ms,
            )
            .await?;
            return Ok(false);
        }
    };
    let Some(mutation_id) = parsed.mutation_id.clone() else {
        insert_trade_quarantine(
            tx,
            Some(parsed.trade_id.as_str()),
            None,
            Some(event.id_str()),
            "canonical trade mutation content is missing mutation_id",
            ingest.observed_at_ms,
        )
        .await?;
        return Ok(false);
    };
    if parsed.author_pubkey.as_str() != event.author_str() {
        insert_trade_quarantine(
            tx,
            Some(parsed.trade_id.as_str()),
            Some(mutation_id.as_str()),
            Some(event.id_str()),
            "trade mutation author_pubkey does not match transport event pubkey",
            ingest.observed_at_ms,
        )
        .await?;
        return Ok(false);
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
    .bind(ingest.observed_at_ms)
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
        ingest.observed_at_ms,
    )
    .await?;
    insert_missing_parent_records(
        tx,
        &parsed,
        &mutation_id,
        event.id_str(),
        ingest.observed_at_ms,
    )
    .await?;
    delete_resolved_missing_parent_records(tx, &mutation_id).await?;
    if let Some(reservation) = seller_reservation_for_mutation(&parsed) {
        insert_seller_reservation(
            tx,
            &parsed,
            &mutation_id,
            reservation,
            ingest.observed_at_ms,
        )
        .await?;
    }
    let _ = contract;
    Ok(true)
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

fn verify_event(event: &RadrootsEventEnvelope) -> RadrootsEventVerificationStatus {
    verification_status_from_nostr(radroots_nostr_verify_event(event))
}

fn verification_status_from_nostr(
    verification: RadrootsNostrEventVerification,
) -> RadrootsEventVerificationStatus {
    match verification {
        RadrootsNostrEventVerification::Verified => RadrootsEventVerificationStatus::Verified,
        RadrootsNostrEventVerification::IdVerified => RadrootsEventVerificationStatus::IdVerified,
        RadrootsNostrEventVerification::IdMismatch => RadrootsEventVerificationStatus::IdMismatch,
        RadrootsNostrEventVerification::SignatureInvalid => {
            RadrootsEventVerificationStatus::SignatureInvalid
        }
        RadrootsNostrEventVerification::MalformedEnvelope => {
            RadrootsEventVerificationStatus::MalformedEnvelope
        }
    }
}

async fn ingest_event_in_transaction(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    ingest: RadrootsEventIngest,
) -> Result<RadrootsEventIngestReceipt, RadrootsEventStoreError> {
    let event = ingest.event();
    validate_event_identity(event)?;
    let verification_status = verify_event(event);
    let classification = classify_event(event);
    let tags = event.tags_as_vec();
    let tags_json = serde_json::to_string(&tags)?;
    let event_id = event.id_str().to_owned();
    let insert = insert_raw_event(
        tx,
        &ingest,
        &classification,
        verification_status,
        ingest.raw_json(),
        tags_json.as_str(),
    )
    .await?;
    let inserted = insert.inserted;
    let mut head_decision = RadrootsEventHeadStoreDecision::Unsupported;
    let mut projection_eligible = classification.base_projection_eligible(verification_status);

    if inserted {
        insert_tags(tx, event, classification.contract).await?;
        if let Some(contract) = classification.contract {
            if projection_eligible {
                if is_trade_mutation_contract_id(contract.id) {
                    projection_eligible =
                        store_trade_mutation_event(tx, &ingest, contract, insert.seq).await?;
                    head_decision = if projection_eligible {
                        RadrootsEventHeadStoreDecision::NotHeadSelected
                    } else {
                        RadrootsEventHeadStoreDecision::Malformed
                    };
                } else {
                    let head = apply_event_head(tx, event, contract, ingest.observed_at_ms).await?;
                    projection_eligible = head.projection_eligible;
                    head_decision = head.decision;
                }
                sqlx::query(
                    "UPDATE event_envelopes SET projection_eligible = ?, updated_at_ms = ? WHERE event_id = ?",
                )
                .bind(bool_i64(projection_eligible))
                .bind(ingest.observed_at_ms)
                .bind(event_id.as_str())
                .execute(&mut **tx)
                .await?;
            } else {
                head_decision = RadrootsEventHeadStoreDecision::NotProjectionEligible;
            }
        }
    } else if classification.contract.is_some() {
        head_decision = RadrootsEventHeadStoreDecision::SkippedDuplicate;
        projection_eligible = false;
    }

    if let Some(observation) = ingest.transport_observation.as_ref() {
        upsert_observation(tx, event_id.as_str(), observation).await?;
    }

    Ok(RadrootsEventIngestReceipt {
        seq: insert.seq,
        event_id,
        inserted,
        verification_status,
        contract_status: classification.contract_status,
        contract_id: classification
            .contract
            .map(|contract| contract.id.to_owned()),
        projection_eligible,
        head_decision,
    })
}

async fn insert_raw_event(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    ingest: &RadrootsEventIngest,
    classification: &EventClassification,
    verification_status: RadrootsEventVerificationStatus,
    raw_json: &str,
    tags_json: &str,
) -> Result<InsertRawEventResult, RadrootsEventStoreError> {
    let event = ingest.event();
    let contract_id = classification.contract.map(|contract| contract.id);
    let event_class = classification
        .contract
        .map(|contract| StoredEventClass::from_event_class(contract.class).as_str());
    let projection_eligible = classification.base_projection_eligible(verification_status);
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
    .bind(verification_status.as_str())
    .bind(classification.contract_status.as_str())
    .bind(contract_id)
    .bind(event_class)
    .bind(bool_i64(projection_eligible))
    .bind(ingest.observed_at_ms)
    .bind(ingest.observed_at_ms)
    .execute(&mut **tx)
    .await?;
    let inserted = result.rows_affected() > 0;
    let seq = event_seq(tx, event.id_str()).await?;
    Ok(InsertRawEventResult { inserted, seq })
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

async fn apply_event_head(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    event: &RadrootsEventEnvelope,
    contract: &RadrootsEventContract,
    updated_at_ms: i64,
) -> Result<AppliedHead, RadrootsEventStoreError> {
    let candidate = match event_head_candidate_for_contract(event, contract) {
        RadrootsEventHeadCandidateResult::Candidate(candidate) => candidate,
        RadrootsEventHeadCandidateResult::NotHeadSelected => {
            return Ok(AppliedHead {
                decision: RadrootsEventHeadStoreDecision::NotHeadSelected,
                projection_eligible: true,
            });
        }
        RadrootsEventHeadCandidateResult::NotPersisted => {
            return Ok(AppliedHead {
                decision: RadrootsEventHeadStoreDecision::NotPersisted,
                projection_eligible: false,
            });
        }
        RadrootsEventHeadCandidateResult::Malformed(_) => {
            return Ok(AppliedHead {
                decision: RadrootsEventHeadStoreDecision::Malformed,
                projection_eligible: false,
            });
        }
    };
    let current = current_event_head(tx, &candidate.coordinate).await?;
    let protocol_decision = select_event_head(candidate.clone(), current.as_ref());
    if let RadrootsEventHeadDecision::Applied(head) = &protocol_decision {
        upsert_head(tx, &candidate, head, updated_at_ms).await?;
    }
    let projection_eligible = matches!(protocol_decision, RadrootsEventHeadDecision::Applied(_));
    Ok(AppliedHead {
        decision: RadrootsEventHeadStoreDecision::from_protocol(&protocol_decision),
        projection_eligible,
    })
}

async fn current_event_head(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    coordinate: &RadrootsEventHeadCoordinate,
) -> Result<Option<RadrootsCurrentEventHead>, RadrootsEventStoreError> {
    let row = match coordinate {
        RadrootsEventHeadCoordinate::Replaceable { kind, pubkey } => {
            sqlx::query(
                "SELECT event_id, created_at FROM event_envelope_head WHERE coordinate_type = 'replaceable' AND kind = ? AND pubkey = ? AND d_tag IS NULL",
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
                "SELECT event_id, created_at FROM event_envelope_head WHERE coordinate_type = 'addressable' AND kind = ? AND pubkey = ? AND d_tag = ?",
            )
            .bind(i64::from(*kind))
            .bind(pubkey.as_str())
            .bind(d_tag.as_str())
            .fetch_optional(&mut **tx)
            .await?
        }
    };
    row.map(|row| {
        let event_id: String = row.try_get("event_id")?;
        let created_at: i64 = row.try_get("created_at")?;
        Ok(RadrootsCurrentEventHead {
            coordinate: coordinate.clone(),
            event_id: RadrootsEventId::parse(event_id)?,
            created_at: u64_from_i64("created_at", created_at)?,
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
fn stored_event_from_row(
    row: sqlx::sqlite::SqliteRow,
) -> Result<RadrootsStoredEvent, RadrootsEventStoreError> {
    let kind = u32_from_i64("kind", row.try_get("kind")?)?;
    let created_at = u64_from_i64("created_at", row.try_get("created_at")?)?;
    let verification_status =
        RadrootsEventVerificationStatus::parse(row.try_get("verification_status")?)?;
    let contract_status =
        RadrootsEventContractStatus::parse(row.try_get("contract_status")?, kind)?;
    let event_class = row
        .try_get::<Option<String>, _>("event_class")?
        .map(|value| StoredEventClass::parse(value.as_str()))
        .transpose()?;
    let projection_eligible = row.try_get::<i64, _>("projection_eligible")? != 0;
    Ok(RadrootsStoredEvent {
        seq: row.try_get("seq")?,
        event_id: row.try_get("event_id")?,
        pubkey: row.try_get("pubkey")?,
        created_at,
        kind,
        tags_json: row.try_get("tags_json")?,
        content: row.try_get("content")?,
        sig: row.try_get("sig")?,
        raw_json: row.try_get("raw_json")?,
        verification_status,
        contract_status,
        contract_id: row.try_get("contract_id")?,
        event_class,
        projection_eligible,
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
        relay_indexed: row.try_get::<i64, _>("relay_indexed")? != 0,
    })
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn stored_head_from_row(
    row: sqlx::sqlite::SqliteRow,
) -> Result<RadrootsStoredEventHead, RadrootsEventStoreError> {
    Ok(RadrootsStoredEventHead {
        coordinate_type: StoredEventClass::parse(row.try_get("coordinate_type")?)?,
        kind: u32_from_i64("kind", row.try_get("kind")?)?,
        pubkey: row.try_get("pubkey")?,
        d_tag: row.try_get("d_tag")?,
        event_id: row.try_get("event_id")?,
        created_at: u64_from_i64("created_at", row.try_get("created_at")?)?,
        updated_at_ms: row.try_get("updated_at_ms")?,
    })
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn projection_cursor_from_row(
    row: sqlx::sqlite::SqliteRow,
) -> Result<RadrootsProjectionCursor, RadrootsEventStoreError> {
    Ok(RadrootsProjectionCursor {
        projection_id: row.try_get("projection_id")?,
        projection_version: u32_from_i64("projection_version", row.try_get("projection_version")?)?,
        last_event_seq: row.try_get("last_event_seq")?,
        updated_at_ms: row.try_get("updated_at_ms")?,
    })
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
    Ok(RadrootsTransportObservationRow {
        event_id,
        transport_kind,
        endpoint_uri,
        endpoint_fingerprint,
        observation_type: RadrootsTransportObservationType::parse(
            row.try_get("observation_type")?,
        )?,
        first_observed_at_ms: row.try_get("first_observed_at_ms")?,
        last_observed_at_ms: row.try_get("last_observed_at_ms")?,
        observation_count: row.try_get("observation_count")?,
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

fn validate_tag_query(tag_name: &str, limit: u32) -> Result<(), RadrootsEventStoreError> {
    if tag_name.is_empty() {
        return Err(RadrootsEventStoreError::EmptyTagName);
    }
    if !(1..=RADROOTS_EVENT_STORE_QUERY_LIMIT_MAX).contains(&limit) {
        return Err(RadrootsEventStoreError::QueryLimitOutOfRange {
            min: 1,
            max: RADROOTS_EVENT_STORE_QUERY_LIMIT_MAX,
            actual: limit,
        });
    }
    Ok(())
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
    if !(1..=RADROOTS_EVENT_STORE_QUERY_LIMIT_MAX).contains(&limit) {
        return Err(RadrootsEventStoreError::QueryLimitOutOfRange {
            min: 1,
            max: RADROOTS_EVENT_STORE_QUERY_LIMIT_MAX,
            actual: limit,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use nostr::EventBuilder;
    use radroots_event::draft::RadrootsSignedEvent;
    use radroots_event::event_head::event_head_candidate_for_event;
    use radroots_event::ids::{RadrootsClassifiedListingAddress, RadrootsInventoryBinId};
    use radroots_event::kinds::{KIND_CLASSIFIED_LISTING, KIND_GEOCHAT, KIND_POST, KIND_PROFILE};
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
    use radroots_nostr::prelude::{
        RadrootsNostrKeys, RadrootsNostrKind, RadrootsNostrSecretKey, RadrootsNostrTag,
        RadrootsNostrTagKind, RadrootsNostrTimestamp,
    };

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
            event_head_candidate_for_event(event.envelope()).expect("head candidate")
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

    #[test]
    fn verification_status_values_round_trip() {
        for status in [
            RadrootsEventVerificationStatus::NotChecked,
            RadrootsEventVerificationStatus::IdVerified,
            RadrootsEventVerificationStatus::Verified,
            RadrootsEventVerificationStatus::IdMismatch,
            RadrootsEventVerificationStatus::SignatureInvalid,
            RadrootsEventVerificationStatus::MalformedEnvelope,
        ] {
            assert_eq!(
                RadrootsEventVerificationStatus::parse(status.as_str()).expect("status"),
                status
            );
        }
        assert!(RadrootsEventVerificationStatus::parse("invalid").is_err());
    }

    #[test]
    fn verification_status_mapper_covers_all_nostr_results() {
        assert_eq!(
            verification_status_from_nostr(RadrootsNostrEventVerification::Verified),
            RadrootsEventVerificationStatus::Verified
        );
        assert_eq!(
            verification_status_from_nostr(RadrootsNostrEventVerification::IdVerified),
            RadrootsEventVerificationStatus::IdVerified
        );
        assert_eq!(
            verification_status_from_nostr(RadrootsNostrEventVerification::IdMismatch),
            RadrootsEventVerificationStatus::IdMismatch
        );
        assert_eq!(
            verification_status_from_nostr(RadrootsNostrEventVerification::SignatureInvalid),
            RadrootsEventVerificationStatus::SignatureInvalid
        );
        assert_eq!(
            verification_status_from_nostr(RadrootsNostrEventVerification::MalformedEnvelope),
            RadrootsEventVerificationStatus::MalformedEnvelope
        );
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
    async fn status_summary_counts_events_projections_and_transport_observations() {
        let store = RadrootsEventStore::open_memory().await.expect("open");

        let empty = store.status_summary().await.expect("empty status");
        assert_eq!(empty.total_events, 0);
        assert_eq!(empty.projection_eligible_events, 0);
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
        assert_eq!(status.total_events, 1);
        assert_eq!(status.projection_eligible_events, 1);
        assert_eq!(status.transport_observations, 1);
        assert_eq!(status.last_event_seq, Some(1));
        assert_eq!(status.last_event_updated_at_ms, Some(1_000));
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
    async fn migration_can_run_down() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        store.migrate_down().await.expect("down");

        let missing = sqlx::query("SELECT COUNT(*) FROM event_envelopes")
            .fetch_one(store.pool())
            .await
            .expect_err("table should be removed");
        assert!(missing.to_string().contains("event_envelopes"));
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
            .get_event(event.id_str())
            .await
            .expect("get")
            .expect("stored");

        assert!(first.inserted);
        assert!(!second.inserted);
        assert_eq!(first.seq, second.seq);
        assert_eq!(
            second.head_decision,
            RadrootsEventHeadStoreDecision::SkippedDuplicate
        );
        assert_eq!(
            first.verification_status,
            RadrootsEventVerificationStatus::Verified
        );
        assert_eq!(stored.seq, first.seq);
        assert_eq!(stored.raw_json, event.raw_json());
        assert_eq!(stored.content, "hello");
        assert_eq!(stored.tags_json, "[[\"t\",\"soil\"]]");
        assert_eq!(
            stored.contract_status,
            RadrootsEventContractStatus::Supported
        );
        assert!(stored.projection_eligible);
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
        assert!(decision_receipt.projection_eligible);

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
        assert!(receipt.projection_eligible);

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
    async fn malformed_trade_mutations_are_quarantined_and_not_projected() {
        let store = RadrootsEventStore::open_memory().await.expect("store");
        let proposal = canonical_trade_mutation_content(proposal_envelope()).expect("proposal");

        let malformed = signed_trade_content_with_keys(
            &proposal,
            format!("{} ", proposal.content),
            &fixture_keys(),
        );
        let malformed_receipt = store
            .ingest_event(RadrootsEventIngest::new(malformed, 2_400))
            .await
            .expect("malformed ingest");
        assert!(!malformed_receipt.projection_eligible);
        assert_eq!(
            malformed_receipt.head_decision,
            RadrootsEventHeadStoreDecision::Malformed
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
        let missing_id_receipt = store
            .ingest_event(RadrootsEventIngest::new(missing_id, 2_500))
            .await
            .expect("missing id ingest");
        assert!(!missing_id_receipt.projection_eligible);

        let mismatched_author =
            signed_trade_content_with_keys(&proposal, proposal.content.clone(), &alternate_keys());
        let mismatched_author_receipt = store
            .ingest_event(RadrootsEventIngest::new(mismatched_author, 2_600))
            .await
            .expect("mismatched author ingest");
        assert!(!mismatched_author_receipt.projection_eligible);

        let quarantined: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM trade_projection_quarantine")
                .fetch_one(store.pool())
                .await
                .expect("quarantine count");
        assert_eq!(quarantined, 3);
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
            .get_event(event.id_str())
            .await
            .expect("get")
            .expect("stored");

        assert_eq!(
            receipt.contract_status,
            RadrootsEventContractStatus::UnsupportedKind(999)
        );
        assert_eq!(
            stored.verification_status,
            RadrootsEventVerificationStatus::Verified
        );
        assert!(!stored.projection_eligible);

        let duplicate = store
            .ingest_event(RadrootsEventIngest::new(event, 2_100))
            .await
            .expect("duplicate");
        assert!(!duplicate.inserted);
        assert_eq!(
            duplicate.head_decision,
            RadrootsEventHeadStoreDecision::Unsupported
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
                .events_since_cursor("social", 10)
                .await
                .expect("events")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn signature_invalid_events_are_stored_but_not_projected() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let event = tamper_signature(&signed_event(KIND_POST, 13, Vec::new(), "hello"));
        let receipt = store
            .ingest_event(RadrootsEventIngest::new(event.clone(), 2_200))
            .await
            .expect("ingest");
        let stored = store
            .get_event(event.id_str())
            .await
            .expect("get")
            .expect("stored");

        assert_eq!(
            receipt.verification_status,
            RadrootsEventVerificationStatus::SignatureInvalid
        );
        assert_eq!(
            stored.verification_status,
            RadrootsEventVerificationStatus::SignatureInvalid
        );
        assert!(!stored.projection_eligible);
        assert!(
            store
                .events_since_cursor("social", 10)
                .await
                .expect("events")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn malformed_envelope_events_are_stored_but_not_projected() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let event = synthetic_signed_event(u32::from(u16::MAX) + 1, 13, Vec::new(), "hello");

        let receipt = store
            .ingest_event(RadrootsEventIngest::new(event.clone(), 2_250))
            .await
            .expect("ingest");
        let stored = store
            .get_event(event.id_str())
            .await
            .expect("get")
            .expect("stored");

        assert_eq!(
            receipt.verification_status,
            RadrootsEventVerificationStatus::MalformedEnvelope
        );
        assert_eq!(
            stored.verification_status,
            RadrootsEventVerificationStatus::MalformedEnvelope
        );
        assert!(!stored.projection_eligible);
    }

    #[tokio::test]
    async fn ephemeral_events_are_not_persisted_as_heads() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let event = signed_event(KIND_GEOCHAT, 15, Vec::new(), "hello");

        let receipt = store
            .ingest_event(RadrootsEventIngest::new(event.clone(), 2_260))
            .await
            .expect("ingest");
        let stored = store
            .get_event(event.id_str())
            .await
            .expect("get")
            .expect("stored");

        assert_eq!(
            receipt.contract_status,
            RadrootsEventContractStatus::Supported
        );
        assert_eq!(
            receipt.head_decision,
            RadrootsEventHeadStoreDecision::NotProjectionEligible
        );
        assert!(!receipt.projection_eligible);
        assert!(!stored.projection_eligible);
    }

    #[tokio::test]
    async fn event_head_helper_maps_not_persisted_candidates() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let event = signed_event(KIND_GEOCHAT, 17, Vec::new(), "hello");
        let classification = classify_event(event.envelope());
        let contract = classification.contract.expect("contract");
        let mut tx = store.pool.begin().await.expect("tx");

        let head = apply_event_head(&mut tx, event.envelope(), contract, 2_280)
            .await
            .expect("head");

        assert_eq!(head.decision, RadrootsEventHeadStoreDecision::NotPersisted);
        assert!(!head.projection_eligible);
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
            .get_event(event.id_str())
            .await
            .expect("get")
            .expect("stored");

        assert_eq!(
            receipt.contract_status,
            RadrootsEventContractStatus::UnsupportedShape(KIND_CLASSIFIED_LISTING)
        );
        assert_eq!(
            receipt.head_decision,
            RadrootsEventHeadStoreDecision::Unsupported
        );
        assert!(!receipt.projection_eligible);
        assert!(!stored.projection_eligible);
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
            .event_head(&coordinate)
            .await
            .expect("head")
            .expect("stored head");

        assert_eq!(first.head_decision, RadrootsEventHeadStoreDecision::Applied);
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

        let receipt = store
            .ingest_event(RadrootsEventIngest::new(invalid.clone(), 2_600))
            .await
            .expect("invalid");
        let head = store
            .event_head(&coordinate)
            .await
            .expect("head")
            .expect("stored head");

        assert_eq!(
            receipt.verification_status,
            RadrootsEventVerificationStatus::SignatureInvalid
        );
        assert_eq!(
            receipt.head_decision,
            RadrootsEventHeadStoreDecision::NotProjectionEligible
        );
        assert!(!receipt.projection_eligible);
        assert_eq!(head.event_id, original.id_str());
    }

    #[tokio::test]
    async fn duplicate_invalid_addressable_events_do_not_update_heads() {
        let store = RadrootsEventStore::open_memory().await.expect("open");
        let original = signed_event(
            KIND_CLASSIFIED_LISTING,
            21,
            operational_listing_tags("listing-3"),
            "{}",
        );
        store
            .ingest_event(RadrootsEventIngest::new(original.clone(), 2_700))
            .await
            .expect("original");
        let coordinate = head_coordinate_for_event(&original);
        let invalid = tamper_signature(&signed_event(
            KIND_CLASSIFIED_LISTING,
            22,
            operational_listing_tags("listing-3"),
            "{}",
        ));

        let first_invalid = store
            .ingest_event(RadrootsEventIngest::new(invalid.clone(), 2_800))
            .await
            .expect("first invalid");
        let second_invalid = store
            .ingest_event(RadrootsEventIngest::new(invalid.clone(), 2_900))
            .await
            .expect("second invalid");
        let head = store
            .event_head(&coordinate)
            .await
            .expect("head")
            .expect("stored head");

        assert!(first_invalid.inserted);
        assert!(!second_invalid.inserted);
        assert_eq!(first_invalid.seq, second_invalid.seq);
        assert_eq!(
            first_invalid.head_decision,
            RadrootsEventHeadStoreDecision::NotProjectionEligible
        );
        assert_eq!(
            second_invalid.head_decision,
            RadrootsEventHeadStoreDecision::SkippedDuplicate
        );
        assert_eq!(head.event_id, original.id_str());
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
            .event_head(&coordinate)
            .await
            .expect("head")
            .expect("stored head");

        assert!(first.inserted);
        assert!(!second.inserted);
        assert_eq!(first.seq, second.seq);
        assert_eq!(first.head_decision, RadrootsEventHeadStoreDecision::Applied);
        assert_eq!(
            second.head_decision,
            RadrootsEventHeadStoreDecision::SkippedDuplicate
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
            .get_event(event.id_str())
            .await
            .expect("get")
            .expect("stored");

        assert_eq!(
            receipt.verification_status,
            RadrootsEventVerificationStatus::Verified
        );
        assert_eq!(
            receipt.head_decision,
            RadrootsEventHeadStoreDecision::NotHeadSelected
        );
        assert!(receipt.projection_eligible);
        assert!(stored.projection_eligible);
    }

    #[tokio::test]
    async fn events_by_tag_validates_inputs_and_returns_projection_events_in_sequence_order() {
        let store = RadrootsEventStore::open_memory().await.expect("store");

        assert!(matches!(
            store.events_by_tag("", "soil", 1).await,
            Err(RadrootsEventStoreError::EmptyTagName)
        ));
        assert!(matches!(
            store.events_by_tag("t", "soil", 0).await,
            Err(RadrootsEventStoreError::QueryLimitOutOfRange { .. })
        ));
        assert!(matches!(
            store
                .events_by_tag("t", "soil", RADROOTS_EVENT_STORE_QUERY_LIMIT_MAX + 1)
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
            .events_by_tag("t", "soil", 10)
            .await
            .expect("tag query");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_id, high_created_at.id_str());
        assert_eq!(events[1].event_id, low_created_at.id_str());
        assert!(events.iter().all(|event| event.projection_eligible));

        let limited = store
            .events_by_tag("t", "soil", 1)
            .await
            .expect("limited tag query");
        assert_eq!(limited.len(), 1);
        assert_eq!(limited[0].event_id, high_created_at.id_str());
    }

    #[tokio::test]
    async fn events_by_contract_and_tag_enforces_trade_contract_tag_and_projection_filters() {
        let store = RadrootsEventStore::open_memory().await.expect("store");

        assert!(matches!(
            store
                .events_by_contract_and_tag::<&str>(&[], "p", FIXTURE_ALICE_PUBLIC_KEY_HEX, 1)
                .await,
            Err(RadrootsEventStoreError::EmptyContractList)
        ));
        let too_many_contracts = vec![
            RADROOTS_TRADE_PROPOSAL_CONTRACT_ID;
            RADROOTS_EVENT_STORE_CONTRACT_QUERY_LIMIT_MAX + 1
        ];
        assert!(matches!(
            store
                .events_by_contract_and_tag(
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
            .events_by_contract_and_tag(
                &[RADROOTS_TRADE_PROPOSAL_CONTRACT_ID],
                "p",
                FIXTURE_ALICE_PUBLIC_KEY_HEX,
                10,
            )
            .await
            .expect("contract tag query");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_id, matching_trade.id_str());
        assert_eq!(
            events[0].contract_id.as_deref(),
            Some(RADROOTS_TRADE_PROPOSAL_CONTRACT_ID)
        );
        assert!(events[0].projection_eligible);
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
            .event_head(&profile_coordinate())
            .await
            .expect("head")
            .expect("stored head");

        assert_eq!(first.head_decision, RadrootsEventHeadStoreDecision::Applied);
        assert_eq!(
            second.head_decision,
            RadrootsEventHeadStoreDecision::Applied
        );
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
            .event_head(&profile_coordinate())
            .await
            .expect("head")
            .expect("stored head");

        assert_eq!(
            second.head_decision,
            RadrootsEventHeadStoreDecision::SkippedSameTimestampHigherEventId
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
        assert!(first_receipt.seq < second_receipt.seq);

        let replay = store
            .events_since_cursor("social", 10)
            .await
            .expect("initial replay");
        assert_eq!(replay.len(), 2);
        assert_eq!(replay[0].event_id, first.id_str());
        assert_eq!(replay[1].event_id, second.id_str());
        store
            .update_projection_cursor(&RadrootsProjectionCursor {
                projection_id: "social".to_owned(),
                projection_version: 1,
                last_event_seq: first_receipt.seq,
                updated_at_ms: 6_200,
            })
            .await
            .expect("cursor");
        let replay = store
            .events_since_cursor("social", 10)
            .await
            .expect("next replay");
        assert_eq!(replay.len(), 1);
        assert_eq!(replay[0].event_id, second.id_str());
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
                last_transport_event_seq: Some(proposal_receipt.seq),
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
            assert!(receipt.inserted);
            assert_eq!(
                receipt.verification_status,
                RadrootsEventVerificationStatus::Verified
            );
        }

        let replay = store
            .events_since_cursor("smoke", 10_000)
            .await
            .expect("replay");
        assert_eq!(replay.len(), 10_000);
        assert_eq!(replay[0].seq, 1);
        assert_eq!(replay[9_999].seq, 10_000);

        store
            .update_projection_cursor(&RadrootsProjectionCursor {
                projection_id: "smoke".to_owned(),
                projection_version: 1,
                last_event_seq: replay[4_999].seq,
                updated_at_ms: 25_000,
            })
            .await
            .expect("cursor");
        let replay = store
            .events_since_cursor("smoke", 10_000)
            .await
            .expect("replay after cursor");
        assert_eq!(replay.len(), 5_000);
        assert_eq!(replay[0].seq, 5_001);
        assert_eq!(replay[4_999].seq, 10_000);
    }
}
