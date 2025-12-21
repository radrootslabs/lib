#![cfg(feature = "serde_json")]

use radroots_core::{
    RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCoreQuantity,
    RadrootsCoreQuantityPrice, RadrootsCoreUnit,
};
use radroots_events::listing::{
    RadrootsListing, RadrootsListingProduct, RadrootsListingQuantity,
};
use radroots_events::tags::TAG_D;
use radroots_events_codec::error::{EventEncodeError, EventParseError};
use radroots_events_codec::listing::decode::listing_from_event;
use radroots_events_codec::listing::encode::{listing_build_tags, to_wire_parts};

fn sample_listing(d_tag: &str) -> RadrootsListing {
    let quantity = RadrootsCoreQuantity::new(RadrootsCoreDecimal::from(1u32), RadrootsCoreUnit::Each);
    let price = RadrootsCoreQuantityPrice::new(
        RadrootsCoreMoney::new(RadrootsCoreDecimal::from(10u32), RadrootsCoreCurrency::USD),
        quantity.clone(),
    );

    RadrootsListing {
        d_tag: d_tag.to_string(),
        product: RadrootsListingProduct {
            key: "sku".to_string(),
            title: "Widget".to_string(),
            category: "Tools".to_string(),
            summary: None,
            process: None,
            lot: None,
            location: None,
            profile: None,
            year: None,
        },
        quantities: vec![RadrootsListingQuantity {
            value: quantity,
            label: None,
            count: Some(1),
        }],
        prices: vec![price],
        discounts: None,
        location: None,
        images: None,
    }
}

#[test]
fn listing_build_tags_requires_d_tag() {
    let listing = sample_listing("");
    let err = listing_build_tags(&listing).unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("d")
    ));
}

#[test]
fn listing_roundtrip_from_event() {
    let listing = sample_listing("listing-1");
    let parts = to_wire_parts(&listing).unwrap();

    let decoded = listing_from_event(parts.kind, &parts.tags, &parts.content).unwrap();
    assert_eq!(decoded.d_tag, listing.d_tag);
    assert_eq!(decoded.product.key, listing.product.key);
    assert_eq!(decoded.product.title, listing.product.title);
    assert_eq!(decoded.quantities.len(), listing.quantities.len());
    assert_eq!(decoded.prices.len(), listing.prices.len());
}

#[test]
fn listing_from_event_fills_missing_d_tag() {
    let listing = sample_listing("");
    let content = serde_json::to_string(&listing).unwrap();
    let tags = vec![vec![TAG_D.to_string(), "filled".to_string()]];

    let decoded = listing_from_event(30402, &tags, &content).unwrap();
    assert_eq!(decoded.d_tag, "filled");
}

#[test]
fn listing_from_event_rejects_mismatched_d_tag() {
    let listing = sample_listing("a");
    let content = serde_json::to_string(&listing).unwrap();
    let tags = vec![vec![TAG_D.to_string(), "b".to_string()]];

    let err = listing_from_event(30402, &tags, &content).unwrap_err();
    assert!(matches!(err, EventParseError::InvalidTag(TAG_D)));
}

#[test]
fn listing_from_event_rejects_wrong_kind() {
    let listing = sample_listing("listing-1");
    let content = serde_json::to_string(&listing).unwrap();
    let tags = vec![vec![TAG_D.to_string(), "listing-1".to_string()]];

    let err = listing_from_event(1, &tags, &content).unwrap_err();
    assert!(matches!(
        err,
        EventParseError::InvalidKind {
            expected: "30402",
            got: 1
        }
    ));
}
