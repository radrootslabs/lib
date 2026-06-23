use futures::future::BoxFuture;
use nostr::JsonUtil;
use radroots_event_store::{RadrootsEventStore, RadrootsEventVerificationStatus};
use radroots_events::draft::{RadrootsFrozenEventDraft, RadrootsSignedNostrEvent};
use radroots_events::kinds::KIND_POST;
use radroots_nostr::prelude::{
    RadrootsNostrKeys, RadrootsNostrSecretKey, RadrootsNostrTimestamp, radroots_nostr_build_event,
    radroots_nostr_sign_frozen_draft,
};
use radroots_outbox::{
    RadrootsOutbox, RadrootsOutboxClaimedEvent, RadrootsOutboxEventState,
    RadrootsOutboxOperationInput, RadrootsOutboxOperationStatus, RadrootsOutboxRelayStatus,
};
use radroots_relay_transport::{
    RadrootsMockRelayFetchAdapter, RadrootsMockRelayPublishAdapter, RadrootsOutboxPublishPolicy,
    RadrootsRelayFetchItem, RadrootsRelayFetchOutcomeKind, RadrootsRelayFetchRequest,
    RadrootsRelayOutcome, RadrootsRelayOutcomeKind, RadrootsRelayPublishAdapter,
    RadrootsRelayPublishRelayReceipt, RadrootsRelayPublishRequest, RadrootsRelayTargetSet,
    RadrootsRelayTransportError, RadrootsRelayUrl, RadrootsRelayUrlPolicy,
    fetch_and_ingest_relay_events, publish_claimed_outbox_event, publish_signed_event,
};
use std::net::{IpAddr, Ipv4Addr};

const FIXTURE_ALICE_SECRET_KEY_HEX: &str =
    "10c5304d6c9ae3a1a16f7860f1cc8f5e3a76225a2663b3a989a0d775919b7df5";
const FIXTURE_ALICE_PUBLIC_KEY_HEX: &str =
    "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df";
const RELAY_PRIMARY_WSS: &str = "wss://relay.example.com";
const RELAY_SECONDARY_WSS: &str = "wss://relay-2.example.com";
const RELAY_TERTIARY_WSS: &str = "wss://relay-3.example.com";

struct TransportFailurePublishAdapter;

impl RadrootsRelayPublishAdapter for TransportFailurePublishAdapter {
    fn publish<'a>(
        &'a self,
        _request: RadrootsRelayPublishRequest,
    ) -> BoxFuture<'a, Result<Vec<RadrootsRelayPublishRelayReceipt>, RadrootsRelayTransportError>>
    {
        Box::pin(async {
            Err(RadrootsRelayTransportError::Transport(
                "adapter boundary unavailable".to_owned(),
            ))
        })
    }
}

struct NostrJsonFailurePublishAdapter;

impl RadrootsRelayPublishAdapter for NostrJsonFailurePublishAdapter {
    fn publish<'a>(
        &'a self,
        _request: RadrootsRelayPublishRequest,
    ) -> BoxFuture<'a, Result<Vec<RadrootsRelayPublishRelayReceipt>, RadrootsRelayTransportError>>
    {
        Box::pin(async {
            Err(RadrootsRelayTransportError::NostrEventJson(
                "adapter rejected raw event".to_owned(),
            ))
        })
    }
}

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

async fn complete_claimed_signing(
    outbox: &RadrootsOutbox,
    claimed: &RadrootsOutboxClaimedEvent,
    now_ms: i64,
) -> RadrootsSignedNostrEvent {
    if let Some(signed_event) = claimed.signed_event.clone() {
        return signed_event;
    }
    let signed_event =
        radroots_nostr_sign_frozen_draft(&fixture_keys(), &claimed.draft).expect("signed event");
    outbox
        .complete_signing(
            claimed.outbox_event_id,
            claimed.claim_token.as_str(),
            signed_event,
            now_ms,
        )
        .await
        .expect("complete signing")
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
    assert_eq!(relay.clone().into_string(), RELAY_PRIMARY_WSS);
    let relay_path = RadrootsRelayUrl::parse(
        "wss://Relay.Example.com/nostr",
        RadrootsRelayUrlPolicy::Public,
    )
    .expect("relay path");
    assert_eq!(relay_path.as_str(), "wss://relay.example.com/nostr");

    assert!(
        RadrootsRelayUrl::parse("ws://127.0.0.1:7777", RadrootsRelayUrlPolicy::Public).is_err()
    );
    let local = RadrootsRelayUrl::parse("ws://localhost:7777", RadrootsRelayUrlPolicy::Localhost)
        .expect("local relay");
    assert_eq!(local.as_str(), "ws://localhost:7777");
    let local_ipv4 =
        RadrootsRelayUrl::parse("ws://127.0.0.1:7777", RadrootsRelayUrlPolicy::Localhost)
            .expect("local ipv4 relay");
    assert_eq!(local_ipv4.as_str(), "ws://127.0.0.1:7777");
    let local_ipv6 = RadrootsRelayUrl::parse("ws://[::1]:7777", RadrootsRelayUrlPolicy::Localhost)
        .expect("local ipv6 relay");
    assert_eq!(local_ipv6.as_str(), "ws://[::1]:7777");
    assert!(
        RadrootsRelayUrl::parse("ws://example.com", RadrootsRelayUrlPolicy::Localhost).is_err()
    );
    assert!(
        RadrootsRelayUrl::parse("ws://192.168.1.10:7777", RadrootsRelayUrlPolicy::Localhost)
            .is_err()
    );
    assert!(matches!(
        RadrootsRelayUrl::parse("wss://127.0.0.1", RadrootsRelayUrlPolicy::Public),
        Err(RadrootsRelayTransportError::RelayUrlForbiddenDestination { .. })
    ));
    assert!(matches!(
        RadrootsRelayUrl::parse("wss://10.1.2.3", RadrootsRelayUrlPolicy::Public),
        Err(RadrootsRelayTransportError::RelayUrlForbiddenDestination { .. })
    ));
    assert!(matches!(
        RadrootsRelayUrl::parse("wss://[::1]", RadrootsRelayUrlPolicy::Public),
        Err(RadrootsRelayTransportError::RelayUrlForbiddenDestination { .. })
    ));
    assert!(matches!(
        RadrootsRelayUrl::parse("wss://[fd00::1]", RadrootsRelayUrlPolicy::Public),
        Err(RadrootsRelayTransportError::RelayUrlForbiddenDestination { .. })
    ));
    let public_relay =
        RadrootsRelayUrl::parse("wss://relay.example.com", RadrootsRelayUrlPolicy::Public)
            .expect("public relay");
    public_relay
        .validate_public_resolved_ip_addrs([IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34))])
        .expect("public resolved ip");
    assert!(matches!(
        public_relay
            .validate_public_resolved_ip_addrs([IpAddr::V4(Ipv4Addr::new(192, 168, 1, 10))]),
        Err(RadrootsRelayTransportError::RelayUrlResolvedForbiddenDestination { .. })
    ));

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
    assert!(matches!(
        RadrootsRelayUrl::parse(
            "wss://user:password@relay.example.com",
            RadrootsRelayUrlPolicy::Public
        ),
        Err(RadrootsRelayTransportError::RelayUrlUserinfo { .. })
    ));
    assert!(matches!(
        RadrootsRelayUrl::parse(
            "wss://:password@relay.example.com",
            RadrootsRelayUrlPolicy::Public
        ),
        Err(RadrootsRelayTransportError::RelayUrlUserinfo { .. })
    ));
    assert!(
        RadrootsRelayUrl::parse(
            "wss://relay.example.com:bad",
            RadrootsRelayUrlPolicy::Public
        )
        .is_err()
    );
    assert!(RadrootsRelayUrl::parse("wss://", RadrootsRelayUrlPolicy::Public).is_err());
    assert!(matches!(
        RadrootsRelayUrl::parse("radroots:relay", RadrootsRelayUrlPolicy::Public),
        Err(RadrootsRelayTransportError::EmptyRelayHost { .. })
    ));
    assert!(matches!(
        RadrootsRelayUrl::parse("relay.example.com", RadrootsRelayUrlPolicy::Public),
        Err(RadrootsRelayTransportError::RelayUrlParse { .. })
    ));
    assert!(
        RadrootsRelayUrl::parse(
            "wss://relay.example.com?subscription=1",
            RadrootsRelayUrlPolicy::Public
        )
        .is_err()
    );
    assert!(
        RadrootsRelayUrl::parse(
            "wss://relay.example.com#fragment",
            RadrootsRelayUrlPolicy::Public
        )
        .is_err()
    );

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
            RELAY_TERTIARY_WSS.to_owned(),
            RELAY_PRIMARY_WSS.to_owned(),
            RELAY_SECONDARY_WSS.to_owned()
        ]
    );

    let from_urls = RadrootsRelayTargetSet::from_urls(vec![
        relay_path.clone(),
        relay_path.clone(),
        RadrootsRelayUrl::parse(RELAY_SECONDARY_WSS, RadrootsRelayUrlPolicy::Public)
            .expect("secondary"),
    ])
    .expect("from urls");
    assert_eq!(from_urls.len(), 2);
    assert!(!from_urls.is_empty());
    assert_eq!(from_urls.relays()[0], relay_path);
    assert_eq!(
        from_urls.relays()[0].to_string(),
        "wss://relay.example.com/nostr"
    );
    assert!(matches!(
        RadrootsRelayTargetSet::new(Vec::<&str>::new(), RadrootsRelayUrlPolicy::Public),
        Err(RadrootsRelayTransportError::EmptyTargetSet)
    ));
    assert!(matches!(
        RadrootsRelayTargetSet::from_urls(Vec::new()),
        Err(RadrootsRelayTransportError::EmptyTargetSet)
    ));
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
        ("mute: pubkey muted", RadrootsRelayOutcomeKind::Muted),
        (
            "unsupported: event kind",
            RadrootsRelayOutcomeKind::Unsupported,
        ),
        (
            "payment-required: paid relay",
            RadrootsRelayOutcomeKind::PaymentRequired,
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
    assert!(
        RadrootsRelayOutcome::skipped_already_accepted("already accepted").counts_toward_quorum()
    );
    assert!(RadrootsRelayOutcome::classify("auth-required: challenge").is_retryable());
    assert!(RadrootsRelayOutcome::classify("restricted: denied").is_terminal_failure());
    assert!(RadrootsRelayOutcome::relay_url_rejected("unsafe relay").is_terminal_failure());
    assert!(RadrootsRelayOutcome::classify("mute: pubkey muted").is_terminal_failure());
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
async fn publish_receipts_track_terminal_skipped_and_adapter_errors() {
    let signed = signed_post("terminal");
    let targets = RadrootsRelayTargetSet::new(
        vec![RELAY_PRIMARY_WSS, RELAY_SECONDARY_WSS],
        RadrootsRelayUrlPolicy::Public,
    )
    .expect("targets");
    let adapter = RadrootsMockRelayPublishAdapter::new().with_outcome(
        RELAY_SECONDARY_WSS,
        RadrootsRelayOutcome::classify("restricted: group write denied"),
    );

    let receipt = publish_signed_event(
        &adapter,
        RadrootsRelayPublishRequest::new(signed.clone(), targets, 1_050).with_accepted_quorum(2),
    )
    .await
    .expect("publish");

    assert_eq!(receipt.event_id, signed.id);
    assert_eq!(receipt.attempted_count, 2);
    assert_eq!(receipt.accepted_count, 1);
    assert_eq!(receipt.retryable_count, 0);
    assert_eq!(receipt.terminal_count, 1);
    assert_eq!(receipt.quorum, 2);
    assert!(!receipt.quorum_met);

    let skipped = RadrootsRelayPublishRelayReceipt::skipped(
        RELAY_TERTIARY_WSS,
        RadrootsRelayOutcome::timeout("timeout: no OK"),
    );
    assert_eq!(skipped.relay_url, RELAY_TERTIARY_WSS);
    assert!(!skipped.attempted);
    assert_eq!(skipped.outcome.kind, RadrootsRelayOutcomeKind::Timeout);

    let error = publish_signed_event(
        &TransportFailurePublishAdapter,
        RadrootsRelayPublishRequest::new(
            signed,
            RadrootsRelayTargetSet::new(vec![RELAY_PRIMARY_WSS], RadrootsRelayUrlPolicy::Public)
                .expect("targets"),
            1_060,
        ),
    )
    .await
    .expect_err("transport failure");
    assert!(matches!(error, RadrootsRelayTransportError::Transport(_)));
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
            message: "auth-required: challenge".to_owned(),
        },
        RadrootsRelayFetchItem::Closed {
            relay_url: RELAY_TERTIARY_WSS.to_owned(),
            message: "restricted: group write denied".to_owned(),
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
    assert_eq!(receipt.closed_count, 2);
    assert_eq!(receipt.notice_count, 1);
    assert_eq!(receipt.relay_outcomes.len(), 4);
    assert_eq!(receipt.relay_outcomes[0].relay_url, RELAY_PRIMARY_WSS);
    assert_eq!(
        receipt.relay_outcomes[0].kind,
        RadrootsRelayFetchOutcomeKind::Eose
    );
    assert!(receipt.relay_outcomes[0].relay_outcome.is_none());
    assert_eq!(receipt.relay_outcomes[1].relay_url, RELAY_SECONDARY_WSS);
    assert_eq!(
        receipt.relay_outcomes[1]
            .relay_outcome
            .as_ref()
            .expect("auth outcome")
            .kind,
        RadrootsRelayOutcomeKind::AuthRequired
    );
    assert_eq!(receipt.relay_outcomes[2].relay_url, RELAY_TERTIARY_WSS);
    assert_eq!(
        receipt.relay_outcomes[2]
            .relay_outcome
            .as_ref()
            .expect("restricted outcome")
            .kind,
        RadrootsRelayOutcomeKind::Restricted
    );
    assert_eq!(
        receipt.relay_outcomes[3].kind,
        RadrootsRelayFetchOutcomeKind::Notice
    );
    assert!(receipt.relay_outcomes[3].relay_outcome.is_none());
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
async fn fetch_event_cap_preserves_later_control_outcomes() {
    let first = signed_post("first capped event");
    let skipped = signed_post("skipped capped event");
    let store = RadrootsEventStore::open_memory().await.expect("store");
    let adapter = RadrootsMockRelayFetchAdapter::new(vec![
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
            raw_json: first.raw_json.clone(),
            observed_at_ms: 1_100,
        },
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
            raw_json: skipped.raw_json,
            observed_at_ms: 1_101,
        },
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_SECONDARY_WSS.to_owned(),
            raw_json: "{not json".to_owned(),
            observed_at_ms: 1_102,
        },
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_SECONDARY_WSS.to_owned(),
            raw_json: unsupported_raw_event(),
            observed_at_ms: 1_103,
        },
        RadrootsRelayFetchItem::Eose {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
        },
        RadrootsRelayFetchItem::Closed {
            relay_url: RELAY_SECONDARY_WSS.to_owned(),
            message: "auth-required: challenge".to_owned(),
        },
        RadrootsRelayFetchItem::Notice {
            relay_url: RELAY_TERTIARY_WSS.to_owned(),
            message: "notice: still visible".to_owned(),
        },
    ]);

    let receipt =
        fetch_and_ingest_relay_events(&adapter, &store, RadrootsRelayFetchRequest::fetch(1_100, 1))
            .await
            .expect("fetch ingest");

    assert_eq!(receipt.inserted_count, 1);
    assert_eq!(receipt.duplicate_count, 0);
    assert_eq!(receipt.unsupported_count, 0);
    assert_eq!(receipt.malformed_count, 0);
    assert_eq!(receipt.events.len(), 1);
    assert_eq!(receipt.eose_count, 1);
    assert_eq!(receipt.closed_count, 1);
    assert_eq!(receipt.notice_count, 1);
    assert_eq!(receipt.relay_outcomes.len(), 3);
    assert_eq!(
        receipt.relay_outcomes[0].kind,
        RadrootsRelayFetchOutcomeKind::Eose
    );
    assert_eq!(
        receipt.relay_outcomes[1]
            .relay_outcome
            .as_ref()
            .expect("closed outcome")
            .kind,
        RadrootsRelayOutcomeKind::AuthRequired
    );
    assert_eq!(
        receipt.relay_outcomes[2].kind,
        RadrootsRelayFetchOutcomeKind::Notice
    );
}

#[tokio::test]
async fn fetch_subscription_mode_and_store_errors_are_reported() {
    let signed = signed_post("subscription");
    let store = RadrootsEventStore::open_memory().await.expect("store");
    let adapter = RadrootsMockRelayFetchAdapter::new(vec![RadrootsRelayFetchItem::Event {
        relay_url: RELAY_PRIMARY_WSS.to_owned(),
        raw_json: signed.raw_json.clone(),
        observed_at_ms: 1_200,
    }]);

    let receipt = fetch_and_ingest_relay_events(
        &adapter,
        &store,
        RadrootsRelayFetchRequest::subscription(1_200, 10),
    )
    .await
    .expect("fetch ingest");

    assert_eq!(receipt.inserted_count, 1);
    let observations = store
        .observations_for_event(signed.id.as_str())
        .await
        .expect("observations");
    assert_eq!(observations.len(), 1);
    assert_eq!(observations[0].observation_type, "subscription");

    let closed_store = RadrootsEventStore::open_memory().await.expect("store");
    closed_store.pool().close().await;
    let adapter = RadrootsMockRelayFetchAdapter::new(vec![RadrootsRelayFetchItem::Event {
        relay_url: RELAY_PRIMARY_WSS.to_owned(),
        raw_json: signed.raw_json,
        observed_at_ms: 1_210,
    }]);
    let receipt = fetch_and_ingest_relay_events(
        &adapter,
        &closed_store,
        RadrootsRelayFetchRequest::fetch(1_210, 10),
    )
    .await
    .expect("fetch ingest");

    assert_eq!(receipt.inserted_count, 0);
    assert_eq!(receipt.malformed_count, 1);
    assert!(receipt.events[0].malformed);
    assert!(receipt.events[0].message.is_some());
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
    let signed = complete_claimed_signing(&outbox, &claimed, 1_100).await;
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

#[tokio::test]
async fn outbox_publish_transport_failure_releases_retryable_claim() {
    let signed = signed_post("adapter transport failure");
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
            vec![RELAY_PRIMARY_WSS.to_owned(), RELAY_SECONDARY_WSS.to_owned()],
            1_000,
        ))
        .await
        .expect("enqueue");
    let claimed = outbox
        .claim_next_ready_event("signer", "sign-a", 2_000, 1_000)
        .await
        .expect("claim")
        .expect("claim");
    complete_claimed_signing(&outbox, &claimed, 1_100).await;
    outbox.recover_expired_claims(2_001).await.expect("recover");
    let publish_claim = outbox
        .claim_next_ready_event("publisher", "publish-a", 3_000, 2_100)
        .await
        .expect("claim")
        .expect("publish claim");

    let published = publish_claimed_outbox_event(
        &outbox,
        &store,
        &TransportFailurePublishAdapter,
        &publish_claim,
        RadrootsOutboxPublishPolicy::new(2_500),
        2_200,
    )
    .await
    .expect("publish");

    assert_eq!(published.publish.attempted_count, 2);
    assert_eq!(published.publish.accepted_count, 0);
    assert_eq!(published.publish.retryable_count, 2);
    assert_eq!(published.publish.terminal_count, 0);
    assert!(!published.publish.quorum_met);
    assert!(
        published
            .publish
            .relays
            .iter()
            .all(|relay| relay.outcome.kind == RadrootsRelayOutcomeKind::ConnectionFailed)
    );

    let event = outbox
        .get_event(receipt.outbox_event_id)
        .await
        .expect("event")
        .expect("event");
    assert_eq!(event.state, RadrootsOutboxEventState::PublishRetryable);
    assert!(event.claim_token.is_none());
    assert_eq!(event.next_attempt_after_ms, 2_500);

    let statuses = outbox
        .relay_statuses(receipt.outbox_event_id)
        .await
        .expect("statuses");
    assert_eq!(statuses.len(), 2);
    assert!(
        statuses
            .iter()
            .all(|status| status.status == RadrootsOutboxRelayStatus::FailedRetryable)
    );
    assert!(
        outbox
            .claim_next_ready_event("publisher", "publish-b", 4_000, 2_499)
            .await
            .expect("early claim")
            .is_none()
    );
    let retry_claim = outbox
        .claim_next_ready_event("publisher", "publish-b", 4_000, 2_500)
        .await
        .expect("retry claim")
        .expect("retry claim");
    assert_eq!(retry_claim.outbox_event_id, receipt.outbox_event_id);
    assert_eq!(retry_claim.state, RadrootsOutboxEventState::Publishing);
}

#[tokio::test]
async fn outbox_publish_marks_published_without_adapter_when_all_relays_already_accepted() {
    let signed = signed_post("already accepted");
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
            vec![RELAY_PRIMARY_WSS.to_owned(), RELAY_SECONDARY_WSS.to_owned()],
            1_000,
        ))
        .await
        .expect("enqueue");
    let claimed = outbox
        .claim_next_ready_event("signer", "sign-a", 2_000, 1_000)
        .await
        .expect("claim")
        .expect("claim");
    let signed = complete_claimed_signing(&outbox, &claimed, 1_100).await;
    outbox.recover_expired_claims(2_001).await.expect("recover");
    let publish_claim = outbox
        .claim_next_ready_event("publisher", "publish-a", 3_000, 2_100)
        .await
        .expect("claim")
        .expect("publish claim");
    outbox
        .mark_relay_accepted(
            publish_claim.outbox_event_id,
            publish_claim.claim_token.as_str(),
            RELAY_PRIMARY_WSS,
            2_150,
        )
        .await
        .expect("primary accepted");
    outbox
        .mark_relay_accepted(
            publish_claim.outbox_event_id,
            publish_claim.claim_token.as_str(),
            RELAY_SECONDARY_WSS,
            2_151,
        )
        .await
        .expect("secondary accepted");

    let adapter = RadrootsMockRelayPublishAdapter::new();
    let published = publish_claimed_outbox_event(
        &outbox,
        &store,
        &adapter,
        &publish_claim,
        RadrootsOutboxPublishPolicy::new(2_500),
        2_200,
    )
    .await
    .expect("publish");

    assert_eq!(published.local_ingest.event_id, signed.id);
    assert_eq!(published.publish.event_id, signed.id);
    assert_eq!(published.publish.attempted_count, 0);
    assert_eq!(published.publish.accepted_count, 2);
    assert_eq!(published.publish.quorum, 2);
    assert!(published.publish.quorum_met);
    assert!(published.publish.relays.is_empty());
    assert!(adapter.captured_raw_events().is_empty());

    let event = outbox
        .get_event(receipt.outbox_event_id)
        .await
        .expect("event")
        .expect("event");
    assert_eq!(event.state, RadrootsOutboxEventState::Published);
    assert_eq!(event.accepted_quorum, 2);
    assert!(event.claim_token.is_none());
    let operation = outbox
        .get_operation(receipt.operation_id)
        .await
        .expect("operation")
        .expect("operation");
    assert_eq!(operation.status, RadrootsOutboxOperationStatus::Complete);
}

#[tokio::test]
async fn outbox_publish_uses_persisted_accepted_count_for_explicit_quorum() {
    let signed = signed_post("explicit quorum already accepted");
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
    complete_claimed_signing(&outbox, &claimed, 1_100).await;
    outbox.recover_expired_claims(2_001).await.expect("recover");
    let publish_claim = outbox
        .claim_next_ready_event("publisher", "publish-a", 3_000, 2_100)
        .await
        .expect("claim")
        .expect("publish claim");
    outbox
        .mark_relay_accepted(
            publish_claim.outbox_event_id,
            publish_claim.claim_token.as_str(),
            RELAY_PRIMARY_WSS,
            2_150,
        )
        .await
        .expect("primary accepted");
    outbox
        .mark_relay_accepted(
            publish_claim.outbox_event_id,
            publish_claim.claim_token.as_str(),
            RELAY_SECONDARY_WSS,
            2_151,
        )
        .await
        .expect("secondary accepted");

    let adapter = RadrootsMockRelayPublishAdapter::new();
    let published = publish_claimed_outbox_event(
        &outbox,
        &store,
        &adapter,
        &publish_claim,
        RadrootsOutboxPublishPolicy::new(2_500).with_accepted_quorum(2),
        2_200,
    )
    .await
    .expect("publish");

    assert_eq!(published.publish.attempted_count, 0);
    assert_eq!(published.publish.accepted_count, 2);
    assert_eq!(published.publish.quorum, 2);
    assert!(published.publish.quorum_met);
    assert!(adapter.captured_raw_events().is_empty());

    let event = outbox
        .get_event(receipt.outbox_event_id)
        .await
        .expect("event")
        .expect("event");
    assert_eq!(event.state, RadrootsOutboxEventState::Published);
    assert_eq!(event.accepted_quorum, 2);
    let statuses = outbox
        .relay_statuses(receipt.outbox_event_id)
        .await
        .expect("statuses");
    assert_eq!(
        statuses
            .iter()
            .find(|status| status.relay_url == RELAY_TERTIARY_WSS)
            .expect("tertiary")
            .status,
        RadrootsOutboxRelayStatus::Pending
    );
}

#[tokio::test]
async fn outbox_publish_marks_published_when_policy_quorum_is_met_with_failure_diagnostics() {
    let signed = signed_post("quorum");
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
    let signed = complete_claimed_signing(&outbox, &claimed, 1_100).await;
    outbox.recover_expired_claims(2_001).await.expect("recover");
    let publish_claim = outbox
        .claim_next_ready_event("publisher", "publish-a", 3_000, 2_100)
        .await
        .expect("claim")
        .expect("publish claim");

    let adapter = RadrootsMockRelayPublishAdapter::new()
        .with_outcome(RELAY_PRIMARY_WSS, RadrootsRelayOutcome::accepted())
        .with_outcome(
            RELAY_SECONDARY_WSS,
            RadrootsRelayOutcome::duplicate_accepted("duplicate: already have it"),
        )
        .with_outcome(
            RELAY_TERTIARY_WSS,
            RadrootsRelayOutcome::classify("restricted: group write denied"),
        );
    let published = publish_claimed_outbox_event(
        &outbox,
        &store,
        &adapter,
        &publish_claim,
        RadrootsOutboxPublishPolicy::new(2_500).with_accepted_quorum(2),
        2_200,
    )
    .await
    .expect("publish");

    assert_eq!(published.publish.quorum, 2);
    assert_eq!(published.publish.accepted_count, 2);
    assert_eq!(published.publish.terminal_count, 1);
    assert!(published.publish.quorum_met);

    let event = outbox
        .get_event(receipt.outbox_event_id)
        .await
        .expect("event")
        .expect("event");
    assert_eq!(event.state, RadrootsOutboxEventState::Published);
    assert_eq!(event.accepted_quorum, 2);
    assert!(event.claim_token.is_none());
    let operation = outbox
        .get_operation(receipt.operation_id)
        .await
        .expect("operation")
        .expect("operation");
    assert_eq!(operation.status, RadrootsOutboxOperationStatus::Complete);

    let statuses = outbox
        .relay_statuses(receipt.outbox_event_id)
        .await
        .expect("statuses");
    assert_eq!(
        statuses
            .iter()
            .find(|status| status.relay_url == RELAY_TERTIARY_WSS)
            .expect("tertiary")
            .status,
        RadrootsOutboxRelayStatus::FailedTerminal
    );
    assert!(
        outbox
            .claim_next_ready_event("publisher", "publish-b", 4_000, 2_300)
            .await
            .expect("claim")
            .is_none()
    );

    let observations = store
        .observations_for_event(signed.id.as_str())
        .await
        .expect("observations");
    assert_eq!(observations.len(), 2);
}

#[tokio::test]
async fn outbox_publish_republishes_accepted_relays_when_policy_requests_it() {
    let signed = signed_post("republish accepted");
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
            vec![RELAY_PRIMARY_WSS.to_owned(), RELAY_SECONDARY_WSS.to_owned()],
            1_000,
        ))
        .await
        .expect("enqueue");
    let claimed = outbox
        .claim_next_ready_event("signer", "sign-a", 2_000, 1_000)
        .await
        .expect("claim")
        .expect("claim");
    let signed = complete_claimed_signing(&outbox, &claimed, 1_100).await;
    outbox.recover_expired_claims(2_001).await.expect("recover");
    let publish_claim = outbox
        .claim_next_ready_event("publisher", "publish-a", 3_000, 2_100)
        .await
        .expect("claim")
        .expect("publish claim");
    outbox
        .mark_relay_accepted(
            publish_claim.outbox_event_id,
            publish_claim.claim_token.as_str(),
            RELAY_PRIMARY_WSS,
            2_150,
        )
        .await
        .expect("primary accepted");

    let adapter = RadrootsMockRelayPublishAdapter::new()
        .with_outcome(RELAY_PRIMARY_WSS, RadrootsRelayOutcome::accepted())
        .with_outcome(RELAY_SECONDARY_WSS, RadrootsRelayOutcome::accepted());
    let published = publish_claimed_outbox_event(
        &outbox,
        &store,
        &adapter,
        &publish_claim,
        RadrootsOutboxPublishPolicy::new(2_500)
            .republish_accepted_relays(true)
            .relay_url_policy(RadrootsRelayUrlPolicy::Public),
        2_200,
    )
    .await
    .expect("publish");

    assert_eq!(published.local_ingest.event_id, signed.id);
    assert_eq!(published.publish.attempted_count, 2);
    assert_eq!(published.publish.accepted_count, 2);
    assert_eq!(published.publish.quorum, 1);
    assert!(published.publish.quorum_met);
    assert_eq!(adapter.captured_raw_events().len(), 1);

    let event = outbox
        .get_event(receipt.outbox_event_id)
        .await
        .expect("event")
        .expect("event");
    assert_eq!(event.state, RadrootsOutboxEventState::Published);
    let statuses = outbox
        .relay_statuses(receipt.outbox_event_id)
        .await
        .expect("statuses");
    assert!(
        statuses
            .iter()
            .all(|status| status.status == RadrootsOutboxRelayStatus::Accepted)
    );
}

#[tokio::test]
async fn outbox_publish_requires_claimed_signed_event() {
    let signed = signed_post("missing signature");
    let outbox = RadrootsOutbox::open_memory().await.expect("outbox");
    let store = RadrootsEventStore::open_memory().await.expect("store");
    let draft = RadrootsFrozenEventDraft::new(
        "radroots.social.post.v1",
        KIND_POST,
        signed.created_at,
        signed.tags,
        signed.content,
        signed.pubkey.as_str(),
    )
    .expect("draft");
    let receipt = outbox
        .enqueue_operation(RadrootsOutboxOperationInput::new(
            "publish_post",
            draft,
            vec![RELAY_PRIMARY_WSS.to_owned()],
            1_000,
        ))
        .await
        .expect("enqueue");
    let claimed = outbox
        .claim_next_ready_event("signer", "sign-a", 2_000, 1_000)
        .await
        .expect("claim")
        .expect("claim");
    let adapter = RadrootsMockRelayPublishAdapter::new();

    let error = publish_claimed_outbox_event(
        &outbox,
        &store,
        &adapter,
        &claimed,
        RadrootsOutboxPublishPolicy::new(2_500),
        1_100,
    )
    .await
    .expect_err("missing signed event");

    assert!(matches!(
        error,
        RadrootsRelayTransportError::MissingSignedOutboxEvent(event_id)
            if event_id == receipt.outbox_event_id
    ));
    assert!(adapter.captured_raw_events().is_empty());
}

#[tokio::test]
async fn outbox_publish_propagates_non_transport_adapter_errors_after_target_filtering() {
    let signed = signed_post("adapter non transport failure");
    let outbox = RadrootsOutbox::open_memory().await.expect("outbox");
    let store = RadrootsEventStore::open_memory().await.expect("store");
    let draft = RadrootsFrozenEventDraft::new(
        "radroots.social.post.v1",
        KIND_POST,
        signed.created_at,
        signed.tags,
        signed.content,
        signed.pubkey.as_str(),
    )
    .expect("draft");
    let receipt = outbox
        .enqueue_operation(RadrootsOutboxOperationInput::new(
            "publish_post",
            draft,
            vec![RELAY_PRIMARY_WSS.to_owned(), RELAY_SECONDARY_WSS.to_owned()],
            1_000,
        ))
        .await
        .expect("enqueue");
    let claimed = outbox
        .claim_next_ready_event("signer", "sign-a", 2_000, 1_000)
        .await
        .expect("claim")
        .expect("claim");
    complete_claimed_signing(&outbox, &claimed, 1_100).await;
    outbox.recover_expired_claims(2_001).await.expect("recover");
    let mut publish_claim = outbox
        .claim_next_ready_event("publisher", "publish-a", 3_000, 2_100)
        .await
        .expect("claim")
        .expect("publish claim");
    publish_claim.target_relays = vec![RELAY_PRIMARY_WSS.to_owned()];

    let error = publish_claimed_outbox_event(
        &outbox,
        &store,
        &NostrJsonFailurePublishAdapter,
        &publish_claim,
        RadrootsOutboxPublishPolicy::new(2_500),
        2_200,
    )
    .await
    .expect_err("adapter error");

    assert!(matches!(
        error,
        RadrootsRelayTransportError::NostrEventJson(_)
    ));
    let event = outbox
        .get_event(receipt.outbox_event_id)
        .await
        .expect("event")
        .expect("event");
    assert_eq!(event.accepted_quorum, 1);
}

#[tokio::test]
async fn smoke_relay_fetch_processes_one_thousand_event_receipts() {
    let store = RadrootsEventStore::open_memory().await.expect("store");
    let mut items = Vec::new();
    for index in 0..1_000 {
        let signed = signed_post(format!("fetch-smoke-{index}").as_str());
        let relay_url = match index % 3 {
            0 => RELAY_PRIMARY_WSS,
            1 => RELAY_SECONDARY_WSS,
            _ => RELAY_TERTIARY_WSS,
        };
        items.push(RadrootsRelayFetchItem::Event {
            relay_url: relay_url.to_owned(),
            raw_json: signed.raw_json,
            observed_at_ms: 10_000 + index,
        });
    }
    let adapter = RadrootsMockRelayFetchAdapter::new(items);
    let receipt = fetch_and_ingest_relay_events(
        &adapter,
        &store,
        RadrootsRelayFetchRequest::fetch(10_000, 1_000),
    )
    .await
    .expect("fetch");

    assert_eq!(receipt.inserted_count, 1_000);
    assert_eq!(receipt.duplicate_count, 0);
    assert_eq!(receipt.malformed_count, 0);
    assert_eq!(receipt.unsupported_count, 0);
    assert_eq!(receipt.events.len(), 1_000);
    assert!(receipt.events.iter().all(|event| event.projection_eligible));
    let replay = store
        .events_since_cursor("fetch-smoke", 1_000)
        .await
        .expect("replay");
    assert_eq!(replay.len(), 1_000);
}
