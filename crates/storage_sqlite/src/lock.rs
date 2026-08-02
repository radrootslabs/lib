//! SQLite process and writer locking boundary.

use crate::{Error, OpenMode, Paths};
use fs2::FileExt;
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};

const WRITER_LOCK_FILE_NAME: &str = ".radroots-storage-writer.lock";

/// Lifetime guard for the one writable backend permitted for an owned store.
///
/// The persistent empty sidecar is never removed during normal operation.
/// Keeping one inode avoids an unlink/recreate race between competing clients;
/// dropping the file descriptor releases the operating-system advisory lock.
pub(crate) struct WriterLock {
    file: File,
    path: PathBuf,
}

impl WriterLock {
    /// Acquires the governed lock for writable modes without hidden waiting.
    /// Read-only clients deliberately acquire no advisory lock.
    pub(crate) fn acquire(paths: &Paths, mode: OpenMode) -> Result<Option<Self>, Error> {
        if !mode.is_writable() {
            return Ok(None);
        }
        let path = writer_lock_path(paths)?;
        let file = open_lock_file(&path)?;
        match FileExt::try_lock_exclusive(&file) {
            Ok(()) => Ok(Some(Self { file, path })),
            Err(source) if source.kind() == std::io::ErrorKind::WouldBlock => {
                Err(Error::WriterAlreadyActive { path })
            }
            Err(source) => Err(Error::WriterLockFailed { path, source }),
        }
    }

    /// Explicitly releases the writer lock for the later asynchronous close
    /// lifecycle. Dropping the guard remains a fail-safe release path.
    #[allow(dead_code)] // Used by the explicit close lifecycle in its ordered RCL checkpoint.
    pub(crate) fn release(self) -> Result<(), Error> {
        FileExt::unlock(&self.file).map_err(|source| Error::WriterUnlockFailed {
            path: self.path.clone(),
            source,
        })
    }

    #[cfg(test)]
    fn path(&self) -> &Path {
        &self.path
    }
}

fn writer_lock_path(paths: &Paths) -> Result<PathBuf, Error> {
    let parent = paths
        .runtime()
        .parent()
        .ok_or_else(|| Error::InvalidPath(paths.runtime().to_path_buf()))?;
    let canonical_parent = fs::canonicalize(parent).map_err(|source| Error::Inspect {
        path: parent.to_path_buf(),
        source,
    })?;
    Ok(canonical_parent.join(WRITER_LOCK_FILE_NAME))
}

fn open_lock_file(path: &Path) -> Result<File, Error> {
    match create_lock_file(path) {
        Ok(file) => validate_open_file(path, file),
        Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => {
            let metadata = fs::symlink_metadata(path).map_err(|source| Error::WriterLockOpen {
                path: path.to_path_buf(),
                source,
            })?;
            if metadata.file_type().is_symlink() {
                return Err(Error::SymlinkPath(path.to_path_buf()));
            }
            if !metadata.is_file() {
                return Err(Error::NotAFile(path.to_path_buf()));
            }
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .open(path)
                .map_err(|source| Error::WriterLockOpen {
                    path: path.to_path_buf(),
                    source,
                })?;
            validate_open_file(path, file)
        }
        Err(source) => Err(Error::WriterLockOpen {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn create_lock_file(path: &Path) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options.create_new(true).read(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

fn validate_open_file(path: &Path, file: File) -> Result<File, Error> {
    let metadata = file.metadata().map_err(|source| Error::WriterLockOpen {
        path: path.to_path_buf(),
        source,
    })?;
    if !metadata.is_file() {
        return Err(Error::NotAFile(path.to_path_buf()));
    }
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    const POLICY: &str = include_str!("../../../contracts/storage/writer_policy_v1.toml");

    #[derive(Deserialize)]
    struct Policy {
        schema_version: u32,
        lock_file_name: String,
        lock_scope: String,
        acquisition: String,
        sidecar_lifecycle: String,
        writable_modes: Vec<String>,
        read_only_lock: String,
        concurrent_readers: bool,
        reader_with_writer: bool,
        concurrent_writers: bool,
    }

    fn paths(directory: &Path) -> Paths {
        Paths::from_directory(directory).expect("owned paths")
    }

    #[test]
    fn implementation_matches_the_governed_multi_client_policy() {
        let policy = toml::from_str::<Policy>(POLICY).expect("writer policy");
        assert_eq!(policy.schema_version, 1);
        assert_eq!(policy.lock_file_name, WRITER_LOCK_FILE_NAME);
        assert_eq!(policy.lock_scope, "canonical_runtime_parent");
        assert_eq!(policy.acquisition, "non_blocking_exclusive_advisory");
        assert_eq!(policy.sidecar_lifecycle, "persistent_empty_file");
        assert_eq!(policy.writable_modes, ["read_write_existing", "create"]);
        assert_eq!(policy.read_only_lock, "none");
        assert!(policy.concurrent_readers);
        assert!(policy.reader_with_writer);
        assert!(!policy.concurrent_writers);
    }

    #[test]
    fn one_writer_excludes_other_clients_until_explicit_release() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let paths = paths(directory.path());
        let first = WriterLock::acquire(&paths, OpenMode::Create)
            .expect("first writer")
            .expect("writer guard");
        assert_eq!(first.path(), directory.path().join(WRITER_LOCK_FILE_NAME));
        assert!(first.path().is_file());
        assert!(matches!(
            WriterLock::acquire(&paths, OpenMode::ReadWriteExisting),
            Err(Error::WriterAlreadyActive { .. })
        ));

        first.release().expect("release first writer");
        let next = WriterLock::acquire(&paths, OpenMode::ReadWriteExisting)
            .expect("next writer")
            .expect("writer guard");
        drop(next);
        assert!(
            directory.path().join(WRITER_LOCK_FILE_NAME).is_file(),
            "the stable sidecar must not be unlinked"
        );
    }

    #[test]
    fn read_only_clients_take_no_lock_and_can_coexist_with_a_writer() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let paths = paths(directory.path());
        let writer = WriterLock::acquire(&paths, OpenMode::Create)
            .expect("writer")
            .expect("writer guard");
        assert!(
            WriterLock::acquire(&paths, OpenMode::ReadOnly)
                .expect("first reader")
                .is_none()
        );
        assert!(
            WriterLock::acquire(&paths, OpenMode::ReadOnly)
                .expect("second reader")
                .is_none()
        );
        drop(writer);
    }

    #[test]
    fn writer_lock_is_observed_by_a_distinct_process() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let paths = paths(directory.path());
        let writer = WriterLock::acquire(&paths, OpenMode::Create)
            .expect("writer")
            .expect("writer guard");
        let status = std::process::Command::new(std::env::current_exe().expect("test executable"))
            .arg("--exact")
            .arg("lock::tests::writer_lock_child_probe")
            .arg("--nocapture")
            .env("RADROOTS_WRITER_LOCK_CHILD_PATH", directory.path())
            .status()
            .expect("run lock child");
        assert!(status.success(), "child must observe writer contention");
        drop(writer);
    }

    #[test]
    fn writer_lock_child_probe() {
        let Some(directory) = std::env::var_os("RADROOTS_WRITER_LOCK_CHILD_PATH") else {
            return;
        };
        assert!(matches!(
            WriterLock::acquire(&paths(Path::new(&directory)), OpenMode::Create),
            Err(Error::WriterAlreadyActive { .. })
        ));
    }

    #[test]
    fn stores_in_distinct_canonical_directories_do_not_contend() {
        let first_directory = tempfile::tempdir().expect("first directory");
        let second_directory = tempfile::tempdir().expect("second directory");
        let first = WriterLock::acquire(&paths(first_directory.path()), OpenMode::Create)
            .expect("first writer")
            .expect("first guard");
        let second = WriterLock::acquire(&paths(second_directory.path()), OpenMode::Create)
            .expect("second writer")
            .expect("second guard");
        assert_ne!(first.path(), second.path());
    }

    #[cfg(unix)]
    #[test]
    fn symlink_lock_files_fail_closed() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().expect("temporary directory");
        let target = directory.path().join("unrelated");
        fs::write(&target, []).expect("target");
        symlink(&target, directory.path().join(WRITER_LOCK_FILE_NAME)).expect("lock symlink");
        assert!(matches!(
            WriterLock::acquire(&paths(directory.path()), OpenMode::Create),
            Err(Error::SymlinkPath(_))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn canonical_parent_aliases_share_one_writer_lock() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().expect("temporary root");
        let actual = root.path().join("actual");
        let alias = root.path().join("alias");
        fs::create_dir(&actual).expect("actual directory");
        symlink(&actual, &alias).expect("directory alias");
        let writer = WriterLock::acquire(&paths(&actual), OpenMode::Create)
            .expect("writer")
            .expect("writer guard");
        assert!(matches!(
            WriterLock::acquire(&paths(&alias), OpenMode::Create),
            Err(Error::WriterAlreadyActive { .. })
        ));
        drop(writer);
    }

    #[test]
    fn non_file_lock_paths_fail_closed() {
        let directory = tempfile::tempdir().expect("temporary directory");
        fs::create_dir(directory.path().join(WRITER_LOCK_FILE_NAME)).expect("lock directory");
        assert!(matches!(
            WriterLock::acquire(&paths(directory.path()), OpenMode::Create),
            Err(Error::NotAFile(_))
        ));
    }
}
