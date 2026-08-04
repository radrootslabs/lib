//! Mutation draft preparation for Radroots Listing v1.

#![forbid(unsafe_code)]

use core::fmt;

use std::string::{String, ToString};

use radroots_event::id::ClassifiedListingAddress;
use radroots_event::{
    draft::{DraftError, EventDraft},
    envelope::kind::KIND_CLASSIFIED_LISTING,
};
use radroots_event_codec::encode::operational_listing::to_wire_parts_with_kind;

use crate::listing::operational_listing::draft::RadrootsOperationalListingCanonicalEdit;

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
    EncodeListing(String),
    FrozenDraft(DraftError),
}

impl fmt::Display for RadrootsOperationalListingMutationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedMutation => f.write_str("listing mutation is not supported"),
            Self::EncodeListing(error) => {
                write!(f, "failed to encode listing mutation: {error}")
            }
            Self::FrozenDraft(error) => {
                write!(f, "failed to build listing mutation draft: {error}")
            }
        }
    }
}

impl core::error::Error for RadrootsOperationalListingMutationError {}

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
