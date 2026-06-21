#![cfg(feature = "serde_json")]

use radroots_core::{
    RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreDiscount, RadrootsCoreDiscountScope,
    RadrootsCoreDiscountThreshold, RadrootsCoreDiscountValue, RadrootsCoreMoney,
    RadrootsCoreQuantity, RadrootsCoreQuantityPrice, RadrootsCoreUnit,
};
use radroots_events::{
    RadrootsNostrEvent,
    farm::RadrootsFarmRef,
    ids::{RadrootsDTag, RadrootsInventoryBinId},
    kinds::{
        KIND_FARM, KIND_LISTING, KIND_LISTING_DRAFT, KIND_PLOT, KIND_POST, KIND_RESOURCE_AREA,
    },
    listing::{
        RadrootsListing, RadrootsListingAvailability, RadrootsListingBin,
        RadrootsListingDeliveryMethod, RadrootsListingImage, RadrootsListingImageSize,
        RadrootsListingLocation, RadrootsListingProduct, RadrootsListingStatus,
    },
    plot::RadrootsPlotRef,
    resource_area::RadrootsResourceAreaRef,
    tags::{TAG_D, TAG_PUBLISHED_AT},
};
use radroots_events_codec::error::{EventEncodeError, EventParseError};
use radroots_events_codec::listing::decode::{
    data_from_event, data_from_nostr_event, listing_from_event, parsed_from_event,
    parsed_from_nostr_event,
};
use radroots_events_codec::listing::encode::{
    listing_build_tags, to_wire_parts, to_wire_parts_with_kind,
};
use radroots_events_codec::listing::tags::{
    ListingTagOptions, listing_tags_full, listing_tags_with_options,
};
use std::str::FromStr;

fn listing_d_tag(raw: &str) -> RadrootsDTag {
    raw.parse().unwrap()
}

fn bin_id(raw: &str) -> RadrootsInventoryBinId {
    raw.parse().unwrap()
}

fn sample_listing_tags() -> Vec<Vec<String>> {
    listing_build_tags(&sample_listing("AAAAAAAAAAAAAAAAAAAAAg")).unwrap()
}

fn remove_tags(tags: &mut Vec<Vec<String>>, name: &str) {
    tags.retain(|tag| tag.first().map(|value| value.as_str()) != Some(name));
}

fn replace_first_tag(tags: &mut [Vec<String>], name: &str, replacement: Vec<&str>) {
    let tag = tags
        .iter_mut()
        .find(|tag| tag.first().map(|value| value.as_str()) == Some(name))
        .expect("tag");
    *tag = replacement.into_iter().map(str::to_string).collect();
}

fn assert_missing_tag(tags: Vec<Vec<String>>, expected: &'static str) {
    match listing_from_event(KIND_LISTING, &tags, "# Widget") {
        Err(EventParseError::MissingTag(tag)) => assert_eq!(tag, expected),
        other => panic!("expected missing tag {expected}: {other:?}"),
    }
}

fn assert_invalid_tag(tags: Vec<Vec<String>>, expected: &'static str) {
    match listing_from_event(KIND_LISTING, &tags, "# Widget") {
        Err(EventParseError::InvalidTag(tag)) => assert_eq!(tag, expected),
        other => panic!("expected invalid tag {expected}: {other:?}"),
    }
}

fn sample_listing(d_tag: &str) -> RadrootsListing {
    let quantity =
        RadrootsCoreQuantity::new(RadrootsCoreDecimal::from(1u32), RadrootsCoreUnit::Each);
    let price = RadrootsCoreQuantityPrice::new(
        RadrootsCoreMoney::new(RadrootsCoreDecimal::from(10u32), RadrootsCoreCurrency::USD),
        quantity.clone(),
    );

    RadrootsListing {
        d_tag: listing_d_tag(d_tag),
        published_at: None,
        farm: RadrootsFarmRef {
            pubkey: "farm_pubkey".to_string(),
            d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_string(),
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
        primary_bin_id: bin_id("bin-1"),
        bins: vec![RadrootsListingBin {
            bin_id: bin_id("bin-1"),
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
        d_tag: listing_d_tag(d_tag),
        published_at: None,
        farm: RadrootsFarmRef {
            pubkey: "farm_pubkey".to_string(),
            d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_string(),
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
        primary_bin_id: bin_id("bin-1"),
        bins: vec![RadrootsListingBin {
            bin_id: bin_id("bin-1"),
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
    assert!(RadrootsDTag::parse("").is_err());
}

#[test]
fn listing_build_tags_rejects_invalid_d_tag() {
    let listing = sample_listing("invalid:tag");
    let err = listing_build_tags(&listing).unwrap_err();
    assert!(matches!(err, EventEncodeError::InvalidField("d")));
}

#[test]
fn listing_roundtrip_from_event() {
    let listing = sample_listing("AAAAAAAAAAAAAAAAAAAAAg");
    let parts = to_wire_parts(&listing).unwrap();

    assert_eq!(parts.content, "# Widget");

    let decoded = listing_from_event(parts.kind, &parts.tags, &parts.content).unwrap();
    assert_eq!(decoded.d_tag, listing.d_tag);
    assert_eq!(decoded.product.key, listing.product.key);
    assert_eq!(decoded.product.title, listing.product.title);
    assert_eq!(decoded.primary_bin_id, listing.primary_bin_id);
    assert_eq!(decoded.bins.len(), listing.bins.len());
}

#[test]
fn listing_from_event_reconstructs_from_tags_with_markdown_content() {
    let listing = sample_listing_full("FAAAAAAAAAAAAAAAAAAAAA");
    let tags = listing_build_tags(&listing).unwrap();

    let decoded = listing_from_event(KIND_LISTING, &tags, "### Markdown listing").unwrap();
    assert_eq!(decoded.d_tag, listing.d_tag);
    assert_eq!(decoded.product.summary, listing.product.summary);
    assert_eq!(decoded.primary_bin_id, listing.primary_bin_id);
    assert_eq!(
        decoded
            .location
            .as_ref()
            .map(|location| location.primary.as_str()),
        Some("Moyobamba")
    );
}

#[test]
fn listing_from_event_rejects_invalid_d_tag() {
    let mut tags = listing_build_tags(&sample_listing("AAAAAAAAAAAAAAAAAAAAAg")).unwrap();
    let d_tag = tags
        .iter_mut()
        .find(|tag| tag.first().map(|value| value.as_str()) == Some(TAG_D))
        .expect("d tag");
    d_tag[1] = "invalid:tag".to_string();

    let err = listing_from_event(KIND_LISTING, &tags, "# Widget").unwrap_err();
    assert!(matches!(err, EventParseError::InvalidTag(TAG_D)));
}

#[test]
fn listing_from_event_rejects_wrong_kind() {
    let tags = listing_build_tags(&sample_listing("AAAAAAAAAAAAAAAAAAAAAg")).unwrap();

    let err = listing_from_event(KIND_POST, &tags, "# Widget").unwrap_err();
    assert!(matches!(
        err,
        EventParseError::InvalidKind {
            expected: "30402 or 30403",
            got: KIND_POST
        }
    ));
}

#[test]
fn listing_from_event_covers_reference_tag_error_paths() {
    let mut tags = sample_listing_tags();
    remove_tags(&mut tags, TAG_D);
    assert_missing_tag(tags, TAG_D);

    let mut tags = sample_listing_tags();
    replace_first_tag(&mut tags, TAG_D, vec![TAG_D]);
    assert_invalid_tag(tags, TAG_D);

    let mut tags = sample_listing_tags();
    replace_first_tag(&mut tags, TAG_D, vec![TAG_D, " "]);
    assert_invalid_tag(tags, TAG_D);

    let mut tags = sample_listing_tags();
    remove_tags(&mut tags, "a");
    assert_missing_tag(tags, "a");

    let mut tags = sample_listing_tags();
    replace_first_tag(&mut tags, "a", vec!["a"]);
    assert_invalid_tag(tags, "a");

    let mut tags = sample_listing_tags();
    replace_first_tag(&mut tags, "a", vec!["a", "bad:farm_pubkey:farm"]);
    assert_invalid_tag(tags, "a");

    let mut tags = sample_listing_tags();
    replace_first_tag(&mut tags, "a", vec!["a", "30340"]);
    assert_invalid_tag(tags, "a");

    let mut tags = sample_listing_tags();
    replace_first_tag(&mut tags, "a", vec!["a", "30340::farm"]);
    assert_invalid_tag(tags, "a");

    let mut tags = sample_listing_tags();
    replace_first_tag(&mut tags, "a", vec!["a", "30340:farm_pubkey:"]);
    assert_invalid_tag(tags, "a");

    let mut tags = sample_listing_tags();
    replace_first_tag(&mut tags, "a", vec!["a", "30340:farm_pubkey:bad d"]);
    assert_invalid_tag(tags, "a");

    let mut tags = sample_listing_tags();
    replace_first_tag(&mut tags, "a", vec!["a", "30023:other:article"]);
    assert_missing_tag(tags, "a");

    let mut tags = sample_listing_tags();
    remove_tags(&mut tags, "p");
    assert_missing_tag(tags, "p");

    let mut tags = sample_listing_tags();
    replace_first_tag(&mut tags, "p", vec!["p"]);
    assert_invalid_tag(tags, "p");

    let mut tags = sample_listing_tags();
    replace_first_tag(&mut tags, "p", vec!["p", " "]);
    assert_invalid_tag(tags, "p");

    let mut tags = sample_listing_tags();
    replace_first_tag(&mut tags, "p", vec!["p", "other_pubkey"]);
    assert_invalid_tag(tags, "p");
}

#[test]
fn listing_from_event_covers_resource_and_plot_reference_paths() {
    let mut listing = sample_listing("AAAAAAAAAAAAAAAAAAAAAw");
    listing.resource_area = Some(RadrootsResourceAreaRef {
        pubkey: "resource_pubkey".to_string(),
        d_tag: "AAAAAAAAAAAAAAAAAAAABQ".to_string(),
    });
    listing.plot = Some(RadrootsPlotRef {
        pubkey: "plot_pubkey".to_string(),
        d_tag: "AAAAAAAAAAAAAAAAAAAAAw".to_string(),
    });
    let tags = listing_build_tags(&listing).unwrap();
    let decoded = listing_from_event(KIND_LISTING, &tags, "# Widget").unwrap();
    assert_eq!(
        decoded
            .resource_area
            .as_ref()
            .map(|area| area.d_tag.as_str()),
        Some("AAAAAAAAAAAAAAAAAAAABQ")
    );
    assert_eq!(
        decoded.plot.as_ref().map(|plot| plot.d_tag.as_str()),
        Some("AAAAAAAAAAAAAAAAAAAAAw")
    );

    let mut tags = sample_listing_tags();
    tags.push(vec!["radroots:resource_area".to_string()]);
    assert_invalid_tag(tags, "radroots:resource_area");

    let mut tags = sample_listing_tags();
    tags.push(vec![
        "radroots:resource_area".to_string(),
        format!("{KIND_FARM}:resource_pubkey:resource-area-1"),
    ]);
    assert_invalid_tag(tags, "radroots:resource_area");

    let mut tags = sample_listing_tags();
    tags.push(vec![
        "radroots:resource_area".to_string(),
        format!("{KIND_RESOURCE_AREA}::resource-area-1"),
    ]);
    assert_invalid_tag(tags, "radroots:resource_area");

    let mut tags = sample_listing_tags();
    tags.push(vec![
        "radroots:resource_area".to_string(),
        format!("{KIND_RESOURCE_AREA}:resource_pubkey:"),
    ]);
    assert_invalid_tag(tags, "radroots:resource_area");

    let mut tags = sample_listing_tags();
    tags.push(vec![
        "radroots:resource_area".to_string(),
        format!("{KIND_RESOURCE_AREA}:resource_pubkey:bad d"),
    ]);
    assert_invalid_tag(tags, "radroots:resource_area");

    let mut tags = sample_listing_tags();
    tags.push(vec!["radroots:plot".to_string()]);
    assert_invalid_tag(tags, "radroots:plot");

    let mut tags = sample_listing_tags();
    tags.push(vec![
        "radroots:plot".to_string(),
        format!("{KIND_RESOURCE_AREA}:plot_pubkey:plot-1"),
    ]);
    assert_invalid_tag(tags, "radroots:plot");

    let mut tags = sample_listing_tags();
    tags.push(vec![
        "radroots:plot".to_string(),
        format!("{KIND_PLOT}:plot_pubkey:"),
    ]);
    assert_invalid_tag(tags, "radroots:plot");

    let mut tags = sample_listing_tags();
    tags.push(vec![
        "radroots:plot".to_string(),
        format!("{KIND_PLOT}:plot_pubkey:bad d"),
    ]);
    assert_invalid_tag(tags, "radroots:plot");
}

#[test]
fn listing_from_event_covers_bin_and_price_error_paths() {
    let mut tags = sample_listing_tags();
    remove_tags(&mut tags, "radroots:primary_bin");
    assert_missing_tag(tags, "radroots:primary_bin");

    let mut tags = sample_listing_tags();
    tags.push(vec![
        "radroots:primary_bin".to_string(),
        "bin-2".to_string(),
    ]);
    assert_invalid_tag(tags, "radroots:primary_bin");

    let mut tags = sample_listing_tags();
    replace_first_tag(
        &mut tags,
        "radroots:primary_bin",
        vec!["radroots:primary_bin", "bin-2"],
    );
    assert_invalid_tag(tags, "radroots:primary_bin");

    let mut tags = sample_listing_tags();
    remove_tags(&mut tags, "radroots:bin");
    assert_missing_tag(tags, "radroots:bin");

    let mut tags = sample_listing_tags();
    remove_tags(&mut tags, "radroots:price");
    assert_missing_tag(tags, "radroots:price");

    let mut tags = sample_listing_tags();
    replace_first_tag(&mut tags, "radroots:bin", vec!["radroots:bin"]);
    assert_invalid_tag(tags, "radroots:bin");

    let mut tags = sample_listing_tags();
    replace_first_tag(
        &mut tags,
        "radroots:bin",
        vec!["radroots:bin", "bin-1", "1", "kg"],
    );
    assert_invalid_tag(tags, "radroots:bin");

    let mut tags = sample_listing_tags();
    replace_first_tag(
        &mut tags,
        "radroots:bin",
        vec!["radroots:bin", "bin-1", "1", "each", "1"],
    );
    assert_invalid_tag(tags, "radroots:bin");

    let mut tags = sample_listing_tags();
    replace_first_tag(
        &mut tags,
        "radroots:bin",
        vec![
            "radroots:bin",
            "bin-1",
            "1",
            "each",
            "1",
            "each",
            "label",
            "extra",
        ],
    );
    assert_invalid_tag(tags, "radroots:bin");

    let mut tags = sample_listing_tags();
    replace_first_tag(
        &mut tags,
        "radroots:bin",
        vec!["radroots:bin", "bin-1", "1", "each", "1", "each"],
    );
    let decoded = listing_from_event(KIND_LISTING, &tags, "# Widget").unwrap();
    assert_eq!(
        decoded.bins[0].display_amount,
        Some(RadrootsCoreDecimal::from(1u32))
    );
    assert_eq!(decoded.bins[0].display_unit, Some(RadrootsCoreUnit::Each));
    assert_eq!(decoded.bins[0].display_label, None);

    let mut tags = sample_listing_tags();
    tags.push(vec![
        "radroots:bin".to_string(),
        "bin-1".to_string(),
        "1".to_string(),
        "each".to_string(),
    ]);
    assert_invalid_tag(tags, "radroots:bin");

    let mut tags = sample_listing_tags();
    replace_first_tag(&mut tags, "radroots:price", vec!["radroots:price"]);
    assert_invalid_tag(tags, "radroots:price");

    let mut tags = sample_listing_tags();
    replace_first_tag(
        &mut tags,
        "radroots:price",
        vec!["radroots:price", "bin-1", "10", "USD", "1", "kg"],
    );
    assert_invalid_tag(tags, "radroots:price");

    let mut tags = sample_listing_tags();
    replace_first_tag(
        &mut tags,
        "radroots:price",
        vec!["radroots:price", "bin-1", "10", "USD", "1", "each", "10"],
    );
    assert_invalid_tag(tags, "radroots:price");

    let mut tags = sample_listing_tags();
    replace_first_tag(
        &mut tags,
        "radroots:price",
        vec![
            "radroots:price",
            "bin-1",
            "10",
            "USD",
            "1",
            "each",
            "10",
            "each",
            "extra",
        ],
    );
    assert_invalid_tag(tags, "radroots:price");

    let mut tags = sample_listing_tags();
    tags.push(vec![
        "radroots:price".to_string(),
        "bin-1".to_string(),
        "10".to_string(),
        "USD".to_string(),
        "1".to_string(),
        "each".to_string(),
    ]);
    assert_invalid_tag(tags, "radroots:price");

    let mut tags = sample_listing_tags();
    replace_first_tag(
        &mut tags,
        "radroots:price",
        vec!["radroots:price", "bin-1", "10", "USD", "1", "g"],
    );
    assert_invalid_tag(tags, "radroots:price");
}

#[test]
fn listing_from_event_covers_trade_location_delivery_and_image_paths() {
    let mut tags = sample_listing_tags();
    tags.push(vec!["location".to_string(), "Farm shelf".to_string()]);
    let decoded = listing_from_event(KIND_LISTING, &tags, "# Widget").unwrap();
    assert_eq!(
        decoded
            .location
            .as_ref()
            .map(|location| location.primary.as_str()),
        Some("Farm shelf")
    );

    let mut tags = sample_listing_tags();
    tags.push(vec!["location".to_string(), "Farm shelf".to_string()]);
    tags.push(vec![
        "location".to_string(),
        "Peru".to_string(),
        "Moyobamba".to_string(),
        "San Martin".to_string(),
        "PE".to_string(),
    ]);
    tags.push(vec!["g".to_string(), "6gkzwgjzn".to_string()]);
    let decoded = listing_from_event(KIND_LISTING, &tags, "# Widget").unwrap();
    assert_eq!(decoded.product.location.as_deref(), Some("Farm shelf"));
    assert_eq!(
        decoded.location.as_ref().map(|location| {
            (
                location.primary.as_str(),
                location.city.as_deref(),
                location.geohash.as_deref(),
            )
        }),
        Some(("Peru", Some("Moyobamba"), Some("6gkzwgjzn")))
    );

    let mut tags = sample_listing_tags();
    tags.push(vec![
        "location".to_string(),
        " ".to_string(),
        "Moyobamba".to_string(),
    ]);
    assert_invalid_tag(tags, "location");

    let mut tags = sample_listing_tags();
    tags.push(vec!["inventory".to_string()]);
    assert_invalid_tag(tags, "inventory");

    let mut tags = sample_listing_tags();
    tags.push(vec!["inventory".to_string(), "bad".to_string()]);
    assert_invalid_tag(tags, "inventory");

    let mut tags = sample_listing_tags();
    tags.push(vec!["inventory".to_string(), "12.5".to_string()]);
    tags.push(vec![
        "radroots:availability_start".to_string(),
        "1730".to_string(),
    ]);
    tags.push(vec!["expires_at".to_string(), "1740".to_string()]);
    tags.push(vec!["delivery".to_string(), "pickup".to_string()]);
    tags.push(vec!["image".to_string(), " ".to_string()]);
    tags.push(vec!["g".to_string(), " ".to_string()]);
    tags.push(vec![
        "image".to_string(),
        "https://example.test/a.jpg".to_string(),
    ]);
    tags.push(vec![
        "image".to_string(),
        "https://example.test/b.jpg".to_string(),
        "bad-size".to_string(),
    ]);
    let decoded = listing_from_event(KIND_LISTING, &tags, "# Widget").unwrap();
    let Some(RadrootsListingAvailability::Window { start, end }) = decoded.availability else {
        panic!("expected availability window");
    };
    assert_eq!(start, Some(1730));
    assert_eq!(end, Some(1740));
    assert!(matches!(
        decoded.delivery_method,
        Some(RadrootsListingDeliveryMethod::Pickup)
    ));
    assert_eq!(decoded.images.as_ref().map(Vec::len), Some(2));
    assert!(decoded.images.as_ref().unwrap()[1].size.is_none());

    let mut tags = sample_listing_tags();
    tags.push(vec!["delivery".to_string(), "local_delivery".to_string()]);
    let decoded = listing_from_event(KIND_LISTING, &tags, "# Widget").unwrap();
    assert!(matches!(
        decoded.delivery_method,
        Some(RadrootsListingDeliveryMethod::LocalDelivery)
    ));

    let mut tags = sample_listing_tags();
    tags.push(vec!["delivery".to_string(), "shipping".to_string()]);
    let decoded = listing_from_event(KIND_LISTING, &tags, "# Widget").unwrap();
    assert!(matches!(
        decoded.delivery_method,
        Some(RadrootsListingDeliveryMethod::Shipping)
    ));

    let mut tags = sample_listing_tags();
    tags.push(vec![
        "delivery".to_string(),
        "other".to_string(),
        "bike courier".to_string(),
    ]);
    let decoded = listing_from_event(KIND_LISTING, &tags, "# Widget").unwrap();
    let Some(RadrootsListingDeliveryMethod::Other { method }) = decoded.delivery_method else {
        panic!("expected other delivery method");
    };
    assert_eq!(method, "bike courier");

    let mut tags = sample_listing_tags();
    tags.push(vec!["delivery".to_string(), "drone".to_string()]);
    let decoded = listing_from_event(KIND_LISTING, &tags, "# Widget").unwrap();
    let Some(RadrootsListingDeliveryMethod::Other { method }) = decoded.delivery_method else {
        panic!("expected fallback delivery method");
    };
    assert_eq!(method, "drone");

    let mut tags = sample_listing_tags();
    tags.push(vec!["status".to_string(), "active".to_string()]);
    let decoded = listing_from_event(KIND_LISTING, &tags, "# Widget").unwrap();
    assert!(matches!(
        decoded.availability,
        Some(RadrootsListingAvailability::Status {
            status: RadrootsListingStatus::Active
        })
    ));

    let mut tags = sample_listing_tags();
    tags.push(vec!["status".to_string(), "sold".to_string()]);
    let decoded = listing_from_event(KIND_LISTING, &tags, "# Widget").unwrap();
    assert!(matches!(
        decoded.availability,
        Some(RadrootsListingAvailability::Status {
            status: RadrootsListingStatus::Sold
        })
    ));

    let mut tags = sample_listing_tags();
    tags.push(vec!["status".to_string(), "paused".to_string()]);
    let decoded = listing_from_event(KIND_LISTING, &tags, "# Widget").unwrap();
    let Some(RadrootsListingAvailability::Status {
        status: RadrootsListingStatus::Other { value },
    }) = decoded.availability
    else {
        panic!("expected other availability status");
    };
    assert_eq!(value, "paused");
}

#[test]
fn listing_from_event_covers_remaining_edge_paths() {
    let mut tags = sample_listing_tags();
    tags.insert(0, Vec::new());
    tags.push(vec!["location".to_string()]);
    let decoded = listing_from_event(KIND_LISTING, &tags, "# Widget").unwrap();
    assert_eq!(decoded.product.location, None);

    let mut tags = sample_listing_tags();
    tags.push(vec![
        "radroots:plot".to_string(),
        format!("{KIND_PLOT}::AAAAAAAAAAAAAAAAAAAAAw"),
    ]);
    assert_invalid_tag(tags, "radroots:plot");

    let mut tags = sample_listing_tags();
    tags.push(vec![
        "radroots:primary_bin".to_string(),
        "bin-2".to_string(),
    ]);
    assert_invalid_tag(tags, "radroots:primary_bin");

    let mut tags = sample_listing_tags();
    let primary_position = tags
        .iter()
        .position(|tag| tag.first().map(String::as_str) == Some("radroots:primary_bin"))
        .expect("primary bin tag");
    tags.insert(
        primary_position + 1,
        vec!["radroots:primary_bin".to_string(), "bin-2".to_string()],
    );
    assert_invalid_tag(tags, "radroots:primary_bin");

    let mut tags = sample_listing_tags();
    tags.insert(0, vec!["key".to_string(), " ".to_string()]);
    tags.push(vec!["key".to_string(), "ignored".to_string()]);
    tags.insert(0, vec!["summary".to_string(), " ".to_string()]);
    tags.push(vec!["summary".to_string(), "first summary".to_string()]);
    tags.push(vec!["summary".to_string(), "ignored summary".to_string()]);
    tags.push(vec!["process".to_string(), "null".to_string()]);
    tags.push(vec!["lot".to_string(), " null ".to_string()]);
    tags.push(vec!["profile".to_string(), "null".to_string()]);
    tags.push(vec!["year".to_string(), "null".to_string()]);
    let decoded = listing_from_event(KIND_LISTING, &tags, "# Widget").unwrap();
    assert_eq!(decoded.product.key, "sku");
    assert_eq!(decoded.product.summary.as_deref(), Some("first summary"));
    assert_eq!(decoded.product.process, None);
    assert_eq!(decoded.product.lot, None);
    assert_eq!(decoded.product.profile, None);
    assert_eq!(decoded.product.year, None);

    let mut tags = sample_listing_tags();
    tags.push(vec!["radroots:availability_start".to_string()]);
    assert_invalid_tag(tags, "radroots:availability_start");

    let mut tags = sample_listing_tags();
    tags.push(vec![
        "radroots:availability_start".to_string(),
        "bad".to_string(),
    ]);
    assert_invalid_tag(tags, "radroots:availability_start");
}

#[test]
fn listing_parsed_wrappers_preserve_event_metadata() {
    let listing = sample_listing("AAAAAAAAAAAAAAAAAAAAAQ");
    let parts = to_wire_parts(&listing).unwrap();
    let data = data_from_event(
        "event-id".to_string(),
        "author-pubkey".to_string(),
        7,
        parts.kind,
        parts.content.clone(),
        parts.tags.clone(),
    )
    .unwrap();
    assert_eq!(data.id, "event-id");
    assert_eq!(data.author, "author-pubkey");
    assert_eq!(data.published_at, 7);
    assert_eq!(data.kind, KIND_LISTING);
    assert_eq!(data.data.d_tag, listing.d_tag);

    let parsed = parsed_from_event(
        "event-id".to_string(),
        "author-pubkey".to_string(),
        7,
        parts.kind,
        parts.content.clone(),
        parts.tags.clone(),
        "sig".to_string(),
    )
    .unwrap();
    assert_eq!(parsed.event.id, "event-id");
    assert_eq!(parsed.event.author, "author-pubkey");
    assert_eq!(parsed.event.created_at, 7);
    assert_eq!(parsed.event.sig, "sig");
    assert_eq!(parsed.data.data.d_tag, listing.d_tag);

    let event = RadrootsNostrEvent {
        id: "event-id".to_string(),
        author: "author-pubkey".to_string(),
        created_at: 7,
        kind: parts.kind,
        tags: parts.tags,
        content: parts.content,
        sig: "sig".to_string(),
    };
    let data = data_from_nostr_event(&event).unwrap();
    assert_eq!(data.data.d_tag, listing.d_tag);
    let parsed = parsed_from_nostr_event(&event).unwrap();
    assert_eq!(parsed.event.sig, "sig");
    assert_eq!(parsed.data.data.d_tag, listing.d_tag);

    let err = parsed_from_event(
        "event-id".to_string(),
        "author-pubkey".to_string(),
        7,
        KIND_POST,
        event.content,
        event.tags,
        "sig".to_string(),
    )
    .unwrap_err();
    assert!(matches!(
        err,
        EventParseError::InvalidKind {
            expected: "30402 or 30403",
            got: KIND_POST
        }
    ));
}

#[test]
fn draft_listing_roundtrip_from_event() {
    let mut listing = sample_listing("AAAAAAAAAAAAAAAAAAAAAQ");
    listing.published_at = Some(1_781_895_600);
    let parts = to_wire_parts_with_kind(&listing, KIND_LISTING_DRAFT).unwrap();

    let decoded = listing_from_event(parts.kind, &parts.tags, &parts.content).unwrap();
    assert_eq!(parts.kind, KIND_LISTING_DRAFT);
    assert_eq!(parts.content, "# Widget");
    assert_eq!(decoded.d_tag, listing.d_tag);
    assert_eq!(decoded.published_at, Some(1_781_895_600));
}

#[test]
fn listing_roundtrips_published_at_for_active_and_rejects_bad_value() {
    let mut listing = sample_listing("AAAAAAAAAAAAAAAAAAAAAg");
    listing.published_at = Some(1_781_895_600);
    let parts = to_wire_parts_with_kind(&listing, KIND_LISTING).unwrap();
    assert!(parts.tags.iter().any(|tag| {
        tag.first().map(|value| value.as_str()) == Some(TAG_PUBLISHED_AT)
            && tag.get(1).map(|value| value.as_str()) == Some("1781895600")
    }));

    let decoded = listing_from_event(parts.kind, &parts.tags, &parts.content).unwrap();
    assert_eq!(decoded.published_at, Some(1_781_895_600));

    let mut tags = parts.tags;
    let published_at = tags
        .iter_mut()
        .find(|tag| tag.first().map(|value| value.as_str()) == Some(TAG_PUBLISHED_AT))
        .expect("published_at tag");
    published_at[1] = "bad".to_string();
    let err = listing_from_event(KIND_LISTING, &tags, "# Widget").unwrap_err();
    assert!(matches!(err, EventParseError::InvalidTag(TAG_PUBLISHED_AT)));
}

#[test]
fn to_wire_parts_rejects_non_listing_kind() {
    let err =
        to_wire_parts_with_kind(&sample_listing("AAAAAAAAAAAAAAAAAAAAAg"), KIND_POST).unwrap_err();
    assert!(matches!(err, EventEncodeError::InvalidKind(KIND_POST)));
}

#[test]
fn listing_build_tags_includes_listing_fields() {
    let listing = sample_listing_full("AAAAAAAAAAAAAAAAAAAAAg");
    let tags = listing_build_tags(&listing).unwrap();

    assert!(tags.iter().any(|t| {
        t.get(0).map(|s| s.as_str()) == Some(TAG_D)
            && t.get(1).map(|s| s.as_str()) == Some("AAAAAAAAAAAAAAAAAAAAAg")
    }));
    assert!(tags.iter().any(|t| {
        t.get(0).map(|s| s.as_str()) == Some("p")
            && t.get(1).map(|s| s.as_str()) == Some("farm_pubkey")
    }));
    assert!(tags.iter().any(|t| {
        t.get(0).map(|s| s.as_str()) == Some("a")
            && t.get(1).map(|s| s.as_str()) == Some("30340:farm_pubkey:AAAAAAAAAAAAAAAAAAAAAA")
    }));
    assert!(tags.iter().any(|t| {
        t.get(0).map(|s| s.as_str()) == Some("key") && t.get(1).map(|s| s.as_str()) == Some("sku")
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
    assert!(
        discount_tag
            .get(1)
            .map(|s| s.contains("\"scope\":\"bin\""))
            .unwrap_or(false)
    );

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
        t.get(0).map(|s| s.as_str()) == Some("L") && t.get(1).map(|s| s.as_str()) == Some("dd.lat")
    }));
    assert!(tags.iter().any(|t| {
        t.get(0).map(|s| s.as_str()) == Some("L") && t.get(1).map(|s| s.as_str()) == Some("dd.lon")
    }));
    assert!(tags.iter().any(|t| {
        t.get(0).map(|s| s.as_str()) == Some("l") && t.get(2).map(|s| s.as_str()) == Some("dd.lat")
    }));
    assert!(tags.iter().any(|t| {
        t.get(0).map(|s| s.as_str()) == Some("l") && t.get(2).map(|s| s.as_str()) == Some("dd.lon")
    }));

    assert!(tags.iter().any(|t| {
        t.get(0).map(|s| s.as_str()) == Some("image")
            && t.get(1).map(|s| s.as_str()) == Some("http://example.com/widget.jpg")
            && t.get(2).map(|s| s.as_str()) == Some("1200x800")
    }));
}

#[test]
fn listing_tags_full_uses_single_generic_price_for_primary_bin() {
    let mut listing = sample_listing_full("AAAAAAAAAAAAAAAAAAAAAw");
    listing.bins.push(RadrootsListingBin {
        bin_id: bin_id("bin-2"),
        quantity: RadrootsCoreQuantity::new(
            RadrootsCoreDecimal::from_str("500").unwrap(),
            RadrootsCoreUnit::MassG,
        ),
        price_per_canonical_unit: RadrootsCoreQuantityPrice::new(
            RadrootsCoreMoney::new(
                RadrootsCoreDecimal::from_str("0.02").unwrap(),
                RadrootsCoreCurrency::USD,
            ),
            RadrootsCoreQuantity::new(RadrootsCoreDecimal::from(1u32), RadrootsCoreUnit::MassG),
        ),
        display_amount: Some(RadrootsCoreDecimal::from(500u32)),
        display_unit: Some(RadrootsCoreUnit::MassG),
        display_label: Some("sample".to_string()),
        display_price: Some(RadrootsCoreMoney::new(
            RadrootsCoreDecimal::from_str("10").unwrap(),
            RadrootsCoreCurrency::USD,
        )),
        display_price_unit: Some(RadrootsCoreUnit::MassG),
    });

    let tags = listing_tags_full(&listing).unwrap();
    let generic_price_tags: Vec<&Vec<String>> = tags
        .iter()
        .filter(|tag| tag.first().map(|value| value.as_str()) == Some("price"))
        .collect();
    assert_eq!(generic_price_tags.len(), 1);
    assert_eq!(
        generic_price_tags[0].get(1).map(|value| value.as_str()),
        Some("10")
    );
    assert_eq!(
        generic_price_tags[0].get(2).map(|value| value.as_str()),
        Some("USD")
    );
}

#[test]
fn listing_tags_full_includes_trade_fields() {
    let mut listing = sample_listing("AAAAAAAAAAAAAAAAAAAAAg");
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
        t.get(0).map(|s| s.as_str()) == Some("radroots:availability_start")
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
    let mut listing = sample_listing("AAAAAAAAAAAAAAAAAAAAAg");
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
    let mut listing = sample_listing_full("AAAAAAAAAAAAAAAAAAAAAg");
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
    assert!(
        !tags
            .iter()
            .any(|tag| tag.iter().any(|value| value == "null"))
    );
}

#[test]
fn listing_tags_with_options_cover_location_fallback_paths() {
    let mut geohash_only = sample_listing("AAAAAAAAAAAAAAAAAAAAAg");
    geohash_only.location = Some(RadrootsListingLocation {
        primary: "Moyobamba".to_string(),
        city: None,
        region: None,
        country: None,
        lat: None,
        lng: None,
        geohash: Some("6gkzwgjzn".to_string()),
    });
    let tags = listing_tags_with_options(&geohash_only, ListingTagOptions::default()).unwrap();
    assert!(
        tags.iter()
            .any(|tag| tag.get(0).map(|value| value.as_str()) == Some("g"))
    );
    assert!(tags.iter().any(|tag| {
        tag.get(0).map(|value| value.as_str()) == Some("l")
            && tag.get(2).map(|value| value.as_str()) == Some("dd")
    }));

    let mut no_coordinates = sample_listing("AAAAAAAAAAAAAAAAAAAAAQ");
    no_coordinates.location = Some(RadrootsListingLocation {
        primary: "Moyobamba".to_string(),
        city: None,
        region: None,
        country: None,
        lat: None,
        lng: None,
        geohash: None,
    });
    let tags = listing_tags_with_options(&no_coordinates, ListingTagOptions::default()).unwrap();
    assert!(
        !tags
            .iter()
            .any(|tag| tag.get(0).map(|value| value.as_str()) == Some("L"))
    );
    assert!(
        !tags
            .iter()
            .any(|tag| tag.get(0).map(|value| value.as_str()) == Some("g"))
    );

    let mut no_gps = sample_listing("AAAAAAAAAAAAAAAAAAAAAw");
    no_gps.location = Some(RadrootsListingLocation {
        primary: "Moyobamba".to_string(),
        city: None,
        region: None,
        country: None,
        lat: Some(-6.0346),
        lng: Some(-76.9714),
        geohash: None,
    });
    let tags = listing_tags_with_options(
        &no_gps,
        ListingTagOptions {
            include_gps: false,
            ..ListingTagOptions::default()
        },
    )
    .unwrap();
    assert!(
        tags.iter()
            .any(|tag| tag.get(0).map(|value| value.as_str()) == Some("g"))
    );
    assert!(
        !tags
            .iter()
            .any(|tag| tag.get(0).map(|value| value.as_str()) == Some("L"))
    );
}
