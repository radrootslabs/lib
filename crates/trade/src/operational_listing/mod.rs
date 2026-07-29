pub mod draft;
pub mod model;
pub mod mutation;
pub mod price_ext;
pub mod validation;

use radroots_event::{
    envelope::RadrootsEventEnvelope,
    id::{
        RadrootsAddressableCoordinateParts, RadrootsClassifiedListingAddress, RadrootsDTag,
        RadrootsIdParseError,
    },
    listing::operational::{RadrootsOperationalListing, RadrootsOperationalListingParseError},
};
use radroots_event_codec::operational_listing::decode::operational_listing_from_nostr_event;
use radroots_identity::PublicKey;

pub use self::draft::{
    RadrootsOperationalListingCanonicalEdit, RadrootsOperationalListingEditDocumentV1,
    RadrootsOperationalListingEditError, canonicalize_operational_listing_edit,
};
#[cfg(feature = "serde_json")]
pub use self::mutation::build_operational_listing_mutation_draft;
pub use self::mutation::{
    RadrootsOperationalListingLifecycleState, RadrootsOperationalListingMutation,
    RadrootsOperationalListingMutationError,
};
pub use self::validation::{
    RadrootsOperationalListingTradeProjection, validate_operational_listing_event,
    validate_operational_listing_model,
};
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsClassifiedListingAddressParts {
    pub address: RadrootsClassifiedListingAddress,
    pub kind: u32,
    pub seller_pubkey: PublicKey,
    pub listing_id: RadrootsDTag,
}

impl RadrootsClassifiedListingAddressParts {
    pub fn parse(value: impl AsRef<str>) -> Result<Self, RadrootsIdParseError> {
        parse_classified_listing_address(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsPublicClassifiedListingAddress {
    pub address: RadrootsClassifiedListingAddress,
    pub kind: u32,
    pub seller_pubkey: PublicKey,
    pub listing_id: RadrootsDTag,
}

impl RadrootsPublicClassifiedListingAddress {
    pub fn parse(value: impl AsRef<str>) -> Result<Self, RadrootsIdParseError> {
        parse_public_classified_listing_address(value)
    }
}

pub fn parse_classified_listing_address(
    value: impl AsRef<str>,
) -> Result<RadrootsClassifiedListingAddressParts, RadrootsIdParseError> {
    let value = value.as_ref();
    let address = RadrootsClassifiedListingAddress::parse(value)?;
    let parts = RadrootsAddressableCoordinateParts::parse(address.as_str())
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
) -> Result<RadrootsPublicClassifiedListingAddress, RadrootsIdParseError> {
    let parts = parse_classified_listing_address(value)?;
    Ok(RadrootsPublicClassifiedListingAddress {
        address: parts.address,
        kind: parts.kind,
        seller_pubkey: parts.seller_pubkey,
        listing_id: parts.listing_id,
    })
}

pub fn parse_operational_listing_event(
    event: &RadrootsEventEnvelope,
) -> Result<RadrootsOperationalListing, RadrootsOperationalListingParseError> {
    operational_listing_from_nostr_event(event)
}

#[cfg(test)]
mod tests {
    use super::{
        RadrootsClassifiedListingAddressParts, RadrootsPublicClassifiedListingAddress,
        parse_classified_listing_address, parse_operational_listing_event,
        parse_public_classified_listing_address,
    };
    use radroots_event::{
        envelope::RadrootsEventEnvelope,
        envelope::RadrootsEventEnvelopeParts,
        envelope::kind::{KIND_CLASSIFIED_LISTING, KIND_PROFILE},
        id::RadrootsClassifiedListingAddress,
        listing::operational::RadrootsOperationalListingParseError,
    };

    const SELLER: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn listing_event() -> RadrootsEventEnvelope {
        RadrootsEventEnvelope::new(RadrootsEventEnvelopeParts {
            id: "9".repeat(64),
            author: SELLER.to_string(),
            created_at: 1,
            kind: KIND_CLASSIFIED_LISTING,
            tags: vec![
                vec!["d".into(), "AAAAAAAAAAAAAAAAAAAAAg".into()],
                vec!["p".into(), SELLER.into()],
                vec!["a".into(), format!("30340:{SELLER}:AAAAAAAAAAAAAAAAAAAAAA")],
                vec!["key".into(), "coffee".into()],
                vec!["title".into(), "Coffee".into()],
                vec!["category".into(), "coffee".into()],
                vec!["summary".into(), "Single origin".into()],
                vec!["radroots:primary_bin".into(), "bin-1".into()],
                vec![
                    "radroots:bin".into(),
                    "bin-1".into(),
                    "1000".into(),
                    "g".into(),
                ],
                vec![
                    "radroots:price".into(),
                    "bin-1".into(),
                    "20".into(),
                    "USD".into(),
                    "1".into(),
                    "g".into(),
                ],
            ],
            content: String::new(),
            sig: "f".repeat(128),
        })
        .expect("listing event")
    }

    #[test]
    fn parse_operational_listing_event_rejects_non_listing_kind() {
        let event = RadrootsEventEnvelope::new(RadrootsEventEnvelopeParts {
            id: "8".repeat(64),
            author: SELLER.to_string(),
            created_at: 1,
            kind: KIND_PROFILE,
            tags: vec![],
            content: String::new(),
            sig: "f".repeat(128),
        })
        .expect("profile event");

        assert!(matches!(
            parse_operational_listing_event(&event),
            Err(RadrootsOperationalListingParseError::InvalidKind(
                KIND_PROFILE
            ))
        ));
    }

    #[test]
    fn parse_operational_listing_event_accepts_listing_kind() {
        let listing = parse_operational_listing_event(&listing_event()).expect("listing");

        assert_eq!(listing.d_tag.as_str(), "AAAAAAAAAAAAAAAAAAAAAg");
        assert_eq!(listing.farm.pubkey, SELLER);
        assert_eq!(listing.primary_bin_id.as_str(), "bin-1");
    }

    #[test]
    fn listing_address_associated_parsers_delegate_to_public_parsers() {
        let raw = format!("{KIND_CLASSIFIED_LISTING}:{SELLER}:listing-1");

        let listing =
            RadrootsClassifiedListingAddressParts::parse(raw.clone()).expect("listing address");
        let public = RadrootsPublicClassifiedListingAddress::parse(&raw).expect("public address");
        let typed = parse_public_classified_listing_address(
            RadrootsClassifiedListingAddress::parse(&raw).expect("typed addr"),
        )
        .expect("typed public address");

        assert_eq!(listing.address.as_str(), raw);
        assert_eq!(public.address.as_str(), raw);
        assert_eq!(typed.address.as_str(), raw);
        assert_eq!(listing.seller_pubkey.to_hex(), SELLER);
        assert_eq!(public.seller_pubkey.to_hex(), SELLER);
        assert_eq!(typed.seller_pubkey.to_hex(), SELLER);
    }

    #[test]
    fn parse_public_classified_listing_address_accepts_public_listing_kind() {
        let raw = format!("{KIND_CLASSIFIED_LISTING}:{SELLER}:listing-1");
        let parsed = parse_public_classified_listing_address(&raw).expect("public listing address");

        assert_eq!(parsed.address.as_str(), raw);
        assert_eq!(parsed.kind, KIND_CLASSIFIED_LISTING);
        assert_eq!(parsed.seller_pubkey.to_hex(), SELLER);
        assert_eq!(parsed.listing_id.as_str(), "listing-1");
    }

    #[test]
    fn parse_classified_listing_address_rejects_retired_listing_kind() {
        let raw = format!("30403:{SELLER}:listing-1");

        assert!(matches!(
            parse_classified_listing_address(&raw),
            Err(radroots_event::id::RadrootsIdParseError::UnexpectedKind {
                expected: KIND_CLASSIFIED_LISTING,
                actual: 30403,
            })
        ));
        assert!(matches!(
            parse_public_classified_listing_address(&raw),
            Err(radroots_event::id::RadrootsIdParseError::UnexpectedKind {
                expected: KIND_CLASSIFIED_LISTING,
                actual: 30403,
            })
        ));
    }

    #[test]
    fn parse_public_classified_listing_address_maps_invalid_listing_addresses() {
        assert!(matches!(
            parse_public_classified_listing_address("not-an-address"),
            Err(radroots_event::id::RadrootsIdParseError::InvalidFormat)
        ));

        let raw = format!("{KIND_PROFILE}:{SELLER}:listing-1");
        assert!(matches!(
            parse_public_classified_listing_address(&raw),
            Err(radroots_event::id::RadrootsIdParseError::UnexpectedKind {
                expected: KIND_CLASSIFIED_LISTING,
                actual: KIND_PROFILE,
            })
        ));
        assert!(RadrootsClassifiedListingAddress::parse(&raw).is_err());
    }

    #[test]
    fn parse_classified_listing_address_rejects_non_listing_kind() {
        let raw = format!("{KIND_PROFILE}:{SELLER}:listing-1");

        assert!(matches!(
            parse_classified_listing_address(&raw),
            Err(radroots_event::id::RadrootsIdParseError::UnexpectedKind {
                expected: KIND_CLASSIFIED_LISTING,
                actual: KIND_PROFILE,
            })
        ));
    }
}
