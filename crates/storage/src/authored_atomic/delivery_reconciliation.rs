//! Current-authority command for reconciling already retained delivery facts.

use super::{AuthoredAtomicCommand, RecordDeliveryFact, WorkFence};
use crate::{
    Error,
    authored::RetrySchedule,
    authored_delivery::{AuthoredDeliveryHistory, AuthoredDeliveryPlan, AuthoredDeliveryPlanId},
};
use core::num::NonZeroU64;
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReconcileDeliveryFacts {
    plan_id: AuthoredDeliveryPlanId,
    revision: NonZeroU64,
    facts_digest: [u8; 32],
    fence: Option<WorkFence>,
    retry: Option<RetrySchedule>,
    reconciled_at_unix_ms: u64,
}

impl ReconcileDeliveryFacts {
    pub fn new(
        plan: &AuthoredDeliveryPlan,
        fence: Option<WorkFence>,
        retry: Option<RetrySchedule>,
        reconciled_at_unix_ms: u64,
    ) -> Result<Self, Error> {
        plan.validate()?;
        if reconciled_at_unix_ms == 0 || plan.pending_delivery_facts().next().is_none() {
            return Err(Error::AtomicWorkflowMismatch);
        }
        Ok(Self {
            plan_id: plan.plan_id(),
            revision: plan.revision(),
            facts_digest: facts_digest(plan)?,
            fence,
            retry,
            reconciled_at_unix_ms,
        })
    }
    pub const fn plan_id(&self) -> AuthoredDeliveryPlanId {
        self.plan_id
    }
    pub const fn revision(&self) -> NonZeroU64 {
        self.revision
    }
    pub const fn reconciled_at_unix_ms(&self) -> u64 {
        self.reconciled_at_unix_ms
    }
    /// Backends supply their own consistent original-claim history.
    pub fn apply_to(
        &self,
        history: &AuthoredDeliveryHistory,
    ) -> Result<AuthoredDeliveryPlan, Error> {
        history.require_pending_fact_provenance()?;
        let mut plan = history.plan().clone();
        if plan.plan_id() != self.plan_id
            || plan.revision() != self.revision
            || facts_digest(&plan)? != self.facts_digest
        {
            return Err(Error::DeliveryPlanClaimConflict);
        }
        plan.reconcile_delivery_facts(
            self.fence.as_ref(),
            self.retry.clone(),
            self.reconciled_at_unix_ms,
            history.claims(),
        )?;
        Ok(plan)
    }

    pub(super) fn matches_plan(&self, plan: &AuthoredDeliveryPlan) -> bool {
        plan.plan_id() == self.plan_id
            && self.revision.get().checked_add(1) == Some(plan.revision().get())
            && plan.updated_at_unix_ms() == self.reconciled_at_unix_ms
            && plan.claim_evidence().is_none()
            && plan.stop_requested_at_unix_ms().is_none()
            && plan.pending_delivery_facts().next().is_none()
            && facts_digest(plan).ok() == Some(self.facts_digest)
            && plan.retry() == self.retry.as_ref()
    }

    pub(super) fn hash_into(&self, hasher: &mut Sha256) {
        hasher.update(self.revision.get().to_be_bytes());
        hasher.update(self.facts_digest);
        hasher.update(self.reconciled_at_unix_ms.to_be_bytes());
        hasher.update([self.fence.is_some() as u8]);
        if let Some(fence) = &self.fence {
            hasher.update(fence.token());
            hasher.update(fence.generation().get().to_be_bytes());
            hasher.update(fence.row_revision().get().to_be_bytes());
        }
        hasher.update([self.retry.is_some() as u8]);
        if let Some(retry) = &self.retry {
            hasher.update(retry.attempt().get().to_be_bytes());
            hasher.update(retry.not_before_unix_ms().to_be_bytes());
            super::hash_failure(hasher, Some(retry.failure()));
        }
    }
}

fn facts_digest(plan: &AuthoredDeliveryPlan) -> Result<[u8; 32], Error> {
    let mut hasher = Sha256::new();
    super::hash_field(&mut hasher, b"radroots.authored.delivery-facts.v1");
    hasher.update(plan.plan_id().as_bytes());
    hasher.update(plan.artifact_id().as_bytes());
    for fact in plan.delivery_facts() {
        let command = AuthoredAtomicCommand::RecordDelivery(RecordDeliveryFact::new(
            plan.plan_id(),
            plan.artifact_id(),
            fact.claim().clone(),
            fact.outcome().clone(),
            fact.observed_at_unix_ms(),
        )?);
        hasher.update(command.digest().as_bytes());
        hasher.update(fact.observed_at_unix_ms().to_be_bytes());
    }
    Ok(hasher.finalize().into())
}
