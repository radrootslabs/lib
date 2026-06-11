use crate::farm::{RadrootsFarmRef, RadrootsGcsLocation};

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsPlotRef {
    pub pubkey: String,
    pub d_tag: String,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsPlot {
    pub d_tag: String,
    pub farm: RadrootsFarmRef,
    pub name: String,
    pub about: Option<String>,
    pub location: Option<RadrootsPlotLocation>,
    pub tags: Option<Vec<String>>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsPlotLocation {
    pub primary: Option<String>,
    pub city: Option<String>,
    pub region: Option<String>,
    pub country: Option<String>,
    pub gcs: RadrootsGcsLocation,
}
