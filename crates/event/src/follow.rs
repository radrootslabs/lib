#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

#[cfg_attr(all(test, feature = "std"), derive(dto_bindgen::Dto))]
#[cfg_attr(all(test, feature = "std"), dto(export))]
#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug)]
pub struct RadrootsFollow {
    pub list: Vec<RadrootsFollowProfile>,
}

#[cfg_attr(all(test, feature = "std"), derive(dto_bindgen::Dto))]
#[cfg_attr(all(test, feature = "std"), dto(export))]
#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug)]
pub struct RadrootsFollowProfile {
    #[cfg_attr(all(test, feature = "std"), dto(int = "json_number"))]
    pub published_at: u64,
    pub public_key: String,
    pub relay_url: Option<String>,
    pub contact_name: Option<String>,
}
