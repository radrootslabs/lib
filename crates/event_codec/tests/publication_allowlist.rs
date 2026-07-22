#![cfg(feature = "serde_json")]

use std::collections::BTreeSet;

use radroots_event_codec::wire::publication::{
    RadrootsPhase1PublicationArtifact,
    allowlist::{
        RadrootsPhase1AllowlistedPublicationArtifact, RadrootsPhase1PublicationAllowlistError,
        RadrootsPhase1PublicationLeaf, allow_phase1_publication_artifact,
        allow_phase1_publication_canonical_json,
    },
};
use serde::Deserialize;

const ALLOWLIST_VECTOR: &str = include_str!("fixtures/phase1_publication_allowlist.v1.json");

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
    surface: String,
    canonical_json: String,
    family: Option<String>,
    event_kind: Option<u32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VectorExpected {
    decision: String,
    leaf: Option<String>,
    event_kind: Option<u32>,
    error: Option<String>,
    source_error: Option<String>,
}

struct Decision {
    leaf: Option<RadrootsPhase1PublicationLeaf>,
    event_kind: Option<u32>,
    error: Option<&'static str>,
    source_error: Option<String>,
}

#[test]
fn publication_allowlist_conformance_vector_executes_every_case() {
    let suite: VectorSuite = serde_json::from_str(ALLOWLIST_VECTOR).unwrap();
    assert_eq!(suite.suite, "phase1_publication_allowlist");
    assert_eq!(suite.contract_version, "1.0.0");
    assert_eq!(suite.vectors.len(), 39);

    let mut direct_allowed_leaves = Vec::new();
    let mut canonical_allowed_leaves = Vec::new();
    let mut rejected_families = BTreeSet::new();
    for case in suite.vectors {
        assert!(matches!(
            case.kind.as_str(),
            "publication_allowlist.allow_artifact.valid"
                | "publication_allowlist.allow_canonical_json.valid"
                | "publication_allowlist.allow_canonical_json.invalid"
        ));
        let decision = execute_case(&case.kind, &case.input);
        match case.expected.decision.as_str() {
            "allow" => {
                let leaf = decision.leaf.unwrap_or_else(|| panic!("{}", case.id));
                assert_eq!(
                    leaf.as_str(),
                    case.expected.leaf.as_deref().unwrap(),
                    "{}",
                    case.id
                );
                assert_eq!(decision.event_kind, case.expected.event_kind, "{}", case.id);
                assert_eq!(decision.error, None, "{}", case.id);
                assert_eq!(decision.source_error, None, "{}", case.id);
                match case.kind.as_str() {
                    "publication_allowlist.allow_artifact.valid" => {
                        direct_allowed_leaves.push(leaf);
                    }
                    "publication_allowlist.allow_canonical_json.valid" => {
                        canonical_allowed_leaves.push(leaf);
                    }
                    other => panic!("{} has invalid allow case kind {other}", case.id),
                }
            }
            "reject" => {
                assert_eq!(decision.leaf, None, "{}", case.id);
                assert_eq!(
                    decision.error,
                    case.expected.error.as_deref(),
                    "{}",
                    case.id
                );
                assert_eq!(
                    decision.source_error.as_deref(),
                    case.expected.source_error.as_deref(),
                    "{}",
                    case.id
                );
                if let Some(family) = case.input.family {
                    rejected_families.insert(family);
                }
            }
            other => panic!("{} has unknown decision {other}", case.id),
        }
    }

    let expected_leaves = vec![
        RadrootsPhase1PublicationLeaf::Profile,
        RadrootsPhase1PublicationLeaf::Update,
        RadrootsPhase1PublicationLeaf::PhotoUpdate,
        RadrootsPhase1PublicationLeaf::Ask,
        RadrootsPhase1PublicationLeaf::EventDate,
        RadrootsPhase1PublicationLeaf::EventTime,
        RadrootsPhase1PublicationLeaf::FoodAvailability,
    ];
    direct_allowed_leaves.sort();
    canonical_allowed_leaves.sort();
    assert_eq!(direct_allowed_leaves, expected_leaves);
    assert_eq!(canonical_allowed_leaves, expected_leaves);
    assert_eq!(
        rejected_families,
        [
            "bud11_authorization",
            "calendar_collection",
            "calendar_rsvp",
            "commerce_order",
            "comment",
            "deletion_request",
            "ephemeral_event",
            "generic_calendar",
            "generic_nip99",
            "generic_root_note",
            "group",
            "mixed_classified_listing",
            "operational_listing",
            "operations",
            "reply",
            "route_delivery",
            "trade",
            "unsealed_calendar_event",
            "unsealed_profile",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect()
    );
}

#[test]
fn publication_allowlist_public_operation_accepts_only_the_sealed_artifact_type() {
    type Operation = fn(
        RadrootsPhase1PublicationArtifact,
    ) -> Result<
        RadrootsPhase1AllowlistedPublicationArtifact,
        RadrootsPhase1PublicationAllowlistError,
    >;
    let operation: Operation = allow_phase1_publication_artifact;
    let _ = operation;

    type CanonicalOperation = fn(
        &[u8],
    ) -> Result<
        RadrootsPhase1AllowlistedPublicationArtifact,
        RadrootsPhase1PublicationAllowlistError,
    >;
    let canonical_operation: CanonicalOperation = allow_phase1_publication_canonical_json;
    let _ = canonical_operation;
}

fn execute_case(case_kind: &str, input: &VectorInput) -> Decision {
    assert!(serde_json::from_str::<serde_json::Value>(&input.canonical_json).is_ok());
    if case_kind == "publication_allowlist.allow_artifact.valid" {
        assert_eq!(input.surface, "sealed_artifact_json");
        let artifact =
            RadrootsPhase1PublicationArtifact::from_canonical_json(input.canonical_json.as_bytes())
                .unwrap();
        let allowed = allow_phase1_publication_artifact(artifact).unwrap();
        return Decision {
            leaf: Some(allowed.leaf()),
            event_kind: Some(allowed.artifact().draft().kind()),
            error: None,
            source_error: None,
        };
    }
    assert!(matches!(
        (case_kind, input.surface.as_str()),
        (
            "publication_allowlist.allow_canonical_json.valid",
            "canonical_artifact_json"
        ) | (
            "publication_allowlist.allow_canonical_json.invalid",
            "artifact_candidate_json" | "raw_nip01_event_json" | "non_event_product_surface"
        )
    ));
    match allow_phase1_publication_canonical_json(input.canonical_json.as_bytes()) {
        Ok(allowed) => Decision {
            leaf: Some(allowed.leaf()),
            event_kind: Some(allowed.artifact().draft().kind()),
            error: None,
            source_error: None,
        },
        Err(error) => Decision {
            leaf: None,
            event_kind: input.event_kind,
            error: Some(error.code()),
            source_error: error.source_code().map(str::to_owned),
        },
    }
}
