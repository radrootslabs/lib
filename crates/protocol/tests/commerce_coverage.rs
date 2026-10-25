#![cfg(feature = "commerce")]
use radroots_domain::{causal::*, *};
use radroots_protocol::commerce::{CapturedCausalCoverage, DecodedRecord, RecordWireError, decode};
use serde_json::Value;
use std::collections::BTreeMap;

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/commerce_coverage.v1.json")).unwrap()
}
fn scope(value: &Value) -> CausalScope {
    CausalScope::new(
        value["thread"].as_str().unwrap().parse().unwrap(),
        value["order"].as_str().unwrap().parse().unwrap(),
        value["buyer"].as_str().unwrap().parse().unwrap(),
        value["seller"].as_str().unwrap().parse().unwrap(),
    )
    .unwrap()
}
fn records(value: &Value) -> BTreeMap<String, DecodedRecord> {
    value["records"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(name, row)| {
            (
                name.clone(),
                decode(row["wire"].as_str().unwrap().as_bytes()).unwrap(),
            )
        })
        .collect()
}
fn ids(value: &Value) -> Vec<SourceId> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|id| id.as_str().unwrap().parse().unwrap())
        .collect()
}
fn readiness(value: CausalReadiness) -> &'static str {
    match value {
        CausalReadiness::Complete => "complete",
        CausalReadiness::MissingHistory => "missing_history",
        CausalReadiness::Conflicted => "conflicted",
        CausalReadiness::Uninterpreted => "uninterpreted",
    }
}

#[test]
fn independent_fixed_width_scope_transcripts_match_complete_graphs_without_transport_churn() {
    let fixture = fixture();
    let decoded = records(&fixture);
    let scope = scope(&fixture["scope"]);
    for row in fixture["vectors"].as_array().unwrap() {
        let nodes = row["records"]
            .as_array()
            .unwrap()
            .iter()
            .map(|name| {
                let record = &decoded[name.as_str().unwrap()];
                CausalRecord::new(record.record_id(), record.content())
            })
            .collect::<Vec<_>>();
        let capture = CapturedCausalCoverage::capture(
            scope,
            &nodes,
            &ids(&row["external"]),
            &ids(&row["uninterpreted"]),
        )
        .unwrap();
        assert_eq!(
            readiness(capture.analysis().readiness()),
            row["readiness"].as_str().unwrap()
        );
        if row["checkpoint"].is_null() {
            assert!(capture.checkpoint().is_none(), "{}", row["name"]);
        } else {
            let checkpoint = capture.checkpoint().unwrap();
            assert_eq!(
                checkpoint.digest().to_string(),
                row["checkpoint"]["digest"].as_str().unwrap(),
                "{}",
                row["name"]
            );
            assert_eq!(
                checkpoint.sequence().get(),
                row["checkpoint"]["sequence"].as_u64().unwrap()
            );
            assert_eq!(checkpoint.coverage(), Coverage::ScopeSatisfied);
            assert!(checkpoint.missing().is_empty());
        }
        assert_eq!(
            format!("{capture:?}"),
            "CapturedCausalCoverage(<local coverage claims>)"
        );
        let analysis = capture.analysis().clone();
        let checkpoint = capture.checkpoint().cloned();
        let parts = capture.into_parts();
        assert_eq!(parts, (analysis, checkpoint));
    }
    assert_eq!(
        fixture["vectors"][0]["checkpoint"],
        fixture["vectors"][1]["checkpoint"]
    );
    assert_ne!(
        fixture["vectors"][0]["checkpoint"],
        fixture["vectors"][2]["checkpoint"]
    );
}

#[test]
fn claimed_ids_scope_and_limits_are_checked_before_a_coverage_descriptor_is_available() {
    let fixture = fixture();
    let decoded = records(&fixture);
    let scope = scope(&fixture["scope"]);
    let record = &decoded["request"];
    let fake = CausalRecord::new(RecordId::from_bytes([0; 32]), record.content());
    assert_eq!(
        CapturedCausalCoverage::capture(scope, &[fake], &[], &[]).unwrap_err(),
        RecordWireError::IdentityMismatch
    );
    let correct = CausalRecord::new(record.record_id(), record.content());
    assert_eq!(correct.record_id(), record.record_id());
    assert_eq!(correct.content(), record.content());
    let different = CausalScope::new(
        ThreadId::from_bytes([0x55; 32]),
        scope.order_id(),
        scope.buyer(),
        scope.seller(),
    )
    .unwrap();
    assert_eq!(
        CapturedCausalCoverage::capture(different, &[correct], &[], &[]).unwrap_err(),
        RecordWireError::InvalidInput
    );
    assert_eq!(
        CapturedCausalCoverage::capture(scope, &vec![correct; 1001], &[], &[]).unwrap_err(),
        RecordWireError::LimitExceeded
    );
    assert_eq!(
        CapturedCausalCoverage::capture(
            scope,
            &[],
            &vec![SourceId::from_bytes([0; 32]); 8001],
            &[]
        )
        .unwrap_err(),
        RecordWireError::LimitExceeded
    );
}

#[test]
fn scope_binding_remains_explicit_even_for_empty_claims_and_does_not_invent_a_request() {
    let fixture = fixture();
    let scope = scope(&fixture["scope"]);
    let first = CapturedCausalCoverage::capture(scope, &[], &[], &[]).unwrap();
    let different = CausalScope::new(
        scope.thread_id(),
        OrderId::from_bytes([0x55; 32]),
        scope.buyer(),
        scope.seller(),
    )
    .unwrap();
    let second = CapturedCausalCoverage::capture(different, &[], &[], &[]).unwrap();
    assert_ne!(
        first.checkpoint().unwrap().digest(),
        second.checkpoint().unwrap().digest()
    );
    assert_eq!(first.checkpoint().unwrap().sequence().get(), 0);
    assert!(first.analysis().buyer_head().is_none());
    assert!(first.analysis().seller_head().is_none());
}

#[test]
fn initial_request_claims_require_live_exact_sources_and_refuse_observed_withdrawal_or_decline() {
    let fixture = fixture();
    let decoded = records(&fixture);
    let scope = scope(&fixture["scope"]);
    let request = decoded["request"].record_id();
    let base = fixture["vectors"][0].clone();
    let external = ids(&base["external"]);
    let nodes = |names: &[&str]| {
        names
            .iter()
            .map(|name| {
                let record = &decoded[*name];
                CausalRecord::new(record.record_id(), record.content())
            })
            .collect::<Vec<_>>()
    };
    let base_nodes = nodes(&["quote", "request"]);
    let claim = prepare_initial_request(
        scope,
        request,
        &base_nodes,
        &external,
        &[],
        UnixSeconds::new(150).unwrap(),
    )
    .unwrap();
    assert_eq!(claim.request(), request);
    assert_eq!(claim.quote(), Some(decoded["quote"].record_id()));
    assert_eq!(claim.expires_at().get(), 240);
    let CommercePayload::Request(original) = decoded["request"].content().payload() else {
        panic!()
    };
    assert_eq!(claim.terms(), original.terms());
    assert_eq!(format!("{claim:?}"), "InitialRequestClaim(<private data>)");
    for (names, at, expected) in [
        (vec!["quote", "request"], 240, RequestClaimError::Expired),
        (vec!["quote", "request"], 100, RequestClaimError::ClockAhead),
        (
            vec!["quote", "request", "withdrawal"],
            150,
            RequestClaimError::Withdrawn,
        ),
        (
            vec!["quote", "request", "decline"],
            150,
            RequestClaimError::Declined,
        ),
        (
            vec!["quote", "request", "withdrawal", "buyer_fork"],
            150,
            RequestClaimError::Conflicted,
        ),
        (vec!["request"], 150, RequestClaimError::MissingHistory),
    ] {
        assert_eq!(
            prepare_initial_request(
                scope,
                request,
                &nodes(&names),
                &external,
                &[],
                UnixSeconds::new(at).unwrap()
            )
            .unwrap_err(),
            expected
        );
    }
    assert_eq!(
        prepare_initial_request(
            scope,
            decoded["quote"].record_id(),
            &base_nodes,
            &external,
            &[],
            UnixSeconds::new(150).unwrap()
        )
        .unwrap_err(),
        RequestClaimError::WrongRequest
    );
    let unknown = ids(&fixture["vectors"][7]["uninterpreted"]);
    assert_eq!(
        prepare_initial_request(
            scope,
            request,
            &base_nodes,
            &external,
            &unknown,
            UnixSeconds::new(150).unwrap()
        )
        .unwrap_err(),
        RequestClaimError::Uninterpreted
    );
}
