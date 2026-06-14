#![forbid(unsafe_code)]

use radroots_events::{
    ids::{RadrootsIdParseError, RadrootsListingAddress, RadrootsPublicKey},
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

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsCanonicalListingDraft {
    pub seller_pubkey: RadrootsPublicKey,
    pub listing_addr: RadrootsListingAddress,
    pub document: RadrootsListingDraftDocumentV1,
}

impl RadrootsCanonicalListingDraft {
    pub fn new(
        seller_pubkey: RadrootsPublicKey,
        listing_addr: RadrootsListingAddress,
        document: RadrootsListingDraftDocumentV1,
    ) -> Self {
        Self {
            seller_pubkey,
            listing_addr,
            document,
        }
    }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum RadrootsListingDraftError {
    #[error("invalid listing draft seller pubkey: {0}")]
    InvalidSellerPubkey(RadrootsIdParseError),
    #[error("invalid listing draft address: {0}")]
    InvalidListingAddress(RadrootsIdParseError),
}

#[cfg(test)]
mod tests {
    use radroots_core::{
        RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCoreQuantity,
        RadrootsCoreQuantityPrice, RadrootsCoreUnit,
    };
    use radroots_events::{
        farm::RadrootsFarmRef,
        ids::{RadrootsDTag, RadrootsInventoryBinId, RadrootsListingAddress, RadrootsPublicKey},
        kinds::KIND_LISTING_DRAFT,
        listing::{RadrootsListing, RadrootsListingBin, RadrootsListingProduct},
    };

    use super::{
        RadrootsCanonicalListingDraft, RadrootsListingDraftDocumentV1, RadrootsListingDraftError,
    };

    const SELLER: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

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

    #[test]
    fn draft_document_wraps_listing() {
        let document = RadrootsListingDraftDocumentV1::new(listing());

        assert_eq!(document.listing.d_tag.as_str(), "AAAAAAAAAAAAAAAAAAAAAg");
        assert_eq!(document.listing.product.title, "Coffee");
    }

    #[test]
    fn canonical_draft_carries_seller_and_address() {
        let seller_pubkey = RadrootsPublicKey::parse(SELLER).expect("seller");
        let listing_addr = RadrootsListingAddress::parse(format!(
            "{KIND_LISTING_DRAFT}:{SELLER}:AAAAAAAAAAAAAAAAAAAAAg"
        ))
        .expect("listing address");
        let document = RadrootsListingDraftDocumentV1::new(listing());

        let canonical = RadrootsCanonicalListingDraft::new(
            seller_pubkey.clone(),
            listing_addr.clone(),
            document,
        );

        assert_eq!(canonical.seller_pubkey, seller_pubkey);
        assert_eq!(canonical.listing_addr, listing_addr);
        assert_eq!(
            canonical.document.listing.d_tag.as_str(),
            "AAAAAAAAAAAAAAAAAAAAAg"
        );
    }

    #[test]
    fn listing_draft_error_variants_are_precise() {
        assert!(matches!(
            RadrootsListingDraftError::InvalidSellerPubkey(
                RadrootsPublicKey::parse("bad").unwrap_err()
            ),
            RadrootsListingDraftError::InvalidSellerPubkey(_)
        ));
        assert!(matches!(
            RadrootsListingDraftError::InvalidListingAddress(
                RadrootsListingAddress::parse("bad").unwrap_err()
            ),
            RadrootsListingDraftError::InvalidListingAddress(_)
        ));
    }
}
