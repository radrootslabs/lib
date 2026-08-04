use futures_executor::block_on;
use radroots_event::{
    EventId, SignedEvent, VerifiedEvent,
    admission::{AdmissionPolicy, RawEvent, VisibilityPolicy, VisibleEvent},
    wire::Nip01EventWire,
};
use radroots_storage::{
    Error, EventStore,
    event::{
        AdmissionDisposition, AdmissionReceipt, AdmissionStage, EventAdmission, EventPage,
        EventPosition, EventQuery, EventQueryBounds, EventSequence, SourceGeneration,
        StoredEventProvenance, StoredRawEvent, StoredVerifiedEvent, StoredVisibleEvent,
    },
    status::{EventStoreHealth, EventStoreMode, EventStoreStatus},
};
use radroots_transport::{
    BoxFuture, Target, TransportId,
    source::{EventProvenance, ObservedEvent},
};
use std::sync::Mutex;

#[derive(Clone)]
struct Entry {
    position: EventPosition,
    admission: EventAdmission,
    provenance: Vec<EventProvenance>,
}

struct MemoryEventStore {
    generation: SourceGeneration,
    entries: Mutex<Vec<Entry>>,
}

impl MemoryEventStore {
    fn new() -> Self {
        Self {
            generation: SourceGeneration::new([7; 32]).expect("non-zero generation"),
            entries: Mutex::new(Vec::new()),
        }
    }

    fn selected(&self, query: &EventQuery) -> Result<Vec<Entry>, Error> {
        if let Some(cursor) = query.bounds().cursor()
            && cursor.generation() != self.generation
        {
            return Err(Error::SourceGenerationChanged);
        }
        let after = query
            .bounds()
            .cursor()
            .map_or(0, |cursor| cursor.sequence().get());
        Ok(self
            .entries
            .lock()
            .expect("test store lock")
            .iter()
            .filter(|entry| {
                entry.position.sequence().get() > after && query.selects(entry.admission.event_id())
            })
            .take(usize::from(query.bounds().limit()))
            .cloned()
            .collect())
    }
}

impl EventStore for MemoryEventStore {
    fn status(&self) -> BoxFuture<'_, Result<EventStoreStatus, Error>> {
        Box::pin(async move {
            let entries = self.entries.lock().expect("test store lock");
            let raw = entries.len() as u64;
            let verified = entries
                .iter()
                .filter(|entry| entry.admission.stage() >= AdmissionStage::Verified)
                .count() as u64;
            let visible = entries
                .iter()
                .filter(|entry| entry.admission.stage() == AdmissionStage::Visible)
                .count() as u64;
            EventStoreStatus::new(
                self.generation,
                EventStoreMode::ReadWrite,
                EventStoreHealth::Available,
                raw,
                verified,
                visible,
            )
        })
    }

    fn admit(&self, admission: EventAdmission) -> BoxFuture<'_, Result<AdmissionReceipt, Error>> {
        Box::pin(async move {
            let mut entries = self.entries.lock().expect("test store lock");
            if let Some(entry) = entries
                .iter_mut()
                .find(|entry| entry.admission.event_id() == admission.event_id())
            {
                if entry.admission.event() != admission.event() {
                    return Err(Error::EventConflict);
                }
                if admission.stage() < entry.admission.stage() {
                    return Err(Error::AdmissionRegression);
                }
                let disposition = if admission.stage() == entry.admission.stage() {
                    AdmissionDisposition::Duplicate
                } else {
                    AdmissionDisposition::Advanced
                };
                if !entry.provenance.contains(admission.provenance()) {
                    entry.provenance.push(admission.provenance().clone());
                }
                entry.admission = admission;
                return Ok(AdmissionReceipt::new(
                    *entry.admission.event_id(),
                    entry.position,
                    entry.admission.stage(),
                    disposition,
                ));
            }

            let sequence = EventSequence::new(entries.len() as u64 + 1)?;
            let position = EventPosition::new(self.generation, sequence);
            let receipt = AdmissionReceipt::new(
                *admission.event_id(),
                position,
                admission.stage(),
                AdmissionDisposition::Inserted,
            );
            let provenance = vec![admission.provenance().clone()];
            entries.push(Entry {
                position,
                admission,
                provenance,
            });
            Ok(receipt)
        })
    }

    fn query_raw(
        &self,
        query: EventQuery,
    ) -> BoxFuture<'_, Result<EventPage<StoredRawEvent>, Error>> {
        Box::pin(async move {
            let items = self
                .selected(&query)?
                .into_iter()
                .map(|entry| {
                    StoredRawEvent::new(
                        entry.position,
                        entry.admission.event().clone(),
                        entry.admission.stage(),
                    )
                })
                .collect();
            EventPage::new(self.generation, items, None, query.bounds())
        })
    }

    fn query_verified(
        &self,
        query: EventQuery,
    ) -> BoxFuture<'_, Result<EventPage<StoredVerifiedEvent>, Error>> {
        Box::pin(async move {
            let items = self
                .selected(&query)?
                .into_iter()
                .filter_map(|entry| {
                    (entry.admission.stage() >= AdmissionStage::Verified).then(|| {
                        StoredVerifiedEvent::new(entry.position, entry.admission.event().clone())
                    })
                })
                .collect();
            EventPage::new(self.generation, items, None, query.bounds())
        })
    }

    fn query_visible(
        &self,
        query: EventQuery,
    ) -> BoxFuture<'_, Result<EventPage<StoredVisibleEvent>, Error>> {
        Box::pin(async move {
            let items = self
                .selected(&query)?
                .into_iter()
                .filter_map(|entry| {
                    (entry.admission.stage() == AdmissionStage::Visible).then(|| {
                        StoredVisibleEvent::new(entry.position, entry.admission.event().clone())
                    })
                })
                .collect();
            EventPage::new(self.generation, items, None, query.bounds())
        })
    }

    fn query_provenance(
        &self,
        event_id: EventId,
        bounds: EventQueryBounds,
    ) -> BoxFuture<'_, Result<EventPage<StoredEventProvenance>, Error>> {
        Box::pin(async move {
            let entries = self.entries.lock().expect("test store lock");
            let entry = entries
                .iter()
                .find(|entry| entry.admission.event_id() == &event_id)
                .ok_or(Error::EventNotFound)?;
            let items = entry
                .provenance
                .iter()
                .take(usize::from(bounds.limit()))
                .cloned()
                .map(|provenance| StoredEventProvenance::new(entry.position, provenance))
                .collect();
            EventPage::new(self.generation, items, None, bounds)
        })
    }
}

struct Allow;

impl radroots_event::admission::SignatureVerifier for Allow {
    fn verify_signature(
        &self,
        _event: &radroots_event::Event,
    ) -> Result<(), radroots_event::Error> {
        Ok(())
    }
}

impl AdmissionPolicy for Allow {
    type Error = core::convert::Infallible;

    fn policy_id(&self) -> &'static str {
        "test.storage.admission.v1"
    }

    fn admit(
        &self,
        _event: &radroots_event::admission::ContractValidatedEvent,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl VisibilityPolicy for Allow {
    type Error = core::convert::Infallible;

    fn policy_id(&self) -> &'static str {
        "test.storage.visibility.v1"
    }

    fn make_visible(
        &self,
        _event: &radroots_event::admission::AdmittedEvent,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}

fn signed_event_with_signature(signature_byte: &str) -> SignedEvent {
    let mut wire = Nip01EventWire {
        id: "0".repeat(64),
        pubkey: "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df".to_owned(),
        created_at: 1_800_000_100,
        kind: 0,
        tags: vec![],
        content: "{\"display_name\":\"Moss Street Farm\",\"bot\":false}".to_owned(),
        sig: signature_byte.repeat(64),
        extra: Default::default(),
    };
    wire.id = wire
        .computed_event_id()
        .expect("canonical event id")
        .to_hex();
    let raw_json = serde_json::json!({
        "id": &wire.id,
        "pubkey": &wire.pubkey,
        "created_at": wire.created_at,
        "kind": wire.kind,
        "tags": &wire.tags,
        "content": &wire.content,
        "sig": &wire.sig,
    })
    .to_string();
    SignedEvent::from_wire_verified_id(wire, raw_json).expect("signed event")
}

fn signed_event() -> SignedEvent {
    signed_event_with_signature("42")
}

fn observed(event: SignedEvent, observed_at: u64) -> ObservedEvent {
    let target = Target::new(TransportId::NOSTR, "wss://relay.example").expect("relay target");
    let provenance = EventProvenance::new(
        TransportId::NOSTR,
        target.fingerprint().clone(),
        observed_at,
    )
    .expect("provenance");
    ObservedEvent::new(event, provenance)
}

fn verified(event: &SignedEvent) -> VerifiedEvent {
    RawEvent::new(event.envelope().clone())
        .verify_id()
        .expect("event id")
        .verify_signature(&Allow)
        .expect("signature")
}

fn visible(event: &SignedEvent) -> VisibleEvent {
    verified(event)
        .validate_contract()
        .expect("contract")
        .admit_with(&Allow)
        .expect("admission")
        .make_visible_with(&Allow)
        .expect("visibility")
}

#[test]
fn event_store_is_dyn_compatible_and_enforces_monotonic_admission() {
    let store = MemoryEventStore::new();
    let dynamic: &dyn EventStore = &store;
    let event = signed_event();

    let inserted = block_on(dynamic.admit(EventAdmission::raw(observed(event.clone(), 1))))
        .expect("raw insertion");
    assert_eq!(inserted.disposition(), AdmissionDisposition::Inserted);
    assert_eq!(inserted.stage(), AdmissionStage::Raw);

    let advanced = block_on(
        dynamic.admit(
            EventAdmission::verified(observed(event.clone(), 2), verified(&event))
                .expect("verified admission"),
        ),
    )
    .expect("verified advancement");
    assert_eq!(advanced.disposition(), AdmissionDisposition::Advanced);
    assert_eq!(advanced.position(), inserted.position());

    let visible_event = visible(&event);
    let advanced = block_on(
        dynamic.admit(
            EventAdmission::visible(observed(event.clone(), 3), visible_event)
                .expect("visible admission"),
        ),
    )
    .expect("visible advancement");
    assert_eq!(advanced.stage(), AdmissionStage::Visible);

    let duplicate = block_on(
        dynamic.admit(
            EventAdmission::visible(observed(event.clone(), 3), visible(&event))
                .expect("duplicate visible admission"),
        ),
    )
    .expect("idempotent duplicate");
    assert_eq!(duplicate.disposition(), AdmissionDisposition::Duplicate);

    assert_eq!(
        block_on(dynamic.admit(EventAdmission::raw(observed(event, 4)))),
        Err(Error::AdmissionRegression)
    );

    let conflicting = signed_event_with_signature("43");
    assert_eq!(
        EventAdmission::verified(observed(conflicting.clone(), 5), verified(&signed_event())),
        Err(Error::AdmissionEventMismatch)
    );
    assert_eq!(
        block_on(dynamic.admit(EventAdmission::raw(observed(conflicting, 6)))),
        Err(Error::EventConflict)
    );
}

#[test]
fn event_queries_preserve_stage_generation_bounds_and_provenance() {
    let store = MemoryEventStore::new();
    let event = signed_event();
    let event_id = *event.id();
    block_on(
        store.admit(
            EventAdmission::visible(observed(event.clone(), 5), visible(&event))
                .expect("visible admission"),
        ),
    )
    .expect("insert visible event");

    let bounds = EventQueryBounds::first(1).expect("query bounds");
    let query = EventQuery::for_ids(bounds, vec![event_id]).expect("id query");
    let raw = block_on(store.query_raw(query.clone())).expect("raw page");
    let verified = block_on(store.query_verified(query.clone())).expect("verified page");
    let visible = block_on(store.query_visible(query)).expect("visible page");
    let provenance = block_on(store.query_provenance(event_id, bounds)).expect("provenance page");

    assert_eq!(raw.items().len(), 1);
    assert_eq!(raw.items()[0].stage(), AdmissionStage::Visible);
    assert_eq!(verified.items().len(), 1);
    assert_eq!(visible.items().len(), 1);
    assert_eq!(provenance.items().len(), 1);
    assert_eq!(raw.generation(), store.generation);

    let status = block_on(store.status()).expect("status");
    assert_eq!(status.raw_events(), 1);
    assert_eq!(status.verified_events(), 1);
    assert_eq!(status.visible_events(), 1);
}

#[test]
fn bounds_generations_and_status_reject_invalid_state() {
    assert_eq!(
        SourceGeneration::new([0; 32]),
        Err(Error::InvalidSourceGeneration)
    );
    assert_eq!(EventSequence::new(0), Err(Error::InvalidEventSequence));
    assert_eq!(
        EventQueryBounds::first(0),
        Err(Error::InvalidEventQueryLimit)
    );
    assert_eq!(
        EventStoreStatus::new(
            SourceGeneration::new([1; 32]).expect("generation"),
            EventStoreMode::ReadOnly,
            EventStoreHealth::Degraded,
            1,
            2,
            0,
        ),
        Err(Error::CorruptStoredEvent)
    );
}

#[test]
fn event_value_models_cover_bounds_accessors_and_durable_reconstruction() {
    let generation = SourceGeneration::new([1; 32]).expect("generation");
    let other_generation = SourceGeneration::new([2; 32]).expect("other generation");
    let sequence = EventSequence::new(1).expect("sequence");
    let position = EventPosition::new(generation, sequence);
    assert_eq!(generation.as_bytes(), &[1; 32]);
    assert_eq!(sequence.get(), 1);
    assert_eq!(position.generation(), generation);
    assert_eq!(position.sequence(), sequence);

    assert_eq!(
        EventQueryBounds::first(radroots_storage::event::EVENT_QUERY_LIMIT_MAX + 1),
        Err(Error::InvalidEventQueryLimit)
    );
    let bounds = EventQueryBounds::first(1).expect("bounds").after(position);
    assert_eq!(bounds.limit(), 1);
    assert_eq!(bounds.cursor(), Some(position));
    let event = signed_event();
    let event_id = *event.id();
    assert_eq!(
        EventQuery::for_ids(bounds, Vec::new()),
        Err(Error::EmptyEventQueryIds)
    );
    assert_eq!(
        EventQuery::for_ids(bounds, vec![event_id, event_id]),
        Err(Error::DuplicateEventQueryId)
    );
    assert_eq!(
        EventQuery::for_ids(
            bounds,
            vec![event_id; radroots_storage::event::EVENT_QUERY_ID_MAX + 1]
        ),
        Err(Error::TooManyEventQueryIds)
    );
    let all = EventQuery::all(bounds);
    assert!(all.event_ids().is_empty());
    assert!(all.selects(&event_id));
    let selected = EventQuery::for_ids(bounds, vec![event_id]).expect("selected query");
    assert_eq!(selected.bounds(), bounds);
    assert_eq!(selected.event_ids(), &[event_id]);
    assert!(selected.selects(&event_id));
    let other_event_id = EventId::parse("f".repeat(64)).expect("other event id");
    assert!(!selected.selects(&other_event_id));

    let raw_admission = EventAdmission::raw(observed(event.clone(), 1));
    assert_eq!(raw_admission.stage(), AdmissionStage::Raw);
    assert_eq!(raw_admission.event(), &event);
    assert_eq!(raw_admission.event_id(), &event_id);
    assert_eq!(raw_admission.provenance().observed_at_unix_ms(), 1);
    assert!(raw_admission.verified_event().is_none());
    assert!(raw_admission.visible_event().is_none());
    let verified_admission =
        EventAdmission::verified(observed(event.clone(), 2), verified(&event)).expect("verified");
    assert!(verified_admission.verified_event().is_some());
    assert!(verified_admission.visible_event().is_none());
    let visible_admission =
        EventAdmission::visible(observed(event.clone(), 3), visible(&event)).expect("visible");
    assert!(visible_admission.verified_event().is_some());
    assert!(visible_admission.visible_event().is_some());
    assert_eq!(
        EventAdmission::visible(
            observed(signed_event_with_signature("43"), 4),
            visible(&event)
        ),
        Err(Error::AdmissionEventMismatch)
    );

    let receipt = AdmissionReceipt::new(
        event_id,
        position,
        AdmissionStage::Raw,
        AdmissionDisposition::Inserted,
    );
    assert_eq!(receipt.event_id(), &event_id);
    assert_eq!(receipt.position(), position);
    assert_eq!(receipt.stage(), AdmissionStage::Raw);
    assert_eq!(receipt.disposition(), AdmissionDisposition::Inserted);
    let stored_raw = StoredRawEvent::new(position, event.clone(), AdmissionStage::Raw);
    assert_eq!(stored_raw.position(), position);
    assert_eq!(stored_raw.event(), &event);
    assert_eq!(stored_raw.stage(), AdmissionStage::Raw);
    let stored_verified = StoredVerifiedEvent::new(position, event.clone());
    assert_eq!(stored_verified.position(), position);
    assert_eq!(stored_verified.event(), &event);
    let stored_visible = StoredVisibleEvent::new(position, event);
    assert_eq!(stored_visible.position(), position);
    assert_eq!(stored_visible.event().id(), &event_id);

    assert_eq!(
        EventPage::new(
            generation,
            vec![1, 2],
            None,
            EventQueryBounds::first(1).unwrap()
        ),
        Err(Error::EventPageLimitExceeded)
    );
    assert_eq!(
        EventPage::<u8>::new(
            generation,
            vec![],
            Some(EventPosition::new(other_generation, sequence)),
            EventQueryBounds::first(1).unwrap(),
        ),
        Err(Error::CursorGenerationMismatch)
    );
    let page = EventPage::new(generation, vec![1], Some(position), bounds).expect("page");
    assert_eq!(page.generation(), generation);
    assert_eq!(page.items(), &[1]);
    assert_eq!(page.next_cursor(), Some(position));

    let provenance = observed(signed_event(), 5).provenance().clone();
    let stored = StoredEventProvenance::new(position, provenance.clone());
    assert_eq!(stored.position(), position);
    assert_eq!(stored.provenance(), &provenance);
    let reconstructed = StoredEventProvenance::from_stored_parts(
        position,
        "nostr",
        provenance.target().as_str(),
        5,
        Some("cursor"),
    )
    .expect("stored provenance");
    assert_eq!(
        reconstructed.provenance().cursor().unwrap().as_str(),
        "cursor"
    );
    for (transport, target, observed_at, cursor) in [
        ("BAD ID", provenance.target().as_str(), 5, None),
        ("nostr", "bad", 5, None),
        ("nostr", provenance.target().as_str(), 0, None),
        ("nostr", provenance.target().as_str(), 5, Some(" bad")),
    ] {
        assert_eq!(
            StoredEventProvenance::from_stored_parts(
                position,
                transport,
                target,
                observed_at,
                cursor,
            ),
            Err(Error::CorruptStoredEvent)
        );
    }
}
