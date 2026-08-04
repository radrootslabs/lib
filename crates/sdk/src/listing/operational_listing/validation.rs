#![forbid(unsafe_code)]

use std::{
    format,
    string::{String, ToString},
};

use radroots_core::{Decimal, Money, Quantity, Unit};
use radroots_event::{
    envelope::kind::{KIND_CLASSIFIED_LISTING, is_classified_listing_kind},
    farm::location::{has_textual_locality, is_public_geohash5},
    id::ClassifiedListingAddress,
    listing::classified::{ClassifiedListingPartition, classify_classified_listing_tags},
    listing::operational::{
        OperationalListing, OperationalListingAvailability, OperationalListingBin,
        OperationalListingDeliveryMethod, OperationalListingPublicLocation,
    },
    trade::validation::OperationalListingValidationError,
};
use radroots_identity::PublicKey;

use radroots_event_codec::{
    decode::operational_listing::operational_listing_from_nostr_event,
    verify::RadrootsSignatureVerifiedEvent,
};

#[derive(Clone, Debug)]
pub struct RadrootsOperationalListingTradeProjection {
    pub listing_id: String,
    pub listing_addr: ClassifiedListingAddress,
    pub seller_pubkey: String,
    pub title: String,
    pub description: String,
    pub product_type: String,
    pub primary_bin_id: String,
    pub bin_quantity: Quantity,
    pub unit: Unit,
    pub unit_price: Money,
    pub inventory_available: Decimal,
    pub availability: OperationalListingAvailability,
    pub location: OperationalListingPublicLocation,
    pub delivery_method: OperationalListingDeliveryMethod,
    pub listing: OperationalListing,
}

/// Validates a signature-verified Operational Listing event.
///
/// A plain envelope cannot cross this boundary:
///
/// ```compile_fail
/// use radroots_event::envelope::EventEnvelope;
/// use radroots_sdk::listing::validate_operational_listing_event;
///
/// fn validate_unverified(event: &EventEnvelope) {
///     let _ = validate_operational_listing_event(event);
/// }
/// ```
pub fn validate_operational_listing_event(
    verified_event: &RadrootsSignatureVerifiedEvent,
) -> Result<RadrootsOperationalListingTradeProjection, OperationalListingValidationError> {
    let event = verified_event.event();
    if !is_classified_listing_kind(event.kind_u32()) {
        return Err(OperationalListingValidationError::InvalidKind {
            kind: event.kind_u32(),
        });
    }
    if classify_classified_listing_tags(event.tags())
        != ClassifiedListingPartition::OperationalListing
    {
        return Err(OperationalListingValidationError::InvalidProfile);
    }

    let listing = operational_listing_from_nostr_event(event)
        .map_err(|error| OperationalListingValidationError::ParseError { error })?;
    validate_operational_listing_model(listing, event.author())
}

/// Validates the trade semantics of an unsigned Operational Listing model.
///
/// The seller is typed independently from the model because it is the
/// authority against which the listing's farm identity is checked. This
/// function does not perform event-kind, profile, decoding, or signature
/// checks; callers handling Nostr events must use
/// [`validate_operational_listing_event`] instead.
pub fn validate_operational_listing_model(
    listing: OperationalListing,
    seller_pubkey: &PublicKey,
) -> Result<RadrootsOperationalListingTradeProjection, OperationalListingValidationError> {
    let listing_id = listing.d_tag.as_str().trim().to_string();

    if listing.farm.pubkey != seller_pubkey.to_hex() {
        return Err(OperationalListingValidationError::InvalidSeller);
    }
    let listing_addr_raw = format!("{KIND_CLASSIFIED_LISTING}:{}:{listing_id}", seller_pubkey);
    let listing_addr = ClassifiedListingAddress::parse(&listing_addr_raw)
        .expect("validated listing identity must form a listing address");

    let title = listing.product.title.trim().to_string();
    if title.is_empty() {
        return Err(OperationalListingValidationError::MissingTitle);
    }

    let description = listing
        .product
        .summary
        .as_ref()
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    if description.is_empty() {
        return Err(OperationalListingValidationError::MissingDescription);
    }

    let product_type = if !listing.product.category.trim().is_empty() {
        listing.product.category.trim().to_string()
    } else {
        listing.product.key.trim().to_string()
    };
    if product_type.is_empty() {
        return Err(OperationalListingValidationError::MissingProductType);
    }

    if listing.bins.is_empty() {
        return Err(OperationalListingValidationError::MissingBins);
    }
    let primary_bin_id = listing.primary_bin_id.as_str().trim().to_string();
    let primary_bin_index = listing
        .bins
        .iter()
        .position(|bin| bin.bin_id.as_str() == primary_bin_id)
        .ok_or(OperationalListingValidationError::MissingPrimaryBin)?;
    for (index, bin) in listing.bins.iter().enumerate() {
        if listing.bins[..index]
            .iter()
            .any(|seen| seen.bin_id == bin.bin_id)
        {
            return Err(OperationalListingValidationError::InvalidBin);
        }
    }
    let primary_bin = &listing.bins[primary_bin_index];

    validate_listing_bin(primary_bin)?;
    for (index, bin) in listing.bins.iter().enumerate() {
        if index != primary_bin_index {
            validate_listing_bin(bin)?;
        }
    }

    let inventory_available = listing
        .inventory_available
        .ok_or(OperationalListingValidationError::MissingInventory)?;
    if inventory_available.is_sign_negative() {
        return Err(OperationalListingValidationError::InvalidInventory);
    }

    let availability = listing
        .availability
        .clone()
        .ok_or(OperationalListingValidationError::MissingAvailability)?;
    let location = listing
        .location
        .clone()
        .ok_or(OperationalListingValidationError::MissingLocation)?;
    if !has_textual_locality(
        &location.primary,
        location.city.as_deref(),
        location.region.as_deref(),
        location.country.as_deref(),
    ) {
        return Err(OperationalListingValidationError::MissingLocationLocality);
    }
    validate_listing_location_geohash(&location.geohash)?;
    let delivery_method = listing
        .delivery_method
        .clone()
        .ok_or(OperationalListingValidationError::MissingDeliveryMethod)?;

    Ok(RadrootsOperationalListingTradeProjection {
        listing_id,
        listing_addr,
        seller_pubkey: seller_pubkey.to_hex(),
        title,
        description,
        product_type,
        primary_bin_id: primary_bin_id.clone(),
        bin_quantity: primary_bin.quantity.clone(),
        unit: primary_bin.quantity.unit(),
        unit_price: primary_bin.price_per_canonical_unit.amount().clone(),
        inventory_available,
        availability,
        location,
        delivery_method,
        listing,
    })
}

fn validate_listing_bin(
    bin: &OperationalListingBin,
) -> Result<(), OperationalListingValidationError> {
    if bin.quantity.amount().is_sign_negative() || !bin.quantity.is_canonical() {
        return Err(OperationalListingValidationError::InvalidBin);
    }
    if !bin.price_per_canonical_unit.is_price_per_canonical_unit()
        || bin
            .price_per_canonical_unit
            .amount()
            .amount()
            .is_sign_negative()
        || bin.price_per_canonical_unit.quantity().unit() != bin.quantity.unit()
    {
        return Err(OperationalListingValidationError::InvalidPrice);
    }
    Ok(())
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn validate_listing_location_geohash(
    geohash: &str,
) -> Result<(), OperationalListingValidationError> {
    if geohash.trim().is_empty() {
        return Err(OperationalListingValidationError::MissingLocationGeohash);
    }
    if !is_public_geohash5(geohash) {
        return Err(OperationalListingValidationError::InvalidLocationGeohash);
    }
    Ok(())
}
