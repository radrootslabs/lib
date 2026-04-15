#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
use alloc::string::String;

use radroots_events::RadrootsNostrEvent;
use radroots_events::kinds::{KIND_LISTING, KIND_LISTING_DRAFT, is_listing_kind};
use radroots_events::listing::RadrootsListing;
use radroots_events_codec::listing::encode::to_wire_parts_with_kind;
use thiserror::Error;

use crate::listing::validation::{RadrootsTradeListing, validate_listing_event};

#[derive(Debug, Error)]
pub enum RadrootsTradeListingPublishError {
    #[error("listing kind must be {KIND_LISTING} or {KIND_LISTING_DRAFT}")]
    InvalidKind,
    #[error("invalid listing contract: {0}")]
    InvalidContract(String),
}

pub fn resolve_listing_kind(kind: Option<u32>) -> Result<u32, RadrootsTradeListingPublishError> {
    let kind = kind.unwrap_or(KIND_LISTING);
    if !is_listing_kind(kind) {
        return Err(RadrootsTradeListingPublishError::InvalidKind);
    }
    Ok(kind)
}

pub fn canonicalize_listing_for_seller(
    mut listing: RadrootsListing,
    seller_pubkey: &str,
) -> RadrootsListing {
    if listing.farm.pubkey.trim().is_empty() {
        listing.farm.pubkey = seller_pubkey.to_string();
    }
    listing
}

pub fn validate_listing_for_seller(
    listing: RadrootsListing,
    seller_pubkey: &str,
    kind: u32,
) -> Result<RadrootsTradeListing, RadrootsTradeListingPublishError> {
    let parts = to_wire_parts_with_kind(&listing, kind)
        .map_err(|error| RadrootsTradeListingPublishError::InvalidContract(error.to_string()))?;
    let canonical = RadrootsNostrEvent {
        id: String::new(),
        author: seller_pubkey.to_string(),
        created_at: 0,
        kind: parts.kind,
        tags: parts.tags,
        content: parts.content,
        sig: String::new(),
    };
    validate_listing_event(&canonical)
        .map_err(|error| RadrootsTradeListingPublishError::InvalidContract(error.to_string()))
}

#[cfg(test)]
mod tests {
    use radroots_core::{
        RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCoreQuantity,
        RadrootsCoreQuantityPrice, RadrootsCoreUnit,
    };
    use radroots_events::farm::RadrootsFarmRef;
    use radroots_events::listing::{
        RadrootsListing, RadrootsListingAvailability, RadrootsListingBin,
        RadrootsListingDeliveryMethod, RadrootsListingLocation, RadrootsListingProduct,
    };

    use super::{
        canonicalize_listing_for_seller, resolve_listing_kind, validate_listing_for_seller,
    };

    fn base_listing() -> RadrootsListing {
        RadrootsListing {
            d_tag: "AAAAAAAAAAAAAAAAAAAAAg".into(),
            farm: RadrootsFarmRef {
                pubkey: String::new(),
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

    #[test]
    fn resolve_listing_kind_accepts_supported_kinds() {
        assert_eq!(
            resolve_listing_kind(None).unwrap(),
            radroots_events::kinds::KIND_LISTING
        );
        assert_eq!(
            resolve_listing_kind(Some(radroots_events::kinds::KIND_LISTING_DRAFT)).unwrap(),
            radroots_events::kinds::KIND_LISTING_DRAFT
        );
    }

    #[test]
    fn canonicalize_listing_sets_missing_farm_pubkey() {
        let listing = canonicalize_listing_for_seller(base_listing(), "seller");
        assert_eq!(listing.farm.pubkey, "seller");
    }

    #[test]
    fn validate_listing_for_seller_returns_listing_addr() {
        let listing = canonicalize_listing_for_seller(base_listing(), "seller");
        let validated =
            validate_listing_for_seller(listing, "seller", radroots_events::kinds::KIND_LISTING)
                .expect("validated listing");
        assert_eq!(validated.seller_pubkey, "seller");
        assert!(validated.listing_addr.contains(":seller:"));
    }
}
