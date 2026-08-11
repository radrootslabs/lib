use std::collections::BTreeSet;

const MANIFEST: &str = include_str!("../Cargo.toml");
const ROOT: &str = include_str!("../src/lib.rs");
const ERROR_SOURCE: &str = include_str!("../src/error.rs");
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
        BTreeSet::from(["radroots_runtime_paths", "serde"])
    );
    assert_eq!(
        dependency_keys(MANIFEST, "[dev-dependencies]"),
        BTreeSet::from(["serde_json"])
    );
    assert_eq!(
        private_modules(ROOT),
        BTreeSet::from(["error", "open", "status"])
    );
    assert!(public_modules(ROOT).is_empty());

    for required in [
        "ServiceSqliteErrorCode",
        "ServiceSqliteErrorKind",
        "SafeServiceSqliteError",
        "ServiceSqliteError",
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
        "sqlx",
        "rusqlite",
        "tokio",
        "std::fs",
        "OpenOptions",
        "create_dir",
        "Connection",
        "Pool",
    ] {
        assert!(
            !ERROR_SOURCE.contains(forbidden)
                && !OPEN_SOURCE.contains(forbidden)
                && !STATUS_SOURCE.contains(forbidden),
            "service SQLite source contains deferred surface `{forbidden}`"
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
        "fs2",
        "rustix",
        "lock_exclusive",
    ] {
        assert!(
            !OPEN_SOURCE.contains(forbidden),
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
