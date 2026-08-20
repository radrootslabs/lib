#![cfg(feature = "json")]

use std::error::Error as _;

use nostr::{EventBuilder, Keys, Kind, Tag, Timestamp};
use radroots_event::{
    admission::RawEvent,
    envelope::{EventEnvelope, EventEnvelopeParts},
    id::EventId,
    trade::canonical_jcs_value,
};
use radroots_event_codec::{
    decode::rhi::{
        RadrootsRhiEvidenceAttestationError, RadrootsRhiEvidenceAttestationV1,
        rhi_evidence_attestation_from_event, rhi_evidence_attestation_from_verified_event,
        validate_rhi_evidence_attestation_supersession, validate_rhi_evidence_attestation_tags,
    },
    encode::rhi::{
        RADROOTS_RHI_EVIDENCE_ATTESTATION_MAXIMUM_BYTES, rhi_evidence_attestation_event_build,
        rhi_evidence_attestation_event_build_with_extra_tags,
    },
    verify::Nip01SignatureVerifier,
};
use serde_json::Value;
use sha2::{Digest as _, Sha256};

const VECTORS: &str = include_str!(
    "../../../contracts/conformance/vectors/rhi/evidence_attestation_decision.v1.json"
);
const DIGEST_DOMAIN: &[u8] = b"radroots:rhi-evidence-attestation-statement:v1\0";

fn vectors() -> Value {
    serde_json::from_str(VECTORS).expect("RHI conformance vectors")
}

fn vector<'a>(vectors: &'a Value, id: &str) -> &'a Value {
    vectors["vectors"]
        .as_array()
        .expect("vector list")
        .iter()
        .find(|vector| vector["id"] == id)
        .unwrap_or_else(|| panic!("missing vector {id}"))
}

fn positive_content(vector: &Value) -> &str {
    vector["expected"]["canonical_event_content_utf8"]
        .as_str()
        .expect("canonical event content")
}

fn tags(vector: &Value) -> Vec<Vec<String>> {
    vector["expected"]["tags"]
        .as_array()
        .expect("tag list")
        .iter()
        .map(|tag| {
            tag.as_array()
                .expect("tag")
                .iter()
                .map(|value| value.as_str().expect("tag value").to_owned())
                .collect()
        })
        .collect()
}

fn structural_event(
    kind: u32,
    author: &str,
    tags: Vec<Vec<String>>,
    content: String,
) -> EventEnvelope {
    EventEnvelope::new(EventEnvelopeParts {
        id: "0".repeat(64),
        author: author.to_owned(),
        created_at: 1_800_000_000,
        kind,
        tags,
        content,
        sig: "1".repeat(128),
    })
    .expect("structurally valid event")
}

fn canonical_report_from_statement(statement: Value) -> String {
    let payload = canonical_jcs_value(&statement).expect("canonical statement");
    let mut hasher = Sha256::new();
    hasher.update(DIGEST_DOMAIN);
    hasher.update(payload.as_bytes());
    let digest = hex::encode(hasher.finalize());
    let mut report = statement;
    let object = report.as_object_mut().expect("statement object");
    object.insert("report_id".to_owned(), Value::String(digest.clone()));
    object.insert("statement_digest".to_owned(), Value::String(digest));
    canonical_jcs_value(&report).expect("canonical report")
}

#[test]
fn all_frozen_rhi_vectors_execute_their_governed_boundaries() {
    let vectors = vectors();
    let current_vector = vector(&vectors, "rhi_evidence_attestation_current_001");
    let superseding_vector = vector(&vectors, "rhi_evidence_attestation_superseding_002");
    let current =
        RadrootsRhiEvidenceAttestationV1::from_canonical_content(positive_content(current_vector))
            .expect("current report");
    let superseding = RadrootsRhiEvidenceAttestationV1::from_canonical_content(positive_content(
        superseding_vector,
    ))
    .expect("superseding report");

    for (fixture, attestation) in [
        (current_vector, &current),
        (superseding_vector, &superseding),
    ] {
        let built = rhi_evidence_attestation_event_build(attestation);
        assert_eq!(built.kind, 3441);
        assert_eq!(built.tags, tags(fixture));
        assert_eq!(built.content, positive_content(fixture));
        validate_rhi_evidence_attestation_tags(attestation, &built.tags)
            .expect("independent tag validator");
        let parsed = rhi_evidence_attestation_from_event(&structural_event(
            built.kind,
            &attestation.issuer().to_hex(),
            built.tags,
            built.content,
        ))
        .expect("structural parser");
        assert_eq!(&parsed, attestation);
    }

    let negative_ids = vectors["vectors"]
        .as_array()
        .expect("vectors")
        .iter()
        .filter(|vector| vector["kind"] == "rhi.evidence_attestation.invalid")
        .map(|vector| vector["id"].as_str().expect("vector id"))
        .collect::<Vec<_>>();
    assert_eq!(negative_ids.len(), 11);

    let current_parts = rhi_evidence_attestation_event_build(&current);
    let error = rhi_evidence_attestation_from_event(&structural_event(
        3440,
        &current.issuer().to_hex(),
        current_parts.tags.clone(),
        current_parts.content.clone(),
    ))
    .expect_err("wrong kind");
    assert_vector_error(&vectors, "rhi_evidence_attestation_wrong_kind_003", error);

    let error = rhi_evidence_attestation_from_event(&structural_event(
        current_parts.kind,
        "e0266e3cfb0d2886f91c73f5f868f3b98273713e5fcd97c081663f5518a4b3af",
        current_parts.tags.clone(),
        current_parts.content.clone(),
    ))
    .expect_err("wrong author");
    assert_vector_error(&vectors, "rhi_evidence_attestation_wrong_author_004", error);

    let error = rhi_evidence_attestation_from_event(&structural_event(
        current_parts.kind,
        &current.issuer().to_hex(),
        current_parts.tags.clone(),
        format!("{} ", current_parts.content),
    ))
    .expect_err("noncanonical content");
    assert_vector_error(
        &vectors,
        "rhi_evidence_attestation_noncanonical_content_005",
        error,
    );

    let mut digest_mismatch: Value = serde_json::from_str(&current_parts.content).unwrap();
    digest_mismatch["statement_digest"] = Value::String("0".repeat(64));
    let error = RadrootsRhiEvidenceAttestationV1::from_canonical_content(
        canonical_jcs_value(&digest_mismatch).unwrap(),
    )
    .expect_err("digest mismatch");
    assert_vector_error(
        &vectors,
        "rhi_evidence_attestation_digest_mismatch_006",
        error,
    );

    let mut unknown_outcome: Value = serde_json::from_str(&current_parts.content).unwrap();
    unknown_outcome["outcome"] = Value::String("complete".to_owned());
    let error = RadrootsRhiEvidenceAttestationV1::from_canonical_content(
        canonical_jcs_value(&unknown_outcome).unwrap(),
    )
    .expect_err("unknown outcome");
    assert_vector_error(
        &vectors,
        "rhi_evidence_attestation_unknown_outcome_007",
        error,
    );

    let mut missing_claim = current_parts.tags.clone();
    missing_claim.retain(|tag| tag.get(2).map(String::as_str) != Some("claim"));
    assert_vector_error(
        &vectors,
        "rhi_evidence_attestation_missing_claim_tag_008",
        validate_rhi_evidence_attestation_tags(&current, &missing_claim).unwrap_err(),
    );

    let mut duplicate_trade = current_parts.tags.clone();
    duplicate_trade.push(vec!["d".to_owned(), "9".repeat(32)]);
    assert_vector_error(
        &vectors,
        "rhi_evidence_attestation_duplicate_trade_tag_009",
        validate_rhi_evidence_attestation_tags(&current, &duplicate_trade).unwrap_err(),
    );

    let mut duplicate_statement = current_parts.tags.clone();
    duplicate_statement.push(vec!["x".to_owned(), "9".repeat(64), "statement".to_owned()]);
    assert_vector_error(
        &vectors,
        "rhi_evidence_attestation_duplicate_statement_tag_010",
        validate_rhi_evidence_attestation_tags(&current, &duplicate_statement).unwrap_err(),
    );

    let mut incomplete: Value = serde_json::from_str(&current_parts.content).unwrap();
    incomplete["supersedes_report_id"] = Value::String("7".repeat(64));
    let error = RadrootsRhiEvidenceAttestationV1::from_canonical_content(
        canonical_jcs_value(&incomplete).unwrap(),
    )
    .expect_err("incomplete supersession");
    assert_vector_error(
        &vectors,
        "rhi_evidence_attestation_incomplete_supersession_011",
        error,
    );

    let mut current_statement = current_vector["input"]["statement_payload"].clone();
    current_statement["trade_generation"] = Value::from(8);
    current_statement["observed_at_unix_s"] = Value::from(1_800_000_100_u64);
    let ordered_current = RadrootsRhiEvidenceAttestationV1::from_canonical_content(
        canonical_report_from_statement(current_statement.clone()),
    )
    .unwrap();
    let current_event_id = EventId::parse("8".repeat(64)).unwrap();
    current_statement["trade_generation"] = Value::from(7);
    current_statement["observed_at_unix_s"] = Value::from(1_800_000_200_u64);
    current_statement["supersedes_report_id"] =
        Value::String(hex::encode(ordered_current.statement_digest()));
    current_statement["supersedes_event_id"] = Value::String(current_event_id.to_hex());
    let stale = RadrootsRhiEvidenceAttestationV1::from_canonical_content(
        canonical_report_from_statement(current_statement),
    )
    .unwrap();
    assert_vector_error(
        &vectors,
        "rhi_evidence_attestation_stale_supersession_012",
        validate_rhi_evidence_attestation_supersession(&ordered_current, &current_event_id, &stale)
            .unwrap_err(),
    );

    assert_vector_error(
        &vectors,
        "rhi_evidence_attestation_caller_structural_tag_013",
        rhi_evidence_attestation_event_build_with_extra_tags(
            &current,
            &[vec!["d".to_owned(), current.trade_id().to_hex()]],
        )
        .unwrap_err(),
    );
}

fn assert_vector_error(vectors: &Value, id: &str, error: RadrootsRhiEvidenceAttestationError) {
    assert_eq!(
        error.code(),
        vector(vectors, id)["expected"]["error_code"]
            .as_str()
            .expect("error code"),
        "{id}"
    );
}

#[test]
fn signed_attestation_requires_and_preserves_the_verified_typestate() {
    let vectors = vectors();
    let base = vector(&vectors, "rhi_evidence_attestation_current_001");
    let keys = Keys::parse("0101010101010101010101010101010101010101010101010101010101010101")
        .expect("fixture keys");
    let mut statement = base["input"]["statement_payload"].clone();
    statement["issuer_pubkey"] = Value::String(keys.public_key().to_hex());
    let report = RadrootsRhiEvidenceAttestationV1::from_canonical_content(
        canonical_report_from_statement(statement),
    )
    .expect("fixture report");
    let parts = rhi_evidence_attestation_event_build(&report);
    let event = EventBuilder::new(Kind::Custom(parts.kind as u16), parts.content)
        .tags(
            parts
                .tags
                .into_iter()
                .map(Tag::parse)
                .collect::<Result<Vec<_>, _>>()
                .expect("tags"),
        )
        .custom_created_at(Timestamp::from_secs(1_800_000_000))
        .sign_with_keys(&keys)
        .expect("signed event");
    let envelope = EventEnvelope::new(EventEnvelopeParts {
        id: event.id.to_hex(),
        author: event.pubkey.to_hex(),
        created_at: event.created_at.as_secs(),
        kind: u32::from(event.kind.as_u16()),
        tags: event
            .tags
            .iter()
            .map(|tag| tag.as_slice().to_vec())
            .collect(),
        content: event.content,
        sig: event.sig.to_string(),
    })
    .expect("event envelope");
    let verified = RawEvent::new(envelope)
        .verify_id()
        .expect("verified id")
        .verify_signature(&Nip01SignatureVerifier)
        .expect("verified signature");
    assert_eq!(
        rhi_evidence_attestation_from_verified_event(&verified).unwrap(),
        report
    );
}

#[test]
fn malformed_shapes_bounds_and_diagnostics_fail_closed() {
    let vectors = vectors();
    let fixture = vector(&vectors, "rhi_evidence_attestation_current_001");
    let report =
        RadrootsRhiEvidenceAttestationV1::from_canonical_content(positive_content(fixture))
            .expect("report");
    let parts = rhi_evidence_attestation_event_build(&report);

    let mut reordered = parts.tags.clone();
    reordered.swap(0, 1);
    assert_eq!(
        validate_rhi_evidence_attestation_tags(&report, &reordered).unwrap_err(),
        RadrootsRhiEvidenceAttestationError::InvalidTagShape
    );
    let mut malformed_marker = parts.tags.clone();
    malformed_marker[2][2] = "unknown".to_owned();
    assert_eq!(
        validate_rhi_evidence_attestation_tags(&report, &malformed_marker).unwrap_err(),
        RadrootsRhiEvidenceAttestationError::MissingClaimTag
    );
    let mut unknown = parts.tags.clone();
    unknown.push(vec!["a".to_owned(), "value".to_owned()]);
    assert_eq!(
        validate_rhi_evidence_attestation_tags(&report, &unknown).unwrap_err(),
        RadrootsRhiEvidenceAttestationError::UnexpectedTag
    );
    assert_eq!(
        RadrootsRhiEvidenceAttestationV1::from_canonical_content(vec![
            b'x';
            RADROOTS_RHI_EVIDENCE_ATTESTATION_MAXIMUM_BYTES
                + 1
        ])
        .unwrap_err(),
        RadrootsRhiEvidenceAttestationError::NoncanonicalReportContent
    );
    let mut missing_supersession_fields: Value =
        serde_json::from_str(parts.content.as_str()).expect("report JSON");
    let object = missing_supersession_fields
        .as_object_mut()
        .expect("report object");
    object.remove("supersedes_event_id");
    object.remove("supersedes_report_id");
    assert_eq!(
        RadrootsRhiEvidenceAttestationV1::from_canonical_content(
            canonical_jcs_value(&missing_supersession_fields).expect("canonical malformed report")
        )
        .unwrap_err(),
        RadrootsRhiEvidenceAttestationError::NoncanonicalReportContent
    );

    let diagnostic = format!(
        "{0:?} {0}",
        RadrootsRhiEvidenceAttestationError::StatementDigestMismatch
    );
    for secret in [
        report.issuer().to_hex(),
        report.trade_id().to_hex(),
        hex::encode(report.statement_digest()),
    ] {
        assert!(!diagnostic.contains(&secret));
    }
    assert!(
        RadrootsRhiEvidenceAttestationError::StatementDigestMismatch
            .source()
            .is_none()
    );
    assert!(!format!("{report:?}").contains(&report.issuer().to_hex()));
}
