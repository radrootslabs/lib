mod codec;
pub mod draft;
pub mod model;
pub mod mutation;
pub mod price_ext;
pub mod validation;

use radroots_events::{
    RadrootsNostrEvent,
    ids::{
        RadrootsAddressableCoordinateParts, RadrootsDTag, RadrootsIdParseError,
        RadrootsListingAddress, RadrootsPublicKey,
    },
    kinds::{KIND_LISTING, is_listing_kind},
    listing::RadrootsListing,
};
use thiserror::Error;

pub use self::draft::{
    RadrootsCanonicalListingDraft, RadrootsListingDraftDocumentV1, RadrootsListingDraftError,
    canonicalize_listing_draft,
};
#[cfg(feature = "serde_json")]
pub use self::mutation::build_listing_mutation_draft;
pub use self::mutation::{
    RadrootsListingLifecycleState, RadrootsListingMutation, RadrootsListingMutationError,
};
pub use radroots_events::order::RadrootsListingParseError as ListingParseError;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum RadrootsListingAddressError {
    #[error("invalid listing address: {0}")]
    InvalidAddress(RadrootsIdParseError),
    #[error("listing address must reference a listing kind")]
    InvalidKind { actual: u32 },
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum RadrootsPublicListingAddressError {
    #[error("invalid listing address: {0}")]
    InvalidAddress(RadrootsIdParseError),
    #[error("listing address must reference a listing kind")]
    InvalidListingKind { actual: u32 },
    #[error("listing address must reference a public NIP-99 listing")]
    InvalidKind { actual: u32 },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsListingAddressParts {
    pub address: RadrootsListingAddress,
    pub kind: u32,
    pub seller_pubkey: RadrootsPublicKey,
    pub listing_id: RadrootsDTag,
}

impl RadrootsListingAddressParts {
    pub fn parse(value: impl AsRef<str>) -> Result<Self, RadrootsListingAddressError> {
        parse_listing_address(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsPublicListingAddress {
    pub address: RadrootsListingAddress,
    pub kind: u32,
    pub seller_pubkey: RadrootsPublicKey,
    pub listing_id: RadrootsDTag,
}

impl RadrootsPublicListingAddress {
    pub fn parse(value: impl AsRef<str>) -> Result<Self, RadrootsPublicListingAddressError> {
        parse_public_listing_address(value)
    }
}

pub fn parse_listing_address(
    value: impl AsRef<str>,
) -> Result<RadrootsListingAddressParts, RadrootsListingAddressError> {
    let value = value.as_ref();
    let address = RadrootsListingAddress::parse(value)
        .map_err(RadrootsListingAddressError::InvalidAddress)?;
    let parts = RadrootsAddressableCoordinateParts::parse(address.as_str())
        .map_err(RadrootsListingAddressError::InvalidAddress)?;
    if !is_listing_kind(parts.kind) {
        return Err(RadrootsListingAddressError::InvalidKind { actual: parts.kind });
    }
    Ok(RadrootsListingAddressParts {
        address,
        kind: parts.kind,
        seller_pubkey: parts.pubkey,
        listing_id: parts.d_tag,
    })
}

pub fn parse_public_listing_address(
    value: impl AsRef<str>,
) -> Result<RadrootsPublicListingAddress, RadrootsPublicListingAddressError> {
    let parts = parse_listing_address(value).map_err(|error| match error {
        RadrootsListingAddressError::InvalidAddress(error) => {
            RadrootsPublicListingAddressError::InvalidAddress(error)
        }
        RadrootsListingAddressError::InvalidKind { actual } => {
            RadrootsPublicListingAddressError::InvalidListingKind { actual }
        }
    })?;
    if parts.kind != KIND_LISTING {
        return Err(RadrootsPublicListingAddressError::InvalidKind { actual: parts.kind });
    }
    Ok(RadrootsPublicListingAddress {
        address: parts.address,
        kind: parts.kind,
        seller_pubkey: parts.seller_pubkey,
        listing_id: parts.listing_id,
    })
}

pub fn parse_listing_event(
    event: &RadrootsNostrEvent,
) -> Result<RadrootsListing, ListingParseError> {
    if !is_listing_kind(event.kind) {
        return Err(ListingParseError::InvalidKind(event.kind));
    }
    self::codec::listing_from_event_parts(&event.tags, &event.content)
}

#[cfg(test)]
mod tests {
    use super::{
        RadrootsListingAddressError, RadrootsPublicListingAddressError, parse_listing_address,
        parse_listing_event, parse_public_listing_address,
    };
    use radroots_events::{
        RadrootsNostrEvent,
        kinds::{KIND_LISTING, KIND_LISTING_DRAFT, KIND_PROFILE},
        order::RadrootsListingParseError,
    };

    const SELLER: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    #[test]
    fn parse_listing_event_rejects_non_listing_kind() {
        let event = RadrootsNostrEvent {
            id: "event-1".into(),
            author: "seller".into(),
            created_at: 1,
            kind: KIND_PROFILE,
            tags: vec![],
            content: String::new(),
            sig: String::new(),
        };

        assert!(matches!(
            parse_listing_event(&event),
            Err(RadrootsListingParseError::InvalidKind(KIND_PROFILE))
        ));
    }

    #[test]
    fn parse_public_listing_address_accepts_public_listing_kind() {
        let raw = format!("{KIND_LISTING}:{SELLER}:listing-1");
        let parsed = parse_public_listing_address(&raw).expect("public listing address");

        assert_eq!(parsed.address.as_str(), raw);
        assert_eq!(parsed.kind, KIND_LISTING);
        assert_eq!(parsed.seller_pubkey.as_str(), SELLER);
        assert_eq!(parsed.listing_id.as_str(), "listing-1");
    }

    #[test]
    fn parse_listing_address_accepts_draft_listing_kind() {
        let raw = format!("{KIND_LISTING_DRAFT}:{SELLER}:listing-1");
        let parsed = parse_listing_address(&raw).expect("listing address");

        assert_eq!(parsed.address.as_str(), raw);
        assert_eq!(parsed.kind, KIND_LISTING_DRAFT);
        assert_eq!(parsed.seller_pubkey.as_str(), SELLER);
        assert_eq!(parsed.listing_id.as_str(), "listing-1");
    }

    #[test]
    fn parse_public_listing_address_rejects_draft_listing_kind() {
        let raw = format!("{KIND_LISTING_DRAFT}:{SELLER}:listing-1");

        assert!(matches!(
            parse_public_listing_address(&raw),
            Err(RadrootsPublicListingAddressError::InvalidKind {
                actual: KIND_LISTING_DRAFT
            })
        ));
    }

    #[test]
    fn parse_listing_address_rejects_non_listing_kind() {
        let raw = format!("{KIND_PROFILE}:{SELLER}:listing-1");

        assert!(matches!(
            parse_listing_address(&raw),
            Err(RadrootsListingAddressError::InvalidKind {
                actual: KIND_PROFILE
            })
        ));
    }
}
