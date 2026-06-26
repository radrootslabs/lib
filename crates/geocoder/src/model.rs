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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GeocoderLocalityCandidate {
    pub id: i64,
    pub name: String,
    pub admin1_id: Option<i64>,
    pub admin1_name: Option<String>,
    pub country_id: String,
    pub country_name: Option<String>,
    pub point: GeocoderPoint,
    pub display_name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeocoderStructuredLocalityQuery {
    pub locality: String,
    pub region: Option<String>,
    pub country: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GeocoderLocalityInput {
    Structured(GeocoderStructuredLocalityQuery),
    Query(String),
    FeatureId(i64),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeocoderLocalityQuery {
    pub input: GeocoderLocalityInput,
    pub limit: usize,
}

impl GeocoderLocalityQuery {
    pub fn structured(locality: impl Into<String>) -> Self {
        Self {
            input: GeocoderLocalityInput::Structured(GeocoderStructuredLocalityQuery {
                locality: locality.into(),
                region: None,
                country: None,
            }),
            limit: 10,
        }
    }

    pub fn query(query: impl Into<String>) -> Self {
        Self {
            input: GeocoderLocalityInput::Query(query.into()),
            limit: 10,
        }
    }

    pub fn feature_id(id: i64) -> Self {
        Self {
            input: GeocoderLocalityInput::FeatureId(id),
            limit: 10,
        }
    }

    pub fn with_region(mut self, region: impl Into<String>) -> Self {
        if let GeocoderLocalityInput::Structured(query) = &mut self.input {
            query.region = Some(region.into());
        }
        self
    }

    pub fn with_country(mut self, country: impl Into<String>) -> Self {
        if let GeocoderLocalityInput::Structured(query) = &mut self.input {
            query.country = Some(country.into());
        }
        self
    }

    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum GeocoderLocalityLookup {
    Unique {
        candidate: GeocoderLocalityCandidate,
    },
    NoMatch,
    Ambiguous {
        candidates: Vec<GeocoderLocalityCandidate>,
    },
}
