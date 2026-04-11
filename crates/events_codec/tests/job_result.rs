mod common;
#[path = "../src/test_fixtures.rs"]
mod test_fixtures;

use radroots_events::job::{JobInputType, JobPaymentRequest};
use radroots_events::job_request::RadrootsJobInput;
use radroots_events::job_result::RadrootsJobResult;
use radroots_events::kinds::{KIND_JOB_REQUEST_MIN, KIND_JOB_RESULT_MIN};
use radroots_events_codec::job::encode::JobEncodeError;
use radroots_events_codec::job::error::JobParseError;
use radroots_events_codec::job::result::decode::{job_result_from_tags, parsed_from_event};
use radroots_events_codec::job::result::encode::to_wire_parts;
use test_fixtures::{APP_PRIMARY_HTTPS, RELAY_PRIMARY_WSS, RELAY_SECONDARY_WSS};

fn sample_result() -> RadrootsJobResult {
    RadrootsJobResult {
        kind: (KIND_JOB_RESULT_MIN + 1) as u16,
        request_event: common::event_ptr("req", Some(RELAY_PRIMARY_WSS)),
        request_json: Some("{\"foo\":\"bar\"}".to_string()),
        inputs: vec![RadrootsJobInput {
            data: APP_PRIMARY_HTTPS.to_string(),
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
fn job_result_roundtrip_with_empty_content_sets_none() {
    let res = sample_result();
    let parts = to_wire_parts(&res, "").unwrap();
    let decoded = job_result_from_tags(parts.kind, &parts.tags, "").unwrap();
    assert!(decoded.content.is_none());
}

#[test]
fn job_result_roundtrip_preserves_input_relay_and_marker() {
    let mut res = sample_result();
    res.inputs = vec![RadrootsJobInput {
        data: "note1payload".to_string(),
        input_type: JobInputType::Event,
        relay: Some(RELAY_SECONDARY_WSS.to_string()),
        marker: Some("root".to_string()),
    }];
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
fn job_result_encrypted_adds_flag_and_rejects_inputs() {
    let mut encrypted = sample_result();
    encrypted.encrypted = true;
    encrypted.inputs.clear();
    let content = encrypted.content.clone().unwrap();
    let parts = to_wire_parts(&encrypted, &content).unwrap();
    assert!(
        parts
            .tags
            .iter()
            .any(|tag| tag.first().map(|v| v.as_str()) == Some("encrypted"))
    );
    assert!(
        !parts
            .tags
            .iter()
            .any(|tag| tag.first().map(|v| v.as_str()) == Some("i"))
    );

    let mut invalid = sample_result();
    invalid.encrypted = true;
    let err = to_wire_parts(&invalid, "payload").unwrap_err();
    assert!(matches!(
        err,
        JobEncodeError::EmptyRequiredField("inputs-when-encrypted")
    ));
}

#[test]
fn job_result_build_tags_supports_minimal_optional_fields() {
    let mut res = sample_result();
    res.request_json = None;
    res.inputs.clear();
    res.customer_pubkey = None;
    res.payment = None;
    let parts = to_wire_parts(&res, "payload").unwrap();
    assert!(
        parts
            .tags
            .iter()
            .any(|tag| tag.first().map(|v| v.as_str()) == Some("e"))
    );
    assert!(
        !parts
            .tags
            .iter()
            .any(|tag| tag.first().map(|v| v.as_str()) == Some("request"))
    );
    assert!(
        !parts
            .tags
            .iter()
            .any(|tag| tag.first().map(|v| v.as_str()) == Some("i"))
    );
    assert!(
        !parts
            .tags
            .iter()
            .any(|tag| tag.first().map(|v| v.as_str()) == Some("p"))
    );
    assert!(
        !parts
            .tags
            .iter()
            .any(|tag| tag.first().map(|v| v.as_str()) == Some("amount"))
    );
}

#[test]
fn job_result_build_tags_omits_request_relay_when_absent() {
    let mut res = sample_result();
    res.request_event.relays = None;
    let parts = to_wire_parts(&res, "payload").unwrap();
    let request = parts
        .tags
        .iter()
        .find(|tag| tag.first().map(|v| v.as_str()) == Some("e"))
        .expect("request tag");
    assert_eq!(request.len(), 2);
}

#[test]
fn job_result_requires_request_event_tag() {
    let tags = vec![vec!["p".to_string(), "customer".to_string()]];
    let err = job_result_from_tags(KIND_JOB_RESULT_MIN + 1, &tags, "payload").unwrap_err();
    assert!(matches!(err, JobParseError::MissingTag("e")));

    let tags = vec![
        vec!["e".to_string()],
        vec!["amount".to_string(), "not-a-number".to_string()],
    ];
    let err = job_result_from_tags(KIND_JOB_RESULT_MIN + 1, &tags, "payload").unwrap_err();
    assert!(matches!(err, JobParseError::InvalidTag("e")));

    let tags = vec![
        vec!["e".to_string(), "req".to_string()],
        vec!["amount".to_string(), "not-a-number".to_string()],
    ];
    let err = job_result_from_tags(KIND_JOB_RESULT_MIN + 1, &tags, "payload").unwrap_err();
    assert!(matches!(err, JobParseError::InvalidNumber("amount", _)));
}

#[test]
fn job_result_metadata_rejects_wrong_kind() {
    let err = radroots_events_codec::job::result::decode::data_from_event(
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

#[test]
fn job_result_data_from_event_success_path() {
    let result = sample_result();
    let content = result.content.clone().unwrap();
    let parts = to_wire_parts(&result, &content).expect("wire parts");
    let data = radroots_events_codec::job::result::decode::data_from_event(
        "id".to_string(),
        "author".to_string(),
        1,
        parts.kind,
        content,
        parts.tags,
    )
    .expect("job result data");
    assert_eq!(data.id, "id");
    assert_eq!(data.author, "author");
    assert_eq!(data.kind, KIND_JOB_RESULT_MIN + 1);
    assert_eq!(data.data.request_event.id, "req");
}

#[test]
fn job_result_data_from_event_propagates_decode_errors_with_valid_kind() {
    let err = radroots_events_codec::job::result::decode::data_from_event(
        "id".to_string(),
        "author".to_string(),
        1,
        KIND_JOB_RESULT_MIN + 1,
        "payload".to_string(),
        Vec::new(),
    )
    .unwrap_err();
    assert!(matches!(err, JobParseError::MissingTag("e")));
}

#[test]
fn job_result_index_from_event_propagates_parse_errors() {
    let err = parsed_from_event(
        "id".to_string(),
        "author".to_string(),
        1,
        KIND_JOB_REQUEST_MIN,
        "payload".to_string(),
        Vec::new(),
        "sig".to_string(),
    )
    .unwrap_err();
    assert!(matches!(
        err,
        JobParseError::InvalidTag("kind (expected 6000-6999)")
    ));
}
