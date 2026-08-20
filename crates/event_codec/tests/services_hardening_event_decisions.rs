#![forbid(unsafe_code)]

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

use radroots_event::{
    id::{MutationId, TradeId},
    trade::{TradeMutationEnvelopeV1, trade_mutation_from_canonical_content},
};
use radroots_event_codec::{
    decode::trade::{RadrootsTradeMutationError, validate_trade_mutation_tags},
    encode::trade::{trade_mutation_event_build, trade_mutation_event_build_with_extra_tags},
};

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
const AUTHORED_CORPUS: &str =
    include_str!("../../../contracts/conformance/vectors/event/authored_operations.v1.json");

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
    assert_eq!(
        registered
            .iter()
            .filter(|kind| **kind == attestation_kind)
            .count(),
        1,
        "implemented attestation kind must be uniquely registered"
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
fn every_trade_mutation_vector_executes_the_public_boundary() {
    let vectors = json(TRADE_VECTORS);
    let mut bases = BTreeMap::new();
    let mut positive_count = 0;
    for vector in vectors["vectors"]
        .as_array()
        .expect("trade vectors")
        .iter()
        .filter(|vector| vector["kind"] == "trade.mutation_index_tags.valid")
    {
        let id = vector["id"].as_str().expect("vector id");
        let envelope = trade_vector_envelope(vector);
        let expected_kind = vector["expected"]["kind"].as_u64().expect("kind") as u32;
        assert_eq!(envelope.mutation_kind().nostr_kind(), expected_kind, "{id}");
        let built = trade_mutation_event_build(envelope).expect("typed trade builder");
        assert_eq!(built.kind, expected_kind, "{id}");
        let parsed = trade_mutation_from_canonical_content(&built.content)
            .expect("builder emits canonical trade content");
        if let Some(tags) = vector["expected"].get("tags") {
            assert_eq!(built.tags, json_tags(tags), "{id}");
        } else {
            assert_eq!(
                tag_semantics(&built.tags),
                vector["expected"]["tag_names_and_semantics"]
                    .as_array()
                    .expect("tag semantics")
                    .iter()
                    .map(|value| value.as_str().expect("semantic").to_owned())
                    .collect::<Vec<_>>(),
                "{id}"
            );
            assert_lineage_tags_match_vector(&built.tags, &vector["input"]);
        }
        validate_trade_mutation_tags(&parsed, &built.tags).expect("positive vector");
        assert!(bases.insert(id.to_owned(), (parsed, built.tags)).is_none());
        positive_count += 1;
    }
    assert_eq!(positive_count, 5);

    let proposal = bases
        .get("trade_mutation_index_proposal_002")
        .expect("proposal base")
        .0
        .clone();
    for vector in vectors["vectors"]
        .as_array()
        .expect("trade vectors")
        .iter()
        .filter(|vector| vector["kind"] == "trade.mutation_index_tags.invalid")
    {
        let id = vector["id"].as_str().expect("vector id");
        let expected = vector["expected"]["error_code"]
            .as_str()
            .expect("error code");
        let input = &vector["input"];
        let actual = if let Some(extra_tags) = input.get("builder_extra_tags") {
            assert_eq!(vector["expected"]["layer"], "builder", "{id}");
            trade_mutation_event_build_with_extra_tags(proposal.clone(), &json_tags(extra_tags))
                .expect_err("builder negative must fail")
        } else {
            assert_eq!(vector["expected"]["layer"], "wire", "{id}");
            let base = input["base"].as_str().expect("wire negative base");
            let (envelope, mut tags) = bases.get(base).expect("known positive base").clone();
            if let Some(pattern) = input.get("remove_tag") {
                let before = tags.len();
                tags.retain(|tag| !tag_matches_pattern(tag, pattern));
                assert_eq!(tags.len() + 1, before, "{id}");
            }
            if let Some(patterns) = input.get("remove_tags") {
                for pattern in patterns.as_array().expect("remove tag patterns") {
                    let before = tags.len();
                    tags.retain(|tag| !tag_matches_pattern(tag, pattern));
                    assert!(tags.len() < before, "{id}");
                }
            }
            if let Some(tag) = input.get("append_tag") {
                tags.push(json_tag(tag));
            }
            if let Some(indexes) = input.get("swap_tag_indexes") {
                let indexes = indexes.as_array().expect("swap indexes");
                tags.swap(
                    indexes[0].as_u64().expect("left index") as usize,
                    indexes[1].as_u64().expect("right index") as usize,
                );
            }
            if let Some(replacement) = input.get("replace_tag") {
                let index = replacement["index"].as_u64().expect("replace index") as usize;
                tags[index] = json_tag(&replacement["tag"]);
            }
            validate_trade_mutation_tags(&envelope, &tags).expect_err("negative vector must fail")
        };
        assert_eq!(actual.code(), expected, "{id}");
    }
}

fn json_tag(value: &Value) -> Vec<String> {
    value
        .as_array()
        .expect("tag array")
        .iter()
        .map(|value| value.as_str().expect("tag value").to_owned())
        .collect()
}

fn json_tags(value: &Value) -> Vec<Vec<String>> {
    value
        .as_array()
        .expect("tag list")
        .iter()
        .map(json_tag)
        .collect()
}

fn tag_matches_pattern(tag: &[String], pattern: &Value) -> bool {
    let pattern = json_tag(pattern);
    if pattern.first().map(String::as_str) == Some("x") && pattern.len() == 2 {
        tag.first() == pattern.first() && tag.get(2) == pattern.get(1)
    } else {
        tag == pattern
    }
}

fn tag_semantics(tags: &[Vec<String>]) -> Vec<String> {
    let mut party = 0;
    tags.iter()
        .map(|tag| match tag.first().map(String::as_str) {
            Some("contract") => "contract".to_owned(),
            Some("d") => "d:trade".to_owned(),
            Some("x") => format!("x:{}", tag.get(2).expect("x marker")),
            Some("p") => {
                party += 1;
                format!(
                    "p:{}",
                    if party == 1 {
                        "buyer-first"
                    } else {
                        "seller-second"
                    }
                )
            }
            _ => panic!("unexpected structural tag"),
        })
        .collect()
}

fn trade_from_corpus(contract_id: &str) -> TradeMutationEnvelopeV1 {
    let corpus = json(AUTHORED_CORPUS);
    let content = corpus["vectors"]
        .as_array()
        .expect("authored vectors")
        .iter()
        .find(|vector| vector["input"]["contract_id"] == contract_id)
        .and_then(|vector| vector["expected"]["content"].as_str())
        .expect("typed trade content");
    trade_mutation_from_canonical_content(content).expect("canonical trade mutation")
}

fn trade_vector_envelope(vector: &Value) -> TradeMutationEnvelopeV1 {
    let input = &vector["input"];
    let contract_id = input["contract_id"].as_str().expect("contract id");
    let mut envelope = trade_from_corpus(contract_id);
    envelope.mutation_id = None;
    envelope.root_mutation_id = input
        .get("root_mutation_id")
        .and_then(Value::as_str)
        .map(|value| MutationId::parse(value).expect("root mutation id"));
    envelope.parent_mutation_ids = input
        .get("parent_mutation_ids")
        .and_then(Value::as_array)
        .map(|parents| {
            parents
                .iter()
                .map(|value| {
                    MutationId::parse(value.as_str().expect("parent mutation id"))
                        .expect("parent mutation id")
                })
                .collect()
        })
        .unwrap_or_default();
    if let Some(trade_id) = input.get("trade_id").and_then(Value::as_str) {
        envelope.trade_id = TradeId::parse(trade_id).expect("trade id");
    }
    if let Some(buyer) = input.get("buyer_pubkey").and_then(Value::as_str) {
        envelope.buyer_pubkey = radroots_identity::PublicKey::from_hex(buyer).expect("buyer key");
        envelope.author_pubkey = envelope.buyer_pubkey;
    }
    if let Some(seller) = input.get("seller_pubkey").and_then(Value::as_str) {
        envelope.seller_pubkey =
            radroots_identity::PublicKey::from_hex(seller).expect("seller key");
        envelope.counterparty_pubkey = envelope.seller_pubkey;
    }
    envelope
}

fn assert_lineage_tags_match_vector(tags: &[Vec<String>], input: &Value) {
    let root = tags
        .iter()
        .find(|tag| tag.get(2).map(String::as_str) == Some("root"))
        .map(|tag| tag[1].as_str());
    assert_eq!(root, input.get("root_mutation_id").and_then(Value::as_str));
    let parents = tags
        .iter()
        .filter(|tag| tag.get(2).map(String::as_str) == Some("parent"))
        .map(|tag| tag[1].as_str())
        .collect::<Vec<_>>();
    let expected: Vec<&str> = input
        .get("parent_mutation_ids")
        .and_then(Value::as_array)
        .map(|values| values.iter().map(|value| value.as_str().unwrap()).collect())
        .unwrap_or_default();
    assert_eq!(parents, expected);
}

fn synthetic_all_fields_envelope() -> TradeMutationEnvelopeV1 {
    trade_vector_envelope(&json(TRADE_VECTORS)["vectors"][0])
}

#[test]
fn additional_trade_mutation_shape_permutations_and_bounds_fail_closed() {
    let built = trade_mutation_event_build(synthetic_all_fields_envelope()).expect("trade builder");
    let envelope = trade_mutation_from_canonical_content(&built.content).expect("trade content");
    let tags = built.tags;

    let mut malformed_x = tags.clone();
    malformed_x[2].pop();
    assert_eq!(
        validate_trade_mutation_tags(&envelope, &malformed_x).unwrap_err(),
        RadrootsTradeMutationError::InvalidTagShape
    );

    let mut unknown_marker = tags.clone();
    unknown_marker[2][2] = "unknown".to_owned();
    assert_eq!(
        validate_trade_mutation_tags(&envelope, &unknown_marker).unwrap_err(),
        RadrootsTradeMutationError::InvalidTagShape
    );

    let mut duplicate_mutation = tags.clone();
    duplicate_mutation.insert(3, duplicate_mutation[2].clone());
    assert_eq!(
        validate_trade_mutation_tags(&envelope, &duplicate_mutation).unwrap_err(),
        RadrootsTradeMutationError::InvalidTagShape
    );

    let mut duplicate_root = tags.clone();
    duplicate_root.insert(4, duplicate_root[3].clone());
    assert_eq!(
        validate_trade_mutation_tags(&envelope, &duplicate_root).unwrap_err(),
        RadrootsTradeMutationError::InvalidTagShape
    );

    let mut five_parents = tags.clone();
    for value in ["6", "7", "8"] {
        five_parents.insert(
            five_parents.len() - 2,
            vec!["x".to_owned(), value.repeat(64), "parent".to_owned()],
        );
    }
    assert_eq!(
        validate_trade_mutation_tags(&envelope, &five_parents).unwrap_err(),
        RadrootsTradeMutationError::InvalidTagShape
    );

    let mut duplicate_parent = tags.clone();
    duplicate_parent.insert(6, duplicate_parent[5].clone());
    assert_eq!(
        validate_trade_mutation_tags(&envelope, &duplicate_parent).unwrap_err(),
        RadrootsTradeMutationError::NoncanonicalParentOrder
    );

    let mut uppercase_identifier = tags.clone();
    uppercase_identifier[2][1] = "A".repeat(64);
    assert_eq!(
        validate_trade_mutation_tags(&envelope, &uppercase_identifier).unwrap_err(),
        RadrootsTradeMutationError::InvalidIdentifier
    );

    let mut unknown_tag = tags.clone();
    unknown_tag.push(vec!["t".to_owned(), "trade".to_owned()]);
    assert_eq!(
        validate_trade_mutation_tags(&envelope, &unknown_tag).unwrap_err(),
        RadrootsTradeMutationError::UnexpectedTag
    );

    let mut missing_party = tags;
    missing_party.pop();
    assert_eq!(
        validate_trade_mutation_tags(&envelope, &missing_party).unwrap_err(),
        RadrootsTradeMutationError::InvalidTagShape
    );

    let oversized = vec![vec!["t".to_owned(), "trade".to_owned()]; 10_000];
    assert_eq!(
        validate_trade_mutation_tags(&envelope, &oversized).unwrap_err(),
        RadrootsTradeMutationError::InvalidTagShape
    );

    assert_eq!(
        trade_mutation_event_build_with_extra_tags(
            trade_from_corpus("radroots.trade.proposal.v1"),
            &[vec!["t".to_owned(), "trade".to_owned()]],
        )
        .unwrap_err(),
        RadrootsTradeMutationError::UnexpectedTag
    );
    assert_eq!(
        trade_mutation_event_build_with_extra_tags(
            trade_from_corpus("radroots.trade.proposal.v1"),
            &[
                vec!["t".to_owned(), "trade".to_owned()],
                vec!["p".to_owned(), "0".repeat(64)],
            ],
        )
        .unwrap_err(),
        RadrootsTradeMutationError::CallerStructuralTagForbidden
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
