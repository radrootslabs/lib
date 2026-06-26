#![cfg(feature = "runtime")]

use std::path::{Path, PathBuf};

use crate::{GeoNamesClient, RadrootsSdkError};
pub use radroots_geocoder::{
    GEONAMES_1_0_ASSET, GEONAMES_ASSET_BYTE_SIZE, GEONAMES_ASSET_FILE_NAME, GEONAMES_ASSET_HOST,
    GEONAMES_ASSET_SHA256, GEONAMES_ASSET_URL, GEONAMES_ASSET_VERSION, GeoNamesAssetFetcher,
    GeoNamesAssetSpec, GeoNamesAssetState, GeoNamesAssetStatus, GeoNamesBlockingHttpFetcher,
    Geocoder, GeocoderCountryListResult, GeocoderError, GeocoderLocalityCandidate,
    GeocoderLocalityInput, GeocoderLocalityLookup, GeocoderLocalityQuery, GeocoderPoint,
    GeocoderReverseOptions, GeocoderReverseResult, GeocoderStructuredLocalityQuery,
};
use radroots_geocoder::{
    ensure_default_geonames_asset_in_cache_root, ensure_geonames_asset_in_cache_root_with_fetcher,
    inspect_default_geonames_asset_in_cache_root, inspect_geonames_asset_path,
};
use radroots_runtime_paths::{
    default_shared_geonames_database_path_from_cache_root,
    default_shared_geonames_root_from_cache_root,
};

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RadrootsGeoNamesConfig {
    pub cache_root: PathBuf,
}

impl RadrootsGeoNamesConfig {
    pub fn new(cache_root: impl Into<PathBuf>) -> Self {
        Self {
            cache_root: cache_root.into(),
        }
    }

    pub fn root_path(&self) -> PathBuf {
        default_shared_geonames_root_from_cache_root(&self.cache_root)
    }

    pub fn database_path(&self) -> PathBuf {
        geonames_database_path_from_cache_root(&self.cache_root)
    }

    pub fn database_path_for_version(&self, version: &str) -> PathBuf {
        geonames_database_path_from_cache_root_for_version(&self.cache_root, version)
    }
}

impl<'sdk> GeoNamesClient<'sdk> {
    pub fn config(&self) -> Option<&RadrootsGeoNamesConfig> {
        self.sdk.geonames_config()
    }

    pub fn root_path(&self) -> Result<PathBuf, RadrootsSdkError> {
        Ok(self.required_config()?.root_path())
    }

    pub fn database_path(&self) -> Result<PathBuf, RadrootsSdkError> {
        Ok(self.required_config()?.database_path())
    }

    pub fn database_path_for_version(&self, version: &str) -> Result<PathBuf, RadrootsSdkError> {
        Ok(self.required_config()?.database_path_for_version(version))
    }

    pub fn inspect(&self) -> Result<GeoNamesAssetStatus, RadrootsSdkError> {
        inspect_default_geonames_asset_in_cache_root(&self.required_config()?.cache_root)
            .map_err(RadrootsSdkError::from)
    }

    pub fn inspect_path_with_spec(
        &self,
        path: impl AsRef<Path>,
        spec: &GeoNamesAssetSpec,
    ) -> Result<GeoNamesAssetStatus, RadrootsSdkError> {
        inspect_geonames_asset_path(path, spec).map_err(RadrootsSdkError::from)
    }

    pub fn ensure(&self) -> Result<GeoNamesAssetStatus, RadrootsSdkError> {
        ensure_default_geonames_asset_in_cache_root(&self.required_config()?.cache_root)
            .map_err(RadrootsSdkError::from)
    }

    pub fn ensure_with_fetcher<F>(
        &self,
        fetcher: &F,
    ) -> Result<GeoNamesAssetStatus, RadrootsSdkError>
    where
        F: GeoNamesAssetFetcher,
    {
        ensure_geonames_asset_in_cache_root_with_fetcher(
            &self.required_config()?.cache_root,
            &GEONAMES_1_0_ASSET,
            fetcher,
        )
        .map_err(RadrootsSdkError::from)
    }

    pub fn ensure_with_spec_and_fetcher<F>(
        &self,
        spec: &GeoNamesAssetSpec,
        fetcher: &F,
    ) -> Result<GeoNamesAssetStatus, RadrootsSdkError>
    where
        F: GeoNamesAssetFetcher,
    {
        ensure_geonames_asset_in_cache_root_with_fetcher(
            &self.required_config()?.cache_root,
            spec,
            fetcher,
        )
        .map_err(RadrootsSdkError::from)
    }

    pub fn open_verified(&self) -> Result<Geocoder, RadrootsSdkError> {
        Geocoder::open_verified_geonames_asset(self.database_path()?, &GEONAMES_1_0_ASSET)
            .map_err(RadrootsSdkError::from)
    }

    pub fn open_verified_path_with_spec(
        &self,
        path: impl AsRef<Path>,
        spec: &GeoNamesAssetSpec,
    ) -> Result<Geocoder, RadrootsSdkError> {
        Geocoder::open_verified_geonames_asset(path, spec).map_err(RadrootsSdkError::from)
    }

    fn required_config(&self) -> Result<&RadrootsGeoNamesConfig, RadrootsSdkError> {
        self.config()
            .ok_or_else(RadrootsSdkError::missing_geonames_config)
    }
}

fn geonames_database_path_from_cache_root(cache_root: impl AsRef<Path>) -> PathBuf {
    default_shared_geonames_database_path_from_cache_root(cache_root, GEONAMES_ASSET_VERSION)
}

fn geonames_database_path_from_cache_root_for_version(
    cache_root: impl AsRef<Path>,
    version: &str,
) -> PathBuf {
    default_shared_geonames_database_path_from_cache_root(cache_root, version)
}
