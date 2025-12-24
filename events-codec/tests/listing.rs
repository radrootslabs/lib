#![cfg(feature = "serde_json")]

use radroots_core::{
    RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCoreQuantity,
    RadrootsCoreQuantityPrice, RadrootsCoreUnit,
};
use radroots_events::listing::{
    RadrootsListing, RadrootsListingDiscount, RadrootsListingImage, RadrootsListingImageSize,
    RadrootsListingLocation, RadrootsListingProduct, RadrootsListingQuantity,
};
use radroots_events::tags::TAG_D;
use radroots_events_codec::error::{EventEncodeError, EventParseError};
use radroots_events_codec::listing::decode::listing_from_event;
use radroots_events_codec::listing::encode::{listing_build_tags, to_wire_parts};
use std::str::FromStr;

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
        inventory_available: None,
        availability: None,
        delivery_method: None,
        location: None,
        images: None,
    }
}

fn sample_listing_full(d_tag: &str) -> RadrootsListing {
    let qty_amount = RadrootsCoreDecimal::from_str("1").unwrap();
    let price_amount = RadrootsCoreDecimal::from_str("24.50").unwrap();
    let discount_threshold = RadrootsCoreDecimal::from_str("10").unwrap();
    let discount_amount = RadrootsCoreDecimal::from_str("20").unwrap();

    let quantity = RadrootsCoreQuantity::new(qty_amount, RadrootsCoreUnit::MassLb).with_label("bag");
    let price_quantity =
        RadrootsCoreQuantity::new(qty_amount, RadrootsCoreUnit::MassLb).with_label("bag");

    RadrootsListing {
        d_tag: d_tag.to_string(),
        product: RadrootsListingProduct {
            key: "sku".to_string(),
            title: "Widget".to_string(),
            category: "Tools".to_string(),
            summary: Some("Compact widget".to_string()),
            process: Some("milled".to_string()),
            lot: Some("lot-1".to_string()),
            location: Some("Warehouse".to_string()),
            profile: Some("standard".to_string()),
            year: Some("2024".to_string()),
        },
        quantities: vec![RadrootsListingQuantity {
            value: quantity,
            label: None,
            count: Some(120),
        }],
        prices: vec![RadrootsCoreQuantityPrice::new(
            RadrootsCoreMoney::new(price_amount, RadrootsCoreCurrency::USD),
            price_quantity,
        )],
        discounts: Some(vec![RadrootsListingDiscount::Quantity {
            ref_quantity: "bag".to_string(),
            threshold: RadrootsCoreQuantity::new(discount_threshold, RadrootsCoreUnit::MassLb),
            value: RadrootsCoreMoney::new(discount_amount, RadrootsCoreCurrency::USD),
        }]),
        inventory_available: None,
        availability: None,
        delivery_method: None,
        location: Some(RadrootsListingLocation {
            primary: "Moyobamba".to_string(),
            city: Some("Moyobamba".to_string()),
            region: Some("San Martin".to_string()),
            country: Some("PE".to_string()),
            lat: Some(-6.0346),
            lng: Some(-76.9714),
            geohash: None,
        }),
        images: Some(vec![RadrootsListingImage {
            url: "http://example.com/widget.jpg".to_string(),
            size: Some(RadrootsListingImageSize { w: 1200, h: 800 }),
        }]),
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

#[test]
fn listing_build_tags_includes_listing_fields() {
    let listing = sample_listing_full("listing-1");
    let tags = listing_build_tags(&listing).unwrap();

    assert!(tags.iter().any(|t| {
        t.get(0).map(|s| s.as_str()) == Some(TAG_D)
            && t.get(1).map(|s| s.as_str()) == Some("listing-1")
    }));
    assert!(tags.iter().any(|t| {
        t.get(0).map(|s| s.as_str()) == Some("key")
            && t.get(1).map(|s| s.as_str()) == Some("sku")
    }));
    assert!(tags.iter().any(|t| {
        t.get(0).map(|s| s.as_str()) == Some("title")
            && t.get(1).map(|s| s.as_str()) == Some("Widget")
    }));

    let qty_tag = tags
        .iter()
        .find(|t| t.get(0).map(|s| s.as_str()) == Some("quantity"))
        .expect("quantity tag");
    assert_eq!(qty_tag.get(2).map(|s| s.as_str()), Some("lb"));
    assert_eq!(qty_tag.get(3).map(|s| s.as_str()), Some("bag"));
    assert_eq!(qty_tag.get(4).map(|s| s.as_str()), Some("120"));

    let price_tag = tags
        .iter()
        .find(|t| t.get(0).map(|s| s.as_str()) == Some("price"))
        .expect("price tag");
    assert_eq!(price_tag.get(2).map(|s| s.as_str()), Some("usd"));
    assert_eq!(price_tag.get(4).map(|s| s.as_str()), Some("lb"));
    assert_eq!(price_tag.get(5).map(|s| s.as_str()), Some("bag"));

    let discount_tag = tags
        .iter()
        .find(|t| t.get(0).map(|s| s.as_str()) == Some("price-discount-quantity"))
        .expect("discount tag");
    assert!(discount_tag
        .get(1)
        .map(|s| s.contains("\"ref_quantity\":\"bag\""))
        .unwrap_or(false));

    assert!(tags.iter().any(|t| {
        t.get(0).map(|s| s.as_str()) == Some("location")
            && t.get(1).map(|s| s.as_str()) == Some("Moyobamba")
    }));

    let g_tags: Vec<&Vec<String>> = tags
        .iter()
        .filter(|t| t.get(0).map(|s| s.as_str()) == Some("g"))
        .collect();
    assert!(!g_tags.is_empty());
    let full_len = g_tags[0][1].len();
    assert_eq!(g_tags.len(), full_len);
    for (idx, tag) in g_tags.iter().enumerate() {
        assert_eq!(tag[1].len(), full_len - idx);
    }
    assert!(tags.iter().any(|t| {
        t.get(0).map(|s| s.as_str()) == Some("L")
            && t.get(1).map(|s| s.as_str()) == Some("dd.lat")
    }));
    assert!(tags.iter().any(|t| {
        t.get(0).map(|s| s.as_str()) == Some("L")
            && t.get(1).map(|s| s.as_str()) == Some("dd.lon")
    }));
    assert!(tags.iter().any(|t| {
        t.get(0).map(|s| s.as_str()) == Some("l")
            && t.get(2).map(|s| s.as_str()) == Some("dd.lat")
    }));
    assert!(tags.iter().any(|t| {
        t.get(0).map(|s| s.as_str()) == Some("l")
            && t.get(2).map(|s| s.as_str()) == Some("dd.lon")
    }));

    assert!(tags.iter().any(|t| {
        t.get(0).map(|s| s.as_str()) == Some("image")
            && t.get(1).map(|s| s.as_str()) == Some("http://example.com/widget.jpg")
            && t.get(2).map(|s| s.as_str()) == Some("1200x800")
    }));
}
