use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct GeocoderPoint {
    pub lat: f64,
    pub lng: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct GeocoderReverseOptions {
    pub limit: usize,
    pub degree_offset: f64,
}

impl Default for GeocoderReverseOptions {
    fn default() -> Self {
        Self {
            limit: 1,
            degree_offset: 0.5,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GeocoderReverseResult {
    pub id: i64,
    pub name: String,
    pub admin1_id: Option<i64>,
    pub admin1_name: Option<String>,
    pub country_id: String,
    pub country_name: Option<String>,
    pub latitude: f64,
    pub longitude: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GeocoderCountryListResult {
    pub country_id: String,
    pub country: Option<String>,
    pub lat: f64,
    pub lng: f64,
}
