use futures_executor::block_on;
use radroots_event::{SignedEvent, wire::Nip01EventWire};
use radroots_storage::{
    Error,
    atomic::{
        AtomicCommit, AtomicCommitDigest, AtomicCommitDisposition, AtomicCommitId,
        AtomicCommitOutcome, AtomicCommitReceipt, AtomicStorage, AtomicWorkflow,
        AtomicWorkflowKind, CommitIngested,
    },
    event::{
        AdmissionDisposition, AdmissionReceipt, AdmissionStage, EventAdmission, EventPosition,
        EventSequence, SourceGeneration,
    },
    memory::MemoryStorage,
};
use radroots_transport::{
    Target, TransportId,
    source::{EventProvenance, ObservedEvent},
};

fn signed_event() -> SignedEvent {
    let raw = r#"{"id":"56bfc78223bb2221bad82b539efdec1ade0f56d0eb0e1f592fd387df4b2ceee0","pubkey":"585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df","created_at":1700000001,"kind":0,"tags":[],"content":"{}","sig":"dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"}"#;
    SignedEvent::from_wire_verified_id(Nip01EventWire::parse_json(raw).expect("wire event"), raw)
        .expect("signed event")
}

fn admission(event: SignedEvent) -> EventAdmission {
    let target = Target::nostr_relay("wss://relay.example").expect("target");
    let provenance = EventProvenance::new(TransportId::NOSTR, target.fingerprint().clone(), 100)
        .expect("provenance");
    EventAdmission::raw(ObservedEvent::new(event, provenance))
}

fn commit_request(id: u8, digest: u8) -> AtomicCommit {
    AtomicCommit::new(
        AtomicCommitId::new([id; 16]).expect("commit ID"),
        AtomicCommitDigest::new([digest; 32]),
        100,
        AtomicWorkflow::Ingested(Box::new(CommitIngested::new(
            admission(signed_event()),
            None,
        ))),
    )
    .expect("commit")
}

fn outcome() -> AtomicCommitOutcome {
    let event = signed_event();
    AtomicCommitOutcome::Ingested {
        admission: AdmissionReceipt::new(
            *event.id(),
            EventPosition::new(
                SourceGeneration::new([1; 32]).expect("generation"),
                EventSequence::new(1).expect("sequence"),
            ),
            AdmissionStage::Raw,
            AdmissionDisposition::Inserted,
        ),
        projection: None,
    }
}

#[test]
fn atomic_identity_workflow_and_receipt_models_are_exact() {
    assert_eq!(
        AtomicCommitId::new([0; 16]),
        Err(Error::InvalidAtomicCommitId)
    );
    let request = commit_request(1, 2);
    assert_eq!(request.commit_id().as_bytes(), &[1; 16]);
    assert_eq!(request.digest().as_bytes(), &[2; 32]);
    assert_eq!(request.requested_at_unix_ms(), 100);
    assert_eq!(request.workflow().kind(), AtomicWorkflowKind::Ingested);
    let AtomicWorkflow::Ingested(ingested) = request.workflow();
    assert_eq!(ingested.admission().event().id(), signed_event().id());
    assert_eq!(ingested.projection(), None);

    assert_eq!(
        AtomicCommit::new(
            request.commit_id(),
            request.digest(),
            0,
            request.workflow().clone(),
        ),
        Err(Error::InvalidAtomicCommitTimestamp)
    );

    let receipt =
        AtomicCommitReceipt::new(&request, AtomicCommitDisposition::Committed, 100, outcome())
            .expect("receipt");
    assert_eq!(receipt.commit_id(), request.commit_id());
    assert_eq!(receipt.digest(), request.digest());
    assert_eq!(receipt.disposition(), AtomicCommitDisposition::Committed);
    assert_eq!(receipt.committed_at_unix_ms(), 100);
    assert_eq!(receipt.outcome().kind(), AtomicWorkflowKind::Ingested);
    assert_eq!(
        AtomicCommitReceipt::new(&request, AtomicCommitDisposition::Committed, 99, outcome(),),
        Err(Error::AtomicWorkflowMismatch)
    );
}

#[test]
fn durable_receipt_reconstruction_rejects_timestamp_incoherence() {
    let request = commit_request(1, 2);
    for (requested_at, committed_at) in [(0, 100), (101, 100)] {
        assert_eq!(
            AtomicCommitReceipt::from_durable_parts(
                request.commit_id(),
                request.digest(),
                AtomicCommitDisposition::Replay,
                requested_at,
                committed_at,
                AtomicWorkflowKind::Ingested,
                outcome(),
            ),
            Err(Error::AtomicWorkflowMismatch)
        );
    }

    let receipt = AtomicCommitReceipt::from_durable_parts(
        request.commit_id(),
        request.digest(),
        AtomicCommitDisposition::Replay,
        100,
        101,
        AtomicWorkflowKind::Ingested,
        outcome(),
    )
    .expect("durable receipt");
    assert_eq!(receipt.disposition(), AtomicCommitDisposition::Replay);
    assert_eq!(receipt.committed_at_unix_ms(), 101);
}

#[test]
fn memory_atomic_boundary_replays_exactly_and_conflicts_on_digest_reuse() {
    fn accepts_dyn(_: &dyn AtomicStorage) {}

    let store = MemoryStorage::default();
    accepts_dyn(&store);
    let request = commit_request(1, 2);
    assert_eq!(
        block_on(store.commit(request.clone()))
            .expect("commit")
            .disposition(),
        AtomicCommitDisposition::Committed
    );
    assert_eq!(
        block_on(store.commit(request.clone()))
            .expect("replay")
            .disposition(),
        AtomicCommitDisposition::Replay
    );
    assert_eq!(
        block_on(store.commit(commit_request(1, 3))),
        Err(Error::AtomicCommitConflict)
    );
    assert_eq!(
        block_on(store.receipt(request.commit_id()))
            .expect("receipt")
            .expect("durable receipt")
            .digest(),
        request.digest()
    );
}
