//! Immutable delivery observations, independent of the scheduling revision.

use super::{
    AuthoredDeliveryPlan, DELIVERY_PLAN_ATTEMPTS_MAX, DeliveryAttemptOutcome, Error,
    SatisfactionState, WorkClaim,
};

/// One final sink result from an exactly identified durable claim.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthoredDeliveryFact {
    claim: WorkClaim,
    outcome: DeliveryAttemptOutcome,
    observed_at_unix_ms: u64,
}

impl AuthoredDeliveryFact {
    pub const fn claim(&self) -> &WorkClaim {
        &self.claim
    }
    pub const fn outcome(&self) -> &DeliveryAttemptOutcome {
        &self.outcome
    }
    pub const fn observed_at_unix_ms(&self) -> u64 {
        self.observed_at_unix_ms
    }
}

impl AuthoredDeliveryPlan {
    pub fn delivery_facts(&self) -> &[AuthoredDeliveryFact] {
        &self.delivery_facts
    }

    pub const fn stop_requested_at_unix_ms(&self) -> Option<u64> {
        self.stop_requested_at_unix_ms
    }

    /// Cumulative transport evidence, independent of cancellation or retry state.
    pub fn delivery_satisfaction(&self) -> Result<SatisfactionState, Error> {
        self.evaluate_outcomes(
            self.attempts
                .iter()
                .map(|attempt| &attempt.outcome)
                .chain(self.delivery_facts.iter().map(|fact| &fact.outcome)),
        )
    }

    pub(super) fn validate_facts(&self) -> Result<(), Error> {
        if self.delivery_facts.len() > DELIVERY_PLAN_ATTEMPTS_MAX as usize
            || self.stop_requested_at_unix_ms.is_some_and(|at| {
                at < self.created_at_unix_ms
                    || at > self.updated_at_unix_ms
                    || !self.state.is_terminal()
            })
        {
            return Err(Error::InvalidAuthoredDeliveryPlan);
        }
        let mut claims = std::collections::BTreeSet::new();
        for fact in &self.delivery_facts {
            if fact.claim.validate().is_err()
                || fact.claim.acquired_at_unix_ms() < self.created_at_unix_ms
                || fact.claim.acquired_at_unix_ms() > self.updated_at_unix_ms
                || fact.claim.row_revision() >= self.revision
                || fact.observed_at_unix_ms < fact.claim.acquired_at_unix_ms()
                || self
                    .request
                    .as_ref()
                    .is_none_or(|request| fact.outcome.validate_for(request).is_err())
                || !claims.insert((
                    fact.claim.token(),
                    fact.claim.owner(),
                    fact.claim.generation(),
                    fact.claim.row_revision(),
                    fact.claim.acquired_at_unix_ms(),
                    fact.claim.expires_at_unix_ms(),
                ))
            {
                return Err(Error::InvalidAuthoredDeliveryPlan);
            }
        }
        Ok(())
    }

    /// Called only after the atomic owner has checked its original claim receipt.
    pub(crate) fn record_delivery_fact(
        &mut self,
        claim: WorkClaim,
        outcome: DeliveryAttemptOutcome,
        observed_at_unix_ms: u64,
    ) -> Result<(), Error> {
        if let Some(prior) = self.delivery_facts.iter().find(|fact| fact.claim == claim) {
            return if prior.outcome == outcome {
                Ok(())
            } else {
                Err(Error::AtomicWorkflowMismatch)
            };
        }
        if self.delivery_facts.len() >= DELIVERY_PLAN_ATTEMPTS_MAX as usize {
            return Err(Error::DeliveryAttemptOverflow);
        }
        self.delivery_facts.push(AuthoredDeliveryFact {
            claim,
            outcome,
            observed_at_unix_ms,
        });
        // Facts have their own observation time and immutable atomic receipt. They
        // do not mutate scheduling revision/time, fences, attempts or backoff.
        if let Err(error) = self.validate() {
            self.delivery_facts.pop();
            return Err(error);
        }
        Ok(())
    }
}
