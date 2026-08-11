//! Safe public errors with separately preserved internal sources.

use core::fmt;
use std::error::Error;

/// Maximum byte length of every public host-error message.
pub const MAX_SAFE_ERROR_MESSAGE_BYTES: usize = 96;

/// Stable public error codes for host-mechanism failures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostErrorCode {
    ConfigDocument,
    Lifecycle,
    AdminTransport,
    OperationsBind,
    OperationsServe,
    PathContext,
    TaskFailure,
    InvalidHostContract,
}

impl HostErrorCode {
    /// Returns the stable lowercase wire representation of this code.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ConfigDocument => "host_config_document",
            Self::Lifecycle => "host_lifecycle",
            Self::AdminTransport => "host_admin_transport",
            Self::OperationsBind => "host_operations_bind",
            Self::OperationsServe => "host_operations_serve",
            Self::PathContext => "host_path_context",
            Self::TaskFailure => "host_task_failure",
            Self::InvalidHostContract => "invalid_host_contract",
        }
    }
}

impl fmt::Display for HostErrorCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Service-neutral failure categories accepted by the host boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostErrorKind {
    ConfigDocument,
    Lifecycle,
    AdminTransport,
    OperationsBind,
    OperationsServe,
    PathContext,
    TaskFailure,
    InvalidHostContract,
}

impl HostErrorKind {
    /// Returns the bounded representation safe for public boundaries.
    #[must_use]
    pub const fn safe_error(self) -> SafeHostError {
        match self {
            Self::ConfigDocument => SafeHostError::new(
                HostErrorCode::ConfigDocument,
                "service configuration document is invalid",
            ),
            Self::Lifecycle => SafeHostError::new(
                HostErrorCode::Lifecycle,
                "service lifecycle operation failed",
            ),
            Self::AdminTransport => SafeHostError::new(
                HostErrorCode::AdminTransport,
                "local administration transport failed",
            ),
            Self::OperationsBind => SafeHostError::new(
                HostErrorCode::OperationsBind,
                "operations listener could not be bound",
            ),
            Self::OperationsServe => SafeHostError::new(
                HostErrorCode::OperationsServe,
                "operations listener stopped unexpectedly",
            ),
            Self::PathContext => SafeHostError::new(
                HostErrorCode::PathContext,
                "service path context is invalid",
            ),
            Self::TaskFailure => SafeHostError::new(
                HostErrorCode::TaskFailure,
                "authoritative service task failed",
            ),
            Self::InvalidHostContract => SafeHostError::new(
                HostErrorCode::InvalidHostContract,
                "service host contract is invalid",
            ),
        }
    }
}

/// Error information safe for logs, status, metrics, and wire envelopes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SafeHostError {
    code: HostErrorCode,
    message: &'static str,
}

impl SafeHostError {
    const fn new(code: HostErrorCode, message: &'static str) -> Self {
        assert!(message.len() <= MAX_SAFE_ERROR_MESSAGE_BYTES);
        Self { code, message }
    }

    /// Returns the typed public code.
    #[must_use]
    pub const fn code(self) -> HostErrorCode {
        self.code
    }

    /// Returns the stable string used to serialize the public code.
    #[must_use]
    pub const fn code_str(self) -> &'static str {
        self.code.as_str()
    }

    /// Returns the bounded public message.
    #[must_use]
    pub const fn message(self) -> &'static str {
        self.message
    }
}

impl fmt::Display for SafeHostError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

/// A typed host failure whose public display never includes its internal cause.
#[derive(Debug)]
pub struct HostError {
    kind: HostErrorKind,
    source: Option<Box<dyn Error + Send + Sync + 'static>>,
}

impl HostError {
    /// Creates a host failure without an upstream cause.
    #[must_use]
    pub const fn new(kind: HostErrorKind) -> Self {
        Self { kind, source: None }
    }

    /// Creates a host failure while retaining its cause for trusted inspection.
    pub fn with_source(kind: HostErrorKind, source: impl Error + Send + Sync + 'static) -> Self {
        Self {
            kind,
            source: Some(Box::new(source)),
        }
    }

    /// Returns the service-neutral failure category.
    #[must_use]
    pub const fn kind(&self) -> HostErrorKind {
        self.kind
    }

    /// Returns the bounded representation safe for public boundaries.
    #[must_use]
    pub const fn safe_error(&self) -> SafeHostError {
        self.kind.safe_error()
    }
}

impl fmt::Display for HostError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.safe_error().fmt(formatter)
    }
}

impl Error for HostError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn Error + 'static))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct SensitiveCause;

    impl fmt::Display for SensitiveCause {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("secret upstream detail")
        }
    }

    impl Error for SensitiveCause {}

    #[test]
    fn display_is_safe_while_source_is_preserved() {
        let error = HostError::with_source(HostErrorKind::ConfigDocument, SensitiveCause);

        assert_eq!(
            error.to_string(),
            "host_config_document: service configuration document is invalid"
        );
        assert!(!error.to_string().contains("secret"));
        assert_eq!(
            error.source().map(ToString::to_string).as_deref(),
            Some("secret upstream detail")
        );
    }

    #[test]
    fn safe_codes_have_stable_serialization_helpers() {
        let expected = [
            (HostErrorKind::ConfigDocument, "host_config_document"),
            (HostErrorKind::Lifecycle, "host_lifecycle"),
            (HostErrorKind::AdminTransport, "host_admin_transport"),
            (HostErrorKind::OperationsBind, "host_operations_bind"),
            (HostErrorKind::OperationsServe, "host_operations_serve"),
            (HostErrorKind::PathContext, "host_path_context"),
            (HostErrorKind::TaskFailure, "host_task_failure"),
            (HostErrorKind::InvalidHostContract, "invalid_host_contract"),
        ];

        for (kind, code) in expected {
            let safe = kind.safe_error();
            assert_eq!(safe.code_str(), code);
            assert_eq!(safe.code().to_string(), code);
        }
    }

    #[test]
    fn all_public_messages_are_nonempty_bounded_and_fixed() {
        for kind in [
            HostErrorKind::ConfigDocument,
            HostErrorKind::Lifecycle,
            HostErrorKind::AdminTransport,
            HostErrorKind::OperationsBind,
            HostErrorKind::OperationsServe,
            HostErrorKind::PathContext,
            HostErrorKind::TaskFailure,
            HostErrorKind::InvalidHostContract,
        ] {
            let safe = kind.safe_error();
            assert!(!safe.message().is_empty());
            assert!(safe.message().len() <= MAX_SAFE_ERROR_MESSAGE_BYTES);
            assert!(safe.message().is_ascii());
        }
    }
}
