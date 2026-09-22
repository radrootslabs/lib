use radroots_storage::backup::BackupCapabilityError;

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
