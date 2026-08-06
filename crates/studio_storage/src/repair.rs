use std::fs::{self, File, OpenOptions};
use std::io::Read;
use std::path::{Path, PathBuf};

use hmac::{Hmac, Mac};
use radroots_studio_domain::{SafeError, SafeErrorCode, SafeMessage};
use rusqlite::{Connection, MAIN_DB, OpenFlags};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::compatibility::{DatabasePreflight, preflight};
use crate::db::{CURRENT_SCHEMA_VERSION, restrict_file_permissions};

type HmacSha256 = Hmac<Sha256>;
const EXPORT_DOMAIN: &[u8] = b"radroots-studio-quarantine-export-v1";
const REPAIR_DOMAIN: &[u8] = b"radroots-studio-repair-candidate-v1";

pub struct RepairAuthorization(Zeroizing<[u8; 32]>);

impl RepairAuthorization {
    /// Moves an exact 256-bit caller authorization secret into zeroizing storage.
    ///
    /// # Errors
    ///
    /// Returns a safe authorization error for every other input length.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, SafeError> {
        let bytes = Zeroizing::new(bytes);
        let value = <[u8; 32]>::try_from(bytes.as_slice()).map_err(|_| unauthorized())?;
        Ok(Self(Zeroizing::new(value)))
    }

    fn expose(&self) -> &[u8; 32] {
        &self.0
    }
}

pub struct QuarantineExportReceipt {
    path: PathBuf,
    sha256: String,
    authentication_tag: String,
}

impl QuarantineExportReceipt {
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub fn sha256(&self) -> &str {
        &self.sha256
    }

    #[must_use]
    pub fn authentication_tag(&self) -> &str {
        &self.authentication_tag
    }
}

pub struct RepairCandidate {
    path: PathBuf,
    sha256: String,
    authentication_tag: String,
}

impl RepairCandidate {
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

pub(crate) fn export_quarantined(
    source: &Path,
    destination: &Path,
    authorization: &RepairAuthorization,
) -> Result<QuarantineExportReceipt, SafeError> {
    if !matches!(preflight(source)?, DatabasePreflight::Quarantined { .. }) {
        return Err(not_quarantined());
    }
    ensure_new_destination(destination)?;
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY
        | OpenFlags::SQLITE_OPEN_NO_MUTEX
        | OpenFlags::SQLITE_OPEN_NOFOLLOW;
    let connection = Connection::open_with_flags(source, flags).map_err(|_| storage_error())?;
    if connection.backup(MAIN_DB, destination, None).is_err() {
        let _ = fs::remove_file(destination);
        return Err(storage_error());
    }
    restrict_file_permissions(destination)?;
    File::open(destination)
        .and_then(|file| file.sync_all())
        .map_err(|_| storage_error())?;
    let sha256 = digest_file(destination)?;
    let authentication_tag = authenticate(authorization, EXPORT_DOMAIN, &sha256)?;
    Ok(QuarantineExportReceipt {
        path: destination.to_path_buf(),
        sha256,
        authentication_tag,
    })
}

pub(crate) fn authenticate_candidate(
    path: &Path,
    authorization: &RepairAuthorization,
) -> Result<RepairCandidate, SafeError> {
    if !matches!(
        preflight(path)?,
        DatabasePreflight::Ready { schema_version } if schema_version <= CURRENT_SCHEMA_VERSION
    ) {
        return Err(storage_error());
    }
    let sha256 = digest_file(path)?;
    let authentication_tag = authenticate(authorization, REPAIR_DOMAIN, &sha256)?;
    Ok(RepairCandidate {
        path: path.to_path_buf(),
        sha256,
        authentication_tag,
    })
}

pub(crate) fn install_candidate(
    target: &Path,
    candidate: &RepairCandidate,
    authorization: &RepairAuthorization,
) -> Result<(), SafeError> {
    if !matches!(preflight(target)?, DatabasePreflight::Quarantined { .. }) {
        return Err(not_quarantined());
    }
    let digest = digest_file(&candidate.path)?;
    if digest != candidate.sha256
        || authenticate(authorization, REPAIR_DOMAIN, &digest)? != candidate.authentication_tag
    {
        return Err(unauthorized());
    }
    if !matches!(preflight(&candidate.path)?, DatabasePreflight::Ready { .. }) {
        return Err(storage_error());
    }
    let parent = target.parent().ok_or_else(storage_error)?;
    let replacement = parent.join(".authenticated-repair.tmp");
    if replacement.try_exists().map_err(|_| storage_error())? {
        return Err(storage_error());
    }
    copy_secure(&candidate.path, &replacement)?;
    let retained = parent.join("studio.sqlite3.quarantined-evidence");
    if retained.try_exists().map_err(|_| storage_error())? {
        let _ = fs::remove_file(&replacement);
        return Err(storage_error());
    }
    fs::rename(target, &retained).map_err(|_| storage_error())?;
    if fs::rename(&replacement, target).is_err() {
        let _ = fs::rename(&retained, target);
        let _ = fs::remove_file(&replacement);
        return Err(storage_error());
    }
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| storage_error())
}

fn ensure_new_destination(path: &Path) -> Result<(), SafeError> {
    if path.try_exists().map_err(|_| storage_error())? {
        return Err(storage_error());
    }
    let parent = path.parent().ok_or_else(storage_error)?;
    let metadata = fs::symlink_metadata(parent).map_err(|_| storage_error())?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(storage_error());
    }
    Ok(())
}

fn copy_secure(source: &Path, destination_path: &Path) -> Result<(), SafeError> {
    let mut source = secure_read(source)?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(
            i32::try_from((rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::CLOEXEC).bits())
                .map_err(|_| storage_error())?,
        );
    }
    let mut destination = options
        .open(destination_path)
        .map_err(|_| storage_error())?;
    std::io::copy(&mut source, &mut destination).map_err(|_| storage_error())?;
    destination.sync_all().map_err(|_| storage_error())?;
    restrict_file_permissions(destination_path)
}

fn secure_read(path: &Path) -> Result<File, SafeError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| storage_error())?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(storage_error());
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(
            i32::try_from((rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::CLOEXEC).bits())
                .map_err(|_| storage_error())?,
        );
    }
    options.open(path).map_err(|_| storage_error())
}

fn digest_file(path: &Path) -> Result<String, SafeError> {
    let mut file = secure_read(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|_| storage_error())?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(hex(&digest.finalize()))
}

fn authenticate(
    authorization: &RepairAuthorization,
    domain: &[u8],
    digest: &str,
) -> Result<String, SafeError> {
    let mut hmac =
        HmacSha256::new_from_slice(authorization.expose()).map_err(|_| unauthorized())?;
    hmac.update(domain);
    hmac.update(digest.as_bytes());
    Ok(hex(&hmac.finalize().into_bytes()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

const fn unauthorized() -> SafeError {
    SafeError::new(
        SafeErrorCode::RepairUnauthorized,
        SafeMessage::new("The database repair authorization is invalid."),
    )
}

const fn not_quarantined() -> SafeError {
    SafeError::new(
        SafeErrorCode::InvalidApplicationState,
        SafeMessage::new("The database is not in quarantine."),
    )
}

const fn storage_error() -> SafeError {
    SafeError::new(
        SafeErrorCode::StorageUnavailable,
        SafeMessage::new("The database repair operation could not be completed."),
    )
}
