//! Normalized secret-operation errors.

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

/// A normalized, secret-safe package failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// A secret identifier failed validation.
    InvalidSecretId(SecretIdError),
    /// Key versions start at one; zero is never a valid version.
    InvalidKeyVersion,
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
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}
