use super::*;
use radroots_event::admission::RawEvent;
use radroots_event_codec::verify::{self, Nip01SignatureVerifier};
use radroots_storage::event::{AdmissionStage, EventAdmission};
use radroots_transport::source::{EventProvenance, ObservedEvent};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Variant {
    Reordered,
    Limit,
    Extra,
    Signature,
    Duplicate,
    Oversize,
}

impl Variant {
    fn permits_admission(self) -> bool {
        matches!(self, Self::Reordered | Self::Limit)
    }
}

fn verified_admission(event: SignedEvent) -> EventAdmission {
    let target = Target::new(TransportId::NOSTR, "wss://relay.example").expect("target");
    let verified = verify::signature(
        verify::id(RawEvent::new(event.envelope().clone())).expect("identifier"),
        &Nip01SignatureVerifier,
    )
    .expect("actual Schnorr signature");
    let observed = ObservedEvent::new(
        event,
        EventProvenance::new(
            TransportId::NOSTR,
            target.fingerprint().clone(),
            1_800_000_200_000,
        )
        .expect("provenance"),
    );
    EventAdmission::verified(observed, verified).expect("verified incoming event")
}

fn alternate(original: &SignedEvent, variant: Variant) -> SignedEvent {
    let mut value: serde_json::Value =
        serde_json::from_str(original.raw_json()).expect("wire JSON");
    match variant {
        Variant::Extra => value["future_attribute"] = serde_json::json!("retained extension"),
        Variant::Signature => {
            value["sig"] = serde_json::json!(
                Secp256k1::new()
                    .sign_schnorr_with_aux_rand(
                        &Message::from_digest(*original.id().as_bytes()),
                        &signing_keypair(),
                        &[2; 32],
                    )
                    .to_string()
            );
        }
        _ => {}
    }
    let mut raw = serde_json::to_string_pretty(&value).expect("alternate serialization");
    if variant == Variant::Duplicate {
        assert_eq!(raw.pop(), Some('}'));
        raw.push_str(&format!(",\"id\":\"{}\"}}", original.id().to_hex()));
    }
    if matches!(variant, Variant::Limit | Variant::Oversize) {
        let limit = 64 * 1024 + usize::from(variant == Variant::Oversize);
        raw.insert_str(0, &" ".repeat(limit - raw.len()));
        assert_eq!(raw.len(), limit);
    }
    let wire = radroots_event::wire::Nip01EventWire::parse_json(&raw).expect("valid identifier");
    SignedEvent::from_wire_verified_id(wire, raw).expect("exact raw/wire binding")
}

async fn exercise(
    storage: Arc<dyn SyncStorage>,
    variant: Variant,
) -> (SignedEvent, SignedEvent, SyncId) {
    let sink = Arc::new(ScriptedSink::new([DeliveryBehavior::Outcomes(vec![
        DeliveryOutcome::accepted(),
    ])]));
    let signer = Arc::new(MockSigner::new(SignBehavior::Success {
        completed_at_unix_ms: 1_800_000_200_500,
    }));
    let engine = Engine::builder(
        storage.clone(),
        Arc::new(TestClock(AtomicU64::new(1_800_000_200_000))),
        Arc::new(TestIds(AtomicU64::new(10))),
        DeadlinePolicy::new(10_000, 10_000, 10_000).expect("deadlines"),
    )
    .sink(sink.clone())
    .signer(signer.clone())
    .build()
    .expect("engine");
    let push = request(119, "wss://relay.example");
    let signed = engine
        .sign_prepared(push.clone())
        .await
        .expect("actual signed artifact");
    let original = signed
        .artifact()
        .signed()
        .expect("signed bytes")
        .event()
        .clone();
    let remote = alternate(&original, variant);
    assert_ne!(remote.raw_json(), original.raw_json());
    assert_eq!(remote.id(), original.id());
    assert_eq!(
        remote.wire() == original.wire(),
        !matches!(variant, Variant::Extra | Variant::Signature)
    );
    EventStore::admit(storage.as_ref(), verified_admission(remote.clone()))
        .await
        .expect("initial canonical observation");
    // The store's strict byte-conflict invariant remains in force.
    assert_eq!(
        EventStore::admit(storage.as_ref(), verified_admission(original.clone())).await,
        Err(radroots_storage::Error::EventConflict)
    );

    let result = engine.admit_signed(push.operation_id()).await;
    if variant.permits_admission() {
        assert!(
            result
                .expect("equivalent-wire admission")
                .artifact()
                .admission_state()
                .is_admitted()
        );
        assert!(
            engine
                .admit_signed(push.operation_id())
                .await
                .expect("exact admission replay")
                .is_replay()
        );
        engine
            .deliver_push(push.operation_id())
            .await
            .expect("original delivery");
        let requests = sink.requests.lock().expect("delivery records");
        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0].payload().event().raw_json(),
            original.raw_json()
        );
    } else {
        assert_eq!(result, Err(Error::AdmissionFailed));
    }
    let status = engine
        .push_status(push.operation_id())
        .await
        .expect("status")
        .expect("retained operation");
    assert_eq!(
        status
            .artifact()
            .signed()
            .expect("original artifact")
            .event()
            .raw_json(),
        original.raw_json()
    );
    assert_eq!(signer.calls.load(Ordering::Relaxed), 1);
    let canonical = EventStore::query_raw(
        storage.as_ref(),
        EventQuery::for_ids(
            EventQueryBounds::first(1).expect("bounds"),
            vec![*original.id()],
        )
        .expect("identifier query"),
    )
    .await
    .expect("canonical event");
    assert_eq!(canonical.items().len(), 1);
    assert_eq!(canonical.items()[0].event().raw_json(), remote.raw_json());
    assert_eq!(
        canonical.items()[0].stage(),
        if variant.permits_admission() {
            AdmissionStage::Visible
        } else {
            AdmissionStage::Verified
        }
    );
    (original, remote, push.operation_id())
}

#[test]
fn memory_wire_variants_preserve_both_originals_and_refuse_profile_or_signature_changes() {
    for (generation, variant) in [
        (81, Variant::Reordered),
        (82, Variant::Limit),
        (83, Variant::Extra),
        (84, Variant::Signature),
        (85, Variant::Duplicate),
        (86, Variant::Oversize),
    ] {
        block_on(exercise(
            Arc::new(MemoryStorage::new(
                SourceGeneration::new([generation; 32]).expect("generation"),
            )),
            variant,
        ));
    }
}

#[tokio::test]
async fn sqlite_wire_variants_preserve_both_originals_across_explicit_close_and_reopen() {
    for (generation, variant) in [
        (91, Variant::Reordered),
        (92, Variant::Limit),
        (93, Variant::Extra),
        (94, Variant::Signature),
        (95, Variant::Duplicate),
        (96, Variant::Oversize),
    ] {
        let directory = tempfile::tempdir().expect("database directory");
        let paths = Paths::from_directory(directory.path()).expect("paths");
        let storage = Arc::new(
            SqliteStorage::open(
                OpenOptions::new(paths.clone(), OpenMode::Create)
                    .with_source_generation(
                        SourceGeneration::new([generation; 32]).expect("generation"),
                        1,
                    )
                    .expect("source generation"),
            )
            .await
            .expect("SQLite"),
        );
        let (original, remote, operation_id) = exercise(storage.clone(), variant).await;
        storage.close().await.expect("explicit close");
        let reopened = Arc::new(
            SqliteStorage::open(OpenOptions::new(paths, OpenMode::ReadOnly))
                .await
                .expect("read-only reopen"),
        );
        let events = reopened
            .query_raw(EventQuery::all(EventQueryBounds::first(1).expect("bounds")))
            .await
            .expect("retained canonical row");
        assert_eq!(events.items().len(), 1);
        assert_eq!(events.items()[0].event().raw_json(), remote.raw_json());
        assert_eq!(
            events.items()[0].stage(),
            if variant.permits_admission() {
                AdmissionStage::Visible
            } else {
                AdmissionStage::Verified
            }
        );
        let inspection: Arc<dyn SyncStorage> = reopened.clone();
        let reader = Engine::builder(
            inspection,
            Arc::new(TestClock(AtomicU64::new(1_800_000_201_000))),
            Arc::new(TestIds(AtomicU64::new(100))),
            DeadlinePolicy::new(10_000, 10_000, 10_000).expect("deadlines"),
        )
        .sink(Arc::new(MockSink))
        .build()
        .expect("read-only inspection without signer");
        let status = reader
            .push_status(operation_id)
            .await
            .expect("read-only status")
            .expect("original operation");
        assert_eq!(
            status
                .artifact()
                .signed()
                .expect("original retained evidence")
                .event()
                .raw_json(),
            original.raw_json()
        );
        assert_eq!(
            status.artifact().admission_state().is_admitted(),
            variant.permits_admission()
        );
        reopened.close().await.expect("close inspection");
    }
}
