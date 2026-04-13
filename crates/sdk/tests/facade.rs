use radroots_core::{
    RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCoreQuantity,
    RadrootsCoreQuantityPrice, RadrootsCoreUnit,
};
use radroots_events::farm::RadrootsFarm;
use radroots_events::kinds::{
    KIND_FARM, KIND_LISTING, KIND_PROFILE, KIND_TRADE_LISTING_VALIDATE_REQ,
};
use radroots_events::listing::{
    RadrootsListing, RadrootsListingAvailability, RadrootsListingBin,
    RadrootsListingDeliveryMethod, RadrootsListingFarmRef, RadrootsListingLocation,
    RadrootsListingProduct, RadrootsListingStatus,
};
use radroots_events::profile::{RadrootsProfile, RadrootsProfileType};
use radroots_events::trade::{RadrootsTradeListingValidateRequest, RadrootsTradeMessagePayload};
use radroots_sdk::{
    RadrootsNostrEvent, farm, listing, profile, trade,
};

fn sample_profile() -> RadrootsProfile {
    RadrootsProfile {
        name: "North Farm".into(),
        display_name: Some("North Farm".into()),
        nip05: None,
        about: Some("Organic coffee".into()),
        website: Some("https://example.com".into()),
        picture: None,
        banner: None,
        lud06: None,
        lud16: None,
        bot: None,
    }
}

fn sample_farm() -> RadrootsFarm {
    RadrootsFarm {
        d_tag: "AAAAAAAAAAAAAAAAAAAAAA".into(),
        name: "North Farm".into(),
        about: Some("Organic coffee".into()),
        website: None,
        picture: None,
        banner: None,
        location: None,
        tags: Some(vec!["coffee".into()]),
    }
}

fn sample_listing() -> RadrootsListing {
    RadrootsListing {
        d_tag: "AAAAAAAAAAAAAAAAAAAAAg".into(),
        farm: RadrootsListingFarmRef {
            pubkey: "seller".into(),
            d_tag: "AAAAAAAAAAAAAAAAAAAAAA".into(),
        },
        product: RadrootsListingProduct {
            key: "coffee".into(),
            title: "Coffee".into(),
            category: "coffee".into(),
            summary: Some("Single origin coffee".into()),
            process: None,
            lot: None,
            location: None,
            profile: None,
            year: None,
        },
        primary_bin_id: "bin-1".into(),
        bins: vec![RadrootsListingBin {
            bin_id: "bin-1".into(),
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
            primary: "North Farm".into(),
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

fn listing_event(listing_value: &RadrootsListing) -> RadrootsNostrEvent {
    let parts = listing::build_draft(listing_value).expect("listing draft");
    RadrootsNostrEvent {
        id: "event-1".into(),
        author: "seller".into(),
        created_at: 1,
        kind: parts.as_wire_parts().kind,
        tags: parts.as_wire_parts().tags.clone(),
        content: parts.as_wire_parts().content.clone(),
        sig: String::new(),
    }
}

#[test]
fn profile_build_draft_wraps_profile_encoder() {
    let parts =
        profile::build_draft(&sample_profile(), Some(RadrootsProfileType::Farm)).expect("profile");

    assert_eq!(parts.kind, KIND_PROFILE);
    assert!(parts.tags.iter().any(|tag| {
        tag.first().map(|value| value.as_str()) == Some("t")
            && tag.get(1).map(|value| value.as_str()) == Some("radroots:type:farm")
    }));
}

#[test]
fn farm_build_draft_wraps_farm_encoder() {
    let parts = farm::build_draft(&sample_farm()).expect("farm");

    assert_eq!(parts.kind, KIND_FARM);
    assert!(parts
        .tags
        .iter()
        .any(|tag| tag.first().map(|value| value.as_str()) == Some("d")));
}

#[test]
fn listing_facade_wraps_build_parse_and_validate() {
    let listing_value = sample_listing();
    let tags = listing::build_tags(&listing_value).expect("listing tags");
    assert!(!tags.is_empty());

    let event = listing_event(&listing_value);
    let parsed = listing::parse_event(&event).expect("parsed listing");
    assert_eq!(parsed.d_tag, listing_value.d_tag);

    let validated = trade::validate_listing_event(&event).expect("validated listing");
    assert_eq!(validated.listing_id, listing_value.d_tag);
    assert_eq!(event.kind, KIND_LISTING);
}

#[test]
fn listing_parse_rejects_non_listing_kind() {
    let listing_value = sample_listing();
    let mut event = listing_event(&listing_value);
    event.kind = KIND_PROFILE;

    assert!(matches!(
        listing::parse_event(&event),
        Err(listing::RadrootsTradeListingParseError::InvalidKind(KIND_PROFILE))
    ));
}

#[test]
fn trade_facade_wraps_build_parse_and_address_ops() {
    let listing_value = sample_listing();
    let listing_addr = format!("{KIND_LISTING}:seller:{}", listing_value.d_tag);
    let payload =
        RadrootsTradeMessagePayload::ListingValidateRequest(RadrootsTradeListingValidateRequest {
            listing_event: None,
        });

    let parts = trade::build_envelope_draft(
        "buyer",
        payload.message_type(),
        listing_addr.clone(),
        None,
        None,
        None,
        None,
        &payload,
    )
    .expect("trade envelope draft");

    assert_eq!(parts.kind, KIND_TRADE_LISTING_VALIDATE_REQ);

    let parsed_addr = trade::parse_listing_address(&listing_addr).expect("listing address");
    assert_eq!(parsed_addr.listing_id, listing_value.d_tag);

    let event = RadrootsNostrEvent {
        id: "trade-event".into(),
        author: "seller".into(),
        created_at: 2,
        kind: parts.kind,
        tags: parts.tags,
        content: parts.content,
        sig: String::new(),
    };
    let envelope = trade::parse_envelope(&event).expect("trade envelope");
    assert_eq!(envelope.message_type, payload.message_type());
    assert_eq!(envelope.listing_addr, listing_addr);
}
