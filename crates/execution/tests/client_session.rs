#![cfg(all(feature = "client_session", not(target_arch = "wasm32")))]

use core::{
    future::Future,
    num::NonZeroU16,
    task::{Context, Poll},
};
use futures::{
    executor::block_on,
    future::poll_fn,
    task::{AtomicWaker, noop_waker_ref},
};
use radroots_execution::{
    Code,
    session::{ClientSession, HostFuture, SessionHost, SessionState},
};
use radroots_identity::PublicKey;
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
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering},
};

type Trace = Arc<Mutex<Vec<&'static str>>>;
struct Host(Trace);
impl Host {
    fn run(&self, name: &'static str) -> HostFuture<'_, radroots_execution::Result<()>> {
        Box::pin(async move {
            self.0.lock().unwrap().push(name);
            Ok(())
        })
    }
}
impl SessionHost for Host {
    fn start(&self) -> HostFuture<'_, radroots_execution::Result<()>> {
        self.run("start")
    }
    fn quiesce(&self) -> HostFuture<'_, radroots_execution::Result<()>> {
        self.run("quiesce")
    }
    fn disconnect(&self) -> HostFuture<'_, radroots_execution::Result<()>> {
        self.run("disconnect")
    }
    fn dispose_callbacks(&self) -> HostFuture<'_, radroots_execution::Result<()>> {
        self.run("host_callbacks")
    }
    fn close_storage(&self) -> HostFuture<'_, radroots_execution::Result<()>> {
        self.run("storage")
    }
}
struct Provider {
    trace: Trace,
    mode: AtomicU8,
    calls: AtomicUsize,
    quiesced: AtomicBool,
    fail_quiesce: AtomicBool,
    wake: AtomicWaker,
}
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
            self.calls.fetch_add(1, Ordering::SeqCst);
            poll_fn(|cx| {
                self.wake.register(cx.waker());
                if self.quiesced.load(Ordering::SeqCst) {
                    return Poll::Ready(Err(ProviderError::Cancelled));
                }
                match self.mode.load(Ordering::SeqCst) {
                    0 => Poll::Ready(Err(ProviderError::CapabilityUnavailable)),
                    2 => Poll::Pending,
                    3 => Poll::Ready(Err(ProviderError::IdentityChanged)),
                    _ => Poll::Ready(Ok(ProviderState::new(
                        PublicKey::from_hex(
                            "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
                        )
                        .unwrap(),
                        PrivateCryptoCapability::Unavailable,
                        PrivateCryptoCapability::Unavailable,
                        ReplayCapability::NonReplayable,
                    ))),
                }
            })
            .await
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
impl SignerCallbacks for Provider {
    fn quiesce(&self) -> BoxFuture<'_, Result<(), ProviderError>> {
        Box::pin(async move {
            self.trace.lock().unwrap().push("signer_quiesce");
            if self.fail_quiesce.swap(false, Ordering::SeqCst) {
                return Err(ProviderError::ProviderFailure);
            }
            self.quiesced.store(true, Ordering::SeqCst);
            self.wake.wake();
            Ok(())
        })
    }
    fn close(&self) -> BoxFuture<'_, Result<(), ProviderError>> {
        Box::pin(async move {
            self.trace.lock().unwrap().push("signer_callbacks");
            Ok(())
        })
    }
}
fn provider(trace: &Trace, mode: u8) -> Arc<Provider> {
    Arc::new(Provider {
        trace: trace.clone(),
        mode: AtomicU8::new(mode),
        calls: AtomicUsize::new(0),
        quiesced: AtomicBool::new(false),
        fail_quiesce: AtomicBool::new(false),
        wake: AtomicWaker::new(),
    })
}
fn handle(provider: &Arc<Provider>) -> SignerHandle {
    SignerHandle::new(provider.clone(), provider.clone())
}
fn client(trace: &Trace) -> ClientSession<Host> {
    ClientSession::new(Host(trace.clone()), NonZeroU16::MIN)
}

#[test]
fn attachment_before_start_is_owned_and_closed_in_the_actual_composition_order() {
    let trace = Trace::default();
    let client = client(&trace);
    let provider = provider(&trace, 1);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    let scope = block_on(client.attach_signer(handle(&provider))).unwrap();
    assert!(trace.lock().unwrap().is_empty());
    assert_eq!(client.state(), SessionState::Ready);
    assert!(client.signer_session().unwrap().observed().is_some());
    assert!(block_on(client.attach_signer(handle(&provider))).is_err());
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
    block_on(client.start()).unwrap();
    block_on(client.clone().close()).unwrap();
    block_on(client.close()).unwrap();
    assert_eq!(scope.state(), SessionState::Closed);
    assert_eq!(
        *trace.lock().unwrap(),
        [
            "start",
            "quiesce",
            "signer_quiesce",
            "disconnect",
            "signer_callbacks",
            "host_callbacks",
            "storage"
        ]
    );
    assert!(block_on(client.attach_signer(handle(&provider))).is_err());
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
}

#[test]
fn close_quiesces_a_pending_attachment_before_waiting_for_its_resource_permit() {
    let trace = Trace::default();
    let client = client(&trace);
    let provider = provider(&trace, 2);
    let mut attach = Box::pin(client.attach_signer(handle(&provider)));
    let mut cx = Context::from_waker(noop_waker_ref());
    assert!(attach.as_mut().poll(&mut cx).is_pending());
    assert!(client.signer_session().is_some());
    let mut close = Box::pin(client.close());
    assert!(close.as_mut().poll(&mut cx).is_pending());
    assert_eq!(client.state(), SessionState::Closing);
    assert!(matches!(attach.as_mut().poll(&mut cx), Poll::Ready(Err(_))));
    assert!(matches!(close.as_mut().poll(&mut cx), Poll::Ready(Ok(()))));
    assert_eq!(
        *trace.lock().unwrap(),
        [
            "quiesce",
            "signer_quiesce",
            "disconnect",
            "signer_callbacks",
            "host_callbacks",
            "storage"
        ]
    );
}

#[test]
fn failed_and_cancelled_attachment_remain_reachable_and_explicitly_replaceable() {
    for mode in [0, 2, 3] {
        let trace = Trace::default();
        let client = client(&trace);
        let original = provider(&trace, mode);
        if mode == 2 {
            let mut attach = Box::pin(client.attach_signer(handle(&original)));
            assert!(
                attach
                    .as_mut()
                    .poll(&mut Context::from_waker(noop_waker_ref()))
                    .is_pending()
            );
            drop(attach);
        } else {
            let error = match block_on(client.attach_signer(handle(&original))) {
                Err(error) => error,
                Ok(_) => panic!("failed observation accepted"),
            };
            assert_eq!(
                error.code(),
                if mode == 3 {
                    Code::IdentityChanged
                } else {
                    Code::SignerUnavailable
                }
            );
        }
        let retained = client.signer_session().unwrap();
        assert!(retained.observed().is_none());
        block_on(retained.close()).unwrap();
        let replacement = provider(&trace, 1);
        assert!(
            block_on(client.attach_signer(handle(&replacement)))
                .unwrap()
                .observed()
                .is_some()
        );
        block_on(client.close()).unwrap();
    }
}

#[test]
fn resuming_signer_quiesce_does_not_repeat_the_completed_host_hook() {
    let trace = Trace::default();
    let client = client(&trace);
    let provider = provider(&trace, 1);
    block_on(client.attach_signer(handle(&provider))).unwrap();
    provider.fail_quiesce.store(true, Ordering::SeqCst);
    assert!(block_on(client.close()).is_err());
    assert_eq!(*trace.lock().unwrap(), ["quiesce", "signer_quiesce"]);
    block_on(client.close()).unwrap();
    assert_eq!(
        *trace.lock().unwrap(),
        [
            "quiesce",
            "signer_quiesce",
            "signer_quiesce",
            "disconnect",
            "signer_callbacks",
            "host_callbacks",
            "storage"
        ]
    );
    let no_signer = self::client(&Trace::default());
    assert!(no_signer.signer_session().is_none());
    block_on(no_signer.close()).unwrap();
}

#[test]
fn overlapping_attachment_and_close_never_publish_an_unquiesced_scope() {
    use std::{
        sync::{Barrier, mpsc},
        thread,
        time::Duration,
    };
    for _ in 0..32 {
        let trace = Trace::default();
        let client = client(&trace);
        let provider = provider(&trace, 1);
        let barrier = Arc::new(Barrier::new(3));
        let (attached, observation) = mpsc::sync_channel(1);
        let (closed, completion) = mpsc::sync_channel(1);
        let joining = {
            let client = client.clone();
            let provider = provider.clone();
            let barrier = barrier.clone();
            thread::spawn(move || {
                barrier.wait();
                attached
                    .send(block_on(client.attach_signer(handle(&provider))))
                    .unwrap();
            })
        };
        let closing = {
            let client = client.clone();
            let barrier = barrier.clone();
            thread::spawn(move || {
                barrier.wait();
                closed.send(block_on(client.close())).unwrap();
            })
        };
        barrier.wait();
        let scope = observation
            .recv_timeout(Duration::from_secs(10))
            .expect("attachment completed");
        completion
            .recv_timeout(Duration::from_secs(10))
            .expect("close completed")
            .unwrap();
        joining.join().unwrap();
        closing.join().unwrap();
        assert_eq!(client.state(), SessionState::Closed);
        if let Ok(scope) = scope {
            assert_eq!(scope.state(), SessionState::Closed);
        }
        if let Some(scope) = client.signer_session() {
            assert_eq!(scope.state(), SessionState::Closed);
        }
        assert!(provider.calls.load(Ordering::SeqCst) <= 1);
        let trace = trace.lock().unwrap();
        assert_eq!(trace.last(), Some(&"storage"));
        if let Some(callback) = trace.iter().position(|item| *item == "signer_callbacks") {
            assert!(callback < trace.iter().position(|item| *item == "storage").unwrap());
        }
    }
}
