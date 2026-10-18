use nostr::JsonUtil;
use radroots_domain::OperationId;
use radroots_event::GenericEventDraft;
use radroots_event_codec::authoring::AuthoredEventPlan;
use radroots_identity::PublicKey;
use radroots_signing::{
    issuance::*,
    provider::{ProviderCall, ProviderError},
    request::CancellationSignal,
};
use std::{num::NonZeroU64, str::FromStr};

const AUTHOR: &str = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
const CREATED: u64 = 1_700_000_000;
fn plan(content: &str, tags: Vec<Vec<String>>) -> AuthoredEventPlan {
    AuthoredEventPlan::from_generic(
        GenericEventDraft::new(
            "radroots.social.geochat.v1",
            20_000,
            CREATED,
            tags,
            content,
            AUTHOR,
        )
        .unwrap(),
    )
    .unwrap()
}
fn intent(content: &str, tags: Vec<Vec<String>>, operation: u8, digest: u8) -> SigningIntent {
    SigningIntent::new(
        OperationId::from_bytes([operation; 32]),
        [digest; 32],
        plan(content, tags),
        ArtifactRole::PublicEvent,
        None,
        ProviderCall::new(
            PublicKey::from_str(AUTHOR).unwrap(),
            NonZeroU64::new(CREATED * 1000 + 10_000).unwrap(),
            CancellationSignal::new(),
        ),
    )
    .unwrap()
}
fn signed(intent: &SigningIntent) -> String {
    let keys = nostr::Keys::new(
        nostr::SecretKey::from_slice(&{
            let mut bytes = [0; 32];
            bytes[31] = 1;
            bytes
        })
        .unwrap(),
    );
    intent.with_plan(|plan| {
        nostr::UnsignedEvent {
            id: Some(nostr::EventId::from_slice(plan.expected_event_id().as_bytes()).unwrap()),
            pubkey: keys.public_key(),
            created_at: nostr::Timestamp::from_secs(plan.created_at()),
            kind: nostr::Kind::Custom(plan.body().kind() as u16),
            tags: nostr::Tags::from_list(
                plan.body()
                    .tags()
                    .iter()
                    .cloned()
                    .map(|tag| nostr::Tag::parse(tag).unwrap())
                    .collect(),
            ),
            content: plan.body().content().into(),
        }
        .sign_with_keys(&keys)
        .unwrap()
        .as_json()
    })
}
#[test]
fn measured_complete_wire_budget_matches_the_actual_codec_for_unicode_and_escapes() {
    for content in [
        "",
        "simple",
        "café — soil",
        "quotes\" and \\ slash\n\r\t\u{0000}\u{0008}\u{000c}",
    ] {
        let intent = intent(
            content,
            vec![vec!["subject".into(), "local\" crop\t".into()]],
            7,
            8,
        );
        assert_eq!(intent.signed_json_bytes(), signed(&intent).len());
        assert_eq!(intent.binding().operation_id().as_bytes(), &[7; 32]);
    }
    let empty = intent("", vec![], 1, 2);
    let available = SIGNED_EVENT_MAX_BYTES - empty.signed_json_bytes();
    let maximum = intent(&"x".repeat(available), vec![], 1, 2);
    assert_eq!(signed(&maximum).len(), SIGNED_EVENT_MAX_BYTES);
    let too_big = SigningIntent::new(
        OperationId::from_bytes([1; 32]),
        [2; 32],
        plan(&"x".repeat(available + 1), vec![]),
        ArtifactRole::PublicEvent,
        None,
        ProviderCall::new(
            PublicKey::from_str(AUTHOR).unwrap(),
            NonZeroU64::new(CREATED * 1000 + 10000).unwrap(),
            CancellationSignal::new(),
        ),
    );
    assert!(matches!(too_big, Err(ProviderError::LimitExceeded)));
}
#[test]
fn authenticated_bytes_survive_unknown_or_out_of_range_observation_and_never_serialize_implicitly()
{
    let intent = intent("synthetic bounded payload", vec![], 7, 8);
    let raw = signed(&intent);
    for observed in [None, NonZeroU64::new(1), NonZeroU64::new(u64::MAX)] {
        let outcome = SigningOutcome::capture(&intent, raw.clone(), observed);
        let OutcomeKind::Obtained(artifact) = outcome.kind() else {
            panic!("valid artifact lost: {outcome:?}");
        };
        assert_eq!(artifact.with_json(str::to_owned), raw);
        assert_eq!(
            artifact.event_id(),
            intent.with_plan(|plan| *plan.expected_event_id())
        );
        assert_eq!(
            artifact.observed_at_unix_ms(),
            observed.filter(|at| at.get() <= MAX_EVENT_UNIX_SECONDS * 1000 + 999)
        );
        assert!(!format!("{outcome:?}").contains("synthetic bounded payload"));
        assert_eq!(format!("{intent:?}"), "SigningIntent(<redacted>)");
    }
}
#[test]
fn callback_provenance_never_rebinds_another_operation_or_command_as_success() {
    let original = intent("same body", vec![], 1, 2);
    let raw = signed(&original);
    for other in [
        intent("same body", vec![], 9, 2),
        intent("same body", vec![], 1, 9),
    ] {
        assert_ne!(original.binding(), other.binding());
        let outcome =
            SigningOutcome::capture(&original, raw.clone(), None).enforce_binding(other.binding());
        let OutcomeKind::Rejected(rejected) = outcome.kind() else {
            panic!("cross-intent success");
        };
        assert_eq!(rejected.reason(), RejectionReason::BindingMismatch);
        assert_eq!(rejected.fingerprint().unwrap().total_bytes(), raw.len());
    }
    let expected = original.binding();
    assert!(matches!(
        SigningOutcome::unknown(expected, ProviderError::ProviderFailure)
            .enforce_binding(expected)
            .kind(),
        OutcomeKind::Unknown(_)
    ));
}
#[test]
fn malformed_wrong_field_signature_and_oversized_output_are_bounded_forensics() {
    let intent = intent("expected", vec![], 1, 2);
    let raw = signed(&intent);
    let mut signature: serde_json::Value = serde_json::from_str(&raw).unwrap();
    signature["sig"] = "00".repeat(64).into();
    // The callback can preserve every expected field while substituting only
    // the returned event ID. That response is not authenticated exact evidence.
    let mut wrong_id: serde_json::Value = serde_json::from_str(&raw).unwrap();
    wrong_id["id"] = "11".repeat(32).into();
    let different = signed(&self::intent("other", vec![], 1, 2));
    for (raw, reason) in [
        (
            String::from("malformed private response"),
            RejectionReason::InvalidArtifact,
        ),
        (different, RejectionReason::InvalidArtifact),
        (wrong_id.to_string(), RejectionReason::InvalidArtifact),
        (signature.to_string(), RejectionReason::InvalidSignature),
        (
            "x".repeat(SIGNED_EVENT_MAX_BYTES + 1),
            RejectionReason::TooLarge,
        ),
    ] {
        let outcome = SigningOutcome::capture(&intent, raw.clone(), None);
        let OutcomeKind::Rejected(rejected) = outcome.kind() else {
            panic!("untrusted output promoted");
        };
        assert_eq!(rejected.reason(), reason);
        let fingerprint = rejected.fingerprint().unwrap();
        assert_eq!(
            fingerprint.covered_bytes(),
            raw.len().min(SIGNED_EVENT_MAX_BYTES)
        );
        assert_eq!(fingerprint.total_bytes(), raw.len());
        assert!(!format!("{outcome:?}").contains("malformed private response"));
    }
}
#[test]
fn input_identity_recipient_role_and_time_are_validated_without_host_effects() {
    let input = |role, recipient, call| {
        SigningIntent::new(
            OperationId::from_bytes([1; 32]),
            [2; 32],
            plan("bounded", vec![]),
            role,
            recipient,
            call,
        )
    };
    let call = || {
        ProviderCall::new(
            PublicKey::from_str(AUTHOR).unwrap(),
            NonZeroU64::new(CREATED * 1000 + 10_000).unwrap(),
            CancellationSignal::new(),
        )
    };
    assert!(matches!(
        input(ArtifactRole::BusinessSeal, None, call()),
        Err(ProviderError::InvalidInput)
    ));
    assert!(matches!(
        input(
            ArtifactRole::GiftWrap,
            Some(PublicKey::from_str(AUTHOR).unwrap()),
            call()
        ),
        Err(ProviderError::InvalidInput)
    ));
    assert!(matches!(
        input(
            ArtifactRole::PublicEvent,
            Some(PublicKey::from_str(AUTHOR).unwrap()),
            call()
        ),
        Err(ProviderError::InvalidInput)
    ));
    assert!(matches!(
        input(
            ArtifactRole::PublicEvent,
            None,
            ProviderCall::new(
                PublicKey::from_str(AUTHOR).unwrap(),
                NonZeroU64::new(u64::MAX).unwrap(),
                CancellationSignal::new()
            )
        ),
        Err(ProviderError::InvalidInput)
    ));
}

#[test]
fn unsupported_provider_explicitly_refuses_without_invoking_the_legacy_signer() {
    use radroots_signing::{
        SignReceipt, SignRequest, Signer, SignerStatus,
        provider::{ParticipantProvider, PrivateCiphertext, PrivateText, ProviderState},
        recovery::ReplayCapability,
        signer::BoxFuture,
    };
    use std::{
        sync::atomic::{AtomicUsize, Ordering},
        task::{Context, Poll, Waker},
    };
    struct Unsupported(AtomicUsize);
    impl Signer for Unsupported {
        fn status(&self) -> BoxFuture<'_, Result<SignerStatus, radroots_signing::Error>> {
            Box::pin(async { Ok(SignerStatus::unavailable()) })
        }
        fn sign(
            &self,
            _: SignRequest,
        ) -> BoxFuture<'_, Result<SignReceipt, radroots_signing::Error>> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Box::pin(async {
                Err(radroots_signing::Error::new(
                    radroots_signing::error::Kind::SignerUnavailable,
                ))
            })
        }
    }
    impl ParticipantProvider for Unsupported {
        fn provider_state(&self) -> BoxFuture<'_, Result<ProviderState, ProviderError>> {
            Box::pin(async {
                Ok(ProviderState::new(
                    PublicKey::from_str(AUTHOR).unwrap(),
                    radroots_signing::provider::PrivateCryptoCapability::Unavailable,
                    radroots_signing::provider::PrivateCryptoCapability::Unavailable,
                    ReplayCapability::NonReplayable,
                ))
            })
        }
        fn encrypt_private(
            &self,
            _: ProviderCall,
            _: PublicKey,
            _: PrivateText,
        ) -> BoxFuture<'_, Result<PrivateCiphertext, ProviderError>> {
            Box::pin(async { Err(ProviderError::CapabilityUnavailable) })
        }
        fn decrypt_private(
            &self,
            _: ProviderCall,
            _: PublicKey,
            _: PrivateCiphertext,
        ) -> BoxFuture<'_, Result<PrivateText, ProviderError>> {
            Box::pin(async { Err(ProviderError::CapabilityUnavailable) })
        }
    }
    let provider = Unsupported(AtomicUsize::new(0));
    let input = intent("bounded", vec![], 1, 2);
    let expected = input.binding();
    let mut issue = provider.sign_exact(input);
    let Poll::Ready(outcome) = issue.as_mut().poll(&mut Context::from_waker(Waker::noop())) else {
        panic!("unsupported call pending");
    };
    assert_eq!(outcome.binding(), expected);
    assert!(matches!(
        outcome.kind(),
        OutcomeKind::NotDispatched(ProviderError::CapabilityUnavailable)
    ));
    assert_eq!(provider.0.load(Ordering::SeqCst), 0);
}

#[test]
fn intent_binding_preserves_low_operation_bytes_deadline_role_and_recipient() {
    let original = intent("bounded", vec![], 1, 2);
    let mut full_id = [1; 32];
    full_id[31] = 2;
    let different = SigningIntent::new(
        OperationId::from_bytes(full_id),
        [2; 32],
        plan("bounded", vec![]),
        ArtifactRole::PublicEvent,
        None,
        ProviderCall::new(
            PublicKey::from_str(AUTHOR).unwrap(),
            NonZeroU64::new(CREATED * 1000 + 10_000).unwrap(),
            CancellationSignal::new(),
        ),
    )
    .unwrap();
    assert_ne!(original.binding().digest(), different.binding().digest());
    assert_eq!(different.binding().operation_id().into_bytes(), full_id);
    assert_eq!(original.binding().command_digest(), &[2; 32]);
    assert_eq!(original.role(), ArtifactRole::PublicEvent);
    assert_eq!(original.recipient(), None);
    let later = SigningIntent::new(
        OperationId::from_bytes([1; 32]),
        [2; 32],
        plan("bounded", vec![]),
        ArtifactRole::PublicEvent,
        None,
        ProviderCall::new(
            PublicKey::from_str(AUTHOR).unwrap(),
            NonZeroU64::new(CREATED * 1000 + 10_001).unwrap(),
            CancellationSignal::new(),
        ),
    )
    .unwrap();
    assert_ne!(later.binding(), original.binding());
    let peer =
        PublicKey::from_str("c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5")
            .unwrap();
    for (role, contract, kind, tags) in [
        (
            ArtifactRole::BusinessSeal,
            "radroots.message.seal.v1",
            13,
            vec![],
        ),
        (
            ArtifactRole::GiftWrap,
            "radroots.message.gift_wrap.v1",
            1059,
            vec![vec!["p".into(), peer.to_hex()]],
        ),
    ] {
        // Ciphertext is opaque at this signing boundary; NIP-44 effects have
        // their separate concrete provider/interoperability qualification.
        let opaque = AuthoredEventPlan::from_generic(
            GenericEventDraft::new(
                contract,
                kind,
                CREATED,
                tags,
                "synthetic ciphertext preimage",
                AUTHOR,
            )
            .unwrap(),
        )
        .unwrap();
        // A valid private carrier still requires its separate exact recipient
        // binding; public seal bytes deliberately contain no recipient tag.
        assert!(matches!(
            SigningIntent::new(
                OperationId::from_bytes([1; 32]),
                [2; 32],
                opaque.clone(),
                role,
                None,
                ProviderCall::new(
                    PublicKey::from_str(AUTHOR).unwrap(),
                    NonZeroU64::new(CREATED * 1000 + 10_000).unwrap(),
                    CancellationSignal::new(),
                ),
            ),
            Err(ProviderError::InvalidInput)
        ));
        let input = SigningIntent::new(
            OperationId::from_bytes([1; 32]),
            [2; 32],
            opaque,
            role,
            Some(peer),
            ProviderCall::new(
                PublicKey::from_str(AUTHOR).unwrap(),
                NonZeroU64::new(CREATED * 1000 + 10_000).unwrap(),
                CancellationSignal::new(),
            ),
        )
        .unwrap();
        assert_eq!(input.role(), role);
        assert_eq!(input.recipient(), Some(peer));
        let raw = signed(&input);
        assert_eq!(input.signed_json_bytes(), raw.len());
        let output = SigningOutcome::capture(&input, raw.clone(), None);
        assert!(matches!(output.kind(), OutcomeKind::Obtained(_)));
        let mut malformed: serde_json::Value = serde_json::from_str(&raw).unwrap();
        malformed["extra"] = "unsupported".into();
        assert!(matches!(
            SigningOutcome::capture(&input, malformed.to_string(), None).kind(),
            OutcomeKind::Rejected(_)
        ));
    }
}

#[test]
fn duplicate_escaped_duplicate_unknown_and_deep_root_fields_never_become_signature_facts() {
    let input = intent("quoted \\\" key: { [ is content", vec![], 1, 2);
    let raw = signed(&input);
    assert!(matches!(
        SigningOutcome::capture(&input, raw.clone(), None).kind(),
        OutcomeKind::Obtained(_)
    ));
    for key in ["id", "\\u0069d", "content", "sig", "unknown"] {
        let duplicate = format!("{{\"{key}\":\"untrusted first field\",{}", &raw[1..]);
        assert!(matches!(
            SigningOutcome::capture(&input, duplicate, None).kind(),
            OutcomeKind::Rejected(_)
        ));
    }
    let deep = format!(
        "{{\"id\":{}0{},{}",
        "[".repeat(17),
        "]".repeat(17),
        &raw[1..]
    );
    assert!(matches!(
        SigningOutcome::capture(&input, deep, None).kind(),
        OutcomeKind::Rejected(_)
    ));
}

#[test]
fn frozen_identity_time_and_preflight_refuse_changes_without_effects() {
    let author = PublicKey::from_str(AUTHOR).unwrap();
    let peer =
        PublicKey::from_str("c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5")
            .unwrap();
    let call = |identity| {
        ProviderCall::new(
            identity,
            NonZeroU64::new(CREATED * 1000 + 10_000).unwrap(),
            CancellationSignal::new(),
        )
    };
    assert!(matches!(
        SigningIntent::new(
            OperationId::from_bytes([1; 32]),
            [2; 32],
            plan("bounded", vec![]),
            ArtifactRole::PublicEvent,
            None,
            call(peer)
        ),
        Err(ProviderError::InvalidInput)
    ));
    let future = AuthoredEventPlan::from_generic(
        GenericEventDraft::new(
            "radroots.social.geochat.v1",
            20_000,
            MAX_EVENT_UNIX_SECONDS + 1,
            vec![],
            "bounded",
            AUTHOR,
        )
        .unwrap(),
    )
    .unwrap();
    assert!(matches!(
        SigningIntent::new(
            OperationId::from_bytes([1; 32]),
            [2; 32],
            future,
            ArtifactRole::PublicEvent,
            None,
            call(author)
        ),
        Err(ProviderError::InvalidInput)
    ));
    let cancellation = CancellationSignal::new();
    let input = SigningIntent::new(
        OperationId::from_bytes([1; 32]),
        [2; 32],
        plan("bounded", vec![]),
        ArtifactRole::PublicEvent,
        None,
        ProviderCall::new(
            author,
            NonZeroU64::new(CREATED * 1000 + 10_000).unwrap(),
            cancellation.clone(),
        ),
    )
    .unwrap();
    assert_eq!(input.author(), author);
    assert_eq!(
        input.check_before_effect(author, CREATED * 1000 + 1),
        Ok(())
    );
    assert_eq!(
        input.check_before_effect(peer, CREATED * 1000 + 1),
        Err(ProviderError::IdentityChanged)
    );
    assert_eq!(
        input.check_before_effect(author, CREATED * 1000 + 10_000),
        Err(ProviderError::DeadlineExceeded)
    );
    cancellation.cancel();
    assert_eq!(
        input.check_before_effect(author, CREATED * 1000 + 1),
        Err(ProviderError::Cancelled)
    );
}

#[test]
fn complete_budget_checks_raw_content_and_multiple_tag_encoding() {
    let huge = plan(&"x".repeat(SIGNED_EVENT_MAX_BYTES + 1), vec![]);
    let call = ProviderCall::new(
        PublicKey::from_str(AUTHOR).unwrap(),
        NonZeroU64::new(CREATED * 1000 + 10_000).unwrap(),
        CancellationSignal::new(),
    );
    assert!(matches!(
        SigningIntent::new(
            OperationId::from_bytes([1; 32]),
            [2; 32],
            huge,
            ArtifactRole::PublicEvent,
            None,
            call
        ),
        Err(ProviderError::LimitExceeded)
    ));
    let input = intent(
        "bounded",
        vec![
            vec!["subject".into(), "first".into()],
            vec!["local_note".into(), "second".into()],
        ],
        1,
        2,
    );
    assert_eq!(input.signed_json_bytes(), signed(&input).len());
}

#[test]
fn changed_authenticated_fields_are_refused_without_erasing_forensic_identity() {
    use sha2::{Digest, Sha256};
    let input = intent("expected", vec![], 1, 2);
    let original = nostr::Event::from_json(signed(&input)).unwrap();
    for changed in 0..5 {
        // Fixed public interoperability identities; no custody material.
        let mut scalar = [0; 32];
        scalar[31] = if changed == 0 { 2 } else { 1 };
        let keys = nostr::Keys::new(nostr::SecretKey::from_slice(&scalar).unwrap());
        let mut unsigned = nostr::UnsignedEvent {
            id: None,
            pubkey: keys.public_key(),
            created_at: original.created_at,
            kind: original.kind,
            tags: original.tags.clone(),
            content: original.content.clone(),
        };
        match changed {
            0 => {}
            1 => unsigned.created_at = nostr::Timestamp::from_secs(CREATED + 1),
            2 => unsigned.kind = nostr::Kind::Custom(20_001),
            3 => {
                unsigned.tags = nostr::Tags::from_list(vec![
                    nostr::Tag::parse(vec!["subject", "different"]).unwrap(),
                ])
            }
            _ => unsigned.content = "different".into(),
        }
        let event = unsigned.sign_with_keys(&keys).unwrap();
        event.verify().unwrap();
        let raw = event.as_json();
        let hash: [u8; 32] = Sha256::digest(raw.as_bytes()).into();
        let (binding, kind) = SigningOutcome::capture(&input, raw.clone(), None).into_parts();
        assert_eq!(binding, input.binding());
        let OutcomeKind::Rejected(rejected) = kind else {
            panic!("changed authenticated field accepted")
        };
        assert_eq!(rejected.reason(), RejectionReason::InvalidArtifact);
        let fingerprint = rejected.fingerprint().unwrap();
        assert_eq!(fingerprint.sha256(), &hash);
        assert_eq!(fingerprint.covered_bytes(), raw.len());
    }
    let other = intent("expected", vec![], 2, 2).binding();
    let rejected = SigningOutcome::capture(&input, "invalid response".into(), None);
    let prior = match rejected.kind() {
        OutcomeKind::Rejected(value) => value.fingerprint(),
        _ => panic!("untrusted output accepted"),
    };
    let rebound = rejected.enforce_binding(other);
    let OutcomeKind::Rejected(value) = rebound.kind() else {
        panic!("mismatch accepted")
    };
    assert_eq!(value.reason(), RejectionReason::BindingMismatch);
    assert_eq!(value.fingerprint(), prior);
    let negative =
        SigningOutcome::not_dispatched(input.binding(), ProviderError::CapabilityUnavailable)
            .enforce_binding(other);
    assert!(matches!(negative.kind(), OutcomeKind::Rejected(_)));
}
