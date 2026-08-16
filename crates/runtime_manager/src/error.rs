use thiserror::Error;

/// Stable, value-free runtime-management metadata failures.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum RadrootsRuntimeManagerError {
    #[error("runtime management contract exceeds its size limit")]
    ContractTooLarge,
    #[error("parse runtime management contract failed")]
    Parse,
    #[error("runtime management schema is unsupported")]
    UnexpectedSchema,
    #[error("runtime management schema version is unsupported")]
    UnexpectedSchemaVersion,
    #[error("runtime management contract violates the hardened service inventory")]
    InvalidContract,
    #[error("management mode does not support the selected profile")]
    UnsupportedProfile,
    #[error("runtime is not a hardened service target")]
    UnsupportedServiceTarget,
}
