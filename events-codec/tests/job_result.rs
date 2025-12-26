mod common;

use radroots_events::job::{JobInputType, JobPaymentRequest};
use radroots_events::kinds::{KIND_JOB_REQUEST_MIN, KIND_JOB_RESULT_MIN};
use radroots_events::job_request::RadrootsJobInput;
use radroots_events::job_result::RadrootsJobResult;
use radroots_events_codec::job::encode::JobEncodeError;
use radroots_events_codec::job::error::JobParseError;
use radroots_events_codec::job::result::decode::job_result_from_tags;
use radroots_events_codec::job::result::encode::to_wire_parts;

fn sample_result() -> RadrootsJobResult {
    RadrootsJobResult {
        kind: (KIND_JOB_RESULT_MIN + 1) as u16,
        request_event: common::event_ptr("req", Some("wss://relay")),
        request_json: Some("{\"foo\":\"bar\"}".to_string()),
        inputs: vec![RadrootsJobInput {
            data: "https://example.com".to_string(),
            input_type: JobInputType::Url,
            relay: None,
            marker: None,
        }],
        customer_pubkey: Some("customer".to_string()),
        payment: Some(JobPaymentRequest {
            amount_sat: 50,
            bolt11: Some("bolt".to_string()),
        }),
        content: Some("payload".to_string()),
        encrypted: false,
    }
}

#[test]
fn job_result_roundtrip_from_tags() {
    let res = sample_result();
    let content = res.content.clone().unwrap();
    let parts = to_wire_parts(&res, &content).unwrap();

    let decoded = job_result_from_tags(parts.kind, &parts.tags, &content).unwrap();
    assert_eq!(decoded, res);
}

#[test]
fn job_result_requires_valid_kind() {
    let mut res = sample_result();
    res.kind = KIND_JOB_REQUEST_MIN as u16;

    let err = to_wire_parts(&res, "payload").unwrap_err();
    assert!(matches!(
        err,
        JobEncodeError::InvalidKind(KIND_JOB_REQUEST_MIN)
    ));
}

#[test]
fn job_result_requires_request_event_tag() {
    let tags = vec![vec!["p".to_string(), "customer".to_string()]];
    let err = job_result_from_tags(KIND_JOB_RESULT_MIN + 1, &tags, "payload").unwrap_err();
    assert!(matches!(err, JobParseError::MissingTag("e")));
}

#[test]
fn job_result_metadata_rejects_wrong_kind() {
    let err = radroots_events_codec::job::result::decode::metadata_from_event(
        "id".to_string(),
        "author".to_string(),
        1,
        KIND_JOB_REQUEST_MIN,
        "payload".to_string(),
        Vec::new(),
    )
    .unwrap_err();

    assert!(matches!(
        err,
        JobParseError::InvalidTag("kind (expected 6000-6999)")
    ));
}
