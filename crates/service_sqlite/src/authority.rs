//! Lifetime authority for the sole writable service database owner.

use core::fmt;
use std::{error::Error, fs::File};

#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::path::PathBuf;

use fs2::FileExt;

use crate::{OpenMode, ServiceSqliteError, ServiceSqliteErrorKind, ServiceSqlitePaths};

/// Exclusive lifetime capability required by writable SQLite open modes.
///
/// This capability is deliberately non-cloneable and exposes no descriptor or
/// lock path:
///
/// ```compile_fail
/// use radroots_service_sqlite::WriterAuthority;
///
/// fn require_clone<T: Clone>() {}
/// require_clone::<WriterAuthority>();
/// ```
pub struct WriterAuthority {
    file: Option<File>,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    database_path: PathBuf,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    directory: File,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    directory_device: u64,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    directory_inode: u64,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    lock_device: u64,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    lock_inode: u64,
}

impl WriterAuthority {
    /// Acquires writer authority without waiting or touching database state.
    ///
    /// Read-only inspection requires no writer authority and performs no
    /// filesystem operation.
    pub fn acquire(
        paths: &ServiceSqlitePaths,
        mode: OpenMode,
    ) -> Result<Option<Self>, ServiceSqliteError> {
        if !mode.requires_writer_authority() {
            return Ok(None);
        }
        acquire_supported(paths).map(Some).map_err(authority_error)
    }

    /// Returns whether this capability still holds the advisory lock.
    #[must_use]
    pub fn is_held(&self) -> bool {
        self.file.is_some()
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    pub(crate) fn directory(&self) -> &File {
        &self.directory
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    pub(crate) fn validate_for(
        &self,
        paths: &ServiceSqlitePaths,
    ) -> Result<(), ServiceSqliteError> {
        require_authority_condition(
            self.is_held() && self.database_path == paths.state_database(),
            WriterAuthorityCause::Mismatched,
        )
        .map_err(authority_error)?;

        #[cfg(any(target_os = "linux", target_os = "macos"))]
        validate_authority_binding(self, paths).map_err(authority_error)?;

        Ok(())
    }

    /// Explicitly releases writer authority; subsequent calls are no-ops.
    pub fn release(&mut self) -> Result<(), ServiceSqliteError> {
        let Some(file) = self.file.as_ref() else {
            return Ok(());
        };
        FileExt::unlock(file).map_err(|_| authority_error(WriterAuthorityCause::UnlockFailed))?;
        self.file.take();
        Ok(())
    }
}

impl fmt::Debug for WriterAuthority {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WriterAuthority")
            .field("state", &if self.is_held() { "held" } else { "released" })
            .finish()
    }
}

impl Drop for WriterAuthority {
    fn drop(&mut self) {
        if let Some(file) = self.file.take() {
            let _ = FileExt::unlock(&file);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WriterAuthorityCause {
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    UnsupportedPlatform,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    StateDirectoryUnavailable,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    StateDirectoryInvalidType,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    StateDirectoryWrongOwner,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    StateDirectoryInsecurePermissions,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    LockUnavailable,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    LockInvalidType,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    LockMultipleLinks,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    LockWrongOwner,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    Contended,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    Mismatched,
    UnlockFailed,
}

impl fmt::Display for WriterAuthorityCause {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            #[cfg(not(any(target_os = "linux", target_os = "macos")))]
            Self::UnsupportedPlatform => "SQLite writer authority is unsupported on this platform",
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            Self::StateDirectoryUnavailable => "SQLite state directory is unavailable",
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            Self::StateDirectoryInvalidType => "SQLite state directory has an invalid type",
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            Self::StateDirectoryWrongOwner => "SQLite state directory has the wrong owner",
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            Self::StateDirectoryInsecurePermissions => {
                "SQLite state directory permissions are insecure"
            }
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            Self::LockUnavailable => "SQLite writer lock is unavailable",
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            Self::LockInvalidType => "SQLite writer lock has an invalid type",
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            Self::LockMultipleLinks => "SQLite writer lock has multiple links",
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            Self::LockWrongOwner => "SQLite writer lock has the wrong owner",
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            Self::Contended => "another SQLite writer is active",
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            Self::Mismatched => "SQLite writer authority does not match this database",
            Self::UnlockFailed => "SQLite writer authority could not be released",
        })
    }
}

impl Error for WriterAuthorityCause {}

fn authority_error(cause: WriterAuthorityCause) -> ServiceSqliteError {
    ServiceSqliteError::with_source(ServiceSqliteErrorKind::Authority, cause)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn acquire_supported(paths: &ServiceSqlitePaths) -> Result<WriterAuthority, WriterAuthorityCause> {
    use rustix::{
        fs::{FileType, Mode, OFlags, fchmod, fstat, open, openat},
        process::geteuid,
    };

    let state_directory = paths
        .state_lock()
        .parent()
        .ok_or(WriterAuthorityCause::StateDirectoryUnavailable)?;
    let directory = open(
        state_directory,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|_| WriterAuthorityCause::StateDirectoryUnavailable)?;
    let directory_status =
        fstat(&directory).map_err(|_| WriterAuthorityCause::StateDirectoryUnavailable)?;
    validate_directory(
        FileType::from_raw_mode(directory_status.st_mode).is_dir(),
        directory_status.st_uid,
        crate::native_metadata::mode(directory_status.st_mode),
        geteuid().as_raw(),
    )?;

    let descriptor = openat(
        &directory,
        radroots_runtime_paths::SERVICE_STATE_LOCK_FILE_NAME,
        OFlags::RDWR | OFlags::CREATE | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
        Mode::RUSR | Mode::WUSR,
    )
    .map_err(|_| WriterAuthorityCause::LockUnavailable)?;
    let lock_status = fstat(&descriptor).map_err(|_| WriterAuthorityCause::LockUnavailable)?;
    validate_lock(
        FileType::from_raw_mode(lock_status.st_mode).is_file(),
        crate::native_metadata::link_count(lock_status.st_nlink),
        lock_status.st_uid,
        geteuid().as_raw(),
    )?;
    fchmod(&descriptor, Mode::RUSR | Mode::WUSR)
        .map_err(|_| WriterAuthorityCause::LockUnavailable)?;

    let lock_status = fstat(&descriptor).map_err(|_| WriterAuthorityCause::LockUnavailable)?;
    require_authority_condition(
        crate::native_metadata::mode(lock_status.st_mode) & 0o777 == 0o600,
        WriterAuthorityCause::LockUnavailable,
    )?;
    let directory_device = crate::native_metadata::device(directory_status.st_dev)
        .map_err(|_| WriterAuthorityCause::StateDirectoryUnavailable)?;
    let lock_device = crate::native_metadata::device(lock_status.st_dev)
        .map_err(|_| WriterAuthorityCause::LockUnavailable)?;
    let file = File::from(descriptor);
    let directory = File::from(directory);
    match FileExt::try_lock_exclusive(&file) {
        Ok(()) => {
            let authority = WriterAuthority {
                file: Some(file),
                database_path: paths.state_database().to_path_buf(),
                directory,
                directory_device,
                directory_inode: directory_status.st_ino,
                lock_device,
                lock_inode: lock_status.st_ino,
            };
            validate_authority_binding(&authority, paths)?;
            Ok(authority)
        }
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
            Err(WriterAuthorityCause::Contended)
        }
        Err(_) => Err(WriterAuthorityCause::LockUnavailable),
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn acquire_supported(_paths: &ServiceSqlitePaths) -> Result<WriterAuthority, WriterAuthorityCause> {
    Err(WriterAuthorityCause::UnsupportedPlatform)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn validate_directory(
    is_directory: bool,
    actual_uid: u32,
    mode: u32,
    expected_uid: u32,
) -> Result<(), WriterAuthorityCause> {
    if !is_directory {
        return Err(WriterAuthorityCause::StateDirectoryInvalidType);
    }
    if actual_uid != expected_uid {
        return Err(WriterAuthorityCause::StateDirectoryWrongOwner);
    }
    if mode & 0o022 != 0 {
        return Err(WriterAuthorityCause::StateDirectoryInsecurePermissions);
    }
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn validate_lock(
    is_regular_file: bool,
    link_count: u64,
    actual_uid: u32,
    expected_uid: u32,
) -> Result<(), WriterAuthorityCause> {
    if !is_regular_file {
        return Err(WriterAuthorityCause::LockInvalidType);
    }
    if link_count != 1 {
        return Err(WriterAuthorityCause::LockMultipleLinks);
    }
    if actual_uid != expected_uid {
        return Err(WriterAuthorityCause::LockWrongOwner);
    }
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn validate_authority_binding(
    authority: &WriterAuthority,
    paths: &ServiceSqlitePaths,
) -> Result<(), WriterAuthorityCause> {
    use rustix::{
        fs::{FileType, Mode, OFlags, fstat, open, openat},
        process::geteuid,
    };

    let directory_path = paths
        .state_database()
        .parent()
        .filter(|parent| Some(*parent) == paths.state_lock().parent())
        .ok_or(WriterAuthorityCause::Mismatched)?;
    let current_directory = open(
        directory_path,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|_| WriterAuthorityCause::Mismatched)?;
    let held_directory =
        fstat(&authority.directory).map_err(|_| WriterAuthorityCause::Mismatched)?;
    let current_directory_status =
        fstat(&current_directory).map_err(|_| WriterAuthorityCause::Mismatched)?;
    let held_directory_device = crate::native_metadata::device(held_directory.st_dev)
        .map_err(|_| WriterAuthorityCause::Mismatched)?;
    let current_directory_device = crate::native_metadata::device(current_directory_status.st_dev)
        .map_err(|_| WriterAuthorityCause::Mismatched)?;
    require_authority_condition(
        crate::all_constraints([
            crate::native_metadata::secure_directory(
                FileType::from_raw_mode(held_directory.st_mode).is_dir(),
                held_directory.st_uid,
                geteuid().as_raw(),
                crate::native_metadata::mode(held_directory.st_mode),
            ),
            crate::native_metadata::secure_directory(
                FileType::from_raw_mode(current_directory_status.st_mode).is_dir(),
                current_directory_status.st_uid,
                geteuid().as_raw(),
                crate::native_metadata::mode(current_directory_status.st_mode),
            ),
            crate::native_metadata::identity_pair_matches(
                held_directory_device,
                held_directory.st_ino,
                current_directory_device,
                current_directory_status.st_ino,
                authority.directory_device,
                authority.directory_inode,
            ),
        ]),
        WriterAuthorityCause::Mismatched,
    )?;

    let current_lock = openat(
        &current_directory,
        radroots_runtime_paths::SERVICE_STATE_LOCK_FILE_NAME,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
        Mode::empty(),
    )
    .map_err(|_| WriterAuthorityCause::Mismatched)?;
    let held_lock = fstat(
        authority
            .file
            .as_ref()
            .ok_or(WriterAuthorityCause::Mismatched)?,
    )
    .map_err(|_| WriterAuthorityCause::Mismatched)?;
    let current_lock_status = fstat(&current_lock).map_err(|_| WriterAuthorityCause::Mismatched)?;
    let held_lock_device = crate::native_metadata::device(held_lock.st_dev)
        .map_err(|_| WriterAuthorityCause::Mismatched)?;
    let current_lock_device = crate::native_metadata::device(current_lock_status.st_dev)
        .map_err(|_| WriterAuthorityCause::Mismatched)?;
    require_authority_condition(
        crate::all_constraints([
            crate::native_metadata::exact_regular_file(
                FileType::from_raw_mode(held_lock.st_mode).is_file(),
                crate::native_metadata::link_count(held_lock.st_nlink),
                held_lock.st_uid,
                geteuid().as_raw(),
                crate::native_metadata::mode(held_lock.st_mode),
            ),
            crate::native_metadata::exact_regular_file(
                FileType::from_raw_mode(current_lock_status.st_mode).is_file(),
                crate::native_metadata::link_count(current_lock_status.st_nlink),
                current_lock_status.st_uid,
                geteuid().as_raw(),
                crate::native_metadata::mode(current_lock_status.st_mode),
            ),
            crate::native_metadata::identity_pair_matches(
                held_lock_device,
                held_lock.st_ino,
                current_lock_device,
                current_lock_status.st_ino,
                authority.lock_device,
                authority.lock_inode,
            ),
        ]),
        WriterAuthorityCause::Mismatched,
    )?;
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn require_authority_condition(
    condition: bool,
    cause: WriterAuthorityCause,
) -> Result<(), WriterAuthorityCause> {
    condition.then_some(()).ok_or(cause)
}

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
mod tests {
    use std::{
        fs,
        os::unix::fs::{MetadataExt, PermissionsExt, symlink},
        path::Path,
    };

    use radroots_runtime_paths::{
        InstanceId, RadrootsHostEnvironment, RadrootsPathProfile, RadrootsPathResolver,
        RadrootsPlatform, RuntimeContext, RuntimeContextBootstrap, RuntimeContextSource, ServiceId,
    };

    use super::*;

    fn paths(root: &Path, instance: &str) -> ServiceSqlitePaths {
        let context = RuntimeContext::resolve(
            &RadrootsPathResolver::new(RadrootsPlatform::Linux, RadrootsHostEnvironment::default()),
            RuntimeContextBootstrap::new(
                RadrootsPathProfile::RepoLocal,
                Some(root.to_path_buf()),
                RuntimeContextSource::BootstrapCli,
                RuntimeContextSource::BootstrapCli,
            )
            .expect("bootstrap"),
            ServiceId::new("myc").expect("service"),
            InstanceId::new(instance).expect("instance"),
        )
        .expect("runtime context");
        ServiceSqlitePaths::from_runtime_context(&context).expect("SQLite paths")
    }

    fn prepare(paths: &ServiceSqlitePaths) {
        fs::create_dir_all(paths.state_lock().parent().expect("state directory"))
            .expect("create state directory");
    }

    #[test]
    fn one_writer_excludes_a_second_until_release_or_drop() {
        let root = tempfile::tempdir().expect("root");
        let paths = paths(root.path(), "primary");
        prepare(&paths);

        let mut first = WriterAuthority::acquire(&paths, OpenMode::Initialize)
            .expect("first acquisition")
            .expect("writer capability");
        assert!(first.is_held());
        let contended = WriterAuthority::acquire(&paths, OpenMode::ReadWriteExisting)
            .expect_err("second writer must fail");
        assert_eq!(contended.kind(), ServiceSqliteErrorKind::Authority);
        assert_eq!(
            contended.source().map(ToString::to_string).as_deref(),
            Some("another SQLite writer is active")
        );

        first.release().expect("release");
        assert!(!first.is_held());
        first.release().expect("idempotent release");
        let next = WriterAuthority::acquire(&paths, OpenMode::ReadWriteExisting)
            .expect("reacquire")
            .expect("writer capability");
        drop(next);
        assert!(
            WriterAuthority::acquire(&paths, OpenMode::ReadWriteExisting)
                .expect("reacquire after drop")
                .is_some()
        );
    }

    #[test]
    fn retained_stale_lock_inode_is_reused_without_content_mutation() {
        let root = tempfile::tempdir().expect("root");
        let paths = paths(root.path(), "stale");
        prepare(&paths);
        fs::write(paths.state_lock(), b"stale-evidence").expect("stale lock");
        let before = fs::metadata(paths.state_lock()).expect("before metadata");

        let mut authority = WriterAuthority::acquire(&paths, OpenMode::Initialize)
            .expect("acquire stale inode")
            .expect("writer capability");
        authority.release().expect("release stale inode");

        let after = fs::metadata(paths.state_lock()).expect("after metadata");
        assert_eq!(before.dev(), after.dev());
        assert_eq!(before.ino(), after.ino());
        assert_eq!(
            fs::read(paths.state_lock()).expect("stale content"),
            b"stale-evidence"
        );
        assert_eq!(after.permissions().mode() & 0o777, 0o600);
    }

    #[test]
    fn read_only_inspection_has_no_filesystem_side_effect() {
        let root = tempfile::tempdir().expect("root");
        let paths = paths(root.path(), "inspection");
        let state_directory = paths.state_lock().parent().expect("state directory");
        assert!(!state_directory.exists());
        assert!(
            WriterAuthority::acquire(&paths, OpenMode::ReadOnlyInspection)
                .expect("read-only declaration")
                .is_none()
        );
        assert!(!state_directory.exists());
        assert!(!paths.state_lock().exists());
    }

    #[test]
    fn unsafe_directory_and_lock_shapes_fail_closed() {
        assert_eq!(
            validate_directory(true, 10, 0o40700, 11),
            Err(WriterAuthorityCause::StateDirectoryWrongOwner)
        );
        assert_eq!(
            validate_directory(true, 10, 0o40720, 10),
            Err(WriterAuthorityCause::StateDirectoryInsecurePermissions)
        );
        assert_eq!(
            validate_directory(false, 10, 0o100600, 10),
            Err(WriterAuthorityCause::StateDirectoryInvalidType)
        );
        assert_eq!(
            validate_lock(false, 1, 10, 10),
            Err(WriterAuthorityCause::LockInvalidType)
        );
        assert_eq!(
            validate_lock(true, 2, 10, 10),
            Err(WriterAuthorityCause::LockMultipleLinks)
        );
        assert_eq!(
            validate_lock(true, 1, 10, 11),
            Err(WriterAuthorityCause::LockWrongOwner)
        );

        let root = tempfile::tempdir().expect("root");
        let symlink_paths = paths(root.path(), "symlink");
        prepare(&symlink_paths);
        let target = root.path().join("target");
        fs::write(&target, []).expect("target");
        symlink(&target, symlink_paths.state_lock()).expect("lock symlink");
        assert!(WriterAuthority::acquire(&symlink_paths, OpenMode::Initialize).is_err());

        let hardlink_paths = paths(root.path(), "hardlink");
        prepare(&hardlink_paths);
        fs::hard_link(&target, hardlink_paths.state_lock()).expect("lock hard link");
        assert!(WriterAuthority::acquire(&hardlink_paths, OpenMode::Initialize).is_err());

        let directory_paths = paths(root.path(), "directory");
        prepare(&directory_paths);
        fs::create_dir(directory_paths.state_lock()).expect("directory lock");
        assert!(WriterAuthority::acquire(&directory_paths, OpenMode::Initialize).is_err());

        let insecure_paths = paths(root.path(), "insecure");
        prepare(&insecure_paths);
        fs::set_permissions(
            insecure_paths
                .state_lock()
                .parent()
                .expect("state directory"),
            fs::Permissions::from_mode(0o722),
        )
        .expect("insecure mode");
        assert!(WriterAuthority::acquire(&insecure_paths, OpenMode::Initialize).is_err());
    }

    #[test]
    fn authority_errors_and_debug_are_path_redacted() {
        let root = tempfile::tempdir().expect("root");
        let sensitive_root = root.path().join("secret-state-root");
        fs::create_dir(&sensitive_root).expect("sensitive root");
        let paths = paths(&sensitive_root, "redacted");
        let error = WriterAuthority::acquire(&paths, OpenMode::Initialize)
            .expect_err("missing state directory");
        let display = error.to_string();
        let debug = format!("{error:?}");
        let source = error.source().map(ToString::to_string).unwrap();
        for projection in [display, debug, source] {
            assert!(!projection.contains("secret-state-root"));
            assert!(!projection.contains("state.lock"));
            assert!(!projection.contains(root.path().to_string_lossy().as_ref()));
        }

        prepare(&paths);
        let mut authority = WriterAuthority::acquire(&paths, OpenMode::Initialize)
            .expect("authority")
            .expect("writer capability");
        assert_eq!(
            format!("{authority:?}"),
            "WriterAuthority { state: \"held\" }"
        );
        authority.release().expect("release");
        assert_eq!(
            format!("{authority:?}"),
            "WriterAuthority { state: \"released\" }"
        );
    }

    #[test]
    fn fixed_private_cause_inventory_is_path_free() {
        let causes = [
            WriterAuthorityCause::StateDirectoryUnavailable,
            WriterAuthorityCause::StateDirectoryInvalidType,
            WriterAuthorityCause::StateDirectoryWrongOwner,
            WriterAuthorityCause::StateDirectoryInsecurePermissions,
            WriterAuthorityCause::LockUnavailable,
            WriterAuthorityCause::LockInvalidType,
            WriterAuthorityCause::LockMultipleLinks,
            WriterAuthorityCause::LockWrongOwner,
            WriterAuthorityCause::Contended,
            WriterAuthorityCause::UnlockFailed,
        ];
        for cause in causes {
            let display = cause.to_string();
            assert!(display.is_ascii());
            assert!(!display.contains('/'));
            assert!(!display.contains(".sqlite"));
            assert!(!display.contains("state.lock"));
            assert!(require_authority_condition(true, cause).is_ok());
            assert_eq!(require_authority_condition(false, cause), Err(cause));
        }
    }
}
