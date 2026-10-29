//! Explicit wrapping never renews original business signing or dispatch authority.
use super::*;
use crate::session::stock_evidence::DeliveryContext;
use radroots_storage::inventory::{StockEnvelopeRequest, StockRewrapOutcome, StockRewrapRequest};

fn request(
    selection: &AcceptanceSelection,
    recipient: PublicKey,
) -> Result<StockEnvelopeRequest, OperationError> {
    StockEnvelopeRequest::new(
        selection.binding.clone(),
        selection.minimum,
        selection.operation,
        selection.digest,
        recipient,
        None,
    )
}
/// Separate authenticated observations, never a new wrapping or send permit.
pub(in crate::session) async fn status(
    inventory: &Shared<dyn InventoryStore>,
    selection: &AcceptanceSelection,
    recipient: PublicKey,
    id: OperationId,
    proof: &AttachmentProof,
) -> Result<AcceptanceRewrapStatus, OperationError> {
    let state = inventory
        .stock_rewrap_attempt(
            StockRewrapRequest::new(request(selection, recipient)?, id),
            copy(proof)?,
        )
        .await?;
    if let Some(state) = &state {
        let admission = state.admission();
        let original = admission.original();
        if admission.rewrap_id() != id
            || !same(original.attempt(), selection)
            || original.recipient() != recipient
            || admission.checkpoint().authority() != selection.minimum.authority()
        {
            return Err(OperationError::RecoveryRequired);
        }
    }
    let delivery = if let Some(outer) = state.as_ref().and_then(|state| state.envelope()) {
        let summary = inventory
            .stock_delivery_summary(
                request(selection, recipient)?.with_envelope(outer.event_id()),
                copy(proof)?,
            )
            .await?;
        if summary.source() != Some(outer) {
            return Err(OperationError::RecoveryRequired);
        }
        Some(summary)
    } else {
        None
    };
    Ok(AcceptanceRewrapStatus {
        id,
        recipient,
        state,
        delivery,
    })
}
impl CommandRuntime {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::session) async fn run_stock_rewrap(
        &self,
        store: Shared<dyn AcceptanceStore>,
        inventory: Shared<dyn InventoryStore>,
        selection: AcceptanceSelection,
        proof: &AttachmentProof,
        resources: SigningResources,
        recipient: PublicKey,
        id: OperationId,
        mode: RunMode,
    ) -> crate::ExecutionResult<AcceptanceRewrapStatus> {
        let fail = |e| {
            crate::ExecutionFailure::from(Error::operation(
                e,
                OperationContext::retained(selection.operation),
            ))
        };
        if !self.io.enabled() {
            return Err(fail(OperationError::UnsupportedOperation));
        }
        let _creation = resources.stock_captures.delivery_serial.lock().await;
        resources.stock_captures.ensure_ready().map_err(fail)?;
        let snapshot = store
            .snapshot(selection.clone(), proof)
            .await
            .map_err(fail)?;
        observe(&selection, snapshot.as_ref()).map_err(fail)?;
        let installed = snapshot
            .installation()
            .ok_or_else(|| fail(OperationError::MissingHistory))?;
        if snapshot.aborted() {
            return Err(fail(OperationError::InvalidCommand));
        }
        let evidence = snapshot
            .recipients()
            .iter()
            .find(|entry| entry.recipient == recipient)
            .and_then(|entry| entry.evidence)
            .ok_or_else(|| fail(OperationError::MissingHistory))?;
        let mut current = selection.clone();
        current.minimum = snapshot.checkpoint();
        let original = inventory
            .stock_envelope(
                request(&current, recipient).map_err(fail)?,
                copy(proof).map_err(fail)?,
            )
            .await
            .map_err(fail)?
            .ok_or_else(|| fail(OperationError::MissingHistory))?;
        let original = original.receipt();
        if original.rewrap_admission_transaction_id().is_some()
            || original.evidence() != evidence
            || original.installation_transaction_id()
                != installed
                    .attempt
                    .transaction_id()
                    .ok_or_else(|| fail(OperationError::RecoveryRequired))?
        {
            return Err(fail(OperationError::RecoveryRequired));
        }
        let route = snapshot
            .plan()
            .and_then(|plan| {
                plan.routes()
                    .iter()
                    .find(|route| route.participant() == recipient)
            })
            .ok_or_else(|| fail(OperationError::RecoveryRequired))?;
        let mut view = status(&inventory, &current, recipient, id, proof)
            .await
            .map_err(fail)?;
        if matches!(mode, RunMode::Command) && view.state.is_none() {
            let index = snapshot
                .recipients()
                .iter()
                .position(|entry| entry.recipient == recipient)
                .ok_or_else(|| fail(OperationError::RecoveryRequired))?;
            let prior = snapshot
                .delivery()
                .and_then(|values| values.get(index))
                .ok_or_else(|| fail(OperationError::UnsupportedOperation))?;
            let (targets, _) = self
                .stock_publication_policy(route, prior.original_intent())
                .map_err(fail)?;
            if targets.targets().iter().all(|target| {
                prior.targets().iter().any(|entry| {
                    entry.target() == target.fingerprint() && entry.potential_attempts() >= 12
                })
            }) {
                return Err(fail(OperationError::DeliveryAttemptLimit));
            }
            let source = store
                .signing_evidence(current.clone(), evidence, proof)
                .await
                .map_err(fail)?
                .ok_or_else(|| fail(OperationError::MissingHistory))?;
            if source.receipt() != evidence {
                return Err(fail(OperationError::RecoveryRequired));
            }
            let context = DeliveryContext {
                selection: current.clone(),
                recipient,
                store: inventory.clone(),
                proof: copy(proof).map_err(fail)?,
                _lease: resources.lease.clone(),
                rewrap: None,
            };
            let pending = resources
                .stock_captures
                .retain_rewrap_admission(context, original, id);
            match resources.stock_captures.prepare_rewrap(&pending).await? {
                StockRewrapOutcome::Claimed(admitted) => {
                    // Prepare original credentials before local key generation.
                    let context = DeliveryContext {
                        selection: current.clone(),
                        recipient,
                        store: inventory.clone(),
                        proof: copy(proof).map_err(fail)?,
                        _lease: resources.lease.clone(),
                        rewrap: Some(*admitted),
                    };
                    let now = self.io.now().map_err(fail)?;
                    if now < admitted.prepared_at() || now >= admitted.expires_at() {
                        return Err(fail(OperationError::DeadlineExceeded));
                    }
                    let candidate = source
                        .with_json(|raw| self.io.private_envelope(recipient, raw))
                        .map_err(fail)?;
                    let observed = self
                        .io
                        .now()
                        .ok()
                        .and_then(|v| v.get().checked_mul(1000))
                        .and_then(NonZeroU64::new);
                    let pending = resources
                        .stock_captures
                        .retain_envelope(context, candidate, observed);
                    resources.stock_captures.capture_envelope(&pending).await?;
                }
                StockRewrapOutcome::Retained(_) => {}
                StockRewrapOutcome::Unknown(_) => {
                    return Err(fail(OperationError::RecoveryRequired));
                }
            }
            view = status(&inventory, &current, recipient, id, proof)
                .await
                .map_err(fail)?;
        }
        // Existing/uncaptured admissions remain unknown. Resume never generates
        // keys or another inner signature, even after restart or a lost waiter.
        if let Some(outer) = view.state.as_ref().and_then(|state| state.envelope()) {
            let summary = view
                .delivery
                .as_ref()
                .ok_or_else(|| fail(OperationError::RecoveryRequired))?;
            if let Some(plan) = self
                .stock_delivery_plan(summary, route, &mode)
                .map_err(fail)?
            {
                let context = DeliveryContext {
                    selection: current.clone(),
                    recipient,
                    store: inventory.clone(),
                    proof: copy(proof).map_err(fail)?,
                    _lease: resources.lease.clone(),
                    rewrap: None,
                };
                self.send_stock_delivery(
                    &resources.stock_captures,
                    context,
                    outer,
                    summary,
                    plan,
                    Some(outer.event_id()),
                )
                .await?;
                view = status(&inventory, &current, recipient, id, proof)
                    .await
                    .map_err(fail)?;
            }
        }
        Ok(view)
    }
}
