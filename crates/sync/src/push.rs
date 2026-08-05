//! Signing, durable enqueue, delivery, and satisfaction orchestration.

use core::num::{NonZeroU32, NonZeroU64};
use radroots_event::admission::RawEvent;
use radroots_event_codec::{
    authoring::AuthoredEventPlan,
    verify::{self, Nip01SignatureVerifier},
};
use radroots_protocol::runtime::v1::OperationId;
use radroots_signing::{
    Actor, AuthoredArtifactId as SigningArtifactId, SigningIntentId, SigningOperationId,
    recovery::{RecoveryDisposition, ReplayCapability, recovery_disposition},
    request::{CancellationPolicy, SignPolicy},
};
use radroots_storage::{
    Journal, Outbox,
    atomic::{
        AtomicCommit, AtomicCommitDigest, AtomicCommitDisposition, AtomicCommitId,
        AtomicCommitOutcome, AtomicWorkflow, CommitEnqueued, CommitSigned,
    },
    authored::{
        AdmissionState, AuthoredArtifact, AuthoredArtifactId, AuthoredOperation, FailureClass,
        OperationSettlement, RetrySchedule, SigningState, WorkClaim, WorkFailure, WorkPhase,
    },
    authored_atomic::{
        ApplyAdmissionResult, ApplyDeliveryAttempt, ApplySignedArtifact, ApplyWorkFailure,
        AuthoredAtomicCommand, AuthoredAtomicOutcome, AuthoredWorkTarget, CancelAuthoredTarget,
        CancelAuthoredWork, ClaimAuthoredTarget, ClaimAuthoredWork, PrepareAuthoredOperation,
        WorkFence,
    },
    authored_delivery::{
        AuthoredDeliveryIntent, AuthoredDeliveryPlan, AuthoredDeliveryPlanId,
        DELIVERY_PLAN_ATTEMPTS_MAX, DeliveryAttemptOutcome,
    },
    event::{AdmissionDisposition, EventAdmission, EventStore},
    journal::{
        IdempotencyDigest, IdempotencyKey, JournalStage, JournalState, OperationInstanceId,
        PrepareOperation,
    },
    outbox::{
        ClaimOutboxItems, DeliveryAttempt, DeliveryAttemptEvidence, DeliveryPlanDigest,
        EnqueueOutboxItem, LeaseId, LeaseOwner, OUTBOX_CLAIM_LIMIT_MAX, OutboxItemId, OutboxRecord,
    },
};
use radroots_transport::{
    DeliveryReceipt, DeliveryRequest, SinkFailure, Target, TransportId,
    outcome::{DeliveryOutcome, DeliveryOutcomeKind, Retryability},
    policy::{SatisfactionPolicy, SatisfactionState},
    sink::{DeliveryPayload, DeliveryTargetReceipt},
    source::{EventProvenance, ObservedEvent},
    target::TargetSet,
};
use sha2::{Digest, Sha256};

use crate::{
    Engine,
    ingest::{AdmissionDecision, AdmissionPolicy, RegistryPolicy},
    policy::{Error, OperationKind, SyncId},
};

const MAX_DELIVERY_LEASE_MS: u64 = 86_400_000;
const SIGNING_CLAIM_OWNER_EXACT: &str = "radroots-sync-signing-exact";
const SIGNING_CLAIM_OWNER_LOCAL: &str = "radroots-sync-signing-local";
const SIGNING_CLAIM_OWNER_NON_REPLAYABLE: &str = "radroots-sync-signing-non-replayable";
const ADMISSION_CLAIM_OWNER: &str = "radroots-sync-admission";
const DELIVERY_CLAIM_OWNER: &str = "radroots-sync-delivery";
const WORK_RETRY_DELAY_MS: u64 = 1_000;

/// Caller-owned, replay-stable inputs for one outbound operation.
#[derive(Clone)]
pub struct PushRequest {
    operation_id: SyncId,
    idempotency_key: IdempotencyKey,
    actor: Actor,
    plan: AuthoredEventPlan,
    targets: TargetSet,
    satisfaction: SatisfactionPolicy,
    delivery_deadline_unix_ms: u64,
    cancellation: CancellationPolicy,
}

impl PushRequest {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        operation_id: SyncId,
        idempotency_key: IdempotencyKey,
        actor: Actor,
        plan: AuthoredEventPlan,
        targets: TargetSet,
        satisfaction: SatisfactionPolicy,
        delivery_deadline_unix_ms: u64,
        cancellation: CancellationPolicy,
    ) -> Result<Self, Error> {
        if satisfaction.validate_for(&targets).is_err()
            || delivery_deadline_unix_ms == 0
            || actor.public_key() != *plan.author()
        {
            return Err(Error::InvalidPushRequest);
        }
        Ok(Self {
            operation_id,
            idempotency_key,
            actor,
            plan,
            targets,
            satisfaction,
            delivery_deadline_unix_ms,
            cancellation,
        })
    }

    pub const fn operation_id(&self) -> SyncId {
        self.operation_id
    }
    pub const fn idempotency_key(&self) -> &IdempotencyKey {
        &self.idempotency_key
    }
    pub const fn actor(&self) -> &Actor {
        &self.actor
    }
    pub const fn plan(&self) -> &AuthoredEventPlan {
        &self.plan
    }
    pub const fn targets(&self) -> &TargetSet {
        &self.targets
    }
    pub const fn satisfaction(&self) -> &SatisfactionPolicy {
        &self.satisfaction
    }
    pub const fn delivery_deadline_unix_ms(&self) -> u64 {
        self.delivery_deadline_unix_ms
    }
    pub const fn cancellation(&self) -> CancellationPolicy {
        self.cancellation
    }
}

impl core::fmt::Debug for PushRequest {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("PushRequest")
            .field("operation_id", &self.operation_id)
            .field("idempotency_key", &self.idempotency_key)
            .field("actor", &self.actor)
            .field("plan", &"[redacted exact authored plan]")
            .field("targets", &self.targets)
            .field("satisfaction", &self.satisfaction)
            .field("delivery_deadline_unix_ms", &self.delivery_deadline_unix_ms)
            .field("cancellation", &self.cancellation)
            .finish()
    }
}

/// Complete durable intent created before any signer or transport effect.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PushPreparation {
    operation: AuthoredOperation,
    artifact: AuthoredArtifact,
    delivery_plan: AuthoredDeliveryPlan,
    replay: bool,
}

impl PushPreparation {
    pub const fn operation(&self) -> &AuthoredOperation {
        &self.operation
    }
    pub const fn artifact(&self) -> &AuthoredArtifact {
        &self.artifact
    }
    pub const fn delivery_plan(&self) -> &AuthoredDeliveryPlan {
        &self.delivery_plan
    }
    pub const fn is_replay(&self) -> bool {
        self.replay
    }
}

/// Current durable authored state for one push operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PushStatus {
    operation: AuthoredOperation,
    artifact: AuthoredArtifact,
    delivery_plan: AuthoredDeliveryPlan,
    settlement: OperationSettlement,
}

impl PushStatus {
    pub const fn operation(&self) -> &AuthoredOperation {
        &self.operation
    }
    pub const fn artifact(&self) -> &AuthoredArtifact {
        &self.artifact
    }
    pub const fn delivery_plan(&self) -> &AuthoredDeliveryPlan {
        &self.delivery_plan
    }
    pub const fn settlement(&self) -> OperationSettlement {
        self.settlement
    }
}

/// Durable result of one bounded signing execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SigningRunReceipt {
    artifact: AuthoredArtifact,
    replay: bool,
}

impl SigningRunReceipt {
    pub const fn artifact(&self) -> &AuthoredArtifact {
        &self.artifact
    }
    pub const fn is_replay(&self) -> bool {
        self.replay
    }
}

/// Durable result of one bounded local-admission execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmissionRunReceipt {
    artifact: AuthoredArtifact,
    replay: bool,
}

/// Durable result of one bounded authored-delivery execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeliveryExecutionReceipt {
    plan: AuthoredDeliveryPlan,
    replay: bool,
}

impl DeliveryExecutionReceipt {
    pub const fn plan(&self) -> &AuthoredDeliveryPlan {
        &self.plan
    }
    pub const fn is_replay(&self) -> bool {
        self.replay
    }
}

impl AdmissionRunReceipt {
    pub const fn artifact(&self) -> &AuthoredArtifact {
        &self.artifact
    }
    pub const fn is_replay(&self) -> bool {
        self.replay
    }
}

/// Durable result after signing and atomic outbox enqueue.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PushReceipt {
    operation_id: SyncId,
    outbox: OutboxRecord,
    replay: bool,
}

impl PushReceipt {
    pub const fn operation_id(&self) -> SyncId {
        self.operation_id
    }
    pub const fn outbox(&self) -> &OutboxRecord {
        &self.outbox
    }
    pub const fn is_replay(&self) -> bool {
        self.replay
    }
}

/// Bounds and lease authority for one explicit outbox delivery pass.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeliveryRunRequest {
    owner: LeaseOwner,
    lease_seed: SyncId,
    lease_duration_ms: u64,
    limit: u16,
}

impl DeliveryRunRequest {
    pub fn new(
        owner: LeaseOwner,
        lease_seed: SyncId,
        lease_duration_ms: u64,
        limit: u16,
    ) -> Result<Self, Error> {
        if lease_duration_ms == 0
            || lease_duration_ms > MAX_DELIVERY_LEASE_MS
            || limit == 0
            || limit > OUTBOX_CLAIM_LIMIT_MAX
        {
            return Err(Error::InvalidDeliveryRequest);
        }
        Ok(Self {
            owner,
            lease_seed,
            lease_duration_ms,
            limit,
        })
    }
}

/// Independent durable outcomes from one bounded delivery pass.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeliveryRunReceipt {
    outcomes: Vec<Result<OutboxRecord, Error>>,
}

impl DeliveryRunReceipt {
    pub fn outcomes(&self) -> &[Result<OutboxRecord, Error>] {
        self.outcomes.as_slice()
    }
    pub fn succeeded(&self) -> usize {
        self.outcomes
            .iter()
            .filter(|outcome| outcome.is_ok())
            .count()
    }
    pub fn failed(&self) -> usize {
        self.outcomes.len() - self.succeeded()
    }
}

impl Engine {
    /// Atomically persists the complete parent, exact plan, and delivery intent.
    ///
    /// This method invokes neither a signer nor a transport. Exact request
    /// replay returns the original durable preparation; conflicting reuse of
    /// the operation identity fails closed.
    pub async fn prepare_push(&self, request: PushRequest) -> Result<PushPreparation, Error> {
        let prepared_at = self.clock.now_unix_ms()?;
        let (operation_id, artifact_id, delivery_plan_id) = authored_ids(request.operation_id)?;
        let operation = AuthoredOperation::new(operation_id, vec![artifact_id], prepared_at)
            .map_err(map_storage_error)?;
        let artifact =
            AuthoredArtifact::planned(artifact_id, operation_id, 0, request.plan(), prepared_at)
                .map_err(map_storage_error)?;
        let intent = AuthoredDeliveryIntent::new(
            delivery_request_id(request.operation_id),
            request.targets.clone(),
            request.satisfaction.clone(),
            request.delivery_deadline_unix_ms,
        )
        .map_err(map_storage_error)?;
        let delivery_plan =
            AuthoredDeliveryPlan::new(delivery_plan_id, artifact_id, intent, prepared_at)
                .map_err(map_storage_error)?;
        let command = AuthoredAtomicCommand::Prepare(
            PrepareAuthoredOperation::new(
                operation,
                vec![artifact],
                vec![delivery_plan],
                authored_push_input_digest(&request)?,
                prepared_at,
            )
            .map_err(map_storage_error)?,
        );
        let receipt = self
            .storage
            .execute_authored(command)
            .await
            .map_err(map_storage_error)?;
        let AuthoredAtomicOutcome::Prepared {
            operation,
            artifacts,
            delivery_plans,
        } = receipt.outcome()
        else {
            return Err(Error::StorageFailed);
        };
        let [artifact] = artifacts.as_slice() else {
            return Err(Error::StorageFailed);
        };
        let [delivery_plan] = delivery_plans.as_slice() else {
            return Err(Error::StorageFailed);
        };
        if operation.operation_id() != operation_id
            || artifact.artifact_id() != artifact_id
            || delivery_plan.plan_id() != delivery_plan_id
        {
            return Err(Error::StorageFailed);
        }
        Ok(PushPreparation {
            operation: operation.clone(),
            artifact: artifact.clone(),
            delivery_plan: delivery_plan.clone(),
            replay: receipt.disposition() == AtomicCommitDisposition::Replay,
        })
    }

    /// Loads the complete durable state for one prepared push operation.
    pub async fn push_status(&self, operation_id: SyncId) -> Result<Option<PushStatus>, Error> {
        let (operation_id, expected_artifact_id, expected_plan_id) = authored_ids(operation_id)?;
        let Some(operation) = self
            .storage
            .authored_operation(operation_id)
            .await
            .map_err(map_storage_error)?
        else {
            return Ok(None);
        };
        if operation.artifact_ids() != [expected_artifact_id] {
            return Err(Error::StorageFailed);
        }
        let artifact = self
            .storage
            .authored_artifact(expected_artifact_id)
            .await
            .map_err(map_storage_error)?
            .ok_or(Error::StorageFailed)?;
        let delivery_plan = self
            .storage
            .authored_delivery_plan(expected_plan_id)
            .await
            .map_err(map_storage_error)?
            .ok_or(Error::StorageFailed)?;
        if artifact.operation_id() != operation.operation_id()
            || delivery_plan.artifact_id() != artifact.artifact_id()
        {
            return Err(Error::StorageFailed);
        }
        let settlement = OperationSettlement::evaluate_complete(
            &operation,
            core::slice::from_ref(&artifact),
            core::slice::from_ref(&delivery_plan),
        )
        .map_err(map_storage_error)?;
        Ok(Some(PushStatus {
            operation,
            artifact,
            delivery_plan,
            settlement,
        }))
    }

    /// Claims and executes one prepared signing artifact.
    pub async fn sign_prepared(&self, request: PushRequest) -> Result<SigningRunReceipt, Error> {
        let signer = self.signer.as_deref().ok_or(Error::MissingSigner)?;
        self.prepare_push(request.clone()).await?;
        let status = self
            .push_status(request.operation_id)
            .await?
            .ok_or(Error::StorageFailed)?;
        let artifact = status.artifact;
        match artifact.signing_state() {
            SigningState::Signed => {
                return Ok(SigningRunReceipt {
                    artifact,
                    replay: true,
                });
            }
            SigningState::Indeterminate => return Err(Error::SigningIndeterminate),
            SigningState::FailedTerminal | SigningState::Cancelled => {
                return Err(Error::SignerFailed);
            }
            SigningState::Planned | SigningState::Retryable => {}
        }

        let now = self.clock.now_unix_ms()?.max(artifact.updated_at_unix_ms());
        if let Some(existing) = artifact.signing_claim() {
            if now < existing.expires_at_unix_ms() {
                return Err(Error::WorkClaimConflict);
            }
            if existing.owner() == SIGNING_CLAIM_OWNER_NON_REPLAYABLE {
                let (claimed, fence) = self
                    .claim_artifact(
                        artifact,
                        ClaimAuthoredTarget::ArtifactSigning,
                        SIGNING_CLAIM_OWNER_NON_REPLAYABLE,
                        now,
                    )
                    .await?;
                let failure = WorkFailure::new(
                    "signing_effect_unknown_after_restart",
                    WorkPhase::Signing,
                    FailureClass::Indeterminate,
                    None,
                    None,
                )
                .map_err(map_storage_error)?;
                self.apply_artifact_failure(claimed.artifact_id(), fence, failure, None)
                    .await?;
                return Err(Error::SigningIndeterminate);
            }
        }

        let signer_status = signer.status().await.map_err(|_| Error::SignerFailed)?;
        let replay_capability = signer_replay_capability(&signer_status)?;
        let (claimed, fence) = self
            .claim_artifact(
                artifact,
                ClaimAuthoredTarget::ArtifactSigning,
                signing_claim_owner(replay_capability),
                now,
            )
            .await?;
        let persisted_plan = claimed
            .plan()
            .ok_or(Error::StorageFailed)?
            .decode()
            .map_err(map_storage_error)?
            .into_plan();
        if persisted_plan != request.plan {
            return Err(Error::StorageConflict);
        }
        let deadline = self.deadlines.deadline_unix_ms(OperationKind::Sign, now)?;
        let signing_operation = SigningOperationId::new(*request.operation_id.as_bytes())
            .map_err(|_| Error::InvalidPushRequest)?;
        let signing_artifact = SigningArtifactId::new(*claimed.artifact_id().as_bytes())
            .map_err(|_| Error::InvalidPushRequest)?;
        let sign_request = radroots_signing::SignRequest::new(
            OperationId::SyncPush,
            SigningIntentId::new(signing_operation, signing_artifact),
            request.actor,
            persisted_plan,
            SignPolicy::new(deadline, request.cancellation)
                .map_err(|_| Error::InvalidPushRequest)?,
        )
        .map_err(|_| Error::InvalidPushRequest)?;

        match signer.sign(sign_request).await {
            Ok(receipt) => {
                let command = AuthoredAtomicCommand::ApplySigned(
                    ApplySignedArtifact::new(
                        claimed.artifact_id(),
                        fence,
                        receipt.signed_event().clone(),
                        receipt.completed_at_unix_ms(),
                    )
                    .map_err(map_storage_error)?,
                );
                let applied = self
                    .storage
                    .execute_authored(command)
                    .await
                    .map_err(map_storage_error)?;
                let AuthoredAtomicOutcome::Artifact(artifact) = applied.outcome() else {
                    return Err(Error::StorageFailed);
                };
                Ok(SigningRunReceipt {
                    artifact: artifact.clone(),
                    replay: false,
                })
            }
            Err(error) => {
                let applied_at = self.clock.now_unix_ms()?;
                if error.kind() == radroots_signing::error::Kind::SignerCancelled {
                    let command = AuthoredAtomicCommand::Cancel(
                        CancelAuthoredWork::new(
                            CancelAuthoredTarget::ArtifactSigning(claimed.artifact_id()),
                            claimed.revision(),
                            applied_at,
                        )
                        .map_err(map_storage_error)?,
                    );
                    self.storage
                        .execute_authored(command)
                        .await
                        .map_err(map_storage_error)?;
                    return Err(Error::SigningCancelled);
                }
                let disposition = recovery_disposition(
                    replay_capability,
                    error.remote_effect(),
                    error.retryable(),
                );
                let class = match disposition {
                    RecoveryDisposition::RetryExactRequest | RecoveryDisposition::RetryLocal => {
                        FailureClass::Retryable
                    }
                    RecoveryDisposition::Indeterminate => FailureClass::Indeterminate,
                    RecoveryDisposition::Failed => FailureClass::Terminal,
                    _ => FailureClass::Indeterminate,
                };
                let retry_at = if class == FailureClass::Retryable {
                    Some(
                        applied_at
                            .checked_add(WORK_RETRY_DELAY_MS)
                            .ok_or(Error::DeadlineOverflow)?,
                    )
                } else {
                    None
                };
                let failure =
                    WorkFailure::new(error.code(), WorkPhase::Signing, class, retry_at, None)
                        .map_err(map_storage_error)?;
                let retry = retry_schedule(claimed.signing_retry(), &failure, retry_at)?;
                self.apply_artifact_failure(claimed.artifact_id(), fence, failure, retry)
                    .await?;
                match disposition {
                    RecoveryDisposition::Indeterminate => Err(Error::SigningIndeterminate),
                    RecoveryDisposition::RetryExactRequest
                    | RecoveryDisposition::RetryLocal
                    | RecoveryDisposition::Failed => {
                        if error.kind() == radroots_signing::error::Kind::DeadlineExceeded {
                            Err(Error::SignerDeadlineExceeded)
                        } else {
                            Err(Error::SignerFailed)
                        }
                    }
                    _ => Err(Error::SigningIndeterminate),
                }
            }
        }
    }

    /// Claims and executes local admission for one durably signed artifact.
    pub async fn admit_signed(&self, operation_id: SyncId) -> Result<AdmissionRunReceipt, Error> {
        let status = self
            .push_status(operation_id)
            .await?
            .ok_or(Error::StorageFailed)?;
        let artifact = status.artifact;
        if artifact.signing_state() != SigningState::Signed {
            return Err(Error::InvalidSignerOutput);
        }
        if artifact.admission_state().is_admitted() {
            return Ok(AdmissionRunReceipt {
                artifact,
                replay: true,
            });
        }
        if matches!(
            artifact.admission_state(),
            AdmissionState::Rejected | AdmissionState::Cancelled
        ) {
            return Err(Error::AdmissionFailed);
        }
        let now = self.clock.now_unix_ms()?.max(artifact.updated_at_unix_ms());
        if artifact
            .admission_claim()
            .is_some_and(|claim| now < claim.expires_at_unix_ms())
        {
            return Err(Error::WorkClaimConflict);
        }
        let (claimed, fence) = self
            .claim_artifact(
                artifact,
                ClaimAuthoredTarget::ArtifactAdmission,
                ADMISSION_CLAIM_OWNER,
                now,
            )
            .await?;
        let event = claimed
            .signed()
            .ok_or(Error::InvalidSignerOutput)?
            .event()
            .clone();
        let admission = match outbound_admission(&event, now) {
            Ok(admission) => admission,
            Err(error) => {
                let failure = WorkFailure::new(
                    "invalid_signed_artifact",
                    WorkPhase::Admission,
                    FailureClass::Terminal,
                    None,
                    None,
                )
                .map_err(map_storage_error)?;
                self.apply_artifact_failure(claimed.artifact_id(), fence, failure, None)
                    .await?;
                return Err(error);
            }
        };
        let admission_receipt = match EventStore::admit(self.storage.as_ref(), admission).await {
            Ok(receipt) => receipt,
            Err(error) => {
                let applied_at = self.clock.now_unix_ms()?;
                let terminal = matches!(
                    error,
                    radroots_storage::Error::EventConflict
                        | radroots_storage::Error::AdmissionRegression
                );
                let retry_at = if terminal {
                    None
                } else {
                    Some(
                        applied_at
                            .checked_add(WORK_RETRY_DELAY_MS)
                            .ok_or(Error::DeadlineOverflow)?,
                    )
                };
                let failure = WorkFailure::new(
                    if terminal {
                        "admission_conflict"
                    } else {
                        "admission_storage_unavailable"
                    },
                    WorkPhase::Admission,
                    if terminal {
                        FailureClass::Terminal
                    } else {
                        FailureClass::Retryable
                    },
                    retry_at,
                    None,
                )
                .map_err(map_storage_error)?;
                let retry = retry_schedule(claimed.admission_retry(), &failure, retry_at)?;
                self.apply_artifact_failure(claimed.artifact_id(), fence, failure, retry)
                    .await?;
                return Err(Error::AdmissionFailed);
            }
        };
        let state = match admission_receipt.disposition() {
            AdmissionDisposition::Duplicate => AdmissionState::Duplicate,
            AdmissionDisposition::Inserted | AdmissionDisposition::Advanced => {
                AdmissionState::Inserted
            }
        };
        let applied_at = self.clock.now_unix_ms()?.max(claimed.updated_at_unix_ms());
        let command = AuthoredAtomicCommand::ApplyAdmission(
            ApplyAdmissionResult::new(claimed.artifact_id(), fence, state, None, None, applied_at)
                .map_err(map_storage_error)?,
        );
        let applied = self
            .storage
            .execute_authored(command)
            .await
            .map_err(map_storage_error)?;
        let AuthoredAtomicOutcome::Artifact(artifact) = applied.outcome() else {
            return Err(Error::StorageFailed);
        };
        Ok(AdmissionRunReceipt {
            artifact: artifact.clone(),
            replay: false,
        })
    }

    /// Claims and executes one durable authored delivery plan.
    ///
    /// Terminal plans replay without invoking the sink. Retry timing and the
    /// request deadline are enforced from durable intent before any adapter
    /// call, and every adapter result is fenced into the plan as evidence.
    pub async fn deliver_push(
        &self,
        operation_id: SyncId,
    ) -> Result<DeliveryExecutionReceipt, Error> {
        let sink = self.sink.as_deref().ok_or(Error::MissingSink)?;
        let status = self
            .push_status(operation_id)
            .await?
            .ok_or(Error::StorageFailed)?;
        if status.artifact.signing_state() != SigningState::Signed {
            return Err(Error::InvalidSignerOutput);
        }
        if !status.artifact.admission_state().is_admitted() {
            return Err(Error::AdmissionFailed);
        }
        let plan = status.delivery_plan;
        if plan.state().is_terminal() {
            return Ok(DeliveryExecutionReceipt { plan, replay: true });
        }
        let request = plan.request().cloned().ok_or(Error::InvalidSignerOutput)?;
        let now = self.clock.now_unix_ms()?.max(plan.updated_at_unix_ms());
        if plan
            .claim_evidence()
            .is_some_and(|claim| now < claim.expires_at_unix_ms())
        {
            return Err(Error::WorkClaimConflict);
        }
        if plan
            .retry()
            .is_some_and(|retry| now < retry.not_before_unix_ms())
        {
            return Err(Error::DeliveryDeferred);
        }
        let (claimed, fence) = self.claim_delivery_plan(plan, now).await?;
        let execution_started_at = self.clock.now_unix_ms()?.max(claimed.updated_at_unix_ms());
        let result = if execution_started_at >= request.deadline_unix_ms() {
            DeliveryAttemptOutcome::SinkFailure(
                SinkFailure::for_request(
                    &request,
                    "delivery_deadline_exceeded",
                    Retryability::Terminal,
                    None,
                    None,
                    Vec::new(),
                )
                .map_err(|_| Error::InvalidDeliveryRequest)?,
            )
        } else {
            match sink.deliver(request.clone()).await {
                Ok(receipt) if receipt.validate_for_request(&request).is_ok() => {
                    DeliveryAttemptOutcome::Receipt(receipt)
                }
                Ok(_) => {
                    DeliveryAttemptOutcome::SinkFailure(SinkFailure::invalid_contract(&request))
                }
                Err(failure) if failure.validate_for_request(&request).is_ok() => {
                    DeliveryAttemptOutcome::SinkFailure(failure)
                }
                Err(_) => {
                    DeliveryAttemptOutcome::SinkFailure(SinkFailure::invalid_contract(&request))
                }
            }
        };
        let attempted_at = self.clock.now_unix_ms()?.max(execution_started_at);
        let outcome = match result {
            DeliveryAttemptOutcome::SinkFailure(failure) => DeliveryAttemptOutcome::SinkFailure(
                normalize_sink_failure(&request, failure, attempted_at)?,
            ),
            outcome => outcome,
        };
        let satisfaction = claimed
            .evaluate_next_attempt(&outcome)
            .map_err(map_storage_error)?;
        let retry = delivery_retry_schedule(&claimed, &outcome, satisfaction, attempted_at)?;
        let command = AuthoredAtomicCommand::ApplyDelivery(
            ApplyDeliveryAttempt::new(claimed.plan_id(), fence, outcome, retry, attempted_at)
                .map_err(map_storage_error)?,
        );
        let receipt = self
            .storage
            .execute_authored(command)
            .await
            .map_err(map_claim_error)?;
        let AuthoredAtomicOutcome::DeliveryPlan(plan) = receipt.outcome() else {
            return Err(Error::StorageFailed);
        };
        Ok(DeliveryExecutionReceipt {
            plan: plan.clone(),
            replay: false,
        })
    }

    async fn claim_delivery_plan(
        &self,
        plan: AuthoredDeliveryPlan,
        acquired_at: u64,
    ) -> Result<(AuthoredDeliveryPlan, WorkFence), Error> {
        let generation = plan
            .claim_evidence()
            .map_or(1, |claim| claim.generation().get().saturating_add(1));
        let generation = NonZeroU64::new(generation).ok_or(Error::StorageFailed)?;
        let expires_at = acquired_at
            .checked_add(self.deadlines.timeout_ms(OperationKind::Deliver))
            .ok_or(Error::DeadlineOverflow)?;
        let claim = WorkClaim::new(
            *self.ids.next_id(OperationKind::Deliver)?.as_bytes(),
            DELIVERY_CLAIM_OWNER,
            generation,
            acquired_at,
            expires_at,
            plan.revision(),
        )
        .map_err(map_storage_error)?;
        let fence = WorkFence::new(*claim.token(), claim.generation(), claim.row_revision())
            .map_err(map_storage_error)?;
        let command = AuthoredAtomicCommand::Claim(ClaimAuthoredWork::new(
            ClaimAuthoredTarget::DeliveryPlan(plan.plan_id()),
            claim,
        ));
        let receipt = self
            .storage
            .execute_authored(command)
            .await
            .map_err(map_claim_error)?;
        let AuthoredAtomicOutcome::DeliveryPlan(claimed) = receipt.outcome() else {
            return Err(Error::StorageFailed);
        };
        Ok((claimed.clone(), fence))
    }

    async fn claim_artifact(
        &self,
        artifact: AuthoredArtifact,
        target: fn(AuthoredArtifactId) -> ClaimAuthoredTarget,
        owner: &'static str,
        acquired_at: u64,
    ) -> Result<(AuthoredArtifact, WorkFence), Error> {
        let existing = match target(artifact.artifact_id()) {
            ClaimAuthoredTarget::ArtifactSigning(_) => artifact.signing_claim(),
            ClaimAuthoredTarget::ArtifactAdmission(_) => artifact.admission_claim(),
            ClaimAuthoredTarget::DeliveryPlan(_) => return Err(Error::StorageFailed),
        };
        let generation = existing.map_or(1, |claim| claim.generation().get().saturating_add(1));
        let generation = NonZeroU64::new(generation).ok_or(Error::StorageFailed)?;
        let expires_at = acquired_at
            .checked_add(self.deadlines.timeout_ms(OperationKind::Sign))
            .ok_or(Error::DeadlineOverflow)?;
        let claim = WorkClaim::new(
            *self.ids.next_id(OperationKind::Sign)?.as_bytes(),
            owner,
            generation,
            acquired_at,
            expires_at,
            artifact.revision(),
        )
        .map_err(map_storage_error)?;
        let fence = WorkFence::new(*claim.token(), claim.generation(), claim.row_revision())
            .map_err(map_storage_error)?;
        let command = AuthoredAtomicCommand::Claim(ClaimAuthoredWork::new(
            target(artifact.artifact_id()),
            claim,
        ));
        let receipt = self
            .storage
            .execute_authored(command)
            .await
            .map_err(map_claim_error)?;
        let AuthoredAtomicOutcome::Artifact(claimed) = receipt.outcome() else {
            return Err(Error::StorageFailed);
        };
        Ok((claimed.clone(), fence))
    }

    async fn apply_artifact_failure(
        &self,
        artifact_id: AuthoredArtifactId,
        fence: WorkFence,
        failure: WorkFailure,
        retry: Option<RetrySchedule>,
    ) -> Result<AuthoredArtifact, Error> {
        let applied_at = self.clock.now_unix_ms()?;
        let command = AuthoredAtomicCommand::ApplyFailure(
            ApplyWorkFailure::new(
                AuthoredWorkTarget::Artifact(artifact_id),
                fence,
                failure,
                retry,
                applied_at,
            )
            .map_err(map_storage_error)?,
        );
        let receipt = self
            .storage
            .execute_authored(command)
            .await
            .map_err(map_storage_error)?;
        let AuthoredAtomicOutcome::Artifact(artifact) = receipt.outcome() else {
            return Err(Error::StorageFailed);
        };
        Ok(artifact.clone())
    }

    /// Authorizes, signs, verifies, and durably enqueues one outbound event.
    ///
    /// Dropping the future before the final atomic enqueue leaves either a
    /// prepared or signed recoverable journal record. Once that commit returns,
    /// cancellation cannot claim rollback; replay returns the durable outbox.
    pub async fn sign_and_enqueue(&self, request: PushRequest) -> Result<PushReceipt, Error> {
        let instance_id = OperationInstanceId::new(*request.operation_id.as_bytes())
            .map_err(map_storage_error)?;
        let item_id =
            OutboxItemId::new(*request.operation_id.as_bytes()).map_err(map_storage_error)?;
        let input_digest = push_input_digest(&request)?;

        if let Some(existing) = Journal::operation(self.storage.as_ref(), instance_id)
            .await
            .map_err(map_storage_error)?
        {
            if [
                existing.operation_id() != OperationId::SyncPush,
                existing.idempotency_key() != request.idempotency_key(),
                existing.input_digest() != input_digest,
            ]
            .contains(&true)
            {
                return Err(Error::StorageConflict);
            }
            if existing.state().stage() == JournalStage::Committed {
                let outbox = Outbox::item(self.storage.as_ref(), item_id)
                    .await
                    .map_err(map_storage_error)?
                    .ok_or(Error::StorageFailed)?;
                return Ok(PushReceipt {
                    operation_id: request.operation_id,
                    outbox,
                    replay: true,
                });
            }
        }

        let signed = self.sign_prepared(request.clone()).await?;
        self.admit_signed(request.operation_id).await?;
        let event = signed
            .artifact()
            .signed()
            .ok_or(Error::InvalidSignerOutput)?
            .event()
            .clone();

        let prepared_at = self.clock.now_unix_ms()?;
        let prepare = PrepareOperation::new(
            instance_id,
            OperationId::SyncPush,
            request.idempotency_key.clone(),
            input_digest,
            prepared_at,
        )
        .map_err(map_storage_error)?;
        let prepare_commit = AtomicCommit::new(
            next_commit_id(self, OperationKind::Sign)?,
            AtomicCommitDigest::new(*input_digest.as_bytes()),
            prepared_at,
            AtomicWorkflow::Prepared(prepare),
        )
        .map_err(map_storage_error)?;
        let prepared_receipt = self
            .storage
            .commit(prepare_commit)
            .await
            .map_err(map_storage_error)?;
        let AtomicCommitOutcome::Prepared { journal: prepared } = prepared_receipt.outcome() else {
            return Err(Error::StorageFailed);
        };

        let signed_record = if prepared.state().stage() == JournalStage::Signed {
            if !matches!(
                prepared.state(),
                JournalState::Signed { event_id } if event_id == event.id()
            ) {
                return Err(Error::InvalidSignerOutput);
            }
            prepared.clone()
        } else {
            let signed_commit = AtomicCommit::new(
                next_commit_id(self, OperationKind::Sign)?,
                atomic_digest(b"radroots.sync.signed.v1", event.raw_json().as_bytes()),
                self.clock.now_unix_ms()?,
                AtomicWorkflow::Signed(Box::new(CommitSigned::new(
                    instance_id,
                    prepared.revision(),
                    event.clone(),
                ))),
            )
            .map_err(map_storage_error)?;
            let receipt = self
                .storage
                .commit(signed_commit)
                .await
                .map_err(map_storage_error)?;
            let AtomicCommitOutcome::Signed { journal, .. } = receipt.outcome() else {
                return Err(Error::StorageFailed);
            };
            journal.clone()
        };

        let admission = outbound_admission(&event, self.clock.now_unix_ms()?)?;
        let delivery = DeliveryRequest::new(
            delivery_request_id(request.operation_id),
            DeliveryPayload::new(event),
            request.targets,
            request.satisfaction,
            request.delivery_deadline_unix_ms,
        )
        .map_err(|_| Error::InvalidPushRequest)?;
        let plan_digest = delivery_plan_digest(&delivery);
        let committed_at = self.clock.now_unix_ms()?;
        let outbox =
            EnqueueOutboxItem::new(item_id, instance_id, plan_digest, delivery, committed_at)
                .map_err(map_storage_error)?;
        let enqueued = CommitEnqueued::new(
            instance_id,
            signed_record.revision(),
            admission,
            outbox,
            committed_at,
        )
        .map_err(map_storage_error)?;
        let receipt = self
            .storage
            .commit(
                AtomicCommit::new(
                    next_commit_id(self, OperationKind::Sign)?,
                    AtomicCommitDigest::new(*plan_digest.as_bytes()),
                    committed_at,
                    AtomicWorkflow::Enqueued(Box::new(enqueued)),
                )
                .map_err(map_storage_error)?,
            )
            .await
            .map_err(map_storage_error)?;
        let AtomicCommitOutcome::Enqueued { outbox, .. } = receipt.outcome() else {
            return Err(Error::StorageFailed);
        };
        Ok(PushReceipt {
            operation_id: request.operation_id,
            outbox: (**outbox).clone(),
            replay: false,
        })
    }

    /// Claims and delivers at most the caller-bounded number of outbox items.
    pub async fn deliver_pending(
        &self,
        request: DeliveryRunRequest,
    ) -> Result<DeliveryRunReceipt, Error> {
        if request.lease_duration_ms > self.deadlines.timeout_ms(OperationKind::Deliver) {
            return Err(Error::InvalidDeliveryRequest);
        }
        let sink = self.sink.as_deref().ok_or(Error::MissingSink)?;
        let now = self.clock.now_unix_ms()?;
        let expires = now
            .checked_add(request.lease_duration_ms)
            .ok_or(Error::DeadlineOverflow)?;
        let claimed = Outbox::claim(
            self.storage.as_ref(),
            ClaimOutboxItems::new(
                request.owner,
                LeaseId::new(*request.lease_seed.as_bytes()).map_err(map_storage_error)?,
                now,
                expires,
                request.limit,
            )
            .map_err(map_storage_error)?,
        )
        .await
        .map_err(map_storage_error)?;
        let mut outcomes = Vec::with_capacity(claimed.len());
        for item in claimed {
            let delivery_request = item.record().request().clone();
            let attempted_at = self.clock.now_unix_ms()?;
            let receipt = if attempted_at >= delivery_request.deadline_unix_ms() {
                synthetic_receipt(&delivery_request, false)?
            } else {
                match sink.deliver(delivery_request.clone()).await {
                    Ok(receipt) => receipt,
                    Err(_) => synthetic_receipt(&delivery_request, true)?,
                }
            };
            if receipt.validate_for_request(&delivery_request).is_err() {
                let released_at = self.clock.now_unix_ms()?;
                let released = Outbox::release(
                    self.storage.as_ref(),
                    item.record().item_id(),
                    item.lease().id(),
                    item.record().revision(),
                    released_at,
                    None,
                )
                .await
                .map_err(map_storage_error);
                outcomes.push(match released {
                    Ok(_) => Err(Error::InvalidDeliveryRequest),
                    Err(error) => Err(error),
                });
                continue;
            }
            let attempt = DeliveryAttempt::new(
                item.record()
                    .last_attempt()
                    .map_or(1, |attempt| attempt.get().saturating_add(1)),
            )
            .map_err(map_storage_error)?;
            let evidence = DeliveryAttemptEvidence::new(
                item.record().item_id(),
                item.lease().id(),
                item.record().revision(),
                attempt,
                receipt,
                attempted_at,
            )
            .map_err(map_storage_error)?;
            let digest = delivery_evidence_digest(&evidence);
            let commit = AtomicCommit::new(
                next_commit_id(self, OperationKind::Deliver)?,
                digest,
                attempted_at,
                AtomicWorkflow::Delivered(Box::new(evidence)),
            )
            .map_err(map_storage_error)?;
            let outcome = match self.storage.commit(commit).await {
                Ok(receipt) => match receipt.outcome() {
                    AtomicCommitOutcome::Delivered { outbox } => Ok((**outbox).clone()),
                    _ => Err(Error::StorageFailed),
                },
                Err(error) => Err(map_storage_error(error)),
            };
            outcomes.push(outcome);
        }
        Ok(DeliveryRunReceipt { outcomes })
    }
}

fn synthetic_receipt(request: &DeliveryRequest, retryable: bool) -> Result<DeliveryReceipt, Error> {
    let outcome = if retryable {
        DeliveryOutcome::unavailable()
    } else {
        DeliveryOutcome::failed(Retryability::Terminal)
            .map_err(|_| Error::InvalidDeliveryRequest)?
    };
    let targets = request
        .target_set()
        .targets()
        .iter()
        .cloned()
        .map(|target| DeliveryTargetReceipt::skipped(target, outcome.clone()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| Error::InvalidDeliveryRequest)?;
    DeliveryReceipt::for_request(request, targets).map_err(|_| Error::InvalidDeliveryRequest)
}

fn signer_replay_capability(
    status: &radroots_signing::SignerStatus,
) -> Result<ReplayCapability, Error> {
    let mut capabilities = status.capabilities().iter();
    let first = capabilities
        .next()
        .ok_or(Error::SignerCapabilityUnavailable)?
        .replay();
    if capabilities.all(|capability| capability.replay() == first) {
        Ok(first)
    } else {
        Ok(ReplayCapability::NonReplayable)
    }
}

fn signing_claim_owner(replay: ReplayCapability) -> &'static str {
    match replay {
        ReplayCapability::ExactReplayByRequestId => SIGNING_CLAIM_OWNER_EXACT,
        ReplayCapability::LocalReplaySafe => SIGNING_CLAIM_OWNER_LOCAL,
        ReplayCapability::NonReplayable => SIGNING_CLAIM_OWNER_NON_REPLAYABLE,
        _ => SIGNING_CLAIM_OWNER_NON_REPLAYABLE,
    }
}

fn retry_schedule(
    previous: Option<&RetrySchedule>,
    failure: &WorkFailure,
    retry_at: Option<u64>,
) -> Result<Option<RetrySchedule>, Error> {
    let Some(retry_at) = retry_at else {
        return Ok(None);
    };
    let schedule = match previous {
        Some(previous) => previous.next_attempt(retry_at, failure.clone()),
        None => RetrySchedule::new(NonZeroU32::MIN, retry_at, failure.clone()),
    }
    .map_err(map_storage_error)?;
    Ok(Some(schedule))
}

fn normalize_sink_failure(
    request: &DeliveryRequest,
    failure: SinkFailure,
    attempted_at_unix_ms: u64,
) -> Result<SinkFailure, Error> {
    if failure.retryability() != Retryability::Retryable {
        return Ok(failure);
    }
    let default_retry_at = attempted_at_unix_ms
        .checked_add(WORK_RETRY_DELAY_MS)
        .ok_or(Error::DeadlineOverflow)?;
    let retry_at = failure
        .retry_after_unix_ms()
        .unwrap_or(default_retry_at)
        .max(
            attempted_at_unix_ms
                .checked_add(1)
                .ok_or(Error::DeadlineOverflow)?,
        );
    if failure.retry_after_unix_ms() == Some(retry_at) {
        return Ok(failure);
    }
    SinkFailure::for_request(
        request,
        failure.code(),
        failure.retryability(),
        Some(retry_at),
        failure.message().map(str::to_owned),
        failure.partial_evidence().to_vec(),
    )
    .map_err(|_| Error::InvalidDeliveryRequest)
}

fn delivery_retry_schedule(
    plan: &AuthoredDeliveryPlan,
    outcome: &DeliveryAttemptOutcome,
    satisfaction: SatisfactionState,
    attempted_at_unix_ms: u64,
) -> Result<Option<RetrySchedule>, Error> {
    let attempt = plan
        .attempt_count()
        .checked_add(1)
        .and_then(NonZeroU32::new)
        .ok_or(Error::StorageFailed)?;
    if satisfaction != SatisfactionState::Pending || attempt.get() >= DELIVERY_PLAN_ATTEMPTS_MAX {
        return Ok(None);
    }
    let failure = match outcome {
        DeliveryAttemptOutcome::Receipt(_) => {
            let retry_at = attempted_at_unix_ms
                .checked_add(WORK_RETRY_DELAY_MS)
                .ok_or(Error::DeadlineOverflow)?;
            WorkFailure::new(
                "delivery_pending",
                WorkPhase::Delivery,
                FailureClass::Retryable,
                Some(retry_at),
                None,
            )
            .map_err(map_storage_error)?
        }
        DeliveryAttemptOutcome::SinkFailure(failure)
            if failure.retryability() == Retryability::Retryable =>
        {
            WorkFailure::new(
                failure.code(),
                WorkPhase::Delivery,
                FailureClass::Retryable,
                failure.retry_after_unix_ms(),
                failure.message().map(str::to_owned),
            )
            .map_err(map_storage_error)?
        }
        DeliveryAttemptOutcome::SinkFailure(_) => return Ok(None),
    };
    let retry_at = failure.retry_after_unix_ms().unwrap_or(
        attempted_at_unix_ms
            .checked_add(WORK_RETRY_DELAY_MS)
            .ok_or(Error::DeadlineOverflow)?,
    );
    RetrySchedule::new(attempt, retry_at, failure)
        .map(Some)
        .map_err(map_storage_error)
}

fn delivery_evidence_digest(evidence: &DeliveryAttemptEvidence) -> AtomicCommitDigest {
    let mut hasher = Sha256::new();
    hash_field(&mut hasher, b"radroots.sync.delivery-evidence.v1");
    hash_field(&mut hasher, evidence.item_id().as_bytes());
    hash_field(&mut hasher, evidence.lease_id().as_bytes());
    hasher.update(evidence.expected_revision().get().to_be_bytes());
    hasher.update(evidence.attempt().get().to_be_bytes());
    hasher.update(evidence.recorded_at_unix_ms().to_be_bytes());
    hash_field(
        &mut hasher,
        evidence.receipt().request_id().as_str().as_bytes(),
    );
    for target in evidence.receipt().target_receipts() {
        hash_field(
            &mut hasher,
            target.target().fingerprint().as_str().as_bytes(),
        );
        hasher.update([u8::from(target.was_attempted())]);
        hasher.update([match target.outcome().kind() {
            DeliveryOutcomeKind::Accepted => 0,
            DeliveryOutcomeKind::Delivered => 1,
            DeliveryOutcomeKind::Rejected => 2,
            DeliveryOutcomeKind::Unavailable => 3,
            DeliveryOutcomeKind::Failed => 4,
        }]);
        hasher.update([match target.outcome().retryability() {
            Retryability::NotApplicable => 0,
            Retryability::Retryable => 1,
            Retryability::Terminal => 2,
        }]);
        hash_field(
            &mut hasher,
            target.outcome().code().unwrap_or_default().as_bytes(),
        );
        hash_field(
            &mut hasher,
            target.outcome().message().unwrap_or_default().as_bytes(),
        );
    }
    AtomicCommitDigest::new(hasher.finalize().into())
}

fn outbound_admission(
    event: &radroots_event::SignedEvent,
    observed_at_unix_ms: u64,
) -> Result<EventAdmission, Error> {
    let verified = verify::signature(
        verify::id(RawEvent::new(event.envelope().clone()))
            .map_err(|_| Error::InvalidSignerOutput)?,
        &Nip01SignatureVerifier,
    )
    .map_err(|_| Error::InvalidSignerOutput)?;
    let validated = verify::contract(verified).map_err(|_| Error::InvalidSignerOutput)?;
    let policy = RegistryPolicy::visible();
    if policy.decide(&validated) != AdmissionDecision::Visible {
        return Err(Error::InvalidSignerOutput);
    }
    struct Evidence;
    impl radroots_event::admission::AdmissionPolicy for Evidence {
        type Error = core::convert::Infallible;
        fn policy_id(&self) -> &'static str {
            "radroots.registry_v7"
        }
        fn admit(
            &self,
            _: &radroots_event::admission::ContractValidatedEvent,
        ) -> Result<(), Self::Error> {
            Ok(())
        }
    }
    impl radroots_event::admission::VisibilityPolicy for Evidence {
        type Error = core::convert::Infallible;
        fn policy_id(&self) -> &'static str {
            "radroots.registry_v7"
        }
        fn make_visible(
            &self,
            _: &radroots_event::admission::AdmittedEvent,
        ) -> Result<(), Self::Error> {
            Ok(())
        }
    }
    let visible = validated
        .admit_with(&Evidence)
        .and_then(|event| event.make_visible_with(&Evidence))
        .map_err(|never| match never {})?;
    let local = Target::new(TransportId::LOCAL, "local:sync-authored")
        .map_err(|_| Error::InvalidPushRequest)?;
    let provenance = EventProvenance::new(
        TransportId::LOCAL,
        local.fingerprint().clone(),
        observed_at_unix_ms,
    )
    .map_err(|_| Error::InvalidPushRequest)?;
    EventAdmission::visible(ObservedEvent::new(event.clone(), provenance), visible)
        .map_err(map_storage_error)
}

fn push_input_digest(request: &PushRequest) -> Result<IdempotencyDigest, Error> {
    Ok(IdempotencyDigest::new(
        *authored_push_input_digest(request)?.as_bytes(),
    ))
}

fn authored_push_input_digest(request: &PushRequest) -> Result<AtomicCommitDigest, Error> {
    let mut hasher = Sha256::new();
    hash_field(&mut hasher, b"radroots.sync.authored-push.v2");
    hash_field(&mut hasher, request.operation_id.as_bytes());
    hash_field(&mut hasher, request.idempotency_key.as_str().as_bytes());
    hash_field(&mut hasher, request.plan.digest().as_bytes());
    hash_field(&mut hasher, request.actor.public_key().as_bytes());
    let source = match request.actor.source() {
        radroots_signing::actor::ActorSource::LocalAccount(_) => 0,
        radroots_signing::actor::ActorSource::ExplicitPublicKey => 1,
        radroots_signing::actor::ActorSource::RemoteSigner(_) => 2,
        radroots_signing::actor::ActorSource::Service(_) => 3,
        _ => return Err(Error::InvalidPushRequest),
    };
    hasher.update([source]);
    if let Some(account_id) = request.actor.account_id() {
        hash_field(&mut hasher, account_id.as_bytes());
    }
    for role in request.actor.roles() {
        hash_field(&mut hasher, role.as_str().as_bytes());
    }
    for target in request.targets.targets() {
        hash_field(&mut hasher, target.fingerprint().as_str().as_bytes());
    }
    hash_satisfaction(&mut hasher, &request.satisfaction);
    hasher.update(request.delivery_deadline_unix_ms.to_be_bytes());
    let cancellation = match request.cancellation {
        CancellationPolicy::PreservePublishedRequest => 0,
        CancellationPolicy::LocalCooperative => 1,
        _ => return Err(Error::InvalidPushRequest),
    };
    hasher.update([cancellation]);
    Ok(AtomicCommitDigest::new(hasher.finalize().into()))
}

fn delivery_plan_digest(request: &DeliveryRequest) -> DeliveryPlanDigest {
    let mut hasher = Sha256::new();
    hash_field(&mut hasher, b"radroots.sync.delivery.v1");
    hash_field(&mut hasher, request.payload().event().raw_json().as_bytes());
    for target in request.target_set().targets() {
        hash_field(&mut hasher, target.fingerprint().as_str().as_bytes());
    }
    hash_satisfaction(&mut hasher, request.satisfaction());
    DeliveryPlanDigest::new(hasher.finalize().into())
}

fn hash_satisfaction(hasher: &mut Sha256, policy: &SatisfactionPolicy) {
    hasher.update([match policy.class() {
        radroots_transport::policy::SatisfactionClass::Accepted => 0,
        radroots_transport::policy::SatisfactionClass::Delivered => 1,
    }]);
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

fn atomic_digest(domain: &[u8], input: &[u8]) -> AtomicCommitDigest {
    let mut hasher = Sha256::new();
    hash_field(&mut hasher, domain);
    hash_field(&mut hasher, input);
    AtomicCommitDigest::new(hasher.finalize().into())
}

fn authored_ids(
    operation_id: SyncId,
) -> Result<
    (
        OperationInstanceId,
        AuthoredArtifactId,
        AuthoredDeliveryPlanId,
    ),
    Error,
> {
    let operation =
        OperationInstanceId::new(*operation_id.as_bytes()).map_err(map_storage_error)?;
    let artifact = AuthoredArtifactId::new(derive_child_id(
        b"radroots.sync.authored-artifact.v2",
        operation_id.as_bytes(),
    ))
    .map_err(map_storage_error)?;
    let delivery = AuthoredDeliveryPlanId::new(derive_child_id(
        b"radroots.sync.authored-delivery.v2",
        artifact.as_bytes(),
    ))
    .map_err(map_storage_error)?;
    Ok((operation, artifact, delivery))
}

fn derive_child_id(domain: &[u8], parent: &[u8; 16]) -> [u8; 16] {
    let mut hasher = Sha256::new();
    hash_field(&mut hasher, domain);
    hash_field(&mut hasher, parent);
    let digest: [u8; 32] = hasher.finalize().into();
    let mut child = [0_u8; 16];
    child.copy_from_slice(&digest[..16]);
    child
}

fn hash_field(hasher: &mut Sha256, value: &[u8]) {
    hasher.update(u64::try_from(value.len()).unwrap_or(u64::MAX).to_be_bytes());
    hasher.update(value);
}

fn next_commit_id(engine: &Engine, operation: OperationKind) -> Result<AtomicCommitId, Error> {
    AtomicCommitId::new(*engine.ids.next_id(operation)?.as_bytes()).map_err(map_storage_error)
}

fn delivery_request_id(id: SyncId) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut value = String::from("push-");
    for byte in id.as_bytes() {
        value.push(HEX[(byte >> 4) as usize] as char);
        value.push(HEX[(byte & 0x0f) as usize] as char);
    }
    value
}

fn map_storage_error(error: radroots_storage::Error) -> Error {
    match error {
        radroots_storage::Error::IdempotencyConflict
        | radroots_storage::Error::OperationIdentityMismatch
        | radroots_storage::Error::JournalRevisionConflict
        | radroots_storage::Error::OutboxPlanConflict
        | radroots_storage::Error::AtomicCommitConflict => Error::StorageConflict,
        _ => Error::StorageFailed,
    }
}

fn map_claim_error(error: radroots_storage::Error) -> Error {
    match error {
        radroots_storage::Error::AtomicCommitConflict
        | radroots_storage::Error::InvalidAuthoredTransition
        | radroots_storage::Error::InvalidWorkClaim
        | radroots_storage::Error::DeliveryPlanClaimConflict => Error::WorkClaimConflict,
        other => map_storage_error(other),
    }
}
