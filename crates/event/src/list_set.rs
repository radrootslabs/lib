use crate::list::RadrootsListEntry;

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsListSet {
    pub d_tag: String,
    pub content: String,
    pub entries: Vec<RadrootsListEntry>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub image: Option<String>,
}
