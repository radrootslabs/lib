//! Signing, durable enqueue, delivery, and satisfaction orchestration.

use radroots_event::admission::RawEvent;
use radroots_event_codec::{
    authoring::AuthoredEventPlan,
    verify::{self, Nip01SignatureVerifier},
};
use radroots_protocol::runtime::v1::OperationId;
use radroots_signing::{
    Actor, AuthoredArtifactId as SigningArtifactId, SigningIntentId, SigningOperationId,
    request::{CancellationPolicy, SignPolicy},
};
use radroots_storage::{
    Journal, Outbox,
    atomic::{
        AtomicCommit, AtomicCommitDigest, AtomicCommitDisposition, AtomicCommitId,
        AtomicCommitOutcome, AtomicWorkflow, CommitEnqueued, CommitSigned,
    },
    authored::{AuthoredArtifact, AuthoredArtifactId, AuthoredOperation},
    authored_atomic::{AuthoredAtomicCommand, AuthoredAtomicOutcome, PrepareAuthoredOperation},
    authored_delivery::{AuthoredDeliveryIntent, AuthoredDeliveryPlan, AuthoredDeliveryPlanId},
    event::EventAdmission,
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
    DeliveryReceipt, DeliveryRequest, Target, TransportId,
    outcome::{DeliveryOutcome, DeliveryOutcomeKind, Retryability},
    policy::SatisfactionPolicy,
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
        Ok(Some(PushStatus {
            operation,
            artifact,
            delivery_plan,
        }))
    }

    /// Authorizes, signs, verifies, and durably enqueues one outbound event.
    ///
    /// Dropping the future before the final atomic enqueue leaves either a
    /// prepared or signed recoverable journal record. Once that commit returns,
    /// cancellation cannot claim rollback; replay returns the durable outbox.
    pub async fn sign_and_enqueue(&self, request: PushRequest) -> Result<PushReceipt, Error> {
        let signer = self.signer.as_deref().ok_or(Error::MissingSigner)?;
        let preparation = self.prepare_push(request.clone()).await?;
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

        let sign_deadline_ms = self
            .deadlines
            .deadline_unix_ms(OperationKind::Sign, self.clock.now_unix_ms()?)?;
        let signing_operation = SigningOperationId::new(*request.operation_id.as_bytes())
            .map_err(|_| Error::InvalidPushRequest)?;
        let signing_artifact =
            SigningArtifactId::new(*preparation.artifact().artifact_id().as_bytes())
                .map_err(|_| Error::InvalidPushRequest)?;
        let sign_request = radroots_signing::SignRequest::new(
            OperationId::SyncPush,
            SigningIntentId::new(signing_operation, signing_artifact),
            request.actor.clone(),
            request.plan.clone(),
            SignPolicy::new(sign_deadline_ms, request.cancellation)
                .map_err(|_| Error::InvalidPushRequest)?,
        )
        .map_err(|_| Error::InvalidPushRequest)?;
        let signed = signer.sign(sign_request).await.map_err(|error| {
            if error.kind() == radroots_signing::error::Kind::DeadlineExceeded {
                Error::SignerDeadlineExceeded
            } else {
                Error::SignerFailed
            }
        })?;
        if signed.completed_at_unix_ms() > sign_deadline_ms {
            return Err(Error::SignerDeadlineExceeded);
        }
        let event = signed.signed_event().clone();

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
