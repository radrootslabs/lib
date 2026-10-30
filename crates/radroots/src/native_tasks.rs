//! One explicitly started native executor for library-owned command futures.
use futures::{
    FutureExt, StreamExt,
    future::{Either, Shared, select},
    stream::FuturesUnordered,
};
use radroots_execution::{
    Code, Error, OperationContext, RetryClass,
    session::{HostFuture, tasks::TaskHost},
};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    thread::JoinHandle,
};
use tokio::sync::{mpsc, oneshot};

const CAPACITY: usize = 32;
const UNAVAILABLE: Error = match Error::new(
    Code::CapabilityUnavailable,
    OperationContext::proposed(None),
    RetryClass::Never,
    None,
) {
    Ok(error) => error,
    Err(_) => panic!("invalid constant task-host error"),
};
type Completion = Shared<HostFuture<'static, crate::Result<()>>>;
enum Job {
    Work(HostFuture<'static, ()>),
    Control(
        HostFuture<'static, crate::Result<()>>,
        oneshot::Sender<crate::Result<()>>,
    ),
    Stop,
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum ControlKind {
    Drain,
    Disconnect,
}
struct Control {
    id: u64,
    kind: ControlKind,
    completed: Completion,
}
struct Running {
    sender: mpsc::Sender<Job>,
    ready: Completion,
    done: Completion,
    thread: Option<JoinHandle<()>>,
    stop_sent: bool,
}
#[derive(Default)]
struct State {
    running: Option<Running>,
    accepting: bool,
    quiesced: bool,
    closed: bool,
    control: Option<Control>,
    next_control: u64,
}
#[derive(Default)]
pub(crate) struct NativeTasks {
    state: Mutex<State>,
    closing: futures::lock::Mutex<()>,
    admitted: Arc<AtomicUsize>,
}
struct WorkLease(Arc<AtomicUsize>);
impl Drop for WorkLease {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}
fn completion(receiver: oneshot::Receiver<crate::Result<()>>) -> Completion {
    let future: HostFuture<'static, crate::Result<()>> =
        Box::pin(async move { receiver.await.unwrap_or(Err(UNAVAILABLE)) });
    future.shared()
}
fn run_worker(
    mut receiver: mpsc::Receiver<Job>,
    ready: oneshot::Sender<crate::Result<()>>,
    done: oneshot::Sender<crate::Result<()>>,
    build: impl FnOnce() -> std::io::Result<tokio::runtime::Runtime>,
) {
    let runtime = match build() {
        Ok(runtime) => runtime,
        Err(_) => {
            let _ = ready.send(Err(UNAVAILABLE));
            let _ = done.send(Err(UNAVAILABLE));
            return;
        }
    };
    let _ = ready.send(Ok(()));
    runtime.block_on(async move {
        let mut active = FuturesUnordered::new();
        loop {
            let next = if active.is_empty() {
                receiver.recv().await
            } else {
                match select(Box::pin(receiver.recv()), active.next()).await {
                    Either::Left((job, _)) => job,
                    Either::Right((_, _)) => continue,
                }
            };
            let Some(job) = next else {
                while active.next().await.is_some() {}
                break;
            };
            match job {
                Job::Work(task) => active.push(task),
                Job::Control(task, reply) => {
                    // Later submissions cannot cross a control barrier.
                    while active.next().await.is_some() {}
                    let result = task.await;
                    let _ = reply.send(result);
                }
                Job::Stop => {
                    while active.next().await.is_some() {}
                    break;
                }
            }
        }
    });
    drop(runtime);
    let _ = done.send(Ok(()));
}
impl NativeTasks {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) async fn start(&self) -> crate::Result<()> {
        let ready = {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if state.quiesced || state.closed {
                return Err(UNAVAILABLE);
            }
            if state.running.is_none() {
                let (sender, receiver) = mpsc::channel::<Job>(CAPACITY);
                let (ready, ready_receiver) = oneshot::channel();
                let (done, done_receiver) = oneshot::channel();
                let thread = std::thread::Builder::new()
                    .name("radroots_commands".into())
                    .spawn(move || {
                        run_worker(receiver, ready, done, || {
                            tokio::runtime::Builder::new_current_thread()
                                .enable_all()
                                .build()
                        })
                    })
                    .map_err(|_| UNAVAILABLE)?;
                state.running = Some(Running {
                    sender,
                    ready: completion(ready_receiver),
                    done: completion(done_receiver),
                    thread: Some(thread),
                    stop_sent: false,
                });
            }
            state.running.as_ref().ok_or(UNAVAILABLE)?.ready.clone()
        };
        ready.await?;
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.quiesced || state.closed {
            return Err(UNAVAILABLE);
        }
        state.accepting = true;
        Ok(())
    }

    pub(crate) fn quiesce(&self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.accepting = false;
        state.quiesced = true;
    }

    pub(crate) async fn control(
        &self,
        kind: ControlKind,
        task: HostFuture<'static, crate::Result<()>>,
    ) -> crate::Result<()> {
        let ready = {
            let state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if state.closed {
                return Err(UNAVAILABLE);
            }
            state.running.as_ref().map(|running| running.ready.clone())
        };
        let Some(ready) = ready else {
            return Ok(());
        };
        if ready.await.is_err() {
            return Ok(());
        }
        let (id, completed) = {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if state.closed {
                return Err(UNAVAILABLE);
            }
            if let Some(control) = &state.control {
                if control.kind != kind {
                    return Err(UNAVAILABLE);
                }
                (control.id, control.completed.clone())
            } else {
                let id = state.next_control.checked_add(1).ok_or(UNAVAILABLE)?;
                let Some(running) = state.running.as_ref() else {
                    return Ok(());
                };
                if running.stop_sent {
                    return Err(UNAVAILABLE);
                }
                let (reply, wait) = oneshot::channel();
                if let Err(error) = running.sender.try_send(Job::Control(task, reply)) {
                    drop(state);
                    drop(error);
                    return Err(UNAVAILABLE);
                }
                let completed = completion(wait);
                state.next_control = id;
                state.control = Some(Control {
                    id,
                    kind,
                    completed: completed.clone(),
                });
                (id, completed)
            }
        };
        let result = completed.await;
        let retired = {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if state
                .control
                .as_ref()
                .is_some_and(|control| control.id == id)
            {
                state.control.take()
            } else {
                None
            }
        };
        drop(retired);
        result
    }

    pub(crate) async fn close(&self) -> crate::Result<()> {
        self.quiesce();
        let _closing = self.closing.lock().await;
        let ready = {
            let state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            state.running.as_ref().map(|running| running.ready.clone())
        };
        let failed_start = match ready {
            Some(ready) => ready.await.is_err(),
            None => false,
        };
        let done = {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if state.closed {
                return Ok(());
            }
            let Some(running) = state.running.as_mut() else {
                state.closed = true;
                return Ok(());
            };
            if !failed_start && !running.stop_sent {
                running
                    .sender
                    .try_send(Job::Stop)
                    .map_err(|_| UNAVAILABLE)?;
                running.stop_sent = true;
            }
            running.done.clone()
        };
        let result = done.await;
        if !failed_start {
            result?;
        }
        let thread = {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            state
                .running
                .as_mut()
                .and_then(|running| running.thread.take())
        };
        if let Some(thread) = thread {
            let result = thread.join().map_err(|_| UNAVAILABLE);
            if !failed_start {
                result?;
            }
        }
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .closed = true;
        Ok(())
    }
}
impl TaskHost for NativeTasks {
    fn submit(&self, task: HostFuture<'static, ()>) -> crate::Result<()> {
        let state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !state.accepting || state.quiesced || state.closed {
            return Err(UNAVAILABLE);
        }
        let running = state.running.as_ref().ok_or(UNAVAILABLE)?;
        if self
            .admitted
            .try_update(Ordering::AcqRel, Ordering::Acquire, |used| {
                used.checked_add(1).filter(|next| *next <= CAPACITY)
            })
            .is_err()
        {
            drop(state);
            return Err(UNAVAILABLE);
        }
        let lease = WorkLease(self.admitted.clone());
        let task = Box::pin(async move {
            task.await;
            drop(lease);
        });
        let result = running.sender.try_send(Job::Work(task));
        drop(state);
        result.map_err(|_| UNAVAILABLE)
    }
}

#[cfg(test)]
mod tests {
    use super::{CAPACITY, ControlKind, NativeTasks, Running, UNAVAILABLE, completion, run_worker};
    use futures::{FutureExt, executor::block_on, task::noop_waker_ref};
    use radroots_execution::session::{HostFuture, tasks::TaskHost};
    use std::{
        future::Future,
        pin::Pin,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        task::{Context, Poll},
        time::Duration,
    };
    use tokio::sync::{mpsc, oneshot};

    fn pending<F: Future>(future: Pin<&mut F>) {
        assert!(
            future
                .poll(&mut Context::from_waker(noop_waker_ref()))
                .is_pending()
        );
    }
    fn gate_ready(tasks: &NativeTasks) -> oneshot::Sender<()> {
        let mut state = tasks.state.lock().unwrap();
        let running = state.running.as_mut().unwrap();
        let original = running.ready.clone();
        let (release, wait) = oneshot::channel();
        let gated: HostFuture<'static, crate::Result<()>> = Box::pin(async move {
            original.await?;
            wait.await.map_err(|_| UNAVAILABLE)
        });
        running.ready = gated.shared();
        release
    }

    #[test]
    fn never_started_driver_quiesces_and_closes_without_dispatch() {
        let tasks = NativeTasks::new();
        block_on(tasks.control(
            ControlKind::Drain,
            Box::pin(async { panic!("unstarted control dispatched") }),
        ))
        .unwrap();
        tasks.quiesce();
        assert!(block_on(tasks.start()).is_err());
        block_on(tasks.close()).unwrap();
        assert!(block_on(tasks.control(ControlKind::Drain, Box::pin(async { Ok(()) }))).is_err());
        block_on(tasks.close()).unwrap();
    }

    #[test]
    fn cancelled_close_retains_stop_progress_and_refuses_late_control() {
        let tasks = NativeTasks::new();
        block_on(tasks.start()).unwrap();
        let (release, wait) = oneshot::channel();
        tasks
            .submit(Box::pin(async move {
                wait.await.unwrap();
            }))
            .unwrap();
        let mut closing = Box::pin(tasks.close());
        pending(closing.as_mut());
        drop(closing);
        assert!(block_on(tasks.control(ControlKind::Drain, Box::pin(async { Ok(()) }))).is_err());
        release.send(()).unwrap();
        block_on(tasks.close()).unwrap();
        assert!(
            tasks
                .state
                .lock()
                .unwrap()
                .running
                .as_ref()
                .unwrap()
                .thread
                .is_none()
        );
    }

    #[test]
    fn coalesced_control_failure_retry_and_stale_observer_keep_the_current_generation() {
        let tasks = NativeTasks::new();
        block_on(tasks.start()).unwrap();
        block_on(tasks.start()).unwrap();
        let (release, wait) = oneshot::channel();
        let mut first = Box::pin(tasks.control(
            ControlKind::Drain,
            Box::pin(async move {
                wait.await.unwrap();
                Err(UNAVAILABLE)
            }),
        ));
        pending(first.as_mut());
        let mut stale = Box::pin(tasks.control(
            ControlKind::Drain,
            Box::pin(async { panic!("coalesced work dispatched twice") }),
        ));
        pending(stale.as_mut());
        assert!(
            block_on(tasks.control(ControlKind::Disconnect, Box::pin(async { Ok(()) }))).is_err()
        );
        release.send(()).unwrap();
        assert!(block_on(first).is_err());
        let (release, wait) = oneshot::channel();
        let mut current = Box::pin(tasks.control(
            ControlKind::Drain,
            Box::pin(async move {
                wait.await.unwrap();
                Ok(())
            }),
        ));
        pending(current.as_mut());
        let generation = tasks.state.lock().unwrap().control.as_ref().unwrap().id;
        assert!(block_on(stale).is_err());
        assert_eq!(
            tasks.state.lock().unwrap().control.as_ref().unwrap().id,
            generation
        );
        release.send(()).unwrap();
        block_on(current).unwrap();
        block_on(tasks.close()).unwrap();
    }

    #[test]
    fn delayed_ready_observation_cannot_reopen_a_closed_driver_or_dispatch_control() {
        let tasks = NativeTasks::new();
        block_on(tasks.start()).unwrap();
        let release = gate_ready(&tasks);
        let mut start = Box::pin(tasks.start());
        pending(start.as_mut());
        let mut control = Box::pin(tasks.control(
            ControlKind::Drain,
            Box::pin(async { panic!("closed control dispatched") }),
        ));
        pending(control.as_mut());
        release.send(()).unwrap();
        block_on(tasks.close()).unwrap();
        assert!(block_on(start).is_err());
        assert!(block_on(control).is_err());
    }

    fn injected_driver(bootstrap_failure: bool, panic_after_drain: bool) -> NativeTasks {
        let tasks = NativeTasks::new();
        let (sender, receiver) = mpsc::channel(CAPACITY);
        let (ready, ready_receiver) = oneshot::channel();
        let (done, done_receiver) = oneshot::channel();
        let thread = std::thread::spawn(move || {
            run_worker(receiver, ready, done, || {
                if bootstrap_failure {
                    Err(std::io::Error::other("injected bootstrap failure"))
                } else {
                    tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                }
            });
            assert!(!panic_after_drain, "injected post-drain worker failure");
        });
        tasks.state.lock().unwrap().running = Some(Running {
            sender,
            ready: completion(ready_receiver),
            done: completion(done_receiver),
            thread: Some(thread),
            stop_sent: false,
        });
        tasks
    }

    #[test]
    fn injected_bootstrap_failure_and_post_drain_join_failure_preserve_actual_close_progress() {
        let failed = injected_driver(true, false);
        assert!(block_on(failed.start()).is_err());
        block_on(failed.control(
            ControlKind::Drain,
            Box::pin(async { panic!("failed bootstrap dispatched") }),
        ))
        .unwrap();
        block_on(failed.close()).unwrap();
        let failed = injected_driver(false, true);
        block_on(failed.start()).unwrap();
        assert!(block_on(failed.close()).is_err());
        assert!(!failed.state.lock().unwrap().closed);
        block_on(failed.close()).unwrap();
    }

    #[test]
    fn dropping_an_empty_driver_ends_its_actual_channel_without_claiming_close() {
        let tasks = NativeTasks::new();
        block_on(tasks.start()).unwrap();
        let (done, thread) = {
            let mut state = tasks.state.lock().unwrap();
            let running = state.running.as_mut().unwrap();
            (running.done.clone(), running.thread.take().unwrap())
        };
        drop(tasks);
        block_on(done).unwrap();
        thread.join().unwrap();
    }

    #[test]
    fn dropping_the_driver_does_not_cancel_accepted_pending_work() {
        let tasks = NativeTasks::new();
        block_on(tasks.start()).unwrap();
        let (started, observed) = std::sync::mpsc::sync_channel(1);
        let (release, wait) = oneshot::channel();
        let calls = Arc::new(AtomicUsize::new(0));
        let completed = calls.clone();
        tasks
            .submit(Box::pin(async move {
                started.send(()).unwrap();
                wait.await.unwrap();
                completed.fetch_add(1, Ordering::SeqCst);
            }))
            .unwrap();
        observed.recv_timeout(Duration::from_secs(10)).unwrap();
        let (done, thread) = {
            let mut state = tasks.state.lock().unwrap();
            let running = state.running.as_mut().unwrap();
            (running.done.clone(), running.thread.take().unwrap())
        };
        drop(tasks);
        release.send(()).unwrap();
        block_on(done).unwrap();
        thread.join().unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn accepted_tasks_and_control_survive_lost_observation_on_the_actual_native_executor() {
        let tasks = NativeTasks::new();
        let count = Arc::new(AtomicUsize::new(0));
        assert!(tasks.submit(Box::pin(async {})).is_err());
        drop(tasks.start());
        assert!(tasks.state.lock().unwrap().running.is_none());
        block_on(tasks.start()).unwrap();
        let (ready, observed) = std::sync::mpsc::sync_channel(1);
        let (release, wait) = oneshot::channel();
        let calls = count.clone();
        tasks
            .submit(Box::pin(async move {
                ready.send(std::thread::current().id()).unwrap();
                wait.await.unwrap();
                calls.fetch_add(1, Ordering::SeqCst);
            }))
            .unwrap();
        assert_ne!(
            observed.recv_timeout(Duration::from_secs(10)).unwrap(),
            std::thread::current().id()
        );
        let calls = count.clone();
        tasks
            .submit(Box::pin(async move {
                calls.fetch_add(10, Ordering::SeqCst);
            }))
            .unwrap();
        tasks.quiesce();
        assert!(tasks.submit(Box::pin(async {})).is_err());
        let calls = count.clone();
        let mut drain = Box::pin(tasks.control(
            ControlKind::Drain,
            Box::pin(async move {
                calls.fetch_add(100, Ordering::SeqCst);
                Ok(())
            }),
        ));
        assert!(matches!(
            drain
                .as_mut()
                .poll(&mut Context::from_waker(noop_waker_ref())),
            Poll::Pending
        ));
        drop(drain);
        release.send(()).unwrap();
        let calls = count.clone();
        block_on(tasks.control(
            ControlKind::Drain,
            Box::pin(async move {
                calls.fetch_add(1000, Ordering::SeqCst);
                Ok(())
            }),
        ))
        .unwrap();
        assert_eq!(count.load(Ordering::SeqCst), 111);
        block_on(tasks.control(ControlKind::Disconnect, Box::pin(async { Ok(()) }))).unwrap();
        block_on(tasks.close()).unwrap();
        block_on(tasks.close()).unwrap();
        assert!(block_on(tasks.start()).is_err());
        assert!(
            tasks
                .state
                .lock()
                .unwrap()
                .running
                .as_ref()
                .unwrap()
                .thread
                .is_none()
        );
    }

    struct ReentrantDrop(Arc<NativeTasks>, Arc<AtomicUsize>);
    impl Drop for ReentrantDrop {
        fn drop(&mut self) {
            assert!(self.0.state.try_lock().is_ok());
            self.1.fetch_add(1, Ordering::SeqCst);
        }
    }
    #[test]
    fn pending_work_allows_reads_but_controls_and_later_work_keep_their_barrier() {
        let tasks = NativeTasks::new();
        block_on(tasks.start()).unwrap();
        let (release, wait) = oneshot::channel();
        tasks
            .submit(Box::pin(async move {
                wait.await.unwrap();
            }))
            .unwrap();
        let (read, observed) = std::sync::mpsc::sync_channel(1);
        tasks
            .submit(Box::pin(async move {
                read.send(()).unwrap();
            }))
            .unwrap();
        observed.recv_timeout(Duration::from_secs(10)).unwrap();
        let (controlled, control_seen) = std::sync::mpsc::sync_channel(1);
        let (release_control, wait_control) = oneshot::channel();
        let mut control = Box::pin(tasks.control(
            ControlKind::Drain,
            Box::pin(async move {
                controlled.send(()).unwrap();
                wait_control.await.unwrap();
                Ok(())
            }),
        ));
        pending(control.as_mut());
        let (later, later_seen) = std::sync::mpsc::sync_channel(1);
        tasks
            .submit(Box::pin(async move {
                later.send(()).unwrap();
            }))
            .unwrap();
        assert!(matches!(
            control_seen.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Empty)
        ));
        release.send(()).unwrap();
        control_seen.recv_timeout(Duration::from_secs(10)).unwrap();
        assert!(matches!(
            later_seen.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Empty)
        ));
        release_control.send(()).unwrap();
        block_on(control).unwrap();
        later_seen.recv_timeout(Duration::from_secs(10)).unwrap();
        block_on(tasks.close()).unwrap();
        assert_eq!(tasks.admitted.load(Ordering::Acquire), 0);
    }
    #[test]
    fn native_queue_refusal_drops_captured_roots_outside_the_gate_and_close_drains_accepted_tasks()
    {
        let tasks = Arc::new(NativeTasks::new());
        block_on(tasks.start()).unwrap();
        let (ready, observed) = std::sync::mpsc::sync_channel(1);
        let (release, wait) = oneshot::channel();
        let wait = wait.shared();
        let first_wait = wait.clone();
        tasks
            .submit(Box::pin(async move {
                ready.send(()).unwrap();
                first_wait.await.unwrap();
            }))
            .unwrap();
        observed.recv_timeout(Duration::from_secs(10)).unwrap();
        let count = Arc::new(AtomicUsize::new(0));
        let (finished, completed) = oneshot::channel();
        let mut finished = Some(finished);
        let (started, observed) = std::sync::mpsc::sync_channel(CAPACITY);
        for index in 0..CAPACITY - 1 {
            let count = count.clone();
            let wait = wait.clone();
            let started = started.clone();
            let finished = if index + 2 == CAPACITY {
                finished.take()
            } else {
                None
            };
            tasks
                .submit(Box::pin(async move {
                    started.send(()).unwrap();
                    wait.await.unwrap();
                    count.fetch_add(1, Ordering::SeqCst);
                    if let Some(finished) = finished {
                        let _ = finished.send(());
                    }
                }))
                .unwrap();
        }
        // All32 work slots are active; control/stop admission must not depend
        // on whether the worker happened to empty its channel before polling.
        for _ in 0..CAPACITY - 1 {
            observed.recv_timeout(Duration::from_secs(10)).unwrap();
        }
        let drops = Arc::new(AtomicUsize::new(0));
        let probe = ReentrantDrop(tasks.clone(), drops.clone());
        assert!(
            tasks
                .submit(Box::pin(async move {
                    let _probe = probe;
                }))
                .is_err()
        );
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        assert_eq!(tasks.admitted.load(Ordering::Acquire), CAPACITY);
        let mut drain = Box::pin(tasks.control(ControlKind::Drain, Box::pin(async { Ok(()) })));
        pending(drain.as_mut());
        let mut closing = Box::pin(tasks.close());
        pending(closing.as_mut());
        drop(closing);
        let probe = ReentrantDrop(tasks.clone(), drops.clone());
        assert!(
            tasks
                .submit(Box::pin(async move {
                    let _probe = probe;
                }))
                .is_err()
        );
        assert_eq!(drops.load(Ordering::SeqCst), 2);
        release.send(()).unwrap();
        block_on(completed).unwrap();
        block_on(drain).unwrap();
        block_on(tasks.close()).unwrap();
        assert_eq!(count.load(Ordering::SeqCst), CAPACITY - 1);
        assert_eq!(tasks.admitted.load(Ordering::Acquire), 0);
    }
}
