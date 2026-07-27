use futures::future::BoxFuture;
use nostr::{EventBuilder, JsonUtil};
use radroots_event::draft::{RadrootsEventDraft, RadrootsSignedEvent, RadrootsVerifiedSignedEvent};
use radroots_event::kinds::{
    KIND_DELETION_REQUEST, KIND_FOLLOW, KIND_GEOCHAT, KIND_POST, KIND_PROFILE,
};
use radroots_event::wire::v1::DEFAULT_RAW_JSON_MAX_BYTES;
use radroots_event_store::{
    RadrootsEventStore, RadrootsTransportObservationRow, RadrootsTransportObservationType,
};
use radroots_nostr::prelude::{
    RadrootsNostrFilter, RadrootsNostrKeys, RadrootsNostrKind, RadrootsNostrSecretKey,
    RadrootsNostrTag, RadrootsNostrTagKind, RadrootsNostrTimestamp, radroots_nostr_filter_tag,
    radroots_nostr_sign_frozen_draft,
};
use radroots_outbox::{
    RadrootsOutbox, RadrootsOutboxClaimedEvent, RadrootsOutboxDeliveryPlanInput,
    RadrootsOutboxDeliveryTargetStatus, RadrootsOutboxEventState, RadrootsOutboxOperationInput,
    RadrootsOutboxOperationStatus,
};
use radroots_transport::{
    RADROOTS_TRANSPORT_DIAGNOSTIC_MAX_BYTES, RADROOTS_TRANSPORT_ENDPOINT_URI_MAX_BYTES,
    RADROOTS_TRANSPORT_TARGET_MAX_COUNT, RadrootsTransport, RadrootsTransportDeliveryReceipt,
    RadrootsTransportDeliveryRequest, RadrootsTransportDeliveryTargetStatus,
    RadrootsTransportError, RadrootsTransportFetchReceipt, RadrootsTransportFetchRequest,
    RadrootsTransportFuture, RadrootsTransportImplementationState, RadrootsTransportKind,
    RadrootsTransportMeshScopeId, RadrootsTransportOutcome, RadrootsTransportOutcomeKind,
    RadrootsTransportPayload, RadrootsTransportSatisfactionClass,
    RadrootsTransportSatisfactionPolicy, RadrootsTransportStatus, RadrootsTransportTarget,
    RadrootsTransportTargetLabel, RadrootsTransportTargetReceipt, RadrootsTransportTargetSet,
};
use radroots_transport_nostr::{
    RADROOTS_RELAY_FETCH_EVENT_LIMIT_MAX, RADROOTS_RELAY_FETCH_FILTER_JSON_BYTE_LIMIT_MAX,
    RADROOTS_RELAY_FETCH_FILTER_LIMIT_MAX, RADROOTS_RELAY_FETCH_FILTER_SET_JSON_BYTE_LIMIT_MAX,
    RADROOTS_RELAY_FETCH_RAW_EVENT_LIMIT_MAX, RADROOTS_RELAY_FETCH_RAW_JSON_BYTE_LIMIT_MAX,
    RADROOTS_RELAY_FETCH_TIMEOUT_MS_MAX, RadrootsMockRelayFetchAdapter,
    RadrootsMockRelayPublishAdapter, RadrootsNostrTransport, RadrootsOutboxPublishPolicy,
    RadrootsRelayFetchEventAdmission, RadrootsRelayFetchEventValidStream,
    RadrootsRelayFetchEventVerification, RadrootsRelayFetchEventVisibility,
    RadrootsRelayFetchFilters, RadrootsRelayFetchItem, RadrootsRelayFetchMode,
    RadrootsRelayFetchOutcomeKind, RadrootsRelayFetchRequest, RadrootsRelayOutcome,
    RadrootsRelayOutcomeKind, RadrootsRelayPublishAdapter, RadrootsRelayPublishRelayReceipt,
    RadrootsRelayPublishRequest, RadrootsRelayTargetSet, RadrootsRelayTransportError,
    RadrootsRelayUrl, RadrootsRelayUrlPolicy, fetch_and_ingest_relay_events, fetch_relay_events,
    fetch_relay_events_blocking, publish_claimed_outbox_event,
    publish_claimed_outbox_event_with_transport, publish_signed_event,
    verified_signed_event_payload,
};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

const FIXTURE_ALICE_SECRET_KEY_HEX: &str =
    "10c5304d6c9ae3a1a16f7860f1cc8f5e3a76225a2663b3a989a0d775919b7df5";
const FIXTURE_ALICE_PUBLIC_KEY_HEX: &str =
    "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df";
const RELAY_PRIMARY_WSS: &str = "wss://relay.example.com";
const RELAY_SECONDARY_WSS: &str = "wss://relay-2.example.com";
const RELAY_TERTIARY_WSS: &str = "wss://relay-3.example.com";

fn bounded_relay_outcome(
    outcome: Result<RadrootsRelayOutcome, RadrootsRelayTransportError>,
) -> RadrootsRelayOutcome {
    outcome.expect("bounded relay outcome")
}

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
                .targets()
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

struct DuplicateRelayReceiptPublishAdapter;

impl RadrootsRelayPublishAdapter for DuplicateRelayReceiptPublishAdapter {
    fn publish<'a>(
        &'a self,
        request: RadrootsRelayPublishRequest,
    ) -> BoxFuture<'a, Result<Vec<RadrootsRelayPublishRelayReceipt>, RadrootsRelayTransportError>>
    {
        Box::pin(async move {
            let relay = request
                .targets()
                .relays()
                .first()
                .expect("fixture target")
                .as_str()
                .to_owned();
            Ok(vec![
                RadrootsRelayPublishRelayReceipt::attempted(
                    relay.clone(),
                    RadrootsRelayOutcome::accepted(),
                ),
                RadrootsRelayPublishRelayReceipt::attempted(
                    format!("{relay}/"),
                    RadrootsRelayOutcome::accepted(),
                ),
            ])
        })
    }
}

struct InvalidRelayReceiptPublishAdapter;

impl RadrootsRelayPublishAdapter for InvalidRelayReceiptPublishAdapter {
    fn publish<'a>(
        &'a self,
        _request: RadrootsRelayPublishRequest,
    ) -> BoxFuture<'a, Result<Vec<RadrootsRelayPublishRelayReceipt>, RadrootsRelayTransportError>>
    {
        Box::pin(async {
            Ok(vec![RadrootsRelayPublishRelayReceipt::attempted(
                "not a relay URL",
                RadrootsRelayOutcome::accepted(),
            )])
        })
    }
}

struct SkippedAcceptedRelayReceiptPublishAdapter;

impl RadrootsRelayPublishAdapter for SkippedAcceptedRelayReceiptPublishAdapter {
    fn publish<'a>(
        &'a self,
        request: RadrootsRelayPublishRequest,
    ) -> BoxFuture<'a, Result<Vec<RadrootsRelayPublishRelayReceipt>, RadrootsRelayTransportError>>
    {
        Box::pin(async move {
            let relay = request
                .targets()
                .relays()
                .first()
                .expect("fixture target")
                .as_str();
            Ok(vec![RadrootsRelayPublishRelayReceipt::skipped(
                relay,
                RadrootsRelayOutcome::accepted(),
            )])
        })
    }
}

struct AttemptedSkippedRelayReceiptPublishAdapter;

impl RadrootsRelayPublishAdapter for AttemptedSkippedRelayReceiptPublishAdapter {
    fn publish<'a>(
        &'a self,
        request: RadrootsRelayPublishRequest,
    ) -> BoxFuture<'a, Result<Vec<RadrootsRelayPublishRelayReceipt>, RadrootsRelayTransportError>>
    {
        Box::pin(async move {
            let relay = request
                .targets()
                .relays()
                .first()
                .expect("fixture target")
                .as_str();
            Ok(vec![RadrootsRelayPublishRelayReceipt::attempted(
                relay,
                bounded_relay_outcome(RadrootsRelayOutcome::skipped_already_accepted(
                    "already accepted",
                )),
            )])
        })
    }
}

#[derive(Clone)]
struct ScriptedTransport {
    kind: RadrootsTransportKind,
    outcomes: Vec<RadrootsTransportOutcome>,
}

impl ScriptedTransport {
    fn new(outcomes: Vec<RadrootsTransportOutcome>) -> Self {
        Self {
            kind: RadrootsTransportKind::Nostr,
            outcomes,
        }
    }

    fn with_kind(mut self, kind: RadrootsTransportKind) -> Self {
        self.kind = kind;
        self
    }
}

impl RadrootsTransport for ScriptedTransport {
    fn transport_kind(&self) -> RadrootsTransportKind {
        self.kind.clone()
    }

    fn status<'a>(&'a self) -> RadrootsTransportFuture<'a, RadrootsTransportStatus> {
        Box::pin(async {
            Ok(RadrootsTransportStatus::new(
                RadrootsTransportKind::Nostr,
                true,
                RadrootsTransportImplementationState::Real,
                true,
                "scripted",
            ))
        })
    }

    fn deliver<'a>(
        &'a self,
        request: RadrootsTransportDeliveryRequest,
    ) -> RadrootsTransportFuture<'a, RadrootsTransportDeliveryReceipt> {
        Box::pin(async move {
            let target_receipts = request
                .target_set()
                .targets()
                .iter()
                .cloned()
                .zip(self.outcomes.iter().cloned())
                .map(|(target, outcome)| RadrootsTransportTargetReceipt::new(target, outcome))
                .collect();
            RadrootsTransportDeliveryReceipt::for_request(&request, target_receipts)
        })
    }

    fn fetch<'a>(
        &'a self,
        _request: RadrootsTransportFetchRequest,
    ) -> RadrootsTransportFuture<'a, RadrootsTransportFetchReceipt> {
        Box::pin(async { Err(RadrootsTransportError::UnsupportedOperation) })
    }
}

#[derive(Clone, Copy)]
enum ForgedDeliveryReceipt {
    RequestId,
    TargetSet,
}

struct ForgedReceiptTransport {
    forged: ForgedDeliveryReceipt,
}

impl RadrootsTransport for ForgedReceiptTransport {
    fn transport_kind(&self) -> RadrootsTransportKind {
        RadrootsTransportKind::Nostr
    }

    fn status<'a>(&'a self) -> RadrootsTransportFuture<'a, RadrootsTransportStatus> {
        Box::pin(async {
            Ok(RadrootsTransportStatus::new(
                RadrootsTransportKind::Nostr,
                true,
                RadrootsTransportImplementationState::Real,
                true,
                "forged receipt fixture",
            ))
        })
    }

    fn deliver<'a>(
        &'a self,
        request: RadrootsTransportDeliveryRequest,
    ) -> RadrootsTransportFuture<'a, RadrootsTransportDeliveryReceipt> {
        Box::pin(async move {
            match self.forged {
                ForgedDeliveryReceipt::RequestId => RadrootsTransportDeliveryReceipt::new(
                    "forged-request",
                    request.target_set().clone(),
                    request
                        .target_set()
                        .targets()
                        .iter()
                        .cloned()
                        .map(|target| {
                            RadrootsTransportTargetReceipt::new(
                                target,
                                RadrootsTransportOutcome::new(
                                    RadrootsTransportOutcomeKind::Accepted,
                                ),
                            )
                        })
                        .collect(),
                ),
                ForgedDeliveryReceipt::TargetSet => {
                    let target = nostr_target(RELAY_TERTIARY_WSS);
                    RadrootsTransportDeliveryReceipt::new(
                        request.request_id(),
                        RadrootsTransportTargetSet::new(vec![target.clone()])
                            .expect("forged target set"),
                        vec![RadrootsTransportTargetReceipt::new(
                            target,
                            RadrootsTransportOutcome::new(RadrootsTransportOutcomeKind::Accepted),
                        )],
                    )
                }
            }
        })
    }

    fn fetch<'a>(
        &'a self,
        _request: RadrootsTransportFetchRequest,
    ) -> RadrootsTransportFuture<'a, RadrootsTransportFetchReceipt> {
        Box::pin(async { Err(RadrootsTransportError::UnsupportedOperation) })
    }
}

fn fixture_keys() -> RadrootsNostrKeys {
    let secret_key =
        RadrootsNostrSecretKey::from_hex(FIXTURE_ALICE_SECRET_KEY_HEX).expect("secret key");
    RadrootsNostrKeys::new(secret_key)
}

fn test_event_builder(
    kind: u32,
    content: impl Into<String>,
    tags: Vec<Vec<String>>,
) -> EventBuilder {
    let tags: Vec<_> = tags
        .into_iter()
        .filter(|tag| !tag.is_empty())
        .map(|mut tag| {
            let key = tag.remove(0);
            RadrootsNostrTag::custom(RadrootsNostrTagKind::Custom(key.into()), tag)
        })
        .collect();
    EventBuilder::new(
        RadrootsNostrKind::Custom(u16::try_from(kind).expect("test kind must fit NIP-01")),
        content.into(),
    )
    .tags(tags)
    .allow_self_tagging()
}

fn signed_post(content: &str) -> RadrootsSignedEvent {
    signed_event_with_kind_and_hashtag(content, KIND_POST, "soil")
}

fn signed_ephemeral(content: &str) -> RadrootsSignedEvent {
    let raw_event = test_event_builder(KIND_GEOCHAT, content, Vec::new())
        .custom_created_at(RadrootsNostrTimestamp::from_secs(1_700_000_000))
        .sign_with_keys(&fixture_keys())
        .expect("signed ephemeral event");
    let raw_json = raw_event.as_json();
    let wire = radroots_event::wire::RadrootsNip01EventWire::parse_json(&raw_json).expect("wire");
    RadrootsSignedEvent::from_wire_verified_id(wire, raw_json).expect("signed event")
}

fn generic_draft(content: &str) -> RadrootsEventDraft {
    RadrootsEventDraft::new(
        "radroots.social.follow_list.v1",
        KIND_FOLLOW,
        1_700_000_000,
        Vec::new(),
        serde_json::json!({ "label": content }).to_string(),
        FIXTURE_ALICE_PUBLIC_KEY_HEX,
    )
    .expect("generic draft")
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
    let raw_event = signed_raw_event_with_kind_and_hashtag(content, kind, hashtag);
    let raw_json = raw_event.as_json();
    let wire = radroots_event::wire::RadrootsNip01EventWire::parse_json(&raw_json).expect("wire");
    RadrootsSignedEvent::from_wire_verified_id(wire, raw_json).expect("signed event")
}

fn verified_signed_event(signed_event: RadrootsSignedEvent) -> RadrootsVerifiedSignedEvent {
    signed_event
        .verify_signature()
        .expect("fixture signature must verify")
}

fn signed_raw_event_with_kind_and_hashtag(content: &str, kind: u32, hashtag: &str) -> nostr::Event {
    test_event_builder(
        kind,
        content,
        vec![vec!["t".to_owned(), hashtag.to_owned()]],
    )
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
        return signed_event.into_signed_event();
    }
    let signed_event =
        radroots_nostr_sign_frozen_draft(&fixture_keys(), &claimed.draft).expect("signed event");
    outbox
        .complete_signing(
            claimed.outbox_event_id,
            claimed.claim_token.as_str(),
            verified_signed_event(signed_event),
            now_ms,
        )
        .await
        .expect("complete signing")
        .into_signed_event()
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
    let event = test_event_builder(999, "unsupported", Vec::new())
        .custom_created_at(RadrootsNostrTimestamp::from_secs(1_700_000_001))
        .sign_with_keys(&fixture_keys())
        .expect("signed unsupported event");
    event.as_json()
}

fn invalid_contract_shape_raw_event() -> String {
    let event = test_event_builder(
        KIND_POST,
        "invalid reply",
        vec![
            vec!["t".to_owned(), "soil".to_owned()],
            vec![
                "e".to_owned(),
                "invalid-event-id".to_owned(),
                String::new(),
                "root".to_owned(),
            ],
        ],
    )
    .custom_created_at(RadrootsNostrTimestamp::from_secs(1_700_000_002))
    .sign_with_keys(&fixture_keys())
    .expect("signed contract-invalid event");
    event.as_json()
}

fn post_relay_fetch_filter(limit: usize) -> RadrootsNostrFilter {
    radroots_nostr_filter_tag(
        RadrootsNostrFilter::new()
            .kind(RadrootsNostrKind::Custom(
                u16::try_from(KIND_POST).expect("post kind must fit NIP-01"),
            ))
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

fn relay_fetch_filter_with_json_bytes(target_bytes: usize) -> RadrootsNostrFilter {
    let base = radroots_nostr_filter_tag(RadrootsNostrFilter::new(), "t", vec!["x".to_owned()])
        .expect("base relay fetch filter");
    let base_bytes = base.as_json().len();
    assert!(target_bytes >= base_bytes);
    let filter = radroots_nostr_filter_tag(
        RadrootsNostrFilter::new(),
        "t",
        vec!["x".repeat(target_bytes - base_bytes + 1)],
    )
    .expect("sized relay fetch filter");
    assert_eq!(filter.as_json().len(), target_bytes);
    filter
}

fn fixture_relay_targets() -> RadrootsRelayTargetSet {
    RadrootsRelayTargetSet::new(
        [RELAY_PRIMARY_WSS, RELAY_SECONDARY_WSS, RELAY_TERTIARY_WSS],
        RadrootsRelayUrlPolicy::Public,
    )
    .expect("fixture relay targets")
}

fn primary_relay_target() -> RadrootsRelayTargetSet {
    RadrootsRelayTargetSet::new([RELAY_PRIMARY_WSS], RadrootsRelayUrlPolicy::Public)
        .expect("primary relay target")
}

fn fixture_relay_fetch_request(
    observed_at_ms: i64,
    max_events: usize,
) -> RadrootsRelayFetchRequest {
    RadrootsRelayFetchRequest::fetch(
        observed_at_ms,
        max_events,
        fixture_relay_targets(),
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
        fixture_relay_targets(),
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
    assert!(
        RadrootsRelayUrl::parse("wss://192.168.1.10", RadrootsRelayUrlPolicy::Localhost).is_err()
    );
    assert!(
        RadrootsRelayUrl::parse("wss://relay.example.com", RadrootsRelayUrlPolicy::Localhost)
            .is_err()
    );
    assert!(RadrootsRelayUrl::parse("wss://localhost", RadrootsRelayUrlPolicy::Localhost).is_ok());
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
        "wss://192.88.99.2",
        "wss://198.18.0.1",
        "wss://240.0.0.1",
        "wss://[::]",
        "wss://[64:ff9b::7f00:1]",
        "wss://[64:ff9b::a00:1]",
        "wss://[64:ff9b::5db8:d822]",
        "wss://[64:ff9b:1::1]",
        "wss://[100::1]",
        "wss://[100:0:0:1::1]",
        "wss://[ff02::1]",
        "wss://[fe80::1]",
        "wss://[2001:db8::1]",
        "wss://[2001:1::1]",
        "wss://[2002::1]",
        "wss://[3fff::1]",
        "wss://[5f00::1]",
        "wss://[::ffff:192.168.1.10]",
        "wss://localhost",
        "wss://relay.local",
        "wss://relay.home.arpa",
        "wss://relay",
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
    assert!(matches!(
        public_relay.validate_public_resolved_ip_addrs([IpAddr::V6(
            "64:ff9b::5db8:d822"
                .parse::<Ipv6Addr>()
                .expect("translation prefix"),
        )]),
        Err(RadrootsRelayTransportError::RelayUrlResolvedForbiddenDestination { .. })
    ));
    assert!(matches!(
        public_relay.validate_public_resolved_ip_addrs(Vec::<IpAddr>::new()),
        Err(RadrootsRelayTransportError::RelayUrlResolvedNoAddresses { .. })
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
        vec![RELAY_TERTIARY_WSS, RELAY_PRIMARY_WSS, RELAY_SECONDARY_WSS],
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
    assert!(matches!(
        RadrootsRelayTargetSet::new(
            [RELAY_PRIMARY_WSS, "WSS://Relay.Example.com/"],
            RadrootsRelayUrlPolicy::Public,
        ),
        Err(RadrootsRelayTransportError::DuplicateRelayUrl { .. })
    ));
    assert!(matches!(
        RadrootsRelayTargetSet::from_urls(vec![relay_path.clone(), relay_path]),
        Err(RadrootsRelayTransportError::DuplicateRelayUrl { .. })
    ));
}

#[test]
fn relay_urls_and_target_sets_enforce_resource_bounds() {
    let prefix = "wss://relay.example.com/";
    let exact = format!(
        "{prefix}{}",
        "x".repeat(RADROOTS_TRANSPORT_ENDPOINT_URI_MAX_BYTES - prefix.len())
    );
    let relay = RadrootsRelayUrl::parse(&exact, RadrootsRelayUrlPolicy::Public)
        .expect("maximum-length relay URL");
    assert_eq!(
        relay.as_str().len(),
        RADROOTS_TRANSPORT_ENDPOINT_URI_MAX_BYTES
    );

    let one_over = format!("{exact}x");
    assert!(matches!(
        RadrootsRelayUrl::parse(one_over, RadrootsRelayUrlPolicy::Public),
        Err(RadrootsRelayTransportError::RelayUrlParse { url, reason })
            if url == "<oversized>"
                && reason.contains(&RADROOTS_TRANSPORT_ENDPOINT_URI_MAX_BYTES.to_string())
    ));

    let exact_targets = (0..RADROOTS_TRANSPORT_TARGET_MAX_COUNT)
        .map(|index| format!("wss://relay-{index}.example.com"))
        .collect::<Vec<_>>();
    let targets = RadrootsRelayTargetSet::new(
        exact_targets.iter().map(String::as_str),
        RadrootsRelayUrlPolicy::Public,
    )
    .expect("maximum relay target set");
    assert_eq!(targets.len(), RADROOTS_TRANSPORT_TARGET_MAX_COUNT);

    let one_over_targets = (0..=RADROOTS_TRANSPORT_TARGET_MAX_COUNT)
        .map(|index| format!("wss://relay-{index}.example.com"))
        .collect::<Vec<_>>();
    assert!(matches!(
        RadrootsRelayTargetSet::new(
            one_over_targets.iter().map(String::as_str),
            RadrootsRelayUrlPolicy::Public,
        ),
        Err(RadrootsRelayTransportError::FetchLimitTooLarge {
            field: "relay_target_count",
            max: RADROOTS_TRANSPORT_TARGET_MAX_COUNT,
            actual,
        }) if actual == RADROOTS_TRANSPORT_TARGET_MAX_COUNT + 1
    ));

    let parsed_one_over = one_over_targets
        .iter()
        .map(|url| {
            RadrootsRelayUrl::parse(url, RadrootsRelayUrlPolicy::Public).expect("bounded relay URL")
        })
        .collect::<Vec<_>>();
    assert!(matches!(
        RadrootsRelayTargetSet::from_urls(parsed_one_over),
        Err(RadrootsRelayTransportError::FetchLimitTooLarge {
            field: "relay_target_count",
            max: RADROOTS_TRANSPORT_TARGET_MAX_COUNT,
            actual,
        }) if actual == RADROOTS_TRANSPORT_TARGET_MAX_COUNT + 1
    ));
}

#[test]
fn transport_target_and_relay_adapter_share_canonical_url_identity() {
    for (raw, policy) in [
        (
            "WSS://Relay.Example.com:443/",
            RadrootsRelayUrlPolicy::Public,
        ),
        (
            "wss://relay.example.com/nostr/%2Ffeed",
            RadrootsRelayUrlPolicy::Public,
        ),
        (
            "WSS://[2001:4860:4860:0:0:0:0:8888]:443/",
            RadrootsRelayUrlPolicy::Public,
        ),
        ("ws://LOCALHOST:80/", RadrootsRelayUrlPolicy::Localhost),
        (
            "ws://[0:0:0:0:0:0:0:1]:7777/",
            RadrootsRelayUrlPolicy::Localhost,
        ),
    ] {
        let target = RadrootsTransportTarget::nostr_relay(raw).expect("transport target");
        let relay = RadrootsRelayUrl::parse(raw, policy).expect("relay URL");
        assert_eq!(target.uri().as_str(), relay.as_str(), "{raw}");
    }

    for raw in [
        " wss://relay.example.com",
        "wss://relay.example.com ",
        "wss://relay.example.com/a/./b",
        "wss://relay.example.com/a/../b",
        "wss://relay.example.com/a/%2E/b",
        "wss://relay.example.com\\path",
        "wss://relay.example.com:0",
        "wss://relay.example.com:01",
        "wss://relay.example.com.",
        "wss://xn--fa-hia.example.com",
        "wss://faß.example.com",
        "wss://%65xample.com",
        "wss://relay.example.com/%2f",
        "wss://relay.example.com?subscription=1",
    ] {
        assert!(
            RadrootsTransportTarget::nostr_relay(raw).is_err(),
            "transport target accepted {raw}"
        );
        assert!(
            RadrootsRelayUrl::parse(raw, RadrootsRelayUrlPolicy::Public).is_err(),
            "relay adapter accepted {raw}"
        );
    }
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
        let outcome = bounded_relay_outcome(RadrootsRelayOutcome::classify(message));
        assert_eq!(outcome.kind(), kind);
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

    assert!(
        bounded_relay_outcome(RadrootsRelayOutcome::classify("duplicate: already have it"))
            .counts_toward_quorum()
    );
    assert!(
        bounded_relay_outcome(RadrootsRelayOutcome::skipped_already_accepted(
            "already accepted"
        ))
        .counts_toward_quorum()
    );
    assert!(
        bounded_relay_outcome(RadrootsRelayOutcome::classify("auth-required: challenge"))
            .is_retryable()
    );
    assert!(
        bounded_relay_outcome(RadrootsRelayOutcome::classify("restricted: denied"))
            .is_terminal_failure()
    );
    assert!(
        bounded_relay_outcome(RadrootsRelayOutcome::relay_url_rejected("unsafe relay"))
            .is_terminal_failure()
    );
    assert!(
        bounded_relay_outcome(RadrootsRelayOutcome::classify("mute: pubkey muted"))
            .is_terminal_failure()
    );
    assert_eq!(
        RadrootsRelayOutcome::accepted()
            .to_transport_outcome()
            .expect("bounded outcome")
            .kind(),
        radroots_transport::RadrootsTransportOutcomeKind::Accepted
    );
    assert_eq!(
        RadrootsRelayOutcome::accepted()
            .to_transport_outcome()
            .expect("bounded outcome")
            .status(),
        radroots_transport::RadrootsTransportDeliveryTargetStatus::Accepted
    );
    assert_eq!(
        bounded_relay_outcome(RadrootsRelayOutcome::timeout("timeout: no OK"))
            .to_transport_outcome()
            .expect("bounded outcome")
            .kind(),
        radroots_transport::RadrootsTransportOutcomeKind::Timeout
    );
    assert_eq!(
        bounded_relay_outcome(RadrootsRelayOutcome::timeout("timeout: no OK"))
            .to_transport_outcome()
            .expect("bounded outcome")
            .status(),
        radroots_transport::RadrootsTransportDeliveryTargetStatus::FailedRetryable
    );
    assert_eq!(
        bounded_relay_outcome(RadrootsRelayOutcome::classify("restricted: denied"))
            .to_transport_outcome()
            .expect("bounded outcome")
            .kind(),
        radroots_transport::RadrootsTransportOutcomeKind::Rejected
    );
    assert_eq!(
        bounded_relay_outcome(RadrootsRelayOutcome::classify("restricted: denied"))
            .to_transport_outcome()
            .expect("bounded outcome")
            .status(),
        radroots_transport::RadrootsTransportDeliveryTargetStatus::FailedTerminal
    );
    assert_eq!(
        bounded_relay_outcome(RadrootsRelayOutcome::relay_url_rejected("unsafe"))
            .to_transport_outcome()
            .expect("bounded outcome")
            .kind(),
        radroots_transport::RadrootsTransportOutcomeKind::RouteUnavailable
    );
    assert_eq!(
        bounded_relay_outcome(RadrootsRelayOutcome::connection_failed("offline"))
            .kind()
            .as_str(),
        "connection_failed"
    );
    assert_eq!(
        bounded_relay_outcome(RadrootsRelayOutcome::unknown("adapter omitted receipt"))
            .to_transport_outcome()
            .expect("bounded outcome")
            .kind(),
        radroots_transport::RadrootsTransportOutcomeKind::TransportUnavailable
    );
    assert_eq!(
        bounded_relay_outcome(RadrootsRelayOutcome::relay_url_rejected("unsafe"))
            .kind()
            .as_str(),
        "relay_url_rejected"
    );
}

#[test]
fn relay_outcome_messages_are_bounded_and_strictly_decoded() {
    let exact_message = "x".repeat(RADROOTS_TRANSPORT_DIAGNOSTIC_MAX_BYTES);
    let outcome = RadrootsRelayOutcome::unknown(exact_message.clone())
        .expect("maximum relay outcome message");
    assert_eq!(outcome.kind(), RadrootsRelayOutcomeKind::Unknown);
    assert_eq!(outcome.message(), Some(exact_message.as_str()));

    let wire = serde_json::to_value(&outcome).expect("relay outcome JSON");
    let decoded =
        serde_json::from_value::<RadrootsRelayOutcome>(wire).expect("strict relay outcome reload");
    assert_eq!(decoded, outcome);

    let one_over = "x".repeat(RADROOTS_TRANSPORT_DIAGNOSTIC_MAX_BYTES + 1);
    assert!(matches!(
        RadrootsRelayOutcome::unknown(one_over.clone()),
        Err(RadrootsRelayTransportError::DiagnosticLimitExceeded {
            field: "relay_outcome_message",
            max: RADROOTS_TRANSPORT_DIAGNOSTIC_MAX_BYTES,
            actual,
        }) if actual == RADROOTS_TRANSPORT_DIAGNOSTIC_MAX_BYTES + 1
    ));
    let error = serde_json::from_value::<RadrootsRelayOutcome>(serde_json::json!({
        "kind": "Unknown",
        "message": one_over,
    }))
    .expect_err("oversized relay outcome message rejected");
    assert!(error.to_string().contains("relay_outcome_message"));
    assert!(
        serde_json::from_value::<RadrootsRelayOutcome>(serde_json::json!({
            "kind": "Accepted",
            "message": null,
            "extra": true,
        }))
        .is_err()
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
            bounded_relay_outcome(RadrootsRelayOutcome::classify("duplicate: already have it")),
        )
        .with_outcome(
            RELAY_TERTIARY_WSS,
            bounded_relay_outcome(RadrootsRelayOutcome::classify("auth-required: challenge")),
        );

    let receipt = publish_signed_event(
        &adapter,
        radroots_transport_nostr::RadrootsRelayPublishRequest::new(
            verified_signed_event(signed.clone()),
            targets,
            1_000,
        )
        .expect("publish request")
        .with_satisfaction_policy(
            RadrootsTransportSatisfactionPolicy::quorum_accepted(2).expect("valid quorum"),
        ),
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
    let expected_status = RadrootsTransportStatus::new(
        RadrootsTransportKind::Nostr,
        true,
        RadrootsTransportImplementationState::Real,
        true,
        "fixture ready",
    );
    let transport = RadrootsNostrTransport::new(&adapter).with_status(expected_status.clone());
    assert_eq!(transport.transport_kind(), RadrootsTransportKind::Nostr);
    assert!(transport.adapter().captured_raw_events().is_empty());
    let target = nostr_target(RELAY_PRIMARY_WSS);
    let request = RadrootsTransportDeliveryRequest::new(
        "facade-request-1",
        RadrootsTransportPayload::unchecked_signed_event_json(signed.id_str(), signed.raw_json())
            .expect("payload"),
        RadrootsTransportTargetSet::new(vec![target.clone()]).expect("targets"),
        RadrootsTransportSatisfactionPolicy::all_accepted(),
    )
    .expect("delivery request");

    let receipt = transport.deliver(request).await.expect("delivery");
    let status = transport.status().await.expect("status");

    assert_eq!(
        adapter.captured_raw_events(),
        vec![signed.raw_json().to_owned()]
    );
    assert_eq!(status, expected_status);
    assert_eq!(receipt.request_id(), "facade-request-1");
    assert_eq!(receipt.target_receipts().len(), 1);
    assert_eq!(receipt.target_receipts()[0].target(), &target);
    assert_eq!(
        receipt.target_receipts()[0].outcome().kind(),
        radroots_transport::RadrootsTransportOutcomeKind::Accepted
    );
    assert!(
        receipt
            .is_satisfied_by(&RadrootsTransportSatisfactionPolicy::all_accepted())
            .expect("satisfaction")
    );
}

#[tokio::test]
async fn nostr_transport_facade_rejects_invalid_signature_before_adapter_publish() {
    let signed = signed_post("invalid facade signature");
    let mut wire: serde_json::Value =
        serde_json::from_str(signed.raw_json()).expect("signed event JSON");
    wire["sig"] = serde_json::Value::String("0".repeat(128));
    let invalid_raw_json = serde_json::to_string(&wire).expect("invalid signature JSON");
    let adapter = RadrootsMockRelayPublishAdapter::new();
    let transport = RadrootsNostrTransport::new(&adapter);
    let request = RadrootsTransportDeliveryRequest::new(
        "facade-invalid-signature",
        RadrootsTransportPayload::unchecked_signed_event_json(signed.id_str(), invalid_raw_json)
            .expect("transport payload"),
        RadrootsTransportTargetSet::new(vec![nostr_target(RELAY_PRIMARY_WSS)]).expect("targets"),
        RadrootsTransportSatisfactionPolicy::all_accepted(),
    )
    .expect("delivery request");

    let error = transport
        .deliver(request)
        .await
        .expect_err("invalid signature must fail before publish");

    assert_eq!(error, RadrootsTransportError::InvalidPayloadSignature);
    assert!(adapter.captured_raw_events().is_empty());
}

#[test]
fn verified_signed_event_payload_preserves_transport_payload_identity() {
    let signed = signed_post("verified payload");
    let payload = verified_signed_event_payload(&verified_signed_event(signed.clone()))
        .expect("verified payload");
    let (event_id, raw_json) = payload
        .signed_event_json_parts()
        .expect("signed event payload");

    assert_eq!(event_id, signed.id_str());
    assert_eq!(raw_json, signed.raw_json());
    assert_eq!(payload.digest().len(), 64);
}

#[tokio::test]
async fn nostr_transport_facade_reports_fetch_as_unsupported_operation() {
    let transport = RadrootsNostrTransport::new(RadrootsMockRelayPublishAdapter::new());
    let target_set =
        RadrootsTransportTargetSet::new(vec![nostr_target(RELAY_PRIMARY_WSS)]).expect("targets");
    let error = transport
        .fetch(
            RadrootsTransportFetchRequest::new("facade-fetch-unsupported", target_set)
                .expect("fetch request"),
        )
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
        .deliver(
            RadrootsTransportDeliveryRequest::new(
                "facade-request-payload",
                RadrootsTransportPayload::opaque_bytes("not-signed-event", [1, 2, 3])
                    .expect("payload"),
                target_set,
                RadrootsTransportSatisfactionPolicy::all_accepted(),
            )
            .expect("delivery request"),
        )
        .await
        .expect_err("payload rejected");
    assert_eq!(payload_error, RadrootsTransportError::InvalidPayloadBytes);

    let non_nostr_target = RadrootsTransportTarget::reticulum().expect("reticulum target");
    let target_error = transport
        .deliver(
            RadrootsTransportDeliveryRequest::new(
                "facade-request-target",
                RadrootsTransportPayload::unchecked_signed_event_json(
                    signed.id_str(),
                    signed.raw_json(),
                )
                .expect("payload"),
                RadrootsTransportTargetSet::new(vec![non_nostr_target]).expect("targets"),
                RadrootsTransportSatisfactionPolicy::all_accepted(),
            )
            .expect("delivery request"),
        )
        .await
        .expect_err("target rejected");
    assert_eq!(target_error, RadrootsTransportError::InvalidTargetUri);

    let invalid_json_error = transport
        .deliver(
            RadrootsTransportDeliveryRequest::new(
                "facade-request-invalid-json",
                RadrootsTransportPayload::unchecked_signed_event_json(signed.id_str(), "{}")
                    .expect("payload"),
                RadrootsTransportTargetSet::new(vec![nostr_target(RELAY_PRIMARY_WSS)])
                    .expect("targets"),
                RadrootsTransportSatisfactionPolicy::all_accepted(),
            )
            .expect("delivery request"),
        )
        .await
        .expect_err("invalid event json rejected");
    assert_eq!(
        invalid_json_error,
        RadrootsTransportError::InvalidPayloadBytes
    );

    let mismatched_id_error = transport
        .deliver(
            RadrootsTransportDeliveryRequest::new(
                "facade-request-mismatched-id",
                RadrootsTransportPayload::unchecked_signed_event_json(
                    "00".repeat(32),
                    signed.raw_json(),
                )
                .expect("payload"),
                RadrootsTransportTargetSet::new(vec![nostr_target(RELAY_PRIMARY_WSS)])
                    .expect("targets"),
                RadrootsTransportSatisfactionPolicy::all_accepted(),
            )
            .expect("delivery request"),
        )
        .await
        .expect_err("mismatched event id rejected");
    assert_eq!(
        mismatched_id_error,
        RadrootsTransportError::InvalidPayloadId
    );

    let mut tampered =
        serde_json::from_str::<serde_json::Value>(signed.raw_json()).expect("signed event json");
    tampered["content"] = serde_json::Value::String("tampered".to_owned());
    let tampered_raw = serde_json::to_string(&tampered).expect("tampered event json");
    let tampered_error = transport
        .deliver(
            RadrootsTransportDeliveryRequest::new(
                "facade-request-tampered-event",
                RadrootsTransportPayload::unchecked_signed_event_json(
                    signed.id_str(),
                    tampered_raw,
                )
                .expect("payload"),
                RadrootsTransportTargetSet::new(vec![nostr_target(RELAY_PRIMARY_WSS)])
                    .expect("targets"),
                RadrootsTransportSatisfactionPolicy::all_accepted(),
            )
            .expect("delivery request"),
        )
        .await
        .expect_err("tampered event rejected");
    assert_eq!(tampered_error, RadrootsTransportError::InvalidPayloadBytes);

    let forbidden_target_error = transport
        .deliver(
            RadrootsTransportDeliveryRequest::new(
                "facade-request-forbidden-target",
                RadrootsTransportPayload::unchecked_signed_event_json(
                    signed.id_str(),
                    signed.raw_json(),
                )
                .expect("payload"),
                RadrootsTransportTargetSet::new(vec![nostr_target("wss://127.0.0.1")])
                    .expect("targets"),
                RadrootsTransportSatisfactionPolicy::all_accepted(),
            )
            .expect("delivery request"),
        )
        .await
        .expect_err("forbidden relay rejected");
    assert_eq!(
        forbidden_target_error,
        RadrootsTransportError::InvalidTargetUri
    );

    let local_receipt = transport
        .deliver(
            RadrootsTransportDeliveryRequest::new(
                "facade-request-local-relay",
                RadrootsTransportPayload::unchecked_signed_event_json(
                    signed.id_str(),
                    signed.raw_json(),
                )
                .expect("payload"),
                RadrootsTransportTargetSet::new(vec![nostr_target("ws://127.0.0.1:21002")])
                    .expect("targets"),
                RadrootsTransportSatisfactionPolicy::all_accepted(),
            )
            .expect("delivery request"),
        )
        .await
        .expect("localhost relay accepted");
    assert_eq!(local_receipt.target_receipts().len(), 1);
}

#[tokio::test]
async fn nostr_transport_facade_preserves_adapter_failure_and_omission_evidence() {
    let signed = signed_post("facade failures");
    let payload =
        RadrootsTransportPayload::unchecked_signed_event_json(signed.id_str(), signed.raw_json())
            .expect("payload");
    let targets = RadrootsTransportTargetSet::new(vec![
        nostr_target(RELAY_PRIMARY_WSS),
        nostr_target(RELAY_SECONDARY_WSS),
    ])
    .expect("targets");

    let transport = RadrootsNostrTransport::new(TransportFailurePublishAdapter);
    let failed = transport
        .deliver(
            RadrootsTransportDeliveryRequest::new(
                "facade-transport-failure",
                payload.clone(),
                targets.clone(),
                RadrootsTransportSatisfactionPolicy::all_accepted(),
            )
            .expect("delivery request"),
        )
        .await
        .expect("failure receipts");
    assert_eq!(failed.target_receipts().len(), 2);
    assert!(failed.target_receipts().iter().all(|receipt| {
        receipt.outcome().kind() == RadrootsTransportOutcomeKind::ConnectionFailed
            && receipt.status() == RadrootsTransportDeliveryTargetStatus::FailedRetryable
    }));

    let partial = RadrootsNostrTransport::new(PartialPublishAdapter)
        .deliver(
            RadrootsTransportDeliveryRequest::new(
                "facade-partial",
                payload.clone(),
                targets.clone(),
                RadrootsTransportSatisfactionPolicy::all_accepted(),
            )
            .expect("delivery request"),
        )
        .await
        .expect("partial receipts");
    assert_eq!(partial.target_receipts().len(), 2);
    assert_eq!(
        partial.target_receipts()[1].outcome().kind(),
        RadrootsTransportOutcomeKind::TransportUnavailable
    );

    let error = RadrootsNostrTransport::new(NostrJsonFailurePublishAdapter)
        .deliver(
            RadrootsTransportDeliveryRequest::new(
                "facade-json-failure",
                payload,
                targets,
                RadrootsTransportSatisfactionPolicy::all_accepted(),
            )
            .expect("delivery request"),
        )
        .await
        .expect_err("adapter JSON error");
    assert_eq!(error, RadrootsTransportError::InvalidPayloadBytes);
}

#[tokio::test]
async fn nostr_transport_facade_matches_canonical_equivalent_relay_receipts() {
    let signed = signed_post("facade canonical receipt");
    let target = nostr_target(RELAY_PRIMARY_WSS);
    let policy = RadrootsTransportSatisfactionPolicy::required_targets(
        RadrootsTransportSatisfactionClass::Accepted,
        vec![target.fingerprint().clone()],
    )
    .expect("required target policy");
    let transport = RadrootsNostrTransport::new(SlashSpelledRelayReceiptPublishAdapter);
    let receipt = transport
        .deliver(
            RadrootsTransportDeliveryRequest::new(
                "facade-canonical-receipt",
                RadrootsTransportPayload::unchecked_signed_event_json(
                    signed.id_str(),
                    signed.raw_json(),
                )
                .expect("payload"),
                RadrootsTransportTargetSet::new(vec![target.clone()]).expect("target set"),
                policy.clone(),
            )
            .expect("delivery request"),
        )
        .await
        .expect("delivery");

    assert_eq!(receipt.target_receipts().len(), 1);
    assert_eq!(receipt.target_receipts()[0].target(), &target);
    assert_eq!(
        receipt.target_receipts()[0].status(),
        radroots_transport::RadrootsTransportDeliveryTargetStatus::Accepted
    );
    assert!(receipt.is_satisfied_by(&policy).expect("satisfaction"));

    let relay_receipt = publish_signed_event(
        &SlashSpelledRelayReceiptPublishAdapter,
        RadrootsRelayPublishRequest::new(
            verified_signed_event(signed),
            RadrootsRelayTargetSet::new(vec![RELAY_PRIMARY_WSS], RadrootsRelayUrlPolicy::Public)
                .expect("targets"),
            1_070,
        )
        .expect("publish request")
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
        vec![first.fingerprint().clone(), second.fingerprint().clone()],
    )
    .expect("required targets");
    let request = RadrootsTransportDeliveryRequest::new(
        "facade-request-scoped",
        RadrootsTransportPayload::unchecked_signed_event_json(signed.id_str(), signed.raw_json())
            .expect("payload"),
        RadrootsTransportTargetSet::new(vec![first.clone(), second.clone()]).expect("targets"),
        policy.clone(),
    )
    .expect("delivery request");

    let receipt = transport.deliver(request).await.expect("delivery");

    assert_eq!(receipt.target_receipts().len(), 2);
    assert_eq!(receipt.target_receipts()[0].target(), &first);
    assert_eq!(receipt.target_receipts()[1].target(), &second);
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
        bounded_relay_outcome(RadrootsRelayOutcome::classify(
            "restricted: group write denied",
        )),
    );

    let receipt = publish_signed_event(
        &adapter,
        RadrootsRelayPublishRequest::new(verified_signed_event(signed.clone()), targets, 1_050)
            .expect("publish request")
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
        bounded_relay_outcome(RadrootsRelayOutcome::timeout("timeout: no OK")),
    );
    assert_eq!(skipped.relay_url, RELAY_TERTIARY_WSS);
    assert!(!skipped.attempted);
    assert_eq!(skipped.outcome.kind(), RadrootsRelayOutcomeKind::Timeout);

    let error = publish_signed_event(
        &TransportFailurePublishAdapter,
        RadrootsRelayPublishRequest::new(
            verified_signed_event(signed),
            RadrootsRelayTargetSet::new(vec![RELAY_PRIMARY_WSS], RadrootsRelayUrlPolicy::Public)
                .expect("targets"),
            1_060,
        )
        .expect("publish request"),
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
            bounded_relay_outcome(RadrootsRelayOutcome::classify(
                "restricted: required relay rejected",
            )),
        )
        .with_outcome(RELAY_SECONDARY_WSS, RadrootsRelayOutcome::accepted());

    let receipt = publish_signed_event(
        &adapter,
        RadrootsRelayPublishRequest::new(verified_signed_event(signed), targets, 1_070)
            .expect("publish request")
            .with_satisfaction_policy(
                RadrootsTransportSatisfactionPolicy::required_targets(
                    RadrootsTransportSatisfactionClass::Accepted,
                    vec![required_target.fingerprint().clone()],
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
        RadrootsRelayPublishRequest::new(
            verified_signed_event(signed.clone()),
            targets.clone(),
            1_080,
        )
        .expect("publish request")
        .with_satisfaction_policy(RadrootsTransportSatisfactionPolicy::all_accepted()),
    )
    .await
    .expect("publish");

    assert_eq!(receipt.attempted_count, 1);
    assert_eq!(receipt.accepted_count, 1);
    assert_eq!(receipt.retryable_count, 1);
    assert_eq!(receipt.quorum, 2);
    assert!(!receipt.quorum_met);
    assert_eq!(receipt.relays.len(), 2);
    assert!(!receipt.relays[1].attempted);
    assert_eq!(
        receipt.relays[1].outcome.kind(),
        RadrootsRelayOutcomeKind::Unknown
    );

    let no_wait = publish_signed_event(
        &PartialPublishAdapter,
        RadrootsRelayPublishRequest::new(verified_signed_event(signed), targets, 1_081)
            .expect("publish request")
            .with_satisfaction_policy(RadrootsTransportSatisfactionPolicy::no_wait()),
    )
    .await
    .expect("no-wait publish");
    assert_eq!(no_wait.quorum, 0);
    assert!(no_wait.quorum_met);
}

#[tokio::test]
async fn publish_rejects_untrusted_adapter_receipt_provenance() {
    let signed = signed_post("adapter provenance");
    let request = || {
        RadrootsRelayPublishRequest::new(
            verified_signed_event(signed.clone()),
            primary_relay_target(),
            1_090,
        )
        .expect("publish request")
    };

    assert!(matches!(
        publish_signed_event(&UnknownRelayReceiptPublishAdapter, request()).await,
        Err(RadrootsRelayTransportError::UnexpectedPublishReceiptRelayUrl { url })
            if url == RELAY_TERTIARY_WSS
    ));
    assert!(matches!(
        publish_signed_event(&DuplicateRelayReceiptPublishAdapter, request()).await,
        Err(RadrootsRelayTransportError::DuplicatePublishReceiptRelayUrl { url })
            if url == RELAY_PRIMARY_WSS
    ));
    assert!(matches!(
        publish_signed_event(&InvalidRelayReceiptPublishAdapter, request()).await,
        Err(RadrootsRelayTransportError::InvalidPublishReceiptRelayUrl { url, .. })
            if url == "not a relay URL"
    ));
    assert!(matches!(
        publish_signed_event(&SkippedAcceptedRelayReceiptPublishAdapter, request()).await,
        Err(RadrootsRelayTransportError::InvalidPublishReceiptAttemptState { url })
            if url == RELAY_PRIMARY_WSS
    ));
    assert!(matches!(
        publish_signed_event(&AttemptedSkippedRelayReceiptPublishAdapter, request()).await,
        Err(RadrootsRelayTransportError::InvalidPublishReceiptAttemptState { url })
            if url == RELAY_PRIMARY_WSS
    ));
}

#[test]
fn relay_publish_request_rejects_negative_time() {
    let signed = signed_post("negative publish time");

    assert!(matches!(
        RadrootsRelayPublishRequest::new(verified_signed_event(signed), primary_relay_target(), -1,),
        Err(RadrootsRelayTransportError::InvalidTimestamp {
            field: "now_ms",
            value: -1,
        })
    ));
}

#[test]
fn relay_publish_request_seals_fields_and_validates_idempotency_keys() {
    let signed = signed_post("sealed publish request");
    let request = RadrootsRelayPublishRequest::new(
        verified_signed_event(signed.clone()),
        primary_relay_target(),
        7,
    )
    .expect("publish request");
    assert_eq!(request.signed_event().signed_event(), &signed);
    assert_eq!(request.targets().len(), 1);
    assert_eq!(
        request.satisfaction_policy(),
        &RadrootsTransportSatisfactionPolicy::all_accepted()
    );
    assert_eq!(request.idempotency_key(), None);
    assert_eq!(request.now_ms(), 7);

    for invalid in ["", " ", " leading", "trailing ", "line\nbreak"] {
        assert!(matches!(
            request.clone().try_with_idempotency_key(invalid),
            Err(RadrootsRelayTransportError::InvalidIdempotencyKey { .. })
        ));
    }
    assert!(matches!(
        request.clone().try_with_idempotency_key("x".repeat(
            radroots_transport_nostr::RADROOTS_RELAY_PUBLISH_IDEMPOTENCY_KEY_MAX_BYTES + 1,
        ),),
        Err(RadrootsRelayTransportError::InvalidIdempotencyKey { .. })
    ));
    let request = request
        .try_with_idempotency_key("publish-7")
        .expect("idempotency key");
    assert_eq!(request.idempotency_key(), Some("publish-7"));
}

#[tokio::test]
async fn relay_publish_request_rejects_unrequested_required_target_before_adapter() {
    let signed = signed_post("missing required target");
    let required = RadrootsTransportTarget::nostr_relay(RELAY_SECONDARY_WSS)
        .expect("required target")
        .fingerprint()
        .clone();
    let request =
        RadrootsRelayPublishRequest::new(verified_signed_event(signed), primary_relay_target(), 8)
            .expect("publish request")
            .with_satisfaction_policy(
                RadrootsTransportSatisfactionPolicy::required_targets(
                    RadrootsTransportSatisfactionClass::Accepted,
                    vec![required.clone()],
                )
                .expect("required policy"),
            );
    let adapter = RadrootsMockRelayPublishAdapter::new();

    assert!(matches!(
        publish_signed_event(&adapter, request).await,
        Err(RadrootsRelayTransportError::RequiredTargetNotRequested { fingerprint })
            if fingerprint == required.as_str()
    ));
    assert!(adapter.captured_raw_events().is_empty());
}

#[test]
fn fetch_requests_reject_empty_filter_sets() {
    assert!(matches!(
        RadrootsRelayFetchRequest::fetch(
            1_000,
            10,
            primary_relay_target(),
            Vec::<RadrootsNostrFilter>::new(),
        ),
        Err(RadrootsRelayTransportError::EmptyFetchFilters)
    ));
    assert!(matches!(
        RadrootsRelayFetchRequest::subscription(
            1_000,
            10,
            primary_relay_target(),
            Vec::<RadrootsNostrFilter>::new(),
        ),
        Err(RadrootsRelayTransportError::EmptyFetchFilters)
    ));
}

#[test]
fn fetch_filters_enforce_count_and_compact_json_bounds() {
    let maximum_filter =
        relay_fetch_filter_with_json_bytes(RADROOTS_RELAY_FETCH_FILTER_JSON_BYTE_LIMIT_MAX);
    let exact_filters = vec![maximum_filter.clone(); RADROOTS_RELAY_FETCH_FILTER_LIMIT_MAX];
    let filters = RadrootsRelayFetchFilters::new(exact_filters).expect("maximum filter set");
    assert_eq!(
        filters.as_slice().len(),
        RADROOTS_RELAY_FETCH_FILTER_LIMIT_MAX
    );
    assert_eq!(
        filters
            .as_slice()
            .iter()
            .map(|filter| filter.as_json().len())
            .sum::<usize>(),
        RADROOTS_RELAY_FETCH_FILTER_SET_JSON_BYTE_LIMIT_MAX
    );
    assert_eq!(
        RADROOTS_RELAY_FETCH_FILTER_LIMIT_MAX * RADROOTS_RELAY_FETCH_FILTER_JSON_BYTE_LIMIT_MAX,
        RADROOTS_RELAY_FETCH_FILTER_SET_JSON_BYTE_LIMIT_MAX
    );

    assert!(matches!(
        RadrootsRelayFetchFilters::new(vec![
            post_relay_fetch_filter(1);
            RADROOTS_RELAY_FETCH_FILTER_LIMIT_MAX + 1
        ]),
        Err(RadrootsRelayTransportError::FetchLimitTooLarge {
            field: "filter_count",
            max: RADROOTS_RELAY_FETCH_FILTER_LIMIT_MAX,
            actual,
        }) if actual == RADROOTS_RELAY_FETCH_FILTER_LIMIT_MAX + 1
    ));

    let oversized_filter =
        relay_fetch_filter_with_json_bytes(RADROOTS_RELAY_FETCH_FILTER_JSON_BYTE_LIMIT_MAX + 1);
    assert!(matches!(
        RadrootsRelayFetchFilters::new([oversized_filter]),
        Err(RadrootsRelayTransportError::FetchLimitTooLarge {
            field: "filter_json_bytes",
            max: RADROOTS_RELAY_FETCH_FILTER_JSON_BYTE_LIMIT_MAX,
            actual,
        }) if actual == RADROOTS_RELAY_FETCH_FILTER_JSON_BYTE_LIMIT_MAX + 1
    ));
}

#[test]
fn fetch_requests_reject_zero_limits_and_timeouts() {
    let filter = post_relay_fetch_filter(1);
    let filters = RadrootsRelayFetchFilters::new([filter.clone()]).expect("filters");
    let as_ref_filters: &[RadrootsNostrFilter] = filters.as_ref();
    assert_eq!(as_ref_filters.len(), 1);

    assert!(matches!(
        RadrootsRelayFetchRequest::fetch(-1, 1, primary_relay_target(), [filter.clone()]),
        Err(RadrootsRelayTransportError::InvalidTimestamp {
            field: "observed_at_ms",
            value: -1,
        })
    ));
    assert!(matches!(
        RadrootsRelayFetchRequest::fetch(1_000, 0, primary_relay_target(), [filter.clone()]),
        Err(RadrootsRelayTransportError::InvalidFetchLimit { field }) if field == "max_events"
    ));
    assert!(matches!(
        RadrootsRelayFetchRequest::subscription(
            1_000,
            0,
            primary_relay_target(),
            [filter.clone()],
        ),
        Err(RadrootsRelayTransportError::InvalidFetchLimit { field }) if field == "max_events"
    ));

    let request = RadrootsRelayFetchRequest::fetch(1_000, 1, primary_relay_target(), [filter])
        .expect("valid fetch request");
    assert!(matches!(
        request.clone().with_timeout_ms(0),
        Err(RadrootsRelayTransportError::InvalidFetchLimit { field }) if field == "timeout_ms"
    ));
    let maximum_timeout = request
        .clone()
        .with_timeout_ms(RADROOTS_RELAY_FETCH_TIMEOUT_MS_MAX)
        .expect("maximum timeout");
    assert_eq!(
        maximum_timeout.timeout_ms(),
        RADROOTS_RELAY_FETCH_TIMEOUT_MS_MAX
    );
    assert!(matches!(
        request
            .clone()
            .with_timeout_ms(RADROOTS_RELAY_FETCH_TIMEOUT_MS_MAX + 1),
        Err(RadrootsRelayTransportError::FetchLimitTooLarge {
            field: "timeout_ms",
            max,
            actual,
        }) if max == RADROOTS_RELAY_FETCH_TIMEOUT_MS_MAX as usize
            && actual == RADROOTS_RELAY_FETCH_TIMEOUT_MS_MAX as usize + 1
    ));
    assert!(matches!(
        request.clone().with_raw_event_scan_limit(0),
        Err(RadrootsRelayTransportError::InvalidFetchLimit { field }) if field == "max_raw_events"
    ));
    assert!(matches!(
        request.clone().with_raw_json_byte_limit(0),
        Err(RadrootsRelayTransportError::InvalidFetchLimit { field }) if field == "max_raw_json_bytes"
    ));

    let request = request
        .with_timeout_ms(1)
        .expect("minimum timeout")
        .with_raw_event_scan_limit(1)
        .expect("minimum raw scan limit")
        .with_raw_json_byte_limit(1)
        .expect("minimum raw JSON byte limit");
    assert_eq!(request.timeout_ms(), 1);
    assert_eq!(request.max_raw_events(), 1);
    assert_eq!(request.max_raw_json_bytes(), 1);

    let request = RadrootsRelayFetchRequest::subscription(
        1_005,
        2,
        RadrootsRelayTargetSet::new(
            [RELAY_PRIMARY_WSS, RELAY_SECONDARY_WSS],
            RadrootsRelayUrlPolicy::Public,
        )
        .expect("relay targets"),
        [post_relay_fetch_filter(2)],
    )
    .expect("subscription request")
    .with_timeout_ms(25)
    .expect("timeout")
    .with_raw_event_scan_limit(3)
    .expect("raw limit")
    .with_raw_json_byte_limit(4_096)
    .expect("raw JSON byte limit");
    assert_eq!(request.mode(), RadrootsRelayFetchMode::Subscription);
    assert_eq!(request.observed_at_ms(), 1_005);
    assert_eq!(request.max_events(), 2);
    assert_eq!(request.max_raw_events(), 3);
    assert_eq!(request.max_raw_json_bytes(), 4_096);
    assert_eq!(
        request.relay_targets().relay_strings(),
        vec![RELAY_PRIMARY_WSS.to_owned(), RELAY_SECONDARY_WSS.to_owned()]
    );
    assert_eq!(request.filters().len(), 1);
    assert_eq!(request.timeout_ms(), 25);
}

#[test]
fn fetch_requests_enforce_the_coherent_visibility_batch_limit() {
    let filter = post_relay_fetch_filter(RADROOTS_RELAY_FETCH_EVENT_LIMIT_MAX);
    let fetch = RadrootsRelayFetchRequest::fetch(
        1_006,
        RADROOTS_RELAY_FETCH_EVENT_LIMIT_MAX,
        primary_relay_target(),
        [filter.clone()],
    )
    .expect("maximum fetch request");
    assert_eq!(fetch.max_events(), RADROOTS_RELAY_FETCH_EVENT_LIMIT_MAX);
    assert_eq!(
        fetch.max_raw_events(),
        RADROOTS_RELAY_FETCH_RAW_EVENT_LIMIT_MAX
    );
    assert_eq!(
        fetch.max_raw_json_bytes(),
        RADROOTS_RELAY_FETCH_RAW_JSON_BYTE_LIMIT_MAX
    );
    let exact_raw_limits = fetch
        .clone()
        .with_raw_event_scan_limit(RADROOTS_RELAY_FETCH_RAW_EVENT_LIMIT_MAX)
        .expect("maximum raw event limit")
        .with_raw_json_byte_limit(RADROOTS_RELAY_FETCH_RAW_JSON_BYTE_LIMIT_MAX)
        .expect("maximum raw JSON byte limit");
    assert_eq!(
        exact_raw_limits.max_raw_events(),
        RADROOTS_RELAY_FETCH_RAW_EVENT_LIMIT_MAX
    );
    assert_eq!(
        exact_raw_limits.max_raw_json_bytes(),
        RADROOTS_RELAY_FETCH_RAW_JSON_BYTE_LIMIT_MAX
    );
    let subscription = RadrootsRelayFetchRequest::subscription(
        1_006,
        RADROOTS_RELAY_FETCH_EVENT_LIMIT_MAX,
        primary_relay_target(),
        [filter.clone()],
    )
    .expect("maximum subscription request");
    assert_eq!(
        subscription.max_events(),
        RADROOTS_RELAY_FETCH_EVENT_LIMIT_MAX
    );

    let above_max = RADROOTS_RELAY_FETCH_EVENT_LIMIT_MAX + 1;
    assert!(matches!(
        RadrootsRelayFetchRequest::fetch(
            1_006,
            above_max,
            primary_relay_target(),
            [filter.clone()],
        ),
        Err(RadrootsRelayTransportError::FetchLimitTooLarge {
            field: "max_events",
            max,
            actual,
        }) if max == RADROOTS_RELAY_FETCH_EVENT_LIMIT_MAX && actual == above_max
    ));
    assert!(matches!(
        RadrootsRelayFetchRequest::subscription(
            1_006,
            above_max,
            primary_relay_target(),
            [filter],
        ),
        Err(RadrootsRelayTransportError::FetchLimitTooLarge {
            field: "max_events",
            max,
            actual,
        }) if max == RADROOTS_RELAY_FETCH_EVENT_LIMIT_MAX && actual == above_max
    ));
    assert!(matches!(
        fetch
            .clone()
            .with_raw_event_scan_limit(RADROOTS_RELAY_FETCH_RAW_EVENT_LIMIT_MAX + 1),
        Err(RadrootsRelayTransportError::FetchLimitTooLarge {
            field: "max_raw_events",
            max: RADROOTS_RELAY_FETCH_RAW_EVENT_LIMIT_MAX,
            actual,
        }) if actual == RADROOTS_RELAY_FETCH_RAW_EVENT_LIMIT_MAX + 1
    ));
    assert!(matches!(
        fetch.with_raw_json_byte_limit(RADROOTS_RELAY_FETCH_RAW_JSON_BYTE_LIMIT_MAX + 1),
        Err(RadrootsRelayTransportError::FetchLimitTooLarge {
            field: "max_raw_json_bytes",
            max: RADROOTS_RELAY_FETCH_RAW_JSON_BYTE_LIMIT_MAX,
            actual,
        }) if actual == RADROOTS_RELAY_FETCH_RAW_JSON_BYTE_LIMIT_MAX + 1
    ));
}

#[test]
fn fetch_blocking_facade_runs_mock_adapter() {
    let signed = signed_post("blocking fetch");
    let accepted_id = signed.id_str().to_owned();
    let adapter = RadrootsMockRelayFetchAdapter::new(vec![
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
            raw_json: signed.raw_json().to_owned(),
        },
        RadrootsRelayFetchItem::Eose {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
        },
    ]);

    let receipt = fetch_relay_events_blocking(&adapter, post_relay_fetch_request(1_090, 10))
        .expect("blocking fetch");

    assert_eq!(receipt.events.len(), 1);
    assert_eq!(receipt.events[0].event.id.to_hex(), accepted_id);
    assert_eq!(receipt.events[0].observed_at_ms, 1_090);
    assert_eq!(receipt.connected_relays, vec![RELAY_PRIMARY_WSS]);
}

#[tokio::test]
async fn fetch_canonicalizes_adapter_relay_spelling_and_uses_request_observation_time() {
    let signed = signed_post("canonical fetch relay");
    let adapter = RadrootsMockRelayFetchAdapter::new(vec![
        RadrootsRelayFetchItem::Event {
            relay_url: "wss://RELAY.EXAMPLE.COM/".to_owned(),
            raw_json: signed.raw_json().to_owned(),
        },
        RadrootsRelayFetchItem::Eose {
            relay_url: "wss://RELAY.EXAMPLE.COM/".to_owned(),
        },
    ]);

    let receipt = fetch_relay_events(&adapter, post_relay_fetch_request(1_091, 10))
        .await
        .expect("canonical fetch");

    assert_eq!(receipt.events.len(), 1);
    assert_eq!(receipt.events[0].relay_url, RELAY_PRIMARY_WSS);
    assert_eq!(receipt.events[0].observed_at_ms, 1_091);
    assert_eq!(receipt.connected_relays, vec![RELAY_PRIMARY_WSS]);
}

#[tokio::test]
async fn fetch_verifies_events_before_acceptance_budgeting() {
    let accepted = signed_post("verified after tampered");
    let accepted_id = accepted.id_str().to_owned();
    let adapter = RadrootsMockRelayFetchAdapter::new(vec![
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
            raw_json: tampered_raw_event(),
        },
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
            raw_json: accepted.raw_json().to_owned(),
        },
    ]);

    let receipt = fetch_relay_events(&adapter, post_relay_fetch_request(1_091, 1))
        .await
        .expect("verified fetch");

    assert_eq!(receipt.events.len(), 1);
    assert_eq!(receipt.events[0].event.id.to_hex(), accepted_id);
    assert_eq!(receipt.verification_failed_count, 1);
    assert_eq!(receipt.skipped_over_limit_count, 0);
    assert_eq!(
        receipt.event_receipts[0].verification,
        RadrootsRelayFetchEventVerification::Failed
    );
    assert_eq!(
        receipt.event_receipts[1].verification,
        RadrootsRelayFetchEventVerification::Verified
    );
}

#[tokio::test]
async fn fetch_deduplicates_event_ids_without_starving_unique_events() {
    let first = signed_post("first unique");
    let second = signed_post("second unique");
    let first_id = first.id_str().to_owned();
    let second_id = second.id_str().to_owned();
    let adapter = RadrootsMockRelayFetchAdapter::new(vec![
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
            raw_json: first.raw_json().to_owned(),
        },
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_SECONDARY_WSS.to_owned(),
            raw_json: first.raw_json().to_owned(),
        },
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_SECONDARY_WSS.to_owned(),
            raw_json: second.raw_json().to_owned(),
        },
    ]);

    let receipt = fetch_relay_events(&adapter, post_relay_fetch_request(1_091, 2))
        .await
        .expect("deduplicated fetch");

    assert_eq!(
        receipt
            .events
            .iter()
            .map(|event| event.event.id.to_hex())
            .collect::<Vec<_>>(),
        vec![first_id.clone(), second_id]
    );
    assert_eq!(receipt.duplicate_count, 1);
    assert_eq!(receipt.skipped_over_limit_count, 0);
    assert_eq!(receipt.event_receipts.len(), 3);
    assert_eq!(
        receipt.event_receipts[1].event_id.as_deref(),
        Some(first_id.as_str())
    );
    assert!(receipt.event_receipts[1].duplicate);
}

#[tokio::test]
async fn fetch_reports_local_truncation_without_claiming_eose() {
    let adapter = RadrootsMockRelayFetchAdapter::new(vec![RadrootsRelayFetchItem::Truncated {
        relay_url: RELAY_PRIMARY_WSS.to_owned(),
        message: "local budget reached".to_owned(),
    }]);

    let receipt = fetch_relay_events(&adapter, post_relay_fetch_request(1_091, 1))
        .await
        .expect("truncated fetch");

    assert!(receipt.events.is_empty());
    assert!(receipt.connected_relays.is_empty());
    assert_eq!(receipt.eose_count, 0);
    assert_eq!(receipt.truncated_count, 1);
    assert_eq!(receipt.relay_outcomes.len(), 1);
    assert_eq!(
        receipt.relay_outcomes[0].kind,
        RadrootsRelayFetchOutcomeKind::Truncated
    );
}

#[tokio::test]
async fn fetch_rejects_duplicate_and_conflicting_terminal_outcomes() {
    let duplicate = RadrootsMockRelayFetchAdapter::new(vec![
        RadrootsRelayFetchItem::Eose {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
        },
        RadrootsRelayFetchItem::Eose {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
        },
    ]);
    assert!(matches!(
        fetch_relay_events(&duplicate, post_relay_fetch_request(1_091, 1)).await,
        Err(RadrootsRelayTransportError::DuplicateFetchTerminalRelayUrl { url })
            if url == RELAY_PRIMARY_WSS
    ));

    let conflicting = RadrootsMockRelayFetchAdapter::new(vec![
        RadrootsRelayFetchItem::Eose {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
        },
        RadrootsRelayFetchItem::Closed {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
            message: "closed after EOSE".to_owned(),
        },
    ]);
    assert!(matches!(
        fetch_relay_events(&conflicting, post_relay_fetch_request(1_091, 1)).await,
        Err(RadrootsRelayTransportError::ConflictingFetchTerminalRelayUrl {
            url,
            first: "eose",
            next: "closed",
        }) if url == RELAY_PRIMARY_WSS
    ));
}

#[tokio::test]
async fn fetch_rejects_unrequested_adapter_relay_for_every_item_before_store_mutation() {
    let signed = signed_post("forged fetch relay");
    let unexpected_relay = "wss://unexpected.example.com";
    let forged_items = [
        RadrootsRelayFetchItem::Event {
            relay_url: unexpected_relay.to_owned(),
            raw_json: signed.raw_json().to_owned(),
        },
        RadrootsRelayFetchItem::Eose {
            relay_url: unexpected_relay.to_owned(),
        },
        RadrootsRelayFetchItem::Truncated {
            relay_url: unexpected_relay.to_owned(),
            message: "truncated".to_owned(),
        },
        RadrootsRelayFetchItem::Closed {
            relay_url: unexpected_relay.to_owned(),
            message: "closed".to_owned(),
        },
        RadrootsRelayFetchItem::Notice {
            relay_url: unexpected_relay.to_owned(),
            message: "notice".to_owned(),
        },
    ];

    for forged_item in forged_items {
        let store = RadrootsEventStore::open_memory().await.expect("store");
        let adapter = RadrootsMockRelayFetchAdapter::new(vec![
            RadrootsRelayFetchItem::Event {
                relay_url: RELAY_PRIMARY_WSS.to_owned(),
                raw_json: signed.raw_json().to_owned(),
            },
            forged_item,
        ]);
        let error =
            fetch_and_ingest_relay_events(&adapter, &store, post_relay_fetch_request(1_092, 10))
                .await
                .expect_err("unrequested relay must fail the whole fetch");

        assert!(matches!(
            error,
            RadrootsRelayTransportError::UnexpectedFetchItemRelayUrl { ref url }
                if url == unexpected_relay
        ));
        assert!(
            store
                .raw_event(signed.id_str())
                .await
                .expect("raw event")
                .is_none()
        );
    }
}

#[tokio::test]
async fn fetch_ingests_events_and_records_transport_observations() {
    let signed = signed_post("hello");
    let store = RadrootsEventStore::open_memory().await.expect("store");
    let adapter = RadrootsMockRelayFetchAdapter::new(vec![
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
            raw_json: signed.raw_json().to_owned(),
        },
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
            raw_json: signed.raw_json().to_owned(),
        },
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_SECONDARY_WSS.to_owned(),
            raw_json: unsupported_raw_event(),
        },
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_SECONDARY_WSS.to_owned(),
            raw_json: invalid_contract_shape_raw_event(),
        },
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_TERTIARY_WSS.to_owned(),
            raw_json: tampered_raw_event(),
        },
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_TERTIARY_WSS.to_owned(),
            raw_json: "{not json".to_owned(),
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
    assert_eq!(receipt.not_persisted_count, 0);
    assert_eq!(receipt.verification_failed_count, 1);
    assert_eq!(receipt.admission_unsupported_count, 1);
    assert_eq!(receipt.admission_invalid_count, 1);
    assert_eq!(receipt.valid_stream_eligible_count, 2);
    assert_eq!(receipt.visible_count, 2);
    assert_eq!(receipt.not_admitted_count, 2);
    assert_eq!(receipt.not_current_count, 0);
    assert_eq!(receipt.suppressed_count, 0);
    assert_eq!(receipt.malformed_count, 1);
    assert_eq!(receipt.eose_count, 1);
    assert_eq!(receipt.closed_count, 2);
    assert_eq!(receipt.notice_count, 1);
    assert_eq!(
        receipt.inserted_count,
        receipt.events.iter().filter(|event| event.inserted).count()
    );
    assert_eq!(
        receipt.duplicate_count,
        receipt
            .events
            .iter()
            .filter(|event| event.duplicate)
            .count()
    );
    assert_eq!(
        receipt.not_persisted_count,
        receipt
            .events
            .iter()
            .filter(|event| event.not_persisted)
            .count()
    );
    assert_eq!(
        receipt.admission_unsupported_count,
        receipt
            .events
            .iter()
            .filter(|event| event.admission == RadrootsRelayFetchEventAdmission::Unsupported)
            .count()
    );
    assert_eq!(
        receipt.admission_invalid_count,
        receipt
            .events
            .iter()
            .filter(|event| event.admission == RadrootsRelayFetchEventAdmission::Invalid)
            .count()
    );
    assert_eq!(
        receipt.verification_failed_count,
        receipt
            .events
            .iter()
            .filter(|event| event.verification == RadrootsRelayFetchEventVerification::Failed)
            .count()
    );
    assert_eq!(
        receipt.malformed_count,
        receipt
            .events
            .iter()
            .filter(|event| event.malformed)
            .count()
    );
    assert!(receipt.events.iter().all(|event| {
        usize::from(event.inserted)
            + usize::from(event.duplicate)
            + usize::from(event.not_persisted)
            <= 1
            && (!event.malformed
                || event.verification == RadrootsRelayFetchEventVerification::NotEvaluated)
    }));
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
            .kind(),
        RadrootsRelayOutcomeKind::AuthRequired
    );
    assert_eq!(receipt.relay_outcomes[2].relay_url, RELAY_TERTIARY_WSS);
    assert_eq!(
        receipt.relay_outcomes[2]
            .relay_outcome
            .as_ref()
            .expect("restricted outcome")
            .kind(),
        RadrootsRelayOutcomeKind::Restricted
    );
    assert_eq!(
        receipt.relay_outcomes[3].kind,
        RadrootsRelayFetchOutcomeKind::Notice
    );
    assert!(receipt.relay_outcomes[3].relay_outcome.is_none());
    assert_eq!(
        receipt.events[0].admission,
        RadrootsRelayFetchEventAdmission::Admitted
    );
    assert_eq!(
        receipt.events[0].valid_stream,
        RadrootsRelayFetchEventValidStream::Eligible
    );
    assert_eq!(
        receipt.events[0].visibility,
        RadrootsRelayFetchEventVisibility::Visible
    );
    assert_eq!(
        receipt.events[1].admission,
        RadrootsRelayFetchEventAdmission::Admitted
    );
    assert_eq!(
        receipt.events[1].valid_stream,
        RadrootsRelayFetchEventValidStream::Eligible
    );
    assert_eq!(
        receipt.events[1].visibility,
        RadrootsRelayFetchEventVisibility::Visible
    );
    assert_eq!(
        receipt.events[2].admission,
        RadrootsRelayFetchEventAdmission::Unsupported
    );
    assert_eq!(
        receipt.events[2].admission_code.as_deref(),
        Some("unsupported_kind")
    );
    assert_eq!(
        receipt.events[2].valid_stream,
        RadrootsRelayFetchEventValidStream::Ineligible
    );
    assert_eq!(
        receipt.events[2].visibility,
        RadrootsRelayFetchEventVisibility::NotAdmitted
    );
    assert_eq!(
        receipt.events[3].admission,
        RadrootsRelayFetchEventAdmission::Invalid
    );
    assert_eq!(
        receipt.events[3].admission_code.as_deref(),
        Some("reply_event_id_invalid")
    );
    assert_eq!(
        receipt.events[3].valid_stream,
        RadrootsRelayFetchEventValidStream::Ineligible
    );
    assert_eq!(
        receipt.events[3].visibility,
        RadrootsRelayFetchEventVisibility::NotAdmitted
    );
    assert_eq!(
        receipt.events[4].verification,
        RadrootsRelayFetchEventVerification::Failed
    );
    assert_eq!(
        receipt.events[4].admission,
        RadrootsRelayFetchEventAdmission::NotEvaluated
    );
    assert_eq!(
        receipt.events[5].verification,
        RadrootsRelayFetchEventVerification::NotEvaluated
    );
    assert_eq!(
        receipt.events[5].admission,
        RadrootsRelayFetchEventAdmission::NotEvaluated
    );

    let serialized = serde_json::to_value(&receipt).expect("serialized fetch receipt");
    assert!(serialized.get("verification_failed_count").is_some());
    assert!(serialized.get("admission_invalid_count").is_some());
    assert!(serialized.get("visible_count").is_some());
    assert!(serialized.get("not_persisted_count").is_some());
    assert!(serialized.get("truncated_count").is_some());
    let serialized_event = serialized["events"][0]
        .as_object()
        .expect("serialized event receipt");
    assert_eq!(serialized_event.len(), 14);
    for field in [
        "relay_url",
        "event_id",
        "inserted",
        "duplicate",
        "not_persisted",
        "malformed",
        "out_of_filter",
        "skipped_over_limit",
        "verification",
        "admission",
        "admission_code",
        "valid_stream",
        "visibility",
        "message",
    ] {
        assert!(
            serialized_event.contains_key(field),
            "serialized receipt must contain {field}"
        );
    }
    assert!(!serialized_event.contains_key("projection_eligible"));
    assert!(!serialized_event.contains_key("admission_status"));

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
async fn fetch_reports_final_replaceable_visibility_when_newer_arrives_first() {
    let newer = test_event_builder(KIND_PROFILE, r#"{"name":"newer"}"#, Vec::new())
        .custom_created_at(RadrootsNostrTimestamp::from_secs(1_700_000_001))
        .sign_with_keys(&fixture_keys())
        .expect("signed newer profile");
    let older = test_event_builder(KIND_PROFILE, r#"{"name":"older"}"#, Vec::new())
        .custom_created_at(RadrootsNostrTimestamp::from_secs(1_700_000_000))
        .sign_with_keys(&fixture_keys())
        .expect("signed older profile");
    let store = RadrootsEventStore::open_memory().await.expect("store");
    let adapter = RadrootsMockRelayFetchAdapter::new(vec![
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
            raw_json: newer.as_json(),
        },
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
            raw_json: older.as_json(),
        },
    ]);
    let filter = RadrootsNostrFilter::new()
        .kind(RadrootsNostrKind::Custom(
            u16::try_from(KIND_PROFILE).expect("profile kind must fit NIP-01"),
        ))
        .limit(2);
    let request = RadrootsRelayFetchRequest::fetch(1_001, 2, primary_relay_target(), [filter])
        .expect("profile fetch request");

    let receipt = fetch_and_ingest_relay_events(&adapter, &store, request)
        .await
        .expect("fetch ingest");

    assert_eq!(receipt.inserted_count, 2);
    assert_eq!(receipt.valid_stream_eligible_count, 2);
    assert_eq!(receipt.visible_count, 1);
    assert_eq!(receipt.not_current_count, 1);
    assert_eq!(
        receipt.events[0].visibility,
        RadrootsRelayFetchEventVisibility::Visible
    );
    assert_eq!(
        receipt.events[1].admission,
        RadrootsRelayFetchEventAdmission::Admitted
    );
    assert_eq!(
        receipt.events[1].valid_stream,
        RadrootsRelayFetchEventValidStream::Eligible
    );
    assert_eq!(
        receipt.events[1].visibility,
        RadrootsRelayFetchEventVisibility::NotCurrent
    );
}

#[tokio::test]
async fn fetch_reports_final_replaceable_visibility_when_older_arrives_first() {
    let older = test_event_builder(KIND_PROFILE, r#"{"name":"older"}"#, Vec::new())
        .custom_created_at(RadrootsNostrTimestamp::from_secs(1_700_000_000))
        .sign_with_keys(&fixture_keys())
        .expect("signed older profile");
    let newer = test_event_builder(KIND_PROFILE, r#"{"name":"newer"}"#, Vec::new())
        .custom_created_at(RadrootsNostrTimestamp::from_secs(1_700_000_001))
        .sign_with_keys(&fixture_keys())
        .expect("signed newer profile");
    let store = RadrootsEventStore::open_memory().await.expect("store");
    let adapter = RadrootsMockRelayFetchAdapter::new(vec![
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
            raw_json: older.as_json(),
        },
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
            raw_json: newer.as_json(),
        },
    ]);
    let filter = RadrootsNostrFilter::new()
        .kind(RadrootsNostrKind::Custom(
            u16::try_from(KIND_PROFILE).expect("profile kind must fit NIP-01"),
        ))
        .limit(2);
    let request = RadrootsRelayFetchRequest::fetch(1_001, 2, primary_relay_target(), [filter])
        .expect("profile fetch request");

    let receipt = fetch_and_ingest_relay_events(&adapter, &store, request)
        .await
        .expect("fetch ingest");

    assert_eq!(receipt.inserted_count, 2);
    assert_eq!(receipt.valid_stream_eligible_count, 2);
    assert_eq!(receipt.visible_count, 1);
    assert_eq!(receipt.not_current_count, 1);
    assert_eq!(
        receipt.events[0].visibility,
        RadrootsRelayFetchEventVisibility::NotCurrent
    );
    assert_eq!(
        receipt.events[1].visibility,
        RadrootsRelayFetchEventVisibility::Visible
    );
}

#[tokio::test]
async fn fetch_maps_one_final_visibility_snapshot_back_to_duplicate_receipts() {
    let older = test_event_builder(KIND_PROFILE, r#"{"name":"older"}"#, Vec::new())
        .custom_created_at(RadrootsNostrTimestamp::from_secs(1_700_000_000))
        .sign_with_keys(&fixture_keys())
        .expect("signed older profile");
    let newer = test_event_builder(KIND_PROFILE, r#"{"name":"newer"}"#, Vec::new())
        .custom_created_at(RadrootsNostrTimestamp::from_secs(1_700_000_001))
        .sign_with_keys(&fixture_keys())
        .expect("signed newer profile");
    let older_id = older.id.to_hex();
    let newer_id = newer.id.to_hex();
    let store = RadrootsEventStore::open_memory().await.expect("store");
    let adapter = RadrootsMockRelayFetchAdapter::new(vec![
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
            raw_json: older.as_json(),
        },
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
            raw_json: newer.as_json(),
        },
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_SECONDARY_WSS.to_owned(),
            raw_json: older.as_json(),
        },
    ]);
    let filter = RadrootsNostrFilter::new()
        .kind(RadrootsNostrKind::Custom(
            u16::try_from(KIND_PROFILE).expect("profile kind must fit NIP-01"),
        ))
        .limit(2);
    let targets = RadrootsRelayTargetSet::new(
        [RELAY_PRIMARY_WSS, RELAY_SECONDARY_WSS],
        RadrootsRelayUrlPolicy::Public,
    )
    .expect("relay targets");
    let request = RadrootsRelayFetchRequest::fetch(1_001, 2, targets, [filter])
        .expect("profile fetch request");

    let receipt = fetch_and_ingest_relay_events(&adapter, &store, request)
        .await
        .expect("fetch ingest");

    assert_eq!(receipt.inserted_count, 2);
    assert_eq!(receipt.duplicate_count, 1);
    assert_eq!(receipt.valid_stream_eligible_count, 3);
    assert_eq!(receipt.visible_count, 1);
    assert_eq!(receipt.not_current_count, 2);
    assert_eq!(receipt.events.len(), 3);
    assert_eq!(
        receipt
            .events
            .iter()
            .map(|event| event.event_id.as_deref())
            .collect::<Vec<_>>(),
        vec![
            Some(older_id.as_str()),
            Some(newer_id.as_str()),
            Some(older_id.as_str()),
        ]
    );
    assert_eq!(
        receipt
            .events
            .iter()
            .map(|event| event.visibility)
            .collect::<Vec<_>>(),
        vec![
            RadrootsRelayFetchEventVisibility::NotCurrent,
            RadrootsRelayFetchEventVisibility::Visible,
            RadrootsRelayFetchEventVisibility::NotCurrent,
        ]
    );
    assert!(receipt.events[2].duplicate);
}

#[tokio::test]
async fn fetch_reports_store_suppression_when_deletion_precedes_target_replay() {
    let target = test_event_builder(KIND_POST, "deleted target", Vec::new())
        .custom_created_at(RadrootsNostrTimestamp::from_secs(1_700_000_000))
        .sign_with_keys(&fixture_keys())
        .expect("signed target");
    let deletion = test_event_builder(
        KIND_DELETION_REQUEST,
        "",
        vec![vec!["e".to_owned(), target.id.to_hex()]],
    )
    .custom_created_at(RadrootsNostrTimestamp::from_secs(1_700_000_001))
    .sign_with_keys(&fixture_keys())
    .expect("signed deletion");
    let store = RadrootsEventStore::open_memory().await.expect("store");
    let adapter = RadrootsMockRelayFetchAdapter::new(vec![
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
            raw_json: deletion.as_json(),
        },
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
            raw_json: target.as_json(),
        },
    ]);
    let deletion_filter = RadrootsNostrFilter::new().kind(RadrootsNostrKind::Custom(
        u16::try_from(KIND_DELETION_REQUEST).expect("deletion kind must fit NIP-01"),
    ));
    let post_filter = RadrootsNostrFilter::new().kind(RadrootsNostrKind::Custom(
        u16::try_from(KIND_POST).expect("post kind must fit NIP-01"),
    ));
    let request = RadrootsRelayFetchRequest::fetch(
        1_002,
        2,
        primary_relay_target(),
        [deletion_filter, post_filter],
    )
    .expect("deletion replay fetch request");

    let receipt = fetch_and_ingest_relay_events(&adapter, &store, request)
        .await
        .expect("fetch ingest");

    assert_eq!(receipt.inserted_count, 2);
    assert_eq!(receipt.valid_stream_eligible_count, 2);
    assert_eq!(receipt.visible_count, 1);
    assert_eq!(receipt.suppressed_count, 1);
    assert_eq!(receipt.not_current_count, 0);
    assert_eq!(
        receipt.events[0].visibility,
        RadrootsRelayFetchEventVisibility::Visible
    );
    assert_eq!(
        receipt.events[1].event_id.as_deref(),
        Some(target.id.to_hex().as_str())
    );
    assert_eq!(
        receipt.events[1].admission,
        RadrootsRelayFetchEventAdmission::Admitted
    );
    assert_eq!(
        receipt.events[1].valid_stream,
        RadrootsRelayFetchEventValidStream::Eligible
    );
    assert_eq!(
        receipt.events[1].visibility,
        RadrootsRelayFetchEventVisibility::Suppressed
    );
}

#[tokio::test]
async fn fetch_reports_ephemeral_events_as_not_persisted_without_duplicate_or_store_state() {
    let signed = signed_ephemeral("live geochat");
    let event_id = signed.id_str().to_owned();
    let store = RadrootsEventStore::open_memory().await.expect("store");
    let adapter = RadrootsMockRelayFetchAdapter::new(vec![
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
            raw_json: signed.raw_json().to_owned(),
        },
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
            raw_json: signed.raw_json().to_owned(),
        },
    ]);
    let filter = RadrootsNostrFilter::new()
        .kind(RadrootsNostrKind::Custom(
            u16::try_from(KIND_GEOCHAT).expect("ephemeral kind"),
        ))
        .limit(10);
    let request = RadrootsRelayFetchRequest::fetch(1_010, 10, primary_relay_target(), [filter])
        .expect("ephemeral fetch request");

    let receipt = fetch_and_ingest_relay_events(&adapter, &store, request)
        .await
        .expect("ephemeral fetch ingest");

    assert_eq!(receipt.inserted_count, 0);
    assert_eq!(receipt.duplicate_count, 0);
    assert_eq!(receipt.not_persisted_count, 2);
    assert_eq!(receipt.malformed_count, 0);
    assert_eq!(receipt.verification_failed_count, 0);
    assert_eq!(receipt.admission_unsupported_count, 0);
    assert_eq!(receipt.admission_invalid_count, 0);
    assert_eq!(receipt.valid_stream_eligible_count, 0);
    assert_eq!(receipt.events.len(), 2);
    assert!(receipt.events.iter().all(|event| {
        !event.inserted
            && !event.duplicate
            && event.not_persisted
            && event.verification == RadrootsRelayFetchEventVerification::Verified
            && event.admission == RadrootsRelayFetchEventAdmission::Admitted
            && event.admission_code.is_none()
            && event.valid_stream == RadrootsRelayFetchEventValidStream::Ineligible
            && event.visibility == RadrootsRelayFetchEventVisibility::NotPersisted
    }));
    assert!(
        store
            .raw_event(event_id.as_str())
            .await
            .expect("raw event")
            .is_none()
    );
    assert!(
        store
            .observations_for_event(event_id.as_str())
            .await
            .expect("observations")
            .is_empty()
    );
    let summary = store.status_summary().await.expect("status summary");
    assert_eq!(summary.total_events, 0);
    assert_eq!(summary.valid_stream_events, 0);
    assert_eq!(summary.transport_observations, 0);
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
        },
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
            raw_json: accepted.raw_json().to_owned(),
        },
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_SECONDARY_WSS.to_owned(),
            raw_json: wrong_kind.as_json(),
        },
        RadrootsRelayFetchItem::Eose {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
        },
    ]);
    let filter = radroots_nostr_filter_tag(
        RadrootsNostrFilter::new()
            .kind(RadrootsNostrKind::Custom(
                u16::try_from(KIND_POST).expect("post kind must fit NIP-01"),
            ))
            .limit(10),
        "t",
        vec!["soil".to_owned()],
    )
    .expect("filter");

    let receipt = fetch_and_ingest_relay_events(
        &adapter,
        &store,
        RadrootsRelayFetchRequest::fetch(1_005, 10, fixture_relay_targets(), [filter])
            .expect("fetch request"),
    )
    .await
    .expect("fetch ingest");

    assert_eq!(receipt.inserted_count, 1);
    assert_eq!(receipt.out_of_filter_count, 2);
    assert_eq!(receipt.malformed_count, 0);
    assert_eq!(receipt.admission_unsupported_count, 0);
    assert_eq!(receipt.events.len(), 3);
    assert!(receipt.events[0].out_of_filter);
    assert!(!receipt.events[1].out_of_filter);
    assert!(receipt.events[2].out_of_filter);
    assert!(
        store
            .raw_event(accepted.id_str())
            .await
            .expect("accepted lookup")
            .is_some()
    );
    assert!(
        store
            .raw_event(wrong_tag.id_str())
            .await
            .expect("wrong tag lookup")
            .is_none()
    );
    assert!(
        store
            .raw_event(wrong_kind_event_id.as_str())
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
        },
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
            raw_json: wrong_tag.raw_json().to_owned(),
        },
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
            raw_json: accepted.raw_json().to_owned(),
        },
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
            raw_json: skipped.raw_json().to_owned(),
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
    assert_eq!(receipt.admission_unsupported_count, 0);
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
            .kind(),
        RadrootsRelayOutcomeKind::AuthRequired
    );
    assert_eq!(
        receipt.relay_outcomes[2].kind,
        RadrootsRelayFetchOutcomeKind::Notice
    );
    assert!(
        store
            .raw_event(accepted_id.as_str())
            .await
            .expect("accepted lookup")
            .is_some()
    );
    assert!(
        store
            .raw_event(skipped_id.as_str())
            .await
            .expect("skipped lookup")
            .is_none()
    );
    assert!(
        store
            .raw_event(wrong_tag_id.as_str())
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
            .kind(RadrootsNostrKind::Custom(
                u16::try_from(KIND_POST).expect("post kind must fit NIP-01"),
            ))
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
        },
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
            raw_json: wrong_tag.raw_json().to_owned(),
        },
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
            raw_json: accepted.raw_json().to_owned(),
        },
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
            raw_json: skipped.raw_json().to_owned(),
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
        RadrootsRelayFetchRequest::fetch(2_100, 1, fixture_relay_targets(), [filter])
            .expect("fetch request"),
    )
    .await
    .expect("fetch events");

    assert_eq!(
        receipt.target_relays,
        vec![RELAY_PRIMARY_WSS, RELAY_SECONDARY_WSS, RELAY_TERTIARY_WSS]
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
        },
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
            raw_json: wrong_tag.raw_json().to_owned(),
        },
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
            raw_json: accepted.raw_json().to_owned(),
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
            .raw_event(accepted_id.as_str())
            .await
            .expect("accepted lookup")
            .is_none()
    );
}

#[tokio::test]
async fn fetch_raw_json_byte_limit_is_exact_global_and_sticky() {
    let first = signed_post("raw byte first");
    let second = signed_post("raw byte second");
    let third = signed_post("raw byte third");
    let exact_bytes = first.raw_json().len() + second.raw_json().len();
    let targets = RadrootsRelayTargetSet::new(
        [RELAY_PRIMARY_WSS, RELAY_SECONDARY_WSS],
        RadrootsRelayUrlPolicy::Public,
    )
    .expect("relay targets");
    let items = vec![
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
            raw_json: first.raw_json().to_owned(),
        },
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_SECONDARY_WSS.to_owned(),
            raw_json: second.raw_json().to_owned(),
        },
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
            raw_json: third.raw_json().to_owned(),
        },
        RadrootsRelayFetchItem::Eose {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
        },
        RadrootsRelayFetchItem::Eose {
            relay_url: RELAY_SECONDARY_WSS.to_owned(),
        },
    ];

    let exact_receipt = fetch_relay_events(
        &RadrootsMockRelayFetchAdapter::new(items.clone()),
        RadrootsRelayFetchRequest::fetch(1_131, 3, targets.clone(), [post_relay_fetch_filter(3)])
            .expect("fetch request")
            .with_raw_json_byte_limit(exact_bytes)
            .expect("exact raw JSON byte limit"),
    )
    .await
    .expect("exact fetch");
    assert_eq!(exact_receipt.events.len(), 2);
    assert_eq!(exact_receipt.skipped_over_limit_count, 1);
    assert_eq!(exact_receipt.eose_count, 2);

    let crossing_receipt = fetch_relay_events(
        &RadrootsMockRelayFetchAdapter::new(items),
        RadrootsRelayFetchRequest::fetch(1_132, 3, targets, [post_relay_fetch_filter(3)])
            .expect("fetch request")
            .with_raw_json_byte_limit(exact_bytes - 1)
            .expect("crossing raw JSON byte limit"),
    )
    .await
    .expect("crossing fetch");
    assert_eq!(crossing_receipt.events.len(), 1);
    assert_eq!(crossing_receipt.skipped_over_limit_count, 2);
    assert_eq!(crossing_receipt.eose_count, 2);
}

#[tokio::test]
async fn fetch_raw_json_budget_charges_every_preparse_event_class() {
    let malformed = "{not json".to_owned();
    let wrong_tag = signed_event_with_kind_and_hashtag("raw bytes wrong tag", KIND_POST, "compost");
    let accepted = signed_post("raw bytes accepted");
    let over_accepted_limit = signed_post("raw bytes over accepted limit");
    let sticky_skip = signed_post("raw bytes sticky skip");
    let exact_bytes = malformed.len()
        + wrong_tag.raw_json().len()
        + accepted.raw_json().len() * 2
        + over_accepted_limit.raw_json().len();
    let adapter = RadrootsMockRelayFetchAdapter::new(vec![
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
            raw_json: malformed,
        },
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
            raw_json: wrong_tag.raw_json().to_owned(),
        },
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
            raw_json: accepted.raw_json().to_owned(),
        },
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_SECONDARY_WSS.to_owned(),
            raw_json: accepted.raw_json().to_owned(),
        },
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_SECONDARY_WSS.to_owned(),
            raw_json: over_accepted_limit.raw_json().to_owned(),
        },
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_SECONDARY_WSS.to_owned(),
            raw_json: sticky_skip.raw_json().to_owned(),
        },
        RadrootsRelayFetchItem::Eose {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
        },
        RadrootsRelayFetchItem::Eose {
            relay_url: RELAY_SECONDARY_WSS.to_owned(),
        },
    ]);
    let targets = RadrootsRelayTargetSet::new(
        [RELAY_PRIMARY_WSS, RELAY_SECONDARY_WSS],
        RadrootsRelayUrlPolicy::Public,
    )
    .expect("relay targets");
    let store = RadrootsEventStore::open_memory().await.expect("store");

    let receipt = fetch_and_ingest_relay_events(
        &adapter,
        &store,
        RadrootsRelayFetchRequest::fetch(1_133, 1, targets, [post_relay_fetch_filter(6)])
            .expect("fetch request")
            .with_raw_json_byte_limit(exact_bytes)
            .expect("raw JSON byte limit"),
    )
    .await
    .expect("fetch ingest");

    assert_eq!(receipt.inserted_count, 1);
    assert_eq!(receipt.malformed_count, 1);
    assert_eq!(receipt.out_of_filter_count, 1);
    assert_eq!(receipt.duplicate_count, 1);
    assert_eq!(receipt.skipped_over_limit_count, 2);
    assert_eq!(receipt.events.len(), 5);
    assert!(receipt.events[4].skipped_over_limit);
    assert_eq!(receipt.eose_count, 2);
}

#[tokio::test]
async fn fetch_rejects_oversized_raw_json_before_radroots_adapter_parsing() {
    let accepted = signed_post("after oversized raw event");
    let adapter = RadrootsMockRelayFetchAdapter::new(vec![
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
            raw_json: "x".repeat(DEFAULT_RAW_JSON_MAX_BYTES + 1),
        },
        RadrootsRelayFetchItem::Event {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
            raw_json: accepted.raw_json().to_owned(),
        },
        RadrootsRelayFetchItem::Eose {
            relay_url: RELAY_PRIMARY_WSS.to_owned(),
        },
    ]);

    let receipt = fetch_relay_events(&adapter, post_relay_fetch_request(1_134, 1))
        .await
        .expect("fetch events");

    assert_eq!(receipt.events.len(), 1);
    assert_eq!(receipt.verification_failed_count, 1);
    assert_eq!(receipt.malformed_count, 0);
    assert_eq!(receipt.event_receipts.len(), 2);
    assert_eq!(receipt.event_receipts[0].event_id, None);
    assert_eq!(
        receipt.event_receipts[0].verification,
        RadrootsRelayFetchEventVerification::Failed
    );
    assert_eq!(receipt.eose_count, 1);
}

#[tokio::test]
async fn fetch_subscription_mode_and_store_errors_are_propagated() {
    let signed = signed_post("subscription");
    let store = RadrootsEventStore::open_memory().await.expect("store");
    let adapter = RadrootsMockRelayFetchAdapter::new(vec![RadrootsRelayFetchItem::Event {
        relay_url: RELAY_PRIMARY_WSS.to_owned(),
        raw_json: signed.raw_json().to_owned(),
    }]);

    let receipt = fetch_and_ingest_relay_events(
        &adapter,
        &store,
        RadrootsRelayFetchRequest::subscription(
            1_200,
            10,
            primary_relay_target(),
            [post_relay_fetch_filter(10)],
        )
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
    }]);
    let error =
        fetch_and_ingest_relay_events(&adapter, &closed_store, post_relay_fetch_request(1_210, 10))
            .await
            .expect_err("closed local store must fail the fetch ingest");

    assert!(matches!(error, RadrootsRelayTransportError::EventStore(_)));
}

#[tokio::test]
async fn fetch_ingest_rejects_invalid_observation_endpoint() {
    let signed = signed_post("invalid observation endpoint");
    let store = RadrootsEventStore::open_memory().await.expect("store");
    let adapter = RadrootsMockRelayFetchAdapter::new(vec![RadrootsRelayFetchItem::Event {
        relay_url: " ".to_owned(),
        raw_json: signed.raw_json().to_owned(),
    }]);

    let error =
        fetch_and_ingest_relay_events(&adapter, &store, post_relay_fetch_request(1_300, 10))
            .await
            .expect_err("invalid observation endpoint");

    assert!(matches!(
        error,
        RadrootsRelayTransportError::InvalidFetchItemRelayUrl { .. }
    ));
}

#[tokio::test]
async fn outbox_publish_persists_partial_success_and_skips_accepted_retry() {
    let outbox = RadrootsOutbox::open_memory().await.expect("outbox");
    let store = RadrootsEventStore::open_memory().await.expect("store");
    let draft = generic_draft("hello");
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
            bounded_relay_outcome(RadrootsRelayOutcome::timeout("timeout: no OK")),
        )
        .with_outcome(
            RELAY_TERTIARY_WSS,
            bounded_relay_outcome(RadrootsRelayOutcome::duplicate_accepted(
                "duplicate: already have it",
            )),
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
async fn outbox_transport_facade_persists_partial_success_and_retryable_failures() {
    let outbox = RadrootsOutbox::open_memory().await.expect("outbox");
    let store = RadrootsEventStore::open_memory().await.expect("store");
    let draft = generic_draft("transport facade outbox");
    let receipt = outbox
        .enqueue_operation(all_accepted_outbox_operation_input(
            draft,
            [RELAY_PRIMARY_WSS, RELAY_SECONDARY_WSS],
        ))
        .await
        .expect("enqueue");
    let claimed = outbox
        .claim_next_ready_event("signer", "transport-sign", 2_000, 1_000)
        .await
        .expect("sign claim")
        .expect("sign claim");
    let signed = complete_claimed_signing(&outbox, &claimed, 1_100).await;
    let publish_claim = outbox
        .claim_next_ready_event("publisher", "transport-publish", 3_000, 1_100)
        .await
        .expect("publish claim")
        .expect("publish claim");

    let adapter = RadrootsMockRelayPublishAdapter::new()
        .with_outcome(RELAY_PRIMARY_WSS, RadrootsRelayOutcome::accepted())
        .with_outcome(
            RELAY_SECONDARY_WSS,
            bounded_relay_outcome(RadrootsRelayOutcome::timeout("timeout: transport facade")),
        );
    let transport = RadrootsNostrTransport::new(adapter);
    let published = publish_claimed_outbox_event_with_transport(
        &outbox,
        &store,
        &transport,
        &publish_claim,
        RadrootsOutboxPublishPolicy::new(2_500),
        2_200,
    )
    .await
    .expect("transport publish");

    assert_eq!(published.event_id, signed.id_str());
    assert_eq!(published.attempted_count, 2);
    assert_eq!(published.accepted_count, 1);
    assert_eq!(published.retryable_count, 1);
    assert_eq!(published.terminal_count, 0);
    assert!(!published.quorum_met);
    assert_eq!(published.relay_receipts.len(), 2);
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
    let event = outbox
        .get_event(receipt.outbox_event_id)
        .await
        .expect("event")
        .expect("event");
    assert_eq!(event.state, RadrootsOutboxEventState::PublishRetryable);
    let observations = store
        .observations_for_event(signed.id_str())
        .await
        .expect("observations");
    assert_outbox_publish_observations(&observations, 1);
    assert_eq!(
        observations
            .iter()
            .find(|observation| {
                observation.observation_type == RadrootsTransportObservationType::PublishAck
            })
            .expect("publish acknowledgement")
            .observation_count,
        1
    );
}

#[tokio::test]
async fn outbox_transport_facade_persists_every_delivery_status() {
    let outbox = RadrootsOutbox::open_memory().await.expect("outbox");
    let store = RadrootsEventStore::open_memory().await.expect("store");
    let draft = generic_draft("transport outcome matrix");
    let relays = (0..14)
        .map(|index| format!("wss://relay-{index}.example.com"))
        .collect::<Vec<_>>();
    let receipt = outbox
        .enqueue_operation(all_accepted_outbox_operation_input(draft, &relays))
        .await
        .expect("enqueue");
    let claimed = outbox
        .claim_next_ready_event("signer", "matrix-sign", 2_000, 1_000)
        .await
        .expect("sign claim")
        .expect("sign claim");
    let signed = complete_claimed_signing(&outbox, &claimed, 1_100).await;
    let publish_claim = outbox
        .claim_next_ready_event("publisher", "matrix-publish", 3_000, 1_100)
        .await
        .expect("publish claim")
        .expect("publish claim");
    let outcomes = [
        RadrootsTransportOutcomeKind::Accepted,
        RadrootsTransportOutcomeKind::DuplicateAccepted,
        RadrootsTransportOutcomeKind::Delivered,
        RadrootsTransportOutcomeKind::Forwarded,
        RadrootsTransportOutcomeKind::StoredByGateway,
        RadrootsTransportOutcomeKind::Seen,
        RadrootsTransportOutcomeKind::DeferredUntilImplemented,
        RadrootsTransportOutcomeKind::Rejected,
        RadrootsTransportOutcomeKind::RouteUnavailable,
        RadrootsTransportOutcomeKind::PayloadTooLarge,
        RadrootsTransportOutcomeKind::PolicyDenied,
        RadrootsTransportOutcomeKind::Timeout,
        RadrootsTransportOutcomeKind::ConnectionFailed,
        RadrootsTransportOutcomeKind::TransportUnavailable,
    ]
    .into_iter()
    .map(RadrootsTransportOutcome::new)
    .collect();
    let transport = ScriptedTransport::new(outcomes);

    let published = publish_claimed_outbox_event_with_transport(
        &outbox,
        &store,
        &transport,
        &publish_claim,
        RadrootsOutboxPublishPolicy::new(2_500),
        2_200,
    )
    .await
    .expect("transport publish");

    assert_eq!(published.event_id, signed.id_str());
    assert_eq!(published.attempted_count, 14);
    assert_eq!(published.accepted_count, 6);
    assert_eq!(published.retryable_count, 3);
    assert_eq!(published.terminal_count, 5);
    assert!(!published.quorum_met);
    assert_eq!(published.target_receipts.len(), 14);
    assert_eq!(published.relay_receipts.len(), 14);
    let targets = outbox
        .delivery_targets(receipt.outbox_event_id)
        .await
        .expect("targets");
    let expected_statuses = [
        RadrootsOutboxDeliveryTargetStatus::Accepted,
        RadrootsOutboxDeliveryTargetStatus::Accepted,
        RadrootsOutboxDeliveryTargetStatus::Delivered,
        RadrootsOutboxDeliveryTargetStatus::Forwarded,
        RadrootsOutboxDeliveryTargetStatus::StoredByGateway,
        RadrootsOutboxDeliveryTargetStatus::Seen,
        RadrootsOutboxDeliveryTargetStatus::DeferredUntilImplemented,
        RadrootsOutboxDeliveryTargetStatus::FailedTerminal,
        RadrootsOutboxDeliveryTargetStatus::FailedTerminal,
        RadrootsOutboxDeliveryTargetStatus::FailedTerminal,
        RadrootsOutboxDeliveryTargetStatus::SkippedPolicyDenied,
        RadrootsOutboxDeliveryTargetStatus::FailedRetryable,
        RadrootsOutboxDeliveryTargetStatus::FailedRetryable,
        RadrootsOutboxDeliveryTargetStatus::FailedRetryable,
    ];
    assert_eq!(targets.len(), expected_statuses.len());
    for (target, expected_status) in targets.iter().zip(expected_statuses) {
        assert_eq!(target.status, expected_status);
    }
    let event = outbox
        .get_event(receipt.outbox_event_id)
        .await
        .expect("event")
        .expect("event");
    assert_eq!(event.state, RadrootsOutboxEventState::PublishRetryable);
    let observations = store
        .observations_for_event(signed.id_str())
        .await
        .expect("observations");
    assert_outbox_publish_observations(&observations, 6);
}

#[test]
fn transport_outcome_wire_rejects_pending_accepted_status() {
    let error = serde_json::from_value::<RadrootsTransportOutcome>(serde_json::json!({
        "kind": "Accepted",
        "status": "Pending",
        "code": null,
        "message": null,
    }))
    .expect_err("pending accepted outcome rejected before transport execution");
    assert!(error.to_string().contains("status"));
}

#[tokio::test]
async fn outbox_transport_facade_rejects_receipts_forged_for_another_request() {
    for forged in [
        ForgedDeliveryReceipt::RequestId,
        ForgedDeliveryReceipt::TargetSet,
    ] {
        let outbox = RadrootsOutbox::open_memory().await.expect("outbox");
        let store = RadrootsEventStore::open_memory().await.expect("store");
        let receipt = outbox
            .enqueue_operation(all_accepted_outbox_operation_input(
                generic_draft("forged transport receipt"),
                [RELAY_PRIMARY_WSS],
            ))
            .await
            .expect("enqueue");
        let claimed = outbox
            .claim_next_ready_event("signer", "forged-sign", 2_000, 1_000)
            .await
            .expect("sign claim")
            .expect("sign claim");
        let signed = complete_claimed_signing(&outbox, &claimed, 1_100).await;
        let publish_claim = outbox
            .claim_next_ready_event("publisher", "forged-publish", 3_000, 1_100)
            .await
            .expect("publish claim")
            .expect("publish claim");

        let error = publish_claimed_outbox_event_with_transport(
            &outbox,
            &store,
            &ForgedReceiptTransport { forged },
            &publish_claim,
            RadrootsOutboxPublishPolicy::new(2_500),
            2_200,
        )
        .await
        .expect_err("forged receipt rejected");
        assert!(matches!(
            error,
            RadrootsRelayTransportError::TransportContract(_)
        ));
        let event = outbox
            .get_event(receipt.outbox_event_id)
            .await
            .expect("event")
            .expect("event");
        assert_eq!(event.state, RadrootsOutboxEventState::Publishing);
        let observations = store
            .observations_for_event(signed.id_str())
            .await
            .expect("observations");
        assert_outbox_publish_observations(&observations, 0);
    }
}

#[tokio::test]
async fn outbox_transport_facade_handles_empty_and_invalid_claim_plans() {
    let outbox = RadrootsOutbox::open_memory().await.expect("outbox");
    let store = RadrootsEventStore::open_memory().await.expect("store");
    let draft = generic_draft("transport plan edges");
    let receipt = outbox
        .enqueue_operation(all_accepted_outbox_operation_input(
            draft,
            [RELAY_PRIMARY_WSS, RELAY_SECONDARY_WSS],
        ))
        .await
        .expect("enqueue");
    let claimed = outbox
        .claim_next_ready_event("signer", "plan-sign", 2_000, 1_000)
        .await
        .expect("sign claim")
        .expect("sign claim");
    let signed = complete_claimed_signing(&outbox, &claimed, 1_100).await;
    let publish_claim = outbox
        .claim_next_ready_event("publisher", "plan-publish", 3_000, 1_100)
        .await
        .expect("publish claim")
        .expect("publish claim");
    for target in &publish_claim.delivery_targets {
        outbox
            .mark_delivery_target_accepted(
                publish_claim.outbox_event_id,
                publish_claim.claim_token.as_str(),
                target.delivery_target_id,
                2_150,
            )
            .await
            .expect("accepted target");
    }
    let published = publish_claimed_outbox_event_with_transport(
        &outbox,
        &store,
        &ScriptedTransport::new(Vec::new()),
        &publish_claim,
        RadrootsOutboxPublishPolicy::new(2_500),
        2_200,
    )
    .await
    .expect("already satisfied publish");
    assert_eq!(published.event_id, signed.id_str());
    assert_eq!(published.attempted_count, 0);
    assert_eq!(published.accepted_count, 2);
    assert!(published.quorum_met);

    let second_draft = generic_draft("invalid claimed plan");
    outbox
        .enqueue_operation(all_accepted_outbox_operation_input(
            second_draft,
            [RELAY_PRIMARY_WSS],
        ))
        .await
        .expect("second enqueue");
    let second_claimed = outbox
        .claim_next_ready_event("signer", "invalid-plan-sign", 3_000, 2_200)
        .await
        .expect("second sign claim")
        .expect("second sign claim");
    complete_claimed_signing(&outbox, &second_claimed, 2_300).await;
    let second_publish_claim = outbox
        .claim_next_ready_event("publisher", "invalid-plan-publish", 4_000, 2_300)
        .await
        .expect("second publish claim")
        .expect("second publish claim");
    let mut invalid_claim = second_publish_claim.clone();
    invalid_claim.active_delivery_plan_id = None;
    let error = publish_claimed_outbox_event_with_transport(
        &outbox,
        &store,
        &ScriptedTransport::new(Vec::new()),
        &invalid_claim,
        RadrootsOutboxPublishPolicy::new(3_500),
        2_400,
    )
    .await
    .expect_err("missing plan rejected");
    assert!(matches!(error, RadrootsRelayTransportError::Transport(_)));

    invalid_claim.active_delivery_plan_id = Some(i64::MAX);
    let error = publish_claimed_outbox_event_with_transport(
        &outbox,
        &store,
        &ScriptedTransport::new(Vec::new()),
        &invalid_claim,
        RadrootsOutboxPublishPolicy::new(3_500),
        2_401,
    )
    .await
    .expect_err("unknown plan rejected");
    assert!(matches!(error, RadrootsRelayTransportError::Transport(_)));

    let event = outbox
        .get_event(receipt.outbox_event_id)
        .await
        .expect("event")
        .expect("event");
    assert_eq!(event.state, RadrootsOutboxEventState::Published);
}

#[tokio::test]
async fn outbox_transport_facade_requires_signed_claims() {
    let outbox = RadrootsOutbox::open_memory().await.expect("outbox");
    let store = RadrootsEventStore::open_memory().await.expect("store");
    let draft = generic_draft("missing transport signature");
    let receipt = outbox
        .enqueue_operation(all_accepted_outbox_operation_input(
            draft,
            [RELAY_PRIMARY_WSS],
        ))
        .await
        .expect("enqueue");
    let claimed = outbox
        .claim_next_ready_event("signer", "unsigned-transport", 2_000, 1_000)
        .await
        .expect("claim")
        .expect("claim");

    let error = publish_claimed_outbox_event_with_transport(
        &outbox,
        &store,
        &ScriptedTransport::new(Vec::new()),
        &claimed,
        RadrootsOutboxPublishPolicy::new(2_500),
        1_100,
    )
    .await
    .expect_err("missing signature rejected");
    assert!(matches!(
        error,
        RadrootsRelayTransportError::MissingSignedOutboxEvent(event_id)
            if event_id == receipt.outbox_event_id
    ));
}

#[tokio::test]
async fn outbox_transport_facade_rejects_non_nostr_transport_before_mutation() {
    let outbox = RadrootsOutbox::open_memory().await.expect("outbox");
    let store = RadrootsEventStore::open_memory().await.expect("store");
    let receipt = outbox
        .enqueue_operation(all_accepted_outbox_operation_input(
            generic_draft("misrouted transport"),
            [RELAY_PRIMARY_WSS],
        ))
        .await
        .expect("enqueue");
    let claimed = outbox
        .claim_next_ready_event("signer", "misrouted-sign", 2_000, 1_000)
        .await
        .expect("sign claim")
        .expect("sign claim");
    let signed = complete_claimed_signing(&outbox, &claimed, 1_100).await;
    let publish_claim = outbox
        .claim_next_ready_event("publisher", "misrouted-publish", 3_000, 1_100)
        .await
        .expect("publish claim")
        .expect("publish claim");
    let transport = ScriptedTransport::new(Vec::new()).with_kind(RadrootsTransportKind::Reticulum);

    let error = publish_claimed_outbox_event_with_transport(
        &outbox,
        &store,
        &transport,
        &publish_claim,
        RadrootsOutboxPublishPolicy::new(2_500),
        2_200,
    )
    .await
    .expect_err("non-Nostr transport rejected");
    assert!(matches!(
        error,
        RadrootsRelayTransportError::UnexpectedTransportKind {
            expected: "nostr",
            actual,
        } if actual == "reticulum"
    ));
    assert!(
        store
            .raw_event(signed.id_str())
            .await
            .expect("raw event")
            .is_none()
    );
    assert!(
        store
            .observations_for_event(signed.id_str())
            .await
            .expect("observations")
            .is_empty()
    );
    let event = outbox
        .get_event(receipt.outbox_event_id)
        .await
        .expect("event")
        .expect("event");
    assert_eq!(event.state, RadrootsOutboxEventState::Publishing);
    let targets = outbox
        .delivery_targets(receipt.outbox_event_id)
        .await
        .expect("targets");
    assert!(targets.iter().all(|target| target.attempt_count == 0));
}

#[tokio::test]
async fn outbox_publish_fans_out_endpoint_receipts_to_scoped_logical_targets() {
    let outbox = RadrootsOutbox::open_memory().await.expect("outbox");
    let store = RadrootsEventStore::open_memory().await.expect("store");
    let draft = generic_draft("scoped duplicate relay");
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
async fn outbox_transport_facade_fans_out_endpoint_receipts_to_scoped_logical_targets() {
    let outbox = RadrootsOutbox::open_memory().await.expect("outbox");
    let store = RadrootsEventStore::open_memory().await.expect("store");
    let receipt = outbox
        .enqueue_operation(RadrootsOutboxOperationInput::new(
            "publish_post",
            generic_draft("transport scoped duplicate relay"),
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
    let transport = RadrootsNostrTransport::new(adapter.clone());

    let published = publish_claimed_outbox_event_with_transport(
        &outbox,
        &store,
        &transport,
        &publish_claim,
        RadrootsOutboxPublishPolicy::new(2_500),
        2_200,
    )
    .await
    .expect("transport publish");

    assert_eq!(published.local_ingest.event_id, signed.id_str());
    assert_eq!(published.event_id, signed.id_str());
    assert_eq!(published.attempted_count, 2);
    assert_eq!(published.accepted_count, 2);
    assert_eq!(published.retryable_count, 0);
    assert_eq!(published.terminal_count, 0);
    assert_eq!(published.quorum, 2);
    assert!(published.quorum_met);
    assert_eq!(published.relay_receipts.len(), 1);
    assert_eq!(published.target_receipts.len(), 2);
    assert!(published.target_receipts.iter().all(|target| {
        target.endpoint_uri == RELAY_PRIMARY_WSS
            && target.attempted
            && target.transport_status == RadrootsTransportDeliveryTargetStatus::Accepted
    }));
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
    let observations = store
        .observations_for_event(signed.id_str())
        .await
        .expect("observations");
    assert_outbox_publish_observations(&observations, 1);
    assert_eq!(
        observations
            .iter()
            .find(|observation| {
                observation.observation_type == RadrootsTransportObservationType::PublishAck
            })
            .expect("scoped publish acknowledgement")
            .observation_count,
        1
    );
}

#[tokio::test]
async fn outbox_publish_required_target_failure_is_not_satisfied_by_optional_success() {
    let outbox = RadrootsOutbox::open_memory().await.expect("outbox");
    let store = RadrootsEventStore::open_memory().await.expect("store");
    let draft = generic_draft("required target optional success");
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
                    vec![required.fingerprint().clone()],
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
        .find(|target| &target.endpoint_fingerprint == optional.fingerprint())
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
        bounded_relay_outcome(RadrootsRelayOutcome::timeout("required relay timeout")),
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
        &target.endpoint_fingerprint == optional.fingerprint()
            && target.status == RadrootsOutboxDeliveryTargetStatus::Accepted
    }));
    assert!(targets.iter().any(|target| {
        &target.endpoint_fingerprint == required.fingerprint()
            && target.status == RadrootsOutboxDeliveryTargetStatus::FailedRetryable
    }));
}

#[tokio::test]
async fn outbox_publish_required_target_success_is_not_blocked_by_optional_retryable_failure() {
    let outbox = RadrootsOutbox::open_memory().await.expect("outbox");
    let store = RadrootsEventStore::open_memory().await.expect("store");
    let draft = generic_draft("required target optional failure");
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
                    vec![required.fingerprint().clone()],
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
        .find(|target| &target.endpoint_fingerprint == optional.fingerprint())
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
        &target.endpoint_fingerprint == optional.fingerprint()
            && target.status == RadrootsOutboxDeliveryTargetStatus::FailedRetryable
    }));
    assert!(targets.iter().any(|target| {
        &target.endpoint_fingerprint == required.fingerprint()
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
    let outbox = RadrootsOutbox::open_memory().await.expect("outbox");
    let store = RadrootsEventStore::open_memory().await.expect("store");
    let draft = generic_draft("required target scoped duplicate relay");
    let required = scoped_nostr_target(RELAY_PRIMARY_WSS, "foodshed.west", "West foodshed");
    let optional = scoped_nostr_target(RELAY_PRIMARY_WSS, "foodshed.east", "East foodshed");
    let terminal = scoped_nostr_target(RELAY_PRIMARY_WSS, "foodshed.closed", "Closed foodshed");
    let receipt = outbox
        .enqueue_operation(RadrootsOutboxOperationInput::new(
            "publish_post",
            draft,
            RadrootsOutboxDeliveryPlanInput::new(
                "transport.nostr.local",
                1,
                RadrootsTransportSatisfactionPolicy::required_targets(
                    RadrootsTransportSatisfactionClass::Accepted,
                    vec![required.fingerprint().clone()],
                )
                .expect("required target policy"),
                vec![
                    required.clone(),
                    optional.clone(),
                    terminal.clone(),
                    RadrootsTransportTarget::reticulum().expect("reticulum target"),
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
    let optional_record = publish_claim
        .delivery_targets
        .iter()
        .find(|target| &target.endpoint_fingerprint == optional.fingerprint())
        .expect("optional target");
    outbox
        .mark_delivery_target_accepted(
            publish_claim.outbox_event_id,
            publish_claim.claim_token.as_str(),
            optional_record.delivery_target_id,
            2_150,
        )
        .await
        .expect("optional target accepted");
    let terminal_record = publish_claim
        .delivery_targets
        .iter()
        .find(|target| &target.endpoint_fingerprint == terminal.fingerprint())
        .expect("terminal target");
    outbox
        .mark_delivery_target_failed_terminal(
            publish_claim.outbox_event_id,
            publish_claim.claim_token.as_str(),
            terminal_record.delivery_target_id,
            "terminal target",
            2_151,
        )
        .await
        .expect("terminal target completed");
    let adapter = RadrootsMockRelayPublishAdapter::new()
        .with_outcome(RELAY_PRIMARY_WSS, RadrootsRelayOutcome::accepted());

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

    assert_eq!(published.attempted_count, 2);
    assert_eq!(published.accepted_count, 2);
    assert_eq!(published.quorum, 1);
    assert!(published.quorum_met);
    assert_eq!(published.relay_receipts.len(), 1);
    assert_eq!(published.target_receipts.len(), 2);
    assert!(published.target_receipts.iter().any(|target| {
        &target.endpoint_fingerprint == required.fingerprint()
            && target.target_scope.as_deref() == Some("foodshed.west")
    }));
    assert!(published.target_receipts.iter().any(|target| {
        &target.endpoint_fingerprint == optional.fingerprint()
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
    assert_eq!(targets.len(), 4);
    assert!(
        targets
            .iter()
            .filter(|target| { target.transport_kind == RadrootsTransportKind::Nostr })
            .filter(|target| &target.endpoint_fingerprint != terminal.fingerprint())
            .all(|target| {
                target.endpoint_uri.as_str() == RELAY_PRIMARY_WSS
                    && target.status == RadrootsOutboxDeliveryTargetStatus::Accepted
            })
    );
    assert!(targets.iter().any(|target| {
        &target.endpoint_fingerprint == terminal.fingerprint()
            && target.status == RadrootsOutboxDeliveryTargetStatus::FailedTerminal
    }));
    assert!(targets.iter().any(|target| {
        target.transport_kind == RadrootsTransportKind::Reticulum
            && target.status == RadrootsOutboxDeliveryTargetStatus::DeferredUntilImplemented
    }));
}

#[tokio::test]
async fn outbox_transport_publish_failure_releases_retryable_claim() {
    let outbox = RadrootsOutbox::open_memory().await.expect("outbox");
    let store = RadrootsEventStore::open_memory().await.expect("store");
    let draft = generic_draft("adapter transport failure");
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
            .all(|relay| relay.outcome.kind() == RadrootsRelayOutcomeKind::ConnectionFailed)
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
    let outbox = RadrootsOutbox::open_memory().await.expect("outbox");
    let store = RadrootsEventStore::open_memory().await.expect("store");
    let draft = generic_draft("already accepted");
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
async fn outbox_publish_rejects_unknown_adapter_receipts() {
    let outbox = RadrootsOutbox::open_memory().await.expect("outbox");
    let store = RadrootsEventStore::open_memory().await.expect("store");
    let draft = generic_draft("unknown receipt");
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

    let error = publish_claimed_outbox_event(
        &outbox,
        &store,
        &UnknownRelayReceiptPublishAdapter,
        &publish_claim,
        RadrootsOutboxPublishPolicy::new(2_500),
        2_200,
    )
    .await
    .expect_err("unknown adapter receipt");

    assert!(matches!(
        error,
        RadrootsRelayTransportError::UnexpectedPublishReceiptRelayUrl { url }
            if url == RELAY_TERTIARY_WSS
    ));
    let event = outbox
        .get_event(receipt.outbox_event_id)
        .await
        .expect("event")
        .expect("event");
    assert_eq!(event.state, RadrootsOutboxEventState::Publishing);
    let observations = store
        .observations_for_event(signed.id_str())
        .await
        .expect("observations");
    assert_outbox_publish_observations(&observations, 0);
}

#[tokio::test]
async fn outbox_publish_skips_non_nostr_targets() {
    let outbox = RadrootsOutbox::open_memory().await.expect("outbox");
    let store = RadrootsEventStore::open_memory().await.expect("store");
    let draft = generic_draft("mixed target");
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
                    RadrootsTransportTarget::reticulum().expect("reticulum target"),
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
            && target.status == RadrootsOutboxDeliveryTargetStatus::DeferredUntilImplemented
    }));
}

#[tokio::test]
async fn outbox_publish_marks_published_when_delivery_plan_satisfaction_is_met_with_failure_diagnostics()
 {
    let outbox = RadrootsOutbox::open_memory().await.expect("outbox");
    let store = RadrootsEventStore::open_memory().await.expect("store");
    let draft = generic_draft("quorum");
    let receipt = outbox
        .enqueue_operation(outbox_operation_input(
            draft,
            vec![
                RELAY_PRIMARY_WSS.to_owned(),
                RELAY_SECONDARY_WSS.to_owned(),
                RELAY_TERTIARY_WSS.to_owned(),
            ],
            RadrootsTransportSatisfactionPolicy::quorum_accepted(2).expect("valid quorum"),
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
            bounded_relay_outcome(RadrootsRelayOutcome::duplicate_accepted(
                "duplicate: already have it",
            )),
        )
        .with_outcome(
            RELAY_TERTIARY_WSS,
            bounded_relay_outcome(RadrootsRelayOutcome::classify(
                "restricted: group write denied",
            )),
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
    let outbox = RadrootsOutbox::open_memory().await.expect("outbox");
    let store = RadrootsEventStore::open_memory().await.expect("store");
    let draft = generic_draft("republish accepted");
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
    let outbox = RadrootsOutbox::open_memory().await.expect("outbox");
    let store = RadrootsEventStore::open_memory().await.expect("store");
    let draft = generic_draft("republish terminal excluded");
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
    let outbox = RadrootsOutbox::open_memory().await.expect("outbox");
    let store = RadrootsEventStore::open_memory().await.expect("store");
    let draft = generic_draft("missing signature");
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
    let outbox = RadrootsOutbox::open_memory().await.expect("outbox");
    let store = RadrootsEventStore::open_memory().await.expect("store");
    let draft = generic_draft("adapter non transport failure");
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
    let outbox = RadrootsOutbox::open_memory().await.expect("outbox");
    let store = RadrootsEventStore::open_memory().await.expect("store");
    let draft = generic_draft("invalid relay target");
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

    let transport_error = publish_claimed_outbox_event_with_transport(
        &outbox,
        &store,
        &ScriptedTransport::new(Vec::new()),
        &publish_claim,
        RadrootsOutboxPublishPolicy::new(2_500),
        2_201,
    )
    .await
    .expect_err("invalid transport relay target");
    assert!(matches!(
        transport_error,
        RadrootsRelayTransportError::RelayUrlForbiddenDestination { .. }
    ));
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
    assert_eq!(receipt.verification_failed_count, 0);
    assert_eq!(receipt.admission_unsupported_count, 0);
    assert_eq!(receipt.admission_invalid_count, 0);
    assert_eq!(receipt.valid_stream_eligible_count, 1_000);
    assert_eq!(receipt.visible_count, 1_000);
    assert_eq!(receipt.events.len(), 1_000);
    assert!(receipt.events.iter().all(|event| event.valid_stream
        == RadrootsRelayFetchEventValidStream::Eligible
        && event.visibility == RadrootsRelayFetchEventVisibility::Visible));
    let replay = store.valid_stream_after(0, 1_000).await.expect("replay");
    assert_eq!(replay.len(), 1_000);
}
