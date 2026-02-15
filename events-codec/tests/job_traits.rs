use radroots_events::RadrootsNostrEvent;
use radroots_events::job::JobInputType;
use radroots_events::job_request::{RadrootsJobInput, RadrootsJobParam, RadrootsJobRequest};
use radroots_events::kinds::KIND_JOB_REQUEST_MIN;
use radroots_events_codec::job::request::encode::to_wire_parts;
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
    let parts = to_wire_parts(&req, "payload").unwrap();

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
