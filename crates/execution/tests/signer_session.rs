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
    current: Mutex<ProviderState>,
    calls: AtomicUsize,
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
        current: Mutex::new(state(false, false)),
        calls: AtomicUsize::new(0),
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
    block_on(session.close()).unwrap();
}
