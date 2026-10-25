use std::collections::BTreeSet;

const MANIFEST: &str = include_str!("../Cargo.toml");
const ROOT: &str = include_str!("../src/lib.rs");

#[test]
fn sqlite_storage_declares_the_final_backend_boundaries() {
    fn assert_complete_storage<T: radroots_storage::Storage>() {}
    assert_complete_storage::<radroots_storage_sqlite::SqliteStorage>();

    for required in [
        "name = \"radroots_storage_sqlite\"",
        "version = \"0.1.0-alpha\"",
        "publish = [\"crates-io\"]",
        "[lib]\nname = \"radroots_storage_sqlite\"",
    ] {
        assert!(
            MANIFEST.contains(required),
            "manifest is missing `{required}`"
        );
    }
    assert!(!MANIFEST.contains("[features]"));
    assert_eq!(
        dependency_keys(MANIFEST),
        BTreeSet::from([
            "fs2",
            "getrandom",
            "nostr",
            "radroots_domain",
            "radroots_event",
            "radroots_event_codec",
            "radroots_nostr",
            "radroots_protocol",
            "radroots_secrets",
            "radroots_signing",
            "radroots_storage",
            "radroots_transport",
            "serde",
            "serde_json",
            "sha2",
            "rustix",
            "subtle",
            "sqlx",
            "tokio",
            "zeroize",
        ])
    );
    let manifest: toml::Value = toml::from_str(MANIFEST).expect("checked manifest");
    let crypto = manifest["dependencies"]["nostr"]
        .as_table()
        .expect("explicit crypto dependency");
    assert_eq!(
        crypto.keys().map(String::as_str).collect::<BTreeSet<_>>(),
        BTreeSet::from(["workspace", "default-features", "features"])
    );
    assert_eq!(crypto["workspace"].as_bool(), Some(true));
    assert_eq!(crypto["default-features"].as_bool(), Some(false));
    assert_eq!(
        crypto["features"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["std", "nip44"])
    );
    assert!(!ROOT.contains("SqlitePool"));
    assert!(!ROOT.contains("sqlx"));
    for forbidden in [
        "radroots_event_store",
        "radroots_outbox",
        "radroots_runtime_store",
        "radroots_protected_store",
        "radroots_secret_vault",
        "radroots_nostr_accounts",
    ] {
        assert!(!MANIFEST.contains(forbidden));
        assert!(!ROOT.contains(forbidden));
    }

    assert_eq!(
        public_modules(ROOT),
        BTreeSet::from([
            "backup",
            "config",
            "integrity",
            "legacy",
            "lock",
            "migration",
            "open",
            "status",
            "workspace",
        ])
    );
}

fn public_modules(source: &str) -> BTreeSet<&str> {
    source
        .lines()
        .filter_map(|line| line.trim().strip_prefix("pub mod "))
        .filter_map(|module| module.strip_suffix(';'))
        .collect()
}

fn dependency_keys(manifest: &str) -> BTreeSet<&str> {
    manifest
        .split_once("[dependencies]")
        .map(|(_, dependencies)| dependencies)
        .unwrap_or_default()
        .lines()
        .skip(1)
        .take_while(|line| !line.starts_with('['))
        .filter_map(|line| line.split_once('=').map(|(key, _)| key.trim()))
        .filter(|key| !key.is_empty())
        .collect()
}
