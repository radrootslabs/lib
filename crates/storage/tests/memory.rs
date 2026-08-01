use futures_executor::block_on;
use radroots_event::{SignedEvent, wire::Nip01EventWire};
use radroots_protocol::runtime::v1::OperationId;
use radroots_storage::{
    AtomicStorage, EventStore, Journal,
    atomic::{
        AtomicCommit, AtomicCommitDigest, AtomicCommitDisposition, AtomicCommitId, AtomicWorkflow,
        CommitIngested, CommitSigned,
    },
    event::{EventAdmission, EventQuery, EventQueryBounds, SourceGeneration},
    journal::{
        IdempotencyDigest, IdempotencyKey, JournalRevision, JournalStage, OperationInstanceId,
        PrepareOperation,
    },
    memory::MemoryStorage,
    projection::{ProjectionCheckpoint, ProjectionGeneration, ProjectionId},
};
use radroots_transport::{
    Target, TransportId,
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
    let raw = serde_json::to_string(&wire).expect("event JSON");
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
    assert_eq!(block_on(store.status()).expect("status").raw_events(), 1);

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
    assert_eq!(block_on(store.status()).expect("status").raw_events(), 1);
}

#[test]
fn unsupported_atomic_projection_leaves_event_state_unchanged() {
    let store = MemoryStorage::default();
    let checkpoint = ProjectionCheckpoint::new(
        ProjectionId::parse("memory.test").expect("projection id"),
        ProjectionGeneration::new([4; 32]).expect("projection generation"),
        None,
        0,
        200,
    )
    .expect("checkpoint");
    let request = atomic(
        4,
        4,
        AtomicWorkflow::Ingested(Box::new(CommitIngested::new(
            admission(signed_event(), 20),
            Some(checkpoint),
        ))),
    );
    assert_eq!(
        block_on(store.commit(request)),
        Err(radroots_storage::Error::BackendUnavailable)
    );
    assert_eq!(block_on(store.status()).expect("status").raw_events(), 0);
}
