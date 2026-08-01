//! Explicit file-backed secret adapters.

use crate::SecretRef;
use crate::envelope::Nonce;
use crate::error::{Error, Operation};
use crate::id::BackendKind;
use crate::provider::{CapabilitySupport, ResidencySupport, SecretCapabilities, SecretProvider};
use crate::wrapping::{
    BoxFuture, KeyWrapping, SecretMaterial, UnwrapRequest, WrapRequest, WrappedSecret,
};
use alloc::string::String;
use alloc::vec::Vec;
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use core::fmt;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use tempfile::NamedTempFile;

const FILE_MAGIC: &[u8; 4] = b"RRK1";
const FILE_VERSION: u8 = 1;
const DATA_KEY_BYTES: usize = 32;
const NONCE_BYTES: usize = 24;
const MAX_FILE_BYTES: usize = 128 * 1024;

/// Explicit root handling for a file provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum FileOpenMode {
    /// Create a new root and fail if it already exists.
    CreateNew,
    /// Open an existing secure root without creating it.
    OpenExisting,
}

/// File-backed provider using a caller-supplied in-memory master key.
pub struct FileProvider {
    root: PathBuf,
    master_key: SecretMaterial,
}

impl FileProvider {
    /// Creates or opens an absolute normalized provider root.
    pub fn open(
        root: impl AsRef<Path>,
        mode: FileOpenMode,
        master_key: SecretMaterial,
    ) -> Result<Self, Error> {
        validate_master_key(&master_key)?;
        let root = root.as_ref();
        validate_requested_root(root)?;
        match mode {
            FileOpenMode::CreateNew => {
                fs::create_dir(root).map_err(|_| backend_failure(Operation::Open))?;
                set_directory_permissions(root)?;
            }
            FileOpenMode::OpenExisting => validate_existing_root(root)?,
        }
        let root = fs::canonicalize(root).map_err(|_| backend_failure(Operation::Open))?;
        validate_existing_root(&root)?;
        Ok(Self { root, master_key })
    }

    /// Persists caller-owned material with an explicit unique nonce.
    pub fn provision(
        &self,
        reference: &SecretRef,
        material: &SecretMaterial,
        nonce: Nonce,
    ) -> Result<(), Error> {
        validate_file_reference(reference)?;
        let path = self.entry_path(reference);
        reject_existing_path(&path)?;
        let encoded = self.seal_entry(reference, material, nonce)?;
        self.persist_noclobber(path.as_path(), encoded.as_slice())
    }

    /// Resumes safely if the next version reached disk before old-version removal.
    pub fn rotate(
        &self,
        current: &SecretRef,
        next: &SecretRef,
        material: &SecretMaterial,
        nonce: Nonce,
    ) -> Result<(), Error> {
        validate_file_reference(current)?;
        validate_file_reference(next)?;
        if current.id().as_str() != next.id().as_str()
            || next.key_version() <= current.key_version()
        {
            return Err(Error::InvalidRotation);
        }
        if !self.contains(current)? {
            return Err(not_found(current));
        }

        if self.contains(next)? {
            let persisted = self.read_entry(next)?;
            let matches = persisted
                .expose_secret(|expected| material.expose_secret(|actual| expected == actual));
            if !matches {
                return Err(Error::SecretAlreadyExists {
                    backend: BackendKind::File,
                    key_version: next.key_version().get(),
                });
            }
        } else {
            self.provision(next, material, nonce)?;
        }
        self.remove(current)?;
        Ok(())
    }

    /// Removes one exact key revision when present.
    pub fn remove(&self, reference: &SecretRef) -> Result<bool, Error> {
        validate_file_reference(reference)?;
        let path = self.entry_path(reference);
        match fs::symlink_metadata(&path) {
            Ok(metadata) => {
                validate_entry_metadata(&metadata)?;
                validate_file_permissions(&metadata)?;
                fs::remove_file(path).map_err(|_| backend_failure(Operation::Remove))?;
                sync_directory(&self.root)?;
                Ok(true)
            }
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(_) => Err(backend_failure(Operation::Remove)),
        }
    }

    /// Reports whether one exact regular-file entry exists.
    pub fn contains(&self, reference: &SecretRef) -> Result<bool, Error> {
        validate_file_reference(reference)?;
        match fs::symlink_metadata(self.entry_path(reference)) {
            Ok(metadata) => {
                validate_entry_metadata(&metadata)?;
                validate_file_permissions(&metadata)?;
                Ok(true)
            }
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(_) => Err(backend_failure(Operation::Read)),
        }
    }

    fn entry_path(&self, reference: &SecretRef) -> PathBuf {
        self.root.join(entry_name(reference))
    }

    fn seal_entry(
        &self,
        reference: &SecretRef,
        material: &SecretMaterial,
        nonce: Nonce,
    ) -> Result<Vec<u8>, Error> {
        let aad = entry_aad(reference);
        let ciphertext = self.master_key.expose_secret(|master_key| {
            material.expose_secret(|plaintext| {
                let cipher = XChaCha20Poly1305::new(Key::from_slice(master_key));
                cipher
                    .encrypt(
                        XNonce::from_slice(nonce.as_bytes()),
                        Payload {
                            msg: plaintext,
                            aad: aad.as_slice(),
                        },
                    )
                    .map_err(|_| Error::EncryptFailed)
            })
        })?;
        let ciphertext_len =
            u32::try_from(ciphertext.len()).map_err(|_| backend_failure(Operation::Write))?;
        let mut encoded = Vec::with_capacity(4 + 1 + NONCE_BYTES + 4 + ciphertext.len());
        encoded.extend_from_slice(FILE_MAGIC);
        encoded.push(FILE_VERSION);
        encoded.extend_from_slice(nonce.as_bytes());
        encoded.extend_from_slice(&ciphertext_len.to_be_bytes());
        encoded.extend_from_slice(ciphertext.as_slice());
        if encoded.len() > MAX_FILE_BYTES {
            return Err(Error::EnvelopeTooLarge {
                actual_bytes: encoded.len(),
                max_bytes: MAX_FILE_BYTES,
            });
        }
        Ok(encoded)
    }

    fn read_entry(&self, reference: &SecretRef) -> Result<SecretMaterial, Error> {
        let path = self.entry_path(reference);
        let metadata = fs::symlink_metadata(&path).map_err(|source| {
            if source.kind() == std::io::ErrorKind::NotFound {
                not_found(reference)
            } else {
                backend_failure(Operation::Read)
            }
        })?;
        validate_entry_metadata(&metadata)?;
        validate_file_permissions(&metadata)?;
        let file = File::open(path).map_err(|_| backend_failure(Operation::Read))?;
        let mut encoded = Vec::new();
        file.take((MAX_FILE_BYTES + 1) as u64)
            .read_to_end(&mut encoded)
            .map_err(|_| backend_failure(Operation::Read))?;
        if encoded.len() > MAX_FILE_BYTES {
            return Err(Error::EnvelopeTooLarge {
                actual_bytes: encoded.len(),
                max_bytes: MAX_FILE_BYTES,
            });
        }
        self.open_entry(reference, encoded.as_slice())
    }

    fn open_entry(&self, reference: &SecretRef, encoded: &[u8]) -> Result<SecretMaterial, Error> {
        let minimum = 4 + 1 + NONCE_BYTES + 4 + 16;
        if encoded.len() < minimum || &encoded[..4] != FILE_MAGIC {
            return Err(Error::EnvelopeMalformed);
        }
        if encoded[4] != FILE_VERSION {
            return Err(Error::UnsupportedEnvelopeVersion {
                version: u16::from(encoded[4]),
            });
        }
        let nonce: [u8; NONCE_BYTES] = encoded[5..5 + NONCE_BYTES]
            .try_into()
            .map_err(|_| Error::EnvelopeMalformed)?;
        let length_offset = 5 + NONCE_BYTES;
        let ciphertext_len = u32::from_be_bytes(
            encoded[length_offset..length_offset + 4]
                .try_into()
                .map_err(|_| Error::EnvelopeMalformed)?,
        ) as usize;
        let ciphertext = &encoded[length_offset + 4..];
        if ciphertext.len() != ciphertext_len {
            return Err(Error::EnvelopeMalformed);
        }
        let aad = entry_aad(reference);
        let plaintext = self.master_key.expose_secret(|master_key| {
            let cipher = XChaCha20Poly1305::new(Key::from_slice(master_key));
            cipher
                .decrypt(
                    XNonce::from_slice(&nonce),
                    Payload {
                        msg: ciphertext,
                        aad: aad.as_slice(),
                    },
                )
                .map_err(|_| Error::DecryptFailed)
        })?;
        SecretMaterial::from_slice(plaintext.as_slice())
    }

    fn persist_noclobber(&self, path: &Path, encoded: &[u8]) -> Result<(), Error> {
        let mut temporary =
            NamedTempFile::new_in(&self.root).map_err(|_| backend_failure(Operation::Write))?;
        set_file_permissions(temporary.as_file())?;
        temporary
            .write_all(encoded)
            .map_err(|_| backend_failure(Operation::Write))?;
        temporary
            .as_file()
            .sync_all()
            .map_err(|_| backend_failure(Operation::Write))?;
        temporary.persist_noclobber(path).map_err(|error| {
            if error.error.kind() == std::io::ErrorKind::AlreadyExists {
                Error::SecretAlreadyExists {
                    backend: BackendKind::File,
                    key_version: key_version_from_path(path).unwrap_or(0),
                }
            } else {
                backend_failure(Operation::Write)
            }
        })?;
        sync_directory(&self.root)
    }
}

impl fmt::Debug for FileProvider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("FileProvider(<redacted>)")
    }
}

impl KeyWrapping for FileProvider {
    fn wrap<'a>(&'a self, request: WrapRequest<'a>) -> BoxFuture<'a, Result<WrappedSecret, Error>> {
        Box::pin(async move {
            validate_file_reference(request.reference())?;
            let persisted = self.read_entry(request.reference())?;
            let matches = persisted.expose_secret(|expected| {
                request
                    .plaintext()
                    .expose_secret(|actual| expected == actual)
            });
            if !matches {
                return Err(backend_failure(Operation::Wrap));
            }
            WrappedSecret::from_bytes(entry_token(request.reference()))
        })
    }

    fn unwrap<'a>(
        &'a self,
        request: UnwrapRequest<'a>,
    ) -> BoxFuture<'a, Result<SecretMaterial, Error>> {
        Box::pin(async move {
            validate_file_reference(request.reference())?;
            if request.wrapped().as_bytes() != entry_token(request.reference()).as_slice() {
                return Err(backend_failure(Operation::Unwrap));
            }
            self.read_entry(request.reference())
        })
    }
}

impl SecretProvider for FileProvider {
    fn backend_kind(&self) -> BackendKind {
        BackendKind::File
    }

    fn capabilities(&self) -> SecretCapabilities {
        SecretCapabilities::available(
            ResidencySupport::UserProfile,
            CapabilitySupport::Unavailable,
            CapabilitySupport::Unavailable,
        )
    }
}

fn validate_requested_root(root: &Path) -> Result<(), Error> {
    if !root.is_absolute()
        || root
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        return Err(Error::UnsafePath);
    }
    if let Ok(metadata) = fs::symlink_metadata(root)
        && metadata.file_type().is_symlink()
    {
        return Err(Error::UnsafePath);
    }
    Ok(())
}

fn validate_existing_root(root: &Path) -> Result<(), Error> {
    let metadata = fs::symlink_metadata(root).map_err(|_| backend_failure(Operation::Open))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(Error::UnsafePath);
    }
    validate_directory_permissions(&metadata)
}

fn validate_entry_metadata(metadata: &fs::Metadata) -> Result<(), Error> {
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(Error::UnsafePath);
    }
    Ok(())
}

fn validate_master_key(master_key: &SecretMaterial) -> Result<(), Error> {
    if master_key.len() != DATA_KEY_BYTES {
        return Err(Error::InvalidDataKeyLength {
            actual_bytes: master_key.len(),
        });
    }
    Ok(())
}

fn validate_file_reference(reference: &SecretRef) -> Result<(), Error> {
    if reference.backend() != BackendKind::File {
        return Err(Error::BackendMismatch {
            provider: BackendKind::File,
            reference: reference.backend(),
        });
    }
    Ok(())
}

fn reject_existing_path(path: &Path) -> Result<(), Error> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            validate_entry_metadata(&metadata)?;
            Err(Error::SecretAlreadyExists {
                backend: BackendKind::File,
                key_version: key_version_from_path(path).unwrap_or(0),
            })
        }
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(backend_failure(Operation::Write)),
    }
}

fn entry_name(reference: &SecretRef) -> String {
    alloc::format!(
        "{}.v{}.rrk",
        hex_identifier(reference.id().as_str()),
        reference.key_version().get()
    )
}

fn hex_identifier(identifier: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(identifier.len() * 2);
    for byte in identifier.as_bytes() {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

fn entry_token(reference: &SecretRef) -> Vec<u8> {
    let mut token = Vec::from(b"radroots-file-key-v1\0".as_slice());
    token.extend_from_slice(&reference.key_version().get().to_be_bytes());
    token.extend_from_slice(reference.id().as_str().as_bytes());
    token
}

fn entry_aad(reference: &SecretRef) -> Vec<u8> {
    entry_token(reference)
}

fn key_version_from_path(path: &Path) -> Option<u32> {
    let name = path.file_name()?.to_str()?;
    let (_, suffix) = name.rsplit_once(".v")?;
    suffix.strip_suffix(".rrk")?.parse().ok()
}

fn not_found(reference: &SecretRef) -> Error {
    Error::SecretNotFound {
        backend: BackendKind::File,
        key_version: reference.key_version().get(),
    }
}

const fn backend_failure(operation: Operation) -> Error {
    Error::BackendFailure {
        backend: BackendKind::File,
        operation,
    }
}

#[cfg(unix)]
fn set_directory_permissions(path: &Path) -> Result<(), Error> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(|_| backend_failure(Operation::Open))
}

#[cfg(not(unix))]
fn set_directory_permissions(_path: &Path) -> Result<(), Error> {
    Ok(())
}

#[cfg(unix)]
fn validate_directory_permissions(metadata: &fs::Metadata) -> Result<(), Error> {
    use std::os::unix::fs::PermissionsExt;
    if metadata.permissions().mode() & 0o077 != 0 {
        return Err(Error::InsecurePermissions);
    }
    Ok(())
}

#[cfg(not(unix))]
fn validate_directory_permissions(_metadata: &fs::Metadata) -> Result<(), Error> {
    Ok(())
}

#[cfg(unix)]
fn set_file_permissions(file: &File) -> Result<(), Error> {
    use std::os::unix::fs::PermissionsExt;
    file.set_permissions(fs::Permissions::from_mode(0o600))
        .map_err(|_| backend_failure(Operation::Write))
}

#[cfg(not(unix))]
fn set_file_permissions(_file: &File) -> Result<(), Error> {
    Ok(())
}

#[cfg(unix)]
fn validate_file_permissions(metadata: &fs::Metadata) -> Result<(), Error> {
    use std::os::unix::fs::PermissionsExt;
    if metadata.permissions().mode() & 0o077 != 0 {
        return Err(Error::InsecurePermissions);
    }
    Ok(())
}

#[cfg(not(unix))]
fn validate_file_permissions(_metadata: &fs::Metadata) -> Result<(), Error> {
    Ok(())
}

#[cfg(unix)]
fn sync_directory(root: &Path) -> Result<(), Error> {
    File::open(root)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| backend_failure(Operation::Write))
}

#[cfg(not(unix))]
fn sync_directory(_root: &Path) -> Result<(), Error> {
    Ok(())
}
