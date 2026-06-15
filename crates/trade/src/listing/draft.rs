//! Canonicalization for Radroots Listing v1 drafts.
//!
//! Listing v1 uses NIP-99 listing kind numbers and Radroots-specific JSON
//! content. Strict NIP-99 Markdown-content interoperability is protocol-v2 work.
//! Canonical drafts derive both addresses from the same seller pubkey and
//! d-tag: the public address is for publish or update intent, and the draft
//! address is for save-draft intent.

#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
use alloc::{format, string::ToString, vec::Vec};

#[cfg(feature = "std")]
use std::{string::ToString, vec::Vec};

use radroots_authority::RadrootsActorContext;
use radroots_events::{
    contract::RadrootsActorRole,
    ids::{
        RadrootsIdParseError, RadrootsInventoryBinId, RadrootsListingAddress, RadrootsPublicKey,
    },
    kinds::{KIND_LISTING, KIND_LISTING_DRAFT},
    listing::RadrootsListing,
};
use thiserror::Error;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsListingDraftDocumentV1 {
    pub listing: RadrootsListing,
}

impl RadrootsListingDraftDocumentV1 {
    pub fn new(listing: RadrootsListing) -> Self {
        Self { listing }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Debug)]
pub struct RadrootsCanonicalListingDraft {
    listing: RadrootsListing,
    seller_pubkey: RadrootsPublicKey,
    public_listing_addr: RadrootsListingAddress,
    draft_listing_addr: RadrootsListingAddress,
}

impl RadrootsCanonicalListingDraft {
    pub fn new(
        mut listing: RadrootsListing,
        seller_pubkey: RadrootsPublicKey,
    ) -> Result<Self, RadrootsListingDraftError> {
        let farm_pubkey = RadrootsPublicKey::parse(listing.farm.pubkey.as_str())
            .map_err(RadrootsListingDraftError::InvalidFarmPubkey)?;
        if farm_pubkey != seller_pubkey {
            return Err(RadrootsListingDraftError::FarmPubkeyMismatch {
                expected_pubkey: seller_pubkey,
                actual_pubkey: farm_pubkey,
            });
        }
        listing.farm.pubkey = farm_pubkey.as_str().to_string();
        validate_listing_bins(&listing)?;

        let public_listing_addr =
            listing_addr(KIND_LISTING, &seller_pubkey, listing.d_tag.as_str())?;
        let draft_listing_addr =
            listing_addr(KIND_LISTING_DRAFT, &seller_pubkey, listing.d_tag.as_str())?;

        Ok(Self {
            listing,
            seller_pubkey,
            public_listing_addr,
            draft_listing_addr,
        })
    }

    pub fn listing(&self) -> &RadrootsListing {
        &self.listing
    }

    pub fn seller_pubkey(&self) -> &RadrootsPublicKey {
        &self.seller_pubkey
    }

    pub fn public_listing_addr(&self) -> &RadrootsListingAddress {
        &self.public_listing_addr
    }

    pub fn draft_listing_addr(&self) -> &RadrootsListingAddress {
        &self.draft_listing_addr
    }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum RadrootsListingDraftError {
    #[error("invalid listing draft farm pubkey: {0}")]
    InvalidFarmPubkey(RadrootsIdParseError),
    #[error("invalid listing draft address: {0}")]
    InvalidListingAddress(RadrootsIdParseError),
    #[error("listing draft actor does not satisfy required role {required_role:?}")]
    ActorRoleUnsatisfied { required_role: RadrootsActorRole },
    #[error("listing draft farm pubkey does not match seller")]
    FarmPubkeyMismatch {
        expected_pubkey: RadrootsPublicKey,
        actual_pubkey: RadrootsPublicKey,
    },
    #[error("listing draft primary bin is missing")]
    MissingPrimaryBin {
        primary_bin_id: RadrootsInventoryBinId,
    },
    #[error("listing draft contains duplicate bin ID")]
    DuplicateBinId { bin_id: RadrootsInventoryBinId },
}

fn validate_listing_bins(listing: &RadrootsListing) -> Result<(), RadrootsListingDraftError> {
    let primary_bin_id = listing.primary_bin_id.clone();
    let mut seen_bin_ids = Vec::new();
    let mut primary_bin_found = false;
    for bin in &listing.bins {
        if seen_bin_ids
            .iter()
            .any(|seen_bin_id| seen_bin_id == &bin.bin_id)
        {
            return Err(RadrootsListingDraftError::DuplicateBinId {
                bin_id: bin.bin_id.clone(),
            });
        }
        if bin.bin_id == primary_bin_id {
            primary_bin_found = true;
        }
        seen_bin_ids.push(bin.bin_id.clone());
    }

    if !primary_bin_found {
        return Err(RadrootsListingDraftError::MissingPrimaryBin { primary_bin_id });
    }
    Ok(())
}

fn listing_addr(
    kind: u32,
    seller_pubkey: &RadrootsPublicKey,
    d_tag: &str,
) -> Result<RadrootsListingAddress, RadrootsListingDraftError> {
    RadrootsListingAddress::parse(format!("{kind}:{}:{d_tag}", seller_pubkey.as_str()))
        .map_err(RadrootsListingDraftError::InvalidListingAddress)
}

pub fn canonicalize_listing_draft(
    actor: &RadrootsActorContext,
    mut document: RadrootsListingDraftDocumentV1,
) -> Result<RadrootsCanonicalListingDraft, RadrootsListingDraftError> {
    if !actor.satisfies(RadrootsActorRole::Seller) {
        return Err(RadrootsListingDraftError::ActorRoleUnsatisfied {
            required_role: RadrootsActorRole::Seller,
        });
    }

    let seller_pubkey = actor.pubkey().clone();
    let farm_pubkey = document.listing.farm.pubkey.as_str();
    if farm_pubkey.is_empty() {
        document.listing.farm.pubkey = seller_pubkey.as_str().to_string();
    }

    RadrootsCanonicalListingDraft::new(document.listing, seller_pubkey)
}

#[cfg(test)]
mod tests {
    use radroots_authority::RadrootsActorContext;
    use radroots_core::{
        RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCoreQuantity,
        RadrootsCoreQuantityPrice, RadrootsCoreUnit,
    };
    use radroots_events::{
        contract::RadrootsActorRole,
        farm::RadrootsFarmRef,
        ids::{RadrootsDTag, RadrootsInventoryBinId, RadrootsListingAddress, RadrootsPublicKey},
        kinds::{KIND_LISTING, KIND_LISTING_DRAFT},
        listing::{RadrootsListing, RadrootsListingBin, RadrootsListingProduct},
    };

    use super::{
        RadrootsCanonicalListingDraft, RadrootsListingDraftDocumentV1, RadrootsListingDraftError,
        canonicalize_listing_draft,
    };

    const SELLER: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const OTHER: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    fn d_tag(raw: &str) -> RadrootsDTag {
        RadrootsDTag::parse(raw).expect("d tag")
    }

    fn bin_id(raw: &str) -> RadrootsInventoryBinId {
        RadrootsInventoryBinId::parse(raw).expect("bin id")
    }

    fn listing() -> RadrootsListing {
        RadrootsListing {
            d_tag: d_tag("AAAAAAAAAAAAAAAAAAAAAg"),
            published_at: None,
            farm: RadrootsFarmRef {
                pubkey: SELLER.to_string(),
                d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_string(),
            },
            product: RadrootsListingProduct {
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
            bins: vec![RadrootsListingBin {
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
            inventory_available: None,
            availability: None,
            delivery_method: None,
            location: None,
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
        let document = RadrootsListingDraftDocumentV1::new(listing());

        assert_eq!(document.listing.d_tag.as_str(), "AAAAAAAAAAAAAAAAAAAAAg");
        assert_eq!(document.listing.product.title, "Coffee");
    }

    #[cfg(feature = "serde_json")]
    #[test]
    fn draft_document_deserializes_as_untrusted_input() {
        let json = serde_json::to_string(&RadrootsListingDraftDocumentV1::new(listing()))
            .expect("serialize document");

        let document: RadrootsListingDraftDocumentV1 =
            serde_json::from_str(&json).expect("deserialize document");
        let canonical =
            canonicalize_listing_draft(&seller_actor(), document).expect("canonical draft");

        assert_eq!(canonical.seller_pubkey().as_str(), SELLER);
        assert_eq!(canonical.listing().product.title, "Coffee");
    }

    #[test]
    fn canonical_draft_carries_seller_listing_and_addresses() {
        let seller_pubkey = RadrootsPublicKey::parse(SELLER).expect("seller");
        let listing = listing();

        let canonical =
            RadrootsCanonicalListingDraft::new(listing, seller_pubkey.clone()).expect("canonical");

        assert_eq!(canonical.seller_pubkey(), &seller_pubkey);
        assert_eq!(
            canonical.public_listing_addr().as_str(),
            format!("{KIND_LISTING}:{SELLER}:AAAAAAAAAAAAAAAAAAAAAg")
        );
        assert_eq!(
            canonical.draft_listing_addr().as_str(),
            format!("{KIND_LISTING_DRAFT}:{SELLER}:AAAAAAAAAAAAAAAAAAAAAg")
        );
        assert_eq!(canonical.listing().d_tag.as_str(), "AAAAAAAAAAAAAAAAAAAAAg");
    }

    #[test]
    fn listing_draft_error_variants_are_precise() {
        assert!(matches!(
            RadrootsListingDraftError::InvalidFarmPubkey(
                RadrootsPublicKey::parse("bad").unwrap_err()
            ),
            RadrootsListingDraftError::InvalidFarmPubkey(_)
        ));
        assert!(matches!(
            RadrootsListingDraftError::InvalidListingAddress(
                RadrootsListingAddress::parse("bad").unwrap_err()
            ),
            RadrootsListingDraftError::InvalidListingAddress(_)
        ));
    }

    #[test]
    fn canonicalize_listing_draft_fills_missing_farm_pubkey_and_derives_address() {
        let mut listing = listing();
        listing.farm.pubkey.clear();
        let document = RadrootsListingDraftDocumentV1::new(listing);

        let canonical =
            canonicalize_listing_draft(&seller_actor(), document).expect("canonical draft");

        assert_eq!(canonical.seller_pubkey().as_str(), SELLER);
        assert_eq!(
            canonical.public_listing_addr().as_str(),
            format!("{KIND_LISTING}:{SELLER}:AAAAAAAAAAAAAAAAAAAAAg")
        );
        assert_eq!(
            canonical.draft_listing_addr().as_str(),
            format!("{KIND_LISTING_DRAFT}:{SELLER}:AAAAAAAAAAAAAAAAAAAAAg")
        );
        assert_eq!(canonical.listing().farm.pubkey, SELLER);
    }

    #[test]
    fn canonicalize_listing_draft_rejects_non_seller_actor() {
        let document = RadrootsListingDraftDocumentV1::new(listing());

        let error = canonicalize_listing_draft(&buyer_actor(), document).unwrap_err();

        assert_eq!(
            error,
            RadrootsListingDraftError::ActorRoleUnsatisfied {
                required_role: RadrootsActorRole::Seller
            }
        );
    }

    #[test]
    fn canonicalize_listing_draft_rejects_mismatched_farm_pubkey() {
        let mut listing = listing();
        listing.farm.pubkey = OTHER.to_string();
        let document = RadrootsListingDraftDocumentV1::new(listing);

        let error = canonicalize_listing_draft(&seller_actor(), document).unwrap_err();

        assert!(matches!(
            error,
            RadrootsListingDraftError::FarmPubkeyMismatch { .. }
        ));
    }

    #[test]
    fn canonicalize_listing_draft_rejects_invalid_farm_pubkey() {
        let mut listing = listing();
        listing.farm.pubkey = "bad".to_string();
        let document = RadrootsListingDraftDocumentV1::new(listing);

        let error = canonicalize_listing_draft(&seller_actor(), document).unwrap_err();

        assert!(matches!(
            error,
            RadrootsListingDraftError::InvalidFarmPubkey(_)
        ));
    }

    #[test]
    fn canonical_draft_new_rejects_mismatched_farm_pubkey() {
        let mut listing = listing();
        listing.farm.pubkey = OTHER.to_string();

        let error = RadrootsCanonicalListingDraft::new(
            listing,
            RadrootsPublicKey::parse(SELLER).expect("seller"),
        )
        .unwrap_err();

        assert!(matches!(
            error,
            RadrootsListingDraftError::FarmPubkeyMismatch { .. }
        ));
    }

    #[test]
    fn canonical_draft_new_rejects_invalid_farm_pubkey() {
        let mut listing = listing();
        listing.farm.pubkey = "bad".to_string();

        let error = RadrootsCanonicalListingDraft::new(
            listing,
            RadrootsPublicKey::parse(SELLER).expect("seller"),
        )
        .unwrap_err();

        assert!(matches!(
            error,
            RadrootsListingDraftError::InvalidFarmPubkey(_)
        ));
    }

    #[test]
    fn canonical_draft_new_rejects_empty_farm_pubkey() {
        let mut listing = listing();
        listing.farm.pubkey.clear();

        let error = RadrootsCanonicalListingDraft::new(
            listing,
            RadrootsPublicKey::parse(SELLER).expect("seller"),
        )
        .unwrap_err();

        assert!(matches!(
            error,
            RadrootsListingDraftError::InvalidFarmPubkey(_)
        ));
    }

    #[test]
    fn canonicalize_listing_draft_rejects_missing_primary_bin() {
        let mut listing = listing();
        listing.primary_bin_id = bin_id("bin-2");
        let document = RadrootsListingDraftDocumentV1::new(listing);

        let error = canonicalize_listing_draft(&seller_actor(), document).unwrap_err();

        assert_eq!(
            error,
            RadrootsListingDraftError::MissingPrimaryBin {
                primary_bin_id: bin_id("bin-2")
            }
        );
    }

    #[test]
    fn canonical_draft_new_rejects_missing_primary_bin() {
        let mut listing = listing();
        listing.primary_bin_id = bin_id("bin-2");

        let error = RadrootsCanonicalListingDraft::new(
            listing,
            RadrootsPublicKey::parse(SELLER).expect("seller"),
        )
        .unwrap_err();

        assert_eq!(
            error,
            RadrootsListingDraftError::MissingPrimaryBin {
                primary_bin_id: bin_id("bin-2")
            }
        );
    }

    #[test]
    fn canonicalize_listing_draft_rejects_duplicate_bin_ids() {
        let mut listing = listing();
        listing.bins.push(listing.bins[0].clone());
        let document = RadrootsListingDraftDocumentV1::new(listing);

        let error = canonicalize_listing_draft(&seller_actor(), document).unwrap_err();

        assert_eq!(
            error,
            RadrootsListingDraftError::DuplicateBinId {
                bin_id: bin_id("bin-1")
            }
        );
    }

    #[test]
    fn canonical_draft_new_rejects_duplicate_bin_ids() {
        let mut listing = listing();
        listing.bins.push(listing.bins[0].clone());

        let error = RadrootsCanonicalListingDraft::new(
            listing,
            RadrootsPublicKey::parse(SELLER).expect("seller"),
        )
        .unwrap_err();

        assert_eq!(
            error,
            RadrootsListingDraftError::DuplicateBinId {
                bin_id: bin_id("bin-1")
            }
        );
    }
}
