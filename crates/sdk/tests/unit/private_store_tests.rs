use super::*;
use radroots_events::ids::RadrootsAddressableCoordinate;

fn farm_addr() -> RadrootsAddressableCoordinate {
    RadrootsAddressableCoordinate::parse(format!(
        "{}:{}:{}",
        radroots_events::kinds::KIND_FARM,
        "a".repeat(64),
        "AAAAAAAAAAAAAAAAAAAAAA"
    ))
    .expect("farm addr")
}

fn private_store_error_message<T>(result: Result<T, RadrootsSdkError>) -> String {
    match result.err().expect("private store error") {
        RadrootsSdkError::PrivateStore { message } => message,
        other => panic!("expected private store error, got {other:?}"),
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
        let row = sqlx::query(sql.as_str())
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
