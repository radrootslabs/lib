//! Real local signatures with the original issuance window kept intact.
use super::*;
use radroots_signing::issuance::{
    ExactRecoveryCapability, ExactRecoveryKind, ExactRecoveryRequest,
};

fn request(original: SigningIntent, cancellation: CancellationSignal) -> ExactRecoveryRequest {
    ExactRecoveryRequest::new(
        original,
        ProviderCall::new(
            AUTHOR.parse().unwrap(),
            NonZeroU64::new(DEADLINE + 10_000).unwrap(),
            cancellation,
        ),
    )
    .unwrap()
}

#[tokio::test]
async fn local_recovery_returns_only_original_exact_fields_and_keeps_obtained_bytes_after_observation_failure()
 {
    for mode in [0, 1, 2, 6, 7, 9] {
        let (signer, original) = setup(mode);
        let expected = original.binding();
        let event_id = original.with_event(|_, id| id);
        let state = signer.provider_state().await.unwrap();
        assert_eq!(
            state.exact_issuance(),
            Some(ReplayCapability::NonReplayable)
        );
        assert_eq!(
            state.exact_recovery(),
            ExactRecoveryCapability::LocalExactReplay
        );
        let outcome = signer
            .recover_exact(request(original, CancellationSignal::new()))
            .await;
        assert_eq!(outcome.binding(), expected);
        let ExactRecoveryKind::Obtained(artifact) = outcome.kind() else {
            panic!("actual original signature lost: {outcome:?}")
        };
        artifact.with_json(|raw| {
            let event = nostr::Event::from_json(raw).unwrap();
            event.verify().unwrap();
            assert_eq!(event.id.to_hex(), event_id.to_hex());
            assert_eq!(event.pubkey.to_hex(), AUTHOR);
            assert_eq!(event.content, "synthetic unchanged exact preimage");
        });
        assert_eq!(
            artifact.observed_at_unix_ms().is_none(),
            matches!(mode, 1 | 7 | 9)
        );
        CLOCK.with(|state| assert_eq!(state.borrow().calls, 2));
    }
}

#[tokio::test]
async fn fresh_recovery_budget_never_renews_expired_original_or_proves_it_unissued() {
    for mode in [3, 4, 5, 8] {
        let (signer, original) = setup(mode);
        let expected = original.binding();
        let outcome = signer
            .recover_exact(request(original, CancellationSignal::new()))
            .await;
        assert_eq!(outcome.binding(), expected);
        assert!(matches!(
            outcome.kind(),
            ExactRecoveryKind::Indeterminate(
                ProviderError::ProviderFailure | ProviderError::DeadlineExceeded
            )
        ));
        CLOCK.with(|state| assert_eq!(state.borrow().calls, 1));
    }
}

#[tokio::test]
async fn current_cancellation_wrong_actual_key_and_shorter_recovery_budget_refuse_before_crypto() {
    let (signer, original) = setup(0);
    let signal = CancellationSignal::new();
    signal.cancel();
    let outcome = signer.recover_exact(request(original, signal)).await;
    assert!(matches!(
        outcome.kind(),
        ExactRecoveryKind::Indeterminate(ProviderError::Cancelled)
    ));
    CLOCK.with(|state| assert_eq!(state.borrow().calls, 1));
    let (_, original) = setup(0);
    let other = LocalSigner::with_clock(
        parse_secret_key("0000000000000000000000000000000000000000000000000000000000000002")
            .unwrap(),
        clock,
    )
    .unwrap();
    let outcome = other
        .recover_exact(request(original, CancellationSignal::new()))
        .await;
    assert!(matches!(
        outcome.kind(),
        ExactRecoveryKind::Indeterminate(ProviderError::IdentityChanged)
    ));
    CLOCK.with(|state| assert_eq!(state.borrow().calls, 0));
    let (signer, original) = setup(0);
    let call = ProviderCall::new(
        signer.public_key(),
        NonZeroU64::new(CREATED * 1000).unwrap(),
        CancellationSignal::new(),
    );
    let outcome = signer
        .recover_exact(ExactRecoveryRequest::new(original, call).unwrap())
        .await;
    assert!(matches!(
        outcome.kind(),
        ExactRecoveryKind::Indeterminate(ProviderError::DeadlineExceeded)
    ));
    CLOCK.with(|state| assert_eq!(state.borrow().calls, 1));
}
