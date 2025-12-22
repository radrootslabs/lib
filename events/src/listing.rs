use radroots_core::{
    RadrootsCoreDiscountValue, RadrootsCoreMoney, RadrootsCorePercent, RadrootsCoreQuantity,
    RadrootsCoreQuantityPrice,
};
#[cfg(feature = "ts-rs")]
use ts_rs::TS;

use crate::RadrootsNostrEvent;

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsListingEventIndex {
    pub event: RadrootsNostrEvent,
    pub metadata: RadrootsListingEventMetadata,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsListingEventMetadata {
    pub id: String,
    pub author: String,
    pub published_at: u32,
    pub kind: u32,
    pub listing: RadrootsListing,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsListing {
    pub d_tag: String,
    pub product: RadrootsListingProduct,
    pub quantities: Vec<RadrootsListingQuantity>,
    #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsCoreQuantityPrice[]"))]
    pub prices: Vec<RadrootsCoreQuantityPrice>,
    #[cfg_attr(
        feature = "ts-rs",
        ts(optional, type = "RadrootsListingDiscount[] | null")
    )]
    pub discounts: Option<Vec<RadrootsListingDiscount>>,
    #[cfg_attr(
        feature = "ts-rs",
        ts(optional, type = "RadrootsListingLocation | null")
    )]
    pub location: Option<RadrootsListingLocation>,
    #[cfg_attr(
        feature = "ts-rs",
        ts(optional, type = "RadrootsListingImage[] | null")
    )]
    pub images: Option<Vec<RadrootsListingImage>>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsListingProduct {
    pub key: String,
    pub title: String,
    pub category: String,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub summary: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub process: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub lot: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub location: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub profile: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub year: Option<String>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsListingQuantity {
    #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsCoreQuantity"))]
    pub value: RadrootsCoreQuantity,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub label: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "number | null"))]
    pub count: Option<u32>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case", tag = "kind", content = "amount"))]
#[derive(Clone, Debug)]
pub enum RadrootsListingDiscount {
    Quantity {
        ref_quantity: String,
        #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsCoreQuantity"))]
        threshold: RadrootsCoreQuantity,
        #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsCoreMoney"))]
        value: RadrootsCoreMoney,
    },
    Mass {
        #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsCoreQuantity"))]
        threshold: RadrootsCoreQuantity,
        #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsCoreMoney"))]
        value: RadrootsCoreMoney,
    },
    Subtotal {
        #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsCoreMoney"))]
        threshold: RadrootsCoreMoney,
        #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsCoreDiscountValue"))]
        value: RadrootsCoreDiscountValue,
    },
    Total {
        #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsCoreMoney"))]
        total_min: RadrootsCoreMoney,
        #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsCorePercent"))]
        value: RadrootsCorePercent,
    },
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsListingLocation {
    pub primary: String,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub city: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub region: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub country: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "number | null"))]
    pub lat: Option<f64>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "number | null"))]
    pub lng: Option<f64>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub geohash: Option<String>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsListingImage {
    pub url: String,
    #[cfg_attr(
        feature = "ts-rs",
        ts(optional, type = "RadrootsListingImageSize | null")
    )]
    pub size: Option<RadrootsListingImageSize>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsListingImageSize {
    pub w: u32,
    pub h: u32,
}
