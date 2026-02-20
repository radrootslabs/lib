use thiserror::Error;

#[cfg(not(feature = "std"))]
use alloc::string::String;

#[cfg(all(feature = "std", feature = "json-file"))]
use radroots_runtime::RuntimeJsonError;
#[cfg(feature = "std")]
use std::{io, path::PathBuf};

#[derive(Debug, Error)]
pub enum IdentityError {
    #[cfg(feature = "std")]
    #[error("identity file missing at {0}")]
    NotFound(PathBuf),

    #[cfg(feature = "std")]
    #[error(
        "identity file missing at {0} and generation is not permitted \
        (pass --allow-generate-identity)"
    )]
    GenerationNotAllowed(PathBuf),

    #[cfg(feature = "std")]
    #[error("failed to read identity file at {0}: {1}")]
    Read(PathBuf, #[source] io::Error),

    #[error("invalid identity JSON: {0}")]
    InvalidJson(#[from] serde_json::Error),

    #[error("invalid secret key: {0}")]
    InvalidSecretKey(#[from] nostr::key::Error),

    #[error("invalid public key: {0}")]
    InvalidPublicKey(String),

    #[error("public key does not match secret key")]
    PublicKeyMismatch,

    #[error("unsupported identity file format")]
    InvalidIdentityFormat,

    #[cfg(all(feature = "std", feature = "json-file"))]
    #[error(transparent)]
    Store(#[from] RuntimeJsonError),
}
