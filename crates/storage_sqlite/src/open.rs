//! SQLite storage lifecycle modes and owned paths.

use std::error::Error as StdError;
use std::fmt;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

const RUNTIME_DATABASE_NAME: &str = "runtime.sqlite";
const PRIVATE_DATABASE_NAME: &str = "private.sqlite";

/// Explicit behavior for opening owned SQLite files.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum OpenMode {
    /// Open both existing files without permitting mutation or migration.
    ReadOnly,
    /// Open both existing files with write and migration authority.
    ReadWriteExisting,
    /// Open or create the two owned files with write and migration authority.
    Create,
}

impl OpenMode {
    /// Returns whether the mode may mutate owned database files.
    pub fn is_writable(self) -> bool {
        matches!(self, Self::ReadWriteExisting | Self::Create)
    }

    /// Returns whether missing owned files may be created.
    pub fn may_create(self) -> bool {
        matches!(self, Self::Create)
    }
}

/// The complete SQLite file set owned by one Radroots storage backend.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Paths {
    runtime: PathBuf,
    private: PathBuf,
}

impl Paths {
    /// Derives the governed file names from an absolute owner directory.
    pub fn from_directory(directory: impl AsRef<Path>) -> Result<Self, Error> {
        let directory = directory.as_ref();
        validate_absolute_normal_path(directory)?;
        Self::from_files(
            directory.join(RUNTIME_DATABASE_NAME),
            directory.join(PRIVATE_DATABASE_NAME),
        )
    }

    /// Validates explicit runtime and private file paths.
    pub fn from_files(
        runtime: impl Into<PathBuf>,
        private: impl Into<PathBuf>,
    ) -> Result<Self, Error> {
        let runtime = runtime.into();
        let private = private.into();
        validate_owned_file_path(&runtime, RUNTIME_DATABASE_NAME)?;
        validate_owned_file_path(&private, PRIVATE_DATABASE_NAME)?;
        if runtime == private {
            return Err(Error::PathsOverlap(runtime));
        }
        Ok(Self { runtime, private })
    }

    /// Returns the canonical event, journal, outbox, and projection database.
    pub fn runtime(&self) -> &Path {
        &self.runtime
    }

    /// Returns the encrypted private-artifact database.
    pub fn private(&self) -> &Path {
        &self.private
    }

    pub(crate) fn validate_filesystem(&self, mode: OpenMode) -> Result<(), Error> {
        for path in [&self.runtime, &self.private] {
            validate_parent(path)?;
            match std::fs::symlink_metadata(path) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    return Err(Error::SymlinkPath(path.clone()));
                }
                Ok(metadata) if !metadata.is_file() => {
                    return Err(Error::NotAFile(path.clone()));
                }
                Ok(_) => {}
                Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
                    if !mode.may_create() {
                        return Err(Error::MissingFile(path.clone()));
                    }
                }
                Err(source) => {
                    return Err(Error::Inspect {
                        path: path.clone(),
                        source,
                    });
                }
            }
        }
        Ok(())
    }
}

fn validate_owned_file_path(path: &Path, expected_name: &'static str) -> Result<(), Error> {
    validate_absolute_normal_path(path)?;
    if path.file_name().and_then(|name| name.to_str()) != Some(expected_name) {
        return Err(Error::UnexpectedFileName {
            path: path.to_path_buf(),
            expected: expected_name,
        });
    }
    Ok(())
}

fn validate_absolute_normal_path(path: &Path) -> Result<(), Error> {
    if !path.is_absolute()
        || path
            .components()
            .any(|part| matches!(part, Component::CurDir | Component::ParentDir))
    {
        return Err(Error::InvalidPath(path.to_path_buf()));
    }
    Ok(())
}

fn validate_parent(path: &Path) -> Result<(), Error> {
    let parent = path
        .parent()
        .ok_or_else(|| Error::InvalidPath(path.to_path_buf()))?;
    match std::fs::metadata(parent) {
        Ok(metadata) if metadata.is_dir() => Ok(()),
        Ok(_) => Err(Error::ParentNotDirectory(parent.to_path_buf())),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
            Err(Error::MissingParent(parent.to_path_buf()))
        }
        Err(source) => Err(Error::Inspect {
            path: parent.to_path_buf(),
            source,
        }),
    }
}

/// Configuration or filesystem failure detected before SQLite is opened.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    InvalidPath(PathBuf),
    UnexpectedFileName {
        path: PathBuf,
        expected: &'static str,
    },
    PathsOverlap(PathBuf),
    MissingParent(PathBuf),
    ParentNotDirectory(PathBuf),
    MissingFile(PathBuf),
    SymlinkPath(PathBuf),
    NotAFile(PathBuf),
    Inspect {
        path: PathBuf,
        source: std::io::Error,
    },
    InvalidBusyTimeout {
        minimum: Duration,
        maximum: Duration,
        actual: Duration,
    },
    WriterLockOpen {
        path: PathBuf,
        source: std::io::Error,
    },
    WriterAlreadyActive {
        path: PathBuf,
    },
    WriterLockFailed {
        path: PathBuf,
        source: std::io::Error,
    },
    WriterUnlockFailed {
        path: PathBuf,
        source: std::io::Error,
    },
    SchemaMetadataUnavailable {
        database: &'static str,
    },
    SchemaIdentityMismatch {
        database: &'static str,
        expected: u32,
        actual: u32,
    },
    SchemaTooOld {
        database: &'static str,
        minimum: u32,
        actual: u32,
    },
    SchemaTooNew {
        database: &'static str,
        supported: u32,
        actual: u32,
    },
    SchemaMigrationRequired {
        database: &'static str,
        current: u32,
        actual: u32,
    },
    UnrecognizedSchema {
        database: &'static str,
    },
    SchemaCatalogMismatch {
        database: &'static str,
        version: u32,
    },
    SchemaMigrationFailed {
        database: &'static str,
        target_version: u32,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPath(path) => {
                write!(formatter, "invalid owned SQLite path: {}", path.display())
            }
            Self::UnexpectedFileName { path, expected } => write!(
                formatter,
                "owned SQLite path {} must use file name {expected}",
                path.display()
            ),
            Self::PathsOverlap(path) => {
                write!(formatter, "owned SQLite paths overlap: {}", path.display())
            }
            Self::MissingParent(path) => write!(
                formatter,
                "owned SQLite parent is missing: {}",
                path.display()
            ),
            Self::ParentNotDirectory(path) => write!(
                formatter,
                "owned SQLite parent is not a directory: {}",
                path.display()
            ),
            Self::MissingFile(path) => write!(
                formatter,
                "owned SQLite file is missing: {}",
                path.display()
            ),
            Self::SymlinkPath(path) => write!(
                formatter,
                "owned SQLite path cannot be a symlink: {}",
                path.display()
            ),
            Self::NotAFile(path) => write!(
                formatter,
                "owned SQLite path is not a file: {}",
                path.display()
            ),
            Self::Inspect { path, .. } => write!(
                formatter,
                "failed to inspect owned SQLite path: {}",
                path.display()
            ),
            Self::InvalidBusyTimeout {
                minimum,
                maximum,
                actual,
            } => write!(
                formatter,
                "SQLite busy timeout {actual:?} must be within {minimum:?}..={maximum:?}"
            ),
            Self::WriterLockOpen { path, .. } => write!(
                formatter,
                "failed to open SQLite writer lock: {}",
                path.display()
            ),
            Self::WriterAlreadyActive { path } => write!(
                formatter,
                "another writable SQLite storage client holds: {}",
                path.display()
            ),
            Self::WriterLockFailed { path, .. } => write!(
                formatter,
                "failed to acquire SQLite writer lock: {}",
                path.display()
            ),
            Self::WriterUnlockFailed { path, .. } => write!(
                formatter,
                "failed to release SQLite writer lock: {}",
                path.display()
            ),
            Self::SchemaMetadataUnavailable { database } => {
                write!(formatter, "failed to inspect {database} schema metadata")
            }
            Self::SchemaIdentityMismatch {
                database,
                expected,
                actual,
            } => write!(
                formatter,
                "{database} application id {actual} does not match required id {expected}"
            ),
            Self::SchemaTooOld {
                database,
                minimum,
                actual,
            } => write!(
                formatter,
                "{database} schema version {actual} is older than supported version {minimum}"
            ),
            Self::SchemaTooNew {
                database,
                supported,
                actual,
            } => write!(
                formatter,
                "{database} schema version {actual} is newer than supported version {supported}"
            ),
            Self::SchemaMigrationRequired {
                database,
                current,
                actual,
            } => write!(
                formatter,
                "{database} schema version {actual} requires writable migration to {current}"
            ),
            Self::UnrecognizedSchema { database } => {
                write!(
                    formatter,
                    "{database} has an unrecognized unversioned schema"
                )
            }
            Self::SchemaCatalogMismatch { database, version } => write!(
                formatter,
                "{database} object catalog does not match schema version {version}"
            ),
            Self::SchemaMigrationFailed {
                database,
                target_version,
            } => write!(
                formatter,
                "{database} migration to schema version {target_version} failed"
            ),
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Inspect { source, .. }
            | Self::WriterLockOpen { source, .. }
            | Self::WriterLockFailed { source, .. }
            | Self::WriterUnlockFailed { source, .. } => Some(source),
            _ => None,
        }
    }
}
