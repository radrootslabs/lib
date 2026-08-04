use super::*;

#[test]
fn admitted_contract_resolves_registry_entry() {
    use nostr::secp256k1::Message;
    use nostr::{Keys, SECP256K1};
    use radroots_event::{
        envelope::EventEnvelopeParts, envelope::kind::KIND_DELETION_REQUEST,
        wire::compute_canonical_nip01_event_id,
    };
    use radroots_test_fixtures::FIXTURE_ALICE_SECRET_KEY_HEX;

    let keys =
        Keys::parse(FIXTURE_ALICE_SECRET_KEY_HEX).expect("fixed fixture secret key must parse");
    let author = keys.public_key().to_string();
    let created_at = 1_800_000_300;
    let tags = vec![vec!["e".to_string(), "a".repeat(64)]];
    let content = "superseded";
    let id = compute_canonical_nip01_event_id(
        author.as_str(),
        created_at,
        KIND_DELETION_REQUEST,
        &tags,
        content,
    )
    .expect("canonical deletion request id");
    let nostr_id = nostr::EventId::from_hex(&id.to_hex()).expect("Nostr event id");
    let message = Message::from_digest(nostr_id.to_bytes());
    let signature = SECP256K1.sign_schnorr_no_aux_rand(
        &message,
        &nostr::secp256k1::Keypair::from_secret_key(SECP256K1, keys.secret_key()),
    );
    let event = EventEnvelope::new(EventEnvelopeParts {
        id: id.into_string(),
        author,
        created_at,
        kind: KIND_DELETION_REQUEST,
        tags,
        content: content.to_string(),
        sig: signature.to_string(),
    })
    .expect("valid deletion request envelope");

    let admitted =
        verify_and_admit_nip09_deletion_request_event(event).expect("admitted deletion request");

    assert_eq!(
        admitted.contract().id,
        "radroots.social.deletion_request.v1"
    );
    assert_eq!(admitted.contract().kind, KIND_DELETION_REQUEST);
}

#[test]
fn stable_error_codes_delegate_to_verification_and_projection() {
    let verification = RadrootsNip09DeletionAdmissionError::Nip01Verification(
        RadrootsNip01VerificationError::SignatureInvalid,
    );
    let projection = RadrootsNip09DeletionAdmissionError::Projection(
        RadrootsNip09DeletionProjectionError::TargetMissing,
    );
    assert_eq!(verification.code(), "signature_invalid");
    assert_eq!(projection.code(), "deletion_target_missing");
    assert!(!verification.to_string().is_empty());
    assert!(!projection.to_string().is_empty());
}
