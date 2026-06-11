#![forbid(unsafe_code)]

use crate::farm::RadrootsGcsLocation;

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsCoop {
    pub d_tag: String,
    pub name: String,
    pub about: Option<String>,
    pub website: Option<String>,
    pub picture: Option<String>,
    pub banner: Option<String>,
    pub location: Option<RadrootsCoopLocation>,
    pub tags: Option<Vec<String>>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsCoopRef {
    pub pubkey: String,
    pub d_tag: String,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsCoopLocation {
    pub primary: Option<String>,
    pub city: Option<String>,
    pub region: Option<String>,
    pub country: Option<String>,
    pub gcs: RadrootsGcsLocation,
}
