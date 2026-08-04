#![cfg(feature = "memory")]

use futures_executor::block_on;
use radroots_event::{SignedEvent, wire::Nip01EventWire};
use radroots_protocol::runtime::v1::OperationId;
use radroots_storage::{
    Error, EventStore, Journal, Outbox, ProjectionStore,
    atomic::{
        AtomicCommit, AtomicCommitDigest, AtomicCommitDisposition, AtomicCommitId, AtomicStorage,
        AtomicWorkflow, CommitIngested, CommitSigned,
    },
    event::{
        EventAdmission, EventPosition, EventQuery, EventQueryBounds, EventSequence,
        SourceGeneration,
    },
    journal::{
        IdempotencyDigest, IdempotencyKey, JournalRevision, JournalStage, OperationInstanceId,
        PrepareOperation,
    },
    memory::MemoryStorage,
    outbox::{
        ClaimOutboxItems, DeliveryPlanDigest, EnqueueDisposition, EnqueueOutboxItem, LeaseId,
        LeaseOwner, OutboxItemId, OutboxStage,
    },
    private_artifact::{
        ArtifactCommitment, ArtifactKind, ArtifactSchemaId, DurableSecretReference,
        PrivateArtifactId, PrivateArtifactMetadata, PrivateArtifactRevision, PrivateArtifactStore,
        RetentionPolicy,
    },
    projection::{
        InvalidationReason, ProjectionCheckpoint, ProjectionGeneration, ProjectionHealth,
        ProjectionId, ProjectionInvalidation, ProjectionRevision, RawSourceDigest, RebuildStage,
        RebuildTicket, RebuildTicketId, RebuildTransition,
    },
};
use radroots_transport::{
    DeliveryRequest, Target, TargetSet, TransportId,
    policy::{SatisfactionClass, SatisfactionPolicy, TargetPolicy},
    sink::DeliveryPayload,
    source::{EventProvenance, ObservedEvent},
};

fn signed_event() -> SignedEvent {
    let mut wire = Nip01EventWire {
        id: "0".repeat(64),
        pubkey: "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df".to_owned(),
        created_at: 1_800_000_100,
        kind: 0,
        tags: vec![],
        content: "memory-backend".to_owned(),
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

fn admission(event: SignedEvent, at: u64) -> EventAdmission {
    let target = Target::new(TransportId::NOSTR, "wss://relay.example").expect("target");
    let provenance = EventProvenance::new(TransportId::NOSTR, target.fingerprint().clone(), at)
        .expect("provenance");
    EventAdmission::raw(ObservedEvent::new(event, provenance))
}

fn prepare(instance: OperationInstanceId) -> PrepareOperation {
    PrepareOperation::new(
        instance,
        OperationId::SyncPush,
        IdempotencyKey::parse("memory-operation").expect("key"),
        IdempotencyDigest::new([3; 32]),
        100,
    )
    .expect("prepare")
}

fn atomic(id: u8, digest: u8, workflow: AtomicWorkflow) -> AtomicCommit {
    AtomicCommit::new(
        AtomicCommitId::new([id; 16]).expect("commit id"),
        AtomicCommitDigest::new([digest; 32]),
        200,
        workflow,
    )
    .expect("atomic commit")
}

fn delivery_request(event: SignedEvent) -> DeliveryRequest {
    DeliveryRequest::new(
        "memory-delivery",
        DeliveryPayload::new(event),
        TargetSet::new(vec![
            Target::nostr_relay("wss://relay.example").expect("target"),
        ])
        .expect("target set"),
        SatisfactionPolicy::new(SatisfactionClass::Accepted, TargetPolicy::all()),
        1_000,
    )
    .expect("delivery request")
}

fn private_metadata() -> PrivateArtifactMetadata {
    PrivateArtifactMetadata::new(
        PrivateArtifactId::new([9; 16]).expect("artifact id"),
        ArtifactKind::parse("memory.private").expect("kind"),
        ArtifactSchemaId::parse("memory.private.v1").expect("schema"),
        ArtifactCommitment::new([8; 32]),
        64,
        DurableSecretReference::new("memory", "caller-owned-key", 1).expect("secret reference"),
        RetentionPolicy::new(None, Some(300)).expect("retention"),
        100,
    )
    .expect("metadata")
}

#[test]
fn memory_event_and_journal_implement_the_canonical_spis() {
    let store = MemoryStorage::new(SourceGeneration::new([7; 32]).expect("generation"));
    let event = signed_event();
    let event_id = *event.id();
    block_on(store.admit(admission(event, 10))).expect("admit");
    let raw = block_on(store.query_raw(EventQuery::all(
        EventQueryBounds::first(10).expect("bounds"),
    )))
    .expect("query");
    assert_eq!(raw.items().len(), 1);
    assert_eq!(raw.items()[0].event().id(), &event_id);
    assert_eq!(
        block_on(EventStore::status(&store))
            .expect("status")
            .raw_events(),
        1
    );

    let instance = OperationInstanceId::new([1; 16]).expect("instance");
    let prepared = block_on(store.prepare(prepare(instance))).expect("prepare");
    let signed = block_on(
        store.transition(radroots_storage::journal::JournalTransition::signed(
            instance,
            prepared.record().revision(),
            event_id,
        )),
    )
    .expect("signed");
    assert_eq!(signed.state().stage(), JournalStage::Signed);
    assert_eq!(
        block_on(store.by_idempotency_key(
            OperationId::SyncPush,
            IdempotencyKey::parse("memory-operation").expect("key"),
        ))
        .expect("lookup")
        .expect("record")
        .instance_id(),
        instance
    );
}

#[test]
fn memory_atomic_commits_share_event_and_journal_state_and_replay() {
    let store = MemoryStorage::default();
    let instance = OperationInstanceId::new([2; 16]).expect("instance");
    let prepared_request = atomic(1, 1, AtomicWorkflow::Prepared(prepare(instance)));
    let prepared = block_on(store.commit(prepared_request.clone())).expect("atomic prepare");
    assert_eq!(prepared.disposition(), AtomicCommitDisposition::Committed);
    assert_eq!(
        block_on(store.commit(prepared_request))
            .expect("atomic replay")
            .disposition(),
        AtomicCommitDisposition::Replay
    );

    let event = signed_event();
    let signed_request = atomic(
        2,
        2,
        AtomicWorkflow::Signed(Box::new(CommitSigned::new(
            instance,
            JournalRevision::INITIAL,
            event.clone(),
        ))),
    );
    block_on(store.commit(signed_request)).expect("atomic signed");
    assert_eq!(
        block_on(store.operation(instance))
            .expect("operation")
            .expect("journal record")
            .state()
            .stage(),
        JournalStage::Signed
    );

    let ingest = atomic(
        3,
        3,
        AtomicWorkflow::Ingested(Box::new(CommitIngested::new(admission(event, 20), None))),
    );
    block_on(store.commit(ingest)).expect("atomic ingest");
    assert_eq!(
        block_on(EventStore::status(&store))
            .expect("status")
            .raw_events(),
        1
    );
}

#[test]
fn memory_outbox_uses_caller_supplied_time_and_lease_identity() {
    let store = MemoryStorage::default();
    let item = EnqueueOutboxItem::new(
        OutboxItemId::new([5; 16]).expect("item id"),
        OperationInstanceId::new([5; 16]).expect("instance"),
        DeliveryPlanDigest::new([5; 32]),
        delivery_request(signed_event()),
        100,
    )
    .expect("enqueue");
    assert_eq!(
        block_on(store.enqueue(item.clone()))
            .expect("enqueue")
            .disposition(),
        EnqueueDisposition::Created
    );
    assert_eq!(
        block_on(store.enqueue(item)).expect("replay").disposition(),
        EnqueueDisposition::Replay
    );
    let claimed = block_on(
        store.claim(
            ClaimOutboxItems::new(
                LeaseOwner::parse("memory-worker").expect("owner"),
                LeaseId::new([6; 16]).expect("lease seed"),
                200,
                250,
                1,
            )
            .expect("claim"),
        ),
    )
    .expect("claim");
    assert_eq!(claimed.len(), 1);
    assert_eq!(claimed[0].record().stage(), OutboxStage::Leased);
    assert_eq!(block_on(Outbox::status(&store)).expect("status").leased, 1);
}

#[test]
fn memory_projection_rebuild_and_private_metadata_share_deterministic_state() {
    let store = MemoryStorage::default();
    let projection_id = ProjectionId::parse("memory.test").expect("projection id");
    let initial_generation = ProjectionGeneration::new([4; 32]).expect("generation");
    let replacement_generation = ProjectionGeneration::new([5; 32]).expect("generation");
    let checkpoint =
        ProjectionCheckpoint::new(projection_id.clone(), initial_generation, None, 0, 200)
            .expect("checkpoint");
    assert_eq!(
        block_on(store.checkpoint(checkpoint))
            .expect("checkpoint")
            .health(),
        ProjectionHealth::Ready
    );
    let invalidation = ProjectionInvalidation::new(
        projection_id,
        initial_generation,
        replacement_generation,
        InvalidationReason::ProjectionGenerationChanged,
        210,
    )
    .expect("invalidation");
    assert_eq!(
        block_on(store.invalidate(invalidation.clone()))
            .expect("invalidate")
            .health(),
        ProjectionHealth::Invalidated
    );
    let ticket_id = RebuildTicketId::new([7; 16]).expect("ticket id");
    block_on(
        store.request_rebuild(
            RebuildTicket::requested(
                ticket_id,
                invalidation,
                store.generation(),
                None,
                RawSourceDigest::new([8; 32]),
            )
            .expect("ticket"),
        ),
    )
    .expect("request rebuild");
    let running = block_on(store.transition_rebuild(RebuildTransition::start(
        ticket_id,
        ProjectionRevision::INITIAL,
        220,
    )))
    .expect("start rebuild");
    assert_eq!(running.stage(), RebuildStage::Running);

    let metadata = private_metadata();
    block_on(store.put_metadata(metadata.clone())).expect("put metadata");
    assert_eq!(
        block_on(store.expired(299, 1)).expect("not expired").len(),
        0
    );
    assert_eq!(block_on(store.expired(300, 1)).expect("expired").len(), 1);
    block_on(store.mark_expired(
        metadata.artifact_id(),
        PrivateArtifactRevision::INITIAL,
        300,
    ))
    .expect("mark expired");
    assert_eq!(
        block_on(PrivateArtifactStore::status(&store))
            .expect("status")
            .expired,
        1
    );
}

#[test]
fn atomic_projection_failure_leaves_event_and_checkpoint_unchanged() {
    let store = MemoryStorage::default();
    let projection_id = ProjectionId::parse("memory.atomic").expect("projection id");
    let generation = ProjectionGeneration::new([4; 32]).expect("projection generation");
    let first = ProjectionCheckpoint::new(projection_id.clone(), generation, None, 2, 200)
        .expect("checkpoint");
    block_on(store.checkpoint(first)).expect("initial checkpoint");
    let regressing = ProjectionCheckpoint::new(projection_id.clone(), generation, None, 1, 201)
        .expect("checkpoint");
    let request = atomic(
        4,
        4,
        AtomicWorkflow::Ingested(Box::new(CommitIngested::new(
            admission(signed_event(), 20),
            Some(regressing),
        ))),
    );
    assert_eq!(
        block_on(store.commit(request)),
        Err(radroots_storage::Error::ProjectionCheckpointRegression)
    );
    assert_eq!(
        block_on(EventStore::status(&store))
            .expect("status")
            .raw_events(),
        0
    );
    assert_eq!(
        block_on(ProjectionStore::status(&store, projection_id))
            .expect("projection status")
            .expect("projection")
            .checkpoint()
            .expect("checkpoint")
            .projected_rows(),
        2
    );
}

#[test]
fn memory_event_journal_and_closed_state_fail_closed() {
    let generation = SourceGeneration::new([7; 32]).unwrap();
    let store = MemoryStorage::new(generation);
    assert_eq!(store.generation(), generation);
    let event = signed_event();
    let inserted = block_on(store.admit(admission(event.clone(), 10))).unwrap();
    assert_eq!(
        block_on(store.admit(admission(event.clone(), 10)))
            .unwrap()
            .disposition(),
        radroots_storage::event::AdmissionDisposition::Duplicate
    );
    let wrong_cursor = EventPosition::new(
        SourceGeneration::new([8; 32]).unwrap(),
        EventSequence::new(1).unwrap(),
    );
    let query = EventQuery::all(EventQueryBounds::first(1).unwrap().after(wrong_cursor));
    assert_eq!(
        block_on(store.query_raw(query)),
        Err(Error::SourceGenerationChanged)
    );
    assert_eq!(
        block_on(store.query_provenance(
            *event.id(),
            EventQueryBounds::first(1).unwrap().after(wrong_cursor),
        )),
        Err(Error::SourceGenerationChanged)
    );
    let after = EventQueryBounds::first(1)
        .unwrap()
        .after(inserted.position());
    assert!(
        block_on(store.query_provenance(*event.id(), after))
            .unwrap()
            .items()
            .is_empty()
    );
    assert_eq!(
        block_on(store.query_provenance(
            radroots_event::EventId::parse("f".repeat(64)).unwrap(),
            EventQueryBounds::first(1).unwrap(),
        )),
        Err(Error::EventNotFound)
    );

    let instance = OperationInstanceId::new([1; 16]).unwrap();
    let operation = prepare(instance);
    block_on(store.prepare(operation.clone())).unwrap();
    assert_eq!(
        block_on(
            store.prepare(
                PrepareOperation::new(
                    instance,
                    OperationId::SyncPush,
                    IdempotencyKey::parse("memory-operation").unwrap(),
                    IdempotencyDigest::new([4; 32]),
                    100,
                )
                .unwrap()
            )
        ),
        Err(Error::IdempotencyConflict)
    );
    assert_eq!(
        block_on(
            store.prepare(
                PrepareOperation::new(
                    instance,
                    OperationId::SyncPush,
                    IdempotencyKey::parse("different-key").unwrap(),
                    IdempotencyDigest::new([3; 32]),
                    100,
                )
                .unwrap()
            )
        ),
        Err(Error::OperationIdentityMismatch)
    );
    assert!(
        block_on(store.operation(OperationInstanceId::new([2; 16]).unwrap()))
            .unwrap()
            .is_none()
    );
    assert!(
        block_on(store.by_idempotency_key(
            OperationId::SyncPull,
            IdempotencyKey::parse("memory-operation").unwrap()
        ))
        .unwrap()
        .is_none()
    );
    assert_eq!(
        block_on(store.recoverable(0)),
        Err(Error::InvalidJournalQueryLimit)
    );

    block_on(radroots_storage::backup::StorageReliability::close(&store)).unwrap();
    assert_eq!(
        block_on(EventStore::status(&store)),
        Err(Error::BackendUnavailable)
    );
    assert_eq!(
        block_on(store.admit(admission(event, 11))),
        Err(Error::BackendUnavailable)
    );
}

#[test]
fn memory_outbox_conflict_and_claim_matrix_is_complete() {
    let store = MemoryStorage::default();
    let make_item = |item: u8, instance: u8, digest: u8, created: u64| {
        EnqueueOutboxItem::new(
            OutboxItemId::new([item; 16]).unwrap(),
            OperationInstanceId::new([instance; 16]).unwrap(),
            DeliveryPlanDigest::new([digest; 32]),
            delivery_request(signed_event()),
            created,
        )
        .unwrap()
    };
    block_on(store.enqueue(make_item(1, 1, 1, 100))).unwrap();
    assert_eq!(
        block_on(store.enqueue(make_item(1, 1, 2, 100))),
        Err(Error::OutboxPlanConflict)
    );
    assert_eq!(
        block_on(store.enqueue(make_item(2, 1, 1, 100))),
        Err(Error::OutboxPlanConflict)
    );
    assert!(
        block_on(store.item(OutboxItemId::new([9; 16]).unwrap()))
            .unwrap()
            .is_none()
    );
    let first_claim = ClaimOutboxItems::new(
        LeaseOwner::parse("worker").unwrap(),
        LeaseId::new([3; 16]).unwrap(),
        200,
        300,
        1,
    )
    .unwrap();
    assert_eq!(block_on(store.claim(first_claim)).unwrap().len(), 1);
    let concurrent = ClaimOutboxItems::new(
        LeaseOwner::parse("worker-two").unwrap(),
        LeaseId::new([4; 16]).unwrap(),
        250,
        350,
        1,
    )
    .unwrap();
    assert!(block_on(store.claim(concurrent)).unwrap().is_empty());
    assert_eq!(
        block_on(store.release(
            OutboxItemId::new([9; 16]).unwrap(),
            LeaseId::new([4; 16]).unwrap(),
            radroots_storage::outbox::OutboxRevision::INITIAL,
            260,
            None,
        )),
        Err(Error::OutboxItemNotFound)
    );
}

#[test]
fn memory_projection_and_private_artifact_conflict_matrix_is_complete() {
    let store = MemoryStorage::default();
    let projection_id = ProjectionId::parse("memory.matrix").unwrap();
    let initial = ProjectionGeneration::new([4; 32]).unwrap();
    let replacement = ProjectionGeneration::new([5; 32]).unwrap();
    let initial_checkpoint =
        ProjectionCheckpoint::new(projection_id.clone(), initial, None, 1, 100).unwrap();
    block_on(store.checkpoint(initial_checkpoint.clone())).unwrap();
    assert_eq!(
        block_on(store.checkpoint(
            ProjectionCheckpoint::new(projection_id.clone(), replacement, None, 2, 101).unwrap()
        )),
        Err(Error::ProjectionCheckpointMismatch)
    );
    assert_eq!(
        block_on(store.checkpoint(
            ProjectionCheckpoint::new(projection_id.clone(), initial, None, 0, 101).unwrap()
        )),
        Err(Error::ProjectionCheckpointRegression)
    );
    let missing = ProjectionInvalidation::new(
        ProjectionId::parse("missing").unwrap(),
        initial,
        replacement,
        InvalidationReason::OperatorRequested,
        110,
    )
    .unwrap();
    assert_eq!(
        block_on(store.invalidate(missing)),
        Err(Error::ProjectionCheckpointMismatch)
    );
    let wrong = ProjectionInvalidation::new(
        projection_id.clone(),
        replacement,
        ProjectionGeneration::new([6; 32]).unwrap(),
        InvalidationReason::OperatorRequested,
        110,
    )
    .unwrap();
    assert_eq!(
        block_on(store.invalidate(wrong)),
        Err(Error::ProjectionCheckpointMismatch)
    );
    let invalidation = ProjectionInvalidation::new(
        projection_id.clone(),
        initial,
        replacement,
        InvalidationReason::OperatorRequested,
        110,
    )
    .unwrap();
    block_on(store.invalidate(invalidation.clone())).unwrap();
    assert!(
        block_on(store.invalidation(projection_id.clone(), replacement))
            .unwrap()
            .is_some()
    );
    let ticket = RebuildTicket::requested(
        RebuildTicketId::new([7; 16]).unwrap(),
        invalidation,
        store.generation(),
        None,
        RawSourceDigest::new([8; 32]),
    )
    .unwrap();
    assert_eq!(
        block_on(store.request_rebuild(ticket.clone())).unwrap(),
        ticket
    );
    assert_eq!(
        block_on(store.request_rebuild(ticket.clone())).unwrap(),
        ticket
    );
    assert!(
        block_on(store.rebuild(ticket.ticket_id()))
            .unwrap()
            .is_some()
    );

    let metadata = private_metadata();
    assert_eq!(
        block_on(store.put_metadata(metadata.clone())).unwrap(),
        metadata
    );
    assert_eq!(
        block_on(store.put_metadata(metadata.clone())).unwrap(),
        metadata
    );
    let conflict = PrivateArtifactMetadata::new(
        metadata.artifact_id(),
        ArtifactKind::parse("memory.private").unwrap(),
        ArtifactSchemaId::parse("memory.private.v1").unwrap(),
        ArtifactCommitment::new([9; 32]),
        64,
        DurableSecretReference::new("memory", "caller-owned-key", 1).unwrap(),
        RetentionPolicy::new(None, Some(300)).unwrap(),
        100,
    )
    .unwrap();
    assert_eq!(
        block_on(store.put_metadata(conflict)),
        Err(Error::PrivateArtifactConflict)
    );
    assert!(
        block_on(store.metadata(PrivateArtifactId::new([8; 16]).unwrap()))
            .unwrap()
            .is_none()
    );
    assert_eq!(
        block_on(store.expired(0, 1)),
        Err(Error::InvalidExpiredArtifactQueryLimit)
    );
    assert_eq!(
        block_on(store.expired(1, 0)),
        Err(Error::InvalidExpiredArtifactQueryLimit)
    );
}
