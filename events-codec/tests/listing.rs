#![cfg(feature = "serde_json")]

use radroots_core::{
    RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreDiscount,
    RadrootsCoreDiscountScope, RadrootsCoreDiscountThreshold, RadrootsCoreDiscountValue,
    RadrootsCoreMoney, RadrootsCoreQuantity, RadrootsCoreQuantityPrice, RadrootsCoreUnit,
};
use radroots_events::{
    kinds::{KIND_LISTING, KIND_POST},
    listing::{
        RadrootsListing, RadrootsListingAvailability, RadrootsListingDeliveryMethod,
        RadrootsListingBin, RadrootsListingFarmRef, RadrootsListingImage,
        RadrootsListingImageSize, RadrootsListingLocation, RadrootsListingProduct,
        RadrootsListingStatus,
    },
};
use radroots_events::tags::TAG_D;
use radroots_events_codec::error::{EventEncodeError, EventParseError};
use radroots_events_codec::listing::decode::listing_from_event;
use radroots_events_codec::listing::encode::{listing_build_tags, to_wire_parts};
use radroots_events_codec::listing::tags::listing_tags_full;
use std::str::FromStr;

fn sample_listing(d_tag: &str) -> RadrootsListing {
    let quantity = RadrootsCoreQuantity::new(RadrootsCoreDecimal::from(1u32), RadrootsCoreUnit::Each);
    let price = RadrootsCoreQuantityPrice::new(
        RadrootsCoreMoney::new(RadrootsCoreDecimal::from(10u32), RadrootsCoreCurrency::USD),
        quantity.clone(),
    );

    RadrootsListing {
        d_tag: d_tag.to_string(),
        farm: RadrootsListingFarmRef {
            pubkey: "farm_pubkey".to_string(),
            d_tag: "farm-1".to_string(),
        },
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
        primary_bin_id: "bin-1".to_string(),
        bins: vec![RadrootsListingBin {
            bin_id: "bin-1".to_string(),
            quantity,
            price_per_canonical_unit: price,
            display_amount: None,
            display_unit: None,
            display_label: None,
            display_price: None,
            display_price_unit: None,
        }],
        resource_area: None,
        plot: None,
        discounts: None,
        inventory_available: None,
        availability: None,
        delivery_method: None,
        location: None,
        images: None,
    }
}

fn sample_listing_full(d_tag: &str) -> RadrootsListing {
    let qty_amount = RadrootsCoreDecimal::from_str("1000").unwrap();
    let price_amount = RadrootsCoreDecimal::from_str("0.01").unwrap();
    let display_qty = RadrootsCoreDecimal::from_str("1").unwrap();
    let display_price = RadrootsCoreDecimal::from_str("10").unwrap();

    RadrootsListing {
        d_tag: d_tag.to_string(),
        farm: RadrootsListingFarmRef {
            pubkey: "farm_pubkey".to_string(),
            d_tag: "farm-1".to_string(),
        },
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
        primary_bin_id: "bin-1".to_string(),
        bins: vec![RadrootsListingBin {
            bin_id: "bin-1".to_string(),
            quantity: RadrootsCoreQuantity::new(qty_amount, RadrootsCoreUnit::MassG),
            price_per_canonical_unit: RadrootsCoreQuantityPrice::new(
                RadrootsCoreMoney::new(price_amount, RadrootsCoreCurrency::USD),
                RadrootsCoreQuantity::new(RadrootsCoreDecimal::from(1u32), RadrootsCoreUnit::MassG),
            ),
            display_amount: Some(display_qty),
            display_unit: Some(RadrootsCoreUnit::MassKg),
            display_label: Some("bag".to_string()),
            display_price: Some(RadrootsCoreMoney::new(
                display_price,
                RadrootsCoreCurrency::USD,
            )),
            display_price_unit: Some(RadrootsCoreUnit::MassKg),
        }],
        resource_area: None,
        plot: None,
        discounts: Some(vec![RadrootsCoreDiscount {
            scope: RadrootsCoreDiscountScope::Bin,
            threshold: RadrootsCoreDiscountThreshold::BinCount {
                bin_id: "bin-1".to_string(),
                min: 5,
            },
            value: RadrootsCoreDiscountValue::MoneyPerBin(RadrootsCoreMoney::new(
                RadrootsCoreDecimal::from_str("2").unwrap(),
                RadrootsCoreCurrency::USD,
            )),
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
fn listing_build_tags_rejects_invalid_d_tag() {
    let listing = sample_listing("invalid:tag");
    let err = listing_build_tags(&listing).unwrap_err();
    assert!(matches!(err, EventEncodeError::InvalidField("d")));
}

#[test]
fn listing_roundtrip_from_event() {
    let listing = sample_listing("listing-1");
    let parts = to_wire_parts(&listing).unwrap();

    let decoded = listing_from_event(parts.kind, &parts.tags, &parts.content).unwrap();
    assert_eq!(decoded.d_tag, listing.d_tag);
    assert_eq!(decoded.product.key, listing.product.key);
    assert_eq!(decoded.product.title, listing.product.title);
    assert_eq!(decoded.primary_bin_id, listing.primary_bin_id);
    assert_eq!(decoded.bins.len(), listing.bins.len());
}

#[test]
fn listing_from_event_fills_missing_d_tag() {
    let listing = sample_listing("");
    let content = serde_json::to_string(&listing).unwrap();
    let tags = vec![
        vec![TAG_D.to_string(), "filled".to_string()],
        vec!["p".to_string(), "farm_pubkey".to_string()],
        vec!["a".to_string(), "30340:farm_pubkey:farm-1".to_string()],
    ];

    let decoded = listing_from_event(KIND_LISTING, &tags, &content).unwrap();
    assert_eq!(decoded.d_tag, "filled");
}

#[test]
fn listing_from_event_rejects_mismatched_d_tag() {
    let listing = sample_listing("a");
    let content = serde_json::to_string(&listing).unwrap();
    let tags = vec![
        vec![TAG_D.to_string(), "b".to_string()],
        vec!["p".to_string(), "farm_pubkey".to_string()],
        vec!["a".to_string(), "30340:farm_pubkey:farm-1".to_string()],
    ];

    let err = listing_from_event(KIND_LISTING, &tags, &content).unwrap_err();
    assert!(matches!(err, EventParseError::InvalidTag(TAG_D)));
}

#[test]
fn listing_from_event_rejects_wrong_kind() {
    let listing = sample_listing("listing-1");
    let content = serde_json::to_string(&listing).unwrap();
    let tags = vec![
        vec![TAG_D.to_string(), "listing-1".to_string()],
        vec!["p".to_string(), "farm_pubkey".to_string()],
        vec!["a".to_string(), "30340:farm_pubkey:farm-1".to_string()],
    ];

    let err = listing_from_event(KIND_POST, &tags, &content).unwrap_err();
    assert!(matches!(
        err,
        EventParseError::InvalidKind {
            expected: "30402",
            got: KIND_POST
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
        t.get(0).map(|s| s.as_str()) == Some("p")
            && t.get(1).map(|s| s.as_str()) == Some("farm_pubkey")
    }));
    assert!(tags.iter().any(|t| {
        t.get(0).map(|s| s.as_str()) == Some("a")
            && t.get(1).map(|s| s.as_str()) == Some("30340:farm_pubkey:farm-1")
    }));
    assert!(tags.iter().any(|t| {
        t.get(0).map(|s| s.as_str()) == Some("key")
            && t.get(1).map(|s| s.as_str()) == Some("sku")
    }));
    assert!(tags.iter().any(|t| {
        t.get(0).map(|s| s.as_str()) == Some("title")
            && t.get(1).map(|s| s.as_str()) == Some("Widget")
    }));

    let primary_tag = tags
        .iter()
        .find(|t| t.get(0).map(|s| s.as_str()) == Some("radroots:primary_bin"))
        .expect("primary bin tag");
    assert_eq!(primary_tag.get(1).map(|s| s.as_str()), Some("bin-1"));

    let bin_tag = tags
        .iter()
        .find(|t| t.get(0).map(|s| s.as_str()) == Some("radroots:bin"))
        .expect("bin tag");
    assert_eq!(bin_tag.get(1).map(|s| s.as_str()), Some("bin-1"));
    assert_eq!(bin_tag.get(2).map(|s| s.as_str()), Some("1000"));
    assert_eq!(bin_tag.get(3).map(|s| s.as_str()), Some("g"));
    assert_eq!(bin_tag.get(4).map(|s| s.as_str()), Some("1"));
    assert_eq!(bin_tag.get(5).map(|s| s.as_str()), Some("kg"));
    assert_eq!(bin_tag.get(6).map(|s| s.as_str()), Some("bag"));

    let price_tag = tags
        .iter()
        .find(|t| t.get(0).map(|s| s.as_str()) == Some("radroots:price"))
        .expect("radroots price tag");
    assert_eq!(price_tag.get(1).map(|s| s.as_str()), Some("bin-1"));
    assert_eq!(price_tag.get(2).map(|s| s.as_str()), Some("0.01"));
    assert_eq!(price_tag.get(3).map(|s| s.as_str()), Some("USD"));
    assert_eq!(price_tag.get(4).map(|s| s.as_str()), Some("1"));
    assert_eq!(price_tag.get(5).map(|s| s.as_str()), Some("g"));
    assert_eq!(price_tag.get(6).map(|s| s.as_str()), Some("10"));
    assert_eq!(price_tag.get(7).map(|s| s.as_str()), Some("kg"));

    let generic_price_tag = tags
        .iter()
        .find(|t| {
            t.get(0).map(|s| s.as_str()) == Some("price")
                && t.get(1).map(|s| s.as_str()) == Some("10")
        })
        .expect("generic price tag");
    assert_eq!(generic_price_tag.get(2).map(|s| s.as_str()), Some("USD"));

    let discount_tag = tags
        .iter()
        .find(|t| t.get(0).map(|s| s.as_str()) == Some("radroots:discount"))
        .expect("discount tag");
    assert!(discount_tag
        .get(1)
        .map(|s| s.contains("\"scope\":\"bin\""))
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

#[test]
fn listing_tags_full_includes_trade_fields() {
    let mut listing = sample_listing("listing-1");
    let inventory = RadrootsCoreDecimal::from_str("12.5").unwrap();
    let inventory_value = inventory.to_string();
    listing.inventory_available = Some(inventory);
    listing.availability = Some(RadrootsListingAvailability::Window {
        start: Some(1730000000),
        end: Some(1731000000),
    });
    listing.delivery_method = Some(RadrootsListingDeliveryMethod::Shipping);

    let tags = listing_tags_full(&listing).unwrap();

    assert!(tags.iter().any(|t| {
        t.get(0).map(|s| s.as_str()) == Some("inventory")
            && t.get(1).map(|s| s.as_str()) == Some(inventory_value.as_str())
    }));
    assert!(tags.iter().any(|t| {
        t.get(0).map(|s| s.as_str()) == Some("published_at")
            && t.get(1).map(|s| s.as_str()) == Some("1730000000")
    }));
    assert!(tags.iter().any(|t| {
        t.get(0).map(|s| s.as_str()) == Some("expires_at")
            && t.get(1).map(|s| s.as_str()) == Some("1731000000")
    }));
    assert!(tags.iter().any(|t| {
        t.get(0).map(|s| s.as_str()) == Some("delivery")
            && t.get(1).map(|s| s.as_str()) == Some("shipping")
    }));
}

#[test]
fn listing_tags_full_includes_status_tag() {
    let mut listing = sample_listing("listing-1");
    listing.availability = Some(RadrootsListingAvailability::Status {
        status: RadrootsListingStatus::Active,
    });

    let tags = listing_tags_full(&listing).unwrap();

    assert!(tags.iter().any(|t| {
        t.get(0).map(|s| s.as_str()) == Some("status")
            && t.get(1).map(|s| s.as_str()) == Some("active")
    }));
}

#[test]
fn listing_build_tags_ignores_null_strings() {
    let mut listing = sample_listing_full("listing-1");
    listing.product.summary = Some("null".to_string());
    listing.product.process = Some("null".to_string());
    listing.product.lot = Some("null".to_string());
    listing.product.location = Some("null".to_string());
    listing.product.profile = Some("null".to_string());
    listing.product.year = Some("null".to_string());
    listing.location = Some(RadrootsListingLocation {
        primary: "Moyobamba".to_string(),
        city: Some("null".to_string()),
        region: Some("San Martin".to_string()),
        country: Some("null".to_string()),
        lat: Some(-6.0346),
        lng: Some(-76.9714),
        geohash: None,
    });
    listing.images = Some(vec![RadrootsListingImage {
        url: "null".to_string(),
        size: None,
    }]);

    let tags = listing_build_tags(&listing).unwrap();
    assert!(!tags
        .iter()
        .any(|tag| tag.iter().any(|value| value == "null")));
}
