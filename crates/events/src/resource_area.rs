#![forbid(unsafe_code)]

use crate::farm::RadrootsGcsLocation;

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsResourceArea {
    pub d_tag: String,
    pub name: String,
    pub about: Option<String>,
    pub location: RadrootsResourceAreaLocation,
    pub tags: Option<Vec<String>>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsResourceAreaRef {
    pub pubkey: String,
    pub d_tag: String,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsResourceAreaLocation {
    pub primary: Option<String>,
    pub city: Option<String>,
    pub region: Option<String>,
    pub country: Option<String>,
    pub gcs: RadrootsGcsLocation,
}
