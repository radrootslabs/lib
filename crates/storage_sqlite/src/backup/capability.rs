use radroots_storage::backup::{BackupCapabilityError, RestoreCapabilityError};

pub(super) fn map_error(error: crate::Error) -> BackupCapabilityError {
    use crate::Error as E;
    match error {
        E::BackupBackendUnavailable => BackupCapabilityError::Unavailable,
        E::BackupRootRequired | E::InvalidBackupRoot(_) => {
            BackupCapabilityError::InvalidConfiguration
        }
        E::UnsupportedBackupVersion => BackupCapabilityError::UnsupportedVersion,
        E::BackupBundleAlreadyExists(_) => BackupCapabilityError::Conflict,
        E::BackupBundleMissing(_)
        | E::BackupVerificationFailed { .. }
        | E::BackupUnexpectedEntry(_) => BackupCapabilityError::VerificationFailed,
        _ => BackupCapabilityError::Failed,
    }
}

pub(super) fn map_restore_error(error: crate::Error) -> RestoreCapabilityError {
    use crate::Error as E;
    match error {
        E::BackupBackendUnavailable | E::RestoreRequiresWritableStorage => {
            RestoreCapabilityError::Unavailable
        }
        E::BackupRootRequired | E::InvalidBackupRoot(_) => {
            RestoreCapabilityError::InvalidConfiguration
        }
        E::UnsupportedBackupVersion => RestoreCapabilityError::UnsupportedVersion,
        E::BackupBundleAlreadyExists(_)
        | E::RestoreStagingAlreadyExists(_)
        | E::RestoreRecoveryConflict(_) => RestoreCapabilityError::Conflict,
        E::BackupBundleMissing(_)
        | E::BackupVerificationFailed { .. }
        | E::BackupUnexpectedEntry(_)
        | E::RestoreStagingFailed { .. }
        | E::RestoreMarkerCorrupt(_) => RestoreCapabilityError::VerificationFailed,
        _ => RestoreCapabilityError::Failed,
    }
}
