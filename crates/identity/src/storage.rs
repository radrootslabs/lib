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
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self::new_path(path.as_ref())
    }

    #[must_use]
    fn new_path(path: &Path) -> Self {
        Self::with_key_slot_path(path, RADROOTS_ENCRYPTED_IDENTITY_DEFAULT_KEY_SLOT)
    }

    #[must_use]
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn with_key_slot(path: impl AsRef<Path>, key_slot: impl Into<Cow<'static, str>>) -> Self {
        Self::with_key_slot_path(path.as_ref(), key_slot)
    }

    #[must_use]
    fn with_key_slot_path(path: &Path, key_slot: impl Into<Cow<'static, str>>) -> Self {
        Self {
            path: path.to_path_buf(),
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

        let payload = identity_file_payload(identity);
        let key_source = RadrootsProtectedFileKeySource::from_sidecar_suffix(
            &self.path,
            RADROOTS_ENCRYPTED_IDENTITY_KEY_SUFFIX,
        );
        let envelope = RadrootsProtectedStoreEnvelope::seal_with_wrapped_key(
            &key_source,
            self.key_slot(),
            &payload,
        )
        .map_err(|error| {
            protected_storage_message(&self.path, "seal encrypted identity", &error)
        })?;
        let encoded = encode_encrypted_identity(&envelope);
        fs::write(&self.path, encoded)
            .map_err(|source| IdentityError::Write(self.path.clone(), source))?;
        apply_secret_permissions(&self.path)?;
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
            protected_storage_message(&self.path, "decode encrypted identity", &error)
        })?;
        let plaintext = envelope
            .open_with_wrapped_key(&key_source)
            .map_err(|error| {
                protected_storage_message(&self.path, "open encrypted identity", &error)
            })?;
        let file: RadrootsIdentityFile = serde_json::from_slice(&plaintext)?;
        RadrootsIdentity::try_from(file)
    }

    pub fn rotate(&self) -> Result<(), IdentityError> {
        let identity = self.load()?;
        let backup = self.rotation_backup()?;

        if let Err(error) = self.store(&identity) {
            let _ = fs::write(&self.path, &backup.envelope);
            let _ = set_secret_permissions(&self.path);
            let _ = fs::write(&backup.key_path, &backup.key);
            let _ = set_secret_permissions(&backup.key_path);
            return Err(error);
        }

        Ok(())
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn rotation_backup(&self) -> Result<EncryptedIdentityRotationBackup, IdentityError> {
        let envelope = fs::read(&self.path)
            .map_err(|source| IdentityError::Read(self.path.clone(), source))?;
        let key_path = self.wrapping_key_path();
        let key =
            fs::read(&key_path).map_err(|source| IdentityError::Read(key_path.clone(), source))?;

        fs::remove_file(&key_path)
            .map_err(|source| IdentityError::Write(key_path.clone(), source))?;

        Ok(EncryptedIdentityRotationBackup {
            envelope,
            key_path,
            key,
        })
    }
}

struct EncryptedIdentityRotationBackup {
    envelope: Vec<u8>,
    key_path: PathBuf,
    key: Vec<u8>,
}

#[must_use]
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn encrypted_identity_wrapping_key_path(path: impl AsRef<Path>) -> PathBuf {
    encrypted_identity_wrapping_key_path_ref(path.as_ref())
}

fn encrypted_identity_wrapping_key_path_ref(path: &Path) -> PathBuf {
    sidecar_path(path, RADROOTS_ENCRYPTED_IDENTITY_KEY_SUFFIX)
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn store_encrypted_identity(
    path: impl AsRef<Path>,
    identity: &RadrootsIdentity,
) -> Result<(), IdentityError> {
    store_encrypted_identity_path(path.as_ref(), identity)
}

fn store_encrypted_identity_path(
    path: &Path,
    identity: &RadrootsIdentity,
) -> Result<(), IdentityError> {
    RadrootsEncryptedIdentityFile::new_path(path).store(identity)
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn store_encrypted_identity_with_key_slot(
    path: impl AsRef<Path>,
    key_slot: impl Into<Cow<'static, str>>,
    identity: &RadrootsIdentity,
) -> Result<(), IdentityError> {
    store_encrypted_identity_with_key_slot_path(path.as_ref(), key_slot, identity)
}

fn store_encrypted_identity_with_key_slot_path(
    path: &Path,
    key_slot: impl Into<Cow<'static, str>>,
    identity: &RadrootsIdentity,
) -> Result<(), IdentityError> {
    RadrootsEncryptedIdentityFile::with_key_slot_path(path, key_slot).store(identity)
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn rotate_encrypted_identity(path: impl AsRef<Path>) -> Result<(), IdentityError> {
    rotate_encrypted_identity_path(path.as_ref())
}

fn rotate_encrypted_identity_path(path: &Path) -> Result<(), IdentityError> {
    RadrootsEncryptedIdentityFile::new_path(path).rotate()
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn rotate_encrypted_identity_with_key_slot(
    path: impl AsRef<Path>,
    key_slot: impl Into<Cow<'static, str>>,
) -> Result<(), IdentityError> {
    rotate_encrypted_identity_with_key_slot_path(path.as_ref(), key_slot)
}

fn rotate_encrypted_identity_with_key_slot_path(
    path: &Path,
    key_slot: impl Into<Cow<'static, str>>,
) -> Result<(), IdentityError> {
    RadrootsEncryptedIdentityFile::with_key_slot_path(path, key_slot).rotate()
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn load_encrypted_identity(path: impl AsRef<Path>) -> Result<RadrootsIdentity, IdentityError> {
    load_encrypted_identity_path(path.as_ref())
}

fn load_encrypted_identity_path(path: &Path) -> Result<RadrootsIdentity, IdentityError> {
    RadrootsEncryptedIdentityFile::new_path(path).load()
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn load_encrypted_identity_with_key_slot(
    path: impl AsRef<Path>,
    key_slot: impl Into<Cow<'static, str>>,
) -> Result<RadrootsIdentity, IdentityError> {
    load_encrypted_identity_with_key_slot_path(path.as_ref(), key_slot)
}

fn load_encrypted_identity_with_key_slot_path(
    path: &Path,
    key_slot: impl Into<Cow<'static, str>>,
) -> Result<RadrootsIdentity, IdentityError> {
    RadrootsEncryptedIdentityFile::with_key_slot_path(path, key_slot).load()
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn store_identity_profile(
    path: impl AsRef<Path>,
    identity: &RadrootsIdentity,
) -> Result<(), IdentityError> {
    store_identity_profile_path(path.as_ref(), identity)
}

fn store_identity_profile_path(
    path: &Path,
    identity: &RadrootsIdentity,
) -> Result<(), IdentityError> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .map_err(|source| IdentityError::CreateDir(parent.to_path_buf(), source))?;
    }

    let encoded = identity_profile_payload(identity);
    fs::write(path, encoded).map_err(|source| IdentityError::Write(path.to_path_buf(), source))?;
    apply_secret_permissions(path)?;
    Ok(())
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub fn load_identity_profile(
    path: impl AsRef<Path>,
) -> Result<RadrootsIdentityPublic, IdentityError> {
    load_identity_profile_path(path.as_ref())
}

fn load_identity_profile_path(path: &Path) -> Result<RadrootsIdentityPublic, IdentityError> {
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

#[cfg_attr(coverage_nightly, coverage(off))]
fn identity_file_payload(identity: &RadrootsIdentity) -> Vec<u8> {
    serde_json::to_vec(&identity.to_file()).expect("identity file serialization is infallible")
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn identity_profile_payload(identity: &RadrootsIdentity) -> Vec<u8> {
    serde_json::to_vec_pretty(&identity.to_public())
        .expect("identity profile serialization is infallible")
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn encode_encrypted_identity(envelope: &RadrootsProtectedStoreEnvelope) -> Vec<u8> {
    envelope
        .encode_json()
        .expect("protected-store envelope serialization is infallible")
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn apply_secret_permissions(path: &Path) -> Result<(), IdentityError> {
    set_secret_permissions(path).map_err(|error| secret_permission_error(path, error))
}

fn protected_storage_message(
    path: &Path,
    action: &str,
    message: &dyn core::fmt::Display,
) -> IdentityError {
    IdentityError::ProtectedStorage {
        path: path.to_path_buf(),
        message: format!("failed to {action}: {message}"),
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn secret_permission_error(path: &Path, error: RadrootsSecretVaultAccessError) -> IdentityError {
    protected_storage_message(path, "update secret-file permissions", &error)
}

#[cfg(unix)]
#[cfg_attr(coverage_nightly, coverage(off))]
fn set_secret_permissions(path: &Path) -> Result<(), RadrootsSecretVaultAccessError> {
    use std::os::unix::fs::PermissionsExt;

    let permissions = std::fs::Permissions::from_mode(0o600);
    fs::set_permissions(path, permissions)
        .map_err(|source| RadrootsSecretVaultAccessError::Backend(source.to_string()))
}

#[cfg(not(unix))]
#[cfg_attr(coverage_nightly, coverage(off))]
fn set_secret_permissions(_path: &Path) -> Result<(), RadrootsSecretVaultAccessError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg_attr(coverage_nightly, coverage(off))]
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

    #[cfg_attr(coverage_nightly, coverage(off))]
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

    #[cfg_attr(coverage_nightly, coverage(off))]
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

    #[cfg_attr(coverage_nightly, coverage(off))]
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

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn encrypted_identity_file_accessors_and_wrappers_use_expected_paths() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("identity.enc.json");
        let identity = RadrootsIdentity::from_secret_key_str(
            "1111111111111111111111111111111111111111111111111111111111111111",
        )
        .expect("identity");

        let default_file = RadrootsEncryptedIdentityFile::new(path.as_path());
        assert_eq!(default_file.path(), path.as_path());
        assert_eq!(
            default_file.key_slot(),
            RADROOTS_ENCRYPTED_IDENTITY_DEFAULT_KEY_SLOT
        );
        assert_eq!(
            default_file.wrapping_key_path(),
            encrypted_identity_wrapping_key_path(path.as_path())
        );

        let custom_file =
            RadrootsEncryptedIdentityFile::with_key_slot(path.as_path(), "custom_identity");
        assert_eq!(custom_file.key_slot(), "custom_identity");

        store_encrypted_identity(path.as_path(), &identity).expect("store encrypted identity");
        rotate_encrypted_identity(path.as_path()).expect("rotate encrypted identity");
        let loaded = load_encrypted_identity(path.as_path()).expect("load encrypted identity");
        assert_eq!(loaded.secret_key_hex(), identity.secret_key_hex());

        store_encrypted_identity_with_key_slot(path.as_path(), "custom_identity", &identity)
            .expect("store encrypted identity with slot");
        rotate_encrypted_identity_with_key_slot(path.as_path(), "custom_identity")
            .expect("rotate encrypted identity with slot");
        let loaded = load_encrypted_identity_with_key_slot(path.as_path(), "custom_identity")
            .expect("load encrypted identity with slot");
        assert_eq!(loaded.secret_key_hex(), identity.secret_key_hex());
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn encrypted_identity_load_reports_read_decode_and_open_errors() {
        let temp = tempfile::tempdir().expect("tempdir");
        let missing = temp.path().join("missing.enc.json");
        let missing_error = load_encrypted_identity(missing.as_path()).expect_err("missing");
        assert!(matches!(missing_error, IdentityError::NotFound(path) if path == missing));

        let read_error = load_encrypted_identity(temp.path()).expect_err("directory read");
        assert!(matches!(read_error, IdentityError::Read(path, _) if path == temp.path()));

        let invalid = temp.path().join("invalid.enc.json");
        fs::write(&invalid, b"not-json").expect("write invalid envelope");
        let decode_error = load_encrypted_identity(invalid.as_path()).expect_err("decode error");
        assert!(matches!(
            decode_error,
            IdentityError::ProtectedStorage { path, message }
                if path == invalid && message.contains("decode encrypted identity")
        ));

        let invalid_plaintext = temp.path().join("invalid-plaintext.enc.json");
        let key_source = RadrootsProtectedFileKeySource::from_sidecar_suffix(
            invalid_plaintext.as_path(),
            RADROOTS_ENCRYPTED_IDENTITY_KEY_SUFFIX,
        );
        let envelope = RadrootsProtectedStoreEnvelope::seal_with_wrapped_key(
            &key_source,
            RADROOTS_ENCRYPTED_IDENTITY_DEFAULT_KEY_SLOT,
            b"not identity json",
        )
        .expect("seal invalid plaintext");
        fs::write(
            &invalid_plaintext,
            envelope.encode_json().expect("encode invalid plaintext"),
        )
        .expect("write invalid plaintext envelope");
        let invalid_plaintext_error =
            load_encrypted_identity(invalid_plaintext.as_path()).expect_err("invalid plaintext");
        assert!(matches!(
            invalid_plaintext_error,
            IdentityError::InvalidJson(_)
        ));

        let path = temp.path().join("identity.enc.json");
        let identity = RadrootsIdentity::from_secret_key_str(
            "1111111111111111111111111111111111111111111111111111111111111111",
        )
        .expect("identity");
        store_encrypted_identity_with_key_slot(path.as_path(), "right_slot", &identity)
            .expect("store encrypted identity");
        fs::write(
            encrypted_identity_wrapping_key_path(path.as_path()),
            b"short",
        )
        .expect("corrupt wrapping key");
        let open_error = load_encrypted_identity(path.as_path()).expect_err("open");
        assert!(matches!(
            open_error,
            IdentityError::ProtectedStorage { path: error_path, message }
                if error_path == path && message.contains("open encrypted identity")
        ));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn encrypted_identity_store_reports_create_write_and_seal_errors() {
        let temp = tempfile::tempdir().expect("tempdir");
        let identity = RadrootsIdentity::from_secret_key_str(
            "1111111111111111111111111111111111111111111111111111111111111111",
        )
        .expect("identity");

        let blocked_parent = temp.path().join("blocked-parent");
        fs::write(&blocked_parent, b"not-a-directory").expect("blocked parent");
        let create_path = blocked_parent.join("identity.enc.json");
        let create_error =
            store_encrypted_identity(create_path.as_path(), &identity).expect_err("create dir");
        assert!(
            matches!(create_error, IdentityError::CreateDir(path, _) if path == blocked_parent)
        );

        let directory_path = temp.path().join("identity-as-directory.enc.json");
        fs::create_dir(&directory_path).expect("identity directory");
        let write_error =
            store_encrypted_identity(directory_path.as_path(), &identity).expect_err("write dir");
        assert!(matches!(write_error, IdentityError::Write(path, _) if path == directory_path));

        let sealed_path = temp.path().join("seal-error.enc.json");
        fs::create_dir(encrypted_identity_wrapping_key_path(sealed_path.as_path()))
            .expect("blocking key directory");
        let seal_error =
            store_encrypted_identity(sealed_path.as_path(), &identity).expect_err("seal");
        assert!(matches!(
            seal_error,
            IdentityError::ProtectedStorage { path, message }
                if path == sealed_path && message.contains("seal encrypted identity")
        ));
    }

    #[cfg(unix)]
    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn encrypted_identity_rotation_restores_wrapping_key_after_store_failure() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("identity.enc.json");
        let identity = RadrootsIdentity::from_secret_key_str(
            "1111111111111111111111111111111111111111111111111111111111111111",
        )
        .expect("identity");

        store_encrypted_identity(path.as_path(), &identity).expect("store encrypted identity");
        let key_path = encrypted_identity_wrapping_key_path(path.as_path());
        let key_before = fs::read(&key_path).expect("key before");

        fs::set_permissions(&path, fs::Permissions::from_mode(0o400)).expect("read only");
        let error = rotate_encrypted_identity(path.as_path()).expect_err("rotate failure");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).expect("writable");

        assert!(matches!(error, IdentityError::Write(error_path, _) if error_path == path));
        assert_eq!(fs::read(&key_path).expect("restored key"), key_before);
        let loaded = load_encrypted_identity(path.as_path()).expect("load restored identity");
        assert_eq!(loaded.secret_key_hex(), identity.secret_key_hex());
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn identity_profile_storage_reports_errors_and_private_fallback() {
        let temp = tempfile::tempdir().expect("tempdir");
        let identity = RadrootsIdentity::from_secret_key_str(
            "1111111111111111111111111111111111111111111111111111111111111111",
        )
        .expect("identity");

        let blocked_parent = temp.path().join("blocked-profile-parent");
        fs::write(&blocked_parent, b"not-a-directory").expect("blocked parent");
        let create_path = blocked_parent.join("profile.json");
        let create_error =
            store_identity_profile(create_path.as_path(), &identity).expect_err("create dir");
        assert!(
            matches!(create_error, IdentityError::CreateDir(path, _) if path == blocked_parent)
        );

        let directory_path = temp.path().join("profile-as-directory.json");
        fs::create_dir(&directory_path).expect("profile directory");
        let write_error =
            store_identity_profile(directory_path.as_path(), &identity).expect_err("write dir");
        assert!(matches!(write_error, IdentityError::Write(path, _) if path == directory_path));

        let missing = temp.path().join("missing-profile.json");
        let missing_error = load_identity_profile(missing.as_path()).expect_err("missing");
        assert!(matches!(missing_error, IdentityError::NotFound(path) if path == missing));

        let read_error = load_identity_profile(temp.path()).expect_err("directory read");
        assert!(matches!(read_error, IdentityError::Read(path, _) if path == temp.path()));

        let private_profile = temp.path().join("private-profile.json");
        fs::write(
            &private_profile,
            serde_json::to_vec(&identity.to_file()).expect("identity file"),
        )
        .expect("write private profile");
        let loaded = load_identity_profile(private_profile.as_path()).expect("load fallback");
        assert_eq!(loaded.id, identity.id());
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn protected_storage_permission_message_uses_operator_action() {
        let path = Path::new("missing-secret-file");
        let error = secret_permission_error(
            path,
            RadrootsSecretVaultAccessError::Backend("permission denied".into()),
        );

        assert!(matches!(
            error,
            IdentityError::ProtectedStorage { path: error_path, message }
                if error_path == path
                    && message.contains("update secret-file permissions")
                    && message.contains("permission denied")
        ));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn storage_supports_parentless_relative_files() {
        let temp = tempfile::tempdir().expect("tempdir");
        let previous = std::env::current_dir().expect("current dir");
        std::env::set_current_dir(temp.path()).expect("set temp cwd");

        let identity = RadrootsIdentity::from_secret_key_str(
            "1111111111111111111111111111111111111111111111111111111111111111",
        )
        .expect("identity");
        let encrypted_path = Path::new("identity.enc.json");
        store_encrypted_identity(encrypted_path, &identity).expect("store encrypted");
        let loaded = load_encrypted_identity(encrypted_path).expect("load encrypted");
        assert_eq!(loaded.secret_key_hex(), identity.secret_key_hex());

        let profile_path = Path::new("profile.json");
        store_identity_profile(profile_path, &identity).expect("store profile");
        let loaded = load_identity_profile(profile_path).expect("load profile");
        assert_eq!(loaded.id, identity.id());

        let empty_path = Path::new("");
        let encrypted_error =
            store_encrypted_identity(empty_path, &identity).expect_err("empty encrypted path");
        assert!(matches!(encrypted_error, IdentityError::Write(_, _)));
        let profile_error =
            store_identity_profile(empty_path, &identity).expect_err("empty profile path");
        assert!(matches!(profile_error, IdentityError::Write(_, _)));

        std::env::set_current_dir(previous).expect("restore cwd");
    }
}
