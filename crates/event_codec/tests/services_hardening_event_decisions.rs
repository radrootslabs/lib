#![forbid(unsafe_code)]

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

const DECISION: &str =
    include_str!("../../../contracts/architecture/decisions/services_hardening_events.v1.json");
const REGISTRY: &str =
    include_str!("../../../contracts/event_store/event_contract_registry_v7.inventory.json");
const TRADE_VECTORS: &str = include_str!(
    "../../../contracts/conformance/vectors/trade/mutation_index_tags_decision.v1.json"
);
const RHI_VECTORS: &str = include_str!(
    "../../../contracts/conformance/vectors/rhi/evidence_attestation_decision.v1.json"
);

fn json(source: &str) -> Value {
    serde_json::from_str(source).expect("services-hardening machine contract must be valid JSON")
}

fn strings(value: &Value, field: &str) -> Vec<String> {
    value[field]
        .as_array()
        .expect("field must be an array")
        .iter()
        .map(|entry| entry.as_str().expect("entry must be a string").to_owned())
        .collect()
}

fn error_codes(vectors: &Value) -> BTreeSet<String> {
    vectors["vectors"]
        .as_array()
        .expect("vector list")
        .iter()
        .filter_map(|vector| vector["expected"]["error_code"].as_str())
        .map(str::to_owned)
        .collect()
}

fn verify_attestation_digest_vector(vector: &Value) -> String {
    let canonical_payload = vector["expected"]["canonical_statement_payload_utf8"]
        .as_str()
        .expect("canonical statement payload");
    assert_eq!(
        serde_json::to_string(&vector["input"]["statement_payload"]).unwrap(),
        canonical_payload
    );
    let mut hasher = Sha256::new();
    hasher.update(b"radroots:rhi-evidence-attestation-statement:v1\0");
    hasher.update(canonical_payload.as_bytes());
    let digest = hex::encode(hasher.finalize());
    assert_eq!(vector["expected"]["statement_digest"], digest);
    assert_eq!(vector["expected"]["report_id"], digest);
    let canonical_event = vector["expected"]["canonical_event_content_utf8"]
        .as_str()
        .expect("canonical event content");
    let mut event_content = json(canonical_event);
    assert_eq!(event_content["report_id"], digest);
    assert_eq!(event_content["statement_digest"], digest);
    let event_object = event_content.as_object_mut().expect("report object");
    event_object.remove("report_id");
    event_object.remove("statement_digest");
    assert_eq!(
        serde_json::to_string(&event_content).unwrap(),
        canonical_payload
    );
    digest
}

#[test]
fn services_hardening_event_decision_reserves_unique_exact_kinds() {
    let decision = json(DECISION);
    assert_eq!(
        decision["schema"],
        "radroots.services-hardening.event-decisions.v1"
    );
    assert_eq!(decision["decision_state"], "reserved_preimplementation");

    let expected_trade = BTreeSet::from([3470_u64, 3471, 3472, 3473, 3474]);
    let trade = decision["trade_mutation"]["event_kinds"]
        .as_array()
        .expect("trade event kinds");
    let actual_trade = trade
        .iter()
        .map(|entry| entry["kind"].as_u64().expect("numeric kind"))
        .collect::<BTreeSet<_>>();
    assert_eq!(actual_trade, expected_trade);

    let registry = json(REGISTRY);
    let registered = registry["kind_contracts"]
        .as_array()
        .expect("registry kind contracts")
        .iter()
        .map(|entry| entry["kind"].as_u64().expect("registered numeric kind"))
        .collect::<Vec<_>>();
    for kind in expected_trade {
        assert!(registered.contains(&kind), "trade kind {kind} must exist");
    }
    let attestation_kind = decision["rhi_attestation"]["kind"]
        .as_u64()
        .expect("attestation kind");
    assert_eq!(attestation_kind, 3441);
    assert!(
        !registered.contains(&attestation_kind),
        "reserved attestation kind must not collide with a registered kind"
    );
}

#[test]
fn services_hardening_trade_tag_cardinality_and_query_contract_is_exact() {
    let decision = json(DECISION);
    let trade = &decision["trade_mutation"];
    assert_eq!(trade["event_class"], "regular_immutable");
    assert_eq!(
        strings(trade, "canonical_tag_order"),
        [
            "contract",
            "d:trade",
            "x:mutation",
            "x:root",
            "x:parent_sorted",
            "p:buyer",
            "p:seller",
        ]
    );
    let tags = trade["tags"].as_array().expect("trade tags");
    assert_eq!(tags.len(), 7);
    assert_eq!(tags[1]["cardinality"], "exactly_one");
    assert_eq!(tags[2]["cardinality"], "exactly_one");
    assert_eq!(
        tags[3]["cardinality"],
        "proposal_zero_other_mutations_exactly_one"
    );
    assert_eq!(
        tags[4]["cardinality"],
        "proposal_zero_other_mutations_one_to_four_sorted_unique"
    );
    assert_eq!(tags[5]["cardinality"], "first_of_exactly_two");
    assert_eq!(tags[6]["cardinality"], "second_of_exactly_two");
    assert_eq!(
        trade["validation"]["legacy_contract_d_p_e_shape_accepted"],
        false
    );

    let vectors = json(TRADE_VECTORS);
    let first = &vectors["vectors"][0];
    assert_eq!(first["expected"]["kind"], 3472);
    assert_eq!(
        first["expected"]["tags"]
            .as_array()
            .expect("exact tags")
            .len(),
        8
    );
    assert_eq!(first["expected"]["tags"][1].as_array().unwrap().len(), 2);
    assert_eq!(first["expected"]["tags"][6].as_array().unwrap().len(), 2);
    assert_eq!(first["expected"]["tags"][7].as_array().unwrap().len(), 2);
    let kinds = vectors["vectors"]
        .as_array()
        .expect("trade decision vectors")
        .iter()
        .filter_map(|vector| vector["expected"]["kind"].as_u64())
        .collect::<BTreeSet<_>>();
    assert_eq!(kinds, BTreeSet::from([3470, 3471, 3472, 3473, 3474]));
    assert_eq!(
        error_codes(&vectors),
        BTreeSet::from([
            "caller_structural_tag_forbidden".to_owned(),
            "duplicate_trade_tag".to_owned(),
            "legacy_parent_event_tag".to_owned(),
            "missing_parent_tag".to_owned(),
            "missing_mutation_tag".to_owned(),
            "missing_root_tag".to_owned(),
            "noncanonical_parent_order".to_owned(),
            "party_tag_order_mismatch".to_owned(),
            "unexpected_parent_tag".to_owned(),
            "unexpected_root_tag".to_owned(),
        ])
    );
}

#[test]
fn services_hardening_attestation_is_immutable_and_fully_bound() {
    let decision = json(DECISION);
    let attestation = &decision["rhi_attestation"];
    assert_eq!(attestation["kind"], 3441);
    assert_eq!(attestation["event_class"], "regular_immutable");
    assert_eq!(attestation["replaceability"], "none");
    assert_eq!(attestation["content_encoding"], "RFC8785_JCS_JSON_UTF8");
    assert_eq!(
        attestation["fixed_values"]["attestation_method"],
        "signed_evidence_snapshot"
    );
    assert_eq!(
        strings(attestation, "canonical_tag_order"),
        [
            "contract",
            "d:trade",
            "x:claim",
            "x:statement",
            "t:outcome",
            "x:supersedes_report",
            "e:supersedes_event",
        ]
    );
    assert_eq!(
        attestation["supersession"]["requires_both_references_or_neither"],
        true
    );
    assert_eq!(
        attestation["supersession"]["report_id_equals_statement_digest"],
        true
    );
    assert_eq!(
        attestation["supersession"]["relay_arrival_order_authoritative"],
        false
    );

    let vectors = json(RHI_VECTORS);
    let positive = &vectors["vectors"][0];
    assert_eq!(positive["expected"]["kind"], 3441);
    verify_attestation_digest_vector(positive);
    assert_eq!(positive["expected"]["tags"][1].as_array().unwrap().len(), 2);
    assert_eq!(positive["expected"]["tags"][4].as_array().unwrap().len(), 2);
    assert_eq!(
        vectors["vectors"][1]["expected"]["mutates_prior_report"],
        false
    );
    verify_attestation_digest_vector(&vectors["vectors"][1]);
    assert_eq!(
        vectors["vectors"][1]["expected"]["tags"][6]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        error_codes(&vectors),
        BTreeSet::from([
            "caller_structural_tag_forbidden".to_owned(),
            "duplicate_statement_tag".to_owned(),
            "duplicate_trade_tag".to_owned(),
            "incomplete_supersession_reference".to_owned(),
            "invalid_attestation_kind".to_owned(),
            "invalid_outcome".to_owned(),
            "issuer_author_mismatch".to_owned(),
            "missing_claim_tag".to_owned(),
            "noncanonical_report_content".to_owned(),
            "stale_trade_generation".to_owned(),
            "statement_digest_mismatch".to_owned(),
        ])
    );
    let stale = &vectors["vectors"][11];
    assert_eq!(stale["expected"]["layer"], "admission");
}
