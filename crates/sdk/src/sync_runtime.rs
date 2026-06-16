#[cfg(feature = "runtime")]
use crate::{RadrootsSdkError, SyncClient, runtime::sdk_now_ms};
#[cfg(all(feature = "runtime", feature = "relay-runtime"))]
use radroots_nostr::prelude::RadrootsNostrClient;
#[cfg(feature = "runtime")]
use radroots_outbox::RadrootsOutboxEventState;
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
const CLAIM_OWNER: &str = "radroots_sdk.sync.push_outbox";
#[cfg(feature = "runtime")]
const CLAIM_TTL_MS: i64 = 30_000;
#[cfg(feature = "runtime")]
const NEXT_ATTEMPT_DELAY_MS: i64 = 60_000;

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PushOutboxRequest {
    pub limit: usize,
    pub republish_accepted_relays: bool,
}

#[cfg(feature = "runtime")]
impl Default for PushOutboxRequest {
    fn default() -> Self {
        Self {
            limit: PUSH_OUTBOX_DEFAULT_LIMIT,
            republish_accepted_relays: false,
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

    fn validate(&self) -> Result<(), RadrootsSdkError> {
        if self.limit == 0 || self.limit > PUSH_OUTBOX_MAX_LIMIT {
            return Err(RadrootsSdkError::InvalidRequest {
                message: format!("push_outbox limit must be between 1 and {PUSH_OUTBOX_MAX_LIMIT}"),
            });
        }
        Ok(())
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
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
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PushOutboxEventReceipt {
    pub event_id: String,
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
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PushOutboxRelayReceipt {
    pub relay_url: String,
    pub outcome_kind: PushOutboxRelayOutcomeKind,
    pub attempted: bool,
    pub message: Option<String>,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PushOutboxRelayOutcomeKind {
    Accepted,
    DuplicateAccepted,
    Blocked,
    RateLimited,
    Invalid,
    PowRequired,
    Restricted,
    AuthRequired,
    Error,
    Timeout,
    ConnectionFailed,
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
            RadrootsRelayOutcomeKind::Error => Self::Error,
            RadrootsRelayOutcomeKind::Timeout => Self::Timeout,
            RadrootsRelayOutcomeKind::ConnectionFailed => Self::ConnectionFailed,
            RadrootsRelayOutcomeKind::Unknown => Self::Unknown,
        }
    }
}

#[cfg(feature = "runtime")]
impl<'sdk> SyncClient<'sdk> {
    pub async fn push_outbox(
        &self,
        request: PushOutboxRequest,
    ) -> Result<PushOutboxReceipt, RadrootsSdkError> {
        #[cfg(feature = "relay-runtime")]
        {
            if self.sdk.relay_urls().is_empty() {
                return Err(RadrootsSdkError::ProductSyncRelaySetupFailure {
                    message: "sync push requires configured relay URLs".to_owned(),
                });
            }
            let adapter =
                RadrootsNostrClientPublishAdapter::new(RadrootsNostrClient::new_signerless());
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

    pub async fn push_outbox_with_adapter<A>(
        &self,
        adapter: &A,
        request: PushOutboxRequest,
    ) -> Result<PushOutboxReceipt, RadrootsSdkError>
    where
        A: RadrootsRelayPublishAdapter,
    {
        request.validate()?;
        let now_ms = sdk_now_ms(self.sdk)?;
        let mut receipt = PushOutboxReceipt::default();
        for _ in 0..request.limit {
            let claim_token = push_outbox_claim_token();
            let Some(claimed) = self
                .sdk
                ._outbox
                .claim_next_ready_signed_event(
                    CLAIM_OWNER,
                    claim_token.as_str(),
                    now_ms.saturating_add(CLAIM_TTL_MS),
                    now_ms,
                )
                .await?
            else {
                break;
            };
            let policy =
                RadrootsOutboxPublishPolicy::new(now_ms.saturating_add(NEXT_ATTEMPT_DELAY_MS))
                    .republish_accepted_relays(request.republish_accepted_relays);
            let publish = publish_claimed_outbox_event(
                &self.sdk._outbox,
                &self.sdk._event_store,
                adapter,
                &claimed,
                policy,
                now_ms,
            )
            .await?;
            let outbox_event = self
                .sdk
                ._outbox
                .get_event(claimed.outbox_event_id)
                .await?
                .ok_or_else(|| RadrootsSdkError::Outbox {
                    message: "published outbox event was not found after sync push".to_owned(),
                })?;
            receipt.push_event(push_event_receipt(
                claimed.outbox_event_id,
                outbox_event.state.into(),
                publish.publish,
            ));
        }
        Ok(receipt)
    }
}

#[cfg(feature = "runtime")]
fn push_outbox_claim_token() -> String {
    format!("radroots-sdk-sync-{}", uuid::Uuid::now_v7())
}

#[cfg(feature = "runtime")]
fn push_event_receipt(
    outbox_event_id: i64,
    final_state: PushOutboxEventState,
    publish: RadrootsRelayPublishReceipt,
) -> PushOutboxEventReceipt {
    PushOutboxEventReceipt {
        event_id: publish.event_id,
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
mod tests {
    use super::push_outbox_claim_token;
    use std::collections::BTreeSet;

    #[test]
    fn push_outbox_claim_tokens_are_unique_under_immediate_generation() {
        let mut tokens = BTreeSet::new();
        for _ in 0..1_024 {
            let token = push_outbox_claim_token();
            assert!(token.starts_with("radroots-sdk-sync-"));
            assert!(tokens.insert(token));
        }
    }
}
