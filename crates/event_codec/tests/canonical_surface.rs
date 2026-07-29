use radroots_event::{
    admission::{Error as VerificationError, RawEvent, SignatureVerifier},
    envelope::{EventEnvelope, EventEnvelopeParts},
};
use radroots_event_codec::{Codec, EncodeError, canonical, verify};

struct AcceptSignature;

impl SignatureVerifier for AcceptSignature {
    fn verify_signature(&self, _event: &EventEnvelope) -> Result<(), VerificationError> {
        Ok(())
    }
}

fn profile_event() -> EventEnvelope {
    EventEnvelope::new(EventEnvelopeParts {
        id: "762bee187e9e645b81ec26ade05a69b5e8398caf527be8de0d9a45311ed0c7a0"
            .to_owned(),
        author: "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df"
            .to_owned(),
        created_at: 1_800_000_100,
        kind: 0,
        tags: vec![],
        content: "{\"display_name\":\"Moss Street Farm\",\"bot\":false,\"website\":\"https://mossstreet.example\",\"picture\":42}".to_owned(),
        sig: "4290da0bb6422986647bc8cd5f63bd52d49f41e7b665d3b47105b8109183e8d596f322c531d4061df53e1d2b70fda12d5d1c14f3720d7a56d9d0a03746af5109".to_owned(),
    })
    .expect("valid fixture envelope")
}

#[test]
fn canonical_surface_preserves_every_verification_stage() {
    let event = profile_event();

    assert_eq!(canonical::id(&event).expect("canonical id"), *event.id());
    assert!(
        canonical::id_preimage(&event)
            .expect("preimage")
            .starts_with("[0,")
    );

    let id_verified = Codec::verify_id(RawEvent::new(event)).expect("id verified");
    let signature_verified =
        Codec::verify_signature(id_verified, &AcceptSignature).expect("signature verified");
    let contract_validated =
        Codec::verify_contract(signature_verified).expect("contract validated");

    assert_eq!(
        contract_validated.contract_id(),
        "radroots.profile.metadata.v1"
    );
}

#[test]
fn module_entrypoints_match_codec_convenience_type() {
    let id_verified = verify::id(RawEvent::new(profile_event())).expect("id verified");
    let signature_verified =
        verify::signature(id_verified, &AcceptSignature).expect("signature verified");
    let contract_validated = verify::contract(signature_verified).expect("contract validated");

    assert_eq!(contract_validated.event(), &profile_event());
}

#[cfg(feature = "serde_json")]
#[test]
fn decode_does_not_hide_identifier_verification() {
    let encoded = Codec::encode_event(&profile_event()).expect("encoded event");
    let mismatched = encoded.replacen(
        "762bee187e9e645b81ec26ade05a69b5e8398caf527be8de0d9a45311ed0c7a0",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        1,
    );

    let raw = Codec::decode_event(&mismatched).expect("structurally valid event");
    let error = Codec::verify_id(raw).expect_err("identifier mismatch");

    assert_eq!(error.code(), "id_mismatch");
}

#[cfg(feature = "serde_json")]
#[test]
fn compact_json_round_trip_is_deterministic() {
    let encoded = Codec::encode_event(&profile_event()).expect("encoded event");
    let decoded = Codec::decode_event(&encoded).expect("decoded event");
    let reencoded = Codec::encode_event(decoded.event()).expect("re-encoded event");

    assert_eq!(reencoded, encoded);
}

#[cfg(feature = "serde_json")]
#[test]
fn public_errors_expose_stable_concise_codes() {
    let decode = Codec::decode_event("{").expect_err("invalid JSON");

    assert_eq!(decode.code(), "invalid_json");
    assert_eq!(EncodeError::Json.code(), "json");
}
