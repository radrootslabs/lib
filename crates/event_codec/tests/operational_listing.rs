#![cfg(feature = "json")]

use radroots_core::pricing::{Discount, DiscountScope, DiscountThreshold, DiscountValue};
use radroots_core::{Currency, Decimal, Money, Quantity, QuantityPrice, Unit};
use radroots_event::{
    envelope::EventEnvelope,
    envelope::EventEnvelopeParts,
    envelope::kind::{
        KIND_CLASSIFIED_LISTING, KIND_FARM, KIND_PLOT, KIND_POST, KIND_RESOURCE_AREA,
    },
    farm::FarmRef,
    farm::plot::PlotRef,
    farm::resource_area::ResourceAreaRef,
    id::{DTag, InventoryBinId},
    listing::operational::OperationalListingParseError,
    listing::operational::{
        OperationalListing, OperationalListingAvailability, OperationalListingBin,
        OperationalListingDeliveryMethod, OperationalListingImage, OperationalListingImageSize,
        OperationalListingProduct, OperationalListingPublicLocation, OperationalListingStatus,
    },
    tag::name::{TAG_D, TAG_PUBLISHED_AT},
};
use radroots_event_codec::decode::EventParseError;
use radroots_event_codec::decode::operational_listing::{
    data_from_event, data_from_nostr_event, operational_listing_from_event,
    operational_listing_from_event_parts, operational_listing_from_nostr_event, parsed_from_event,
    parsed_from_nostr_event,
};
use radroots_event_codec::encode::EventEncodeError;
use radroots_event_codec::encode::operational_listing::{
    OperationalListingTagOptions, operational_listing_tags_full,
    operational_listing_tags_with_options,
};
use radroots_event_codec::encode::operational_listing::{
    operational_listing_build_tags, to_wire_parts, to_wire_parts_with_kind,
};
use std::{borrow::Cow, collections::BTreeSet, fs, path::Path, str::FromStr};

use serde_json::Value;

const EVENT_ID: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const AUTHOR: &str = "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df";
const EVENT_SIG: &str = concat!(
    "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
    "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
);
const PACKAGED_PARSE_VECTORS: &str =
    include_str!("fixtures/operational_listing_parse_event.v1.json");
const WORKSPACE_PARSE_VECTOR_PATH: &str =
    "../../contracts/conformance/vectors/operational_listing/parse_event.v1.json";
const WORKSPACE_CONTRACT_MARKER_PATH: &str = "../../contracts/manifest.toml";

fn listing_d_tag(raw: &str) -> DTag {
    raw.parse().unwrap()
}

fn bin_id(raw: &str) -> InventoryBinId {
    raw.parse().unwrap()
}

fn money(amount: Decimal, currency: Currency) -> Money {
    Money::try_new(amount, currency).unwrap()
}

fn quantity(amount: Decimal, unit: Unit) -> Quantity {
    Quantity::try_new(amount, unit).unwrap()
}

fn quantity_price(amount: Money, quantity: Quantity) -> QuantityPrice {
    QuantityPrice::try_new(amount, quantity).unwrap()
}

fn sample_operational_listing_tags() -> Vec<Vec<String>> {
    operational_listing_build_tags(&sample_listing("AAAAAAAAAAAAAAAAAAAAAg")).unwrap()
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

fn event_envelope(kind: u32, tags: Vec<Vec<String>>, content: String) -> EventEnvelope {
    EventEnvelope::new(EventEnvelopeParts {
        id: EVENT_ID.to_string(),
        author: AUTHOR.to_string(),
        created_at: 7,
        kind,
        tags,
        content,
        sig: EVENT_SIG.to_string(),
    })
    .unwrap()
}

fn assert_missing_tag(tags: Vec<Vec<String>>, expected: &'static str) {
    match operational_listing_from_event(KIND_CLASSIFIED_LISTING, &tags, "# Widget") {
        Err(EventParseError::MissingTag(tag)) => assert_eq!(tag, expected),
        other => panic!("expected missing tag {expected}: {other:?}"),
    }
}

fn assert_invalid_tag(tags: Vec<Vec<String>>, expected: &'static str) {
    match operational_listing_from_event(KIND_CLASSIFIED_LISTING, &tags, "# Widget") {
        Err(EventParseError::InvalidTag(tag)) => assert_eq!(tag, expected),
        other => panic!("expected invalid tag {expected}: {other:?}"),
    }
}

fn sample_listing(d_tag: &str) -> OperationalListing {
    let quantity = quantity(Decimal::from(1u32), Unit::Each);
    let price = quantity_price(money(Decimal::from(10u32), Currency::USD), quantity.clone());

    OperationalListing {
        d_tag: listing_d_tag(d_tag),
        published_at: None,
        farm: FarmRef {
            pubkey: "farm_pubkey".to_string(),
            d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_string(),
        },
        product: OperationalListingProduct {
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
        bins: vec![OperationalListingBin {
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

#[test]
fn operational_listing_from_event_parts_preserves_listing_error_taxonomy() {
    let mut tags = sample_operational_listing_tags();
    remove_tags(&mut tags, TAG_D);
    assert_eq!(
        operational_listing_from_event_parts(&tags, "").unwrap_err(),
        OperationalListingParseError::MissingTag(TAG_D.to_string())
    );

    let mut tags = sample_operational_listing_tags();
    replace_first_tag(&mut tags, TAG_D, vec![TAG_D, "bad d"]);
    assert_eq!(
        operational_listing_from_event_parts(&tags, "").unwrap_err(),
        OperationalListingParseError::InvalidTag(TAG_D.to_string())
    );

    let mut tags = sample_operational_listing_tags();
    replace_first_tag(
        &mut tags,
        "radroots:bin",
        vec!["radroots:bin", "bin-1", "bad", "each"],
    );
    assert_eq!(
        operational_listing_from_event_parts(&tags, "").unwrap_err(),
        OperationalListingParseError::InvalidNumber("radroots:bin".to_string())
    );

    let mut tags = sample_operational_listing_tags();
    replace_first_tag(
        &mut tags,
        "radroots:bin",
        vec!["radroots:bin", "bin-1", "1", "bad"],
    );
    assert_eq!(
        operational_listing_from_event_parts(&tags, "").unwrap_err(),
        OperationalListingParseError::InvalidUnit
    );

    let mut tags = sample_operational_listing_tags();
    replace_first_tag(
        &mut tags,
        "radroots:price",
        vec!["radroots:price", "bin-1", "10", "not-currency", "1", "each"],
    );
    assert_eq!(
        operational_listing_from_event_parts(&tags, "").unwrap_err(),
        OperationalListingParseError::InvalidCurrency
    );

    let mut tags = sample_operational_listing_tags();
    tags.push(vec!["radroots:discount".to_string(), "{".to_string()]);
    assert_eq!(
        operational_listing_from_event_parts(&tags, "").unwrap_err(),
        OperationalListingParseError::InvalidDiscount("radroots:discount".to_string())
    );

    assert_eq!(
        operational_listing_from_event_parts(
            &sample_operational_listing_tags(),
            r#"{"location":{"coordinates":[1,2]}}"#,
        )
        .unwrap_err(),
        OperationalListingParseError::InvalidJson("location".to_string())
    );
}

#[test]
fn operational_listing_from_event_parts_does_not_allow_json_content_to_override_tags() {
    let tags = sample_operational_listing_tags();
    let mut listing = sample_listing("AAAAAAAAAAAAAAAAAAAAAg");
    listing.product.title = "Content override".to_string();
    listing.product.category = "Content category".to_string();
    listing.inventory_available = Some(Decimal::from(99u32));
    listing.bins[0].quantity = quantity(Decimal::from(42u32), Unit::Each);

    let decoded =
        operational_listing_from_event_parts(&tags, &serde_json::to_string(&listing).unwrap())
            .expect("decode tag-authoritative listing");

    assert_eq!(decoded.product.title, "Widget");
    assert_eq!(decoded.product.category, "Tools");
    assert_eq!(decoded.inventory_available, None);
    assert_eq!(decoded.bins[0].quantity.amount(), Decimal::from(1u32));
}

#[test]
fn checked_in_listing_parse_vectors_execute_against_the_typed_decoder() {
    let vectors = listing_parse_vectors();
    let suite: Value = serde_json::from_str(&vectors).expect("listing parse vectors must parse");
    assert_eq!(suite["suite"], "operational_listing");
    assert_eq!(suite["contract_version"], "1.0.0");
    let vectors = suite["vectors"].as_array().expect("listing parse vectors");
    assert_eq!(vectors.len(), 9, "listing parse vector inventory drifted");

    let mut ids = BTreeSet::new();
    for vector in vectors {
        let id = vector["id"].as_str().expect("vector id");
        assert!(ids.insert(id), "duplicate listing parse vector id {id}");
        execute_listing_parse_vector(vector, id);
    }
}

fn listing_parse_vectors() -> Cow<'static, str> {
    let workspace_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(WORKSPACE_PARSE_VECTOR_PATH);
    match fs::read_to_string(&workspace_path) {
        Ok(canonical) => {
            assert_eq!(
                canonical,
                PACKAGED_PARSE_VECTORS,
                "packaged listing parse vectors must match {}",
                workspace_path.display()
            );
            Cow::Owned(canonical)
        }
        Err(error)
            if error.kind() == std::io::ErrorKind::NotFound
                && !Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join(WORKSPACE_CONTRACT_MARKER_PATH)
                    .is_file() =>
        {
            Cow::Borrowed(PACKAGED_PARSE_VECTORS)
        }
        Err(error) => panic!("failed to read {}: {error}", workspace_path.display()),
    }
}

fn execute_listing_parse_vector(vector: &Value, id: &str) {
    let input = &vector["input"];
    let tags: Vec<Vec<String>> =
        serde_json::from_value(input["tags"].clone()).expect("vector tags");
    let content = input["content"].as_str().expect("vector content");
    let event_kind = input["event_kind"].as_u64().expect("vector event kind") as u32;
    let event = event_envelope(event_kind, tags, content.to_string());

    match vector["kind"].as_str().expect("vector kind") {
        "operational_listing.parse_event.valid" => {
            let listing = operational_listing_from_nostr_event(&event)
                .unwrap_or_else(|error| panic!("{id} failed: {error}"));
            let expected = &vector["expected"]["listing"];
            assert_eq!(listing.d_tag.as_str(), expected["d_tag"], "{id}");
            assert_eq!(listing.product.title, expected["title"], "{id}");
            assert_eq!(listing.product.category, expected["category"], "{id}");
            assert_eq!(
                listing.primary_bin_id.as_str(),
                expected["primary_bin_id"],
                "{id}"
            );
            let primary = listing
                .bins
                .iter()
                .find(|bin| bin.bin_id == listing.primary_bin_id)
                .expect("primary listing bin");
            assert_eq!(
                primary.quantity.amount().to_string(),
                expected["quantity_amount"],
                "{id}"
            );
            assert_eq!(
                primary.quantity.unit().code(),
                expected["quantity_unit"],
                "{id}"
            );
            assert_eq!(
                primary
                    .price_per_canonical_unit
                    .amount()
                    .amount()
                    .to_string(),
                expected["price_amount"],
                "{id}"
            );
            assert_eq!(
                primary
                    .price_per_canonical_unit
                    .amount()
                    .currency()
                    .as_str(),
                expected["price_currency"],
                "{id}"
            );
            assert_eq!(
                serde_json::to_value(
                    listing
                        .inventory_available
                        .as_ref()
                        .map(ToString::to_string)
                )
                .expect("inventory projection"),
                expected["inventory_available"],
                "{id}"
            );
        }
        "operational_listing.parse_event.invalid" => {
            let error = operational_listing_from_nostr_event(&event)
                .expect_err("invalid listing parse vector must fail");
            assert_eq!(
                serde_json::to_value(error).expect("listing parse error"),
                vector["expected"]["error"],
                "{id}"
            );
        }
        kind => panic!("{id} uses unsupported listing parse vector kind {kind}"),
    }
}

fn sample_listing_full(d_tag: &str) -> OperationalListing {
    let qty_amount = Decimal::from_str("1000").unwrap();
    let price_amount = Decimal::from_str("0.01").unwrap();
    let display_qty = Decimal::from_str("1").unwrap();
    let display_price = Decimal::from_str("10").unwrap();

    OperationalListing {
        d_tag: listing_d_tag(d_tag),
        published_at: None,
        farm: FarmRef {
            pubkey: "farm_pubkey".to_string(),
            d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_string(),
        },
        product: OperationalListingProduct {
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
        bins: vec![OperationalListingBin {
            bin_id: bin_id("bin-1"),
            quantity: quantity(qty_amount, Unit::MassG),
            price_per_canonical_unit: quantity_price(
                money(price_amount, Currency::USD),
                quantity(Decimal::from(1u32), Unit::MassG),
            ),
            display_amount: Some(display_qty),
            display_unit: Some(Unit::MassKg),
            display_label: Some("bag".to_string()),
            display_price: Some(money(display_price, Currency::USD)),
            display_price_unit: Some(Unit::MassKg),
        }],
        resource_area: None,
        plot: None,
        discounts: Some(vec![
            Discount::try_new(
                DiscountScope::Bin,
                DiscountThreshold::BinCount {
                    bin_id: "bin-1".to_string(),
                    min: 5,
                },
                DiscountValue::MoneyPerBin(money(Decimal::from_str("2").unwrap(), Currency::USD)),
            )
            .unwrap(),
        ]),
        inventory_available: None,
        availability: None,
        delivery_method: None,
        location: Some(OperationalListingPublicLocation {
            primary: "Moyobamba".to_string(),
            city: Some("Moyobamba".to_string()),
            region: Some("San Martin".to_string()),
            country: Some("PE".to_string()),
            geohash: "9q8yy".to_string(),
        }),
        images: Some(vec![OperationalListingImage {
            url: "http://example.com/widget.jpg".to_string(),
            size: Some(OperationalListingImageSize { w: 1200, h: 800 }),
        }]),
    }
}

#[test]
fn operational_listing_build_tags_requires_d_tag() {
    assert!(DTag::parse("").is_err());
}

#[test]
fn operational_listing_build_tags_rejects_invalid_d_tag() {
    let listing = sample_listing("invalid:tag");
    let err = operational_listing_build_tags(&listing).unwrap_err();
    assert!(matches!(err, EventEncodeError::InvalidField("d")));
}

#[test]
fn listing_roundtrip_from_event() {
    let listing = sample_listing("AAAAAAAAAAAAAAAAAAAAAg");
    let parts = to_wire_parts(&listing).unwrap();

    assert_eq!(parts.content, "# Widget");

    let decoded = operational_listing_from_event(parts.kind, &parts.tags, &parts.content).unwrap();
    assert_eq!(decoded.d_tag, listing.d_tag);
    assert_eq!(decoded.product.key, listing.product.key);
    assert_eq!(decoded.product.title, listing.product.title);
    assert_eq!(decoded.primary_bin_id, listing.primary_bin_id);
    assert_eq!(decoded.bins.len(), listing.bins.len());
}

#[test]
fn operational_listing_from_event_reconstructs_from_tags_with_markdown_content() {
    let listing = sample_listing_full("FAAAAAAAAAAAAAAAAAAAAA");
    let tags = operational_listing_build_tags(&listing).unwrap();

    let decoded =
        operational_listing_from_event(KIND_CLASSIFIED_LISTING, &tags, "### Markdown listing")
            .unwrap();
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
fn operational_listing_from_event_rejects_invalid_d_tag() {
    let mut tags =
        operational_listing_build_tags(&sample_listing("AAAAAAAAAAAAAAAAAAAAAg")).unwrap();
    let d_tag = tags
        .iter_mut()
        .find(|tag| tag.first().map(|value| value.as_str()) == Some(TAG_D))
        .expect("d tag");
    d_tag[1] = "invalid:tag".to_string();

    let err =
        operational_listing_from_event(KIND_CLASSIFIED_LISTING, &tags, "# Widget").unwrap_err();
    assert!(matches!(err, EventParseError::InvalidTag(TAG_D)));
}

#[test]
fn operational_listing_from_event_rejects_wrong_kind() {
    let tags = operational_listing_build_tags(&sample_listing("AAAAAAAAAAAAAAAAAAAAAg")).unwrap();

    let err = operational_listing_from_event(KIND_POST, &tags, "# Widget").unwrap_err();
    assert!(matches!(
        err,
        EventParseError::InvalidKind {
            expected: "30402",
            got: KIND_POST
        }
    ));
}

#[test]
fn operational_listing_from_event_covers_reference_tag_error_paths() {
    let mut tags = sample_operational_listing_tags();
    remove_tags(&mut tags, TAG_D);
    assert_missing_tag(tags, TAG_D);

    let mut tags = sample_operational_listing_tags();
    replace_first_tag(&mut tags, TAG_D, vec![TAG_D]);
    assert_invalid_tag(tags, TAG_D);

    let mut tags = sample_operational_listing_tags();
    replace_first_tag(&mut tags, TAG_D, vec![TAG_D, " "]);
    assert_invalid_tag(tags, TAG_D);

    let mut tags = sample_operational_listing_tags();
    remove_tags(&mut tags, "a");
    assert_missing_tag(tags, "a");

    let mut tags = sample_operational_listing_tags();
    replace_first_tag(&mut tags, "a", vec!["a"]);
    assert_invalid_tag(tags, "a");

    let mut tags = sample_operational_listing_tags();
    replace_first_tag(&mut tags, "a", vec!["a", "bad:farm_pubkey:farm"]);
    assert_invalid_tag(tags, "a");

    let mut tags = sample_operational_listing_tags();
    replace_first_tag(&mut tags, "a", vec!["a", "30340"]);
    assert_invalid_tag(tags, "a");

    let mut tags = sample_operational_listing_tags();
    replace_first_tag(&mut tags, "a", vec!["a", "30340::farm"]);
    assert_invalid_tag(tags, "a");

    let mut tags = sample_operational_listing_tags();
    replace_first_tag(&mut tags, "a", vec!["a", "30340:farm_pubkey:"]);
    assert_invalid_tag(tags, "a");

    let mut tags = sample_operational_listing_tags();
    replace_first_tag(&mut tags, "a", vec!["a", "30340:farm_pubkey:bad d"]);
    assert_invalid_tag(tags, "a");

    let mut tags = sample_operational_listing_tags();
    replace_first_tag(&mut tags, "a", vec!["a", "30023:other:article"]);
    assert_missing_tag(tags, "a");

    let mut tags = sample_operational_listing_tags();
    remove_tags(&mut tags, "p");
    assert_missing_tag(tags, "p");

    let mut tags = sample_operational_listing_tags();
    replace_first_tag(&mut tags, "p", vec!["p"]);
    assert_invalid_tag(tags, "p");

    let mut tags = sample_operational_listing_tags();
    replace_first_tag(&mut tags, "p", vec!["p", " "]);
    assert_invalid_tag(tags, "p");

    let mut tags = sample_operational_listing_tags();
    replace_first_tag(&mut tags, "p", vec!["p", "other_pubkey"]);
    assert_invalid_tag(tags, "p");
}

#[test]
fn operational_listing_from_event_covers_resource_and_plot_reference_paths() {
    let mut listing = sample_listing("AAAAAAAAAAAAAAAAAAAAAw");
    listing.resource_area = Some(ResourceAreaRef {
        pubkey: "resource_pubkey".to_string(),
        d_tag: "AAAAAAAAAAAAAAAAAAAABQ".to_string(),
    });
    listing.plot = Some(PlotRef {
        pubkey: "plot_pubkey".to_string(),
        d_tag: "AAAAAAAAAAAAAAAAAAAAAw".to_string(),
    });
    let tags = operational_listing_build_tags(&listing).unwrap();
    let decoded =
        operational_listing_from_event(KIND_CLASSIFIED_LISTING, &tags, "# Widget").unwrap();
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

    let mut tags = sample_operational_listing_tags();
    tags.push(vec!["radroots:resource_area".to_string()]);
    assert_invalid_tag(tags, "radroots:resource_area");

    let mut tags = sample_operational_listing_tags();
    tags.push(vec![
        "radroots:resource_area".to_string(),
        format!("{KIND_FARM}:resource_pubkey:resource-area-1"),
    ]);
    assert_invalid_tag(tags, "radroots:resource_area");

    let mut tags = sample_operational_listing_tags();
    tags.push(vec![
        "radroots:resource_area".to_string(),
        format!("{KIND_RESOURCE_AREA}::resource-area-1"),
    ]);
    assert_invalid_tag(tags, "radroots:resource_area");

    let mut tags = sample_operational_listing_tags();
    tags.push(vec![
        "radroots:resource_area".to_string(),
        format!("{KIND_RESOURCE_AREA}:resource_pubkey:"),
    ]);
    assert_invalid_tag(tags, "radroots:resource_area");

    let mut tags = sample_operational_listing_tags();
    tags.push(vec![
        "radroots:resource_area".to_string(),
        format!("{KIND_RESOURCE_AREA}:resource_pubkey:bad d"),
    ]);
    assert_invalid_tag(tags, "radroots:resource_area");

    let mut tags = sample_operational_listing_tags();
    tags.push(vec!["radroots:plot".to_string()]);
    assert_invalid_tag(tags, "radroots:plot");

    let mut tags = sample_operational_listing_tags();
    tags.push(vec![
        "radroots:plot".to_string(),
        format!("{KIND_RESOURCE_AREA}:plot_pubkey:plot-1"),
    ]);
    assert_invalid_tag(tags, "radroots:plot");

    let mut tags = sample_operational_listing_tags();
    tags.push(vec![
        "radroots:plot".to_string(),
        format!("{KIND_PLOT}:plot_pubkey:"),
    ]);
    assert_invalid_tag(tags, "radroots:plot");

    let mut tags = sample_operational_listing_tags();
    tags.push(vec![
        "radroots:plot".to_string(),
        format!("{KIND_PLOT}:plot_pubkey:bad d"),
    ]);
    assert_invalid_tag(tags, "radroots:plot");
}

#[test]
fn operational_listing_from_event_covers_bin_and_price_error_paths() {
    let mut tags = sample_operational_listing_tags();
    remove_tags(&mut tags, "radroots:primary_bin");
    assert_missing_tag(tags, "radroots:primary_bin");

    let mut tags = sample_operational_listing_tags();
    tags.push(vec![
        "radroots:primary_bin".to_string(),
        "bin-1".to_string(),
    ]);
    let decoded =
        operational_listing_from_event(KIND_CLASSIFIED_LISTING, &tags, "# Widget").unwrap();
    assert_eq!(decoded.primary_bin_id.as_str(), "bin-1");

    let mut tags = sample_operational_listing_tags();
    tags.push(vec![
        "radroots:primary_bin".to_string(),
        "bin-2".to_string(),
    ]);
    assert_invalid_tag(tags, "radroots:primary_bin");

    let mut tags = sample_operational_listing_tags();
    replace_first_tag(
        &mut tags,
        "radroots:primary_bin",
        vec!["radroots:primary_bin", "bin-2"],
    );
    assert_invalid_tag(tags, "radroots:primary_bin");

    let mut tags = sample_operational_listing_tags();
    remove_tags(&mut tags, "radroots:bin");
    assert_missing_tag(tags, "radroots:bin");

    let mut tags = sample_operational_listing_tags();
    remove_tags(&mut tags, "radroots:price");
    assert_missing_tag(tags, "radroots:price");

    let mut tags = sample_operational_listing_tags();
    replace_first_tag(&mut tags, "radroots:bin", vec!["radroots:bin"]);
    assert_invalid_tag(tags, "radroots:bin");

    let mut tags = sample_operational_listing_tags();
    replace_first_tag(
        &mut tags,
        "radroots:bin",
        vec!["radroots:bin", "bin-1", "1", "kg"],
    );
    assert_invalid_tag(tags, "radroots:bin");

    let mut tags = sample_operational_listing_tags();
    replace_first_tag(
        &mut tags,
        "radroots:bin",
        vec!["radroots:bin", "bin-1", "1", "not-a-unit"],
    );
    assert_invalid_tag(tags, "radroots:bin");

    let mut tags = sample_operational_listing_tags();
    replace_first_tag(
        &mut tags,
        "radroots:bin",
        vec!["radroots:bin", "bin-1", "1", "each", "1"],
    );
    assert_invalid_tag(tags, "radroots:bin");

    let mut tags = sample_operational_listing_tags();
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

    let mut tags = sample_operational_listing_tags();
    replace_first_tag(
        &mut tags,
        "radroots:bin",
        vec!["radroots:bin", "bin-1", "1", "each", "1", "each"],
    );
    let decoded =
        operational_listing_from_event(KIND_CLASSIFIED_LISTING, &tags, "# Widget").unwrap();
    assert_eq!(decoded.bins[0].display_amount, Some(Decimal::from(1u32)));
    assert_eq!(decoded.bins[0].display_unit, Some(Unit::Each));
    assert_eq!(decoded.bins[0].display_label, None);

    let mut tags = sample_operational_listing_tags();
    tags.push(vec![
        "radroots:bin".to_string(),
        "bin-1".to_string(),
        "1".to_string(),
        "each".to_string(),
    ]);
    assert_invalid_tag(tags, "radroots:bin");

    let mut tags = sample_operational_listing_tags();
    replace_first_tag(&mut tags, "radroots:price", vec!["radroots:price"]);
    assert_invalid_tag(tags, "radroots:price");

    let mut tags = sample_operational_listing_tags();
    replace_first_tag(
        &mut tags,
        "radroots:price",
        vec!["radroots:price", "bin-1", "10", "USD", "1", "kg"],
    );
    assert_invalid_tag(tags, "radroots:price");

    let mut tags = sample_operational_listing_tags();
    replace_first_tag(
        &mut tags,
        "radroots:price",
        vec!["radroots:price", "bin-1", "10", "not-currency", "1", "each"],
    );
    assert_invalid_tag(tags, "radroots:price");

    let mut tags = sample_operational_listing_tags();
    replace_first_tag(
        &mut tags,
        "radroots:price",
        vec!["radroots:price", "bin-1", "10", "USD", "1", "each", "10"],
    );
    assert_invalid_tag(tags, "radroots:price");

    let mut tags = sample_operational_listing_tags();
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

    let mut tags = sample_operational_listing_tags();
    tags.push(vec![
        "radroots:price".to_string(),
        "bin-1".to_string(),
        "10".to_string(),
        "USD".to_string(),
        "1".to_string(),
        "each".to_string(),
    ]);
    assert_invalid_tag(tags, "radroots:price");

    let mut tags = sample_operational_listing_tags();
    replace_first_tag(
        &mut tags,
        "radroots:price",
        vec!["radroots:price", "bin-1", "10", "USD", "1", "g"],
    );
    assert_invalid_tag(tags, "radroots:price");
}

#[test]
fn operational_listing_from_event_covers_trade_location_delivery_and_image_paths() {
    for expected in ["dd", "dd.lat", "dd.lon", "l", "L"] {
        let mut tags = sample_operational_listing_tags();
        tags.push(vec![expected.to_string(), "synthetic".to_string()]);
        assert_invalid_tag(tags, expected);
    }

    let mut tags = sample_operational_listing_tags();
    tags.push(vec!["location".to_string(), "Farm shelf".to_string()]);
    let decoded =
        operational_listing_from_event(KIND_CLASSIFIED_LISTING, &tags, "# Widget").unwrap();
    assert_eq!(decoded.product.location.as_deref(), Some("Farm shelf"));
    assert!(decoded.location.is_none());

    let mut tags = sample_operational_listing_tags();
    tags.push(vec!["location".to_string(), "Farm shelf".to_string()]);
    tags.push(vec![
        "location".to_string(),
        "Peru".to_string(),
        "Moyobamba".to_string(),
        "San Martin".to_string(),
        "PE".to_string(),
    ]);
    tags.push(vec!["g".to_string(), "6gkzw".to_string()]);
    let decoded =
        operational_listing_from_event(KIND_CLASSIFIED_LISTING, &tags, "# Widget").unwrap();
    assert_eq!(decoded.product.location.as_deref(), Some("Farm shelf"));
    assert_eq!(
        decoded.location.as_ref().map(|location| {
            (
                location.primary.as_str(),
                location.city.as_deref(),
                location.geohash.as_str(),
            )
        }),
        Some(("Peru", Some("Moyobamba"), "6gkzw"))
    );

    let mut tags = sample_operational_listing_tags();
    tags.push(vec![
        "location".to_string(),
        " ".to_string(),
        "Moyobamba".to_string(),
    ]);
    assert_invalid_tag(tags, "location");

    let mut tags = sample_operational_listing_tags();
    tags.push(vec![
        "location".to_string(),
        "Farm stand".to_string(),
        " ".to_string(),
        "null".to_string(),
        " ".to_string(),
    ]);
    tags.push(vec!["g".to_string(), "9q8yy".to_string()]);
    let decoded =
        operational_listing_from_event(KIND_CLASSIFIED_LISTING, &tags, "# Widget").unwrap();
    assert_eq!(
        decoded.location.as_ref().map(|location| {
            (
                location.primary.as_str(),
                location.city.as_deref(),
                location.region.as_deref(),
                location.country.as_deref(),
                location.geohash.as_str(),
            )
        }),
        Some(("Farm stand", None, None, None, "9q8yy"))
    );

    let mut tags = sample_operational_listing_tags();
    tags.push(vec!["g".to_string(), "9q8ya".to_string()]);
    assert_invalid_tag(tags, "g");

    let mut tags = sample_operational_listing_tags();
    tags.push(vec!["g".to_string(), "9q8yy".to_string()]);
    tags.push(vec!["g".to_string(), "6gkzw".to_string()]);
    assert_invalid_tag(tags, "g");

    let mut tags = sample_operational_listing_tags();
    tags.push(vec!["inventory".to_string()]);
    assert_invalid_tag(tags, "inventory");

    let mut tags = sample_operational_listing_tags();
    tags.push(vec!["inventory".to_string(), "bad".to_string()]);
    assert_invalid_tag(tags, "inventory");

    let mut tags = sample_operational_listing_tags();
    tags.push(vec!["inventory".to_string(), "12.5".to_string()]);
    tags.push(vec![
        "radroots:availability_start".to_string(),
        "1730".to_string(),
    ]);
    tags.push(vec!["expires_at".to_string(), "1740".to_string()]);
    tags.push(vec!["delivery".to_string(), "pickup".to_string()]);
    tags.push(vec!["image".to_string(), " ".to_string()]);
    tags.push(vec![
        "image".to_string(),
        "https://example.test/a.jpg".to_string(),
    ]);
    tags.push(vec![
        "image".to_string(),
        "https://example.test/b.jpg".to_string(),
        "bad-size".to_string(),
    ]);
    let decoded =
        operational_listing_from_event(KIND_CLASSIFIED_LISTING, &tags, "# Widget").unwrap();
    let Some(OperationalListingAvailability::Window { start, end }) = decoded.availability else {
        panic!("expected availability window");
    };
    assert_eq!(start, Some(1730));
    assert_eq!(end, Some(1740));
    assert!(matches!(
        decoded.delivery_method,
        Some(OperationalListingDeliveryMethod::Pickup)
    ));
    assert_eq!(decoded.images.as_ref().map(Vec::len), Some(2));
    assert!(decoded.images.as_ref().unwrap()[1].size.is_none());

    let mut tags = sample_operational_listing_tags();
    tags.push(vec!["delivery".to_string(), "local_delivery".to_string()]);
    let decoded =
        operational_listing_from_event(KIND_CLASSIFIED_LISTING, &tags, "# Widget").unwrap();
    assert!(matches!(
        decoded.delivery_method,
        Some(OperationalListingDeliveryMethod::LocalDelivery)
    ));

    let mut tags = sample_operational_listing_tags();
    tags.push(vec!["delivery".to_string(), "shipping".to_string()]);
    let decoded =
        operational_listing_from_event(KIND_CLASSIFIED_LISTING, &tags, "# Widget").unwrap();
    assert!(matches!(
        decoded.delivery_method,
        Some(OperationalListingDeliveryMethod::Shipping)
    ));

    let mut tags = sample_operational_listing_tags();
    tags.push(vec![
        "delivery".to_string(),
        "other".to_string(),
        "bike courier".to_string(),
    ]);
    let decoded =
        operational_listing_from_event(KIND_CLASSIFIED_LISTING, &tags, "# Widget").unwrap();
    let Some(OperationalListingDeliveryMethod::Other { method }) = decoded.delivery_method else {
        panic!("expected other delivery method");
    };
    assert_eq!(method, "bike courier");

    let mut tags = sample_operational_listing_tags();
    tags.push(vec!["delivery".to_string(), "drone".to_string()]);
    let decoded =
        operational_listing_from_event(KIND_CLASSIFIED_LISTING, &tags, "# Widget").unwrap();
    let Some(OperationalListingDeliveryMethod::Other { method }) = decoded.delivery_method else {
        panic!("expected fallback delivery method");
    };
    assert_eq!(method, "drone");

    let mut tags = sample_operational_listing_tags();
    tags.push(vec!["status".to_string(), "active".to_string()]);
    let decoded =
        operational_listing_from_event(KIND_CLASSIFIED_LISTING, &tags, "# Widget").unwrap();
    assert!(matches!(
        decoded.availability,
        Some(OperationalListingAvailability::Status {
            status: OperationalListingStatus::Active
        })
    ));

    let mut tags = sample_operational_listing_tags();
    tags.push(vec!["status".to_string(), "sold".to_string()]);
    let decoded =
        operational_listing_from_event(KIND_CLASSIFIED_LISTING, &tags, "# Widget").unwrap();
    assert!(matches!(
        decoded.availability,
        Some(OperationalListingAvailability::Status {
            status: OperationalListingStatus::Sold
        })
    ));

    let mut tags = sample_operational_listing_tags();
    tags.push(vec!["status".to_string(), "paused".to_string()]);
    let decoded =
        operational_listing_from_event(KIND_CLASSIFIED_LISTING, &tags, "# Widget").unwrap();
    let Some(OperationalListingAvailability::Status {
        status: OperationalListingStatus::Other { value },
    }) = decoded.availability
    else {
        panic!("expected other availability status");
    };
    assert_eq!(value, "paused");
}

#[test]
fn operational_listing_from_event_rejects_private_location_content_edges() {
    let tags = sample_operational_listing_tags();
    for content in [
        "# Widget",
        "{not-json",
        r#"{"name":"Widget"}"#,
        r#"{"location":{"public_label":"Farm shelf"}}"#,
    ] {
        let decoded =
            operational_listing_from_event(KIND_CLASSIFIED_LISTING, &tags, content).unwrap();
        assert_eq!(decoded.product.key, "sku");
    }

    for key in [
        "lat",
        "lng",
        "lon",
        "point",
        "polygon",
        "coordinates",
        "accuracy",
        "altitude",
        "label",
        "tag_0",
        "gcs",
    ] {
        let content = format!(r#"{{"location":{{"{key}":"secret"}}}}"#);
        let err =
            operational_listing_from_event(KIND_CLASSIFIED_LISTING, &tags, &content).unwrap_err();
        assert!(matches!(err, EventParseError::InvalidJson("content")));
    }
}

#[test]
fn operational_listing_from_event_covers_remaining_edge_paths() {
    let mut tags = sample_operational_listing_tags();
    tags.insert(0, Vec::new());
    tags.push(vec!["location".to_string()]);
    let decoded =
        operational_listing_from_event(KIND_CLASSIFIED_LISTING, &tags, "# Widget").unwrap();
    assert_eq!(decoded.product.location, None);

    let mut tags = sample_operational_listing_tags();
    tags.push(vec![
        "radroots:plot".to_string(),
        format!("{KIND_PLOT}::AAAAAAAAAAAAAAAAAAAAAw"),
    ]);
    assert_invalid_tag(tags, "radroots:plot");

    let mut tags = sample_operational_listing_tags();
    tags.push(vec![
        "radroots:primary_bin".to_string(),
        "bin-2".to_string(),
    ]);
    assert_invalid_tag(tags, "radroots:primary_bin");

    let mut tags = sample_operational_listing_tags();
    let primary_position = tags
        .iter()
        .position(|tag| tag.first().map(String::as_str) == Some("radroots:primary_bin"))
        .expect("primary bin tag");
    tags.insert(
        primary_position + 1,
        vec!["radroots:primary_bin".to_string(), "bin-2".to_string()],
    );
    assert_invalid_tag(tags, "radroots:primary_bin");

    let mut tags = sample_operational_listing_tags();
    tags.insert(0, vec!["key".to_string(), " ".to_string()]);
    tags.push(vec!["key".to_string(), "ignored".to_string()]);
    tags.insert(0, vec!["summary".to_string(), " ".to_string()]);
    tags.push(vec!["summary".to_string(), "first summary".to_string()]);
    tags.push(vec!["summary".to_string(), "ignored summary".to_string()]);
    tags.push(vec!["process".to_string(), "null".to_string()]);
    tags.push(vec!["lot".to_string(), " null ".to_string()]);
    tags.push(vec!["profile".to_string(), "null".to_string()]);
    tags.push(vec!["year".to_string(), "null".to_string()]);
    let decoded =
        operational_listing_from_event(KIND_CLASSIFIED_LISTING, &tags, "# Widget").unwrap();
    assert_eq!(decoded.product.key, "sku");
    assert_eq!(decoded.product.summary.as_deref(), Some("first summary"));
    assert_eq!(decoded.product.process, None);
    assert_eq!(decoded.product.lot, None);
    assert_eq!(decoded.product.profile, None);
    assert_eq!(decoded.product.year, None);

    let mut tags = sample_operational_listing_tags();
    tags.push(vec!["radroots:availability_start".to_string()]);
    assert_invalid_tag(tags, "radroots:availability_start");

    let mut tags = sample_operational_listing_tags();
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
        EVENT_ID.to_string(),
        AUTHOR.to_string(),
        7,
        parts.kind,
        parts.content.clone(),
        parts.tags.clone(),
    )
    .unwrap();
    assert_eq!(data.id, EVENT_ID);
    assert_eq!(data.author, AUTHOR);
    assert_eq!(data.published_at, 7);
    assert_eq!(data.kind, KIND_CLASSIFIED_LISTING);
    assert_eq!(data.data.d_tag, listing.d_tag);

    let parsed = parsed_from_event(
        EVENT_ID.to_string(),
        AUTHOR.to_string(),
        7,
        parts.kind,
        parts.content.clone(),
        parts.tags.clone(),
        EVENT_SIG.to_string(),
    )
    .unwrap();
    assert_eq!(parsed.event.id_hex(), EVENT_ID);
    assert_eq!(parsed.event.author().to_hex(), AUTHOR);
    assert_eq!(parsed.event.created_at_u64(), 7);
    assert_eq!(parsed.event.signature_hex(), EVENT_SIG);
    assert_eq!(parsed.data.data.d_tag, listing.d_tag);

    let event = event_envelope(parts.kind, parts.tags, parts.content);
    let data = data_from_nostr_event(&event).unwrap();
    assert_eq!(data.data.d_tag, listing.d_tag);
    let parsed = parsed_from_nostr_event(&event).unwrap();
    assert_eq!(parsed.event.signature_hex(), EVENT_SIG);
    assert_eq!(parsed.data.data.d_tag, listing.d_tag);

    let err = parsed_from_event(
        EVENT_ID.to_string(),
        AUTHOR.to_string(),
        7,
        KIND_POST,
        event.content().to_string(),
        event.tags_as_vec(),
        EVENT_SIG.to_string(),
    )
    .unwrap_err();
    assert!(matches!(
        err,
        EventParseError::InvalidKind {
            expected: "30402",
            got: KIND_POST
        }
    ));
}

#[test]
fn retired_listing_kind_is_rejected() {
    let mut listing = sample_listing("AAAAAAAAAAAAAAAAAAAAAQ");
    listing.published_at = Some(1_781_895_600);

    assert!(matches!(
        to_wire_parts_with_kind(&listing, 30403),
        Err(EventEncodeError::InvalidKind(30403))
    ));
}

#[test]
fn listing_roundtrips_published_at_for_active_and_rejects_bad_value() {
    let mut listing = sample_listing("AAAAAAAAAAAAAAAAAAAAAg");
    listing.published_at = Some(1_781_895_600);
    let parts = to_wire_parts_with_kind(&listing, KIND_CLASSIFIED_LISTING).unwrap();
    assert!(parts.tags.iter().any(|tag| {
        tag.first().map(|value| value.as_str()) == Some(TAG_PUBLISHED_AT)
            && tag.get(1).map(|value| value.as_str()) == Some("1781895600")
    }));

    let decoded = operational_listing_from_event(parts.kind, &parts.tags, &parts.content).unwrap();
    assert_eq!(decoded.published_at, Some(1_781_895_600));

    let mut tags = parts.tags;
    let published_at = tags
        .iter_mut()
        .find(|tag| tag.first().map(|value| value.as_str()) == Some(TAG_PUBLISHED_AT))
        .expect("published_at tag");
    published_at[1] = "bad".to_string();
    let err =
        operational_listing_from_event(KIND_CLASSIFIED_LISTING, &tags, "# Widget").unwrap_err();
    assert!(matches!(err, EventParseError::InvalidTag(TAG_PUBLISHED_AT)));
}

#[test]
fn to_wire_parts_rejects_non_listing_kind() {
    let err =
        to_wire_parts_with_kind(&sample_listing("AAAAAAAAAAAAAAAAAAAAAg"), KIND_POST).unwrap_err();
    assert!(matches!(err, EventEncodeError::InvalidKind(KIND_POST)));
}

#[test]
fn operational_listing_build_tags_includes_listing_fields() {
    let listing = sample_listing_full("AAAAAAAAAAAAAAAAAAAAAg");
    let tags = operational_listing_build_tags(&listing).unwrap();

    assert!(tags.iter().any(|t| {
        t.first().map(|s| s.as_str()) == Some(TAG_D)
            && t.get(1).map(|s| s.as_str()) == Some("AAAAAAAAAAAAAAAAAAAAAg")
    }));
    assert!(tags.iter().any(|t| {
        t.first().map(|s| s.as_str()) == Some("p")
            && t.get(1).map(|s| s.as_str()) == Some("farm_pubkey")
    }));
    assert!(tags.iter().any(|t| {
        t.first().map(|s| s.as_str()) == Some("a")
            && t.get(1).map(|s| s.as_str()) == Some("30340:farm_pubkey:AAAAAAAAAAAAAAAAAAAAAA")
    }));
    assert!(tags.iter().any(|t| {
        t.first().map(|s| s.as_str()) == Some("key") && t.get(1).map(|s| s.as_str()) == Some("sku")
    }));
    assert!(tags.iter().any(|t| {
        t.first().map(|s| s.as_str()) == Some("title")
            && t.get(1).map(|s| s.as_str()) == Some("Widget")
    }));

    let primary_tag = tags
        .iter()
        .find(|t| t.first().map(|s| s.as_str()) == Some("radroots:primary_bin"))
        .expect("primary bin tag");
    assert_eq!(primary_tag.get(1).map(|s| s.as_str()), Some("bin-1"));

    let bin_tag = tags
        .iter()
        .find(|t| t.first().map(|s| s.as_str()) == Some("radroots:bin"))
        .expect("bin tag");
    assert_eq!(bin_tag.get(1).map(|s| s.as_str()), Some("bin-1"));
    assert_eq!(bin_tag.get(2).map(|s| s.as_str()), Some("1000"));
    assert_eq!(bin_tag.get(3).map(|s| s.as_str()), Some("g"));
    assert_eq!(bin_tag.get(4).map(|s| s.as_str()), Some("1"));
    assert_eq!(bin_tag.get(5).map(|s| s.as_str()), Some("kg"));
    assert_eq!(bin_tag.get(6).map(|s| s.as_str()), Some("bag"));

    let price_tag = tags
        .iter()
        .find(|t| t.first().map(|s| s.as_str()) == Some("radroots:price"))
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
            t.first().map(|s| s.as_str()) == Some("price")
                && t.get(1).map(|s| s.as_str()) == Some("10")
        })
        .expect("generic price tag");
    assert_eq!(generic_price_tag.get(2).map(|s| s.as_str()), Some("USD"));

    let discount_tag = tags
        .iter()
        .find(|t| t.first().map(|s| s.as_str()) == Some("radroots:discount"))
        .expect("discount tag");
    assert!(
        discount_tag
            .get(1)
            .map(|s| s.contains("\"scope\":\"bin\""))
            .unwrap_or(false)
    );

    assert!(tags.iter().any(|t| {
        t.first().map(|s| s.as_str()) == Some("location")
            && t.get(1).map(|s| s.as_str()) == Some("Moyobamba")
    }));

    let g_tags: Vec<&Vec<String>> = tags
        .iter()
        .filter(|t| t.first().map(|s| s.as_str()) == Some("g"))
        .collect();
    assert_eq!(g_tags.len(), 1);
    assert_eq!(g_tags[0][1].len(), 5);
    assert!(
        !tags
            .iter()
            .any(|t| t.first().map(|s| s.as_str()) == Some("L"))
    );
    assert!(
        !tags
            .iter()
            .any(|t| t.first().map(|s| s.as_str()) == Some("l"))
    );

    assert!(tags.iter().any(|t| {
        t.first().map(|s| s.as_str()) == Some("image")
            && t.get(1).map(|s| s.as_str()) == Some("http://example.com/widget.jpg")
            && t.get(2).map(|s| s.as_str()) == Some("1200x800")
    }));
}

#[test]
fn operational_listing_tags_full_uses_single_generic_price_for_primary_bin() {
    let mut listing = sample_listing_full("AAAAAAAAAAAAAAAAAAAAAw");
    listing.bins.push(OperationalListingBin {
        bin_id: bin_id("bin-2"),
        quantity: quantity(Decimal::from_str("500").unwrap(), Unit::MassG),
        price_per_canonical_unit: quantity_price(
            money(Decimal::from_str("0.02").unwrap(), Currency::USD),
            quantity(Decimal::from(1u32), Unit::MassG),
        ),
        display_amount: Some(Decimal::from(500u32)),
        display_unit: Some(Unit::MassG),
        display_label: Some("sample".to_string()),
        display_price: Some(money(Decimal::from_str("10").unwrap(), Currency::USD)),
        display_price_unit: Some(Unit::MassG),
    });

    let tags = operational_listing_tags_full(&listing).unwrap();
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
fn operational_listing_tags_full_includes_trade_fields() {
    let mut listing = sample_listing("AAAAAAAAAAAAAAAAAAAAAg");
    let inventory = Decimal::from_str("12.5").unwrap();
    let inventory_value = inventory.to_string();
    listing.inventory_available = Some(inventory);
    listing.availability = Some(OperationalListingAvailability::Window {
        start: Some(1730000000),
        end: Some(1731000000),
    });
    listing.delivery_method = Some(OperationalListingDeliveryMethod::Shipping);

    let tags = operational_listing_tags_full(&listing).unwrap();

    assert!(tags.iter().any(|t| {
        t.first().map(|s| s.as_str()) == Some("inventory")
            && t.get(1).map(|s| s.as_str()) == Some(inventory_value.as_str())
    }));
    assert!(tags.iter().any(|t| {
        t.first().map(|s| s.as_str()) == Some("radroots:availability_start")
            && t.get(1).map(|s| s.as_str()) == Some("1730000000")
    }));
    assert!(tags.iter().any(|t| {
        t.first().map(|s| s.as_str()) == Some("expires_at")
            && t.get(1).map(|s| s.as_str()) == Some("1731000000")
    }));
    assert!(tags.iter().any(|t| {
        t.first().map(|s| s.as_str()) == Some("delivery")
            && t.get(1).map(|s| s.as_str()) == Some("shipping")
    }));
}

#[test]
fn operational_listing_tags_full_includes_status_tag() {
    let mut listing = sample_listing("AAAAAAAAAAAAAAAAAAAAAg");
    listing.availability = Some(OperationalListingAvailability::Status {
        status: OperationalListingStatus::Active,
    });

    let tags = operational_listing_tags_full(&listing).unwrap();

    assert!(tags.iter().any(|t| {
        t.first().map(|s| s.as_str()) == Some("status")
            && t.get(1).map(|s| s.as_str()) == Some("active")
    }));
}

#[test]
fn operational_listing_build_tags_ignores_null_strings() {
    let mut listing = sample_listing_full("AAAAAAAAAAAAAAAAAAAAAg");
    listing.product.summary = Some("null".to_string());
    listing.product.process = Some("null".to_string());
    listing.product.lot = Some("null".to_string());
    listing.product.location = Some("null".to_string());
    listing.product.profile = Some("null".to_string());
    listing.product.year = Some("null".to_string());
    listing.location = Some(OperationalListingPublicLocation {
        primary: "Moyobamba".to_string(),
        city: Some("null".to_string()),
        region: Some("San Martin".to_string()),
        country: Some("null".to_string()),
        geohash: "9q8yy".to_string(),
    });
    listing.images = Some(vec![OperationalListingImage {
        url: "null".to_string(),
        size: None,
    }]);

    let tags = operational_listing_build_tags(&listing).unwrap();
    assert!(
        !tags
            .iter()
            .any(|tag| tag.iter().any(|value| value == "null"))
    );
}

#[test]
fn operational_listing_build_tags_rejects_location_without_public_locality() {
    let mut listing = sample_listing("AAAAAAAAAAAAAAAAAAAAAg");
    listing.location = Some(OperationalListingPublicLocation {
        primary: "Farm stand".to_string(),
        city: Some("null".to_string()),
        region: None,
        country: None,
        geohash: "9q8yy".to_string(),
    });

    assert!(matches!(
        operational_listing_build_tags(&listing),
        Err(EventEncodeError::EmptyRequiredField("location.locality"))
    ));
}

#[test]
fn operational_listing_tags_with_options_cover_location_fallback_paths() {
    let mut geohash_only = sample_listing("AAAAAAAAAAAAAAAAAAAAAg");
    geohash_only.location = Some(OperationalListingPublicLocation {
        primary: "Moyobamba".to_string(),
        city: Some("Moyobamba".to_string()),
        region: None,
        country: None,
        geohash: "6gkzw".to_string(),
    });
    let tags = operational_listing_tags_with_options(
        &geohash_only,
        OperationalListingTagOptions::default(),
    )
    .unwrap();
    assert!(
        tags.iter()
            .any(|tag| tag.first().map(|value| value.as_str()) == Some("g"))
    );
    assert!(
        !tags
            .iter()
            .any(|tag| tag.first().map(|value| value.as_str()) == Some("l"))
    );

    let mut no_coordinates = sample_listing("AAAAAAAAAAAAAAAAAAAAAQ");
    no_coordinates.location = Some(OperationalListingPublicLocation {
        primary: "Moyobamba".to_string(),
        city: Some("Moyobamba".to_string()),
        region: None,
        country: None,
        geohash: "9q8yy".to_string(),
    });
    let tags = operational_listing_tags_with_options(
        &no_coordinates,
        OperationalListingTagOptions::default(),
    )
    .unwrap();
    assert!(
        !tags
            .iter()
            .any(|tag| tag.first().map(|value| value.as_str()) == Some("L"))
    );
    assert!(
        tags.iter()
            .any(|tag| tag.first().map(|value| value.as_str()) == Some("g"))
    );

    let mut without_geohash = sample_listing("AAAAAAAAAAAAAAAAAAAAAw");
    without_geohash.location = Some(OperationalListingPublicLocation {
        primary: "Moyobamba".to_string(),
        city: Some("Moyobamba".to_string()),
        region: None,
        country: None,
        geohash: "9q8yy".to_string(),
    });
    let tags = operational_listing_tags_with_options(
        &without_geohash,
        OperationalListingTagOptions {
            ..OperationalListingTagOptions::default()
        },
    )
    .unwrap();
    assert!(
        tags.iter()
            .any(|tag| tag.first().map(|value| value.as_str()) == Some("g"))
    );
    assert!(
        !tags
            .iter()
            .any(|tag| tag.first().map(|value| value.as_str()) == Some("L"))
    );
}
