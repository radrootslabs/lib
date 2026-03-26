use thiserror::Error;

#[derive(Debug, Error)]
pub enum RadrootsNostrSignerError {
    #[error("store error: {0}")]
    Store(String),

    #[error("missing signer identity")]
    MissingSignerIdentity,

    #[error("connection not found: {0}")]
    ConnectionNotFound(String),

    #[error(
        "connection already exists for client `{client_public_key}` and user `{user_identity_id}`"
    )]
    ConnectionAlreadyExists {
        client_public_key: String,
        user_identity_id: String,
    },

    #[error("connect secret already in use")]
    ConnectSecretAlreadyInUse,

    #[error("invalid auth url `{0}`")]
    InvalidAuthUrl(String),

    #[error("invalid signer state: {0}")]
    InvalidState(String),

    #[error("invalid granted permission `{0}`")]
    InvalidGrantedPermission(String),

    #[error("invalid connection id `{0}`")]
    InvalidConnectionId(String),

    #[error("invalid request id `{0}`")]
    InvalidRequestId(String),
}

impl From<radroots_runtime::RuntimeJsonError> for RadrootsNostrSignerError {
    fn from(value: radroots_runtime::RuntimeJsonError) -> Self {
        Self::Store(value.to_string())
    }
}

#[cfg(feature = "native")]
impl From<radroots_sql_core::SqlError> for RadrootsNostrSignerError {
    fn from(value: radroots_sql_core::SqlError) -> Self {
        Self::Store(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use radroots_runtime::RuntimeJsonError;
    use std::path::PathBuf;

    #[test]
    fn converts_runtime_json_error() {
        let source = RuntimeJsonError::NotFound(PathBuf::from("signer.json"));
        let converted: RadrootsNostrSignerError = source.into();
        assert!(converted.to_string().starts_with("store error:"));
    }
}
