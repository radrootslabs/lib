//! Signing, durable enqueue, delivery, and satisfaction orchestration.

use radroots_event::{EventDraft, admission::RawEvent};
use radroots_event_codec::verify::{self, Nip01SignatureVerifier};
use radroots_protocol::runtime::v1::OperationId;
use radroots_signing::{
    Actor,
    request::{CancellationPolicy, SignPolicy},
};
use radroots_storage::{
    Journal, Outbox,
    atomic::{
        AtomicCommit, AtomicCommitDigest, AtomicCommitId, AtomicCommitOutcome, AtomicWorkflow,
        CommitEnqueued, CommitSigned,
    },
    event::EventAdmission,
    journal::{
        IdempotencyDigest, IdempotencyKey, JournalStage, JournalState, OperationInstanceId,
        PrepareOperation,
    },
    outbox::{DeliveryPlanDigest, EnqueueOutboxItem, OutboxItemId, OutboxRecord},
};
use radroots_transport::{
    DeliveryRequest, Target, TransportId,
    policy::SatisfactionPolicy,
    sink::DeliveryPayload,
    source::{EventProvenance, ObservedEvent},
    target::TargetSet,
};
use sha2::{Digest, Sha256};

use crate::{
    Engine,
    ingest::{AdmissionDecision, AdmissionPolicy, RegistryPolicy},
    policy::{Error, OperationKind, SyncId},
};

/// Caller-owned, replay-stable inputs for one outbound operation.
#[derive(Clone)]
pub struct PushRequest {
    operation_id: SyncId,
    idempotency_key: IdempotencyKey,
    actor: Actor,
    draft: EventDraft,
    targets: TargetSet,
    satisfaction: SatisfactionPolicy,
    cancellation: CancellationPolicy,
}

impl PushRequest {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        operation_id: SyncId,
        idempotency_key: IdempotencyKey,
        actor: Actor,
        draft: EventDraft,
        targets: TargetSet,
        satisfaction: SatisfactionPolicy,
        cancellation: CancellationPolicy,
    ) -> Result<Self, Error> {
        if !valid_satisfaction(&satisfaction, &targets) {
            return Err(Error::InvalidPushRequest);
        }
        Ok(Self {
            operation_id,
            idempotency_key,
            actor,
            draft,
            targets,
            satisfaction,
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
    pub const fn draft(&self) -> &EventDraft {
        &self.draft
    }
    pub const fn targets(&self) -> &TargetSet {
        &self.targets
    }
    pub const fn satisfaction(&self) -> &SatisfactionPolicy {
        &self.satisfaction
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
            .field("draft", &"[redacted frozen event draft]")
            .field("targets", &self.targets)
            .field("satisfaction", &self.satisfaction)
            .field("cancellation", &self.cancellation)
            .finish()
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

impl Engine {
    /// Authorizes, signs, verifies, and durably enqueues one outbound event.
    ///
    /// Dropping the future before the final atomic enqueue leaves either a
    /// prepared or signed recoverable journal record. Once that commit returns,
    /// cancellation cannot claim rollback; replay returns the durable outbox.
    pub async fn sign_and_enqueue(&self, request: PushRequest) -> Result<PushReceipt, Error> {
        let signer = self.signer.as_deref().ok_or(Error::MissingSigner)?;
        let instance_id = OperationInstanceId::new(*request.operation_id.as_bytes())
            .map_err(map_storage_error)?;
        let item_id =
            OutboxItemId::new(*request.operation_id.as_bytes()).map_err(map_storage_error)?;
        let input_digest = push_input_digest(&request);

        if let Some(existing) = Journal::operation(self.storage.as_ref(), instance_id)
            .await
            .map_err(map_storage_error)?
        {
            if existing.operation_id() != OperationId::SyncPush
                || existing.idempotency_key() != request.idempotency_key()
                || existing.input_digest() != input_digest
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
        let sign_deadline_seconds = sign_deadline_ms
            .checked_add(999)
            .ok_or(Error::DeadlineOverflow)?
            / 1_000;
        let sign_request = radroots_signing::SignRequest::new(
            OperationId::SyncPush,
            request.actor.clone(),
            request.draft.clone(),
            SignPolicy::new(sign_deadline_seconds, request.cancellation)
                .map_err(|_| Error::InvalidPushRequest)?,
        )
        .map_err(|_| Error::InvalidPushRequest)?;
        let signed = signer
            .sign(sign_request)
            .await
            .map_err(|_| Error::SignerFailed)?;
        if signed.completed_at_unix() > sign_deadline_seconds {
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
        let delivery_deadline = self
            .deadlines
            .deadline_unix_ms(OperationKind::Deliver, self.clock.now_unix_ms()?)?;
        let delivery = DeliveryRequest::new(
            delivery_request_id(request.operation_id),
            DeliveryPayload::new(event),
            request.targets,
            request.satisfaction,
            delivery_deadline,
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

fn push_input_digest(request: &PushRequest) -> IdempotencyDigest {
    let mut hasher = Sha256::new();
    hash_field(&mut hasher, b"radroots.sync.push.v1");
    hash_field(&mut hasher, request.draft.expected_event_id().as_bytes());
    for target in request.targets.targets() {
        hash_field(&mut hasher, target.fingerprint().as_str().as_bytes());
    }
    hash_satisfaction(&mut hasher, &request.satisfaction);
    IdempotencyDigest::new(hasher.finalize().into())
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

fn valid_satisfaction(policy: &SatisfactionPolicy, targets: &TargetSet) -> bool {
    let selection = policy.targets();
    if let Some(threshold) = selection.quorum_threshold() {
        return usize::from(threshold) <= targets.len();
    }
    if let Some(required) = selection.required_targets() {
        return required.iter().all(|required| {
            targets
                .targets()
                .iter()
                .any(|target| target.fingerprint() == required)
        });
    }
    selection.is_any() || selection.is_all()
}

fn atomic_digest(domain: &[u8], input: &[u8]) -> AtomicCommitDigest {
    let mut hasher = Sha256::new();
    hash_field(&mut hasher, domain);
    hash_field(&mut hasher, input);
    AtomicCommitDigest::new(hasher.finalize().into())
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
