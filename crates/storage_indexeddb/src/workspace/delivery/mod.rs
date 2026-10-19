//! Protected exact transport intent; persistence never performs delivery.
mod codec;
mod graph;
mod json;
mod operations;

use super::State;
use radroots_domain::PublicKey;
use radroots_domain::{CommandDigest, EnvelopeEventId, OperationId, UnixSeconds};
use radroots_secrets::context::{
    EnvelopeContext, EnvelopePurpose, EnvelopeSubject, PayloadSchemaId,
};
use radroots_storage::command::{
    BoundSigningPlan, DeliveryAttempt, DeliveryIntent, OperationError, OperationSnapshot,
    SealEvidence, SignedDmRelaySource,
};
use sha2::{Digest, Sha256};

#[derive(Clone, Copy)]
struct Slot {
    operation: OperationId,
    recipient: u8,
    ordinal: u16,
    commitment: [u8; 32],
}
fn context(
    state: &State,
    slot: Slot,
    part: u8,
    index: bool,
) -> Result<EnvelopeContext, OperationError> {
    let subject = Sha256::new()
        .chain_update(b"radroots.browser.local_outbox.delivery.context.v2\0")
        .chain_update(state.selected.workspace_id().as_bytes())
        .chain_update(slot.operation.as_bytes())
        .chain_update([slot.recipient])
        .chain_update(slot.ordinal.to_be_bytes())
        .chain_update([part, u8::from(index)])
        .chain_update(slot.commitment)
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    Ok(EnvelopeContext::new(
        EnvelopePurpose::parse("radroots.browser.workspace.delivery")
            .map_err(|_| OperationError::InvalidCommand)?,
        EnvelopeSubject::parse("workspace_delivery", subject)
            .map_err(|_| OperationError::InvalidCommand)?,
        PayloadSchemaId::parse(if index {
            "radroots.browser.workspace.delivery_index.v2"
        } else {
            "radroots.browser.workspace.delivery.v2"
        })
        .map_err(|_| OperationError::InvalidCommand)?,
    ))
}
fn routes(
    bound: &BoundSigningPlan,
    recipient: PublicKey,
    intent: &DeliveryIntent,
) -> Result<(), OperationError> {
    let mut selected = None;
    for route in bound
        .plan()
        .routes()
        .iter()
        .filter(|route| route.participant() == recipient)
    {
        let source = SignedDmRelaySource::verify(route.raw_json())?;
        source.validate_capture(route)?;
        if selected.replace(route).is_some() || route.selected_relays().is_empty() {
            return Err(OperationError::UnverifiedApproval);
        }
    }
    let route = selected.ok_or(OperationError::UnverifiedApproval)?;
    if intent.targets().targets().iter().any(|target| {
        !route
            .selected_relays()
            .iter()
            .any(|raw| matches!(target.matches_nostr_relay(raw.as_ref()), Ok(true)))
    }) {
        return Err(OperationError::UnverifiedApproval);
    }
    Ok(())
}
pub(super) use graph::Graph;

#[derive(Clone, Copy)]
pub(super) struct Selection<'a> {
    pub recipient: PublicKey,
    pub request: &'a radroots_transport::sink::DeliveryRequestId,
    pub event: Option<EnvelopeEventId>,
    pub retain_seal: bool,
}
pub(super) struct Facts {
    pub snapshot: Option<OperationSnapshot>,
    pub selected: Option<DeliveryAttempt>,
    pub original: Option<SealEvidence>,
    pub slots: [u16; 2],
}
impl State {
    pub(super) async fn delivery_facts(
        &self,
        view: &crate::ReadSnapshot,
        operation: OperationId,
        digest: Option<CommandDigest>,
        t0: &super::journal::Records,
        observed: UnixSeconds,
        selected: Option<Selection<'_>>,
    ) -> Result<Facts, OperationError> {
        let mut original = super::retained::Graph::read_for_output(
            self,
            view,
            operation,
            digest,
            t0,
            selected.filter(|s| s.retain_seal).map(|s| s.recipient),
        )
        .await?;
        let installed =
            super::installation::Records::read(self, view, &original, operation).await?;
        let deliveries =
            Graph::read(self, view, &original, &installed, operation, selected).await?;
        let output = original.take_output();
        let snapshot = original
            .into_snapshot(observed)?
            .map(|snapshot| {
                snapshot
                    .with_installation(installed.message, installed.work)?
                    .with_delivery(deliveries.summaries, Vec::new())
            })
            .transpose()?;
        Ok(Facts {
            snapshot,
            selected: deliveries.selected,
            original: output,
            slots: deliveries.slots,
        })
    }
}
