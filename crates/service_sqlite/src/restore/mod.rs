//! Private crash-recovery marker mechanics for governed restore.

mod finalize;
mod marker;
mod recover;
mod stage;

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
#[cfg_attr(coverage_nightly, coverage(off))]
mod process_tests;

pub use finalize::finalize_staged_restore;
pub use stage::{StagedServiceRestore, stage_verified_restore};

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) use recover::refuse_unresolved_recovery;

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) use recover::recover_for_open;

#[allow(unused_imports)]
pub(crate) use marker::{
    BACKUP_FILE_NAME, LIVE_FILE_NAME, MARKER_FILE_NAME, MARKER_NEXT_FILE_NAME,
    RestoreArtifactExpectation, RestoreMarkerContractError, RestoreRecoveryLayout,
    RestoreRecoveryMarker, RestoreRecoveryPhase, STAGED_FILE_NAME,
};

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[allow(unused_imports)]
pub(crate) use marker::RestoreMarkerBinding;
