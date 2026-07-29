#[cfg(test)]
use super::protocol_reconciliation_v1::ingest_event_protocol_reconciliation_v1;
use crate::error::RadrootsEventStoreError;
#[cfg(test)]
use crate::model::RadrootsEventIngest;
use crate::model::RadrootsTransportObservation;
use radroots_event::envelope::EventEnvelope;
use radroots_event::id::{CandidateId, MutationId};
use radroots_event::trade::{
    SellerReservationAssertionV1, TradeMutationEnvelopeV1, TradeMutationKindV1,
};
use radroots_transport::RadrootsTransportKind;
use sqlx::{Sqlite, Transaction};

pub(super) struct PostCoreStorageV1<'borrow, 'db> {
    tx: &'borrow mut Transaction<'db, Sqlite>,
}

pub(super) struct TradeProjectionWrite<'a> {
    event: &'a EventEnvelope,
    stored_event_seq: i64,
    mutation: &'a TradeMutationEnvelopeV1,
    mutation_id: &'a MutationId,
    candidate_id: Option<&'a CandidateId>,
    proposal_mutation_id: Option<&'a MutationId>,
    target_claim_mutation_id: Option<&'a MutationId>,
    payload_sha256: &'a str,
    observed_at_ms: i64,
    reservation: Option<&'a SellerReservationAssertionV1>,
}

impl<'a> TradeProjectionWrite<'a> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        event: &'a EventEnvelope,
        stored_event_seq: i64,
        mutation: &'a TradeMutationEnvelopeV1,
        mutation_id: &'a MutationId,
        candidate_id: Option<&'a CandidateId>,
        proposal_mutation_id: Option<&'a MutationId>,
        target_claim_mutation_id: Option<&'a MutationId>,
        payload_sha256: &'a str,
        observed_at_ms: i64,
        reservation: Option<&'a SellerReservationAssertionV1>,
    ) -> Self {
        Self {
            event,
            stored_event_seq,
            mutation,
            mutation_id,
            candidate_id,
            proposal_mutation_id,
            target_claim_mutation_id,
            payload_sha256,
            observed_at_ms,
            reservation,
        }
    }
}

impl<'borrow, 'db> PostCoreStorageV1<'borrow, 'db> {
    pub(super) fn new(tx: &'borrow mut Transaction<'db, Sqlite>) -> Self {
        Self { tx }
    }

    pub(super) async fn quarantine_trade(
        &mut self,
        trade_id: Option<String>,
        mutation_id: Option<String>,
        transport_event_id: Option<String>,
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
        .execute(&mut **self.tx)
        .await?;
        Ok(())
    }

    pub(super) async fn persist_trade_projection(
        &mut self,
        write: TradeProjectionWrite<'_>,
    ) -> Result<(), RadrootsEventStoreError> {
        let root_mutation_id = write
            .mutation
            .root_mutation_id
            .as_ref()
            .map(|mutation_id| mutation_id.to_hex());
        let candidate_id = write.candidate_id.map(|candidate_id| candidate_id.to_hex());
        let proposal_mutation_id = write
            .proposal_mutation_id
            .map(|mutation_id| mutation_id.to_hex());
        let target_claim_mutation_id = write
            .target_claim_mutation_id
            .map(|mutation_id| mutation_id.to_hex());
        sqlx::query(
            "INSERT OR IGNORE INTO trade_mutation(mutation_id, trade_id, root_mutation_id, contract_id, mutation_kind, schema_version, candidate_id, proposal_mutation_id, target_claim_mutation_id, author_pubkey, counterparty_pubkey, buyer_pubkey, seller_pubkey, farm_id, authored_at_unix_s, canonical_payload_bytes, payload_sha256, first_event_seq, first_transport_event_id, inserted_at_ms) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(write.mutation_id.to_hex())
        .bind(write.mutation.trade_id.to_hex())
        .bind(root_mutation_id)
        .bind(write.mutation.contract_id.as_str())
        .bind(trade_mutation_kind_storage_value(
            write.mutation.mutation_kind(),
        ))
        .bind(i64::from(write.mutation.schema_version))
        .bind(candidate_id)
        .bind(proposal_mutation_id)
        .bind(target_claim_mutation_id)
        .bind(write.mutation.author_pubkey.to_hex())
        .bind(write.mutation.counterparty_pubkey.to_hex())
        .bind(write.mutation.buyer_pubkey.to_hex())
        .bind(write.mutation.seller_pubkey.to_hex())
        .bind(write.mutation.farm_id.as_str())
        .bind(i64_from_u64(
            "authored_at_unix_s",
            write.mutation.authored_at_unix_s,
        )?)
        .bind(write.event.content().as_bytes())
        .bind(write.payload_sha256)
        .bind(write.stored_event_seq)
        .bind(write.event.id_hex())
        .bind(write.observed_at_ms)
        .execute(&mut **self.tx)
        .await?;

        self.insert_trade_mutation_parents(write.mutation_id, &write.mutation.parent_mutation_ids)
            .await?;
        self.insert_trade_transport_envelope(&write).await?;
        self.insert_missing_parent_records(&write).await?;
        self.delete_resolved_missing_parent_records(write.mutation_id)
            .await?;
        if let Some(reservation) = write.reservation {
            self.insert_seller_reservation(
                write.mutation,
                write.mutation_id,
                reservation,
                write.observed_at_ms,
            )
            .await?;
        }

        #[cfg(test)]
        self.apply_raw_authority_forge(&write.event.id_hex())
            .await?;
        #[cfg(test)]
        self.apply_schema_forge(&write.event.id_hex()).await?;
        Ok(())
    }

    pub(super) async fn upsert_transport_observation(
        &mut self,
        event_id: &str,
        observation: &RadrootsTransportObservation,
    ) -> Result<(), RadrootsEventStoreError> {
        observation.validate_endpoint_for_event(event_id)?;
        sqlx::query(
            "INSERT INTO event_transport_observation(event_id, transport_kind, endpoint_uri, endpoint_fingerprint, observation_type, first_observed_at_ms, last_observed_at_ms, observation_count, redacted_message) VALUES (?, ?, ?, ?, ?, ?, ?, 1, ?) ON CONFLICT(event_id, transport_kind, endpoint_fingerprint, observation_type) DO UPDATE SET endpoint_uri = CASE WHEN excluded.last_observed_at_ms >= event_transport_observation.last_observed_at_ms THEN excluded.endpoint_uri ELSE event_transport_observation.endpoint_uri END, first_observed_at_ms = min(event_transport_observation.first_observed_at_ms, excluded.first_observed_at_ms), last_observed_at_ms = max(event_transport_observation.last_observed_at_ms, excluded.last_observed_at_ms), observation_count = event_transport_observation.observation_count + 1, redacted_message = CASE WHEN excluded.last_observed_at_ms >= event_transport_observation.last_observed_at_ms AND excluded.redacted_message IS NOT NULL THEN excluded.redacted_message ELSE event_transport_observation.redacted_message END",
        )
        .bind(event_id)
        .bind(observation.transport_kind().canonical_label())
        .bind(observation.endpoint_uri().as_str())
        .bind(observation.endpoint_fingerprint().as_str())
        .bind(observation.observation_type().as_str())
        .bind(observation.observed_at_ms())
        .bind(observation.observed_at_ms())
        .bind(observation.caller_redacted_message())
        .execute(&mut **self.tx)
        .await?;
        Ok(())
    }

    async fn insert_trade_mutation_parents(
        &mut self,
        mutation_id: &MutationId,
        parents: &[MutationId],
    ) -> Result<(), RadrootsEventStoreError> {
        for (index, parent) in parents.iter().enumerate() {
            sqlx::query(
                "INSERT OR IGNORE INTO trade_mutation_parent(mutation_id, parent_mutation_id, parent_index) VALUES (?, ?, ?)",
            )
            .bind(mutation_id.to_hex())
            .bind(parent.to_hex())
            .bind(i64_from_usize("parent_index", index)?)
            .execute(&mut **self.tx)
            .await?;
        }
        Ok(())
    }

    async fn insert_trade_transport_envelope(
        &mut self,
        write: &TradeProjectionWrite<'_>,
    ) -> Result<(), RadrootsEventStoreError> {
        sqlx::query(
            "INSERT OR IGNORE INTO trade_transport_envelope(transport_event_id, mutation_id, trade_id, transport_kind, pubkey, created_at, event_seq, payload_sha256, observed_at_ms) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(write.event.id_hex())
        .bind(write.mutation_id.to_hex())
        .bind(write.mutation.trade_id.to_hex())
        .bind(RadrootsTransportKind::Nostr.canonical_label())
        .bind(write.event.author().to_hex())
        .bind(i64_from_u64("created_at", write.event.created_at_u64())?)
        .bind(write.stored_event_seq)
        .bind(write.payload_sha256)
        .bind(write.observed_at_ms)
        .execute(&mut **self.tx)
        .await?;
        Ok(())
    }

    async fn insert_missing_parent_records(
        &mut self,
        write: &TradeProjectionWrite<'_>,
    ) -> Result<(), RadrootsEventStoreError> {
        for parent in &write.mutation.parent_mutation_ids {
            let exists: Option<i64> =
                sqlx::query_scalar("SELECT 1 FROM trade_mutation WHERE mutation_id = ? LIMIT 1")
                    .bind(parent.to_hex())
                    .fetch_optional(&mut **self.tx)
                    .await?;
            if exists.is_none() {
                sqlx::query(
                    "INSERT OR IGNORE INTO trade_missing_parent(trade_id, mutation_id, missing_parent_mutation_id, first_transport_event_id, first_seen_at_ms) VALUES (?, ?, ?, ?, ?)",
                )
                .bind(write.mutation.trade_id.to_hex())
                .bind(write.mutation_id.to_hex())
                .bind(parent.to_hex())
                .bind(write.event.id_hex())
                .bind(write.observed_at_ms)
                .execute(&mut **self.tx)
                .await?;
            }
        }
        Ok(())
    }

    async fn delete_resolved_missing_parent_records(
        &mut self,
        mutation_id: &MutationId,
    ) -> Result<(), RadrootsEventStoreError> {
        sqlx::query("DELETE FROM trade_missing_parent WHERE missing_parent_mutation_id = ?")
            .bind(mutation_id.to_hex())
            .execute(&mut **self.tx)
            .await?;
        Ok(())
    }

    async fn insert_seller_reservation(
        &mut self,
        mutation: &TradeMutationEnvelopeV1,
        claim_mutation_id: &MutationId,
        reservation: &SellerReservationAssertionV1,
        inserted_at_ms: i64,
    ) -> Result<(), RadrootsEventStoreError> {
        let reservation_json = serde_json::to_string(reservation)?;
        sqlx::query(
            "INSERT OR IGNORE INTO seller_inventory_reservation(reservation_id, trade_id, candidate_id, claim_mutation_id, inventory_authority_pubkey, inventory_epoch, assertion_commitment, reservation_expires_at_unix_s, reservation_json, inserted_at_ms) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(reservation.reservation_id.as_str())
        .bind(mutation.trade_id.to_hex())
        .bind(reservation.candidate_id.to_hex())
        .bind(claim_mutation_id.to_hex())
        .bind(reservation.inventory_authority_id.to_hex())
        .bind(i64_from_u64(
            "inventory_epoch",
            reservation.inventory_epoch,
        )?)
        .bind(reservation.assertion_commitment.as_str())
        .bind(i64_from_u64(
            "reservation_expires_at_unix_s",
            reservation.reservation_expires_at_unix_s,
        )?)
        .bind(reservation_json.as_str())
        .bind(inserted_at_ms)
        .execute(&mut **self.tx)
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
            .execute(&mut **self.tx)
            .await?;
        }
        Ok(())
    }

    #[cfg(test)]
    async fn apply_raw_authority_forge(
        &mut self,
        trigger_event_id: &str,
    ) -> Result<(), RadrootsEventStoreError> {
        let forged_ingest = PROTOCOL_POST_EXTENSION_RAW_AUTHORITY_FORGE
            .get_or_init(|| std::sync::Mutex::new(std::collections::BTreeMap::new()))
            .lock()
            .expect("protocol post-extension test mutation lock")
            .remove(trigger_event_id);
        if let Some(forged_ingest) = forged_ingest {
            let _ = ingest_event_protocol_reconciliation_v1(&mut *self.tx, &forged_ingest).await?;
        }
        Ok(())
    }

    #[cfg(test)]
    async fn apply_schema_forge(
        &mut self,
        trigger_event_id: &str,
    ) -> Result<(), RadrootsEventStoreError> {
        let enabled = PROTOCOL_POST_EXTENSION_SCHEMA_FORGE
            .get_or_init(|| std::sync::Mutex::new(std::collections::BTreeSet::new()))
            .lock()
            .expect("protocol post-extension schema test mutation lock")
            .remove(trigger_event_id);
        if enabled {
            sqlx::query(
                "CREATE TABLE radroots_event_store_post_extension_schema_forge(singleton INTEGER PRIMARY KEY)",
            )
            .execute(&mut **self.tx)
            .await?;
        }
        Ok(())
    }
}

fn trade_mutation_kind_storage_value(kind: TradeMutationKindV1) -> &'static str {
    match kind {
        TradeMutationKindV1::Proposal => "proposal",
        TradeMutationKindV1::Decision => "decision",
        TradeMutationKindV1::RevisionProposal => "revision_proposal",
        TradeMutationKindV1::RevisionDecision => "revision_decision",
        TradeMutationKindV1::Cancellation => "cancellation",
    }
}

fn i64_from_u64(field: &'static str, value: u64) -> Result<i64, RadrootsEventStoreError> {
    match i64::try_from(value) {
        Ok(value) => Ok(value),
        Err(_) => Err(RadrootsEventStoreError::UnsignedIntegerRange { field, value }),
    }
}

fn i64_from_usize(field: &'static str, value: usize) -> Result<i64, RadrootsEventStoreError> {
    match i64::try_from(value) {
        Ok(value) => Ok(value),
        Err(_) => Err(RadrootsEventStoreError::UnsignedIntegerRange {
            field,
            value: value as u64,
        }),
    }
}

#[cfg(test)]
static PROTOCOL_POST_EXTENSION_RAW_AUTHORITY_FORGE: std::sync::OnceLock<
    std::sync::Mutex<std::collections::BTreeMap<String, RadrootsEventIngest>>,
> = std::sync::OnceLock::new();

#[cfg(test)]
static PROTOCOL_POST_EXTENSION_SCHEMA_FORGE: std::sync::OnceLock<
    std::sync::Mutex<std::collections::BTreeSet<String>>,
> = std::sync::OnceLock::new();

#[cfg(test)]
pub(super) fn register_protocol_post_extension_raw_authority_forge(
    trigger_event_id: String,
    forged_ingest: RadrootsEventIngest,
) {
    let prior = PROTOCOL_POST_EXTENSION_RAW_AUTHORITY_FORGE
        .get_or_init(|| std::sync::Mutex::new(std::collections::BTreeMap::new()))
        .lock()
        .expect("protocol post-extension test mutation lock")
        .insert(trigger_event_id, forged_ingest);
    assert!(prior.is_none(), "test mutation trigger must be unique");
}

#[cfg(test)]
pub(super) fn register_protocol_post_extension_schema_forge(trigger_event_id: String) {
    let inserted = PROTOCOL_POST_EXTENSION_SCHEMA_FORGE
        .get_or_init(|| std::sync::Mutex::new(std::collections::BTreeSet::new()))
        .lock()
        .expect("protocol post-extension schema test mutation lock")
        .insert(trigger_event_id);
    assert!(inserted, "test mutation trigger must be unique");
}

#[cfg(test)]
pub(super) fn trade_mutation_kind_storage_value_for_test(
    kind: TradeMutationKindV1,
) -> &'static str {
    trade_mutation_kind_storage_value(kind)
}

#[cfg(test)]
pub(super) fn i64_from_usize_for_test(
    field: &'static str,
    value: usize,
) -> Result<i64, RadrootsEventStoreError> {
    i64_from_usize(field, value)
}
