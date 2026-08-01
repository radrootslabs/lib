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
    /// Create or open a provider root.
    Open,
    /// Provision caller-supplied material.
    Provision,
    /// Rotate caller-supplied material.
    Rotate,
    /// Remove provider-owned material.
    Remove,
    /// Read protected provider state.
    Read,
    /// Persist protected provider state.
    Write,
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
    /// The referenced provider-owned key was not found.
    SecretNotFound {
        /// Provider that did not contain the key.
        backend: BackendKind,
        /// Missing key revision.
        key_version: u32,
    },
    /// The referenced provider-owned key already exists.
    SecretAlreadyExists {
        /// Provider that already contains the key.
        backend: BackendKind,
        /// Existing key revision.
        key_version: u32,
    },
    /// A key rotation did not preserve identity or advance the version.
    InvalidRotation,
    /// A filesystem path was relative, traversing, symlinked, or not a file.
    UnsafePath,
    /// Filesystem permissions allowed access outside the current user.
    InsecurePermissions,
    /// Envelope data exceeded the package-wide bound.
    EnvelopeTooLarge {
        /// Observed byte length.
        actual_bytes: usize,
        /// Maximum accepted byte length.
        max_bytes: usize,
    },
    /// Envelope bytes were truncated or structurally invalid.
    EnvelopeMalformed,
    /// The encoded envelope version is not supported.
    UnsupportedEnvelopeVersion {
        /// Observed version number.
        version: u16,
    },
    /// The encoded cipher identifier is not supported.
    UnsupportedCipher {
        /// Observed cipher identifier.
        cipher: u8,
    },
    /// The encoded key-source identifier is not supported.
    UnsupportedKeySource {
        /// Observed key-source identifier.
        key_source: u8,
    },
    /// The encoded backend identifier is not supported.
    UnsupportedBackend {
        /// Observed backend identifier.
        backend: u8,
    },
    /// A data-encryption key had an invalid length.
    InvalidDataKeyLength {
        /// Observed byte length.
        actual_bytes: usize,
    },
    /// Authenticated encryption failed.
    EncryptFailed,
    /// Authentication or decryption failed.
    DecryptFailed,
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
            Self::SecretNotFound {
                backend,
                key_version,
            } => write!(
                formatter,
                "secret backend {backend:?} has no key at version {key_version}"
            ),
            Self::SecretAlreadyExists {
                backend,
                key_version,
            } => write!(
                formatter,
                "secret backend {backend:?} already has key version {key_version}"
            ),
            Self::InvalidRotation => {
                formatter.write_str("secret rotation must preserve identity and advance version")
            }
            Self::UnsafePath => formatter.write_str("secret provider path is unsafe"),
            Self::InsecurePermissions => {
                formatter.write_str("secret provider permissions are insecure")
            }
            Self::EnvelopeTooLarge {
                actual_bytes,
                max_bytes,
            } => write!(
                formatter,
                "encrypted envelope is too large: {actual_bytes} bytes; maximum is {max_bytes}"
            ),
            Self::EnvelopeMalformed => formatter.write_str("encrypted envelope is malformed"),
            Self::UnsupportedEnvelopeVersion { version } => {
                write!(
                    formatter,
                    "encrypted envelope version {version} is unsupported"
                )
            }
            Self::UnsupportedCipher { cipher } => {
                write!(
                    formatter,
                    "encrypted envelope cipher {cipher} is unsupported"
                )
            }
            Self::UnsupportedKeySource { key_source } => write!(
                formatter,
                "encrypted envelope key source {key_source} is unsupported"
            ),
            Self::UnsupportedBackend { backend } => {
                write!(
                    formatter,
                    "encrypted envelope backend {backend} is unsupported"
                )
            }
            Self::InvalidDataKeyLength { actual_bytes } => write!(
                formatter,
                "envelope data key must be 32 bytes; got {actual_bytes}"
            ),
            Self::EncryptFailed => formatter.write_str("encrypted envelope sealing failed"),
            Self::DecryptFailed => formatter.write_str("encrypted envelope authentication failed"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}
