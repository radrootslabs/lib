use crate::{
    RADROOTS_PROTECTED_STORE_KEY_LENGTH, RADROOTS_PROTECTED_STORE_NONCE_LENGTH,
    RadrootsProtectedStoreEnvelope, error::RadrootsProtectedStoreError,
};
use alloc::borrow::ToOwned;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use getrandom::getrandom;
use radroots_secret_vault::{
    RadrootsSecretKeyWrapping, RadrootsSecretVault, RadrootsSecretVaultAccessError,
};
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use zeroize::Zeroize;

pub const RADROOTS_PROTECTED_FILE_SECRET_SUFFIX: &str = ".secret.json";
pub const RADROOTS_PROTECTED_FILE_WRAPPING_KEY_FILE: &str = ".vault.key";
pub const RADROOTS_PROTECTED_FILE_WRAPPED_KEY_VERSION: u8 = 1;

#[derive(Debug, Clone)]
pub struct RadrootsProtectedFileKeySource {
    key_path: PathBuf,
}

impl RadrootsProtectedFileKeySource {
    #[must_use]
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            key_path: path.as_ref().to_path_buf(),
        }
    }

    #[must_use]
    pub fn from_sidecar_suffix(path: impl AsRef<Path>, suffix: &str) -> Self {
        Self::new(sidecar_path(path, suffix))
    }

    #[must_use]
    pub fn key_path(&self) -> &Path {
        self.key_path.as_path()
    }

    fn load_or_create_wrapping_key(
        &self,
    ) -> Result<[u8; RADROOTS_PROTECTED_STORE_KEY_LENGTH], RadrootsSecretVaultAccessError> {
        if self.key_path.exists() {
            return self.load_wrapping_key();
        }

        if let Some(parent) = self.key_path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent).map_err(io_backend_error)?;
        }

        let mut key = [0_u8; RADROOTS_PROTECTED_STORE_KEY_LENGTH];
        getrandom(&mut key)
            .map_err(|_| RadrootsSecretVaultAccessError::Backend("entropy unavailable".into()))?;
        fs::write(&self.key_path, key.as_slice()).map_err(io_backend_error)?;
        set_secret_permissions(&self.key_path)?;
        Ok(key)
    }

    fn load_wrapping_key(
        &self,
    ) -> Result<[u8; RADROOTS_PROTECTED_STORE_KEY_LENGTH], RadrootsSecretVaultAccessError> {
        let raw = fs::read(&self.key_path).map_err(io_backend_error)?;
        if raw.len() != RADROOTS_PROTECTED_STORE_KEY_LENGTH {
            return Err(RadrootsSecretVaultAccessError::Backend(format!(
                "protected file wrapping key {} has invalid length {}",
                self.key_path.display(),
                raw.len()
            )));
        }

        let mut key = [0_u8; RADROOTS_PROTECTED_STORE_KEY_LENGTH];
        key.copy_from_slice(&raw);
        Ok(key)
    }
}

impl RadrootsSecretKeyWrapping for RadrootsProtectedFileKeySource {
    type Error = RadrootsSecretVaultAccessError;

    fn wrap_data_key(&self, key_slot: &str, plaintext_key: &[u8]) -> Result<Vec<u8>, Self::Error> {
        let mut master_key = self.load_or_create_wrapping_key()?;
        let mut nonce = [0_u8; RADROOTS_PROTECTED_STORE_NONCE_LENGTH];
        getrandom(&mut nonce)
            .map_err(|_| RadrootsSecretVaultAccessError::Backend("entropy unavailable".into()))?;
        let cipher = XChaCha20Poly1305::new(Key::from_slice(&master_key));
        let ciphertext = cipher
            .encrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: plaintext_key,
                    aad: key_slot.as_bytes(),
                },
            )
            .map_err(|_| {
                RadrootsSecretVaultAccessError::Backend(
                    "failed to wrap protected file data key".into(),
                )
            })?;
        master_key.zeroize();

        let mut encoded = Vec::with_capacity(1 + nonce.len() + ciphertext.len());
        encoded.push(RADROOTS_PROTECTED_FILE_WRAPPED_KEY_VERSION);
        encoded.extend_from_slice(&nonce);
        encoded.extend_from_slice(ciphertext.as_slice());
        Ok(encoded)
    }

    fn unwrap_data_key(&self, key_slot: &str, wrapped_key: &[u8]) -> Result<Vec<u8>, Self::Error> {
        if wrapped_key.len() <= 1 + RADROOTS_PROTECTED_STORE_NONCE_LENGTH {
            return Err(RadrootsSecretVaultAccessError::Backend(
                "wrapped protected file data key is truncated".into(),
            ));
        }
        if wrapped_key[0] != RADROOTS_PROTECTED_FILE_WRAPPED_KEY_VERSION {
            return Err(RadrootsSecretVaultAccessError::Backend(format!(
                "unsupported protected file wrapped data key version {}",
                wrapped_key[0]
            )));
        }

        let mut master_key = self.load_wrapping_key()?;
        let nonce_offset = 1;
        let ciphertext_offset = nonce_offset + RADROOTS_PROTECTED_STORE_NONCE_LENGTH;
        let cipher = XChaCha20Poly1305::new(Key::from_slice(&master_key));
        let plaintext = cipher
            .decrypt(
                XNonce::from_slice(&wrapped_key[nonce_offset..ciphertext_offset]),
                Payload {
                    msg: &wrapped_key[ciphertext_offset..],
                    aad: key_slot.as_bytes(),
                },
            )
            .map_err(|_| {
                RadrootsSecretVaultAccessError::Backend(
                    "failed to unwrap protected file data key".into(),
                )
            })?;
        master_key.zeroize();
        Ok(plaintext)
    }
}

#[derive(Debug, Clone)]
pub struct RadrootsProtectedFileSecretVault {
    secrets_dir: PathBuf,
    secret_suffix: String,
    key_source: RadrootsProtectedFileKeySource,
}

impl RadrootsProtectedFileSecretVault {
    #[must_use]
    pub fn new(path: impl AsRef<Path>) -> Self {
        let secrets_dir = path.as_ref().to_path_buf();
        let key_source = RadrootsProtectedFileKeySource::new(
            secrets_dir.join(RADROOTS_PROTECTED_FILE_WRAPPING_KEY_FILE),
        );
        Self {
            secrets_dir,
            secret_suffix: RADROOTS_PROTECTED_FILE_SECRET_SUFFIX.to_owned(),
            key_source,
        }
    }

    #[must_use]
    pub fn secret_suffix(&self) -> &str {
        self.secret_suffix.as_str()
    }

    #[must_use]
    pub fn key_source(&self) -> &RadrootsProtectedFileKeySource {
        &self.key_source
    }

    fn secret_file_path(&self, slot: &str) -> PathBuf {
        self.secrets_dir
            .join(format!("{slot}{}", self.secret_suffix))
    }
}

impl RadrootsSecretVault for RadrootsProtectedFileSecretVault {
    fn store_secret(&self, slot: &str, secret: &str) -> Result<(), RadrootsSecretVaultAccessError> {
        fs::create_dir_all(&self.secrets_dir).map_err(io_backend_error)?;
        let envelope = RadrootsProtectedStoreEnvelope::seal_with_wrapped_key(
            &self.key_source,
            slot,
            secret.as_bytes(),
        )
        .map_err(protected_store_backend_error)?;
        let encoded = envelope
            .encode_json()
            .map_err(protected_store_backend_error)?;
        let path = self.secret_file_path(slot);
        fs::write(&path, encoded).map_err(io_backend_error)?;
        set_secret_permissions(&path)?;
        Ok(())
    }

    fn load_secret(&self, slot: &str) -> Result<Option<String>, RadrootsSecretVaultAccessError> {
        let path = self.secret_file_path(slot);
        let encoded = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(source) => return Err(io_backend_error(source)),
        };
        let envelope = RadrootsProtectedStoreEnvelope::decode_json(&encoded)
            .map_err(protected_store_backend_error)?;
        let plaintext = envelope
            .open_with_wrapped_key(&self.key_source)
            .map_err(protected_store_backend_error)?;
        String::from_utf8(plaintext)
            .map(Some)
            .map_err(|source| RadrootsSecretVaultAccessError::Backend(source.to_string()))
    }

    fn remove_secret(&self, slot: &str) -> Result<(), RadrootsSecretVaultAccessError> {
        match fs::remove_file(self.secret_file_path(slot)) {
            Ok(()) => Ok(()),
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(io_backend_error(source)),
        }
    }
}

#[must_use]
pub fn sidecar_path(path: impl AsRef<Path>, suffix: &str) -> PathBuf {
    let mut value = OsString::from(path.as_ref().as_os_str());
    value.push(suffix);
    PathBuf::from(value)
}

fn io_backend_error(source: std::io::Error) -> RadrootsSecretVaultAccessError {
    RadrootsSecretVaultAccessError::Backend(source.to_string())
}

fn protected_store_backend_error(
    source: RadrootsProtectedStoreError,
) -> RadrootsSecretVaultAccessError {
    RadrootsSecretVaultAccessError::Backend(source.to_string())
}

#[cfg(unix)]
fn set_secret_permissions(path: &Path) -> Result<(), RadrootsSecretVaultAccessError> {
    use std::os::unix::fs::PermissionsExt;

    let permissions = fs::Permissions::from_mode(0o600);
    fs::set_permissions(path, permissions).map_err(io_backend_error)
}

#[cfg(not(unix))]
fn set_secret_permissions(_path: &Path) -> Result<(), RadrootsSecretVaultAccessError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sidecar_path_appends_suffix() {
        let path = sidecar_path("/tmp/demo.enc.json", ".key");
        assert_eq!(path, PathBuf::from("/tmp/demo.enc.json.key"));
    }

    #[test]
    fn file_key_source_wraps_and_unwraps() {
        let temp = tempfile::tempdir().expect("tempdir");
        let key_source = RadrootsProtectedFileKeySource::new(temp.path().join("vault.key"));
        let wrapped = key_source
            .wrap_data_key("acct_demo", b"deadbeefdeadbeefdeadbeefdeadbeef")
            .expect("wrap");
        let unwrapped = key_source
            .unwrap_data_key("acct_demo", &wrapped)
            .expect("unwrap");
        assert_eq!(unwrapped, b"deadbeefdeadbeefdeadbeefdeadbeef");
    }

    #[test]
    fn file_secret_vault_round_trips_secret() {
        let temp = tempfile::tempdir().expect("tempdir");
        let vault = RadrootsProtectedFileSecretVault::new(temp.path());

        vault.store_secret("acct_demo", "deadbeef").expect("store");
        let loaded = vault.load_secret("acct_demo").expect("load");
        assert_eq!(loaded.as_deref(), Some("deadbeef"));

        let raw = fs::read_to_string(temp.path().join("acct_demo.secret.json")).expect("raw file");
        assert!(!raw.contains("deadbeef"));
        assert!(temp.path().join(".vault.key").is_file());
    }

    #[test]
    fn file_secret_vault_removes_secret() {
        let temp = tempfile::tempdir().expect("tempdir");
        let vault = RadrootsProtectedFileSecretVault::new(temp.path());

        vault.store_secret("acct_demo", "deadbeef").expect("store");
        vault.remove_secret("acct_demo").expect("remove");
        assert!(vault.load_secret("acct_demo").expect("load").is_none());
    }
}
