#![cfg(feature = "runtime")]

use radroots_sdk::{
    GEONAMES_ASSET_HOST, GEONAMES_ASSET_VERSION, GeoNamesAssetState, RadrootsClient,
    RadrootsGeoNamesConfig, RadrootsSdkError, RadrootsSdkErrorClass, RadrootsSdkGeoNamesErrorKind,
    RadrootsSdkRecoveryAction,
};

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
    let error = sdk
        .geonames()
        .database_path()
        .expect_err("missing geonames config");

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
