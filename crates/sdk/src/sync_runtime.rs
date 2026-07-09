#[cfg(all(feature = "runtime", feature = "radrootsd-proxy"))]
use crate::adapters::radrootsd::{
    RadrootsdAuth, RadrootsdError, RadrootsdProxyConfig, RadrootsdProxyPublishAdapter,
    RadrootsdProxyPublishRequest,
};
#[cfg(feature = "runtime")]
use crate::{
    NostrRelayUrlPolicy, ProxyAuth, ProxyProfile, RadrootsSdkError, SyncClient,
    runtime::{RadrootsClient, sdk_now_ms},
    transport::TransportProfile,
};
#[cfg(feature = "runtime")]
use radroots_event_store::{RADROOTS_EVENT_STORE_QUERY_LIMIT_MAX, RadrootsEventStoreStatusSummary};
#[cfg(feature = "runtime")]
use radroots_events::ids::RadrootsEventId;
#[cfg(all(feature = "runtime", feature = "transport-nostr-runtime"))]
use radroots_nostr::prelude::RadrootsNostrClient;
#[cfg(all(feature = "runtime", feature = "radrootsd-proxy"))]
use radroots_outbox::{
    RadrootsOutboxClaimedEvent, RadrootsOutboxDeliveryTargetRecord,
    RadrootsOutboxDeliveryTargetStatus,
};
#[cfg(feature = "runtime")]
use radroots_outbox::{RadrootsOutboxEventState, RadrootsOutboxStatusSummary};
#[cfg(feature = "runtime")]
use radroots_trade::projection::{
    RADROOTS_PRODUCT_PROJECTION_ID, RADROOTS_PRODUCT_PROJECTION_VERSION,
    RadrootsProjectionRefreshReceipt, RadrootsProjectionRefreshRequest,
    refresh_product_projections,
};
#[cfg(all(feature = "runtime", feature = "radrootsd-proxy"))]
use radroots_transport::RadrootsTransportSatisfactionPolicy;
#[cfg(feature = "runtime")]
use radroots_transport::{
    RadrootsTransportImplementationState, RadrootsTransportKind, RadrootsTransportReadinessState,
    RadrootsTransportStatus, RadrootsTransportTarget,
};
#[cfg(all(feature = "runtime", feature = "transport-nostr-runtime"))]
use radroots_transport_nostr::RadrootsNostrClientPublishAdapter;
#[cfg(feature = "runtime")]
use radroots_transport_nostr::{
    RadrootsOutboxPublishPolicy, RadrootsRelayOutcomeKind, RadrootsRelayPublishAdapter,
    RadrootsRelayPublishReceipt, RadrootsRelayPublishRelayReceipt, publish_claimed_outbox_event,
};
#[cfg(all(feature = "runtime", feature = "radrootsd-proxy"))]
use radroots_transport_publish_protocol::{
    NostrPublishTargetSourcePolicy, TransportPublishDeliveryPolicy, TransportPublishJobStatus,
    TransportPublishJobView, TransportPublishOutcomeKind, TransportPublishTarget,
    TransportPublishTargetOutcome, TransportPublishTargetPolicy,
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
pub const SYNC_PROJECTION_REFRESH_DEFAULT_LIMIT: u32 = RADROOTS_EVENT_STORE_QUERY_LIMIT_MAX;
#[cfg(feature = "runtime")]
pub const SYNC_PROJECTION_REFRESH_MAX_LIMIT: u32 = RADROOTS_EVENT_STORE_QUERY_LIMIT_MAX;

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
    pub transport_profile: SyncTransportProfileSummary,
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
    pub transport_observations: i64,
    pub last_event_seq: Option<i64>,
    pub last_event_updated_at_ms: Option<i64>,
}

#[cfg(feature = "runtime")]
impl From<RadrootsEventStoreStatusSummary> for SyncEventStoreStatus {
    fn from(summary: RadrootsEventStoreStatusSummary) -> Self {
        Self {
            total_events: summary.total_events,
            projection_eligible_events: summary.projection_eligible_events,
            transport_observations: summary.transport_observations,
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
    pub preview_unavailable_events: i64,
    pub deferred_until_implemented_events: i64,
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
            preview_unavailable_events: summary.preview_unavailable_events,
            deferred_until_implemented_events: summary.deferred_until_implemented_events,
            ready_signed_events: summary.ready_signed_events,
            publishing_events: summary.publishing_events,
            last_attempt_at_ms: summary.last_attempt_at_ms,
            last_error: summary.last_error,
        }
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct SyncTransportProfileSummary {
    pub transport_profile_id: String,
    pub configured_transport_target_count: usize,
    pub configured_transport_targets: Vec<SyncTransportTargetSummary>,
    pub transport_statuses: Vec<SyncTransportStatusSummary>,
}

#[cfg(feature = "runtime")]
impl SyncTransportProfileSummary {
    fn from_transport_profile(profile: &TransportProfile) -> Result<Self, RadrootsSdkError> {
        let configured_transport_targets = profile
            .configured_transport_targets()?
            .iter()
            .map(SyncTransportTargetSummary::from_transport_target)
            .collect::<Vec<_>>();
        Ok(Self {
            transport_profile_id: profile.transport_profile_id().to_owned(),
            configured_transport_target_count: configured_transport_targets.len(),
            configured_transport_targets,
            transport_statuses: profile
                .transport_statuses()
                .into_iter()
                .map(SyncTransportStatusSummary::from_transport_status)
                .collect(),
        })
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct SyncTransportTargetSummary {
    pub transport_kind: String,
    pub endpoint_uri: String,
    pub endpoint_fingerprint: String,
}

#[cfg(feature = "runtime")]
impl SyncTransportTargetSummary {
    fn from_transport_target(target: &RadrootsTransportTarget) -> Self {
        Self {
            transport_kind: target.kind.canonical_label(),
            endpoint_uri: target.uri.as_str().to_owned(),
            endpoint_fingerprint: target.fingerprint.as_str().to_owned(),
        }
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct SyncTransportStatusSummary {
    pub transport_kind: String,
    pub profile_id: Option<String>,
    pub endpoint_uri: Option<String>,
    pub implementation_state: String,
    pub readiness: String,
    pub publish_usable: bool,
    pub fetch_usable: bool,
    pub redacted_message: Option<String>,
}

#[cfg(feature = "runtime")]
impl SyncTransportStatusSummary {
    fn from_transport_status(status: RadrootsTransportStatus) -> Self {
        Self {
            transport_kind: status.kind.canonical_label(),
            profile_id: status.profile_id,
            endpoint_uri: status.endpoint_uri,
            implementation_state: transport_implementation_state_label(status.implementation_state)
                .to_owned(),
            readiness: transport_readiness_state_label(status.readiness).to_owned(),
            publish_usable: status.publish_usable,
            fetch_usable: status.fetch_usable,
            redacted_message: status.redacted_message,
        }
    }
}

#[cfg(feature = "runtime")]
fn transport_implementation_state_label(
    state: RadrootsTransportImplementationState,
) -> &'static str {
    match state {
        RadrootsTransportImplementationState::Available => "available",
        RadrootsTransportImplementationState::Disabled => "disabled",
        RadrootsTransportImplementationState::Misconfigured => "misconfigured",
        RadrootsTransportImplementationState::PreviewUnavailable => "preview_unavailable",
    }
}

#[cfg(feature = "runtime")]
fn transport_readiness_state_label(state: RadrootsTransportReadinessState) -> &'static str {
    match state {
        RadrootsTransportReadinessState::Ready => "ready",
        RadrootsTransportReadinessState::Disabled => "disabled",
        RadrootsTransportReadinessState::Misconfigured => "misconfigured",
        RadrootsTransportReadinessState::PreviewUnavailable => "preview_unavailable",
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[non_exhaustive]
pub struct SyncProjectionRefreshRequest {
    pub limit: u32,
}

#[cfg(feature = "runtime")]
impl Default for SyncProjectionRefreshRequest {
    fn default() -> Self {
        Self {
            limit: SYNC_PROJECTION_REFRESH_DEFAULT_LIMIT,
        }
    }
}

#[cfg(feature = "runtime")]
impl SyncProjectionRefreshRequest {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_limit(mut self, limit: u32) -> Self {
        self.limit = limit;
        self
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct SyncProjectionRefreshReceipt {
    pub projection_id: &'static str,
    pub projection_version: u32,
    pub refreshed_at_ms: i64,
    pub scanned_events: usize,
    pub listing_upserts: usize,
    pub trade_upserts: usize,
    pub validation_receipts: usize,
    pub transport_observations: i64,
    pub last_event_seq: Option<i64>,
}

#[cfg(feature = "runtime")]
impl SyncProjectionRefreshReceipt {
    fn from_trade(receipt: RadrootsProjectionRefreshReceipt, refreshed_at_ms: i64) -> Self {
        Self {
            projection_id: RADROOTS_PRODUCT_PROJECTION_ID,
            projection_version: RADROOTS_PRODUCT_PROJECTION_VERSION,
            refreshed_at_ms,
            scanned_events: receipt.scanned_events,
            listing_upserts: receipt.listing_upserts,
            trade_upserts: receipt.trade_upserts,
            validation_receipts: receipt.validation_receipts,
            transport_observations: receipt.transport_observations,
            last_event_seq: receipt.last_event_seq,
        }
    }
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
    pub outbox_event_id: Option<i64>,
    pub republish_accepted_targets: bool,
    pub nostr_relay_url_policy: NostrRelayUrlPolicy,
    pub auth_policy: SdkRelayAuthPolicy,
    pub claim_ttl_ms: i64,
    pub next_attempt_delay_ms: i64,
}

#[cfg(feature = "runtime")]
impl Default for PushOutboxRequest {
    fn default() -> Self {
        Self {
            limit: PUSH_OUTBOX_DEFAULT_LIMIT,
            outbox_event_id: None,
            republish_accepted_targets: false,
            nostr_relay_url_policy: NostrRelayUrlPolicy::Public,
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

    pub fn with_outbox_event_id(mut self, outbox_event_id: i64) -> Self {
        self.outbox_event_id = Some(outbox_event_id);
        self.limit = 1;
        self
    }

    pub fn republish_accepted_targets(mut self, enabled: bool) -> Self {
        self.republish_accepted_targets = enabled;
        self
    }

    pub fn with_nostr_relay_url_policy(mut self, policy: NostrRelayUrlPolicy) -> Self {
        self.nostr_relay_url_policy = policy;
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
        if self.limit == 0 {
            return Err(RadrootsSdkError::InvalidRequest {
                message: format!("push_outbox limit must be between 1 and {PUSH_OUTBOX_MAX_LIMIT}"),
            });
        }
        if self.limit > PUSH_OUTBOX_MAX_LIMIT {
            return Err(RadrootsSdkError::InvalidRequest {
                message: format!("push_outbox limit must be between 1 and {PUSH_OUTBOX_MAX_LIMIT}"),
            });
        }
        if let Some(outbox_event_id) = self.outbox_event_id
            && outbox_event_id <= 0
        {
            return Err(RadrootsSdkError::InvalidRequest {
                message: "push_outbox outbox event id must be positive".to_owned(),
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
    pub targets: Vec<PushOutboxTargetReceipt>,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct PushOutboxTargetReceipt {
    pub transport_kind: String,
    pub endpoint_uri: String,
    pub outcome_kind: PushOutboxTargetOutcomeKind,
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
    DeferredUntilImplemented,
    PreviewUnavailable,
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
            RadrootsOutboxEventState::DeferredUntilImplemented => Self::DeferredUntilImplemented,
            RadrootsOutboxEventState::PreviewUnavailable => Self::PreviewUnavailable,
            RadrootsOutboxEventState::FailedTerminal => Self::FailedTerminal,
            RadrootsOutboxEventState::Cancelled => Self::Cancelled,
        }
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PushOutboxTargetOutcomeKind {
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
    TargetUriRejected,
    SkippedAlreadyAccepted,
    DeferredUntilImplemented,
    PreviewUnavailable,
    Unknown,
}

#[cfg(feature = "runtime")]
impl From<RadrootsRelayOutcomeKind> for PushOutboxTargetOutcomeKind {
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
            RadrootsRelayOutcomeKind::RelayUrlRejected => Self::TargetUriRejected,
            RadrootsRelayOutcomeKind::SkippedAlreadyAccepted => Self::SkippedAlreadyAccepted,
            RadrootsRelayOutcomeKind::Unknown => Self::Unknown,
        }
    }
}

#[cfg(feature = "runtime")]
impl<'sdk> SyncClient<'sdk> {
    pub async fn refresh_projections(
        &self,
        request: SyncProjectionRefreshRequest,
    ) -> Result<SyncProjectionRefreshReceipt, RadrootsSdkError> {
        refresh_product_projections_for_sdk(self.sdk, request).await
    }

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
            transport_profile: SyncTransportProfileSummary::from_transport_profile(
                self.sdk.transport_profile(),
            )?,
        })
    }

    pub async fn push_outbox(
        &self,
        request: PushOutboxRequest,
    ) -> Result<PushOutboxReceipt, RadrootsSdkError> {
        match self.sdk.transport_profile() {
            TransportProfile::Nostr { .. } | TransportProfile::Hybrid { .. } => {
                #[cfg(feature = "transport-nostr-runtime")]
                {
                    let adapter = RadrootsNostrClientPublishAdapter::new(
                        RadrootsNostrClient::new_signerless(),
                    );
                    self.push_outbox_with_adapter(&adapter, request).await
                }

                #[cfg(not(feature = "transport-nostr-runtime"))]
                {
                    let _ = request;
                    Err(RadrootsSdkError::ProductSyncUnsupported {
                        operation: "sync.push_outbox",
                        required_feature: "transport-nostr-runtime",
                    })
                }
            }
            #[cfg(feature = "radrootsd-proxy")]
            TransportProfile::Proxy { profile } => {
                let adapter =
                    RadrootsdProxyPublishAdapter::new(radrootsd_proxy_config_from_profile(profile));
                self.push_outbox_with_proxy_adapter(&adapter, request).await
            }
            TransportProfile::LocalOnly => {
                if self.push_outbox_has_no_ready_signed_work(&request).await? {
                    return Ok(PushOutboxReceipt::default());
                }
                Err(RadrootsSdkError::ProductSyncUnsupported {
                    operation: "sync.push_outbox",
                    required_feature: "delivery-capable transport profile",
                })
            }
            TransportProfile::ReticulumPreview { profile } => {
                if self
                    .push_outbox_has_no_reticulum_preview_work(&request)
                    .await?
                {
                    return Ok(PushOutboxReceipt::default());
                }
                Err(RadrootsSdkError::reticulum_preview_transport_unavailable(
                    "sync.push_outbox",
                    profile.endpoint_uri(),
                    profile.behavior(),
                ))
            }
        }
    }

    async fn push_outbox_has_no_ready_signed_work(
        &self,
        request: &PushOutboxRequest,
    ) -> Result<bool, RadrootsSdkError> {
        request.validate()?;
        let now_ms = sdk_now_ms(self.sdk)?;
        let summary = self.sdk._outbox.status_summary(now_ms).await?;
        Ok(summary.ready_signed_events == 0)
    }

    async fn push_outbox_has_no_reticulum_preview_work(
        &self,
        request: &PushOutboxRequest,
    ) -> Result<bool, RadrootsSdkError> {
        request.validate()?;
        if let Some(outbox_event_id) = request.outbox_event_id {
            let Some(event) = self.sdk._outbox.get_event(outbox_event_id).await? else {
                return Ok(true);
            };
            if !matches!(
                event.state,
                RadrootsOutboxEventState::Signed | RadrootsOutboxEventState::PublishRetryable
            ) || event.signed_event.is_none()
            {
                return Ok(true);
            }
            let targets = self.sdk._outbox.delivery_targets(outbox_event_id).await?;
            return Ok(!targets.iter().any(|target| {
                target.transport_kind == RadrootsTransportKind::Reticulum
                    && (target.status.is_deferred_preview() || target.status.is_ready_for_attempt())
            }));
        }
        let now_ms = sdk_now_ms(self.sdk)?;
        let summary = self.sdk._outbox.status_summary(now_ms).await?;
        Ok(summary.ready_signed_events == 0
            && summary.pending_events == 0
            && summary.retryable_events == 0
            && summary.preview_unavailable_events == 0
            && summary.deferred_until_implemented_events == 0)
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
        let recovery_now_ms = sdk_now_ms(self.sdk)?;
        recover_expired_outbox_claims_for_push(self.sdk, recovery_now_ms).await?;
        let mut receipt = PushOutboxReceipt::default();
        for index in 0..request.limit {
            let claim_now_ms = if index == 0 {
                recovery_now_ms
            } else {
                sdk_now_ms(self.sdk)?
            };
            let claim_token = push_outbox_claim_token();
            let Some(claimed) = claim_ready_signed_event_for_push(
                self.sdk,
                &request,
                claim_token.as_str(),
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
            .republish_accepted_relays(request.republish_accepted_targets)
            .relay_url_policy(request.nostr_relay_url_policy.nostr_transport_policy());
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
            )?);
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
        let recovery_now_ms = sdk_now_ms(self.sdk)?;
        recover_expired_outbox_claims_for_push(self.sdk, recovery_now_ms).await?;
        let mut receipt = PushOutboxReceipt::default();
        for index in 0..request.limit {
            let claim_now_ms = if index == 0 {
                recovery_now_ms
            } else {
                sdk_now_ms(self.sdk)?
            };
            let claim_token = push_outbox_claim_token();
            let Some(claimed) = claim_ready_signed_event_for_push(
                self.sdk,
                &request,
                claim_token.as_str(),
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
            receipt.push_event(publish);
        }
        Ok(receipt)
    }
}

#[cfg(all(feature = "runtime", feature = "radrootsd-proxy"))]
fn radrootsd_proxy_config_from_profile(profile: &ProxyProfile) -> RadrootsdProxyConfig {
    let config = RadrootsdProxyConfig::new(profile.endpoint_url().to_owned());
    match profile.auth() {
        ProxyAuth::None => config,
        ProxyAuth::BearerToken(token) => {
            config.with_auth(RadrootsdAuth::BearerToken(token.to_owned()))
        }
    }
}

#[cfg(feature = "runtime")]
async fn recover_expired_outbox_claims_for_push(
    sdk: &RadrootsClient,
    now_ms: i64,
) -> Result<(), RadrootsSdkError> {
    sdk._outbox.recover_expired_claims(now_ms).await?;
    Ok(())
}

#[cfg(feature = "runtime")]
async fn claim_ready_signed_event_for_push(
    sdk: &RadrootsClient,
    request: &PushOutboxRequest,
    claim_token: &str,
    claim_now_ms: i64,
) -> Result<Option<radroots_outbox::RadrootsOutboxClaimedEvent>, RadrootsSdkError> {
    let claim_expires_at_ms = claim_now_ms.saturating_add(request.claim_ttl_ms);
    match request.outbox_event_id {
        Some(outbox_event_id) => Ok(sdk
            ._outbox
            .claim_ready_signed_event(
                outbox_event_id,
                CLAIM_OWNER,
                claim_token,
                claim_expires_at_ms,
                claim_now_ms,
            )
            .await?),
        None => Ok(sdk
            ._outbox
            .claim_next_ready_signed_event(
                CLAIM_OWNER,
                claim_token,
                claim_expires_at_ms,
                claim_now_ms,
            )
            .await?),
    }
}

#[cfg(feature = "runtime")]
pub(crate) async fn refresh_product_projections_for_sdk(
    sdk: &RadrootsClient,
    request: SyncProjectionRefreshRequest,
) -> Result<SyncProjectionRefreshReceipt, RadrootsSdkError> {
    let refreshed_at_ms = sdk_now_ms(sdk)?;
    let receipt = refresh_product_projections(
        &sdk._event_store,
        RadrootsProjectionRefreshRequest::new().with_limit(request.limit),
        refreshed_at_ms,
    )
    .await?;
    Ok(SyncProjectionRefreshReceipt::from_trade(
        receipt,
        refreshed_at_ms,
    ))
}

#[cfg(all(feature = "runtime", feature = "radrootsd-proxy"))]
async fn push_proxy_claimed_outbox_event(
    sync: &SyncClient<'_>,
    adapter: &RadrootsdProxyPublishAdapter,
    claimed: &RadrootsOutboxClaimedEvent,
    next_attempt_delay_ms: i64,
    now_ms: i64,
) -> Result<PushOutboxEventReceipt, RadrootsSdkError> {
    let signed_event = claimed.signed_event.clone().ok_or(
        radroots_transport_nostr::RadrootsRelayTransportError::MissingSignedOutboxEvent(
            claimed.outbox_event_id,
        ),
    )?;
    let target_policy = match proxy_transport_publish_target_policy(claimed) {
        Ok(target_policy) => target_policy,
        Err(error) => {
            return fail_proxy_local_validation(sync, claimed, error, now_ms).await;
        }
    };
    let delivery_policy = match proxy_delivery_policy(sync, claimed).await {
        Ok(delivery_policy) => delivery_policy,
        Err(error) => {
            return fail_proxy_local_validation(sync, claimed, error, now_ms).await;
        }
    };
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
        delivery_policy: delivery_policy.clone(),
        target_policy,
        idempotency_key: Some(proxy_outbox_idempotency_key(
            claimed.outbox_event_id,
            claimed.attempt_count,
            signed_event.id.as_str(),
            active_delivery_plan_id(claimed, "radrootsd proxy publish")?,
        )),
        timeout_ms: adapter.config().request_timeout_ms,
    };
    let publish = match adapter.publish_signed_event(request).await {
        Ok(response) => response.job,
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
            return proxy_transport_error_receipt(
                claimed,
                &signed_event,
                &delivery_policy,
                message,
            );
        }
    };
    complete_proxy_publish_attempt(sync, claimed, &publish, next_attempt_delay_ms, now_ms).await?;
    push_proxy_event_receipt(claimed.outbox_event_id, publish)
}

#[cfg(all(feature = "runtime", feature = "radrootsd-proxy"))]
async fn fail_proxy_local_validation(
    sync: &SyncClient<'_>,
    claimed: &RadrootsOutboxClaimedEvent,
    error: RadrootsSdkError,
    now_ms: i64,
) -> Result<PushOutboxEventReceipt, RadrootsSdkError> {
    let message = error.to_string();
    sync.sdk
        ._outbox
        .mark_active_delivery_plan_failed_terminal(
            claimed.outbox_event_id,
            claimed.claim_token.as_str(),
            message.as_str(),
            now_ms,
        )
        .await?;
    Err(error)
}

#[cfg(all(feature = "runtime", feature = "radrootsd-proxy"))]
async fn proxy_delivery_policy(
    sync: &SyncClient<'_>,
    claimed: &RadrootsOutboxClaimedEvent,
) -> Result<TransportPublishDeliveryPolicy, RadrootsSdkError> {
    let active_delivery_plan_id = active_delivery_plan_id(claimed, "radrootsd proxy publish")?;
    let plans = sync
        .sdk
        ._outbox
        .delivery_plans(claimed.outbox_event_id)
        .await?;
    let plan = plans
        .iter()
        .find(|plan| plan.delivery_plan_id == active_delivery_plan_id)
        .ok_or_else(|| RadrootsSdkError::InvalidRequest {
            message: format!(
                "outbox event {} active delivery plan {} was not found for proxy publish",
                claimed.outbox_event_id, active_delivery_plan_id
            ),
        })?;
    let targets = sync
        .sdk
        ._outbox
        .delivery_targets(claimed.outbox_event_id)
        .await?;
    let active_targets = targets
        .iter()
        .filter(|target| target.delivery_plan_id == active_delivery_plan_id)
        .collect::<Vec<_>>();
    let satisfied_count = active_targets
        .iter()
        .filter(|target| {
            target
                .status
                .counts_as_transport_satisfaction(plan.satisfaction_policy.class())
        })
        .count();
    let ready_target_count = active_targets
        .iter()
        .filter(|target| target.status.is_ready_for_attempt())
        .count();
    let required_remaining = (plan.required_success_count as usize).saturating_sub(satisfied_count);
    proxy_delivery_policy_from_remaining(
        ready_target_count,
        required_remaining,
        &plan.satisfaction_policy,
    )
}

#[cfg(all(feature = "runtime", feature = "radrootsd-proxy"))]
fn proxy_delivery_policy_from_remaining(
    ready_target_count: usize,
    required_remaining: usize,
    satisfaction_policy: &RadrootsTransportSatisfactionPolicy,
) -> Result<TransportPublishDeliveryPolicy, RadrootsSdkError> {
    if ready_target_count == 0 || required_remaining == 0 {
        return Ok(TransportPublishDeliveryPolicy::Any);
    }
    Ok(match satisfaction_policy {
        RadrootsTransportSatisfactionPolicy::Any { .. } => TransportPublishDeliveryPolicy::Any,
        RadrootsTransportSatisfactionPolicy::All { .. } => TransportPublishDeliveryPolicy::All,
        RadrootsTransportSatisfactionPolicy::Quorum { .. } => {
            if required_remaining >= ready_target_count {
                TransportPublishDeliveryPolicy::All
            } else if required_remaining == 1 {
                TransportPublishDeliveryPolicy::Any
            } else {
                TransportPublishDeliveryPolicy::Quorum {
                    quorum: required_remaining,
                }
            }
        }
    })
}

#[cfg(all(feature = "runtime", feature = "radrootsd-proxy"))]
fn proxy_outbox_idempotency_key(
    outbox_event_id: i64,
    attempt_count: i64,
    event_id: &str,
    active_delivery_plan_id: i64,
) -> String {
    format!(
        "radroots-sdk-outbox-{outbox_event_id}-{attempt_count}-{event_id}-{active_delivery_plan_id}"
    )
}

#[cfg(all(feature = "runtime", feature = "radrootsd-proxy"))]
fn active_delivery_plan_id(
    claimed: &RadrootsOutboxClaimedEvent,
    operation: &'static str,
) -> Result<i64, RadrootsSdkError> {
    claimed
        .active_delivery_plan_id
        .ok_or_else(|| RadrootsSdkError::InvalidRequest {
            message: format!(
                "outbox event {} has no active delivery plan for {operation}",
                claimed.outbox_event_id
            ),
        })
}

#[cfg(all(feature = "runtime", feature = "radrootsd-proxy"))]
async fn complete_proxy_publish_attempt(
    sync: &SyncClient<'_>,
    claimed: &RadrootsOutboxClaimedEvent,
    publish: &TransportPublishJobView,
    next_attempt_delay_ms: i64,
    now_ms: i64,
) -> Result<(), RadrootsSdkError> {
    let mut completed_target_ids = std::collections::BTreeSet::new();
    let mut matched_outcomes = Vec::new();
    for outcome in &publish.targets {
        let matched_targets = claimed
            .delivery_targets
            .iter()
            .filter(|target| target.status.is_ready_for_attempt())
            .filter(|target| proxy_target_matches_outcome(target, outcome))
            .collect::<Vec<_>>();
        if matched_targets.is_empty() {
            continue;
        }
        if matched_targets.len() > 1 {
            return Err(RadrootsSdkError::InvalidRequest {
                message: format!(
                    "radrootsd proxy publish outcome for {} {} matched multiple ready delivery targets on outbox event {}",
                    outcome.transport_kind, outcome.endpoint_uri, claimed.outbox_event_id
                ),
            });
        }
        let target = matched_targets[0];
        if !completed_target_ids.insert(target.delivery_target_id) {
            return Err(RadrootsSdkError::InvalidRequest {
                message: format!(
                    "radrootsd proxy publish outcome for {} {} matched delivery target {} more than once on outbox event {}",
                    outcome.transport_kind,
                    outcome.endpoint_uri,
                    target.delivery_target_id,
                    claimed.outbox_event_id
                ),
            });
        }
        matched_outcomes.push((target, outcome));
    }
    for (target, outcome) in matched_outcomes {
        complete_proxy_delivery_target(sync, claimed, target, outcome, now_ms).await?;
    }
    for target in claimed
        .delivery_targets
        .iter()
        .filter(|target| target.status.is_ready_for_attempt())
        .filter(|target| !completed_target_ids.contains(&target.delivery_target_id))
    {
        complete_missing_proxy_delivery_target(sync, claimed, target, publish, now_ms).await?;
    }
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
    Ok(())
}

#[cfg(all(feature = "runtime", feature = "radrootsd-proxy"))]
fn proxy_transport_publish_target_policy(
    claimed: &RadrootsOutboxClaimedEvent,
) -> Result<TransportPublishTargetPolicy, RadrootsSdkError> {
    let ready_targets = claimed
        .delivery_targets
        .iter()
        .filter(|target| target.status.is_ready_for_attempt())
        .collect::<Vec<_>>();
    if ready_targets
        .iter()
        .any(|target| is_proxy_delegate_target(target))
    {
        if ready_targets.len() != 1 || !is_proxy_delegate_target(ready_targets[0]) {
            return Err(RadrootsSdkError::InvalidRequest {
                message: format!(
                    "radrootsd proxy outbox publish does not accept mixed proxy delegate targets for outbox event {}",
                    claimed.outbox_event_id
                ),
            });
        }
        Ok(TransportPublishTargetPolicy::nostr(
            NostrPublishTargetSourcePolicy::RequestThenAuthorWriteThenDaemonDefault,
            Vec::new(),
        ))
    } else {
        Ok(TransportPublishTargetPolicy::explicit_targets(
            ready_targets
                .into_iter()
                .map(transport_publish_target_from_outbox_target)
                .collect::<Result<Vec<_>, _>>()?,
        ))
    }
}

#[cfg(all(feature = "runtime", feature = "radrootsd-proxy"))]
fn transport_publish_target_from_outbox_target(
    target: &RadrootsOutboxDeliveryTargetRecord,
) -> Result<TransportPublishTarget, RadrootsSdkError> {
    if target.transport_kind != RadrootsTransportKind::Nostr {
        return Err(RadrootsSdkError::InvalidRequest {
            message: format!(
                "radrootsd proxy outbox publish explicit targets are Nostr-only and cannot publish {} target {}",
                target.transport_kind.canonical_label(),
                target.endpoint_uri.as_str()
            ),
        });
    }
    Ok(TransportPublishTarget {
        transport_kind: target.transport_kind.canonical_label(),
        endpoint_uri: target.endpoint_uri.as_str().to_owned(),
        preview_behavior: None,
    })
}

#[cfg(all(feature = "runtime", feature = "radrootsd-proxy"))]
fn proxy_target_matches_outcome(
    target: &RadrootsOutboxDeliveryTargetRecord,
    outcome: &TransportPublishTargetOutcome,
) -> bool {
    target.transport_kind.canonical_label() == outcome.transport_kind
        && target.endpoint_uri.as_str() == outcome.endpoint_uri
}

#[cfg(all(feature = "runtime", feature = "radrootsd-proxy"))]
fn is_proxy_delegate_target(target: &RadrootsOutboxDeliveryTargetRecord) -> bool {
    target.transport_kind == RadrootsTransportKind::Proxy
}

#[cfg(all(feature = "runtime", feature = "radrootsd-proxy"))]
async fn complete_proxy_delivery_target(
    sync: &SyncClient<'_>,
    claimed: &RadrootsOutboxClaimedEvent,
    target: &RadrootsOutboxDeliveryTargetRecord,
    outcome: &TransportPublishTargetOutcome,
    now_ms: i64,
) -> Result<(), RadrootsSdkError> {
    if outcome.outcome_kind.counts_toward_satisfaction() {
        sync.sdk
            ._outbox
            .mark_delivery_target_accepted(
                claimed.outbox_event_id,
                claimed.claim_token.as_str(),
                target.delivery_target_id,
                now_ms,
            )
            .await?;
    } else if outcome.outcome_kind.is_retryable() {
        sync.sdk
            ._outbox
            .mark_delivery_target_failed_retryable(
                claimed.outbox_event_id,
                claimed.claim_token.as_str(),
                target.delivery_target_id,
                outcome
                    .message
                    .as_deref()
                    .unwrap_or("radrootsd proxy publish retryable"),
                now_ms,
            )
            .await?;
    } else if outcome.outcome_kind == TransportPublishOutcomeKind::DeferredUntilImplemented {
        sync.sdk
            ._outbox
            .mark_delivery_target_deferred_until_implemented(
                claimed.outbox_event_id,
                claimed.claim_token.as_str(),
                target.delivery_target_id,
                outcome
                    .message
                    .as_deref()
                    .unwrap_or("radrootsd proxy publish deferred until implemented"),
                now_ms,
            )
            .await?;
    } else if outcome.outcome_kind == TransportPublishOutcomeKind::PreviewUnavailable {
        sync.sdk
            ._outbox
            .mark_delivery_target_preview_unavailable(
                claimed.outbox_event_id,
                claimed.claim_token.as_str(),
                target.delivery_target_id,
                outcome
                    .message
                    .as_deref()
                    .unwrap_or("radrootsd proxy publish preview unavailable"),
                now_ms,
            )
            .await?;
    } else {
        sync.sdk
            ._outbox
            .mark_delivery_target_failed_terminal(
                claimed.outbox_event_id,
                claimed.claim_token.as_str(),
                target.delivery_target_id,
                outcome
                    .message
                    .as_deref()
                    .unwrap_or("radrootsd proxy publish terminal"),
                now_ms,
            )
            .await?;
    }
    Ok(())
}

#[cfg(all(feature = "runtime", feature = "radrootsd-proxy"))]
async fn complete_missing_proxy_delivery_target(
    sync: &SyncClient<'_>,
    claimed: &RadrootsOutboxClaimedEvent,
    target: &RadrootsOutboxDeliveryTargetRecord,
    publish: &TransportPublishJobView,
    now_ms: i64,
) -> Result<(), RadrootsSdkError> {
    if is_proxy_delegate_target(target) && publish.delivery_satisfied {
        sync.sdk
            ._outbox
            .mark_delivery_target_accepted(
                claimed.outbox_event_id,
                claimed.claim_token.as_str(),
                target.delivery_target_id,
                now_ms,
            )
            .await?;
    } else if publish.status == TransportPublishJobStatus::DeliveryDeferred {
        sync.sdk
            ._outbox
            .mark_delivery_target_deferred_until_implemented(
                claimed.outbox_event_id,
                claimed.claim_token.as_str(),
                target.delivery_target_id,
                "radrootsd proxy publish deferred until implemented",
                now_ms,
            )
            .await?;
    } else if publish.status == TransportPublishJobStatus::DeliveryPreviewUnavailable {
        sync.sdk
            ._outbox
            .mark_delivery_target_preview_unavailable(
                claimed.outbox_event_id,
                claimed.claim_token.as_str(),
                target.delivery_target_id,
                "radrootsd proxy publish preview unavailable",
                now_ms,
            )
            .await?;
    } else if publish.retryable_count > 0
        || !publish.terminal
        || target.status == RadrootsOutboxDeliveryTargetStatus::FailedRetryable
    {
        sync.sdk
            ._outbox
            .mark_delivery_target_failed_retryable(
                claimed.outbox_event_id,
                claimed.claim_token.as_str(),
                target.delivery_target_id,
                "radrootsd proxy publish incomplete",
                now_ms,
            )
            .await?;
    } else {
        sync.sdk
            ._outbox
            .mark_delivery_target_failed_terminal(
                claimed.outbox_event_id,
                claimed.claim_token.as_str(),
                target.delivery_target_id,
                "radrootsd proxy publish terminal",
                now_ms,
            )
            .await?;
    }
    Ok(())
}

#[cfg(all(feature = "runtime", feature = "radrootsd-proxy"))]
fn proxy_error_message(error: &RadrootsdError) -> String {
    format!("radrootsd proxy publish failed: {error}")
}

#[cfg(all(feature = "runtime", feature = "radrootsd-proxy"))]
fn proxy_transport_error_receipt(
    claimed: &RadrootsOutboxClaimedEvent,
    event: &radroots_events::draft::RadrootsSignedNostrEvent,
    delivery_policy: &TransportPublishDeliveryPolicy,
    message: String,
) -> Result<PushOutboxEventReceipt, RadrootsSdkError> {
    let ready_targets = claimed
        .delivery_targets
        .iter()
        .filter(|target| target.status.is_ready_for_attempt())
        .collect::<Vec<_>>();
    let target_count = ready_targets.len();
    let event_id = push_receipt_event_id(event.id.as_str(), "proxy transport failure event id")?;
    Ok(PushOutboxEventReceipt {
        event_id,
        outbox_event_id: claimed.outbox_event_id,
        final_state: PushOutboxEventState::PublishRetryable,
        attempted_count: 0,
        accepted_count: 0,
        retryable_count: target_count,
        terminal_count: 0,
        quorum: delivery_policy.required_target_count(target_count),
        quorum_met: false,
        targets: ready_targets
            .into_iter()
            .map(|target| PushOutboxTargetReceipt {
                transport_kind: target.transport_kind.canonical_label(),
                endpoint_uri: target.endpoint_uri.as_str().to_owned(),
                outcome_kind: PushOutboxTargetOutcomeKind::ConnectionFailed,
                attempted: false,
                message: Some(message.clone()),
            })
            .collect(),
    })
}

#[cfg(all(feature = "runtime", feature = "radrootsd-proxy"))]
fn proxy_push_event_final_state(publish: &TransportPublishJobView) -> PushOutboxEventState {
    if publish.delivery_satisfied {
        PushOutboxEventState::Published
    } else if publish.status == TransportPublishJobStatus::DeliveryDeferred {
        PushOutboxEventState::DeferredUntilImplemented
    } else if publish.status == TransportPublishJobStatus::DeliveryPreviewUnavailable {
        PushOutboxEventState::PreviewUnavailable
    } else if publish.retryable_count > 0 || !publish.terminal {
        PushOutboxEventState::PublishRetryable
    } else {
        PushOutboxEventState::FailedTerminal
    }
}

#[cfg(all(feature = "runtime", feature = "radrootsd-proxy"))]
fn push_proxy_event_receipt(
    outbox_event_id: i64,
    publish: TransportPublishJobView,
) -> Result<PushOutboxEventReceipt, RadrootsSdkError> {
    let event_id = push_receipt_event_id(
        publish.event_id.as_str(),
        "transport publish daemon job event id",
    )?;
    let quorum = publish
        .delivery_policy
        .required_target_count(publish.target_count);
    Ok(PushOutboxEventReceipt {
        event_id,
        outbox_event_id,
        final_state: proxy_push_event_final_state(&publish),
        attempted_count: publish
            .targets
            .iter()
            .filter(|target| target.attempted)
            .count(),
        accepted_count: publish.acknowledged_count,
        retryable_count: publish.retryable_count,
        terminal_count: publish.terminal_count,
        quorum,
        quorum_met: publish.delivery_satisfied,
        targets: publish
            .targets
            .into_iter()
            .map(push_proxy_target_receipt)
            .collect(),
    })
}

#[cfg(all(feature = "runtime", feature = "radrootsd-proxy"))]
fn push_proxy_target_receipt(outcome: TransportPublishTargetOutcome) -> PushOutboxTargetReceipt {
    PushOutboxTargetReceipt {
        transport_kind: outcome.transport_kind,
        endpoint_uri: outcome.endpoint_uri,
        outcome_kind: push_proxy_target_outcome_kind(outcome.outcome_kind),
        attempted: outcome.attempted,
        message: outcome.message,
    }
}

#[cfg(all(feature = "runtime", feature = "radrootsd-proxy"))]
fn push_proxy_target_outcome_kind(
    outcome_kind: TransportPublishOutcomeKind,
) -> PushOutboxTargetOutcomeKind {
    match outcome_kind {
        TransportPublishOutcomeKind::Accepted => PushOutboxTargetOutcomeKind::Accepted,
        TransportPublishOutcomeKind::DuplicateAccepted => {
            PushOutboxTargetOutcomeKind::DuplicateAccepted
        }
        TransportPublishOutcomeKind::Blocked => PushOutboxTargetOutcomeKind::Blocked,
        TransportPublishOutcomeKind::RateLimited => PushOutboxTargetOutcomeKind::RateLimited,
        TransportPublishOutcomeKind::Invalid => PushOutboxTargetOutcomeKind::Invalid,
        TransportPublishOutcomeKind::PowRequired => PushOutboxTargetOutcomeKind::PowRequired,
        TransportPublishOutcomeKind::Restricted => PushOutboxTargetOutcomeKind::Restricted,
        TransportPublishOutcomeKind::AuthRequired => PushOutboxTargetOutcomeKind::AuthRequired,
        TransportPublishOutcomeKind::Muted => PushOutboxTargetOutcomeKind::Muted,
        TransportPublishOutcomeKind::Unsupported => PushOutboxTargetOutcomeKind::Unsupported,
        TransportPublishOutcomeKind::PaymentRequired => {
            PushOutboxTargetOutcomeKind::PaymentRequired
        }
        TransportPublishOutcomeKind::Error => PushOutboxTargetOutcomeKind::Error,
        TransportPublishOutcomeKind::Timeout => PushOutboxTargetOutcomeKind::Timeout,
        TransportPublishOutcomeKind::ConnectionFailed => {
            PushOutboxTargetOutcomeKind::ConnectionFailed
        }
        TransportPublishOutcomeKind::TargetRejected => {
            PushOutboxTargetOutcomeKind::TargetUriRejected
        }
        TransportPublishOutcomeKind::SkippedAlreadyAccepted => {
            PushOutboxTargetOutcomeKind::SkippedAlreadyAccepted
        }
        TransportPublishOutcomeKind::DeferredUntilImplemented => {
            PushOutboxTargetOutcomeKind::DeferredUntilImplemented
        }
        TransportPublishOutcomeKind::PreviewUnavailable => {
            PushOutboxTargetOutcomeKind::PreviewUnavailable
        }
        TransportPublishOutcomeKind::Unknown => PushOutboxTargetOutcomeKind::Unknown,
    }
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
) -> Result<PushOutboxEventReceipt, RadrootsSdkError> {
    let event_id = push_receipt_event_id(
        publish.event_id.as_str(),
        "relay transport publish receipt event id",
    )?;
    Ok(PushOutboxEventReceipt {
        event_id,
        outbox_event_id,
        final_state,
        attempted_count: publish.attempted_count,
        accepted_count: publish.accepted_count,
        retryable_count: publish.retryable_count,
        terminal_count: publish.terminal_count,
        quorum: publish.quorum,
        quorum_met: publish.quorum_met,
        targets: publish
            .relays
            .into_iter()
            .map(push_target_receipt)
            .collect(),
    })
}

#[cfg(feature = "runtime")]
fn push_receipt_event_id(value: &str, field: &str) -> Result<RadrootsEventId, RadrootsSdkError> {
    RadrootsEventId::parse(value).map_err(|error| RadrootsSdkError::InvalidRequest {
        message: format!("{field} is invalid: {error}"),
    })
}

#[cfg(feature = "runtime")]
fn push_target_receipt(relay: RadrootsRelayPublishRelayReceipt) -> PushOutboxTargetReceipt {
    PushOutboxTargetReceipt {
        transport_kind: RadrootsTransportKind::Nostr.canonical_label(),
        endpoint_uri: relay.relay_url,
        outcome_kind: relay.outcome.kind.into(),
        attempted: relay.attempted,
        message: relay.outcome.message,
    }
}

#[cfg(all(test, feature = "runtime"))]
#[path = "../tests/unit/sync_runtime_tests.rs"]
mod tests;
