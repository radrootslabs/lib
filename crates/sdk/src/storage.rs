//! Canonical storage capability composition.

/// Backend-neutral storage status owned by `radroots_storage`.
pub type Status = radroots_storage::StorageStatus;

/// Backend-neutral integrity status owned by `radroots_storage`.
pub type IntegrityStatus = radroots_storage::status::IntegrityStatus;

/// Validated SQLite open configuration; this contains no connection or pool.
#[cfg(feature = "sqlite")]
pub type SqliteOptions = radroots_storage_sqlite::OpenOptions;

/// Explicit SQLite lifecycle mode.
#[cfg(feature = "sqlite")]
pub type SqliteOpenMode = radroots_storage_sqlite::OpenMode;

/// Validated SQLite-owned paths; this contains no backend handle.
#[cfg(feature = "sqlite")]
pub type SqlitePaths = radroots_storage_sqlite::Paths;
