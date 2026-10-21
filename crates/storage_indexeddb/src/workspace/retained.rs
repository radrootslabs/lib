//! Authenticated bounded T0/T1/S data; mutation authority remains in the host.
use super::{State, address, issuance::Claim, journal::Records};
use crate::ReadSnapshot;
use radroots_domain::{CommandDigest, EnvelopeEventId, OperationId, PublicKey, UnixSeconds};
use radroots_storage::command::{
    ArtifactEvidence, ArtifactSnapshot, BoundSigningPlan, ClaimSnapshot, EvidenceReceipt,
    OperationError, OperationSnapshot, RetainedOperation, SigningTarget, protected,
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
        if matches!(kind, address::PREPARED | address::PLAN) {
            continue;
        }
        if matches!(kind, address::LISTING_INSTALLATION | address::PUBLIC_WORK) {
            if plan.listing().is_none() {
                return Err(OperationError::RecoveryRequired);
            }
        } else if matches!(kind, address::INSTALLATION | address::PUBLICATION) {
            if plan.chat().is_none() {
                return Err(OperationError::RecoveryRequired);
            }
            if kind == address::PUBLICATION
                && recipient as usize
                    >= plan
                        .chat()
                        .ok_or(OperationError::RecoveryRequired)?
                        .seals()
                        .len()
            {
                return Err(OperationError::RecoveryRequired);
            }
        } else {
            let valid = if plan.listing().is_some() {
                recipient == 2
            } else {
                (recipient as usize)
                    < plan
                        .chat()
                        .ok_or(OperationError::RecoveryRequired)?
                        .seals()
                        .len()
            };
            if !valid {
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
        selected: Option<SigningTarget>,
    ) -> Result<Self, OperationError> {
        let retained = header(state, view, operation, digest, t0).await?;
        let mut recipients = Vec::with_capacity(2);
        let mut output = None;
        let retained = match retained {
            Some(RetainedOperation::Planned(bound)) => {
                let mut bound = *bound;
                let keys = match bound.plan().chat() {
                    Some(chat) => chat
                        .seals()
                        .iter()
                        .map(|seal| SigningTarget::BusinessSeal(seal.recipient()))
                        .collect::<Vec<_>>(),
                    None => vec![SigningTarget::PublicEvent],
                };
                for recipient in keys {
                    let expected = EnvelopeEventId::from_bytes(
                        bound
                            .plan()
                            .artifact(recipient)
                            .map_err(|_| OperationError::RecoveryRequired)?
                            .event_id(),
                    );
                    let index = super::issuance::position(&bound, recipient)?
                        .ok_or(OperationError::RecoveryRequired)?;
                    match state.claim(view, bound, recipient, index).await? {
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
                            let obtained = state.evidence(view, &attempt, index).await?;
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
                            bound = (*attempt).into_bound_plan();
                        }
                    }
                }
                Some(RetainedOperation::Planned(Box::new(bound)))
            }
            Some(RetainedOperation::Prepared(prepared)) => {
                let mut keys = match prepared.request().command().chat() {
                    Some(command) => vec![
                        SigningTarget::BusinessSeal(command.context().actor().participant()),
                        SigningTarget::BusinessSeal(command.peer()),
                    ],
                    None => vec![SigningTarget::PublicEvent],
                };
                keys.sort_unstable_by_key(|target| target.key());
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
        self.receipt_target(SigningTarget::BusinessSeal(recipient))
    }
    pub(super) fn receipt_target(
        &self,
        target: SigningTarget,
    ) -> Result<EvidenceReceipt, OperationError> {
        self.recipients
            .iter()
            .find(|value| value.target() == target)
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
