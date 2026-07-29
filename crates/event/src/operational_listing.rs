use radroots_core::pricing::Discount;
use radroots_core::{Decimal, Money, Quantity, QuantityPrice, Unit};

use crate::farm::FarmRef;
use crate::farm::plot::PlotRef;
use crate::farm::resource_area::ResourceAreaRef;
use crate::id::{DTag, InventoryBinId};

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(all(test, feature = "std"), derive(dto_bindgen::Dto))]
#[cfg_attr(all(test, feature = "std"), dto(export))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OperationalListingParseError {
    InvalidKind(u32),
    MissingTag(String),
    InvalidTag(String),
    InvalidNumber(String),
    InvalidUnit,
    InvalidCurrency,
    InvalidJson(String),
    InvalidDiscount(String),
}

impl core::fmt::Display for OperationalListingParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidKind(kind) => write!(f, "invalid operational listing kind: {kind}"),
            Self::MissingTag(tag) => write!(f, "missing required tag: {tag}"),
            Self::InvalidTag(tag) => write!(f, "invalid tag: {tag}"),
            Self::InvalidNumber(field) => write!(f, "invalid number: {field}"),
            Self::InvalidUnit => write!(f, "invalid unit"),
            Self::InvalidCurrency => write!(f, "invalid currency"),
            Self::InvalidJson(field) => write!(f, "invalid json: {field}"),
            Self::InvalidDiscount(kind) => write!(f, "invalid discount data for {kind}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for OperationalListingParseError {}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(all(test, feature = "std"), derive(dto_bindgen::Dto))]
#[cfg_attr(all(test, feature = "std"), dto(export))]
#[cfg_attr(
    feature = "serde",
    serde(rename_all = "snake_case", tag = "kind", content = "amount")
)]
#[derive(Clone, Debug)]
pub enum OperationalListingAvailability {
    Window {
        #[cfg_attr(all(test, feature = "std"), dto(int = "json_string"))]
        start: Option<u64>,
        #[cfg_attr(all(test, feature = "std"), dto(int = "json_string"))]
        end: Option<u64>,
    },
    Status {
        status: OperationalListingStatus,
    },
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(all(test, feature = "std"), derive(dto_bindgen::Dto))]
#[cfg_attr(all(test, feature = "std"), dto(export))]
#[cfg_attr(
    feature = "serde",
    serde(rename_all = "snake_case", tag = "kind", content = "amount")
)]
#[derive(Clone, Debug)]
pub enum OperationalListingStatus {
    Active,
    Sold,
    Other { value: String },
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(all(test, feature = "std"), derive(dto_bindgen::Dto))]
#[cfg_attr(all(test, feature = "std"), dto(export))]
#[cfg_attr(
    feature = "serde",
    serde(rename_all = "snake_case", tag = "kind", content = "amount")
)]
#[derive(Clone, Debug)]
pub enum OperationalListingDeliveryMethod {
    Pickup,
    LocalDelivery,
    Shipping,
    Other { method: String },
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(all(test, feature = "std"), derive(dto_bindgen::Dto))]
#[cfg_attr(all(test, feature = "std"), dto(export))]
#[derive(Clone, Debug)]
pub struct OperationalListing {
    pub d_tag: DTag,
    #[cfg_attr(all(test, feature = "std"), dto(int = "json_string"))]
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub published_at: Option<u64>,
    #[cfg_attr(any(feature = "serde", test), serde(default))]
    pub farm: FarmRef,
    pub product: OperationalListingProduct,
    pub primary_bin_id: InventoryBinId,
    pub bins: Vec<OperationalListingBin>,
    pub resource_area: Option<ResourceAreaRef>,
    pub plot: Option<PlotRef>,
    #[cfg_attr(all(test, feature = "std"), dto(ts(type = "Array<Discount>")))]
    pub discounts: Option<Vec<Discount>>,
    #[cfg_attr(all(test, feature = "std"), dto(ts(type = "Decimal")))]
    pub inventory_available: Option<Decimal>,
    pub availability: Option<OperationalListingAvailability>,
    pub delivery_method: Option<OperationalListingDeliveryMethod>,
    pub location: Option<OperationalListingPublicLocation>,
    pub images: Option<Vec<OperationalListingImage>>,
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(all(test, feature = "std"), derive(dto_bindgen::Dto))]
#[cfg_attr(all(test, feature = "std"), dto(export))]
#[derive(Clone, Debug)]
pub struct OperationalListingProduct {
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

pub const RADROOTS_OPERATIONAL_LISTING_PRODUCT_TAG_KEYS: [&str; 9] = [
    "key", "title", "category", "summary", "process", "lot", "location", "profile", "year",
];

pub struct OperationalListingProductTagKeys;

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(all(test, feature = "std"), derive(dto_bindgen::Dto))]
#[cfg_attr(all(test, feature = "std"), dto(export))]
#[derive(Clone, Debug)]
pub struct OperationalListingBin {
    pub bin_id: InventoryBinId,
    #[cfg_attr(all(test, feature = "std"), dto(ts(type = "Quantity")))]
    pub quantity: Quantity,
    #[cfg_attr(all(test, feature = "std"), dto(ts(type = "QuantityPrice")))]
    pub price_per_canonical_unit: QuantityPrice,
    #[cfg_attr(all(test, feature = "std"), dto(ts(type = "Decimal")))]
    pub display_amount: Option<Decimal>,
    #[cfg_attr(all(test, feature = "std"), dto(ts(type = "Unit")))]
    pub display_unit: Option<Unit>,
    pub display_label: Option<String>,
    #[cfg_attr(all(test, feature = "std"), dto(ts(type = "Money")))]
    pub display_price: Option<Money>,
    #[cfg_attr(all(test, feature = "std"), dto(ts(type = "Unit")))]
    pub display_price_unit: Option<Unit>,
}

#[cfg_attr(all(test, feature = "std"), derive(dto_bindgen::Dto))]
#[cfg_attr(all(test, feature = "std"), dto(export))]
#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug)]
pub struct OperationalListingPublicLocation {
    pub primary: String,
    pub city: Option<String>,
    pub region: Option<String>,
    pub country: Option<String>,
    pub geohash: String,
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(all(test, feature = "std"), derive(dto_bindgen::Dto))]
#[cfg_attr(all(test, feature = "std"), dto(export))]
#[derive(Clone, Debug)]
pub struct OperationalListingImage {
    pub url: String,
    pub size: Option<OperationalListingImageSize>,
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(all(test, feature = "std"), derive(dto_bindgen::Dto))]
#[derive(Clone, Debug)]
pub struct OperationalListingImageSize {
    pub w: u32,
    pub h: u32,
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use crate::farm::FarmRef;

    #[test]
    fn defaults_listing_farm_ref_to_empty_values() {
        let farm_ref = FarmRef::default();
        assert!(farm_ref.pubkey.is_empty());
        assert!(farm_ref.d_tag.is_empty());
    }

    #[test]
    fn listing_model_covers_published_metadata() {
        use crate::envelope::kind::{KIND_CLASSIFIED_LISTING, is_classified_listing_kind};

        let listing = super::OperationalListing {
            d_tag: "listing-draft".parse().unwrap(),
            published_at: Some(1_700_000_000),
            farm: FarmRef::default(),
            product: super::OperationalListingProduct {
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
        assert!(is_classified_listing_kind(KIND_CLASSIFIED_LISTING));
    }

    #[test]
    #[cfg(feature = "serde")]
    fn listing_deserializes_missing_farm_to_default_ref() {
        let listing = super::OperationalListing {
            d_tag: "listing-draft".parse().unwrap(),
            published_at: Some(1_700_000_000),
            farm: FarmRef {
                pubkey: "farm-pubkey".to_string(),
                d_tag: "farm-d-tag".to_string(),
            },
            product: super::OperationalListingProduct {
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
        let mut json = serde_json::to_value(&listing).unwrap();
        json.as_object_mut().unwrap().remove("farm");

        let parsed: super::OperationalListing = serde_json::from_value(json).unwrap();

        assert!(parsed.farm.pubkey.is_empty());
        assert!(parsed.farm.d_tag.is_empty());
    }
}
