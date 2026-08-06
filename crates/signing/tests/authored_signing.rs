use nostr::{EventBuilder, JsonUtil, Keys, Kind as NostrKind, SecretKey, Timestamp};
use radroots_event::{
    GenericEventDraft,
    contract::AuthorRole,
    food::availability::{
        FoodAvailabilityDetails, FoodAvailabilityDetailsParts, FoodAvailabilityStatus, FoodContent,
        FoodCurrency, FoodIdentifier, FoodPrice, FoodPublishedAt, FoodText, FoodUnit,
    },
    wire::Nip01EventWire,
};
use radroots_event_codec::authoring::AuthoredEventPlan;
use radroots_identity::{AccountId, PublicKey};
use radroots_protocol::runtime::v1::OperationId;
use radroots_signing::{
    Actor, AuthoredArtifactId, CurrentAuthoringAuthority, CurrentAuthoringDecision, Error,
    SignReceipt, SignRequest, SigningIntentId, SigningOperationId,
    actor::ActorSource,
    authorization::ManagedSigningPolicy,
    error::Kind,
    recovery::{RecoveryDisposition, RemoteEffect, ReplayCapability, recovery_disposition},
    request::{CancellationPolicy, CancellationSignal, ProgressObserver, SignPolicy},
    status::{SignProgress, SignProgressStage},
};

const SECRET: &str = "7e0112ad58b2d2d13fb80532625195dc169b86d72b0e1db48347837a785cae90";
const CREATED_AT: u64 = 1_700_000_000;
const DEADLINE_MS: u64 = 1_700_000_100_000;

#[derive(Clone, Copy)]
struct FixedAuthority(CurrentAuthoringDecision);

impl CurrentAuthoringAuthority for FixedAuthority {
    fn evaluate(&self, _plan: &AuthoredEventPlan) -> CurrentAuthoringDecision {
        self.0
    }
}

fn keys() -> Keys {
    Keys::new(SecretKey::from_hex(SECRET).expect("secret fixture"))
}

fn public_key() -> PublicKey {
    PublicKey::from_hex(&keys().public_key().to_hex()).expect("public key")
}

fn plan() -> AuthoredEventPlan {
    AuthoredEventPlan::from_generic(
        GenericEventDraft::new(
            "radroots.social.geochat.v1",
            20_000,
            CREATED_AT,
            Vec::new(),
            "exact signing plan",
            public_key().to_hex(),
        )
        .expect("generic draft"),
    )
    .expect("authored plan")
}

fn actor(source: ActorSource, roles: impl IntoIterator<Item = AuthorRole>) -> Actor {
    Actor::new(public_key(), source, roles).expect("actor")
}

fn intent(operation: u8, artifact: u8) -> SigningIntentId {
    SigningIntentId::new(
        SigningOperationId::new([operation; 16]).expect("operation ID"),
        AuthoredArtifactId::new([artifact; 16]).expect("artifact ID"),
    )
}

fn policy() -> SignPolicy {
    SignPolicy::new(DEADLINE_MS, CancellationPolicy::LocalCooperative).expect("policy")
}

fn request() -> SignRequest {
    SignRequest::new(
        OperationId::SyncPush,
        intent(1, 2),
        actor(ActorSource::ExplicitPublicKey, [AuthorRole::Any]),
        plan(),
        policy(),
    )
    .expect("request")
}

fn signed_event() -> radroots_event::SignedEvent {
    signed_event_with(
        &keys(),
        20_000,
        "exact signing plan",
        CREATED_AT,
        Vec::new(),
    )
}

fn signed_event_with(
    signer: &Keys,
    kind: u16,
    content: &str,
    created_at: u64,
    tags: Vec<nostr::Tag>,
) -> radroots_event::SignedEvent {
    let event = EventBuilder::new(NostrKind::Custom(kind), content)
        .tags(tags)
        .custom_created_at(Timestamp::from_secs(created_at))
        .sign_with_keys(signer)
        .expect("signed fixture");
    let raw = event.as_json();
    let wire = Nip01EventWire::parse_json(&raw).expect("wire");
    radroots_event::SignedEvent::from_wire_verified_id(wire, raw).expect("signed event")
}

#[test]
fn authorization_decisions_are_current_and_explicit() {
    let base_actor = actor(ActorSource::ExplicitPublicKey, [AuthorRole::Any]);
    let base_plan = plan();
    for decision in [
        CurrentAuthoringDecision::Blocked { code: "blocked" },
        CurrentAuthoringDecision::Revoked { code: "revoked" },
    ] {
        let error = SignRequest::new_with_authority(
            OperationId::SyncPush,
            intent(1, 2),
            base_actor.clone(),
            base_plan.clone(),
            policy(),
            &FixedAuthority(decision),
        )
        .expect_err("denied decision");
        assert_eq!(error.kind(), Kind::AuthorizationDenied);
    }

    let deprecated = FixedAuthority(CurrentAuthoringDecision::AllowedDeprecated {
        warning_code: "deprecated",
    });
    assert_eq!(
        SignRequest::new_with_authority(
            OperationId::SyncPush,
            intent(1, 2),
            base_actor.clone(),
            base_plan.clone(),
            policy(),
            &deprecated,
        )
        .expect_err("deprecated requires opt in")
        .kind(),
        Kind::AuthorizationDenied
    );
    let allowed = SignRequest::new_with_authority(
        OperationId::SyncPush,
        intent(1, 2),
        base_actor,
        base_plan,
        policy().allowing_deprecated(),
        &deprecated,
    )
    .expect("explicitly allowed deprecated plan");
    assert!(matches!(
        allowed.authorization(),
        CurrentAuthoringDecision::AllowedDeprecated { .. }
    ));
}

#[test]
fn authorization_enforces_key_role_and_host_provenance() {
    let wrong_key =
        PublicKey::from_hex("e0266e3cfb0d2886f91c73f5f868f3b98273713e5fcd97c081663f5518a4b3af")
            .expect("other public key");
    let wrong_actor =
        Actor::new(wrong_key, ActorSource::ExplicitPublicKey, [AuthorRole::Any]).expect("actor");
    assert_eq!(
        SignRequest::new(
            OperationId::SyncPush,
            intent(1, 2),
            wrong_actor,
            plan(),
            policy(),
        )
        .expect_err("key drift")
        .kind(),
        Kind::AuthorizationDenied
    );

    let account = AccountId::from_hex(&public_key().to_hex()).expect("account ID");
    let local_actor = actor(ActorSource::LocalAccount(account), [AuthorRole::Any]);
    SignRequest::new(
        OperationId::SyncPush,
        intent(1, 2),
        local_actor,
        plan(),
        policy().with_managed_signing_policy(ManagedSigningPolicy::LocalAccountOnly),
    )
    .expect("local account is allowed");
    assert_eq!(
        SignRequest::new(
            OperationId::SyncPush,
            intent(1, 2),
            actor(ActorSource::ExplicitPublicKey, [AuthorRole::Any]),
            plan(),
            policy().with_managed_signing_policy(ManagedSigningPolicy::AccountBackedOnly),
        )
        .expect_err("unmanaged explicit key")
        .kind(),
        Kind::AuthorizationDenied
    );

    let food = FoodAvailabilityDetails::new(FoodAvailabilityDetailsParts {
        content: FoodContent::new("Carrots available.").unwrap(),
        identifier: FoodIdentifier::parse("carrots").unwrap(),
        title: FoodText::new("Carrots").unwrap(),
        summary: FoodText::new("Fresh bunches").unwrap(),
        published_at: FoodPublishedAt::new(CREATED_AT).unwrap(),
        location: FoodText::new("Saanich").unwrap(),
        price: FoodPrice::new("3", FoodCurrency::parse("CAD").unwrap(), FoodUnit::Pound).unwrap(),
        quantity: None,
        status: FoodAvailabilityStatus::Active,
        images: Vec::new(),
    })
    .unwrap();
    let seller_plan =
        AuthoredEventPlan::from_food_availability(&food, CREATED_AT, public_key().to_hex())
            .unwrap();
    assert_eq!(
        SignRequest::new(
            OperationId::SyncPush,
            intent(1, 3),
            actor(ActorSource::ExplicitPublicKey, [AuthorRole::Buyer]),
            seller_plan,
            policy(),
        )
        .expect_err("role drift")
        .kind(),
        Kind::AuthorizationDenied
    );
}

#[test]
fn request_identity_deadline_and_cancellation_are_exact() {
    let request = request();
    let replay = request.clone();
    assert_eq!(request.operation_kind(), OperationId::SyncPush);
    assert_eq!(request.intent_id(), intent(1, 2));
    assert_eq!(request.actor().public_key(), public_key());
    assert_eq!(request.plan().digest(), plan().digest());
    assert_eq!(request.policy(), policy());
    assert!(!request.cancellation_signal().is_cancelled());
    assert_eq!(request.signer_request_id(), replay.signer_request_id());
    let other_artifact = SignRequest::new(
        OperationId::SyncPush,
        intent(1, 3),
        actor(ActorSource::ExplicitPublicKey, [AuthorRole::Any]),
        plan(),
        policy(),
    )
    .unwrap();
    assert_ne!(
        request.signer_request_id(),
        other_artifact.signer_request_id()
    );
    assert!(request.ensure_active(DEADLINE_MS - 1).is_ok());
    assert_eq!(
        request.ensure_active(DEADLINE_MS).unwrap_err().kind(),
        Kind::DeadlineExceeded
    );

    let signal = CancellationSignal::new();
    let cancelled = request.clone().with_cancellation_signal(signal.clone());
    signal.cancel();
    assert_eq!(
        cancelled.ensure_active(DEADLINE_MS - 1).unwrap_err().kind(),
        Kind::SignerCancelled
    );

    struct Counter(std::sync::atomic::AtomicUsize);
    impl ProgressObserver for Counter {
        fn on_progress(&self, _progress: &SignProgress) {
            self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    }
    let observer = std::sync::Arc::new(Counter(std::sync::atomic::AtomicUsize::new(0)));
    let observed = request.with_progress_observer(observer.clone());
    observed.report_progress(&SignProgress::stage(SignProgressStage::Validating).unwrap());
    assert_eq!(observer.0.load(std::sync::atomic::Ordering::Relaxed), 1);
}

#[test]
fn receipt_requires_exact_fields_and_a_valid_schnorr_signature() {
    let request = request();
    let receipt = SignReceipt::from_signed_event(&request, signed_event(), DEADLINE_MS - 1)
        .expect("verified receipt");
    assert_eq!(receipt.intent_id(), request.intent_id());
    assert_eq!(receipt.signer_request_id(), request.signer_request_id());
    assert_eq!(receipt.operation_kind(), OperationId::SyncPush);
    assert_eq!(receipt.completed_at_unix_ms(), DEADLINE_MS - 1);
    assert_eq!(receipt.signed_event().id(), signed_event().id());

    let other_keys = Keys::generate();
    let mismatches = [
        signed_event_with(
            &other_keys,
            20_000,
            "exact signing plan",
            CREATED_AT,
            Vec::new(),
        ),
        signed_event_with(
            &keys(),
            20_000,
            "exact signing plan",
            CREATED_AT + 1,
            Vec::new(),
        ),
        signed_event_with(
            &keys(),
            20_001,
            "exact signing plan",
            CREATED_AT,
            Vec::new(),
        ),
        signed_event_with(
            &keys(),
            20_000,
            "exact signing plan",
            CREATED_AT,
            vec![nostr::Tag::parse(["t", "mismatch"]).expect("tag")],
        ),
        signed_event_with(&keys(), 20_000, "different content", CREATED_AT, Vec::new()),
    ];
    for mismatch in mismatches {
        assert_eq!(
            SignReceipt::from_signed_event(&request, mismatch, DEADLINE_MS - 1)
                .expect_err("mismatched exact plan must fail")
                .kind(),
            Kind::SignerOutputInvalid
        );
    }

    let valid = signed_event();
    let mut wire = valid.wire().clone();
    wire.sig = "f".repeat(128);
    let raw = serde_json::to_string(&wire).expect("raw event");
    let invalid = radroots_event::SignedEvent::from_wire_verified_id(wire, raw)
        .expect("ID-valid event with hostile signature");
    assert_eq!(
        SignReceipt::from_signed_event(&request, invalid, DEADLINE_MS - 1)
            .expect_err("invalid signature")
            .kind(),
        Kind::SignerOutputInvalid
    );
}

#[test]
fn uncertain_remote_effects_never_become_unsafe_automatic_retries() {
    let uncertain = Error::new(Kind::SignerTimeout).with_possible_remote_effect();
    assert_eq!(uncertain.remote_effect(), RemoteEffect::MayHaveOccurred);
    assert_eq!(
        recovery_disposition(
            ReplayCapability::ExactReplayByRequestId,
            uncertain.remote_effect(),
            uncertain.retryable(),
        ),
        RecoveryDisposition::RetryExactRequest
    );
    assert_eq!(
        recovery_disposition(
            ReplayCapability::NonReplayable,
            uncertain.remote_effect(),
            uncertain.retryable(),
        ),
        RecoveryDisposition::Indeterminate
    );
    assert_eq!(
        recovery_disposition(ReplayCapability::LocalReplaySafe, RemoteEffect::None, true,),
        RecoveryDisposition::RetryLocal
    );
}
