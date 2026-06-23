use super::{
    PushOutboxEventReceipt, PushOutboxEventState, PushOutboxReceipt, PushOutboxRelayOutcomeKind,
    SdkRelayAuthPolicy, SyncEventStoreStatus, SyncOutboxStatus, push_event_final_state,
    push_event_receipt, push_outbox_claim_token,
};
use crate::RadrootsSdkError;
use futures::future::BoxFuture;
use radroots_event_store::RadrootsEventStoreStatusSummary;
use radroots_events::ids::RadrootsEventId;
use radroots_outbox::{RadrootsOutboxEventState, RadrootsOutboxStatusSummary};
use radroots_relay_transport::{
    RadrootsRelayOutcomeKind, RadrootsRelayPublishAdapter, RadrootsRelayPublishReceipt,
    RadrootsRelayPublishRelayReceipt, RadrootsRelayPublishRequest, RadrootsRelayTransportError,
};
use std::collections::BTreeSet;

struct UnusedPublishAdapter;

impl RadrootsRelayPublishAdapter for UnusedPublishAdapter {
    fn publish<'a>(
        &'a self,
        _request: RadrootsRelayPublishRequest,
    ) -> BoxFuture<'a, Result<Vec<RadrootsRelayPublishRelayReceipt>, RadrootsRelayTransportError>>
    {
        Box::pin(async { Ok(Vec::new()) })
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
        .republish_accepted_relays(true)
        .with_relay_url_policy(crate::SdkRelayUrlPolicy::Localhost)
        .with_auth_policy(SdkRelayAuthPolicy::DetectOnly)
        .with_claim_ttl_ms(7)
        .with_next_attempt_delay_ms(11);
    assert_eq!(request.limit, 2);
    assert!(request.republish_accepted_relays);
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
    let event_store_closed = crate::RadrootsSdk::builder().build().await.expect("sdk");
    event_store_closed._event_store.pool().close().await;
    assert!(matches!(
        event_store_closed
            .sync()
            .status(super::SyncStatusRequest::new())
            .await,
        Err(RadrootsSdkError::EventStore { .. })
    ));

    let outbox_closed = crate::RadrootsSdk::builder().build().await.expect("sdk");
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
    let sdk = crate::RadrootsSdk::builder()
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
