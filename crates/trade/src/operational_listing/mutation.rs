//! Mutation draft preparation for Radroots Listing v1.

#![forbid(unsafe_code)]

use core::fmt;

#[cfg(all(feature = "serde_json", not(feature = "std")))]
use alloc::string::{String, ToString};

#[cfg(all(feature = "serde_json", feature = "std"))]
use std::string::{String, ToString};

use radroots_event::id::ClassifiedListingAddress;
#[cfg(feature = "serde_json")]
use radroots_event::{
    draft::{DraftError, EventDraft},
    envelope::kind::KIND_CLASSIFIED_LISTING,
};
#[cfg(feature = "serde_json")]
use radroots_event_codec::operational_listing::encode::to_wire_parts_with_kind;

use crate::operational_listing::draft::RadrootsOperationalListingCanonicalEdit;

/// Listing v1 mutation intent for draft preparation only.
///
/// Publish and update target the public listing event, while local-only draft
/// persistence and archive are intentionally unsupported as wire events.
#[derive(Clone, Debug)]
pub enum RadrootsOperationalListingMutation {
    Publish {
        draft: RadrootsOperationalListingCanonicalEdit,
    },
    Update {
        draft: RadrootsOperationalListingCanonicalEdit,
    },
    SaveDraft {
        draft: RadrootsOperationalListingCanonicalEdit,
    },
    Archive {
        listing_addr: ClassifiedListingAddress,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsOperationalListingLifecycleState {
    Draft,
    Published,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsOperationalListingMutationError {
    UnsupportedMutation,
    #[cfg(feature = "serde_json")]
    EncodeListing(String),
    #[cfg(feature = "serde_json")]
    FrozenDraft(DraftError),
}

impl fmt::Display for RadrootsOperationalListingMutationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedMutation => f.write_str("listing mutation is not supported"),
            #[cfg(feature = "serde_json")]
            Self::EncodeListing(error) => {
                write!(f, "failed to encode listing mutation: {error}")
            }
            #[cfg(feature = "serde_json")]
            Self::FrozenDraft(error) => {
                write!(f, "failed to build listing mutation draft: {error}")
            }
        }
    }
}

impl core::error::Error for RadrootsOperationalListingMutationError {}

#[cfg(feature = "serde_json")]
const OPERATIONAL_LISTING_PUBLISHED_CONTRACT_ID: &str = "radroots.operational_listing.published.v1";

impl RadrootsOperationalListingMutation {
    pub fn publish(draft: RadrootsOperationalListingCanonicalEdit) -> Self {
        Self::Publish { draft }
    }

    pub fn update(draft: RadrootsOperationalListingCanonicalEdit) -> Self {
        Self::Update { draft }
    }

    pub fn save_draft(draft: RadrootsOperationalListingCanonicalEdit) -> Self {
        Self::SaveDraft { draft }
    }

    pub fn archive(listing_addr: ClassifiedListingAddress) -> Self {
        Self::Archive { listing_addr }
    }

    pub fn lifecycle_state(
        &self,
    ) -> Result<RadrootsOperationalListingLifecycleState, RadrootsOperationalListingMutationError>
    {
        match self {
            Self::Publish { .. } | Self::Update { .. } => {
                Ok(RadrootsOperationalListingLifecycleState::Published)
            }
            Self::SaveDraft { .. } => Ok(RadrootsOperationalListingLifecycleState::Draft),
            Self::Archive { .. } => {
                Err(RadrootsOperationalListingMutationError::UnsupportedMutation)
            }
        }
    }

    pub fn canonical_draft(
        &self,
    ) -> Result<&RadrootsOperationalListingCanonicalEdit, RadrootsOperationalListingMutationError>
    {
        match self {
            Self::Publish { draft } | Self::Update { draft } | Self::SaveDraft { draft } => {
                Ok(draft)
            }
            Self::Archive { .. } => {
                Err(RadrootsOperationalListingMutationError::UnsupportedMutation)
            }
        }
    }

    pub fn listing_addr(
        &self,
    ) -> Result<&ClassifiedListingAddress, RadrootsOperationalListingMutationError> {
        match self {
            Self::Publish { draft } | Self::Update { draft } => Ok(draft.public_listing_addr()),
            Self::SaveDraft { draft } => Ok(draft.public_listing_addr()),
            Self::Archive { .. } => {
                Err(RadrootsOperationalListingMutationError::UnsupportedMutation)
            }
        }
    }
}

#[cfg(feature = "serde_json")]
pub fn build_operational_listing_mutation_draft(
    mutation: &RadrootsOperationalListingMutation,
    created_at: u64,
) -> Result<EventDraft, RadrootsOperationalListingMutationError> {
    let (draft, kind, contract_id) = match mutation {
        RadrootsOperationalListingMutation::Publish { draft }
        | RadrootsOperationalListingMutation::Update { draft } => (
            draft,
            KIND_CLASSIFIED_LISTING,
            OPERATIONAL_LISTING_PUBLISHED_CONTRACT_ID,
        ),
        RadrootsOperationalListingMutation::SaveDraft { .. }
        | RadrootsOperationalListingMutation::Archive { .. } => {
            return Err(RadrootsOperationalListingMutationError::UnsupportedMutation);
        }
    };
    let parts = to_wire_parts_with_kind(draft.listing(), kind).map_err(|error| {
        RadrootsOperationalListingMutationError::EncodeListing(error.to_string())
    })?;
    EventDraft::new(
        contract_id,
        parts.kind,
        created_at,
        parts.tags,
        parts.content,
        draft.seller_pubkey().to_hex(),
    )
    .map_err(RadrootsOperationalListingMutationError::FrozenDraft)
}

#[cfg(test)]
mod tests {
    use nostr::{EventBuilder, JsonUtil, Keys, Kind, Tag, Timestamp};
    use radroots_core::{Currency, Decimal, Money, Quantity, QuantityPrice, Unit};
    use radroots_event::{
        contract::validate_event_contract_shape,
        draft::EventDraft,
        envelope::kind::KIND_CLASSIFIED_LISTING,
        farm::FarmRef,
        farm::resource_area::ResourceAreaRef,
        id::{ClassifiedListingAddress, DTag, InventoryBinId},
        listing::operational::{
            OperationalListing, OperationalListingAvailability, OperationalListingBin,
            OperationalListingDeliveryMethod, OperationalListingProduct,
            OperationalListingPublicLocation, OperationalListingStatus,
        },
        wire::Nip01EventWire,
    };
    use radroots_event_codec::verification::verify_nip01_event;
    use radroots_identity::PublicKey;
    use radroots_test_fixtures::{FIXTURE_ALICE_PUBLIC_KEY_HEX, FIXTURE_ALICE_SECRET_KEY_HEX};

    use crate::operational_listing::draft::RadrootsOperationalListingCanonicalEdit;
    use crate::operational_listing::validation::validate_operational_listing_event;

    use super::{
        OPERATIONAL_LISTING_PUBLISHED_CONTRACT_ID, RadrootsOperationalListingLifecycleState,
        RadrootsOperationalListingMutation, RadrootsOperationalListingMutationError,
        build_operational_listing_mutation_draft,
    };

    const SELLER: &str = FIXTURE_ALICE_PUBLIC_KEY_HEX;

    fn d_tag(raw: &str) -> DTag {
        DTag::parse(raw).expect("d tag")
    }

    fn bin_id(raw: &str) -> InventoryBinId {
        InventoryBinId::parse(raw).expect("bin id")
    }

    fn sign_draft(draft: &EventDraft) -> radroots_event::envelope::EventEnvelope {
        let keys = Keys::parse(FIXTURE_ALICE_SECRET_KEY_HEX).expect("fixture signing key");
        assert_eq!(keys.public_key().to_hex(), draft.expected_pubkey().to_hex());
        let tags = draft
            .tags_as_vec()
            .into_iter()
            .map(|tag| Tag::parse(tag).expect("draft tag"))
            .collect::<Vec<_>>();
        let event = EventBuilder::new(
            Kind::Custom(u16::try_from(draft.kind_u32()).expect("NIP-01 kind")),
            draft.content(),
        )
        .tags(tags)
        .allow_self_tagging()
        .custom_created_at(Timestamp::from_secs(draft.created_at_u64()))
        .sign_with_keys(&keys)
        .expect("signed listing event");
        assert_eq!(event.id.to_hex(), draft.expected_event_id_hex());
        let raw_json = event.as_json();
        Nip01EventWire::parse_json(raw_json.as_str())
            .expect("canonical event wire")
            .into_envelope()
            .expect("event envelope")
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
                primary: "Farm".to_string(),
                city: Some("Town".to_string()),
                region: Some("Region".to_string()),
                country: Some("US".to_string()),
                geohash: "9q8yy".to_string(),
            }),
            images: None,
        }
    }

    fn canonical_draft() -> RadrootsOperationalListingCanonicalEdit {
        RadrootsOperationalListingCanonicalEdit::new(
            listing(),
            PublicKey::from_hex(SELLER).expect("seller"),
        )
        .expect("canonical listing edit")
    }

    #[test]
    fn supported_mutations_report_lifecycle_states() {
        assert_eq!(
            RadrootsOperationalListingMutation::publish(canonical_draft())
                .lifecycle_state()
                .expect("state"),
            RadrootsOperationalListingLifecycleState::Published
        );
        assert_eq!(
            RadrootsOperationalListingMutation::update(canonical_draft())
                .lifecycle_state()
                .expect("state"),
            RadrootsOperationalListingLifecycleState::Published
        );
        assert_eq!(
            RadrootsOperationalListingMutation::save_draft(canonical_draft())
                .lifecycle_state()
                .expect("state"),
            RadrootsOperationalListingLifecycleState::Draft
        );
    }

    #[test]
    fn supported_mutations_expose_canonical_drafts() {
        let publish = RadrootsOperationalListingMutation::publish(canonical_draft());
        let update = RadrootsOperationalListingMutation::update(canonical_draft());
        let save_draft = RadrootsOperationalListingMutation::save_draft(canonical_draft());

        assert_eq!(
            publish
                .canonical_draft()
                .expect("draft")
                .seller_pubkey()
                .to_hex(),
            SELLER
        );
        assert_eq!(
            update
                .canonical_draft()
                .expect("draft")
                .seller_pubkey()
                .to_hex(),
            SELLER
        );
        assert_eq!(
            save_draft
                .canonical_draft()
                .expect("draft")
                .seller_pubkey()
                .to_hex(),
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
        let publish = RadrootsOperationalListingMutation::publish(canonical_draft());
        let update = RadrootsOperationalListingMutation::update(canonical_draft());
        let save_draft = RadrootsOperationalListingMutation::save_draft(canonical_draft());

        assert_eq!(
            publish.listing_addr().expect("address").as_str(),
            format!("{KIND_CLASSIFIED_LISTING}:{SELLER}:AAAAAAAAAAAAAAAAAAAAAg")
        );
        assert_eq!(
            update.listing_addr().expect("address").as_str(),
            format!("{KIND_CLASSIFIED_LISTING}:{SELLER}:AAAAAAAAAAAAAAAAAAAAAg")
        );
        assert_eq!(
            save_draft.listing_addr().expect("address").as_str(),
            format!("{KIND_CLASSIFIED_LISTING}:{SELLER}:AAAAAAAAAAAAAAAAAAAAAg")
        );
    }

    #[test]
    fn archive_is_explicitly_unsupported() {
        let archive = RadrootsOperationalListingMutation::archive(
            ClassifiedListingAddress::parse(format!(
                "{KIND_CLASSIFIED_LISTING}:{SELLER}:AAAAAAAAAAAAAAAAAAAAAg"
            ))
            .expect("listing address"),
        );

        assert_eq!(
            archive.lifecycle_state().unwrap_err(),
            RadrootsOperationalListingMutationError::UnsupportedMutation
        );
        assert_eq!(
            archive.canonical_draft().unwrap_err(),
            RadrootsOperationalListingMutationError::UnsupportedMutation
        );
        assert_eq!(
            archive.listing_addr().unwrap_err(),
            RadrootsOperationalListingMutationError::UnsupportedMutation
        );
    }

    #[test]
    fn build_operational_listing_mutation_draft_maps_publish_and_update_to_published_listing() {
        let publish = RadrootsOperationalListingMutation::publish(canonical_draft());
        let update = RadrootsOperationalListingMutation::update(canonical_draft());

        let publish_draft =
            build_operational_listing_mutation_draft(&publish, 1_700_000_000).expect("draft");
        let update_draft =
            build_operational_listing_mutation_draft(&update, 1_700_000_000).expect("draft");

        assert_eq!(publish_draft.kind_u32(), KIND_CLASSIFIED_LISTING);
        assert_eq!(
            publish_draft.contract_id(),
            OPERATIONAL_LISTING_PUBLISHED_CONTRACT_ID
        );
        assert_eq!(publish_draft.expected_pubkey().to_hex(), SELLER);
        assert_eq!(publish_draft.created_at_u64(), 1_700_000_000);
        assert_eq!(publish_draft.content(), "# Coffee\n\nSingle origin coffee");
        assert_eq!(update_draft.kind_u32(), KIND_CLASSIFIED_LISTING);
        assert_eq!(
            update_draft.contract_id(),
            OPERATIONAL_LISTING_PUBLISHED_CONTRACT_ID
        );
        assert_eq!(update_draft.expected_pubkey().to_hex(), SELLER);
    }

    #[test]
    fn build_operational_listing_mutation_draft_rejects_save_draft() {
        let save_draft = RadrootsOperationalListingMutation::save_draft(canonical_draft());

        assert_eq!(
            build_operational_listing_mutation_draft(&save_draft, 1_700_000_000).unwrap_err(),
            RadrootsOperationalListingMutationError::UnsupportedMutation
        );
    }

    #[test]
    fn build_operational_listing_mutation_draft_rejects_archive() {
        let archive = RadrootsOperationalListingMutation::archive(
            ClassifiedListingAddress::parse(format!(
                "{KIND_CLASSIFIED_LISTING}:{SELLER}:AAAAAAAAAAAAAAAAAAAAAg"
            ))
            .expect("listing address"),
        );

        assert_eq!(
            build_operational_listing_mutation_draft(&archive, 1_700_000_000).unwrap_err(),
            RadrootsOperationalListingMutationError::UnsupportedMutation
        );
    }

    #[test]
    fn build_operational_listing_mutation_draft_reports_encode_errors() {
        let mut listing = listing();
        listing.resource_area = Some(ResourceAreaRef {
            pubkey: SELLER.to_string(),
            d_tag: "bad d tag".to_string(),
        });
        let draft = RadrootsOperationalListingCanonicalEdit::new(
            listing,
            PublicKey::from_hex(SELLER).expect("seller"),
        )
        .expect("canonical listing edit");
        let publish = RadrootsOperationalListingMutation::publish(draft);

        let err = build_operational_listing_mutation_draft(&publish, 1_700_000_000).unwrap_err();

        assert!(matches!(
            err,
            RadrootsOperationalListingMutationError::EncodeListing(_)
        ));
    }

    #[test]
    fn build_operational_listing_mutation_draft_event_id_is_stable_for_fixed_input() {
        let publish = RadrootsOperationalListingMutation::publish(canonical_draft());

        let first =
            build_operational_listing_mutation_draft(&publish, 1_700_000_000).expect("draft");
        let second =
            build_operational_listing_mutation_draft(&publish, 1_700_000_000).expect("draft");

        assert_eq!(
            first.expected_event_id_hex(),
            second.expected_event_id_hex()
        );
        assert_eq!(first.expected_event_id_hex().len(), 64);
        assert_eq!(first.tags_as_vec(), second.tags_as_vec());
        assert_eq!(first.content(), second.content());
    }

    #[test]
    fn build_operational_listing_mutation_draft_output_validates_as_operational_listing() {
        let publish = RadrootsOperationalListingMutation::publish(canonical_draft());
        let draft =
            build_operational_listing_mutation_draft(&publish, 1_700_000_000).expect("draft");

        let signed = sign_draft(&draft);
        validate_event_contract_shape(&signed, OPERATIONAL_LISTING_PUBLISHED_CONTRACT_ID)
            .expect("operational listing contract");
        let verified = verify_nip01_event(signed).expect("verified listing");
        let validated = validate_operational_listing_event(&verified).expect("validated listing");

        assert_eq!(validated.seller_pubkey, SELLER);
        assert!(
            validated
                .listing_addr
                .as_str()
                .contains(&format!(":{SELLER}:"))
        );
    }
}
