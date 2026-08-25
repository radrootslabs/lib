//! Private per-operation durability failpoints used by deterministic tests.

use core::fmt;

#[cfg(test)]
use std::{
    io::{self, Write},
    sync::{Arc, Condvar, Mutex},
};

#[cfg(test)]
const PROCESS_BARRIER_READY: &[u8] = b"\nRSHR_STEP073_READY\n";

#[cfg(test)]
pub(crate) fn storage_full_error() -> io::Error {
    io::Error::from(io::ErrorKind::StorageFull)
}

/// Closed inventory of durability edges exercised by the crash-boundary harness.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DurabilityFailpoint {
    InitializeBeforeCreate,
    InitializeAfterCreate,
    InitializeBeforeReservationDirectorySync,
    InitializeAfterReservationDirectorySync,
    InitializeBeforeFileSync,
    InitializeAfterFileSync,
    InitializeBeforeCommitDirectorySync,
    InitializeAfterCommitDirectorySync,
    TransactionBeforeBegin,
    TransactionAfterBegin,
    TransactionBeforeCommit,
    TransactionAfterCommit,
    BackupBeforeCreate,
    BackupAfterCreate,
    BackupBeforeCopy,
    BackupAfterCopy,
    BackupBeforeFileSync,
    BackupAfterFileSync,
    BackupBeforeDirectorySync,
    BackupAfterDirectorySync,
    MarkerBeforeCreate,
    MarkerAfterCreate,
    MarkerBeforeFileSync,
    MarkerAfterFileSync,
    MarkerBeforeDirectorySync,
    MarkerAfterDirectorySync,
    MarkerAdvanceBeforeWriteAndFileSync,
    MarkerAdvanceAfterWriteAndFileSync,
    MarkerAdvanceBeforeReplace,
    MarkerAdvanceAfterReplace,
    MarkerAdvanceBeforeDirectorySync,
    MarkerAdvanceAfterDirectorySync,
    RestoreBeforeRetainLiveRename,
    RestoreAfterRetainLiveRename,
    RestoreBeforeRetainLiveSync,
    RestoreAfterRetainLiveSync,
    RestoreBeforeInstallStageRename,
    RestoreAfterInstallStageRename,
    RestoreBeforeInstallStageSync,
    RestoreAfterInstallStageSync,
    CloseBeforeDrain,
    CloseAfterDrain,
    CloseBeforeCheckpoint,
    CloseAfterCheckpoint,
    CloseBeforeConnectionClose,
    CloseAfterConnectionClose,
    CloseBeforeAuthorityRelease,
    CloseAfterAuthorityRelease,
}

impl DurabilityFailpoint {
    #[cfg(test)]
    pub(crate) const ALL: [Self; 48] = [
        Self::InitializeBeforeCreate,
        Self::InitializeAfterCreate,
        Self::InitializeBeforeReservationDirectorySync,
        Self::InitializeAfterReservationDirectorySync,
        Self::InitializeBeforeFileSync,
        Self::InitializeAfterFileSync,
        Self::InitializeBeforeCommitDirectorySync,
        Self::InitializeAfterCommitDirectorySync,
        Self::TransactionBeforeBegin,
        Self::TransactionAfterBegin,
        Self::TransactionBeforeCommit,
        Self::TransactionAfterCommit,
        Self::BackupBeforeCreate,
        Self::BackupAfterCreate,
        Self::BackupBeforeCopy,
        Self::BackupAfterCopy,
        Self::BackupBeforeFileSync,
        Self::BackupAfterFileSync,
        Self::BackupBeforeDirectorySync,
        Self::BackupAfterDirectorySync,
        Self::MarkerBeforeCreate,
        Self::MarkerAfterCreate,
        Self::MarkerBeforeFileSync,
        Self::MarkerAfterFileSync,
        Self::MarkerBeforeDirectorySync,
        Self::MarkerAfterDirectorySync,
        Self::MarkerAdvanceBeforeWriteAndFileSync,
        Self::MarkerAdvanceAfterWriteAndFileSync,
        Self::MarkerAdvanceBeforeReplace,
        Self::MarkerAdvanceAfterReplace,
        Self::MarkerAdvanceBeforeDirectorySync,
        Self::MarkerAdvanceAfterDirectorySync,
        Self::RestoreBeforeRetainLiveRename,
        Self::RestoreAfterRetainLiveRename,
        Self::RestoreBeforeRetainLiveSync,
        Self::RestoreAfterRetainLiveSync,
        Self::RestoreBeforeInstallStageRename,
        Self::RestoreAfterInstallStageRename,
        Self::RestoreBeforeInstallStageSync,
        Self::RestoreAfterInstallStageSync,
        Self::CloseBeforeDrain,
        Self::CloseAfterDrain,
        Self::CloseBeforeCheckpoint,
        Self::CloseAfterCheckpoint,
        Self::CloseBeforeConnectionClose,
        Self::CloseAfterConnectionClose,
        Self::CloseBeforeAuthorityRelease,
        Self::CloseAfterAuthorityRelease,
    ];
}

/// Disabled in ordinary builds; tests may arm one edge on one owned controller.
#[derive(Clone, Default)]
pub(crate) struct DurabilityFailpoints {
    #[cfg(test)]
    state: Arc<Mutex<TestState>>,
}

#[cfg(test)]
#[derive(Default)]
struct TestState {
    armed: Option<DurabilityFailpoint>,
    fired: bool,
    reached: Vec<DurabilityFailpoint>,
    observations: Vec<(DurabilityFailpoint, u8)>,
    process_barrier: Option<TestProcessBarrier>,
}

#[cfg(test)]
#[derive(Clone)]
struct TestProcessBarrier {
    point: DurabilityFailpoint,
    occurrence: u8,
    seen: u8,
    gate: Arc<TestProcessBarrierGate>,
}

#[cfg(test)]
#[derive(Default)]
struct TestProcessBarrierGate {
    state: Mutex<TestProcessBarrierGateState>,
    changed: Condvar,
}

#[cfg(test)]
#[derive(Default)]
struct TestProcessBarrierGateState {
    ready: bool,
    released: bool,
}

impl DurabilityFailpoints {
    pub(crate) fn hit(&self, point: DurabilityFailpoint) -> Result<(), DurabilityFailpointError> {
        #[cfg(test)]
        {
            let (injected, process_gate) = {
                let mut state = self.state.lock().map_err(|_| DurabilityFailpointError)?;
                if state.reached.len() < DurabilityFailpoint::ALL.len() {
                    state.reached.push(point);
                }
                let injected = state.armed == Some(point) && !state.fired;
                let process_gate = if !state.fired {
                    state.process_barrier.as_mut().and_then(|barrier| {
                        if barrier.point != point {
                            return None;
                        }
                        barrier.seen = barrier.seen.saturating_add(1);
                        (barrier.seen == barrier.occurrence).then(|| Arc::clone(&barrier.gate))
                    })
                } else {
                    None
                };
                if injected || process_gate.is_some() {
                    state.fired = true;
                }
                (injected, process_gate)
            };
            if let Some(process_gate) = process_gate {
                process_gate.notify_and_wait()?;
                return Err(DurabilityFailpointError);
            }
            if injected {
                return Err(DurabilityFailpointError);
            }
        }
        #[cfg(not(test))]
        let _ = point;
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn armed(point: DurabilityFailpoint) -> Self {
        Self {
            state: Arc::new(Mutex::new(TestState {
                armed: Some(point),
                fired: false,
                reached: Vec::new(),
                observations: Vec::new(),
                process_barrier: None,
            })),
        }
    }

    #[cfg(test)]
    pub(crate) fn process_barrier(point: DurabilityFailpoint, occurrence: u8) -> Self {
        assert!(
            matches!(occurrence, 1 | 2),
            "process occurrence must be 1 or 2"
        );
        Self {
            state: Arc::new(Mutex::new(TestState {
                armed: None,
                fired: false,
                reached: Vec::new(),
                observations: Vec::new(),
                process_barrier: Some(TestProcessBarrier {
                    point,
                    occurrence,
                    seen: 0,
                    gate: Arc::new(TestProcessBarrierGate::default()),
                }),
            })),
        }
    }

    #[cfg(test)]
    fn wait_for_process_barrier(&self) {
        let gate = self
            .state
            .lock()
            .expect("durability failpoint state")
            .process_barrier
            .as_ref()
            .map(|barrier| Arc::clone(&barrier.gate))
            .expect("process barrier");
        gate.wait_until_ready();
    }

    #[cfg(test)]
    fn release_process_barrier(&self) {
        let gate = self
            .state
            .lock()
            .expect("durability failpoint state")
            .process_barrier
            .as_ref()
            .map(|barrier| Arc::clone(&barrier.gate))
            .expect("process barrier");
        gate.release();
    }

    #[cfg(test)]
    pub(crate) fn arm(&self, point: DurabilityFailpoint) {
        let mut state = self.state.lock().expect("durability failpoint state");
        state.armed = Some(point);
        state.fired = false;
        state.reached.clear();
        state.observations.clear();
    }

    #[cfg(test)]
    pub(crate) fn disarm(&self) {
        let mut state = self.state.lock().expect("durability failpoint state");
        state.armed = None;
    }

    #[cfg(test)]
    pub(crate) fn fired(&self) -> bool {
        self.state.lock().is_ok_and(|state| state.fired)
    }

    #[cfg(test)]
    pub(crate) fn reached(&self) -> Vec<DurabilityFailpoint> {
        self.state
            .lock()
            .map_or_else(|_| Vec::new(), |state| state.reached.clone())
    }

    #[cfg(test)]
    pub(crate) fn observe(&self, point: DurabilityFailpoint, state_value: u8) {
        let mut state = self.state.lock().expect("durability failpoint state");
        state.observations.push((point, state_value));
    }

    #[cfg(test)]
    pub(crate) fn observation(&self, point: DurabilityFailpoint) -> Option<u8> {
        self.state.lock().ok().and_then(|state| {
            state
                .observations
                .iter()
                .find_map(|(observed, value)| (*observed == point).then_some(*value))
        })
    }
}

#[cfg(test)]
impl TestProcessBarrierGate {
    fn notify_and_wait(&self) -> Result<(), DurabilityFailpointError> {
        {
            let mut state = self.state.lock().map_err(|_| DurabilityFailpointError)?;
            state.ready = true;
            self.changed.notify_all();
        }
        let mut stdout = io::stdout().lock();
        stdout
            .write_all(PROCESS_BARRIER_READY)
            .and_then(|()| stdout.flush())
            .map_err(|_| DurabilityFailpointError)?;
        let state = self.state.lock().map_err(|_| DurabilityFailpointError)?;
        drop(
            self.changed
                .wait_while(state, |state| !state.released)
                .map_err(|_| DurabilityFailpointError)?,
        );
        Ok(())
    }

    fn wait_until_ready(&self) {
        let state = self.state.lock().expect("process barrier gate");
        drop(
            self.changed
                .wait_while(state, |state| !state.ready)
                .expect("process barrier ready"),
        );
    }

    fn release(&self) {
        let mut state = self.state.lock().expect("process barrier gate");
        state.released = true;
        self.changed.notify_all();
    }
}

impl fmt::Debug for DurabilityFailpoints {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("DurabilityFailpoints([redacted])")
    }
}

/// Source-free injected failure; subsystem adapters retain their stable error kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DurabilityFailpointError;

impl fmt::Display for DurabilityFailpointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("injected durability boundary failure")
    }
}

impl std::error::Error for DurabilityFailpointError {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn storage_full_injection_uses_the_semantic_io_kind() {
        assert_eq!(storage_full_error().kind(), io::ErrorKind::StorageFull);
    }

    #[test]
    fn every_closed_point_fires_once_on_its_owned_plan() {
        for point in DurabilityFailpoint::ALL {
            let plan = DurabilityFailpoints::armed(point);
            assert_eq!(plan.hit(point), Err(DurabilityFailpointError));
            assert_eq!(plan.hit(point), Ok(()));
            assert!(plan.fired());
            assert_eq!(plan.reached(), [point, point]);
        }
    }

    #[test]
    fn plans_are_instance_local_and_disabled_plan_never_fails() {
        let first = DurabilityFailpoints::armed(DurabilityFailpoint::TransactionBeforeCommit);
        let second = DurabilityFailpoints::armed(DurabilityFailpoint::BackupBeforeCopy);
        assert_eq!(first.hit(DurabilityFailpoint::BackupBeforeCopy), Ok(()));
        assert!(!first.fired());
        assert_eq!(
            second.hit(DurabilityFailpoint::BackupBeforeCopy),
            Err(DurabilityFailpointError)
        );
        assert!(!first.fired());
        assert!(second.fired());
        assert_eq!(
            DurabilityFailpoints::default().hit(DurabilityFailpoint::CloseBeforeDrain),
            Ok(())
        );
    }

    #[test]
    fn process_barrier_waits_for_the_selected_occurrence_and_releases_once() {
        let point = DurabilityFailpoint::MarkerAdvanceAfterDirectorySync;
        let plan = DurabilityFailpoints::process_barrier(point, 2);
        let worker_plan = plan.clone();
        let worker = thread::spawn(move || {
            assert_eq!(worker_plan.hit(point), Ok(()));
            worker_plan.hit(point)
        });

        plan.wait_for_process_barrier();
        assert!(plan.fired());
        plan.release_process_barrier();
        assert_eq!(
            worker.join().expect("barrier worker"),
            Err(DurabilityFailpointError)
        );
        assert_eq!(plan.reached(), [point, point]);
    }
}
