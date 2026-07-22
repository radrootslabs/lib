#![cfg(feature = "serde_json")]

use radroots_blossom::{
    RadrootsBlossomBlobDescriptor, RadrootsBlossomBlobUrl, RadrootsBlossomByteVerifiedDescriptor,
    RadrootsBlossomMediaType, RadrootsBlossomSha256,
};
use radroots_event::{
    RadrootsAuthoredImage,
    calendar::{
        RadrootsAuthoredCalendarDateEvent, RadrootsAuthoredCalendarTimeEvent, RadrootsCalendarDate,
    },
    food_availability::{
        RadrootsFoodAvailabilityDetails, RadrootsFoodAvailabilityDetailsParts,
        RadrootsFoodAvailabilityImage, RadrootsFoodAvailabilityStatus, RadrootsFoodContent,
        RadrootsFoodCurrency, RadrootsFoodIdentifier, RadrootsFoodImageDimensions,
        RadrootsFoodPrice, RadrootsFoodPublishedAt, RadrootsFoodQuantity, RadrootsFoodText,
        RadrootsFoodUnit,
    },
    post::{
        RadrootsAuthoredAsk, RadrootsAuthoredPhotoUpdate, RadrootsAuthoredPostImage,
        RadrootsAuthoredUpdate, RadrootsPostImageDimensions,
    },
    profile::{RadrootsAuthoredProfile, RadrootsNip05Identifier},
    wire::compute_canonical_nip01_event_id,
};
use radroots_event_codec::wire::publication::{
    RADROOTS_PHASE1_PUBLICATION_ARTIFACT_MAX_BYTES, RADROOTS_PHASE1_PUBLICATION_MEDIA_MAX_COUNT,
    RadrootsPhase1PublicationArtifact, RadrootsPhase1PublicationArtifactError,
    RadrootsPhase1PublicationEventVariant, RadrootsPhase1PublicationSemanticVariant,
    validate_phase1_publication_artifact,
};
use serde::Deserialize;
use serde_json::Value;

const AUTHOR: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const CREATED_AT: u64 = 1_784_347_200;

const PUBLICATION_ARTIFACT_VECTOR: &str =
    include_str!("fixtures/phase1_publication_artifact.v1.json");

#[derive(Deserialize)]
struct VectorSuite {
    suite: String,
    contract_version: String,
    vectors: Vec<VectorCase>,
}

#[derive(Deserialize)]
struct VectorCase {
    id: String,
    kind: String,
    input: VectorInput,
    expected: VectorExpected,
}

#[derive(Deserialize)]
struct VectorInput {
    fixture: String,
    mutation: Option<String>,
}

#[derive(Deserialize)]
struct VectorExpected {
    semantic_variant: Option<String>,
    operation_id: Option<String>,
    contract_id: Option<String>,
    event_kind: Option<u32>,
    media_count: Option<usize>,
    expected_event_id: Option<String>,
    artifact_digest: Option<String>,
    canonical_json_bytes: Option<usize>,
    canonical_json_sha256: Option<String>,
    canonical_json: Option<String>,
    error: Option<String>,
}

#[test]
fn publication_artifact_conformance_vector_executes_every_case() {
    let suite: VectorSuite = serde_json::from_str(PUBLICATION_ARTIFACT_VECTOR).unwrap();
    assert_eq!(suite.suite, "phase1_publication_artifact");
    assert_eq!(suite.contract_version, "1.0.0");
    assert_eq!(suite.vectors.len(), 41);
    let artifacts = all_artifacts();

    for case in suite.vectors {
        let artifact = artifact_fixture(&artifacts, &case.input.fixture);
        match case.kind.as_str() {
            kind if kind.starts_with("publication_artifact.build_") && kind.ends_with(".valid") => {
                assert_eq!(case.input.mutation, None, "{}", case.id);
                assert_eq!(
                    artifact.semantic_variant().as_str(),
                    case.expected.semantic_variant.as_deref().unwrap(),
                    "{}",
                    case.id
                );
                assert_eq!(
                    artifact.authored_operation_id(),
                    case.expected.operation_id.as_deref().unwrap(),
                    "{}",
                    case.id
                );
                assert_eq!(
                    artifact.event_contract_id(),
                    case.expected.contract_id.as_deref().unwrap(),
                    "{}",
                    case.id
                );
                assert_eq!(
                    artifact.draft().kind(),
                    case.expected.event_kind.unwrap(),
                    "{}",
                    case.id
                );
                assert_eq!(
                    artifact.media_references().len(),
                    case.expected.media_count.unwrap(),
                    "{}",
                    case.id
                );
                assert_eq!(
                    artifact.expected_event_id().as_str(),
                    case.expected.expected_event_id.as_deref().unwrap(),
                    "{}",
                    case.id
                );
                assert_eq!(
                    artifact.artifact_digest().to_string(),
                    case.expected.artifact_digest.unwrap(),
                    "{}",
                    case.id
                );
                assert_eq!(
                    artifact.to_canonical_json().len(),
                    case.expected.canonical_json_bytes.unwrap(),
                    "{}",
                    case.id
                );
                assert_eq!(
                    RadrootsBlossomSha256::digest(&artifact.to_canonical_json()).to_hex(),
                    case.expected.canonical_json_sha256.unwrap(),
                    "{}",
                    case.id
                );
                assert_eq!(
                    artifact.to_canonical_json(),
                    case.expected.canonical_json.unwrap().as_bytes(),
                    "{}",
                    case.id
                );
                assert_eq!(
                    RadrootsPhase1PublicationArtifact::from_canonical_json(
                        &artifact.to_canonical_json()
                    )
                    .unwrap(),
                    *artifact,
                    "{}",
                    case.id
                );
            }
            "publication_artifact.to_canonical_json.valid" => {
                assert_eq!(case.input.mutation, None, "{}", case.id);
                let bytes = artifact.to_canonical_json();
                assert_eq!(
                    bytes.len(),
                    case.expected.canonical_json_bytes.unwrap(),
                    "{}",
                    case.id
                );
                assert_eq!(
                    RadrootsBlossomSha256::digest(&bytes).to_hex(),
                    case.expected.canonical_json_sha256.unwrap(),
                    "{}",
                    case.id
                );
            }
            "publication_artifact.from_canonical_json.valid" => {
                assert_eq!(case.input.mutation, None, "{}", case.id);
                let bytes = artifact.to_canonical_json();
                let reloaded =
                    RadrootsPhase1PublicationArtifact::from_canonical_json(&bytes).unwrap();
                assert_eq!(
                    reloaded.semantic_variant().as_str(),
                    case.expected.semantic_variant.as_deref().unwrap(),
                    "{}",
                    case.id
                );
                assert_eq!(
                    RadrootsBlossomSha256::digest(&reloaded.to_canonical_json()).to_hex(),
                    case.expected.canonical_json_sha256.unwrap(),
                    "{}",
                    case.id
                );
                assert_eq!(reloaded, *artifact, "{}", case.id);
            }
            "publication_artifact.from_canonical_json.invalid" => {
                if case.input.mutation.as_deref() == Some("all_cross_variants") {
                    assert_every_cross_variant_is_rejected(
                        &artifacts,
                        case.expected.error.as_deref().unwrap(),
                        &case.id,
                    );
                    continue;
                }
                let bytes = mutate_artifact(
                    &artifact.to_canonical_json(),
                    case.input.mutation.as_deref().unwrap(),
                );
                assert_eq!(
                    RadrootsPhase1PublicationArtifact::from_canonical_json(&bytes)
                        .unwrap_err()
                        .code(),
                    case.expected.error.as_deref().unwrap(),
                    "{}",
                    case.id
                );
            }
            other => panic!("{} has unknown case kind {other}", case.id),
        }
    }
}

#[test]
fn publication_artifact_round_trips_every_closed_variant() {
    let artifacts = all_artifacts();
    let expected = [
        (
            RadrootsPhase1PublicationSemanticVariant::Profile,
            "profile.build_authored_draft",
            "radroots.profile.metadata.v1",
            0,
            2,
        ),
        (
            RadrootsPhase1PublicationSemanticVariant::Update,
            "social.update.build_authored_draft",
            "radroots.social.update.v1",
            1,
            0,
        ),
        (
            RadrootsPhase1PublicationSemanticVariant::PhotoUpdate,
            "social.photo_update.build_authored_draft",
            "radroots.social.photo_update.v1",
            1,
            2,
        ),
        (
            RadrootsPhase1PublicationSemanticVariant::Ask,
            "social.ask.build_authored_draft",
            "radroots.social.ask.v1",
            1,
            2,
        ),
        (
            RadrootsPhase1PublicationSemanticVariant::Event(
                RadrootsPhase1PublicationEventVariant::Date,
            ),
            "social.calendar_date_event.build_authored_draft",
            "radroots.calendar.date_event.v1",
            31_922,
            1,
        ),
        (
            RadrootsPhase1PublicationSemanticVariant::Event(
                RadrootsPhase1PublicationEventVariant::Time,
            ),
            "social.calendar_time_event.build_authored_draft",
            "radroots.calendar.time_event.v1",
            31_923,
            1,
        ),
        (
            RadrootsPhase1PublicationSemanticVariant::FoodAvailability,
            "food_availability.build_authored_draft",
            "radroots.food.availability.v1",
            30_402,
            1,
        ),
    ];

    for (artifact, (variant, operation, contract, kind, media_count)) in
        artifacts.iter().zip(expected)
    {
        assert_eq!(artifact.semantic_variant(), variant);
        assert_eq!(artifact.authored_operation_id(), operation);
        assert_eq!(artifact.event_contract_id(), contract);
        assert_eq!(artifact.expected_author().as_str(), AUTHOR);
        assert_eq!(artifact.draft().kind(), kind);
        assert_eq!(artifact.media_references().len(), media_count);
        let canonical_json = artifact.to_canonical_json();
        assert!(canonical_json.len() <= RADROOTS_PHASE1_PUBLICATION_ARTIFACT_MAX_BYTES);
        let reloaded =
            RadrootsPhase1PublicationArtifact::from_canonical_json(&canonical_json).unwrap();
        assert_eq!(&reloaded, artifact);
        assert_eq!(reloaded.to_canonical_json(), canonical_json);
        validate_phase1_publication_artifact(artifact).unwrap();
    }
}

#[test]
fn publication_artifact_inventory_uses_full_urls_and_includes_ask_fallbacks() {
    let ask = &all_artifacts()[3];
    let urls = ask
        .media_references()
        .iter()
        .map(|reference| reference.url().as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        urls,
        vec![
            "https://backup.example/51bcf96cda2475475246e3a994aabfee7ff9d7694ba0db9bdc74408632827ad0.webp",
            "https://media.example/51bcf96cda2475475246e3a994aabfee7ff9d7694ba0db9bdc74408632827ad0.webp",
        ]
    );
    assert_eq!(
        ask.media_references()[0].sha256(),
        ask.media_references()[1].sha256()
    );
}

#[test]
fn publication_artifact_accepts_text_only_ask() {
    let ask = RadrootsAuthoredAsk::new("When will the carrots be ready?", Vec::new()).unwrap();
    let artifact = RadrootsPhase1PublicationArtifact::from_ask(&ask, CREATED_AT, AUTHOR).unwrap();
    assert!(artifact.media_references().is_empty());
    assert_eq!(
        artifact.semantic_variant(),
        RadrootsPhase1PublicationSemanticVariant::Ask
    );
    assert_eq!(
        RadrootsPhase1PublicationArtifact::from_canonical_json(&artifact.to_canonical_json())
            .unwrap(),
        artifact
    );
}

#[test]
fn publication_artifact_reload_rejects_cross_variant_and_every_envelope_tamper() {
    let artifact = &all_artifacts()[0];
    let canonical = artifact.to_canonical_json();

    let mut leading_space = vec![b' '];
    leading_space.extend_from_slice(&canonical);
    assert_error(
        &leading_space,
        RadrootsPhase1PublicationArtifactError::NonCanonicalJson,
    );

    for (field, replacement, expected) in [
        (
            "schema_version",
            Value::from(2),
            RadrootsPhase1PublicationArtifactError::UnsupportedSchemaVersion {
                expected: 1,
                actual: 2,
            },
        ),
        (
            "semantic_variant",
            Value::from("update"),
            RadrootsPhase1PublicationArtifactError::AuthoredOperationMismatch,
        ),
        (
            "authored_operation_id",
            Value::from("social.update.build_authored_draft"),
            RadrootsPhase1PublicationArtifactError::AuthoredOperationMismatch,
        ),
        (
            "event_contract_id",
            Value::from("radroots.social.update.v1"),
            RadrootsPhase1PublicationArtifactError::EventContractMismatch,
        ),
        (
            "expected_author",
            Value::from("not-a-key"),
            RadrootsPhase1PublicationArtifactError::InvalidExpectedAuthor,
        ),
        (
            "artifact_digest",
            Value::from("00".repeat(32)),
            RadrootsPhase1PublicationArtifactError::DigestMismatch,
        ),
    ] {
        let mut value: Value = serde_json::from_slice(&canonical).unwrap();
        value[field] = replacement;
        assert_error(&serde_json::to_vec(&value).unwrap(), expected);
    }

    let mut value: Value = serde_json::from_slice(&canonical).unwrap();
    value["draft"]["content"] = Value::from("changed");
    assert_error(
        &serde_json::to_vec(&value).unwrap(),
        RadrootsPhase1PublicationArtifactError::ExpectedEventIdMismatch,
    );

    let mut value: Value = serde_json::from_slice(&canonical).unwrap();
    value["expected_event_id"] = Value::from("00".repeat(32));
    assert_error(
        &serde_json::to_vec(&value).unwrap(),
        RadrootsPhase1PublicationArtifactError::ExpectedEventIdMismatch,
    );

    let mut value: Value = serde_json::from_slice(&canonical).unwrap();
    value["unknown"] = Value::Bool(true);
    assert_error(
        &serde_json::to_vec(&value).unwrap(),
        RadrootsPhase1PublicationArtifactError::InvalidJson,
    );
}

#[test]
fn publication_artifact_envelope_uses_the_exact_contract_field_order() {
    let canonical = String::from_utf8(all_artifacts()[0].to_canonical_json()).unwrap();
    let fields = [
        "\"schema_version\"",
        "\"semantic_variant\"",
        "\"authored_operation_id\"",
        "\"event_contract_id\"",
        "\"expected_author\"",
        "\"draft\"",
        "\"expected_event_id\"",
        "\"media_references\"",
        "\"artifact_digest\"",
    ];
    let positions = fields.map(|field| {
        canonical
            .find(field)
            .unwrap_or_else(|| panic!("missing {field}"))
    });
    assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));

    let draft_start = canonical.find("\"draft\":{").unwrap();
    let draft_end = canonical[draft_start..]
        .find("},\"expected_event_id\"")
        .map(|offset| draft_start + offset)
        .unwrap();
    let draft = &canonical[draft_start..draft_end];
    let draft_positions = ["\"created_at\"", "\"kind\"", "\"tags\"", "\"content\""].map(|field| {
        draft
            .find(field)
            .unwrap_or_else(|| panic!("missing {field}"))
    });
    assert!(draft_positions.windows(2).all(|pair| pair[0] < pair[1]));
    let media = &canonical[positions[7]..positions[8]];
    let media_positions = ["\"url\"", "\"sha256\"", "\"size\"", "\"media_type\""].map(|field| {
        media
            .find(field)
            .unwrap_or_else(|| panic!("missing {field}"))
    });
    assert!(media_positions.windows(2).all(|pair| pair[0] < pair[1]));
    let value: Value = serde_json::from_str(&canonical).unwrap();
    assert!(value["expected_event_id"].is_string());
}

#[test]
fn publication_artifact_reload_rejects_media_order_and_commitment_tamper() {
    let artifact = &all_artifacts()[0];
    let mut value: Value = serde_json::from_slice(&artifact.to_canonical_json()).unwrap();
    value["media_references"].as_array_mut().unwrap().reverse();
    assert_error(
        &serde_json::to_vec(&value).unwrap(),
        RadrootsPhase1PublicationArtifactError::NonCanonicalMediaInventory,
    );

    let mut value: Value = serde_json::from_slice(&artifact.to_canonical_json()).unwrap();
    value["media_references"][0]["size"] = Value::from(999);
    assert_error(
        &serde_json::to_vec(&value).unwrap(),
        RadrootsPhase1PublicationArtifactError::DigestMismatch,
    );

    let photo = &all_artifacts()[2];
    let mut value: Value = serde_json::from_slice(&photo.to_canonical_json()).unwrap();
    value["media_references"][0]["size"] = Value::from(999);
    assert_error(
        &serde_json::to_vec(&value).unwrap(),
        RadrootsPhase1PublicationArtifactError::MediaInventoryMismatch,
    );
}

#[test]
fn publication_artifact_decode_is_bounded_before_json_parsing() {
    let exact = vec![b' '; RADROOTS_PHASE1_PUBLICATION_ARTIFACT_MAX_BYTES];
    assert_eq!(
        RadrootsPhase1PublicationArtifact::from_canonical_json(&exact).unwrap_err(),
        RadrootsPhase1PublicationArtifactError::InvalidJson
    );

    let bytes = vec![b' '; RADROOTS_PHASE1_PUBLICATION_ARTIFACT_MAX_BYTES + 1];
    assert_eq!(
        RadrootsPhase1PublicationArtifact::from_canonical_json(&bytes).unwrap_err(),
        RadrootsPhase1PublicationArtifactError::ArtifactTooLarge {
            max: RADROOTS_PHASE1_PUBLICATION_ARTIFACT_MAX_BYTES,
            actual: bytes.len(),
        }
    );
}

#[test]
fn publication_artifact_media_count_accepts_exact_limit_and_rejects_one_over() {
    let artifact = &all_artifacts()[1];
    let mut value: Value = serde_json::from_slice(&artifact.to_canonical_json()).unwrap();
    let sha256 = "11".repeat(32);
    let references = (0..RADROOTS_PHASE1_PUBLICATION_MEDIA_MAX_COUNT)
        .map(|index| {
            serde_json::json!({
                "url": format!("https://media-{index:04}.example/{sha256}.png"),
                "sha256": sha256.clone(),
                "size": 1,
                "media_type": "image/png"
            })
        })
        .collect::<Vec<_>>();
    value["media_references"] = Value::Array(references.clone());
    assert_error(
        &serde_json::to_vec(&value).unwrap(),
        RadrootsPhase1PublicationArtifactError::InvalidPostProfile,
    );

    let mut one_over = references;
    one_over.push(serde_json::json!({
        "url": format!("https://media-4096.example/{sha256}.png"),
        "sha256": sha256,
        "size": 1,
        "media_type": "image/png"
    }));
    value["media_references"] = Value::Array(one_over);
    assert_error(
        &serde_json::to_vec(&value).unwrap(),
        RadrootsPhase1PublicationArtifactError::TooManyMediaReferences {
            max: RADROOTS_PHASE1_PUBLICATION_MEDIA_MAX_COUNT,
            actual: RADROOTS_PHASE1_PUBLICATION_MEDIA_MAX_COUNT + 1,
        },
    );
}

#[test]
fn publication_artifact_reload_requires_extensions_for_primary_media() {
    let artifacts = all_artifacts();
    for (index, host) in [
        (0, "media.example"),
        (2, "media.example"),
        (4, "events.example"),
        (6, "food.example"),
    ] {
        let mut value: Value =
            serde_json::from_slice(&artifacts[index].to_canonical_json()).unwrap();
        let reference = value["media_references"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|reference| reference["url"].as_str().unwrap().contains(host))
            .unwrap();
        let original = reference["url"].as_str().unwrap().to_string();
        let extension_start = original.rfind('.').unwrap();
        let extensionless = original[..extension_start].to_string();
        reference["url"] = Value::from(extensionless.clone());
        replace_json_string(&mut value["draft"], &original, &extensionless);
        rebuild_expected_event_id(&mut value);
        assert_error(
            &serde_json::to_vec(&value).unwrap(),
            RadrootsPhase1PublicationArtifactError::InvalidMediaReference,
        );
    }
}

#[test]
fn publication_artifact_reload_accepts_extensionless_post_fallback() {
    let image = authored_image(
        b"extensionless-fallback",
        "media.example",
        "webp",
        "image/webp",
    );
    let hash = image.descriptor().sha256();
    let fallback = RadrootsBlossomBlobUrl::parse(&format!("https://backup.example/{hash}"))
        .unwrap()
        .approve()
        .unwrap();
    let post_image = RadrootsAuthoredPostImage::new(
        image,
        RadrootsPostImageDimensions::new(1200, 900).unwrap(),
        "Fresh strawberries",
    )
    .unwrap()
    .try_with_fallback(fallback)
    .unwrap();
    let url = post_image.url().to_string();
    let ask =
        RadrootsAuthoredAsk::new(format!("Available this week? {url}"), vec![post_image]).unwrap();
    let artifact = RadrootsPhase1PublicationArtifact::from_ask(&ask, CREATED_AT, AUTHOR).unwrap();
    assert_eq!(
        RadrootsPhase1PublicationArtifact::from_canonical_json(&artifact.to_canonical_json())
            .unwrap(),
        artifact
    );
}

fn assert_error(bytes: &[u8], expected: RadrootsPhase1PublicationArtifactError) {
    assert_eq!(
        RadrootsPhase1PublicationArtifact::from_canonical_json(bytes).unwrap_err(),
        expected
    );
}

fn assert_every_cross_variant_is_rejected(
    artifacts: &[RadrootsPhase1PublicationArtifact],
    expected_error: &str,
    case_id: &str,
) {
    let variants = [
        "profile",
        "update",
        "photo_update",
        "ask",
        "event_date",
        "event_time",
        "food_availability",
    ];
    let mut executed = 0usize;
    for artifact in artifacts {
        for target in variants {
            if target == artifact.semantic_variant().as_str() {
                continue;
            }
            let mut value: Value = serde_json::from_slice(&artifact.to_canonical_json()).unwrap();
            value["semantic_variant"] = Value::from(target);
            let error = RadrootsPhase1PublicationArtifact::from_canonical_json(
                &serde_json::to_vec(&value).unwrap(),
            )
            .unwrap_err();
            assert_eq!(error.code(), expected_error, "{case_id}: {target}");
            executed += 1;
        }
    }
    assert_eq!(executed, 42, "{case_id}");
}

fn artifact_fixture<'a>(
    artifacts: &'a [RadrootsPhase1PublicationArtifact],
    fixture: &str,
) -> &'a RadrootsPhase1PublicationArtifact {
    let index = match fixture {
        "profile" => 0,
        "update" => 1,
        "photo_update" => 2,
        "ask" => 3,
        "event_date" => 4,
        "event_time" => 5,
        "food_availability" => 6,
        other => panic!("unknown publication fixture {other}"),
    };
    &artifacts[index]
}

fn mutate_artifact(canonical: &[u8], mutation: &str) -> Vec<u8> {
    match mutation {
        "leading_whitespace" => {
            let mut bytes = vec![b' '];
            bytes.extend_from_slice(canonical);
            return bytes;
        }
        "artifact_exact_byte_limit" => {
            return vec![b' '; RADROOTS_PHASE1_PUBLICATION_ARTIFACT_MAX_BYTES];
        }
        "artifact_one_over_byte_limit" => {
            return vec![b' '; RADROOTS_PHASE1_PUBLICATION_ARTIFACT_MAX_BYTES + 1];
        }
        "duplicate_expected_event_id" => {
            return duplicate_expected_event_id(canonical);
        }
        _ => {}
    }
    let mut value: Value = serde_json::from_slice(canonical).unwrap();
    match mutation {
        "unknown_field" => value["unknown"] = Value::Bool(true),
        "unknown_draft_field" => value["draft"]["unknown"] = Value::Bool(true),
        "nested_expected_event_id" => {
            value["draft"]["expected_event_id"] = value["expected_event_id"].clone();
        }
        "missing_expected_event_id" => {
            value.as_object_mut().unwrap().remove("expected_event_id");
        }
        "malformed_expected_event_id" => {
            value["expected_event_id"] = Value::from("not-an-event-id");
        }
        "uppercase_expected_event_id" => {
            value["expected_event_id"] = Value::from(
                value["expected_event_id"]
                    .as_str()
                    .unwrap()
                    .to_ascii_uppercase(),
            );
        }
        "unknown_media_field" => value["media_references"][0]["unknown"] = Value::Bool(true),
        "json_field_order" => {}
        "schema_version" => value["schema_version"] = Value::from(2),
        "operation_id" => {
            value["authored_operation_id"] = Value::from("social.update.build_authored_draft");
        }
        "contract_id" => {
            value["event_contract_id"] = Value::from("radroots.social.update.v1");
        }
        "kind" => value["draft"]["kind"] = Value::from(1),
        "author" => value["expected_author"] = Value::from("not-a-key"),
        "created_at" => value["draft"]["created_at"] = Value::from(CREATED_AT + 1),
        "draft_tags" => value["draft"]["tags"] = serde_json::json!([["x"]]),
        "draft_content" => value["draft"]["content"] = Value::from("changed"),
        "noncanonical_nip05" => {
            let content = value["draft"]["content"]
                .as_str()
                .unwrap()
                .replace("farm@example.com", "farm@EXAMPLE.COM");
            assert!(content.contains("farm@EXAMPLE.COM"));
            value["draft"]["content"] = Value::from(content);
            rebuild_expected_event_id(&mut value);
        }
        "empty_ask_content" => {
            value["draft"]["content"] = Value::from(" \t");
            rebuild_expected_event_id(&mut value);
        }
        "expected_event_id" => {
            value["expected_event_id"] = Value::from("00".repeat(32));
        }
        "digest" => value["artifact_digest"] = Value::from("00".repeat(32)),
        "media_order" => value["media_references"].as_array_mut().unwrap().reverse(),
        "media_size" => value["media_references"][0]["size"] = Value::from(999),
        "media_url" => {
            let url = value["media_references"][0]["url"].as_str().unwrap();
            value["media_references"][0]["url"] =
                Value::from(url.replacen("https://media.example", "https://alternate.example", 1));
        }
        "media_url_casing" => {
            let url = value["media_references"][0]["url"].as_str().unwrap();
            value["media_references"][0]["url"] =
                Value::from(url.replacen("https://", "HTTPS://", 1));
        }
        "media_hash" => value["media_references"][0]["sha256"] = Value::from("00".repeat(32)),
        "media_type" => value["media_references"][0]["media_type"] = Value::from("image/jpeg"),
        other => panic!("unknown publication mutation {other}"),
    }
    serde_json::to_vec(&value).unwrap()
}

fn duplicate_expected_event_id(canonical: &[u8]) -> Vec<u8> {
    let value: Value = serde_json::from_slice(canonical).unwrap();
    let event_id = value["expected_event_id"].as_str().unwrap();
    let field = format!("\"expected_event_id\":\"{event_id}\"");
    let duplicate = format!("{field},{field}");
    let canonical = core::str::from_utf8(canonical).unwrap();
    let mutated = canonical.replacen(&field, &duplicate, 1);
    assert_ne!(mutated, canonical);
    mutated.into_bytes()
}

fn rebuild_expected_event_id(value: &mut Value) {
    let author = value["expected_author"].as_str().unwrap();
    let created_at = value["draft"]["created_at"].as_u64().unwrap();
    let kind = value["draft"]["kind"].as_u64().unwrap() as u32;
    let tags: Vec<Vec<String>> = serde_json::from_value(value["draft"]["tags"].clone()).unwrap();
    let content = value["draft"]["content"].as_str().unwrap();
    value["expected_event_id"] = Value::from(
        compute_canonical_nip01_event_id(author, created_at, kind, &tags, content)
            .unwrap()
            .to_string(),
    );
}

fn replace_json_string(value: &mut Value, from: &str, to: &str) {
    match value {
        Value::String(string) => *string = string.replace(from, to),
        Value::Array(values) => {
            for value in values {
                replace_json_string(value, from, to);
            }
        }
        Value::Object(values) => {
            for value in values.values_mut() {
                replace_json_string(value, from, to);
            }
        }
        _ => {}
    }
}

fn all_artifacts() -> Vec<RadrootsPhase1PublicationArtifact> {
    let picture = authored_image(b"profile-picture", "media.example", "png", "image/png");
    let banner = authored_image(b"profile-banner", "media.example", "webp", "image/webp");
    let profile = RadrootsAuthoredProfile::new("victoria-farm")
        .unwrap()
        .with_display_name("Victoria Farm")
        .with_about("Seasonal produce from the Saanich Peninsula")
        .with_picture(picture)
        .with_banner(banner)
        .with_nip05(RadrootsNip05Identifier::parse("farm@example.com").unwrap())
        .with_bot(false);

    let post_image = authored_post_image(b"ask-and-photo");
    let post_url = post_image.url().to_string();
    let photo = RadrootsAuthoredPhotoUpdate::new(
        format!("Strawberries at the farm stand {post_url}"),
        vec![post_image.clone()],
    )
    .unwrap();
    let ask = RadrootsAuthoredAsk::new(
        format!("When will strawberries be ready? {post_url}"),
        vec![post_image],
    )
    .unwrap();

    let event_image = authored_image(b"farm-event", "events.example", "jpeg", "image/jpeg");
    let date = RadrootsAuthoredCalendarDateEvent::new(
        "farmers-market-2026",
        "Moss Street Farmers Market",
        RadrootsCalendarDate::parse("2026-07-25").unwrap(),
    )
    .unwrap()
    .with_end(RadrootsCalendarDate::parse("2026-07-26").unwrap())
    .unwrap()
    .with_description("Saturday market in Victoria")
    .unwrap()
    .with_locations(vec!["Victoria, BC".to_string()])
    .unwrap()
    .with_image(event_image.clone())
    .unwrap();
    let time = RadrootsAuthoredCalendarTimeEvent::new(
        "farm-tour-2026",
        "Saanich Farm Tour",
        1_785_003_600,
    )
    .unwrap()
    .with_end(1_785_007_200)
    .unwrap()
    .with_start_tzid("America/Vancouver")
    .unwrap()
    .with_description("A one-hour farm tour")
    .unwrap()
    .with_image(event_image)
    .unwrap();

    vec![
        RadrootsPhase1PublicationArtifact::from_profile(&profile, CREATED_AT, AUTHOR).unwrap(),
        RadrootsPhase1PublicationArtifact::from_update(
            &RadrootsAuthoredUpdate::new("Carrots harvested today").unwrap(),
            CREATED_AT,
            AUTHOR,
        )
        .unwrap(),
        RadrootsPhase1PublicationArtifact::from_photo_update(&photo, CREATED_AT, AUTHOR).unwrap(),
        RadrootsPhase1PublicationArtifact::from_ask(&ask, CREATED_AT, AUTHOR).unwrap(),
        RadrootsPhase1PublicationArtifact::from_calendar_date_event(&date, CREATED_AT, AUTHOR)
            .unwrap(),
        RadrootsPhase1PublicationArtifact::from_calendar_time_event(&time, CREATED_AT, AUTHOR)
            .unwrap(),
        RadrootsPhase1PublicationArtifact::from_food_availability(
            &food_details(),
            CREATED_AT,
            AUTHOR,
        )
        .unwrap(),
    ]
}

fn authored_post_image(bytes: &[u8]) -> RadrootsAuthoredPostImage {
    let image = authored_image(bytes, "media.example", "webp", "image/webp");
    let hash = image.descriptor().sha256();
    let fallback = RadrootsBlossomBlobUrl::parse(&format!("https://backup.example/{hash}.webp"))
        .unwrap()
        .approve()
        .unwrap();
    RadrootsAuthoredPostImage::new(
        image,
        RadrootsPostImageDimensions::new(1200, 900).unwrap(),
        "Fresh strawberries",
    )
    .unwrap()
    .try_with_fallback(fallback)
    .unwrap()
}

fn food_details() -> RadrootsFoodAvailabilityDetails {
    let image = RadrootsFoodAvailabilityImage::new(
        authored_image(b"nantes-carrots", "food.example", "png", "image/png"),
        RadrootsFoodImageDimensions::new(1200, 800).unwrap(),
    );
    RadrootsFoodAvailabilityDetails::new(RadrootsFoodAvailabilityDetailsParts {
        content: RadrootsFoodContent::new("Fresh Nantes carrots available this week.").unwrap(),
        identifier: RadrootsFoodIdentifier::parse("nantes-carrots").unwrap(),
        title: RadrootsFoodText::new("Nantes Carrots").unwrap(),
        summary: RadrootsFoodText::new("Fresh bunches").unwrap(),
        published_at: RadrootsFoodPublishedAt::new(CREATED_AT - 60).unwrap(),
        location: RadrootsFoodText::new("Central Saanich, BC").unwrap(),
        price: RadrootsFoodPrice::new(
            "3",
            RadrootsFoodCurrency::parse("CAD").unwrap(),
            RadrootsFoodUnit::Pound,
        )
        .unwrap(),
        quantity: Some(RadrootsFoodQuantity::new("24", RadrootsFoodUnit::Pound).unwrap()),
        status: RadrootsFoodAvailabilityStatus::Active,
        images: vec![image],
    })
    .unwrap()
}

fn authored_image(
    bytes: &[u8],
    host: &str,
    extension: &str,
    media_type: &str,
) -> RadrootsAuthoredImage {
    RadrootsAuthoredImage::try_from(verified_descriptor(bytes, host, extension, media_type))
        .unwrap()
}

fn verified_descriptor(
    bytes: &[u8],
    host: &str,
    extension: &str,
    media_type: &str,
) -> RadrootsBlossomByteVerifiedDescriptor {
    let sha256 = RadrootsBlossomSha256::digest(bytes);
    let media_type = RadrootsBlossomMediaType::parse(media_type).unwrap();
    RadrootsBlossomBlobDescriptor::new(
        RadrootsBlossomBlobUrl::parse(&format!("https://{host}/{sha256}.{extension}")).unwrap(),
        sha256,
        bytes.len() as u64,
        media_type.clone(),
        CREATED_AT,
    )
    .unwrap()
    .approve_reference()
    .unwrap()
    .verify_bytes(bytes, &media_type)
    .unwrap()
}
