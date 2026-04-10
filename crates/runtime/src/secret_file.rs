use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use getrandom::getrandom;
use radroots_protected_store::{
    RADROOTS_PROTECTED_STORE_KEY_LENGTH, RADROOTS_PROTECTED_STORE_NONCE_LENGTH,
    RadrootsProtectedStoreEnvelope,
};
use radroots_secret_vault::{RadrootsSecretKeyWrapping, RadrootsSecretVaultAccessError};
use zeroize::Zeroize;

use crate::error::RuntimeProtectedFileError;

const LOCAL_WRAPPING_KEY_SUFFIX: &str = ".key";
const WRAPPED_KEY_VERSION: u8 = 1;

#[derive(Debug, Clone)]
struct LocalWrappedKeySource {
    key_path: PathBuf,
}

impl LocalWrappedKeySource {
    fn new(path: &Path) -> Self {
        Self {
            key_path: local_wrapping_key_path(path),
        }
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
                "wrapping key {} has invalid length {}",
                self.key_path.display(),
                raw.len()
            )));
        }

        let mut key = [0_u8; RADROOTS_PROTECTED_STORE_KEY_LENGTH];
        key.copy_from_slice(&raw);
        Ok(key)
    }
}

impl RadrootsSecretKeyWrapping for LocalWrappedKeySource {
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
                    "failed to wrap protected secret data key".into(),
                )
            })?;
        master_key.zeroize();

        let mut encoded = Vec::with_capacity(1 + nonce.len() + ciphertext.len());
        encoded.push(WRAPPED_KEY_VERSION);
        encoded.extend_from_slice(&nonce);
        encoded.extend_from_slice(ciphertext.as_slice());
        Ok(encoded)
    }

    fn unwrap_data_key(&self, key_slot: &str, wrapped_key: &[u8]) -> Result<Vec<u8>, Self::Error> {
        if wrapped_key.len() <= 1 + RADROOTS_PROTECTED_STORE_NONCE_LENGTH {
            return Err(RadrootsSecretVaultAccessError::Backend(
                "wrapped protected secret data key is truncated".into(),
            ));
        }
        if wrapped_key[0] != WRAPPED_KEY_VERSION {
            return Err(RadrootsSecretVaultAccessError::Backend(format!(
                "unsupported wrapped protected secret data key version {}",
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
                    "failed to unwrap protected secret data key".into(),
                )
            })?;
        master_key.zeroize();
        Ok(plaintext)
    }
}

pub fn local_wrapping_key_path(path: impl AsRef<Path>) -> PathBuf {
    let path = path.as_ref();
    let mut value = OsString::from(path.as_os_str());
    value.push(LOCAL_WRAPPING_KEY_SUFFIX);
    PathBuf::from(value)
}

pub fn seal_local_secret_file(
    path: impl AsRef<Path>,
    key_slot: &str,
    payload: &[u8],
) -> Result<(), RuntimeProtectedFileError> {
    let path = path.as_ref();
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|source| RuntimeProtectedFileError::CreateDir {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    let key_source = LocalWrappedKeySource::new(path);
    let envelope =
        RadrootsProtectedStoreEnvelope::seal_with_wrapped_key(&key_source, key_slot, payload)
            .map_err(|error| RuntimeProtectedFileError::Seal {
                path: path.to_path_buf(),
                message: error.to_string(),
            })?;
    let encoded = envelope
        .encode_json()
        .map_err(|error| RuntimeProtectedFileError::Seal {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;
    fs::write(path, encoded).map_err(|source| RuntimeProtectedFileError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    set_secret_permissions(path).map_err(|error| RuntimeProtectedFileError::Permissions {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    Ok(())
}

pub fn open_local_secret_file(
    path: impl AsRef<Path>,
    key_slot: &str,
) -> Result<Vec<u8>, RuntimeProtectedFileError> {
    let path = path.as_ref();
    let encoded = fs::read(path).map_err(|source| RuntimeProtectedFileError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let key_source = LocalWrappedKeySource::new(path);
    let envelope = RadrootsProtectedStoreEnvelope::decode_json(&encoded).map_err(|error| {
        RuntimeProtectedFileError::Decode {
            path: path.to_path_buf(),
            message: error.to_string(),
        }
    })?;
    if envelope.header.key_slot != key_slot {
        return Err(RuntimeProtectedFileError::Open {
            path: path.to_path_buf(),
            message: format!(
                "expected key slot {key_slot}, found {}",
                envelope.header.key_slot
            ),
        });
    }
    envelope
        .open_with_wrapped_key(&key_source)
        .map_err(|error| RuntimeProtectedFileError::Open {
            path: path.to_path_buf(),
            message: error.to_string(),
        })
}

fn io_backend_error(source: std::io::Error) -> RadrootsSecretVaultAccessError {
    RadrootsSecretVaultAccessError::Backend(source.to_string())
}

#[cfg(unix)]
fn set_secret_permissions(path: &Path) -> Result<(), RadrootsSecretVaultAccessError> {
    use std::os::unix::fs::PermissionsExt;

    let permissions = std::fs::Permissions::from_mode(0o600);
    fs::set_permissions(path, permissions).map_err(io_backend_error)
}

#[cfg(not(unix))]
fn set_secret_permissions(_path: &Path) -> Result<(), RadrootsSecretVaultAccessError> {
    Ok(())
}

#[cfg(test)]
mod tests;
