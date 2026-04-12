use std::borrow::Cow;
use std::fs;
use std::path::{Path, PathBuf};

use radroots_protected_store::{
    RadrootsProtectedFileKeySource, RadrootsProtectedStoreEnvelope, sidecar_path,
};
use radroots_secret_vault::RadrootsSecretVaultAccessError;

use crate::{IdentityError, RadrootsIdentity, RadrootsIdentityFile, RadrootsIdentityPublic};

pub const RADROOTS_ENCRYPTED_IDENTITY_DEFAULT_KEY_SLOT: &str = "radroots_identity";
pub const RADROOTS_ENCRYPTED_IDENTITY_KEY_SUFFIX: &str = ".key";

#[derive(Debug, Clone)]
pub struct RadrootsEncryptedIdentityFile {
    path: PathBuf,
    key_slot: Cow<'static, str>,
}

impl RadrootsEncryptedIdentityFile {
    #[must_use]
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self::with_key_slot(path, RADROOTS_ENCRYPTED_IDENTITY_DEFAULT_KEY_SLOT)
    }

    #[must_use]
    pub fn with_key_slot(path: impl AsRef<Path>, key_slot: impl Into<Cow<'static, str>>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            key_slot: key_slot.into(),
        }
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        self.path.as_path()
    }

    #[must_use]
    pub fn key_slot(&self) -> &str {
        self.key_slot.as_ref()
    }

    #[must_use]
    pub fn wrapping_key_path(&self) -> PathBuf {
        encrypted_identity_wrapping_key_path(&self.path)
    }

    pub fn store(&self, identity: &RadrootsIdentity) -> Result<(), IdentityError> {
        if let Some(parent) = self.path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent)
                .map_err(|source| IdentityError::CreateDir(parent.to_path_buf(), source))?;
        }

        let payload = serde_json::to_vec(&identity.to_file())?;
        let key_source = RadrootsProtectedFileKeySource::from_sidecar_suffix(
            &self.path,
            RADROOTS_ENCRYPTED_IDENTITY_KEY_SUFFIX,
        );
        let envelope = RadrootsProtectedStoreEnvelope::seal_with_wrapped_key(
            &key_source,
            self.key_slot(),
            &payload,
        )
        .map_err(|error| protected_storage_message(&self.path, "seal encrypted identity", error))?;
        let encoded = envelope.encode_json().map_err(|error| {
            protected_storage_message(&self.path, "encode encrypted identity", error)
        })?;
        fs::write(&self.path, encoded)
            .map_err(|source| IdentityError::Write(self.path.clone(), source))?;
        set_secret_permissions(&self.path).map_err(secret_permission_error(&self.path))?;
        Ok(())
    }

    pub fn load(&self) -> Result<RadrootsIdentity, IdentityError> {
        let encoded = fs::read(&self.path).map_err(|source| {
            if source.kind() == std::io::ErrorKind::NotFound {
                IdentityError::NotFound(self.path.clone())
            } else {
                IdentityError::Read(self.path.clone(), source)
            }
        })?;
        let key_source = RadrootsProtectedFileKeySource::from_sidecar_suffix(
            &self.path,
            RADROOTS_ENCRYPTED_IDENTITY_KEY_SUFFIX,
        );
        let envelope = RadrootsProtectedStoreEnvelope::decode_json(&encoded).map_err(|error| {
            protected_storage_message(&self.path, "decode encrypted identity", error)
        })?;
        let plaintext = envelope
            .open_with_wrapped_key(&key_source)
            .map_err(|error| {
                protected_storage_message(&self.path, "open encrypted identity", error)
            })?;
        let file: RadrootsIdentityFile = serde_json::from_slice(&plaintext)?;
        RadrootsIdentity::try_from(file)
    }

    pub fn rotate(&self) -> Result<(), IdentityError> {
        let identity = self.load()?;
        let envelope_backup = fs::read(&self.path)
            .map_err(|source| IdentityError::Read(self.path.clone(), source))?;
        let key_path = self.wrapping_key_path();
        let key_backup = if key_path.exists() {
            Some(
                fs::read(&key_path)
                    .map_err(|source| IdentityError::Read(key_path.clone(), source))?,
            )
        } else {
            None
        };

        if key_path.exists() {
            fs::remove_file(&key_path)
                .map_err(|source| IdentityError::Write(key_path.clone(), source))?;
        }

        if let Err(error) = self.store(&identity) {
            let _ = fs::write(&self.path, &envelope_backup);
            let _ = set_secret_permissions(&self.path);
            match key_backup {
                Some(key_backup) => {
                    let _ = fs::write(&key_path, &key_backup);
                    let _ = set_secret_permissions(&key_path);
                }
                None => {
                    let _ = fs::remove_file(&key_path);
                }
            }
            return Err(error);
        }

        Ok(())
    }
}

#[must_use]
pub fn encrypted_identity_wrapping_key_path(path: impl AsRef<Path>) -> PathBuf {
    sidecar_path(path, RADROOTS_ENCRYPTED_IDENTITY_KEY_SUFFIX)
}

pub fn store_encrypted_identity(
    path: impl AsRef<Path>,
    identity: &RadrootsIdentity,
) -> Result<(), IdentityError> {
    RadrootsEncryptedIdentityFile::new(path).store(identity)
}

pub fn store_encrypted_identity_with_key_slot(
    path: impl AsRef<Path>,
    key_slot: impl Into<Cow<'static, str>>,
    identity: &RadrootsIdentity,
) -> Result<(), IdentityError> {
    RadrootsEncryptedIdentityFile::with_key_slot(path, key_slot).store(identity)
}

pub fn rotate_encrypted_identity(path: impl AsRef<Path>) -> Result<(), IdentityError> {
    RadrootsEncryptedIdentityFile::new(path).rotate()
}

pub fn rotate_encrypted_identity_with_key_slot(
    path: impl AsRef<Path>,
    key_slot: impl Into<Cow<'static, str>>,
) -> Result<(), IdentityError> {
    RadrootsEncryptedIdentityFile::with_key_slot(path, key_slot).rotate()
}

pub fn load_encrypted_identity(path: impl AsRef<Path>) -> Result<RadrootsIdentity, IdentityError> {
    RadrootsEncryptedIdentityFile::new(path).load()
}

pub fn load_encrypted_identity_with_key_slot(
    path: impl AsRef<Path>,
    key_slot: impl Into<Cow<'static, str>>,
) -> Result<RadrootsIdentity, IdentityError> {
    RadrootsEncryptedIdentityFile::with_key_slot(path, key_slot).load()
}

pub fn store_identity_profile(
    path: impl AsRef<Path>,
    identity: &RadrootsIdentity,
) -> Result<(), IdentityError> {
    let path = path.as_ref();
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .map_err(|source| IdentityError::CreateDir(parent.to_path_buf(), source))?;
    }

    let encoded = serde_json::to_vec_pretty(&identity.to_public())?;
    fs::write(path, encoded).map_err(|source| IdentityError::Write(path.to_path_buf(), source))?;
    set_secret_permissions(path).map_err(secret_permission_error(path))?;
    Ok(())
}

pub fn load_identity_profile(
    path: impl AsRef<Path>,
) -> Result<RadrootsIdentityPublic, IdentityError> {
    let path = path.as_ref();
    let encoded = match fs::read(path) {
        Ok(encoded) => encoded,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
            return Err(IdentityError::NotFound(path.to_path_buf()));
        }
        Err(source) => return Err(IdentityError::Read(path.to_path_buf(), source)),
    };
    if let Ok(public_identity) = serde_json::from_slice::<RadrootsIdentityPublic>(&encoded) {
        return Ok(public_identity);
    }
    RadrootsIdentity::load_from_path_auto(path).map(|identity| identity.to_public())
}

fn protected_storage_message(
    path: &Path,
    action: &str,
    message: impl core::fmt::Display,
) -> IdentityError {
    IdentityError::ProtectedStorage {
        path: path.to_path_buf(),
        message: format!("failed to {action}: {message}"),
    }
}

fn secret_permission_error(
    path: &Path,
) -> impl FnOnce(RadrootsSecretVaultAccessError) -> IdentityError + '_ {
    move |error| protected_storage_message(path, "update secret-file permissions", error)
}

#[cfg(unix)]
fn set_secret_permissions(path: &Path) -> Result<(), RadrootsSecretVaultAccessError> {
    use std::os::unix::fs::PermissionsExt;

    let permissions = std::fs::Permissions::from_mode(0o600);
    fs::set_permissions(path, permissions)
        .map_err(|source| RadrootsSecretVaultAccessError::Backend(source.to_string()))
}

#[cfg(not(unix))]
fn set_secret_permissions(_path: &Path) -> Result<(), RadrootsSecretVaultAccessError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypted_identity_round_trips() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("identity.enc.json");
        let identity = RadrootsIdentity::from_secret_key_str(
            "1111111111111111111111111111111111111111111111111111111111111111",
        )
        .expect("identity");

        store_encrypted_identity(&path, &identity).expect("store encrypted identity");

        let loaded = load_encrypted_identity(&path).expect("load encrypted identity");
        assert_eq!(loaded.id(), identity.id());
        assert_eq!(loaded.secret_key_hex(), identity.secret_key_hex());
        assert!(encrypted_identity_wrapping_key_path(&path).is_file());
    }

    #[test]
    fn encrypted_identity_rotation_rewraps_key() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("identity.enc.json");
        let identity = RadrootsIdentity::from_secret_key_str(
            "1111111111111111111111111111111111111111111111111111111111111111",
        )
        .expect("identity");

        store_encrypted_identity(&path, &identity).expect("store encrypted identity");
        let key_path = encrypted_identity_wrapping_key_path(&path);
        let before = fs::read(&key_path).expect("key before");

        rotate_encrypted_identity(&path).expect("rotate encrypted identity");

        let after = fs::read(&key_path).expect("key after");
        assert_ne!(before, after);
        let loaded = load_encrypted_identity(&path).expect("load rotated identity");
        assert_eq!(loaded.secret_key_hex(), identity.secret_key_hex());
    }

    #[test]
    fn encrypted_identity_supports_custom_key_slot() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("identity.enc.json");
        let identity = RadrootsIdentity::from_secret_key_str(
            "1111111111111111111111111111111111111111111111111111111111111111",
        )
        .expect("identity");

        store_encrypted_identity_with_key_slot(&path, "myc_identity", &identity)
            .expect("store encrypted identity");
        let loaded = load_encrypted_identity_with_key_slot(&path, "myc_identity")
            .expect("load encrypted identity");
        assert_eq!(loaded.secret_key_hex(), identity.secret_key_hex());
    }

    #[test]
    fn identity_profile_round_trips() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("profile.json");
        let mut identity = RadrootsIdentity::from_secret_key_str(
            "1111111111111111111111111111111111111111111111111111111111111111",
        )
        .expect("identity");
        identity.set_profile(crate::RadrootsIdentityProfile::default());

        store_identity_profile(&path, &identity).expect("store profile");

        let loaded = load_identity_profile(&path).expect("load profile");
        assert_eq!(loaded.id, identity.id());
    }
}
