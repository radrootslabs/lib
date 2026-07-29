#[path = "../src/test_fixtures.rs"]
mod test_fixtures;

use radroots_event::envelope::kind::{
    KIND_JOB_FEEDBACK, KIND_JOB_REQUEST_MIN, KIND_JOB_RESULT_MIN,
};
use radroots_event::social::job::{JobFeedbackStatus, JobInputType, JobPaymentRequest};
use radroots_event::social::job_feedback::RadrootsJobFeedback;
use radroots_event::social::job_request::{RadrootsJobInput, RadrootsJobParam, RadrootsJobRequest};
use radroots_event::social::job_result::RadrootsJobResult;
use radroots_event::{envelope::RadrootsEventEnvelope, envelope::RadrootsEventEnvelopeParts};
use radroots_event_codec::job::feedback::encode::to_wire_parts as to_feedback_wire_parts;
use radroots_event_codec::job::request::encode::to_wire_parts as to_request_wire_parts;
use radroots_event_codec::job::result::encode::to_wire_parts as to_result_wire_parts;
use radroots_event_codec::job::traits::{BorrowedEventAdapter, JobEventLike};
use test_fixtures::{FIXTURE_ALICE_PUBLIC_KEY_HEX, RELAY_PRIMARY_WSS};

const EVENT_ID: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const AUTHOR: &str = "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df";
const EVENT_SIG: &str = concat!(
    "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
    "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
);

fn event_envelope(kind: u32, tags: Vec<Vec<String>>, content: &str) -> RadrootsEventEnvelope {
    RadrootsEventEnvelope::new(RadrootsEventEnvelopeParts {
        id: EVENT_ID.to_string(),
        author: AUTHOR.to_string(),
        created_at: 42,
        kind,
        tags,
        content: content.to_string(),
        sig: EVENT_SIG.to_string(),
    })
    .unwrap()
}

fn sample_request() -> RadrootsJobRequest {
    RadrootsJobRequest {
        kind: u16::try_from(KIND_JOB_REQUEST_MIN + 1).expect("request kind must fit NIP-01"),
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

    let event = event_envelope(parts.kind, parts.tags.clone(), "payload");
    let tags = event.tags_as_vec();

    let adapter =
        BorrowedEventAdapter::new(&event, event.created_at_u64(), &tags, event.signature_hex());
    let metadata = adapter.to_job_request_metadata().unwrap();

    assert_eq!(metadata.id, event.id_hex());
    assert_eq!(metadata.author, event.author().to_hex());
    assert_eq!(metadata.published_at, event.created_at_u64());
    assert_eq!(metadata.kind, event.kind_u32());
    assert_eq!(metadata.data, req);
}

fn sample_result() -> RadrootsJobResult {
    RadrootsJobResult {
        kind: u16::try_from(KIND_JOB_RESULT_MIN + 1).expect("result kind must fit NIP-01"),
        request_event: radroots_event::tag::RadrootsEventPtr {
            id: "req".to_string(),
            relays: Some(RELAY_PRIMARY_WSS.to_string()),
        },
        request_json: Some("{\"foo\":\"bar\"}".to_string()),
        inputs: vec![RadrootsJobInput {
            data: "hello".to_string(),
            input_type: JobInputType::Text,
            relay: None,
            marker: None,
        }],
        customer_pubkey: Some(FIXTURE_ALICE_PUBLIC_KEY_HEX.to_string()),
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
        kind: u16::try_from(KIND_JOB_FEEDBACK).expect("feedback kind must fit NIP-01"),
        status: JobFeedbackStatus::Processing,
        extra_info: Some("processing".to_string()),
        request_event: radroots_event::tag::RadrootsEventPtr {
            id: "req".to_string(),
            relays: Some(RELAY_PRIMARY_WSS.to_string()),
        },
        customer_pubkey: Some(FIXTURE_ALICE_PUBLIC_KEY_HEX.to_string()),
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
    let event = event_envelope(parts.kind, parts.tags, "payload");
    let tags = event.tags_as_vec();

    let adapter =
        BorrowedEventAdapter::new(&event, event.created_at_u64(), &tags, event.signature_hex());
    assert_eq!(adapter.raw_id(), EVENT_ID);
    assert_eq!(adapter.raw_author(), AUTHOR);
    assert_eq!(adapter.raw_published_at(), 42);
    assert_eq!(adapter.raw_kind(), event.kind_u32());
    assert_eq!(adapter.raw_content(), "payload");
    assert_eq!(adapter.raw_tags().len(), tags.len());
    assert_eq!(adapter.raw_sig(), EVENT_SIG);

    let index = adapter.to_job_request_event_index().unwrap();
    assert_eq!(index.event.id_hex(), event.id_hex());
    assert_eq!(index.event.author().to_hex(), event.author().to_hex());
    assert_eq!(index.event.created_at_u64(), event.created_at_u64());
    assert_eq!(index.event.kind_u32(), event.kind_u32());
    assert_eq!(index.event.content(), event.content());
    assert_eq!(index.event.signature_hex(), event.signature_hex());
}

#[test]
fn borrowed_event_adapter_builds_result_metadata_and_index() {
    let result = sample_result();
    let parts = to_result_wire_parts(&result, "payload").unwrap();
    let event = event_envelope(parts.kind, parts.tags, "payload");
    let tags = event.tags_as_vec();

    let adapter =
        BorrowedEventAdapter::new(&event, event.created_at_u64(), &tags, event.signature_hex());
    let metadata = adapter.to_job_result_metadata().unwrap();
    assert_eq!(metadata.id, event.id_hex());
    assert_eq!(metadata.author, event.author().to_hex());
    assert_eq!(metadata.published_at, event.created_at_u64());
    assert_eq!(metadata.kind, event.kind_u32());
    assert_eq!(metadata.data.kind, result.kind);
    assert_eq!(metadata.data.request_event.id, "req");
    assert_eq!(metadata.data.content.as_deref(), Some("payload"));

    let index = adapter.to_job_result_event_index().unwrap();
    assert_eq!(index.event.id_hex(), event.id_hex());
    assert_eq!(index.event.author().to_hex(), event.author().to_hex());
    assert_eq!(index.event.created_at_u64(), event.created_at_u64());
    assert_eq!(index.event.kind_u32(), event.kind_u32());
    assert_eq!(index.event.content(), event.content());
    assert_eq!(index.event.signature_hex(), event.signature_hex());
}

#[test]
fn borrowed_event_adapter_builds_feedback_metadata_and_index() {
    let feedback = sample_feedback();
    let parts = to_feedback_wire_parts(&feedback, "payload").unwrap();
    let event = event_envelope(parts.kind, parts.tags, "payload");
    let tags = event.tags_as_vec();

    let adapter =
        BorrowedEventAdapter::new(&event, event.created_at_u64(), &tags, event.signature_hex());
    let metadata = adapter.to_job_feedback_metadata().unwrap();
    assert_eq!(metadata.id, event.id_hex());
    assert_eq!(metadata.author, event.author().to_hex());
    assert_eq!(metadata.published_at, event.created_at_u64());
    assert_eq!(metadata.kind, event.kind_u32());
    assert_eq!(metadata.data.kind, feedback.kind);
    assert_eq!(metadata.data.request_event.id, "req");
    assert_eq!(metadata.data.content.as_deref(), Some("payload"));

    let index = adapter.to_job_feedback_event_index().unwrap();
    assert_eq!(index.event.id_hex(), event.id_hex());
    assert_eq!(index.event.author().to_hex(), event.author().to_hex());
    assert_eq!(index.event.created_at_u64(), event.created_at_u64());
    assert_eq!(index.event.kind_u32(), event.kind_u32());
    assert_eq!(index.event.content(), event.content());
    assert_eq!(index.event.signature_hex(), event.signature_hex());
}
