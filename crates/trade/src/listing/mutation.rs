#![forbid(unsafe_code)]

use radroots_events::ids::RadrootsListingAddress;
use thiserror::Error;

use crate::listing::draft::RadrootsCanonicalListingDraft;

/// Listing v1 mutation intent for draft preparation only.
///
/// Publish and update target the public listing event, save-draft targets the
/// secret listing-draft event, and archive is intentionally unsupported because
/// listing v1 defines no archive wire event.
#[derive(Clone, Debug)]
pub enum RadrootsListingMutation {
    Publish {
        draft: RadrootsCanonicalListingDraft,
    },
    Update {
        draft: RadrootsCanonicalListingDraft,
    },
    SaveDraft {
        draft: RadrootsCanonicalListingDraft,
    },
    Archive {
        listing_addr: RadrootsListingAddress,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsListingLifecycleState {
    Draft,
    Published,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum RadrootsListingMutationError {
    #[error("listing mutation is not supported")]
    UnsupportedMutation,
}

impl RadrootsListingMutation {
    pub fn publish(draft: RadrootsCanonicalListingDraft) -> Self {
        Self::Publish { draft }
    }

    pub fn update(draft: RadrootsCanonicalListingDraft) -> Self {
        Self::Update { draft }
    }

    pub fn save_draft(draft: RadrootsCanonicalListingDraft) -> Self {
        Self::SaveDraft { draft }
    }

    pub fn archive(listing_addr: RadrootsListingAddress) -> Self {
        Self::Archive { listing_addr }
    }

    pub fn lifecycle_state(
        &self,
    ) -> Result<RadrootsListingLifecycleState, RadrootsListingMutationError> {
        match self {
            Self::Publish { .. } | Self::Update { .. } => {
                Ok(RadrootsListingLifecycleState::Published)
            }
            Self::SaveDraft { .. } => Ok(RadrootsListingLifecycleState::Draft),
            Self::Archive { .. } => Err(RadrootsListingMutationError::UnsupportedMutation),
        }
    }

    pub fn canonical_draft(
        &self,
    ) -> Result<&RadrootsCanonicalListingDraft, RadrootsListingMutationError> {
        match self {
            Self::Publish { draft } | Self::Update { draft } | Self::SaveDraft { draft } => {
                Ok(draft)
            }
            Self::Archive { .. } => Err(RadrootsListingMutationError::UnsupportedMutation),
        }
    }
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

    use crate::listing::draft::{RadrootsCanonicalListingDraft, RadrootsListingDraftDocumentV1};

    use super::{
        RadrootsListingLifecycleState, RadrootsListingMutation, RadrootsListingMutationError,
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

    fn canonical_draft() -> RadrootsCanonicalListingDraft {
        RadrootsCanonicalListingDraft::new(
            RadrootsPublicKey::parse(SELLER).expect("seller"),
            RadrootsListingAddress::parse(format!(
                "{KIND_LISTING_DRAFT}:{SELLER}:AAAAAAAAAAAAAAAAAAAAAg"
            ))
            .expect("listing address"),
            RadrootsListingDraftDocumentV1::new(listing()),
        )
    }

    #[test]
    fn supported_mutations_report_lifecycle_states() {
        assert_eq!(
            RadrootsListingMutation::publish(canonical_draft())
                .lifecycle_state()
                .expect("state"),
            RadrootsListingLifecycleState::Published
        );
        assert_eq!(
            RadrootsListingMutation::update(canonical_draft())
                .lifecycle_state()
                .expect("state"),
            RadrootsListingLifecycleState::Published
        );
        assert_eq!(
            RadrootsListingMutation::save_draft(canonical_draft())
                .lifecycle_state()
                .expect("state"),
            RadrootsListingLifecycleState::Draft
        );
    }

    #[test]
    fn supported_mutations_expose_canonical_drafts() {
        let publish = RadrootsListingMutation::publish(canonical_draft());
        let update = RadrootsListingMutation::update(canonical_draft());
        let save_draft = RadrootsListingMutation::save_draft(canonical_draft());

        assert_eq!(
            publish.canonical_draft().expect("draft").seller_pubkey,
            SELLER
        );
        assert_eq!(
            update.canonical_draft().expect("draft").seller_pubkey,
            SELLER
        );
        assert_eq!(
            save_draft.canonical_draft().expect("draft").seller_pubkey,
            SELLER
        );
    }

    #[test]
    fn archive_is_explicitly_unsupported() {
        let archive = RadrootsListingMutation::archive(
            RadrootsListingAddress::parse(format!(
                "{KIND_LISTING_DRAFT}:{SELLER}:AAAAAAAAAAAAAAAAAAAAAg"
            ))
            .expect("listing address"),
        );

        assert_eq!(
            archive.lifecycle_state().unwrap_err(),
            RadrootsListingMutationError::UnsupportedMutation
        );
        assert_eq!(
            archive.canonical_draft().unwrap_err(),
            RadrootsListingMutationError::UnsupportedMutation
        );
    }
}
