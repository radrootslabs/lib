//! Authenticated bounded T0/T1/S data; mutation authority remains in the host.
use super::{State, address, issuance::Claim, journal::Records};
use crate::ReadSnapshot;
use radroots_domain::{CommandDigest, EnvelopeEventId, OperationId, PublicKey, UnixSeconds};
use radroots_storage::command::{
    ArtifactEvidence, ArtifactSnapshot, BoundSigningPlan, ClaimSnapshot, EvidenceReceipt,
    OperationError, OperationSnapshot, RetainedOperation, protected,
};

pub(super) async fn header(
    state: &State,
    view: &ReadSnapshot,
    operation: OperationId,
    digest: Option<CommandDigest>,
    t0: &Records,
) -> Result<Option<RetainedOperation>, OperationError> {
    let Some(prepared) = state.preparation(operation, t0).await? else {
        if !view.is_empty() {
            return Err(OperationError::RecoveryRequired);
        }
        return Ok(None);
    };
    if prepared.request().binding().input() != &state.selected
        || prepared.request().command().context().actor().participant()
            != state.selected.participant()
    {
        return Err(OperationError::Unauthorized);
    }
    protected::check_digest(
        &prepared,
        prepared.request().command().context().actor(),
        digest,
    )?;
    let records = state.plan_records(view, operation).await?;
    let Some((manifest, plan)) = state.retained_plan(&prepared, &records).await? else {
        state.absent_plans(view, operation).await?;
        return Ok(Some(RetainedOperation::Prepared(prepared)));
    };
    for key in view.keys() {
        let (kind, recipient, _) = address::fields(key)?;
        if kind >= address::CLAIM {
            let chat = plan.chat().ok_or(OperationError::UnsupportedOperation)?;
            if recipient as usize >= chat.seals().len() {
                return Err(OperationError::RecoveryRequired);
            }
        }
    }
    Ok(Some(RetainedOperation::Planned(Box::new(
        manifest.bind(prepared, plan)?,
    ))))
}

pub(super) struct Graph {
    retained: Option<RetainedOperation>,
    recipients: Vec<ArtifactSnapshot>,
    output: Option<ArtifactEvidence>,
}
impl Graph {
    pub(super) async fn read_for_output(
        state: &State,
        view: &ReadSnapshot,
        operation: OperationId,
        digest: Option<CommandDigest>,
        t0: &Records,
        selected: Option<PublicKey>,
    ) -> Result<Self, OperationError> {
        let retained = header(state, view, operation, digest, t0).await?;
        let mut recipients = Vec::with_capacity(2);
        let mut output = None;
        let retained = match retained {
            Some(RetainedOperation::Planned(bound)) => {
                let mut bound = *bound;
                let keys = bound
                    .chat_plan()?
                    .seals()
                    .iter()
                    .map(|seal| seal.recipient())
                    .collect::<Vec<_>>();
                for (index, recipient) in keys.into_iter().enumerate() {
                    let expected =
                        EnvelopeEventId::from_bytes(bound.chat_plan()?.seals()[index].event_id());
                    match state.claim(view, bound, recipient, index as u8).await? {
                        Claim::Absent(returned, _) => {
                            recipients.push(ArtifactSnapshot::from_store_report(
                                recipient,
                                Some(expected),
                                None,
                                None,
                            ));
                            bound = returned;
                        }
                        Claim::Present(attempt) => {
                            let claim = ClaimSnapshot::from_store_report(&attempt)?;
                            // Verify one bounded exact seal at a time; retaining a
                            // receipt does not duplicate private plan/raw bodies.
                            let obtained = state.evidence(view, &attempt, index as u8).await?;
                            let receipt = obtained.as_ref().map(|(_, receipt)| *receipt);
                            if Some(recipient) == selected {
                                output = obtained.map(|(value, _)| value);
                            }
                            recipients.push(ArtifactSnapshot::from_store_report(
                                recipient,
                                Some(expected),
                                Some(claim),
                                receipt,
                            ));
                            bound = attempt.into_bound_plan();
                        }
                    }
                }
                Some(RetainedOperation::Planned(Box::new(bound)))
            }
            Some(RetainedOperation::Prepared(prepared)) => {
                let command = prepared.request().chat_command()?;
                let mut keys = vec![command.context().actor().participant(), command.peer()];
                keys.sort_unstable();
                keys.dedup();
                for recipient in keys {
                    recipients.push(ArtifactSnapshot::from_store_report(
                        recipient, None, None, None,
                    ));
                }
                Some(RetainedOperation::Prepared(prepared))
            }
            None => None,
        };
        Ok(Self {
            retained,
            recipients,
            output,
        })
    }
    pub(super) fn take_output(&mut self) -> Option<ArtifactEvidence> {
        self.output.take()
    }
    pub(super) fn bound(&self) -> Option<&BoundSigningPlan> {
        self.retained
            .as_ref()
            .and_then(RetainedOperation::bound_plan)
    }
    pub(super) fn receipt(&self, recipient: PublicKey) -> Result<EvidenceReceipt, OperationError> {
        self.recipients
            .iter()
            .find(|value| value.target().recipient() == Some(recipient))
            .and_then(|value| value.obtained())
            .ok_or(OperationError::RecoveryRequired)
    }
    pub(super) fn into_snapshot(
        self,
        observed: UnixSeconds,
    ) -> Result<Option<OperationSnapshot>, OperationError> {
        self.retained
            .map(|source| OperationSnapshot::from_store_report(source, self.recipients, observed))
            .transpose()
    }
}
