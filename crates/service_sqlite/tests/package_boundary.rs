use std::collections::BTreeSet;

const MANIFEST: &str = include_str!("../Cargo.toml");
const README: &str = include_str!("../README.md");
const ROOT: &str = include_str!("../src/lib.rs");
const AUTHORITY_SOURCE: &str = include_str!("../src/authority.rs");
const BACKUP_SOURCE: &str = include_str!("../src/backup/manifest.rs");
const BACKUP_CAPTURE_SOURCE: &str = include_str!("../src/backup/capture.rs");
const BACKUP_VERIFY_SOURCE: &str = include_str!("../src/backup/verify.rs");
const CONFIG_SOURCE: &str = include_str!("../src/config.rs");
const CONNECTION_SOURCE: &str = include_str!("../src/connection.rs");
const ERROR_SOURCE: &str = include_str!("../src/error.rs");
const INITIALIZE_SOURCE: &str = include_str!("../src/initialize.rs");
const INTEGRITY_SOURCE: &str = include_str!("../src/integrity/mod.rs");
const INTEGRITY_CATALOG_SOURCE: &str = include_str!("../src/integrity/catalog.rs");
const INTEGRITY_INSPECTION_SOURCE: &str = include_str!("../src/integrity/inspection.rs");
const METADATA_SOURCE: &str = include_str!("../src/metadata.rs");
const MIGRATION_SOURCE: &str = include_str!("../src/migration.rs");
const OPEN_SOURCE: &str = include_str!("../src/open.rs");
const RESTORE_MARKER_SOURCE: &str = include_str!("../src/restore/marker.rs");
const RESTORE_FINALIZE_SOURCE: &str = include_str!("../src/restore/finalize.rs");
const RESTORE_RECOVER_SOURCE: &str = include_str!("../src/restore/recover.rs");
const RESTORE_ROOT_SOURCE: &str = include_str!("../src/restore/mod.rs");
const RESTORE_STAGE_SOURCE: &str = include_str!("../src/restore/stage.rs");
const STATUS_SOURCE: &str = include_str!("../src/status.rs");
const TRANSACTION_CONTROL_SOURCE: &str = include_str!("../src/transaction_control.rs");

#[test]
fn service_sqlite_is_unpublished_lint_governed_and_dependency_bounded() {
    let readme_words = README.split_whitespace().collect::<Vec<_>>().join(" ");
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
            "rusqlite",
            "rustix",
            "serde",
            "serde_json",
            "sha2",
            "sqlx",
            "tokio"
        ])
    );
    assert_eq!(
        dependency_keys(MANIFEST, "[dev-dependencies]"),
        BTreeSet::from(["tempfile", "tokio"])
    );
    assert_eq!(
        private_modules(ROOT),
        BTreeSet::from([
            "authority",
            "backup",
            "config",
            "connection",
            "error",
            "initialize",
            "integrity",
            "metadata",
            "migration",
            "open",
            "restore",
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
        "Every host must be closed explicitly with `ServiceSqliteHost::close`",
        "permanently stops new transaction admission",
        "drains transactions that were",
        "safe to call sequentially or concurrently",
        "fixed `PRAGMA wal_checkpoint(TRUNCATE)` policy",
        "requires an unblocked checkpoint",
        "releases its shared inspection guard without checkpointing or mutating",
        "Cancelling close before terminal completion",
        "private connect, checkpoint, and explicit connection-close driver remains host-owned",
        "without losing the SQLite handle or its close proof",
        "a later call resumes close",
        "stable outer result is cached",
        "Dropping a host performs no asynchronous close work",
        "`ServiceBackupManifest` is the stable model-only v1 backup identity",
        "compact canonical UTF-8 JSON in the frozen field order, capped at 1,024 bytes",
        "external manifest SHA-256 over those exact bytes",
        "exactly one `state.sqlite` member",
        "integrity are exactly `ok`, and protected material is always excluded",
        "Parsing proves only the strict structural and canonical contract",
        "unknown, duplicate, null, reordered, whitespace-altered, or version-drifted input",
        "Constructing or parsing the manifest model performs no filesystem or SQLite work",
        "Writable hosts provide `ServiceSqliteHost::capture_online_backup`",
        "one incremental, point-in-time SQLite capture at a time",
        "exact new absolute staging-directory path",
        "directory with mode `0700` and its sole `state.sqlite` member with mode `0600`",
        "online-backup API without checkpointing or copying the live source file",
        "requires exact service metadata, bounded `integrity_check`, an empty `foreign_key_check`",
        "SHA-256, and file, staging, and parent synchronization",
        "returning the canonical manifest in memory",
        "No manifest file, bundle identifier, credential, or protected material",
        "Dropping the capture future requests cancellation",
        "retains its checked-out pool admission, writer authority, and exact staging identities",
        "Host close therefore drains capture and cancellation cleanup",
        "Capture has no hidden timeout",
        "callers own any deadline by cancelling the future",
        "does not provide restore or replacement behavior",
        "`verify_backup_bundle` is the synchronous, task-free boundary",
        "independently protected manifest SHA-256",
        "positive maximum state-file size",
        "restrictive owner-only directory containing only `state.sqlite`",
        "performs no filesystem mutation and does not create a task or hidden deadline",
        "non-forgeable `VerifiedServiceBackup`",
        "exposing only the canonical manifest and actual database metadata",
        "It is not restore or replacement authority",
        "copy from the retained member and reverify the staged copy",
        "pathname verification alone is insufficient",
        "Restore crash recovery uses a private sealed v1 marker",
        "state.restore-staged.sqlite",
        "state.restore-backup.sqlite",
        "state.restore-marker.v1.next",
        "compact canonical JSON is capped at 2,048 bytes",
        "domain-separated checksum binds the canonical fields and detects corruption",
        "it is not an authenticity credential",
        "only legal durable sequence is `prepared` to `live_retained` to `replacement_installed`",
        "repeating the current phase is byte-idempotent",
        "descriptor-relative, no-follow, single-link, owner-owned regular files with mode `0600`",
        "compare-and-reloads the current bytes",
        "create-new scratch, atomically replaces the marker",
        "reads do not repair or remove it",
        "does not stage, copy, open, rename, replace, or delete a database",
        "No marker type, path, raw descriptor, or store operation is public API",
        "`stage_verified_restore` is the offline boundary",
        "acquires exclusive writer authority",
        "fixed adjacent `state.restore-staged.sqlite`",
        "A live writable or read-only host",
        "opened create-new, no-follow, owner-only, and single-link with mode `0600`",
        "copies the exact manifest-bound bytes from the verifier's retained member descriptor",
        "opens SQLite only through the retained staged descriptor",
        "exact applied migration prefix and schema-object catalog",
        "bounded `integrity_check(1)`, and empty `foreign_key_check`",
        "Live database bytes, identity, permissions, and timestamps remain untouched",
        "sealed non-cloneable `StagedServiceRestore`",
        "attempts an identity-checked unlink and state-directory synchronization",
        "cleanup failure leaves staging or recovery evidence",
        "detached work retains authority and exact cleanup ownership",
        "does not create or advance a recovery marker",
        "`finalize_staged_restore` consumes that sealed stage",
        "bound the exact live inode, length, and digest",
        "creates and synchronizes the `prepared` marker",
        "descriptor-relative no-replace operations",
        "marker advance to `live_retained` or `replacement_installed`",
        "Cancellation observed before the worker's atomic commit-ownership handoff",
        "interval before `prepared` becomes durable",
        "Once `prepared` is durable, staged-artifact cleanup is disarmed",
        "unknown immediate outcome",
        "retains the old live database and final marker",
        "Read-write-existing open is the sole recovery path",
        "before opening SQLite",
        "Read-only inspection, initialization, and an initialized open never recover",
        "reject any stage, backup, marker, or marker scratch as `Recovery` without mutation",
        "Recovery uses exact topology as the durable authority",
        "rolls back by removing only the exact stage and then the marker",
        "recovery advances and rolls forward",
        "removes the exact old backup before retiring the marker",
        "repeated recovery is idempotent",
        "A marker scratch is admitted only when it is the canonical one-edge successor",
        "Recovery removes only the exact bound scratch inode",
        "then reproduces the transition through the governed marker-advance path",
        "preserves the valid current marker if the scratch pathname was replaced",
        "Recovery has no await point or hidden task",
        "each synchronous filesystem step and its authority checks complete",
        "Finalization itself does not reconcile or reopen the database",
        "`ServiceSqliteHost::inspect_integrity` is the explicit active operator check",
        "available on initialized, writable-existing, and read-only inspection hosts",
        "admits at most one check per host",
        "uses one deferred read transaction as the SQLite snapshot",
        "caller injects a positive wall-clock `IntegrityCheckedAtUnixMs`",
        "does not read an ambient clock or create a timer",
        "only `verified` or `failed` for SQLite integrity and foreign keys",
        "at most the fixed `sqlite_integrity_failed` and `foreign_key_violation` diagnostic codes",
        "canonical order",
        "projected to the passive `StorageIntegrity` vocabulary",
        "does not persist or cache the report",
        "never publishes raw SQLite diagnostics, table or row identity",
        "Inability to execute, decode, or finish either bounded check is an `Integrity` error",
        "Authority is revalidated after every await and has precedence",
        "operation has no hidden timeout or task",
        "Callers own a positive monotonic deadline by dropping the future",
        "cancellation returns no report, writes nothing, quarantines the checked-out connection",
        "leaves it in a host-owned close driver",
        "Retry or host close explicitly awaits that retained close future",
        "until the prior SQLite worker terminates",
        "before any new check or authority release",
        "retry uses a newly injected wall-clock time",
        "strict backup and restore integrity verifier remains a separate fail-closed boundary",
    ] {
        assert!(
            readme_words.contains(required),
            "Step 061 README contract is missing `{required}`"
        );
    }
    let authority_production = AUTHORITY_SOURCE
        .split_once("#[cfg(all(test")
        .map(|(production, _)| production)
        .expect("authority source must keep tests separated");
    let open_production = OPEN_SOURCE
        .split_once("#[cfg(test)]\nmod tests")
        .map(|(production, _)| production)
        .expect("open source must keep tests separated");
    let migration_production = MIGRATION_SOURCE
        .split_once("#[cfg(test)]")
        .map(|(production, _)| production)
        .expect("migration source must keep tests separated");
    let connection_production = CONNECTION_SOURCE
        .split_once("#[cfg(test)]\nmod tests")
        .map(|(production, _)| production)
        .expect("connection source must keep tests separated");
    let backup_production = BACKUP_SOURCE
        .split_once("#[cfg(test)]")
        .map(|(production, _)| production)
        .expect("backup source must keep tests separated");
    let backup_capture_production = BACKUP_CAPTURE_SOURCE
        .split_once("#[cfg(test)]\nmod tests")
        .map(|(production, _)| production)
        .expect("backup capture source must keep tests separated");
    let backup_verify_production = BACKUP_VERIFY_SOURCE
        .split_once("#[cfg(test)]")
        .map_or(BACKUP_VERIFY_SOURCE, |(production, _)| production);
    let integrity_inspection_production = INTEGRITY_INSPECTION_SOURCE
        .split_once("#[cfg(all(test, any(target_os = \"linux\", target_os = \"macos\")))]")
        .map(|(production, _)| production)
        .expect("integrity inspection source must keep test seams separated");
    let restore_marker_production = RESTORE_MARKER_SOURCE
        .split_once("#[cfg(test)]\nmod tests")
        .map(|(production, _)| production)
        .expect("restore marker source must keep tests separated");

    for required in [
        "ServiceSqliteErrorCode",
        "ServiceSqliteErrorKind",
        "SafeServiceSqliteError",
        "ServiceSqliteError",
        "WriterAuthority",
        "BackupCreatedAtUnixMs",
        "BackupManifestContractError",
        "BackupManifestIntegrity",
        "BackupManifestSha256",
        "BackupMemberSha256",
        "ServiceBackupManifest",
        "ServiceBackupMember",
        "VerifiedServiceBackup",
        "verify_backup_bundle",
        "BACKUP_MANIFEST_CANONICAL_MAX_BYTES",
        "BACKUP_MANIFEST_SCHEMA",
        "BACKUP_MANIFEST_SCHEMA_VERSION",
        "BACKUP_STATE_MEMBER_NAME",
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
        "IntegrityCheckOutcome",
        "IntegrityCheckedAtUnixMs",
        "IntegrityDiagnosticCode",
        "ServiceSqliteIntegrityReport",
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
        "radroots.service-backup",
        "BACKUP_MANIFEST_SCHEMA_VERSION: u32 = 1",
        "BACKUP_MANIFEST_CANONICAL_MAX_BYTES: usize = 1_024",
        "BACKUP_STATE_MEMBER_NAME: &str = \"state.sqlite\"",
        "pub fn from_canonical_bytes",
        "manifest.canonical_bytes.as_ref() != bytes",
        "serde(deny_unknown_fields)",
        "Sha256::digest(&canonical_bytes)",
        "pub(crate) fn from_capture",
        "ServiceDatabaseMetadata",
        "InvalidMemberInventory",
        "ProtectedMaterialIncluded",
    ] {
        assert!(
            backup_production.contains(required),
            "Step 063 backup source is missing `{required}`"
        );
    }
    for forbidden in [
        "impl Serialize for ServiceBackupManifest",
        "impl<'de> Deserialize<'de> for ServiceBackupManifest",
        "pub fn from_capture",
        "std::fs",
        "sqlx",
        "tokio",
        "SystemTime",
        "WallClock",
        "PathBuf",
        "OpenOptions",
    ] {
        assert!(
            !backup_production.contains(forbidden),
            "Step 063 backup source contains deferred or forgeable surface `{forbidden}`"
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
        "pub async fn close(&self)",
        "pub async fn capture_online_backup(",
        "pub async fn inspect_integrity(",
        "closing.store(true, Ordering::Release)",
        "close_state.lock().await",
        "ServiceSqliteHostCloseState::Complete",
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
        "impl Drop for ServiceSqliteHost",
        "pub async fn checkpoint",
        "pub fn checkpoint",
        "checkpoint_mode",
    ] {
        assert!(
            !connection_production.contains(forbidden) && !ROOT.contains(forbidden),
            "Step 061 source exposes forbidden raw authority `{forbidden}`"
        );
    }

    for required in [
        "pub struct IntegrityCheckedAtUnixMs",
        "pub enum IntegrityCheckOutcome",
        "pub enum IntegrityDiagnosticCode",
        "pub struct ServiceSqliteIntegrityReport",
        "SqliteIntegrityFailed",
        "ForeignKeyViolation",
        "Box<[IntegrityDiagnosticCode]>",
        "pub const fn storage_integrity",
        "PRAGMA integrity_check(1)",
        "SELECT 1 FROM pragma_foreign_key_check LIMIT 1",
        "connection.begin().await",
        "transaction.rollback().await",
        "validate()?",
    ] {
        assert!(
            integrity_inspection_production.contains(required),
            "Step 070 integrity inspection source is missing `{required}`"
        );
    }
    for required in [
        "integrity_driver: tokio::sync::Mutex<IntegrityInspectionDriver>",
        ".try_lock()",
        "driver.close_retained().await",
        "QuarantinedConnection::new",
        "crate::integrity::inspect_database_integrity",
        "connection.trust()",
    ] {
        assert!(
            connection_production.contains(required),
            "Step 070 host integration is missing `{required}`"
        );
    }
    for forbidden in [
        "pub use sqlx",
        "SqlitePool",
        "PoolConnection",
        "SystemTime",
        "Instant",
        "tokio::time",
        "tokio::task",
        "spawn",
        "std::fs",
        "OpenOptions",
        "write_all",
        "persist",
        "cache",
        "myc_",
        "rhi_",
    ] {
        assert!(
            !integrity_inspection_production.contains(forbidden),
            "Step 070 integrity inspection contains forbidden authority `{forbidden}`"
        );
    }

    for required in [
        "rusqlite::backup::Backup::new",
        "tokio::task::spawn_blocking",
        "BACKUP_PAGES_PER_STEP",
        "HASH_BUFFER_BYTES",
        "OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC",
        "Mode::RUSR | Mode::WUSR | Mode::XUSR",
        "Mode::RUSR | Mode::WUSR",
        "PRAGMA integrity_check(1)",
        "ValueRef::Text",
        "MAX_INTEGRITY_RESULT_UTF8_BYTES",
        "PRAGMA foreign_key_check",
        "BackupSourceValidator",
        "PoolConnection<Sqlite>",
        "CaptureCancellation",
        "CapturePermit",
        "ServiceBackupManifest::from_capture",
        "sync_state",
        "sync_directories",
        "hash_state",
        "validate_inventory",
    ] {
        assert!(
            backup_capture_production.contains(required),
            "Step 064 backup capture source is missing `{required}`"
        );
    }
    for forbidden in [
        "pub use rusqlite",
        "pub fn restore",
        "pub async fn restore",
        "pub fn verify_backup",
        "pub async fn verify_backup",
        "VACUUM INTO",
        "SystemTime::now",
        "tokio::time::timeout",
        "manifest.json",
        "create_dir_all",
        "std::fs::copy",
    ] {
        assert!(
            !backup_capture_production.contains(forbidden) && !ROOT.contains(forbidden),
            "Step 064 backup capture source contains deferred or public authority `{forbidden}`"
        );
    }

    for required in [
        "pub fn verify_backup_bundle(",
        "NonZeroU64",
        "pub struct VerifiedServiceBackup",
        "pub const fn manifest(&self)",
        "pub const fn database_metadata(&self)",
        "Sha256::digest(manifest_bytes)",
        "BACKUP_MANIFEST_CANONICAL_MAX_BYTES",
        "OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC",
        "OFlags::RDONLY | OFlags::NONBLOCK | OFlags::NOFOLLOW | OFlags::CLOEXEC",
        "Dir::read_from(&self.directory)",
        "matches!(mode, 0o500 | 0o700)",
        "matches!(mode, 0o400 | 0o600)",
        "open_sqlite_from_retained_state",
        "/proc/self/fd/{descriptor}",
        "/dev/fd/{descriptor}",
        "?mode=ro&immutable=1",
        "pragma_update(None, \"query_only\", true)",
        "pragma_update(None, \"trusted_schema\", false)",
        "verify_connection_policy",
        "PRAGMA database_list",
        "FROM main.sqlite_schema",
        "ValueRef::Text(b\"table\")",
        "PRAGMA integrity_check(1)",
        "PRAGMA foreign_key_check",
        "state_schema_version() > expected.supported_state_schema_version()",
        "binding.hash_state(maximum_state_bytes)",
    ] {
        assert!(
            backup_verify_production.contains(required),
            "Step 065 backup verifier is missing `{required}`"
        );
    }
    for forbidden in [
        "tokio::task::spawn_blocking",
        "pub fn state_file",
        "pub fn directory",
        "pub fn path",
        "pub fn restore",
        "pub async fn restore",
        "ServiceSqliteErrorKind::Restore",
        "create_dir",
        "remove_file",
        "std::fs::copy",
        "rename",
        "SystemTime::now",
        "tokio::time::timeout",
    ] {
        assert!(
            !backup_verify_production.contains(forbidden),
            "Step 065 backup verifier contains deferred or raw authority `{forbidden}`"
        );
    }

    for required in [
        "radroots.service-sqlite.restore-marker",
        "RESTORE_MARKER_SCHEMA_VERSION: u32 = 1",
        "RESTORE_MARKER_MAX_BYTES: usize = 2_048",
        "radroots.service_sqlite.restore_marker.v1\\0",
        "state.restore-staged.sqlite",
        "state.restore-backup.sqlite",
        "state.restore-marker.v1",
        "state.restore-marker.v1.next",
        "enum RestoreRecoveryPhase",
        "Prepared",
        "LiveRetained",
        "ReplacementInstalled",
        "fn transitioned_to(",
        "serde(deny_unknown_fields)",
        "OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK",
        "OFlags::RDWR",
        "OFlags::CREATE",
        "OFlags::EXCL",
        "Mode::RUSR | Mode::WUSR",
        "renameat(",
        "sync_all()",
        "cleanup_exact",
        "ServiceSqliteErrorKind::Restore",
        "ServiceSqliteErrorKind::Recovery",
    ] {
        assert!(
            restore_marker_production.contains(required),
            "Step 066 restore marker is missing `{required}`"
        );
    }
    for forbidden in [
        "pub mod restore",
        "pub use marker",
        "pub struct Restore",
        "pub enum Restore",
        "pub fn restore",
        "pub async fn restore",
        "tokio::",
        "sqlx::",
        "rusqlite::",
        "std::fs::copy",
        "state_database().rename",
        "remove_dir_all",
        "SystemTime",
        "timeout(",
    ] {
        assert!(
            !restore_marker_production.contains(forbidden)
                && !RESTORE_ROOT_SOURCE.contains(forbidden)
                && !ROOT.contains(forbidden),
            "Step 066 restore marker exposes or implements deferred authority `{forbidden}`"
        );
    }

    let restore_stage_production = RESTORE_STAGE_SOURCE
        .split_once(
            "#[cfg(all(test, any(target_os = \"linux\", target_os = \"macos\")))]\nmod tests",
        )
        .map(|(production, _)| production)
        .expect("restore stage source must keep tests separated");
    for required in [
        "pub async fn stage_verified_restore(",
        "pub struct StagedServiceRestore",
        "VerifiedServiceBackup",
        "WriterAuthority::acquire(paths, OpenMode::ReadWriteExisting)",
        "tokio::spawn(async move",
        "tokio::task::spawn_blocking",
        "OFlags::CREATE",
        "OFlags::EXCL",
        "OFlags::NOFOLLOW",
        "OFlags::NONBLOCK",
        "STAGED_FILE_NAME",
        "verified.state_file()",
        ".validate_binding()",
        "verify_database_metadata",
        "verify_migration_history",
        "verify_database_integrity",
        "/proc/self/fd/{descriptor}",
        "/dev/fd/{descriptor}",
        "PRAGMA query_only = ON",
        "PRAGMA trusted_schema = OFF",
        "PRAGMA database_list",
        "cleanup_exact_stage",
        "authority.release()",
        "StagedServiceRestore([redacted])",
    ] {
        assert!(
            restore_stage_production.contains(required),
            "Step 067 restore staging source is missing `{required}`"
        );
    }
    for forbidden in [
        "pub fn state_file",
        "pub fn directory",
        "pub fn path",
        "pub fn authority",
        "pub fn artifact",
        "RestoreMarkerBinding::create",
        "RestoreRecoveryPhase::LiveRetained",
        "RestoreRecoveryPhase::ReplacementInstalled",
        "renameat(",
        "std::fs::rename",
        "remove_dir_all",
        "SystemTime::now",
        "tokio::time::timeout",
    ] {
        assert!(
            !restore_stage_production.contains(forbidden),
            "Step 067 restore staging source contains deferred or raw authority `{forbidden}`"
        );
    }
    let restore_finalize_production = RESTORE_FINALIZE_SOURCE
        .split_once(
            "#[cfg(all(test, any(target_os = \"linux\", target_os = \"macos\")))]\nstruct FailingFinalizeOperations",
        )
        .map(|(production, _)| production)
        .expect("restore finalization source must keep failure injection separated");
    for required in [
        "pub async fn finalize_staged_restore(",
        "tokio::task::spawn_blocking",
        "RestoreRecoveryMarker::prepared(",
        "staged.disarm_cleanup()",
        "renameat_with(",
        "RenameFlags::NOREPLACE",
        "directory.sync_all()",
        "RestoreRecoveryPhase::LiveRetained",
        "RestoreRecoveryPhase::ReplacementInstalled",
        "verify_named_artifact(",
        "CancellationOnDrop",
    ] {
        assert!(
            restore_finalize_production.contains(required),
            "Step 068 restore finalization source is missing `{required}`"
        );
    }
    for forbidden in [
        "pub struct RestoreRecovery",
        "pub enum RestoreRecovery",
        "pub fn marker",
        "pub fn directory",
        "pub fn path",
        "sqlx::",
        "rusqlite::",
        "remove_file",
        "unlinkat",
        "remove_dir_all",
        "tokio::time::timeout",
        "ServiceSqliteHost",
    ] {
        assert!(
            !restore_finalize_production.contains(forbidden),
            "Step 068 restore finalization source contains deferred or raw authority `{forbidden}`"
        );
    }
    for required in ["refuse_unresolved_recovery", "recover_for_open"] {
        assert!(
            RESTORE_ROOT_SOURCE.contains(required),
            "Step 068 recovery-open guard is missing `{required}`"
        );
    }
    assert!(OPEN_SOURCE.contains("crate::restore::refuse_unresolved_recovery"));
    assert!(OPEN_SOURCE.contains("crate::restore::recover_for_open(paths, identity, authority)"));

    let restore_recover_production = RESTORE_RECOVER_SOURCE
        .split_once("#[cfg(all(test, any(target_os = \"linux\", target_os = \"macos\")))]")
        .map(|(production, _)| production)
        .expect("restore recovery source must keep tests separated");
    for required in [
        "pub(crate) fn recover_for_open(",
        "WriterAuthority",
        "RestoreMarkerBinding::load_for_recovery",
        "matches_identity(identity)",
        "interrupted_transition(paths, authority)",
        "promote_interrupted_transition",
        "advance_for_recovery",
        "RestoreRecoveryPhase::Prepared",
        "RestoreRecoveryPhase::LiveRetained",
        "RestoreRecoveryPhase::ReplacementInstalled",
        "RenameFlags::NOREPLACE",
        "verify_named_artifact(",
        "hash_exact(",
        "remove_exact_artifact(",
        "marker.retire(paths, authority)",
        "state.sqlite-wal",
        "state.sqlite-shm",
        "state.sqlite-journal",
        "ServiceSqliteErrorKind::Recovery",
    ] {
        assert!(
            restore_recover_production.contains(required),
            "Step 069 restore recovery source is missing `{required}`"
        );
    }
    for forbidden in [
        "pub fn recover",
        "pub async fn recover",
        "pub struct PendingRestore",
        "sqlx::",
        "rusqlite::",
        "tokio::",
        "spawn_blocking",
        "tokio::time::timeout",
        "remove_dir_all",
        "SystemTime::now",
    ] {
        assert!(
            !restore_recover_production.contains(forbidden) && !ROOT.contains(forbidden),
            "Step 069 recovery exposes or implements forbidden authority `{forbidden}`"
        );
    }
    for required in [
        "STAGED_FILE_NAME",
        "BACKUP_FILE_NAME",
        "MARKER_FILE_NAME",
        "MARKER_NEXT_FILE_NAME",
    ] {
        assert!(
            RESTORE_RECOVER_SOURCE.contains(required),
            "Step 069 refusal inventory is missing `{required}`"
        );
    }
    assert!(ROOT.contains(
        "pub use restore::{StagedServiceRestore, finalize_staged_restore, stage_verified_restore};"
    ));

    for required in [
        "self.pool.close().await",
        "PRAGMA wal_checkpoint(TRUNCATE)",
        "if busy == 0",
        ".close()",
        "authority.release()?",
        "inspection.release()?",
        "release_resources",
        "CheckpointBusy",
        "close_driver: tokio::sync::Mutex<PrivateCloseDriver>",
        "PrivateCloseDriver::Connecting",
        "PrivateCloseDriver::Connected",
        "PrivateCloseDriver::Closing",
        "connect.as_mut().await",
        "future.as_mut().await",
    ] {
        assert!(
            open_production.contains(required),
            "Step 062 close source is missing `{required}`"
        );
    }
    for forbidden in [
        "tokio::spawn",
        "Runtime::new",
        "pub enum Checkpoint",
        "pub struct Checkpoint",
    ] {
        assert!(
            !connection_production.contains(forbidden) && !open_production.contains(forbidden),
            "Step 062 close source contains forbidden authority `{forbidden}`"
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
        ".inspection_guard",
        ".validate_for(&self.paths)?",
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
