use futures::future::BoxFuture;
use nostr::JsonUtil;
use radroots_event::draft::{RadrootsEventDraft, RadrootsSignedEvent};
use radroots_event::kinds::KIND_POST;
use radroots_event_store::{
    RadrootsEventStore, RadrootsEventVerificationStatus, RadrootsTransportObservationRow,
    RadrootsTransportObservationType,
};
use radroots_nostr::prelude::{
    RadrootsNostrFilter, RadrootsNostrKeys, RadrootsNostrKind, RadrootsNostrSecretKey,
    RadrootsNostrTimestamp, radroots_nostr_build_event, radroots_nostr_filter_tag,
    radroots_nostr_sign_frozen_draft,
};
use radroots_outbox::{
    RadrootsOutbox, RadrootsOutboxClaimedEvent, RadrootsOutboxDeliveryPlanInput,
    RadrootsOutboxDeliveryTargetStatus, RadrootsOutboxEventState, RadrootsOutboxOperationInput,
    RadrootsOutboxOperationStatus,
};
use radroots_transport::{
    RadrootsTransport, RadrootsTransportDeliveryRequest, RadrootsTransportError,
    RadrootsTransportFetchRequest, RadrootsTransportKind, RadrootsTransportMeshScopeId,
    RadrootsTransportPayload, RadrootsTransportSatisfactionClass,
    RadrootsTransportSatisfactionPolicy, RadrootsTransportTarget, RadrootsTransportTargetLabel,
    RadrootsTransportTargetSet,
};
use radroots_transport_nostr::{
    RadrootsMockRelayFetchAdapter, RadrootsMockRelayPublishAdapter, RadrootsNostrTransport,
    RadrootsOutboxPublishPolicy, RadrootsRelayFetchFilters, RadrootsRelayFetchItem,
    RadrootsRelayFetchMode, RadrootsRelayFetchOutcomeKind, RadrootsRelayFetchRequest,
    RadrootsRelayOutcome, RadrootsRelayOutcomeKind, RadrootsRelayPublishAdapter,
    RadrootsRelayPublishRelayReceipt, RadrootsRelayPublishRequest, RadrootsRelayTargetSet,
    RadrootsRelayTransportError, RadrootsRelayUrl, RadrootsRelayUrlPolicy,
    fetch_and_ingest_relay_events, fetch_relay_events, fetch_relay_events_blocking,
    publish_claimed_outbox_event, publish_signed_event, verified_signed_event_payload,
};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

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

struct PartialPublishAdapter;

impl RadrootsRelayPublishAdapter for PartialPublishAdapter {
    fn publish<'a>(
        &'a self,
        _request: RadrootsRelayPublishRequest,
    ) -> BoxFuture<'a, Result<Vec<RadrootsRelayPublishRelayReceipt>, RadrootsRelayTransportError>>
    {
        Box::pin(async {
            Ok(vec![RadrootsRelayPublishRelayReceipt::attempted(
                RELAY_PRIMARY_WSS,
                RadrootsRelayOutcome::accepted(),
            )])
        })
    }
}

struct SlashSpelledRelayReceiptPublishAdapter;

impl RadrootsRelayPublishAdapter for SlashSpelledRelayReceiptPublishAdapter {
    fn publish<'a>(
        &'a self,
        _request: RadrootsRelayPublishRequest,
    ) -> BoxFuture<'a, Result<Vec<RadrootsRelayPublishRelayReceipt>, RadrootsRelayTransportError>>
    {
        Box::pin(async {
            Ok(vec![RadrootsRelayPublishRelayReceipt::attempted(
                format!("{RELAY_PRIMARY_WSS}/"),
                RadrootsRelayOutcome::accepted(),
            )])
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

struct UnknownRelayReceiptPublishAdapter;

impl RadrootsRelayPublishAdapter for UnknownRelayReceiptPublishAdapter {
    fn publish<'a>(
        &'a self,
        request: RadrootsRelayPublishRequest,
    ) -> BoxFuture<'a, Result<Vec<RadrootsRelayPublishRelayReceipt>, RadrootsRelayTransportError>>
    {
        Box::pin(async move {
            let relay = request
                .targets
                .relays()
                .first()
                .expect("fixture target")
                .as_str()
                .to_owned();
            Ok(vec![
                RadrootsRelayPublishRelayReceipt::attempted(
                    relay,
                    RadrootsRelayOutcome::accepted(),
                ),
                RadrootsRelayPublishRelayReceipt::attempted(
                    RELAY_TERTIARY_WSS,
                    RadrootsRelayOutcome::accepted(),
                ),
            ])
        })
    }
}

fn fixture_keys() -> RadrootsNostrKeys {
    let secret_key =
        RadrootsNostrSecretKey::from_hex(FIXTURE_ALICE_SECRET_KEY_HEX).expect("secret key");
    RadrootsNostrKeys::new(secret_key)
}

fn signed_post(content: &str) -> RadrootsSignedEvent {
    signed_event_with_kind_and_hashtag(content, KIND_POST, "soil")
}

fn assert_outbox_publish_observations(
    observations: &[RadrootsTransportObservationRow],
    publish_ack_count: usize,
) {
    assert_eq!(observations.len(), publish_ack_count + 1);
    assert_eq!(
        observations
            .iter()
            .filter(|observation| observation.observation_type
                == RadrootsTransportObservationType::LocalImport
                && observation.endpoint_uri.as_str() == "local:outbox")
            .count(),
        1
    );
    assert_eq!(
        observations
            .iter()
            .filter(|observation| observation.observation_type
                == RadrootsTransportObservationType::PublishAck)
            .count(),
        publish_ack_count
    );
}

fn signed_event_with_kind_and_hashtag(
    content: &str,
    kind: u32,
    hashtag: &str,
) -> RadrootsSignedEvent {
    let draft = RadrootsEventDraft::new(
        "radroots.social.post.v1",
        kind,
        1_700_000_000,
        vec![vec!["t".to_owned(), hashtag.to_owned()]],
        content,
        FIXTURE_ALICE_PUBLIC_KEY_HEX,
    )
    .expect("draft");
    radroots_nostr_sign_frozen_draft(&fixture_keys(), &draft).expect("signed event")
}

fn signed_raw_event_with_kind_and_hashtag(content: &str, kind: u32, hashtag: &str) -> nostr::Event {
    radroots_nostr_build_event(
        kind,
        content,
        vec![vec!["t".to_owned(), hashtag.to_owned()]],
    )
    .expect("event builder")
    .custom_created_at(RadrootsNostrTimestamp::from_secs(1_700_000_000))
    .sign_with_keys(&fixture_keys())
    .expect("signed event")
}

async fn complete_claimed_signing(
    outbox: &RadrootsOutbox,
    claimed: &RadrootsOutboxClaimedEvent,
    now_ms: i64,
) -> RadrootsSignedEvent {
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

fn nostr_target(relay_url: &str) -> RadrootsTransportTarget {
    RadrootsTransportTarget::nostr_relay(relay_url).expect("nostr target")
}

fn scoped_nostr_target(relay_url: &str, scope: &str, label: &str) -> RadrootsTransportTarget {
    RadrootsTransportTarget::nostr_relay_with_metadata(
        relay_url,
        Some(RadrootsTransportMeshScopeId::parse(scope).expect("target scope")),
        Some(RadrootsTransportTargetLabel::parse(label).expect("target label")),
    )
    .expect("scoped nostr target")
}

fn outbox_operation_input<I, S>(
    draft: RadrootsEventDraft,
    relays: I,
    satisfaction_policy: RadrootsTransportSatisfactionPolicy,
) -> RadrootsOutboxOperationInput
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let targets = relays
        .into_iter()
        .map(|relay| nostr_target(relay.as_ref()))
        .collect::<Vec<_>>();
    RadrootsOutboxOperationInput::new(
        "publish_post",
        draft,
        RadrootsOutboxDeliveryPlanInput::new(
            "transport.nostr.local",
            1,
            satisfaction_policy,
            targets,
        ),
        1_000,
    )
}

fn all_accepted_outbox_operation_input<I, S>(
    draft: RadrootsEventDraft,
    relays: I,
) -> RadrootsOutboxOperationInput
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    outbox_operation_input(
        draft,
        relays,
        RadrootsTransportSatisfactionPolicy::all_accepted(),
    )
}

fn unsupported_raw_event() -> String {
    let event = radroots_nostr_build_event(999, "unsupported", Vec::new())
        .expect("event builder")
        .custom_created_at(RadrootsNostrTimestamp::from_secs(1_700_000_001))
        .sign_with_keys(&fixture_keys())
        .expect("signed unsupported event");
    event.as_json()
}

fn post_relay_fetch_filter(limit: usize) -> RadrootsNostrFilter {
    radroots_nostr_filter_tag(
        RadrootsNostrFilter::new()
            .kind(RadrootsNostrKind::Custom(KIND_POST as u16))
            .limit(limit),
        "t",
        vec!["soil".to_owned()],
    )
    .expect("post relay fetch filter")
}

fn unsupported_relay_fetch_filter(limit: usize) -> RadrootsNostrFilter {
    RadrootsNostrFilter::new()
        .kind(RadrootsNostrKind::Custom(999))
        .limit(limit)
}

fn fixture_relay_fetch_request(
    observed_at_ms: i64,
    max_events: usize,
) -> RadrootsRelayFetchRequest {
    RadrootsRelayFetchRequest::fetch(
        observed_at_ms,
        max_events,
        [
            post_relay_fetch_filter(max_events),
            unsupported_relay_fetch_filter(max_events),
        ],
    )
    .expect("fixture relay fetch request")
}

fn post_relay_fetch_request(observed_at_ms: i64, max_events: usize) -> RadrootsRelayFetchRequest {
    RadrootsRelayFetchRequest::fetch(
        observed_at_ms,
        max_events,
        [post_relay_fetch_filter(max_events)],
    )
    .expect("post relay fetch request")
}

fn tampered_raw_event() -> String {
    let signed = signed_post("trusted");
    let mut value = serde_json::from_str::<serde_json::Value>(signed.raw_json()).expect("raw json");
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
    for relay_url in [
        "wss://0.0.0.0",
        "wss://169.254.1.2",
        "wss://224.0.0.1",
        "wss://255.255.255.255",
        "wss://100.64.0.1",
        "wss://192.0.0.8",
        "wss://198.18.0.1",
        "wss://240.0.0.1",
        "wss://[::]",
        "wss://[ff02::1]",
        "wss://[fe80::1]",
        "wss://[2001:db8::1]",
        "wss://[2001:1::1]",
        "wss://[::ffff:192.168.1.10]",
    ] {
        assert!(matches!(
            RadrootsRelayUrl::parse(relay_url, RadrootsRelayUrlPolicy::Public),
            Err(RadrootsRelayTransportError::RelayUrlForbiddenDestination { .. })
        ));
    }
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
    assert!(matches!(
        public_relay.validate_public_resolved_ip_addrs([IpAddr::V6(
            "::ffff:192.168.1.10"
                .parse::<Ipv6Addr>()
                .expect("mapped ipv6")
        )]),
        Err(RadrootsRelayTransportError::RelayUrlResolvedForbiddenDestination { .. })
    ));
    public_relay
        .validate_public_resolved_ip_addrs([IpAddr::V6(
            "2001:4860:4860::8888"
                .parse::<Ipv6Addr>()
                .expect("public ipv6"),
        )])
        .expect("public resolved ipv6");
    public_relay
        .validate_public_resolved_ip_addrs(Vec::<IpAddr>::new())
        .expect("empty resolved set");

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
    let labels = [
        (RadrootsRelayOutcomeKind::Accepted, "accepted"),
        (
            RadrootsRelayOutcomeKind::DuplicateAccepted,
            "duplicate_accepted",
        ),
        (RadrootsRelayOutcomeKind::Blocked, "blocked"),
        (RadrootsRelayOutcomeKind::RateLimited, "rate_limited"),
        (RadrootsRelayOutcomeKind::Invalid, "invalid"),
        (RadrootsRelayOutcomeKind::PowRequired, "pow_required"),
        (RadrootsRelayOutcomeKind::Restricted, "restricted"),
        (RadrootsRelayOutcomeKind::AuthRequired, "auth_required"),
        (RadrootsRelayOutcomeKind::Muted, "muted"),
        (RadrootsRelayOutcomeKind::Unsupported, "unsupported"),
        (
            RadrootsRelayOutcomeKind::PaymentRequired,
            "payment_required",
        ),
        (RadrootsRelayOutcomeKind::Error, "error"),
        (RadrootsRelayOutcomeKind::Timeout, "timeout"),
        (
            RadrootsRelayOutcomeKind::ConnectionFailed,
            "connection_failed",
        ),
        (
            RadrootsRelayOutcomeKind::RelayUrlRejected,
            "relay_url_rejected",
        ),
        (
            RadrootsRelayOutcomeKind::SkippedAlreadyAccepted,
            "skipped_already_accepted",
        ),
        (RadrootsRelayOutcomeKind::Unknown, "unknown"),
    ];
    for (kind, label) in labels {
        assert_eq!(kind.as_str(), label);
    }

    assert!(RadrootsRelayOutcome::classify("duplicate: already have it").counts_toward_quorum());
    assert!(
        RadrootsRelayOutcome::skipped_already_accepted("already accepted").counts_toward_quorum()
    );
    assert!(RadrootsRelayOutcome::classify("auth-required: challenge").is_retryable());
    assert!(RadrootsRelayOutcome::classify("restricted: denied").is_terminal_failure());
    assert!(RadrootsRelayOutcome::relay_url_rejected("unsafe relay").is_terminal_failure());
    assert!(RadrootsRelayOutcome::classify("mute: pubkey muted").is_terminal_failure());
    assert_eq!(
        RadrootsRelayOutcome::accepted().to_transport_outcome().kind,
        radroots_transport::RadrootsTransportOutcomeKind::Accepted
    );
    assert_eq!(
        RadrootsRelayOutcome::accepted()
            .to_transport_outcome()
            .status,
        radroots_transport::RadrootsTransportDeliveryTargetStatus::Accepted
    );
    assert_eq!(
        RadrootsRelayOutcome::timeout("timeout: no OK")
            .to_transport_outcome()
            .kind,
        radroots_transport::RadrootsTransportOutcomeKind::Timeout
    );
    assert_eq!(
        RadrootsRelayOutcome::timeout("timeout: no OK")
            .to_transport_outcome()
            .status,
        radroots_transport::RadrootsTransportDeliveryTargetStatus::FailedRetryable
    );
    assert_eq!(
        RadrootsRelayOutcome::classify("restricted: denied")
            .to_transport_outcome()
            .kind,
        radroots_transport::RadrootsTransportOutcomeKind::Rejected
    );
    assert_eq!(
        RadrootsRelayOutcome::classify("restricted: denied")
            .to_transport_outcome()
            .status,
        radroots_transport::RadrootsTransportDeliveryTargetStatus::FailedTerminal
    );
    assert_eq!(
        RadrootsRelayOutcome::relay_url_rejected("unsafe")
            .to_transport_outcome()
            .kind,
        radroots_transport::RadrootsTransportOutcomeKind::RouteUnavailable
    );
    assert_eq!(
        RadrootsRelayOutcome::connection_failed("offline")
            .kind
            .as_str(),
        "connection_failed"
    );
    assert_eq!(
        RadrootsRelayOutcome::relay_url_rejected("unsafe")
            .kind
            .as_str(),
        "relay_url_rejected"
    );
}

#[test]
fn relay_transport_error_wraps_transport_contract_errors() {
    let error = RadrootsRelayTransportError::from(
        radroots_transport::RadrootsTransportError::EmptyTargetSet,
    );

    assert_eq!(
        error.to_string(),
        "Transport contract error: transport target set is empty"
    );
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
        radroots_transport_nostr::RadrootsRelayPublishRequest::new(signed.clone(), targets, 1_000)
            .with_satisfaction_policy(RadrootsTransportSatisfactionPolicy::quorum_accepted(2)),
    )
    .await
    .expect("publish");

    assert_eq!(
        adapter.captured_raw_events(),
        vec![signed.raw_json().to_owned()]
    );
    assert_eq!(receipt.attempted_count, 3);
    assert_eq!(receipt.accepted_count, 2);
    assert_eq!(receipt.retryable_count, 1);
    assert!(receipt.quorum_met);
    serde_json::to_string(&receipt).expect("receipt json");
}

#[tokio::test]
async fn nostr_transport_facade_delivers_signed_event_payloads() {
    let signed = signed_post("facade payload");
    let adapter = RadrootsMockRelayPublishAdapter::new();
    let transport = RadrootsNostrTransport::new(&adapter);
    let target = nostr_target(RELAY_PRIMARY_WSS);
    let request = RadrootsTransportDeliveryRequest::new(
        "facade-request-1",
        RadrootsTransportPayload::unchecked_signed_event_json(
            signed.id_str().to_owned(),
            signed.raw_json().to_owned(),
        )
        .expect("payload"),
        RadrootsTransportTargetSet::new(vec![target.clone()]).expect("targets"),
        RadrootsTransportSatisfactionPolicy::all_accepted(),
    );

    let receipt = transport.deliver(request).await.expect("delivery");
    let status = transport.status().await.expect("status");

    assert_eq!(
        adapter.captured_raw_events(),
        vec![signed.raw_json().to_owned()]
    );
    assert!(status.capabilities.deliver);
    assert!(!status.capabilities.fetch);
    assert_eq!(receipt.request_id, "facade-request-1");
    assert_eq!(receipt.target_receipts.len(), 1);
    assert_eq!(receipt.target_receipts[0].target, target);
    assert_eq!(
        receipt.target_receipts[0].outcome.kind,
        radroots_transport::RadrootsTransportOutcomeKind::Accepted
    );
    assert!(
        receipt
            .is_satisfied_by(&RadrootsTransportSatisfactionPolicy::all_accepted())
            .expect("satisfaction")
    );
}

#[test]
fn verified_signed_event_payload_preserves_transport_payload_identity() {
    let signed = signed_post("verified payload");
    let payload = verified_signed_event_payload(&signed).expect("verified payload");
    let RadrootsTransportPayload::SignedEventJson {
        event_id,
        raw_json,
        digest,
    } = payload
    else {
        panic!("signed event payload expected");
    };

    assert_eq!(event_id, signed.id_str());
    assert_eq!(raw_json, signed.raw_json().to_owned());
    assert_eq!(digest.len(), 64);

    let mismatched =
        RadrootsSignedEvent::from_wire_unchecked(signed.wire().clone(), "{}").expect("mismatch");
    assert_eq!(
        verified_signed_event_payload(&mismatched).expect_err("mismatched raw json"),
        RadrootsTransportError::InvalidPayloadBytes
    );
}

#[tokio::test]
async fn nostr_transport_facade_reports_fetch_as_unsupported_operation() {
    let transport = RadrootsNostrTransport::new(RadrootsMockRelayPublishAdapter::new());
    let target_set =
        RadrootsTransportTargetSet::new(vec![nostr_target(RELAY_PRIMARY_WSS)]).expect("targets");
    let error = transport
        .fetch(RadrootsTransportFetchRequest::new(
            "facade-fetch-unsupported",
            target_set,
        ))
        .await
        .expect_err("fetch unsupported");

    assert_eq!(error, RadrootsTransportError::UnsupportedOperation);
}

#[tokio::test]
async fn nostr_transport_facade_rejects_unsupported_payloads_and_targets() {
    let signed = signed_post("facade rejected");
    let transport = RadrootsNostrTransport::new(RadrootsMockRelayPublishAdapter::new());
    let target_set =
        RadrootsTransportTargetSet::new(vec![nostr_target(RELAY_PRIMARY_WSS)]).expect("targets");
    let payload_error = transport
        .deliver(RadrootsTransportDeliveryRequest::new(
            "facade-request-payload",
            RadrootsTransportPayload::opaque_bytes("not-signed-event", [1, 2, 3]).expect("payload"),
            target_set,
            RadrootsTransportSatisfactionPolicy::all_accepted(),
        ))
        .await
        .expect_err("payload rejected");
    assert_eq!(payload_error, RadrootsTransportError::InvalidPayloadBytes);

    let non_nostr_target = RadrootsTransportTarget::reticulum_preview().expect("reticulum target");
    let target_error = transport
        .deliver(RadrootsTransportDeliveryRequest::new(
            "facade-request-target",
            RadrootsTransportPayload::unchecked_signed_event_json(
                signed.id_str().to_owned(),
                signed.raw_json().to_owned(),
            )
            .expect("payload"),
            RadrootsTransportTargetSet::new(vec![non_nostr_target]).expect("targets"),
            RadrootsTransportSatisfactionPolicy::all_accepted(),
        ))
        .await
        .expect_err("target rejected");
    assert_eq!(target_error, RadrootsTransportError::InvalidTargetUri);
}

#[tokio::test]
async fn nostr_transport_facade_matches_canonical_equivalent_relay_receipts() {
    let signed = signed_post("facade canonical receipt");
    let target = nostr_target(RELAY_PRIMARY_WSS);
    let policy = RadrootsTransportSatisfactionPolicy::required_targets(
        RadrootsTransportSatisfactionClass::Accepted,
        vec![target.fingerprint.clone()],
    )
    .expect("required target policy");
    let transport = RadrootsNostrTransport::new(SlashSpelledRelayReceiptPublishAdapter);
    let receipt = transport
        .deliver(RadrootsTransportDeliveryRequest::new(
            "facade-canonical-receipt",
            RadrootsTransportPayload::unchecked_signed_event_json(
                signed.id_str().to_owned(),
                signed.raw_json().to_owned(),
            )
            .expect("payload"),
            RadrootsTransportTargetSet::new(vec![target.clone()]).expect("target set"),
            policy.clone(),
        ))
        .await
        .expect("delivery");

    assert_eq!(receipt.target_receipts.len(), 1);
    assert_eq!(receipt.target_receipts[0].target, target);
    assert_eq!(
        receipt.target_receipts[0].status,
        radroots_transport::RadrootsTransportDeliveryTargetStatus::Accepted
    );
    assert!(receipt.is_satisfied_by(&policy).expect("satisfaction"));

    let relay_receipt = publish_signed_event(
        &SlashSpelledRelayReceiptPublishAdapter,
        RadrootsRelayPublishRequest::new(
            signed,
            RadrootsRelayTargetSet::new(vec![RELAY_PRIMARY_WSS], RadrootsRelayUrlPolicy::Public)
                .expect("targets"),
            1_070,
        )
        .with_satisfaction_policy(policy),
    )
    .await
    .expect("relay publish");
    assert!(relay_receipt.quorum_met);
}

#[tokio::test]
async fn nostr_transport_facade_preserves_scoped_duplicate_target_metadata() {
    let signed = signed_post("facade scoped duplicate");
    let adapter = RadrootsMockRelayPublishAdapter::new();
    let transport = RadrootsNostrTransport::new(&adapter);
    let first = scoped_nostr_target(RELAY_PRIMARY_WSS, "local_food_buyers", "buyers");
    let second = scoped_nostr_target(RELAY_PRIMARY_WSS, "local_food_farmers", "farmers");
    let policy = RadrootsTransportSatisfactionPolicy::required_targets(
        RadrootsTransportSatisfactionClass::Accepted,
        vec![first.fingerprint.clone(), second.fingerprint.clone()],
    )
    .expect("required targets");
    let request = RadrootsTransportDeliveryRequest::new(
        "facade-request-scoped",
        RadrootsTransportPayload::unchecked_signed_event_json(
            signed.id_str().to_owned(),
            signed.raw_json().to_owned(),
        )
        .expect("payload"),
        RadrootsTransportTargetSet::new(vec![first.clone(), second.clone()]).expect("targets"),
        policy.clone(),
    );

    let receipt = transport.deliver(request).await.expect("delivery");

    assert_eq!(receipt.target_receipts.len(), 2);
    assert_eq!(receipt.target_receipts[0].target, first);
    assert_eq!(receipt.target_receipts[1].target, second);
    assert!(receipt.is_satisfied_by(&policy).expect("satisfaction"));
    assert_eq!(adapter.captured_raw_events().len(), 1);
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
        RadrootsRelayPublishRequest::new(signed.clone(), targets, 1_050)
            .with_satisfaction_policy(RadrootsTransportSatisfactionPolicy::all_accepted()),
    )
    .await
    .expect("publish");

    assert_eq!(receipt.event_id, signed.id_str());
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
async fn publish_required_target_policy_uses_relay_fingerprints() {
    let signed = signed_post("required relay");
    let required_target =
        RadrootsTransportTarget::nostr_relay(RELAY_PRIMARY_WSS).expect("required target");
    let targets = RadrootsRelayTargetSet::new(
        vec![RELAY_PRIMARY_WSS, RELAY_SECONDARY_WSS],
        RadrootsRelayUrlPolicy::Public,
    )
    .expect("targets");
    let adapter = RadrootsMockRelayPublishAdapter::new()
        .with_outcome(
            RELAY_PRIMARY_WSS,
            RadrootsRelayOutcome::classify("restricted: required relay rejected"),
        )
        .with_outcome(RELAY_SECONDARY_WSS, RadrootsRelayOutcome::accepted());

    let receipt = publish_signed_event(
        &adapter,
        RadrootsRelayPublishRequest::new(signed, targets, 1_070).with_satisfaction_policy(
            RadrootsTransportSatisfactionPolicy::required_targets(
                RadrootsTransportSatisfactionClass::Accepted,
                vec![required_target.fingerprint],
            )
            .expect("required relay policy"),
        ),
    )
    .await
    .expect("publish");

    assert_eq!(receipt.accepted_count, 1);
    assert_eq!(receipt.quorum, 1);
    assert!(!receipt.quorum_met);
}

#[tokio::test]
async fn publish_all_policy_uses_requested_target_count() {
    let signed = signed_post("partial adapter");
    let targets = RadrootsRelayTargetSet::new(
        vec![RELAY_PRIMARY_WSS, RELAY_SECONDARY_WSS],
        RadrootsRelayUrlPolicy::Public,
    )
    .expect("targets");

    let receipt = publish_signed_event(
        &PartialPublishAdapter,
        RadrootsRelayPublishRequest::new(signed, targets, 1_080)
            .with_satisfaction_policy(RadrootsTransportSatisfactionPolicy::all_accepted()),
    )
    .await
    .expect("publish");

    assert_eq!(receipt.attempted_count, 1);
    assert_eq!(receipt.accepted_count, 1);
    assert_eq!(receipt.quorum, 2);
    assert!(!receipt.quorum_met);
}

#[test]
fn fetch_requests_reject_empty_filter_sets() {
    assert!(matches!(
        RadrootsRelayFetchRequest::fetch(1_000, 10, Vec::<RadrootsNostrFilter>::new()),
        Err(RadrootsRelayTransportError::EmptyFetchFilters)
    ));
    assert!(matches!(
        RadrootsRelayFetchRequest::subscription(1_000, 10, Vec::<RadrootsNostrFilter>::new()),
        Err(RadrootsRelayTransportError::EmptyFetchFilters)
    ));
}

#[test]
fn fetch_requests_reject_zero_limits_and_timeouts() {
    let filter = post_relay_fetch_filter(1);
    let filters = RadrootsRelayFetchFilters::new([filter.clone()]).expect("filters");
    let as_ref_filters: &[RadrootsNostrFilter] = filters.as_ref();
    assert_eq!(as_ref_filters.len(), 1);

    assert!(matches!(
        RadrootsRelayFetchRequest::fetch(1_000, 0, [filter.clone()]),
        Err(RadrootsRelayTransportError::InvalidFetchLimit { field }) if field == "max_events"
    ));
    assert!(matches!(
        RadrootsRelayFetchRequest::subscription(1_000, 0, [filter.clone()]),
        Err(RadrootsRelayTransportError::InvalidFetchLimit { field }) if field == "max_events"
    ));

    let request =
        RadrootsRelayFetchRequest::fetch(1_000, 1, [filter]).expect("valid fetch request");
    assert!(matches!(
        request.clone().with_timeout_ms(0),
        Err(RadrootsRelayTransportError::InvalidFetchLimit { field }) if field == "timeout_ms"
    ));
    assert!(matches!(
        request.clone().with_raw_event_scan_limit(0),
        Err(RadrootsRelayTransportError::InvalidFetchLimit { field }) if field == "max_raw_events"
    ));

    let request = request
        .with_timeout_ms(1)
        .expect("minimum timeout")
        .with_raw_event_scan_limit(1)
        .expect("minimum raw scan limit");
    assert_eq!(request.timeout_ms(), 1);
    assert_eq!(request.max_raw_events(), 1);

    let request = RadrootsRelayFetchRequest::subscription(1_005, 2, [post_relay_fetch_filter(2)])
        .expect("subscription request")
        .with_relay_urls([RELAY_PRIMARY_WSS, RELAY_SECONDARY_WSS])
        .with_timeout_ms(25)
        .expect("timeout")
        .with_raw_event_scan_limit(3)
        .expect("raw limit");
    assert_eq!(request.mode(), RadrootsRelayFetchMode::Subscription);
    assert_eq!(request.observed_at_ms(), 1_005);
    assert_eq!(request.max_events(), 2);
    assert_eq!(request.max_raw_events(), 3);
    assert_eq!(
        request.relay_urls(),
        &[RELAY_PRIMARY_WSS.to_owned(), RELAY_SECONDARY_WSS.to_owned()]
    );
    assert_eq!(request.filters().len(), 1);
    assert_eq!(request.timeout_ms(), 25);
}

#[test]
fn fetch_blocking_facade_runs_mock_adapter() {
    let signed = signed_post("blocking fetch");
    let accepted_id = signed.id_str().to_owned();
    let adapter = RadrootsMockRelayFetchAdapter::new(vec![
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
            raw_json: signed.raw_json().to_owned(),
            observed_at_ms: 1_090,
        },
        RadrootsRelayFetchItem::Eose {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
        },
    ]);

    let receipt = fetch_relay_events_blocking(&adapter, post_relay_fetch_request(1_090, 10))
        .expect("blocking fetch");

    assert_eq!(receipt.events.len(), 1);
    assert_eq!(receipt.events[0].event.id.to_hex(), accepted_id);
    assert_eq!(receipt.connected_relays, vec![RELAY_PRIMARY_WSS]);
}

#[tokio::test]
async fn fetch_ingests_events_and_records_transport_observations() {
    let signed = signed_post("hello");
    let store = RadrootsEventStore::open_memory().await.expect("store");
    let adapter = RadrootsMockRelayFetchAdapter::new(vec![
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
            raw_json: signed.raw_json().to_owned(),
            observed_at_ms: 1_000,
        },
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
            raw_json: signed.raw_json().to_owned(),
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

    let receipt =
        fetch_and_ingest_relay_events(&adapter, &store, fixture_relay_fetch_request(1_000, 10))
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
        .observations_for_event(signed.id_str())
        .await
        .expect("observations");
    assert_eq!(observations.len(), 1);
    assert_eq!(observations[0].transport_kind, RadrootsTransportKind::Nostr);
    assert_eq!(observations[0].endpoint_uri.as_str(), RELAY_PRIMARY_WSS);
    assert_eq!(
        observations[0].observation_type,
        RadrootsTransportObservationType::Fetch
    );
    assert_eq!(observations[0].observation_count, 2);
}

#[tokio::test]
async fn fetch_rejects_out_of_filter_events_before_store_mutation() {
    let accepted = signed_post("filter match");
    let wrong_tag = signed_event_with_kind_and_hashtag("filter wrong tag", KIND_POST, "compost");
    let wrong_kind = signed_raw_event_with_kind_and_hashtag("filter wrong kind", 999, "soil");
    let wrong_kind_event_id = wrong_kind.id.to_hex();
    let store = RadrootsEventStore::open_memory().await.expect("store");
    let adapter = RadrootsMockRelayFetchAdapter::new(vec![
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
            raw_json: wrong_tag.raw_json().to_owned(),
            observed_at_ms: 1_005,
        },
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
            raw_json: accepted.raw_json().to_owned(),
            observed_at_ms: 1_006,
        },
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_SECONDARY_WSS.to_owned(),
            raw_json: wrong_kind.as_json(),
            observed_at_ms: 1_007,
        },
        RadrootsRelayFetchItem::Eose {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
        },
    ]);
    let filter = radroots_nostr_filter_tag(
        RadrootsNostrFilter::new()
            .kind(RadrootsNostrKind::Custom(KIND_POST as u16))
            .limit(10),
        "t",
        vec!["soil".to_owned()],
    )
    .expect("filter");

    let receipt = fetch_and_ingest_relay_events(
        &adapter,
        &store,
        RadrootsRelayFetchRequest::fetch(1_005, 10, [filter]).expect("fetch request"),
    )
    .await
    .expect("fetch ingest");

    assert_eq!(receipt.inserted_count, 1);
    assert_eq!(receipt.out_of_filter_count, 2);
    assert_eq!(receipt.malformed_count, 0);
    assert_eq!(receipt.unsupported_count, 0);
    assert_eq!(receipt.events.len(), 3);
    assert!(receipt.events[0].out_of_filter);
    assert!(!receipt.events[1].out_of_filter);
    assert!(receipt.events[2].out_of_filter);
    assert!(
        store
            .get_event(accepted.id_str())
            .await
            .expect("accepted lookup")
            .is_some()
    );
    assert!(
        store
            .get_event(wrong_tag.id_str())
            .await
            .expect("wrong tag lookup")
            .is_none()
    );
    assert!(
        store
            .get_event(wrong_kind_event_id.as_str())
            .await
            .expect("wrong kind lookup")
            .is_none()
    );
}

#[tokio::test]
async fn fetch_event_cap_counts_accepted_in_filter_events_and_preserves_later_control_outcomes() {
    let accepted = signed_post("accepted capped event");
    let skipped = signed_post("skipped capped event");
    let wrong_tag = signed_event_with_kind_and_hashtag("wrong capped tag", KIND_POST, "compost");
    let accepted_id = accepted.id_str().to_owned();
    let skipped_id = skipped.id_str().to_owned();
    let wrong_tag_id = wrong_tag.id_str().to_owned();
    let store = RadrootsEventStore::open_memory().await.expect("store");
    let adapter = RadrootsMockRelayFetchAdapter::new(vec![
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
            raw_json: "{not json".to_owned(),
            observed_at_ms: 1_099,
        },
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
            raw_json: wrong_tag.raw_json().to_owned(),
            observed_at_ms: 1_100,
        },
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
            raw_json: accepted.raw_json().to_owned(),
            observed_at_ms: 1_101,
        },
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
            raw_json: skipped.raw_json().to_owned(),
            observed_at_ms: 1_102,
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
        fetch_and_ingest_relay_events(&adapter, &store, post_relay_fetch_request(1_100, 1))
            .await
            .expect("fetch ingest");

    assert_eq!(receipt.inserted_count, 1);
    assert_eq!(receipt.duplicate_count, 0);
    assert_eq!(receipt.unsupported_count, 0);
    assert_eq!(receipt.malformed_count, 1);
    assert_eq!(receipt.out_of_filter_count, 1);
    assert_eq!(receipt.skipped_over_limit_count, 1);
    assert_eq!(receipt.events.len(), 4);
    assert!(receipt.events[0].malformed);
    assert!(receipt.events[1].out_of_filter);
    assert!(receipt.events[2].inserted);
    assert!(receipt.events[3].skipped_over_limit);
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
    assert!(
        store
            .get_event(accepted_id.as_str())
            .await
            .expect("accepted lookup")
            .is_some()
    );
    assert!(
        store
            .get_event(skipped_id.as_str())
            .await
            .expect("skipped lookup")
            .is_none()
    );
    assert!(
        store
            .get_event(wrong_tag_id.as_str())
            .await
            .expect("wrong tag lookup")
            .is_none()
    );
}

#[tokio::test]
async fn fetch_relay_events_applies_shared_filter_limit_and_outcome_evidence() {
    let accepted = signed_event_with_kind_and_hashtag("shared fetch accepted", KIND_POST, "soil");
    let skipped = signed_event_with_kind_and_hashtag("shared fetch skipped", KIND_POST, "soil");
    let wrong_tag =
        signed_event_with_kind_and_hashtag("shared fetch wrong tag", KIND_POST, "compost");
    let filter = radroots_nostr_filter_tag(
        RadrootsNostrFilter::new()
            .kind(RadrootsNostrKind::Custom(KIND_POST as u16))
            .limit(10),
        "t",
        vec!["soil".to_owned()],
    )
    .expect("filter");
    let accepted_id = accepted.id_str().to_owned();
    let adapter = RadrootsMockRelayFetchAdapter::new(vec![
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
            raw_json: "{not json".to_owned(),
            observed_at_ms: 2_100,
        },
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
            raw_json: wrong_tag.raw_json().to_owned(),
            observed_at_ms: 2_101,
        },
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
            raw_json: accepted.raw_json().to_owned(),
            observed_at_ms: 2_102,
        },
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
            raw_json: skipped.raw_json().to_owned(),
            observed_at_ms: 2_103,
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

    let receipt = fetch_relay_events(
        &adapter,
        RadrootsRelayFetchRequest::fetch(2_100, 1, [filter])
            .expect("fetch request")
            .with_relay_urls([RELAY_PRIMARY_WSS, RELAY_SECONDARY_WSS]),
    )
    .await
    .expect("fetch events");

    assert_eq!(
        receipt.target_relays,
        vec![RELAY_PRIMARY_WSS, RELAY_SECONDARY_WSS]
    );
    assert_eq!(receipt.connected_relays, vec![RELAY_PRIMARY_WSS]);
    assert_eq!(receipt.failed_relays.len(), 1);
    assert_eq!(receipt.failed_relays[0].relay_url, RELAY_SECONDARY_WSS);
    assert_eq!(receipt.events.len(), 1);
    assert_eq!(receipt.events[0].event.id.to_hex(), accepted_id);
    assert_eq!(receipt.malformed_count, 1);
    assert_eq!(receipt.out_of_filter_count, 1);
    assert_eq!(receipt.skipped_over_limit_count, 1);
    assert_eq!(receipt.eose_count, 1);
    assert_eq!(receipt.closed_count, 1);
    assert_eq!(receipt.notice_count, 1);
    assert_eq!(receipt.event_receipts.len(), 4);
    assert!(receipt.event_receipts[0].malformed);
    assert!(receipt.event_receipts[1].out_of_filter);
    assert!(!receipt.event_receipts[2].malformed);
    assert!(receipt.event_receipts[3].skipped_over_limit);
}

#[tokio::test]
async fn fetch_raw_scan_limit_bounds_noisy_adapter_output() {
    let accepted = signed_post("raw scan accepted event");
    let wrong_tag = signed_event_with_kind_and_hashtag("raw scan wrong tag", KIND_POST, "compost");
    let accepted_id = accepted.id_str().to_owned();
    let store = RadrootsEventStore::open_memory().await.expect("store");
    let adapter = RadrootsMockRelayFetchAdapter::new(vec![
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
            raw_json: "{not json".to_owned(),
            observed_at_ms: 1_130,
        },
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
            raw_json: wrong_tag.raw_json().to_owned(),
            observed_at_ms: 1_131,
        },
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
            raw_json: accepted.raw_json().to_owned(),
            observed_at_ms: 1_132,
        },
        RadrootsRelayFetchItem::Eose {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
        },
    ]);

    let receipt = fetch_and_ingest_relay_events(
        &adapter,
        &store,
        post_relay_fetch_request(1_130, 1)
            .with_raw_event_scan_limit(2)
            .expect("raw scan limit"),
    )
    .await
    .expect("fetch ingest");

    assert_eq!(receipt.inserted_count, 0);
    assert_eq!(receipt.malformed_count, 1);
    assert_eq!(receipt.out_of_filter_count, 1);
    assert_eq!(receipt.skipped_over_limit_count, 1);
    assert_eq!(receipt.events.len(), 2);
    assert_eq!(receipt.eose_count, 1);
    assert!(
        store
            .get_event(accepted_id.as_str())
            .await
            .expect("accepted lookup")
            .is_none()
    );
}

#[tokio::test]
async fn fetch_subscription_mode_and_store_errors_are_reported() {
    let signed = signed_post("subscription");
    let store = RadrootsEventStore::open_memory().await.expect("store");
    let adapter = RadrootsMockRelayFetchAdapter::new(vec![RadrootsRelayFetchItem::Event {
        relay_url: RELAY_PRIMARY_WSS.to_owned(),
        raw_json: signed.raw_json().to_owned(),
        observed_at_ms: 1_200,
    }]);

    let receipt = fetch_and_ingest_relay_events(
        &adapter,
        &store,
        RadrootsRelayFetchRequest::subscription(1_200, 10, [post_relay_fetch_filter(10)])
            .expect("subscription request"),
    )
    .await
    .expect("fetch ingest");

    assert_eq!(receipt.inserted_count, 1);
    let observations = store
        .observations_for_event(signed.id_str())
        .await
        .expect("observations");
    assert_eq!(observations.len(), 1);
    assert_eq!(
        observations[0].observation_type,
        RadrootsTransportObservationType::Subscription
    );

    let closed_store = RadrootsEventStore::open_memory().await.expect("store");
    closed_store.pool().close().await;
    let adapter = RadrootsMockRelayFetchAdapter::new(vec![RadrootsRelayFetchItem::Event {
        relay_url: RELAY_PRIMARY_WSS.to_owned(),
        raw_json: signed.raw_json().to_owned(),
        observed_at_ms: 1_210,
    }]);
    let receipt =
        fetch_and_ingest_relay_events(&adapter, &closed_store, post_relay_fetch_request(1_210, 10))
            .await
            .expect("fetch ingest");

    assert_eq!(receipt.inserted_count, 0);
    assert_eq!(receipt.malformed_count, 1);
    assert!(receipt.events[0].malformed);
    assert!(receipt.events[0].message.is_some());
}

#[tokio::test]
async fn fetch_ingest_rejects_invalid_observation_endpoint() {
    let signed = signed_post("invalid observation endpoint");
    let store = RadrootsEventStore::open_memory().await.expect("store");
    let adapter = RadrootsMockRelayFetchAdapter::new(vec![RadrootsRelayFetchItem::Event {
        relay_url: " ".to_owned(),
        raw_json: signed.raw_json().to_owned(),
        observed_at_ms: 1_300,
    }]);

    let error =
        fetch_and_ingest_relay_events(&adapter, &store, post_relay_fetch_request(1_300, 10))
            .await
            .expect_err("invalid observation endpoint");

    assert!(matches!(
        error,
        RadrootsRelayTransportError::EventStore(
            radroots_event_store::RadrootsEventStoreError::Transport(
                radroots_transport::RadrootsTransportError::EmptyTargetUri
            )
        )
    ));
}

#[tokio::test]
async fn outbox_publish_persists_partial_success_and_skips_accepted_retry() {
    let signed = signed_post("hello");
    let outbox = RadrootsOutbox::open_memory().await.expect("outbox");
    let store = RadrootsEventStore::open_memory().await.expect("store");
    let draft = RadrootsEventDraft::new(
        "radroots.social.post.v1",
        KIND_POST,
        signed.created_at(),
        signed.tags_as_vec(),
        signed.content().to_owned(),
        signed.pubkey_str(),
    )
    .expect("draft");
    let receipt = outbox
        .enqueue_operation(all_accepted_outbox_operation_input(
            draft,
            vec![
                RELAY_PRIMARY_WSS.to_owned(),
                RELAY_SECONDARY_WSS.to_owned(),
                RELAY_TERTIARY_WSS.to_owned(),
            ],
        ))
        .await
        .expect("enqueue");
    let claimed = outbox
        .claim_next_ready_event("signer", "sign-a", 2_000, 1_000)
        .await
        .expect("claim")
        .expect("claim");
    let signed = complete_claimed_signing(&outbox, &claimed, 1_100).await;
    let publish_claim = outbox
        .claim_next_ready_event("publisher", "publish-a", 3_000, 1_100)
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

    assert_eq!(first.attempted_count, 3);
    assert_eq!(first.accepted_count, 2);
    assert!(!first.quorum_met);
    let event = outbox
        .get_event(receipt.outbox_event_id)
        .await
        .expect("event")
        .expect("event");
    assert_eq!(event.state, RadrootsOutboxEventState::PublishRetryable);

    let targets = outbox
        .delivery_targets(receipt.outbox_event_id)
        .await
        .expect("targets");
    assert_eq!(
        targets
            .iter()
            .find(|target| target.endpoint_uri.as_str() == RELAY_PRIMARY_WSS)
            .expect("primary")
            .status,
        RadrootsOutboxDeliveryTargetStatus::Accepted
    );
    assert_eq!(
        targets
            .iter()
            .find(|target| target.endpoint_uri.as_str() == RELAY_SECONDARY_WSS)
            .expect("secondary")
            .status,
        RadrootsOutboxDeliveryTargetStatus::FailedRetryable
    );
    assert_eq!(
        targets
            .iter()
            .find(|target| target.endpoint_uri.as_str() == RELAY_TERTIARY_WSS)
            .expect("tertiary")
            .status,
        RadrootsOutboxDeliveryTargetStatus::Accepted
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

    assert_eq!(second.local_ingest.event_id, signed.id_str());
    assert_eq!(second.attempted_count, 1);
    assert_eq!(retry_adapter.captured_raw_events().len(), 1);

    let event = outbox
        .get_event(receipt.outbox_event_id)
        .await
        .expect("event")
        .expect("event");
    assert_eq!(event.state, RadrootsOutboxEventState::Published);
    let operation = outbox
        .get_operation(receipt.operation_id)
        .await
        .expect("operation")
        .expect("operation");
    assert_eq!(operation.status, RadrootsOutboxOperationStatus::Complete);

    let observations = store
        .observations_for_event(signed.id_str())
        .await
        .expect("observations");
    assert_outbox_publish_observations(&observations, 3);
}

#[tokio::test]
async fn outbox_publish_fans_out_endpoint_receipts_to_scoped_logical_targets() {
    let signed = signed_post("scoped duplicate relay");
    let outbox = RadrootsOutbox::open_memory().await.expect("outbox");
    let store = RadrootsEventStore::open_memory().await.expect("store");
    let draft = RadrootsEventDraft::new(
        "radroots.social.post.v1",
        KIND_POST,
        signed.created_at(),
        signed.tags_as_vec(),
        signed.content().to_owned(),
        signed.pubkey_str(),
    )
    .expect("draft");
    let receipt = outbox
        .enqueue_operation(RadrootsOutboxOperationInput::new(
            "publish_post",
            draft,
            RadrootsOutboxDeliveryPlanInput::new(
                "transport.nostr.local",
                2,
                RadrootsTransportSatisfactionPolicy::all_accepted(),
                vec![
                    scoped_nostr_target(RELAY_PRIMARY_WSS, "foodshed.west", "West foodshed"),
                    scoped_nostr_target(RELAY_PRIMARY_WSS, "foodshed.east", "East foodshed"),
                ],
            ),
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
    let publish_claim = outbox
        .claim_next_ready_event("publisher", "publish-a", 3_000, 1_100)
        .await
        .expect("claim")
        .expect("publish claim");
    let adapter = RadrootsMockRelayPublishAdapter::new()
        .with_outcome(RELAY_PRIMARY_WSS, RadrootsRelayOutcome::accepted());

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

    assert_eq!(published.local_ingest.event_id, signed.id_str());
    assert_eq!(published.event_id, signed.id_str());
    assert_eq!(published.attempted_count, 2);
    assert_eq!(published.accepted_count, 2);
    assert_eq!(published.retryable_count, 0);
    assert_eq!(published.terminal_count, 0);
    assert_eq!(published.quorum, 2);
    assert!(published.quorum_met);
    assert_eq!(published.relay_receipts.len(), 1);
    assert_eq!(published.relay_receipts[0].relay_url, RELAY_PRIMARY_WSS);
    assert_eq!(published.target_receipts.len(), 2);
    assert!(
        published
            .target_receipts
            .iter()
            .all(|target| target.endpoint_uri == RELAY_PRIMARY_WSS && target.attempted)
    );
    assert!(published.target_receipts.iter().any(|target| {
        target.target_scope.as_deref() == Some("foodshed.west")
            && target.target_label.as_deref() == Some("West foodshed")
    }));
    assert!(published.target_receipts.iter().any(|target| {
        target.target_scope.as_deref() == Some("foodshed.east")
            && target.target_label.as_deref() == Some("East foodshed")
    }));
    assert_eq!(adapter.captured_raw_events().len(), 1);

    let event = outbox
        .get_event(receipt.outbox_event_id)
        .await
        .expect("event")
        .expect("event");
    assert_eq!(event.state, RadrootsOutboxEventState::Published);
    let targets = outbox
        .delivery_targets(receipt.outbox_event_id)
        .await
        .expect("targets");
    assert_eq!(targets.len(), 2);
    assert!(targets.iter().all(|target| {
        target.endpoint_uri.as_str() == RELAY_PRIMARY_WSS
            && target.status == RadrootsOutboxDeliveryTargetStatus::Accepted
            && target.attempt_count == 1
    }));
    assert!(targets.iter().any(|target| {
        target.target_scope.as_ref().map(|scope| scope.as_str()) == Some("foodshed.west")
            && target.target_label.as_ref().map(|label| label.as_str()) == Some("West foodshed")
    }));
    assert!(targets.iter().any(|target| {
        target.target_scope.as_ref().map(|scope| scope.as_str()) == Some("foodshed.east")
            && target.target_label.as_ref().map(|label| label.as_str()) == Some("East foodshed")
    }));
    let observations = store
        .observations_for_event(signed.id_str())
        .await
        .expect("observations");
    assert_outbox_publish_observations(&observations, 1);
}

#[tokio::test]
async fn outbox_publish_required_target_failure_is_not_satisfied_by_optional_success() {
    let signed = signed_post("required target optional success");
    let outbox = RadrootsOutbox::open_memory().await.expect("outbox");
    let store = RadrootsEventStore::open_memory().await.expect("store");
    let draft = RadrootsEventDraft::new(
        "radroots.social.post.v1",
        KIND_POST,
        signed.created_at(),
        signed.tags_as_vec(),
        signed.content().to_owned(),
        signed.pubkey_str(),
    )
    .expect("draft");
    let optional = nostr_target(RELAY_PRIMARY_WSS);
    let required = nostr_target(RELAY_SECONDARY_WSS);
    let receipt = outbox
        .enqueue_operation(RadrootsOutboxOperationInput::new(
            "publish_post",
            draft,
            RadrootsOutboxDeliveryPlanInput::new(
                "transport.nostr.local",
                1,
                RadrootsTransportSatisfactionPolicy::required_targets(
                    RadrootsTransportSatisfactionClass::Accepted,
                    vec![required.fingerprint.clone()],
                )
                .expect("required target policy"),
                vec![optional.clone(), required.clone()],
            ),
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
    let publish_claim = outbox
        .claim_next_ready_event("publisher", "publish-a", 3_000, 1_100)
        .await
        .expect("claim")
        .expect("publish claim");
    let optional_target_id = publish_claim
        .delivery_targets
        .iter()
        .find(|target| target.endpoint_fingerprint == optional.fingerprint)
        .expect("optional target")
        .delivery_target_id;
    outbox
        .mark_delivery_target_accepted(
            publish_claim.outbox_event_id,
            publish_claim.claim_token.as_str(),
            optional_target_id,
            2_000,
        )
        .await
        .expect("optional accepted");

    let adapter = RadrootsMockRelayPublishAdapter::new().with_outcome(
        RELAY_SECONDARY_WSS,
        RadrootsRelayOutcome::timeout("required relay timeout"),
    );
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

    assert_eq!(published.attempted_count, 1);
    assert_eq!(published.accepted_count, 0);
    assert_eq!(published.retryable_count, 1);
    assert_eq!(published.quorum, 1);
    assert!(!published.quorum_met);
    assert_eq!(published.relay_receipts.len(), 1);
    assert_eq!(published.relay_receipts[0].relay_url, RELAY_SECONDARY_WSS);
    let event = outbox
        .get_event(receipt.outbox_event_id)
        .await
        .expect("event")
        .expect("event");
    assert_eq!(event.state, RadrootsOutboxEventState::PublishRetryable);
    let targets = outbox
        .delivery_targets(receipt.outbox_event_id)
        .await
        .expect("targets");
    assert!(targets.iter().any(|target| {
        target.endpoint_fingerprint == optional.fingerprint
            && target.status == RadrootsOutboxDeliveryTargetStatus::Accepted
    }));
    assert!(targets.iter().any(|target| {
        target.endpoint_fingerprint == required.fingerprint
            && target.status == RadrootsOutboxDeliveryTargetStatus::FailedRetryable
    }));
}

#[tokio::test]
async fn outbox_publish_required_target_success_is_not_blocked_by_optional_retryable_failure() {
    let signed = signed_post("required target optional failure");
    let outbox = RadrootsOutbox::open_memory().await.expect("outbox");
    let store = RadrootsEventStore::open_memory().await.expect("store");
    let draft = RadrootsEventDraft::new(
        "radroots.social.post.v1",
        KIND_POST,
        signed.created_at(),
        signed.tags_as_vec(),
        signed.content().to_owned(),
        signed.pubkey_str(),
    )
    .expect("draft");
    let optional = nostr_target(RELAY_PRIMARY_WSS);
    let required = nostr_target(RELAY_SECONDARY_WSS);
    let receipt = outbox
        .enqueue_operation(RadrootsOutboxOperationInput::new(
            "publish_post",
            draft,
            RadrootsOutboxDeliveryPlanInput::new(
                "transport.nostr.local",
                1,
                RadrootsTransportSatisfactionPolicy::required_targets(
                    RadrootsTransportSatisfactionClass::Accepted,
                    vec![required.fingerprint.clone()],
                )
                .expect("required target policy"),
                vec![optional.clone(), required.clone()],
            ),
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
    let publish_claim = outbox
        .claim_next_ready_event("publisher", "publish-a", 3_000, 1_100)
        .await
        .expect("claim")
        .expect("publish claim");
    let optional_target_id = publish_claim
        .delivery_targets
        .iter()
        .find(|target| target.endpoint_fingerprint == optional.fingerprint)
        .expect("optional target")
        .delivery_target_id;
    outbox
        .mark_delivery_target_failed_retryable(
            publish_claim.outbox_event_id,
            publish_claim.claim_token.as_str(),
            optional_target_id,
            "optional relay timeout",
            2_000,
        )
        .await
        .expect("optional retryable");

    let adapter = RadrootsMockRelayPublishAdapter::new()
        .with_outcome(RELAY_SECONDARY_WSS, RadrootsRelayOutcome::accepted());
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

    assert_eq!(published.local_ingest.event_id, signed.id_str());
    assert_eq!(published.attempted_count, 1);
    assert_eq!(published.accepted_count, 1);
    assert_eq!(published.retryable_count, 0);
    assert_eq!(published.quorum, 1);
    assert!(published.quorum_met);
    let event = outbox
        .get_event(receipt.outbox_event_id)
        .await
        .expect("event")
        .expect("event");
    assert_eq!(event.state, RadrootsOutboxEventState::Published);
    let targets = outbox
        .delivery_targets(receipt.outbox_event_id)
        .await
        .expect("targets");
    assert!(targets.iter().any(|target| {
        target.endpoint_fingerprint == optional.fingerprint
            && target.status == RadrootsOutboxDeliveryTargetStatus::FailedRetryable
    }));
    assert!(targets.iter().any(|target| {
        target.endpoint_fingerprint == required.fingerprint
            && target.status == RadrootsOutboxDeliveryTargetStatus::Accepted
    }));
    let observations = store
        .observations_for_event(signed.id_str())
        .await
        .expect("observations");
    assert_outbox_publish_observations(&observations, 1);
}

#[tokio::test]
async fn outbox_publish_required_targets_fan_out_same_endpoint_scoped_receipts() {
    let signed = signed_post("required target scoped duplicate relay");
    let outbox = RadrootsOutbox::open_memory().await.expect("outbox");
    let store = RadrootsEventStore::open_memory().await.expect("store");
    let draft = RadrootsEventDraft::new(
        "radroots.social.post.v1",
        KIND_POST,
        signed.created_at(),
        signed.tags_as_vec(),
        signed.content().to_owned(),
        signed.pubkey_str(),
    )
    .expect("draft");
    let required = scoped_nostr_target(RELAY_PRIMARY_WSS, "foodshed.west", "West foodshed");
    let optional = scoped_nostr_target(RELAY_PRIMARY_WSS, "foodshed.east", "East foodshed");
    let receipt = outbox
        .enqueue_operation(RadrootsOutboxOperationInput::new(
            "publish_post",
            draft,
            RadrootsOutboxDeliveryPlanInput::new(
                "transport.nostr.local",
                1,
                RadrootsTransportSatisfactionPolicy::required_targets(
                    RadrootsTransportSatisfactionClass::Accepted,
                    vec![required.fingerprint.clone()],
                )
                .expect("required target policy"),
                vec![required.clone(), optional.clone()],
            ),
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
    let publish_claim = outbox
        .claim_next_ready_event("publisher", "publish-a", 3_000, 1_100)
        .await
        .expect("claim")
        .expect("publish claim");
    let adapter = RadrootsMockRelayPublishAdapter::new()
        .with_outcome(RELAY_PRIMARY_WSS, RadrootsRelayOutcome::accepted());

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

    assert_eq!(published.attempted_count, 2);
    assert_eq!(published.accepted_count, 2);
    assert_eq!(published.quorum, 1);
    assert!(published.quorum_met);
    assert_eq!(published.relay_receipts.len(), 1);
    assert_eq!(published.target_receipts.len(), 2);
    assert!(published.target_receipts.iter().any(|target| {
        target.endpoint_fingerprint == required.fingerprint
            && target.target_scope.as_deref() == Some("foodshed.west")
    }));
    assert!(published.target_receipts.iter().any(|target| {
        target.endpoint_fingerprint == optional.fingerprint
            && target.target_scope.as_deref() == Some("foodshed.east")
    }));
    let event = outbox
        .get_event(receipt.outbox_event_id)
        .await
        .expect("event")
        .expect("event");
    assert_eq!(event.state, RadrootsOutboxEventState::Published);
    let targets = outbox
        .delivery_targets(receipt.outbox_event_id)
        .await
        .expect("targets");
    assert_eq!(targets.len(), 2);
    assert!(targets.iter().all(|target| {
        target.endpoint_uri.as_str() == RELAY_PRIMARY_WSS
            && target.status == RadrootsOutboxDeliveryTargetStatus::Accepted
    }));
}

#[tokio::test]
async fn outbox_transport_publish_failure_releases_retryable_claim() {
    let signed = signed_post("adapter transport failure");
    let outbox = RadrootsOutbox::open_memory().await.expect("outbox");
    let store = RadrootsEventStore::open_memory().await.expect("store");
    let draft = RadrootsEventDraft::new(
        "radroots.social.post.v1",
        KIND_POST,
        signed.created_at(),
        signed.tags_as_vec(),
        signed.content().to_owned(),
        signed.pubkey_str(),
    )
    .expect("draft");
    let receipt = outbox
        .enqueue_operation(all_accepted_outbox_operation_input(
            draft,
            vec![RELAY_PRIMARY_WSS.to_owned(), RELAY_SECONDARY_WSS.to_owned()],
        ))
        .await
        .expect("enqueue");
    let claimed = outbox
        .claim_next_ready_event("signer", "sign-a", 2_000, 1_000)
        .await
        .expect("claim")
        .expect("claim");
    complete_claimed_signing(&outbox, &claimed, 1_100).await;
    let publish_claim = outbox
        .claim_next_ready_event("publisher", "publish-a", 3_000, 1_100)
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

    assert_eq!(published.attempted_count, 2);
    assert_eq!(published.accepted_count, 0);
    assert_eq!(published.retryable_count, 2);
    assert_eq!(published.terminal_count, 0);
    assert!(!published.quorum_met);
    assert!(
        published
            .relay_receipts
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

    let targets = outbox
        .delivery_targets(receipt.outbox_event_id)
        .await
        .expect("targets");
    assert_eq!(targets.len(), 2);
    assert!(
        targets
            .iter()
            .all(|target| target.status == RadrootsOutboxDeliveryTargetStatus::FailedRetryable)
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
    let draft = RadrootsEventDraft::new(
        "radroots.social.post.v1",
        KIND_POST,
        signed.created_at(),
        signed.tags_as_vec(),
        signed.content().to_owned(),
        signed.pubkey_str(),
    )
    .expect("draft");
    let receipt = outbox
        .enqueue_operation(all_accepted_outbox_operation_input(
            draft,
            vec![RELAY_PRIMARY_WSS.to_owned(), RELAY_SECONDARY_WSS.to_owned()],
        ))
        .await
        .expect("enqueue");
    let claimed = outbox
        .claim_next_ready_event("signer", "sign-a", 2_000, 1_000)
        .await
        .expect("claim")
        .expect("claim");
    let signed = complete_claimed_signing(&outbox, &claimed, 1_100).await;
    let publish_claim = outbox
        .claim_next_ready_event("publisher", "publish-a", 3_000, 1_100)
        .await
        .expect("claim")
        .expect("publish claim");
    let initial_targets = publish_claim.delivery_targets.clone();
    outbox
        .mark_delivery_target_accepted(
            publish_claim.outbox_event_id,
            publish_claim.claim_token.as_str(),
            initial_targets[0].delivery_target_id,
            2_150,
        )
        .await
        .expect("primary accepted");
    outbox
        .mark_delivery_target_accepted(
            publish_claim.outbox_event_id,
            publish_claim.claim_token.as_str(),
            initial_targets[1].delivery_target_id,
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

    assert_eq!(published.local_ingest.event_id, signed.id_str());
    assert_eq!(published.event_id, signed.id_str());
    assert_eq!(published.attempted_count, 0);
    assert_eq!(published.accepted_count, 2);
    assert_eq!(published.quorum, 0);
    assert!(published.quorum_met);
    assert!(published.target_receipts.is_empty());
    assert!(published.relay_receipts.is_empty());
    assert!(adapter.captured_raw_events().is_empty());

    let event = outbox
        .get_event(receipt.outbox_event_id)
        .await
        .expect("event")
        .expect("event");
    assert_eq!(event.state, RadrootsOutboxEventState::Published);
    assert!(event.claim_token.is_none());
    let operation = outbox
        .get_operation(receipt.operation_id)
        .await
        .expect("operation")
        .expect("operation");
    assert_eq!(operation.status, RadrootsOutboxOperationStatus::Complete);
}

#[tokio::test]
async fn outbox_publish_ignores_unknown_adapter_receipts() {
    let signed = signed_post("unknown receipt");
    let outbox = RadrootsOutbox::open_memory().await.expect("outbox");
    let store = RadrootsEventStore::open_memory().await.expect("store");
    let draft = RadrootsEventDraft::new(
        "radroots.social.post.v1",
        KIND_POST,
        signed.created_at(),
        signed.tags_as_vec(),
        signed.content().to_owned(),
        signed.pubkey_str(),
    )
    .expect("draft");
    let receipt = outbox
        .enqueue_operation(all_accepted_outbox_operation_input(
            draft,
            vec![RELAY_PRIMARY_WSS.to_owned()],
        ))
        .await
        .expect("enqueue");
    let claimed = outbox
        .claim_next_ready_event("signer", "sign-a", 2_000, 1_000)
        .await
        .expect("claim")
        .expect("claim");
    let signed = complete_claimed_signing(&outbox, &claimed, 1_100).await;
    let publish_claim = outbox
        .claim_next_ready_event("publisher", "publish-a", 3_000, 1_100)
        .await
        .expect("claim")
        .expect("publish claim");

    let published = publish_claimed_outbox_event(
        &outbox,
        &store,
        &UnknownRelayReceiptPublishAdapter,
        &publish_claim,
        RadrootsOutboxPublishPolicy::new(2_500),
        2_200,
    )
    .await
    .expect("publish");

    assert_eq!(published.attempted_count, 1);
    assert_eq!(published.accepted_count, 1);
    assert_eq!(published.target_receipts.len(), 1);
    assert_eq!(published.relay_receipts.len(), 2);
    assert!(published.quorum_met);
    let event = outbox
        .get_event(receipt.outbox_event_id)
        .await
        .expect("event")
        .expect("event");
    assert_eq!(event.state, RadrootsOutboxEventState::Published);
    let observations = store
        .observations_for_event(signed.id_str())
        .await
        .expect("observations");
    assert_outbox_publish_observations(&observations, 1);
    assert!(observations.iter().any(|observation| {
        observation.observation_type == RadrootsTransportObservationType::PublishAck
            && observation.endpoint_uri.as_str() == RELAY_PRIMARY_WSS
    }));
}

#[tokio::test]
async fn outbox_publish_skips_non_nostr_targets() {
    let signed = signed_post("mixed target");
    let outbox = RadrootsOutbox::open_memory().await.expect("outbox");
    let store = RadrootsEventStore::open_memory().await.expect("store");
    let draft = RadrootsEventDraft::new(
        "radroots.social.post.v1",
        KIND_POST,
        signed.created_at(),
        signed.tags_as_vec(),
        signed.content().to_owned(),
        signed.pubkey_str(),
    )
    .expect("draft");
    let receipt = outbox
        .enqueue_operation(RadrootsOutboxOperationInput::new(
            "publish_post",
            draft,
            RadrootsOutboxDeliveryPlanInput::new(
                "transport.mixed.local",
                1,
                RadrootsTransportSatisfactionPolicy::all_accepted(),
                vec![
                    nostr_target(RELAY_PRIMARY_WSS),
                    RadrootsTransportTarget::reticulum_preview().expect("reticulum target"),
                ],
            ),
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
    let publish_claim = outbox
        .claim_next_ready_event("publisher", "publish-a", 3_000, 1_100)
        .await
        .expect("claim")
        .expect("publish claim");
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

    assert_eq!(published.attempted_count, 1);
    assert_eq!(adapter.captured_raw_events().len(), 1);
    let event = outbox
        .get_event(receipt.outbox_event_id)
        .await
        .expect("event")
        .expect("event");
    assert_eq!(event.state, RadrootsOutboxEventState::Published);
    let targets = outbox
        .delivery_targets(receipt.outbox_event_id)
        .await
        .expect("targets");
    assert!(targets.iter().any(|target| {
        target.transport_kind == RadrootsTransportKind::Reticulum
            && target.status == RadrootsOutboxDeliveryTargetStatus::PreviewUnavailable
    }));
}

#[tokio::test]
async fn outbox_publish_marks_published_when_delivery_plan_satisfaction_is_met_with_failure_diagnostics()
 {
    let signed = signed_post("quorum");
    let outbox = RadrootsOutbox::open_memory().await.expect("outbox");
    let store = RadrootsEventStore::open_memory().await.expect("store");
    let draft = RadrootsEventDraft::new(
        "radroots.social.post.v1",
        KIND_POST,
        signed.created_at(),
        signed.tags_as_vec(),
        signed.content().to_owned(),
        signed.pubkey_str(),
    )
    .expect("draft");
    let receipt = outbox
        .enqueue_operation(outbox_operation_input(
            draft,
            vec![
                RELAY_PRIMARY_WSS.to_owned(),
                RELAY_SECONDARY_WSS.to_owned(),
                RELAY_TERTIARY_WSS.to_owned(),
            ],
            RadrootsTransportSatisfactionPolicy::quorum_accepted(2),
        ))
        .await
        .expect("enqueue");
    let claimed = outbox
        .claim_next_ready_event("signer", "sign-a", 2_000, 1_000)
        .await
        .expect("claim")
        .expect("claim");
    let signed = complete_claimed_signing(&outbox, &claimed, 1_100).await;
    let publish_claim = outbox
        .claim_next_ready_event("publisher", "publish-a", 3_000, 1_100)
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
        RadrootsOutboxPublishPolicy::new(2_500),
        2_200,
    )
    .await
    .expect("publish");

    assert_eq!(published.quorum, 2);
    assert_eq!(published.accepted_count, 2);
    assert_eq!(published.terminal_count, 1);
    assert!(published.quorum_met);

    let event = outbox
        .get_event(receipt.outbox_event_id)
        .await
        .expect("event")
        .expect("event");
    assert_eq!(event.state, RadrootsOutboxEventState::Published);
    assert!(event.claim_token.is_none());
    let operation = outbox
        .get_operation(receipt.operation_id)
        .await
        .expect("operation")
        .expect("operation");
    assert_eq!(operation.status, RadrootsOutboxOperationStatus::Complete);

    let targets = outbox
        .delivery_targets(receipt.outbox_event_id)
        .await
        .expect("targets");
    assert_eq!(
        targets
            .iter()
            .find(|target| target.endpoint_uri.as_str() == RELAY_TERTIARY_WSS)
            .expect("tertiary")
            .status,
        RadrootsOutboxDeliveryTargetStatus::FailedTerminal
    );
    assert!(
        outbox
            .claim_next_ready_event("publisher", "publish-b", 4_000, 2_300)
            .await
            .expect("claim")
            .is_none()
    );

    let observations = store
        .observations_for_event(signed.id_str())
        .await
        .expect("observations");
    assert_outbox_publish_observations(&observations, 2);
}

#[tokio::test]
async fn outbox_publish_republishes_accepted_relays_when_policy_requests_it() {
    let signed = signed_post("republish accepted");
    let outbox = RadrootsOutbox::open_memory().await.expect("outbox");
    let store = RadrootsEventStore::open_memory().await.expect("store");
    let draft = RadrootsEventDraft::new(
        "radroots.social.post.v1",
        KIND_POST,
        signed.created_at(),
        signed.tags_as_vec(),
        signed.content().to_owned(),
        signed.pubkey_str(),
    )
    .expect("draft");
    let receipt = outbox
        .enqueue_operation(all_accepted_outbox_operation_input(
            draft,
            vec![RELAY_PRIMARY_WSS.to_owned(), RELAY_SECONDARY_WSS.to_owned()],
        ))
        .await
        .expect("enqueue");
    let claimed = outbox
        .claim_next_ready_event("signer", "sign-a", 2_000, 1_000)
        .await
        .expect("claim")
        .expect("claim");
    let signed = complete_claimed_signing(&outbox, &claimed, 1_100).await;
    let publish_claim = outbox
        .claim_next_ready_event("publisher", "publish-a", 3_000, 1_100)
        .await
        .expect("claim")
        .expect("publish claim");
    let initial_targets = publish_claim.delivery_targets.clone();
    outbox
        .mark_delivery_target_accepted(
            publish_claim.outbox_event_id,
            publish_claim.claim_token.as_str(),
            initial_targets[0].delivery_target_id,
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

    assert_eq!(published.local_ingest.event_id, signed.id_str());
    assert_eq!(published.attempted_count, 2);
    assert_eq!(published.accepted_count, 2);
    assert_eq!(published.quorum, 1);
    assert!(published.quorum_met);
    assert_eq!(adapter.captured_raw_events().len(), 1);

    let event = outbox
        .get_event(receipt.outbox_event_id)
        .await
        .expect("event")
        .expect("event");
    assert_eq!(event.state, RadrootsOutboxEventState::Published);
    let targets = outbox
        .delivery_targets(receipt.outbox_event_id)
        .await
        .expect("targets");
    assert!(
        targets
            .iter()
            .all(|target| target.status == RadrootsOutboxDeliveryTargetStatus::Accepted)
    );
}

#[tokio::test]
async fn outbox_publish_republish_policy_keeps_terminal_targets_excluded() {
    let signed = signed_post("republish terminal excluded");
    let outbox = RadrootsOutbox::open_memory().await.expect("outbox");
    let store = RadrootsEventStore::open_memory().await.expect("store");
    let draft = RadrootsEventDraft::new(
        "radroots.social.post.v1",
        KIND_POST,
        signed.created_at(),
        signed.tags_as_vec(),
        signed.content().to_owned(),
        signed.pubkey_str(),
    )
    .expect("draft");
    let receipt = outbox
        .enqueue_operation(all_accepted_outbox_operation_input(
            draft,
            vec![RELAY_PRIMARY_WSS.to_owned(), RELAY_SECONDARY_WSS.to_owned()],
        ))
        .await
        .expect("enqueue");
    let claimed = outbox
        .claim_next_ready_event("signer", "sign-a", 2_000, 1_000)
        .await
        .expect("claim")
        .expect("claim");
    complete_claimed_signing(&outbox, &claimed, 1_100).await;
    let publish_claim = outbox
        .claim_next_ready_event("publisher", "publish-a", 3_000, 1_100)
        .await
        .expect("claim")
        .expect("publish claim");
    let initial_targets = publish_claim.delivery_targets.clone();
    outbox
        .mark_delivery_target_accepted(
            publish_claim.outbox_event_id,
            publish_claim.claim_token.as_str(),
            initial_targets[0].delivery_target_id,
            2_150,
        )
        .await
        .expect("primary accepted");
    outbox
        .mark_delivery_target_failed_terminal(
            publish_claim.outbox_event_id,
            publish_claim.claim_token.as_str(),
            initial_targets[1].delivery_target_id,
            "terminal",
            2_151,
        )
        .await
        .expect("secondary terminal");
    let adapter = RadrootsMockRelayPublishAdapter::new()
        .with_outcome(RELAY_PRIMARY_WSS, RadrootsRelayOutcome::accepted())
        .with_outcome(RELAY_SECONDARY_WSS, RadrootsRelayOutcome::accepted());

    let published = publish_claimed_outbox_event(
        &outbox,
        &store,
        &adapter,
        &publish_claim,
        RadrootsOutboxPublishPolicy::new(2_500).republish_accepted_relays(true),
        2_200,
    )
    .await
    .expect("publish");

    assert_eq!(published.attempted_count, 1);
    assert_eq!(published.accepted_count, 1);
    assert_eq!(published.quorum, 1);
    assert!(published.quorum_met);
    assert_eq!(adapter.captured_raw_events().len(), 1);
    let event = outbox
        .get_event(receipt.outbox_event_id)
        .await
        .expect("event")
        .expect("event");
    assert_eq!(event.state, RadrootsOutboxEventState::FailedTerminal);
}

#[tokio::test]
async fn outbox_publish_requires_claimed_signed_event() {
    let signed = signed_post("missing signature");
    let outbox = RadrootsOutbox::open_memory().await.expect("outbox");
    let store = RadrootsEventStore::open_memory().await.expect("store");
    let draft = RadrootsEventDraft::new(
        "radroots.social.post.v1",
        KIND_POST,
        signed.created_at(),
        signed.tags_as_vec(),
        signed.content().to_owned(),
        signed.pubkey_str(),
    )
    .expect("draft");
    let receipt = outbox
        .enqueue_operation(all_accepted_outbox_operation_input(
            draft,
            vec![RELAY_PRIMARY_WSS.to_owned()],
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
    let draft = RadrootsEventDraft::new(
        "radroots.social.post.v1",
        KIND_POST,
        signed.created_at(),
        signed.tags_as_vec(),
        signed.content().to_owned(),
        signed.pubkey_str(),
    )
    .expect("draft");
    let receipt = outbox
        .enqueue_operation(all_accepted_outbox_operation_input(
            draft,
            vec![RELAY_PRIMARY_WSS.to_owned(), RELAY_SECONDARY_WSS.to_owned()],
        ))
        .await
        .expect("enqueue");
    let claimed = outbox
        .claim_next_ready_event("signer", "sign-a", 2_000, 1_000)
        .await
        .expect("claim")
        .expect("claim");
    complete_claimed_signing(&outbox, &claimed, 1_100).await;
    let mut publish_claim = outbox
        .claim_next_ready_event("publisher", "publish-a", 3_000, 1_100)
        .await
        .expect("claim")
        .expect("publish claim");
    publish_claim.delivery_targets.truncate(1);

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
    assert_eq!(event.state, RadrootsOutboxEventState::Publishing);
}

#[tokio::test]
async fn outbox_publish_rejects_invalid_relay_target_uri_before_adapter_publish() {
    let signed = signed_post("invalid relay target");
    let outbox = RadrootsOutbox::open_memory().await.expect("outbox");
    let store = RadrootsEventStore::open_memory().await.expect("store");
    let draft = RadrootsEventDraft::new(
        "radroots.social.post.v1",
        KIND_POST,
        signed.created_at(),
        signed.tags_as_vec(),
        signed.content().to_owned(),
        signed.pubkey_str(),
    )
    .expect("draft");
    let receipt = outbox
        .enqueue_operation(all_accepted_outbox_operation_input(
            draft,
            vec!["ws://127.0.0.1:9999".to_owned()],
        ))
        .await
        .expect("enqueue");
    let claimed = outbox
        .claim_next_ready_event("signer", "sign-a", 2_000, 1_000)
        .await
        .expect("claim")
        .expect("claim");
    complete_claimed_signing(&outbox, &claimed, 1_100).await;
    let publish_claim = outbox
        .claim_next_ready_event("publisher", "publish-a", 3_000, 1_100)
        .await
        .expect("claim")
        .expect("publish claim");
    let adapter = RadrootsMockRelayPublishAdapter::new();

    let error = publish_claimed_outbox_event(
        &outbox,
        &store,
        &adapter,
        &publish_claim,
        RadrootsOutboxPublishPolicy::new(2_500),
        2_200,
    )
    .await
    .expect_err("invalid relay target");

    assert!(matches!(
        error,
        RadrootsRelayTransportError::RelayUrlForbiddenDestination { .. }
    ));
    assert!(adapter.captured_raw_events().is_empty());
    let event = outbox
        .get_event(receipt.outbox_event_id)
        .await
        .expect("event")
        .expect("event");
    assert_eq!(event.state, RadrootsOutboxEventState::Publishing);
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
            raw_json: signed.raw_json().to_owned(),
            observed_at_ms: 10_000 + index,
        });
    }
    let adapter = RadrootsMockRelayFetchAdapter::new(items);
    let receipt =
        fetch_and_ingest_relay_events(&adapter, &store, post_relay_fetch_request(10_000, 1_000))
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
