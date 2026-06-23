//! Mutation draft preparation for Radroots Listing v1.
//!
//! Publish and update produce the stable public NIP-99 listing-kind event with
//! Radroots-specific JSON content, save-draft produces the stable listing-draft
//! event, and archive remains unsupported because Listing v1 has no archive
//! wire event. Strict NIP-99 Markdown-content interoperability is protocol-v2
//! work.

#![forbid(unsafe_code)]

#[cfg(all(feature = "serde_json", not(feature = "std")))]
use alloc::string::{String, ToString};

#[cfg(all(feature = "serde_json", feature = "std"))]
use std::string::{String, ToString};

use radroots_events::ids::RadrootsListingAddress;
#[cfg(feature = "serde_json")]
use radroots_events::{
    draft::{RadrootsDraftError, RadrootsFrozenEventDraft},
    kinds::{KIND_LISTING, KIND_LISTING_DRAFT},
};
#[cfg(feature = "serde_json")]
use radroots_events_codec::{listing::encode::to_wire_parts_with_kind, wire::to_frozen_draft};
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
    #[cfg(feature = "serde_json")]
    #[error("failed to encode listing mutation: {0}")]
    EncodeListing(String),
    #[cfg(feature = "serde_json")]
    #[error("failed to build listing mutation draft: {0}")]
    FrozenDraft(RadrootsDraftError),
}

const LISTING_PUBLISHED_CONTRACT_ID: &str = "radroots.listing.published.v1";
const LISTING_DRAFT_CONTRACT_ID: &str = "radroots.listing.draft.v1";

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

    pub fn listing_addr(&self) -> Result<&RadrootsListingAddress, RadrootsListingMutationError> {
        match self {
            Self::Publish { draft } | Self::Update { draft } => Ok(draft.public_listing_addr()),
            Self::SaveDraft { draft } => Ok(draft.draft_listing_addr()),
            Self::Archive { .. } => Err(RadrootsListingMutationError::UnsupportedMutation),
        }
    }
}

#[cfg(feature = "serde_json")]
pub fn build_listing_mutation_draft(
    mutation: &RadrootsListingMutation,
    created_at: u32,
) -> Result<RadrootsFrozenEventDraft, RadrootsListingMutationError> {
    let (draft, kind, contract_id) = match mutation {
        RadrootsListingMutation::Publish { draft } | RadrootsListingMutation::Update { draft } => {
            (draft, KIND_LISTING, LISTING_PUBLISHED_CONTRACT_ID)
        }
        RadrootsListingMutation::SaveDraft { draft } => {
            (draft, KIND_LISTING_DRAFT, LISTING_DRAFT_CONTRACT_ID)
        }
        RadrootsListingMutation::Archive { .. } => {
            return Err(RadrootsListingMutationError::UnsupportedMutation);
        }
    };
    let parts = to_wire_parts_with_kind(draft.listing(), kind)
        .map_err(|error| RadrootsListingMutationError::EncodeListing(error.to_string()))?;
    to_frozen_draft(
        parts,
        contract_id,
        draft.seller_pubkey().as_str(),
        created_at,
    )
    .map_err(RadrootsListingMutationError::FrozenDraft)
}

#[cfg(test)]
mod tests {
    use radroots_core::{
        RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCoreQuantity,
        RadrootsCoreQuantityPrice, RadrootsCoreUnit,
    };
    use radroots_events::{
        RadrootsNostrEvent,
        farm::RadrootsFarmRef,
        ids::{RadrootsDTag, RadrootsInventoryBinId, RadrootsListingAddress, RadrootsPublicKey},
        kinds::{KIND_LISTING, KIND_LISTING_DRAFT},
        listing::{
            RadrootsListing, RadrootsListingAvailability, RadrootsListingBin,
            RadrootsListingDeliveryMethod, RadrootsListingLocation, RadrootsListingProduct,
            RadrootsListingStatus,
        },
        resource_area::RadrootsResourceAreaRef,
    };

    use crate::listing::draft::RadrootsCanonicalListingDraft;
    use crate::listing::validation::validate_listing_event;

    use super::{
        LISTING_DRAFT_CONTRACT_ID, LISTING_PUBLISHED_CONTRACT_ID, RadrootsListingLifecycleState,
        RadrootsListingMutation, RadrootsListingMutationError, build_listing_mutation_draft,
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
            inventory_available: Some(RadrootsCoreDecimal::from(5u32)),
            availability: Some(RadrootsListingAvailability::Status {
                status: RadrootsListingStatus::Active,
            }),
            delivery_method: Some(RadrootsListingDeliveryMethod::Pickup),
            location: Some(RadrootsListingLocation {
                primary: "Farm".to_string(),
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

    fn canonical_draft() -> RadrootsCanonicalListingDraft {
        RadrootsCanonicalListingDraft::new(
            listing(),
            RadrootsPublicKey::parse(SELLER).expect("seller"),
        )
        .expect("canonical listing draft")
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
            publish
                .canonical_draft()
                .expect("draft")
                .seller_pubkey()
                .as_str(),
            SELLER
        );
        assert_eq!(
            update
                .canonical_draft()
                .expect("draft")
                .seller_pubkey()
                .as_str(),
            SELLER
        );
        assert_eq!(
            save_draft
                .canonical_draft()
                .expect("draft")
                .seller_pubkey()
                .as_str(),
            SELLER
        );
        assert_eq!(
            publish
                .canonical_draft()
                .expect("draft")
                .listing()
                .d_tag
                .as_str(),
            "AAAAAAAAAAAAAAAAAAAAAg"
        );
    }

    #[test]
    fn supported_mutations_report_listing_addresses() {
        let publish = RadrootsListingMutation::publish(canonical_draft());
        let update = RadrootsListingMutation::update(canonical_draft());
        let save_draft = RadrootsListingMutation::save_draft(canonical_draft());

        assert_eq!(
            publish.listing_addr().expect("address").as_str(),
            format!("{KIND_LISTING}:{SELLER}:AAAAAAAAAAAAAAAAAAAAAg")
        );
        assert_eq!(
            update.listing_addr().expect("address").as_str(),
            format!("{KIND_LISTING}:{SELLER}:AAAAAAAAAAAAAAAAAAAAAg")
        );
        assert_eq!(
            save_draft.listing_addr().expect("address").as_str(),
            format!("{KIND_LISTING_DRAFT}:{SELLER}:AAAAAAAAAAAAAAAAAAAAAg")
        );
    }

    #[test]
    fn archive_is_explicitly_unsupported() {
        let archive = RadrootsListingMutation::archive(
            RadrootsListingAddress::parse(format!(
                "{KIND_LISTING}:{SELLER}:AAAAAAAAAAAAAAAAAAAAAg"
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
        assert_eq!(
            archive.listing_addr().unwrap_err(),
            RadrootsListingMutationError::UnsupportedMutation
        );
    }

    #[test]
    fn build_listing_mutation_draft_maps_publish_and_update_to_published_listing() {
        let publish = RadrootsListingMutation::publish(canonical_draft());
        let update = RadrootsListingMutation::update(canonical_draft());

        let publish_draft = build_listing_mutation_draft(&publish, 1_700_000_000).expect("draft");
        let update_draft = build_listing_mutation_draft(&update, 1_700_000_000).expect("draft");

        assert_eq!(publish_draft.kind, KIND_LISTING);
        assert_eq!(publish_draft.contract_id, LISTING_PUBLISHED_CONTRACT_ID);
        assert_eq!(publish_draft.expected_pubkey, SELLER);
        assert_eq!(publish_draft.created_at, 1_700_000_000);
        assert_eq!(update_draft.kind, KIND_LISTING);
        assert_eq!(update_draft.contract_id, LISTING_PUBLISHED_CONTRACT_ID);
        assert_eq!(update_draft.expected_pubkey, SELLER);
    }

    #[test]
    fn build_listing_mutation_draft_maps_save_draft_to_listing_draft() {
        let save_draft = RadrootsListingMutation::save_draft(canonical_draft());

        let draft = build_listing_mutation_draft(&save_draft, 1_700_000_000).expect("draft");

        assert_eq!(draft.kind, KIND_LISTING_DRAFT);
        assert_eq!(draft.contract_id, LISTING_DRAFT_CONTRACT_ID);
        assert_eq!(draft.expected_pubkey, SELLER);
        assert_eq!(draft.created_at, 1_700_000_000);
    }

    #[test]
    fn build_listing_mutation_draft_rejects_archive() {
        let archive = RadrootsListingMutation::archive(
            RadrootsListingAddress::parse(format!(
                "{KIND_LISTING}:{SELLER}:AAAAAAAAAAAAAAAAAAAAAg"
            ))
            .expect("listing address"),
        );

        assert_eq!(
            build_listing_mutation_draft(&archive, 1_700_000_000).unwrap_err(),
            RadrootsListingMutationError::UnsupportedMutation
        );
    }

    #[test]
    fn build_listing_mutation_draft_reports_encode_errors() {
        let mut listing = listing();
        listing.resource_area = Some(RadrootsResourceAreaRef {
            pubkey: SELLER.to_string(),
            d_tag: "bad d tag".to_string(),
        });
        let draft = RadrootsCanonicalListingDraft::new(
            listing,
            RadrootsPublicKey::parse(SELLER).expect("seller"),
        )
        .expect("canonical listing draft");
        let publish = RadrootsListingMutation::publish(draft);

        let err = build_listing_mutation_draft(&publish, 1_700_000_000).unwrap_err();

        assert!(matches!(
            err,
            RadrootsListingMutationError::EncodeListing(_)
        ));
    }

    #[test]
    fn build_listing_mutation_draft_event_id_is_stable_for_fixed_input() {
        let publish = RadrootsListingMutation::publish(canonical_draft());

        let first = build_listing_mutation_draft(&publish, 1_700_000_000).expect("draft");
        let second = build_listing_mutation_draft(&publish, 1_700_000_000).expect("draft");

        assert_eq!(first.expected_event_id, second.expected_event_id);
        assert_eq!(first.expected_event_id.len(), 64);
        assert_eq!(first.tags, second.tags);
        assert_eq!(first.content, second.content);
    }

    #[test]
    fn build_listing_mutation_draft_output_validates_as_trade_listing() {
        let publish = RadrootsListingMutation::publish(canonical_draft());
        let draft = build_listing_mutation_draft(&publish, 1_700_000_000).expect("draft");

        let event = RadrootsNostrEvent {
            id: String::new(),
            author: draft.expected_pubkey.clone(),
            created_at: draft.created_at,
            kind: draft.kind,
            tags: draft.tags,
            content: draft.content,
            sig: String::new(),
        };
        let validated = validate_listing_event(&event).expect("validated listing");

        assert_eq!(validated.seller_pubkey, SELLER);
        assert!(validated.listing_addr.contains(&format!(":{SELLER}:")));
    }
}
