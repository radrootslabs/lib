#![cfg(feature = "signer_session")]

use core::task::Poll;
#[cfg(not(target_arch = "wasm32"))]
use core::{future::Future, pin::pin, task::Context};
#[cfg(not(target_arch = "wasm32"))]
use futures::task::noop_waker_ref;
use futures::{channel::oneshot, executor::block_on, task::AtomicWaker};
use radroots_execution::session::{SessionState, SignerSession};
use radroots_identity::PublicKey;
use radroots_signing::host::Shared;
use radroots_signing::{
    SignReceipt, SignRequest, Signer, SignerStatus,
    provider::{
        ParticipantProvider, PrivateCiphertext, PrivateCryptoCapability, PrivateText, ProviderCall,
        ProviderError, ProviderState, SignerCallbacks, SignerHandle,
    },
    recovery::ReplayCapability,
    signer::BoxFuture,
};
use std::sync::{
    Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
#[cfg(all(feature = "commerce", not(target_arch = "wasm32")))]
#[path = "support/signer_commerce.rs"]
mod commerce;
#[cfg(not(target_arch = "wasm32"))]
#[path = "support/signer_recovery.rs"]
mod recovery;

fn key(second: bool) -> PublicKey {
    PublicKey::from_hex(if second {
        "c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5"
    } else {
        "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
    })
    .unwrap()
}

fn state(second: bool, encrypt: bool) -> ProviderState {
    ProviderState::new(
        key(second),
        if encrypt {
            PrivateCryptoCapability::Nip44V2
        } else {
            PrivateCryptoCapability::Unavailable
        },
        PrivateCryptoCapability::Nip44V2,
        ReplayCapability::NonReplayable,
    )
}

struct Control {
    commerce_plaintext: Mutex<Option<String>>,
    recovery_calls: AtomicUsize,
    recovery_response:
        Mutex<Option<oneshot::Receiver<radroots_signing::issuance::ExactRecoveryOutcome>>>,
    recovery_binding: Mutex<Option<radroots_signing::issuance::IntentBinding>>,
    current: Mutex<ProviderState>,
    calls: AtomicUsize,
    sign_calls: AtomicUsize,
    crypto_calls: AtomicUsize,
    crypto_response: Mutex<Option<oneshot::Receiver<()>>>,
    sign_response: Mutex<Option<oneshot::Receiver<radroots_signing::issuance::SigningOutcome>>>,
    sign_binding: Mutex<Option<radroots_signing::issuance::IntentBinding>>,
    response: Mutex<Option<oneshot::Receiver<()>>>,
    quiesces: AtomicUsize,
    quiesced: AtomicBool,
    close_attempts: AtomicUsize,
    closes: AtomicUsize,
    close_ready: AtomicBool,
    fail_close_once: AtomicBool,
    wake: AtomicWaker,
    #[cfg(target_arch = "wasm32")]
    _local: std::rc::Rc<std::cell::Cell<u8>>,
}

struct Provider(Shared<Control>);
struct Callbacks(Shared<Control>);

impl Signer for Provider {
    fn status(&self) -> BoxFuture<'_, Result<SignerStatus, radroots_signing::Error>> {
        Box::pin(async { Ok(SignerStatus::unavailable()) })
    }
    fn sign(&self, _: SignRequest) -> BoxFuture<'_, Result<SignReceipt, radroots_signing::Error>> {
        Box::pin(async {
            Err(radroots_signing::Error::new(
                radroots_signing::error::Kind::SignerUnavailable,
            ))
        })
    }
}

impl ParticipantProvider for Provider {
    fn recover_exact(
        &self,
        request: radroots_signing::issuance::ExactRecoveryRequest,
    ) -> BoxFuture<'_, radroots_signing::issuance::ExactRecoveryOutcome> {
        use radroots_signing::issuance::ExactRecoveryOutcome;
        Box::pin(async move {
            self.0.recovery_calls.fetch_add(1, Ordering::SeqCst);
            let binding = self
                .0
                .recovery_binding
                .lock()
                .unwrap()
                .unwrap_or(request.binding());
            let response = self.0.recovery_response.lock().unwrap().take();
            if let Some(response) = response {
                response.await.unwrap_or_else(|_| {
                    ExactRecoveryOutcome::indeterminate(binding, ProviderError::ProviderFailure)
                })
            } else {
                ExactRecoveryOutcome::indeterminate(binding, ProviderError::ProviderFailure)
            }
        })
    }
    fn sign_exact(
        &self,
        intent: radroots_signing::issuance::SigningIntent,
    ) -> BoxFuture<'_, radroots_signing::issuance::SigningOutcome> {
        use radroots_signing::issuance::SigningOutcome;
        Box::pin(async move {
            self.0.sign_calls.fetch_add(1, Ordering::SeqCst);
            let binding = self
                .0
                .sign_binding
                .lock()
                .unwrap()
                .unwrap_or(intent.binding());
            let response = self.0.sign_response.lock().unwrap().take();
            if let Some(response) = response {
                response.await.unwrap_or_else(|_| {
                    SigningOutcome::unknown(binding, ProviderError::ProviderFailure)
                })
            } else {
                SigningOutcome::unknown(binding, ProviderError::ProviderFailure)
            }
        })
    }

    fn provider_state(&self) -> BoxFuture<'_, Result<ProviderState, ProviderError>> {
        Box::pin(async move {
            self.0.calls.fetch_add(1, Ordering::SeqCst);
            let response = self.0.response.lock().unwrap().take();
            if let Some(response) = response {
                response.await.map_err(|_| ProviderError::Cancelled)?;
            }
            if self.0.quiesced.load(Ordering::SeqCst) {
                return Err(ProviderError::Cancelled);
            }
            Ok(*self.0.current.lock().unwrap())
        })
    }
    fn encrypt_private(
        &self,
        _: ProviderCall,
        _: PublicKey,
        _: PrivateText,
    ) -> BoxFuture<'_, Result<PrivateCiphertext, ProviderError>> {
        Box::pin(async move {
            self.0.crypto_calls.fetch_add(1, Ordering::SeqCst);
            let response = self.0.crypto_response.lock().unwrap().take();
            if let Some(response) = response {
                response.await.map_err(|_| ProviderError::Cancelled)?;
            }
            Err(ProviderError::ProviderFailure)
        })
    }
    fn decrypt_private(
        &self,
        _: ProviderCall,
        _: PublicKey,
        _: PrivateCiphertext,
    ) -> BoxFuture<'_, Result<PrivateText, ProviderError>> {
        Box::pin(async move {
            self.0.crypto_calls.fetch_add(1, Ordering::SeqCst);
            let response = self.0.crypto_response.lock().unwrap().take();
            if let Some(response) = response {
                response.await.map_err(|_| ProviderError::Cancelled)?;
            }
            self.0
                .commerce_plaintext
                .lock()
                .unwrap()
                .as_deref()
                .map(PrivateText::new)
                .unwrap_or(Err(ProviderError::ProviderFailure))
        })
    }
}

impl SignerCallbacks for Callbacks {
    fn quiesce(&self) -> BoxFuture<'_, Result<(), ProviderError>> {
        Box::pin(async move {
            self.0.quiesces.fetch_add(1, Ordering::SeqCst);
            self.0.quiesced.store(true, Ordering::SeqCst);
            Ok(())
        })
    }
    fn close(&self) -> BoxFuture<'_, Result<(), ProviderError>> {
        Box::pin(async move {
            self.0.close_attempts.fetch_add(1, Ordering::SeqCst);
            if self.0.fail_close_once.swap(false, Ordering::SeqCst) {
                return Err(ProviderError::CapabilityUnavailable);
            }
            core::future::poll_fn(|cx| {
                self.0.wake.register(cx.waker());
                if self.0.close_ready.load(Ordering::SeqCst) {
                    Poll::Ready(())
                } else {
                    Poll::Pending
                }
            })
            .await;
            self.0.closes.fetch_add(1, Ordering::SeqCst);
            Ok(())
        })
    }
}

#[cfg(not(target_arch = "wasm32"))]
struct Fixture {
    session: SignerSession,
    control: Shared<Control>,
    provider: std::sync::Weak<Provider>,
    callbacks: std::sync::Weak<Callbacks>,
}

#[cfg(not(target_arch = "wasm32"))]
fn scope() -> Fixture {
    // Native weak handles make root release independently observable.
    let control = control();
    let provider = Shared::new(Provider(control.clone()));
    let callbacks = Shared::new(Callbacks(control.clone()));
    let weak_provider = Shared::downgrade(&provider);
    let weak_callbacks = Shared::downgrade(&callbacks);
    Fixture {
        session: SignerSession::new(SignerHandle::new(provider, callbacks)),
        control,
        provider: weak_provider,
        callbacks: weak_callbacks,
    }
}

fn control() -> Shared<Control> {
    Shared::new(Control {
        commerce_plaintext: Mutex::new(None),
        recovery_calls: AtomicUsize::new(0),
        recovery_response: Mutex::new(None),
        recovery_binding: Mutex::new(None),
        current: Mutex::new(state(false, false)),
        calls: AtomicUsize::new(0),
        sign_calls: AtomicUsize::new(0),
        crypto_calls: AtomicUsize::new(0),
        crypto_response: Mutex::new(None),
        sign_response: Mutex::new(None),
        sign_binding: Mutex::new(None),
        response: Mutex::new(None),
        quiesces: AtomicUsize::new(0),
        quiesced: AtomicBool::new(false),
        close_attempts: AtomicUsize::new(0),
        closes: AtomicUsize::new(0),
        close_ready: AtomicBool::new(true),
        fail_close_once: AtomicBool::new(false),
        wake: AtomicWaker::new(),
        #[cfg(target_arch = "wasm32")]
        _local: std::rc::Rc::new(std::cell::Cell::new(0)),
    })
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn construction_is_inert_and_clones_share_identity_and_revocation() {
    let Fixture {
        session,
        control,
        provider,
        callbacks,
    } = scope();
    assert_eq!(session.state(), SessionState::Ready);
    assert!(session.observed().is_none());
    assert_eq!(control.calls.load(Ordering::SeqCst), 0);
    assert_eq!(control.quiesces.load(Ordering::SeqCst), 0);
    let close = session.close();
    drop(close);
    assert_eq!(session.state(), SessionState::Ready);
    let clone = session.clone();
    assert_eq!(block_on(session.observe()).unwrap().identity(), key(false));
    *control.current.lock().unwrap() = state(false, true);
    assert_eq!(
        block_on(clone.observe()).unwrap().encryption(),
        PrivateCryptoCapability::Nip44V2
    );
    *control.current.lock().unwrap() = state(true, false);
    assert_eq!(
        block_on(session.observe()),
        Err(ProviderError::IdentityChanged)
    );
    assert_eq!(session.observed().unwrap().identity(), key(false));
    assert!(session.identity_changed());
    *control.current.lock().unwrap() = state(false, false);
    assert_eq!(
        block_on(session.observe()),
        Err(ProviderError::IdentityChanged)
    );
    assert_eq!(control.calls.load(Ordering::SeqCst), 3);
    block_on(clone.close()).unwrap();
    assert_eq!(session.state(), SessionState::Closed);
    assert!(provider.upgrade().is_none() && callbacks.upgrade().is_none());
    assert_eq!(
        block_on(session.observe()),
        Err(ProviderError::CapabilityUnavailable)
    );
    block_on(session.close()).unwrap();
    assert_eq!(control.quiesces.load(Ordering::SeqCst), 1);
    assert_eq!(control.closes.load(Ordering::SeqCst), 1);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn close_before_observation_disposes_roots_without_querying_identity() {
    let Fixture {
        session,
        control,
        provider,
        callbacks,
    } = scope();
    block_on(session.close()).unwrap();
    assert_eq!(control.calls.load(Ordering::SeqCst), 0);
    assert_eq!(control.quiesces.load(Ordering::SeqCst), 1);
    assert_eq!(control.closes.load(Ordering::SeqCst), 1);
    assert!(provider.upgrade().is_none() && callbacks.upgrade().is_none());
    assert!(session.observed().is_none());
    assert_eq!(session.state(), SessionState::Closed);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn close_quiesces_before_draining_and_retains_inflight_roots() {
    let Fixture {
        session,
        control,
        provider,
        ..
    } = scope();
    let (send, receive) = oneshot::channel();
    *control.response.lock().unwrap() = Some(receive);
    let mut observe = pin!(session.observe());
    let mut cx = Context::from_waker(noop_waker_ref());
    assert!(observe.as_mut().poll(&mut cx).is_pending());
    assert_eq!(
        block_on(session.observe()),
        Err(ProviderError::CapabilityUnavailable)
    );
    assert_eq!(control.calls.load(Ordering::SeqCst), 1);
    let mut close = pin!(session.close());
    assert!(close.as_mut().poll(&mut cx).is_pending());
    assert_eq!(control.quiesces.load(Ordering::SeqCst), 1);
    assert_eq!(control.close_attempts.load(Ordering::SeqCst), 0);
    assert!(provider.upgrade().is_some());
    send.send(()).unwrap();
    assert_eq!(
        observe.as_mut().poll(&mut cx),
        Poll::Ready(Err(ProviderError::Cancelled))
    );
    assert!(matches!(close.as_mut().poll(&mut cx), Poll::Ready(Ok(()))));
    assert!(provider.upgrade().is_none());
    assert!(session.observed().is_none());
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn failed_and_cancelled_callback_disposal_keep_the_unfinished_stage() {
    for fail in [false, true] {
        let Fixture {
            session,
            control,
            provider,
            callbacks,
        } = scope();
        block_on(session.observe()).unwrap();
        if fail {
            control.fail_close_once.store(true, Ordering::SeqCst);
            assert!(block_on(session.close()).is_err());
        } else {
            control.close_ready.store(false, Ordering::SeqCst);
            let mut close = Box::pin(session.close());
            assert!(
                close
                    .as_mut()
                    .poll(&mut Context::from_waker(noop_waker_ref()))
                    .is_pending()
            );
            drop(close);
        }
        assert_eq!(session.state(), SessionState::Closing);
        assert!(provider.upgrade().is_some() && callbacks.upgrade().is_some());
        control.close_ready.store(true, Ordering::SeqCst);
        control.wake.wake();
        block_on(session.close()).unwrap();
        assert!(provider.upgrade().is_none() && callbacks.upgrade().is_none());
        assert_eq!(control.quiesces.load(Ordering::SeqCst), 1);
        assert_eq!(control.close_attempts.load(Ordering::SeqCst), 2);
        assert_eq!(control.closes.load(Ordering::SeqCst), 1);
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn failed_observation_does_not_freeze_a_key_and_lost_observer_keeps_host_roots() {
    let Fixture {
        session,
        control,
        provider,
        callbacks,
    } = scope();
    let (send, receive) = oneshot::channel();
    drop(send);
    *control.response.lock().unwrap() = Some(receive);
    assert_eq!(block_on(session.observe()), Err(ProviderError::Cancelled));
    assert!(session.observed().is_none());
    *control.current.lock().unwrap() = state(true, false);
    assert_eq!(block_on(session.observe()).unwrap().identity(), key(true));

    let (send, receive) = oneshot::channel();
    *control.response.lock().unwrap() = Some(receive);
    let mut observe = Box::pin(session.observe());
    let mut cx = Context::from_waker(noop_waker_ref());
    assert!(observe.as_mut().poll(&mut cx).is_pending());
    drop(observe);
    control.close_ready.store(false, Ordering::SeqCst);
    let mut close = Box::pin(session.close());
    assert!(close.as_mut().poll(&mut cx).is_pending());
    assert_eq!(control.close_attempts.load(Ordering::SeqCst), 1);
    assert!(provider.upgrade().is_some() && callbacks.upgrade().is_some());
    assert!(send.send(()).is_err());
    control.close_ready.store(true, Ordering::SeqCst);
    control.wake.wake();
    assert!(matches!(close.as_mut().poll(&mut cx), Poll::Ready(Ok(()))));
    assert!(provider.upgrade().is_none() && callbacks.upgrade().is_none());
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn native_scope_and_owned_futures_retain_thread_guarantees() {
    fn send_sync<T: Send + Sync>() {}
    fn send<T: Send>(_: T) {}
    send_sync::<SignerHandle>();
    send_sync::<SignerSession>();
    let Fixture { session, .. } = scope();
    send(session.observe());
    send(session.close());
}

#[cfg(target_arch = "wasm32")]
#[test]
fn browser_scope_accepts_rc_local_provider_and_callbacks() {
    let control = control();
    let session = SignerSession::new(SignerHandle::new(
        Shared::new(Provider(control.clone())),
        Shared::new(Callbacks(control)),
    ));
    assert_eq!(block_on(session.observe()).unwrap().identity(), key(false));
    assert!(matches!(
        block_on(session.sign_exact(exact_intent(false, 1))).kind(),
        radroots_signing::issuance::OutcomeKind::NotDispatched(
            ProviderError::CapabilityUnavailable
        )
    ));
    block_on(session.close()).unwrap();
    assert_eq!(session.state(), SessionState::Closed);
}

#[cfg(all(target_arch = "wasm32", feature = "client_session"))]
#[test]
fn browser_client_composes_local_host_and_retained_callback_scope() {
    use radroots_execution::session::{ClientSession, HostFuture, SessionHost};
    struct LocalHost(std::rc::Rc<std::cell::Cell<u8>>);
    impl LocalHost {
        fn run(&self) -> HostFuture<'_, radroots_execution::Result<()>> {
            Box::pin(async move {
                self.0.set(self.0.get() + 1);
                Ok(())
            })
        }
    }
    impl SessionHost for LocalHost {
        fn start(&self) -> HostFuture<'_, radroots_execution::Result<()>> {
            self.run()
        }
        fn quiesce(&self) -> HostFuture<'_, radroots_execution::Result<()>> {
            self.run()
        }
        fn disconnect(&self) -> HostFuture<'_, radroots_execution::Result<()>> {
            self.run()
        }
        fn dispose_callbacks(&self) -> HostFuture<'_, radroots_execution::Result<()>> {
            self.run()
        }
        fn close_storage(&self) -> HostFuture<'_, radroots_execution::Result<()>> {
            self.run()
        }
    }
    let observations = std::rc::Rc::new(std::cell::Cell::new(0));
    let client = ClientSession::new(LocalHost(observations.clone()), core::num::NonZeroU16::MIN);
    let control = control();
    block_on(client.attach_signer(SignerHandle::new(
        Shared::new(Provider(control.clone())),
        Shared::new(Callbacks(control)),
    )))
    .unwrap();
    assert_eq!(observations.get(), 0);
    block_on(client.start()).unwrap();
    block_on(client.close()).unwrap();
    assert_eq!(observations.get(), 5);
}

fn exact_intent(second: bool, operation: u8) -> radroots_signing::issuance::SigningIntent {
    use radroots_signing::issuance::{ArtifactRole, SigningIntent};
    let plan = radroots_event_codec::authoring::AuthoredEventPlan::from_generic(
        radroots_event::GenericEventDraft::new(
            "radroots.social.geochat.v1",
            20_000,
            1_700_000_000,
            vec![],
            "synthetic captured scope",
            key(second).to_hex(),
        )
        .unwrap(),
    )
    .unwrap();
    SigningIntent::new(
        radroots_domain::OperationId::from_bytes([operation; 32]),
        [9; 32],
        plan,
        ArtifactRole::PublicEvent,
        None,
        ProviderCall::new(
            key(second),
            core::num::NonZeroU64::new(1_700_000_010_000).unwrap(),
            radroots_signing::request::CancellationSignal::new(),
        ),
    )
    .unwrap()
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn exact_issuance_requires_successful_observation_actual_support_and_the_frozen_key() {
    use radroots_signing::issuance::ExactIssuer;
    use radroots_signing::issuance::OutcomeKind;
    let Fixture {
        session, control, ..
    } = scope();
    assert!(matches!(
        block_on(ExactIssuer::issue_exact(&session, exact_intent(false, 1))).kind(),
        OutcomeKind::NotDispatched(ProviderError::CapabilityUnavailable)
    ));
    block_on(session.observe()).unwrap();
    assert!(matches!(
        block_on(session.sign_exact(exact_intent(false, 1))).kind(),
        OutcomeKind::NotDispatched(ProviderError::CapabilityUnavailable)
    ));
    assert_eq!(control.sign_calls.load(Ordering::SeqCst), 0);
    *control.current.lock().unwrap() =
        state(false, false).with_exact_issuance(Some(ReplayCapability::NonReplayable));
    block_on(session.observe()).unwrap();
    assert!(matches!(
        block_on(session.sign_exact(exact_intent(true, 1))).kind(),
        OutcomeKind::NotDispatched(ProviderError::IdentityChanged)
    ));
    assert!(!session.identity_changed());
    assert!(matches!(
        block_on(ExactIssuer::issue_exact(&session, exact_intent(false, 1))).kind(),
        OutcomeKind::Unknown(_)
    ));
    assert_eq!(control.sign_calls.load(Ordering::SeqCst), 1);
    *control.current.lock().unwrap() =
        state(true, false).with_exact_issuance(Some(ReplayCapability::NonReplayable));
    assert_eq!(
        block_on(session.observe()),
        Err(ProviderError::IdentityChanged)
    );
    assert!(matches!(
        block_on(session.sign_exact(exact_intent(false, 1))).kind(),
        OutcomeKind::NotDispatched(ProviderError::IdentityChanged)
    ));
    assert_eq!(control.sign_calls.load(Ordering::SeqCst), 1);
    block_on(session.close()).unwrap();

    let Fixture {
        session, control, ..
    } = scope();
    let (send, receive) = oneshot::channel();
    drop(send);
    *control.response.lock().unwrap() = Some(receive);
    assert!(block_on(session.observe()).is_err());
    assert!(matches!(
        block_on(session.sign_exact(exact_intent(false, 1))).kind(),
        OutcomeKind::NotDispatched(ProviderError::CapabilityUnavailable)
    ));
    assert_eq!(control.sign_calls.load(Ordering::SeqCst), 0);
    block_on(session.close()).unwrap();
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn pending_exact_issuance_retains_callbacks_until_its_bound_outcome_is_observed() {
    use radroots_signing::issuance::ExactIssuer;
    use radroots_signing::issuance::{OutcomeKind, SigningOutcome};
    let Fixture {
        session,
        control,
        provider,
        callbacks,
    } = scope();
    *control.current.lock().unwrap() =
        state(false, false).with_exact_issuance(Some(ReplayCapability::NonReplayable));
    block_on(session.observe()).unwrap();
    let input = exact_intent(false, 1);
    let binding = input.binding();
    let (send, receive) = oneshot::channel();
    *control.sign_response.lock().unwrap() = Some(receive);
    let mut issue = ExactIssuer::issue_exact(&session, input);
    let mut cx = Context::from_waker(noop_waker_ref());
    assert!(issue.as_mut().poll(&mut cx).is_pending());
    let mut close = Box::pin(session.close());
    assert!(close.as_mut().poll(&mut cx).is_pending());
    assert_eq!(session.state(), SessionState::Closing);
    assert_eq!(control.close_attempts.load(Ordering::SeqCst), 0);
    assert!(provider.upgrade().is_some() && callbacks.upgrade().is_some());
    send.send(SigningOutcome::unknown(
        binding,
        ProviderError::ProviderFailure,
    ))
    .unwrap();
    let Poll::Ready(outcome) = issue.as_mut().poll(&mut cx) else {
        panic!("outcome not received");
    };
    assert_eq!(outcome.binding(), binding);
    assert!(matches!(outcome.kind(), OutcomeKind::Unknown(_)));
    assert!(matches!(close.as_mut().poll(&mut cx), Poll::Ready(Ok(()))));
    assert!(provider.upgrade().is_none() && callbacks.upgrade().is_none());
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn exact_callback_binding_cannot_claim_another_operations_result() {
    use radroots_signing::issuance::{OutcomeKind, RejectionReason};
    let Fixture {
        session, control, ..
    } = scope();
    *control.current.lock().unwrap() =
        state(false, false).with_exact_issuance(Some(ReplayCapability::NonReplayable));
    block_on(session.observe()).unwrap();
    *control.sign_binding.lock().unwrap() = Some(exact_intent(false, 2).binding());
    let input = exact_intent(false, 1);
    let expected = input.binding();
    let outcome = block_on(session.sign_exact(input));
    assert_eq!(outcome.binding(), expected);
    let OutcomeKind::Rejected(rejected) = outcome.kind() else {
        panic!("wrong binding promoted");
    };
    assert_eq!(rejected.reason(), RejectionReason::BindingMismatch);
    assert!(rejected.fingerprint().is_none());
    block_on(session.close()).unwrap();
}

fn crypto_call(second: bool) -> ProviderCall {
    ProviderCall::new(
        key(second),
        core::num::NonZeroU64::new(1_700_000_010_000).unwrap(),
        radroots_signing::request::CancellationSignal::new(),
    )
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn private_crypto_reobserves_the_attached_identity_and_capability_without_fallback() {
    let Fixture {
        session, control, ..
    } = scope();
    assert_eq!(
        block_on(session.encrypt_private(
            crypto_call(false),
            key(true),
            PrivateText::new("private model").unwrap()
        ))
        .unwrap_err(),
        ProviderError::CapabilityUnavailable
    );
    assert_eq!(control.calls.load(Ordering::SeqCst), 0);
    block_on(session.observe()).unwrap();
    assert_eq!(
        block_on(session.encrypt_private(
            crypto_call(false),
            key(true),
            PrivateText::new("private model").unwrap()
        ))
        .unwrap_err(),
        ProviderError::CapabilityUnavailable
    );
    assert_eq!(control.crypto_calls.load(Ordering::SeqCst), 0);
    *control.current.lock().unwrap() = state(false, true);
    assert_eq!(
        block_on(session.encrypt_private(
            crypto_call(true),
            key(true),
            PrivateText::new("private model").unwrap()
        ))
        .unwrap_err(),
        ProviderError::IdentityChanged
    );
    assert!(!session.identity_changed());
    assert_eq!(
        block_on(session.encrypt_private(
            crypto_call(false),
            key(true),
            PrivateText::new("private model").unwrap()
        ))
        .unwrap_err(),
        ProviderError::ProviderFailure
    );
    // This is structural port data, not authenticated NIP-44 ciphertext.
    assert_eq!(
        block_on(session.decrypt_private(
            crypto_call(false),
            key(true),
            PrivateCiphertext::parse(&"A".repeat(132)).unwrap()
        ))
        .unwrap_err(),
        ProviderError::ProviderFailure
    );
    assert_eq!(control.crypto_calls.load(Ordering::SeqCst), 2);
    *control.current.lock().unwrap() = state(true, true);
    assert_eq!(
        block_on(session.encrypt_private(
            crypto_call(false),
            key(true),
            PrivateText::new("private model").unwrap()
        ))
        .unwrap_err(),
        ProviderError::IdentityChanged
    );
    *control.current.lock().unwrap() = state(false, true);
    assert_eq!(
        block_on(session.decrypt_private(
            crypto_call(false),
            key(true),
            PrivateCiphertext::parse(&"A".repeat(132)).unwrap()
        ))
        .unwrap_err(),
        ProviderError::IdentityChanged
    );
    assert_eq!(control.crypto_calls.load(Ordering::SeqCst), 2);
    block_on(session.close()).unwrap();
    assert_eq!(
        block_on(session.encrypt_private(
            crypto_call(false),
            key(true),
            PrivateText::new("private model").unwrap()
        ))
        .unwrap_err(),
        ProviderError::CapabilityUnavailable
    );
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn scoped_private_crypto_retains_callbacks_until_the_issued_call_finishes() {
    let Fixture {
        session,
        control,
        provider,
        callbacks,
    } = scope();
    *control.current.lock().unwrap() = state(false, true);
    block_on(session.observe()).unwrap();
    let (send, receive) = oneshot::channel();
    *control.crypto_response.lock().unwrap() = Some(receive);
    let mut encrypt = Box::pin(session.encrypt_private(
        crypto_call(false),
        key(true),
        PrivateText::new("private model").unwrap(),
    ));
    let mut cx = Context::from_waker(noop_waker_ref());
    assert!(encrypt.as_mut().poll(&mut cx).is_pending());
    let mut close = Box::pin(session.close());
    assert!(close.as_mut().poll(&mut cx).is_pending());
    assert_eq!(session.state(), SessionState::Closing);
    assert_eq!(control.close_attempts.load(Ordering::SeqCst), 0);
    assert!(provider.upgrade().is_some() && callbacks.upgrade().is_some());
    send.send(()).unwrap();
    assert!(matches!(
        encrypt.as_mut().poll(&mut cx),
        Poll::Ready(Err(ProviderError::ProviderFailure))
    ));
    assert!(matches!(close.as_mut().poll(&mut cx), Poll::Ready(Ok(()))));
    assert!(provider.upgrade().is_none() && callbacks.upgrade().is_none());
}
