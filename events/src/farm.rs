use crate::RadrootsNostrEvent;
#[cfg(feature = "ts-rs")]
use ts_rs::TS;

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsFarmEventIndex {
    pub event: RadrootsNostrEvent,
    pub metadata: RadrootsFarmEventMetadata,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsFarmEventMetadata {
    pub id: String,
    pub author: String,
    pub published_at: u32,
    pub kind: u32,
    pub farm: RadrootsFarm,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsFarm {
    pub d_tag: String,
    pub name: String,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub about: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub website: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub picture: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub banner: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "RadrootsFarmLocation | null"))]
    pub location: Option<RadrootsFarmLocation>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string[] | null"))]
    pub tags: Option<Vec<String>>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsFarmRef {
    pub pubkey: String,
    pub d_tag: String,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsGeoJsonPoint {
    #[cfg_attr(feature = "serde", serde(rename = "type"))]
    pub r#type: String,
    pub coordinates: [f64; 2],
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsGeoJsonPolygon {
    #[cfg_attr(feature = "serde", serde(rename = "type"))]
    pub r#type: String,
    pub coordinates: Vec<Vec<[f64; 2]>>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsGcsLocation {
    pub lat: f64,
    pub lng: f64,
    pub geohash: String,
    pub point: RadrootsGeoJsonPoint,
    pub polygon: RadrootsGeoJsonPolygon,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "number | null"))]
    pub accuracy: Option<f64>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "number | null"))]
    pub altitude: Option<f64>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub tag_0: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub label: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "number | null"))]
    pub area: Option<f64>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "number | null"))]
    pub elevation: Option<u32>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub soil: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub climate: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub gc_id: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub gc_name: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub gc_admin1_id: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub gc_admin1_name: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub gc_country_id: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub gc_country_name: Option<String>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsFarmLocation {
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub primary: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub city: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub region: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub country: Option<String>,
    pub gcs: RadrootsGcsLocation,
}
