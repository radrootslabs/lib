//! Ordinary delivery uses the same protected original and fresh stock claims.
use super::*;
use crate::session::stock_evidence::DeliveryContext;
use radroots_storage::{
    command::DeliveryIntent,
    inventory::{StockDeliveryAttemptOutcome, StockDeliveryRequest},
};
use radroots_transport::TargetSet;

impl CommandRuntime {
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn deliver_acceptance(
        &self,
        store: &Shared<dyn AcceptanceStore>,
        inventory: &Shared<dyn InventoryStore>,
        selection: &AcceptanceSelection,
        proof: &AttachmentProof,
        resources: &SigningResources,
        recipient: radroots_domain::PublicKey,
        mode: &RunMode,
    ) -> crate::ExecutionResult<()> {
        let fail = |e| {
            crate::ExecutionFailure::from(Error::operation(
                e,
                OperationContext::retained(selection.operation),
            ))
        };
        // Shared by every acceptance task, including repeats and explicit resume.
        // The store still owns cross-process writer exclusion and final witnesses.
        let _creation = resources.stock_captures.delivery_serial.lock().await;
        resources.stock_captures.ensure_ready().map_err(fail)?;
        let current = store
            .snapshot(selection.clone(), proof)
            .await
            .map_err(fail)?;
        observe(selection, current.as_ref()).map_err(fail)?;
        if current.aborted() || current.installation().is_none() {
            return Ok(());
        }
        let Some(evidence) = current
            .recipients()
            .iter()
            .find(|v| v.recipient == recipient)
            .and_then(|v| v.evidence)
        else {
            return Ok(());
        };
        let mut selection = selection.clone();
        selection.minimum = current.checkpoint();
        let index = current
            .recipients()
            .iter()
            .position(|v| v.recipient == recipient)
            .ok_or_else(|| fail(OperationError::RecoveryRequired))?;
        // This fresh coherent snapshot already authenticates the delivery
        // projection; do not repeat the entire protected-history read.
        let state = current
            .delivery()
            .and_then(|values| values.get(index))
            .ok_or_else(|| fail(OperationError::UnsupportedOperation))?;
        if state.acceptance_satisfied().map_err(fail)?
            || (state.attempt_count() != 0 && matches!(mode, RunMode::Command))
        {
            return Ok(());
        }
        let route = current
            .plan()
            .and_then(|plan| plan.routes().iter().find(|v| v.participant() == recipient))
            .ok_or_else(|| fail(OperationError::RecoveryRequired))?;
        let allowed = self.io.targets(route).map_err(fail)?;
        let (targets, policy) = match state.original_intent() {
            Some(original) => {
                if original.targets().targets().iter().any(|target| {
                    !allowed
                        .targets()
                        .iter()
                        .any(|v| v.fingerprint() == target.fingerprint())
                }) {
                    return Err(fail(OperationError::UnverifiedApproval));
                }
                (original.targets().clone(), original.policy().clone())
            }
            None => (allowed, self.io.publication_policy()),
        };
        let selected = targets
            .targets()
            .iter()
            .filter(|target| {
                !state.targets().iter().any(|v| {
                    v.target() == target.fingerprint()
                        && (v.accepted() || v.potential_attempts() >= 12)
                })
            })
            .cloned()
            .collect::<Vec<_>>();
        if selected.is_empty() {
            return Ok(());
        }
        let selected =
            TargetSet::new(selected).map_err(|_| fail(OperationError::RecoveryRequired))?;
        let context = || -> Result<DeliveryContext, OperationError> {
            Ok(DeliveryContext {
                selection: selection.clone(),
                recipient,
                store: inventory.clone(),
                proof: copy(proof)?,
                _lease: resources.lease.clone(),
            })
        };
        let outer = match state.source() {
            Some(source) => source,
            None => {
                let source = store
                    .signing_evidence(selection.clone(), evidence, proof)
                    .await
                    .map_err(fail)?
                    .ok_or_else(|| fail(OperationError::RecoveryRequired))?;
                if source.receipt() != evidence {
                    return Err(fail(OperationError::RecoveryRequired));
                }
                // Prepare credentials before wrapping, then register the obtained
                // outer before another conversion or persistent write can fail.
                let context = context().map_err(fail)?;
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
                resources.stock_captures.capture_envelope(&pending).await?
            }
        };
        let deadline = self
            .io
            .now()
            .map_err(fail)?
            .get()
            .checked_add(30)
            .and_then(|v| v.checked_mul(1000))
            .and_then(NonZeroU64::new)
            .ok_or_else(|| fail(OperationError::RecoveryRequired))?;
        let intent = DeliveryIntent::new(
            &self.io.delivery_id().map_err(fail)?,
            outer.event_id(),
            &targets,
            &policy,
            deadline,
        )
        .and_then(|v| v.with_selection(&selected))
        .map_err(fail)?;
        let context = context().map_err(fail)?;
        // The original outer's capture checkpoint is historical metadata. Use
        // the authenticated current selection, including authorization state.
        let request = StockDeliveryRequest::new(
            selection.binding.clone(),
            selection.minimum,
            selection.operation,
            selection.digest,
            recipient,
            intent.clone(),
        )
        .and_then(|request| request.with_expected_attempt_count(state.attempt_count()))
        .map_err(fail)?;
        let attempt = match inventory
            .prepare_stock_delivery_attempt(request, copy(proof).map_err(fail)?)
            .await
            .map_err(fail)?
        {
            StockDeliveryAttemptOutcome::Claimed(attempt) => attempt,
            StockDeliveryAttemptOutcome::Retained(_) => return Ok(()),
            StockDeliveryAttemptOutcome::Unknown { .. } => {
                return Err(fail(OperationError::RecoveryRequired));
            }
        };
        if attempt.intent() != &intent || attempt.source().receipt() != outer {
            return Err(fail(OperationError::RecoveryRequired));
        }
        let result = self
            .io
            .publish(
                attempt.transport_request().map_err(fail)?,
                attempt.intent().selected_target_set().map_err(fail)?,
            )
            .await;
        let observed = self
            .io
            .now()
            .ok()
            .and_then(|v| v.get().checked_mul(1000))
            .and_then(NonZeroU64::new);
        let pending = resources
            .stock_captures
            .retain_delivery(context, attempt, result, observed);
        resources.stock_captures.capture_delivery(&pending).await?;
        Ok(())
    }
}
