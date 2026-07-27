use thiserror::Error;

#[cfg(all(feature = "std", feature = "json-file"))]
use radroots_runtime::RuntimeJsonError;
#[cfg(feature = "std")]
use std::{io, path::PathBuf};

/// Errors produced while validating public identity values.
#[non_exhaustive]
#[derive(Debug, Error)]
pub enum Error {
    #[error("identifier byte representation must contain {expected} bytes, but contained {actual}")]
    InvalidByteLength { expected: usize, actual: usize },

    #[error(
        "identifier hexadecimal representation must contain {expected} bytes, but contained {actual}"
    )]
    InvalidHexLength { expected: usize, actual: usize },

    #[error("identifier contains non-hexadecimal data at byte {index}")]
    InvalidHexCharacter { index: usize },

    #[error("public key bytes are not a valid secp256k1 x-only public key")]
    InvalidPublicKeyBytes,

    #[error("public identity identifier does not match its public key")]
    IdentityIdMismatch,

    #[error("username length must be between {min} and {max} ASCII bytes, but was {actual}")]
    InvalidUsernameLength {
        min: usize,
        max: usize,
        actual: usize,
    },

    #[error("username contains an invalid character at byte {index}")]
    InvalidUsernameCharacter { index: usize },

    #[error("username dots cannot be leading, trailing, or consecutive")]
    InvalidUsernameDotPlacement,
}

/// Transitional errors from the legacy filesystem identity API.
#[cfg(feature = "std")]
#[derive(Debug, Error)]
pub enum IdentityError {
    #[error("identity file missing at {0}")]
    NotFound(PathBuf),

    #[error("failed to read identity file at {0}: {1}")]
    Read(PathBuf, #[source] io::Error),

    #[error("failed to create identity directory {0}: {1}")]
    CreateDir(PathBuf, #[source] io::Error),

    #[error("failed to write identity file at {0}: {1}")]
    Write(PathBuf, #[source] io::Error),

    #[error("invalid identity JSON: {0}")]
    InvalidJson(#[from] serde_json::Error),

    #[cfg(feature = "json-file")]
    #[error(transparent)]
    Store(#[from] RuntimeJsonError),

    #[error(transparent)]
    Paths(#[from] radroots_runtime_paths::RadrootsRuntimePathsError),
}
