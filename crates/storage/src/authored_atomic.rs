//! Atomic authored-operation commands with deterministic phase identities.

use core::num::NonZeroU64;
use radroots_event::SignedEvent;
use radroots_transport::BoxFuture;
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, vec::Vec};

use crate::{
    Error,
    atomic::{AtomicCommitDigest, AtomicCommitDisposition, AtomicCommitId},
    authored::{
        AdmissionState, AuthoredArtifact, AuthoredArtifactId, AuthoredOperation, RetrySchedule,
        WorkFailure, WorkPhase,
    },
    authored_delivery::{AuthoredDeliveryPlan, AuthoredDeliveryPlanId, DeliveryAttemptOutcome},
    journal::OperationInstanceId,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkFence {
    token: [u8; 16],
    generation: NonZeroU64,
    row_revision: NonZeroU64,
}

impl WorkFence {
    pub const fn new(
        token: [u8; 16],
        generation: NonZeroU64,
        row_revision: NonZeroU64,
    ) -> Result<Self, Error> {
        if bytes_are_zero(&token) {
            return Err(Error::InvalidWorkClaim);
        }
        Ok(Self {
            token,
            generation,
            row_revision,
        })
    }
    pub const fn token(&self) -> &[u8; 16] {
        &self.token
    }
    pub const fn generation(&self) -> NonZeroU64 {
        self.generation
    }
    pub const fn row_revision(&self) -> NonZeroU64 {
        self.row_revision
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrepareAuthoredOperation {
    operation: AuthoredOperation,
    artifacts: Vec<AuthoredArtifact>,
    delivery_plans: Vec<AuthoredDeliveryPlan>,
    input_digest: AtomicCommitDigest,
    requested_at_unix_ms: u64,
}

impl PrepareAuthoredOperation {
    pub fn new(
        operation: AuthoredOperation,
        artifacts: Vec<AuthoredArtifact>,
        delivery_plans: Vec<AuthoredDeliveryPlan>,
        input_digest: AtomicCommitDigest,
        requested_at_unix_ms: u64,
    ) -> Result<Self, Error> {
        if requested_at_unix_ms == 0
            || artifacts.len() != operation.artifact_ids().len()
            || artifacts.iter().enumerate().any(|(ordinal, artifact)| {
                artifact.operation_id() != operation.operation_id()
                    || artifact.artifact_id() != operation.artifact_ids()[ordinal]
                    || usize::from(artifact.ordinal()) != ordinal
                    || artifact.validate().is_err()
            })
        {
            return Err(Error::AtomicWorkflowMismatch);
        }
        let artifact_ids: BTreeSet<_> = artifacts
            .iter()
            .map(AuthoredArtifact::artifact_id)
            .collect();
        let plan_ids: BTreeSet<_> = delivery_plans
            .iter()
            .map(AuthoredDeliveryPlan::plan_id)
            .collect();
        if plan_ids.len() != delivery_plans.len()
            || delivery_plans
                .iter()
                .any(|plan| !artifact_ids.contains(&plan.artifact_id()) || plan.validate().is_err())
        {
            return Err(Error::AtomicWorkflowMismatch);
        }
        Ok(Self {
            operation,
            artifacts,
            delivery_plans,
            input_digest,
            requested_at_unix_ms,
        })
    }
    pub const fn operation(&self) -> &AuthoredOperation {
        &self.operation
    }
    pub fn artifacts(&self) -> &[AuthoredArtifact] {
        self.artifacts.as_slice()
    }
    pub fn delivery_plans(&self) -> &[AuthoredDeliveryPlan] {
        self.delivery_plans.as_slice()
    }
    pub const fn input_digest(&self) -> AtomicCommitDigest {
        self.input_digest
    }
    pub const fn requested_at_unix_ms(&self) -> u64 {
        self.requested_at_unix_ms
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApplySignedArtifact {
    artifact_id: AuthoredArtifactId,
    fence: WorkFence,
    event: SignedEvent,
    applied_at_unix_ms: u64,
}

impl ApplySignedArtifact {
    pub fn new(
        artifact_id: AuthoredArtifactId,
        fence: WorkFence,
        event: SignedEvent,
        applied_at_unix_ms: u64,
    ) -> Result<Self, Error> {
        if applied_at_unix_ms == 0 {
            return Err(Error::AtomicWorkflowMismatch);
        }
        Ok(Self {
            artifact_id,
            fence,
            event,
            applied_at_unix_ms,
        })
    }
    pub const fn artifact_id(&self) -> AuthoredArtifactId {
        self.artifact_id
    }
    pub const fn fence(&self) -> &WorkFence {
        &self.fence
    }
    pub const fn event(&self) -> &SignedEvent {
        &self.event
    }
    pub const fn applied_at_unix_ms(&self) -> u64 {
        self.applied_at_unix_ms
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApplyAdmissionResult {
    artifact_id: AuthoredArtifactId,
    fence: WorkFence,
    state: AdmissionState,
    failure: Option<WorkFailure>,
    retry: Option<RetrySchedule>,
    applied_at_unix_ms: u64,
}

impl ApplyAdmissionResult {
    pub fn new(
        artifact_id: AuthoredArtifactId,
        fence: WorkFence,
        state: AdmissionState,
        failure: Option<WorkFailure>,
        retry: Option<RetrySchedule>,
        applied_at_unix_ms: u64,
    ) -> Result<Self, Error> {
        if applied_at_unix_ms == 0 {
            return Err(Error::AtomicWorkflowMismatch);
        }
        Ok(Self {
            artifact_id,
            fence,
            state,
            failure,
            retry,
            applied_at_unix_ms,
        })
    }
    pub const fn artifact_id(&self) -> AuthoredArtifactId {
        self.artifact_id
    }
    pub const fn fence(&self) -> &WorkFence {
        &self.fence
    }
    pub const fn state(&self) -> AdmissionState {
        self.state
    }
    pub const fn failure(&self) -> Option<&WorkFailure> {
        self.failure.as_ref()
    }
    pub const fn retry(&self) -> Option<&RetrySchedule> {
        self.retry.as_ref()
    }
    pub const fn applied_at_unix_ms(&self) -> u64 {
        self.applied_at_unix_ms
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApplyDeliveryAttempt {
    plan_id: AuthoredDeliveryPlanId,
    fence: WorkFence,
    outcome: DeliveryAttemptOutcome,
    retry: Option<RetrySchedule>,
    applied_at_unix_ms: u64,
}

impl ApplyDeliveryAttempt {
    pub fn new(
        plan_id: AuthoredDeliveryPlanId,
        fence: WorkFence,
        outcome: DeliveryAttemptOutcome,
        retry: Option<RetrySchedule>,
        applied_at_unix_ms: u64,
    ) -> Result<Self, Error> {
        if applied_at_unix_ms == 0 {
            return Err(Error::AtomicWorkflowMismatch);
        }
        Ok(Self {
            plan_id,
            fence,
            outcome,
            retry,
            applied_at_unix_ms,
        })
    }
    pub const fn plan_id(&self) -> AuthoredDeliveryPlanId {
        self.plan_id
    }
    pub const fn fence(&self) -> &WorkFence {
        &self.fence
    }
    pub const fn outcome(&self) -> &DeliveryAttemptOutcome {
        &self.outcome
    }
    pub const fn retry(&self) -> Option<&RetrySchedule> {
        self.retry.as_ref()
    }
    pub const fn applied_at_unix_ms(&self) -> u64 {
        self.applied_at_unix_ms
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuthoredWorkTarget {
    Artifact(AuthoredArtifactId),
    DeliveryPlan(AuthoredDeliveryPlanId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClaimAuthoredTarget {
    ArtifactSigning(AuthoredArtifactId),
    ArtifactAdmission(AuthoredArtifactId),
    DeliveryPlan(AuthoredDeliveryPlanId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClaimAuthoredWork {
    target: ClaimAuthoredTarget,
    claim: crate::authored::WorkClaim,
}

impl ClaimAuthoredWork {
    pub const fn new(target: ClaimAuthoredTarget, claim: crate::authored::WorkClaim) -> Self {
        Self { target, claim }
    }
    pub const fn target(&self) -> &ClaimAuthoredTarget {
        &self.target
    }
    pub const fn claim(&self) -> &crate::authored::WorkClaim {
        &self.claim
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApplyWorkFailure {
    target: AuthoredWorkTarget,
    fence: WorkFence,
    failure: WorkFailure,
    retry: Option<RetrySchedule>,
    applied_at_unix_ms: u64,
}

impl ApplyWorkFailure {
    pub fn new(
        target: AuthoredWorkTarget,
        fence: WorkFence,
        failure: WorkFailure,
        retry: Option<RetrySchedule>,
        applied_at_unix_ms: u64,
    ) -> Result<Self, Error> {
        if applied_at_unix_ms == 0 {
            return Err(Error::AtomicWorkflowMismatch);
        }
        Ok(Self {
            target,
            fence,
            failure,
            retry,
            applied_at_unix_ms,
        })
    }
    pub const fn target(&self) -> &AuthoredWorkTarget {
        &self.target
    }
    pub const fn fence(&self) -> &WorkFence {
        &self.fence
    }
    pub const fn failure(&self) -> &WorkFailure {
        &self.failure
    }
    pub const fn retry(&self) -> Option<&RetrySchedule> {
        self.retry.as_ref()
    }
    pub const fn applied_at_unix_ms(&self) -> u64 {
        self.applied_at_unix_ms
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CancelAuthoredTarget {
    ArtifactSigning(AuthoredArtifactId),
    ArtifactAdmission(AuthoredArtifactId),
    DeliveryPlan(AuthoredDeliveryPlanId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CancelAuthoredWork {
    target: CancelAuthoredTarget,
    expected_revision: NonZeroU64,
    cancelled_at_unix_ms: u64,
}

impl CancelAuthoredWork {
    pub const fn new(
        target: CancelAuthoredTarget,
        expected_revision: NonZeroU64,
        cancelled_at_unix_ms: u64,
    ) -> Result<Self, Error> {
        if cancelled_at_unix_ms == 0 {
            return Err(Error::AtomicWorkflowMismatch);
        }
        Ok(Self {
            target,
            expected_revision,
            cancelled_at_unix_ms,
        })
    }
    pub const fn target(&self) -> &CancelAuthoredTarget {
        &self.target
    }
    pub const fn expected_revision(&self) -> NonZeroU64 {
        self.expected_revision
    }
    pub const fn cancelled_at_unix_ms(&self) -> u64 {
        self.cancelled_at_unix_ms
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuthoredAtomicCommand {
    Prepare(PrepareAuthoredOperation),
    Claim(ClaimAuthoredWork),
    ApplySigned(ApplySignedArtifact),
    ApplyAdmission(ApplyAdmissionResult),
    ApplyDelivery(ApplyDeliveryAttempt),
    ApplyFailure(ApplyWorkFailure),
    Cancel(CancelAuthoredWork),
}

impl AuthoredAtomicCommand {
    pub fn commit_id(&self) -> AtomicCommitId {
        let digest = self.digest();
        let mut hasher = Sha256::new();
        hash_field(&mut hasher, b"radroots.authored.atomic.id.v2");
        hash_field(&mut hasher, self.phase_bytes());
        hash_field(&mut hasher, &self.target_bytes());
        if let Some(generation) = self.generation() {
            hasher.update(generation.get().to_be_bytes());
        }
        hash_field(&mut hasher, digest.as_bytes());
        let bytes: [u8; 32] = hasher.finalize().into();
        let mut id = [0_u8; 16];
        id.copy_from_slice(&bytes[..16]);
        AtomicCommitId::new(id).expect("SHA-256 derived commit identity is nonzero")
    }

    pub fn digest(&self) -> AtomicCommitDigest {
        let mut hasher = Sha256::new();
        hash_field(&mut hasher, b"radroots.authored.atomic.digest.v2");
        hash_field(&mut hasher, self.phase_bytes());
        hash_field(&mut hasher, &self.target_bytes());
        match self {
            Self::Prepare(value) => hash_field(&mut hasher, value.input_digest.as_bytes()),
            Self::Claim(value) => {
                hash_field(&mut hasher, value.claim.token());
                hasher.update(value.claim.generation().get().to_be_bytes());
                hasher.update(value.claim.row_revision().get().to_be_bytes());
            }
            Self::ApplySigned(value) => hash_field(&mut hasher, value.event.raw_json().as_bytes()),
            Self::ApplyAdmission(value) => {
                hasher.update([value.state as u8]);
                hash_failure(&mut hasher, value.failure.as_ref());
            }
            Self::ApplyDelivery(value) => hash_delivery(&mut hasher, &value.outcome),
            Self::ApplyFailure(value) => hash_failure(&mut hasher, Some(&value.failure)),
            Self::Cancel(value) => hasher.update(value.cancelled_at_unix_ms.to_be_bytes()),
        }
        AtomicCommitDigest::new(hasher.finalize().into())
    }

    pub const fn requested_at_unix_ms(&self) -> u64 {
        match self {
            Self::Prepare(value) => value.requested_at_unix_ms,
            Self::Claim(value) => value.claim.acquired_at_unix_ms(),
            Self::ApplySigned(value) => value.applied_at_unix_ms,
            Self::ApplyAdmission(value) => value.applied_at_unix_ms,
            Self::ApplyDelivery(value) => value.applied_at_unix_ms,
            Self::ApplyFailure(value) => value.applied_at_unix_ms,
            Self::Cancel(value) => value.cancelled_at_unix_ms,
        }
    }

    fn phase_bytes(&self) -> &'static [u8] {
        match self {
            Self::Prepare(_) => b"prepare",
            Self::Claim(_) => b"claim",
            Self::ApplySigned(_) => b"signing",
            Self::ApplyAdmission(_) => b"admission",
            Self::ApplyDelivery(_) => b"delivery",
            Self::ApplyFailure(value) => match value.failure.phase() {
                WorkPhase::Signing => b"signing_failure",
                WorkPhase::Admission => b"admission_failure",
                WorkPhase::Delivery => b"delivery_failure",
            },
            Self::Cancel(_) => b"cancel",
        }
    }

    fn target_bytes(&self) -> [u8; 16] {
        match self {
            Self::Prepare(value) => *value.operation.operation_id().as_bytes(),
            Self::Claim(value) => match &value.target {
                ClaimAuthoredTarget::ArtifactSigning(id)
                | ClaimAuthoredTarget::ArtifactAdmission(id) => *id.as_bytes(),
                ClaimAuthoredTarget::DeliveryPlan(id) => *id.as_bytes(),
            },
            Self::ApplySigned(value) => *value.artifact_id.as_bytes(),
            Self::ApplyAdmission(value) => *value.artifact_id.as_bytes(),
            Self::ApplyDelivery(value) => *value.plan_id.as_bytes(),
            Self::ApplyFailure(value) => match &value.target {
                AuthoredWorkTarget::Artifact(id) => *id.as_bytes(),
                AuthoredWorkTarget::DeliveryPlan(id) => *id.as_bytes(),
            },
            Self::Cancel(value) => match &value.target {
                CancelAuthoredTarget::ArtifactSigning(id)
                | CancelAuthoredTarget::ArtifactAdmission(id) => *id.as_bytes(),
                CancelAuthoredTarget::DeliveryPlan(id) => *id.as_bytes(),
            },
        }
    }

    fn generation(&self) -> Option<NonZeroU64> {
        match self {
            Self::ApplySigned(value) => Some(value.fence.generation),
            Self::Claim(value) => Some(value.claim.generation()),
            Self::ApplyAdmission(value) => Some(value.fence.generation),
            Self::ApplyDelivery(value) => Some(value.fence.generation),
            Self::ApplyFailure(value) => Some(value.fence.generation),
            Self::Prepare(_) | Self::Cancel(_) => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuthoredAtomicOutcome {
    Prepared {
        operation: AuthoredOperation,
        artifacts: Vec<AuthoredArtifact>,
        delivery_plans: Vec<AuthoredDeliveryPlan>,
    },
    Artifact(AuthoredArtifact),
    DeliveryPlan(AuthoredDeliveryPlan),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthoredAtomicReceipt {
    commit_id: AtomicCommitId,
    digest: AtomicCommitDigest,
    disposition: AtomicCommitDisposition,
    committed_at_unix_ms: u64,
    outcome: AuthoredAtomicOutcome,
}

impl AuthoredAtomicReceipt {
    pub fn new(
        command: &AuthoredAtomicCommand,
        disposition: AtomicCommitDisposition,
        committed_at_unix_ms: u64,
        outcome: AuthoredAtomicOutcome,
    ) -> Result<Self, Error> {
        if committed_at_unix_ms < command.requested_at_unix_ms() {
            return Err(Error::AtomicWorkflowMismatch);
        }
        Ok(Self {
            commit_id: command.commit_id(),
            digest: command.digest(),
            disposition,
            committed_at_unix_ms,
            outcome,
        })
    }
    pub const fn commit_id(&self) -> AtomicCommitId {
        self.commit_id
    }
    pub const fn digest(&self) -> AtomicCommitDigest {
        self.digest
    }
    pub const fn disposition(&self) -> AtomicCommitDisposition {
        self.disposition
    }
    pub const fn committed_at_unix_ms(&self) -> u64 {
        self.committed_at_unix_ms
    }
    pub const fn outcome(&self) -> &AuthoredAtomicOutcome {
        &self.outcome
    }
}

pub trait AuthoredAtomicStorage: Send + Sync {
    fn execute_authored(
        &self,
        command: AuthoredAtomicCommand,
    ) -> BoxFuture<'_, Result<AuthoredAtomicReceipt, Error>>;
    fn authored_receipt(
        &self,
        commit_id: AtomicCommitId,
    ) -> BoxFuture<'_, Result<Option<AuthoredAtomicReceipt>, Error>>;
    fn authored_operation(
        &self,
        operation_id: OperationInstanceId,
    ) -> BoxFuture<'_, Result<Option<AuthoredOperation>, Error>>;
    fn authored_artifact(
        &self,
        artifact_id: AuthoredArtifactId,
    ) -> BoxFuture<'_, Result<Option<AuthoredArtifact>, Error>>;
    fn authored_delivery_plan(
        &self,
        plan_id: AuthoredDeliveryPlanId,
    ) -> BoxFuture<'_, Result<Option<AuthoredDeliveryPlan>, Error>>;
}

fn hash_failure(hasher: &mut Sha256, failure: Option<&WorkFailure>) {
    if let Some(failure) = failure {
        hash_field(hasher, failure.code().as_bytes());
        hasher.update([failure.phase() as u8, failure.class() as u8]);
        hasher.update(
            failure
                .retry_after_unix_ms()
                .unwrap_or_default()
                .to_be_bytes(),
        );
        if let Some(diagnostic) = failure.diagnostic() {
            hash_field(hasher, diagnostic.as_bytes());
        }
    }
}

fn hash_delivery(hasher: &mut Sha256, outcome: &DeliveryAttemptOutcome) {
    let entries = match outcome {
        DeliveryAttemptOutcome::Receipt(receipt) => receipt.target_receipts(),
        DeliveryAttemptOutcome::SinkFailure(failure) => {
            hash_field(hasher, failure.code().as_bytes());
            hasher.update([failure.retryability() as u8]);
            failure.partial_evidence()
        }
    };
    for entry in entries {
        hash_field(hasher, entry.target().fingerprint().as_str().as_bytes());
        hasher.update([entry.was_attempted() as u8, entry.outcome().kind() as u8]);
        hasher.update([entry.outcome().retryability() as u8]);
        if let Some(code) = entry.outcome().code() {
            hash_field(hasher, code.as_bytes());
        }
        if let Some(message) = entry.outcome().message() {
            hash_field(hasher, message.as_bytes());
        }
    }
}

fn hash_field(hasher: &mut Sha256, value: &[u8]) {
    hasher.update(u64::try_from(value.len()).unwrap_or(u64::MAX).to_be_bytes());
    hasher.update(value);
}

const fn bytes_are_zero(bytes: &[u8; 16]) -> bool {
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != 0 {
            return false;
        }
        index += 1;
    }
    true
}
