#![cfg(feature = "session")]

use std::{
    future::Future,
    num::NonZeroU16,
    pin::pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    task::{Context, Poll},
};

use futures::{
    executor::block_on,
    future::poll_fn,
    task::{AtomicWaker, noop_waker},
};
use radroots_execution::{
    Code, Error, OperationContext, Result, RetryClass,
    session::{HostFuture, Session, SessionHost, SessionState},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(usize)]
enum Hook {
    Start,
    Quiesce,
    Disconnect,
    Callbacks,
    Storage,
}

#[derive(Default)]
struct Observations {
    calls: Mutex<Vec<Hook>>,
    completed: AtomicUsize,
    blocked: AtomicUsize,
    fail: AtomicUsize,
    passive: AtomicBool,
    waker: AtomicWaker,
}

struct Host(Arc<Observations>);

impl Host {
    fn run(&self, hook: Hook) -> HostFuture<'_, Result<()>> {
        Box::pin(async move {
            self.0.calls.lock().unwrap().push(hook);
            let bit = 1 << hook as usize;
            poll_fn(|context| {
                if !self.0.passive.load(Ordering::SeqCst) {
                    self.0.waker.register(context.waker());
                }
                if self.0.blocked.load(Ordering::SeqCst) & bit != 0 {
                    return Poll::Pending;
                }
                if self.0.fail.fetch_and(!bit, Ordering::SeqCst) & bit != 0 {
                    return Poll::Ready(Err(Error::new(
                        Code::StorageFailure,
                        OperationContext::proposed(None),
                        RetryClass::Never,
                        None,
                    )
                    .unwrap()));
                }
                self.0.completed.fetch_or(bit, Ordering::SeqCst);
                Poll::Ready(Ok(()))
            })
            .await
        })
    }
}

impl SessionHost for Host {
    fn start(&self) -> HostFuture<'_, Result<()>> {
        self.run(Hook::Start)
    }
    fn quiesce(&self) -> HostFuture<'_, Result<()>> {
        self.run(Hook::Quiesce)
    }
    fn disconnect(&self) -> HostFuture<'_, Result<()>> {
        self.run(Hook::Disconnect)
    }
    fn dispose_callbacks(&self) -> HostFuture<'_, Result<()>> {
        self.run(Hook::Callbacks)
    }
    fn close_storage(&self) -> HostFuture<'_, Result<()>> {
        self.run(Hook::Storage)
    }
}

fn fixture(limit: u16) -> (Session<Host>, Arc<Observations>) {
    let observations = Arc::new(Observations::default());
    (
        Session::new(Host(observations.clone()), NonZeroU16::new(limit).unwrap()),
        observations,
    )
}

fn poll<F: Future>(future: std::pin::Pin<&mut F>) -> Poll<F::Output> {
    future.poll(&mut Context::from_waker(&noop_waker()))
}

#[test]
fn construction_and_unpolled_futures_have_no_effects() {
    let (session, observed) = fixture(2);
    assert_eq!(session.state(), SessionState::Ready);
    drop(session.start());
    drop(session.close());
    assert_eq!(session.state(), SessionState::Ready);
    assert!(observed.calls.lock().unwrap().is_empty());
    assert_eq!(
        session.admit().err().unwrap().code(),
        Code::CapabilityUnavailable
    );
    drop(session);
    assert!(observed.calls.lock().unwrap().is_empty());
}

#[test]
fn startup_and_close_are_shared_idempotent_and_storage_is_last() {
    let (session, observed) = fixture(1);
    let clone = session.clone();
    block_on(async {
        let (one, two) = futures::join!(session.start(), clone.start());
        one.unwrap();
        two.unwrap();
        assert_eq!(session.state(), SessionState::Running);
        let (one, two) = futures::join!(session.close(), clone.close());
        one.unwrap();
        two.unwrap();
    });
    assert_eq!(
        observed.calls.lock().unwrap().as_slice(),
        &[
            Hook::Start,
            Hook::Quiesce,
            Hook::Disconnect,
            Hook::Callbacks,
            Hook::Storage
        ]
    );
    assert_eq!(clone.state(), SessionState::Closed);
    assert!(block_on(clone.start()).is_err());
    assert!(clone.admit().is_err());
    block_on(clone.close()).unwrap();
    assert_eq!(observed.calls.lock().unwrap().len(), 5);
}

#[test]
fn admission_is_bounded_and_close_waits_for_owned_work() {
    let (session, observed) = fixture(2);
    block_on(session.start()).unwrap();
    let first = session.admit().unwrap();
    let second = session.clone().admit().unwrap();
    let error = session.admit().err().unwrap();
    assert_eq!(error.code(), Code::CapabilityUnavailable);
    assert_eq!(error.context(), OperationContext::proposed(None));
    let mut closing = pin!(session.close());
    assert!(poll(closing.as_mut()).is_pending());
    assert_eq!(session.state(), SessionState::Closing);
    assert!(session.admit().is_err());
    assert_eq!(
        observed.calls.lock().unwrap().as_slice(),
        &[Hook::Start, Hook::Quiesce]
    );
    drop(first);
    assert!(poll(closing.as_mut()).is_pending());
    drop(second);
    assert_eq!(poll(closing.as_mut()), Poll::Ready(Ok(())));
    assert_eq!(observed.calls.lock().unwrap().last(), Some(&Hook::Storage));
}

#[test]
fn cancellation_and_known_failure_resume_only_the_unfinished_hook() {
    for hook in [
        Hook::Quiesce,
        Hook::Disconnect,
        Hook::Callbacks,
        Hook::Storage,
    ] {
        for failure in [false, true] {
            let (session, observed) = fixture(1);
            block_on(session.start()).unwrap();
            let bit = 1 << hook as usize;
            if failure {
                observed.fail.store(bit, Ordering::SeqCst);
                assert_eq!(
                    block_on(session.close()).unwrap_err().code(),
                    Code::StorageFailure
                );
            } else {
                observed.blocked.store(bit, Ordering::SeqCst);
                let mut close = Box::pin(session.close());
                assert!(poll(close.as_mut()).is_pending());
                drop(close);
                observed.blocked.store(0, Ordering::SeqCst);
            }
            assert_eq!(session.state(), SessionState::Closing);
            assert!(session.admit().is_err());
            assert!(block_on(session.start()).is_err());
            block_on(session.close()).unwrap();
            let calls = observed.calls.lock().unwrap();
            let expected: Vec<_> = [
                Hook::Start,
                Hook::Quiesce,
                Hook::Disconnect,
                Hook::Callbacks,
                Hook::Storage,
            ]
            .into_iter()
            .flat_map(|value| {
                if value == hook {
                    vec![value, value]
                } else {
                    vec![value]
                }
            })
            .collect();
            assert_eq!(*calls, expected);
            assert_eq!(session.state(), SessionState::Closed);
        }
    }
}

#[test]
fn canceled_drain_resumes_and_concurrent_closers_share_completion() {
    let (session, observed) = fixture(1);
    block_on(session.start()).unwrap();
    let work = session.admit().unwrap();
    let mut close = Box::pin(session.close());
    assert!(poll(close.as_mut()).is_pending());
    drop(close);
    drop(work);
    let clone = session.clone();
    block_on(async {
        let (a, b) = futures::join!(clone.close(), session.close());
        a.unwrap();
        b.unwrap();
    });
    assert_eq!(
        observed.calls.lock().unwrap().as_slice(),
        &[
            Hook::Start,
            Hook::Quiesce,
            Hook::Disconnect,
            Hook::Callbacks,
            Hook::Storage
        ]
    );
}

#[test]
fn close_interrupts_pending_start_and_never_reopens_admission() {
    let (session, observed) = fixture(1);
    observed
        .blocked
        .store(1 << Hook::Start as usize, Ordering::SeqCst);
    let mut start = Box::pin(session.start());
    assert!(poll(start.as_mut()).is_pending());
    assert_eq!(session.state(), SessionState::Starting);
    assert!(session.admit().is_err());
    let mut close = Box::pin(session.close());
    assert!(poll(close.as_mut()).is_pending());
    assert_eq!(session.state(), SessionState::Closing);
    assert!(matches!(poll(start.as_mut()), Poll::Ready(Err(_))));
    assert_eq!(poll(close.as_mut()), Poll::Ready(Ok(())));
    assert_eq!(observed.completed.load(Ordering::SeqCst) & 1, 0);
    assert_eq!(observed.calls.lock().unwrap().last(), Some(&Hook::Storage));
}

#[test]
fn canceled_and_failed_startup_cannot_admit_and_may_be_resumed_or_closed() {
    let (session, observed) = fixture(1);
    observed.blocked.store(1, Ordering::SeqCst);
    let mut start = Box::pin(session.start());
    assert!(poll(start.as_mut()).is_pending());
    drop(start);
    assert!(session.admit().is_err());
    observed.blocked.store(0, Ordering::SeqCst);
    observed.fail.store(1, Ordering::SeqCst);
    assert!(block_on(session.start()).is_err());
    assert_eq!(session.state(), SessionState::Starting);
    assert!(session.admit().is_err());
    block_on(session.start()).unwrap();
    drop(session.admit().unwrap());
    block_on(session.close()).unwrap();
    let (unstarted, _) = fixture(1);
    block_on(unstarted.close()).unwrap();
    assert_eq!(unstarted.state(), SessionState::Closed);
}

#[test]
fn work_permit_retains_resources_after_last_client_handle() {
    let (session, observed) = fixture(1);
    block_on(session.start()).unwrap();
    let permit = session.admit().unwrap();
    assert_eq!(Arc::strong_count(&observed), 2);
    drop(session);
    assert_eq!(Arc::strong_count(&observed), 2);
    drop(permit);
    assert_eq!(Arc::strong_count(&observed), 1);
    assert_eq!(observed.calls.lock().unwrap().as_slice(), &[Hook::Start]);
}

#[test]
fn permit_completion_and_close_wake_their_pending_observers() {
    struct WakeCount(AtomicUsize);
    impl futures::task::ArcWake for WakeCount {
        fn wake_by_ref(arc: &Arc<Self>) {
            arc.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    let counter = Arc::new(WakeCount(AtomicUsize::new(0)));
    let waker = futures::task::waker(counter.clone());
    let mut context = Context::from_waker(&waker);
    let (session, _) = fixture(1);
    block_on(session.start()).unwrap();
    let work = session.admit().unwrap();
    let mut close = Box::pin(session.close());
    assert!(close.as_mut().poll(&mut context).is_pending());
    drop(work);
    assert_eq!(counter.0.load(Ordering::SeqCst), 1);
    assert_eq!(close.as_mut().poll(&mut context), Poll::Ready(Ok(())));

    let (session, observed) = fixture(1);
    observed.blocked.store(1, Ordering::SeqCst);
    let mut start = Box::pin(session.start());
    assert!(start.as_mut().poll(&mut context).is_pending());
    let before = counter.0.load(Ordering::SeqCst);
    let mut close = Box::pin(session.close());
    assert!(poll(close.as_mut()).is_pending());
    assert!(counter.0.load(Ordering::SeqCst) > before);
    assert!(matches!(poll(start.as_mut()), Poll::Ready(Err(_))));
    assert_eq!(poll(close.as_mut()), Poll::Ready(Ok(())));
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn replacing_or_clearing_wakers_allows_reentrant_resource_observation() {
    use std::{sync::mpsc, task::Wake, time::Duration};

    struct InspectOnDrop {
        session: Session<Host>,
        drops: Arc<AtomicUsize>,
        close: bool,
    }
    impl Wake for InspectOnDrop {
        fn wake(self: Arc<Self>) {
            let _state = self.session.state();
        }
    }
    impl Drop for InspectOnDrop {
        fn drop(&mut self) {
            let _state = self.session.state();
            if self.close {
                let mut closing = Box::pin(self.session.close());
                assert!(poll(closing.as_mut()).is_pending());
            }
            self.drops.fetch_add(1, Ordering::SeqCst);
        }
    }

    let (done, result) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        let (session, observed) = fixture(1);
        let drops = Arc::new(AtomicUsize::new(0));
        observed.passive.store(true, Ordering::SeqCst);
        observed.blocked.store(1, Ordering::SeqCst);
        let mut start = Box::pin(session.start());
        {
            let waker = Arc::new(InspectOnDrop {
                session: session.clone(),
                drops: drops.clone(),
                close: false,
            })
            .into();
            assert!(
                start
                    .as_mut()
                    .poll(&mut Context::from_waker(&waker))
                    .is_pending()
            );
        }
        // Replacing the last reference to this waker runs its destructor.
        assert!(poll(start.as_mut()).is_pending());
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        assert!(poll(start.as_mut()).is_pending());
        {
            let waker = Arc::new(InspectOnDrop {
                session: session.clone(),
                drops: drops.clone(),
                close: false,
            })
            .into();
            assert!(
                start
                    .as_mut()
                    .poll(&mut Context::from_waker(&waker))
                    .is_pending()
            );
        }
        observed.blocked.store(0, Ordering::SeqCst);
        let waker = Arc::new(InspectOnDrop {
            session: session.clone(),
            drops: drops.clone(),
            close: false,
        })
        .into();
        assert_eq!(
            start.as_mut().poll(&mut Context::from_waker(&waker)),
            Poll::Ready(Ok(()))
        );
        drop(waker);
        assert_eq!(drops.load(Ordering::SeqCst), 3);
        drop(start);

        let work = session.admit().unwrap();
        let mut close = Box::pin(session.close());
        {
            let waker = Arc::new(InspectOnDrop {
                session: session.clone(),
                drops: drops.clone(),
                close: false,
            })
            .into();
            assert!(
                close
                    .as_mut()
                    .poll(&mut Context::from_waker(&waker))
                    .is_pending()
            );
        }
        assert!(poll(close.as_mut()).is_pending());
        assert_eq!(drops.load(Ordering::SeqCst), 4);
        drop(work);
        assert_eq!(poll(close.as_mut()), Poll::Ready(Ok(())));

        let (session, observed) = fixture(1);
        observed.passive.store(true, Ordering::SeqCst);
        observed.blocked.store(1, Ordering::SeqCst);
        let mut start = Box::pin(session.start());
        {
            let waker = Arc::new(InspectOnDrop {
                session: session.clone(),
                drops: drops.clone(),
                close: true,
            })
            .into();
            assert!(
                start
                    .as_mut()
                    .poll(&mut Context::from_waker(&waker))
                    .is_pending()
            );
        }
        observed.blocked.store(0, Ordering::SeqCst);
        // Reentrant close races with a successful host startup result.
        assert!(matches!(poll(start.as_mut()), Poll::Ready(Err(_))));
        assert_eq!(session.state(), SessionState::Closing);
        assert!(session.admit().is_err());
        block_on(session.close()).unwrap();

        let (session, observed) = fixture(1);
        observed.passive.store(true, Ordering::SeqCst);
        observed.blocked.store(1, Ordering::SeqCst);
        let mut start = Box::pin(session.start());
        {
            let waker = Arc::new(InspectOnDrop {
                session: session.clone(),
                drops: drops.clone(),
                close: false,
            })
            .into();
            assert!(
                start
                    .as_mut()
                    .poll(&mut Context::from_waker(&waker))
                    .is_pending()
            );
        }
        let mut close = Box::pin(session.close());
        // Both wake and final destructor may observe the session reentrantly.
        assert!(poll(close.as_mut()).is_pending());
        assert!(matches!(poll(start.as_mut()), Poll::Ready(Err(_))));
        assert_eq!(poll(close.as_mut()), Poll::Ready(Ok(())));
        done.send(()).unwrap();
    });
    // A regression must fail within a bound rather than hang the test process.
    result
        .recv_timeout(Duration::from_secs(5))
        .expect("waker destructor deadlocked session bookkeeping");
    worker.join().unwrap();
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn native_handles_and_futures_are_send_and_concurrent_admission_is_bounded() {
    fn send_sync<T: Send + Sync>() {}
    fn send<T: Send>(_: T) {}
    send_sync::<Session<Host>>();
    let (session, _) = fixture(3);
    send(session.start());
    send(session.close());
    block_on(session.start()).unwrap();
    let admitted: Vec<_> = std::thread::scope(|scope| {
        let attempts: Vec<_> = (0..32).map(|_| scope.spawn(|| session.admit())).collect();
        attempts
            .into_iter()
            .filter_map(|attempt| attempt.join().unwrap().ok())
            .collect()
    });
    assert_eq!(admitted.len(), 3);
    drop(admitted);
    block_on(session.close()).unwrap();
}

#[cfg(target_arch = "wasm32")]
#[test]
fn browser_host_futures_accept_thread_local_callback_state() {
    use std::{cell::Cell, rc::Rc};
    struct Local(Rc<Cell<usize>>);
    impl SessionHost for Local {
        fn start(&self) -> HostFuture<'_, Result<()>> {
            Box::pin(async {
                self.0.set(1);
                Ok(())
            })
        }
        fn quiesce(&self) -> HostFuture<'_, Result<()>> {
            Box::pin(async { Ok(()) })
        }
        fn disconnect(&self) -> HostFuture<'_, Result<()>> {
            Box::pin(async { Ok(()) })
        }
        fn dispose_callbacks(&self) -> HostFuture<'_, Result<()>> {
            Box::pin(async { Ok(()) })
        }
        fn close_storage(&self) -> HostFuture<'_, Result<()>> {
            Box::pin(async { Ok(()) })
        }
    }
    let calls = Rc::new(Cell::new(0));
    let client = Session::new(Local(calls.clone()), NonZeroU16::new(1).unwrap());
    block_on(client.start()).unwrap();
    assert_eq!(calls.get(), 1);
    block_on(client.close()).unwrap();
}
