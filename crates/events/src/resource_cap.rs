#![forbid(unsafe_code)]

use radroots_core::{RadrootsCoreDecimal, RadrootsCoreQuantity, RadrootsCoreUnit};

use crate::RadrootsNostrEvent;
use crate::resource_area::RadrootsResourceAreaRef;
#[cfg(feature = "ts-rs")]
use ts_rs::TS;

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsResourceHarvestCapEventIndex {
    pub event: RadrootsNostrEvent,
    pub metadata: RadrootsResourceHarvestCapEventMetadata,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsResourceHarvestCapEventMetadata {
    pub id: String,
    pub author: String,
    pub published_at: u32,
    pub kind: u32,
    pub cap: RadrootsResourceHarvestCap,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsResourceHarvestProduct {
    pub key: String,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub category: Option<String>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsResourceHarvestCap {
    pub d_tag: String,
    pub resource_area: RadrootsResourceAreaRef,
    pub product: RadrootsResourceHarvestProduct,
    pub start: u64,
    pub end: u64,
    #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsCoreQuantity"))]
    pub cap_quantity: RadrootsCoreQuantity,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "RadrootsCoreDecimal | null"))]
    pub display_amount: Option<RadrootsCoreDecimal>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "RadrootsCoreUnit | null"))]
    pub display_unit: Option<RadrootsCoreUnit>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub display_label: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string[] | null"))]
    pub tags: Option<Vec<String>>,
}
