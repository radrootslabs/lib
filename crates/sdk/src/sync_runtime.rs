#[cfg(all(feature = "runtime", feature = "radrootsd-proxy"))]
use crate::adapters::radrootsd::{
    RadrootsdError, RadrootsdProxyPublishAdapter, RadrootsdProxyPublishRequest,
};
#[cfg(feature = "runtime")]
use crate::{RadrootsSdkError, SdkRelayUrlPolicy, SyncClient, runtime::sdk_now_ms};
#[cfg(feature = "runtime")]
use radroots_event_store::RadrootsEventStoreStatusSummary;
#[cfg(feature = "runtime")]
use radroots_events::ids::RadrootsEventId;
#[cfg(all(feature = "runtime", feature = "relay-runtime"))]
use radroots_nostr::prelude::RadrootsNostrClient;
#[cfg(feature = "runtime")]
use radroots_outbox::{
    RadrootsOutboxClaimedEvent, RadrootsOutboxEventState, RadrootsOutboxStatusSummary,
};
#[cfg(all(feature = "runtime", feature = "radrootsd-proxy"))]
use radroots_publish_proxy_protocol::PublishDeliveryPolicy;
#[cfg(all(feature = "runtime", feature = "relay-runtime"))]
use radroots_relay_transport::RadrootsNostrClientPublishAdapter;
#[cfg(feature = "runtime")]
use radroots_relay_transport::{
    RadrootsOutboxPublishPolicy, RadrootsRelayOutcomeKind, RadrootsRelayPublishAdapter,
    RadrootsRelayPublishReceipt, RadrootsRelayPublishRelayReceipt, publish_claimed_outbox_event,
};

#[cfg(feature = "runtime")]
pub const PUSH_OUTBOX_DEFAULT_LIMIT: usize = 20;
#[cfg(feature = "runtime")]
pub const PUSH_OUTBOX_MAX_LIMIT: usize = 100;
#[cfg(feature = "runtime")]
pub const PUSH_OUTBOX_DEFAULT_CLAIM_TTL_MS: i64 = 30_000;
#[cfg(feature = "runtime")]
pub const PUSH_OUTBOX_DEFAULT_NEXT_ATTEMPT_DELAY_MS: i64 = 60_000;

#[cfg(feature = "runtime")]
const CLAIM_OWNER: &str = "radroots_sdk.sync.push_outbox";

#[cfg(feature = "runtime")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[non_exhaustive]
pub struct SyncStatusRequest {}

#[cfg(feature = "runtime")]
impl SyncStatusRequest {
    pub fn new() -> Self {
        Self::default()
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct SyncStatusReceipt {
    pub source: SyncStatusSource,
    pub observed_at_ms: i64,
    pub event_store: SyncEventStoreStatus,
    pub outbox: SyncOutboxStatus,
    pub relay_targets: SyncRelayTargetSummary,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SyncStatusSource {
    SdkCanonicalStores,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct SyncEventStoreStatus {
    pub total_events: i64,
    pub projection_eligible_events: i64,
    pub relay_observations: i64,
    pub last_event_seq: Option<i64>,
    pub last_event_updated_at_ms: Option<i64>,
}

#[cfg(feature = "runtime")]
impl From<RadrootsEventStoreStatusSummary> for SyncEventStoreStatus {
    fn from(summary: RadrootsEventStoreStatusSummary) -> Self {
        Self {
            total_events: summary.total_events,
            projection_eligible_events: summary.projection_eligible_events,
            relay_observations: summary.relay_observations,
            last_event_seq: summary.last_event_seq,
            last_event_updated_at_ms: summary.last_event_updated_at_ms,
        }
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct SyncOutboxStatus {
    pub total_events: i64,
    pub pending_events: i64,
    pub retryable_events: i64,
    pub terminal_events: i64,
    pub failed_terminal_events: i64,
    pub ready_signed_events: i64,
    pub publishing_events: i64,
    pub last_attempt_at_ms: Option<i64>,
    pub last_error: Option<String>,
}

#[cfg(feature = "runtime")]
impl From<RadrootsOutboxStatusSummary> for SyncOutboxStatus {
    fn from(summary: RadrootsOutboxStatusSummary) -> Self {
        Self {
            total_events: summary.total_events,
            pending_events: summary.pending_events,
            retryable_events: summary.retryable_events,
            terminal_events: summary.terminal_events,
            failed_terminal_events: summary.failed_terminal_events,
            ready_signed_events: summary.ready_signed_events,
            publishing_events: summary.publishing_events,
            last_attempt_at_ms: summary.last_attempt_at_ms,
            last_error: summary.last_error,
        }
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct SyncRelayTargetSummary {
    pub configured_count: usize,
    pub configured_relays: Vec<String>,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SdkRelayAuthPolicy {
    DetectOnly,
}

#[cfg(feature = "runtime")]
impl Default for SdkRelayAuthPolicy {
    fn default() -> Self {
        Self::DetectOnly
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[non_exhaustive]
pub struct PushOutboxRequest {
    pub limit: usize,
    pub republish_accepted_relays: bool,
    pub relay_url_policy: SdkRelayUrlPolicy,
    pub auth_policy: SdkRelayAuthPolicy,
    pub claim_ttl_ms: i64,
    pub next_attempt_delay_ms: i64,
}

#[cfg(feature = "runtime")]
impl Default for PushOutboxRequest {
    fn default() -> Self {
        Self {
            limit: PUSH_OUTBOX_DEFAULT_LIMIT,
            republish_accepted_relays: false,
            relay_url_policy: SdkRelayUrlPolicy::Public,
            auth_policy: SdkRelayAuthPolicy::DetectOnly,
            claim_ttl_ms: PUSH_OUTBOX_DEFAULT_CLAIM_TTL_MS,
            next_attempt_delay_ms: PUSH_OUTBOX_DEFAULT_NEXT_ATTEMPT_DELAY_MS,
        }
    }
}

#[cfg(feature = "runtime")]
impl PushOutboxRequest {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }

    pub fn republish_accepted_relays(mut self, enabled: bool) -> Self {
        self.republish_accepted_relays = enabled;
        self
    }

    pub fn with_relay_url_policy(mut self, policy: SdkRelayUrlPolicy) -> Self {
        self.relay_url_policy = policy;
        self
    }

    pub fn with_auth_policy(mut self, policy: SdkRelayAuthPolicy) -> Self {
        self.auth_policy = policy;
        self
    }

    pub fn with_claim_ttl_ms(mut self, claim_ttl_ms: i64) -> Self {
        self.claim_ttl_ms = claim_ttl_ms;
        self
    }

    pub fn with_next_attempt_delay_ms(mut self, next_attempt_delay_ms: i64) -> Self {
        self.next_attempt_delay_ms = next_attempt_delay_ms;
        self
    }

    fn validate(&self) -> Result<(), RadrootsSdkError> {
        if self.limit == 0 || self.limit > PUSH_OUTBOX_MAX_LIMIT {
            return Err(RadrootsSdkError::InvalidRequest {
                message: format!("push_outbox limit must be between 1 and {PUSH_OUTBOX_MAX_LIMIT}"),
            });
        }
        if self.claim_ttl_ms <= 0 {
            return Err(RadrootsSdkError::InvalidRequest {
                message: "push_outbox claim TTL must be positive".to_owned(),
            });
        }
        if self.next_attempt_delay_ms <= 0 {
            return Err(RadrootsSdkError::InvalidRequest {
                message: "push_outbox next attempt delay must be positive".to_owned(),
            });
        }
        Ok(())
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct PushOutboxReceipt {
    pub attempted_events: usize,
    pub published_events: usize,
    pub retryable_events: usize,
    pub terminal_events: usize,
    pub events: Vec<PushOutboxEventReceipt>,
}

#[cfg(feature = "runtime")]
impl PushOutboxReceipt {
    fn push_event(&mut self, event: PushOutboxEventReceipt) {
        self.attempted_events += 1;
        match event.final_state {
            PushOutboxEventState::Published => self.published_events += 1,
            PushOutboxEventState::PublishRetryable => self.retryable_events += 1,
            PushOutboxEventState::FailedTerminal => self.terminal_events += 1,
            _ => {}
        }
        self.events.push(event);
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct PushOutboxEventReceipt {
    pub event_id: RadrootsEventId,
    pub outbox_event_id: i64,
    pub final_state: PushOutboxEventState,
    pub attempted_count: usize,
    pub accepted_count: usize,
    pub retryable_count: usize,
    pub terminal_count: usize,
    pub quorum: usize,
    pub quorum_met: bool,
    pub relays: Vec<PushOutboxRelayReceipt>,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct PushOutboxRelayReceipt {
    pub relay_url: String,
    pub outcome_kind: PushOutboxRelayOutcomeKind,
    pub attempted: bool,
    pub message: Option<String>,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PushOutboxEventState {
    DraftQueued,
    Signing,
    Signed,
    Publishing,
    Published,
    SignRetryable,
    PublishRetryable,
    FailedTerminal,
    Cancelled,
}

#[cfg(feature = "runtime")]
impl From<RadrootsOutboxEventState> for PushOutboxEventState {
    fn from(state: RadrootsOutboxEventState) -> Self {
        match state {
            RadrootsOutboxEventState::DraftQueued => Self::DraftQueued,
            RadrootsOutboxEventState::Signing => Self::Signing,
            RadrootsOutboxEventState::Signed => Self::Signed,
            RadrootsOutboxEventState::Publishing => Self::Publishing,
            RadrootsOutboxEventState::Published => Self::Published,
            RadrootsOutboxEventState::SignRetryable => Self::SignRetryable,
            RadrootsOutboxEventState::PublishRetryable => Self::PublishRetryable,
            RadrootsOutboxEventState::FailedTerminal => Self::FailedTerminal,
            RadrootsOutboxEventState::Cancelled => Self::Cancelled,
        }
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PushOutboxRelayOutcomeKind {
    Accepted,
    DuplicateAccepted,
    Blocked,
    RateLimited,
    Invalid,
    PowRequired,
    Restricted,
    AuthRequired,
    Muted,
    Unsupported,
    PaymentRequired,
    Error,
    Timeout,
    ConnectionFailed,
    RelayUrlRejected,
    SkippedAlreadyAccepted,
    Unknown,
}

#[cfg(feature = "runtime")]
impl From<RadrootsRelayOutcomeKind> for PushOutboxRelayOutcomeKind {
    fn from(kind: RadrootsRelayOutcomeKind) -> Self {
        match kind {
            RadrootsRelayOutcomeKind::Accepted => Self::Accepted,
            RadrootsRelayOutcomeKind::DuplicateAccepted => Self::DuplicateAccepted,
            RadrootsRelayOutcomeKind::Blocked => Self::Blocked,
            RadrootsRelayOutcomeKind::RateLimited => Self::RateLimited,
            RadrootsRelayOutcomeKind::Invalid => Self::Invalid,
            RadrootsRelayOutcomeKind::PowRequired => Self::PowRequired,
            RadrootsRelayOutcomeKind::Restricted => Self::Restricted,
            RadrootsRelayOutcomeKind::AuthRequired => Self::AuthRequired,
            RadrootsRelayOutcomeKind::Muted => Self::Muted,
            RadrootsRelayOutcomeKind::Unsupported => Self::Unsupported,
            RadrootsRelayOutcomeKind::PaymentRequired => Self::PaymentRequired,
            RadrootsRelayOutcomeKind::Error => Self::Error,
            RadrootsRelayOutcomeKind::Timeout => Self::Timeout,
            RadrootsRelayOutcomeKind::ConnectionFailed => Self::ConnectionFailed,
            RadrootsRelayOutcomeKind::RelayUrlRejected => Self::RelayUrlRejected,
            RadrootsRelayOutcomeKind::SkippedAlreadyAccepted => Self::SkippedAlreadyAccepted,
            RadrootsRelayOutcomeKind::Unknown => Self::Unknown,
        }
    }
}

#[cfg(feature = "runtime")]
impl<'sdk> SyncClient<'sdk> {
    pub async fn status(
        &self,
        _request: SyncStatusRequest,
    ) -> Result<SyncStatusReceipt, RadrootsSdkError> {
        let observed_at_ms = sdk_now_ms(self.sdk)?;
        let event_store = self.sdk._event_store.status_summary().await?;
        let outbox = self.sdk._outbox.status_summary(observed_at_ms).await?;
        Ok(SyncStatusReceipt {
            source: SyncStatusSource::SdkCanonicalStores,
            observed_at_ms,
            event_store: event_store.into(),
            outbox: outbox.into(),
            relay_targets: SyncRelayTargetSummary {
                configured_count: self.sdk.relay_urls().len(),
                configured_relays: self.sdk.relay_urls().to_vec(),
            },
        })
    }

    pub async fn push_outbox(
        &self,
        request: PushOutboxRequest,
    ) -> Result<PushOutboxReceipt, RadrootsSdkError> {
        match self.sdk.publish_transport() {
            crate::runtime::SdkPublishTransport::DirectNostrRelay => {
                #[cfg(feature = "relay-runtime")]
                {
                    let adapter = RadrootsNostrClientPublishAdapter::new(
                        RadrootsNostrClient::new_signerless(),
                    );
                    self.push_outbox_with_adapter(&adapter, request).await
                }

                #[cfg(not(feature = "relay-runtime"))]
                {
                    let _ = request;
                    Err(RadrootsSdkError::ProductSyncUnsupported {
                        operation: "sync.push_outbox",
                        required_feature: "relay-runtime",
                    })
                }
            }
            #[cfg(feature = "radrootsd-proxy")]
            crate::runtime::SdkPublishTransport::RadrootsdProxy(config) => {
                let adapter = RadrootsdProxyPublishAdapter::new(config.clone());
                self.push_outbox_with_proxy_adapter(&adapter, request).await
            }
        }
    }

    pub async fn push_outbox_with_adapter<A>(
        &self,
        adapter: &A,
        request: PushOutboxRequest,
    ) -> Result<PushOutboxReceipt, RadrootsSdkError>
    where
        A: RadrootsRelayPublishAdapter,
    {
        request.validate()?;
        let mut receipt = PushOutboxReceipt::default();
        for _ in 0..request.limit {
            let claim_now_ms = sdk_now_ms(self.sdk)?;
            let claim_token = push_outbox_claim_token();
            let Some(claimed) = self
                .sdk
                ._outbox
                .claim_next_ready_signed_event(
                    CLAIM_OWNER,
                    claim_token.as_str(),
                    claim_now_ms.saturating_add(request.claim_ttl_ms),
                    claim_now_ms,
                )
                .await?
            else {
                break;
            };
            let publish_now_ms = claim_now_ms;
            let policy = RadrootsOutboxPublishPolicy::new(
                publish_now_ms.saturating_add(request.next_attempt_delay_ms),
            )
            .republish_accepted_relays(request.republish_accepted_relays)
            .relay_url_policy(request.relay_url_policy.relay_transport_policy());
            let publish = publish_claimed_outbox_event(
                &self.sdk._outbox,
                &self.sdk._event_store,
                adapter,
                &claimed,
                policy,
                publish_now_ms,
            )
            .await?;
            receipt.push_event(push_event_receipt(
                claimed.outbox_event_id,
                push_event_final_state(&publish.publish),
                publish.publish,
            ));
        }
        Ok(receipt)
    }

    #[cfg(feature = "radrootsd-proxy")]
    pub async fn push_outbox_with_proxy_adapter(
        &self,
        adapter: &RadrootsdProxyPublishAdapter,
        request: PushOutboxRequest,
    ) -> Result<PushOutboxReceipt, RadrootsSdkError> {
        request.validate()?;
        let mut receipt = PushOutboxReceipt::default();
        for _ in 0..request.limit {
            let claim_now_ms = sdk_now_ms(self.sdk)?;
            let claim_token = push_outbox_claim_token();
            let Some(claimed) = self
                .sdk
                ._outbox
                .claim_next_ready_signed_event(
                    CLAIM_OWNER,
                    claim_token.as_str(),
                    claim_now_ms.saturating_add(request.claim_ttl_ms),
                    claim_now_ms,
                )
                .await?
            else {
                break;
            };
            let publish_now_ms = claim_now_ms;
            let publish = push_proxy_claimed_outbox_event(
                self,
                adapter,
                &claimed,
                request.next_attempt_delay_ms,
                publish_now_ms,
            )
            .await?;
            receipt.push_event(push_event_receipt(
                claimed.outbox_event_id,
                push_event_final_state(&publish),
                publish,
            ));
        }
        Ok(receipt)
    }
}

#[cfg(all(feature = "runtime", feature = "radrootsd-proxy"))]
async fn push_proxy_claimed_outbox_event(
    sync: &SyncClient<'_>,
    adapter: &RadrootsdProxyPublishAdapter,
    claimed: &RadrootsOutboxClaimedEvent,
    next_attempt_delay_ms: i64,
    now_ms: i64,
) -> Result<RadrootsRelayPublishReceipt, RadrootsSdkError> {
    let signed_event = claimed.signed_event.clone().ok_or(
        radroots_relay_transport::RadrootsRelayTransportError::MissingSignedOutboxEvent(
            claimed.outbox_event_id,
        ),
    )?;
    sync.sdk
        ._outbox
        .ingest_signed_event_local(
            &sync.sdk._event_store,
            claimed.outbox_event_id,
            claimed.claim_token.as_str(),
            now_ms,
        )
        .await?;
    let request = RadrootsdProxyPublishRequest {
        signed_event: signed_event.clone(),
        relays: claimed.target_relays.clone(),
        delivery_policy: proxy_delivery_policy(claimed.target_relays.len()),
        idempotency_key: Some(proxy_outbox_idempotency_key(
            claimed.outbox_event_id,
            claimed.attempt_count,
            signed_event.id.as_str(),
        )),
        timeout_ms: adapter.config().request_timeout_ms,
    };
    let publish = match adapter.publish_signed_event(request).await {
        Ok(publish) => publish,
        Err(error) => {
            let message = proxy_error_message(&error);
            sync.sdk
                ._outbox
                .mark_publish_retryable(
                    claimed.outbox_event_id,
                    claimed.claim_token.as_str(),
                    message.as_str(),
                    now_ms.saturating_add(next_attempt_delay_ms),
                    now_ms,
                )
                .await?;
            return Ok(proxy_transport_error_receipt(signed_event.id));
        }
    };
    complete_proxy_publish_attempt(sync, claimed, &publish, next_attempt_delay_ms, now_ms).await?;
    Ok(publish)
}

#[cfg(all(feature = "runtime", feature = "radrootsd-proxy"))]
fn proxy_delivery_policy(target_count: usize) -> PublishDeliveryPolicy {
    if target_count == 0 {
        PublishDeliveryPolicy::Any
    } else {
        PublishDeliveryPolicy::All
    }
}

#[cfg(all(feature = "runtime", feature = "radrootsd-proxy"))]
fn proxy_outbox_idempotency_key(
    outbox_event_id: i64,
    attempt_count: i64,
    event_id: &str,
) -> String {
    format!("radroots-sdk-outbox-{outbox_event_id}-{attempt_count}-{event_id}")
}

#[cfg(all(feature = "runtime", feature = "radrootsd-proxy"))]
async fn complete_proxy_publish_attempt(
    sync: &SyncClient<'_>,
    claimed: &RadrootsOutboxClaimedEvent,
    publish: &RadrootsRelayPublishReceipt,
    next_attempt_delay_ms: i64,
    now_ms: i64,
) -> Result<(), RadrootsSdkError> {
    if publish.quorum_met {
        sync.sdk
            ._outbox
            .set_publish_quorum(
                claimed.outbox_event_id,
                claimed.claim_token.as_str(),
                0,
                now_ms,
            )
            .await?;
        sync.sdk
            ._outbox
            .complete_publish_attempt(
                claimed.outbox_event_id,
                claimed.claim_token.as_str(),
                "radrootsd proxy publish incomplete",
                "radrootsd proxy publish terminal",
                now_ms.saturating_add(next_attempt_delay_ms),
                now_ms,
            )
            .await?;
    } else if publish.retryable_count > 0 {
        sync.sdk
            ._outbox
            .mark_publish_retryable(
                claimed.outbox_event_id,
                claimed.claim_token.as_str(),
                "radrootsd proxy publish incomplete",
                now_ms.saturating_add(next_attempt_delay_ms),
                now_ms,
            )
            .await?;
    } else {
        sync.sdk
            ._outbox
            .mark_publish_failed_terminal(
                claimed.outbox_event_id,
                claimed.claim_token.as_str(),
                "radrootsd proxy publish terminal",
                now_ms,
            )
            .await?;
    }
    Ok(())
}

#[cfg(all(feature = "runtime", feature = "radrootsd-proxy"))]
fn proxy_transport_error_receipt(event_id: String) -> RadrootsRelayPublishReceipt {
    RadrootsRelayPublishReceipt {
        event_id,
        attempted_count: 1,
        accepted_count: 0,
        retryable_count: 1,
        terminal_count: 0,
        quorum: 1,
        quorum_met: false,
        relays: Vec::new(),
    }
}

#[cfg(all(feature = "runtime", feature = "radrootsd-proxy"))]
fn proxy_error_message(error: &RadrootsdError) -> String {
    format!("radrootsd proxy publish failed: {error}")
}

#[cfg(feature = "runtime")]
fn push_outbox_claim_token() -> String {
    format!("radroots-sdk-sync-{}", uuid::Uuid::now_v7())
}

#[cfg(feature = "runtime")]
fn push_event_final_state(publish: &RadrootsRelayPublishReceipt) -> PushOutboxEventState {
    if publish.quorum_met {
        PushOutboxEventState::Published
    } else if publish.retryable_count > 0 {
        PushOutboxEventState::PublishRetryable
    } else {
        PushOutboxEventState::FailedTerminal
    }
}

#[cfg(feature = "runtime")]
fn push_event_receipt(
    outbox_event_id: i64,
    final_state: PushOutboxEventState,
    publish: RadrootsRelayPublishReceipt,
) -> PushOutboxEventReceipt {
    let event_id = RadrootsEventId::parse(publish.event_id.as_str())
        .expect("relay transport publish receipt uses signed event id");
    PushOutboxEventReceipt {
        event_id,
        outbox_event_id,
        final_state,
        attempted_count: publish.attempted_count,
        accepted_count: publish.accepted_count,
        retryable_count: publish.retryable_count,
        terminal_count: publish.terminal_count,
        quorum: publish.quorum,
        quorum_met: publish.quorum_met,
        relays: publish.relays.into_iter().map(push_relay_receipt).collect(),
    }
}

#[cfg(feature = "runtime")]
fn push_relay_receipt(relay: RadrootsRelayPublishRelayReceipt) -> PushOutboxRelayReceipt {
    PushOutboxRelayReceipt {
        relay_url: relay.relay_url,
        outcome_kind: relay.outcome.kind.into(),
        attempted: relay.attempted,
        message: relay.outcome.message,
    }
}

#[cfg(all(test, feature = "runtime"))]
#[path = "../tests/unit/sync_runtime_tests.rs"]
mod tests;
