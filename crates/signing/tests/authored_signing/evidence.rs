use super::*;
use radroots_signing::{AuthoredSignEvidence, Signer, SignerStatus, signer::BoxFuture};
use std::{
    sync::atomic::{AtomicUsize, Ordering},
    task::{Context, Poll, Waker},
};

fn ready<T>(mut future: BoxFuture<'_, T>) -> T {
    match future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("fixture must complete without an executor"),
    }
}

#[test]
fn late_and_cancelled_evidence_preserves_exact_bytes_without_active_success() {
    let request = request();
    let event = signed_event();
    let evidence = AuthoredSignEvidence::from_signed_event(&request, event.clone(), DEADLINE_MS)
        .expect("late signature remains a fact");
    assert_eq!(evidence.operation_kind(), request.operation_kind());
    assert_eq!(evidence.intent_id(), request.intent_id());
    assert_eq!(evidence.signer_request_id(), request.signer_request_id());
    assert_eq!(evidence.observed_at_unix_ms(), DEADLINE_MS);
    assert_eq!(evidence.signed_event(), &event);
    assert_eq!(
        SignReceipt::from_signed_event(&request, event.clone(), DEADLINE_MS)
            .unwrap_err()
            .kind(),
        Kind::DeadlineExceeded
    );
    request.cancellation_signal().cancel();
    let later = evidence
        .revalidate(&request, DEADLINE_MS + 1)
        .expect("retain after cancellation");
    assert_eq!(later.signed_event(), &event);
    assert_eq!(later.observed_at_unix_ms(), DEADLINE_MS + 1);
    assert_eq!(
        SignReceipt::from_signed_event(&request, event.clone(), DEADLINE_MS - 1)
            .unwrap_err()
            .kind(),
        Kind::SignerCancelled
    );
    assert_eq!(
        AuthoredSignEvidence::from_signed_event(&request, event, 0)
            .unwrap_err()
            .kind(),
        Kind::InvalidArgument
    );
    assert_eq!(
        later.revalidate(&request, 0).unwrap_err().kind(),
        Kind::InvalidArgument
    );
    let debug = format!("{later:?}");
    assert!(debug.contains(later.signed_event().id_str()));
    assert!(!debug.contains("exact signing plan"));
    assert!(!debug.contains(&later.signed_event().wire().sig));
    assert_eq!(later, later.clone());
    #[cfg(feature = "serde")]
    {
        let json = serde_json::to_value(&later).expect("serialize evidence");
        assert_eq!(json["observed_at_unix_ms"], DEADLINE_MS + 1);
        assert!(json.get("signed_event").is_some());
    }
}

#[test]
fn evidence_rejects_wrong_intent_even_for_the_identical_signed_event() {
    let expected = request();
    let evidence =
        AuthoredSignEvidence::from_signed_event(&expected, signed_event(), DEADLINE_MS).unwrap();
    for identity in [intent(3, 2), intent(1, 3)] {
        let other = SignRequest::new(
            OperationId::SyncPush,
            identity,
            actor(ActorSource::ExplicitPublicKey, [AuthorRole::Any]),
            plan(),
            policy(),
        )
        .unwrap();
        assert_eq!(other.expected_event_id(), expected.expected_event_id());
        assert_eq!(
            evidence.revalidate(&other, DEADLINE_MS).unwrap_err().kind(),
            Kind::SignerOutputInvalid
        );
    }
    let other_kind = SignRequest::new(
        OperationId::SyncPull,
        intent(1, 2),
        actor(ActorSource::ExplicitPublicKey, [AuthorRole::Any]),
        plan(),
        policy(),
    )
    .unwrap();
    assert_eq!(
        evidence
            .revalidate(&other_kind, DEADLINE_MS)
            .unwrap_err()
            .kind(),
        Kind::SignerOutputInvalid
    );
    let other_plan = AuthoredEventPlan::from_generic(
        GenericEventDraft::new(
            "radroots.social.geochat.v1",
            20_000,
            CREATED_AT,
            Vec::new(),
            "different plan",
            public_key().to_hex(),
        )
        .unwrap(),
    )
    .unwrap();
    let other_request = SignRequest::new(
        OperationId::SyncPush,
        intent(1, 2),
        actor(ActorSource::ExplicitPublicKey, [AuthorRole::Any]),
        other_plan,
        policy(),
    )
    .unwrap();
    assert_eq!(
        evidence
            .revalidate(&other_request, DEADLINE_MS)
            .unwrap_err()
            .kind(),
        Kind::SignerOutputInvalid
    );
}

#[test]
fn late_evidence_keeps_cryptographic_and_exact_plan_checks() {
    let request = request();
    let other_keys = Keys::new(SecretKey::from_hex(&"11".repeat(32)).unwrap());
    for event in [
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
            vec![nostr::Tag::parse(["t", "wrong"]).unwrap()],
        ),
        signed_event_with(&keys(), 20_000, "wrong content", CREATED_AT, Vec::new()),
    ] {
        assert_eq!(
            AuthoredSignEvidence::from_signed_event(&request, event, DEADLINE_MS + 1)
                .unwrap_err()
                .kind(),
            Kind::SignerOutputInvalid
        );
    }
    let invalid = invalid_signature_event();
    assert_eq!(
        AuthoredSignEvidence::from_signed_event(&request, invalid, DEADLINE_MS + 1)
            .unwrap_err()
            .kind(),
        Kind::SignerOutputInvalid
    );
}

struct LegacySigner {
    receipt_request: SignRequest,
    calls: AtomicUsize,
    fail: bool,
}
impl Signer for LegacySigner {
    fn status(&self) -> BoxFuture<'_, Result<SignerStatus, Error>> {
        Box::pin(async { Ok(SignerStatus::unavailable()) })
    }
    fn sign(&self, _request: SignRequest) -> BoxFuture<'_, Result<SignReceipt, Error>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Box::pin(async {
            if self.fail {
                return Err(Error::new(Kind::SignerUnavailable));
            }
            SignReceipt::from_signed_event(&self.receipt_request, signed_event(), DEADLINE_MS - 1)
        })
    }
}

#[test]
fn default_hook_preserves_legacy_adapters_and_rejects_wrong_receipt_binding() {
    let signer = LegacySigner {
        receipt_request: request(),
        calls: AtomicUsize::new(0),
        fail: false,
    };
    let object: &dyn Signer = &signer;
    let result = ready(object.sign_authored_evidence(request())).unwrap();
    assert_eq!(result.intent_id(), request().intent_id());
    assert_eq!(result.observed_at_unix_ms(), DEADLINE_MS - 1);
    assert_eq!(signer.calls.load(Ordering::SeqCst), 1);
    let wrong = SignRequest::new(
        OperationId::SyncPush,
        intent(4, 2),
        actor(ActorSource::ExplicitPublicKey, [AuthorRole::Any]),
        plan(),
        policy(),
    )
    .unwrap();
    assert_eq!(
        ready(object.sign_authored_evidence(wrong))
            .unwrap_err()
            .kind(),
        Kind::SignerOutputInvalid
    );
    let unavailable = LegacySigner {
        receipt_request: request(),
        calls: AtomicUsize::new(0),
        fail: true,
    };
    assert_eq!(
        ready(unavailable.sign_authored_evidence(request()))
            .unwrap_err()
            .kind(),
        Kind::SignerUnavailable
    );
}

#[test]
fn blossom_credentials_cannot_use_authored_evidence_or_invoke_default_signer() {
    let request = blossom_request();
    let event = signed_event_with(
        &keys(),
        24_242,
        request.content(),
        request.created_at(),
        request
            .tags()
            .iter()
            .cloned()
            .map(nostr::Tag::parse)
            .collect::<Result<Vec<_>, _>>()
            .unwrap(),
    );
    assert!(SignReceipt::from_signed_event(&request, event.clone(), DEADLINE_MS - 1).is_ok());
    for time in [DEADLINE_MS - 1, DEADLINE_MS, DEADLINE_MS + 1] {
        assert_eq!(
            AuthoredSignEvidence::from_signed_event(&request, event.clone(), time)
                .unwrap_err()
                .kind(),
            Kind::InvalidArgument
        );
    }
    assert_eq!(
        SignReceipt::from_signed_event(&request, event.clone(), DEADLINE_MS)
            .unwrap_err()
            .kind(),
        Kind::DeadlineExceeded
    );
    request.cancellation_signal().cancel();
    assert_eq!(
        SignReceipt::from_signed_event(&request, event, DEADLINE_MS - 1)
            .unwrap_err()
            .kind(),
        Kind::SignerCancelled
    );
    let signer = LegacySigner {
        receipt_request: super::request(),
        calls: AtomicUsize::new(0),
        fail: false,
    };
    assert_eq!(
        ready(signer.sign_authored_evidence(request))
            .unwrap_err()
            .kind(),
        Kind::InvalidArgument
    );
    assert_eq!(signer.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn default_hook_does_not_invoke_an_adapter_for_already_cancelled_work() {
    let request = request();
    request.cancellation_signal().cancel();
    let signer = LegacySigner {
        receipt_request: super::request(),
        calls: AtomicUsize::new(0),
        fail: false,
    };
    assert_eq!(
        ready(signer.sign_authored_evidence(request))
            .unwrap_err()
            .kind(),
        Kind::SignerCancelled
    );
    assert_eq!(signer.calls.load(Ordering::SeqCst), 0);
}
