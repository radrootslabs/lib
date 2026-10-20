//! Shared session admission and resumable resource shutdown.
//!
//! This module creates no runtime or detached task. The executor, rather than a
//! caller's observation future, retains each work permit until its safe boundary.
//! A permit is resource ownership, never proof of durable command admission.

pub(crate) mod host;
pub mod tasks;

#[cfg(feature = "workspace_session")]
pub mod command;

#[cfg(feature = "client_session")]
mod client;

#[cfg(feature = "client_session")]
pub use client::ClientSession;

#[cfg(feature = "signer_session")]
mod signer;

#[cfg(feature = "signer_session")]
pub use signer::SignerSession;

pub use host::{HostBound, HostFuture, SessionHost};

use core::{
    future::poll_fn,
    num::NonZeroU16,
    task::{Poll, Waker},
};
use futures::lock::Mutex as AsyncMutex;
use std::sync::{Mutex, MutexGuard};

use crate::{Error, Reason, Result};
use host::Shared;

/// Explicit task admission for a local resource session. No network is selected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum StartMode {
    /// Enables local resource work without opening sockets or replaying writes.
    LocalOnly,
    /// Permits explicit commands to use the configured transport. Startup
    /// itself performs no query, publication or automatic outbox replay.
    Online,
}

/// Session resource state, independent of durable operation state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionState {
    /// Resources are owned but workers have not started.
    Ready,
    /// Worker startup is pending or must be resumed.
    Starting,
    /// Startup succeeded and new work may be admitted within the fixed limit.
    Running,
    /// Admission is permanently closed and shutdown must be awaited or resumed.
    Closing,
    /// All resource shutdown hooks completed successfully.
    Closed,
}

#[derive(Clone, Copy)]
enum CloseStage {
    Quiesce,
    DrainOwned,
    Drain,
    Disconnect,
    Callbacks,
    Storage,
    Complete,
}

struct Gate {
    state: SessionState,
    active: u16,
    drained: Option<Waker>,
    starting: Option<Waker>,
}

struct Inner<H> {
    host: H,
    limit: NonZeroU16,
    gate: Mutex<Gate>,
    control: AsyncMutex<CloseStage>,
}

impl<H> Inner<H> {
    fn gate(&self) -> MutexGuard<'_, Gate> {
        // Keep bookkeeping short. Waker callbacks and destructors must run
        // outside the gate, including when a pending observer is replaced.
        self.gate
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// Cloneable ownership of one coherent session. Native handles are Send/Sync;
/// browser handles use Rc and cannot cross their owning thread.
pub struct Session<H: SessionHost> {
    inner: Shared<Inner<H>>,
}

impl<H: SessionHost> Clone for Session<H> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<H: SessionHost> Session<H> {
    /// Takes ownership of already constructed resources without starting work.
    /// The explicit limit bounds concurrent owned work; it cannot be enlarged.
    pub fn new(host: H, maximum_in_flight: NonZeroU16) -> Self {
        Self {
            inner: Shared::new(Inner {
                host,
                limit: maximum_in_flight,
                gate: Mutex::new(Gate {
                    state: SessionState::Ready,
                    active: 0,
                    drained: None,
                    starting: None,
                }),
                control: AsyncMutex::new(CloseStage::Quiesce),
            }),
        }
    }

    /// Returns resource lifecycle state without claiming a durable checkpoint.
    pub fn state(&self) -> SessionState {
        self.inner.gate().state
    }

    /// Starts or resumes startup on the caller's host executor.
    /// Concurrent calls serialize; completed startup is not repeated.
    pub async fn start(&self) -> Result<()> {
        let _control = self.inner.control.lock().await;
        {
            let mut gate = self.inner.gate();
            match gate.state {
                SessionState::Running => return Ok(()),
                SessionState::Closing | SessionState::Closed => return Err(unavailable()),
                SessionState::Ready | SessionState::Starting => gate.state = SessionState::Starting,
            }
        }
        let mut startup = self.inner.host.start();
        let result = poll_fn(|context| {
            let replacement = context.waker().clone();
            let previous = {
                let mut gate = self.inner.gate();
                if gate.state == SessionState::Closing {
                    return Poll::Ready(Err(unavailable()));
                }
                if !gate
                    .starting
                    .as_ref()
                    .is_some_and(|w| w.will_wake(context.waker()))
                {
                    gate.starting.replace(replacement)
                } else {
                    None
                }
            };
            drop(previous);
            startup.as_mut().poll(context)
        })
        .await;
        let previous = { self.inner.gate().starting.take() };
        drop(previous);
        result?;
        let mut gate = self.inner.gate();
        if gate.state == SessionState::Closing {
            return Err(unavailable());
        }
        gate.state = SessionState::Running;
        Ok(())
    }

    /// Reserves one unit of live resource ownership for executor work.
    ///
    /// The executor must move this permit into the owned operation, independently
    /// of caller observation, and retain it through result/uncertainty storage.
    /// This call performs no journal write, authorization or signing.
    pub fn admit(&self) -> Result<WorkPermit<H>> {
        self.reserve(false)
    }

    // Attachment is resource ownership before worker startup, never command
    // admission. It shares the same fixed bound and permanent close gate.
    #[cfg(feature = "client_session")]
    fn retain_resource(&self) -> Result<WorkPermit<H>> {
        self.reserve(true)
    }

    fn reserve(&self, before_start: bool) -> Result<WorkPermit<H>> {
        let mut gate = self.inner.gate();
        if matches!(gate.state, SessionState::Closing | SessionState::Closed)
            || (!before_start && gate.state != SessionState::Running)
        {
            return Err(unavailable());
        }
        if gate.active == self.inner.limit.get() {
            return Err(Error::session_unavailable(Reason::OutOfRange));
        }
        gate.active += 1;
        Ok(WorkPermit {
            inner: self.inner.clone(),
        })
    }

    fn stop_admission(&self) -> bool {
        let starting = {
            let mut gate = self.inner.gate();
            if gate.state == SessionState::Closed {
                return false;
            }
            gate.state = SessionState::Closing;
            gate.starting.take()
        };
        if let Some(waker) = starting {
            waker.wake();
        }
        true
    }

    /// Permanently stops admission and quiesces the host without draining or
    /// disposing resources. Composing owners must subsequently await close.
    /// Cancellation resumes the same unfinished hook; completion is shared.
    pub async fn quiesce(&self) -> Result<()> {
        if !self.stop_admission() {
            return Ok(());
        }
        let mut stage = self.inner.control.lock().await;
        if matches!(*stage, CloseStage::Quiesce) {
            self.inner.host.quiesce().await?;
            *stage = CloseStage::DrainOwned;
        }
        Ok(())
    }

    /// Permanently closes admission on first poll, then shuts down in order.
    ///
    /// Cancelling this future retains its last completed stage. Awaiting close
    /// again resumes the unfinished hook. No Drop implementation claims close.
    pub async fn close(&self) -> Result<()> {
        if !self.stop_admission() {
            return Ok(());
        }
        let mut stage = self.inner.control.lock().await;
        loop {
            *stage = match *stage {
                CloseStage::Quiesce => {
                    self.inner.host.quiesce().await?;
                    CloseStage::DrainOwned
                }
                CloseStage::DrainOwned => {
                    self.inner.host.drain().await?;
                    CloseStage::Drain
                }
                CloseStage::Drain => {
                    poll_fn(|context| {
                        let replacement = context.waker().clone();
                        let previous = {
                            let mut gate = self.inner.gate();
                            if gate.active == 0 {
                                return Poll::Ready(());
                            }
                            if !gate
                                .drained
                                .as_ref()
                                .is_some_and(|w| w.will_wake(context.waker()))
                            {
                                gate.drained.replace(replacement)
                            } else {
                                None
                            }
                        };
                        drop(previous);
                        Poll::Pending
                    })
                    .await;
                    CloseStage::Disconnect
                }
                CloseStage::Disconnect => {
                    self.inner.host.disconnect().await?;
                    CloseStage::Callbacks
                }
                CloseStage::Callbacks => {
                    self.inner.host.dispose_callbacks().await?;
                    CloseStage::Storage
                }
                CloseStage::Storage => {
                    self.inner.host.close_storage().await?;
                    CloseStage::Complete
                }
                CloseStage::Complete => {
                    self.inner.gate().state = SessionState::Closed;
                    return Ok(());
                }
            };
        }
    }
}

fn unavailable() -> Error {
    Error::session_unavailable(Reason::Unavailable)
}

/// Live executor work retaining the session's resources until its safe boundary.
/// Dropping a permit releases capacity; it does not cancel or roll back effects.
#[must_use = "retain the permit in owned work through its safe journal boundary"]
pub struct WorkPermit<H: SessionHost> {
    inner: Shared<Inner<H>>,
}

impl<H: SessionHost> Drop for WorkPermit<H> {
    fn drop(&mut self) {
        let wake = {
            let mut gate = self.inner.gate();
            gate.active -= 1;
            if gate.active == 0 {
                gate.drained.take()
            } else {
                None
            }
        };
        if let Some(waker) = wake {
            waker.wake();
        }
    }
}
