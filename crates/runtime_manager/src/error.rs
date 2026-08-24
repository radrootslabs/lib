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
    #[error("runtime context does not match the selected management target")]
    ContextMismatch,
    #[error("bounded local admin client construction failed")]
    AdminClient,
    #[error("bounded local admin request failed")]
    AdminRequest,
    #[error("service status response does not match the selected runtime context")]
    StatusContractMismatch,
}

#[cfg(test)]
mod tests {
    use std::error::Error as _;

    use super::RadrootsRuntimeManagerError;

    #[test]
    fn every_public_failure_is_fixed_value_free_and_source_free() {
        for error in [
            RadrootsRuntimeManagerError::ContractTooLarge,
            RadrootsRuntimeManagerError::Parse,
            RadrootsRuntimeManagerError::UnexpectedSchema,
            RadrootsRuntimeManagerError::UnexpectedSchemaVersion,
            RadrootsRuntimeManagerError::InvalidContract,
            RadrootsRuntimeManagerError::UnsupportedProfile,
            RadrootsRuntimeManagerError::UnsupportedServiceTarget,
            RadrootsRuntimeManagerError::ContextMismatch,
            RadrootsRuntimeManagerError::AdminClient,
            RadrootsRuntimeManagerError::AdminRequest,
            RadrootsRuntimeManagerError::StatusContractMismatch,
        ] {
            let rendered = format!("{error} {error:?}");
            assert!(!rendered.contains('/'));
            assert!(!rendered.contains("secret-value"));
            assert!(error.source().is_none());
        }
    }
}
