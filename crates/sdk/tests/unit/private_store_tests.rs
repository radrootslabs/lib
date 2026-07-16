use super::*;
use radroots_event::ids::RadrootsAddressableCoordinate;
use sqlx::Row;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

fn farm_addr_for(d_tag: &str) -> RadrootsAddressableCoordinate {
    RadrootsAddressableCoordinate::parse(format!(
        "{}:{}:{}",
        radroots_event::kinds::KIND_FARM,
        "a".repeat(64),
        d_tag
    ))
    .expect("farm addr")
}

fn farm_addr() -> RadrootsAddressableCoordinate {
    farm_addr_for("AAAAAAAAAAAAAAAAAAAAAA")
}

fn private_location_record() -> SdkPrivateFarmLocationRecord {
    SdkPrivateFarmLocationRecord {
        farm_addr: farm_addr(),
        farm_pubkey: "a".repeat(64),
        farm_d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_owned(),
        label: Some("Main pickup point".to_owned()),
        latitude: 12.26,
        longitude: -34.51,
        locality_primary: "Fixture Town".to_owned(),
        locality_city: Some("Fixture Town".to_owned()),
        locality_region: Some("Fixture Region".to_owned()),
        locality_country: Some("Fixture Country".to_owned()),
        geohash5: "e4pmw".to_owned(),
        geonames_feature_id: Some(1),
        geonames_country_id: Some("FX".to_owned()),
        updated_at_ms: 1_700_000_123_000,
    }
}

#[tokio::test]
async fn private_store_file_open_rejects_directory_paths() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    assert!(matches!(
        SdkPrivateStore::open_file(tempdir.path()).await,
        Err(RadrootsSdkError::PrivateStore { .. })
    ));
}

#[tokio::test]
async fn private_store_status_update_delete_and_pragmas_round_trip() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let path = tempdir.path().join("private.sqlite");
    let store = SdkPrivateStore::open_file(&path).await.expect("open store");
    assert_eq!(store.pragma_foreign_keys().await.expect("foreign keys"), 1);
    assert_eq!(
        store.pragma_busy_timeout().await.expect("busy timeout"),
        5_000
    );
    assert_eq!(
        store.pragma_journal_mode().await.expect("journal mode"),
        "wal"
    );
    assert_eq!(
        store
            .status_summary()
            .await
            .expect("empty status")
            .farm_private_locations,
        0
    );

    let record = private_location_record();
    assert_eq!(
        store
            .farm_location(&record.farm_addr)
            .await
            .expect("missing lookup"),
        None
    );
    store
        .upsert_farm_location(&record)
        .await
        .expect("insert private location");
    assert_private_farm_location_payload_is_encrypted(&store, &record).await;
    assert_eq!(
        store
            .status_summary()
            .await
            .expect("inserted status")
            .farm_private_locations,
        1
    );
    assert_eq!(
        store
            .farm_location(&record.farm_addr)
            .await
            .expect("stored lookup"),
        Some(record.clone())
    );

    let mut updated = record.clone();
    updated.label = None;
    updated.latitude = 12.5;
    updated.longitude = -34.75;
    updated.locality_primary = "Updated Town".to_owned();
    updated.locality_city = Some("Updated Town".to_owned());
    updated.geonames_feature_id = Some(2);
    updated.updated_at_ms = 1_700_000_124_000;
    store
        .upsert_farm_location(&updated)
        .await
        .expect("update private location");
    assert_eq!(
        store
            .farm_location(&record.farm_addr)
            .await
            .expect("updated lookup"),
        Some(updated.clone())
    );

    let missing_addr = farm_addr_for("AAAAAAAAAAAAAAAAAAAAAQ");
    assert!(
        !store
            .delete_farm_location(&missing_addr)
            .await
            .expect("delete missing")
    );
    assert!(
        store
            .delete_farm_location(&updated.farm_addr)
            .await
            .expect("delete stored")
    );
    assert!(
        !store
            .delete_farm_location(&updated.farm_addr)
            .await
            .expect("delete already cleared")
    );
    assert_eq!(
        store
            .status_summary()
            .await
            .expect("cleared status")
            .farm_private_locations,
        0
    );
}

#[tokio::test]
async fn private_store_file_open_rejects_pre_v1_schema_without_repair() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let path = tempdir.path().join("private.sqlite");
    let options = SqliteConnectOptions::new()
        .filename(&path)
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("old private store pool");
    sqlx::raw_sql(
        r#"
        CREATE TABLE sdk_private_farm_location (
          farm_addr TEXT PRIMARY KEY NOT NULL,
          farm_pubkey TEXT NOT NULL,
          farm_d_tag TEXT NOT NULL,
          latitude REAL NOT NULL CHECK(latitude >= -90.0 AND latitude <= 90.0),
          longitude REAL NOT NULL CHECK(longitude >= -180.0 AND longitude <= 180.0),
          locality_primary TEXT NOT NULL,
          locality_city TEXT,
          locality_region TEXT,
          locality_country TEXT,
          geohash5 TEXT NOT NULL CHECK(length(geohash5) = 5),
          geonames_feature_id INTEGER,
          geonames_country_id TEXT,
          updated_at_ms INTEGER NOT NULL
        );
        "#,
    )
    .execute(&pool)
    .await
    .expect("old private store schema");
    pool.close().await;

    assert!(matches!(
        SdkPrivateStore::open_file(&path).await,
        Err(RadrootsSdkError::UnsupportedProfileSchema { .. })
    ));
}

#[tokio::test]
async fn private_store_schema_uses_v1_private_authority_tables() {
    let store = SdkPrivateStore::open_memory().await.expect("private store");
    let tables = sqlx::query(
        r#"
        SELECT name FROM sqlite_master
        WHERE type = 'table'
        ORDER BY name
        "#,
    )
    .fetch_all(store.pool())
    .await
    .expect("tables")
    .into_iter()
    .map(|row| row.try_get::<String, _>("name").expect("name"))
    .collect::<Vec<_>>();

    for table in [
        "cursor_hmac_key",
        "key_rotation_progress",
        "nip46_session_private",
        "private_farm_location",
        "private_metadata",
        "private_trade_artifacts",
        "wrapped_profile_key",
        "wrapped_signing_secret",
    ] {
        assert!(tables.iter().any(|name| name == table), "missing {table}");
    }
    assert!(
        !tables
            .iter()
            .any(|name| name == "sdk_private_farm_location")
    );

    let columns = sqlx::query("PRAGMA table_info(private_farm_location)")
        .fetch_all(store.pool())
        .await
        .expect("columns")
        .into_iter()
        .map(|row| row.try_get::<String, _>("name").expect("column name"))
        .collect::<Vec<_>>();
    for forbidden in [
        "label",
        "latitude",
        "longitude",
        "locality_primary",
        "locality_city",
        "locality_region",
        "locality_country",
        "geohash5",
        "geonames_feature_id",
        "geonames_country_id",
    ] {
        assert!(
            !columns.iter().any(|column| column == forbidden),
            "private_farm_location must not retain plaintext column {forbidden}"
        );
    }
}

async fn assert_private_farm_location_payload_is_encrypted(
    store: &SdkPrivateStore,
    record: &SdkPrivateFarmLocationRecord,
) {
    let row = sqlx::query(
        r#"
        SELECT ciphertext, nonce
        FROM private_farm_location
        WHERE farm_kind = 30340
        "#,
    )
    .fetch_one(store.pool())
    .await
    .expect("encrypted private location row");
    let ciphertext: Vec<u8> = row.try_get("ciphertext").expect("ciphertext");
    let nonce: Vec<u8> = row.try_get("nonce").expect("nonce");
    assert_eq!(nonce.len(), 24);
    let rendered = String::from_utf8_lossy(ciphertext.as_slice());
    let forbidden_values = vec![
        record.label.clone().expect("label"),
        record.latitude.to_string(),
        record.longitude.to_string(),
        record.locality_primary.clone(),
        record.locality_city.clone().expect("city"),
        record.locality_region.clone().expect("region"),
        record.locality_country.clone().expect("country"),
        record.geohash5.clone(),
        record
            .geonames_country_id
            .clone()
            .expect("geonames country"),
    ];
    for forbidden in forbidden_values {
        assert!(
            !rendered.contains(forbidden.as_str()),
            "encrypted private location payload leaked {forbidden}"
        );
    }
}
