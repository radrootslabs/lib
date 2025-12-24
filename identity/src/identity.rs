use crate::error::IdentityError;
use core::convert::Infallible;
use nostr::{Keys, SecretKey};
use serde::{Deserialize, Serialize};

#[cfg(not(feature = "std"))]
use alloc::string::String;
#[cfg(feature = "std")]
use radroots_runtime::JsonFile;
#[cfg(feature = "std")]
use std::{
    fs,
    path::{Path, PathBuf},
};

pub const DEFAULT_IDENTITY_PATH: &str = "identity.json";

#[derive(Debug, Clone)]
pub struct RadrootsIdentity {
    keys: Keys,
    profile: Option<RadrootsIdentityProfile>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RadrootsIdentityProfile {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identifier: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<nostr::Event>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub application_handler: Option<nostr::Event>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadrootsIdentityFile {
    pub secret_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identifier: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<nostr::Event>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub application_handler: Option<nostr::Event>,
}

#[derive(Debug, Clone, Copy)]
pub enum RadrootsIdentitySecretKeyFormat {
    Hex,
    Nsec,
}

impl RadrootsIdentityProfile {
    pub fn is_empty(&self) -> bool {
        self.identifier.is_none() && self.metadata.is_none() && self.application_handler.is_none()
    }
}

impl RadrootsIdentity {
    pub fn new(keys: Keys) -> Self {
        Self {
            keys,
            profile: None,
        }
    }

    pub fn with_profile(keys: Keys, profile: RadrootsIdentityProfile) -> Self {
        let profile = if profile.is_empty() { None } else { Some(profile) };
        Self { keys, profile }
    }

    #[cfg(feature = "std")]
    pub fn generate() -> Self {
        Self::new(Keys::generate())
    }

    #[cfg(feature = "std")]
    pub fn generate_with_profile(profile: RadrootsIdentityProfile) -> Self {
        Self::with_profile(Keys::generate(), profile)
    }

    pub fn keys(&self) -> &Keys {
        &self.keys
    }

    pub fn into_keys(self) -> Keys {
        self.keys
    }

    pub fn public_key(&self) -> nostr::PublicKey {
        self.keys.public_key()
    }

    pub fn public_key_hex(&self) -> String {
        self.keys.public_key().to_hex()
    }

    pub fn public_key_npub(&self) -> String {
        use nostr::nips::nip19::ToBech32;
        infallible_to_string(self.keys.public_key().to_bech32())
    }

    pub fn npub(&self) -> String {
        self.public_key_npub()
    }

    pub fn secret_key_hex(&self) -> String {
        self.keys.secret_key().to_secret_hex()
    }

    pub fn secret_key_nsec(&self) -> String {
        use nostr::nips::nip19::ToBech32;
        infallible_to_string(self.keys.secret_key().to_bech32())
    }

    pub fn nsec(&self) -> String {
        self.secret_key_nsec()
    }

    pub fn secret_key_bytes(&self) -> [u8; SecretKey::LEN] {
        self.keys.secret_key().to_secret_bytes()
    }

    pub fn profile(&self) -> Option<&RadrootsIdentityProfile> {
        self.profile.as_ref()
    }

    pub fn profile_mut(&mut self) -> Option<&mut RadrootsIdentityProfile> {
        self.profile.as_mut()
    }

    pub fn set_profile(&mut self, profile: RadrootsIdentityProfile) {
        self.profile = if profile.is_empty() { None } else { Some(profile) };
    }

    pub fn clear_profile(&mut self) {
        self.profile = None;
    }

    pub fn to_file(&self) -> RadrootsIdentityFile {
        self.to_file_with_secret_format(RadrootsIdentitySecretKeyFormat::Hex)
    }

    pub fn to_file_with_secret_format(
        &self,
        format: RadrootsIdentitySecretKeyFormat,
    ) -> RadrootsIdentityFile {
        let secret_key = match format {
            RadrootsIdentitySecretKeyFormat::Hex => self.secret_key_hex(),
            RadrootsIdentitySecretKeyFormat::Nsec => self.secret_key_nsec(),
        };
        let (identifier, metadata, application_handler) = match &self.profile {
            Some(profile) => (
                profile.identifier.clone(),
                profile.metadata.clone(),
                profile.application_handler.clone(),
            ),
            None => (None, None, None),
        };
        RadrootsIdentityFile {
            secret_key,
            identifier,
            metadata,
            application_handler,
        }
    }

    #[cfg(feature = "std")]
    pub fn from_file(file: RadrootsIdentityFile) -> Result<Self, IdentityError> {
        Self::try_from(file)
    }

    #[cfg(feature = "std")]
    pub fn from_secret_key_str(secret_key: &str) -> Result<Self, IdentityError> {
        Ok(Self::new(Keys::parse(secret_key)?))
    }

    #[cfg(feature = "std")]
    pub fn from_secret_key_bytes(secret_key: &[u8]) -> Result<Self, IdentityError> {
        if secret_key.len() != SecretKey::LEN {
            return Err(IdentityError::InvalidIdentityFormat);
        }
        let secret_key = SecretKey::from_slice(secret_key)?;
        Ok(Self::new(Keys::new(secret_key)))
    }

    #[cfg(feature = "std")]
    pub fn load_from_path_auto(path: impl AsRef<Path>) -> Result<Self, IdentityError> {
        let path = path.as_ref();
        let bytes = read_identity_bytes(path)?;
        parse_identity_bytes(&bytes)
    }

    #[cfg(feature = "std")]
    pub fn load_or_generate<P: AsRef<Path>>(
        path: Option<P>,
        allow_generate: bool,
    ) -> Result<Self, IdentityError> {
        let path = path
            .map(|p| p.as_ref().to_path_buf())
            .unwrap_or_else(|| PathBuf::from(DEFAULT_IDENTITY_PATH));
        if path.exists() {
            return Self::load_from_path_auto(&path);
        }
        if !allow_generate {
            return Err(IdentityError::GenerationNotAllowed(path));
        }
        let identity = Self::generate();
        identity.save_json(&path)?;
        Ok(identity)
    }

    #[cfg(feature = "std")]
    pub fn save_json(&self, path: impl AsRef<Path>) -> Result<(), IdentityError> {
        let payload = self.to_file();
        let mut store = JsonFile::load_or_create_with(path.as_ref(), || payload.clone())?;
        store.value = payload;
        store.save()?;
        Ok(())
    }
}

#[cfg(feature = "std")]
impl TryFrom<RadrootsIdentityFile> for RadrootsIdentity {
    type Error = IdentityError;

    fn try_from(file: RadrootsIdentityFile) -> Result<Self, Self::Error> {
        let keys = Keys::parse(&file.secret_key)?;
        let profile = RadrootsIdentityProfile {
            identifier: file.identifier,
            metadata: file.metadata,
            application_handler: file.application_handler,
        };
        if profile.is_empty() {
            Ok(Self::new(keys))
        } else {
            Ok(Self::with_profile(keys, profile))
        }
    }
}

impl From<Keys> for RadrootsIdentity {
    fn from(keys: Keys) -> Self {
        Self::new(keys)
    }
}

#[cfg(feature = "std")]
fn read_identity_bytes(path: &Path) -> Result<Vec<u8>, IdentityError> {
    match fs::read(path) {
        Ok(bytes) => Ok(bytes),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            Err(IdentityError::NotFound(path.to_path_buf()))
        }
        Err(err) => Err(IdentityError::Read(path.to_path_buf(), err)),
    }
}

#[cfg(feature = "std")]
fn parse_identity_bytes(bytes: &[u8]) -> Result<RadrootsIdentity, IdentityError> {
    if bytes.len() == SecretKey::LEN {
        return RadrootsIdentity::from_secret_key_bytes(bytes);
    }

    let text = std::str::from_utf8(bytes).map_err(|_| IdentityError::InvalidIdentityFormat)?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(IdentityError::InvalidIdentityFormat);
    }
    if trimmed.starts_with('{') {
        let file: RadrootsIdentityFile = serde_json::from_str(trimmed)?;
        return RadrootsIdentity::from_file(file);
    }
    RadrootsIdentity::from_secret_key_str(trimmed)
}

fn infallible_to_string(value: Result<String, Infallible>) -> String {
    match value {
        Ok(value) => value,
        Err(err) => match err {},
    }
}
