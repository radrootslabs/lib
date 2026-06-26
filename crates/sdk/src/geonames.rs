#![cfg(feature = "runtime")]

use std::path::{Path, PathBuf};

pub use radroots_geocoder::{
    GEONAMES_1_0_ASSET, GEONAMES_ASSET_BYTE_SIZE, GEONAMES_ASSET_FILE_NAME, GEONAMES_ASSET_HOST,
    GEONAMES_ASSET_SHA256, GEONAMES_ASSET_URL, GEONAMES_ASSET_VERSION, GeoNamesAssetFetcher,
    GeoNamesAssetSpec, GeoNamesAssetState, GeoNamesAssetStatus, GeoNamesBlockingHttpFetcher,
    Geocoder, GeocoderCountryListResult, GeocoderError, GeocoderPoint, GeocoderReverseOptions,
    GeocoderReverseResult,
};
use radroots_geocoder::{
    ensure_default_geonames_asset_in_cache_root, ensure_geonames_asset_in_cache_root_with_fetcher,
    inspect_default_geonames_asset_in_cache_root, inspect_geonames_asset_path,
};
use radroots_runtime_paths::{
    default_shared_geonames_database_path_from_cache_root,
    default_shared_geonames_root_from_cache_root,
};

pub fn radroots_sdk_geonames_database_path_from_cache_root(
    cache_root: impl AsRef<Path>,
) -> PathBuf {
    default_shared_geonames_database_path_from_cache_root(cache_root, GEONAMES_ASSET_VERSION)
}

pub fn radroots_sdk_geonames_database_path_from_cache_root_for_version(
    cache_root: impl AsRef<Path>,
    version: &str,
) -> PathBuf {
    default_shared_geonames_database_path_from_cache_root(cache_root, version)
}

pub fn radroots_sdk_geonames_root_from_cache_root(cache_root: impl AsRef<Path>) -> PathBuf {
    default_shared_geonames_root_from_cache_root(cache_root)
}

pub fn radroots_sdk_inspect_geonames_database_in_cache_root(
    cache_root: impl AsRef<Path>,
) -> Result<GeoNamesAssetStatus, GeocoderError> {
    inspect_default_geonames_asset_in_cache_root(cache_root)
}

pub fn radroots_sdk_inspect_geonames_database_path_with_spec(
    path: impl AsRef<Path>,
    spec: &GeoNamesAssetSpec,
) -> Result<GeoNamesAssetStatus, GeocoderError> {
    inspect_geonames_asset_path(path, spec)
}

pub fn radroots_sdk_ensure_geonames_database_in_cache_root(
    cache_root: impl AsRef<Path>,
) -> Result<GeoNamesAssetStatus, GeocoderError> {
    ensure_default_geonames_asset_in_cache_root(cache_root)
}

pub fn radroots_sdk_ensure_geonames_database_in_cache_root_with_fetcher<F>(
    cache_root: impl AsRef<Path>,
    fetcher: &F,
) -> Result<GeoNamesAssetStatus, GeocoderError>
where
    F: GeoNamesAssetFetcher,
{
    ensure_geonames_asset_in_cache_root_with_fetcher(cache_root, &GEONAMES_1_0_ASSET, fetcher)
}

pub fn radroots_sdk_ensure_geonames_database_in_cache_root_with_spec_and_fetcher<F>(
    cache_root: impl AsRef<Path>,
    spec: &GeoNamesAssetSpec,
    fetcher: &F,
) -> Result<GeoNamesAssetStatus, GeocoderError>
where
    F: GeoNamesAssetFetcher,
{
    ensure_geonames_asset_in_cache_root_with_fetcher(cache_root, spec, fetcher)
}

pub fn radroots_sdk_open_verified_geonames_database(
    path: impl AsRef<Path>,
) -> Result<Geocoder, GeocoderError> {
    Geocoder::open_verified_geonames_asset(path, &GEONAMES_1_0_ASSET)
}

pub fn radroots_sdk_open_verified_geonames_database_with_spec(
    path: impl AsRef<Path>,
    spec: &GeoNamesAssetSpec,
) -> Result<Geocoder, GeocoderError> {
    Geocoder::open_verified_geonames_asset(path, spec)
}
