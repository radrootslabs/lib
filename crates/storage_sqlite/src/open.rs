//! SQLite storage lifecycle modes and owned paths.

use std::error::Error as StdError;
use std::fmt;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use crate::{OpenOptions, event::SqliteStorage, lock::WriterLock, migration};
use radroots_storage::event::SourceGeneration;
use sqlx::{
    ConnectOptions, Connection, Row, SqliteConnection, SqlitePool,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
};

const RUNTIME_DATABASE_NAME: &str = "runtime.sqlite";
const PRIVATE_DATABASE_NAME: &str = "private.sqlite";
const MAX_CONNECTIONS_PER_DATABASE: u32 = 4;

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

/// Stable, secret-safe SQLite lifecycle or filesystem failure.
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
    InvalidSourceGenerationTimestamp {
        actual: u64,
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
    DatabaseOpenFailed {
        database: &'static str,
    },
    DatabaseCloseFailed {
        database: &'static str,
    },
    ConnectionPolicyMismatch {
        database: &'static str,
    },
    SourceGenerationRequired,
    SourceGenerationUnavailable,
    SourceGenerationMismatch,
    CorruptSourceGeneration,
    InvalidBackupRoot(PathBuf),
    BackupRootRequired,
    BackupBundleAlreadyExists(PathBuf),
    BackupBackendUnavailable,
    UnsupportedBackupVersion,
    BackupCaptureFailed {
        member: &'static str,
    },
    BackupBundleMissing(PathBuf),
    BackupVerificationFailed {
        member: &'static str,
    },
    BackupUnexpectedEntry(PathBuf),
    RestoreRequiresWritableStorage,
    RestoreStagingAlreadyExists(PathBuf),
    RestoreStagingFailed {
        member: &'static str,
    },
    RestoreMarkerCorrupt(PathBuf),
    RestoreRecoveryConflict(PathBuf),
    RestoreReplacementFailed {
        member: &'static str,
    },
    RestoreFilesystem {
        operation: &'static str,
        source: std::io::Error,
    },
    InvalidLegacyImportPlan,
    InvalidLegacySource(PathBuf),
    LegacyImportBackupAlreadyExists(PathBuf),
    LegacyImportBackupFailed {
        source_kind: &'static str,
    },
    LegacyImportSourceInvalid {
        source_kind: &'static str,
    },
    LegacyImportEvidenceInvalid,
    LegacyImportTargetMismatch,
    LegacyImportMigrationHistoryInvalid,
    InvalidLegacyImportJournal,
    LegacyImportConflict,
    LegacyImportJournalFailed,
    InvalidLegacyImportStageRequest,
    LegacyImportStagingFailed,
    LegacyImportRowInvalid {
        source_kind: &'static str,
        legacy_sequence: i64,
    },
    UnsupportedLegacySchema {
        source_kind: &'static str,
        user_version: i64,
        catalog_sha256: String,
    },
    LegacyImportFilesystem {
        operation: &'static str,
        source: std::io::Error,
    },
    BackupFilesystem {
        operation: &'static str,
        source: std::io::Error,
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
            Self::InvalidSourceGenerationTimestamp { actual } => write!(
                formatter,
                "source generation creation time {actual} must fit a positive SQLite integer"
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
            Self::DatabaseOpenFailed { database } => {
                write!(
                    formatter,
                    "failed to open governed SQLite database {database}"
                )
            }
            Self::DatabaseCloseFailed { database } => write!(
                formatter,
                "failed to close migration connection for {database}"
            ),
            Self::ConnectionPolicyMismatch { database } => write!(
                formatter,
                "{database} does not satisfy the governed SQLite connection policy"
            ),
            Self::SourceGenerationRequired => formatter.write_str(
                "a fresh writable SQLite store requires a host-supplied source generation",
            ),
            Self::SourceGenerationUnavailable => {
                formatter.write_str("SQLite storage has no active source generation")
            }
            Self::SourceGenerationMismatch => formatter
                .write_str("host-supplied source generation does not match durable storage"),
            Self::CorruptSourceGeneration => {
                formatter.write_str("SQLite storage source generation is corrupt")
            }
            Self::InvalidBackupRoot(path) => {
                write!(formatter, "invalid SQLite backup root: {}", path.display())
            }
            Self::BackupRootRequired => {
                formatter.write_str("SQLite backup requires a configured host-owned root")
            }
            Self::BackupBundleAlreadyExists(path) => write!(
                formatter,
                "SQLite backup bundle path already exists: {}",
                path.display()
            ),
            Self::BackupBackendUnavailable => {
                formatter.write_str("SQLite backup backend is unavailable")
            }
            Self::UnsupportedBackupVersion => {
                formatter.write_str("SQLite backup format version is unsupported")
            }
            Self::BackupCaptureFailed { member } => {
                write!(formatter, "failed to capture SQLite backup member {member}")
            }
            Self::BackupBundleMissing(path) => {
                write!(
                    formatter,
                    "SQLite backup bundle is missing: {}",
                    path.display()
                )
            }
            Self::BackupVerificationFailed { member } => {
                write!(
                    formatter,
                    "SQLite backup member failed verification: {member}"
                )
            }
            Self::BackupUnexpectedEntry(path) => write!(
                formatter,
                "SQLite backup bundle contains an unexpected entry: {}",
                path.display()
            ),
            Self::RestoreRequiresWritableStorage => {
                formatter.write_str("SQLite restore requires writable storage authority")
            }
            Self::RestoreStagingAlreadyExists(path) => write!(
                formatter,
                "SQLite restore staging path already exists: {}",
                path.display()
            ),
            Self::RestoreStagingFailed { member } => {
                write!(formatter, "failed to stage SQLite restore member {member}")
            }
            Self::RestoreMarkerCorrupt(path) => write!(
                formatter,
                "SQLite restore interruption marker is corrupt: {}",
                path.display()
            ),
            Self::RestoreRecoveryConflict(path) => write!(
                formatter,
                "SQLite restore recovery state conflicts at: {}",
                path.display()
            ),
            Self::RestoreReplacementFailed { member } => {
                write!(
                    formatter,
                    "failed to replace SQLite restore member {member}"
                )
            }
            Self::RestoreFilesystem { operation, .. } => {
                write!(
                    formatter,
                    "SQLite restore filesystem operation failed: {operation}"
                )
            }
            Self::InvalidLegacyImportPlan => {
                formatter.write_str("SQLite legacy import plan is invalid")
            }
            Self::InvalidLegacySource(path) => write!(
                formatter,
                "SQLite legacy import source is invalid: {}",
                path.display()
            ),
            Self::LegacyImportBackupAlreadyExists(path) => write!(
                formatter,
                "SQLite legacy import backup already exists: {}",
                path.display()
            ),
            Self::LegacyImportBackupFailed { source_kind } => write!(
                formatter,
                "failed to back up SQLite legacy {source_kind} source"
            ),
            Self::LegacyImportSourceInvalid { source_kind } => write!(
                formatter,
                "SQLite legacy {source_kind} source failed integrity validation"
            ),
            Self::LegacyImportEvidenceInvalid => {
                formatter.write_str("SQLite legacy import evidence is invalid")
            }
            Self::LegacyImportTargetMismatch => {
                formatter.write_str("SQLite legacy import target generation does not match")
            }
            Self::LegacyImportMigrationHistoryInvalid => {
                formatter.write_str("SQLite legacy event-store migration history is invalid")
            }
            Self::InvalidLegacyImportJournal => {
                formatter.write_str("SQLite legacy import journal is invalid")
            }
            Self::LegacyImportConflict => {
                formatter.write_str("SQLite legacy import conflicts with durable state")
            }
            Self::LegacyImportJournalFailed => {
                formatter.write_str("SQLite legacy import journal operation failed")
            }
            Self::InvalidLegacyImportStageRequest => {
                formatter.write_str("SQLite legacy import staging request is invalid")
            }
            Self::LegacyImportStagingFailed => {
                formatter.write_str("SQLite legacy import staging operation failed")
            }
            Self::LegacyImportRowInvalid {
                source_kind,
                legacy_sequence,
            } => write!(
                formatter,
                "SQLite legacy {source_kind} row {legacy_sequence} is invalid"
            ),
            Self::UnsupportedLegacySchema {
                source_kind,
                user_version,
                catalog_sha256,
            } => write!(
                formatter,
                "unsupported SQLite legacy {source_kind} schema at user_version {user_version} with catalog SHA-256 {catalog_sha256}"
            ),
            Self::LegacyImportFilesystem { operation, .. } => write!(
                formatter,
                "SQLite legacy import filesystem operation failed: {operation}"
            ),
            Self::BackupFilesystem { operation, .. } => {
                write!(
                    formatter,
                    "SQLite backup filesystem operation failed: {operation}"
                )
            }
        }
    }
}

impl SqliteStorage {
    /// Opens both governed databases, applying only authorized forward
    /// migrations and retaining the writer guard for the backend lifetime.
    pub async fn open(options: OpenOptions) -> Result<Self, Error> {
        let writer_lock = WriterLock::acquire(options.paths(), options.mode())?;
        crate::backup::recover_interrupted_restore(options.paths(), options.mode()).await?;
        options.validate_filesystem()?;
        let runtime_exists =
            options
                .paths()
                .runtime()
                .try_exists()
                .map_err(|source| Error::Inspect {
                    path: options.paths().runtime().to_path_buf(),
                    source,
                })?;
        if options.mode().may_create() && !runtime_exists && options.source_generation().is_none() {
            return Err(Error::SourceGenerationRequired);
        }
        options.validate_filesystem()?;

        let runtime_options = connect_options(
            options.paths().runtime(),
            options.mode(),
            options.busy_timeout(),
        );
        let private_options = connect_options(
            options.paths().private(),
            options.mode(),
            options.busy_timeout(),
        );
        let mut runtime_connection =
            connect(runtime_options.clone(), RUNTIME_DATABASE_NAME).await?;
        let mut private_connection =
            connect(private_options.clone(), PRIVATE_DATABASE_NAME).await?;

        migration::migrate_runtime(&mut runtime_connection, options.mode()).await?;
        migration::migrate_private(&mut private_connection, options.mode()).await?;
        let generation = active_source_generation(
            &mut runtime_connection,
            options.mode(),
            options.source_generation_bootstrap(),
        )
        .await?;
        verify_connection(
            &mut runtime_connection,
            RUNTIME_DATABASE_NAME,
            options.busy_timeout(),
        )
        .await?;
        verify_connection(
            &mut private_connection,
            PRIVATE_DATABASE_NAME,
            options.busy_timeout(),
        )
        .await?;
        runtime_connection
            .close()
            .await
            .map_err(|_| Error::DatabaseCloseFailed {
                database: RUNTIME_DATABASE_NAME,
            })?;
        private_connection
            .close()
            .await
            .map_err(|_| Error::DatabaseCloseFailed {
                database: PRIVATE_DATABASE_NAME,
            })?;

        let runtime_pool = pool(runtime_options, RUNTIME_DATABASE_NAME).await?;
        let private_pool = pool(private_options, PRIVATE_DATABASE_NAME).await?;
        verify_pool(&runtime_pool, RUNTIME_DATABASE_NAME, options.busy_timeout()).await?;
        verify_pool(&private_pool, PRIVATE_DATABASE_NAME, options.busy_timeout()).await?;

        Ok(Self::from_opened(
            runtime_pool,
            private_pool,
            generation,
            &options,
            writer_lock,
        ))
    }
}

fn connect_options(path: &Path, mode: OpenMode, busy_timeout: Duration) -> SqliteConnectOptions {
    let mut options = SqliteConnectOptions::new()
        .filename(path)
        .read_only(!mode.is_writable())
        .create_if_missing(mode.may_create())
        .foreign_keys(true)
        .busy_timeout(busy_timeout)
        .synchronous(SqliteSynchronous::Full)
        .disable_statement_logging();
    if mode.is_writable() {
        options = options.journal_mode(SqliteJournalMode::Wal);
    }
    options
}

async fn connect(
    options: SqliteConnectOptions,
    database: &'static str,
) -> Result<SqliteConnection, Error> {
    SqliteConnection::connect_with(&options)
        .await
        .map_err(|_| Error::DatabaseOpenFailed { database })
}

async fn pool(options: SqliteConnectOptions, database: &'static str) -> Result<SqlitePool, Error> {
    SqlitePoolOptions::new()
        .max_connections(MAX_CONNECTIONS_PER_DATABASE)
        .min_connections(1)
        .connect_with(options)
        .await
        .map_err(|_| Error::DatabaseOpenFailed { database })
}

async fn active_source_generation(
    connection: &mut SqliteConnection,
    mode: OpenMode,
    expected: Option<(SourceGeneration, u64)>,
) -> Result<SourceGeneration, Error> {
    if !mode.is_writable() {
        let rows = active_generation_rows(connection).await?;
        return existing_source_generation(rows.as_slice(), expected);
    }
    let mut transaction = connection
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|_| Error::SourceGenerationUnavailable)?;
    let rows = active_generation_rows(&mut transaction).await?;
    let generation = match rows.as_slice() {
        [] => {
            let (generation, created_at) = expected.ok_or(Error::SourceGenerationRequired)?;
            sqlx::query(
                "INSERT INTO radroots_runtime_source_generations (
                   generation, sequence_head, state, created_at_unix_ms
                 ) VALUES (?, 0, 'active', ?)",
            )
            .bind(generation.as_bytes().as_slice())
            .bind(i64::try_from(created_at).map_err(|_| Error::CorruptSourceGeneration)?)
            .execute(&mut *transaction)
            .await
            .map_err(|_| Error::SourceGenerationUnavailable)?;
            generation
        }
        [_] => existing_source_generation(rows.as_slice(), expected)?,
        _ => return Err(Error::CorruptSourceGeneration),
    };
    transaction
        .commit()
        .await
        .map_err(|_| Error::SourceGenerationUnavailable)?;
    Ok(generation)
}

async fn active_generation_rows(
    connection: &mut SqliteConnection,
) -> Result<Vec<sqlx::sqlite::SqliteRow>, Error> {
    sqlx::query(
        "SELECT generation, created_at_unix_ms
         FROM radroots_runtime_source_generations
         WHERE state = 'active' ORDER BY generation",
    )
    .fetch_all(connection)
    .await
    .map_err(|_| Error::SourceGenerationUnavailable)
}

fn existing_source_generation(
    rows: &[sqlx::sqlite::SqliteRow],
    expected: Option<(SourceGeneration, u64)>,
) -> Result<SourceGeneration, Error> {
    let [row] = rows else {
        return if rows.is_empty() {
            Err(Error::SourceGenerationUnavailable)
        } else {
            Err(Error::CorruptSourceGeneration)
        };
    };
    let durable = decode_source_generation(row)?;
    let created_at = u64::try_from(
        row.try_get::<i64, _>("created_at_unix_ms")
            .map_err(|_| Error::CorruptSourceGeneration)?,
    )
    .map_err(|_| Error::CorruptSourceGeneration)?;
    if expected.is_some_and(|candidate| candidate != (durable, created_at)) {
        Err(Error::SourceGenerationMismatch)
    } else {
        Ok(durable)
    }
}

fn decode_source_generation(row: &sqlx::sqlite::SqliteRow) -> Result<SourceGeneration, Error> {
    SourceGeneration::new(
        row.try_get::<Vec<u8>, _>("generation")
            .map_err(|_| Error::CorruptSourceGeneration)?
            .try_into()
            .map_err(|_| Error::CorruptSourceGeneration)?,
    )
    .map_err(|_| Error::CorruptSourceGeneration)
}

async fn verify_pool(
    pool: &SqlitePool,
    database: &'static str,
    busy_timeout: Duration,
) -> Result<(), Error> {
    let mut connection = pool
        .acquire()
        .await
        .map_err(|_| Error::DatabaseOpenFailed { database })?;
    verify_connection(&mut connection, database, busy_timeout).await
}

async fn verify_connection(
    connection: &mut SqliteConnection,
    database: &'static str,
    busy_timeout: Duration,
) -> Result<(), Error> {
    let foreign_keys = sqlx::query_scalar::<_, i64>("PRAGMA foreign_keys")
        .fetch_one(&mut *connection)
        .await
        .map_err(|_| Error::ConnectionPolicyMismatch { database })?;
    let journal_mode = sqlx::query_scalar::<_, String>("PRAGMA journal_mode")
        .fetch_one(&mut *connection)
        .await
        .map_err(|_| Error::ConnectionPolicyMismatch { database })?;
    let configured_busy_timeout = sqlx::query_scalar::<_, i64>("PRAGMA busy_timeout")
        .fetch_one(&mut *connection)
        .await
        .map_err(|_| Error::ConnectionPolicyMismatch { database })?;
    let synchronous = sqlx::query_scalar::<_, i64>("PRAGMA synchronous")
        .fetch_one(&mut *connection)
        .await
        .map_err(|_| Error::ConnectionPolicyMismatch { database })?;
    let expected_busy_timeout = i64::try_from(busy_timeout.as_millis())
        .map_err(|_| Error::ConnectionPolicyMismatch { database })?;
    if foreign_keys == 1
        && journal_mode.eq_ignore_ascii_case("wal")
        && configured_busy_timeout == expected_busy_timeout
        && synchronous == 2
    {
        Ok(())
    } else {
        Err(Error::ConnectionPolicyMismatch { database })
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Inspect { source, .. }
            | Self::WriterLockOpen { source, .. }
            | Self::WriterLockFailed { source, .. }
            | Self::WriterUnlockFailed { source, .. }
            | Self::RestoreFilesystem { source, .. }
            | Self::LegacyImportFilesystem { source, .. }
            | Self::BackupFilesystem { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod policy_tests {
    use super::*;
    use serde::Deserialize;

    const POLICY: &str = include_str!("../../../contracts/storage/connection_policy_v1.toml");

    #[derive(Deserialize)]
    struct Policy {
        schema_version: u32,
        databases: Vec<String>,
        max_connections_per_database: u32,
        foreign_keys: bool,
        journal_mode: String,
        synchronous: String,
        busy_timeout_min_ms: u64,
        busy_timeout_default_ms: u64,
        busy_timeout_max_ms: u64,
        fresh_source_generation: String,
        read_only_migrations: bool,
        raw_handles_public: bool,
    }

    #[test]
    fn implementation_matches_the_governed_connection_policy() {
        let policy = toml::from_str::<Policy>(POLICY).expect("connection policy");
        assert_eq!(policy.schema_version, 1);
        assert_eq!(
            policy.databases,
            [RUNTIME_DATABASE_NAME, PRIVATE_DATABASE_NAME]
        );
        assert_eq!(
            policy.max_connections_per_database,
            MAX_CONNECTIONS_PER_DATABASE
        );
        assert!(policy.foreign_keys);
        assert_eq!(policy.journal_mode, "wal");
        assert_eq!(policy.synchronous, "full");
        assert_eq!(policy.busy_timeout_min_ms, 1);
        assert_eq!(policy.busy_timeout_default_ms, 5_000);
        assert_eq!(policy.busy_timeout_max_ms, 60_000);
        assert_eq!(
            policy.fresh_source_generation,
            "host_supplied_entropy_and_timestamp"
        );
        assert!(!policy.read_only_migrations);
        assert!(!policy.raw_handles_public);
    }
}
