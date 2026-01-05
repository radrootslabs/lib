#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use radroots_core::{RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCoreQuantity, RadrootsCoreUnit};
use radroots_events::{
    RadrootsNostrEvent,
    kinds::KIND_LISTING,
    listing::{
        RadrootsListing, RadrootsListingAvailability, RadrootsListingDeliveryMethod,
        RadrootsListingLocation,
    },
};
#[cfg(feature = "ts-rs")]
use ts_rs::TS;

use crate::listing::codec::{TradeListingParseError, listing_from_event_parts};
use crate::listing::dvm::TradeListingAddress;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsTradeListing {
    pub listing_id: String,
    pub listing_addr: String,
    pub seller_pubkey: String,
    pub title: String,
    pub description: String,
    pub product_type: String,
    pub primary_bin_id: String,
    #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsCoreQuantity"))]
    pub bin_quantity: RadrootsCoreQuantity,
    #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsCoreUnit"))]
    pub unit: RadrootsCoreUnit,
    #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsCoreMoney"))]
    pub unit_price: RadrootsCoreMoney,
    #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsCoreDecimal"))]
    pub inventory_available: RadrootsCoreDecimal,
    pub availability: RadrootsListingAvailability,
    pub location: RadrootsListingLocation,
    pub delivery_method: RadrootsListingDeliveryMethod,
    pub listing: RadrootsListing,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(rename_all = "snake_case", tag = "kind", content = "amount")
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TradeListingValidationError {
    InvalidKind { kind: u32 },
    MissingListingId,
    ListingEventNotFound { listing_addr: String },
    ListingEventFetchFailed { listing_addr: String },
    ParseError { error: TradeListingParseError },
    InvalidSeller,
    MissingFarmProfile,
    MissingFarmRecord,
    MissingTitle,
    MissingDescription,
    MissingProductType,
    MissingBins,
    MissingPrimaryBin,
    InvalidBin,
    MissingPrice,
    InvalidPrice,
    MissingInventory,
    InvalidInventory,
    MissingAvailability,
    MissingLocation,
    MissingDeliveryMethod,
}

impl core::fmt::Display for TradeListingValidationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            TradeListingValidationError::InvalidKind { kind } => {
                write!(f, "invalid listing kind: {kind}")
            }
            TradeListingValidationError::MissingListingId => write!(f, "missing listing id"),
            TradeListingValidationError::ListingEventNotFound { listing_addr } => {
                write!(f, "listing event not found: {listing_addr}")
            }
            TradeListingValidationError::ListingEventFetchFailed { listing_addr } => {
                write!(f, "listing event fetch failed: {listing_addr}")
            }
            TradeListingValidationError::ParseError { error } => {
                write!(f, "invalid listing data: {error}")
            }
            TradeListingValidationError::InvalidSeller => {
                write!(f, "listing author does not match farm pubkey")
            }
            TradeListingValidationError::MissingFarmProfile => {
                write!(f, "missing farm profile")
            }
            TradeListingValidationError::MissingFarmRecord => {
                write!(f, "missing farm record")
            }
            TradeListingValidationError::MissingTitle => write!(f, "missing listing title"),
            TradeListingValidationError::MissingDescription => {
                write!(f, "missing listing description")
            }
            TradeListingValidationError::MissingProductType => {
                write!(f, "missing listing product type")
            }
            TradeListingValidationError::MissingBins => write!(f, "missing listing bins"),
            TradeListingValidationError::MissingPrimaryBin => {
                write!(f, "missing primary listing bin")
            }
            TradeListingValidationError::InvalidBin => write!(f, "invalid listing bin"),
            TradeListingValidationError::MissingPrice => write!(f, "missing listing price"),
            TradeListingValidationError::InvalidPrice => write!(f, "invalid listing price"),
            TradeListingValidationError::MissingInventory => {
                write!(f, "missing listing inventory")
            }
            TradeListingValidationError::InvalidInventory => {
                write!(f, "invalid listing inventory")
            }
            TradeListingValidationError::MissingAvailability => {
                write!(f, "missing listing availability")
            }
            TradeListingValidationError::MissingLocation => write!(f, "missing listing location"),
            TradeListingValidationError::MissingDeliveryMethod => {
                write!(f, "missing listing delivery method")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for TradeListingValidationError {}

pub fn validate_listing_event(
    event: &RadrootsNostrEvent,
) -> Result<RadrootsTradeListing, TradeListingValidationError> {
    if event.kind != KIND_LISTING {
        return Err(TradeListingValidationError::InvalidKind { kind: event.kind });
    }

    let listing = listing_from_event_parts(&event.tags, &event.content)
        .map_err(|error| TradeListingValidationError::ParseError { error })?;
    let listing_id = listing.d_tag.trim().to_string();
    if listing_id.is_empty() {
        return Err(TradeListingValidationError::MissingListingId);
    }

    let seller_pubkey = event.author.clone();
    if listing.farm.pubkey != seller_pubkey {
        return Err(TradeListingValidationError::InvalidSeller);
    }
    let listing_addr = TradeListingAddress {
        kind: KIND_LISTING as u16,
        seller_pubkey: seller_pubkey.clone(),
        listing_id: listing_id.clone(),
    }
    .as_str();

    let title = listing.product.title.trim().to_string();
    if title.is_empty() {
        return Err(TradeListingValidationError::MissingTitle);
    }

    let description = listing
        .product
        .summary
        .as_ref()
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    if description.is_empty() {
        return Err(TradeListingValidationError::MissingDescription);
    }

    let product_type = if !listing.product.category.trim().is_empty() {
        listing.product.category.trim().to_string()
    } else {
        listing.product.key.trim().to_string()
    };
    if product_type.is_empty() {
        return Err(TradeListingValidationError::MissingProductType);
    }

    if listing.bins.is_empty() {
        return Err(TradeListingValidationError::MissingBins);
    }
    let primary_bin_id = listing.primary_bin_id.trim().to_string();
    if primary_bin_id.is_empty() {
        return Err(TradeListingValidationError::MissingPrimaryBin);
    }
    let primary_bin = listing
        .bins
        .iter()
        .find(|bin| bin.bin_id == primary_bin_id)
        .ok_or(TradeListingValidationError::MissingPrimaryBin)?;

    if primary_bin.quantity.amount.is_sign_negative() {
        return Err(TradeListingValidationError::InvalidBin);
    }
    if !primary_bin.quantity.is_canonical() {
        return Err(TradeListingValidationError::InvalidBin);
    }
    if !primary_bin
        .price_per_canonical_unit
        .is_price_per_canonical_unit()
    {
        return Err(TradeListingValidationError::InvalidPrice);
    }
    if primary_bin
        .price_per_canonical_unit
        .amount
        .amount
        .is_sign_negative()
    {
        return Err(TradeListingValidationError::InvalidPrice);
    }
    if primary_bin.price_per_canonical_unit.quantity.unit != primary_bin.quantity.unit {
        return Err(TradeListingValidationError::InvalidPrice);
    }

    let inventory_available = listing
        .inventory_available
        .clone()
        .ok_or(TradeListingValidationError::MissingInventory)?;
    if inventory_available.is_sign_negative() {
        return Err(TradeListingValidationError::InvalidInventory);
    }

    let availability = listing
        .availability
        .clone()
        .ok_or(TradeListingValidationError::MissingAvailability)?;
    let location = listing
        .location
        .clone()
        .ok_or(TradeListingValidationError::MissingLocation)?;
    let delivery_method = listing
        .delivery_method
        .clone()
        .ok_or(TradeListingValidationError::MissingDeliveryMethod)?;

    Ok(RadrootsTradeListing {
        listing_id,
        listing_addr,
        seller_pubkey,
        title,
        description,
        product_type,
        primary_bin_id: primary_bin_id.clone(),
        bin_quantity: primary_bin.quantity.clone(),
        unit: primary_bin.quantity.unit,
        unit_price: primary_bin.price_per_canonical_unit.amount.clone(),
        inventory_available,
        availability,
        location,
        delivery_method,
        listing,
    })
}

#[cfg(test)]
mod tests {
    use super::{TradeListingValidationError, validate_listing_event};
    use radroots_core::{
        RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCoreQuantity,
        RadrootsCoreQuantityPrice, RadrootsCoreUnit,
    };
    use radroots_events::{
        RadrootsNostrEvent,
        kinds::KIND_LISTING,
        listing::{
            RadrootsListing, RadrootsListingAvailability, RadrootsListingDeliveryMethod,
            RadrootsListingBin, RadrootsListingFarmRef, RadrootsListingLocation,
            RadrootsListingProduct,
        },
    };

    fn base_listing() -> RadrootsListing {
        RadrootsListing {
            d_tag: "AAAAAAAAAAAAAAAAAAAAAg".into(),
            farm: RadrootsListingFarmRef {
                pubkey: "seller".into(),
                d_tag: "AAAAAAAAAAAAAAAAAAAAAA".into(),
            },
            product: RadrootsListingProduct {
                key: "coffee".into(),
                title: "Coffee".into(),
                category: "coffee".into(),
                summary: Some("Single origin coffee".into()),
                process: None,
                lot: None,
                location: None,
                profile: None,
                year: None,
            },
            primary_bin_id: "bin-1".into(),
            bins: vec![RadrootsListingBin {
                bin_id: "bin-1".into(),
                quantity: RadrootsCoreQuantity::new(
                    RadrootsCoreDecimal::from(1000u32),
                    RadrootsCoreUnit::MassG,
                ),
                price_per_canonical_unit: RadrootsCoreQuantityPrice {
                    amount: RadrootsCoreMoney::new(
                        RadrootsCoreDecimal::from(20u32),
                        RadrootsCoreCurrency::USD,
                    ),
                    quantity: RadrootsCoreQuantity::new(
                        RadrootsCoreDecimal::from(1u32),
                        RadrootsCoreUnit::MassG,
                    ),
                },
                display_amount: None,
                display_unit: None,
                display_label: None,
                display_price: None,
                display_price_unit: None,
            }],
            resource_area: None,
            plot: None,
            discounts: None,
            inventory_available: Some(RadrootsCoreDecimal::from(5u32)),
            availability: Some(RadrootsListingAvailability::Status {
                status: radroots_events::listing::RadrootsListingStatus::Active,
            }),
            delivery_method: Some(RadrootsListingDeliveryMethod::Pickup),
            location: Some(RadrootsListingLocation {
                primary: "Farm".into(),
                city: None,
                region: None,
                country: None,
                lat: None,
                lng: None,
                geohash: None,
            }),
            images: None,
        }
    }

    fn base_event(listing: &RadrootsListing) -> RadrootsNostrEvent {
        RadrootsNostrEvent {
            id: "evt".into(),
            author: "seller".into(),
            created_at: 0,
            kind: KIND_LISTING,
            tags: vec![
                vec!["d".into(), listing.d_tag.clone()],
                vec!["p".into(), listing.farm.pubkey.clone()],
                vec![
                    "a".into(),
                    format!("30340:{}:{}", listing.farm.pubkey, listing.farm.d_tag),
                ],
            ],
            content: serde_json::to_string(listing).unwrap(),
            sig: "sig".into(),
        }
    }

    #[test]
    fn validate_listing_ok() {
        let listing = base_listing();
        let event = base_event(&listing);
        assert!(validate_listing_event(&event).is_ok());
    }

    #[test]
    fn validate_listing_rejects_missing_d_tag() {
        let listing = base_listing();
        let mut event = base_event(&listing);
        event.tags.clear();
        let err = validate_listing_event(&event).unwrap_err();
        assert!(matches!(
            err,
            TradeListingValidationError::ParseError { .. }
        ));
    }

    #[test]
    fn validate_listing_rejects_invalid_currency() {
        let mut event = base_event(&base_listing());
        event.content = String::new();
        event.tags = vec![
            vec!["d".into(), "AAAAAAAAAAAAAAAAAAAAAg".into()],
            vec!["p".into(), "seller".into()],
            vec!["a".into(), "30340:seller:AAAAAAAAAAAAAAAAAAAAAA".into()],
            vec!["key".into(), "coffee".into()],
            vec!["title".into(), "Coffee".into()],
            vec!["category".into(), "coffee".into()],
            vec!["summary".into(), "Single origin".into()],
            vec![
                "quantity".into(),
                "1".into(),
                "lb".into(),
                "bag".into(),
                "5".into(),
            ],
            vec![
                "price".into(),
                "20".into(),
                "US".into(),
                "1".into(),
                "lb".into(),
            ],
            vec![
                "location".into(),
                "Farm".into(),
                "Town".into(),
                "Region".into(),
            ],
            vec!["status".into(), "active".into()],
            vec!["delivery".into(), "pickup".into()],
        ];
        let err = validate_listing_event(&event).unwrap_err();
        assert!(matches!(
            err,
            TradeListingValidationError::ParseError { .. }
        ));
    }

    #[test]
    fn validate_listing_rejects_mismatched_seller() {
        let listing = base_listing();
        let mut event = base_event(&listing);
        event.author = "other".into();
        let err = validate_listing_event(&event).unwrap_err();
        assert!(matches!(err, TradeListingValidationError::InvalidSeller));
    }

    #[test]
    fn validate_listing_rejects_missing_inventory() {
        let mut listing = base_listing();
        listing.inventory_available = None;
        let event = base_event(&listing);
        let err = validate_listing_event(&event).unwrap_err();
        assert!(matches!(err, TradeListingValidationError::MissingInventory));
    }
}
