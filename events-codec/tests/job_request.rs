use radroots_events::job::JobInputType;
use radroots_events::job_request::{RadrootsJobInput, RadrootsJobParam, RadrootsJobRequest};
use radroots_events_codec::job::encode::JobEncodeError;
use radroots_events_codec::job::error::JobParseError;
use radroots_events_codec::job::request::decode::job_request_from_tags;
use radroots_events_codec::job::request::encode::to_wire_parts;

fn sample_request() -> RadrootsJobRequest {
    RadrootsJobRequest {
        kind: 5001,
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
    req.kind = 7000;

    let err = to_wire_parts(&req, "payload").unwrap_err();
    assert!(matches!(err, JobEncodeError::InvalidKind(7000)));
}

#[test]
fn job_request_requires_providers_when_encrypted() {
    let mut req = sample_request();
    req.encrypted = true;
    req.providers.clear();

    let err = to_wire_parts(&req, "payload").unwrap_err();
    assert!(matches!(err, JobEncodeError::MissingProvidersForEncrypted));

    let tags = vec![vec!["encrypted".to_string()]];
    let err = job_request_from_tags(5001, &tags).unwrap_err();
    assert!(matches!(err, JobParseError::MissingTag("p")));
}

#[test]
fn job_request_metadata_rejects_wrong_kind() {
    let err = radroots_events_codec::job::request::decode::metadata_from_event(
        "id".to_string(),
        "author".to_string(),
        1,
        1000,
        Vec::new(),
    )
    .unwrap_err();

    assert!(matches!(
        err,
        JobParseError::InvalidTag("kind (expected 5000-5999)")
    ));
}
