#![cfg(feature = "runtime")]

use radroots_sdk::{
    GEONAMES_ASSET_HOST, GEONAMES_ASSET_VERSION, GeoNamesAssetState,
    radroots_sdk_geonames_database_path_from_cache_root,
    radroots_sdk_geonames_database_path_from_cache_root_for_version,
    radroots_sdk_geonames_root_from_cache_root,
    radroots_sdk_inspect_geonames_database_in_cache_root,
};

#[test]
fn sdk_geonames_facade_resolves_shared_cache_paths_and_reports_missing_state() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let cache_root = tempdir.path().join("cache");

    assert_eq!(GEONAMES_ASSET_HOST, "assets.radroots.io");
    assert_eq!(GEONAMES_ASSET_VERSION, "1.0");
    assert_eq!(
        radroots_sdk_geonames_root_from_cache_root(&cache_root),
        cache_root.join("shared").join("geonames")
    );
    assert_eq!(
        radroots_sdk_geonames_database_path_from_cache_root(&cache_root),
        cache_root
            .join("shared")
            .join("geonames")
            .join("geonames-1.0.db")
    );
    assert_eq!(
        radroots_sdk_geonames_database_path_from_cache_root_for_version(&cache_root, "1.1"),
        cache_root
            .join("shared")
            .join("geonames")
            .join("geonames-1.1.db")
    );

    let status =
        radroots_sdk_inspect_geonames_database_in_cache_root(&cache_root).expect("inspection");
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
