//! Owner-only files for the separate native execution namespace.

use super::WorkspaceError;
use fs2::FileExt;
use rustix::fs::{Mode, OFlags, open, openat};
use std::{
    fs::{File, Metadata},
    os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt},
    path::{Component, Path, PathBuf},
};

const DATABASE: &str = "execution_v1.sqlite";
const LOCK: &str = ".execution_v1.lock";

/// Retained identities and writer authority. The persistent lock inode is never
/// unlinked on close, so another opener cannot lock a replacement inode.
pub(super) struct Namespace {
    root: PathBuf,
    directory: File,
    lock: File,
    database: File,
}
impl Namespace {
    pub(super) fn acquire(root: &Path, create: bool) -> Result<Self, WorkspaceError> {
        validate_absolute(root)?;
        if create {
            std::fs::DirBuilder::new()
                .mode(0o700)
                .create(root)
                .map_err(|_| WorkspaceError::NamespaceUnavailable)?;
        }
        validate_ancestors(root)?;
        let directory: File = open(
            root,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| WorkspaceError::NamespaceUnavailable)?
        .into();
        let metadata = directory
            .metadata()
            .map_err(|_| WorkspaceError::NamespaceUnavailable)?;
        validate_directory(&metadata)?;
        let flags = OFlags::RDWR
            | OFlags::NOFOLLOW
            | OFlags::CLOEXEC
            | if create {
                OFlags::CREATE | OFlags::EXCL
            } else {
                OFlags::empty()
            };
        let lock: File = openat(&directory, LOCK, flags, Mode::RUSR | Mode::WUSR)
            .map_err(|_| WorkspaceError::NamespaceUnavailable)?
            .into();
        validate_file(&lock)?;
        if lock
            .metadata()
            .map_err(|_| WorkspaceError::NamespaceUnavailable)?
            .len()
            != 0
        {
            return Err(WorkspaceError::RecoveryRequired);
        }
        FileExt::try_lock_exclusive(&lock).map_err(|error| {
            if error.kind() == std::io::ErrorKind::WouldBlock {
                WorkspaceError::WriterActive
            } else {
                WorkspaceError::NamespaceUnavailable
            }
        })?;
        let database: File = openat(&directory, DATABASE, flags, Mode::RUSR | Mode::WUSR)
            .map_err(|_| WorkspaceError::NamespaceUnavailable)?
            .into();
        validate_file(&database)?;
        if !create
            && database
                .metadata()
                .map_err(|_| WorkspaceError::NamespaceUnavailable)?
                .len()
                == 0
        {
            return Err(WorkspaceError::RecoveryRequired);
        }
        if create {
            database
                .sync_all()
                .map_err(|_| WorkspaceError::NamespaceUnavailable)?;
            lock.sync_all()
                .map_err(|_| WorkspaceError::NamespaceUnavailable)?;
            directory
                .sync_all()
                .map_err(|_| WorkspaceError::NamespaceUnavailable)?;
        }
        let namespace = Self {
            root: root.to_path_buf(),
            directory,
            lock,
            database,
        };
        namespace.validate()?;
        Ok(namespace)
    }
    pub(super) fn database_path(&self) -> PathBuf {
        self.root.join(DATABASE)
    }
    pub(super) fn validate(&self) -> Result<(), WorkspaceError> {
        validate_ancestors(&self.root)?;
        let entries =
            std::fs::read_dir(&self.root).map_err(|_| WorkspaceError::NamespaceUnavailable)?;
        for (index, entry) in entries.enumerate() {
            let entry = entry.map_err(|_| WorkspaceError::NamespaceUnavailable)?;
            if index >= 5
                || !matches!(
                    entry.file_name().to_str(),
                    Some(
                        "execution_v1.sqlite"
                            | ".execution_v1.lock"
                            | "execution_v1.sqlite-wal"
                            | "execution_v1.sqlite-shm"
                            | "execution_v1.sqlite-journal"
                    )
                )
            {
                return Err(WorkspaceError::RecoveryRequired);
            }
        }
        let root = std::fs::symlink_metadata(&self.root)
            .map_err(|_| WorkspaceError::NamespaceUnavailable)?;
        let retained = self
            .directory
            .metadata()
            .map_err(|_| WorkspaceError::NamespaceUnavailable)?;
        validate_directory(&root)?;
        if !root.is_dir() || root.file_type().is_symlink() || !same(&root, &retained) {
            return Err(WorkspaceError::NamespaceUnavailable);
        }
        for (name, file) in [(DATABASE, &self.database), (LOCK, &self.lock)] {
            validate_file(file)?;
            let current = std::fs::symlink_metadata(self.root.join(name))
                .map_err(|_| WorkspaceError::NamespaceUnavailable)?;
            let retained = file
                .metadata()
                .map_err(|_| WorkspaceError::NamespaceUnavailable)?;
            if current.file_type().is_symlink() || !same(&current, &retained) {
                return Err(WorkspaceError::NamespaceUnavailable);
            }
        }
        for suffix in ["-wal", "-shm", "-journal"] {
            match std::fs::symlink_metadata(self.root.join(format!("{DATABASE}{suffix}"))) {
                Ok(metadata) => validate_metadata(&metadata)?,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => return Err(WorkspaceError::NamespaceUnavailable),
            }
        }
        Ok(())
    }
}
fn validate_absolute(root: &Path) -> Result<(), WorkspaceError> {
    if !root.is_absolute()
        || root
            .components()
            .any(|part| matches!(part, Component::CurDir | Component::ParentDir))
    {
        return Err(WorkspaceError::NamespaceUnavailable);
    }
    Ok(())
}
fn validate_ancestors(root: &Path) -> Result<(), WorkspaceError> {
    let mut current = PathBuf::new();
    for part in root.components() {
        current.push(part.as_os_str());
        let metadata = std::fs::symlink_metadata(&current)
            .map_err(|_| WorkspaceError::NamespaceUnavailable)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(WorkspaceError::NamespaceUnavailable);
        }
        // A root-owned sticky ancestor is an ordinary system temporary root;
        // all other mutable ancestors could allow another user to replace it.
        let mode = metadata.permissions().mode();
        let sticky_system = metadata.uid() == 0 && mode & 0o1000 != 0;
        if mode & 0o022 != 0 && !sticky_system {
            return Err(WorkspaceError::NamespaceUnavailable);
        }
    }
    Ok(())
}
fn same(a: &Metadata, b: &Metadata) -> bool {
    a.dev() == b.dev() && a.ino() == b.ino() && a.uid() == b.uid()
}
fn validate_file(file: &File) -> Result<(), WorkspaceError> {
    validate_metadata(
        &file
            .metadata()
            .map_err(|_| WorkspaceError::NamespaceUnavailable)?,
    )
}
fn validate_directory(metadata: &Metadata) -> Result<(), WorkspaceError> {
    if !metadata.is_dir()
        || metadata.uid() != rustix::process::geteuid().as_raw()
        || metadata.permissions().mode() & 0o077 != 0
    {
        return Err(WorkspaceError::NamespaceUnavailable);
    }
    Ok(())
}
fn validate_metadata(metadata: &Metadata) -> Result<(), WorkspaceError> {
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.nlink() != 1
        || metadata.uid() != rustix::process::geteuid().as_raw()
        || metadata.permissions().mode() & 0o7777 != 0o600
    {
        return Err(WorkspaceError::NamespaceUnavailable);
    }
    Ok(())
}
