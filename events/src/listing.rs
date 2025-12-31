use radroots_core::{
    RadrootsCoreDecimal, RadrootsCoreDiscountValue, RadrootsCoreMoney, RadrootsCorePercent,
    RadrootsCoreQuantity, RadrootsCoreQuantityPrice,
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
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case", tag = "kind", content = "amount"))]
#[derive(Clone, Debug)]
pub enum RadrootsListingAvailability {
    Window {
        #[cfg_attr(feature = "ts-rs", ts(optional, type = "number | null"))]
        start: Option<u64>,
        #[cfg_attr(feature = "ts-rs", ts(optional, type = "number | null"))]
        end: Option<u64>,
    },
    Status {
        status: RadrootsListingStatus,
    },
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case", tag = "kind", content = "amount"))]
#[derive(Clone, Debug)]
pub enum RadrootsListingStatus {
    Active,
    Sold,
    Other {
        value: String,
    },
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case", tag = "kind", content = "amount"))]
#[derive(Clone, Debug)]
pub enum RadrootsListingDeliveryMethod {
    Pickup,
    LocalDelivery,
    Shipping,
    Other {
        method: String,
    },
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsListing {
    pub d_tag: String,
    #[cfg_attr(feature = "serde", serde(default))]
    pub farm: RadrootsListingFarmRef,
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
        ts(optional, type = "RadrootsCoreDecimal | null")
    )]
    pub inventory_available: Option<RadrootsCoreDecimal>,
    #[cfg_attr(
        feature = "ts-rs",
        ts(optional, type = "RadrootsListingAvailability | null")
    )]
    pub availability: Option<RadrootsListingAvailability>,
    #[cfg_attr(
        feature = "ts-rs",
        ts(optional, type = "RadrootsListingDeliveryMethod | null")
    )]
    pub delivery_method: Option<RadrootsListingDeliveryMethod>,
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
pub struct RadrootsListingFarmRef {
    pub pubkey: String,
    pub d_tag: String,
}

impl Default for RadrootsListingFarmRef {
    fn default() -> Self {
        Self {
            pubkey: String::new(),
            d_tag: String::new(),
        }
    }
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

pub const RADROOTS_LISTING_PRODUCT_TAG_KEYS: [&str; 9] = [
    "key",
    "title",
    "category",
    "summary",
    "process",
    "lot",
    "location",
    "profile",
    "year",
];

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(
    feature = "ts-rs",
    ts(
        export,
        export_to = "types.ts",
        type = "readonly [\"key\", \"title\", \"category\", \"summary\", \"process\", \"lot\", \"location\", \"profile\", \"year\"]"
    )
)]
pub struct RadrootsListingProductTagKeys;

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

#[cfg(all(test, feature = "ts-rs", feature = "std"))]
mod constants_tests {
    use super::RADROOTS_LISTING_PRODUCT_TAG_KEYS;
    use std::{env, fs, path::Path};

    fn listing_product_tag_keys_literal() -> String {
        let mut out = String::from("[");
        for (idx, key) in RADROOTS_LISTING_PRODUCT_TAG_KEYS.iter().enumerate() {
            if idx > 0 {
                out.push_str(", ");
            }
            out.push('"');
            out.push_str(key);
            out.push('"');
        }
        out.push(']');
        out
    }

    #[test]
    fn export_listing_product_tag_keys_const() {
        let out_dir = env::var("TS_RS_EXPORT_DIR").unwrap_or_else(|_| "./bindings".to_string());
        let path = Path::new(&out_dir).join("constants.ts");
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create ts export dir");
        }
        let keys = listing_product_tag_keys_literal();
        let content = format!(
            "import type {{ RadrootsListingProductTagKeys }} from \"./types.js\";\n\nexport const RADROOTS_LISTING_PRODUCT_TAG_KEYS: RadrootsListingProductTagKeys = {keys};\n"
        );
        fs::write(&path, content).expect("write constants");
    }
}
