use thiserror::Error;

#[cfg(feature = "std")]
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

    #[error("unsupported identity file format")]
    InvalidIdentityFormat,

    #[cfg(feature = "std")]
    #[error(transparent)]
    Store(#[from] RuntimeJsonError),
}
