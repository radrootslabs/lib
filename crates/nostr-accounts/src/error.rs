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
