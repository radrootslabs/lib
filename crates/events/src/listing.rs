use radroots_core::{
    RadrootsCoreDecimal, RadrootsCoreDiscount, RadrootsCoreMoney, RadrootsCoreQuantity,
    RadrootsCoreQuantityPrice, RadrootsCoreUnit,
};

use crate::farm::RadrootsFarmRef;
use crate::ids::{RadrootsDTag, RadrootsInventoryBinId};
use crate::plot::RadrootsPlotRef;
use crate::resource_area::RadrootsResourceAreaRef;

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(rename_all = "snake_case", tag = "kind", content = "amount")
)]
#[derive(Clone, Debug)]
pub enum RadrootsListingAvailability {
    Window {
        start: Option<u64>,
        end: Option<u64>,
    },
    Status {
        status: RadrootsListingStatus,
    },
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(rename_all = "snake_case", tag = "kind", content = "amount")
)]
#[derive(Clone, Debug)]
pub enum RadrootsListingStatus {
    Active,
    Sold,
    Other { value: String },
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(rename_all = "snake_case", tag = "kind", content = "amount")
)]
#[derive(Clone, Debug)]
pub enum RadrootsListingDeliveryMethod {
    Pickup,
    LocalDelivery,
    Shipping,
    Other { method: String },
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsListing {
    pub d_tag: RadrootsDTag,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub published_at: Option<u64>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub farm: RadrootsFarmRef,
    pub product: RadrootsListingProduct,
    pub primary_bin_id: RadrootsInventoryBinId,
    pub bins: Vec<RadrootsListingBin>,
    pub resource_area: Option<RadrootsResourceAreaRef>,
    pub plot: Option<RadrootsPlotRef>,
    pub discounts: Option<Vec<RadrootsCoreDiscount>>,
    pub inventory_available: Option<RadrootsCoreDecimal>,
    pub availability: Option<RadrootsListingAvailability>,
    pub delivery_method: Option<RadrootsListingDeliveryMethod>,
    pub location: Option<RadrootsListingLocation>,
    pub images: Option<Vec<RadrootsListingImage>>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
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

pub const RADROOTS_LISTING_PRODUCT_TAG_KEYS: [&str; 9] = [
    "key", "title", "category", "summary", "process", "lot", "location", "profile", "year",
];

pub struct RadrootsListingProductTagKeys;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsListingBin {
    pub bin_id: RadrootsInventoryBinId,
    pub quantity: RadrootsCoreQuantity,
    pub price_per_canonical_unit: RadrootsCoreQuantityPrice,
    pub display_amount: Option<RadrootsCoreDecimal>,
    pub display_unit: Option<RadrootsCoreUnit>,
    pub display_label: Option<String>,
    pub display_price: Option<RadrootsCoreMoney>,
    pub display_price_unit: Option<RadrootsCoreUnit>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsListingLocation {
    pub primary: String,
    pub city: Option<String>,
    pub region: Option<String>,
    pub country: Option<String>,
    pub lat: Option<f64>,
    pub lng: Option<f64>,
    pub geohash: Option<String>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsListingImage {
    pub url: String,
    pub size: Option<RadrootsListingImageSize>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsListingImageSize {
    pub w: u32,
    pub h: u32,
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use crate::farm::RadrootsFarmRef;

    #[test]
    fn defaults_listing_farm_ref_to_empty_values() {
        let farm_ref = RadrootsFarmRef::default();
        assert!(farm_ref.pubkey.is_empty());
        assert!(farm_ref.d_tag.is_empty());
    }

    #[test]
    fn listing_model_covers_published_draft_metadata() {
        use crate::kinds::{KIND_LISTING_DRAFT, is_listing_kind};

        let listing = super::RadrootsListing {
            d_tag: "listing-draft".parse().unwrap(),
            published_at: Some(1_700_000_000),
            farm: RadrootsFarmRef::default(),
            product: super::RadrootsListingProduct {
                key: "lettuce".to_string(),
                title: "lettuce".to_string(),
                category: "produce".to_string(),
                summary: None,
                process: None,
                lot: None,
                location: None,
                profile: None,
                year: None,
            },
            primary_bin_id: "bin-1".parse().unwrap(),
            bins: vec![],
            resource_area: None,
            plot: None,
            discounts: None,
            inventory_available: None,
            availability: None,
            delivery_method: None,
            location: None,
            images: None,
        };

        assert_eq!(listing.published_at, Some(1_700_000_000));
        assert!(is_listing_kind(KIND_LISTING_DRAFT));
    }
}
