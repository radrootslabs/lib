#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsFollow {
    pub list: Vec<RadrootsFollowProfile>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsFollowProfile {
    pub published_at: u32,
    pub public_key: String,
    pub relay_url: Option<String>,
    pub contact_name: Option<String>,
}
