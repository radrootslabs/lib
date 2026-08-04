#![cfg(feature = "json")]

use std::{borrow::Cow, collections::BTreeSet, fs, path::Path};

use radroots_event::{
    contract::validate_event_contract_shape, envelope::EventEnvelope, envelope::EventEnvelopeParts,
    listing::operational::OperationalListing,
};
use radroots_event_codec::encode::{
    EventEncodeError,
    operational_listing::{
        operational_listing_build_tags, operational_listing_tags_full, to_wire_parts,
    },
};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Map, Value};

const CONTRACT_ID: &str = "radroots.operational_listing.published.v1";
const WORKSPACE_CONTRACT_MARKER_PATH: &str = "../../contracts/manifest.toml";
const PACKAGED_BUILD_TAGS: &str = include_str!("fixtures/operational_listing_build_tags.v1.json");
const WORKSPACE_BUILD_TAGS_PATH: &str =
    "../../contracts/conformance/vectors/operational_listing/build_tags.v1.json";
const PACKAGED_BUILD_DRAFT: &str = include_str!("fixtures/operational_listing_build_draft.v1.json");
const WORKSPACE_BUILD_DRAFT_PATH: &str =
    "../../contracts/conformance/vectors/operational_listing/build_draft.v1.json";
const PACKAGED_FULL_TAGS: &str = include_str!("fixtures/operational_listing_tags_full.v1.json");
const WORKSPACE_FULL_TAGS_PATH: &str =
    "../../contracts/conformance/vectors/events/operational_listing_tags_full.v1.json";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Suite<T> {
    suite: String,
    contract_version: String,
    vectors: Vec<Vector<T>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Vector<T> {
    id: String,
    kind: String,
    input: ListingInput,
    expected: T,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ListingInput {
    listing: Value,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum TagsExpected {
    Success(TagsSuccess),
    Failure(EncodeFailure),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TagsSuccess {
    tags: Vec<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum DraftExpected {
    Success(DraftSuccess),
    Failure(EncodeFailure),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DraftSuccess {
    wire_parts: WirePartsExpected,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WirePartsExpected {
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EncodeFailure {
    error: EncodeErrorExpected,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EncodeErrorExpected {
    code: String,
    field: String,
}

#[test]
fn checked_in_operational_listing_build_tags_vectors_execute() {
    let suite: Suite<TagsExpected> = parse_suite(
        PACKAGED_BUILD_TAGS,
        WORKSPACE_BUILD_TAGS_PATH,
        "operational_listing",
        &[
            "operational_listing_build_tags_empty_bins_002",
            "operational_listing_build_tags_victoria_carrots_001",
        ],
    );

    for vector in &suite.vectors {
        let listing = typed_listing(&vector.input.listing, &vector.id);
        assert_eq!(
            vector.kind, "operational_listing.build_tags",
            "{}",
            vector.id
        );
        match &vector.expected {
            TagsExpected::Success(expected) => {
                let actual = operational_listing_build_tags(&listing)
                    .unwrap_or_else(|error| panic!("{} failed: {error}", vector.id));
                assert_eq!(actual, expected.tags, "{}", vector.id);
            }
            TagsExpected::Failure(expected) => {
                let error = operational_listing_build_tags(&listing)
                    .expect_err("invalid build-tags vector must fail");
                assert_encode_error(&vector.id, &error, &expected.error);
            }
        }
    }

    let mut misspelled = suite.vectors[0].input.listing.clone();
    misspelled["product"]["summmary"] = Value::String("misspelled optional field".to_string());
    assert_eq!(
        validate_listing_fixture_keys(&misspelled),
        Err("listing.product contains unknown key `summmary`".to_string())
    );
}

#[test]
fn checked_in_operational_listing_build_draft_vectors_execute() {
    let suite: Suite<DraftExpected> = parse_suite(
        PACKAGED_BUILD_DRAFT,
        WORKSPACE_BUILD_DRAFT_PATH,
        "operational_listing",
        &[
            "operational_listing_build_draft_empty_bins_002",
            "operational_listing_build_draft_victoria_carrots_001",
        ],
    );

    for vector in &suite.vectors {
        let listing = typed_listing(&vector.input.listing, &vector.id);
        assert_eq!(
            vector.kind, "operational_listing.build_draft",
            "{}",
            vector.id
        );
        match &vector.expected {
            DraftExpected::Success(expected) => {
                let actual = to_wire_parts(&listing)
                    .unwrap_or_else(|error| panic!("{} failed: {error}", vector.id));
                assert_eq!(actual.kind, expected.wire_parts.kind, "{}", vector.id);
                assert_eq!(actual.content, expected.wire_parts.content, "{}", vector.id);
                assert_eq!(actual.tags, expected.wire_parts.tags, "{}", vector.id);
            }
            DraftExpected::Failure(expected) => {
                let error =
                    to_wire_parts(&listing).expect_err("invalid build-draft vector must fail");
                assert_encode_error(&vector.id, &error, &expected.error);
            }
        }
    }
}

#[test]
fn checked_in_operational_listing_full_event_tag_vectors_execute() {
    let suite: Suite<TagsExpected> = parse_suite(
        PACKAGED_FULL_TAGS,
        WORKSPACE_FULL_TAGS_PATH,
        "events",
        &[
            "operational_listing_tags_full_empty_bins_002",
            "operational_listing_tags_full_victoria_carrots_001",
        ],
    );

    for vector in &suite.vectors {
        let listing = typed_listing(&vector.input.listing, &vector.id);
        assert_eq!(
            vector.kind, "operational_listing_tags_full",
            "{}",
            vector.id
        );
        match &vector.expected {
            TagsExpected::Success(expected) => {
                let actual = operational_listing_tags_full(&listing)
                    .unwrap_or_else(|error| panic!("{} failed: {error}", vector.id));
                assert_eq!(actual, expected.tags, "{}", vector.id);

                let wire = to_wire_parts(&listing)
                    .unwrap_or_else(|error| panic!("{} draft failed: {error}", vector.id));
                assert_eq!(wire.tags, actual, "{}", vector.id);
                let event = EventEnvelope::new(EventEnvelopeParts {
                    id: "b".repeat(64),
                    author: listing.farm.pubkey.clone(),
                    created_at: listing.published_at.unwrap_or_default(),
                    kind: wire.kind,
                    tags: wire.tags,
                    content: wire.content,
                    sig: "c".repeat(128),
                })
                .unwrap_or_else(|error| panic!("{} envelope failed: {error}", vector.id));
                validate_event_contract_shape(&event, CONTRACT_ID)
                    .unwrap_or_else(|error| panic!("{} contract failed: {error:?}", vector.id));
            }
            TagsExpected::Failure(expected) => {
                let error = operational_listing_tags_full(&listing)
                    .expect_err("invalid full-tags vector must fail");
                assert_encode_error(&vector.id, &error, &expected.error);
            }
        }
    }
}

fn parse_suite<T: DeserializeOwned>(
    packaged: &'static str,
    workspace_relative: &str,
    expected_suite: &str,
    expected_ids: &[&str],
) -> Suite<T> {
    let vectors = conformance_vectors(packaged, workspace_relative);
    let suite: Suite<T> = serde_json::from_str(&vectors)
        .unwrap_or_else(|error| panic!("{workspace_relative} must parse: {error}"));
    assert_eq!(suite.suite, expected_suite, "{workspace_relative}");
    assert_eq!(suite.contract_version, "1.0.0", "{workspace_relative}");

    let actual_ids = suite
        .vectors
        .iter()
        .map(|vector| vector.id.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        actual_ids.len(),
        suite.vectors.len(),
        "{workspace_relative} contains duplicate vector ids"
    );
    let expected_ids = expected_ids.iter().copied().collect::<BTreeSet<_>>();
    assert_eq!(
        actual_ids, expected_ids,
        "{workspace_relative} inventory drift"
    );
    suite
}

fn conformance_vectors(packaged: &'static str, workspace_relative: &str) -> Cow<'static, str> {
    let workspace_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(workspace_relative);
    match fs::read_to_string(&workspace_path) {
        Ok(canonical) => {
            assert_eq!(
                canonical,
                packaged,
                "packaged vectors must match {}",
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
            Cow::Borrowed(packaged)
        }
        Err(error) => panic!("failed to read {}: {error}", workspace_path.display()),
    }
}

fn assert_encode_error(id: &str, actual: &EventEncodeError, expected: &EncodeErrorExpected) {
    assert_eq!(actual.code(), expected.code, "{id}");
    match actual {
        EventEncodeError::EmptyRequiredField(field) => {
            assert_eq!(*field, expected.field, "{id}");
        }
        other => panic!("{id} returned unexpected encode error {other:?}"),
    }
}

fn typed_listing(raw: &Value, id: &str) -> OperationalListing {
    validate_listing_fixture_keys(raw)
        .unwrap_or_else(|error| panic!("{id} fixture schema failed: {error}"));
    serde_json::from_value(raw.clone())
        .unwrap_or_else(|error| panic!("{id} listing must deserialize: {error}"))
}

fn validate_listing_fixture_keys(raw: &Value) -> Result<(), String> {
    let listing = fixture_object(raw, "listing")?;
    assert_allowed_keys(
        listing,
        &[
            "d_tag",
            "published_at",
            "farm",
            "product",
            "primary_bin_id",
            "bins",
            "inventory_available",
            "availability",
            "delivery_method",
            "location",
        ],
        "listing",
    )?;

    let farm = required_fixture_object(listing, "farm", "listing")?;
    assert_allowed_keys(farm, &["pubkey", "d_tag"], "listing.farm")?;

    let product = required_fixture_object(listing, "product", "listing")?;
    assert_allowed_keys(
        product,
        &["key", "title", "category", "summary"],
        "listing.product",
    )?;

    let bins = listing
        .get("bins")
        .and_then(Value::as_array)
        .ok_or_else(|| "listing.bins must be an array".to_string())?;
    for (index, raw_bin) in bins.iter().enumerate() {
        let bin_path = format!("listing.bins[{index}]");
        let bin = fixture_object(raw_bin, &bin_path)?;
        assert_allowed_keys(
            bin,
            &["bin_id", "quantity", "price_per_canonical_unit"],
            &bin_path,
        )?;

        let quantity = required_fixture_object(bin, "quantity", &bin_path)?;
        assert_allowed_keys(
            quantity,
            &["amount", "unit"],
            &format!("{bin_path}.quantity"),
        )?;

        let price = required_fixture_object(bin, "price_per_canonical_unit", &bin_path)?;
        let price_path = format!("{bin_path}.price_per_canonical_unit");
        assert_allowed_keys(price, &["amount", "quantity"], &price_path)?;
        let money = required_fixture_object(price, "amount", &price_path)?;
        assert_allowed_keys(
            money,
            &["amount", "currency"],
            &format!("{price_path}.amount"),
        )?;
        let price_quantity = required_fixture_object(price, "quantity", &price_path)?;
        assert_allowed_keys(
            price_quantity,
            &["amount", "unit"],
            &format!("{price_path}.quantity"),
        )?;
    }

    if let Some(raw_availability) = listing.get("availability") {
        let availability = fixture_object(raw_availability, "listing.availability")?;
        assert_allowed_keys(availability, &["kind", "amount"], "listing.availability")?;
        let amount = required_fixture_object(availability, "amount", "listing.availability")?;
        assert_allowed_keys(amount, &["status"], "listing.availability.amount")?;
        let status = required_fixture_object(amount, "status", "listing.availability.amount")?;
        assert_allowed_keys(status, &["kind"], "listing.availability.amount.status")?;
    }

    if let Some(raw_delivery) = listing.get("delivery_method") {
        let delivery = fixture_object(raw_delivery, "listing.delivery_method")?;
        assert_allowed_keys(delivery, &["kind"], "listing.delivery_method")?;
    }

    if let Some(raw_location) = listing.get("location") {
        let location = fixture_object(raw_location, "listing.location")?;
        assert_allowed_keys(
            location,
            &["primary", "city", "region", "country", "geohash"],
            "listing.location",
        )?;
    }

    Ok(())
}

fn fixture_object<'a>(raw: &'a Value, path: &str) -> Result<&'a Map<String, Value>, String> {
    raw.as_object()
        .ok_or_else(|| format!("{path} must be an object"))
}

fn required_fixture_object<'a>(
    parent: &'a Map<String, Value>,
    key: &str,
    parent_path: &str,
) -> Result<&'a Map<String, Value>, String> {
    let path = format!("{parent_path}.{key}");
    parent
        .get(key)
        .ok_or_else(|| format!("{path} is required"))
        .and_then(|raw| fixture_object(raw, &path))
}

fn assert_allowed_keys(
    object: &Map<String, Value>,
    allowed: &[&str],
    path: &str,
) -> Result<(), String> {
    for key in object.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(format!("{path} contains unknown key `{key}`"));
        }
    }
    Ok(())
}
