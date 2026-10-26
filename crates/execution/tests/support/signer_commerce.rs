//! Controlled scope behavior; real cryptography remains in Nostr's retained tests.
use super::*;
use radroots_protocol::{
    command::CapturedOrderAcceptance,
    commerce::{self, CommerceSigningPlan, EncodedCommerceRumor},
};
use radroots_signing::request::CancellationSignal;

fn data() -> (EncodedCommerceRumor, CommerceSigningPlan) {
    let command = CapturedOrderAcceptance::parse(include_bytes!(
        "../fixtures/commerce_verification/command_wire.json"
    ))
    .unwrap();
    let record = commerce::decode(include_bytes!(
        "../fixtures/commerce_verification/record_wire.json"
    ))
    .unwrap();
    let rumor = EncodedCommerceRumor::capture(record.content()).unwrap();
    let plan = CommerceSigningPlan::parse(
        command.context().operation_id(),
        command.digest(),
        &rumor,
        include_bytes!("../fixtures/commerce_verification/plan_wire.bin"),
    )
    .unwrap();
    (rumor, plan)
}
fn supported() -> ProviderState {
    state(false, true).with_exact_issuance(Some(ReplayCapability::NonReplayable))
}
fn call(second: bool) -> ProviderCall {
    ProviderCall::new(
        key(second),
        core::num::NonZeroU64::new(600000).unwrap(),
        CancellationSignal::new(),
    )
}
fn configure(control: &Control) {
    *control.current.lock().unwrap() = supported();
    *control.commerce_plaintext.lock().unwrap() = Some(data().0.protected_wire().to_owned());
}
#[test]
fn scope_verifies_complete_original_plan_and_refuses_unobserved_foreign_changed_and_closed_calls() {
    let Fixture {
        session,
        control,
        provider,
        callbacks,
    } = scope();
    configure(&control);
    let (rumor, plan) = data();
    assert!(matches!(
        block_on(session.verify_commerce_plan(call(false), &rumor, plan)),
        Err(ProviderError::CapabilityUnavailable)
    ));
    assert_eq!(control.crypto_calls.load(Ordering::SeqCst), 0);
    block_on(session.observe()).unwrap();
    let (rumor, plan) = data();
    assert!(matches!(
        block_on(session.verify_commerce_plan(call(true), &rumor, plan)),
        Err(ProviderError::IdentityChanged)
    ));
    assert!(!session.identity_changed());
    let (rumor, plan) = data();
    let verified = block_on(session.verify_commerce_plan(call(false), &rumor, plan)).unwrap();
    assert_eq!(
        verified.plan().protected_wire(),
        include_bytes!("../fixtures/commerce_verification/plan_wire.bin")
    );
    assert_eq!(verified.deadline_unix_ms(), 600000);
    assert_eq!(control.crypto_calls.load(Ordering::SeqCst), 2);
    for unavailable in [
        state(false, false).with_exact_issuance(Some(ReplayCapability::NonReplayable)),
        state(false, true),
    ] {
        *control.current.lock().unwrap() = unavailable;
        let (rumor, plan) = data();
        assert!(matches!(
            block_on(session.verify_commerce_plan(call(false), &rumor, plan)),
            Err(ProviderError::CapabilityUnavailable)
        ));
        assert_eq!(control.crypto_calls.load(Ordering::SeqCst), 2);
    }
    *control.current.lock().unwrap() =
        state(true, true).with_exact_issuance(Some(ReplayCapability::NonReplayable));
    let (rumor, plan) = data();
    assert!(matches!(
        block_on(session.verify_commerce_plan(call(false), &rumor, plan)),
        Err(ProviderError::IdentityChanged)
    ));
    assert!(session.identity_changed());
    assert_eq!(control.crypto_calls.load(Ordering::SeqCst), 2);
    assert_eq!(control.sign_calls.load(Ordering::SeqCst), 0);
    block_on(session.close()).unwrap();
    assert!(provider.upgrade().is_none() && callbacks.upgrade().is_none());
    let (rumor, plan) = data();
    assert!(matches!(
        block_on(session.verify_commerce_plan(call(false), &rumor, plan)),
        Err(ProviderError::CapabilityUnavailable)
    ));
}

#[test]
fn pending_actual_decryption_keeps_callback_roots_until_settlement_and_close_refuses_completion() {
    let Fixture {
        session,
        control,
        provider,
        callbacks,
    } = scope();
    configure(&control);
    block_on(session.observe()).unwrap();
    let (send, receive) = oneshot::channel();
    *control.crypto_response.lock().unwrap() = Some(receive);
    let (rumor, plan) = data();
    let mut verify = Box::pin(session.verify_commerce_plan(call(false), &rumor, plan));
    let mut cx = Context::from_waker(noop_waker_ref());
    assert!(verify.as_mut().poll(&mut cx).is_pending());
    assert_eq!(control.crypto_calls.load(Ordering::SeqCst), 1);
    let mut close = Box::pin(session.close());
    assert!(close.as_mut().poll(&mut cx).is_pending());
    assert_eq!(control.close_attempts.load(Ordering::SeqCst), 0);
    assert!(provider.upgrade().is_some() && callbacks.upgrade().is_some());
    send.send(()).unwrap();
    assert!(matches!(
        verify.as_mut().poll(&mut cx),
        Poll::Ready(Err(ProviderError::CapabilityUnavailable))
    ));
    assert!(matches!(close.as_mut().poll(&mut cx), Poll::Ready(Ok(()))));
    assert_eq!(control.crypto_calls.load(Ordering::SeqCst), 1);
    assert_eq!(control.sign_calls.load(Ordering::SeqCst), 0);
    assert!(provider.upgrade().is_none() && callbacks.upgrade().is_none());
}

#[test]
fn identity_change_during_decryption_latches_and_never_confirms_a_plan_or_calls_signing() {
    let Fixture {
        session, control, ..
    } = scope();
    configure(&control);
    block_on(session.observe()).unwrap();
    let (send, receive) = oneshot::channel();
    *control.crypto_response.lock().unwrap() = Some(receive);
    let (rumor, plan) = data();
    let mut verify = Box::pin(session.verify_commerce_plan(call(false), &rumor, plan));
    let mut cx = Context::from_waker(noop_waker_ref());
    assert!(verify.as_mut().poll(&mut cx).is_pending());
    *control.current.lock().unwrap() =
        state(true, true).with_exact_issuance(Some(ReplayCapability::NonReplayable));
    send.send(()).unwrap();
    assert!(matches!(
        block_on(verify),
        Err(ProviderError::IdentityChanged)
    ));
    assert!(session.identity_changed());
    configure(&control);
    let (rumor, plan) = data();
    assert!(matches!(
        block_on(session.verify_commerce_plan(call(false), &rumor, plan)),
        Err(ProviderError::IdentityChanged)
    ));
    assert_eq!(control.crypto_calls.load(Ordering::SeqCst), 1);
    assert_eq!(control.sign_calls.load(Ordering::SeqCst), 0);
    block_on(session.close()).unwrap();
}
