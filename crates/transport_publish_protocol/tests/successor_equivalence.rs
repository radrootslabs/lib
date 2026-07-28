use serde::{Serialize, de::DeserializeOwned};

use radroots_protocol::radrootsd::transport_publish::v5 as successor;
use radroots_transport::RadrootsTransportTarget;
use radroots_transport_publish_protocol as predecessor;

fn assert_json_equivalent<Old, New>(old: &Old) -> New
where
    Old: Serialize,
    New: DeserializeOwned + Serialize,
{
    let old_json = serde_json::to_vec(old).expect("predecessor JSON");
    let new: New = serde_json::from_slice(old_json.as_slice()).expect("successor decode");
    let new_json = serde_json::to_vec(&new).expect("successor JSON");
    assert_eq!(new_json, old_json);
    new
}

fn request() -> predecessor::TransportPublishEventRequest {
    predecessor::TransportPublishEventRequest {
        raw_event_json: "{\"id\":\"event\"}".to_owned(),
        target_policy: predecessor::TransportPublishTargetPolicy::explicit_targets(vec![
            predecessor::TransportPublishTarget::nostr("wss://relay.example.com"),
        ]),
        delivery_policy: predecessor::TransportPublishDeliveryPolicy::Any,
        idempotency_key: Some("idem-1".to_owned()),
        timeout_ms: Some(5_000),
    }
}

fn accepted_job() -> predecessor::TransportPublishJobView {
    predecessor::TransportPublishJobView {
        job_id: "job-1".to_owned(),
        status: predecessor::TransportPublishJobStatus::DeliverySatisfied,
        terminal: true,
        delivery_satisfied: true,
        event_id: "0".repeat(64),
        pubkey: "1".repeat(64),
        event_kind: 30_402,
        target_policy: predecessor::TransportPublishTargetPolicy::explicit_targets(vec![
            predecessor::TransportPublishTarget::nostr("wss://relay.example.com"),
        ]),
        delivery_policy: predecessor::TransportPublishDeliveryPolicy::Any,
        target_count: 1,
        acknowledged_count: 1,
        retryable_count: 0,
        terminal_count: 0,
        requested_at_ms: 1,
        completed_at_ms: Some(2),
        last_error: None,
        targets: vec![predecessor::TransportPublishTargetOutcome {
            transport_kind: "nostr".to_owned(),
            endpoint_uri: "wss://relay.example.com".to_owned(),
            target_scope: None,
            target_label: None,
            source: predecessor::TransportPublishTargetSource::Request,
            attempted: true,
            outcome_kind: predecessor::TransportPublishOutcomeKind::Accepted,
            message: None,
            latency_ms: Some(7),
        }],
    }
}

#[test]
fn request_response_job_and_capability_json_are_byte_identical() {
    let request = request();
    request.validate(1).expect("predecessor request");
    let successor_request = assert_json_equivalent::<_, successor::EventRequest>(&request);
    successor_request.validate(1).expect("successor request");

    let job = accepted_job();
    job.validate().expect("predecessor job");
    let response = predecessor::TransportPublishEventResponse {
        deduplicated: false,
        job,
    };
    let successor_response = assert_json_equivalent::<_, successor::EventResponse>(&response);
    successor_response.job.validate().expect("successor job");

    let capabilities = predecessor::TransportPublishCapabilities::v5(1_024, 10);
    let successor_capabilities =
        assert_json_equivalent::<_, successor::Capabilities>(&capabilities);
    assert_eq!(successor_capabilities.api_version, successor::API_VERSION);
}

#[test]
fn required_target_fingerprint_json_is_byte_identical() {
    let target = RadrootsTransportTarget::nostr_relay("wss://relay.example.com")
        .expect("native Nostr target");
    let predecessor = predecessor::TransportPublishDeliveryPolicy::required_targets(vec![
        target.fingerprint().clone(),
    ])
    .expect("predecessor policy");

    let successor = assert_json_equivalent::<_, successor::DeliveryPolicy>(&predecessor);
    successor.validate().expect("successor policy");
}

#[test]
fn successor_rejects_unknown_and_noncanonical_wire_fields() {
    let mut value = serde_json::to_value(request()).expect("request value");
    value
        .as_object_mut()
        .expect("request object")
        .insert("unknown".to_owned(), serde_json::json!(true));
    assert!(serde_json::from_value::<successor::EventRequest>(value).is_err());

    assert!(
        serde_json::from_str::<successor::DeliveryPolicy>(
            r#"{"mode":"required_targets","targets":["ABCDEF"]}"#,
        )
        .is_err()
    );
}
