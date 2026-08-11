//! Secure Unix-domain admin listener ownership.

use core::fmt;
use fs2::FileExt;
use std::error::Error;
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io;
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::time::Duration;

use rustix::fs::{FileType, Mode, OFlags, fchmod, fstat, open, openat};
use rustix::process::geteuid;

const WRITER_LOCK_FILE_NAME: &str = ".radroots-admin-writer.lock";
/// Final owner-only mode for the runtime directory.
pub const UNIX_ADMIN_OWNER_DIRECTORY_MODE: u32 = 0o700;
/// Final owner-only mode for the socket path.
pub const UNIX_ADMIN_OWNER_SOCKET_MODE: u32 = 0o600;

/// Maximum time spent proving that an existing Unix socket has a live listener.
pub const UNIX_ADMIN_ACTIVE_PROBE_TIMEOUT: Duration = Duration::from_secs(1);

/// A safe, path-redacted failure from Unix admin socket ownership or binding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnixAdminSocketError {
    RuntimeDirectoryNotAbsolute,
    RuntimeDirectoryUnavailable { kind: io::ErrorKind },
    RuntimeDirectoryNotDirectory,
    RuntimeDirectoryWrongOwner,
    RuntimeDirectoryChanged,
    RuntimeDirectoryPermissions { kind: io::ErrorKind },
    InvalidSocketPath,
    WriterLockUnavailable { kind: io::ErrorKind },
    WriterLockInvalidType,
    WriterLockWrongOwner,
    WriterAlreadyActive,
    SocketPathUnavailable { kind: io::ErrorKind },
    SocketPathWrongType,
    SocketPathWrongOwner,
    SocketActive,
    SocketLivenessUnproven,
    StaleSocketCleanup { kind: io::ErrorKind },
    SocketBind { kind: io::ErrorKind },
    SocketPermissions { kind: io::ErrorKind },
    ListenerConfiguration { kind: io::ErrorKind },
}

impl fmt::Display for UnixAdminSocketError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::RuntimeDirectoryNotAbsolute => "admin runtime directory must be absolute",
            Self::RuntimeDirectoryUnavailable { .. } => "admin runtime directory is unavailable",
            Self::RuntimeDirectoryNotDirectory => "admin runtime path is not a directory",
            Self::RuntimeDirectoryWrongOwner => "admin runtime directory has the wrong owner",
            Self::RuntimeDirectoryChanged => "admin runtime directory identity changed",
            Self::RuntimeDirectoryPermissions { .. } => {
                "admin runtime directory permissions could not be secured"
            }
            Self::InvalidSocketPath => "admin socket path is outside its runtime directory",
            Self::WriterLockUnavailable { .. } => "admin writer lock is unavailable",
            Self::WriterLockInvalidType => "admin writer lock path has an unsafe type",
            Self::WriterLockWrongOwner => "admin writer lock has the wrong owner",
            Self::WriterAlreadyActive => "another admin socket writer is active",
            Self::SocketPathUnavailable { .. } => "admin socket path could not be inspected",
            Self::SocketPathWrongType => "admin socket path has an unsafe type",
            Self::SocketPathWrongOwner => "admin socket path has the wrong owner",
            Self::SocketActive => "an admin socket listener is already active",
            Self::SocketLivenessUnproven => "admin socket liveness could not be proven",
            Self::StaleSocketCleanup { .. } => "stale admin socket could not be removed",
            Self::SocketBind { .. } => "admin socket could not be bound",
            Self::SocketPermissions { .. } => "admin socket permissions could not be secured",
            Self::ListenerConfiguration { .. } => "admin listener could not be configured",
        })
    }
}

impl Error for UnixAdminSocketError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FileIdentity {
    device: u64,
    inode: u64,
}

impl FileIdentity {
    fn from_metadata(metadata: &fs::Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
        }
    }
}

/// Exclusive authority required before inspecting or replacing an admin socket.
///
/// The persistent lock sidecar is opened without following symlinks and held for
/// this value's entire lifetime. It is deliberately not exposed as a raw file.
pub struct UnixAdminSocketWriterAuthority {
    runtime_directory: PathBuf,
    _directory: File,
    directory_identity: FileIdentity,
    expected_uid: u32,
    _writer_lock: File,
}

impl fmt::Debug for UnixAdminSocketWriterAuthority {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("UnixAdminSocketWriterAuthority")
            .field("runtime_directory", &"[redacted]")
            .field("writer_lock", &"held")
            .finish()
    }
}

impl UnixAdminSocketWriterAuthority {
    /// Acquires owner-only writer authority for one existing runtime directory.
    pub fn acquire(runtime_directory: impl AsRef<Path>) -> Result<Self, UnixAdminSocketError> {
        let runtime_directory = runtime_directory.as_ref().to_path_buf();
        if !runtime_directory.is_absolute() {
            return Err(UnixAdminSocketError::RuntimeDirectoryNotAbsolute);
        }
        let expected_uid = geteuid().as_raw();
        let directory = open_secure_directory(&runtime_directory, expected_uid)?;
        fchmod(&directory, Mode::RWXU).map_err(|error| {
            UnixAdminSocketError::RuntimeDirectoryPermissions {
                kind: errno_kind(error),
            }
        })?;
        let directory_metadata = directory.metadata().map_err(|error| {
            UnixAdminSocketError::RuntimeDirectoryUnavailable { kind: error.kind() }
        })?;
        let writer_lock = open_writer_lock(&directory, expected_uid)?;
        FileExt::try_lock_exclusive(&writer_lock).map_err(|error| {
            if error.kind() == io::ErrorKind::WouldBlock {
                UnixAdminSocketError::WriterAlreadyActive
            } else {
                UnixAdminSocketError::WriterLockUnavailable { kind: error.kind() }
            }
        })?;

        let authority = Self {
            runtime_directory,
            _directory: directory,
            directory_identity: FileIdentity::from_metadata(&directory_metadata),
            expected_uid,
            _writer_lock: writer_lock,
        };
        authority.ensure_directory_identity()?;
        Ok(authority)
    }

    fn resolve_socket_path(&self, requested: &Path) -> Result<PathBuf, UnixAdminSocketError> {
        let Some(file_name) = requested.file_name() else {
            return Err(UnixAdminSocketError::InvalidSocketPath);
        };
        if requested.parent() != Some(self.runtime_directory.as_path())
            || file_name == OsStr::new(WRITER_LOCK_FILE_NAME)
        {
            return Err(UnixAdminSocketError::InvalidSocketPath);
        }
        Ok(self.runtime_directory.join(file_name))
    }

    fn ensure_directory_identity(&self) -> Result<(), UnixAdminSocketError> {
        let metadata = fs::symlink_metadata(&self.runtime_directory).map_err(|error| {
            UnixAdminSocketError::RuntimeDirectoryUnavailable { kind: error.kind() }
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(UnixAdminSocketError::RuntimeDirectoryChanged);
        }
        if metadata.uid() != self.expected_uid
            || FileIdentity::from_metadata(&metadata) != self.directory_identity
        {
            return Err(UnixAdminSocketError::RuntimeDirectoryChanged);
        }
        Ok(())
    }
}

/// A bound owner-only Unix admin listener with identity-safe cleanup.
pub struct UnixAdminSocketBinding {
    listener: UnixListener,
    socket_path: PathBuf,
    socket_identity: FileIdentity,
    authority: UnixAdminSocketWriterAuthority,
}

impl fmt::Debug for UnixAdminSocketBinding {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("UnixAdminSocketBinding")
            .field("socket_path", &"[redacted]")
            .field("writer_authority", &"held")
            .finish_non_exhaustive()
    }
}

impl UnixAdminSocketBinding {
    /// Validates, probes, and binds one direct child of the authorized runtime directory.
    pub async fn bind(
        authority: UnixAdminSocketWriterAuthority,
        socket_path: impl AsRef<Path>,
    ) -> Result<Self, UnixAdminSocketError> {
        authority.ensure_directory_identity()?;
        let socket_path = authority.resolve_socket_path(socket_path.as_ref())?;
        prepare_socket_path(&authority, &socket_path).await?;
        authority.ensure_directory_identity()?;

        let listener = UnixListener::bind(&socket_path)
            .map_err(|error| UnixAdminSocketError::SocketBind { kind: error.kind() })?;
        let socket_identity = match inspect_socket(&socket_path, authority.expected_uid) {
            Ok(Some(identity)) => identity,
            Ok(None) => {
                drop(listener);
                return Err(UnixAdminSocketError::SocketPathUnavailable {
                    kind: io::ErrorKind::NotFound,
                });
            }
            Err(error) => {
                drop(listener);
                return Err(error);
            }
        };
        if let Err(error) = fs::set_permissions(
            &socket_path,
            fs::Permissions::from_mode(UNIX_ADMIN_OWNER_SOCKET_MODE),
        ) {
            drop(listener);
            remove_matching_socket(&authority, &socket_path, socket_identity);
            return Err(UnixAdminSocketError::SocketPermissions { kind: error.kind() });
        }
        if let Err(error) = listener.set_nonblocking(true) {
            drop(listener);
            remove_matching_socket(&authority, &socket_path, socket_identity);
            return Err(UnixAdminSocketError::ListenerConfiguration { kind: error.kind() });
        }

        Ok(Self {
            listener,
            socket_path,
            socket_identity,
            authority,
        })
    }

    /// Borrows the nonblocking listener without transferring cleanup authority.
    #[must_use]
    pub fn listener(&self) -> &UnixListener {
        &self.listener
    }
}

impl Drop for UnixAdminSocketBinding {
    fn drop(&mut self) {
        remove_matching_socket(&self.authority, &self.socket_path, self.socket_identity);
    }
}

fn open_secure_directory(path: &Path, expected_uid: u32) -> Result<File, UnixAdminSocketError> {
    let directory = open(
        path,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|error| UnixAdminSocketError::RuntimeDirectoryUnavailable {
        kind: errno_kind(error),
    })?;
    let status =
        fstat(&directory).map_err(|error| UnixAdminSocketError::RuntimeDirectoryUnavailable {
            kind: errno_kind(error),
        })?;
    if FileType::from_raw_mode(status.st_mode) != FileType::Directory {
        return Err(UnixAdminSocketError::RuntimeDirectoryNotDirectory);
    }
    validate_owner(status.st_uid, expected_uid)?;
    Ok(File::from(directory))
}

fn validate_owner(actual_uid: u32, expected_uid: u32) -> Result<(), UnixAdminSocketError> {
    if actual_uid == expected_uid {
        Ok(())
    } else {
        Err(UnixAdminSocketError::RuntimeDirectoryWrongOwner)
    }
}

fn open_writer_lock(directory: &File, expected_uid: u32) -> Result<File, UnixAdminSocketError> {
    let descriptor = openat(
        directory,
        WRITER_LOCK_FILE_NAME,
        OFlags::RDWR | OFlags::CREATE | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::RUSR | Mode::WUSR,
    )
    .map_err(|error| UnixAdminSocketError::WriterLockUnavailable {
        kind: errno_kind(error),
    })?;
    let status =
        fstat(&descriptor).map_err(|error| UnixAdminSocketError::WriterLockUnavailable {
            kind: errno_kind(error),
        })?;
    if FileType::from_raw_mode(status.st_mode) != FileType::RegularFile || status.st_nlink != 1 {
        return Err(UnixAdminSocketError::WriterLockInvalidType);
    }
    if status.st_uid != expected_uid {
        return Err(UnixAdminSocketError::WriterLockWrongOwner);
    }
    fchmod(&descriptor, Mode::RUSR | Mode::WUSR).map_err(|error| {
        UnixAdminSocketError::WriterLockUnavailable {
            kind: errno_kind(error),
        }
    })?;
    Ok(File::from(descriptor))
}

async fn prepare_socket_path(
    authority: &UnixAdminSocketWriterAuthority,
    socket_path: &Path,
) -> Result<(), UnixAdminSocketError> {
    let before = match inspect_socket(socket_path, authority.expected_uid)? {
        Some(identity) => identity,
        None => return Ok(()),
    };

    let probe = tokio::time::timeout(
        UNIX_ADMIN_ACTIVE_PROBE_TIMEOUT,
        tokio::net::UnixStream::connect(socket_path),
    )
    .await;
    match probe {
        Ok(Ok(stream)) => {
            drop(stream);
            Err(UnixAdminSocketError::SocketActive)
        }
        Ok(Err(error)) if error.kind() == io::ErrorKind::ConnectionRefused => {
            authority.ensure_directory_identity()?;
            if inspect_socket(socket_path, authority.expected_uid)? != Some(before) {
                return Err(UnixAdminSocketError::SocketLivenessUnproven);
            }
            fs::remove_file(socket_path)
                .map_err(|error| UnixAdminSocketError::StaleSocketCleanup { kind: error.kind() })?;
            Ok(())
        }
        Ok(Err(error)) if error.kind() == io::ErrorKind::NotFound => {
            if inspect_socket(socket_path, authority.expected_uid)?.is_none() {
                Ok(())
            } else {
                Err(UnixAdminSocketError::SocketLivenessUnproven)
            }
        }
        Ok(Err(_)) | Err(_) => Err(UnixAdminSocketError::SocketLivenessUnproven),
    }
}

fn inspect_socket(
    socket_path: &Path,
    expected_uid: u32,
) -> Result<Option<FileIdentity>, UnixAdminSocketError> {
    match fs::symlink_metadata(socket_path) {
        Ok(metadata) => {
            if !metadata.file_type().is_socket() {
                return Err(UnixAdminSocketError::SocketPathWrongType);
            }
            if metadata.uid() != expected_uid {
                return Err(UnixAdminSocketError::SocketPathWrongOwner);
            }
            Ok(Some(FileIdentity::from_metadata(&metadata)))
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(UnixAdminSocketError::SocketPathUnavailable { kind: error.kind() }),
    }
}

fn remove_matching_socket(
    authority: &UnixAdminSocketWriterAuthority,
    socket_path: &Path,
    expected_identity: FileIdentity,
) {
    if authority.ensure_directory_identity().is_err() {
        return;
    }
    let Ok(Some(current_identity)) = inspect_socket(socket_path, authority.expected_uid) else {
        return;
    };
    if expected_identity == current_identity {
        let _ = fs::remove_file(socket_path);
    }
}

fn errno_kind(error: rustix::io::Errno) -> io::ErrorKind {
    io::Error::from_raw_os_error(error.raw_os_error()).kind()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    fn mode(path: &Path) -> u32 {
        fs::symlink_metadata(path)
            .expect("path metadata")
            .permissions()
            .mode()
            & 0o777
    }

    #[tokio::test]
    async fn binds_owner_only_socket_sets_modes_and_cleans_up() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let socket = directory.path().join("admin.sock");
        let authority =
            UnixAdminSocketWriterAuthority::acquire(directory.path()).expect("writer authority");
        let binding = UnixAdminSocketBinding::bind(authority, &socket)
            .await
            .expect("admin binding");

        assert_eq!(mode(directory.path()), UNIX_ADMIN_OWNER_DIRECTORY_MODE);
        assert_eq!(mode(&socket), UNIX_ADMIN_OWNER_SOCKET_MODE);
        assert_eq!(
            mode(&directory.path().join(WRITER_LOCK_FILE_NAME)),
            UNIX_ADMIN_OWNER_SOCKET_MODE
        );
        assert!(
            !binding
                .listener()
                .local_addr()
                .expect("local address")
                .is_unnamed()
        );
        assert!(!format!("{binding:?}").contains(directory.path().to_string_lossy().as_ref()));

        drop(binding);
        assert!(!socket.exists());
        UnixAdminSocketWriterAuthority::acquire(directory.path())
            .expect("writer authority released");
    }

    #[test]
    fn owner_only_mode_inventory_is_literal_and_stable() {
        assert_eq!(UNIX_ADMIN_OWNER_DIRECTORY_MODE, 0o700);
        assert_eq!(UNIX_ADMIN_OWNER_SOCKET_MODE, 0o600);
        assert_eq!(u32::from(Mode::RWXU.bits()), 0o700);
    }

    #[tokio::test]
    async fn refuses_a_live_socket_owned_outside_the_writer_guard() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let socket = directory.path().join("admin.sock");
        let live = UnixListener::bind(&socket).expect("live listener");
        let authority =
            UnixAdminSocketWriterAuthority::acquire(directory.path()).expect("writer authority");

        let error = UnixAdminSocketBinding::bind(authority, &socket)
            .await
            .expect_err("live listener must be retained");
        assert_eq!(error, UnixAdminSocketError::SocketActive);
        assert!(socket.exists());
        drop(live);
    }

    #[tokio::test]
    async fn recovers_only_a_proven_stale_socket() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let socket = directory.path().join("admin.sock");
        drop(UnixListener::bind(&socket).expect("stale listener"));
        assert!(socket.exists());

        let authority =
            UnixAdminSocketWriterAuthority::acquire(directory.path()).expect("writer authority");
        let binding = UnixAdminSocketBinding::bind(authority, &socket)
            .await
            .expect("stale socket recovery");
        assert!(socket.exists());
        drop(binding);
        assert!(!socket.exists());
    }

    #[tokio::test]
    async fn refuses_paths_outside_the_authorized_runtime_directory() {
        let directory = tempfile::tempdir().expect("runtime directory");
        let outside = tempfile::tempdir().expect("outside directory");
        let authority =
            UnixAdminSocketWriterAuthority::acquire(directory.path()).expect("writer authority");

        let error = UnixAdminSocketBinding::bind(authority, outside.path().join("admin.sock"))
            .await
            .expect_err("outside path must fail");
        assert_eq!(error, UnixAdminSocketError::InvalidSocketPath);
    }

    #[tokio::test]
    async fn refuses_non_socket_entries_without_unlinking_them() {
        let directory = tempfile::tempdir().expect("runtime directory");
        let socket = directory.path().join("admin.sock");
        fs::write(&socket, b"not a socket").expect("sentinel file");
        let authority =
            UnixAdminSocketWriterAuthority::acquire(directory.path()).expect("writer authority");

        let error = UnixAdminSocketBinding::bind(authority, &socket)
            .await
            .expect_err("non-socket path must fail");
        assert_eq!(error, UnixAdminSocketError::SocketPathWrongType);
        assert_eq!(
            fs::read(&socket).expect("sentinel retained"),
            b"not a socket"
        );
    }

    #[test]
    fn one_writer_authority_excludes_a_second_writer() {
        let directory = tempfile::tempdir().expect("runtime directory");
        let first = UnixAdminSocketWriterAuthority::acquire(directory.path())
            .expect("first writer authority");
        let error = UnixAdminSocketWriterAuthority::acquire(directory.path())
            .expect_err("second writer must fail");
        assert_eq!(error, UnixAdminSocketError::WriterAlreadyActive);
        drop(first);
    }

    #[test]
    fn refuses_a_symlink_runtime_directory_and_wrong_owner_identity() {
        let target = tempfile::tempdir().expect("runtime directory");
        let link_parent = tempfile::tempdir().expect("link parent");
        let link = link_parent.path().join("runtime");
        symlink(target.path(), &link).expect("runtime symlink");
        assert!(matches!(
            UnixAdminSocketWriterAuthority::acquire(&link),
            Err(UnixAdminSocketError::RuntimeDirectoryUnavailable { .. })
        ));
        assert_eq!(
            validate_owner(41, 42),
            Err(UnixAdminSocketError::RuntimeDirectoryWrongOwner)
        );
    }

    #[test]
    fn refuses_relative_runtime_directory_before_any_file_operation() {
        assert_eq!(
            UnixAdminSocketWriterAuthority::acquire("relative/runtime")
                .expect_err("relative runtime directory must fail"),
            UnixAdminSocketError::RuntimeDirectoryNotAbsolute
        );
    }

    #[tokio::test]
    async fn cleanup_never_unlinks_a_replacement_socket() {
        let directory = tempfile::tempdir().expect("runtime directory");
        let socket = directory.path().join("admin.sock");
        let authority =
            UnixAdminSocketWriterAuthority::acquire(directory.path()).expect("writer authority");
        let binding = UnixAdminSocketBinding::bind(authority, &socket)
            .await
            .expect("admin binding");

        fs::remove_file(&socket).expect("unlink original name");
        let replacement = UnixListener::bind(&socket).expect("replacement listener");
        drop(binding);
        assert!(socket.exists(), "replacement identity must survive cleanup");
        drop(replacement);
        fs::remove_file(&socket).expect("remove replacement");
    }

    #[test]
    fn public_errors_and_authority_debug_never_reveal_runtime_paths() {
        let directory = tempfile::tempdir().expect("runtime directory");
        let authority =
            UnixAdminSocketWriterAuthority::acquire(directory.path()).expect("writer authority");
        let debug = format!("{authority:?}");
        assert!(!debug.contains(directory.path().to_string_lossy().as_ref()));

        let error = UnixAdminSocketError::SocketBind {
            kind: io::ErrorKind::PermissionDenied,
        };
        assert!(!format!("{error:?}").contains('/'));
        assert!(!error.to_string().contains('/'));
    }
}
