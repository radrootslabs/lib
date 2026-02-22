use radroots_events::job::JobInputType;
use radroots_events::job_request::{RadrootsJobInput, RadrootsJobParam, RadrootsJobRequest};
use radroots_events::kinds::{KIND_JOB_FEEDBACK, KIND_JOB_REQUEST_MIN, KIND_JOB_RESULT_MIN};
use radroots_events_codec::job::encode::JobEncodeError;
use radroots_events_codec::job::error::JobParseError;
use radroots_events_codec::job::request::decode::{index_from_event, job_request_from_tags};
use radroots_events_codec::job::request::encode::to_wire_parts;

fn sample_request() -> RadrootsJobRequest {
    RadrootsJobRequest {
        kind: (KIND_JOB_REQUEST_MIN + 1) as u16,
        inputs: vec![RadrootsJobInput {
            data: "https://example.com".to_string(),
            input_type: JobInputType::Url,
            relay: Some("wss://relay".to_string()),
            marker: Some("source".to_string()),
        }],
        output: Some("json".to_string()),
        params: vec![RadrootsJobParam {
            key: "foo".to_string(),
            value: "bar".to_string(),
        }],
        bid_sat: Some(250),
        relays: vec!["wss://relay".to_string()],
        providers: vec!["provider".to_string()],
        topics: vec!["topic".to_string()],
        encrypted: false,
    }
}

#[test]
fn job_request_roundtrip_from_tags() {
    let req = sample_request();
    let parts = to_wire_parts(&req, "payload").unwrap();

    let decoded = job_request_from_tags(parts.kind, &parts.tags).unwrap();
    assert_eq!(decoded, req);
}

#[test]
fn job_request_requires_valid_kind() {
    let mut req = sample_request();
    req.kind = KIND_JOB_FEEDBACK as u16;

    let err = to_wire_parts(&req, "payload").unwrap_err();
    assert!(matches!(
        err,
        JobEncodeError::InvalidKind(KIND_JOB_FEEDBACK)
    ));
}

#[test]
fn job_request_requires_providers_when_encrypted() {
    let mut req = sample_request();
    req.encrypted = true;
    req.providers.clear();

    let err = to_wire_parts(&req, "payload").unwrap_err();
    assert!(matches!(err, JobEncodeError::MissingProvidersForEncrypted));

    let tags = vec![vec!["encrypted".to_string()]];
    let err = job_request_from_tags(KIND_JOB_REQUEST_MIN + 1, &tags).unwrap_err();
    assert!(matches!(err, JobParseError::MissingTag("p")));
}

#[test]
fn job_request_from_tags_accepts_encrypted_with_provider() {
    let request = job_request_from_tags(
        KIND_JOB_REQUEST_MIN + 1,
        &[
            vec!["encrypted".to_string()],
            vec!["p".to_string(), "provider".to_string()],
        ],
    )
    .unwrap();
    assert!(request.encrypted);
    assert_eq!(request.providers, vec!["provider".to_string()]);
}

#[test]
fn job_request_to_wire_parts_allows_encrypted_when_provider_present() {
    let mut req = sample_request();
    req.encrypted = true;
    let parts = to_wire_parts(&req, "payload").unwrap();
    assert!(
        parts
            .tags
            .iter()
            .any(|tag| tag.first().map(|v| v.as_str()) == Some("encrypted"))
    );
}

#[test]
fn job_request_metadata_rejects_wrong_kind() {
    let err = radroots_events_codec::job::request::decode::metadata_from_event(
        "id".to_string(),
        "author".to_string(),
        1,
        KIND_JOB_RESULT_MIN,
        Vec::new(),
    )
    .unwrap_err();

    assert!(matches!(
        err,
        JobParseError::InvalidTag("kind (expected 5000-5999)")
    ));
}

#[test]
fn job_request_index_from_event_propagates_parse_errors() {
    let err = index_from_event(
        "id".to_string(),
        "author".to_string(),
        1,
        KIND_JOB_RESULT_MIN,
        "payload".to_string(),
        Vec::new(),
        "sig".to_string(),
    )
    .unwrap_err();
    assert!(matches!(
        err,
        JobParseError::InvalidTag("kind (expected 5000-5999)")
    ));
}
