//! One bounded mechanical task owner; command semantics stay in execution.
use futures::{future::poll_fn, task::AtomicWaker};
use radroots_execution::{
    Code, Error, OperationContext, RetryClass,
    session::{HostFuture, tasks::TaskHost},
};
use radroots_signing::host::Shared;
use std::{collections::VecDeque, sync::Mutex, task::Poll};

const CAPACITY: usize = 32;
fn unavailable() -> Error {
    Error::new(
        Code::CapabilityUnavailable,
        OperationContext::proposed(None),
        RetryClass::Never,
        None,
    )
    .unwrap_or_else(|_| unreachable!("valid constant task error"))
}
#[derive(Default)]
struct State {
    started: bool,
    // Every stop/close path sets this irreversible admission flag first.
    quiesced: bool,
    stopping: bool,
    closed: bool,
    active: bool,
    queue: VecDeque<HostFuture<'static, ()>>,
}
#[derive(Default)]
struct Inner {
    state: Mutex<State>,
    actor: AtomicWaker,
    drained: AtomicWaker,
    closed: AtomicWaker,
}
#[derive(Default)]
pub(crate) struct OwnedTasks {
    inner: Shared<Inner>,
}
impl OwnedTasks {
    pub(crate) fn start(&self, spawn: impl FnOnce(HostFuture<'static, ()>)) -> crate::Result<()> {
        {
            let mut state = self.inner.state.lock().map_err(|_| unavailable())?;
            if state.quiesced {
                return Err(unavailable());
            }
            if state.started {
                return Ok(());
            }
            state.started = true;
        }
        let inner = self.inner.clone();
        spawn(Box::pin(async move {
            loop {
                let next: crate::Result<Option<HostFuture<'static, ()>>> = poll_fn(|cx| {
                    inner.actor.register(cx.waker());
                    let mut state = inner.state.lock().map_err(|_| unavailable())?;
                    if let Some(task) = state.queue.pop_front() {
                        state.active = true;
                        Poll::Ready(Ok(Some(task)))
                    } else if state.stopping {
                        state.closed = true;
                        Poll::Ready(Ok(None))
                    } else {
                        Poll::Pending
                    }
                })
                .await;
                match next {
                    Ok(Some(task)) => {
                        task.await;
                        let Ok(mut state) = inner.state.lock() else {
                            break;
                        };
                        state.active = false;
                        drop(state);
                        inner.drained.wake();
                    }
                    Ok(None) | Err(_) => break,
                }
            }
            inner.closed.wake();
            inner.drained.wake();
        }));
        Ok(())
    }
    pub(crate) fn quiesce(&self) -> crate::Result<()> {
        self.inner.state.lock().map_err(|_| unavailable())?.quiesced = true;
        Ok(())
    }
    pub(crate) async fn drain(&self) -> crate::Result<()> {
        poll_fn(|cx| {
            self.inner.drained.register(cx.waker());
            let idle = {
                let state = self.inner.state.lock().map_err(|_| unavailable())?;
                !state.active && state.queue.is_empty()
            };
            if idle {
                self.inner.drained.take();
                Poll::Ready(Ok(()))
            } else {
                Poll::Pending
            }
        })
        .await
    }
    pub(crate) async fn close(&self) -> crate::Result<()> {
        {
            let mut state = self.inner.state.lock().map_err(|_| unavailable())?;
            state.quiesced = true;
            state.stopping = true;
            if !state.started {
                state.closed = true;
            }
        }
        self.inner.actor.wake();
        poll_fn(|cx| {
            self.inner.closed.register(cx.waker());
            let closed = self.inner.state.lock().map_err(|_| unavailable())?.closed;
            if closed {
                self.inner.closed.take();
                Poll::Ready(Ok(()))
            } else {
                Poll::Pending
            }
        })
        .await
    }
}
impl TaskHost for OwnedTasks {
    fn submit(&self, task: HostFuture<'static, ()>) -> crate::Result<()> {
        {
            let mut state = self.inner.state.lock().map_err(|_| unavailable())?;
            if !state.started
                || state.quiesced
                || state.queue.len() + usize::from(state.active) >= CAPACITY
            {
                return Err(unavailable());
            }
            state.queue.push_back(task);
        }
        self.inner.actor.wake();
        Ok(())
    }
}
impl Drop for OwnedTasks {
    fn drop(&mut self) {
        if let Ok(mut state) = self.inner.state.lock() {
            state.quiesced = true;
            state.stopping = true;
        }
        self.inner.actor.wake();
    }
}

#[cfg(test)]
#[path = "tasks_tests.rs"]
mod tests;
