use alloc::string::String;
use core::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadrootsSimplexAppStoreError {
    SecretVault(String),
    MissingDatabaseKey,
    InvalidDatabaseKey(String),
    EncryptionUnavailable,
    EncryptionKeyRejected,
    MessageLifecycle(String),
    Schema(String),
    Sqlite(String),
    Io(String),
}

impl fmt::Display for RadrootsSimplexAppStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SecretVault(message) => {
                write!(formatter, "SimpleX app secret-vault error: {message}")
            }
            Self::MissingDatabaseKey => {
                write!(
                    formatter,
                    "SimpleX app database key is missing from host secret storage"
                )
            }
            Self::InvalidDatabaseKey(message) => {
                write!(formatter, "SimpleX app database key is invalid: {message}")
            }
            Self::EncryptionUnavailable => {
                write!(formatter, "SimpleX app store encryption is unavailable")
            }
            Self::EncryptionKeyRejected => {
                write!(formatter, "SimpleX app store encryption key was rejected")
            }
            Self::MessageLifecycle(message) => {
                write!(formatter, "SimpleX app message lifecycle error: {message}")
            }
            Self::Schema(message) => write!(formatter, "SimpleX app store schema error: {message}"),
            Self::Sqlite(message) => write!(formatter, "SimpleX app sqlite error: {message}"),
            Self::Io(message) => write!(formatter, "SimpleX app store io error: {message}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsSimplexAppStoreError {}

#[cfg(feature = "std")]
impl From<radroots_secret_vault::RadrootsSecretVaultAccessError> for RadrootsSimplexAppStoreError {
    fn from(value: radroots_secret_vault::RadrootsSecretVaultAccessError) -> Self {
        Self::SecretVault(value.to_string())
    }
}

#[cfg(all(feature = "std", feature = "sqlcipher"))]
impl From<rusqlite::Error> for RadrootsSimplexAppStoreError {
    fn from(value: rusqlite::Error) -> Self {
        Self::Sqlite(value.to_string())
    }
}
