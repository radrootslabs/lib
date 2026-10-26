//! Bounded command failure diagnostics separate from compact resource errors.
use crate::{Code, Detail, Error, OperationContext, RetryClass};
use core::fmt;
use radroots_signing::issuance::RejectedArtifact;

/// Allocation-free command failure. The primitive Error retains its 64-byte
/// contract; this separate Copy value can also retain fixed rejection metadata.
/// It owns no untrusted raw signer payload, dynamic message or backend cause.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionFailure {
    error: Error,
    rejection: Option<RejectedArtifact>,
}
const _: () = assert!(core::mem::size_of::<ExecutionFailure>() <= 128);
impl From<Error> for ExecutionFailure {
    fn from(error: Error) -> Self {
        Self {
            error,
            rejection: None,
        }
    }
}
impl ExecutionFailure {
    /// Primitive compact diagnostic; it supplies no business/nonissuance proof.
    pub const fn error(self) -> Error {
        self.error
    }
    /// Original fixed diagnostic code, including actual storage failures.
    pub const fn code(self) -> Code {
        self.error.code()
    }
    /// Original reported operation identity and admission observation.
    pub const fn context(self) -> OperationContext {
        self.error.context()
    }
    /// Conservative original-operation continuation requirement.
    pub const fn retry(self) -> RetryClass {
        self.error.retry()
    }
    /// Optional fixed redacted diagnostic categories.
    pub const fn detail(self) -> Option<Detail> {
        self.error.detail()
    }
    /// Actual fixed signer rejection and optional exact fingerprint coverage.
    /// This is caller diagnostic data, never authenticated business evidence.
    pub const fn signer_rejection(self) -> Option<RejectedArtifact> {
        self.rejection
    }
    #[cfg(feature = "workspace_session")]
    pub(crate) const fn rejected(
        operation: radroots_domain::OperationId,
        rejection: RejectedArtifact,
    ) -> Self {
        Self {
            error: Error::signer_tampering(operation),
            rejection: Some(rejection),
        }
    }
    #[cfg(feature = "workspace_session")]
    pub(crate) fn host_after_rejection(
        error: radroots_storage::command::OperationError,
        operation: radroots_domain::OperationId,
        rejection: RejectedArtifact,
    ) -> Self {
        Self {
            error: Error::capture_pending(error, OperationContext::retained(operation)),
            rejection: Some(rejection),
        }
    }
}
impl fmt::Display for ExecutionFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(f)
    }
}
impl core::error::Error for ExecutionFailure {}
