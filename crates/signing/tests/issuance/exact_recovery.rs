//! Original-bound observations, not authorization or durable effect tests.
use super::*;
use radroots_signing::{
    Error, SignReceipt, SignRequest, Signer, SignerStatus,
    provider::{
        ParticipantProvider, PrivateCiphertext, PrivateCryptoCapability, PrivateText, ProviderState,
    },
    recovery::ReplayCapability,
    signer::BoxFuture,
};
use std::{
    sync::atomic::{AtomicUsize, Ordering},
    task::{Context, Poll, Waker},
};

fn call(identity: PublicKey, deadline: u64) -> ProviderCall {
    ProviderCall::new(
        identity,
        NonZeroU64::new(deadline).unwrap(),
        CancellationSignal::new(),
    )
}

#[test]
fn recovery_data_preserves_original_binding_and_rejects_foreign_or_unbounded_call_budget() {
    let original = intent("private retained original", vec![], 7, 8);
    let binding = original.binding();
    let request = ExactRecoveryRequest::new(
        original,
        call(
            PublicKey::from_str(AUTHOR).unwrap(),
            CREATED * 1000 + 20_000,
        ),
    )
    .unwrap();
    assert_eq!(request.binding(), binding);
    assert_eq!(request.original().binding(), binding);
    assert_eq!(
        request.call().deadline_unix_ms().get(),
        CREATED * 1000 + 20_000
    );
    assert_eq!(format!("{request:?}"), "ExactRecoveryRequest(<redacted>)");
    let other =
        PublicKey::from_hex("c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5")
            .unwrap();
    assert!(matches!(
        ExactRecoveryRequest::new(
            intent("x", vec![], 7, 8),
            call(other, CREATED * 1000 + 20_000)
        ),
        Err(ProviderError::IdentityChanged)
    ));
    assert!(matches!(
        ExactRecoveryRequest::new(
            intent("x", vec![], 7, 8),
            call(PublicKey::from_str(AUTHOR).unwrap(), u64::MAX)
        ),
        Err(ProviderError::InvalidInput)
    ));
}

#[test]
fn refusal_of_this_recovery_invocation_never_proves_original_nonissuance() {
    let binding = intent("private retained original", vec![], 7, 8).binding();
    for reason in [
        ProviderError::CapabilityUnavailable,
        ProviderError::Cancelled,
        ProviderError::DeadlineExceeded,
        ProviderError::ProviderFailure,
    ] {
        for observed in [
            SigningOutcome::not_dispatched(binding, reason),
            SigningOutcome::unknown(binding, reason),
        ] {
            let converted = ExactRecoveryOutcome::from_recovery_verification(observed);
            assert_eq!(converted.binding(), binding);
            assert!(
                matches!(converted.kind(), ExactRecoveryKind::Indeterminate(value) if *value==reason)
            );
        }
    }
}

#[test]
fn verified_original_bytes_and_tampering_keep_distinct_recovery_observations() {
    let original = intent("private retained original", vec![], 7, 8);
    let raw = signed(&original);
    let expected = original.binding();
    let recovered = ExactRecoveryOutcome::from_recovery_verification(SigningOutcome::capture(
        &original,
        raw.clone(),
        None,
    ));
    assert_eq!(recovered.binding(), expected);
    let (_, ExactRecoveryKind::Obtained(artifact)) = recovered.into_parts() else {
        panic!("actual original bytes lost")
    };
    assert_eq!(artifact.with_json(str::to_owned), raw);
    assert!(artifact.observed_at_unix_ms().is_none());
    let wrong = signed(&intent("different fields", vec![], 7, 8));
    let rejected = ExactRecoveryOutcome::from_recovery_verification(SigningOutcome::capture(
        &original, wrong, None,
    ));
    assert!(matches!(rejected.kind(), ExactRecoveryKind::Rejected(_)));
    assert!(!format!("{rejected:?}").contains("private retained original"));
}

struct Unsupported(AtomicUsize);
impl Signer for Unsupported {
    fn status(&self) -> BoxFuture<'_, Result<SignerStatus, Error>> {
        panic!("unexpected legacy status")
    }
    fn sign(&self, _request: SignRequest) -> BoxFuture<'_, Result<SignReceipt, Error>> {
        self.0.fetch_add(1, Ordering::SeqCst);
        panic!("legacy fallback")
    }
}
impl ParticipantProvider for Unsupported {
    fn provider_state(&self) -> BoxFuture<'_, Result<ProviderState, ProviderError>> {
        Box::pin(async {
            Ok(ProviderState::new(
                PublicKey::from_str(AUTHOR).unwrap(),
                PrivateCryptoCapability::Unavailable,
                PrivateCryptoCapability::Unavailable,
                ReplayCapability::LocalReplaySafe,
            ))
        })
    }
    fn sign_exact(&self, _intent: SigningIntent) -> BoxFuture<'_, SigningOutcome> {
        self.0.fetch_add(1, Ordering::SeqCst);
        panic!("first-dispatch fallback")
    }
    fn encrypt_private(
        &self,
        _call: ProviderCall,
        _recipient: PublicKey,
        _text: PrivateText,
    ) -> BoxFuture<'_, Result<PrivateCiphertext, ProviderError>> {
        panic!("unexpected encryption")
    }
    fn decrypt_private(
        &self,
        _call: ProviderCall,
        _sender: PublicKey,
        _text: PrivateCiphertext,
    ) -> BoxFuture<'_, Result<PrivateText, ProviderError>> {
        panic!("unexpected decryption")
    }
}

#[test]
fn an_unsupported_recovery_method_never_falls_back_to_declared_legacy_replay_or_signing() {
    let provider = Unsupported(AtomicUsize::new(0));
    let original = intent("private retained original", vec![], 7, 8);
    let expected = original.binding();
    let request = ExactRecoveryRequest::new(
        original,
        call(
            PublicKey::from_str(AUTHOR).unwrap(),
            CREATED * 1000 + 20_000,
        ),
    )
    .unwrap();
    let mut future = provider.recover_exact(request);
    let Poll::Ready(value) = future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    else {
        panic!("unsupported method waited")
    };
    assert_eq!(value.binding(), expected);
    assert!(matches!(
        value.kind(),
        ExactRecoveryKind::Indeterminate(ProviderError::CapabilityUnavailable)
    ));
    assert_eq!(provider.0.load(Ordering::SeqCst), 0);
}
