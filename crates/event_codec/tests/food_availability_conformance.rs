#![cfg(all(feature = "serde_json", feature = "nostr"))]

use std::{borrow::Cow, collections::BTreeSet, fs, path::Path};

use nostr::secp256k1::Message;
use nostr::{Event as NostrEvent, JsonUtil, Keys, SECP256K1};
use radroots_blossom::{BlobDescriptor, BlobUrl, MediaType, Sha256};
use radroots_event::food::availability::{
    FoodAvailabilityDetails, FoodAvailabilityDetailsParts, FoodAvailabilityError,
    FoodAvailabilityImage, FoodAvailabilityStatus, FoodContent, FoodCurrency, FoodIdentifier,
    FoodImageDimensions, FoodPrice, FoodPublishedAt, FoodQuantity, FoodText, FoodUnit,
};
use radroots_event::media::AuthoredImage;
use radroots_event::wire::{Nip01EventWire, compute_canonical_nip01_event_id};
use radroots_event::{
    envelope::EventEnvelope, envelope::EventEnvelopeParts,
    listing::classified::ClassifiedListingPartition,
};
use radroots_event_codec::food_availability::admission::{
    RadrootsFoodAvailabilityAdmissionError, RadrootsFoodAvailabilityAdmissionOutcome,
    verify_and_admit_food_availability_event,
};
use radroots_event_codec::food_availability::authored::{
    RadrootsFoodAvailabilityEncodeError, authored_food_availability_build_tags,
    authored_food_availability_to_wire_parts,
};
use radroots_event_codec::food_availability::inbound::{
    RadrootsFoodAvailabilityImageDiagnostic, RadrootsFoodAvailabilityProjectionError,
    RadrootsFoodAvailabilityProjectionOutcome, RadrootsInboundFoodAvailabilityProjection,
    project_verified_food_availability_event,
};
use radroots_event_codec::food_availability::revision::{
    RadrootsFoodAvailabilityRevisionError, validate_food_availability_revision,
};
use radroots_event_codec::verification::{RadrootsSignatureVerifiedEvent, verify_nip01_event};
use serde::Deserialize;
use serde_json::{Value, json};

const SECRET_KEY_ONE: &str = "0000000000000000000000000000000000000000000000000000000000000001";
const SECRET_KEY_TWO: &str = "0000000000000000000000000000000000000000000000000000000000000002";
const PACKAGED_VECTORS: &str = include_str!("fixtures/food_availability_profile.v1.json");
const WORKSPACE_VECTOR_PATH: &str =
    "../../contracts/conformance/vectors/food_availability/profile.v1.json";
const WORKSPACE_CONTRACT_MARKER_PATH: &str = "../../contracts/manifest.toml";

const VECTOR_IDS: [&str; 40] = [
    "food_authored_unit_g_001",
    "food_authored_unit_kg_002",
    "food_authored_unit_lb_003",
    "food_authored_unit_oz_004",
    "food_authored_unit_each_005",
    "food_authored_unit_dozen_006",
    "food_authored_unit_bunch_007",
    "food_authored_unit_punnet_008",
    "food_authored_unit_bag_009",
    "food_authored_unit_basket_010",
    "food_authored_wire_budget_ascii_max_011",
    "food_authored_wire_budget_escaped_overflow_012",
    "food_authored_future_published_at_013",
    "food_admission_normalizes_decimal_currency_014",
    "food_admission_optional_standard_tags_015",
    "food_admission_excludes_operational_before_validation_016",
    "food_admission_excludes_generic_nip99_017",
    "food_admission_rejects_ambiguous_markers_018",
    "food_admission_rejects_wrong_kind_019",
    "food_admission_rejects_core_tag_shape_020",
    "food_admission_rejects_prohibited_delivery_021",
    "food_admission_rejects_price_frequency_022",
    "food_admission_requires_price_unit_023",
    "food_admission_bounds_raw_decimal_digits_024",
    "food_admission_rejects_malformed_price_unit_025",
    "food_admission_rejects_quantity_unit_mismatch_026",
    "food_admission_rejects_status_027",
    "food_admission_rejects_future_published_at_028",
    "food_admission_preserves_ordered_image_diagnostics_029",
    "food_admission_bounds_image_projection_030",
    "food_admission_rejects_invalid_signature_031",
    "food_revision_accepts_later_created_at_032",
    "food_revision_rejects_invalid_previous_033",
    "food_revision_rejects_invalid_current_034",
    "food_revision_rejects_identifier_coordinate_change_035",
    "food_revision_rejects_author_coordinate_change_036",
    "food_revision_rejects_published_at_change_037",
    "food_revision_rejects_older_created_at_038",
    "food_revision_equal_time_a_current_039",
    "food_revision_equal_time_b_current_040",
];

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Suite {
    suite: String,
    contract_version: String,
    vectors: Vec<Vector>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Vector {
    id: String,
    kind: String,
    input: Value,
    expected: Value,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthoredInput {
    details: DetailsInput,
    created_at: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DetailsInput {
    content: TextRecipe,
    identifier: String,
    title: String,
    summary: String,
    published_at: u64,
    location: String,
    price: PriceInput,
    quantity: Option<QuantityInput>,
    status: String,
    images: Vec<ImageInput>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum TextRecipe {
    Exact(ExactText),
    Repeat(RepeatText),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExactText {
    value: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RepeatText {
    repeat: String,
    count: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PriceInput {
    amount: String,
    currency: String,
    unit: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct QuantityInput {
    amount: String,
    unit: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ImageInput {
    bytes_utf8: String,
    url: String,
    media_type: String,
    uploaded_at: u64,
    dimensions: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EventInput {
    event: Value,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RevisionInput {
    previous: Value,
    current: Value,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEvent {
    id: String,
    pubkey: String,
    created_at: u64,
    kind: u32,
    tags: Vec<Vec<String>>,
    content: String,
    sig: String,
}

#[test]
fn checked_in_food_availability_profile_vectors_execute() {
    let suite = parse_suite();
    for vector in &suite.vectors {
        execute(vector);
    }
}

#[test]
fn strict_revision_rejects_a_direct_envelope_above_the_authored_wire_bound() {
    let suite = parse_suite();
    let vector = suite
        .vectors
        .iter()
        .find(|vector| vector.id == "food_revision_accepts_later_created_at_032")
        .expect("revision fixture");
    let input = revision_input(vector);
    let previous = verified_event(&input.previous, vector.id.as_str());
    let mut current = raw_event(&input.current, vector.id.as_str());
    current.content = core::iter::repeat_n('\0', 64 * 1024).collect();
    let current = directly_signed_verified_event(current, vector.id.as_str());

    let error = validate_food_availability_revision(&previous, &current)
        .expect_err("oversized strict revision must fail");
    match error {
        RadrootsFoodAvailabilityRevisionError::CurrentInvalid(error) => {
            assert_eq!(error.code(), "food_event_wire_too_large");
        }
        error => panic!("unexpected strict revision error: {error:?}"),
    }
}

fn parse_suite() -> Suite {
    let vectors = conformance_vectors();
    let suite: Suite = serde_json::from_str(&vectors)
        .unwrap_or_else(|error| panic!("FoodAvailability vectors must parse: {error}"));
    assert_eq!(suite.suite, "food_availability_profile");
    assert_eq!(suite.contract_version, "1.0.0");
    assert_eq!(suite.vectors.len(), VECTOR_IDS.len());

    let actual = suite
        .vectors
        .iter()
        .map(|vector| vector.id.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(actual.len(), suite.vectors.len(), "duplicate vector ids");
    let expected = VECTOR_IDS.iter().copied().collect::<BTreeSet<_>>();
    assert_eq!(actual, expected, "FoodAvailability vector inventory drift");
    suite
}

fn conformance_vectors() -> Cow<'static, str> {
    let workspace_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(WORKSPACE_VECTOR_PATH);
    match fs::read_to_string(&workspace_path) {
        Ok(canonical) => {
            assert_eq!(
                canonical,
                PACKAGED_VECTORS,
                "packaged FoodAvailability vectors must match {}",
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
            Cow::Borrowed(PACKAGED_VECTORS)
        }
        Err(error) => panic!("failed to read {}: {error}", workspace_path.display()),
    }
}

fn execute(vector: &Vector) {
    match vector.kind.as_str() {
        "food_availability.build_authored_draft.valid" => authored_valid(
            vector,
            vector.expected["wire_parts"]
                .get("content_length")
                .is_some(),
        ),
        "food_availability.build_authored_draft.invalid" => authored_invalid(vector),
        "food_availability.project_verified_event.valid" => projection_valid(vector),
        "food_availability.project_verified_event.invalid" => projection_invalid(vector),
        "food_availability.verify_and_admit_event.valid" => verify_and_admit_valid(vector),
        "food_availability.verify_and_admit_event.invalid" => verify_and_admit_invalid(vector),
        "food_availability.validate_revision.valid" => revision_valid(vector),
        "food_availability.validate_revision.invalid" => revision_invalid(vector),
        kind => panic!("{} uses unsupported vector kind {kind}", vector.id),
    }
}

fn authored_valid(vector: &Vector, summarized_content: bool) {
    let input = authored_input(vector);
    let (details, content) = strict_details(&input.details, &vector.id);
    let tags = authored_food_availability_build_tags(&details, input.created_at)
        .unwrap_or_else(|error| panic!("{} tags failed: {error}", vector.id));
    let wire = authored_food_availability_to_wire_parts(&details, input.created_at)
        .unwrap_or_else(|error| panic!("{} wire failed: {error}", vector.id));
    assert_eq!(wire.tags, tags, "{}", vector.id);
    assert_eq!(wire.content, content, "{}", vector.id);

    let actual = if summarized_content {
        json!({
            "wire_parts": {
                "kind": wire.kind,
                "content_length": wire.content.len(),
                "tags": wire.tags,
            }
        })
    } else {
        json!({
            "wire_parts": {
                "kind": wire.kind,
                "content": wire.content,
                "tags": wire.tags,
            }
        })
    };
    assert_eq!(actual, vector.expected, "{}", vector.id);
}

fn authored_invalid(vector: &Vector) {
    let input = authored_input(vector);
    let (details, _) = strict_details(&input.details, &vector.id);
    let error = authored_food_availability_to_wire_parts(&details, input.created_at)
        .expect_err("invalid authored vector must fail");
    assert_eq!(
        authored_error_value(&error),
        vector.expected["error"],
        "{}",
        vector.id
    );
}

fn authored_input(vector: &Vector) -> AuthoredInput {
    serde_json::from_value(vector.input.clone())
        .unwrap_or_else(|error| panic!("{} authored input is invalid: {error}", vector.id))
}

fn strict_details(input: &DetailsInput, vector_id: &str) -> (FoodAvailabilityDetails, String) {
    let content = input.content.materialize(vector_id);
    let price_unit = FoodUnit::parse(&input.price.unit)
        .unwrap_or_else(|error| panic!("{vector_id} price unit failed: {error}"));
    let price = FoodPrice::new(
        input.price.amount.clone(),
        FoodCurrency::parse(&input.price.currency)
            .unwrap_or_else(|error| panic!("{vector_id} currency failed: {error}")),
        price_unit,
    )
    .unwrap_or_else(|error| panic!("{vector_id} price failed: {error}"));
    let quantity = input.quantity.as_ref().map(|quantity| {
        let unit = FoodUnit::parse(&quantity.unit)
            .unwrap_or_else(|error| panic!("{vector_id} quantity unit failed: {error}"));
        FoodQuantity::new(quantity.amount.clone(), unit)
            .unwrap_or_else(|error| panic!("{vector_id} quantity failed: {error}"))
    });
    let images = input
        .images
        .iter()
        .map(|image| strict_image(image, vector_id))
        .collect();
    let details = FoodAvailabilityDetails::new(FoodAvailabilityDetailsParts {
        content: FoodContent::new(content.clone())
            .unwrap_or_else(|error| panic!("{vector_id} content failed: {error}")),
        identifier: FoodIdentifier::parse(&input.identifier)
            .unwrap_or_else(|error| panic!("{vector_id} identifier failed: {error}")),
        title: FoodText::new(input.title.clone())
            .unwrap_or_else(|error| panic!("{vector_id} title failed: {error}")),
        summary: FoodText::new(input.summary.clone())
            .unwrap_or_else(|error| panic!("{vector_id} summary failed: {error}")),
        published_at: FoodPublishedAt::new(input.published_at)
            .unwrap_or_else(|error| panic!("{vector_id} published_at failed: {error}")),
        location: FoodText::new(input.location.clone())
            .unwrap_or_else(|error| panic!("{vector_id} location failed: {error}")),
        price,
        quantity,
        status: FoodAvailabilityStatus::parse(&input.status)
            .unwrap_or_else(|error| panic!("{vector_id} status failed: {error}")),
        images,
    })
    .unwrap_or_else(|error| panic!("{vector_id} details failed: {error}"));
    (details, content)
}

fn strict_image(input: &ImageInput, vector_id: &str) -> FoodAvailabilityImage {
    let bytes = input.bytes_utf8.as_bytes();
    let hash = Sha256::digest(bytes);
    let media_type = MediaType::parse(&input.media_type)
        .unwrap_or_else(|error| panic!("{vector_id} image media type failed: {error}"));
    let verified = BlobDescriptor::new(
        BlobUrl::parse(&input.url)
            .unwrap_or_else(|error| panic!("{vector_id} image URL failed: {error}")),
        hash,
        bytes.len() as u64,
        media_type.clone(),
        input.uploaded_at,
    )
    .unwrap_or_else(|error| panic!("{vector_id} image descriptor failed: {error}"))
    .approve_reference()
    .unwrap_or_else(|error| panic!("{vector_id} image reference failed: {error}"))
    .verify_bytes(bytes, &media_type)
    .unwrap_or_else(|error| panic!("{vector_id} image bytes failed: {error}"));
    let image = AuthoredImage::try_from_verified_descriptor(verified)
        .unwrap_or_else(|error| panic!("{vector_id} authored image failed: {error}"));
    let dimensions = FoodImageDimensions::parse(&input.dimensions)
        .unwrap_or_else(|error| panic!("{vector_id} image dimensions failed: {error}"));
    FoodAvailabilityImage::new(image, dimensions)
}

impl TextRecipe {
    fn materialize(&self, vector_id: &str) -> String {
        match self {
            Self::Exact(value) => value.value.clone(),
            Self::Repeat(value) => {
                assert_eq!(
                    value.repeat.chars().count(),
                    1,
                    "{vector_id} repeat value must be one character"
                );
                value.repeat.repeat(value.count)
            }
        }
    }
}

fn authored_error_value(error: &RadrootsFoodAvailabilityEncodeError) -> Value {
    match error {
        RadrootsFoodAvailabilityEncodeError::Domain(FoodAvailabilityError::PublishedAtFuture {
            published_at,
            created_at,
        }) => json!({
            "code": error.code(),
            "message": error.to_string(),
            "published_at": published_at,
            "created_at": created_at,
        }),
        RadrootsFoodAvailabilityEncodeError::EventWireTooLarge { max, actual } => json!({
            "code": error.code(),
            "message": error.to_string(),
            "max": max,
            "actual": actual,
        }),
        _ => json!({
            "code": error.code(),
            "message": error.to_string(),
        }),
    }
}

fn projection_valid(vector: &Vector) {
    let input = event_input(vector);
    let verified = verified_event(&input.event, &vector.id);
    let event_id = verified.event().id_hex().to_string();
    let outcome = project_verified_food_availability_event(&verified)
        .unwrap_or_else(|error| panic!("{} projection failed: {error}", vector.id));
    let actual = match outcome {
        RadrootsFoodAvailabilityProjectionOutcome::Focused(projection)
            if vector.expected.get("projection").is_some() =>
        {
            json!({
                "outcome": "focused",
                "event_id": event_id,
                "projection": projection_value(&projection),
            })
        }
        RadrootsFoodAvailabilityProjectionOutcome::Focused(projection) => json!({
            "outcome": "focused",
            "event_id": event_id,
            "image_count": projection.images().len(),
            "diagnostics": diagnostic_codes(projection.diagnostics()),
            "first_raw_tag": projection.images().first().expect("first bounded image").raw_tag(),
            "last_raw_tag": projection.images().last().expect("last bounded image").raw_tag(),
        }),
        RadrootsFoodAvailabilityProjectionOutcome::Excluded(partition) => json!({
            "outcome": "excluded",
            "event_id": event_id,
            "partition": partition_name(partition),
        }),
        _ => panic!("{} returned an unsupported projection outcome", vector.id),
    };
    assert_eq!(actual, vector.expected, "{}", vector.id);
}

fn projection_invalid(vector: &Vector) {
    let input = event_input(vector);
    let verified = verified_event(&input.event, &vector.id);
    let error = project_verified_food_availability_event(&verified)
        .expect_err("invalid focused FoodAvailability projection vector must fail");
    assert_eq!(
        projection_error_value(&error),
        vector.expected["error"],
        "{}",
        vector.id
    );
}

fn verify_and_admit_valid(vector: &Vector) {
    let input = event_input(vector);
    let verified = verified_event(&input.event, &vector.id);
    let outcome = verify_and_admit_food_availability_event(verified.into_event())
        .unwrap_or_else(|error| panic!("{} admission failed: {error}", vector.id));
    let admitted = match outcome {
        RadrootsFoodAvailabilityAdmissionOutcome::Admitted(admitted) => *admitted,
        RadrootsFoodAvailabilityAdmissionOutcome::Excluded(excluded) => {
            panic!(
                "{} was unexpectedly excluded as {:?}",
                vector.id,
                excluded.partition()
            )
        }
        _ => panic!("{} returned an unsupported admission outcome", vector.id),
    };
    let actual = json!({
        "outcome": "admitted",
        "event_id": admitted.event().id_hex(),
        "projection": projection_value(admitted.projection()),
    });
    assert_eq!(actual, vector.expected, "{}", vector.id);
}

fn verify_and_admit_invalid(vector: &Vector) {
    let input = event_input(vector);
    let envelope = canonical_envelope(&input.event, &vector.id);
    let error = verify_and_admit_food_availability_event(envelope)
        .expect_err("invalid signature vector must fail verification");
    assert_eq!(
        admission_error_value(&error),
        vector.expected["error"],
        "{}",
        vector.id
    );
}

fn event_input(vector: &Vector) -> EventInput {
    serde_json::from_value(vector.input.clone())
        .unwrap_or_else(|error| panic!("{} event input is invalid: {error}", vector.id))
}

fn admission_error_value(error: &RadrootsFoodAvailabilityAdmissionError) -> Value {
    json!({
        "code": error.code(),
        "message": error.to_string(),
    })
}

fn projection_error_value(error: &RadrootsFoodAvailabilityProjectionError) -> Value {
    json!({
        "code": error.code(),
        "message": error.to_string(),
    })
}

fn projection_value(projection: &RadrootsInboundFoodAvailabilityProjection) -> Value {
    let quantity = projection.quantity().map(|quantity| {
        json!({
            "amount": quantity.amount(),
            "unit": quantity.unit().as_str(),
        })
    });
    let images = projection
        .images()
        .iter()
        .map(|image| {
            let dimensions = image.dimensions().map(|dimensions| {
                json!({
                    "width": dimensions.width(),
                    "height": dimensions.height(),
                })
            });
            json!({
                "raw_tag": image.raw_tag(),
                "url": image.url(),
                "dimensions": dimensions,
                "diagnostics": diagnostic_codes(image.diagnostics()),
                "qualifies": image.qualifies(),
            })
        })
        .collect::<Vec<_>>();
    json!({
        "content": projection.content().as_str(),
        "identifier": projection.identifier().as_str(),
        "title": projection.title().as_str(),
        "summary": projection.summary().as_str(),
        "published_at": projection.published_at().as_u64(),
        "location": projection.location().as_str(),
        "price": {
            "amount": projection.price().amount(),
            "currency": projection.price().currency().as_str(),
            "unit": projection.price().unit().as_str(),
        },
        "quantity": quantity,
        "status": projection.status().as_str(),
        "images": images,
        "diagnostics": diagnostic_codes(projection.diagnostics()),
    })
}

fn diagnostic_codes(diagnostics: &[RadrootsFoodAvailabilityImageDiagnostic]) -> Vec<&'static str> {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code())
        .collect()
}

fn partition_name(partition: ClassifiedListingPartition) -> &'static str {
    match partition {
        ClassifiedListingPartition::FocusedFoodAvailability => "focused_food_availability",
        ClassifiedListingPartition::OperationalListing => "operational_listing",
        ClassifiedListingPartition::GenericNip99 => "generic_nip99",
        ClassifiedListingPartition::Ambiguous => "ambiguous",
    }
}

fn revision_valid(vector: &Vector) {
    let input = revision_input(vector);
    let previous = verified_event(&input.previous, &vector.id);
    let current = verified_event(&input.current, &vector.id);
    validate_food_availability_revision(&previous, &current)
        .unwrap_or_else(|error| panic!("{} revision failed: {error}", vector.id));
    let actual = json!({
        "result": "accepted",
        "current_event_id": current.event().id_hex(),
    });
    assert_eq!(actual, vector.expected, "{}", vector.id);
}

fn revision_invalid(vector: &Vector) {
    let input = revision_input(vector);
    let previous = verified_event(&input.previous, &vector.id);
    let current = verified_event(&input.current, &vector.id);
    let error = validate_food_availability_revision(&previous, &current)
        .expect_err("invalid revision vector must fail");
    assert_eq!(
        revision_error_value(&error),
        vector.expected["error"],
        "{}",
        vector.id
    );
}

fn revision_input(vector: &Vector) -> RevisionInput {
    serde_json::from_value(vector.input.clone())
        .unwrap_or_else(|error| panic!("{} revision input is invalid: {error}", vector.id))
}

fn revision_error_value(error: &RadrootsFoodAvailabilityRevisionError) -> Value {
    json!({
        "code": error.code(),
        "message": error.to_string(),
    })
}

fn verified_event(value: &Value, vector_id: &str) -> RadrootsSignatureVerifiedEvent {
    let raw = raw_event(value, vector_id);
    let raw_json = serde_json::to_string(value).expect("raw event must serialize");
    let event = NostrEvent::from_json(&raw_json)
        .unwrap_or_else(|error| panic!("{vector_id} is not a NIP-01 event: {error}"));
    assert_eq!(event.id.to_string(), raw.id, "{vector_id}");
    assert_eq!(event.sig.to_string(), raw.sig, "{vector_id}");
    assert_eq!(event.pubkey.to_string(), raw.pubkey, "{vector_id}");
    assert_eq!(event.created_at.as_secs(), raw.created_at, "{vector_id}");
    assert_eq!(u32::from(event.kind.as_u16()), raw.kind, "{vector_id}");
    assert_eq!(
        event
            .tags
            .iter()
            .map(|tag| tag.as_slice().to_vec())
            .collect::<Vec<_>>(),
        raw.tags,
        "{vector_id}"
    );
    assert_eq!(event.content, raw.content, "{vector_id}");
    event
        .verify()
        .unwrap_or_else(|error| panic!("{vector_id} NIP-01 verification failed: {error}"));

    let keys = keys_for_pubkey(&raw.pubkey, vector_id);
    let message = Message::from_digest(event.id.to_bytes());
    let deterministic = SECP256K1.sign_schnorr_no_aux_rand(&message, keys.key_pair(SECP256K1));
    assert_eq!(
        event.sig, deterministic,
        "{vector_id} deterministic signature drift"
    );

    verify_nip01_event(canonical_envelope(value, vector_id))
        .unwrap_or_else(|error| panic!("{vector_id} Radroots verification failed: {error}"))
}

fn canonical_envelope(value: &Value, vector_id: &str) -> EventEnvelope {
    let raw_json = serde_json::to_string(value).expect("raw event must serialize");
    let wire = Nip01EventWire::parse_json(&raw_json)
        .unwrap_or_else(|error| panic!("{vector_id} wire parsing failed: {error}"));
    wire.verify_id()
        .unwrap_or_else(|error| panic!("{vector_id} event id failed: {error}"));
    wire.into_envelope()
        .unwrap_or_else(|error| panic!("{vector_id} envelope conversion failed: {error}"))
}

fn raw_event(value: &Value, vector_id: &str) -> RawEvent {
    serde_json::from_value(value.clone())
        .unwrap_or_else(|error| panic!("{vector_id} raw event is invalid: {error}"))
}

fn directly_signed_verified_event(
    mut raw: RawEvent,
    vector_id: &str,
) -> RadrootsSignatureVerifiedEvent {
    let id = compute_canonical_nip01_event_id(
        &raw.pubkey,
        raw.created_at,
        raw.kind,
        &raw.tags,
        &raw.content,
    )
    .unwrap_or_else(|error| panic!("{vector_id} direct event id failed: {error}"));
    let nostr_id = nostr::EventId::from_hex(&id.to_hex())
        .unwrap_or_else(|error| panic!("{vector_id} direct event id conversion failed: {error}"));
    let keys = keys_for_pubkey(&raw.pubkey, vector_id);
    let message = Message::from_digest(nostr_id.to_bytes());
    let signature = SECP256K1.sign_schnorr_no_aux_rand(&message, keys.key_pair(SECP256K1));
    raw.id = id.into_string();
    raw.sig = signature.to_string();

    let event = EventEnvelope::new(EventEnvelopeParts {
        id: raw.id,
        author: raw.pubkey,
        created_at: raw.created_at,
        kind: raw.kind,
        tags: raw.tags,
        content: raw.content,
        sig: raw.sig,
    })
    .unwrap_or_else(|error| panic!("{vector_id} direct envelope failed: {error}"));
    verify_nip01_event(event)
        .unwrap_or_else(|error| panic!("{vector_id} direct verification failed: {error}"))
}

fn keys_for_pubkey(pubkey: &str, vector_id: &str) -> Keys {
    for secret in [SECRET_KEY_ONE, SECRET_KEY_TWO] {
        let keys = Keys::parse(secret).expect("fixed secret key must parse");
        if keys.public_key().to_string() == pubkey {
            return keys;
        }
    }
    panic!("{vector_id} uses an ungoverned fixture pubkey {pubkey}")
}
