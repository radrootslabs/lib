//! Stable safe errors for shared SQLite mechanics.

use core::fmt;
use std::error::Error;

use serde::{Serialize, Serializer};

const MAX_SAFE_ERROR_MESSAGE_BYTES: usize = 96;

/// Stable public codes for service-neutral SQLite failures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceSqliteErrorCode {
    Authority,
    Open,
    Create,
    Pragma,
    Metadata,
    Migration,
    Backup,
    Restore,
    Integrity,
    Recovery,
}

impl ServiceSqliteErrorCode {
    /// Returns the stable lowercase wire representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Authority => "sqlite_authority",
            Self::Open => "sqlite_open",
            Self::Create => "sqlite_create",
            Self::Pragma => "sqlite_pragma",
            Self::Metadata => "sqlite_metadata",
            Self::Migration => "sqlite_migration",
            Self::Backup => "sqlite_backup",
            Self::Restore => "sqlite_restore",
            Self::Integrity => "sqlite_integrity",
            Self::Recovery => "sqlite_recovery",
        }
    }
}

impl fmt::Display for ServiceSqliteErrorCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl Serialize for ServiceSqliteErrorCode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

/// Service-neutral failure classes used by the SQLite mechanism boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceSqliteErrorKind {
    Authority,
    Open,
    Create,
    Pragma,
    Metadata,
    Migration,
    Backup,
    Restore,
    Integrity,
    Recovery,
}

impl ServiceSqliteErrorKind {
    /// Returns the stable public code for this failure class.
    #[must_use]
    pub const fn code(self) -> ServiceSqliteErrorCode {
        match self {
            Self::Authority => ServiceSqliteErrorCode::Authority,
            Self::Open => ServiceSqliteErrorCode::Open,
            Self::Create => ServiceSqliteErrorCode::Create,
            Self::Pragma => ServiceSqliteErrorCode::Pragma,
            Self::Metadata => ServiceSqliteErrorCode::Metadata,
            Self::Migration => ServiceSqliteErrorCode::Migration,
            Self::Backup => ServiceSqliteErrorCode::Backup,
            Self::Restore => ServiceSqliteErrorCode::Restore,
            Self::Integrity => ServiceSqliteErrorCode::Integrity,
            Self::Recovery => ServiceSqliteErrorCode::Recovery,
        }
    }

    /// Returns the bounded projection safe for logs, status, and wire envelopes.
    #[must_use]
    pub const fn safe_error(self) -> SafeServiceSqliteError {
        let message = match self {
            Self::Authority => "SQLite writer authority could not be established",
            Self::Open => "SQLite state could not be opened",
            Self::Create => "SQLite state could not be created",
            Self::Pragma => "SQLite pragma policy could not be applied",
            Self::Metadata => "SQLite metadata is invalid",
            Self::Migration => "SQLite migration history is invalid",
            Self::Backup => "SQLite backup failed",
            Self::Restore => "SQLite restore failed",
            Self::Integrity => "SQLite integrity verification failed",
            Self::Recovery => "SQLite recovery failed",
        };
        SafeServiceSqliteError::new(self.code(), message)
    }
}

/// Bounded service-neutral SQLite error safe for public observation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct SafeServiceSqliteError {
    code: ServiceSqliteErrorCode,
    message: &'static str,
}

impl SafeServiceSqliteError {
    const fn new(code: ServiceSqliteErrorCode, message: &'static str) -> Self {
        assert!(message.is_ascii());
        assert!(message.len() <= MAX_SAFE_ERROR_MESSAGE_BYTES);
        Self { code, message }
    }

    /// Returns the typed stable error code.
    #[must_use]
    pub const fn code(self) -> ServiceSqliteErrorCode {
        self.code
    }

    /// Returns the stable serialized code.
    #[must_use]
    pub const fn code_str(self) -> &'static str {
        self.code.as_str()
    }

    /// Returns the bounded safe message.
    #[must_use]
    pub const fn message(self) -> &'static str {
        self.message
    }
}

impl fmt::Display for SafeServiceSqliteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

/// SQLite mechanism failure with an optional cause for trusted inspection.
pub struct ServiceSqliteError {
    kind: ServiceSqliteErrorKind,
    source: Option<Box<dyn Error + Send + Sync + 'static>>,
}

impl ServiceSqliteError {
    /// Creates a failure without an upstream cause.
    #[must_use]
    pub const fn new(kind: ServiceSqliteErrorKind) -> Self {
        Self { kind, source: None }
    }

    /// Creates a failure while retaining its cause for trusted inspection.
    pub fn with_source(
        kind: ServiceSqliteErrorKind,
        source: impl Error + Send + Sync + 'static,
    ) -> Self {
        Self {
            kind,
            source: Some(Box::new(source)),
        }
    }

    /// Returns the stable service-neutral failure class.
    #[must_use]
    pub const fn kind(&self) -> ServiceSqliteErrorKind {
        self.kind
    }

    /// Returns the bounded projection safe for public observation.
    #[must_use]
    pub const fn safe_error(&self) -> SafeServiceSqliteError {
        self.kind.safe_error()
    }
}

impl fmt::Debug for ServiceSqliteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ServiceSqliteError")
            .field("kind", &self.kind)
            .field("safe_error", &self.safe_error())
            .field("source", &self.source.as_ref().map(|_| "[redacted]"))
            .finish()
    }
}

impl fmt::Display for ServiceSqliteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.safe_error().fmt(formatter)
    }
}

impl Error for ServiceSqliteError {
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
            formatter.write_str("secret path=/private/state.sqlite")
        }
    }

    impl Error for SensitiveCause {}

    #[test]
    fn error_inventory_codes_messages_and_serialization_are_exact() {
        let inventory = [
            (
                ServiceSqliteErrorKind::Authority,
                "sqlite_authority",
                "SQLite writer authority could not be established",
            ),
            (
                ServiceSqliteErrorKind::Open,
                "sqlite_open",
                "SQLite state could not be opened",
            ),
            (
                ServiceSqliteErrorKind::Create,
                "sqlite_create",
                "SQLite state could not be created",
            ),
            (
                ServiceSqliteErrorKind::Pragma,
                "sqlite_pragma",
                "SQLite pragma policy could not be applied",
            ),
            (
                ServiceSqliteErrorKind::Metadata,
                "sqlite_metadata",
                "SQLite metadata is invalid",
            ),
            (
                ServiceSqliteErrorKind::Migration,
                "sqlite_migration",
                "SQLite migration history is invalid",
            ),
            (
                ServiceSqliteErrorKind::Backup,
                "sqlite_backup",
                "SQLite backup failed",
            ),
            (
                ServiceSqliteErrorKind::Restore,
                "sqlite_restore",
                "SQLite restore failed",
            ),
            (
                ServiceSqliteErrorKind::Integrity,
                "sqlite_integrity",
                "SQLite integrity verification failed",
            ),
            (
                ServiceSqliteErrorKind::Recovery,
                "sqlite_recovery",
                "SQLite recovery failed",
            ),
        ];

        for (kind, code, message) in inventory {
            let safe = kind.safe_error();
            assert_eq!(kind.code().as_str(), code);
            assert_eq!(kind.code().to_string(), code);
            assert_eq!(safe.code(), kind.code());
            assert_eq!(safe.code_str(), code);
            assert_eq!(safe.message(), message);
            assert!(message.is_ascii());
            assert!(message.len() <= MAX_SAFE_ERROR_MESSAGE_BYTES);
            assert_eq!(
                serde_json::to_string(&kind.code()).unwrap(),
                format!(r#""{code}""#)
            );
            assert_eq!(
                serde_json::to_string(&safe).unwrap(),
                format!(r#"{{"code":"{code}","message":"{message}"}}"#)
            );
        }
    }

    #[test]
    fn raw_error_redacts_but_preserves_its_trusted_source() {
        let error = ServiceSqliteError::with_source(ServiceSqliteErrorKind::Open, SensitiveCause);
        let display = error.to_string();
        let debug = format!("{error:?}");
        let serialized = serde_json::to_string(&error.safe_error()).unwrap();

        for public in [&display, &debug, &serialized] {
            assert!(!public.contains("secret"));
            assert!(!public.contains("private"));
            assert!(!public.contains("state.sqlite"));
        }
        assert_eq!(
            error.source().map(ToString::to_string).as_deref(),
            Some("secret path=/private/state.sqlite")
        );
        assert_eq!(error.kind(), ServiceSqliteErrorKind::Open);
    }
}
