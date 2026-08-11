//! Private crash-recovery marker mechanics for governed restore.

mod finalize;
mod marker;
mod stage;

pub use finalize::finalize_staged_restore;
pub use stage::{StagedServiceRestore, stage_verified_restore};

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) fn refuse_unresolved_recovery(
    directory: &impl std::os::fd::AsFd,
) -> Result<(), crate::ServiceSqliteError> {
    use rustix::{
        fs::{AtFlags, statat},
        io::Errno,
    };

    for name in [marker::MARKER_FILE_NAME, marker::MARKER_NEXT_FILE_NAME] {
        match statat(directory, name, AtFlags::SYMLINK_NOFOLLOW) {
            Err(Errno::NOENT) => {}
            Ok(_) | Err(_) => {
                return Err(crate::ServiceSqliteError::new(
                    crate::ServiceSqliteErrorKind::Recovery,
                ));
            }
        }
    }
    Ok(())
}

#[allow(unused_imports)]
pub(crate) use marker::{
    RestoreArtifactExpectation, RestoreMarkerContractError, RestoreRecoveryLayout,
    RestoreRecoveryMarker, RestoreRecoveryPhase,
};

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[allow(unused_imports)]
pub(crate) use marker::RestoreMarkerBinding;
