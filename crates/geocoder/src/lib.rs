#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![forbid(unsafe_code)]

mod error;
mod geocoder;
mod model;

pub use error::GeocoderError;
pub use geocoder::Geocoder;
pub use model::{
    GeocoderCountryListResult, GeocoderPoint, GeocoderReverseOptions, GeocoderReverseResult,
};
