//! Scope mechanics; concrete local cryptography is exercised by the facade.
use super::*;
use radroots_signing::{
    issuance::{
        ExactRecoveryCapability, ExactRecoveryKind, ExactRecoveryOutcome, ExactRecoveryRequest,
        RejectionReason,
    },
    request::CancellationSignal,
};
fn supported(second: bool) -> ProviderState {
    state(second, false).with_exact_recovery(ExactRecoveryCapability::LocalExactReplay)
}
fn request(second: bool, operation: u8) -> ExactRecoveryRequest {
    ExactRecoveryRequest::new(
        exact_intent(second, operation),
        ProviderCall::new(
            key(second),
            core::num::NonZeroU64::new(1_700_000_020_000).unwrap(),
            CancellationSignal::new(),
        ),
    )
    .unwrap()
}
#[test]
fn recovery_refuses_unobserved_unsupported_changed_identity_and_changed_capability_without_fallback()
 {
    let Fixture {
        session, control, ..
    } = scope();
    *control.current.lock().unwrap() = ProviderState::new(
        key(false),
        PrivateCryptoCapability::Unavailable,
        PrivateCryptoCapability::Nip44V2,
        ReplayCapability::LocalReplaySafe,
    );
    for observed in [false, true] {
        if observed {
            block_on(session.observe()).unwrap();
        }
        let request = request(false, 1);
        let expected = request.binding();
        let result = block_on(session.recover_exact(request));
        assert_eq!(result.binding(), expected);
        assert!(matches!(
            result.kind(),
            ExactRecoveryKind::Indeterminate(ProviderError::CapabilityUnavailable)
        ));
    }
    *control.current.lock().unwrap() = supported(false);
    block_on(session.observe()).unwrap();
    assert!(matches!(
        block_on(session.recover_exact(request(true, 1))).kind(),
        ExactRecoveryKind::Indeterminate(ProviderError::IdentityChanged)
    ));
    assert!(!session.identity_changed());
    let result = block_on(session.recover_exact(request(false, 1)));
    assert!(matches!(
        result.kind(),
        ExactRecoveryKind::Indeterminate(ProviderError::ProviderFailure)
    ));
    assert_eq!(control.recovery_calls.load(Ordering::SeqCst), 1);
    *control.current.lock().unwrap() = state(false, false);
    assert!(matches!(
        block_on(session.recover_exact(request(false, 1))).kind(),
        ExactRecoveryKind::Indeterminate(ProviderError::CapabilityUnavailable)
    ));
    assert_eq!(control.recovery_calls.load(Ordering::SeqCst), 1);
    *control.current.lock().unwrap() = supported(false);
    block_on(session.observe()).unwrap();
    *control.current.lock().unwrap() = supported(true);
    assert!(matches!(
        block_on(session.recover_exact(request(false, 1))).kind(),
        ExactRecoveryKind::Indeterminate(ProviderError::IdentityChanged)
    ));
    assert!(session.identity_changed());
    assert_eq!(control.recovery_calls.load(Ordering::SeqCst), 1);
    assert_eq!(control.sign_calls.load(Ordering::SeqCst), 0);
    assert_eq!(control.crypto_calls.load(Ordering::SeqCst), 0);
    block_on(session.close()).unwrap();
    assert!(matches!(
        block_on(session.recover_exact(request(false, 1))).kind(),
        ExactRecoveryKind::Indeterminate(ProviderError::CapabilityUnavailable)
    ));
}

#[test]
fn pending_recovery_keeps_callbacks_until_settlement_and_rejects_a_foreign_callback() {
    let Fixture {
        session,
        control,
        provider,
        callbacks,
    } = scope();
    *control.current.lock().unwrap() = supported(false);
    block_on(session.observe()).unwrap();
    let input = request(false, 1);
    let expected = input.binding();
    let (send, receive) = oneshot::channel();
    *control.recovery_response.lock().unwrap() = Some(receive);
    let mut recover = Box::pin(session.recover_exact(input));
    let mut cx = Context::from_waker(noop_waker_ref());
    assert!(recover.as_mut().poll(&mut cx).is_pending());
    let mut close = Box::pin(session.close());
    assert!(close.as_mut().poll(&mut cx).is_pending());
    assert_eq!(control.close_attempts.load(Ordering::SeqCst), 0);
    assert!(provider.upgrade().is_some() && callbacks.upgrade().is_some());
    send.send(ExactRecoveryOutcome::indeterminate(
        request(false, 2).binding(),
        ProviderError::ProviderFailure,
    ))
    .unwrap();
    let Poll::Ready(result) = recover.as_mut().poll(&mut cx) else {
        panic!("recovery did not settle")
    };
    assert_eq!(result.binding(), expected);
    let ExactRecoveryKind::Rejected(rejected) = result.kind() else {
        panic!("foreign callback promoted")
    };
    assert_eq!(rejected.reason(), RejectionReason::BindingMismatch);
    assert!(rejected.fingerprint().is_none());
    assert!(matches!(close.as_mut().poll(&mut cx), Poll::Ready(Ok(()))));
    assert!(provider.upgrade().is_none() && callbacks.upgrade().is_none());
    assert_eq!(control.sign_calls.load(Ordering::SeqCst), 0);
}

#[test]
fn failed_initial_observation_cannot_authorize_recovery() {
    let Fixture {
        session, control, ..
    } = scope();
    *control.current.lock().unwrap() = supported(false);
    let (send, receive) = oneshot::channel();
    drop(send);
    *control.response.lock().unwrap() = Some(receive);
    assert!(block_on(session.observe()).is_err());
    assert!(matches!(
        block_on(session.recover_exact(request(false, 1))).kind(),
        ExactRecoveryKind::Indeterminate(ProviderError::CapabilityUnavailable)
    ));
    assert_eq!(control.recovery_calls.load(Ordering::SeqCst), 0);
    block_on(session.close()).unwrap();
}
