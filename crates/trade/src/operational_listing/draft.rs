//! Canonicalization for Radroots Listing v1 drafts.

#![forbid(unsafe_code)]

use core::fmt;

#[cfg(not(feature = "std"))]
use alloc::{format, string::ToString, vec::Vec};

#[cfg(feature = "std")]
use std::{string::ToString, vec::Vec};

use radroots_authority::RadrootsActorContext;
use radroots_event::{
    contract::RadrootsActorRole,
    ids::{
        RadrootsClassifiedListingAddress, RadrootsIdParseError, RadrootsInventoryBinId,
        RadrootsPublicKey,
    },
    kinds::KIND_CLASSIFIED_LISTING,
    operational_listing::RadrootsOperationalListing,
    trade_validation::RadrootsOperationalListingValidationError,
};

use super::validation::validate_operational_listing_model;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsOperationalListingEditDocumentV1 {
    pub listing: RadrootsOperationalListing,
}

impl RadrootsOperationalListingEditDocumentV1 {
    pub fn new(listing: RadrootsOperationalListing) -> Self {
        Self { listing }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Debug)]
pub struct RadrootsOperationalListingCanonicalEdit {
    listing: RadrootsOperationalListing,
    seller_pubkey: RadrootsPublicKey,
    public_listing_addr: RadrootsClassifiedListingAddress,
}

impl RadrootsOperationalListingCanonicalEdit {
    pub fn new(
        mut listing: RadrootsOperationalListing,
        seller_pubkey: RadrootsPublicKey,
    ) -> Result<Self, RadrootsOperationalListingEditError> {
        let farm_pubkey = RadrootsPublicKey::parse(listing.farm.pubkey.as_str())
            .map_err(RadrootsOperationalListingEditError::InvalidFarmPubkey)?;
        if farm_pubkey != seller_pubkey {
            return Err(RadrootsOperationalListingEditError::FarmPubkeyMismatch {
                expected_pubkey: seller_pubkey,
                actual_pubkey: farm_pubkey,
            });
        }
        listing.farm.pubkey = farm_pubkey.as_str().to_string();
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

    pub fn listing(&self) -> &RadrootsOperationalListing {
        &self.listing
    }

    pub fn seller_pubkey(&self) -> &RadrootsPublicKey {
        &self.seller_pubkey
    }

    pub fn public_listing_addr(&self) -> &RadrootsClassifiedListingAddress {
        &self.public_listing_addr
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsOperationalListingEditError {
    InvalidFarmPubkey(RadrootsIdParseError),
    InvalidClassifiedListingAddress(RadrootsIdParseError),
    InvalidModel(RadrootsOperationalListingValidationError),
    ActorRoleUnsatisfied {
        required_role: RadrootsActorRole,
    },
    FarmPubkeyMismatch {
        expected_pubkey: RadrootsPublicKey,
        actual_pubkey: RadrootsPublicKey,
    },
    MissingPrimaryBin {
        primary_bin_id: RadrootsInventoryBinId,
    },
    DuplicateBinId {
        bin_id: RadrootsInventoryBinId,
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
            Self::ActorRoleUnsatisfied { required_role } => write!(
                f,
                "listing edit actor does not satisfy required role {required_role:?}"
            ),
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
    listing: &RadrootsOperationalListing,
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

fn listing_addr(
    kind: u32,
    seller_pubkey: &RadrootsPublicKey,
    d_tag: &str,
) -> RadrootsClassifiedListingAddress {
    RadrootsClassifiedListingAddress::parse(format!("{kind}:{}:{d_tag}", seller_pubkey.as_str()))
        .expect("typed listing identity must form a listing address")
}

pub fn canonicalize_operational_listing_edit(
    actor: &RadrootsActorContext,
    mut document: RadrootsOperationalListingEditDocumentV1,
) -> Result<RadrootsOperationalListingCanonicalEdit, RadrootsOperationalListingEditError> {
    if !actor.satisfies(RadrootsActorRole::Seller) {
        return Err(RadrootsOperationalListingEditError::ActorRoleUnsatisfied {
            required_role: RadrootsActorRole::Seller,
        });
    }

    let seller_pubkey = actor.pubkey().clone();
    let farm_pubkey = document.listing.farm.pubkey.as_str();
    if farm_pubkey.is_empty() {
        document.listing.farm.pubkey = seller_pubkey.as_str().to_string();
    }

    RadrootsOperationalListingCanonicalEdit::new(document.listing, seller_pubkey)
}

#[cfg(test)]
mod tests {
    use radroots_authority::RadrootsActorContext;
    use radroots_core::{
        RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCoreQuantity,
        RadrootsCoreQuantityPrice, RadrootsCoreUnit,
    };
    use radroots_event::{
        contract::RadrootsActorRole,
        farm::RadrootsFarmRef,
        ids::{
            RadrootsClassifiedListingAddress, RadrootsDTag, RadrootsInventoryBinId,
            RadrootsPublicKey,
        },
        kinds::KIND_CLASSIFIED_LISTING,
        operational_listing::{
            RadrootsOperationalListing, RadrootsOperationalListingAvailability,
            RadrootsOperationalListingBin, RadrootsOperationalListingDeliveryMethod,
            RadrootsOperationalListingProduct, RadrootsOperationalListingPublicLocation,
            RadrootsOperationalListingStatus,
        },
        trade_validation::RadrootsOperationalListingValidationError,
    };

    use super::{
        RadrootsOperationalListingCanonicalEdit, RadrootsOperationalListingEditDocumentV1,
        RadrootsOperationalListingEditError, canonicalize_operational_listing_edit,
    };

    const SELLER: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const OTHER: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    fn d_tag(raw: &str) -> RadrootsDTag {
        RadrootsDTag::parse(raw).expect("d tag")
    }

    fn bin_id(raw: &str) -> RadrootsInventoryBinId {
        RadrootsInventoryBinId::parse(raw).expect("bin id")
    }

    fn listing() -> RadrootsOperationalListing {
        RadrootsOperationalListing {
            d_tag: d_tag("AAAAAAAAAAAAAAAAAAAAAg"),
            published_at: None,
            farm: RadrootsFarmRef {
                pubkey: SELLER.to_string(),
                d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_string(),
            },
            product: RadrootsOperationalListingProduct {
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
            bins: vec![RadrootsOperationalListingBin {
                bin_id: bin_id("bin-1"),
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
            availability: Some(RadrootsOperationalListingAvailability::Status {
                status: RadrootsOperationalListingStatus::Active,
            }),
            delivery_method: Some(RadrootsOperationalListingDeliveryMethod::Pickup),
            location: Some(RadrootsOperationalListingPublicLocation {
                primary: "Victoria".to_string(),
                city: Some("Victoria".to_string()),
                region: Some("British Columbia".to_string()),
                country: Some("CA".to_string()),
                geohash: "c287g".to_string(),
            }),
            images: None,
        }
    }

    fn seller_actor() -> RadrootsActorContext {
        RadrootsActorContext::explicit_pubkey(SELLER, [RadrootsActorRole::Seller]).expect("actor")
    }

    fn buyer_actor() -> RadrootsActorContext {
        RadrootsActorContext::explicit_pubkey(SELLER, [RadrootsActorRole::Buyer]).expect("actor")
    }

    #[test]
    fn draft_document_wraps_listing() {
        let document = RadrootsOperationalListingEditDocumentV1::new(listing());

        assert_eq!(document.listing.d_tag.as_str(), "AAAAAAAAAAAAAAAAAAAAAg");
        assert_eq!(document.listing.product.title, "Coffee");
    }

    #[cfg(feature = "serde_json")]
    #[test]
    fn draft_document_deserializes_as_untrusted_input() {
        let json = serde_json::to_string(&RadrootsOperationalListingEditDocumentV1::new(listing()))
            .expect("serialize document");

        let document: RadrootsOperationalListingEditDocumentV1 =
            serde_json::from_str(&json).expect("deserialize document");
        let canonical = canonicalize_operational_listing_edit(&seller_actor(), document)
            .expect("canonical draft");

        assert_eq!(canonical.seller_pubkey().as_str(), SELLER);
        assert_eq!(canonical.listing().product.title, "Coffee");
    }

    #[test]
    fn canonical_draft_carries_seller_listing_and_addresses() {
        let seller_pubkey = RadrootsPublicKey::parse(SELLER).expect("seller");
        let listing = listing();

        let canonical =
            RadrootsOperationalListingCanonicalEdit::new(listing, seller_pubkey.clone())
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
                RadrootsPublicKey::parse("bad").unwrap_err()
            ),
            RadrootsOperationalListingEditError::InvalidFarmPubkey(_)
        ));
        assert!(matches!(
            RadrootsOperationalListingEditError::InvalidClassifiedListingAddress(
                RadrootsClassifiedListingAddress::parse("bad").unwrap_err()
            ),
            RadrootsOperationalListingEditError::InvalidClassifiedListingAddress(_)
        ));
    }

    #[test]
    fn canonicalize_operational_listing_edit_fills_missing_farm_pubkey_and_derives_address() {
        let mut listing = listing();
        listing.farm.pubkey.clear();
        let document = RadrootsOperationalListingEditDocumentV1::new(listing);

        let canonical = canonicalize_operational_listing_edit(&seller_actor(), document)
            .expect("canonical draft");

        assert_eq!(canonical.seller_pubkey().as_str(), SELLER);
        assert_eq!(
            canonical.public_listing_addr().as_str(),
            format!("{KIND_CLASSIFIED_LISTING}:{SELLER}:AAAAAAAAAAAAAAAAAAAAAg")
        );
        assert_eq!(canonical.listing().farm.pubkey, SELLER);
    }

    #[test]
    fn canonicalize_operational_listing_edit_rejects_non_seller_actor() {
        let document = RadrootsOperationalListingEditDocumentV1::new(listing());

        let error = canonicalize_operational_listing_edit(&buyer_actor(), document).unwrap_err();

        assert_eq!(
            error,
            RadrootsOperationalListingEditError::ActorRoleUnsatisfied {
                required_role: RadrootsActorRole::Seller
            }
        );
    }

    #[test]
    fn canonicalize_operational_listing_edit_rejects_mismatched_farm_pubkey() {
        let mut listing = listing();
        listing.farm.pubkey = OTHER.to_string();
        let document = RadrootsOperationalListingEditDocumentV1::new(listing);

        let error = canonicalize_operational_listing_edit(&seller_actor(), document).unwrap_err();

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

        let error = canonicalize_operational_listing_edit(&seller_actor(), document).unwrap_err();

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
            RadrootsPublicKey::parse(SELLER).expect("seller"),
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
            RadrootsPublicKey::parse(SELLER).expect("seller"),
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
            RadrootsPublicKey::parse(SELLER).expect("seller"),
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

        let error = canonicalize_operational_listing_edit(&seller_actor(), document).unwrap_err();

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
            RadrootsPublicKey::parse(SELLER).expect("seller"),
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

        let error = canonicalize_operational_listing_edit(&seller_actor(), document).unwrap_err();

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
            RadrootsPublicKey::parse(SELLER).expect("seller"),
        )
        .unwrap_err();

        assert_eq!(
            error,
            RadrootsOperationalListingEditError::InvalidModel(
                RadrootsOperationalListingValidationError::MissingInventory
            )
        );
    }

    #[test]
    fn canonical_draft_new_rejects_invalid_secondary_bin() {
        let mut invalid_quantity_listing = listing();
        let mut secondary_bin = invalid_quantity_listing.bins[0].clone();
        secondary_bin.bin_id = bin_id("bin-2");
        secondary_bin.quantity.amount = "-1".parse().expect("negative decimal");
        invalid_quantity_listing.bins.push(secondary_bin);

        let error = RadrootsOperationalListingCanonicalEdit::new(
            invalid_quantity_listing,
            RadrootsPublicKey::parse(SELLER).expect("seller"),
        )
        .unwrap_err();

        assert_eq!(
            error,
            RadrootsOperationalListingEditError::InvalidModel(
                RadrootsOperationalListingValidationError::InvalidBin
            )
        );

        let mut mismatched_unit_listing = listing();
        let mut secondary_bin = mismatched_unit_listing.bins[0].clone();
        secondary_bin.bin_id = bin_id("bin-2");
        secondary_bin.price_per_canonical_unit.quantity.unit = RadrootsCoreUnit::Each;
        mismatched_unit_listing.bins.push(secondary_bin);

        let error = RadrootsOperationalListingCanonicalEdit::new(
            mismatched_unit_listing,
            RadrootsPublicKey::parse(SELLER).expect("seller"),
        )
        .unwrap_err();

        assert_eq!(
            error,
            RadrootsOperationalListingEditError::InvalidModel(
                RadrootsOperationalListingValidationError::InvalidPrice
            )
        );
    }

    #[test]
    fn canonical_draft_new_rejects_duplicate_bin_ids() {
        let mut listing = listing();
        listing.bins.push(listing.bins[0].clone());

        let error = RadrootsOperationalListingCanonicalEdit::new(
            listing,
            RadrootsPublicKey::parse(SELLER).expect("seller"),
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
