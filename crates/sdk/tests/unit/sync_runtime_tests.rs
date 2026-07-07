#[cfg(feature = "radrootsd-proxy")]
use super::{
    CLAIM_OWNER, complete_proxy_publish_attempt, proxy_delivery_policy_from_remaining,
    proxy_error_message, proxy_outbox_idempotency_key, proxy_transport_error_job,
    push_proxy_claimed_outbox_event, push_proxy_event_receipt,
    transport_publish_target_from_outbox_target,
};
use super::{
    PushOutboxEventReceipt, PushOutboxEventState, PushOutboxReceipt, PushOutboxTargetOutcomeKind,
    SdkRelayAuthPolicy, SyncEventStoreStatus, SyncOutboxStatus, push_event_final_state,
    push_event_receipt, push_outbox_claim_token,
};
use crate::RadrootsSdkError;
#[cfg(feature = "radrootsd-proxy")]
use crate::adapters::radrootsd::{
    RadrootsdError, RadrootsdProxyConfig, RadrootsdProxyPublishAdapter,
};
#[cfg(feature = "radrootsd-proxy")]
use crate::workflow_runtime::{SdkWorkflowEnqueueRequest, enqueue_signed_workflow};
use futures::future::BoxFuture;
#[cfg(feature = "radrootsd-proxy")]
use radroots_authority::{
    RadrootsActorContext, RadrootsEventSigner, RadrootsSignerError, RadrootsSignerIdentity,
};
use radroots_event_store::RadrootsEventStoreStatusSummary;
#[cfg(feature = "radrootsd-proxy")]
use radroots_events::contract::RadrootsActorRole;
#[cfg(feature = "radrootsd-proxy")]
use radroots_events::draft::{RadrootsFrozenEventDraft, RadrootsSignedNostrEvent};
use radroots_events::ids::RadrootsEventId;
#[cfg(feature = "radrootsd-proxy")]
use radroots_events::kinds::KIND_FARM;
#[cfg(feature = "radrootsd-proxy")]
use radroots_events_codec::wire::{WireEventParts, to_frozen_draft};
#[cfg(feature = "radrootsd-proxy")]
use radroots_nostr::prelude::{
    RadrootsNostrKeys, RadrootsNostrSecretKey, radroots_nostr_sign_frozen_draft,
};
#[cfg(feature = "radrootsd-proxy")]
use radroots_outbox::{
    RadrootsOutboxClaimedEvent, RadrootsOutboxDeliveryPlanInput,
    RadrootsOutboxDeliveryTargetRecord, RadrootsOutboxDeliveryTargetStatus,
    RadrootsOutboxOperationInput,
};
use radroots_outbox::{RadrootsOutboxEventState, RadrootsOutboxStatusSummary};
#[cfg(feature = "radrootsd-proxy")]
use radroots_transport::{
    RADROOTS_RETICULUM_PREVIEW_ENDPOINT_URI, RadrootsTransportKind,
    RadrootsTransportSatisfactionPolicy, RadrootsTransportTarget,
};
use radroots_transport_nostr::{
    RadrootsRelayOutcomeKind, RadrootsRelayPublishAdapter, RadrootsRelayPublishReceipt,
    RadrootsRelayPublishRelayReceipt, RadrootsRelayPublishRequest, RadrootsRelayTransportError,
};
#[cfg(feature = "radrootsd-proxy")]
use radroots_transport_publish_protocol::{
    NostrPublishTargetSourcePolicy, TransportPublishDeliveryPolicy, TransportPublishJobStatus,
    TransportPublishJobView, TransportPublishOutcomeKind, TransportPublishTargetOutcome,
    TransportPublishTargetPolicy, TransportPublishTargetSource,
};
use std::collections::BTreeSet;
#[cfg(feature = "radrootsd-proxy")]
use std::io::ErrorKind;
#[cfg(feature = "radrootsd-proxy")]
use std::net::TcpListener;
#[cfg(feature = "radrootsd-proxy")]
use std::time::Duration;

#[cfg(feature = "radrootsd-proxy")]
const PROXY_SIGNER_SECRET_KEY_HEX: &str =
    "10c5304d6c9ae3a1a16f7860f1cc8f5e3a76225a2663b3a989a0d775919b7df5";
#[cfg(feature = "radrootsd-proxy")]
const PROXY_SIGNER_PUBLIC_KEY_HEX: &str =
    "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df";

struct UnusedPublishAdapter;

#[cfg(feature = "radrootsd-proxy")]
struct ProxyFixtureSigner {
    identity: RadrootsSignerIdentity,
    keys: RadrootsNostrKeys,
}

#[cfg(feature = "radrootsd-proxy")]
impl ProxyFixtureSigner {
    fn new() -> Self {
        let secret_key =
            RadrootsNostrSecretKey::from_hex(PROXY_SIGNER_SECRET_KEY_HEX).expect("secret key");
        let keys = RadrootsNostrKeys::new(secret_key);
        Self {
            identity: RadrootsSignerIdentity::new(PROXY_SIGNER_PUBLIC_KEY_HEX).expect("identity"),
            keys,
        }
    }
}

#[cfg(feature = "radrootsd-proxy")]
impl RadrootsEventSigner for ProxyFixtureSigner {
    fn pubkey(&self) -> &radroots_events::ids::RadrootsPublicKey {
        self.identity.pubkey()
    }

    fn sign_frozen_draft(
        &self,
        draft: &RadrootsFrozenEventDraft,
    ) -> Result<RadrootsSignedNostrEvent, RadrootsSignerError> {
        radroots_nostr_sign_frozen_draft(&self.keys, draft).map_err(|error| {
            RadrootsSignerError::SigningFailed {
                message: error.to_string(),
            }
        })
    }
}

impl RadrootsRelayPublishAdapter for UnusedPublishAdapter {
    fn publish<'a>(
        &'a self,
        _request: RadrootsRelayPublishRequest,
    ) -> BoxFuture<'a, Result<Vec<RadrootsRelayPublishRelayReceipt>, RadrootsRelayTransportError>>
    {
        Box::pin(async { Ok(Vec::new()) })
    }
}

#[cfg(feature = "radrootsd-proxy")]
fn proxy_actor() -> RadrootsActorContext {
    RadrootsActorContext::test(PROXY_SIGNER_PUBLIC_KEY_HEX, [RadrootsActorRole::Farmer])
        .expect("actor")
}

#[cfg(feature = "radrootsd-proxy")]
fn proxy_frozen_draft(d_tag: &str) -> RadrootsFrozenEventDraft {
    to_frozen_draft(
        WireEventParts {
            kind: KIND_FARM,
            content: "{}".to_owned(),
            tags: vec![vec!["d".to_owned(), d_tag.to_owned()]],
        },
        "radroots.farm.profile.v1",
        PROXY_SIGNER_PUBLIC_KEY_HEX,
        1_700_000_000,
    )
    .expect("frozen draft")
}

#[cfg(feature = "radrootsd-proxy")]
async fn claimed_proxy_event(d_tag: &str) -> (crate::RadrootsClient, RadrootsOutboxClaimedEvent) {
    let sdk = crate::RadrootsClient::builder()
        .fixed_clock(crate::RadrootsSdkTimestamp::from_unix_seconds(
            1_700_000_000,
        ))
        .build()
        .await
        .expect("sdk");
    let actor = proxy_actor();
    let draft = proxy_frozen_draft(d_tag);
    enqueue_signed_workflow(
        &sdk,
        SdkWorkflowEnqueueRequest {
            operation_kind: "sync.proxy.unit.v1",
            actor: &actor,
            frozen_draft: &draft,
            target_policy: crate::TargetPolicy::try_nostr_relays(
                ["wss://relay.example.com"],
                crate::NostrRelayUrlPolicy::Public,
            )
            .expect("target relays"),
            satisfaction_policy: crate::SatisfactionPolicy::AllTargets,
            idempotency_key: None,
        },
        &ProxyFixtureSigner::new(),
    )
    .await
    .expect("enqueue signed workflow");
    let claimed = sdk
        ._outbox
        .claim_next_ready_signed_event(
            CLAIM_OWNER,
            "proxy-unit-claim",
            1_700_000_060_000,
            1_700_000_000_000,
        )
        .await
        .expect("claim")
        .expect("claimed event");
    (sdk, claimed)
}

#[cfg(feature = "radrootsd-proxy")]
async fn claimed_uningested_proxy_event(
    d_tag: &str,
    proxy_endpoint: &str,
) -> (crate::RadrootsClient, RadrootsOutboxClaimedEvent) {
    let sdk = crate::RadrootsClient::builder()
        .fixed_clock(crate::RadrootsSdkTimestamp::from_unix_seconds(
            1_700_000_000,
        ))
        .build()
        .await
        .expect("sdk");
    let draft = proxy_frozen_draft(d_tag);
    let proxy_target = RadrootsTransportTarget::new(RadrootsTransportKind::Proxy, proxy_endpoint)
        .expect("proxy target");
    let enqueue = sdk
        ._outbox
        .enqueue_operation(
            RadrootsOutboxOperationInput::new(
                "sync.proxy.unit.v1",
                draft,
                RadrootsOutboxDeliveryPlanInput::new(
                    "proxy",
                    1,
                    RadrootsTransportSatisfactionPolicy::all_accepted(),
                    vec![proxy_target],
                ),
                1_700_000_000_000,
            )
            .with_idempotency_key(format!("proxy-uningested-{d_tag}")),
        )
        .await
        .expect("enqueue");
    let signing_claim = sdk
        ._outbox
        .claim_next_ready_event(
            CLAIM_OWNER,
            "proxy-unit-sign",
            1_700_000_000_500,
            1_700_000_000_000,
        )
        .await
        .expect("signing claim")
        .expect("signing claim");
    let signed_event = ProxyFixtureSigner::new()
        .sign_frozen_draft(&signing_claim.draft)
        .expect("signed event");
    sdk._outbox
        .complete_signing(
            enqueue.outbox_event_id,
            "proxy-unit-sign",
            signed_event,
            1_700_000_000_100,
        )
        .await
        .expect("complete signing");
    let stored = sdk
        ._outbox
        .get_event(enqueue.outbox_event_id)
        .await
        .expect("stored")
        .expect("stored");
    assert_eq!(stored.state, RadrootsOutboxEventState::Signed);
    assert!(!stored.event_store_ingested);
    assert_eq!(stored.event_store_ingested_at_ms, None);
    assert_eq!(stored.claim_token, None);
    let claimed = sdk
        ._outbox
        .claim_next_ready_signed_event(
            CLAIM_OWNER,
            "proxy-unit-publish",
            1_700_000_060_000,
            1_700_000_000_100,
        )
        .await
        .expect("publish claim")
        .expect("publish claim");
    (sdk, claimed)
}

#[cfg(feature = "radrootsd-proxy")]
fn assert_no_transport_publish_request(listener: &TcpListener) {
    listener.set_nonblocking(true).expect("nonblocking");
    match listener.accept() {
        Err(error) if error.kind() == ErrorKind::WouldBlock => {}
        Err(error) => panic!("transport publish listener failed before request check: {error}"),
        Ok(_) => panic!("transport publish listener received a request after local validation"),
    }
}

#[cfg(feature = "radrootsd-proxy")]
fn proxy_job(event_id: &str, outcome_kind: TransportPublishOutcomeKind) -> TransportPublishJobView {
    let delivery_satisfied = outcome_kind.counts_toward_satisfaction();
    let retryable = outcome_kind.is_retryable();
    let terminal_failure = outcome_kind.is_terminal_failure();
    let status = if delivery_satisfied {
        TransportPublishJobStatus::DeliverySatisfied
    } else if retryable {
        TransportPublishJobStatus::DeliveryUnsatisfiedRetryable
    } else if outcome_kind == TransportPublishOutcomeKind::DeferredUntilImplemented {
        TransportPublishJobStatus::DeliveryDeferred
    } else if outcome_kind == TransportPublishOutcomeKind::PreviewUnavailable {
        TransportPublishJobStatus::DeliveryPreviewUnavailable
    } else {
        TransportPublishJobStatus::DeliveryUnsatisfiedTerminal
    };
    TransportPublishJobView {
        job_id: "proxy-unit-job".to_owned(),
        status,
        terminal: !retryable,
        delivery_satisfied,
        event_id: event_id.to_owned(),
        pubkey: PROXY_SIGNER_PUBLIC_KEY_HEX.to_owned(),
        event_kind: KIND_FARM,
        target_policy: TransportPublishTargetPolicy::nostr(
            NostrPublishTargetSourcePolicy::RequestThenAuthorWriteThenDaemonDefault,
            vec!["wss://relay.example.com".to_owned()],
        ),
        delivery_policy: TransportPublishDeliveryPolicy::Any,
        target_count: 1,
        acknowledged_count: usize::from(delivery_satisfied),
        retryable_count: usize::from(retryable),
        terminal_count: usize::from(terminal_failure),
        requested_at_ms: 1_700_000_000_000,
        completed_at_ms: Some(1_700_000_000_100),
        last_error: None,
        targets: vec![TransportPublishTargetOutcome {
            transport_kind: "nostr".to_owned(),
            endpoint_uri: "wss://relay.example.com".to_owned(),
            source: TransportPublishTargetSource::Request,
            attempted: true,
            outcome_kind,
            message: Some("daemon outcome".to_owned()),
            latency_ms: Some(4),
        }],
    }
}

#[test]
fn push_outbox_claim_tokens_are_unique_under_immediate_generation() {
    let mut tokens = BTreeSet::new();
    for _ in 0..1_024 {
        let token = push_outbox_claim_token();
        assert!(token.starts_with("radroots-sdk-sync-"));
        assert!(tokens.insert(token));
    }
}

#[test]
fn push_event_receipt_parses_typed_event_id() {
    let event_id = "a".repeat(64);
    let receipt = push_event_receipt(
        1,
        PushOutboxEventState::Published,
        relay_publish_receipt(event_id.as_str()).with_relay(),
    )
    .expect("receipt");

    assert_eq!(
        receipt.event_id,
        RadrootsEventId::parse(event_id).expect("event id")
    );
    assert_eq!(receipt.targets.len(), 1);
    assert_eq!(receipt.targets[0].transport_kind, "nostr");
    assert_eq!(receipt.targets[0].endpoint_uri, "wss://relay.example.com");
    assert!(receipt.targets[0].attempted);
}

#[test]
fn push_event_receipt_returns_typed_error_for_invalid_internal_event_id() {
    let error = push_event_receipt(
        1,
        PushOutboxEventState::Published,
        relay_publish_receipt("not-a-valid-event-id"),
    )
    .expect_err("invalid event id");
    assert!(matches!(
        error,
        RadrootsSdkError::InvalidRequest { message }
            if message.contains("relay transport publish receipt event id is invalid")
    ));
}

#[test]
fn push_event_final_state_follows_publish_quorum_and_retryability() {
    let published = relay_publish_receipt("a".repeat(64).as_str())
        .with_quorum_met(true)
        .with_retryable_count(1);
    assert_eq!(
        push_event_final_state(&published),
        PushOutboxEventState::Published
    );

    let retryable = relay_publish_receipt("b".repeat(64).as_str()).with_retryable_count(1);
    assert_eq!(
        push_event_final_state(&retryable),
        PushOutboxEventState::PublishRetryable
    );

    let terminal = relay_publish_receipt("c".repeat(64).as_str());
    assert_eq!(
        push_event_final_state(&terminal),
        PushOutboxEventState::FailedTerminal
    );
}

#[test]
fn push_relay_outcome_mapping_covers_daemon_proxy_results() {
    assert_eq!(
        PushOutboxTargetOutcomeKind::from(RadrootsRelayOutcomeKind::Muted),
        PushOutboxTargetOutcomeKind::Muted
    );
    assert_eq!(
        PushOutboxTargetOutcomeKind::from(RadrootsRelayOutcomeKind::Unsupported),
        PushOutboxTargetOutcomeKind::Unsupported
    );
    assert_eq!(
        PushOutboxTargetOutcomeKind::from(RadrootsRelayOutcomeKind::PaymentRequired),
        PushOutboxTargetOutcomeKind::PaymentRequired
    );
    assert_eq!(
        PushOutboxTargetOutcomeKind::from(RadrootsRelayOutcomeKind::RelayUrlRejected),
        PushOutboxTargetOutcomeKind::TargetUriRejected
    );
    assert_eq!(
        PushOutboxTargetOutcomeKind::from(RadrootsRelayOutcomeKind::SkippedAlreadyAccepted),
        PushOutboxTargetOutcomeKind::SkippedAlreadyAccepted
    );
}

#[test]
fn auth_policy_defaults_and_outbox_state_mappings_cover_all_public_states() {
    assert_eq!(
        SdkRelayAuthPolicy::default(),
        SdkRelayAuthPolicy::DetectOnly
    );

    assert_eq!(
        PushOutboxEventState::from(RadrootsOutboxEventState::DraftQueued),
        PushOutboxEventState::DraftQueued
    );
    assert_eq!(
        PushOutboxEventState::from(RadrootsOutboxEventState::Signing),
        PushOutboxEventState::Signing
    );
    assert_eq!(
        PushOutboxEventState::from(RadrootsOutboxEventState::Signed),
        PushOutboxEventState::Signed
    );
    assert_eq!(
        PushOutboxEventState::from(RadrootsOutboxEventState::Publishing),
        PushOutboxEventState::Publishing
    );
    assert_eq!(
        PushOutboxEventState::from(RadrootsOutboxEventState::Published),
        PushOutboxEventState::Published
    );
    assert_eq!(
        PushOutboxEventState::from(RadrootsOutboxEventState::SignRetryable),
        PushOutboxEventState::SignRetryable
    );
    assert_eq!(
        PushOutboxEventState::from(RadrootsOutboxEventState::PublishRetryable),
        PushOutboxEventState::PublishRetryable
    );
    assert_eq!(
        PushOutboxEventState::from(RadrootsOutboxEventState::FailedTerminal),
        PushOutboxEventState::FailedTerminal
    );
    assert_eq!(
        PushOutboxEventState::from(RadrootsOutboxEventState::Cancelled),
        PushOutboxEventState::Cancelled
    );

    let mut receipt = PushOutboxReceipt::default();
    receipt.push_event(push_receipt(PushOutboxEventState::Published));
    receipt.push_event(push_receipt(PushOutboxEventState::PublishRetryable));
    receipt.push_event(push_receipt(PushOutboxEventState::FailedTerminal));
    receipt.push_event(push_receipt(PushOutboxEventState::Cancelled));
    assert_eq!(receipt.attempted_events, 4);
    assert_eq!(receipt.terminal_events, 1);
    assert_eq!(receipt.published_events, 1);
    assert_eq!(receipt.retryable_events, 1);
}

#[test]
fn sync_status_summary_conversions_preserve_all_fields() {
    let event_summary = RadrootsEventStoreStatusSummary {
        total_events: 11,
        projection_eligible_events: 7,
        transport_observations: 3,
        last_event_seq: Some(9),
        last_event_updated_at_ms: Some(1_700_000_000_000),
    };
    let event_status = SyncEventStoreStatus::from(event_summary);
    assert_eq!(event_status.total_events, 11);
    assert_eq!(event_status.projection_eligible_events, 7);
    assert_eq!(event_status.transport_observations, 3);
    assert_eq!(event_status.last_event_seq, Some(9));
    assert_eq!(
        event_status.last_event_updated_at_ms,
        Some(1_700_000_000_000)
    );

    let outbox_summary = RadrootsOutboxStatusSummary {
        total_events: 13,
        pending_events: 5,
        retryable_events: 4,
        terminal_events: 2,
        failed_terminal_events: 1,
        preview_unavailable_events: 9,
        deferred_until_implemented_events: 10,
        ready_signed_events: 6,
        publishing_events: 8,
        last_attempt_at_ms: Some(1_700_000_000_001),
        last_error: Some("relay offline".to_owned()),
    };
    let outbox_status = SyncOutboxStatus::from(outbox_summary);
    assert_eq!(outbox_status.total_events, 13);
    assert_eq!(outbox_status.pending_events, 5);
    assert_eq!(outbox_status.retryable_events, 4);
    assert_eq!(outbox_status.terminal_events, 2);
    assert_eq!(outbox_status.failed_terminal_events, 1);
    assert_eq!(outbox_status.preview_unavailable_events, 9);
    assert_eq!(outbox_status.deferred_until_implemented_events, 10);
    assert_eq!(outbox_status.ready_signed_events, 6);
    assert_eq!(outbox_status.publishing_events, 8);
    assert_eq!(outbox_status.last_attempt_at_ms, Some(1_700_000_000_001));
    assert_eq!(outbox_status.last_error.as_deref(), Some("relay offline"));
}

#[test]
fn push_outbox_request_builders_validate_all_bounds() {
    let request = super::PushOutboxRequest::new()
        .with_limit(2)
        .with_outbox_event_id(9)
        .republish_accepted_targets(true)
        .with_nostr_relay_url_policy(crate::NostrRelayUrlPolicy::Localhost)
        .with_auth_policy(SdkRelayAuthPolicy::DetectOnly)
        .with_claim_ttl_ms(7)
        .with_next_attempt_delay_ms(11);
    assert_eq!(request.limit, 1);
    assert_eq!(request.outbox_event_id, Some(9));
    assert!(request.republish_accepted_targets);
    request.validate().expect("valid request");

    assert!(matches!(
        super::PushOutboxRequest::new().with_limit(0).validate(),
        Err(RadrootsSdkError::InvalidRequest { message }) if message.contains("limit")
    ));
    assert!(matches!(
        super::PushOutboxRequest::new()
            .with_limit(super::PUSH_OUTBOX_MAX_LIMIT + 1)
            .validate(),
        Err(RadrootsSdkError::InvalidRequest { message }) if message.contains("limit")
    ));
    assert!(matches!(
        super::PushOutboxRequest::new()
            .with_outbox_event_id(0)
            .validate(),
        Err(RadrootsSdkError::InvalidRequest { message }) if message.contains("outbox event id")
    ));
    assert!(matches!(
        super::PushOutboxRequest::new()
            .with_claim_ttl_ms(0)
            .validate(),
        Err(RadrootsSdkError::InvalidRequest { message }) if message.contains("TTL")
    ));
    assert!(matches!(
        super::PushOutboxRequest::new()
            .with_next_attempt_delay_ms(0)
            .validate(),
        Err(RadrootsSdkError::InvalidRequest { message }) if message.contains("next attempt")
    ));
}

#[test]
fn relay_outcome_kind_mapping_covers_all_transport_outcomes() {
    assert_eq!(
        PushOutboxTargetOutcomeKind::from(RadrootsRelayOutcomeKind::Accepted),
        PushOutboxTargetOutcomeKind::Accepted
    );
    assert_eq!(
        PushOutboxTargetOutcomeKind::from(RadrootsRelayOutcomeKind::DuplicateAccepted),
        PushOutboxTargetOutcomeKind::DuplicateAccepted
    );
    assert_eq!(
        PushOutboxTargetOutcomeKind::from(RadrootsRelayOutcomeKind::Blocked),
        PushOutboxTargetOutcomeKind::Blocked
    );
    assert_eq!(
        PushOutboxTargetOutcomeKind::from(RadrootsRelayOutcomeKind::RateLimited),
        PushOutboxTargetOutcomeKind::RateLimited
    );
    assert_eq!(
        PushOutboxTargetOutcomeKind::from(RadrootsRelayOutcomeKind::Invalid),
        PushOutboxTargetOutcomeKind::Invalid
    );
    assert_eq!(
        PushOutboxTargetOutcomeKind::from(RadrootsRelayOutcomeKind::PowRequired),
        PushOutboxTargetOutcomeKind::PowRequired
    );
    assert_eq!(
        PushOutboxTargetOutcomeKind::from(RadrootsRelayOutcomeKind::Restricted),
        PushOutboxTargetOutcomeKind::Restricted
    );
    assert_eq!(
        PushOutboxTargetOutcomeKind::from(RadrootsRelayOutcomeKind::AuthRequired),
        PushOutboxTargetOutcomeKind::AuthRequired
    );
    assert_eq!(
        PushOutboxTargetOutcomeKind::from(RadrootsRelayOutcomeKind::Error),
        PushOutboxTargetOutcomeKind::Error
    );
    assert_eq!(
        PushOutboxTargetOutcomeKind::from(RadrootsRelayOutcomeKind::Timeout),
        PushOutboxTargetOutcomeKind::Timeout
    );
    assert_eq!(
        PushOutboxTargetOutcomeKind::from(RadrootsRelayOutcomeKind::ConnectionFailed),
        PushOutboxTargetOutcomeKind::ConnectionFailed
    );
    assert_eq!(
        PushOutboxTargetOutcomeKind::from(RadrootsRelayOutcomeKind::Unknown),
        PushOutboxTargetOutcomeKind::Unknown
    );
}

#[tokio::test]
async fn sync_status_maps_closed_store_errors() {
    let event_store_closed = crate::RadrootsClient::builder().build().await.expect("sdk");
    event_store_closed._event_store.pool().close().await;
    assert!(matches!(
        event_store_closed
            .sync()
            .status(super::SyncStatusRequest::new())
            .await,
        Err(RadrootsSdkError::EventStore { .. })
    ));

    let outbox_closed = crate::RadrootsClient::builder().build().await.expect("sdk");
    outbox_closed._outbox.pool().close().await;
    assert!(matches!(
        outbox_closed
            .sync()
            .status(super::SyncStatusRequest::new())
            .await,
        Err(RadrootsSdkError::Outbox { .. })
    ));
}

#[tokio::test]
async fn sync_runtime_reports_clock_errors_before_store_or_relay_work() {
    let sdk = crate::RadrootsClient::builder()
        .clock(crate::RadrootsSdkClock::BeforeUnixEpoch)
        .build()
        .await
        .expect("sdk");
    assert!(matches!(
        sdk.sync().status(super::SyncStatusRequest::new()).await,
        Err(RadrootsSdkError::ClockBeforeUnixEpoch)
    ));
    assert!(matches!(
        sdk.sync()
            .push_outbox_with_adapter(&UnusedPublishAdapter, super::PushOutboxRequest::new())
            .await,
        Err(RadrootsSdkError::ClockBeforeUnixEpoch)
    ));
}

#[cfg(feature = "radrootsd-proxy")]
#[tokio::test]
async fn proxy_push_empty_queue_and_private_helpers_are_deterministic() {
    let sdk = crate::RadrootsClient::builder().build().await.expect("sdk");
    let adapter =
        RadrootsdProxyPublishAdapter::new(RadrootsdProxyConfig::new("http://127.0.0.1:9/rpc"));

    let receipt = sdk
        .sync()
        .push_outbox_with_proxy_adapter(&adapter, super::PushOutboxRequest::new())
        .await
        .expect("empty transport publish push");

    assert_eq!(receipt.attempted_events, 0);
    assert_eq!(
        proxy_delivery_policy_from_remaining(
            0,
            0,
            &RadrootsTransportSatisfactionPolicy::all_accepted()
        )
        .expect("zero-target proxy policy"),
        TransportPublishDeliveryPolicy::Any
    );
    assert_eq!(
        proxy_delivery_policy_from_remaining(
            2,
            2,
            &RadrootsTransportSatisfactionPolicy::all_accepted()
        )
        .expect("all-target proxy policy"),
        TransportPublishDeliveryPolicy::All
    );
    assert_eq!(
        proxy_delivery_policy_from_remaining(
            2,
            1,
            &RadrootsTransportSatisfactionPolicy::any_accepted()
        )
        .expect("any-target proxy policy"),
        TransportPublishDeliveryPolicy::Any
    );
    assert_eq!(
        proxy_outbox_idempotency_key(7, 3, "event-id", 5),
        "radroots-sdk-outbox-7-3-event-id-5"
    );

    let signed_event = ProxyFixtureSigner::new()
        .sign_frozen_draft(&proxy_frozen_draft("proxy-transport-error-job"))
        .expect("signed event");
    let proxy_job = proxy_transport_error_job(&signed_event);
    assert_eq!(proxy_job.event_id, signed_event.id);
    assert_eq!(proxy_job.target_count, 1);
    assert_eq!(proxy_job.retryable_count, 1);
    assert!(!proxy_job.delivery_satisfied);
    assert!(proxy_job.targets.is_empty());
    assert_eq!(
        proxy_error_message(&RadrootsdError::Http("connection refused".to_owned())),
        "radrootsd proxy publish failed: connection refused"
    );
}

#[cfg(feature = "radrootsd-proxy")]
#[test]
fn proxy_outbox_target_conversion_rejects_reticulum_targets_before_behavior_loss() {
    let target = RadrootsTransportTarget::new(
        RadrootsTransportKind::Reticulum,
        "reticulum:preview-unavailable",
    )
    .expect("Reticulum target");
    let record = RadrootsOutboxDeliveryTargetRecord {
        delivery_target_id: 1,
        delivery_plan_id: 1,
        transport_kind: target.kind.clone(),
        endpoint_uri: target.uri.clone(),
        endpoint_fingerprint: target.fingerprint.clone(),
        status: RadrootsOutboxDeliveryTargetStatus::Pending,
        attempt_count: 0,
        last_attempt_at_ms: None,
        completed_at_ms: None,
        last_error: None,
    };

    let error =
        transport_publish_target_from_outbox_target(&record).expect_err("Reticulum rejected");

    assert!(matches!(
        error,
        RadrootsSdkError::InvalidRequest { message }
            if message.contains("radrootsd proxy outbox publish")
                && message.contains("Nostr-only")
                && message.contains("reticulum target")
    ));
}

#[cfg(feature = "radrootsd-proxy")]
#[test]
fn proxy_outbox_target_conversion_rejects_proxy_targets_before_daemon_explicit_target() {
    let target =
        RadrootsTransportTarget::new(RadrootsTransportKind::Proxy, "http://127.0.0.1:8080/rpc")
            .expect("proxy target");
    let record = RadrootsOutboxDeliveryTargetRecord {
        delivery_target_id: 1,
        delivery_plan_id: 1,
        transport_kind: target.kind.clone(),
        endpoint_uri: target.uri.clone(),
        endpoint_fingerprint: target.fingerprint.clone(),
        status: RadrootsOutboxDeliveryTargetStatus::Pending,
        attempt_count: 0,
        last_attempt_at_ms: None,
        completed_at_ms: None,
        last_error: None,
    };

    let error = transport_publish_target_from_outbox_target(&record).expect_err("proxy rejected");

    assert!(matches!(
        error,
        RadrootsSdkError::InvalidRequest { message }
            if message.contains("Nostr-only") && message.contains("proxy target")
    ));
}

#[cfg(feature = "radrootsd-proxy")]
#[tokio::test]
async fn proxy_push_entrypoints_report_request_clock_and_claim_errors() {
    let adapter =
        RadrootsdProxyPublishAdapter::new(RadrootsdProxyConfig::new("http://127.0.0.1:9/rpc"));
    let sdk = crate::RadrootsClient::builder().build().await.expect("sdk");
    assert!(matches!(
        sdk.sync()
            .push_outbox_with_proxy_adapter(&adapter, super::PushOutboxRequest::new().with_limit(0))
            .await,
        Err(RadrootsSdkError::InvalidRequest { .. })
    ));

    let clock_sdk = crate::RadrootsClient::builder()
        .clock(crate::RadrootsSdkClock::BeforeUnixEpoch)
        .build()
        .await
        .expect("clock sdk");
    assert!(matches!(
        clock_sdk
            .sync()
            .push_outbox_with_proxy_adapter(&adapter, super::PushOutboxRequest::new())
            .await,
        Err(RadrootsSdkError::ClockBeforeUnixEpoch)
    ));

    let closed_outbox_sdk = crate::RadrootsClient::builder()
        .build()
        .await
        .expect("closed sdk");
    closed_outbox_sdk._outbox.pool().close().await;
    assert!(matches!(
        closed_outbox_sdk
            .sync()
            .push_outbox_with_proxy_adapter(&adapter, super::PushOutboxRequest::new())
            .await,
        Err(RadrootsSdkError::Outbox { .. })
    ));
}

#[cfg(feature = "radrootsd-proxy")]
#[tokio::test]
async fn proxy_push_reports_missing_signed_claim_before_daemon_publish() {
    let sdk = crate::RadrootsClient::builder().build().await.expect("sdk");
    let sync = sdk.sync();
    let adapter =
        RadrootsdProxyPublishAdapter::new(RadrootsdProxyConfig::new("http://127.0.0.1:9/rpc"));
    let claimed = RadrootsOutboxClaimedEvent {
        outbox_event_id: 41,
        operation_id: 42,
        expected_event_id: "b".repeat(64),
        attempt_count: 3,
        state: RadrootsOutboxEventState::Signed,
        claim_token: "claim-token".to_owned(),
        active_delivery_plan_id: Some(1),
        draft: RadrootsFrozenEventDraft {
            contract_id: "radroots.test".to_owned(),
            contract_registry_version: 1,
            kind: 1,
            created_at: 1_700_000_000,
            tags: Vec::new(),
            content: "{}".to_owned(),
            expected_pubkey: "a".repeat(64),
            expected_event_id: "b".repeat(64),
        },
        signed_event: None,
        delivery_targets: Vec::new(),
    };

    assert!(matches!(
        push_proxy_claimed_outbox_event(&sync, &adapter, &claimed, 60_000, 1_700_000_000_000)
            .await,
        Err(RadrootsSdkError::Transport { message })
            if message.contains("Outbox claim 41 does not contain a signed event")
    ));
}

#[cfg(feature = "radrootsd-proxy")]
#[tokio::test]
async fn proxy_claim_publish_marks_retryable_transport_errors() {
    let (sdk, claimed) = claimed_proxy_event("proxy-transport-error").await;
    let sync = sdk.sync();
    let adapter =
        RadrootsdProxyPublishAdapter::new(RadrootsdProxyConfig::new("http://127.0.0.1:9/rpc"));
    let receipt =
        push_proxy_claimed_outbox_event(&sync, &adapter, &claimed, 60_000, 1_700_000_000_000)
            .await
            .expect("transport error job");

    assert_eq!(receipt.retryable_count, 1);
    let stored = sdk
        ._outbox
        .get_event(claimed.outbox_event_id)
        .await
        .expect("stored")
        .expect("stored");
    assert_eq!(stored.state, RadrootsOutboxEventState::PublishRetryable);
    assert!(stored.claim_token.is_none());
}

#[cfg(feature = "radrootsd-proxy")]
#[tokio::test]
async fn proxy_local_validation_errors_release_claim_before_daemon_publish() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind proxy listener");
    let endpoint = format!("http://{}/rpc", listener.local_addr().expect("addr"));
    let (sdk, mut claimed) =
        claimed_uningested_proxy_event("proxy-local-validation-error", endpoint.as_str()).await;
    let stored_before = sdk
        ._outbox
        .get_event(claimed.outbox_event_id)
        .await
        .expect("stored before")
        .expect("stored before");
    assert!(!stored_before.event_store_ingested);
    assert_eq!(stored_before.event_store_ingested_at_ms, None);
    let reticulum_target = RadrootsTransportTarget::new(
        RadrootsTransportKind::Reticulum,
        RADROOTS_RETICULUM_PREVIEW_ENDPOINT_URI,
    )
    .expect("Reticulum target");
    claimed.delivery_targets[0].transport_kind = reticulum_target.kind;
    claimed.delivery_targets[0].endpoint_uri = reticulum_target.uri;
    claimed.delivery_targets[0].endpoint_fingerprint = reticulum_target.fingerprint;
    let sync = sdk.sync();
    let adapter = RadrootsdProxyPublishAdapter::new(
        RadrootsdProxyConfig::new(endpoint).with_timeout(Duration::from_millis(50)),
    );
    let error =
        push_proxy_claimed_outbox_event(&sync, &adapter, &claimed, 60_000, 1_700_000_000_000)
            .await
            .expect_err("local proxy validation error");
    assert_no_transport_publish_request(&listener);

    assert!(matches!(
        error,
        RadrootsSdkError::InvalidRequest { message }
            if message.contains("radrootsd proxy outbox publish")
                && message.contains("Nostr-only")
                && message.contains("reticulum target")
    ));
    let stored = sdk
        ._outbox
        .get_event(claimed.outbox_event_id)
        .await
        .expect("stored")
        .expect("stored");
    assert_eq!(stored.state, RadrootsOutboxEventState::FailedTerminal);
    assert!(stored.claim_token.is_none());
    assert!(!stored.event_store_ingested);
    assert_eq!(stored.event_store_ingested_at_ms, None);
    assert!(
        stored
            .last_error
            .as_deref()
            .expect("last error")
            .contains("reticulum target")
    );
}

#[cfg(feature = "radrootsd-proxy")]
#[tokio::test]
async fn proxy_completion_updates_outbox_for_success_retryable_and_terminal_receipts() {
    let cases = [
        (
            "proxy-complete-success",
            PushOutboxEventState::Published,
            PushOutboxEventState::Published,
            RadrootsOutboxDeliveryTargetStatus::Accepted,
            TransportPublishOutcomeKind::Accepted,
        ),
        (
            "proxy-complete-retryable",
            PushOutboxEventState::PublishRetryable,
            PushOutboxEventState::PublishRetryable,
            RadrootsOutboxDeliveryTargetStatus::FailedRetryable,
            TransportPublishOutcomeKind::Timeout,
        ),
        (
            "proxy-complete-terminal",
            PushOutboxEventState::FailedTerminal,
            PushOutboxEventState::FailedTerminal,
            RadrootsOutboxDeliveryTargetStatus::FailedTerminal,
            TransportPublishOutcomeKind::Blocked,
        ),
        (
            "proxy-complete-deferred",
            PushOutboxEventState::DeferredUntilImplemented,
            PushOutboxEventState::DeferredUntilImplemented,
            RadrootsOutboxDeliveryTargetStatus::DeferredUntilImplemented,
            TransportPublishOutcomeKind::DeferredUntilImplemented,
        ),
        (
            "proxy-complete-preview-unavailable",
            PushOutboxEventState::PreviewUnavailable,
            PushOutboxEventState::PreviewUnavailable,
            RadrootsOutboxDeliveryTargetStatus::PreviewUnavailable,
            TransportPublishOutcomeKind::PreviewUnavailable,
        ),
    ];

    for (
        d_tag,
        expected_receipt_state,
        expected_stored_state,
        expected_target_status,
        outcome_kind,
    ) in cases
    {
        let (sdk, claimed) = claimed_proxy_event(d_tag).await;
        let publish = proxy_job(
            claimed
                .signed_event
                .as_ref()
                .expect("signed event")
                .id
                .as_str(),
            outcome_kind,
        );
        assert_eq!(
            publish.event_id,
            claimed
                .signed_event
                .as_ref()
                .expect("signed event")
                .id
                .clone()
        );
        assert_eq!(
            publish.pubkey,
            claimed.signed_event.as_ref().expect("signed event").pubkey
        );
        assert_eq!(
            publish.event_kind,
            claimed.signed_event.as_ref().expect("signed event").kind
        );
        let proxy_receipt =
            push_proxy_event_receipt(claimed.outbox_event_id, publish.clone()).expect("receipt");
        assert_eq!(proxy_receipt.final_state, expected_receipt_state);
        let sync = sdk.sync();
        complete_proxy_publish_attempt(&sync, &claimed, &publish, 60_000, 1_700_000_000_000)
            .await
            .expect("complete proxy attempt");
        let stored = sdk
            ._outbox
            .get_event(claimed.outbox_event_id)
            .await
            .expect("stored")
            .expect("stored");
        assert_eq!(
            PushOutboxEventState::from(stored.state),
            expected_stored_state
        );
        assert!(stored.claim_token.is_none());
        let targets = sdk
            ._outbox
            .delivery_targets(claimed.outbox_event_id)
            .await
            .expect("targets");
        assert_eq!(targets[0].status, expected_target_status);
    }
}

#[cfg(feature = "radrootsd-proxy")]
#[test]
fn push_proxy_event_receipt_returns_typed_error_for_invalid_daemon_event_id() {
    let error = push_proxy_event_receipt(
        1,
        proxy_job(
            "not-a-valid-event-id",
            TransportPublishOutcomeKind::Accepted,
        ),
    )
    .expect_err("invalid daemon event id");
    assert!(matches!(
        error,
        RadrootsSdkError::InvalidRequest { message }
            if message.contains("transport publish daemon job event id is invalid")
    ));
}

fn relay_publish_receipt(event_id: &str) -> RadrootsRelayPublishReceipt {
    RadrootsRelayPublishReceipt {
        event_id: event_id.to_owned(),
        attempted_count: 0,
        accepted_count: 0,
        retryable_count: 0,
        terminal_count: 0,
        quorum: 0,
        quorum_met: false,
        relays: Vec::new(),
    }
}

trait RelayReceiptFixture {
    fn with_relay(self) -> Self;
    fn with_quorum_met(self, quorum_met: bool) -> Self;
    fn with_retryable_count(self, retryable_count: usize) -> Self;
}

impl RelayReceiptFixture for RadrootsRelayPublishReceipt {
    fn with_relay(mut self) -> Self {
        self.relays.push(
            radroots_transport_nostr::RadrootsRelayPublishRelayReceipt::attempted(
                "wss://relay.example.com",
                radroots_transport_nostr::RadrootsRelayOutcome::accepted(),
            ),
        );
        self
    }

    fn with_quorum_met(mut self, quorum_met: bool) -> Self {
        self.quorum_met = quorum_met;
        self
    }

    fn with_retryable_count(mut self, retryable_count: usize) -> Self {
        self.retryable_count = retryable_count;
        self
    }
}

fn push_receipt(final_state: PushOutboxEventState) -> PushOutboxEventReceipt {
    PushOutboxEventReceipt {
        event_id: RadrootsEventId::parse("a".repeat(64)).expect("event id"),
        outbox_event_id: 1,
        final_state,
        attempted_count: 0,
        accepted_count: 0,
        retryable_count: 0,
        terminal_count: 0,
        quorum: 0,
        quorum_met: false,
        targets: Vec::new(),
    }
}
