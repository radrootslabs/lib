//! Reconcile immutable facts only under current scheduling authority.

use super::{
    AuthoredDeliveryAttempt, AuthoredDeliveryClaim, AuthoredDeliveryFact, AuthoredDeliveryPlan,
    AuthoredDeliveryState, DELIVERY_PLAN_ATTEMPTS_MAX, DeliveryAttemptOutcome, Error, FailureClass,
    NonZeroU32, RetrySchedule, Retryability, SatisfactionState, WorkFailure, WorkPhase,
};
use crate::authored_atomic::WorkFence;

impl AuthoredDeliveryPlan {
    pub fn pending_delivery_facts(&self) -> impl Iterator<Item = &AuthoredDeliveryFact> {
        self.delivery_facts.iter().filter(|fact| {
            !self
                .attempts
                .iter()
                .any(|attempt| attempt.claim_evidence() == Some(fact.claim()))
        })
    }

    pub(crate) fn reconcile_delivery_facts(
        &mut self,
        fence: Option<&WorkFence>,
        retry: Option<RetrySchedule>,
        at: u64,
        claims: &[AuthoredDeliveryClaim],
    ) -> Result<(), Error> {
        self.validate()?;
        if self.stop_requested_at_unix_ms.is_some() || self.state.is_terminal() {
            return Err(Error::InvalidAuthoredTransition);
        }
        match fence {
            Some(fence) => {
                self.require_claim(fence.token(), fence.generation(), fence.row_revision(), at)?
            }
            None if self
                .claim
                .as_ref()
                .is_some_and(|claim| at < claim.expires_at_unix_ms()) =>
            {
                return Err(Error::DeliveryPlanClaimConflict);
            }
            None => {}
        }
        let pending: Vec<_> = self.pending_delivery_facts().cloned().collect();
        if pending.is_empty()
            || pending.iter().any(|fact| at < fact.observed_at_unix_ms())
            || at < self.updated_at_unix_ms
        {
            return Err(Error::AtomicWorkflowMismatch);
        }
        let mut candidate = self.clone();
        for fact in &pending {
            // A legacy fenced application may already have persisted this exact
            // result. Both its original ordinal and valid lease interval must
            // match: reconciling another fact can clear a live lease early.
            let original = claims
                .iter()
                .find(|entry| entry.claim() == fact.claim())
                .ok_or(Error::AtomicWorkflowMismatch)?;
            if let Some(attempt) = candidate.attempts.iter_mut().find(|attempt| {
                attempt.claim_evidence().is_none()
                    && original.prior_attempt_count().checked_add(1)
                        == Some(attempt.attempt().get())
                    && attempt.recorded_at_unix_ms() >= fact.claim().acquired_at_unix_ms()
                    && attempt.recorded_at_unix_ms() < fact.claim().expires_at_unix_ms()
            }) {
                if attempt.outcome() != fact.outcome() {
                    return Err(Error::AtomicWorkflowMismatch);
                }
                attempt.claim = Some(fact.claim().clone());
                continue;
            }
            if candidate.attempts.len() >= DELIVERY_PLAN_ATTEMPTS_MAX as usize {
                return Err(Error::DeliveryAttemptOverflow);
            }
            let satisfaction = candidate.evaluate_with(fact.outcome().clone())?;
            let next = u32::try_from(candidate.attempts.len() + 1)
                .ok()
                .and_then(NonZeroU32::new)
                .ok_or(Error::DeliveryAttemptOverflow)?;
            candidate.attempts.push(AuthoredDeliveryAttempt {
                attempt: next,
                recorded_at_unix_ms: at,
                outcome: fact.outcome().clone(),
                satisfaction,
                claim: Some(fact.claim().clone()),
            });
        }
        candidate.attempt_count =
            u32::try_from(candidate.attempts.len()).map_err(|_| Error::DeliveryAttemptOverflow)?;
        let last = candidate
            .attempts
            .last()
            .ok_or(Error::AtomicWorkflowMismatch)?;
        let (state, failure) = reconciliation_state(
            last.outcome(),
            last.satisfaction(),
            candidate.attempt_count,
            retry.as_ref(),
        )?;
        candidate.state = state;
        candidate.last_failure = failure;
        candidate.retry = retry;
        candidate.claim = None;
        candidate.advance(at)?;
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }
}

fn reconciliation_state(
    outcome: &DeliveryAttemptOutcome,
    satisfaction: SatisfactionState,
    attempt_count: u32,
    retry: Option<&RetrySchedule>,
) -> Result<(AuthoredDeliveryState, Option<WorkFailure>), Error> {
    let failure = match outcome {
        DeliveryAttemptOutcome::Receipt(_) => None,
        DeliveryAttemptOutcome::SinkFailure(failure) => Some(WorkFailure::new(
            failure.code(),
            WorkPhase::Delivery,
            if failure.retryability() == Retryability::Retryable {
                FailureClass::Retryable
            } else {
                FailureClass::Terminal
            },
            failure.retry_after_unix_ms(),
            failure.message().map(str::to_owned),
        )?),
    };
    match satisfaction {
        SatisfactionState::Satisfied if retry.is_none() => {
            return Ok((AuthoredDeliveryState::Satisfied, None));
        }
        SatisfactionState::Exhausted if retry.is_none() => {
            // Exhausted acceptance is settled even when the last adapter failure
            // was retryable; retain only a compatible terminal diagnostic.
            return Ok((
                AuthoredDeliveryState::Exhausted,
                failure.filter(|failure| failure.class() == FailureClass::Terminal),
            ));
        }
        SatisfactionState::Satisfied | SatisfactionState::Exhausted => {
            return Err(Error::InvalidRetrySchedule);
        }
        SatisfactionState::Pending => {}
    }
    if attempt_count == DELIVERY_PLAN_ATTEMPTS_MAX {
        if retry.is_some() {
            return Err(Error::InvalidRetrySchedule);
        }
        return Ok((
            AuthoredDeliveryState::Exhausted,
            Some(WorkFailure::new(
                "delivery_attempt_limit",
                WorkPhase::Delivery,
                FailureClass::Terminal,
                None,
                None,
            )?),
        ));
    }
    if failure
        .as_ref()
        .is_some_and(|failure| failure.class() == FailureClass::Terminal)
    {
        if retry.is_some() {
            return Err(Error::InvalidRetrySchedule);
        }
        return Ok((AuthoredDeliveryState::FailedTerminal, failure));
    }
    let retry = retry.ok_or(Error::InvalidRetrySchedule)?;
    if retry.attempt().get() != attempt_count
        || retry.failure().phase() != WorkPhase::Delivery
        || failure.as_ref().is_some_and(|failure| {
            retry.failure().code() != failure.code()
                || retry.failure().diagnostic() != failure.diagnostic()
                || failure
                    .retry_after_unix_ms()
                    .is_some_and(|at| retry.not_before_unix_ms() < at)
        })
    {
        return Err(Error::InvalidRetrySchedule);
    }
    Ok((
        AuthoredDeliveryState::Retryable,
        Some(retry.failure().clone()),
    ))
}
