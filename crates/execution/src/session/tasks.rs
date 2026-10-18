//! Mechanical ownership of already bounded execution tasks.
use super::{HostBound, HostFuture};

#[cfg(feature = "workspace_session")]
use super::host::Shared;
#[cfg(feature = "workspace_session")]
use std::sync::atomic::{AtomicUsize, Ordering};

#[cfg(feature = "workspace_session")]
const LIVE_MAX_BYTES: usize = 4 * 1024 * 1024;
#[cfg(feature = "workspace_session")]
const INQUIRY_RESERVATION_BYTES: usize = 2 * 1024 * 1024;

#[cfg(feature = "workspace_session")]
#[derive(Clone)]
pub(crate) struct TaskBudget(Shared<AtomicUsize>);
#[cfg(feature = "workspace_session")]
impl TaskBudget {
    pub(crate) fn new() -> Self {
        Self(Shared::new(AtomicUsize::new(0)))
    }
    pub(crate) fn reserve(&self) -> crate::Result<TaskReservation> {
        self.0
            .try_update(Ordering::AcqRel, Ordering::Acquire, |used| {
                used.checked_add(INQUIRY_RESERVATION_BYTES)
                    .filter(|next| *next <= LIVE_MAX_BYTES)
            })
            .map_err(|_| crate::Error::session_unavailable(crate::Reason::OutOfRange))?;
        Ok(TaskReservation(self.0.clone()))
    }
}
#[cfg(feature = "workspace_session")]
pub(crate) struct TaskReservation(Shared<AtomicUsize>);
#[cfg(feature = "workspace_session")]
impl Drop for TaskReservation {
    fn drop(&mut self) {
        self.0
            .fetch_sub(INQUIRY_RESERVATION_BYTES, Ordering::AcqRel);
    }
}

/// Target-host task admission. The execution owner reserves input/result budget
/// and moves its resource permit into the task before submitting it. Success
/// transfers ownership: losing the observer must not cancel the accepted task.
/// This port confers no command, signer, journal or adapter qualification.
pub trait TaskHost: HostBound {
    /// Accepts an unpolled owned task. Refusal executes no part of the task.
    /// Implementations must bound their accepted task set.
    fn submit(&self, task: HostFuture<'static, ()>) -> crate::Result<()>;
}
