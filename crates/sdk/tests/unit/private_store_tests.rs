use super::*;
use radroots_event::ids::RadrootsAddressableCoordinate;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

fn farm_addr() -> RadrootsAddressableCoordinate {
    farm_addr_for("AAAAAAAAAAAAAAAAAAAAAA")
}

fn farm_addr_for(d_tag: &str) -> RadrootsAddressableCoordinate {
    RadrootsAddressableCoordinate::parse(format!(
        "{}:{}:{}",
        radroots_event::kinds::KIND_FARM,
        "a".repeat(64),
        d_tag
    ))
    .expect("farm addr")
}

fn private_store_error_message<T>(result: Result<T, RadrootsSdkError>) -> String {
    match result.err().expect("private store error") {
        RadrootsSdkError::PrivateStore { message } => message,
        other => panic!("expected private store error, got {other:?}"),
    }
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
async fn private_farm_location_row_decode_reports_each_missing_column() {
    let store = SdkPrivateStore::open_memory().await.expect("private store");
    let columns = [
        (
            "farm_pubkey",
            "'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'",
        ),
        ("farm_d_tag", "'AAAAAAAAAAAAAAAAAAAAAA'"),
        ("label", "'Main pickup point'"),
        ("latitude", "12.26"),
        ("longitude", "-34.51"),
        ("locality_primary", "'Fixture Town'"),
        ("locality_city", "'Fixture Town'"),
        ("locality_region", "'Fixture Region'"),
        ("locality_country", "'Fixture Country'"),
        ("geohash5", "'e4pmw'"),
        ("geonames_feature_id", "1"),
        ("geonames_country_id", "'FX'"),
        ("updated_at_ms", "1700000123000"),
    ];

    for missing in columns.map(|(name, _)| name) {
        let select = columns
            .iter()
            .filter(|(name, _)| *name != missing)
            .map(|(name, value)| format!("{value} AS {name}"))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!("SELECT {select}");
        let row = sqlx::query(sqlx::AssertSqlSafe(sql))
            .fetch_one(store.pool())
            .await
            .expect("row");
        let message = private_store_error_message(private_farm_location_from_row(farm_addr(), row));
        assert!(
            message.contains(missing),
            "{message:?} should mention {missing:?}"
        );
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
async fn private_store_file_open_rejects_pre_label_schema_without_repair() {
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

    let store = SdkPrivateStore::open_file(&path).await.expect("open store");
    let record = private_location_record();
    assert!(matches!(
        store.upsert_farm_location(&record).await,
        Err(RadrootsSdkError::PrivateStore { .. })
    ));
}
