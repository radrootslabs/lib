use radroots_replica_db::migrations;
use radroots_sql_core::{SqlExecutor, SqliteExecutor};
use serde_json::Value;

fn query_rows(exec: &SqliteExecutor, sql: &str) -> Vec<Value> {
    serde_json::from_str(&exec.query_raw(sql, "[]").expect("query should succeed"))
        .expect("query should decode")
}

fn create_legacy_schema_without_secondary_indexes(exec: &SqliteExecutor) {
    let schema = [
        "CREATE TABLE __migrations (id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE, applied_at TEXT NOT NULL DEFAULT (datetime('now')))",
        "CREATE TABLE farm (id CHAR(36) PRIMARY KEY NOT NULL UNIQUE CHECK(length(id) = 36), created_at DATETIME NOT NULL CHECK(length(created_at) = 24), updated_at DATETIME NOT NULL CHECK(length(updated_at) = 24), d_tag TEXT NOT NULL, pubkey TEXT NOT NULL, name TEXT NOT NULL, about TEXT, website TEXT, picture TEXT, banner TEXT, location_primary TEXT, location_city TEXT, location_region TEXT, location_country TEXT)",
        "CREATE TABLE gcs_location (id CHAR(36) PRIMARY KEY NOT NULL UNIQUE CHECK(length(id) = 36), created_at DATETIME NOT NULL CHECK(length(created_at) = 24), updated_at DATETIME NOT NULL CHECK(length(updated_at) = 24), d_tag TEXT NOT NULL, lat REAL NOT NULL, lng REAL NOT NULL, geohash TEXT NOT NULL, point TEXT NOT NULL, polygon TEXT NOT NULL, accuracy REAL, altitude REAL, tag_0 TEXT, label TEXT, area REAL, elevation INTEGER, soil TEXT, climate TEXT, gc_id TEXT, gc_name TEXT, gc_admin1_id TEXT, gc_admin1_name TEXT, gc_country_id TEXT, gc_country_name TEXT)",
        "CREATE TABLE plot (id CHAR(36) PRIMARY KEY NOT NULL UNIQUE CHECK(length(id) = 36), created_at DATETIME NOT NULL CHECK(length(created_at) = 24), updated_at DATETIME NOT NULL CHECK(length(updated_at) = 24), d_tag TEXT NOT NULL, farm_id CHAR(36) NOT NULL, name TEXT NOT NULL, about TEXT, location_primary TEXT, location_city TEXT, location_region TEXT, location_country TEXT, FOREIGN KEY (farm_id) REFERENCES farm(id) ON DELETE CASCADE)",
        "CREATE TABLE nostr_event_state (id CHAR(36) PRIMARY KEY NOT NULL UNIQUE CHECK(length(id) = 36), created_at DATETIME NOT NULL CHECK(length(created_at) = 24), updated_at DATETIME NOT NULL CHECK(length(updated_at) = 24), key TEXT NOT NULL UNIQUE, kind INTEGER NOT NULL, pubkey CHAR(64) NOT NULL CHECK(length(pubkey) = 64), d_tag TEXT NOT NULL, last_event_id CHAR(64) NOT NULL CHECK(length(last_event_id) = 64), last_created_at INTEGER NOT NULL, content_hash TEXT NOT NULL)",
    ];

    for sql in schema {
        exec.exec(sql, "[]")
            .expect("schema statement should succeed");
    }

    for name in [
        "0000_init",
        "0001_log_error",
        "0002_farm",
        "0003_gcs_location",
        "0004_trade_product",
        "0005_nostr_profile",
        "0006_nostr_relay",
        "0007_media_image",
        "0008_farm_gcs_location",
        "0009_nostr_profile_relay",
        "0010_trade_product_location",
        "0011_trade_product_media",
        "0012_plot",
        "0013_plot_gcs_location",
        "0014_farm_tag",
        "0015_plot_tag",
        "0016_farm_member",
        "0017_farm_member_claim",
        "0018_nostr_event_state",
    ] {
        let sql = format!("INSERT INTO __migrations(name) VALUES ('{name}')");
        exec.exec(&sql, "[]")
            .expect("legacy migration marker should succeed");
    }
}

#[test]
fn run_all_up_repairs_missing_indexes_in_legacy_sqlite_dbs() {
    let exec = SqliteExecutor::open_memory().expect("open sqlite memory");
    create_legacy_schema_without_secondary_indexes(&exec);

    let before = query_rows(
        &exec,
        "SELECT name FROM sqlite_master WHERE type = 'index' AND name IN ('farm_pubkey_d_tag_idx', 'gcs_location_geohash_idx', 'plot_farm_d_tag_idx', 'nostr_event_state_kind_idx') ORDER BY name",
    );
    assert!(before.is_empty());

    migrations::run_all_up(&exec).expect("repair migration should succeed");

    let after = query_rows(
        &exec,
        "SELECT name FROM sqlite_master WHERE type = 'index' AND name IN ('farm_pubkey_d_tag_idx', 'gcs_location_geohash_idx', 'plot_farm_d_tag_idx', 'nostr_event_state_kind_idx') ORDER BY name",
    );
    let names = after
        .iter()
        .map(|row| {
            row.get("name")
                .and_then(Value::as_str)
                .expect("index name should exist")
                .to_string()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        vec![
            "farm_pubkey_d_tag_idx".to_string(),
            "gcs_location_geohash_idx".to_string(),
            "nostr_event_state_kind_idx".to_string(),
            "plot_farm_d_tag_idx".to_string(),
        ]
    );
}
