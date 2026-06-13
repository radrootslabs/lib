use nostr::JsonUtil;
use radroots_event_store::{RadrootsEventStore, RadrootsEventVerificationStatus};
use radroots_events::draft::{RadrootsFrozenEventDraft, RadrootsSignedNostrEvent};
use radroots_events::kinds::KIND_POST;
use radroots_nostr::prelude::{
    RadrootsNostrKeys, RadrootsNostrSecretKey, RadrootsNostrTimestamp, radroots_nostr_build_event,
    radroots_nostr_sign_frozen_draft,
};
use radroots_outbox::{
    RadrootsOutbox, RadrootsOutboxEventState, RadrootsOutboxOperationInput,
    RadrootsOutboxOperationStatus, RadrootsOutboxRelayStatus,
};
use radroots_relay_transport::{
    RadrootsMockRelayFetchAdapter, RadrootsMockRelayPublishAdapter, RadrootsOutboxPublishPolicy,
    RadrootsRelayFetchItem, RadrootsRelayFetchRequest, RadrootsRelayOutcome,
    RadrootsRelayOutcomeKind, RadrootsRelayTargetSet, RadrootsRelayUrl, RadrootsRelayUrlPolicy,
    fetch_and_ingest_relay_events, publish_claimed_outbox_event, publish_signed_event,
};

const FIXTURE_ALICE_SECRET_KEY_HEX: &str =
    "10c5304d6c9ae3a1a16f7860f1cc8f5e3a76225a2663b3a989a0d775919b7df5";
const FIXTURE_ALICE_PUBLIC_KEY_HEX: &str =
    "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df";
const RELAY_PRIMARY_WSS: &str = "wss://relay.example.com";
const RELAY_SECONDARY_WSS: &str = "wss://relay-2.example.com";
const RELAY_TERTIARY_WSS: &str = "wss://relay-3.example.com";

fn fixture_keys() -> RadrootsNostrKeys {
    let secret_key =
        RadrootsNostrSecretKey::from_hex(FIXTURE_ALICE_SECRET_KEY_HEX).expect("secret key");
    RadrootsNostrKeys::new(secret_key)
}

fn signed_post(content: &str) -> RadrootsSignedNostrEvent {
    let draft = RadrootsFrozenEventDraft::new(
        "radroots.social.post.v1",
        KIND_POST,
        1_700_000_000,
        vec![vec!["t".to_owned(), "soil".to_owned()]],
        content,
        FIXTURE_ALICE_PUBLIC_KEY_HEX,
    )
    .expect("draft");
    radroots_nostr_sign_frozen_draft(&fixture_keys(), &draft).expect("signed event")
}

fn unsupported_raw_event() -> String {
    let event = radroots_nostr_build_event(999, "unsupported", Vec::new())
        .expect("event builder")
        .custom_created_at(RadrootsNostrTimestamp::from_secs(1_700_000_001))
        .sign_with_keys(&fixture_keys())
        .expect("signed unsupported event");
    event.as_json()
}

fn tampered_raw_event() -> String {
    let signed = signed_post("trusted");
    let mut value =
        serde_json::from_str::<serde_json::Value>(signed.raw_json.as_str()).expect("raw json");
    value["content"] = serde_json::Value::String("tampered".to_owned());
    serde_json::to_string(&value).expect("tampered json")
}

#[test]
fn relay_url_validation_and_target_normalization() {
    let relay = RadrootsRelayUrl::parse("wss://Relay.Example.com", RadrootsRelayUrlPolicy::Public)
        .expect("relay");
    assert_eq!(relay.as_str(), RELAY_PRIMARY_WSS);

    assert!(
        RadrootsRelayUrl::parse("ws://127.0.0.1:7777", RadrootsRelayUrlPolicy::Public).is_err()
    );
    let local = RadrootsRelayUrl::parse("ws://127.0.0.1:7777", RadrootsRelayUrlPolicy::LocalDev)
        .expect("local relay");
    assert_eq!(local.as_str(), "ws://127.0.0.1:7777");

    assert!(
        RadrootsRelayUrl::parse("https://relay.example.com", RadrootsRelayUrlPolicy::Public)
            .is_err()
    );
    assert!(
        RadrootsRelayUrl::parse(
            "wss://user@relay.example.com",
            RadrootsRelayUrlPolicy::Public
        )
        .is_err()
    );
    assert!(
        RadrootsRelayUrl::parse(
            "wss://relay.example.com:bad",
            RadrootsRelayUrlPolicy::Public
        )
        .is_err()
    );
    assert!(RadrootsRelayUrl::parse("wss://", RadrootsRelayUrlPolicy::Public).is_err());

    let targets = RadrootsRelayTargetSet::new(
        vec![
            RELAY_TERTIARY_WSS,
            RELAY_PRIMARY_WSS,
            RELAY_PRIMARY_WSS,
            RELAY_SECONDARY_WSS,
        ],
        RadrootsRelayUrlPolicy::Public,
    )
    .expect("targets");
    assert_eq!(
        targets.relay_strings(),
        vec![
            RELAY_SECONDARY_WSS.to_owned(),
            RELAY_TERTIARY_WSS.to_owned(),
            RELAY_PRIMARY_WSS.to_owned()
        ]
    );
}

#[test]
fn outcome_prefix_classification_covers_required_kinds() {
    let cases = [
        ("blocked: policy", RadrootsRelayOutcomeKind::Blocked),
        (
            "rate-limited: slow down",
            RadrootsRelayOutcomeKind::RateLimited,
        ),
        ("invalid: bad event", RadrootsRelayOutcomeKind::Invalid),
        ("pow: difficulty 24", RadrootsRelayOutcomeKind::PowRequired),
        (
            "restricted: group write denied",
            RadrootsRelayOutcomeKind::Restricted,
        ),
        (
            "auth-required: challenge",
            RadrootsRelayOutcomeKind::AuthRequired,
        ),
        (
            "duplicate: already have it",
            RadrootsRelayOutcomeKind::DuplicateAccepted,
        ),
        ("error: relay failed", RadrootsRelayOutcomeKind::Error),
        ("timeout: no OK", RadrootsRelayOutcomeKind::Timeout),
        ("strange relay text", RadrootsRelayOutcomeKind::Unknown),
    ];

    for (message, kind) in cases {
        let outcome = RadrootsRelayOutcome::classify(message);
        assert_eq!(outcome.kind, kind);
    }

    assert!(RadrootsRelayOutcome::classify("duplicate: already have it").counts_toward_quorum());
    assert!(RadrootsRelayOutcome::classify("auth-required: challenge").is_retryable());
    assert!(RadrootsRelayOutcome::classify("restricted: denied").is_terminal_failure());
}

#[tokio::test]
async fn mock_publish_preserves_exact_raw_json_and_counts_outcomes() {
    let signed = signed_post("hello");
    let targets = RadrootsRelayTargetSet::new(
        vec![RELAY_PRIMARY_WSS, RELAY_SECONDARY_WSS, RELAY_TERTIARY_WSS],
        RadrootsRelayUrlPolicy::Public,
    )
    .expect("targets");
    let adapter = RadrootsMockRelayPublishAdapter::new()
        .with_outcome(
            RELAY_SECONDARY_WSS,
            RadrootsRelayOutcome::classify("duplicate: already have it"),
        )
        .with_outcome(
            RELAY_TERTIARY_WSS,
            RadrootsRelayOutcome::classify("auth-required: challenge"),
        );

    let receipt = publish_signed_event(
        &adapter,
        radroots_relay_transport::RadrootsRelayPublishRequest::new(signed.clone(), targets, 1_000)
            .with_accepted_quorum(2),
    )
    .await
    .expect("publish");

    assert_eq!(adapter.captured_raw_events(), vec![signed.raw_json]);
    assert_eq!(receipt.attempted_count, 3);
    assert_eq!(receipt.accepted_count, 2);
    assert_eq!(receipt.retryable_count, 1);
    assert!(receipt.quorum_met);
    serde_json::to_string(&receipt).expect("receipt json");
}

#[tokio::test]
async fn fetch_ingests_events_and_records_relay_observations() {
    let signed = signed_post("hello");
    let store = RadrootsEventStore::open_memory().await.expect("store");
    let adapter = RadrootsMockRelayFetchAdapter::new(vec![
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
            raw_json: signed.raw_json.clone(),
            observed_at_ms: 1_000,
        },
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
            raw_json: signed.raw_json.clone(),
            observed_at_ms: 1_001,
        },
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_SECONDARY_WSS.to_owned(),
            raw_json: unsupported_raw_event(),
            observed_at_ms: 1_002,
        },
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_SECONDARY_WSS.to_owned(),
            raw_json: tampered_raw_event(),
            observed_at_ms: 1_003,
        },
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_TERTIARY_WSS.to_owned(),
            raw_json: "{not json".to_owned(),
            observed_at_ms: 1_004,
        },
        RadrootsRelayFetchItem::Eose {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
        },
        RadrootsRelayFetchItem::Closed {
            relay_url: RELAY_SECONDARY_WSS.to_owned(),
            message: "closed: done".to_owned(),
        },
        RadrootsRelayFetchItem::Notice {
            relay_url: RELAY_TERTIARY_WSS.to_owned(),
            message: "notice: test".to_owned(),
        },
    ]);

    let receipt = fetch_and_ingest_relay_events(
        &adapter,
        &store,
        RadrootsRelayFetchRequest::fetch(1_000, 10),
    )
    .await
    .expect("fetch ingest");

    assert_eq!(receipt.inserted_count, 3);
    assert_eq!(receipt.duplicate_count, 1);
    assert_eq!(receipt.unsupported_count, 1);
    assert_eq!(receipt.malformed_count, 1);
    assert_eq!(receipt.eose_count, 1);
    assert_eq!(receipt.closed_count, 1);
    assert_eq!(receipt.notice_count, 1);
    assert_eq!(
        receipt.events[0].verification_status.as_deref(),
        Some(RadrootsEventVerificationStatus::Verified.as_str())
    );
    assert!(receipt.events[0].projection_eligible);
    assert_eq!(
        receipt.events[1].verification_status.as_deref(),
        Some(RadrootsEventVerificationStatus::Verified.as_str())
    );
    assert!(!receipt.events[1].projection_eligible);
    assert_eq!(
        receipt.events[2].verification_status.as_deref(),
        Some(RadrootsEventVerificationStatus::Verified.as_str())
    );
    assert!(!receipt.events[2].projection_eligible);
    assert_eq!(
        receipt.events[3].verification_status.as_deref(),
        Some(RadrootsEventVerificationStatus::IdMismatch.as_str())
    );
    assert!(!receipt.events[3].projection_eligible);
    assert_eq!(receipt.events[4].verification_status, None);
    assert!(!receipt.events[4].projection_eligible);

    let observations = store
        .observations_for_event(signed.id.as_str())
        .await
        .expect("observations");
    assert_eq!(observations.len(), 1);
    assert_eq!(observations[0].relay_url, RELAY_PRIMARY_WSS);
    assert_eq!(observations[0].observation_count, 2);
}

#[tokio::test]
async fn outbox_publish_persists_partial_success_and_skips_accepted_retry() {
    let signed = signed_post("hello");
    let outbox = RadrootsOutbox::open_memory().await.expect("outbox");
    let store = RadrootsEventStore::open_memory().await.expect("store");
    let draft = RadrootsFrozenEventDraft::new(
        "radroots.social.post.v1",
        KIND_POST,
        signed.created_at,
        signed.tags.clone(),
        signed.content.clone(),
        signed.pubkey.as_str(),
    )
    .expect("draft");
    let receipt = outbox
        .enqueue_operation(RadrootsOutboxOperationInput::new(
            "publish_post",
            draft,
            vec![
                RELAY_PRIMARY_WSS.to_owned(),
                RELAY_SECONDARY_WSS.to_owned(),
                RELAY_TERTIARY_WSS.to_owned(),
            ],
            1_000,
        ))
        .await
        .expect("enqueue");
    let claimed = outbox
        .claim_next_ready_event("signer", "sign-a", 2_000, 1_000)
        .await
        .expect("claim")
        .expect("claim");
    let signed = outbox
        .sign_claimed_event(&claimed, &fixture_keys(), 1_100)
        .await
        .expect("sign");
    outbox.recover_expired_claims(2_001).await.expect("recover");
    let publish_claim = outbox
        .claim_next_ready_event("publisher", "publish-a", 3_000, 2_100)
        .await
        .expect("claim")
        .expect("publish claim");
    assert_eq!(publish_claim.state, RadrootsOutboxEventState::Publishing);

    let adapter = RadrootsMockRelayPublishAdapter::new()
        .with_outcome(RELAY_PRIMARY_WSS, RadrootsRelayOutcome::accepted())
        .with_outcome(
            RELAY_SECONDARY_WSS,
            RadrootsRelayOutcome::timeout("timeout: no OK"),
        )
        .with_outcome(
            RELAY_TERTIARY_WSS,
            RadrootsRelayOutcome::duplicate_accepted("duplicate: already have it"),
        );
    let first = publish_claimed_outbox_event(
        &outbox,
        &store,
        &adapter,
        &publish_claim,
        RadrootsOutboxPublishPolicy::new(2_500),
        2_200,
    )
    .await
    .expect("publish");

    assert_eq!(first.publish.attempted_count, 3);
    assert_eq!(first.publish.accepted_count, 2);
    assert!(!first.publish.quorum_met);
    let event = outbox
        .get_event(receipt.outbox_event_id)
        .await
        .expect("event")
        .expect("event");
    assert_eq!(event.state, RadrootsOutboxEventState::PublishRetryable);
    assert_eq!(event.accepted_quorum, 3);

    let statuses = outbox
        .relay_statuses(receipt.outbox_event_id)
        .await
        .expect("statuses");
    assert_eq!(
        statuses
            .iter()
            .find(|status| status.relay_url == RELAY_PRIMARY_WSS)
            .expect("primary")
            .status,
        RadrootsOutboxRelayStatus::Accepted
    );
    assert_eq!(
        statuses
            .iter()
            .find(|status| status.relay_url == RELAY_SECONDARY_WSS)
            .expect("secondary")
            .status,
        RadrootsOutboxRelayStatus::FailedRetryable
    );
    assert_eq!(
        statuses
            .iter()
            .find(|status| status.relay_url == RELAY_TERTIARY_WSS)
            .expect("tertiary")
            .status,
        RadrootsOutboxRelayStatus::Accepted
    );

    let retry_claim = outbox
        .claim_next_ready_event("publisher", "publish-b", 4_000, 2_500)
        .await
        .expect("claim")
        .expect("retry claim");
    let retry_adapter = RadrootsMockRelayPublishAdapter::new()
        .with_outcome(RELAY_SECONDARY_WSS, RadrootsRelayOutcome::accepted());
    let second = publish_claimed_outbox_event(
        &outbox,
        &store,
        &retry_adapter,
        &retry_claim,
        RadrootsOutboxPublishPolicy::new(3_000),
        2_600,
    )
    .await
    .expect("retry publish");

    assert_eq!(second.local_ingest.event_id, signed.id);
    assert_eq!(second.publish.attempted_count, 1);
    assert_eq!(retry_adapter.captured_raw_events().len(), 1);

    let event = outbox
        .get_event(receipt.outbox_event_id)
        .await
        .expect("event")
        .expect("event");
    assert_eq!(event.state, RadrootsOutboxEventState::Published);
    assert_eq!(event.accepted_quorum, 3);
    let operation = outbox
        .get_operation(receipt.operation_id)
        .await
        .expect("operation")
        .expect("operation");
    assert_eq!(operation.status, RadrootsOutboxOperationStatus::Complete);

    let observations = store
        .observations_for_event(signed.id.as_str())
        .await
        .expect("observations");
    assert_eq!(observations.len(), 3);
}
