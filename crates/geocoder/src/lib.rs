#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![forbid(unsafe_code)]

mod asset;
mod error;
mod geocoder;
mod model;

pub use asset::{
    GEONAMES_1_0_ASSET, GEONAMES_ASSET_BYTE_SIZE, GEONAMES_ASSET_FILE_NAME, GEONAMES_ASSET_HOST,
    GEONAMES_ASSET_SHA256, GEONAMES_ASSET_URL, GEONAMES_ASSET_VERSION, GeoNamesAssetFetcher,
    GeoNamesAssetSpec, GeoNamesAssetState, GeoNamesAssetStatus, GeoNamesBlockingHttpFetcher,
    default_geonames_asset_path_from_cache_root, ensure_default_geonames_asset_in_cache_root,
    ensure_geonames_asset_in_cache_root_with_fetcher, ensure_geonames_asset_path_with_fetcher,
    inspect_default_geonames_asset_in_cache_root, inspect_geonames_asset_path,
    validate_geonames_asset_file, validate_geonames_asset_spec_source,
};
pub use error::GeocoderError;
pub use geocoder::Geocoder;
pub use model::{
    GeocoderCountryListResult, GeocoderPoint, GeocoderReverseOptions, GeocoderReverseResult,
};
