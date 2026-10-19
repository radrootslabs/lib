//! First normalized target facts under independent expected v2 contexts.
use super::{Slot, context, json};
use crate::workspace::{State, address, crypto, journal::storage_error};
use crate::{Mutation, ReadSnapshot, RecordKey};
use core::num::NonZeroU64;
use radroots_domain::{EnvelopeEventId, TransactionId};
use radroots_secrets::context::{
    EnvelopeContext, EnvelopePurpose, EnvelopeSubject, PayloadSchemaId,
};
use radroots_storage::command::{DeliveryAttempt, DeliveryObservation, OperationError};
use radroots_transport::sink::DeliveryTargetReceipt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use zeroize::Zeroizing;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire<'a> {
    version: u8,
    work: [u8; 32],
    #[serde(borrow)]
    request: &'a str,
    event: EnvelopeEventId,
    envelope_digest: [u8; 32],
    transaction: TransactionId,
    target: DeliveryTargetReceipt,
    kind: u8,
    ordinal: u8,
    observed_at: Option<NonZeroU64>,
}
fn scoped(
    state: &State,
    slot: Slot,
    attempt: &DeliveryAttempt,
    target: u8,
    kind: u8,
) -> Result<EnvelopeContext, OperationError> {
    let base = context(state, slot, 0, false)?;
    let target = attempt
        .intent()
        .targets()
        .targets()
        .get(target as usize)
        .ok_or(OperationError::RecoveryRequired)?;
    let subject = Sha256::new()
        .chain_update(b"radroots.browser.local_outbox.delivery_observation.context.v2\0")
        .chain_update(base.authentication_digest())
        .chain_update(attempt.intent().request_id().as_str().as_bytes())
        .chain_update(attempt.transaction_id().as_bytes())
        .chain_update(attempt.envelope_digest())
        .chain_update(target.fingerprint().as_str().as_bytes())
        .chain_update([kind])
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    Ok(EnvelopeContext::new(
        EnvelopePurpose::parse("radroots.browser.workspace.delivery_observation")
            .map_err(|_| OperationError::InvalidCommand)?,
        EnvelopeSubject::parse("delivery_observation", subject)
            .map_err(|_| OperationError::InvalidCommand)?,
        PayloadSchemaId::parse("radroots.browser.workspace.delivery_observation.v2")
            .map_err(|_| OperationError::InvalidCommand)?,
    ))
}
fn checked(
    attempt: &DeliveryAttempt,
    wire: Wire<'_>,
    slot: Slot,
    target: u8,
    kind: u8,
) -> Result<DeliveryObservation, OperationError> {
    if wire.version != 2
        || wire.work != slot.commitment
        || wire.request != attempt.intent().request_id().as_str()
        || wire.event != attempt.intent().event_id()
        || &wire.envelope_digest != attempt.envelope_digest()
        || wire.transaction != attempt.transaction_id()
        || wire.kind != kind
        || attempt.intent().targets().targets().get(target as usize) != Some(wire.target.target())
    {
        return Err(OperationError::RecoveryRequired);
    }
    let value = DeliveryObservation::from_store_report(attempt, &wire.target, wire.observed_at)
        .map_err(|_| OperationError::RecoveryRequired)?;
    if value.kind().code() != kind || value.target_ordinal() != wire.ordinal {
        return Err(OperationError::RecoveryRequired);
    }
    Ok(value)
}
pub(super) async fn read(
    state: &State,
    view: &ReadSnapshot,
    slot: Slot,
    attempt: &DeliveryAttempt,
    positions: BTreeMap<(u8, u8), RecordKey>,
) -> Result<Vec<DeliveryObservation>, OperationError> {
    let mut reports = Vec::with_capacity(positions.len());
    for ((target, kind), key) in positions {
        let raw = state
            .read_view_key(view, key)
            .await?
            .ok_or(OperationError::RecoveryRequired)?;
        let clear = crypto::open_bound(
            state.provider.as_ref(),
            &state.reference,
            scoped(state, slot, attempt, target, kind)?,
            &raw,
        )
        .await?;
        let report = clear.expose_secret(|raw| {
            if raw.len() > 16384 || !json::bounded(raw, 32) {
                return Err(OperationError::RecoveryRequired);
            }
            let wire: Wire<'_> =
                serde_json::from_slice(raw).map_err(|_| OperationError::RecoveryRequired)?;
            checked(attempt, wire, slot, target, kind)
        })?;
        reports.push(report);
    }
    Ok(reports)
}
pub(super) async fn protect(
    state: &State,
    slot: Slot,
    attempt: &DeliveryAttempt,
    value: &DeliveryObservation,
) -> Result<Mutation, OperationError> {
    let target = attempt
        .intent()
        .targets()
        .targets()
        .iter()
        .position(|target| target == value.target_receipt().target())
        .ok_or(OperationError::InvalidEvidence)? as u8;
    let wire = Wire {
        version: 2,
        work: slot.commitment,
        request: value.request_id().as_str(),
        event: value.event_id(),
        envelope_digest: *value.envelope_digest(),
        transaction: attempt.transaction_id(),
        target: value.target_receipt().clone(),
        kind: value.kind().code(),
        ordinal: value.target_ordinal(),
        observed_at: value.observed_at_unix_ms(),
    };
    let clear =
        Zeroizing::new(serde_json::to_vec(&wire).map_err(|_| OperationError::InvalidEvidence)?);
    if clear.len() > 16384 {
        return Err(OperationError::InvalidEvidence);
    }
    let protected = crypto::seal_bound(
        state.provider.as_ref(),
        &state.reference,
        scoped(state, slot, attempt, target, value.kind().code())?,
        &clear,
    )
    .await?;
    Mutation::new(
        address::observation_key(
            slot.operation,
            slot.recipient,
            slot.ordinal,
            target,
            value.kind().code(),
        ),
        None,
        Some(protected),
    )
    .map_err(storage_error)
}
