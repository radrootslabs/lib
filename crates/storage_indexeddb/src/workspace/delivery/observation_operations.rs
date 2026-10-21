use super::{Selection, codec, observation};
use crate::workspace::{
    BrowserWorkspace, WorkspaceError,
    journal::{self, OperationChange},
    owned,
};
use radroots_domain::{CommandDigest, OperationId};
use radroots_storage::{
    command::{DeliveryObservation, DeliveryObservationOutcome, OperationError},
    workspace::{AttachmentProof, WorkspaceBinding},
};
use radroots_transport::sink::DeliveryRequestId;

impl BrowserWorkspace {
    /// Retains an already obtained target fact under the original proof and
    /// actual attempt. This grants no permission to dispatch or retry.
    pub async fn record_delivery_observation(
        &self,
        binding: WorkspaceBinding,
        proof: AttachmentProof,
        supplied: DeliveryObservation,
    ) -> Result<DeliveryObservationOutcome, OperationError> {
        let capture_identity = crate::workspace::recovery::CaptureIdentity::observation(&supplied);
        let owned_identity = capture_identity.clone();
        let fallback = supplied.clone();
        let work = self.state.admit_capture()?;
        let state = self.state.clone();
        let result = owned(work, async move {
            let input = binding.input().clone();
            let (grant, protected) = state.capture_retained(&input, &proof).await?;
            let actual_work = supplied.work();
            let evidence = actual_work.evidence();
            let operation = evidence.operation_id();
            let view = state.read_view(operation).await?;
            let t0 = state.operation_records(&view, operation).await?;
            let facts = state
                .delivery_facts(
                    &view,
                    operation,
                    Some(evidence.command_digest()),
                    &t0,
                    binding.observed_at(),
                    Some(Selection {
                        recipient: evidence.target(),
                        request: supplied.request_id(),
                        event: None,
                        retain_seal: false,
                    }),
                )
                .await?;
            let snapshot = facts.snapshot.ok_or(OperationError::InvalidEvidence)?;
            radroots_storage::command::protected::same_binding(
                snapshot.prepared().request().binding(),
                &binding,
            )?;
            let attempt = facts.selected.ok_or(OperationError::InvalidEvidence)?;
            if attempt.work() != actual_work {
                return Err(OperationError::InvalidEvidence);
            }
            let checked = DeliveryObservation::from_store_report(
                &attempt,
                supplied.target_receipt(),
                supplied.observed_at_unix_ms(),
            )?;
            if checked != supplied {
                return Err(OperationError::InvalidEvidence);
            }
            let existing = snapshot.delivery_observations().iter().find(|value| {
                value.work() == actual_work
                    && value.request_id() == supplied.request_id()
                    && value.target_receipt().target().fingerprint()
                        == supplied.target_receipt().target().fingerprint()
                    && value.kind() == supplied.kind()
            });
            let retained = existing.cloned().unwrap_or(supplied);
            let mut extra = if existing.is_some() {
                Vec::new()
            } else {
                vec![
                    observation::protect(
                        &state,
                        facts
                            .selected_slot
                            .ok_or(OperationError::RecoveryRequired)?,
                        &attempt,
                        &retained,
                    )
                    .await?,
                ]
            };
            extra.extend(state.coordinate_guard(snapshot.prepared()).await?);
            let batch = journal::mutations(protected, operation, t0, None, extra)?;
            let active = state.active()?;
            match state
                .checked_batch(batch, view, move || {
                    if !active.get() {
                        return Err(WorkspaceError::Closed.into());
                    }
                    grant.authenticate_retained_capture(&input, &proof)?;
                    Ok(())
                })
                .await
            {
                Ok(()) => {
                    state.capture_confirmed(&owned_identity);
                    Ok(DeliveryObservationOutcome::Committed(retained))
                }
                Err(OperationError::Workspace(WorkspaceError::CommitUnknown)) => {
                    state.mark_capture_unknown(owned_identity);
                    Ok(DeliveryObservationOutcome::Unknown(retained))
                }
                Err(error) => Err(error),
            }
        })
        .await;
        match result {
            Err(OperationError::Workspace(WorkspaceError::CommitUnknown)) => {
                self.state.mark_capture_unknown(capture_identity);
                Ok(DeliveryObservationOutcome::Unknown(fallback))
            }
            other => other,
        }
    }

    /// Queries original target facts under current access without dispatching.
    pub async fn delivery_observations(
        &self,
        binding: WorkspaceBinding,
        operation: OperationId,
        digest: CommandDigest,
        target: radroots_storage::command::SigningTarget,
        request: DeliveryRequestId,
        proof: AttachmentProof,
    ) -> Result<Vec<DeliveryObservation>, OperationError> {
        if !codec::valid_request(request.as_str()) {
            return Err(OperationError::InvalidCommand);
        }
        let work = self.state.admit()?;
        let state = self.state.clone();
        owned(work, async move {
            let access = state.operation_access(&binding, proof).await?;
            let recipient = target;
            let view = state.read_view(operation).await?;
            let t0 = state.operation_records(&view, operation).await?;
            let facts = state
                .delivery_facts(
                    &view,
                    operation,
                    Some(digest),
                    &t0,
                    access.binding.observed_at(),
                    Some(Selection {
                        recipient,
                        request: &request,
                        event: None,
                        retain_seal: false,
                    }),
                )
                .await?;
            let result = match facts.snapshot {
                Some(snapshot) => {
                    radroots_storage::command::protected::same_binding(
                        snapshot.prepared().request().binding(),
                        &binding,
                    )?;
                    snapshot
                        .delivery_observations()
                        .iter()
                        .filter(|value| {
                            value.work().target() == recipient && value.request_id() == &request
                        })
                        .cloned()
                        .collect()
                }
                None => Vec::new(),
            };
            state
                .check_operation(access, operation, OperationChange::observe(t0), view)
                .await?;
            Ok(result)
        })
        .await
    }
}
