use radroots_core::{
    RadrootsCoreDiscountValue, RadrootsCoreMoney, RadrootsCorePercent, RadrootsCoreQuantity,
    RadrootsCoreQuantityPrice,
};
use serde::{Deserialize, Serialize};

use crate::RadrootsNostrEvent;

#[typeshare::typeshare]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsListingEventIndex {
    pub event: RadrootsNostrEvent,
    pub metadata: RadrootsListingEventMetadata,
}

#[typeshare::typeshare]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsListingEventMetadata {
    pub id: String,
    pub author: String,
    pub published_at: u32,
    pub kind: u32,
    pub listing: RadrootsListing,
}

#[typeshare::typeshare]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsListing {
    pub d_tag: String,
    pub product: RadrootsListingProduct,
    pub quantities: Vec<RadrootsListingQuantity>,
    pub prices: Vec<RadrootsListingPrice>,
    pub discounts: Option<Vec<RadrootsListingDiscount>>,
    pub location: Option<RadrootsListingLocation>,
    pub images: Option<Vec<RadrootsListingImage>>,
}

#[typeshare::typeshare]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsListingProduct {
    pub key: String,
    pub title: String,
    pub category: String,
    pub summary: Option<String>,
    pub process: Option<String>,
    pub lot: Option<String>,
    pub location: Option<String>,
    pub profile: Option<String>,
    pub year: Option<String>,
}

#[typeshare::typeshare]
pub type RadrootsListingPrice = RadrootsCoreQuantityPrice;

#[typeshare::typeshare]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsListingQuantity {
    pub value: RadrootsCoreQuantity,
    pub label: Option<String>,
    pub count: Option<u32>,
}

#[typeshare::typeshare]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "amount")]
pub enum RadrootsListingDiscount {
    Quantity {
        ref_quantity: String,
        threshold: RadrootsCoreQuantity,
        value: RadrootsCoreMoney,
    },
    Mass {
        threshold: RadrootsCoreQuantity,
        value: RadrootsCoreMoney,
    },
    Subtotal {
        threshold: RadrootsCoreMoney,
        value: RadrootsCoreDiscountValue,
    },
    Total {
        total_min: RadrootsCoreMoney,
        value: RadrootsCorePercent,
    },
}

#[typeshare::typeshare]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsListingLocation {
    pub primary: String,
    pub city: Option<String>,
    pub region: Option<String>,
    pub country: Option<String>,
    pub lat: Option<f64>,
    pub lng: Option<f64>,
    pub geohash: Option<String>,
}

#[typeshare::typeshare]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsListingImage {
    pub url: String,
    pub size: Option<RadrootsListingImageSize>,
}

#[typeshare::typeshare]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsListingImageSize {
    pub w: u32,
    pub h: u32,
}
