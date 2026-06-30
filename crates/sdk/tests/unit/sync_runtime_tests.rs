#[cfg(feature = "radrootsd-proxy")]
use super::{
    CLAIM_OWNER, complete_proxy_publish_attempt, proxy_delivery_policy, proxy_error_message,
    proxy_outbox_idempotency_key, proxy_transport_error_receipt, push_proxy_claimed_outbox_event,
};
use super::{
    PushOutboxEventReceipt, PushOutboxEventState, PushOutboxReceipt, PushOutboxRelayOutcomeKind,
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
use radroots_outbox::RadrootsOutboxClaimedEvent;
use radroots_outbox::{RadrootsOutboxEventState, RadrootsOutboxStatusSummary};
#[cfg(feature = "radrootsd-proxy")]
use radroots_publish_proxy_protocol::PublishDeliveryPolicy;
use radroots_relay_transport::{
    RadrootsRelayOutcomeKind, RadrootsRelayPublishAdapter, RadrootsRelayPublishReceipt,
    RadrootsRelayPublishRelayReceipt, RadrootsRelayPublishRequest, RadrootsRelayTransportError,
};
use std::collections::BTreeSet;

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
            target_relays: crate::SdkRelayTargetPolicy::try_explicit(
                ["wss://relay.example.com"],
                crate::SdkRelayUrlPolicy::Public,
            )
            .expect("target relays"),
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
    );

    assert_eq!(
        receipt.event_id,
        RadrootsEventId::parse(event_id).expect("event id")
    );
    assert_eq!(receipt.relays.len(), 1);
    assert!(receipt.relays[0].attempted);
}

#[test]
#[should_panic(expected = "relay transport publish receipt uses signed event id")]
fn push_event_receipt_panics_on_invalid_internal_event_id() {
    let _ = push_event_receipt(
        1,
        PushOutboxEventState::Published,
        relay_publish_receipt("not-a-valid-event-id"),
    );
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
        PushOutboxRelayOutcomeKind::from(RadrootsRelayOutcomeKind::Muted),
        PushOutboxRelayOutcomeKind::Muted
    );
    assert_eq!(
        PushOutboxRelayOutcomeKind::from(RadrootsRelayOutcomeKind::Unsupported),
        PushOutboxRelayOutcomeKind::Unsupported
    );
    assert_eq!(
        PushOutboxRelayOutcomeKind::from(RadrootsRelayOutcomeKind::PaymentRequired),
        PushOutboxRelayOutcomeKind::PaymentRequired
    );
    assert_eq!(
        PushOutboxRelayOutcomeKind::from(RadrootsRelayOutcomeKind::RelayUrlRejected),
        PushOutboxRelayOutcomeKind::RelayUrlRejected
    );
    assert_eq!(
        PushOutboxRelayOutcomeKind::from(RadrootsRelayOutcomeKind::SkippedAlreadyAccepted),
        PushOutboxRelayOutcomeKind::SkippedAlreadyAccepted
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
        relay_observations: 3,
        last_event_seq: Some(9),
        last_event_updated_at_ms: Some(1_700_000_000_000),
    };
    let event_status = SyncEventStoreStatus::from(event_summary);
    assert_eq!(event_status.total_events, 11);
    assert_eq!(event_status.projection_eligible_events, 7);
    assert_eq!(event_status.relay_observations, 3);
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
        .republish_accepted_relays(true)
        .with_accepted_quorum(2)
        .with_relay_url_policy(crate::SdkRelayUrlPolicy::Localhost)
        .with_auth_policy(SdkRelayAuthPolicy::DetectOnly)
        .with_claim_ttl_ms(7)
        .with_next_attempt_delay_ms(11);
    assert_eq!(request.limit, 1);
    assert_eq!(request.outbox_event_id, Some(9));
    assert!(request.republish_accepted_relays);
    assert_eq!(request.accepted_quorum, Some(2));
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
            .with_accepted_quorum(0)
            .validate(),
        Err(RadrootsSdkError::InvalidRequest { message }) if message.contains("accepted quorum")
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
        PushOutboxRelayOutcomeKind::from(RadrootsRelayOutcomeKind::Accepted),
        PushOutboxRelayOutcomeKind::Accepted
    );
    assert_eq!(
        PushOutboxRelayOutcomeKind::from(RadrootsRelayOutcomeKind::DuplicateAccepted),
        PushOutboxRelayOutcomeKind::DuplicateAccepted
    );
    assert_eq!(
        PushOutboxRelayOutcomeKind::from(RadrootsRelayOutcomeKind::Blocked),
        PushOutboxRelayOutcomeKind::Blocked
    );
    assert_eq!(
        PushOutboxRelayOutcomeKind::from(RadrootsRelayOutcomeKind::RateLimited),
        PushOutboxRelayOutcomeKind::RateLimited
    );
    assert_eq!(
        PushOutboxRelayOutcomeKind::from(RadrootsRelayOutcomeKind::Invalid),
        PushOutboxRelayOutcomeKind::Invalid
    );
    assert_eq!(
        PushOutboxRelayOutcomeKind::from(RadrootsRelayOutcomeKind::PowRequired),
        PushOutboxRelayOutcomeKind::PowRequired
    );
    assert_eq!(
        PushOutboxRelayOutcomeKind::from(RadrootsRelayOutcomeKind::Restricted),
        PushOutboxRelayOutcomeKind::Restricted
    );
    assert_eq!(
        PushOutboxRelayOutcomeKind::from(RadrootsRelayOutcomeKind::AuthRequired),
        PushOutboxRelayOutcomeKind::AuthRequired
    );
    assert_eq!(
        PushOutboxRelayOutcomeKind::from(RadrootsRelayOutcomeKind::Error),
        PushOutboxRelayOutcomeKind::Error
    );
    assert_eq!(
        PushOutboxRelayOutcomeKind::from(RadrootsRelayOutcomeKind::Timeout),
        PushOutboxRelayOutcomeKind::Timeout
    );
    assert_eq!(
        PushOutboxRelayOutcomeKind::from(RadrootsRelayOutcomeKind::ConnectionFailed),
        PushOutboxRelayOutcomeKind::ConnectionFailed
    );
    assert_eq!(
        PushOutboxRelayOutcomeKind::from(RadrootsRelayOutcomeKind::Unknown),
        PushOutboxRelayOutcomeKind::Unknown
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
        .expect("empty proxy push");

    assert_eq!(receipt.attempted_events, 0);
    assert_eq!(proxy_delivery_policy(0, None), PublishDeliveryPolicy::Any);
    assert_eq!(proxy_delivery_policy(2, None), PublishDeliveryPolicy::All);
    assert_eq!(
        proxy_delivery_policy(3, Some(2)),
        PublishDeliveryPolicy::Quorum { quorum: 2 }
    );
    assert_eq!(
        proxy_outbox_idempotency_key(7, 3, "event-id"),
        "radroots-sdk-outbox-7-3-event-id"
    );

    let proxy_receipt = proxy_transport_error_receipt("a".repeat(64));
    assert_eq!(proxy_receipt.attempted_count, 1);
    assert_eq!(proxy_receipt.retryable_count, 1);
    assert_eq!(proxy_receipt.quorum, 1);
    assert!(!proxy_receipt.quorum_met);
    assert!(proxy_receipt.relays.is_empty());
    assert_eq!(
        proxy_error_message(&RadrootsdError::Http("connection refused".to_owned())),
        "radrootsd proxy publish failed: connection refused"
    );
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
        target_relays: vec!["wss://relay.example.com".to_owned()],
    };

    assert!(matches!(
        push_proxy_claimed_outbox_event(&sync, &adapter, &claimed, None, 60_000, 1_700_000_000_000)
            .await,
        Err(RadrootsSdkError::RelayTransport { message })
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
        push_proxy_claimed_outbox_event(&sync, &adapter, &claimed, None, 60_000, 1_700_000_000_000)
            .await
            .expect("transport error receipt");

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
async fn proxy_completion_updates_outbox_for_success_retryable_and_terminal_receipts() {
    let cases = [
        ("proxy-complete-success", PushOutboxEventState::Published, {
            let mut receipt = relay_publish_receipt("a".repeat(64).as_str());
            receipt.quorum_met = true;
            receipt.quorum = 1;
            receipt.accepted_count = 1;
            receipt
        }),
        (
            "proxy-complete-retryable",
            PushOutboxEventState::PublishRetryable,
            {
                let mut receipt = relay_publish_receipt("b".repeat(64).as_str());
                receipt.retryable_count = 1;
                receipt.quorum = 1;
                receipt
            },
        ),
        (
            "proxy-complete-terminal",
            PushOutboxEventState::FailedTerminal,
            {
                let mut receipt = relay_publish_receipt("c".repeat(64).as_str());
                receipt.terminal_count = 1;
                receipt.quorum = 1;
                receipt
            },
        ),
    ];

    for (d_tag, expected_state, mut publish) in cases {
        let (sdk, claimed) = claimed_proxy_event(d_tag).await;
        publish.event_id = claimed
            .signed_event
            .as_ref()
            .expect("signed event")
            .id
            .clone();
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
        assert_eq!(PushOutboxEventState::from(stored.state), expected_state);
        assert!(stored.claim_token.is_none());
    }
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
            radroots_relay_transport::RadrootsRelayPublishRelayReceipt::attempted(
                "wss://relay.example.com",
                radroots_relay_transport::RadrootsRelayOutcome::accepted(),
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
        relays: Vec::new(),
    }
}
