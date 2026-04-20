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

    #[error("invalid account selector: {0}")]
    InvalidAccountSelector(String),

    #[error("account selector is ambiguous: {0}")]
    AmbiguousAccountSelector(String),

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

#[cfg(feature = "std")]
impl From<radroots_secret_vault::RadrootsSecretVaultAccessError> for RadrootsNostrAccountsError {
    fn from(value: radroots_secret_vault::RadrootsSecretVaultAccessError) -> Self {
        Self::Vault(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use radroots_identity::IdentityError;
    use radroots_runtime::RuntimeJsonError;
    use radroots_secret_vault::RadrootsSecretVaultAccessError;
    use std::path::PathBuf;

    #[test]
    fn converts_identity_error() {
        let source = IdentityError::PublicKeyMismatch;
        let converted: RadrootsNostrAccountsError = source.into();
        assert!(converted.to_string().starts_with("identity error:"));
    }

    #[test]
    fn converts_runtime_json_error() {
        let source = RuntimeJsonError::NotFound(PathBuf::from("accounts.json"));
        let converted: RadrootsNostrAccountsError = source.into();
        assert!(converted.to_string().starts_with("store error:"));
    }

    #[test]
    fn converts_secret_vault_access_error() {
        let source = RadrootsSecretVaultAccessError::Backend("vault failed".into());
        let converted: RadrootsNostrAccountsError = source.into();
        assert!(converted.to_string().starts_with("vault error:"));
    }
}
