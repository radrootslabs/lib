#![cfg(all(feature = "knowledge", feature = "contract-manifest"))]

use std::collections::BTreeSet;

use radroots_event::contract::VERSION;
use radroots_event::contract::{
    EventClass, RADROOTS_EVENT_CONTRACT_REGISTRY_VERSION, all_event_contracts,
};
use radroots_event::envelope::kind::{
    KIND_CONTRIBUTION_ATTESTATION, KIND_KNOWLEDGE_CHANGE_PROPOSAL, KIND_KNOWLEDGE_CLAIM,
    KIND_KNOWLEDGE_FIELD_REPORT, KIND_KNOWLEDGE_RELATION, KIND_KNOWLEDGE_REVIEW,
};
use radroots_event::knowledge::{
    RADROOTS_KNOWLEDGE_CLAIM_SCHEMA, RADROOTS_KNOWLEDGE_FIELD_REPORT_SCHEMA,
};
use radroots_event_codec::manifest::{
    RADROOTS_KNOWLEDGE_CONTRACT_MANIFEST_SCHEMA_VERSION, RadrootsKnowledgeContractManifest,
    contract_manifest_json, contract_manifest_sha256, knowledge_contract_manifest,
};
use radroots_test_fixtures::knowledge::RADROOTS_KNOWLEDGE_VALID_CONTRACT_IDS;

const MANIFEST_JSON: &str =
    include_str!("../../../contracts/knowledge/knowledge_event_contract_manifest.v2.json");
const MANIFEST_SHA256: &str =
    include_str!("../../../contracts/knowledge/knowledge_event_contract_manifest.v2.sha256");
const KNOWLEDGE_PUBLIC_SURFACE_VECTOR: &str =
    include_str!("../../../contracts/conformance/vectors/knowledge/public_surface.v1.json");

const MVP_SUPPORT_CONTRACT_IDS: &[&str] = &[
    "radroots.wiki.article.v1",
    "radroots.wiki.redirect.v1",
    "radroots.wiki.merge_request.v1",
    "radroots.knowledge.source.v1",
    "radroots.knowledge.claim.v1",
    "radroots.knowledge.relation.v1",
    "radroots.knowledge.review.v1",
    "radroots.knowledge.field_report.v1",
];

const BETA_CONTRACT_IDS: &[&str] = &[
    "radroots.knowledge.evidence_bounty.v1",
    "radroots.knowledge.change_proposal.v1",
    "radroots.knowledge.contribution_attestation.v1",
];

#[test]
fn knowledge_manifest_is_deterministic_and_matches_artifacts() {
    let first = contract_manifest_json().unwrap();
    let second = contract_manifest_json().unwrap();
    assert_eq!(first, second);
    assert_eq!(first, MANIFEST_JSON);

    let first_hash = contract_manifest_sha256().unwrap();
    let second_hash = contract_manifest_sha256().unwrap();
    assert_eq!(first_hash, second_hash);
    assert_eq!(format!("{first_hash}\n"), MANIFEST_SHA256);
}

#[test]
fn knowledge_manifest_covers_required_fields_for_every_contract() {
    let manifest: RadrootsKnowledgeContractManifest = serde_json::from_str(MANIFEST_JSON).unwrap();
    assert_eq!(
        manifest.schema_version,
        RADROOTS_KNOWLEDGE_CONTRACT_MANIFEST_SCHEMA_VERSION
    );
    assert_eq!(
        manifest.registry_version,
        RADROOTS_EVENT_CONTRACT_REGISTRY_VERSION
    );
    assert_eq!(manifest.radroots_event_version, VERSION);
    assert_eq!(
        manifest.radroots_event_codec_version,
        env!("CARGO_PKG_VERSION")
    );
    assert_eq!(manifest.contract_count, manifest.contracts.len());

    let mut sorted = manifest
        .contracts
        .iter()
        .map(|contract| contract.contract_id.as_str())
        .collect::<Vec<_>>();
    let original = sorted.clone();
    sorted.sort_unstable();
    assert_eq!(original, sorted);

    for contract in &manifest.contracts {
        assert!(!contract.contract_id.trim().is_empty());
        assert!(contract.kind > 0);
        assert!(!contract.class.trim().is_empty());
        assert!(!contract.standard.trim().is_empty());
        assert_eq!(contract.stability, "experimental");
        assert!(!contract.privacy.trim().is_empty());
        assert!(!contract.content_schema.trim().is_empty());
        assert!(!contract.payload_type.trim().is_empty());
        assert!(!contract.discriminators.is_empty());
        assert!(
            !contract.codec_support.verified_decode_requires_nostr
                || contract.codec_support.verified_decode
        );
        assert!(contract.codec_support.encode);
        assert!(contract.codec_support.decode);
        assert!(contract.codec_support.contract_validation);
        assert!(contract.wasm_verified_decode_support);
        assert!(!contract.deprecated);
        assert!(contract.replaced_by.is_none());
        assert!(!contract.introduced_at.trim().is_empty());
    }

    for contract_id in MVP_SUPPORT_CONTRACT_IDS {
        let contract = manifest
            .contracts
            .iter()
            .find(|contract| contract.contract_id == *contract_id)
            .unwrap();
        assert!(contract.sdk_builder_support, "{contract_id}");
        assert!(contract.sdk_draft_support, "{contract_id}");
        assert!(contract.wasm_tag_builder_support, "{contract_id}");
    }

    for contract_id in BETA_CONTRACT_IDS {
        let contract = manifest
            .contracts
            .iter()
            .find(|contract| contract.contract_id == *contract_id)
            .unwrap();
        assert!(!contract.sdk_builder_support, "{contract_id}");
        assert!(!contract.sdk_draft_support, "{contract_id}");
        assert!(!contract.wasm_tag_builder_support, "{contract_id}");
    }

    let merge_request = manifest
        .contracts
        .iter()
        .find(|contract| contract.contract_id == "radroots.wiki.merge_request.v1")
        .unwrap();
    assert_eq!(merge_request.content_schema, "plain_text");

    let manifest_contracts = manifest
        .contracts
        .iter()
        .map(|contract| contract.contract_id.as_str())
        .collect::<BTreeSet<_>>();
    let fixture_contracts = RADROOTS_KNOWLEDGE_VALID_CONTRACT_IDS
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    assert_eq!(manifest_contracts, fixture_contracts);
}

#[test]
fn wiki_article_manifest_includes_reference_tag_contracts() {
    let manifest = knowledge_contract_manifest();
    let article = manifest
        .contracts
        .iter()
        .find(|contract| contract.contract_id == "radroots.wiki.article.v1")
        .unwrap();

    let expected = [
        ("d", "required_one", "identifier", "d_tag", true),
        ("title", "optional_one", "title", "text", false),
        ("summary", "optional_one", "summary", "text", false),
        (
            "published_at",
            "optional_one",
            "published_at",
            "unix_timestamp",
            false,
        ),
        ("t", "optional_many", "topic", "text", true),
        ("source", "optional_many", "source", "event_pointer", false),
        (
            "a",
            "optional_many",
            "addressable_coordinate",
            "addressable_coordinate",
            true,
        ),
        ("e", "optional_many", "event_pointer", "event_id", true),
    ];

    assert_eq!(article.tag_contracts.len(), expected.len());
    for (name, cardinality, semantic, value_type, relay_indexed) in expected {
        let tag = article
            .tag_contracts
            .iter()
            .find(|tag| tag.name == name)
            .unwrap();
        assert_eq!(tag.cardinality, cardinality);
        assert_eq!(tag.semantic, semantic);
        assert_eq!(tag.value_type, value_type);
        assert_eq!(tag.relay_indexed, relay_indexed);
    }
}

#[test]
fn knowledge_manifest_preserves_regular_immutable_classification() {
    let manifest = knowledge_contract_manifest();
    for (contract_id, kind) in [
        (RADROOTS_KNOWLEDGE_CLAIM_SCHEMA, KIND_KNOWLEDGE_CLAIM),
        ("radroots.knowledge.relation.v1", KIND_KNOWLEDGE_RELATION),
        ("radroots.knowledge.review.v1", KIND_KNOWLEDGE_REVIEW),
        (
            RADROOTS_KNOWLEDGE_FIELD_REPORT_SCHEMA,
            KIND_KNOWLEDGE_FIELD_REPORT,
        ),
        (
            "radroots.knowledge.change_proposal.v1",
            KIND_KNOWLEDGE_CHANGE_PROPOSAL,
        ),
        (
            "radroots.knowledge.contribution_attestation.v1",
            KIND_CONTRIBUTION_ATTESTATION,
        ),
    ] {
        let manifest_entry = manifest
            .contracts
            .iter()
            .find(|contract| contract.contract_id == contract_id)
            .unwrap();
        let registry_entry = all_event_contracts()
            .iter()
            .find(|contract| contract.id == contract_id)
            .unwrap();
        assert_eq!(manifest_entry.kind, kind);
        assert_eq!(manifest_entry.class, "regular");
        assert_eq!(registry_entry.class, EventClass::Regular);
    }
}

#[test]
fn knowledge_manifest_public_surface_vector_stays_generalized() {
    let value: serde_json::Value = serde_json::from_str(KNOWLEDGE_PUBLIC_SURFACE_VECTOR).unwrap();
    assert_eq!(value["suite"], "knowledge_public_surface");
    let ids = value["vectors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|vector| vector["id"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        ids,
        [
            "knowledge_claim_public_surface_valid_002",
            "knowledge_field_report_public_surface_valid_004",
            "knowledge_review_public_surface_valid_003",
            "knowledge_source_public_surface_valid_001",
            "knowledge_claim_sdk_builder_public_surface_valid_006",
            "wiki_article_public_surface_valid_005",
        ]
        .into_iter()
        .collect()
    );
    for forbidden in ["dao", "token", "reputation", "score", "canon_synthesis"] {
        assert!(!KNOWLEDGE_PUBLIC_SURFACE_VECTOR.contains(forbidden));
    }
}
