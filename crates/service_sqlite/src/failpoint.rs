//! Private per-operation durability failpoints used by deterministic tests.

use core::fmt;

#[cfg(test)]
use std::sync::{Arc, Mutex};

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
}

impl DurabilityFailpoints {
    pub(crate) fn hit(&self, point: DurabilityFailpoint) -> Result<(), DurabilityFailpointError> {
        #[cfg(test)]
        {
            let mut state = self.state.lock().map_err(|_| DurabilityFailpointError)?;
            if state.reached.len() < DurabilityFailpoint::ALL.len() {
                state.reached.push(point);
            }
            if state.armed == Some(point) && !state.fired {
                state.fired = true;
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
            })),
        }
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
}
