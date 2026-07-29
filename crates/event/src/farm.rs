//! Farm, cooperative, plot, resource, and farm-document event models.

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug)]
pub struct RadrootsFarm {
    pub d_tag: String,
    pub name: String,
    pub about: Option<String>,
    pub website: Option<String>,
    pub picture: Option<String>,
    pub banner: Option<String>,
    pub location: Option<RadrootsFarmPublicLocation>,
    pub tags: Option<Vec<String>>,
}

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug, Default)]
pub struct RadrootsFarmRef {
    pub pubkey: String,
    pub d_tag: String,
}

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug)]
pub struct RadrootsFarmPublicLocation {
    pub primary: String,
    pub city: Option<String>,
    pub region: Option<String>,
    pub country: Option<String>,
    pub geohash: String,
}
#[path = "gcs.rs"]
pub mod change_set;
#[path = "coop.rs"]
pub mod coop;
#[path = "farm_crdt.rs"]
pub mod crdt;
#[path = "farm_file.rs"]
pub mod file;
#[path = "location.rs"]
pub mod location;
#[path = "plot.rs"]
pub mod plot;
#[path = "resource_area.rs"]
pub mod resource_area;
#[path = "resource_cap.rs"]
pub mod resource_cap;
#[path = "farm_workspace.rs"]
pub mod workspace;
