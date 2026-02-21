mod common;

use radroots_events::job::{JobFeedbackStatus, JobPaymentRequest};
use radroots_events::job_feedback::RadrootsJobFeedback;
use radroots_events::kinds::{KIND_JOB_FEEDBACK, KIND_JOB_REQUEST_MIN, KIND_JOB_RESULT_MIN};
use radroots_events_codec::job::encode::JobEncodeError;
use radroots_events_codec::job::error::JobParseError;
use radroots_events_codec::job::feedback::decode::job_feedback_from_tags;
use radroots_events_codec::job::feedback::encode::to_wire_parts;

fn sample_feedback() -> RadrootsJobFeedback {
    RadrootsJobFeedback {
        kind: KIND_JOB_FEEDBACK as u16,
        status: JobFeedbackStatus::Processing,
        extra_info: Some("queued".to_string()),
        request_event: common::event_ptr("req", Some("wss://relay")),
        customer_pubkey: Some("customer".to_string()),
        payment: Some(JobPaymentRequest {
            amount_sat: 12,
            bolt11: None,
        }),
        content: Some("payload".to_string()),
        encrypted: false,
    }
}

#[test]
fn job_feedback_roundtrip_from_tags() {
    let fb = sample_feedback();
    let content = fb.content.clone().unwrap();
    let parts = to_wire_parts(&fb, &content).unwrap();

    let decoded = job_feedback_from_tags(parts.kind, &parts.tags, &content).unwrap();
    assert_eq!(decoded, fb);
}

#[test]
fn job_feedback_requires_valid_kind() {
    let mut fb = sample_feedback();
    fb.kind = KIND_JOB_RESULT_MIN as u16;

    let err = to_wire_parts(&fb, "payload").unwrap_err();
    assert!(matches!(
        err,
        JobEncodeError::InvalidKind(KIND_JOB_RESULT_MIN)
    ));
}

#[test]
fn job_feedback_requires_status_tag() {
    let tags = vec![vec!["e".to_string(), "req".to_string()]];
    let err = job_feedback_from_tags(KIND_JOB_FEEDBACK, &tags, "payload").unwrap_err();
    assert!(matches!(err, JobParseError::MissingTag("status")));
}

#[test]
fn job_feedback_rejects_unknown_status() {
    let tags = vec![
        vec!["status".to_string(), "unknown".to_string()],
        vec!["e".to_string(), "req".to_string()],
    ];
    let err = job_feedback_from_tags(KIND_JOB_FEEDBACK, &tags, "payload").unwrap_err();
    assert!(matches!(err, JobParseError::InvalidTag("status")));
}

#[test]
fn job_feedback_metadata_rejects_wrong_kind() {
    let err = radroots_events_codec::job::feedback::decode::metadata_from_event(
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
        JobParseError::InvalidTag("kind (expected 7000)")
    ));
}

#[test]
fn job_feedback_build_tags_cover_optional_paths() {
    let mut fb = sample_feedback();
    fb.extra_info = None;
    fb.payment = None;
    fb.request_event.relays = None;
    fb.customer_pubkey = None;
    fb.encrypted = true;
    let parts = to_wire_parts(&fb, "payload").unwrap();

    let status = parts
        .tags
        .iter()
        .find(|tag| tag.first().map(|v| v.as_str()) == Some("status"))
        .expect("status tag");
    assert_eq!(status.len(), 2);

    let request = parts
        .tags
        .iter()
        .find(|tag| tag.first().map(|v| v.as_str()) == Some("e"))
        .expect("request tag");
    assert_eq!(request.len(), 2);

    assert!(
        !parts
            .tags
            .iter()
            .any(|tag| tag.first().map(|v| v.as_str()) == Some("amount"))
    );
    assert!(
        !parts
            .tags
            .iter()
            .any(|tag| tag.first().map(|v| v.as_str()) == Some("p"))
    );
    assert!(
        parts
            .tags
            .iter()
            .any(|tag| tag.first().map(|v| v.as_str()) == Some("encrypted"))
    );
}
