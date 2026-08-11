use std::collections::BTreeSet;

const MANIFEST: &str = include_str!("../Cargo.toml");
const README: &str = include_str!("../README.md");
const ROOT: &str = include_str!("../src/lib.rs");
const AUTHORITY_SOURCE: &str = include_str!("../src/authority.rs");
const CONFIG_SOURCE: &str = include_str!("../src/config.rs");
const CONNECTION_SOURCE: &str = include_str!("../src/connection.rs");
const ERROR_SOURCE: &str = include_str!("../src/error.rs");
const INITIALIZE_SOURCE: &str = include_str!("../src/initialize.rs");
const INTEGRITY_SOURCE: &str = include_str!("../src/integrity/mod.rs");
const INTEGRITY_CATALOG_SOURCE: &str = include_str!("../src/integrity/catalog.rs");
const METADATA_SOURCE: &str = include_str!("../src/metadata.rs");
const MIGRATION_SOURCE: &str = include_str!("../src/migration.rs");
const OPEN_SOURCE: &str = include_str!("../src/open.rs");
const STATUS_SOURCE: &str = include_str!("../src/status.rs");
const TRANSACTION_CONTROL_SOURCE: &str = include_str!("../src/transaction_control.rs");

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
            "futures",
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
            "connection",
            "error",
            "initialize",
            "integrity",
            "metadata",
            "migration",
            "open",
            "status",
            "transaction_control"
        ])
    );
    assert!(public_modules(ROOT).is_empty());
    for required in [
        "`ServiceSqliteHost` is the only public connection host",
        "borrowed `ServiceSqliteTransaction` executor",
        "transaction begin, commit, rollback, policy",
        "attached-database exclusion",
        "Writable host opening finishes every pending governed migration",
        "read-only inspection opens only current migration and",
        "with raw database authority",
        "before the runner enables outer commit",
        "leaves no authoritative transaction effect",
        "only after rollback is confirmed",
        "unconfirmed rollback is reported as `RollbackFailed`",
        "Cancelling once outer",
        "must be treated as an unknown commit outcome",
        "require rereading authoritative state",
        "before an idempotent retry",
    ] {
        assert!(
            README.contains(required),
            "Step 061 README contract is missing `{required}`"
        );
    }
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
        "MigrationApplicationOutcome",
        "MigrationBuildIdentity",
        "MigrationCallback",
        "MigrationCallbackBinding",
        "MigrationCallbackFuture",
        "MigrationCatalog",
        "MigrationChecksum",
        "MigrationContractError",
        "MigrationDescriptor",
        "MigrationEvidenceError",
        "MigrationKind",
        "MigrationName",
        "MigrationTransactionExecutor",
        "SchemaCatalog",
        "SchemaCatalogContractError",
        "SchemaDigest",
        "SchemaObject",
        "SchemaObjectKind",
        "SchemaVersionCatalog",
        "ServiceSqlitePathError",
        "ServiceSqlitePaths",
        "OpenMode",
        "ServiceSqliteHost",
        "ServiceSqliteTransaction",
        "ServiceSqliteTransactionError",
        "ServiceSqliteTransactionErrorKind",
        "ServiceSqliteTransactionFuture",
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
        ".begin_with(\"BEGIN IMMEDIATE\")",
        "pub(crate) async fn verify_migration_history",
        "pub(crate) async fn apply_governed_migrations",
        "validate_callback_bindings",
        "advance_schema_version",
        "pub struct MigrationTransactionExecutor",
        "contains_database_control",
        "database_control_rejected",
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

    for required in [
        "pub struct ServiceSqliteHost",
        "pub struct ServiceSqliteTransaction<'connection>",
        "impl<'executor, 'connection> Executor<'executor>",
        "pub enum ServiceSqliteTransactionErrorKind",
        "pub struct ServiceSqliteTransactionError<E>",
        "pub type ServiceSqliteTransactionFuture",
        "BEGIN IMMEDIATE",
        "OpenMode::ReadOnlyInspection => connection.begin().await",
        "verify_before_commit",
        "connection.close_on_drop()",
        "connection.trust()",
        "RestrictedExecute",
        "contains_database_control",
        "RADROOTS_FORBIDDEN_DATABASE_CONTROL",
        "OperationRolledBack",
        "RollbackFailed",
        "CommitOutcomeUnknown",
        "cancelling the future yields no result",
        "must reread authoritative state before any idempotent retry",
    ] {
        assert!(
            CONNECTION_SOURCE.contains(required),
            "Step 061 connection source is missing `{required}`"
        );
    }

    for required in [
        "set_commit_hook",
        "set_rollback_hook",
        "permit_outer_commit",
        "permit_runner_rollback",
        "control_violation_observed",
        "rejected_commit",
        "rejected_commit_rolled_back",
        "remove_commit_hook",
        "remove_rollback_hook",
    ] {
        assert!(
            TRANSACTION_CONTROL_SOURCE.contains(required),
            "Step 061 transaction-control source is missing `{required}`"
        );
    }

    for forbidden in [
        "pub fn pool(",
        "pub fn connection(",
        "pub fn into_inner(",
        "Deref for ServiceSqliteTransaction",
        "AsRef<SqliteConnection>",
        "pub async fn begin(",
        "pub async fn commit(",
        "pub async fn rollback(",
        "pub use sqlx",
    ] {
        assert!(
            !CONNECTION_SOURCE.contains(forbidden) && !ROOT.contains(forbidden),
            "Step 061 source exposes forbidden raw authority `{forbidden}`"
        );
    }

    for required in [
        "radroots.service_sqlite.schema_object.v1\\0",
        "radroots.service_sqlite.schema_snapshot.v1\\0",
        "radroots.service_sqlite.schema_catalog.v1\\0",
        "MAX_SCHEMA_OBJECT_COUNT",
        "MAX_SCHEMA_SQL_UTF8_BYTES",
        "MAX_SCHEMA_CATALOG_UTF8_BYTES",
        ".take(MAX_SCHEMA_OBJECT_COUNT + 1)",
        ".take(MAX_SCHEMA_VERSION_COUNT + 1)",
        "CREATE TABLE radroots_service_metadata",
        "CREATE TABLE schema_migrations",
        "schema_migrations_no_update",
        "schema_migrations_no_delete",
        "pub fn computed_digest",
        "pub fn new<I>",
        "pub(crate) async fn verify_schema_catalog",
        "FROM main.sqlite_schema",
        "LIMIT 4097",
        "typeof(sql) = 'text'",
        "length(CAST(sql AS BLOB)) BETWEEN 1 AND 1048576",
    ] {
        assert!(
            INTEGRITY_SOURCE.contains(required) || INTEGRITY_CATALOG_SOURCE.contains(required),
            "Step 060 integrity source is missing `{required}`"
        );
    }

    for forbidden in [
        "pub async fn verify_schema_catalog",
        "pub fn verify_schema_catalog",
        "pub use sqlx",
        "SqlitePool",
        "PoolConnection",
        "pub fn sql(&self",
        "Serialize",
        "Deserialize",
        "myc_",
        "rhi_",
    ] {
        assert!(
            !INTEGRITY_SOURCE.contains(forbidden) && !INTEGRITY_CATALOG_SOURCE.contains(forbidden),
            "Step 060 integrity source contains forbidden surface `{forbidden}`"
        );
    }

    for forbidden in [
        "pub fn content(&self",
        "pub fn callback_definition",
        "pub fn migration_sql",
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
        "PRAGMA application_id",
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

    for required in [
        "radroots_service_metadata",
        "source_generation BLOB",
        "state_schema_version INTEGER",
        "created_at_unix_ms INTEGER",
        "radroots_service_metadata_guard_update",
        "radroots_service_metadata_no_delete",
        "schema_migrations",
        "applied_at_unix_s",
        "service_commit TEXT",
        "lib_revision TEXT",
        "provider_contract_version INTEGER",
    ] {
        assert!(
            INTEGRITY_CATALOG_SOURCE.contains(required),
            "shared schema authority is missing `{required}`"
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
        "validate_authority_binding",
        "directory_device",
        "directory_inode",
        "lock_device",
        "lock_inode",
        "current_lock_status",
    ] {
        assert!(
            AUTHORITY_SOURCE.contains(required),
            "Step 054 authority source is missing `{required}`"
        );
    }

    for required in [
        "inspection_guard.validate_for(&self.paths)",
        "fn validate_for(&self, paths: &ServiceSqlitePaths)",
        "held_lock_status",
        "lock_device",
        "directory_device",
        "WAL_FILE_NAME",
        "SHARED_MEMORY_FILE_NAME",
        "u32::from(directory_status.st_mode) & 0o022",
    ] {
        assert!(
            OPEN_SOURCE.contains(required),
            "Step 061 live authority source is missing `{required}`"
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
