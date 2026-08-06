//! Canonicalization for Radroots Listing v1 drafts.

#![forbid(unsafe_code)]

use core::fmt;

use std::{format, vec::Vec};

use radroots_event::{
    envelope::kind::KIND_CLASSIFIED_LISTING,
    id::{ClassifiedListingAddress, InventoryBinId, ParseError},
    listing::operational::OperationalListing,
    trade::validation::OperationalListingValidationError,
};
use radroots_identity::{Error as PublicKeyError, PublicKey};

use super::validation::validate_operational_listing_model;

#[derive(Clone, Debug)]
pub struct RadrootsOperationalListingEditDocumentV1 {
    pub listing: OperationalListing,
}

impl RadrootsOperationalListingEditDocumentV1 {
    pub fn new(listing: OperationalListing) -> Self {
        Self { listing }
    }
}

#[derive(Clone, Debug)]
pub struct RadrootsOperationalListingCanonicalEdit {
    listing: OperationalListing,
    seller_pubkey: PublicKey,
    public_listing_addr: ClassifiedListingAddress,
}

impl RadrootsOperationalListingCanonicalEdit {
    pub fn new(
        mut listing: OperationalListing,
        seller_pubkey: PublicKey,
    ) -> Result<Self, RadrootsOperationalListingEditError> {
        let farm_pubkey = PublicKey::from_hex(listing.farm.pubkey.as_str())
            .map_err(RadrootsOperationalListingEditError::InvalidFarmPubkey)?;
        if farm_pubkey != seller_pubkey {
            return Err(RadrootsOperationalListingEditError::FarmPubkeyMismatch {
                expected_pubkey: seller_pubkey,
                actual_pubkey: farm_pubkey,
            });
        }
        listing.farm.pubkey = farm_pubkey.to_hex();
        validate_listing_bins(&listing)?;
        let listing = validate_operational_listing_model(listing, &seller_pubkey)
            .map_err(RadrootsOperationalListingEditError::InvalidModel)?
            .listing;

        let public_listing_addr = listing_addr(
            KIND_CLASSIFIED_LISTING,
            &seller_pubkey,
            listing.d_tag.as_str(),
        );

        Ok(Self {
            listing,
            seller_pubkey,
            public_listing_addr,
        })
    }

    pub fn listing(&self) -> &OperationalListing {
        &self.listing
    }

    pub fn seller_pubkey(&self) -> &PublicKey {
        &self.seller_pubkey
    }

    pub fn public_listing_addr(&self) -> &ClassifiedListingAddress {
        &self.public_listing_addr
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsOperationalListingEditError {
    InvalidFarmPubkey(PublicKeyError),
    InvalidClassifiedListingAddress(ParseError),
    InvalidModel(OperationalListingValidationError),
    FarmPubkeyMismatch {
        expected_pubkey: PublicKey,
        actual_pubkey: PublicKey,
    },
    MissingPrimaryBin {
        primary_bin_id: InventoryBinId,
    },
    DuplicateBinId {
        bin_id: InventoryBinId,
    },
}

impl fmt::Display for RadrootsOperationalListingEditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFarmPubkey(error) => {
                write!(f, "invalid listing edit farm pubkey: {error}")
            }
            Self::InvalidClassifiedListingAddress(error) => {
                write!(f, "invalid listing edit address: {error}")
            }
            Self::InvalidModel(error) => {
                write!(f, "invalid listing edit model: {error}")
            }
            Self::FarmPubkeyMismatch { .. } => {
                f.write_str("listing edit farm pubkey does not match seller")
            }
            Self::MissingPrimaryBin { .. } => f.write_str("listing edit primary bin is missing"),
            Self::DuplicateBinId { .. } => f.write_str("listing edit contains duplicate bin ID"),
        }
    }
}

impl core::error::Error for RadrootsOperationalListingEditError {}

fn validate_listing_bins(
    listing: &OperationalListing,
) -> Result<(), RadrootsOperationalListingEditError> {
    let primary_bin_id = listing.primary_bin_id.clone();
    let mut seen_bin_ids = Vec::new();
    let mut primary_bin_found = false;
    for bin in &listing.bins {
        if seen_bin_ids
            .iter()
            .any(|seen_bin_id| seen_bin_id == &bin.bin_id)
        {
            return Err(RadrootsOperationalListingEditError::DuplicateBinId {
                bin_id: bin.bin_id.clone(),
            });
        }
        if bin.bin_id == primary_bin_id {
            primary_bin_found = true;
        }
        seen_bin_ids.push(bin.bin_id.clone());
    }

    if !primary_bin_found {
        return Err(RadrootsOperationalListingEditError::MissingPrimaryBin { primary_bin_id });
    }
    Ok(())
}

fn listing_addr(kind: u32, seller_pubkey: &PublicKey, d_tag: &str) -> ClassifiedListingAddress {
    ClassifiedListingAddress::parse(format!("{kind}:{seller_pubkey}:{d_tag}"))
        .expect("typed listing identity must form a listing address")
}

/// Canonicalizes an untrusted listing edit for an already-authorized seller.
///
/// Actor provenance and role authorization belong to the signing/workflow
/// boundary. This deterministic function validates only the supplied public
/// identity and listing data and performs no signing or authorization.
pub fn canonicalize_operational_listing_edit(
    seller_pubkey: PublicKey,
    mut document: RadrootsOperationalListingEditDocumentV1,
) -> Result<RadrootsOperationalListingCanonicalEdit, RadrootsOperationalListingEditError> {
    let farm_pubkey = document.listing.farm.pubkey.as_str();
    if farm_pubkey.is_empty() {
        document.listing.farm.pubkey = seller_pubkey.to_hex();
    }

    RadrootsOperationalListingCanonicalEdit::new(document.listing, seller_pubkey)
}

#[cfg(test)]
mod tests {
    const FIXTURE_ALICE_PUBLIC_KEY_HEX: &str =
        "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df";
    const FIXTURE_BOB_PUBLIC_KEY_HEX: &str =
        "e0266e3cfb0d2886f91c73f5f868f3b98273713e5fcd97c081663f5518a4b3af";
    use radroots_core::{Currency, Decimal, Money, Quantity, QuantityPrice, Unit};
    use radroots_event::{
        envelope::kind::KIND_CLASSIFIED_LISTING,
        farm::FarmRef,
        id::{ClassifiedListingAddress, DTag, InventoryBinId},
        listing::operational::{
            OperationalListing, OperationalListingAvailability, OperationalListingBin,
            OperationalListingDeliveryMethod, OperationalListingProduct,
            OperationalListingPublicLocation, OperationalListingStatus,
        },
        trade::validation::OperationalListingValidationError,
    };
    use radroots_identity::PublicKey;

    use super::{
        RadrootsOperationalListingCanonicalEdit, RadrootsOperationalListingEditDocumentV1,
        RadrootsOperationalListingEditError, canonicalize_operational_listing_edit,
    };

    const SELLER: &str = FIXTURE_ALICE_PUBLIC_KEY_HEX;
    const OTHER: &str = FIXTURE_BOB_PUBLIC_KEY_HEX;

    fn d_tag(raw: &str) -> DTag {
        DTag::parse(raw).expect("d tag")
    }

    fn bin_id(raw: &str) -> InventoryBinId {
        InventoryBinId::parse(raw).expect("bin id")
    }

    fn listing() -> OperationalListing {
        OperationalListing {
            d_tag: d_tag("AAAAAAAAAAAAAAAAAAAAAg"),
            published_at: None,
            farm: FarmRef {
                pubkey: SELLER.to_string(),
                d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_string(),
            },
            product: OperationalListingProduct {
                key: "coffee".to_string(),
                title: "Coffee".to_string(),
                category: "coffee".to_string(),
                summary: Some("Single origin coffee".to_string()),
                process: None,
                lot: None,
                location: None,
                profile: None,
                year: None,
            },
            primary_bin_id: bin_id("bin-1"),
            bins: vec![OperationalListingBin {
                bin_id: bin_id("bin-1"),
                quantity: Quantity::try_new(Decimal::from(1000u32), Unit::MassG).unwrap(),
                price_per_canonical_unit: QuantityPrice::try_new(
                    Money::try_new(Decimal::from(20u32), Currency::USD).unwrap(),
                    Quantity::try_new(Decimal::from(1u32), Unit::MassG).unwrap(),
                )
                .unwrap(),
                display_amount: None,
                display_unit: None,
                display_label: None,
                display_price: None,
                display_price_unit: None,
            }],
            resource_area: None,
            plot: None,
            discounts: None,
            inventory_available: Some(Decimal::from(5u32)),
            availability: Some(OperationalListingAvailability::Status {
                status: OperationalListingStatus::Active,
            }),
            delivery_method: Some(OperationalListingDeliveryMethod::Pickup),
            location: Some(OperationalListingPublicLocation {
                primary: "Victoria".to_string(),
                city: Some("Victoria".to_string()),
                region: Some("British Columbia".to_string()),
                country: Some("CA".to_string()),
                geohash: "c287g".to_string(),
            }),
            images: None,
        }
    }

    fn seller_pubkey() -> PublicKey {
        PublicKey::from_hex(SELLER).expect("seller public key")
    }

    #[test]
    fn draft_document_wraps_listing() {
        let document = RadrootsOperationalListingEditDocumentV1::new(listing());

        assert_eq!(document.listing.d_tag.as_str(), "AAAAAAAAAAAAAAAAAAAAAg");
        assert_eq!(document.listing.product.title, "Coffee");
    }

    #[cfg(any())]
    #[test]
    fn draft_document_deserializes_as_untrusted_input() {
        let json = serde_json::to_string(&RadrootsOperationalListingEditDocumentV1::new(listing()))
            .expect("serialize document");

        let document: RadrootsOperationalListingEditDocumentV1 =
            serde_json::from_str(&json).expect("deserialize document");
        let canonical = canonicalize_operational_listing_edit(seller_pubkey(), document)
            .expect("canonical draft");

        assert_eq!(canonical.seller_pubkey().to_hex(), SELLER);
        assert_eq!(canonical.listing().product.title, "Coffee");
    }

    #[test]
    fn canonical_draft_carries_seller_listing_and_addresses() {
        let seller_pubkey = PublicKey::from_hex(SELLER).expect("seller");
        let listing = listing();

        let canonical = RadrootsOperationalListingCanonicalEdit::new(listing, seller_pubkey)
            .expect("canonical");

        assert_eq!(canonical.seller_pubkey(), &seller_pubkey);
        assert_eq!(
            canonical.public_listing_addr().as_str(),
            format!("{KIND_CLASSIFIED_LISTING}:{SELLER}:AAAAAAAAAAAAAAAAAAAAAg")
        );
        assert_eq!(canonical.listing().d_tag.as_str(), "AAAAAAAAAAAAAAAAAAAAAg");
    }

    #[test]
    fn listing_edit_error_variants_are_precise() {
        assert!(matches!(
            RadrootsOperationalListingEditError::InvalidFarmPubkey(
                PublicKey::from_hex("bad").unwrap_err()
            ),
            RadrootsOperationalListingEditError::InvalidFarmPubkey(_)
        ));
        assert!(matches!(
            RadrootsOperationalListingEditError::InvalidClassifiedListingAddress(
                ClassifiedListingAddress::parse("bad").unwrap_err()
            ),
            RadrootsOperationalListingEditError::InvalidClassifiedListingAddress(_)
        ));
    }

    #[test]
    fn canonicalize_operational_listing_edit_fills_missing_farm_pubkey_and_derives_address() {
        let mut listing = listing();
        listing.farm.pubkey.clear();
        let document = RadrootsOperationalListingEditDocumentV1::new(listing);

        let canonical = canonicalize_operational_listing_edit(seller_pubkey(), document)
            .expect("canonical draft");

        assert_eq!(canonical.seller_pubkey().to_hex(), SELLER);
        assert_eq!(
            canonical.public_listing_addr().as_str(),
            format!("{KIND_CLASSIFIED_LISTING}:{SELLER}:AAAAAAAAAAAAAAAAAAAAAg")
        );
        assert_eq!(canonical.listing().farm.pubkey, SELLER);
    }

    #[test]
    fn canonicalize_operational_listing_edit_rejects_mismatched_farm_pubkey() {
        let mut listing = listing();
        listing.farm.pubkey = OTHER.to_string();
        let document = RadrootsOperationalListingEditDocumentV1::new(listing);

        let error = canonicalize_operational_listing_edit(seller_pubkey(), document).unwrap_err();

        assert!(matches!(
            error,
            RadrootsOperationalListingEditError::FarmPubkeyMismatch { .. }
        ));
    }

    #[test]
    fn canonicalize_operational_listing_edit_rejects_invalid_farm_pubkey() {
        let mut listing = listing();
        listing.farm.pubkey = "bad".to_string();
        let document = RadrootsOperationalListingEditDocumentV1::new(listing);

        let error = canonicalize_operational_listing_edit(seller_pubkey(), document).unwrap_err();

        assert!(matches!(
            error,
            RadrootsOperationalListingEditError::InvalidFarmPubkey(_)
        ));
    }

    #[test]
    fn canonical_draft_new_rejects_mismatched_farm_pubkey() {
        let mut listing = listing();
        listing.farm.pubkey = OTHER.to_string();

        let error = RadrootsOperationalListingCanonicalEdit::new(
            listing,
            PublicKey::from_hex(SELLER).expect("seller"),
        )
        .unwrap_err();

        assert!(matches!(
            error,
            RadrootsOperationalListingEditError::FarmPubkeyMismatch { .. }
        ));
    }

    #[test]
    fn canonical_draft_new_rejects_invalid_farm_pubkey() {
        let mut listing = listing();
        listing.farm.pubkey = "bad".to_string();

        let error = RadrootsOperationalListingCanonicalEdit::new(
            listing,
            PublicKey::from_hex(SELLER).expect("seller"),
        )
        .unwrap_err();

        assert!(matches!(
            error,
            RadrootsOperationalListingEditError::InvalidFarmPubkey(_)
        ));
    }

    #[test]
    fn canonical_draft_new_rejects_empty_farm_pubkey() {
        let mut listing = listing();
        listing.farm.pubkey.clear();

        let error = RadrootsOperationalListingCanonicalEdit::new(
            listing,
            PublicKey::from_hex(SELLER).expect("seller"),
        )
        .unwrap_err();

        assert!(matches!(
            error,
            RadrootsOperationalListingEditError::InvalidFarmPubkey(_)
        ));
    }

    #[test]
    fn canonicalize_operational_listing_edit_rejects_missing_primary_bin() {
        let mut listing = listing();
        listing.primary_bin_id = bin_id("bin-2");
        let document = RadrootsOperationalListingEditDocumentV1::new(listing);

        let error = canonicalize_operational_listing_edit(seller_pubkey(), document).unwrap_err();

        assert_eq!(
            error,
            RadrootsOperationalListingEditError::MissingPrimaryBin {
                primary_bin_id: bin_id("bin-2")
            }
        );
    }

    #[test]
    fn canonical_draft_new_rejects_missing_primary_bin() {
        let mut listing = listing();
        listing.primary_bin_id = bin_id("bin-2");

        let error = RadrootsOperationalListingCanonicalEdit::new(
            listing,
            PublicKey::from_hex(SELLER).expect("seller"),
        )
        .unwrap_err();

        assert_eq!(
            error,
            RadrootsOperationalListingEditError::MissingPrimaryBin {
                primary_bin_id: bin_id("bin-2")
            }
        );
    }

    #[test]
    fn canonicalize_operational_listing_edit_rejects_duplicate_bin_ids() {
        let mut listing = listing();
        listing.bins.push(listing.bins[0].clone());
        let document = RadrootsOperationalListingEditDocumentV1::new(listing);

        let error = canonicalize_operational_listing_edit(seller_pubkey(), document).unwrap_err();

        assert_eq!(
            error,
            RadrootsOperationalListingEditError::DuplicateBinId {
                bin_id: bin_id("bin-1")
            }
        );
    }

    #[test]
    fn canonical_draft_new_rejects_invalid_model() {
        let mut listing = listing();
        listing.inventory_available = None;

        let error = RadrootsOperationalListingCanonicalEdit::new(
            listing,
            PublicKey::from_hex(SELLER).expect("seller"),
        )
        .unwrap_err();

        assert_eq!(
            error,
            RadrootsOperationalListingEditError::InvalidModel(
                OperationalListingValidationError::MissingInventory
            )
        );
    }

    #[test]
    fn canonical_draft_new_rejects_invalid_secondary_bin() {
        let mut invalid_quantity_listing = listing();
        let mut secondary_bin = invalid_quantity_listing.bins[0].clone();
        secondary_bin.bin_id = bin_id("bin-2");
        secondary_bin.quantity = Quantity::try_new(Decimal::ONE, Unit::MassKg).unwrap();
        invalid_quantity_listing.bins.push(secondary_bin);

        let error = RadrootsOperationalListingCanonicalEdit::new(
            invalid_quantity_listing,
            PublicKey::from_hex(SELLER).expect("seller"),
        )
        .unwrap_err();

        assert_eq!(
            error,
            RadrootsOperationalListingEditError::InvalidModel(
                OperationalListingValidationError::InvalidBin
            )
        );

        let mut mismatched_unit_listing = listing();
        let mut secondary_bin = mismatched_unit_listing.bins[0].clone();
        secondary_bin.bin_id = bin_id("bin-2");
        secondary_bin.price_per_canonical_unit = QuantityPrice::try_new(
            secondary_bin.price_per_canonical_unit.amount().clone(),
            Quantity::try_new(Decimal::ONE, Unit::Each).unwrap(),
        )
        .unwrap();
        mismatched_unit_listing.bins.push(secondary_bin);

        let error = RadrootsOperationalListingCanonicalEdit::new(
            mismatched_unit_listing,
            PublicKey::from_hex(SELLER).expect("seller"),
        )
        .unwrap_err();

        assert_eq!(
            error,
            RadrootsOperationalListingEditError::InvalidModel(
                OperationalListingValidationError::InvalidPrice
            )
        );
    }

    #[test]
    fn canonical_draft_new_rejects_duplicate_bin_ids() {
        let mut listing = listing();
        listing.bins.push(listing.bins[0].clone());

        let error = RadrootsOperationalListingCanonicalEdit::new(
            listing,
            PublicKey::from_hex(SELLER).expect("seller"),
        )
        .unwrap_err();

        assert_eq!(
            error,
            RadrootsOperationalListingEditError::DuplicateBinId {
                bin_id: bin_id("bin-1")
            }
        );
    }
}
