use futures_executor::block_on;
use radroots_event::{SignedEvent, wire::Nip01EventWire};
use radroots_protocol::runtime::v1::OperationId;
use radroots_storage::{
    EventStore, Journal, Outbox, ProjectionStore,
    atomic::{
        AtomicCommit, AtomicCommitDigest, AtomicCommitDisposition, AtomicCommitId, AtomicStorage,
        AtomicWorkflow, CommitIngested,
    },
    backup::{
        BackupFormatVersion, BackupId, BackupManifest, BackupMember, BackupMemberKind, BackupPlan,
        BackupSecretPolicy, BackupStage, BackupTransition, MemberDigest, MemberVerification,
        RestoreMemberStatus, RestorePlan, RestoreStage, RestoreTransition, StorageReliability,
    },
    event::{EventAdmission, EventQuery, EventQueryBounds},
    journal::{IdempotencyDigest, IdempotencyKey, OperationInstanceId, PrepareOperation},
    outbox::{DeliveryPlanDigest, EnqueueDisposition, EnqueueOutboxItem, OutboxItemId},
    private_artifact::{
        ArtifactCommitment, ArtifactKind, ArtifactSchemaId, DurableSecretReference,
        PrivateArtifactId, PrivateArtifactMetadata, PrivateArtifactStore, RetentionPolicy,
    },
    projection::{ProjectionCheckpoint, ProjectionGeneration, ProjectionId},
    status::{ShutdownState, StorageBackend},
};
use radroots_transport::{
    DeliveryRequest, Target, TargetSet, TransportId,
    policy::{SatisfactionClass, SatisfactionPolicy, TargetPolicy},
    sink::DeliveryPayload,
    source::{EventProvenance, ObservedEvent},
};

/// Backend adapter consumed by the shared storage contract assertions.
pub(crate) trait StorageConformanceHarness {
    fn event_store(&self) -> &dyn EventStore;
    fn journal(&self) -> &dyn Journal;
    fn outbox(&self) -> &dyn Outbox;
    fn projection_store(&self) -> &dyn ProjectionStore;
    fn private_artifact_store(&self) -> &dyn PrivateArtifactStore;
    fn atomic_storage(&self) -> &dyn AtomicStorage;
    fn reliability(&self) -> &dyn StorageReliability;
}

pub(crate) fn assert_shared_state_conformance(harness: &impl StorageConformanceHarness) {
    let event = signed_event("conformance-shared");
    let event_id = *event.id();
    block_on(harness.event_store().admit(admission(event.clone(), 100))).expect("admit event");
    let page = block_on(harness.event_store().query_raw(EventQuery::all(
        EventQueryBounds::first(10).expect("query bounds"),
    )))
    .expect("query events");
    assert_eq!(page.items().len(), 1);
    assert_eq!(page.items()[0].event().id(), &event_id);

    let instance = OperationInstanceId::new([1; 16]).expect("operation instance");
    let prepared = block_on(harness.journal().prepare(prepare(instance, "shared", 1)))
        .expect("prepare operation");
    assert_eq!(
        block_on(harness.journal().operation(instance))
            .expect("journal lookup")
            .expect("journal record"),
        *prepared.record()
    );

    let outbox = enqueue([2; 16], instance, event);
    assert_eq!(
        block_on(harness.outbox().enqueue(outbox.clone()))
            .expect("enqueue")
            .disposition(),
        EnqueueDisposition::Created
    );
    assert_eq!(
        block_on(harness.outbox().enqueue(outbox))
            .expect("enqueue replay")
            .disposition(),
        EnqueueDisposition::Replay
    );

    let checkpoint = ProjectionCheckpoint::new(
        ProjectionId::parse("conformance.shared").expect("projection id"),
        ProjectionGeneration::new([3; 32]).expect("projection generation"),
        None,
        1,
        200,
    )
    .expect("projection checkpoint");
    let projection =
        block_on(harness.projection_store().checkpoint(checkpoint)).expect("store checkpoint");
    assert_eq!(
        projection
            .checkpoint()
            .expect("checkpoint")
            .projected_rows(),
        1
    );

    let metadata = private_metadata([4; 16]);
    assert_eq!(
        block_on(
            harness
                .private_artifact_store()
                .put_metadata(metadata.clone()),
        )
        .expect("put private metadata"),
        metadata
    );
}

pub(crate) fn assert_atomic_failure_isolation(harness: &impl StorageConformanceHarness) {
    let projection_id = ProjectionId::parse("conformance.atomic").expect("projection id");
    let generation = ProjectionGeneration::new([5; 32]).expect("projection generation");
    let current = ProjectionCheckpoint::new(projection_id.clone(), generation, None, 2, 200)
        .expect("current checkpoint");
    block_on(harness.projection_store().checkpoint(current)).expect("store checkpoint");
    let regression = ProjectionCheckpoint::new(projection_id.clone(), generation, None, 1, 201)
        .expect("regressing checkpoint");
    let request = AtomicCommit::new(
        AtomicCommitId::new([6; 16]).expect("commit id"),
        AtomicCommitDigest::new([6; 32]),
        201,
        AtomicWorkflow::Ingested(Box::new(CommitIngested::new(
            admission(signed_event("conformance-rollback"), 201),
            Some(regression),
        ))),
    )
    .expect("atomic request");
    assert_eq!(
        block_on(harness.atomic_storage().commit(request)),
        Err(radroots_storage::Error::ProjectionCheckpointRegression)
    );
    assert_eq!(
        block_on(harness.event_store().status())
            .expect("event status")
            .raw_events(),
        0
    );
    assert_eq!(
        block_on(harness.projection_store().status(projection_id))
            .expect("projection status")
            .expect("projection")
            .checkpoint()
            .expect("checkpoint")
            .projected_rows(),
        2
    );
    assert!(
        block_on(
            harness
                .atomic_storage()
                .receipt(AtomicCommitId::new([6; 16]).expect("commit id")),
        )
        .expect("receipt lookup")
        .is_none()
    );
}

pub(crate) fn assert_conflict_conformance(harness: &impl StorageConformanceHarness) {
    let instance = OperationInstanceId::new([7; 16]).expect("operation instance");
    block_on(harness.journal().prepare(prepare(instance, "conflict", 7)))
        .expect("prepare operation");
    assert_eq!(
        block_on(harness.journal().prepare(prepare(instance, "conflict", 8))),
        Err(radroots_storage::Error::IdempotencyConflict)
    );

    let event = signed_event("conformance-conflict");
    let first = enqueue([8; 16], instance, event.clone());
    block_on(harness.outbox().enqueue(first)).expect("enqueue plan");
    let conflicting = EnqueueOutboxItem::new(
        OutboxItemId::new([8; 16]).expect("item id"),
        instance,
        DeliveryPlanDigest::new([9; 32]),
        delivery_request(event),
        100,
    )
    .expect("conflicting plan");
    assert_eq!(
        block_on(harness.outbox().enqueue(conflicting)),
        Err(radroots_storage::Error::OutboxPlanConflict)
    );
}

pub(crate) fn assert_atomic_workflow_conformance(harness: &impl StorageConformanceHarness) {
    let event = signed_event("conformance-ingest");
    let request = atomic_commit(
        14,
        14,
        150,
        AtomicWorkflow::Ingested(Box::new(CommitIngested::new(
            admission(event, 150),
            Some(
                ProjectionCheckpoint::new(
                    ProjectionId::parse("conformance.ingest").expect("projection id"),
                    ProjectionGeneration::new([14; 32]).expect("projection generation"),
                    None,
                    1,
                    150,
                )
                .expect("projection checkpoint"),
            ),
        ))),
    );
    let ingested =
        block_on(harness.atomic_storage().commit(request.clone())).expect("atomic ingest");
    assert_eq!(ingested.disposition(), AtomicCommitDisposition::Committed);
    assert_eq!(
        ingested.outcome().kind(),
        radroots_storage::atomic::AtomicWorkflowKind::Ingested
    );
    assert_eq!(
        block_on(harness.atomic_storage().commit(request))
            .expect("atomic replay")
            .disposition(),
        AtomicCommitDisposition::Replay
    );
}

pub(crate) fn assert_reliability_and_close_conformance(harness: &impl StorageConformanceHarness) {
    let backup_id = BackupId::new([15; 16]).expect("backup id");
    let plan = BackupPlan::new(
        backup_id,
        BackupFormatVersion::V1,
        BackupSecretPolicy::ExcludeProtectedStorage,
        100,
    )
    .expect("backup plan");
    let planned = block_on(harness.reliability().begin_backup(plan)).expect("begin backup");
    assert_eq!(planned.stage(), BackupStage::Planned);
    let manifest = backup_manifest(backup_id);
    let captured = block_on(harness.reliability().transition_backup(
        backup_id,
        planned.revision(),
        BackupTransition::Captured(manifest.clone()),
        110,
    ))
    .expect("capture backup");
    let verified = block_on(harness.reliability().transition_backup(
        backup_id,
        captured.revision(),
        BackupTransition::Verified,
        120,
    ))
    .expect("verify backup");
    let finalized = block_on(harness.reliability().transition_backup(
        backup_id,
        verified.revision(),
        BackupTransition::Finalize,
        130,
    ))
    .expect("finalize backup");
    assert_eq!(finalized.stage(), BackupStage::Finalized);

    let restore = block_on(
        harness.reliability().begin_restore(
            RestorePlan::new(manifest, BackupSecretPolicy::ExcludeProtectedStorage, 140)
                .expect("restore plan"),
        ),
    )
    .expect("begin restore");
    let verifying = block_on(harness.reliability().transition_restore(
        backup_id,
        restore.revision(),
        RestoreTransition::Staged,
        150,
    ))
    .expect("stage restore");
    let finalizing = block_on(harness.reliability().transition_restore(
        backup_id,
        verifying.revision(),
        RestoreTransition::Verified(vec![
            RestoreMemberStatus::new("memory/state", MemberVerification::Verified)
                .expect("member status"),
        ]),
        160,
    ))
    .expect("verify restore");
    let restored = block_on(harness.reliability().transition_restore(
        backup_id,
        finalizing.revision(),
        RestoreTransition::Finalize,
        170,
    ))
    .expect("finalize restore");
    assert_eq!(restored.stage(), RestoreStage::Finalized);
    assert_eq!(
        block_on(harness.reliability().status())
            .expect("storage status")
            .backend(),
        StorageBackend::Memory
    );
    assert_eq!(
        block_on(harness.reliability().close())
            .expect("close storage")
            .shutdown(),
        ShutdownState::Closed
    );
    assert_eq!(
        block_on(harness.event_store().status()),
        Err(radroots_storage::Error::BackendUnavailable)
    );
}

fn signed_event(content: &str) -> SignedEvent {
    let mut wire = Nip01EventWire {
        id: "0".repeat(64),
        pubkey: "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df".to_owned(),
        created_at: 1_800_000_100,
        kind: 1,
        tags: vec![],
        content: content.to_owned(),
        sig: "42".repeat(64),
        extra: Default::default(),
    };
    wire.id = wire.computed_event_id().expect("event id").to_hex();
    let raw = serde_json::json!({
        "id": &wire.id,
        "pubkey": &wire.pubkey,
        "created_at": wire.created_at,
        "kind": wire.kind,
        "tags": &wire.tags,
        "content": &wire.content,
        "sig": &wire.sig,
    })
    .to_string();
    SignedEvent::from_wire_verified_id(wire, raw).expect("signed event")
}

fn admission(event: SignedEvent, observed_at_unix_ms: u64) -> EventAdmission {
    let target = Target::nostr_relay("wss://conformance.example").expect("target");
    let provenance = EventProvenance::new(
        TransportId::NOSTR,
        target.fingerprint().clone(),
        observed_at_unix_ms,
    )
    .expect("provenance");
    EventAdmission::raw(ObservedEvent::new(event, provenance))
}

fn prepare(instance: OperationInstanceId, key: &str, digest: u8) -> PrepareOperation {
    PrepareOperation::new(
        instance,
        OperationId::SyncPush,
        IdempotencyKey::parse(key).expect("idempotency key"),
        IdempotencyDigest::new([digest; 32]),
        100,
    )
    .expect("prepare operation")
}

fn delivery_request(event: SignedEvent) -> DeliveryRequest {
    DeliveryRequest::new(
        "storage-conformance",
        DeliveryPayload::new(event),
        TargetSet::new(vec![
            Target::nostr_relay("wss://conformance.example").expect("target"),
        ])
        .expect("target set"),
        SatisfactionPolicy::new(SatisfactionClass::Accepted, TargetPolicy::all()),
        1_000,
    )
    .expect("delivery request")
}

fn enqueue(
    item_id: [u8; 16],
    instance: OperationInstanceId,
    event: SignedEvent,
) -> EnqueueOutboxItem {
    EnqueueOutboxItem::new(
        OutboxItemId::new(item_id).expect("item id"),
        instance,
        DeliveryPlanDigest::new([2; 32]),
        delivery_request(event),
        100,
    )
    .expect("enqueue")
}

fn private_metadata(id: [u8; 16]) -> PrivateArtifactMetadata {
    PrivateArtifactMetadata::new(
        PrivateArtifactId::new(id).expect("artifact id"),
        ArtifactKind::parse("conformance.private").expect("artifact kind"),
        ArtifactSchemaId::parse("conformance.private.v1").expect("schema id"),
        ArtifactCommitment::new([4; 32]),
        32,
        DurableSecretReference::new("conformance", "opaque-reference", 1)
            .expect("secret reference"),
        RetentionPolicy::indefinite(),
        100,
    )
    .expect("private metadata")
}

fn atomic_commit(id: u8, digest: u8, at: u64, workflow: AtomicWorkflow) -> AtomicCommit {
    AtomicCommit::new(
        AtomicCommitId::new([id; 16]).expect("commit id"),
        AtomicCommitDigest::new([digest; 32]),
        at,
        workflow,
    )
    .expect("atomic commit")
}

fn backup_manifest(backup_id: BackupId) -> BackupManifest {
    BackupManifest::new(
        BackupFormatVersion::V1,
        backup_id,
        105,
        BackupSecretPolicy::ExcludeProtectedStorage,
        vec![
            BackupMember::new(
                "memory/state",
                BackupMemberKind::Runtime,
                1,
                MemberDigest::new([15; 32]),
            )
            .expect("backup member"),
        ],
    )
    .expect("backup manifest")
}
