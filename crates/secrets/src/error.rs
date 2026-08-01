//! Normalized secret-operation errors.

use crate::id::BackendKind;
use core::fmt;

/// Why a [`crate::SecretId`] failed validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SecretIdError {
    /// The identifier was empty.
    Empty,
    /// The identifier exceeded the package limit.
    TooLong {
        /// Observed UTF-8 byte length.
        actual_bytes: usize,
        /// Maximum accepted UTF-8 byte length.
        max_bytes: usize,
    },
    /// The identifier contained a character outside its portable alphabet.
    InvalidCharacter {
        /// UTF-8 byte offset of the invalid character.
        byte_offset: usize,
    },
}

/// A security property requested by a host but unsupported by a provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PolicyRequirement {
    /// The secret must remain device-local.
    DeviceLocal,
    /// The provider must require user presence.
    UserPresence,
    /// The provider must use hardware-backed protection.
    HardwareBacked,
}

/// A normalized provider operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Operation {
    /// Wrap plaintext key material.
    Wrap,
    /// Unwrap protected key material.
    Unwrap,
}

/// A normalized, secret-safe package failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// A secret identifier failed validation.
    InvalidSecretId(SecretIdError),
    /// Key versions start at one; zero is never a valid version.
    InvalidKeyVersion,
    /// Secret material was empty or exceeded the bounded input limit.
    InvalidSecretLength {
        /// Observed byte length.
        actual_bytes: usize,
        /// Maximum accepted byte length.
        max_bytes: usize,
    },
    /// Wrapped material was empty or exceeded the bounded input limit.
    InvalidWrappedLength {
        /// Observed byte length.
        actual_bytes: usize,
        /// Maximum accepted byte length.
        max_bytes: usize,
    },
    /// No explicitly selected provider was available.
    BackendUnavailable {
        /// Requested adapter family.
        backend: BackendKind,
    },
    /// A provider cannot satisfy a required security property.
    PolicyUnsupported {
        /// Provider that rejected the policy.
        backend: BackendKind,
        /// Unsupported property.
        requirement: PolicyRequirement,
    },
    /// A reference was sent to the wrong provider family.
    BackendMismatch {
        /// Provider selected by the host.
        provider: BackendKind,
        /// Provider recorded by the reference.
        reference: BackendKind,
    },
    /// A provider operation failed without exposing its native diagnostic.
    BackendFailure {
        /// Provider that failed.
        backend: BackendKind,
        /// Normalized operation that failed.
        operation: Operation,
    },
}

impl fmt::Display for SecretIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("secret identifier is empty"),
            Self::TooLong {
                actual_bytes,
                max_bytes,
            } => write!(
                formatter,
                "secret identifier is too long: {actual_bytes} bytes; maximum is {max_bytes}"
            ),
            Self::InvalidCharacter { byte_offset } => write!(
                formatter,
                "secret identifier contains an invalid character at byte offset {byte_offset}"
            ),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSecretId(reason) => reason.fmt(formatter),
            Self::InvalidKeyVersion => formatter.write_str("secret key version must be non-zero"),
            Self::InvalidSecretLength {
                actual_bytes,
                max_bytes,
            } => write!(
                formatter,
                "secret material length is invalid: {actual_bytes} bytes; maximum is {max_bytes}"
            ),
            Self::InvalidWrappedLength {
                actual_bytes,
                max_bytes,
            } => write!(
                formatter,
                "wrapped material length is invalid: {actual_bytes} bytes; maximum is {max_bytes}"
            ),
            Self::BackendUnavailable { backend } => {
                write!(formatter, "secret backend {backend:?} is unavailable")
            }
            Self::PolicyUnsupported {
                backend,
                requirement,
            } => write!(
                formatter,
                "secret backend {backend:?} does not satisfy {requirement:?}"
            ),
            Self::BackendMismatch {
                provider,
                reference,
            } => write!(
                formatter,
                "secret reference backend {reference:?} does not match provider {provider:?}"
            ),
            Self::BackendFailure { backend, operation } => write!(
                formatter,
                "secret backend {backend:?} failed during {operation:?}"
            ),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}
