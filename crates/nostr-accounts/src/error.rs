use thiserror::Error;

#[derive(Debug, Error)]
pub enum RadrootsNostrAccountsError {
    #[error("identity error: {0}")]
    Identity(String),

    #[error("store error: {0}")]
    Store(String),

    #[error("vault error: {0}")]
    Vault(String),

    #[error("account not found: {0}")]
    AccountNotFound(String),

    #[error("account already exists: {0}")]
    AccountAlreadyExists(String),

    #[error("invalid account state: {0}")]
    InvalidState(String),

    #[error("public key does not match secret key")]
    PublicKeyMismatch,
}

#[cfg(feature = "std")]
impl From<radroots_identity::IdentityError> for RadrootsNostrAccountsError {
    fn from(value: radroots_identity::IdentityError) -> Self {
        Self::Identity(value.to_string())
    }
}

#[cfg(feature = "std")]
impl From<radroots_runtime::RuntimeJsonError> for RadrootsNostrAccountsError {
    fn from(value: radroots_runtime::RuntimeJsonError) -> Self {
        Self::Store(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use radroots_identity::IdentityError;
    use radroots_runtime::RuntimeJsonError;
    use std::path::PathBuf;

    #[test]
    fn converts_identity_error() {
        let source = IdentityError::PublicKeyMismatch;
        let converted: RadrootsNostrAccountsError = source.into();
        assert!(matches!(converted, RadrootsNostrAccountsError::Identity(_)));
    }

    #[test]
    fn converts_runtime_json_error() {
        let source = RuntimeJsonError::NotFound(PathBuf::from("accounts.json"));
        let converted: RadrootsNostrAccountsError = source.into();
        assert!(matches!(converted, RadrootsNostrAccountsError::Store(_)));
    }
}
