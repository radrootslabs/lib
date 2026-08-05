use radroots_event::id::Nip01Coordinate;
use radroots_identity::PublicKey;
use radroots_nostr::{
    Error,
    event::{coordinate_from_nostr, coordinate_to_nostr},
    key::{public_key_from_npub, public_key_to_npub},
    tag::{from_parts, to_parts},
};

const PUBLIC_KEY: &str = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";

#[test]
fn public_protocol_conversions_are_canonical_and_typed() {
    let public_key = PublicKey::from_hex(PUBLIC_KEY).expect("canonical public key");
    let npub = public_key_to_npub(public_key).expect("NIP-19 public key");
    assert!(npub.starts_with("npub1"));
    assert_eq!(
        public_key_from_npub(&npub).expect("canonical public key"),
        public_key
    );

    let coordinate = Nip01Coordinate::parse(format!("30402:{PUBLIC_KEY}:listing-1"))
        .expect("canonical coordinate");
    let nostr_coordinate = coordinate_to_nostr(&coordinate).expect("Nostr coordinate");
    assert_eq!(
        coordinate_from_nostr(&nostr_coordinate).expect("canonical coordinate"),
        coordinate
    );

    let parts = vec!["t".to_owned(), "soil".to_owned()];
    let tag = from_parts(parts.clone()).expect("Nostr tag");
    assert_eq!(to_parts(&tag), parts);
    assert!(matches!(from_parts(Vec::new()), Err(Error::TagConversion)));
}

#[cfg(feature = "signing")]
#[tokio::test]
async fn public_local_signer_signs_only_the_exact_authorized_plan() {
    use radroots_event::{GenericEventDraft, contract::AuthorRole, envelope::kind::KIND_GEOCHAT};
    use radroots_event_codec::authoring::AuthoredEventPlan;
    use radroots_nostr::{key::SecretKey, signing::LocalSigner};
    use radroots_protocol::runtime::v1::OperationId;
    use radroots_signing::{
        Actor, AuthoredArtifactId, SignRequest, Signer, SigningIntentId, SigningOperationId,
        actor::ActorSource,
        request::{CancellationPolicy, SignPolicy},
    };

    let secret_key =
        SecretKey::parse("0000000000000000000000000000000000000000000000000000000000000001")
            .expect("fixture secret");
    let public_key = secret_key.public_key().expect("public key");
    assert_eq!(public_key.to_hex(), PUBLIC_KEY);

    let draft = GenericEventDraft::new(
        "radroots.social.geochat.v1",
        KIND_GEOCHAT,
        1_700_000_000,
        Vec::new(),
        "package-conformance-message",
        PUBLIC_KEY,
    )
    .expect("generic event draft");
    let plan = AuthoredEventPlan::from_generic(draft).expect("authored plan");
    let expected_id = plan.expected_event_id().to_hex();
    let actor = Actor::new(
        public_key,
        ActorSource::ExplicitPublicKey,
        [AuthorRole::Any],
    )
    .expect("authorized actor");
    let request = SignRequest::new(
        OperationId::SyncPush,
        SigningIntentId::new(
            SigningOperationId::new([1; 16]).expect("operation ID"),
            AuthoredArtifactId::new([2; 16]).expect("artifact ID"),
        ),
        actor,
        plan,
        SignPolicy::new(u64::MAX, CancellationPolicy::LocalCooperative)
            .expect("bounded signing policy"),
    )
    .expect("signing request");

    let signer = LocalSigner::new(secret_key).expect("local signer");
    let receipt = signer.sign(request).await.expect("signed receipt");

    assert_eq!(receipt.signed_event().id_str(), expected_id);
    assert_eq!(*receipt.signed_event().pubkey(), public_key);
    assert_eq!(
        receipt.signed_event().content(),
        "package-conformance-message"
    );
}
