use core::fmt;

/// Path-free failures from actual owner restore operations. These are not
/// optimistic reliability-record transition errors or permission to deliver
/// historical operations after reopening restored storage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RestoreCapabilityError {
    /// The backend provides no actual restore capability.
    Unsupported,
    /// The owner is closed, unavailable or not writable.
    Unavailable,
    /// An explicit valid host-owned backup location is required.
    InvalidConfiguration,
    /// The retained backup format is unsupported.
    UnsupportedVersion,
    /// Existing staging or recovery state requires reconciliation.
    Conflict,
    /// The supplied inventory or retained members could not be verified.
    VerificationFailed,
    /// Staging or installation did not complete; retain recovery evidence.
    Failed,
}

impl fmt::Display for RestoreCapabilityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Unsupported => "storage restore capability is unsupported",
            Self::Unavailable => "storage restore owner is unavailable",
            Self::InvalidConfiguration => "storage restore configuration is invalid",
            Self::UnsupportedVersion => "storage restore format is unsupported",
            Self::Conflict => "storage restore state requires reconciliation",
            Self::VerificationFailed => "storage restore verification failed",
            Self::Failed => "storage restore did not complete",
        })
    }
}

impl std::error::Error for RestoreCapabilityError {}

#[cfg(test)]
mod tests {
    use super::RestoreCapabilityError as E;

    #[test]
    fn restore_errors_expose_only_bounded_public_reports() {
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
