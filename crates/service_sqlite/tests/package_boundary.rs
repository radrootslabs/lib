use std::collections::BTreeSet;

const MANIFEST: &str = include_str!("../Cargo.toml");
const ROOT: &str = include_str!("../src/lib.rs");
const AUTHORITY_SOURCE: &str = include_str!("../src/authority.rs");
const CONFIG_SOURCE: &str = include_str!("../src/config.rs");
const ERROR_SOURCE: &str = include_str!("../src/error.rs");
const INITIALIZE_SOURCE: &str = include_str!("../src/initialize.rs");
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
        BTreeSet::from(["fs2", "radroots_runtime_paths", "rustix", "serde", "sqlx"])
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

    for required in [
        "ServiceSqliteErrorCode",
        "ServiceSqliteErrorKind",
        "SafeServiceSqliteError",
        "ServiceSqliteError",
        "WriterAuthority",
        "ServiceSqliteConnectionOptions",
        "ServiceSqliteConnectionOptionsError",
        "initialize_database",
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
        "sqlx",
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
