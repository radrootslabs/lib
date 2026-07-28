use super::post_core_storage_v1::{PostCoreStorageV1, TradeProjectionWrite};
use super::protocol_reconciliation_v1::ProtocolReconciliationV1IngestResult;
use crate::error::RadrootsEventStoreError;
use crate::model::RadrootsEventIngest;
use radroots_event::ids::{RadrootsTradeCandidateId, RadrootsTradeMutationId};
use radroots_event::trade::{
    RADROOTS_TRADE_MUTATION_CONTRACT_IDS, RadrootsSellerReservationAssertionV1,
    RadrootsTradeDecisionV1, RadrootsTradeMutationBodyV1, RadrootsTradeMutationEnvelopeV1,
    trade_mutation_from_canonical_content,
};
use sha2::{Digest, Sha256};

pub(super) async fn apply_post_core_extensions_v1(
    storage: &mut PostCoreStorageV1<'_, '_>,
    ingest: &RadrootsEventIngest,
    result: &ProtocolReconciliationV1IngestResult,
) -> Result<(), RadrootsEventStoreError> {
    if let Some(stored_event_seq) = result.inserted_seq
        && result.receipt.valid_stream_eligible
    {
        let is_trade_mutation = match &result.receipt.contract_id {
            Some(contract_id) => is_trade_mutation_contract_id(contract_id.as_str()),
            None => false,
        };
        if is_trade_mutation {
            store_trade_mutation_event(storage, ingest, stored_event_seq).await?;
        }
    }
    if result.record_observation
        && let Some(observation) = ingest.transport_observation()
    {
        storage
            .upsert_transport_observation(result.receipt.event_id.as_str(), observation)
            .await?;
    }
    Ok(())
}

fn is_trade_mutation_contract_id(contract_id: &str) -> bool {
    RADROOTS_TRADE_MUTATION_CONTRACT_IDS.contains(&contract_id)
}

async fn store_trade_mutation_event(
    storage: &mut PostCoreStorageV1<'_, '_>,
    ingest: &RadrootsEventIngest,
    stored_event_seq: i64,
) -> Result<(), RadrootsEventStoreError> {
    let event = ingest.event();
    let payload_sha256 = sha256_hex(event.content().as_bytes());
    let parsed = match trade_mutation_from_canonical_content(event.content()) {
        Ok(envelope) => envelope,
        Err(error) => {
            let reason = error.to_string();
            storage
                .quarantine_trade(
                    None,
                    None,
                    Some(event.id_hex()),
                    reason.as_str(),
                    ingest.observed_at_ms(),
                )
                .await?;
            return Ok(());
        }
    };
    let mutation_id = match &parsed.mutation_id {
        Some(mutation_id) => mutation_id,
        None => {
            storage
                .quarantine_trade(
                    Some(parsed.trade_id.to_hex()),
                    None,
                    Some(event.id_hex()),
                    "canonical trade mutation content is missing mutation_id",
                    ingest.observed_at_ms(),
                )
                .await?;
            return Ok(());
        }
    };
    if &parsed.author_pubkey != event.author() {
        storage
            .quarantine_trade(
                Some(parsed.trade_id.to_hex()),
                Some(mutation_id.to_hex()),
                Some(event.id_hex()),
                "trade mutation author_pubkey does not match transport event pubkey",
                ingest.observed_at_ms(),
            )
            .await?;
        return Ok(());
    }

    let write = TradeProjectionWrite::new(
        event,
        stored_event_seq,
        &parsed,
        mutation_id,
        candidate_id_for_mutation(&parsed),
        proposal_mutation_id_for_mutation(&parsed),
        target_claim_mutation_id_for_mutation(&parsed),
        payload_sha256.as_str(),
        ingest.observed_at_ms(),
        seller_reservation_for_mutation(&parsed),
    );
    storage.persist_trade_projection(write).await
}

fn candidate_id_for_mutation(
    mutation: &RadrootsTradeMutationEnvelopeV1,
) -> Option<&RadrootsTradeCandidateId> {
    match &mutation.body {
        RadrootsTradeMutationBodyV1::Proposal { candidate }
        | RadrootsTradeMutationBodyV1::RevisionProposal { candidate } => {
            candidate.candidate_id.as_ref()
        }
        RadrootsTradeMutationBodyV1::Decision { candidate_id, .. }
        | RadrootsTradeMutationBodyV1::RevisionDecision { candidate_id, .. } => Some(candidate_id),
        RadrootsTradeMutationBodyV1::Cancellation {
            target_candidate_id,
            ..
        } => target_candidate_id.as_ref(),
    }
}

fn proposal_mutation_id_for_mutation(
    mutation: &RadrootsTradeMutationEnvelopeV1,
) -> Option<&RadrootsTradeMutationId> {
    match &mutation.body {
        RadrootsTradeMutationBodyV1::Decision {
            proposal_mutation_id,
            ..
        }
        | RadrootsTradeMutationBodyV1::RevisionDecision {
            proposal_mutation_id,
            ..
        } => Some(proposal_mutation_id),
        _ => None,
    }
}

fn target_claim_mutation_id_for_mutation(
    mutation: &RadrootsTradeMutationEnvelopeV1,
) -> Option<&RadrootsTradeMutationId> {
    match &mutation.body {
        RadrootsTradeMutationBodyV1::Cancellation {
            target_claim_mutation_id,
            ..
        } => target_claim_mutation_id.as_ref(),
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

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[cfg(test)]
pub(super) fn candidate_id_for_mutation_for_test(
    mutation: &RadrootsTradeMutationEnvelopeV1,
) -> Option<RadrootsTradeCandidateId> {
    candidate_id_for_mutation(mutation).cloned()
}

#[cfg(test)]
pub(super) fn proposal_mutation_id_for_mutation_for_test(
    mutation: &RadrootsTradeMutationEnvelopeV1,
) -> Option<RadrootsTradeMutationId> {
    proposal_mutation_id_for_mutation(mutation).cloned()
}

#[cfg(test)]
pub(super) fn target_claim_mutation_id_for_mutation_for_test(
    mutation: &RadrootsTradeMutationEnvelopeV1,
) -> Option<RadrootsTradeMutationId> {
    target_claim_mutation_id_for_mutation(mutation).cloned()
}

#[cfg(test)]
pub(super) fn seller_reservation_for_mutation_for_test(
    mutation: &RadrootsTradeMutationEnvelopeV1,
) -> Option<&RadrootsSellerReservationAssertionV1> {
    seller_reservation_for_mutation(mutation)
}

#[cfg(test)]
pub(super) fn sha256_hex_for_test(bytes: &[u8]) -> String {
    sha256_hex(bytes)
}
