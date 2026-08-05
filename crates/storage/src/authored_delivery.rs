//! Independent durable delivery-plan, attempt, retry, and evidence models.

use core::num::{NonZeroU32, NonZeroU64};
use radroots_transport::{
    DeliveryReceipt, DeliveryRequest, SinkFailure,
    outcome::Retryability,
    policy::{SatisfactionClass, SatisfactionPolicy, SatisfactionState, evaluate_satisfaction},
};
use sha2::{Digest, Sha256};
use std::vec::Vec;

use crate::{
    Error,
    authored::{
        AuthoredArtifactId, FailureClass, RetrySchedule, WorkClaim, WorkFailure, WorkPhase,
    },
};

pub const DELIVERY_PLAN_ATTEMPTS_MAX: u32 = 1_024;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(try_from = "[u8; 16]", into = "[u8; 16]"))]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AuthoredDeliveryPlanId([u8; 16]);

impl AuthoredDeliveryPlanId {
    pub const fn new(value: [u8; 16]) -> Result<Self, Error> {
        if all_zero(&value) {
            Err(Error::InvalidAuthoredDeliveryPlan)
        } else {
            Ok(Self(value))
        }
    }

    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

impl TryFrom<[u8; 16]> for AuthoredDeliveryPlanId {
    type Error = Error;
    fn try_from(value: [u8; 16]) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<AuthoredDeliveryPlanId> for [u8; 16] {
    fn from(value: AuthoredDeliveryPlanId) -> Self {
        value.0
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthoredDeliveryState {
    Pending,
    Retryable,
    Satisfied,
    Exhausted,
    FailedTerminal,
    Cancelled,
}

impl AuthoredDeliveryState {
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Satisfied | Self::Exhausted | Self::FailedTerminal | Self::Cancelled
        )
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeliveryAttemptOutcome {
    Receipt(DeliveryReceipt),
    SinkFailure(SinkFailure),
}

impl DeliveryAttemptOutcome {
    fn validate_for(&self, request: &DeliveryRequest) -> Result<(), Error> {
        match self {
            Self::Receipt(receipt) => receipt
                .validate_for_request(request)
                .map_err(|_| Error::InvalidAuthoredDeliveryPlan),
            Self::SinkFailure(failure) => failure
                .validate_for_request(request)
                .map_err(|_| Error::InvalidAuthoredDeliveryPlan),
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthoredDeliveryAttempt {
    attempt: NonZeroU32,
    recorded_at_unix_ms: u64,
    outcome: DeliveryAttemptOutcome,
    satisfaction: SatisfactionState,
}

impl AuthoredDeliveryAttempt {
    pub fn reconstruct(
        attempt: NonZeroU32,
        recorded_at_unix_ms: u64,
        outcome: DeliveryAttemptOutcome,
        satisfaction: SatisfactionState,
    ) -> Result<Self, Error> {
        if recorded_at_unix_ms == 0 {
            return Err(Error::InvalidAuthoredDeliveryPlan);
        }
        Ok(Self {
            attempt,
            recorded_at_unix_ms,
            outcome,
            satisfaction,
        })
    }

    pub const fn attempt(&self) -> NonZeroU32 {
        self.attempt
    }
    pub const fn recorded_at_unix_ms(&self) -> u64 {
        self.recorded_at_unix_ms
    }
    pub const fn outcome(&self) -> &DeliveryAttemptOutcome {
        &self.outcome
    }
    pub const fn satisfaction(&self) -> SatisfactionState {
        self.satisfaction
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(
        try_from = "AuthoredDeliveryPlanWire",
        into = "AuthoredDeliveryPlanWire"
    )
)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthoredDeliveryPlan {
    plan_id: AuthoredDeliveryPlanId,
    artifact_id: AuthoredArtifactId,
    request_digest: [u8; 32],
    request: DeliveryRequest,
    state: AuthoredDeliveryState,
    attempts: Vec<AuthoredDeliveryAttempt>,
    attempt_count: u32,
    retry: Option<RetrySchedule>,
    claim: Option<WorkClaim>,
    last_failure: Option<WorkFailure>,
    created_at_unix_ms: u64,
    updated_at_unix_ms: u64,
    revision: NonZeroU64,
}

impl AuthoredDeliveryPlan {
    pub fn new(
        plan_id: AuthoredDeliveryPlanId,
        artifact_id: AuthoredArtifactId,
        request: DeliveryRequest,
        created_at_unix_ms: u64,
    ) -> Result<Self, Error> {
        let request_digest = delivery_request_digest(&request);
        Self::reconstruct(Self {
            plan_id,
            artifact_id,
            request_digest,
            request,
            state: AuthoredDeliveryState::Pending,
            attempts: Vec::new(),
            attempt_count: 0,
            retry: None,
            claim: None,
            last_failure: None,
            created_at_unix_ms,
            updated_at_unix_ms: created_at_unix_ms,
            revision: NonZeroU64::MIN,
        })
    }

    pub fn reconstruct(value: Self) -> Result<Self, Error> {
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<(), Error> {
        if self.created_at_unix_ms == 0
            || self.updated_at_unix_ms < self.created_at_unix_ms
            || self.request_digest != delivery_request_digest(&self.request)
            || self.attempt_count > DELIVERY_PLAN_ATTEMPTS_MAX
            || usize::try_from(self.attempt_count).ok() != Some(self.attempts.len())
            || (matches!(self.state, AuthoredDeliveryState::Retryable) != self.retry.is_some())
            || (self.state.is_terminal() && self.claim.is_some())
        {
            return Err(Error::InvalidAuthoredDeliveryPlan);
        }
        for (index, attempt) in self.attempts.iter().enumerate() {
            if attempt.attempt.get() != u32::try_from(index + 1).unwrap_or(u32::MAX)
                || attempt.recorded_at_unix_ms < self.created_at_unix_ms
                || attempt.outcome.validate_for(&self.request).is_err()
                || self
                    .evaluate_outcomes(self.attempts[..=index].iter().map(|value| &value.outcome))
                    .ok()
                    != Some(attempt.satisfaction)
            {
                return Err(Error::InvalidAuthoredDeliveryPlan);
            }
        }
        if self
            .attempts
            .windows(2)
            .any(|pair| pair[0].recorded_at_unix_ms > pair[1].recorded_at_unix_ms)
            || self
                .claim
                .as_ref()
                .is_some_and(|claim| claim.validate().is_err())
            || self.retry.as_ref().is_some_and(|retry| {
                retry.failure().phase() != WorkPhase::Delivery
                    || retry.attempt().get() != self.attempt_count
                    || self.attempts.last().is_none_or(|attempt| {
                        retry.not_before_unix_ms() <= attempt.recorded_at_unix_ms
                    })
            })
            || self.claim.as_ref().is_some_and(|claim| {
                claim.row_revision().get().checked_add(1) != Some(self.revision.get())
                    || claim.acquired_at_unix_ms() != self.updated_at_unix_ms
            })
        {
            return Err(Error::InvalidAuthoredDeliveryPlan);
        }
        match self.state {
            AuthoredDeliveryState::Pending
            | AuthoredDeliveryState::Satisfied
            | AuthoredDeliveryState::Cancelled => {
                if self.retry.is_some() || self.last_failure.is_some() {
                    return Err(Error::InvalidAuthoredDeliveryPlan);
                }
            }
            AuthoredDeliveryState::Exhausted => {
                if self.retry.is_some()
                    || self.last_failure.as_ref().is_some_and(|failure| {
                        failure.phase() != WorkPhase::Delivery
                            || failure.class() != FailureClass::Terminal
                    })
                {
                    return Err(Error::InvalidAuthoredDeliveryPlan);
                }
            }
            AuthoredDeliveryState::Retryable => {
                if self.retry.as_ref().map(RetrySchedule::failure) != self.last_failure.as_ref() {
                    return Err(Error::InvalidAuthoredDeliveryPlan);
                }
            }
            AuthoredDeliveryState::FailedTerminal => {
                if !matches!(
                    self.last_failure.as_ref().map(WorkFailure::class),
                    Some(FailureClass::Terminal)
                ) {
                    return Err(Error::InvalidAuthoredDeliveryPlan);
                }
            }
        }
        Ok(())
    }

    pub fn claim(&mut self, claim: WorkClaim, now_unix_ms: u64) -> Result<(), Error> {
        if self.state.is_terminal()
            || self.claim.is_some()
            || claim.row_revision() != self.revision
            || claim.acquired_at_unix_ms() != now_unix_ms
            || self
                .retry
                .as_ref()
                .is_some_and(|retry| now_unix_ms < retry.not_before_unix_ms())
        {
            return Err(Error::DeliveryPlanClaimConflict);
        }
        claim.validate()?;
        let previous = self.clone();
        self.claim = Some(claim);
        if let Err(error) = self.advance(now_unix_ms) {
            *self = previous;
            return Err(error);
        }
        if let Err(error) = self.validate() {
            *self = previous;
            return Err(error);
        }
        Ok(())
    }

    pub fn apply_receipt(
        &mut self,
        token: &[u8; 16],
        generation: NonZeroU64,
        claim_revision: NonZeroU64,
        receipt: DeliveryReceipt,
        retry: Option<RetrySchedule>,
        recorded_at_unix_ms: u64,
    ) -> Result<(), Error> {
        self.require_claim(token, generation, claim_revision, recorded_at_unix_ms)?;
        receipt
            .validate_for_request(&self.request)
            .map_err(|_| Error::InvalidAuthoredDeliveryPlan)?;
        let satisfaction = self.evaluate_with(DeliveryAttemptOutcome::Receipt(receipt.clone()))?;
        let (state, last_failure) = match satisfaction {
            SatisfactionState::Satisfied if retry.is_none() => {
                (AuthoredDeliveryState::Satisfied, None)
            }
            SatisfactionState::Exhausted if retry.is_none() => {
                (AuthoredDeliveryState::Exhausted, None)
            }
            SatisfactionState::Pending => {
                let schedule = retry.as_ref().ok_or(Error::InvalidRetrySchedule)?;
                if schedule.failure().phase() != WorkPhase::Delivery {
                    return Err(Error::InvalidRetrySchedule);
                }
                (
                    AuthoredDeliveryState::Retryable,
                    Some(schedule.failure().clone()),
                )
            }
            SatisfactionState::Satisfied | SatisfactionState::Exhausted => {
                return Err(Error::InvalidRetrySchedule);
            }
        };
        self.apply_attempt(
            DeliveryAttemptOutcome::Receipt(receipt),
            satisfaction,
            state,
            retry,
            last_failure,
            recorded_at_unix_ms,
        )
    }

    pub fn apply_sink_failure(
        &mut self,
        token: &[u8; 16],
        generation: NonZeroU64,
        claim_revision: NonZeroU64,
        failure: SinkFailure,
        retry: Option<RetrySchedule>,
        recorded_at_unix_ms: u64,
    ) -> Result<(), Error> {
        self.require_claim(token, generation, claim_revision, recorded_at_unix_ms)?;
        failure
            .validate_for_request(&self.request)
            .map_err(|_| Error::InvalidAuthoredDeliveryPlan)?;
        let outcome = DeliveryAttemptOutcome::SinkFailure(failure.clone());
        let satisfaction = self.evaluate_with(outcome.clone())?;
        let typed = WorkFailure::new(
            failure.code(),
            WorkPhase::Delivery,
            match failure.retryability() {
                Retryability::Retryable => FailureClass::Retryable,
                Retryability::Terminal | Retryability::NotApplicable => FailureClass::Terminal,
            },
            failure.retry_after_unix_ms(),
            failure.message().map(str::to_owned),
        )?;
        let (state, retry, last_failure) = if satisfaction == SatisfactionState::Satisfied {
            if retry.is_some() {
                return Err(Error::InvalidRetrySchedule);
            }
            (AuthoredDeliveryState::Satisfied, None, None)
        } else if satisfaction == SatisfactionState::Exhausted {
            if retry.is_some() {
                return Err(Error::InvalidRetrySchedule);
            }
            (AuthoredDeliveryState::Exhausted, None, Some(typed))
        } else if failure.retryability() == Retryability::Retryable {
            let schedule = retry.ok_or(Error::InvalidRetrySchedule)?;
            if schedule.failure() != &typed {
                return Err(Error::InvalidRetrySchedule);
            }
            (
                AuthoredDeliveryState::Retryable,
                Some(schedule),
                Some(typed),
            )
        } else {
            if retry.is_some() {
                return Err(Error::InvalidRetrySchedule);
            }
            (AuthoredDeliveryState::FailedTerminal, None, Some(typed))
        };
        self.apply_attempt(
            outcome,
            satisfaction,
            state,
            retry,
            last_failure,
            recorded_at_unix_ms,
        )
    }

    pub fn cancel(&mut self, cancelled_at_unix_ms: u64) -> Result<(), Error> {
        if self.state.is_terminal() {
            return Err(Error::InvalidAuthoredDeliveryPlan);
        }
        let previous = self.clone();
        self.state = AuthoredDeliveryState::Cancelled;
        self.claim = None;
        self.retry = None;
        self.last_failure = None;
        if let Err(error) = self.advance(cancelled_at_unix_ms) {
            *self = previous;
            return Err(error);
        }
        self.validate()
    }

    fn apply_attempt(
        &mut self,
        outcome: DeliveryAttemptOutcome,
        satisfaction: SatisfactionState,
        state: AuthoredDeliveryState,
        retry: Option<RetrySchedule>,
        last_failure: Option<WorkFailure>,
        recorded_at_unix_ms: u64,
    ) -> Result<(), Error> {
        let next = self
            .attempt_count
            .checked_add(1)
            .filter(|attempt| *attempt <= DELIVERY_PLAN_ATTEMPTS_MAX)
            .and_then(NonZeroU32::new)
            .ok_or(Error::DeliveryAttemptOverflow)?;
        let previous = self.clone();
        self.attempt_count = next.get();
        self.attempts.push(AuthoredDeliveryAttempt::reconstruct(
            next,
            recorded_at_unix_ms,
            outcome,
            satisfaction,
        )?);
        self.state = state;
        self.retry = retry;
        self.claim = None;
        self.last_failure = last_failure;
        if let Err(error) = self.advance(recorded_at_unix_ms) {
            *self = previous;
            return Err(error);
        }
        if let Err(error) = self.validate() {
            *self = previous;
            return Err(error);
        }
        Ok(())
    }

    fn evaluate_with(&self, next: DeliveryAttemptOutcome) -> Result<SatisfactionState, Error> {
        self.evaluate_outcomes(
            self.attempts
                .iter()
                .map(|attempt| &attempt.outcome)
                .chain(core::iter::once(&next)),
        )
    }

    fn evaluate_outcomes<'a, I>(&self, outcomes: I) -> Result<SatisfactionState, Error>
    where
        I: IntoIterator<Item = &'a DeliveryAttemptOutcome>,
    {
        let mut evidence = Vec::new();
        for outcome in outcomes {
            match outcome {
                DeliveryAttemptOutcome::Receipt(receipt) => {
                    evidence.extend(
                        receipt
                            .target_receipts()
                            .iter()
                            .map(|entry| (entry.target().fingerprint(), entry.outcome())),
                    );
                }
                DeliveryAttemptOutcome::SinkFailure(failure) => {
                    evidence.extend(
                        failure
                            .partial_evidence()
                            .iter()
                            .map(|entry| (entry.target().fingerprint(), entry.outcome())),
                    );
                }
            }
        }
        evaluate_satisfaction(
            self.request.satisfaction(),
            self.request.target_set(),
            evidence,
        )
        .map_err(|_| Error::InvalidAuthoredDeliveryPlan)
    }

    fn require_claim(
        &self,
        token: &[u8; 16],
        generation: NonZeroU64,
        claim_revision: NonZeroU64,
        now_unix_ms: u64,
    ) -> Result<(), Error> {
        if !self.claim.as_ref().is_some_and(|claim| {
            claim.matches_fence(token, generation, claim_revision, now_unix_ms)
        }) {
            return Err(Error::DeliveryPlanClaimConflict);
        }
        Ok(())
    }

    fn advance(&mut self, at_unix_ms: u64) -> Result<(), Error> {
        if at_unix_ms < self.updated_at_unix_ms {
            return Err(Error::InvalidAuthoredDeliveryPlan);
        }
        self.revision = self
            .revision
            .get()
            .checked_add(1)
            .and_then(NonZeroU64::new)
            .ok_or(Error::InvalidAuthoredDeliveryPlan)?;
        self.updated_at_unix_ms = at_unix_ms;
        Ok(())
    }

    pub const fn plan_id(&self) -> AuthoredDeliveryPlanId {
        self.plan_id
    }
    pub const fn artifact_id(&self) -> AuthoredArtifactId {
        self.artifact_id
    }
    pub const fn request_digest(&self) -> &[u8; 32] {
        &self.request_digest
    }
    pub const fn request(&self) -> &DeliveryRequest {
        &self.request
    }
    pub const fn state(&self) -> AuthoredDeliveryState {
        self.state
    }
    pub fn attempts(&self) -> &[AuthoredDeliveryAttempt] {
        self.attempts.as_slice()
    }
    pub const fn attempt_count(&self) -> u32 {
        self.attempt_count
    }
    pub const fn retry(&self) -> Option<&RetrySchedule> {
        self.retry.as_ref()
    }
    pub const fn claim_evidence(&self) -> Option<&WorkClaim> {
        self.claim.as_ref()
    }
    pub const fn last_failure(&self) -> Option<&WorkFailure> {
        self.last_failure.as_ref()
    }
    pub const fn revision(&self) -> NonZeroU64 {
        self.revision
    }
}

#[cfg(feature = "serde")]
#[derive(serde::Serialize, serde::Deserialize)]
struct AuthoredDeliveryPlanWire {
    plan_id: AuthoredDeliveryPlanId,
    artifact_id: AuthoredArtifactId,
    request_digest: [u8; 32],
    request: DeliveryRequest,
    state: AuthoredDeliveryState,
    attempts: Vec<AuthoredDeliveryAttempt>,
    attempt_count: u32,
    retry: Option<RetrySchedule>,
    claim: Option<WorkClaim>,
    last_failure: Option<WorkFailure>,
    created_at_unix_ms: u64,
    updated_at_unix_ms: u64,
    revision: NonZeroU64,
}

#[cfg(feature = "serde")]
impl TryFrom<AuthoredDeliveryPlanWire> for AuthoredDeliveryPlan {
    type Error = Error;
    fn try_from(value: AuthoredDeliveryPlanWire) -> Result<Self, Self::Error> {
        Self::reconstruct(Self {
            plan_id: value.plan_id,
            artifact_id: value.artifact_id,
            request_digest: value.request_digest,
            request: value.request,
            state: value.state,
            attempts: value.attempts,
            attempt_count: value.attempt_count,
            retry: value.retry,
            claim: value.claim,
            last_failure: value.last_failure,
            created_at_unix_ms: value.created_at_unix_ms,
            updated_at_unix_ms: value.updated_at_unix_ms,
            revision: value.revision,
        })
    }
}

#[cfg(feature = "serde")]
impl From<AuthoredDeliveryPlan> for AuthoredDeliveryPlanWire {
    fn from(value: AuthoredDeliveryPlan) -> Self {
        Self {
            plan_id: value.plan_id,
            artifact_id: value.artifact_id,
            request_digest: value.request_digest,
            request: value.request,
            state: value.state,
            attempts: value.attempts,
            attempt_count: value.attempt_count,
            retry: value.retry,
            claim: value.claim,
            last_failure: value.last_failure,
            created_at_unix_ms: value.created_at_unix_ms,
            updated_at_unix_ms: value.updated_at_unix_ms,
            revision: value.revision,
        }
    }
}

fn delivery_request_digest(request: &DeliveryRequest) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hash_field(&mut hasher, b"radroots.authored.delivery.v2");
    hash_field(&mut hasher, request.request_id().as_str().as_bytes());
    hash_field(&mut hasher, request.payload().event().raw_json().as_bytes());
    for target in request.target_set().targets() {
        hash_field(&mut hasher, target.fingerprint().as_str().as_bytes());
    }
    let policy = request.satisfaction();
    hasher.update([match policy.class() {
        SatisfactionClass::Accepted => 0,
        SatisfactionClass::Delivered => 1,
    }]);
    hash_policy(&mut hasher, policy);
    hasher.update(request.deadline_unix_ms().to_be_bytes());
    hasher.finalize().into()
}

fn hash_policy(hasher: &mut Sha256, policy: &SatisfactionPolicy) {
    let targets = policy.targets();
    if targets.is_any() {
        hasher.update([0]);
    } else if targets.is_all() {
        hasher.update([1]);
    } else if let Some(threshold) = targets.quorum_threshold() {
        hasher.update([2]);
        hasher.update(threshold.to_be_bytes());
    } else if let Some(required) = targets.required_targets() {
        hasher.update([3]);
        for target in required {
            hash_field(hasher, target.as_str().as_bytes());
        }
    }
}

fn hash_field(hasher: &mut Sha256, value: &[u8]) {
    hasher.update(u64::try_from(value.len()).unwrap_or(u64::MAX).to_be_bytes());
    hasher.update(value);
}

const fn all_zero(bytes: &[u8; 16]) -> bool {
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != 0 {
            return false;
        }
        index += 1;
    }
    true
}
