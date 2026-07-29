#![cfg(feature = "serde_json")]

use radroots_blossom::{
    PublicationReadinessEvidence, RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_BYTES,
};
use radroots_event::wire::compute_canonical_nip01_event_id;
use radroots_event_codec::wire::publication::{
    RADROOTS_PHASE1_PUBLICATION_MEDIA_MAX_COUNT,
    RADROOTS_PHASE1_PUBLICATION_MEDIA_READINESS_BINDING_MAX_BYTES,
    RadrootsPhase1PublicationArtifact, RadrootsPhase1PublicationMediaReadinessError,
    RadrootsPhase1PublicationMediaReference,
    allowlist::{
        RadrootsPhase1AllowlistedPublicationArtifact, allow_phase1_publication_canonical_json,
    },
    bind_phase1_publication_media_readiness, validate_phase1_publication_media_readiness,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

const CANONICAL_VECTOR: &[u8] = include_bytes!(
    "../../../contracts/conformance/vectors/publication/phase1_media_readiness.v1.json"
);
const PACKAGED_VECTOR: &[u8] =
    include_bytes!("fixtures/phase1_publication_media_readiness.v1.json");
const ARTIFACT_VECTOR: &[u8] = include_bytes!("fixtures/phase1_publication_artifact.v1.json");

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VectorSuite {
    suite: String,
    contract_version: String,
    vectors: Vec<VectorCase>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VectorCase {
    id: String,
    kind: String,
    input: VectorInput,
    expected: VectorExpected,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VectorInput {
    fixture: String,
    mutation: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VectorExpected {
    #[serde(default)]
    decision: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Serialize)]
struct EvidenceDimensionsWire {
    width: u32,
    height: u32,
}

#[derive(Serialize)]
struct EvidenceWire<'a> {
    schema_version: u32,
    policy_version: u16,
    url: &'a str,
    sha256: String,
    size: u64,
    media_type: &'a str,
    raster_format: &'a str,
    dimensions: EvidenceDimensionsWire,
    bud02_status: u16,
    bud01_head_status: u16,
    bud01_get_status: u16,
    uploaded: u64,
    evidence_digest: String,
}

#[test]
fn phase1_publication_media_readiness_vector_executes_every_case() {
    assert_eq!(CANONICAL_VECTOR, PACKAGED_VECTOR, "packaged vector drift");
    let suite: VectorSuite = serde_json::from_slice(PACKAGED_VECTOR).expect("media vector");
    assert_eq!(suite.suite, "phase1_publication_media_readiness");
    assert_eq!(suite.contract_version, "1.0.0");
    assert_eq!(suite.vectors.len(), 39);

    for vector in suite.vectors {
        match vector.kind.as_str() {
            kind @ ("publication_media_readiness.bind.valid"
            | "publication_media_readiness.to_canonical_json.valid"
            | "publication_media_readiness.from_canonical_json.valid"
            | "publication_media_readiness.validate.valid") => {
                let ready = ready_fixture(&vector.input.fixture);
                assert_eq!(vector.input.mutation, "none", "{}", vector.id);
                assert_eq!(vector.expected.decision.as_deref(), Some("allow"));
                assert!(vector.expected.error.is_none());
                match kind {
                    "publication_media_readiness.bind.valid" => {}
                    "publication_media_readiness.to_canonical_json.valid" => {
                        let canonical = ready.to_canonical_json();
                        assert_eq!(canonical, ready.canonical_json(), "{}", vector.id);
                    }
                    "publication_media_readiness.from_canonical_json.valid" => {
                        let reloaded =
                            radroots_event_codec::wire::publication::RadrootsPhase1MediaReadyPublicationArtifact::from_canonical_json(
                                allowlisted_fixture(&vector.input.fixture),
                                ready.canonical_json(),
                            )
                            .unwrap_or_else(|error| panic!("{}: {error}", vector.id));
                        assert_eq!(reloaded, ready, "{}", vector.id);
                    }
                    "publication_media_readiness.validate.valid" => {
                        validate_phase1_publication_media_readiness(&ready)
                            .unwrap_or_else(|error| panic!("{}: {error}", vector.id));
                    }
                    _ => unreachable!("closed valid operation kind"),
                }
                assert_eq!(
                    independent_binding_digest(&ready),
                    *ready.binding_digest().as_bytes(),
                    "{}",
                    vector.id
                );
            }
            "publication_media_readiness.bind.invalid"
            | "publication_media_readiness.from_canonical_json.invalid" => {
                assert!(vector.expected.decision.is_none(), "{}", vector.id);
                let error = execute_binding_mutation(&vector.input.fixture, &vector.input.mutation);
                assert_eq!(
                    Some(error.code()),
                    vector.expected.error.as_deref(),
                    "{}",
                    vector.id
                );
            }
            "publication_media_readiness.bind.artifact_invalid" => {
                assert!(vector.expected.decision.is_none(), "{}", vector.id);
                let error =
                    execute_artifact_mutation(&vector.input.fixture, &vector.input.mutation);
                assert_eq!(
                    Some(error.code()),
                    vector.expected.error.as_deref(),
                    "{}",
                    vector.id
                );
            }
            kind => panic!("{} has unsupported kind {kind}", vector.id),
        }
    }
}

#[test]
fn media_readiness_public_accessors_and_error_taxonomy_are_executable() {
    let ready = ready_fixture("ask");
    let expected_allowlisted = allowlisted_fixture("ask");
    assert_eq!(ready.allowlisted_artifact(), &expected_allowlisted);
    assert_eq!(ready.to_canonical_json(), ready.canonical_json());
    assert_eq!(
        ready.binding_digest().to_string(),
        ready.binding_digest().to_hex()
    );
    assert_eq!(
        ready.clone().into_allowlisted_artifact(),
        expected_allowlisted
    );

    let errors = [
        RadrootsPhase1PublicationMediaReadinessError::BindingTooLarge { max: 4, actual: 5 },
        RadrootsPhase1PublicationMediaReadinessError::EvidenceCountExceeded { max: 4, actual: 5 },
        RadrootsPhase1PublicationMediaReadinessError::EvidenceCountMismatch {
            expected: 1,
            actual: 0,
        },
        RadrootsPhase1PublicationMediaReadinessError::InvalidJson,
        RadrootsPhase1PublicationMediaReadinessError::NonCanonicalJson,
        RadrootsPhase1PublicationMediaReadinessError::UnsupportedSchemaVersion {
            expected: 1,
            actual: 2,
        },
        RadrootsPhase1PublicationMediaReadinessError::UnsupportedReadinessPolicyVersion {
            expected: 1,
            actual: 2,
        },
        RadrootsPhase1PublicationMediaReadinessError::ArtifactDigestMismatch,
        RadrootsPhase1PublicationMediaReadinessError::ArtifactProfileInvalid,
        RadrootsPhase1PublicationMediaReadinessError::EvidenceInvalid,
        RadrootsPhase1PublicationMediaReadinessError::EvidenceOrderMismatch { index: 0 },
        RadrootsPhase1PublicationMediaReadinessError::EvidenceFactMismatch { index: 0 },
        RadrootsPhase1PublicationMediaReadinessError::EvidenceDimensionMismatch { index: 0 },
        RadrootsPhase1PublicationMediaReadinessError::InvalidDigest,
        RadrootsPhase1PublicationMediaReadinessError::DigestMismatch,
        RadrootsPhase1PublicationMediaReadinessError::StateMismatch,
        RadrootsPhase1PublicationMediaReadinessError::AllocationFailed,
        RadrootsPhase1PublicationMediaReadinessError::Serialization,
    ];
    for error in errors {
        assert!(error.code().starts_with("publication_media_readiness_"));
        assert!(!error.to_string().is_empty());
    }
}

fn execute_binding_mutation(
    fixture: &str,
    mutation: &str,
) -> RadrootsPhase1PublicationMediaReadinessError {
    let artifact = allowlisted_fixture(fixture);
    let mut evidence = evidence_for_fixture(fixture, artifact.artifact().media_references());
    match mutation {
        "missing" => {
            evidence.pop();
            bind_phase1_publication_media_readiness(artifact, evidence).unwrap_err()
        }
        "extra" => {
            evidence.push(evidence[0].clone());
            bind_phase1_publication_media_readiness(artifact, evidence).unwrap_err()
        }
        "duplicate" => {
            evidence[1] = evidence[0].clone();
            bind_phase1_publication_media_readiness(artifact, evidence).unwrap_err()
        }
        "reordered" => {
            evidence.swap(0, 1);
            bind_phase1_publication_media_readiness(artifact, evidence).unwrap_err()
        }
        "size_mismatch" => {
            let reference = &artifact.artifact().media_references()[0];
            evidence[0] = evidence_for_reference(
                reference,
                expected_dimensions(fixture)[0],
                reference.size() + 1,
            );
            bind_phase1_publication_media_readiness(artifact, evidence).unwrap_err()
        }
        "dimension_mismatch" => {
            let reference = &artifact.artifact().media_references()[0];
            evidence[0] = evidence_for_reference(reference, (1, 1), reference.size());
            bind_phase1_publication_media_readiness(artifact, evidence).unwrap_err()
        }
        "cross_artifact" => {
            let ready = bind_phase1_publication_media_readiness(artifact, evidence).unwrap();
            radroots_event_codec::wire::publication::RadrootsPhase1MediaReadyPublicationArtifact::from_canonical_json(
                allowlisted_fixture("photo_update"),
                ready.canonical_json(),
            )
            .unwrap_err()
        }
        "evidence_count_exact_max" | "evidence_count_over_max" => {
            let sample_artifact = allowlisted_fixture("event_date");
            let sample =
                evidence_for_fixture("event_date", sample_artifact.artifact().media_references())
                    .remove(0);
            let count = RADROOTS_PHASE1_PUBLICATION_MEDIA_MAX_COUNT
                + usize::from(mutation == "evidence_count_over_max");
            bind_phase1_publication_media_readiness(artifact, vec![sample; count]).unwrap_err()
        }
        "wire_evidence_count_exact_max" | "wire_evidence_count_over_max" => {
            let ready =
                bind_phase1_publication_media_readiness(artifact.clone(), evidence).unwrap();
            let count = RADROOTS_PHASE1_PUBLICATION_MEDIA_MAX_COUNT
                + usize::from(mutation == "wire_evidence_count_over_max");
            let bytes = replace_evidence_array(ready.canonical_json(), count);
            radroots_event_codec::wire::publication::RadrootsPhase1MediaReadyPublicationArtifact::from_canonical_json(
                artifact,
                &bytes,
            )
            .unwrap_err()
        }
        mutation => {
            let ready =
                bind_phase1_publication_media_readiness(artifact.clone(), evidence).unwrap();
            let bytes = mutate_binding(ready.canonical_json(), mutation);
            radroots_event_codec::wire::publication::RadrootsPhase1MediaReadyPublicationArtifact::from_canonical_json(
                artifact,
                &bytes,
            )
            .unwrap_err()
        }
    }
}

fn execute_artifact_mutation(
    fixture: &str,
    mutation: &str,
) -> radroots_event_codec::wire::publication::RadrootsPhase1PublicationArtifactError {
    let canonical = artifact_canonical_json(fixture);
    let mut value: Value = serde_json::from_slice(&canonical).unwrap();
    match mutation {
        "url_exact_max" | "url_over_max" => {
            let length = 4_096 + usize::from(mutation == "url_over_max");
            value["media_references"][0]["url"] = Value::String(blob_url_with_length(length));
        }
        "size_zero" => value["media_references"][0]["size"] = Value::from(0_u64),
        "size_over_max" => {
            value["media_references"][0]["size"] =
                Value::from(RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_BYTES + 1);
        }
        "mime_unsupported" => {
            value["media_references"][0]["media_type"] = Value::String("image/gif".to_string());
        }
        "dimensions_over_max" => {
            let tags = value["draft"]["tags"].as_array_mut().unwrap();
            let imeta = tags
                .iter_mut()
                .find(|tag| tag[0].as_str() == Some("imeta"))
                .unwrap()
                .as_array_mut()
                .unwrap();
            let dimension = imeta
                .iter_mut()
                .find(|element| {
                    element
                        .as_str()
                        .is_some_and(|value| value.starts_with("dim "))
                })
                .unwrap();
            *dimension = Value::String("dim 16384x16384".to_string());
            let draft = &value["draft"];
            let tags: Vec<Vec<String>> = serde_json::from_value(draft["tags"].clone()).unwrap();
            let event_id = compute_canonical_nip01_event_id(
                value["expected_author"].as_str().unwrap(),
                draft["created_at"].as_u64().unwrap(),
                u32::try_from(draft["kind"].as_u64().unwrap()).unwrap(),
                &tags,
                draft["content"].as_str().unwrap(),
            )
            .unwrap();
            value["expected_event_id"] = Value::String(event_id.as_str().to_string());
        }
        _ => panic!("unsupported artifact mutation {mutation}"),
    }
    RadrootsPhase1PublicationArtifact::from_canonical_json(&serde_json::to_vec(&value).unwrap())
        .unwrap_err()
}

fn ready_fixture(
    fixture: &str,
) -> radroots_event_codec::wire::publication::RadrootsPhase1MediaReadyPublicationArtifact {
    let artifact = allowlisted_fixture(fixture);
    let evidence = evidence_for_fixture(fixture, artifact.artifact().media_references());
    bind_phase1_publication_media_readiness(artifact, evidence).unwrap()
}

fn allowlisted_fixture(fixture: &str) -> RadrootsPhase1AllowlistedPublicationArtifact {
    allow_phase1_publication_canonical_json(&artifact_canonical_json(fixture)).unwrap()
}

fn artifact_canonical_json(fixture: &str) -> Vec<u8> {
    let root: Value = serde_json::from_slice(ARTIFACT_VECTOR).unwrap();
    root["vectors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|vector| {
            vector["kind"]
                .as_str()
                .is_some_and(|kind| kind.ends_with(".valid"))
                && vector["input"]["fixture"].as_str() == Some(fixture)
        })
        .and_then(|vector| vector["expected"]["canonical_json"].as_str())
        .unwrap_or_else(|| panic!("missing artifact fixture {fixture}"))
        .as_bytes()
        .to_vec()
}

fn evidence_for_fixture(
    fixture: &str,
    media: &[RadrootsPhase1PublicationMediaReference],
) -> Vec<PublicationReadinessEvidence> {
    let dimensions = expected_dimensions(fixture);
    assert_eq!(dimensions.len(), media.len());
    media
        .iter()
        .zip(dimensions)
        .map(|(reference, dimensions)| {
            evidence_for_reference(reference, dimensions, reference.size())
        })
        .collect()
}

fn expected_dimensions(fixture: &str) -> Vec<(u32, u32)> {
    match fixture {
        "profile" => vec![(640, 640), (1_600, 600)],
        "update" => Vec::new(),
        "photo_update" | "ask" => vec![(1_200, 900), (1_200, 900)],
        "event_date" | "event_time" => vec![(640, 480)],
        "food_availability" => vec![(1_200, 800)],
        _ => panic!("unknown fixture {fixture}"),
    }
}

fn evidence_for_reference(
    reference: &RadrootsPhase1PublicationMediaReference,
    dimensions: (u32, u32),
    size: u64,
) -> PublicationReadinessEvidence {
    let media_type = reference.media_type().as_str();
    let (raster_format, format_code) = match media_type {
        "image/jpeg" => ("jpeg", 1),
        "image/png" => ("png", 2),
        "image/webp" => ("still_webp", 3),
        _ => panic!("unsupported test MIME {media_type}"),
    };
    let url = reference.url().as_str();
    let uploaded = 1_800_000_001_u64;
    let evidence_digest = evidence_digest(
        url,
        reference.sha256().as_bytes(),
        size,
        media_type,
        format_code,
        dimensions,
        uploaded,
    );
    let wire = EvidenceWire {
        schema_version: 1,
        policy_version: 1,
        url,
        sha256: reference.sha256().to_hex(),
        size,
        media_type,
        raster_format,
        dimensions: EvidenceDimensionsWire {
            width: dimensions.0,
            height: dimensions.1,
        },
        bud02_status: 201,
        bud01_head_status: 200,
        bud01_get_status: 200,
        uploaded,
        evidence_digest,
    };
    PublicationReadinessEvidence::from_canonical_json(&serde_json::to_vec(&wire).unwrap()).unwrap()
}

fn evidence_digest(
    url: &str,
    sha256: &[u8; 32],
    size: u64,
    media_type: &str,
    format_code: u8,
    dimensions: (u32, u32),
    uploaded: u64,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"radroots.blossom.publication-readiness-evidence.v1\0");
    hasher.update(1_u16.to_be_bytes());
    update_length_prefixed(&mut hasher, url.as_bytes());
    hasher.update(sha256);
    hasher.update(size.to_be_bytes());
    update_length_prefixed(&mut hasher, media_type.as_bytes());
    hasher.update([format_code]);
    hasher.update(dimensions.0.to_be_bytes());
    hasher.update(dimensions.1.to_be_bytes());
    hasher.update(201_u16.to_be_bytes());
    hasher.update(200_u16.to_be_bytes());
    hasher.update(200_u16.to_be_bytes());
    hasher.update(uploaded.to_be_bytes());
    hex::encode(hasher.finalize())
}

fn independent_binding_digest(
    ready: &radroots_event_codec::wire::publication::RadrootsPhase1MediaReadyPublicationArtifact,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"radroots.phase1.publication-media-readiness.v1\0");
    hasher.update(1_u32.to_be_bytes());
    hasher.update(1_u16.to_be_bytes());
    hasher.update(ready.artifact().artifact_digest().as_bytes());
    hasher.update((ready.evidence().len() as u32).to_be_bytes());
    for evidence in ready.evidence() {
        update_length_prefixed(&mut hasher, evidence.url().as_str().as_bytes());
        hasher.update(evidence.evidence_digest().as_sha256().as_bytes());
    }
    hasher.finalize().into()
}

fn update_length_prefixed(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

fn mutate_binding(canonical: &[u8], mutation: &str) -> Vec<u8> {
    match mutation {
        "schema_version" => {
            replace_once(canonical, b"\"schema_version\":1", b"\"schema_version\":2")
        }
        "policy_version" => replace_once(
            canonical,
            b"\"readiness_policy_version\":1",
            b"\"readiness_policy_version\":2",
        ),
        "digest_mismatch" => mutate_binding_digest(canonical),
        "digest_invalid" => replace_binding_digest(canonical, b'G'),
        "leading_whitespace" => [b" ".as_slice(), canonical].concat(),
        "reordered_fields" => replace_once(
            canonical,
            b"{\"schema_version\":1,\"readiness_policy_version\":1",
            b"{\"readiness_policy_version\":1,\"schema_version\":1",
        ),
        "unknown_field" => append_field(canonical, b",\"unknown\":true"),
        "bud11_field" => append_field(canonical, b",\"authorization\":\"Nostr token\""),
        "nested_bud11_field" => replace_once(
            canonical,
            b",\"evidence_digest\"",
            b",\"authorization\":\"Nostr token\",\"evidence_digest\"",
        ),
        "binding_exact_max" => {
            vec![b' '; RADROOTS_PHASE1_PUBLICATION_MEDIA_READINESS_BINDING_MAX_BYTES]
        }
        "binding_over_max" => {
            vec![b' '; RADROOTS_PHASE1_PUBLICATION_MEDIA_READINESS_BINDING_MAX_BYTES + 1]
        }
        _ => panic!("unsupported binding mutation {mutation}"),
    }
}

fn mutate_binding_digest(canonical: &[u8]) -> Vec<u8> {
    let mut output = canonical.to_vec();
    let start = binding_digest_start(canonical);
    output[start] = if output[start] == b'0' { b'1' } else { b'0' };
    output
}

fn replace_binding_digest(canonical: &[u8], replacement: u8) -> Vec<u8> {
    let mut output = canonical.to_vec();
    output[binding_digest_start(canonical)] = replacement;
    output
}

fn binding_digest_start(canonical: &[u8]) -> usize {
    let marker = b"\"binding_digest\":\"";
    canonical
        .windows(marker.len())
        .position(|candidate| candidate == marker)
        .unwrap()
        + marker.len()
}

fn replace_evidence_array(canonical: &[u8], count: usize) -> Vec<u8> {
    let start_marker = b"\"evidence\":[";
    let start = canonical
        .windows(start_marker.len())
        .position(|candidate| candidate == start_marker)
        .unwrap()
        + start_marker.len();
    let end_marker = b"],\"binding_digest\"";
    let end = canonical
        .windows(end_marker.len())
        .position(|candidate| candidate == end_marker)
        .unwrap();
    let mut output = Vec::with_capacity(canonical.len() + count * 3);
    output.extend_from_slice(&canonical[..start]);
    for index in 0..count {
        if index != 0 {
            output.push(b',');
        }
        output.extend_from_slice(b"{}");
    }
    output.extend_from_slice(&canonical[end..]);
    output
}

fn append_field(canonical: &[u8], field: &[u8]) -> Vec<u8> {
    let mut output = canonical[..canonical.len() - 1].to_vec();
    output.extend_from_slice(field);
    output.push(b'}');
    output
}

fn replace_once(input: &[u8], from: &[u8], to: &[u8]) -> Vec<u8> {
    let index = input
        .windows(from.len())
        .position(|candidate| candidate == from)
        .unwrap();
    let mut output = Vec::with_capacity(input.len() - from.len() + to.len());
    output.extend_from_slice(&input[..index]);
    output.extend_from_slice(to);
    output.extend_from_slice(&input[index + from.len()..]);
    output
}

fn blob_url_with_length(length: usize) -> String {
    const PREFIX: &str = concat!(
        "https://media.example/",
        "0a422cbf828d421341c40c678f4cfbd6451841760db126e5f5ac3d2e06fd80b8."
    );
    assert!(length >= PREFIX.len());
    format!("{PREFIX}{}", "a".repeat(length - PREFIX.len()))
}
