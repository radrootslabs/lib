use std::sync::{
    Arc,
    atomic::{AtomicU64, AtomicUsize, Ordering},
};

use futures::{FutureExt, task::noop_waker_ref};
use futures_executor::block_on;
use radroots_event::{EventDraft, SignedEvent, contract::AuthorRole, draft::SignedEventParts};
use radroots_signing::{
    Actor, Error as SigningError, SignReceipt, SignRequest, Signer, SignerStatus,
    actor::ActorSource, error::Kind as SigningErrorKind, request::CancellationPolicy,
};
use radroots_storage::{
    EventStore, Journal, Outbox, Storage,
    event::{EventQuery, EventQueryBounds, SourceGeneration},
    journal::{IdempotencyKey, JournalStage, OperationInstanceId},
    memory::MemoryStorage,
    outbox::OutboxStage,
};
use radroots_sync::{
    Engine, PushRequest,
    policy::{Clock, DeadlinePolicy, Error, IdSource, OperationKind, SyncId},
};
use radroots_transport::{
    DeliveryReceipt, DeliveryRequest, Error as TransportError, EventSink, SinkStatus, Target,
    TargetSet, TransportId,
    policy::{SatisfactionClass, SatisfactionPolicy, TargetPolicy},
};
use secp256k1::{Keypair, Message, Secp256k1, SecretKey};

const CONTENT: &str = "frozen-content";

struct MockSink;

impl EventSink for MockSink {
    fn status(&self) -> radroots_transport::BoxFuture<'_, Result<SinkStatus, TransportError>> {
        Box::pin(async { unreachable!("enqueue does not inspect sink") })
    }
    fn deliver(
        &self,
        _request: DeliveryRequest,
    ) -> radroots_transport::BoxFuture<'_, Result<DeliveryReceipt, TransportError>> {
        Box::pin(async { unreachable!("enqueue does not deliver") })
    }
}

struct TestClock(AtomicU64);

impl Clock for TestClock {
    fn now_unix_ms(&self) -> Result<u64, Error> {
        Ok(self.0.fetch_add(1, Ordering::Relaxed))
    }
}

struct TestIds(AtomicU64);

impl IdSource for TestIds {
    fn next_id(&self, _operation: OperationKind) -> Result<SyncId, Error> {
        let value = self.0.fetch_add(1, Ordering::Relaxed);
        let byte = u8::try_from(value).map_err(|_| Error::InvalidSyncId)?;
        SyncId::new([byte; 16])
    }
}

#[derive(Clone, Copy)]
enum SignBehavior {
    Success { completed_at_unix: u64 },
    Error(SigningErrorKind),
    Pending,
}

struct MockSigner {
    behavior: SignBehavior,
    calls: AtomicUsize,
}

impl MockSigner {
    fn new(behavior: SignBehavior) -> Self {
        Self {
            behavior,
            calls: AtomicUsize::new(0),
        }
    }
}

impl Signer for MockSigner {
    fn status(
        &self,
    ) -> radroots_signing::signer::BoxFuture<'_, Result<SignerStatus, SigningError>> {
        Box::pin(async { unreachable!("enqueue does not inspect signer status") })
    }

    fn sign(
        &self,
        request: SignRequest,
    ) -> radroots_signing::signer::BoxFuture<'_, Result<SignReceipt, SigningError>> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        Box::pin(async move {
            match self.behavior {
                SignBehavior::Success { completed_at_unix } => SignReceipt::from_signed_event(
                    &request,
                    signed_event(&request),
                    completed_at_unix,
                ),
                SignBehavior::Error(kind) => Err(SigningError::new(kind)),
                SignBehavior::Pending => std::future::pending().await,
            }
        })
    }
}

fn signing_keypair() -> Keypair {
    let secret = SecretKey::from_slice(&[1; 32]).expect("secret key");
    Keypair::from_secret_key(&Secp256k1::new(), &secret)
}

fn public_key_hex() -> String {
    signing_keypair().x_only_public_key().0.to_string()
}

fn signed_event(request: &SignRequest) -> SignedEvent {
    let draft = request.draft();
    let id = draft.expected_event_id().to_hex();
    let pubkey = public_key_hex();
    let signature = Secp256k1::new()
        .sign_schnorr_no_aux_rand(
            &Message::from_digest(*draft.expected_event_id().as_bytes()),
            &signing_keypair(),
        )
        .to_string();
    let raw_json = format!(
        "{{\"id\":\"{id}\",\"pubkey\":\"{pubkey}\",\"created_at\":{},\"kind\":{},\"tags\":{:?},\"content\":{content:?},\"sig\":\"{signature}\"}}",
        draft.created_at_u64(),
        draft.kind_u32(),
        draft.tags_as_vec(),
        content = draft.content(),
    );
    SignedEvent::new(SignedEventParts {
        id,
        pubkey,
        created_at: draft.created_at_u64(),
        kind: draft.kind_u32(),
        tags: draft.tags_as_vec(),
        content: draft.content().to_owned(),
        sig: signature,
        raw_json,
    })
    .expect("signed event")
}

fn request(operation_byte: u8, relay: &str) -> PushRequest {
    let pubkey = public_key_hex();
    let draft = EventDraft::new(
        "radroots.social.geochat.v1",
        20_000,
        1_800_000_100,
        vec![],
        CONTENT,
        pubkey,
    )
    .expect("draft");
    let actor = Actor::new(
        *draft.expected_pubkey(),
        ActorSource::ExplicitPublicKey,
        [AuthorRole::Any],
    )
    .expect("actor");
    PushRequest::new(
        SyncId::new([operation_byte; 16]).expect("operation id"),
        IdempotencyKey::parse(format!("push-{operation_byte}")).expect("idempotency key"),
        actor,
        draft,
        TargetSet::new(vec![
            Target::new(TransportId::NOSTR, relay).expect("target"),
        ])
        .expect("targets"),
        SatisfactionPolicy::new(SatisfactionClass::Accepted, TargetPolicy::any()),
        CancellationPolicy::PreservePublishedRequest,
    )
    .expect("push request")
}

fn setup_engine(signer: Arc<MockSigner>) -> (Engine, Arc<MemoryStorage>) {
    let storage = Arc::new(MemoryStorage::new(
        SourceGeneration::new([6; 32]).expect("generation"),
    ));
    let capability: Arc<dyn Storage> = storage.clone();
    let engine = Engine::builder(
        capability,
        Arc::new(TestClock(AtomicU64::new(1_800_000_200_000))),
        Arc::new(TestIds(AtomicU64::new(10))),
        DeadlinePolicy::new(10_000, 10_000, 10_000).expect("deadlines"),
    )
    .sink(Arc::new(MockSink))
    .signer(signer)
    .build()
    .expect("engine");
    (engine, storage)
}

#[test]
fn authorized_signing_atomically_enqueues_and_replays_without_resigning() {
    let signer = Arc::new(MockSigner::new(SignBehavior::Success {
        completed_at_unix: 1_800_000_200,
    }));
    let (engine, storage) = setup_engine(signer.clone());
    let request = request(1, "wss://relay.example");
    let receipt = block_on(engine.sign_and_enqueue(request.clone())).expect("enqueue");
    assert!(!receipt.is_replay());
    assert_eq!(receipt.outbox().stage(), OutboxStage::Pending);
    assert_eq!(signer.calls.load(Ordering::Relaxed), 1);
    let replay = block_on(engine.sign_and_enqueue(request)).expect("replay");
    assert!(replay.is_replay());
    assert_eq!(replay.outbox(), receipt.outbox());
    assert_eq!(signer.calls.load(Ordering::Relaxed), 1);

    let visible = block_on(storage.query_visible(EventQuery::all(
        EventQueryBounds::first(10).expect("bounds"),
    )))
    .expect("visible events");
    assert_eq!(visible.items().len(), 1);
    assert!(
        block_on(Outbox::item(
            &*storage,
            radroots_storage::outbox::OutboxItemId::new([1; 16]).expect("item id"),
        ))
        .expect("outbox item")
        .is_some()
    );
}

#[test]
fn signer_rejection_challenge_and_timeout_fail_before_enqueue() {
    for kind in [
        SigningErrorKind::SignerRejected,
        SigningErrorKind::SignerCapabilityMissing,
        SigningErrorKind::SignerTimeout,
        SigningErrorKind::SignerOutputInvalid,
    ] {
        let signer = Arc::new(MockSigner::new(SignBehavior::Error(kind)));
        let (engine, storage) = setup_engine(signer);
        assert_eq!(
            block_on(engine.sign_and_enqueue(request(2, "wss://relay.example"))),
            Err(Error::SignerFailed)
        );
        assert!(
            block_on(Outbox::item(
                &*storage,
                radroots_storage::outbox::OutboxItemId::new([2; 16]).expect("item id"),
            ))
            .expect("outbox lookup")
            .is_none()
        );
    }

    let late = Arc::new(MockSigner::new(SignBehavior::Success {
        completed_at_unix: 1_800_000_212,
    }));
    let (engine, _) = setup_engine(late);
    assert_eq!(
        block_on(engine.sign_and_enqueue(request(3, "wss://relay.example"))),
        Err(Error::SignerDeadlineExceeded)
    );
}

#[test]
fn idempotency_conflict_and_cancellation_before_commit_fail_closed() {
    let signer = Arc::new(MockSigner::new(SignBehavior::Success {
        completed_at_unix: 1_800_000_200,
    }));
    let (engine, _) = setup_engine(signer.clone());
    block_on(engine.sign_and_enqueue(request(4, "wss://relay.example"))).expect("enqueue");
    assert_eq!(
        block_on(engine.sign_and_enqueue(request(4, "wss://other.example"))),
        Err(Error::StorageConflict)
    );
    assert_eq!(signer.calls.load(Ordering::Relaxed), 1);

    let pending = Arc::new(MockSigner::new(SignBehavior::Pending));
    let (engine, storage) = setup_engine(pending);
    let mut future = Box::pin(engine.sign_and_enqueue(request(5, "wss://relay.example"))).fuse();
    let mut context = std::task::Context::from_waker(noop_waker_ref());
    assert!(future.poll_unpin(&mut context).is_pending());
    drop(future);
    let record = block_on(Journal::operation(
        &*storage,
        OperationInstanceId::new([5; 16]).expect("instance id"),
    ))
    .expect("journal lookup")
    .expect("prepared record");
    assert_eq!(record.state().stage(), JournalStage::Prepared);
    assert!(
        block_on(Outbox::item(
            &*storage,
            radroots_storage::outbox::OutboxItemId::new([5; 16]).expect("item id"),
        ))
        .expect("outbox lookup")
        .is_none()
    );
}
