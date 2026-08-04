pub mod draft;
pub mod model;
pub mod mutation;
pub mod price_ext;
pub mod validation;

use radroots_event::{
    envelope::EventEnvelope,
    id::{AddressableCoordinateParts, ClassifiedListingAddress, DTag, ParseError},
    listing::operational::{OperationalListing, OperationalListingParseError},
};
use radroots_event_codec::decode::operational_listing::operational_listing_from_nostr_event;
use radroots_identity::PublicKey;

pub use self::draft::{
    RadrootsOperationalListingCanonicalEdit, RadrootsOperationalListingEditDocumentV1,
    RadrootsOperationalListingEditError, canonicalize_operational_listing_edit,
};
pub use self::model::{RadrootsOperationalListingSubtotal, RadrootsOperationalListingTotal};
pub use self::mutation::build_operational_listing_mutation_draft;
pub use self::mutation::{
    RadrootsOperationalListingLifecycleState, RadrootsOperationalListingMutation,
    RadrootsOperationalListingMutationError,
};
pub use self::price_ext::BinPricingTryExt;
pub use self::validation::{
    RadrootsOperationalListingTradeProjection, validate_operational_listing_event,
    validate_operational_listing_model,
};
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsClassifiedListingAddressParts {
    pub address: ClassifiedListingAddress,
    pub kind: u32,
    pub seller_pubkey: PublicKey,
    pub listing_id: DTag,
}

impl RadrootsClassifiedListingAddressParts {
    pub fn parse(value: impl AsRef<str>) -> Result<Self, ParseError> {
        parse_classified_listing_address(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsPublicClassifiedListingAddress {
    pub address: ClassifiedListingAddress,
    pub kind: u32,
    pub seller_pubkey: PublicKey,
    pub listing_id: DTag,
}

impl RadrootsPublicClassifiedListingAddress {
    pub fn parse(value: impl AsRef<str>) -> Result<Self, ParseError> {
        parse_public_classified_listing_address(value)
    }
}

pub fn parse_classified_listing_address(
    value: impl AsRef<str>,
) -> Result<RadrootsClassifiedListingAddressParts, ParseError> {
    let value = value.as_ref();
    let address = ClassifiedListingAddress::parse(value)?;
    let parts = AddressableCoordinateParts::parse(address.as_str())
        .expect("typed listing address must contain valid coordinate parts");
    Ok(RadrootsClassifiedListingAddressParts {
        address,
        kind: parts.kind,
        seller_pubkey: parts.pubkey,
        listing_id: parts.d_tag,
    })
}

pub fn parse_public_classified_listing_address(
    value: impl AsRef<str>,
) -> Result<RadrootsPublicClassifiedListingAddress, ParseError> {
    let parts = parse_classified_listing_address(value)?;
    Ok(RadrootsPublicClassifiedListingAddress {
        address: parts.address,
        kind: parts.kind,
        seller_pubkey: parts.seller_pubkey,
        listing_id: parts.listing_id,
    })
}

pub fn parse_operational_listing_event(
    event: &EventEnvelope,
) -> Result<OperationalListing, OperationalListingParseError> {
    operational_listing_from_nostr_event(event)
}
