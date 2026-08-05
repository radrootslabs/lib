//! Durable authored-operation, artifact, claim, failure, and settlement models.

use core::num::{NonZeroU32, NonZeroU64};
use radroots_event::SignedEvent;
use radroots_event_codec::authoring::{AuthoredEventPlan, HistoricalPlanIntegrity, PlanWireV1};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, string::String, vec::Vec};

use crate::{Error, journal::OperationInstanceId};

pub const AUTHORED_OPERATION_ARTIFACTS_MAX: usize = 256;
pub const WORK_CLAIM_OWNER_MAX_BYTES: usize = 128;
pub const WORK_FAILURE_CODE_MAX_BYTES: usize = 64;
pub const WORK_FAILURE_DIAGNOSTIC_MAX_BYTES: usize = 1_024;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(try_from = "[u8; 16]", into = "[u8; 16]"))]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AuthoredArtifactId([u8; 16]);

impl AuthoredArtifactId {
    pub const fn new(value: [u8; 16]) -> Result<Self, Error> {
        if all_zero(&value) {
            Err(Error::InvalidAuthoredArtifact)
        } else {
            Ok(Self(value))
        }
    }

    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

impl TryFrom<[u8; 16]> for AuthoredArtifactId {
    type Error = Error;

    fn try_from(value: [u8; 16]) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<AuthoredArtifactId> for [u8; 16] {
    fn from(value: AuthoredArtifactId) -> Self {
        value.0
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum ArtifactOrigin {
    Planned,
    ImportedSigned,
}

impl ArtifactOrigin {
    pub const fn is_resignable(self) -> bool {
        matches!(self, Self::Planned)
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum SigningState {
    Planned,
    Signed,
    Retryable,
    Indeterminate,
    FailedTerminal,
    Cancelled,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum AdmissionState {
    Pending,
    Inserted,
    Duplicate,
    Retryable,
    Rejected,
    Cancelled,
}

impl AdmissionState {
    pub const fn is_admitted(self) -> bool {
        matches!(self, Self::Inserted | Self::Duplicate)
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum WorkPhase {
    Signing,
    Admission,
    Delivery,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum FailureClass {
    Retryable,
    Terminal,
    Indeterminate,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(try_from = "WorkClaimWire", into = "WorkClaimWire")
)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkClaim {
    token: [u8; 16],
    owner: String,
    generation: NonZeroU64,
    acquired_at_unix_ms: u64,
    expires_at_unix_ms: u64,
    row_revision: NonZeroU64,
}

impl WorkClaim {
    pub fn new(
        token: [u8; 16],
        owner: impl Into<String>,
        generation: NonZeroU64,
        acquired_at_unix_ms: u64,
        expires_at_unix_ms: u64,
        row_revision: NonZeroU64,
    ) -> Result<Self, Error> {
        let claim = Self {
            token,
            owner: owner.into(),
            generation,
            acquired_at_unix_ms,
            expires_at_unix_ms,
            row_revision,
        };
        claim.validate()?;
        Ok(claim)
    }

    pub fn validate(&self) -> Result<(), Error> {
        if all_zero(&self.token)
            || !valid_text(self.owner.as_str(), WORK_CLAIM_OWNER_MAX_BYTES)
            || self.acquired_at_unix_ms == 0
            || self.expires_at_unix_ms <= self.acquired_at_unix_ms
        {
            return Err(Error::InvalidWorkClaim);
        }
        Ok(())
    }

    pub const fn token(&self) -> &[u8; 16] {
        &self.token
    }
    pub fn owner(&self) -> &str {
        self.owner.as_str()
    }
    pub const fn generation(&self) -> NonZeroU64 {
        self.generation
    }
    pub const fn acquired_at_unix_ms(&self) -> u64 {
        self.acquired_at_unix_ms
    }
    pub const fn expires_at_unix_ms(&self) -> u64 {
        self.expires_at_unix_ms
    }
    pub const fn row_revision(&self) -> NonZeroU64 {
        self.row_revision
    }

    pub fn matches_fence(
        &self,
        token: &[u8; 16],
        generation: NonZeroU64,
        row_revision: NonZeroU64,
        now_unix_ms: u64,
    ) -> bool {
        &self.token == token
            && self.generation == generation
            && self.row_revision == row_revision
            && now_unix_ms >= self.acquired_at_unix_ms
            && now_unix_ms < self.expires_at_unix_ms
    }
}

#[cfg(feature = "serde")]
#[derive(serde::Serialize, serde::Deserialize)]
struct WorkClaimWire {
    token: [u8; 16],
    owner: String,
    generation: NonZeroU64,
    acquired_at_unix_ms: u64,
    expires_at_unix_ms: u64,
    row_revision: NonZeroU64,
}

#[cfg(feature = "serde")]
impl TryFrom<WorkClaimWire> for WorkClaim {
    type Error = Error;
    fn try_from(value: WorkClaimWire) -> Result<Self, Self::Error> {
        Self::new(
            value.token,
            value.owner,
            value.generation,
            value.acquired_at_unix_ms,
            value.expires_at_unix_ms,
            value.row_revision,
        )
    }
}

#[cfg(feature = "serde")]
impl From<WorkClaim> for WorkClaimWire {
    fn from(value: WorkClaim) -> Self {
        Self {
            token: value.token,
            owner: value.owner,
            generation: value.generation,
            acquired_at_unix_ms: value.acquired_at_unix_ms,
            expires_at_unix_ms: value.expires_at_unix_ms,
            row_revision: value.row_revision,
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(try_from = "WorkFailureWire", into = "WorkFailureWire")
)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkFailure {
    code: String,
    phase: WorkPhase,
    class: FailureClass,
    retry_after_unix_ms: Option<u64>,
    diagnostic: Option<String>,
}

impl WorkFailure {
    pub fn new(
        code: impl Into<String>,
        phase: WorkPhase,
        class: FailureClass,
        retry_after_unix_ms: Option<u64>,
        diagnostic: Option<String>,
    ) -> Result<Self, Error> {
        let failure = Self {
            code: code.into(),
            phase,
            class,
            retry_after_unix_ms,
            diagnostic,
        };
        failure.validate()?;
        Ok(failure)
    }

    pub fn validate(&self) -> Result<(), Error> {
        if !valid_code(self.code.as_str())
            || matches!(self.retry_after_unix_ms, Some(0))
            || (self.retry_after_unix_ms.is_some()
                && !matches!(self.class, FailureClass::Retryable))
            || self
                .diagnostic
                .as_deref()
                .is_some_and(|value| !valid_text(value, WORK_FAILURE_DIAGNOSTIC_MAX_BYTES))
        {
            return Err(Error::InvalidWorkFailure);
        }
        Ok(())
    }

    pub fn code(&self) -> &str {
        self.code.as_str()
    }
    pub const fn phase(&self) -> WorkPhase {
        self.phase
    }
    pub const fn class(&self) -> FailureClass {
        self.class
    }
    pub const fn retry_after_unix_ms(&self) -> Option<u64> {
        self.retry_after_unix_ms
    }
    pub fn diagnostic(&self) -> Option<&str> {
        self.diagnostic.as_deref()
    }
}

#[cfg(feature = "serde")]
#[derive(serde::Serialize, serde::Deserialize)]
struct WorkFailureWire {
    code: String,
    phase: WorkPhase,
    class: FailureClass,
    retry_after_unix_ms: Option<u64>,
    diagnostic: Option<String>,
}

#[cfg(feature = "serde")]
impl TryFrom<WorkFailureWire> for WorkFailure {
    type Error = Error;
    fn try_from(value: WorkFailureWire) -> Result<Self, Self::Error> {
        Self::new(
            value.code,
            value.phase,
            value.class,
            value.retry_after_unix_ms,
            value.diagnostic,
        )
    }
}

#[cfg(feature = "serde")]
impl From<WorkFailure> for WorkFailureWire {
    fn from(value: WorkFailure) -> Self {
        Self {
            code: value.code,
            phase: value.phase,
            class: value.class,
            retry_after_unix_ms: value.retry_after_unix_ms,
            diagnostic: value.diagnostic,
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(try_from = "RetryScheduleWire", into = "RetryScheduleWire")
)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetrySchedule {
    attempt: NonZeroU32,
    not_before_unix_ms: u64,
    failure: WorkFailure,
}

impl RetrySchedule {
    pub fn new(
        attempt: NonZeroU32,
        not_before_unix_ms: u64,
        failure: WorkFailure,
    ) -> Result<Self, Error> {
        if not_before_unix_ms == 0
            || !matches!(failure.class(), FailureClass::Retryable)
            || failure
                .retry_after_unix_ms()
                .is_some_and(|retry_after| retry_after != not_before_unix_ms)
        {
            return Err(Error::InvalidRetrySchedule);
        }
        Ok(Self {
            attempt,
            not_before_unix_ms,
            failure,
        })
    }

    pub fn next_attempt(
        &self,
        not_before_unix_ms: u64,
        failure: WorkFailure,
    ) -> Result<Self, Error> {
        let attempt = self
            .attempt
            .get()
            .checked_add(1)
            .and_then(NonZeroU32::new)
            .ok_or(Error::InvalidRetrySchedule)?;
        Self::new(attempt, not_before_unix_ms, failure)
    }

    pub const fn attempt(&self) -> NonZeroU32 {
        self.attempt
    }
    pub const fn not_before_unix_ms(&self) -> u64 {
        self.not_before_unix_ms
    }
    pub const fn failure(&self) -> &WorkFailure {
        &self.failure
    }
}

#[cfg(feature = "serde")]
#[derive(serde::Serialize, serde::Deserialize)]
struct RetryScheduleWire {
    attempt: NonZeroU32,
    not_before_unix_ms: u64,
    failure: WorkFailure,
}

#[cfg(feature = "serde")]
impl TryFrom<RetryScheduleWire> for RetrySchedule {
    type Error = Error;
    fn try_from(value: RetryScheduleWire) -> Result<Self, Self::Error> {
        Self::new(value.attempt, value.not_before_unix_ms, value.failure)
    }
}

#[cfg(feature = "serde")]
impl From<RetrySchedule> for RetryScheduleWire {
    fn from(value: RetrySchedule) -> Self {
        Self {
            attempt: value.attempt,
            not_before_unix_ms: value.not_before_unix_ms,
            failure: value.failure,
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(try_from = "ExactSignedArtifactWire", into = "ExactSignedArtifactWire")
)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactSignedArtifact {
    event: SignedEvent,
    raw_json_sha256: [u8; 32],
}

#[cfg(feature = "serde")]
#[derive(serde::Serialize, serde::Deserialize)]
struct ExactSignedArtifactWire {
    event: SignedEvent,
    raw_json_sha256: [u8; 32],
}

#[cfg(feature = "serde")]
impl TryFrom<ExactSignedArtifactWire> for ExactSignedArtifact {
    type Error = Error;
    fn try_from(value: ExactSignedArtifactWire) -> Result<Self, Self::Error> {
        Self::reconstruct(value.event, value.raw_json_sha256)
    }
}

#[cfg(feature = "serde")]
impl From<ExactSignedArtifact> for ExactSignedArtifactWire {
    fn from(value: ExactSignedArtifact) -> Self {
        Self {
            event: value.event,
            raw_json_sha256: value.raw_json_sha256,
        }
    }
}

/// Exact versioned authored-plan wire retained for durable reconstruction.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(try_from = "DurableAuthoredPlanWire", into = "DurableAuthoredPlanWire")
)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurableAuthoredPlan {
    wire_json: Vec<u8>,
}

#[cfg(feature = "serde")]
#[derive(serde::Serialize, serde::Deserialize)]
struct DurableAuthoredPlanWire {
    wire_json: Vec<u8>,
}

#[cfg(feature = "serde")]
impl TryFrom<DurableAuthoredPlanWire> for DurableAuthoredPlan {
    type Error = Error;
    fn try_from(value: DurableAuthoredPlanWire) -> Result<Self, Self::Error> {
        Self::reconstruct(value.wire_json)
    }
}

#[cfg(feature = "serde")]
impl From<DurableAuthoredPlan> for DurableAuthoredPlanWire {
    fn from(value: DurableAuthoredPlan) -> Self {
        Self {
            wire_json: value.wire_json,
        }
    }
}

impl DurableAuthoredPlan {
    pub fn from_plan(plan: &AuthoredEventPlan) -> Result<Self, Error> {
        let wire_json = PlanWireV1::from_plan(plan)
            .to_json()
            .map_err(|_| Error::InvalidAuthoredArtifact)?;
        Ok(Self { wire_json })
    }

    pub fn reconstruct(wire_json: Vec<u8>) -> Result<Self, Error> {
        PlanWireV1::from_json(wire_json.as_slice()).map_err(|_| Error::InvalidAuthoredArtifact)?;
        Ok(Self { wire_json })
    }

    pub fn decode(&self) -> Result<HistoricalPlanIntegrity, Error> {
        PlanWireV1::from_json(self.wire_json.as_slice()).map_err(|_| Error::InvalidAuthoredArtifact)
    }

    pub fn wire_json(&self) -> &[u8] {
        self.wire_json.as_slice()
    }
}

impl ExactSignedArtifact {
    pub fn new(event: SignedEvent) -> Self {
        let raw_json_sha256 = Sha256::digest(event.raw_json().as_bytes()).into();
        Self {
            event,
            raw_json_sha256,
        }
    }

    pub fn reconstruct(event: SignedEvent, raw_json_sha256: [u8; 32]) -> Result<Self, Error> {
        let artifact = Self {
            event,
            raw_json_sha256,
        };
        if Sha256::digest(artifact.event.raw_json().as_bytes()).as_slice()
            != artifact.raw_json_sha256
        {
            return Err(Error::InvalidAuthoredArtifact);
        }
        Ok(artifact)
    }

    pub const fn event(&self) -> &SignedEvent {
        &self.event
    }
    pub const fn raw_json_sha256(&self) -> &[u8; 32] {
        &self.raw_json_sha256
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(try_from = "AuthoredArtifactWire", into = "AuthoredArtifactWire")
)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthoredArtifact {
    artifact_id: AuthoredArtifactId,
    operation_id: OperationInstanceId,
    ordinal: u16,
    origin: ArtifactOrigin,
    plan: Option<DurableAuthoredPlan>,
    signing_state: SigningState,
    admission_state: AdmissionState,
    signed: Option<ExactSignedArtifact>,
    signing_claim: Option<WorkClaim>,
    admission_claim: Option<WorkClaim>,
    signing_retry: Option<RetrySchedule>,
    admission_retry: Option<RetrySchedule>,
    last_failure: Option<WorkFailure>,
    created_at_unix_ms: u64,
    updated_at_unix_ms: u64,
    revision: NonZeroU64,
}

#[cfg(feature = "serde")]
#[derive(serde::Serialize, serde::Deserialize)]
struct AuthoredArtifactWire {
    artifact_id: AuthoredArtifactId,
    operation_id: OperationInstanceId,
    ordinal: u16,
    origin: ArtifactOrigin,
    plan: Option<DurableAuthoredPlan>,
    signing_state: SigningState,
    admission_state: AdmissionState,
    signed: Option<ExactSignedArtifact>,
    signing_claim: Option<WorkClaim>,
    admission_claim: Option<WorkClaim>,
    signing_retry: Option<RetrySchedule>,
    admission_retry: Option<RetrySchedule>,
    last_failure: Option<WorkFailure>,
    created_at_unix_ms: u64,
    updated_at_unix_ms: u64,
    revision: NonZeroU64,
}

#[cfg(feature = "serde")]
impl TryFrom<AuthoredArtifactWire> for AuthoredArtifact {
    type Error = Error;
    fn try_from(value: AuthoredArtifactWire) -> Result<Self, Self::Error> {
        Self::reconstruct(Self {
            artifact_id: value.artifact_id,
            operation_id: value.operation_id,
            ordinal: value.ordinal,
            origin: value.origin,
            plan: value.plan,
            signing_state: value.signing_state,
            admission_state: value.admission_state,
            signed: value.signed,
            signing_claim: value.signing_claim,
            admission_claim: value.admission_claim,
            signing_retry: value.signing_retry,
            admission_retry: value.admission_retry,
            last_failure: value.last_failure,
            created_at_unix_ms: value.created_at_unix_ms,
            updated_at_unix_ms: value.updated_at_unix_ms,
            revision: value.revision,
        })
    }
}

#[cfg(feature = "serde")]
impl From<AuthoredArtifact> for AuthoredArtifactWire {
    fn from(value: AuthoredArtifact) -> Self {
        Self {
            artifact_id: value.artifact_id,
            operation_id: value.operation_id,
            ordinal: value.ordinal,
            origin: value.origin,
            plan: value.plan,
            signing_state: value.signing_state,
            admission_state: value.admission_state,
            signed: value.signed,
            signing_claim: value.signing_claim,
            admission_claim: value.admission_claim,
            signing_retry: value.signing_retry,
            admission_retry: value.admission_retry,
            last_failure: value.last_failure,
            created_at_unix_ms: value.created_at_unix_ms,
            updated_at_unix_ms: value.updated_at_unix_ms,
            revision: value.revision,
        }
    }
}

impl AuthoredArtifact {
    pub fn planned(
        artifact_id: AuthoredArtifactId,
        operation_id: OperationInstanceId,
        ordinal: u16,
        plan: &AuthoredEventPlan,
        created_at_unix_ms: u64,
    ) -> Result<Self, Error> {
        Self::reconstruct(Self {
            artifact_id,
            operation_id,
            ordinal,
            origin: ArtifactOrigin::Planned,
            plan: Some(DurableAuthoredPlan::from_plan(plan)?),
            signing_state: SigningState::Planned,
            admission_state: AdmissionState::Pending,
            signed: None,
            signing_claim: None,
            admission_claim: None,
            signing_retry: None,
            admission_retry: None,
            last_failure: None,
            created_at_unix_ms,
            updated_at_unix_ms: created_at_unix_ms,
            revision: NonZeroU64::MIN,
        })
    }

    pub fn imported_signed(
        artifact_id: AuthoredArtifactId,
        operation_id: OperationInstanceId,
        ordinal: u16,
        event: SignedEvent,
        created_at_unix_ms: u64,
    ) -> Result<Self, Error> {
        Self::reconstruct(Self {
            artifact_id,
            operation_id,
            ordinal,
            origin: ArtifactOrigin::ImportedSigned,
            plan: None,
            signing_state: SigningState::Signed,
            admission_state: AdmissionState::Pending,
            signed: Some(ExactSignedArtifact::new(event)),
            signing_claim: None,
            admission_claim: None,
            signing_retry: None,
            admission_retry: None,
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
        if self.created_at_unix_ms == 0 || self.updated_at_unix_ms < self.created_at_unix_ms {
            return Err(Error::InvalidAuthoredArtifact);
        }
        match self.origin {
            ArtifactOrigin::Planned if self.plan.is_none() => {
                return Err(Error::InvalidAuthoredArtifact);
            }
            ArtifactOrigin::ImportedSigned
                if self.plan.is_some()
                    || self.signing_state != SigningState::Signed
                    || self.signed.is_none()
                    || self.signing_claim.is_some()
                    || self.signing_retry.is_some() =>
            {
                return Err(Error::InvalidAuthoredArtifact);
            }
            ArtifactOrigin::Planned | ArtifactOrigin::ImportedSigned => {}
        }
        if self
            .plan
            .as_ref()
            .is_some_and(|plan| plan.decode().is_err())
        {
            return Err(Error::InvalidAuthoredArtifact);
        }
        if matches!(self.signing_state, SigningState::Signed) != self.signed.is_some() {
            return Err(Error::InvalidAuthoredArtifact);
        }
        if self.signed.is_none() && self.admission_state != AdmissionState::Pending {
            return Err(Error::InvalidAuthoredArtifact);
        }
        if matches!(self.signing_state, SigningState::Retryable) != self.signing_retry.is_some()
            || matches!(self.admission_state, AdmissionState::Retryable)
                != self.admission_retry.is_some()
        {
            return Err(Error::InvalidAuthoredArtifact);
        }
        if self.signing_claim.as_ref().is_some_and(|claim| {
            claim.validate().is_err()
                || !matches!(
                    self.signing_state,
                    SigningState::Planned | SigningState::Retryable
                )
        }) || self.admission_claim.as_ref().is_some_and(|claim| {
            claim.validate().is_err()
                || self.signed.is_none()
                || !matches!(
                    self.admission_state,
                    AdmissionState::Pending | AdmissionState::Retryable
                )
        }) {
            return Err(Error::InvalidAuthoredArtifact);
        }
        if self
            .signing_retry
            .as_ref()
            .is_some_and(|retry| retry.failure().phase() != WorkPhase::Signing)
            || self
                .admission_retry
                .as_ref()
                .is_some_and(|retry| retry.failure().phase() != WorkPhase::Admission)
        {
            return Err(Error::InvalidAuthoredArtifact);
        }
        if self
            .last_failure
            .as_ref()
            .is_some_and(|failure| failure.validate().is_err())
        {
            return Err(Error::InvalidAuthoredArtifact);
        }
        let expected_failure = match self.signing_state {
            SigningState::Retryable => self.signing_retry.as_ref().map(RetrySchedule::failure),
            SigningState::Indeterminate => self.last_failure.as_ref().filter(|failure| {
                failure.phase() == WorkPhase::Signing
                    && failure.class() == FailureClass::Indeterminate
            }),
            SigningState::FailedTerminal => self.last_failure.as_ref().filter(|failure| {
                failure.phase() == WorkPhase::Signing && failure.class() == FailureClass::Terminal
            }),
            SigningState::Signed => match self.admission_state {
                AdmissionState::Retryable => {
                    self.admission_retry.as_ref().map(RetrySchedule::failure)
                }
                AdmissionState::Rejected | AdmissionState::Cancelled => {
                    self.last_failure.as_ref().filter(|failure| {
                        failure.phase() == WorkPhase::Admission
                            && failure.class() == FailureClass::Terminal
                    })
                }
                AdmissionState::Pending | AdmissionState::Inserted | AdmissionState::Duplicate => {
                    None
                }
            },
            SigningState::Planned | SigningState::Cancelled => None,
        };
        let failure_required = matches!(
            self.signing_state,
            SigningState::Retryable | SigningState::Indeterminate | SigningState::FailedTerminal
        ) || (self.signing_state == SigningState::Signed
            && matches!(
                self.admission_state,
                AdmissionState::Retryable | AdmissionState::Rejected | AdmissionState::Cancelled
            ));
        if failure_required {
            if expected_failure != self.last_failure.as_ref() {
                return Err(Error::InvalidAuthoredArtifact);
            }
        } else if self.last_failure.is_some() {
            return Err(Error::InvalidAuthoredArtifact);
        }
        if let Some(signed) = &self.signed {
            ExactSignedArtifact::reconstruct(signed.event.clone(), signed.raw_json_sha256)?;
            if let Some(plan) = &self.plan {
                let integrity = plan.decode()?;
                let plan = integrity.plan();
                let event = signed.event();
                if event.id() != plan.expected_event_id()
                    || event.pubkey() != plan.author()
                    || event.created_at() != plan.created_at()
                    || event.wire().kind != plan.body().kind()
                    || event.wire().tags != plan.body().tags()
                    || event.wire().content != plan.body().content()
                {
                    return Err(Error::InvalidAuthoredArtifact);
                }
            }
        }
        Ok(())
    }

    pub fn record_signed(&mut self, event: SignedEvent, at_unix_ms: u64) -> Result<(), Error> {
        if !self.origin.is_resignable()
            || !matches!(
                self.signing_state,
                SigningState::Planned | SigningState::Retryable
            )
        {
            return Err(Error::InvalidAuthoredTransition);
        }
        let previous = self.clone();
        self.signed = Some(ExactSignedArtifact::new(event));
        self.signing_state = SigningState::Signed;
        self.signing_claim = None;
        self.signing_retry = None;
        self.last_failure = None;
        if let Err(error) = self.advance(at_unix_ms) {
            *self = previous;
            return Err(error);
        }
        if let Err(error) = self.validate() {
            *self = previous;
            return Err(error);
        }
        Ok(())
    }

    pub fn set_signing_claim(&mut self, claim: WorkClaim, at_unix_ms: u64) -> Result<(), Error> {
        if !self.origin.is_resignable()
            || !matches!(
                self.signing_state,
                SigningState::Planned | SigningState::Retryable
            )
            || self.signing_claim.is_some()
            || claim.row_revision() != self.revision
        {
            return Err(Error::InvalidAuthoredTransition);
        }
        claim.validate()?;
        let previous = self.clone();
        self.signing_claim = Some(claim);
        if let Err(error) = self.advance(at_unix_ms) {
            *self = previous;
            return Err(error);
        }
        self.validate()
    }

    pub fn record_signing_failure(
        &mut self,
        failure: WorkFailure,
        retry: Option<RetrySchedule>,
        at_unix_ms: u64,
    ) -> Result<(), Error> {
        if failure.phase() != WorkPhase::Signing
            || !self.origin.is_resignable()
            || !matches!(
                self.signing_state,
                SigningState::Planned | SigningState::Retryable
            )
            || retry
                .as_ref()
                .is_some_and(|schedule| schedule.failure() != &failure)
        {
            return Err(Error::InvalidAuthoredTransition);
        }
        let previous = self.clone();
        self.signing_state = match failure.class() {
            FailureClass::Retryable if retry.is_some() => SigningState::Retryable,
            FailureClass::Terminal if retry.is_none() => SigningState::FailedTerminal,
            FailureClass::Indeterminate if retry.is_none() => SigningState::Indeterminate,
            FailureClass::Retryable | FailureClass::Terminal | FailureClass::Indeterminate => {
                return Err(Error::InvalidAuthoredTransition);
            }
        };
        self.signing_claim = None;
        self.signing_retry = retry;
        self.last_failure = Some(failure);
        if let Err(error) = self.advance(at_unix_ms) {
            *self = previous;
            return Err(error);
        }
        if let Err(error) = self.validate() {
            *self = previous;
            return Err(error);
        }
        Ok(())
    }

    pub fn cancel_signing(&mut self, at_unix_ms: u64) -> Result<(), Error> {
        if !self.origin.is_resignable()
            || !matches!(
                self.signing_state,
                SigningState::Planned | SigningState::Retryable
            )
        {
            return Err(Error::InvalidAuthoredTransition);
        }
        let previous = self.clone();
        self.signing_state = SigningState::Cancelled;
        self.signing_claim = None;
        self.signing_retry = None;
        self.last_failure = None;
        if let Err(error) = self.advance(at_unix_ms) {
            *self = previous;
            return Err(error);
        }
        if let Err(error) = self.validate() {
            *self = previous;
            return Err(error);
        }
        Ok(())
    }

    pub fn set_admission_claim(&mut self, claim: WorkClaim, at_unix_ms: u64) -> Result<(), Error> {
        if self.signing_state != SigningState::Signed
            || !matches!(
                self.admission_state,
                AdmissionState::Pending | AdmissionState::Retryable
            )
            || self.admission_claim.is_some()
            || claim.row_revision() != self.revision
        {
            return Err(Error::InvalidAuthoredTransition);
        }
        claim.validate()?;
        let previous = self.clone();
        self.admission_claim = Some(claim);
        if let Err(error) = self.advance(at_unix_ms) {
            *self = previous;
            return Err(error);
        }
        self.validate()
    }

    pub fn record_admission(
        &mut self,
        state: AdmissionState,
        failure: Option<WorkFailure>,
        retry: Option<RetrySchedule>,
        at_unix_ms: u64,
    ) -> Result<(), Error> {
        if self.signing_state != SigningState::Signed
            || self.signed.is_none()
            || !matches!(
                self.admission_state,
                AdmissionState::Pending | AdmissionState::Retryable
            )
            || matches!(state, AdmissionState::Pending)
            || failure
                .as_ref()
                .is_some_and(|value| value.phase() != WorkPhase::Admission)
            || (matches!(state, AdmissionState::Retryable) != retry.is_some())
            || retry.as_ref().is_some_and(|schedule| {
                failure
                    .as_ref()
                    .is_none_or(|value| schedule.failure() != value)
            })
            || (matches!(state, AdmissionState::Retryable)
                && !matches!(
                    failure.as_ref().map(WorkFailure::class),
                    Some(FailureClass::Retryable)
                ))
            || (matches!(state, AdmissionState::Rejected | AdmissionState::Cancelled)
                && (failure.is_none() || retry.is_some()))
            || (matches!(state, AdmissionState::Inserted | AdmissionState::Duplicate)
                && (failure.is_some() || retry.is_some()))
        {
            return Err(Error::InvalidAuthoredTransition);
        }
        let previous = self.clone();
        self.admission_state = state;
        self.admission_claim = None;
        self.admission_retry = retry;
        self.last_failure = failure;
        if let Err(error) = self.advance(at_unix_ms) {
            *self = previous;
            return Err(error);
        }
        if let Err(error) = self.validate() {
            *self = previous;
            return Err(error);
        }
        Ok(())
    }

    fn advance(&mut self, at_unix_ms: u64) -> Result<(), Error> {
        if at_unix_ms < self.updated_at_unix_ms {
            return Err(Error::InvalidAuthoredTransition);
        }
        self.revision = self
            .revision
            .get()
            .checked_add(1)
            .and_then(NonZeroU64::new)
            .ok_or(Error::InvalidAuthoredTransition)?;
        self.updated_at_unix_ms = at_unix_ms;
        Ok(())
    }

    pub const fn artifact_id(&self) -> AuthoredArtifactId {
        self.artifact_id
    }
    pub const fn operation_id(&self) -> OperationInstanceId {
        self.operation_id
    }
    pub const fn ordinal(&self) -> u16 {
        self.ordinal
    }
    pub const fn origin(&self) -> ArtifactOrigin {
        self.origin
    }
    pub const fn plan(&self) -> Option<&DurableAuthoredPlan> {
        self.plan.as_ref()
    }
    pub const fn signing_state(&self) -> SigningState {
        self.signing_state
    }
    pub const fn admission_state(&self) -> AdmissionState {
        self.admission_state
    }
    pub const fn signed(&self) -> Option<&ExactSignedArtifact> {
        self.signed.as_ref()
    }
    pub const fn signing_claim(&self) -> Option<&WorkClaim> {
        self.signing_claim.as_ref()
    }
    pub const fn admission_claim(&self) -> Option<&WorkClaim> {
        self.admission_claim.as_ref()
    }
    pub const fn signing_retry(&self) -> Option<&RetrySchedule> {
        self.signing_retry.as_ref()
    }
    pub const fn admission_retry(&self) -> Option<&RetrySchedule> {
        self.admission_retry.as_ref()
    }
    pub const fn last_failure(&self) -> Option<&WorkFailure> {
        self.last_failure.as_ref()
    }
    pub const fn revision(&self) -> NonZeroU64 {
        self.revision
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(try_from = "AuthoredOperationWire", into = "AuthoredOperationWire")
)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthoredOperation {
    operation_id: OperationInstanceId,
    artifact_ids: Vec<AuthoredArtifactId>,
    created_at_unix_ms: u64,
    updated_at_unix_ms: u64,
    revision: NonZeroU64,
}

#[cfg(feature = "serde")]
#[derive(serde::Serialize, serde::Deserialize)]
struct AuthoredOperationWire {
    operation_id: OperationInstanceId,
    artifact_ids: Vec<AuthoredArtifactId>,
    created_at_unix_ms: u64,
    updated_at_unix_ms: u64,
    revision: NonZeroU64,
}

#[cfg(feature = "serde")]
impl TryFrom<AuthoredOperationWire> for AuthoredOperation {
    type Error = Error;
    fn try_from(value: AuthoredOperationWire) -> Result<Self, Self::Error> {
        Self::reconstruct(
            value.operation_id,
            value.artifact_ids,
            value.created_at_unix_ms,
            value.updated_at_unix_ms,
            value.revision,
        )
    }
}

#[cfg(feature = "serde")]
impl From<AuthoredOperation> for AuthoredOperationWire {
    fn from(value: AuthoredOperation) -> Self {
        Self {
            operation_id: value.operation_id,
            artifact_ids: value.artifact_ids,
            created_at_unix_ms: value.created_at_unix_ms,
            updated_at_unix_ms: value.updated_at_unix_ms,
            revision: value.revision,
        }
    }
}

impl AuthoredOperation {
    pub fn new(
        operation_id: OperationInstanceId,
        artifact_ids: Vec<AuthoredArtifactId>,
        created_at_unix_ms: u64,
    ) -> Result<Self, Error> {
        Self::reconstruct(
            operation_id,
            artifact_ids,
            created_at_unix_ms,
            created_at_unix_ms,
            NonZeroU64::MIN,
        )
    }

    pub fn reconstruct(
        operation_id: OperationInstanceId,
        artifact_ids: Vec<AuthoredArtifactId>,
        created_at_unix_ms: u64,
        updated_at_unix_ms: u64,
        revision: NonZeroU64,
    ) -> Result<Self, Error> {
        if artifact_ids.is_empty()
            || artifact_ids.len() > AUTHORED_OPERATION_ARTIFACTS_MAX
            || created_at_unix_ms == 0
            || updated_at_unix_ms < created_at_unix_ms
            || artifact_ids.iter().collect::<BTreeSet<_>>().len() != artifact_ids.len()
        {
            return Err(Error::InvalidAuthoredOperation);
        }
        Ok(Self {
            operation_id,
            artifact_ids,
            created_at_unix_ms,
            updated_at_unix_ms,
            revision,
        })
    }

    pub const fn operation_id(&self) -> OperationInstanceId {
        self.operation_id
    }
    pub fn artifact_ids(&self) -> &[AuthoredArtifactId] {
        self.artifact_ids.as_slice()
    }
    pub const fn created_at_unix_ms(&self) -> u64 {
        self.created_at_unix_ms
    }
    pub const fn updated_at_unix_ms(&self) -> u64 {
        self.updated_at_unix_ms
    }
    pub const fn revision(&self) -> NonZeroU64 {
        self.revision
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OperationSettlement {
    artifacts: u16,
    signed: u16,
    admitted: u16,
    pending: u16,
    retryable: u16,
    indeterminate: u16,
    failed_terminal: u16,
    cancelled: u16,
}

impl OperationSettlement {
    pub fn evaluate(
        operation: &AuthoredOperation,
        artifacts: &[AuthoredArtifact],
    ) -> Result<Self, Error> {
        if artifacts.len() != operation.artifact_ids.len()
            || artifacts.iter().enumerate().any(|(ordinal, artifact)| {
                artifact.operation_id != operation.operation_id
                    || artifact.artifact_id != operation.artifact_ids[ordinal]
                    || usize::from(artifact.ordinal) != ordinal
                    || artifact.validate().is_err()
            })
        {
            return Err(Error::InvalidAuthoredOperation);
        }
        let mut settlement = Self {
            artifacts: u16::try_from(artifacts.len())
                .map_err(|_| Error::InvalidAuthoredOperation)?,
            signed: 0,
            admitted: 0,
            pending: 0,
            retryable: 0,
            indeterminate: 0,
            failed_terminal: 0,
            cancelled: 0,
        };
        for artifact in artifacts {
            if artifact.signed.is_some() {
                settlement.signed += 1;
            }
            if artifact.admission_state.is_admitted() {
                settlement.admitted += 1;
            }
            match artifact.signing_state {
                SigningState::Planned => settlement.pending += 1,
                SigningState::Retryable => settlement.retryable += 1,
                SigningState::Indeterminate => settlement.indeterminate += 1,
                SigningState::FailedTerminal => settlement.failed_terminal += 1,
                SigningState::Cancelled => settlement.cancelled += 1,
                SigningState::Signed => match artifact.admission_state {
                    AdmissionState::Pending => settlement.pending += 1,
                    AdmissionState::Retryable => settlement.retryable += 1,
                    AdmissionState::Rejected => settlement.failed_terminal += 1,
                    AdmissionState::Cancelled => settlement.cancelled += 1,
                    AdmissionState::Inserted | AdmissionState::Duplicate => {}
                },
            }
        }
        Ok(settlement)
    }

    pub const fn artifacts(self) -> u16 {
        self.artifacts
    }
    pub const fn signed(self) -> u16 {
        self.signed
    }
    pub const fn admitted(self) -> u16 {
        self.admitted
    }
    pub const fn pending(self) -> u16 {
        self.pending
    }
    pub const fn retryable(self) -> u16 {
        self.retryable
    }
    pub const fn indeterminate(self) -> u16 {
        self.indeterminate
    }
    pub const fn failed_terminal(self) -> u16 {
        self.failed_terminal
    }
    pub const fn cancelled(self) -> u16 {
        self.cancelled
    }
    pub const fn is_settled(self) -> bool {
        self.pending == 0 && self.retryable == 0 && self.indeterminate == 0
    }
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

fn valid_text(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && value == value.trim()
        && !value.chars().any(char::is_control)
}

fn valid_code(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= WORK_FAILURE_CODE_MAX_BYTES
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-' | b'.')
        })
}
