use std::collections::BTreeSet;

const MANIFEST: &str = include_str!("../Cargo.toml");
const ROOT: &str = include_str!("../src/lib.rs");
const AUTHORITY_SOURCE: &str = include_str!("../src/authority.rs");
const CONFIG_SOURCE: &str = include_str!("../src/config.rs");
const ERROR_SOURCE: &str = include_str!("../src/error.rs");
const INITIALIZE_SOURCE: &str = include_str!("../src/initialize.rs");
const METADATA_SOURCE: &str = include_str!("../src/metadata.rs");
const MIGRATION_SOURCE: &str = include_str!("../src/migration.rs");
const OPEN_SOURCE: &str = include_str!("../src/open.rs");
const STATUS_SOURCE: &str = include_str!("../src/status.rs");

#[test]
fn service_sqlite_is_unpublished_lint_governed_and_dependency_bounded() {
    for required in [
        "name = \"radroots_service_sqlite\"",
        "publish = false",
        "version = \"0.1.0-alpha\"",
        "[lints]\nworkspace = true",
    ] {
        assert!(
            MANIFEST.contains(required),
            "manifest is missing `{required}`"
        );
    }

    assert_eq!(
        dependency_keys(MANIFEST, "[dependencies]"),
        BTreeSet::from([
            "fs2",
            "radroots_runtime_paths",
            "radroots_storage",
            "rustix",
            "serde",
            "sha2",
            "sqlx"
        ])
    );
    assert_eq!(
        dependency_keys(MANIFEST, "[dev-dependencies]"),
        BTreeSet::from(["serde_json", "tempfile", "tokio"])
    );
    assert_eq!(
        private_modules(ROOT),
        BTreeSet::from([
            "authority",
            "config",
            "error",
            "initialize",
            "metadata",
            "migration",
            "open",
            "status"
        ])
    );
    assert!(public_modules(ROOT).is_empty());
    let authority_production = AUTHORITY_SOURCE
        .split_once("#[cfg(all(test")
        .map(|(production, _)| production)
        .expect("authority source must keep tests separated");
    let open_production = OPEN_SOURCE
        .split_once("#[cfg(test)]")
        .map(|(production, _)| production)
        .expect("open source must keep tests separated");
    let migration_production = MIGRATION_SOURCE
        .split_once("#[cfg(test)]")
        .map(|(production, _)| production)
        .expect("migration source must keep tests separated");

    for required in [
        "ServiceSqliteErrorCode",
        "ServiceSqliteErrorKind",
        "SafeServiceSqliteError",
        "ServiceSqliteError",
        "WriterAuthority",
        "ServiceSqliteConnectionOptions",
        "ServiceSqliteConnectionOptionsError",
        "initialize_database",
        "ServiceDatabaseIdentity",
        "ServiceDatabaseMetadata",
        "ServiceSqliteApplicationId",
        "ServiceSqliteMetadataValueError",
        "MigrationAppliedAtUnixSeconds",
        "MigrationBuildIdentity",
        "MigrationCatalog",
        "MigrationChecksum",
        "MigrationContractError",
        "MigrationDescriptor",
        "MigrationEvidenceError",
        "MigrationKind",
        "MigrationName",
        "ServiceSqlitePathError",
        "ServiceSqlitePaths",
        "OpenMode",
        "StorageHealth",
        "StorageIntegrity",
        "StorageStatus",
    ] {
        assert!(
            ROOT.contains(required),
            "crate root is missing `{required}`"
        );
    }

    for required in [
        "radroots.service_sqlite.migration_content.v1\\0",
        "radroots.service_sqlite.migration_catalog.v1\\0",
        "MAX_MIGRATION_NAME_UTF8_BYTES",
        "MAX_MIGRATION_CONTENT_BYTES",
        "MAX_MIGRATION_COUNT",
        "pub const fn from_bytes",
        "pub fn for_sql",
        "pub fn for_callback",
        ".take(MAX_MIGRATION_COUNT + 1)",
        "schema_migrations",
        "applied_at_unix_s",
        "service_commit TEXT",
        "lib_revision TEXT",
        "provider_contract_version INTEGER",
        "schema_migrations_no_update",
        "schema_migrations_no_delete",
        ".begin_with(\"BEGIN IMMEDIATE\")",
        "pub(crate) async fn verify_migration_history",
        "pub(crate) async fn apply_governed_migrations",
        "validate_callback_bindings",
        "advance_schema_version",
        "pub(crate) struct MigrationTransactionExecutor",
        "set_commit_hook",
        "set_rollback_hook",
        "permit_outer_commit",
        "permit_runner_rollback",
        "reject_observed_rollback",
        "SAVEPOINT radroots_migration_transaction_probe",
        "FROM pragma_database_list",
        "CASE WHEN typeof(name) = 'text'",
        "CASE WHEN typeof(checksum) = 'blob'",
    ] {
        assert!(
            migration_production.contains(required),
            "Step 059 migration source is missing `{required}`"
        );
    }

    for forbidden in [
        "pub fn content(&self",
        "pub fn callback_definition",
        "pub fn migration_sql",
        "pub type MigrationCallback",
        "pub struct MigrationCallbackBinding",
        "pub struct MigrationTransactionExecutor",
        "pub fn apply_governed_migrations",
        "pub async fn apply_governed_migrations",
        "pub use sqlx",
        "Serialize",
        "Deserialize",
    ] {
        assert!(
            !migration_production.contains(forbidden),
            "Step 059 migration source contains deferred surface `{forbidden}`"
        );
    }

    for forbidden in [
        "radroots_service_host",
        "rusqlite",
        "tokio",
        "std::fs",
        "OpenOptions",
        "create_dir",
    ] {
        assert!(
            !authority_production.contains(forbidden)
                && !ERROR_SOURCE.contains(forbidden)
                && !CONFIG_SOURCE.contains(forbidden)
                && !STATUS_SOURCE.contains(forbidden),
            "service SQLite source contains deferred surface `{forbidden}`"
        );
    }

    for required in [
        "radroots_service_metadata",
        "PRAGMA application_id",
        "source_generation BLOB",
        "state_schema_version INTEGER",
        "created_at_unix_ms INTEGER",
        "radroots_service_metadata_guard_update",
        "radroots_service_metadata_no_delete",
        "LIMIT 2",
        "SourceGeneration",
        "NonZeroU32",
        "pub(crate) async fn write_database_metadata",
        "pub(crate) async fn verify_database_metadata",
        "MigrationLedgerInitializationFailure",
        "ServiceSqliteErrorKind::Migration",
    ] {
        assert!(
            METADATA_SOURCE.contains(required),
            "Step 057 metadata source is missing `{required}`"
        );
    }

    for forbidden in [
        "pub use sqlx",
        "pub fn write_database_metadata",
        "pub async fn write_database_metadata",
        "pub fn verify_database_metadata",
        "pub async fn verify_database_metadata",
        "myc_",
        "rhi_",
        "SystemTime::now",
        "getrandom",
        "tokio::runtime",
    ] {
        assert!(
            !METADATA_SOURCE.contains(forbidden),
            "Step 057 metadata source contains forbidden surface `{forbidden}`"
        );
    }

    for required in [
        "journal_mode(SqliteJournalMode::Wal)",
        "synchronous(SqliteSynchronous::Full)",
        ".foreign_keys(true)",
        ".pragma(\"trusted_schema\", \"OFF\")",
        ".create_if_missing(false)",
        ".statement_cache_capacity(STATEMENT_CACHE_CAPACITY)",
        ".command_buffer_size(COMMAND_BUFFER_CAPACITY)",
        ".row_buffer_size(ROW_BUFFER_CAPACITY)",
        ".immutable(true)",
        "\"query_only\"",
        "sqlite_header[18] != 2",
        "sqlite_header[19] != 2",
        "WAL_FILE_NAME",
        "SHARED_MEMORY_FILE_NAME",
        ".min_connections(1)",
        ".max_connections(policy.max_connections())",
        ".acquire_timeout(policy.busy_timeout())",
        ".idle_timeout(None)",
        ".max_lifetime(None)",
        ".after_connect",
        ".before_acquire",
        "PRAGMA busy_timeout",
        "connection.close_on_drop()",
    ] {
        assert!(
            OPEN_SOURCE.contains(required),
            "Step 056 connection source is missing `{required}`"
        );
    }

    for forbidden in [
        "pub use sqlx",
        "pub use sqlx::SqliteConnection",
        "pub use sqlx::SqlitePool",
        "pub use sqlx::Pool",
        "pub struct PrivateConnectionPool",
        "pub fn open_connection_pool",
        "pub async fn open_connection_pool",
        "pub struct MigrationCallbackBinding",
        "pub type MigrationCallback",
        "pub struct MigrationApplicationOutcome",
        "tokio::runtime",
        "Runtime::new",
    ] {
        assert!(
            !ROOT.contains(forbidden)
                && !CONFIG_SOURCE.contains(forbidden)
                && !OPEN_SOURCE.contains(forbidden),
            "Step 056 source exposes forbidden pool/runtime surface `{forbidden}`"
        );
    }

    for required in [
        "OFlags::EXCL",
        "OFlags::NOFOLLOW",
        "OFlags::CLOEXEC",
        "SERVICE_STATE_DATABASE_FILE_NAME",
        "sync_database",
        "sync_directory",
        "validate_entry",
        "unlink_database",
    ] {
        assert!(
            INITIALIZE_SOURCE.contains(required),
            "Step 055 initialization source is missing `{required}`"
        );
    }

    for forbidden in [
        "create_dir",
        "create_dir_all",
        "OpenOptions::new",
        "remove_file",
        "tokio",
        "rusqlite",
        "Command::new",
        "std::process",
        "pub fn directory",
    ] {
        let production = INITIALIZE_SOURCE
            .split_once("#[cfg(test)]")
            .map(|(source, _)| source)
            .expect("initialization source must keep tests separated");
        assert!(
            !production.contains(forbidden),
            "Step 055 production source contains deferred or bypass surface `{forbidden}`"
        );
    }

    for required in [
        "fs2::FileExt",
        "rustix",
        "try_lock_exclusive",
        "NOFOLLOW",
        "CLOEXEC",
        "st_nlink",
        "fchmod",
        "pub fn release(&mut self)",
        "database_path: paths.state_database().to_path_buf()",
        "pub(crate) fn validate_for",
    ] {
        assert!(
            AUTHORITY_SOURCE.contains(required),
            "Step 054 authority source is missing `{required}`"
        );
    }

    for forbidden in [
        "remove_file",
        "create_dir",
        "set_len",
        "truncate",
        "std::process",
        "Command::new",
        "sqlx",
        "rusqlite",
        "tokio",
    ] {
        assert!(
            !authority_production.contains(forbidden),
            "Step 054 authority production source contains deferred surface `{forbidden}`"
        );
    }

    for forbidden in [
        "Deserialize",
        "impl Default for OpenMode",
        "pub fn from_paths",
        "pub fn new(",
        "std::fs",
        "symlink_metadata",
        "create_dir",
        "File::open",
        "OpenOptions",
    ] {
        assert!(
            !open_production.contains(forbidden),
            "Step 053 open source contains deferred or forgeable surface `{forbidden}`"
        );
    }
}

fn public_modules(root: &str) -> BTreeSet<&str> {
    root.lines()
        .filter_map(|line| line.strip_prefix("pub mod "))
        .filter_map(|module| module.strip_suffix(';'))
        .collect()
}

fn private_modules(root: &str) -> BTreeSet<&str> {
    root.lines()
        .filter_map(|line| line.strip_prefix("mod "))
        .filter_map(|module| module.strip_suffix(';'))
        .collect()
}

fn dependency_keys<'a>(manifest: &'a str, section: &str) -> BTreeSet<&'a str> {
    manifest
        .split_once(section)
        .map(|(_, dependencies)| dependencies)
        .unwrap_or_default()
        .lines()
        .take_while(|line| !line.starts_with('['))
        .filter_map(|line| line.split_once('=').map(|(key, _)| key.trim()))
        .filter(|key| !key.is_empty())
        .collect()
}
