use radroots_core::{
    RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCoreQuantity,
    RadrootsCoreQuantityPrice, RadrootsCoreUnit,
};
use radroots_events::farm::{RadrootsFarm, RadrootsFarmRef};
use radroots_events::kinds::{KIND_FARM, KIND_LISTING, KIND_ORDER_REQUEST, KIND_PROFILE};
use radroots_events::listing::{
    RadrootsListing, RadrootsListingAvailability, RadrootsListingBin,
    RadrootsListingDeliveryMethod, RadrootsListingLocation, RadrootsListingProduct,
    RadrootsListingStatus,
};
use radroots_events::order::{
    RadrootsOrderEconomicItem, RadrootsOrderEconomics, RadrootsOrderItem,
    RadrootsOrderPricingBasis, RadrootsOrderRequest,
};
use radroots_events::profile::{RadrootsProfile, RadrootsProfileType};
use radroots_sdk::{RadrootsNostrEvent, RadrootsNostrEventPtr, farm, listing, order, profile};

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
        published_at: None,
        farm: RadrootsFarmRef {
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

fn listing_event_ptr() -> RadrootsNostrEventPtr {
    RadrootsNostrEventPtr {
        id: "listing-event-1".into(),
        relays: Some("wss://listing.relay.example".into()),
    }
}

fn sample_order_request() -> RadrootsOrderRequest {
    RadrootsOrderRequest {
        order_id: "order-1".into(),
        listing_addr: format!("{KIND_LISTING}:seller:AAAAAAAAAAAAAAAAAAAAAg"),
        buyer_pubkey: "buyer".into(),
        seller_pubkey: "seller".into(),
        items: vec![RadrootsOrderItem {
            bin_id: "bin-1".into(),
            bin_count: 2,
        }],
        economics: RadrootsOrderEconomics {
            quote_id: "quote-1".into(),
            quote_version: 1,
            pricing_basis: RadrootsOrderPricingBasis::ListingEvent,
            currency: RadrootsCoreCurrency::USD,
            items: vec![RadrootsOrderEconomicItem {
                bin_id: "bin-1".into(),
                bin_count: 2,
                quantity_amount: RadrootsCoreDecimal::from(1u32),
                quantity_unit: RadrootsCoreUnit::Each,
                unit_price_amount: RadrootsCoreDecimal::from(5u32),
                unit_price_currency: RadrootsCoreCurrency::USD,
                line_subtotal: RadrootsCoreMoney::new(
                    RadrootsCoreDecimal::from(10u32),
                    RadrootsCoreCurrency::USD,
                ),
            }],
            discounts: Vec::new(),
            adjustments: Vec::new(),
            subtotal: RadrootsCoreMoney::new(
                RadrootsCoreDecimal::from(10u32),
                RadrootsCoreCurrency::USD,
            ),
            discount_total: RadrootsCoreMoney::new(
                RadrootsCoreDecimal::from(0u32),
                RadrootsCoreCurrency::USD,
            ),
            adjustment_total: RadrootsCoreMoney::new(
                RadrootsCoreDecimal::from(0u32),
                RadrootsCoreCurrency::USD,
            ),
            total: RadrootsCoreMoney::new(
                RadrootsCoreDecimal::from(10u32),
                RadrootsCoreCurrency::USD,
            ),
        },
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
    assert!(
        parts
            .tags
            .iter()
            .any(|tag| tag.first().map(|value| value.as_str()) == Some("d"))
    );
}

#[test]
fn listing_facade_wraps_build_parse_and_validate() {
    let listing_value = sample_listing();
    let tags = listing::build_tags(&listing_value).expect("listing tags");
    assert!(!tags.is_empty());

    let event = listing_event(&listing_value);
    let parsed = listing::parse_event(&event).expect("parsed listing");
    assert_eq!(parsed.d_tag, listing_value.d_tag);

    let validated = order::validate_listing_event(&event).expect("validated listing");
    assert_eq!(validated.listing_id, listing_value.d_tag);
    assert_eq!(event.kind, KIND_LISTING);
}

#[test]
fn listing_parse_rejects_non_listing_kind() {
    let listing_value = sample_listing();
    let mut event = listing_event(&listing_value);
    event.kind = KIND_PROFILE;

    assert_eq!(
        listing::parse_event(&event).expect_err("listing kind error"),
        listing::RadrootsListingParseError::InvalidKind(KIND_PROFILE)
    );
}

#[test]
fn order_facade_wraps_build_parse_and_address_ops() {
    let listing_value = sample_listing();
    let listing_addr = format!("{KIND_LISTING}:seller:{}", listing_value.d_tag);
    let payload = sample_order_request();
    let parts =
        order::build_order_request_draft(&listing_event_ptr(), &payload).expect("order draft");

    assert_eq!(parts.as_wire_parts().kind, KIND_ORDER_REQUEST);

    let parsed_addr = order::parse_listing_address(&listing_addr).expect("listing address");
    assert_eq!(parsed_addr.listing_id, listing_value.d_tag);

    let event = RadrootsNostrEvent {
        id: "order-event".into(),
        author: "buyer".into(),
        created_at: 2,
        kind: parts.as_wire_parts().kind,
        tags: parts.as_wire_parts().tags.clone(),
        content: parts.as_wire_parts().content.clone(),
        sig: String::new(),
    };
    let envelope = order::parse_order_request(&event).expect("order envelope");
    assert_eq!(envelope.payload.order_id, payload.order_id);
    assert_eq!(envelope.payload.listing_addr, listing_addr);
}
