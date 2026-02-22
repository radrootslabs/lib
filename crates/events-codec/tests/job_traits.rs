use radroots_events::RadrootsNostrEvent;
use radroots_events::job::{JobFeedbackStatus, JobInputType, JobPaymentRequest};
use radroots_events::job_feedback::RadrootsJobFeedback;
use radroots_events::job_request::{RadrootsJobInput, RadrootsJobParam, RadrootsJobRequest};
use radroots_events::job_result::RadrootsJobResult;
use radroots_events::kinds::{KIND_JOB_FEEDBACK, KIND_JOB_REQUEST_MIN, KIND_JOB_RESULT_MIN};
use radroots_events_codec::job::feedback::encode::to_wire_parts as to_feedback_wire_parts;
use radroots_events_codec::job::request::encode::to_wire_parts as to_request_wire_parts;
use radroots_events_codec::job::result::encode::to_wire_parts as to_result_wire_parts;
use radroots_events_codec::job::traits::{BorrowedEventAdapter, JobEventLike};

fn sample_request() -> RadrootsJobRequest {
    RadrootsJobRequest {
        kind: (KIND_JOB_REQUEST_MIN + 1) as u16,
        inputs: vec![RadrootsJobInput {
            data: "hello".to_string(),
            input_type: JobInputType::Text,
            relay: None,
            marker: None,
        }],
        output: None,
        params: vec![RadrootsJobParam {
            key: "foo".to_string(),
            value: "bar".to_string(),
        }],
        bid_sat: None,
        relays: Vec::new(),
        providers: vec!["provider".to_string()],
        topics: Vec::new(),
        encrypted: false,
    }
}

#[test]
fn borrowed_event_adapter_builds_request_metadata() {
    let req = sample_request();
    let parts = to_request_wire_parts(&req, "payload").unwrap();

    let event = RadrootsNostrEvent {
        id: "id".to_string(),
        author: "author".to_string(),
        created_at: 42,
        kind: parts.kind,
        tags: parts.tags.clone(),
        content: "payload".to_string(),
        sig: "sig".to_string(),
    };

    let adapter = BorrowedEventAdapter::new(&event, event.created_at, &event.tags, &event.sig);
    let metadata = adapter.to_job_request_metadata().unwrap();

    assert_eq!(metadata.id, event.id);
    assert_eq!(metadata.author, event.author);
    assert_eq!(metadata.published_at, event.created_at);
    assert_eq!(metadata.kind, event.kind);
    assert_eq!(metadata.job_request, req);
}

fn sample_result() -> RadrootsJobResult {
    RadrootsJobResult {
        kind: (KIND_JOB_RESULT_MIN + 1) as u16,
        request_event: radroots_events::RadrootsNostrEventPtr {
            id: "req".to_string(),
            relays: Some("wss://relay.example.com".to_string()),
        },
        request_json: Some("{\"foo\":\"bar\"}".to_string()),
        inputs: vec![RadrootsJobInput {
            data: "hello".to_string(),
            input_type: JobInputType::Text,
            relay: None,
            marker: None,
        }],
        customer_pubkey: Some(
            "58e318557257f2ab58a415d21bb57082b4824cf667a1d64e72bcbc5acc018c62".to_string(),
        ),
        payment: Some(JobPaymentRequest {
            amount_sat: 1,
            bolt11: None,
        }),
        content: Some("payload".to_string()),
        encrypted: false,
    }
}

fn sample_feedback() -> RadrootsJobFeedback {
    RadrootsJobFeedback {
        kind: KIND_JOB_FEEDBACK as u16,
        status: JobFeedbackStatus::Processing,
        extra_info: Some("processing".to_string()),
        request_event: radroots_events::RadrootsNostrEventPtr {
            id: "req".to_string(),
            relays: Some("wss://relay.example.com".to_string()),
        },
        customer_pubkey: Some(
            "58e318557257f2ab58a415d21bb57082b4824cf667a1d64e72bcbc5acc018c62".to_string(),
        ),
        payment: Some(JobPaymentRequest {
            amount_sat: 2,
            bolt11: None,
        }),
        content: Some("payload".to_string()),
        encrypted: false,
    }
}

#[test]
fn borrowed_event_adapter_builds_request_metadata_and_index() {
    let req = sample_request();
    let parts = to_request_wire_parts(&req, "payload").unwrap();
    let event = RadrootsNostrEvent {
        id: "id".to_string(),
        author: "author".to_string(),
        created_at: 42,
        kind: parts.kind,
        tags: parts.tags,
        content: "payload".to_string(),
        sig: "sig".to_string(),
    };

    let adapter = BorrowedEventAdapter::new(&event, event.created_at, &event.tags, &event.sig);
    assert_eq!(adapter.raw_id(), "id");
    assert_eq!(adapter.raw_author(), "author");
    assert_eq!(adapter.raw_published_at(), 42);
    assert_eq!(adapter.raw_kind(), event.kind);
    assert_eq!(adapter.raw_content(), "payload");
    assert_eq!(adapter.raw_tags().len(), event.tags.len());
    assert_eq!(adapter.raw_sig(), "sig");

    let index = adapter.to_job_request_event_index().unwrap();
    assert_eq!(index.event.id, event.id);
    assert_eq!(index.event.author, event.author);
    assert_eq!(index.event.created_at, event.created_at);
    assert_eq!(index.event.kind, event.kind);
    assert_eq!(index.event.content, event.content);
    assert_eq!(index.event.sig, event.sig);
}

#[test]
fn borrowed_event_adapter_builds_result_metadata_and_index() {
    let result = sample_result();
    let parts = to_result_wire_parts(&result, "payload").unwrap();
    let event = RadrootsNostrEvent {
        id: "id".to_string(),
        author: "author".to_string(),
        created_at: 42,
        kind: parts.kind,
        tags: parts.tags,
        content: "payload".to_string(),
        sig: "sig".to_string(),
    };

    let adapter = BorrowedEventAdapter::new(&event, event.created_at, &event.tags, &event.sig);
    let metadata = adapter.to_job_result_metadata().unwrap();
    assert_eq!(metadata.id, event.id);
    assert_eq!(metadata.author, event.author);
    assert_eq!(metadata.published_at, event.created_at);
    assert_eq!(metadata.kind, event.kind);
    assert_eq!(metadata.job_result.kind, result.kind);
    assert_eq!(metadata.job_result.request_event.id, "req");
    assert_eq!(metadata.job_result.content.as_deref(), Some("payload"));

    let index = adapter.to_job_result_event_index().unwrap();
    assert_eq!(index.event.id, event.id);
    assert_eq!(index.event.author, event.author);
    assert_eq!(index.event.created_at, event.created_at);
    assert_eq!(index.event.kind, event.kind);
    assert_eq!(index.event.content, event.content);
    assert_eq!(index.event.sig, event.sig);
}

#[test]
fn borrowed_event_adapter_builds_feedback_metadata_and_index() {
    let feedback = sample_feedback();
    let parts = to_feedback_wire_parts(&feedback, "payload").unwrap();
    let event = RadrootsNostrEvent {
        id: "id".to_string(),
        author: "author".to_string(),
        created_at: 42,
        kind: parts.kind,
        tags: parts.tags,
        content: "payload".to_string(),
        sig: "sig".to_string(),
    };

    let adapter = BorrowedEventAdapter::new(&event, event.created_at, &event.tags, &event.sig);
    let metadata = adapter.to_job_feedback_metadata().unwrap();
    assert_eq!(metadata.id, event.id);
    assert_eq!(metadata.author, event.author);
    assert_eq!(metadata.published_at, event.created_at);
    assert_eq!(metadata.kind, event.kind);
    assert_eq!(metadata.job_feedback.kind, feedback.kind);
    assert_eq!(metadata.job_feedback.request_event.id, "req");
    assert_eq!(metadata.job_feedback.content.as_deref(), Some("payload"));

    let index = adapter.to_job_feedback_event_index().unwrap();
    assert_eq!(index.event.id, event.id);
    assert_eq!(index.event.author, event.author);
    assert_eq!(index.event.created_at, event.created_at);
    assert_eq!(index.event.kind, event.kind);
    assert_eq!(index.event.content, event.content);
    assert_eq!(index.event.sig, event.sig);
}
