#![cfg(feature = "runtime")]

use radroots_sdk::{
    GEONAMES_ASSET_HOST, GEONAMES_ASSET_VERSION, GeoNamesAssetFetcher, GeoNamesAssetSpec,
    GeoNamesAssetState, GeocoderError, RadrootsClient, RadrootsGeoNamesConfig, RadrootsSdkError,
    RadrootsSdkErrorClass, RadrootsSdkGeoNamesErrorKind, RadrootsSdkRecoveryAction,
};

const TEST_SPEC: GeoNamesAssetSpec = GeoNamesAssetSpec {
    version: "test",
    file_name: "geonames-test.db",
    url: "https://assets.radroots.io/data/geonames/geonames-test.db",
    allowed_host: "assets.radroots.io",
    byte_size: 4,
    sha256: "53bc5cce8c5764019bb4ce6e597ec3885b71608668c9b6ef4940d364d7a914fa",
};

const BAD_HOST_SPEC: GeoNamesAssetSpec = GeoNamesAssetSpec {
    version: "bad-host",
    file_name: "geonames-bad-host.db",
    url: "https://example.com/data/geonames/geonames-bad-host.db",
    allowed_host: "assets.radroots.io",
    byte_size: 4,
    sha256: "53bc5cce8c5764019bb4ce6e597ec3885b71608668c9b6ef4940d364d7a914fa",
};

struct BytesFetcher(Vec<u8>);

impl GeoNamesAssetFetcher for BytesFetcher {
    fn fetch(&self, _url: &str) -> Result<Vec<u8>, GeocoderError> {
        Ok(self.0.clone())
    }
}

fn geonames_error<T>(result: Result<T, RadrootsSdkError>) -> RadrootsSdkError {
    match result {
        Ok(_) => panic!("expected GeoNames error"),
        Err(error) => error,
    }
}

#[tokio::test]
async fn sdk_geonames_client_resolves_shared_cache_paths_and_reports_missing_state() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let cache_root = tempdir.path().join("cache");
    let sdk = RadrootsClient::builder()
        .geonames_cache_root(cache_root.clone())
        .build()
        .await
        .expect("sdk");
    let geonames = sdk.geonames();

    assert_eq!(GEONAMES_ASSET_HOST, "assets.radroots.io");
    assert_eq!(GEONAMES_ASSET_VERSION, "1.0");
    assert_eq!(
        sdk.geonames_config(),
        Some(&RadrootsGeoNamesConfig::new(cache_root.clone()))
    );
    assert_eq!(
        geonames.root_path().expect("geonames root"),
        cache_root.join("shared").join("geonames")
    );
    assert_eq!(
        geonames.database_path().expect("geonames database path"),
        cache_root
            .join("shared")
            .join("geonames")
            .join("geonames-1.0.db")
    );
    assert_eq!(
        geonames
            .database_path_for_version("1.1")
            .expect("geonames version path"),
        cache_root
            .join("shared")
            .join("geonames")
            .join("geonames-1.1.db")
    );

    let status = geonames.inspect().expect("inspection");
    assert_eq!(status.state, GeoNamesAssetState::Missing);
    assert_eq!(status.version, "1.0");
    assert_eq!(
        status.path,
        cache_root
            .join("shared")
            .join("geonames")
            .join("geonames-1.0.db")
    );
}

#[tokio::test]
async fn sdk_geonames_client_reports_missing_config_as_structured_error() {
    let sdk = RadrootsClient::builder().build().await.expect("sdk");
    let geonames = sdk.geonames();
    assert!(geonames.config().is_none());
    for error in [
        geonames.root_path().expect_err("missing root path config"),
        geonames
            .database_path()
            .expect_err("missing geonames config"),
        geonames
            .database_path_for_version("1.1")
            .expect_err("missing version path config"),
        geonames.inspect().expect_err("missing inspect config"),
        geonames.ensure().expect_err("missing ensure config"),
        geonames
            .ensure_with_fetcher(&BytesFetcher(b"bad!".to_vec()))
            .expect_err("missing ensure fetcher config"),
        geonames
            .ensure_with_spec_and_fetcher(&TEST_SPEC, &BytesFetcher(b"bad!".to_vec()))
            .expect_err("missing ensure spec config"),
    ] {
        assert_missing_config_error(error);
    }

    let error = geonames
        .database_path()
        .expect_err("missing geonames config");

    assert_missing_config_error(error);
}

fn assert_missing_config_error(error: RadrootsSdkError) {
    match &error {
        RadrootsSdkError::GeoNames { kind, .. } => {
            assert_eq!(*kind, RadrootsSdkGeoNamesErrorKind::Configuration);
        }
        other => panic!("expected geonames config error, got {other}"),
    }
    assert_eq!(error.code(), "geonames_configuration");
    assert_eq!(error.class(), RadrootsSdkErrorClass::Configuration);
    assert!(!error.retryable());
    assert_eq!(
        error.recovery_actions(),
        vec![RadrootsSdkRecoveryAction::ConfigureGeoNamesCache]
    );
    assert_eq!(error.detail_json()["detail"]["kind"], "configuration");
}

#[tokio::test]
async fn sdk_geonames_client_inspects_and_ensures_versioned_assets_without_network() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let cache_root = tempdir.path().join("cache");
    let sdk = RadrootsClient::builder()
        .geonames_config(RadrootsGeoNamesConfig::new(cache_root.clone()))
        .build()
        .await
        .expect("sdk");
    let geonames = sdk.geonames();
    let custom_path = cache_root.join("custom").join(TEST_SPEC.file_name);

    let missing = geonames
        .inspect_path_with_spec(&custom_path, &TEST_SPEC)
        .expect("missing inspect");
    assert_eq!(missing.state, GeoNamesAssetState::Missing);
    assert_eq!(missing.version, "test");
    assert_eq!(missing.path, custom_path);

    let default_ensure = geonames
        .ensure_with_fetcher(&BytesFetcher(b"bad!".to_vec()))
        .expect_err("default ensure invalid sqlite");
    assert!(matches!(
        default_ensure,
        RadrootsSdkError::GeoNames {
            kind: RadrootsSdkGeoNamesErrorKind::Integrity,
            ..
        }
    ));

    let ensure_error = geonames
        .ensure_with_spec_and_fetcher(&TEST_SPEC, &BytesFetcher(b"bad!".to_vec()))
        .expect_err("ensure invalid sqlite");
    assert!(matches!(
        ensure_error,
        RadrootsSdkError::GeoNames {
            kind: RadrootsSdkGeoNamesErrorKind::Integrity,
            ..
        }
    ));

    std::fs::create_dir_all(custom_path.parent().expect("custom parent")).expect("custom parent");
    std::fs::write(&custom_path, b"bad!").expect("invalid sqlite");
    let invalid = geonames
        .inspect_path_with_spec(&custom_path, &TEST_SPEC)
        .expect("invalid inspect");
    assert_eq!(invalid.state, GeoNamesAssetState::Invalid);
    assert_eq!(invalid.byte_size, Some(4));
    assert_eq!(
        invalid.sha256.as_deref(),
        Some("53bc5cce8c5764019bb4ce6e597ec3885b71608668c9b6ef4940d364d7a914fa")
    );
    assert!(
        invalid
            .validation_error
            .expect("validation error")
            .contains("SQLite")
    );

    let open_error =
        geonames_error(geonames.open_verified_path_with_spec(&custom_path, &TEST_SPEC));
    assert!(matches!(
        open_error,
        RadrootsSdkError::GeoNames {
            kind: RadrootsSdkGeoNamesErrorKind::Integrity,
            ..
        }
    ));

    let default_open_error = geonames_error(geonames.open_verified());
    assert!(matches!(
        default_open_error,
        RadrootsSdkError::GeoNames {
            kind: RadrootsSdkGeoNamesErrorKind::Cache,
            ..
        }
    ));

    let config_error = geonames
        .ensure_with_spec_and_fetcher(&BAD_HOST_SPEC, &BytesFetcher(b"bad!".to_vec()))
        .expect_err("bad host");
    assert!(matches!(
        config_error,
        RadrootsSdkError::GeoNames {
            kind: RadrootsSdkGeoNamesErrorKind::Configuration,
            ..
        }
    ));
}
