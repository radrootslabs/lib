#![forbid(unsafe_code)]

use radroots_core::{RadrootsCoreDecimal, RadrootsCoreQuantity, RadrootsCoreUnit};

use crate::resource_area::RadrootsResourceAreaRef;

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsResourceHarvestProduct {
    pub key: String,
    pub category: Option<String>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[derive(Clone, Debug)]
pub struct RadrootsResourceHarvestCap {
    pub d_tag: String,
    pub resource_area: RadrootsResourceAreaRef,
    pub product: RadrootsResourceHarvestProduct,
    #[cfg_attr(feature = "dto-bindgen", dto(int = "json_string"))]
    pub start: u64,
    #[cfg_attr(feature = "dto-bindgen", dto(int = "json_string"))]
    pub end: u64,
    pub cap_quantity: RadrootsCoreQuantity,
    pub display_amount: Option<RadrootsCoreDecimal>,
    pub display_unit: Option<RadrootsCoreUnit>,
    pub display_label: Option<String>,
    pub tags: Option<Vec<String>>,
}
