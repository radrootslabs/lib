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
    atomic::{AtomicCommitDigest, AtomicCommitDisposition},
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
    journal::{IdempotencyKey, OperationInstanceId},
};
use radroots_transport::{
    DeliveryRequest, SinkFailure, Target, TransportId,
    outcome::Retryability,
    policy::{SatisfactionPolicy, SatisfactionState},
    source::{EventProvenance, ObservedEvent},
    target::TargetSet,
};
use sha2::{Digest, Sha256};

use crate::{
    Engine,
    ingest::{AdmissionDecision, AdmissionPolicy, RegistryPolicy},
    policy::{Error, OperationKind, SyncId},
};

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
                self.apply_artifact_failure(claimed.artifact_id(), fence, failure, None, now)
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
                self.apply_artifact_failure(
                    claimed.artifact_id(),
                    fence,
                    failure,
                    retry,
                    applied_at,
                )
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
                self.apply_artifact_failure(claimed.artifact_id(), fence, failure, None, now)
                    .await?;
                return Err(error);
            }
        };
        let admission_receipt = match EventStore::admit(self.storage.as_ref(), admission).await {
            Ok(receipt) => receipt,
            Err(error) => {
                let applied_at = self.clock.now_unix_ms()?.max(claimed.updated_at_unix_ms());
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
                self.apply_artifact_failure(
                    claimed.artifact_id(),
                    fence,
                    failure,
                    retry,
                    applied_at,
                )
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
        applied_at: u64,
    ) -> Result<AuthoredArtifact, Error> {
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

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use radroots_event::{GenericEventDraft, contract::AuthorRole};
    use radroots_signing::{
        SignerStatus,
        actor::ActorSource,
        capability::{CancellationSupport, SignerCapability, SignerKind},
        status::SignerAvailability,
    };
    use radroots_transport::{
        DeliveryReceipt,
        outcome::DeliveryOutcome,
        policy::{SatisfactionClass, TargetPolicy},
        sink::{DeliveryPayload, DeliveryTargetReceipt},
    };

    const AUTHOR: &str = "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df";
    const OTHER_AUTHOR: &str = "e0266e3cfb0d2886f91c73f5f868f3b98273713e5fcd97c081663f5518a4b3af";

    fn request_with(
        target_policy: TargetPolicy,
        deadline: u64,
        cancellation: CancellationPolicy,
    ) -> PushRequest {
        let plan = AuthoredEventPlan::from_generic(
            GenericEventDraft::new(
                "radroots.social.geochat.v1",
                20_000,
                1_800_000_100,
                Vec::new(),
                "push helper coverage",
                AUTHOR,
            )
            .unwrap(),
        )
        .unwrap();
        let actor =
            Actor::from_public_key_hex(AUTHOR, ActorSource::ExplicitPublicKey, [AuthorRole::Any])
                .unwrap();
        PushRequest::new(
            SyncId::new([31; 16]).unwrap(),
            IdempotencyKey::parse("push-helper-coverage").unwrap(),
            actor,
            plan,
            TargetSet::new(vec![Target::nostr_relay("wss://helper.example").unwrap()]).unwrap(),
            SatisfactionPolicy::new(SatisfactionClass::Accepted, target_policy),
            deadline,
            cancellation,
        )
        .unwrap()
    }

    fn delivery_plan(request: &PushRequest) -> AuthoredDeliveryPlan {
        let (_, artifact_id, plan_id) = authored_ids(request.operation_id()).unwrap();
        let delivery_request = DeliveryRequest::new(
            delivery_request_id(request.operation_id()),
            DeliveryPayload::new(
                radroots_event_codec::Codec::decode_signed_event(
                    r#"{"id":"762bee187e9e645b81ec26ade05a69b5e8398caf527be8de0d9a45311ed0c7a0","pubkey":"585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df","created_at":1800000100,"kind":0,"tags":[],"content":"{\"display_name\":\"Moss Street Farm\",\"bot\":false,\"website\":\"https://mossstreet.example\",\"picture\":42}","sig":"4290da0bb6422986647bc8cd5f63bd52d49f41e7b665d3b47105b8109183e8d596f322c531d4061df53e1d2b70fda12d5d1c14f3720d7a56d9d0a03746af5109"}"#,
                )
                .unwrap(),
            ),
            request.targets().clone(),
            request.satisfaction().clone(),
            request.delivery_deadline_unix_ms(),
        )
        .unwrap();
        AuthoredDeliveryPlan::new_bound(plan_id, artifact_id, delivery_request, 10).unwrap()
    }

    fn capability(replay: ReplayCapability) -> SignerCapability {
        SignerCapability::new(
            SignerKind::Local,
            replay,
            CancellationSupport::BeforePublication,
            false,
            false,
        )
    }

    #[test]
    fn push_request_validates_every_binding_and_exposes_redacted_accessors() {
        let request = request_with(
            TargetPolicy::any(),
            1_800_000_300_000,
            CancellationPolicy::LocalCooperative,
        );
        assert_eq!(request.idempotency_key().as_str(), "push-helper-coverage");
        assert_eq!(request.actor().public_key(), *request.plan().author());
        assert_eq!(request.targets().len(), 1);
        assert!(request.satisfaction().targets().is_any());
        assert_eq!(request.delivery_deadline_unix_ms(), 1_800_000_300_000);
        assert_eq!(request.cancellation(), CancellationPolicy::LocalCooperative);
        let debug = format!("{request:?}");
        assert!(debug.contains("[redacted exact authored plan]"));
        assert!(!debug.contains("push helper coverage"));

        let invalid_policy = TargetPolicy::required(vec![
            Target::nostr_relay("wss://absent.example")
                .unwrap()
                .fingerprint()
                .clone(),
        ])
        .unwrap();
        let mut invalid = request.clone();
        invalid.satisfaction = SatisfactionPolicy::new(SatisfactionClass::Accepted, invalid_policy);
        assert!(matches!(
            PushRequest::new(
                invalid.operation_id,
                invalid.idempotency_key,
                invalid.actor,
                invalid.plan,
                invalid.targets,
                invalid.satisfaction,
                invalid.delivery_deadline_unix_ms,
                invalid.cancellation,
            ),
            Err(Error::InvalidPushRequest)
        ));

        let valid = request_with(
            TargetPolicy::any(),
            1_800_000_300_000,
            CancellationPolicy::PreservePublishedRequest,
        );
        assert!(matches!(
            PushRequest::new(
                valid.operation_id,
                valid.idempotency_key.clone(),
                valid.actor.clone(),
                valid.plan.clone(),
                valid.targets.clone(),
                valid.satisfaction.clone(),
                0,
                valid.cancellation,
            ),
            Err(Error::InvalidPushRequest)
        ));
        let wrong_actor = Actor::from_public_key_hex(
            OTHER_AUTHOR,
            ActorSource::ExplicitPublicKey,
            [AuthorRole::Any],
        )
        .unwrap();
        assert!(matches!(
            PushRequest::new(
                valid.operation_id,
                valid.idempotency_key,
                wrong_actor,
                valid.plan,
                valid.targets,
                valid.satisfaction,
                valid.delivery_deadline_unix_ms,
                valid.cancellation,
            ),
            Err(Error::InvalidPushRequest)
        ));
    }

    #[test]
    fn signer_and_retry_helpers_cover_every_stable_policy_variant() {
        let empty = SignerStatus::new(SignerAvailability::Ready, Vec::new(), None);
        assert_eq!(
            signer_replay_capability(&empty),
            Err(Error::SignerCapabilityUnavailable)
        );
        let exact = SignerStatus::new(
            SignerAvailability::Ready,
            vec![capability(ReplayCapability::ExactReplayByRequestId)],
            None,
        );
        assert_eq!(
            signer_replay_capability(&exact).unwrap(),
            ReplayCapability::ExactReplayByRequestId
        );
        let mixed = SignerStatus::new(
            SignerAvailability::Ready,
            vec![
                capability(ReplayCapability::ExactReplayByRequestId),
                capability(ReplayCapability::LocalReplaySafe),
            ],
            None,
        );
        assert_eq!(
            signer_replay_capability(&mixed).unwrap(),
            ReplayCapability::NonReplayable
        );
        assert_eq!(
            signing_claim_owner(ReplayCapability::ExactReplayByRequestId),
            SIGNING_CLAIM_OWNER_EXACT
        );
        assert_eq!(
            signing_claim_owner(ReplayCapability::LocalReplaySafe),
            SIGNING_CLAIM_OWNER_LOCAL
        );
        assert_eq!(
            signing_claim_owner(ReplayCapability::NonReplayable),
            SIGNING_CLAIM_OWNER_NON_REPLAYABLE
        );

        let failure = WorkFailure::new(
            "retry",
            WorkPhase::Signing,
            FailureClass::Retryable,
            Some(20),
            None,
        )
        .unwrap();
        assert!(retry_schedule(None, &failure, None).unwrap().is_none());
        let first = retry_schedule(None, &failure, Some(20)).unwrap().unwrap();
        assert_eq!(first.attempt(), NonZeroU32::MIN);
        let next_failure = WorkFailure::new(
            "retry",
            WorkPhase::Signing,
            FailureClass::Retryable,
            Some(21),
            None,
        )
        .unwrap();
        assert_eq!(
            retry_schedule(Some(&first), &next_failure, Some(21))
                .unwrap()
                .unwrap()
                .attempt()
                .get(),
            2
        );
    }

    #[test]
    fn delivery_failure_and_retry_helpers_normalize_all_outcome_classes() {
        let request = request_with(
            TargetPolicy::any(),
            1_800_000_300_000,
            CancellationPolicy::PreservePublishedRequest,
        );
        let plan = delivery_plan(&request);
        let delivery_request = plan.request().unwrap();
        let terminal = SinkFailure::for_request(
            delivery_request,
            "terminal",
            Retryability::Terminal,
            None,
            None,
            Vec::new(),
        )
        .unwrap();
        assert_eq!(
            normalize_sink_failure(delivery_request, terminal.clone(), 100).unwrap(),
            terminal
        );
        let retryable = SinkFailure::for_request(
            delivery_request,
            "retryable",
            Retryability::Retryable,
            None,
            Some("retry".to_owned()),
            Vec::new(),
        )
        .unwrap();
        let normalized = normalize_sink_failure(delivery_request, retryable, 100).unwrap();
        assert_eq!(normalized.retry_after_unix_ms(), Some(1_100));
        assert_eq!(
            normalize_sink_failure(delivery_request, normalized.clone(), 100).unwrap(),
            normalized
        );
        let past = SinkFailure::for_request(
            delivery_request,
            "past",
            Retryability::Retryable,
            Some(99),
            None,
            Vec::new(),
        )
        .unwrap();
        assert_eq!(
            normalize_sink_failure(delivery_request, past, 100)
                .unwrap()
                .retry_after_unix_ms(),
            Some(101)
        );

        let receipt = DeliveryReceipt::for_request(
            delivery_request,
            delivery_request
                .target_set()
                .targets()
                .iter()
                .cloned()
                .map(|target| {
                    DeliveryTargetReceipt::attempted(target, DeliveryOutcome::unavailable())
                })
                .collect(),
        )
        .unwrap();
        let receipt_outcome = DeliveryAttemptOutcome::Receipt(receipt);
        assert!(
            delivery_retry_schedule(&plan, &receipt_outcome, SatisfactionState::Satisfied, 100)
                .unwrap()
                .is_none()
        );
        let schedule =
            delivery_retry_schedule(&plan, &receipt_outcome, SatisfactionState::Pending, 100)
                .unwrap()
                .unwrap();
        assert_eq!(schedule.not_before_unix_ms(), 1_100);
        let retry_outcome = DeliveryAttemptOutcome::SinkFailure(normalized);
        assert!(
            delivery_retry_schedule(&plan, &retry_outcome, SatisfactionState::Pending, 100)
                .unwrap()
                .is_some()
        );
        let terminal_outcome = DeliveryAttemptOutcome::SinkFailure(terminal);
        assert!(
            delivery_retry_schedule(&plan, &terminal_outcome, SatisfactionState::Pending, 100)
                .unwrap()
                .is_none()
        );

        for policy in [
            TargetPolicy::any(),
            TargetPolicy::all(),
            TargetPolicy::quorum(1).unwrap(),
            TargetPolicy::required(vec![request.targets().targets()[0].fingerprint().clone()])
                .unwrap(),
        ] {
            let request = request_with(
                policy,
                1_800_000_300_000,
                CancellationPolicy::LocalCooperative,
            );
            assert_ne!(
                authored_push_input_digest(&request).unwrap().as_bytes(),
                &[0; 32]
            );
        }
    }

    #[test]
    fn storage_error_maps_are_explicit_and_fail_closed() {
        assert_eq!(
            map_storage_error(radroots_storage::Error::AtomicCommitConflict),
            Error::StorageConflict
        );
        assert_eq!(
            map_storage_error(radroots_storage::Error::BackendUnavailable),
            Error::StorageFailed
        );
        assert_eq!(
            map_claim_error(radroots_storage::Error::DeliveryPlanClaimConflict),
            Error::WorkClaimConflict
        );
        assert_eq!(
            map_claim_error(radroots_storage::Error::BackendUnavailable),
            Error::StorageFailed
        );
        assert_eq!(
            delivery_request_id(SyncId::new([0xab; 16]).unwrap()).len(),
            37
        );
    }
}
