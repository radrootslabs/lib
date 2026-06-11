#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
use alloc::string::String;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsGeoChat {
    pub geohash: String,
    pub content: String,
    pub nickname: Option<String>,
    pub teleported: bool,
}
