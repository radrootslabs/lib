use crate::social::list::ListEntry;

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

#[cfg_attr(all(test, feature = "std"), derive(dto_bindgen::Dto))]
#[cfg_attr(all(test, feature = "std"), dto(export))]
#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug)]
pub struct ListSet {
    pub d_tag: String,
    pub content: String,
    pub entries: Vec<ListEntry>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub image: Option<String>,
}
