use core::fmt;

/// Path-free failures from actual owner backup operations. These are distinct
/// from the optimistic metadata-transition errors of reliability records.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum BackupCapabilityError {
    /// This backend provides no actual snapshot capability.
    Unsupported,
    /// The owner is closed or its storage is unavailable.
    Unavailable,
    /// An explicit valid host-owned backup location is required.
    InvalidConfiguration,
    /// The requested backup format is not supported by the owner.
    UnsupportedVersion,
    /// Existing staged or finalized state requires reconciliation.
    Conflict,
    /// The supplied plan, inventory or member bytes could not be verified.
    VerificationFailed,
    /// Capture or durable filesystem publication did not complete successfully.
    Failed,
}

impl fmt::Display for BackupCapabilityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Unsupported => "storage backup capability is unsupported",
            Self::Unavailable => "storage backup owner is unavailable",
            Self::InvalidConfiguration => "storage backup configuration is invalid",
            Self::UnsupportedVersion => "storage backup format is unsupported",
            Self::Conflict => "storage backup state requires reconciliation",
            Self::VerificationFailed => "storage backup verification failed",
            Self::Failed => "storage backup did not complete",
        })
    }
}

impl std::error::Error for BackupCapabilityError {}

#[cfg(test)]
mod tests {
    use super::BackupCapabilityError as E;

    #[test]
    fn errors_are_bounded_public_reports_without_paths_or_backend_sources() {
        for error in [
            E::Unsupported,
            E::Unavailable,
            E::InvalidConfiguration,
            E::UnsupportedVersion,
            E::Conflict,
            E::VerificationFailed,
            E::Failed,
        ] {
            let report = error.to_string();
            assert!(report.len() < 80);
            assert!(!report.contains('/'));
            assert!(std::error::Error::source(&error).is_none());
        }
    }
}
