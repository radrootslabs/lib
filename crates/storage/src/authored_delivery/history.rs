//! Bounded projections of backend-owned delivery claim receipts.

use super::{AuthoredDeliveryPlan, DELIVERY_PLAN_ATTEMPTS_MAX, WorkClaim};
use crate::{
    Error,
    authored_atomic::{
        AuthoredAtomicCommand, AuthoredAtomicOutcome, AuthoredAtomicReceipt, ClaimAuthoredTarget,
        ClaimAuthoredWork,
    },
};

/// One issued claim and its original scheduling-attempt boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthoredDeliveryClaim {
    claim: WorkClaim,
    prior_attempt_count: u32,
}

impl AuthoredDeliveryClaim {
    pub const fn claim(&self) -> &WorkClaim {
        &self.claim
    }
    pub const fn prior_attempt_count(&self) -> u32 {
        self.prior_attempt_count
    }
}

/// A consistent, bounded read of a plan and its immutable issued claims.
///
/// Backends must supply their own retained receipts from the same read snapshot.
/// A truncated or unproven legacy history never proves absence of an attempt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthoredDeliveryHistory {
    plan: AuthoredDeliveryPlan,
    claims: Vec<AuthoredDeliveryClaim>,
    initial_boundary_proven: bool,
    truncated: bool,
}

impl AuthoredDeliveryHistory {
    pub fn new(
        plan: AuthoredDeliveryPlan,
        preparation: Option<&AuthoredAtomicReceipt>,
    ) -> Result<Self, Error> {
        plan.validate()?;
        let initial_boundary_proven = match preparation.map(AuthoredAtomicReceipt::outcome) {
            Some(AuthoredAtomicOutcome::Prepared { delivery_plans, .. }) => {
                Self::initial_boundary(&plan, delivery_plans)?
            }
            Some(AuthoredAtomicOutcome::Submitted(value)) => {
                value.validate()?;
                Self::initial_boundary(&plan, value.preparation().delivery_plans())?
            }
            Some(_) => return Err(Error::AtomicWorkflowMismatch),
            None => false,
        };
        Ok(Self {
            plan,
            claims: Vec::new(),
            initial_boundary_proven,
            truncated: false,
        })
    }

    fn initial_boundary(
        plan: &AuthoredDeliveryPlan,
        originals: &[AuthoredDeliveryPlan],
    ) -> Result<bool, Error> {
        let original = originals
            .iter()
            .find(|original| original.plan_id() == plan.plan_id())
            .ok_or(Error::AtomicWorkflowMismatch)?;
        original.validate()?;
        if original.artifact_id() != plan.artifact_id()
            || original.intent() != plan.intent()
            || original.created_at_unix_ms() != plan.created_at_unix_ms()
            || original.revision() > plan.revision()
            || original.updated_at_unix_ms() > plan.updated_at_unix_ms()
        {
            return Err(Error::AtomicWorkflowMismatch);
        }
        Ok(original.claim_evidence().is_none() && original.attempts().is_empty())
    }

    pub fn push_claim(&mut self, receipt: &AuthoredAtomicReceipt) -> Result<(), Error> {
        if self.claims.len() >= DELIVERY_PLAN_ATTEMPTS_MAX as usize {
            return Err(Error::DeliveryAttemptOverflow);
        }
        let AuthoredAtomicOutcome::DeliveryPlan(original) = receipt.outcome() else {
            return Err(Error::AtomicWorkflowMismatch);
        };
        original.validate()?;
        let claim = original
            .claim_evidence()
            .ok_or(Error::AtomicWorkflowMismatch)?;
        let command = AuthoredAtomicCommand::Claim(ClaimAuthoredWork::new(
            ClaimAuthoredTarget::DeliveryPlan(self.plan.plan_id()),
            claim.clone(),
        ));
        if !receipt.matches_command(&command)
            || receipt.committed_at_unix_ms() != claim.acquired_at_unix_ms()
            || original.plan_id() != self.plan.plan_id()
            || original.artifact_id() != self.plan.artifact_id()
            || original.request().is_none()
            || original.request() != self.plan.request()
            || original.intent() != self.plan.intent()
            || original.created_at_unix_ms() != self.plan.created_at_unix_ms()
            || original.revision() > self.plan.revision()
            || original.updated_at_unix_ms() > self.plan.updated_at_unix_ms()
            || self.claims.iter().any(|entry| entry.claim == *claim)
        {
            return Err(Error::AtomicWorkflowMismatch);
        }
        self.claims.push(AuthoredDeliveryClaim {
            claim: claim.clone(),
            prior_attempt_count: original.attempt_count(),
        });
        Ok(())
    }

    /// Records that additional retained claims exceeded this bounded read.
    pub fn mark_truncated(&mut self) {
        self.truncated = true;
    }
    pub const fn plan(&self) -> &AuthoredDeliveryPlan {
        &self.plan
    }
    pub fn claims(&self) -> &[AuthoredDeliveryClaim] {
        &self.claims
    }
    pub const fn is_complete(&self) -> bool {
        self.initial_boundary_proven && !self.truncated
    }
    pub const fn is_truncated(&self) -> bool {
        self.truncated
    }
    pub fn validate(&self) -> Result<(), Error> {
        self.plan.validate()?;
        if self.is_complete()
            && (self
                .plan
                .claim_evidence()
                .is_some_and(|claim| !self.claims.iter().any(|entry| entry.claim() == claim))
                || self.plan.delivery_facts().iter().any(|fact| {
                    !self
                        .claims
                        .iter()
                        .any(|entry| entry.claim() == fact.claim())
                }))
        {
            return Err(Error::AtomicWorkflowMismatch);
        }
        Ok(())
    }
    /// Require original backend claim provenance before scheduling reconciliation.
    pub fn require_pending_fact_provenance(&self) -> Result<(), Error> {
        self.validate()?;
        if self.is_truncated() {
            return Err(Error::DeliveryAttemptOverflow);
        }
        if self.plan.pending_delivery_facts().any(|fact| {
            !self
                .claims
                .iter()
                .any(|entry| entry.claim() == fact.claim())
        }) {
            return Err(Error::AtomicWorkflowMismatch);
        }
        Ok(())
    }
    pub fn proves_no_issued_attempt(&self) -> bool {
        self.is_complete()
            && self.claims.is_empty()
            && self.plan.attempts().is_empty()
            && self.plan.delivery_facts().is_empty()
            && self.plan.claim_evidence().is_none()
    }

    pub fn has_unresolved_claims(&self) -> bool {
        !self.is_complete()
            || self.claims.iter().any(|entry| {
                !self
                    .plan
                    .delivery_facts()
                    .iter()
                    .any(|fact| fact.claim() == &entry.claim)
                    && !self.plan.attempts().iter().any(|attempt| {
                        // Historical fenced applications have no explicit claim
                        // marker. Their original attempt boundary and valid lease
                        // interval together identify the retained application.
                        attempt.claim_evidence().is_none()
                            && entry.prior_attempt_count.checked_add(1)
                                == Some(attempt.attempt().get())
                            && attempt.recorded_at_unix_ms() >= entry.claim.acquired_at_unix_ms()
                            && attempt.recorded_at_unix_ms() < entry.claim.expires_at_unix_ms()
                    })
            })
    }
}
