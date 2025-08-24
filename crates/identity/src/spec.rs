use crate::{DEFAULT_IDENTITY_PATH, error::IdentityError};
use radroots_runtime::JsonFile;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{
    path::{Path, PathBuf},
    str::FromStr,
};
use uuid::Uuid;

/// Trait that identity file types must implement.
pub trait IdentitySpec: Serialize + DeserializeOwned + Sized {
    /// The runtime key material type (e.g. `nostr::Keys`).
    type Keys;

    /// Error type when parsing stored material into keys.
    type ParseError: std::error::Error + Send + Sync + 'static;

    /// Create a brand new identity value if the file does not exist.
    fn generate_new() -> Self;

    /// Turn this identity into runtime key material.
    fn to_keys(&self) -> Result<Self::Keys, Self::ParseError>;
}

/// Convert an identity into its keys, mapped into the shared error type.
pub fn to_keys<I: IdentitySpec>(id: &I) -> Result<I::Keys, IdentityError> {
    id.to_keys()
        .map_err(|e| IdentityError::Invalid(Box::new(e)))
}

/// Load an identity file, or generate a new one if allowed.
/// Defaults to [`DEFAULT_IDENTITY_PATH`] if no path is provided.
pub fn load_or_generate<I, P>(
    path: Option<P>,
    allow_generate: bool,
) -> Result<JsonFile<I>, IdentityError>
where
    I: IdentitySpec + Serialize + for<'de> Deserialize<'de>,
    P: AsRef<Path>,
{
    let p = path
        .map(|p| p.as_ref().to_path_buf())
        .unwrap_or_else(|| PathBuf::from(DEFAULT_IDENTITY_PATH));

    if p.exists() {
        let store = JsonFile::load(&p)?;
        return Ok(store);
    }

    if !allow_generate {
        return Err(IdentityError::GenerationNotAllowed(p));
    }

    let store = JsonFile::load_or_create_with(&p, I::generate_new)?;
    Ok(store)
}

/// A minimal identity: just a secret key string.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinimalIdentity {
    pub key: String,
}

impl IdentitySpec for MinimalIdentity {
    type Keys = nostr::Keys;
    type ParseError = nostr::key::Error;

    fn generate_new() -> Self {
        let keys = nostr::Keys::generate();
        Self {
            key: keys.secret_key().to_secret_hex(),
        }
    }

    fn to_keys(&self) -> Result<Self::Keys, Self::ParseError> {
        nostr::Keys::from_str(&self.key)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtendedIdentity {
    pub key: String,
    pub identifier: String,
    pub metadata: Option<nostr::Event>,
    pub application_handler: Option<nostr::Event>,
}

impl IdentitySpec for ExtendedIdentity {
    type Keys = nostr::Keys;
    type ParseError = nostr::key::Error;

    fn generate_new() -> Self {
        let keys = nostr::Keys::generate();
        Self {
            key: keys.secret_key().to_secret_hex(),
            identifier: Uuid::new_v4().to_string(),
            metadata: None,
            application_handler: None,
        }
    }

    fn to_keys(&self) -> Result<Self::Keys, Self::ParseError> {
        nostr::Keys::from_str(&self.key)
    }
}
