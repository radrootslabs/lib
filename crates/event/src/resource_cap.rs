#![forbid(unsafe_code)]

use radroots_core::{Decimal, Quantity, Unit};

use crate::farm::resource_area::RadrootsResourceAreaRef;

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug)]
pub struct RadrootsResourceHarvestProduct {
    pub key: String,
    pub category: Option<String>,
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
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
    #[cfg_attr(feature = "dto-bindgen", dto(ts(type = "Quantity")))]
    pub cap_quantity: Quantity,
    #[cfg_attr(feature = "dto-bindgen", dto(ts(type = "Decimal")))]
    pub display_amount: Option<Decimal>,
    #[cfg_attr(feature = "dto-bindgen", dto(ts(type = "Unit")))]
    pub display_unit: Option<Unit>,
    pub display_label: Option<String>,
    pub tags: Option<Vec<String>>,
}
