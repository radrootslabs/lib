//! Delivery facts authorized by the backend's original immutable claim receipt.

use super::{
    AuthoredAtomicCommand, AuthoredAtomicOutcome, AuthoredAtomicReceipt, ClaimAuthoredTarget,
    ClaimAuthoredWork,
};
use crate::{
    Error,
    authored::{AuthoredArtifactId, WorkClaim},
    authored_delivery::{AuthoredDeliveryPlan, AuthoredDeliveryPlanId, DeliveryAttemptOutcome},
};

/// Retains a completed sink result without extending a lease or granting work.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordDeliveryFact {
    plan_id: AuthoredDeliveryPlanId,
    artifact_id: AuthoredArtifactId,
    claim: WorkClaim,
    outcome: DeliveryAttemptOutcome,
    observed_at_unix_ms: u64,
}

impl RecordDeliveryFact {
    pub fn new(
        plan_id: AuthoredDeliveryPlanId,
        artifact_id: AuthoredArtifactId,
        claim: WorkClaim,
        outcome: DeliveryAttemptOutcome,
        observed_at_unix_ms: u64,
    ) -> Result<Self, Error> {
        claim.validate()?;
        if observed_at_unix_ms < claim.acquired_at_unix_ms() {
            return Err(Error::AtomicWorkflowMismatch);
        }
        Ok(Self {
            plan_id,
            artifact_id,
            claim,
            outcome,
            observed_at_unix_ms,
        })
    }

    pub const fn plan_id(&self) -> AuthoredDeliveryPlanId {
        self.plan_id
    }
    pub const fn artifact_id(&self) -> AuthoredArtifactId {
        self.artifact_id
    }
    pub const fn claim(&self) -> &WorkClaim {
        &self.claim
    }
    pub const fn outcome(&self) -> &DeliveryAttemptOutcome {
        &self.outcome
    }
    pub const fn observed_at_unix_ms(&self) -> u64 {
        self.observed_at_unix_ms
    }

    pub fn claim_command(&self) -> AuthoredAtomicCommand {
        AuthoredAtomicCommand::Claim(ClaimAuthoredWork::new(
            ClaimAuthoredTarget::DeliveryPlan(self.plan_id),
            self.claim.clone(),
        ))
    }

    /// The backend must retrieve `original_claim` inside its own transaction.
    /// Caller-provided receipts must never substitute for persisted provenance.
    pub fn apply_to(
        &self,
        plan: &mut AuthoredDeliveryPlan,
        original_claim: &AuthoredAtomicReceipt,
    ) -> Result<(), Error> {
        let AuthoredAtomicOutcome::DeliveryPlan(original) = original_claim.outcome() else {
            return Err(Error::AtomicWorkflowMismatch);
        };
        original.validate()?;
        plan.validate()?;
        if !original_claim.matches_command(&self.claim_command())
            || original_claim.committed_at_unix_ms() != self.claim.acquired_at_unix_ms()
            || original.claim_evidence() != Some(&self.claim)
            || original.plan_id() != self.plan_id
            || plan.plan_id() != self.plan_id
            || original.artifact_id() != self.artifact_id
            || plan.artifact_id() != self.artifact_id
            || original.request().is_none()
            || original.request() != plan.request()
            || original.intent() != plan.intent()
            || original.created_at_unix_ms() != plan.created_at_unix_ms()
            || original.revision() > plan.revision()
            || original.updated_at_unix_ms() > plan.updated_at_unix_ms()
        {
            return Err(Error::AtomicWorkflowMismatch);
        }
        plan.record_delivery_fact(
            self.claim.clone(),
            self.outcome.clone(),
            self.observed_at_unix_ms,
        )
    }

    pub(super) fn hash_outcome(&self, hasher: &mut sha2::Sha256) {
        use sha2::Digest;
        let entries = match &self.outcome {
            DeliveryAttemptOutcome::Receipt(receipt) => {
                hasher.update([0]);
                super::hash_field(hasher, receipt.request_id().as_str().as_bytes());
                receipt.target_receipts()
            }
            DeliveryAttemptOutcome::SinkFailure(failure) => {
                hasher.update([1]);
                hasher.update([failure.retry_after_unix_ms().is_some() as u8]);
                hasher.update(
                    failure
                        .retry_after_unix_ms()
                        .unwrap_or_default()
                        .to_be_bytes(),
                );
                hash_optional(hasher, failure.message());
                failure.partial_evidence()
            }
        };
        hasher.update(
            u64::try_from(entries.len())
                .unwrap_or(u64::MAX)
                .to_be_bytes(),
        );
        super::hash_delivery(hasher, &self.outcome);
        for entry in entries {
            hash_optional(hasher, entry.target().label().map(|label| label.as_str()));
        }
    }

    pub(super) fn matches_plan(&self, plan: &AuthoredDeliveryPlan) -> bool {
        plan.plan_id() == self.plan_id
            && plan.artifact_id() == self.artifact_id
            && plan
                .delivery_facts()
                .iter()
                .any(|fact| fact.claim() == &self.claim && fact.outcome() == &self.outcome)
    }
}

fn hash_optional(hasher: &mut sha2::Sha256, value: Option<&str>) {
    use sha2::Digest;
    hasher.update([value.is_some() as u8]);
    if let Some(value) = value {
        super::hash_field(hasher, value.as_bytes());
    }
}
