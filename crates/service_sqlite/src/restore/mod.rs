//! Private crash-recovery marker mechanics for governed restore.

mod marker;
mod stage;

pub use stage::{StagedServiceRestore, stage_verified_restore};

#[allow(unused_imports)]
pub(crate) use marker::{
    RestoreArtifactExpectation, RestoreMarkerContractError, RestoreRecoveryLayout,
    RestoreRecoveryMarker, RestoreRecoveryPhase,
};

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[allow(unused_imports)]
pub(crate) use marker::RestoreMarkerBinding;
