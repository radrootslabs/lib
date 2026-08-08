use std::sync::{
    Arc,
    atomic::{AtomicU8, Ordering},
};

use futures_executor::block_on;
use radroots_event::{
    SignedEvent,
    draft::SignedEventParts,
    food::availability::{
        FoodAvailabilityDetails, FoodAvailabilityDetailsParts, FoodAvailabilityStatus, FoodContent,
        FoodCurrency, FoodIdentifier, FoodPrice, FoodPublishedAt, FoodText, FoodUnit,
    },
};
use radroots_event_codec::authoring::AuthoredEventPlan;
use radroots_storage::{
    EventStore,
    event::{AdmissionDisposition, AdmissionStage, EventQuery, EventQueryBounds, SourceGeneration},
    memory::MemoryStorage,
};
use radroots_sync::{
    Engine,
    ingest::{AdmissionDecision, AdmissionPolicy, RegistryPolicy},
    policy::{Clock, DeadlinePolicy, Error, IdSource, OperationKind, SyncId, SyncStorage},
};
use radroots_transport::{
    Error as TransportError, EventSource, FetchPage, FetchRequest, SourceStatus, Target,
    TransportId,
    source::{EventProvenance, ObservedEvent},
};
use secp256k1::{Keypair, Message, Secp256k1, SecretKey};

const EVENT_ID: &str = "762bee187e9e645b81ec26ade05a69b5e8398caf527be8de0d9a45311ed0c7a0";
const PUBKEY: &str = "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df";
const SIGNATURE: &str = "4290da0bb6422986647bc8cd5f63bd52d49f41e7b665d3b47105b8109183e8d596f322c531d4061df53e1d2b70fda12d5d1c14f3720d7a56d9d0a03746af5109";
const CONTENT: &str = "{\"display_name\":\"Moss Street Farm\",\"bot\":false,\"website\":\"https://mossstreet.example\",\"picture\":42}";

struct MockSource;

impl EventSource for MockSource {
    fn status(&self) -> radroots_transport::BoxFuture<'_, Result<SourceStatus, TransportError>> {
        Box::pin(async { unreachable!("ingest does not inspect source status") })
    }

    fn fetch(
        &self,
        _request: FetchRequest,
    ) -> radroots_transport::BoxFuture<'_, Result<FetchPage, TransportError>> {
        Box::pin(async { unreachable!("ingest does not fetch") })
    }
}

struct FixedClock;

impl Clock for FixedClock {
    fn now_unix_ms(&self) -> Result<u64, Error> {
        Ok(1_800_000_200_000)
    }
}

struct SequenceIds(AtomicU8);

impl IdSource for SequenceIds {
    fn next_id(&self, operation: OperationKind) -> Result<SyncId, Error> {
        assert_eq!(operation, OperationKind::Ingest);
        let byte = self.0.fetch_add(1, Ordering::Relaxed);
        SyncId::new([byte; 16])
    }
}

struct ConstantIds(u8);

impl IdSource for ConstantIds {
    fn next_id(&self, operation: OperationKind) -> Result<SyncId, Error> {
        assert_eq!(operation, OperationKind::Ingest);
        SyncId::new([self.0; 16])
    }
}

struct Reject;

impl AdmissionPolicy for Reject {
    fn policy_id(&self) -> &'static str {
        "test.reject.v1"
    }

    fn decide(
        &self,
        _event: &radroots_event::admission::ContractValidatedEvent,
    ) -> AdmissionDecision {
        AdmissionDecision::Reject
    }
}

struct FoodAvailabilityPolicy;

impl AdmissionPolicy for FoodAvailabilityPolicy {
    fn policy_id(&self) -> &'static str {
        "test.food-availability.v1"
    }

    fn select_contract(
        &self,
        _event: &radroots_event::admission::SignatureVerifiedEvent,
    ) -> Option<&'static str> {
        Some("radroots.food.availability.v1")
    }

    fn decide(
        &self,
        _event: &radroots_event::admission::ContractValidatedEvent,
    ) -> AdmissionDecision {
        AdmissionDecision::Visible
    }
}

fn setup_engine(first_id: u8) -> (Engine, Arc<MemoryStorage>) {
    setup_engine_with_ids(Arc::new(SequenceIds(AtomicU8::new(first_id))))
}

fn setup_engine_with_ids(ids: Arc<dyn IdSource>) -> (Engine, Arc<MemoryStorage>) {
    let storage = Arc::new(MemoryStorage::new(
        SourceGeneration::new([7; 32]).expect("source generation"),
    ));
    let storage_capability: Arc<dyn SyncStorage> = storage.clone();
    let engine = Engine::builder(
        storage_capability,
        Arc::new(FixedClock),
        ids,
        DeadlinePolicy::new(10_000, 10_000, 10_000).expect("deadlines"),
    )
    .source(Arc::new(MockSource))
    .build()
    .expect("engine");
    (engine, storage)
}

fn signed_event(signature: &str) -> SignedEvent {
    let raw_json = format!(
        "{{\"id\":\"{EVENT_ID}\",\"pubkey\":\"{PUBKEY}\",\"created_at\":1800000100,\"kind\":0,\"tags\":[],\"content\":{content:?},\"sig\":\"{signature}\"}}",
        content = CONTENT,
    );
    SignedEvent::new(SignedEventParts {
        id: EVENT_ID.to_owned(),
        pubkey: PUBKEY.to_owned(),
        created_at: 1_800_000_100,
        kind: 0,
        tags: vec![],
        content: CONTENT.to_owned(),
        sig: signature.to_owned(),
        raw_json,
    })
    .expect("ID-valid signed event")
}

fn observed(signature: &str, observed_at: u64) -> ObservedEvent {
    let target = Target::new(TransportId::NOSTR, "wss://relay.example").expect("target");
    let provenance = EventProvenance::new(
        TransportId::NOSTR,
        target.fingerprint().clone(),
        observed_at,
    )
    .expect("provenance");
    ObservedEvent::new(signed_event(signature), provenance)
}

fn observed_food(observed_at: u64) -> ObservedEvent {
    let created_at = 1_800_000_100;
    let keypair = Keypair::from_secret_key(
        &Secp256k1::new(),
        &SecretKey::from_slice(&[1; 32]).expect("food fixture secret"),
    );
    let public_key = keypair.x_only_public_key().0.to_string();
    let details = FoodAvailabilityDetails::new(FoodAvailabilityDetailsParts {
        content: FoodContent::new("Carrots available this week.").expect("content"),
        identifier: FoodIdentifier::parse("nantes-carrots").expect("identifier"),
        title: FoodText::new("Nantes Carrots").expect("title"),
        summary: FoodText::new("Fresh bunches").expect("summary"),
        published_at: FoodPublishedAt::new(created_at).expect("published at"),
        location: FoodText::new("Central Saanich, BC").expect("location"),
        price: FoodPrice::new(
            "3",
            FoodCurrency::parse("CAD").expect("currency"),
            FoodUnit::Pound,
        )
        .expect("price"),
        quantity: None,
        status: FoodAvailabilityStatus::Active,
        images: Vec::new(),
    })
    .expect("food availability");
    let plan = AuthoredEventPlan::from_food_availability(&details, created_at, &public_key)
        .expect("food plan");
    let id = plan.expected_event_id().to_hex();
    let signature = Secp256k1::new()
        .sign_schnorr_no_aux_rand(
            &Message::from_digest(*plan.expected_event_id().as_bytes()),
            &keypair,
        )
        .to_string();
    let raw_json = format!(
        "{{\"id\":\"{id}\",\"pubkey\":\"{public_key}\",\"created_at\":{created_at},\"kind\":{},\"tags\":{:?},\"content\":{content:?},\"sig\":\"{signature}\"}}",
        plan.body().kind(),
        plan.body().tags(),
        content = plan.body().content(),
    );
    let event = SignedEvent::new(SignedEventParts {
        id,
        pubkey: public_key,
        created_at,
        kind: plan.body().kind(),
        tags: plan.body().tags().to_vec(),
        content: plan.body().content().to_owned(),
        sig: signature,
        raw_json,
    })
    .expect("signed food event");
    let target = Target::new(TransportId::NOSTR, "wss://relay.example").expect("target");
    let provenance = EventProvenance::new(
        TransportId::NOSTR,
        target.fingerprint().clone(),
        observed_at,
    )
    .expect("provenance");
    ObservedEvent::new(event, provenance)
}

#[test]
fn valid_visible_ingest_is_atomic_and_preserves_provenance() {
    let (engine, storage) = setup_engine(1);
    let receipt = block_on(engine.ingest(
        observed(SIGNATURE, 1_800_000_100_000),
        &RegistryPolicy::visible(),
    ))
    .expect("visible ingest");
    assert_eq!(receipt.sync_id().as_bytes(), &[1; 16]);
    assert_eq!(
        receipt.commit_disposition(),
        radroots_storage::atomic::AtomicCommitDisposition::Committed
    );
    assert_eq!(receipt.committed_at_unix_ms(), 1_800_000_200_000);
    assert_eq!(receipt.admission().stage(), AdmissionStage::Visible);
    assert_eq!(
        receipt.admission().disposition(),
        AdmissionDisposition::Inserted
    );

    let bounds = EventQueryBounds::first(10).expect("bounds");
    let visible = block_on(storage.query_visible(EventQuery::all(bounds))).expect("visible query");
    assert_eq!(visible.items().len(), 1);
    let provenance = block_on(storage.query_provenance(*receipt.admission().event_id(), bounds))
        .expect("provenance query");
    assert_eq!(provenance.items().len(), 1);
    assert_eq!(
        provenance.items()[0].provenance().observed_at_unix_ms(),
        1_800_000_100_000
    );
}

#[test]
fn admission_policy_selects_and_fully_validates_admission_only_wire_profiles() {
    let (engine, storage) = setup_engine(1);
    assert_eq!(
        block_on(engine.ingest(observed_food(1), &RegistryPolicy::visible())),
        Err(Error::VerificationFailed)
    );
    let receipt = block_on(engine.ingest(observed_food(2), &FoodAvailabilityPolicy))
        .expect("policy-selected food admission");
    assert_eq!(receipt.admission().stage(), AdmissionStage::Visible);
    let visible = block_on(storage.query_visible(EventQuery::all(
        EventQueryBounds::first(10).expect("bounds"),
    )))
    .expect("visible query");
    assert_eq!(visible.items().len(), 1);
    assert_eq!(visible.items()[0].event().kind(), 30_402);
}

#[test]
fn invalid_policy_rejected_and_verified_only_inputs_fail_closed() {
    let (engine, storage) = setup_engine(1);
    let invalid_signature = format!("0{}", &SIGNATURE[1..]);
    assert_eq!(
        block_on(engine.ingest(observed(&invalid_signature, 1), &RegistryPolicy::visible())),
        Err(Error::VerificationFailed)
    );
    assert_eq!(
        block_on(engine.ingest(observed(SIGNATURE, 2), &Reject)),
        Err(Error::PolicyRejected)
    );

    let receipt = block_on(engine.ingest(observed(SIGNATURE, 3), &RegistryPolicy::verified()))
        .expect("verified ingest");
    assert_eq!(receipt.admission().stage(), AdmissionStage::Verified);
    let page = block_on(storage.query_visible(EventQuery::all(
        EventQueryBounds::first(10).expect("bounds"),
    )))
    .expect("visible query");
    assert!(page.items().is_empty());
}

#[test]
fn duplicate_conflict_and_partial_batch_outcomes_are_normalized() {
    let (engine, storage) = setup_engine(1);
    let inserted = block_on(engine.ingest(observed(SIGNATURE, 10), &RegistryPolicy::visible()))
        .expect("insert");
    let duplicate = block_on(engine.ingest(observed(SIGNATURE, 11), &RegistryPolicy::visible()))
        .expect("duplicate");
    assert_eq!(
        inserted.admission().position(),
        duplicate.admission().position()
    );
    assert_eq!(
        duplicate.admission().disposition(),
        AdmissionDisposition::Duplicate
    );
    let provenance = block_on(storage.query_provenance(
        *inserted.admission().event_id(),
        EventQueryBounds::first(10).expect("bounds"),
    ))
    .expect("provenance query");
    assert_eq!(provenance.items().len(), 2);

    let (collision_engine, _) = setup_engine_with_ids(Arc::new(ConstantIds(9)));
    block_on(collision_engine.ingest(observed(SIGNATURE, 20), &RegistryPolicy::visible()))
        .expect("first identity use");
    assert_eq!(
        block_on(collision_engine.ingest(observed(SIGNATURE, 21), &RegistryPolicy::visible())),
        Err(Error::StorageConflict)
    );

    let invalid_signature = format!("0{}", &SIGNATURE[1..]);
    let batch = block_on(engine.ingest_batch(
        vec![
            observed(SIGNATURE, 30),
            observed(&invalid_signature, 31),
            observed(SIGNATURE, 32),
        ],
        &RegistryPolicy::visible(),
    ));
    assert_eq!(batch.accepted(), 2);
    assert_eq!(batch.rejected(), 1);
    assert_eq!(batch.outcomes()[1], Err(Error::VerificationFailed));
}
