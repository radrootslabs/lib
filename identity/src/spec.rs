use crate::{DEFAULT_IDENTITY_PATH, error::IdentityError};
use radroots_runtime::JsonFile;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{
    path::{Path, PathBuf},
    str::FromStr,
};
use uuid::Uuid;

pub trait IdentitySpec: Serialize + DeserializeOwned + Sized {
    type Keys;

    type ParseError: std::error::Error + Send + Sync + 'static;

    fn generate_new() -> Self;

    fn to_keys(&self) -> Result<Self::Keys, Self::ParseError>;
}

pub fn to_keys<I: IdentitySpec>(id: &I) -> Result<I::Keys, IdentityError> {
    id.to_keys()
        .map_err(|e| IdentityError::Invalid(Box::new(e)))
}

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
