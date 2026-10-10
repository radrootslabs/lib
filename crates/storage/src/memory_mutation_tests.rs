use super::*;
use futures_executor::block_on;
use radroots_event::{
    SignedEvent,
    admission::{RawEvent, SignatureVerifier},
    wire::Nip01EventWire,
};
use radroots_transport::{Target, TransportId, source::ObservedEvent};

fn admission() -> EventAdmission {
    let raw = r#"{"id":"56bfc78223bb2221bad82b539efdec1ade0f56d0eb0e1f592fd387df4b2ceee0","pubkey":"585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df","created_at":1700000001,"kind":0,"tags":[],"content":"{}","sig":"dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"}"#;
    let event =
        SignedEvent::from_wire_verified_id(Nip01EventWire::parse_json(raw).unwrap(), raw).unwrap();
    let target = Target::nostr_relay("wss://relay.example").unwrap();
    let provenance =
        EventProvenance::new(TransportId::NOSTR, target.fingerprint().clone(), 100).unwrap();
    EventAdmission::raw(ObservedEvent::new(event, provenance))
}

struct Allow;
impl SignatureVerifier for Allow {
    fn verify_signature(&self, _: &radroots_event::Event) -> Result<(), radroots_event::Error> {
        Ok(())
    }
}

#[test]
fn u64_epoch_maximum_rejects_semantic_mutation_without_partial_effects() {
    let generation = SourceGeneration::new([78; 32]).unwrap();
    let storage = MemoryStorage::new(generation);
    storage.state().unwrap().event_mutation_epoch = u64::MAX - 1;
    let original = admission();
    block_on(storage.admit(original.clone())).unwrap();
    let maximum = EventSourceToken::new(generation, u64::MAX);
    assert_eq!(block_on(storage.source_token()).unwrap(), maximum);
    block_on(storage.admit(original.clone())).unwrap();
    let verified = RawEvent::new(original.event().envelope().clone())
        .verify_id()
        .unwrap()
        .verify_signature(&Allow)
        .unwrap();
    let advanced = EventAdmission::verified(
        ObservedEvent::new(original.event().clone(), original.provenance().clone()),
        verified,
    )
    .unwrap();
    assert_eq!(
        block_on(storage.admit(advanced)),
        Err(Error::EventMutationOverflow)
    );
    assert_eq!(block_on(storage.source_token()).unwrap(), maximum);
    let state = storage.state().unwrap();
    assert_eq!(state.events.len(), 1);
    assert_eq!(state.events[0].admission.stage(), AdmissionStage::Raw);
    assert_eq!(state.events[0].provenance.len(), 1);
    let mut candidate = State {
        event_mutation_epoch: u64::MAX,
        ..State::default()
    };
    assert_eq!(
        storage.admit_locked(&mut candidate, original),
        Err(Error::EventMutationOverflow)
    );
    assert!(candidate.events.is_empty());
}
