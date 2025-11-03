use radroots_runtime::RuntimeJsonError;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum IdentityError {
    #[error(transparent)]
    Store(#[from] RuntimeJsonError),

    #[error("invalid identity: {0}")]
    Invalid(#[source] Box<dyn std::error::Error + Send + Sync>),

    #[error(
        "identity file missing at {0} and generation is not permitted \
        (pass --allow-generate-identity)"
    )]
    GenerationNotAllowed(PathBuf),
}
