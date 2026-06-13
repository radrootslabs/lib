#![forbid(unsafe_code)]

use crate::{
    RadrootsRelayOutcomeKind, RadrootsRelayPublishAdapter, RadrootsRelayPublishReceipt,
    RadrootsRelayPublishRequest, RadrootsRelayTargetSet, RadrootsRelayTransportError,
    RadrootsRelayUrlPolicy, publish_signed_event,
};
use radroots_event_store::{
    RadrootsEventIngest, RadrootsEventStore, RadrootsRelayObservation, RadrootsRelayObservationType,
};
use radroots_events::RadrootsNostrEvent;
use radroots_events::draft::RadrootsSignedNostrEvent;
use radroots_outbox::{
    RadrootsOutbox, RadrootsOutboxClaimedEvent, RadrootsOutboxEventStoreIngestReceipt,
    RadrootsOutboxRelayStatus,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOutboxPublishPolicy {
    pub accepted_quorum: Option<usize>,
    pub next_attempt_after_ms: i64,
    pub republish_accepted_relays: bool,
    pub relay_url_policy: RadrootsRelayUrlPolicy,
}

impl RadrootsOutboxPublishPolicy {
    pub fn new(next_attempt_after_ms: i64) -> Self {
        Self {
            accepted_quorum: None,
            next_attempt_after_ms,
            republish_accepted_relays: false,
            relay_url_policy: RadrootsRelayUrlPolicy::Public,
        }
    }

    pub fn with_accepted_quorum(mut self, accepted_quorum: usize) -> Self {
        self.accepted_quorum = Some(accepted_quorum);
        self
    }

    pub fn republish_accepted_relays(mut self, enabled: bool) -> Self {
        self.republish_accepted_relays = enabled;
        self
    }

    pub fn relay_url_policy(mut self, policy: RadrootsRelayUrlPolicy) -> Self {
        self.relay_url_policy = policy;
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOutboxPublishReceipt {
    pub local_ingest: RadrootsOutboxEventStoreIngestReceipt,
    pub publish: RadrootsRelayPublishReceipt,
}

pub async fn publish_claimed_outbox_event<A>(
    outbox: &RadrootsOutbox,
    event_store: &RadrootsEventStore,
    adapter: &A,
    claimed: &RadrootsOutboxClaimedEvent,
    policy: RadrootsOutboxPublishPolicy,
    now_ms: i64,
) -> Result<RadrootsOutboxPublishReceipt, RadrootsRelayTransportError>
where
    A: RadrootsRelayPublishAdapter,
{
    let signed_event = claimed.signed_event.clone().ok_or(
        RadrootsRelayTransportError::MissingSignedOutboxEvent(claimed.outbox_event_id),
    )?;
    let local_ingest = outbox
        .ingest_signed_event_local(
            event_store,
            claimed.outbox_event_id,
            claimed.claim_token.as_str(),
            now_ms,
        )
        .await?;
    let publishable = publishable_relays(outbox, claimed, policy.republish_accepted_relays).await?;
    let targets = RadrootsRelayTargetSet::new(publishable.relays, policy.relay_url_policy)?;
    let overall_quorum = policy
        .accepted_quorum
        .unwrap_or(publishable.total_target_count);
    outbox
        .set_publish_quorum(
            claimed.outbox_event_id,
            claimed.claim_token.as_str(),
            overall_quorum as i64,
            now_ms,
        )
        .await?;
    let quorum = overall_quorum.saturating_sub(publishable.accepted_count);
    let request = RadrootsRelayPublishRequest::new(signed_event.clone(), targets, now_ms)
        .with_accepted_quorum(quorum);
    let publish = publish_signed_event(adapter, request).await?;

    for relay in &publish.relays {
        match relay.outcome.kind {
            RadrootsRelayOutcomeKind::Accepted | RadrootsRelayOutcomeKind::DuplicateAccepted => {
                outbox
                    .mark_relay_accepted(
                        claimed.outbox_event_id,
                        claimed.claim_token.as_str(),
                        relay.relay_url.as_str(),
                        now_ms,
                    )
                    .await?;
                ingest_publish_observation(
                    event_store,
                    &signed_event,
                    relay.relay_url.as_str(),
                    relay.outcome.message.as_deref(),
                    now_ms,
                )
                .await?;
            }
            _ if relay.outcome.is_retryable() => {
                outbox
                    .mark_relay_failed_retryable(
                        claimed.outbox_event_id,
                        claimed.claim_token.as_str(),
                        relay.relay_url.as_str(),
                        relay
                            .outcome
                            .message
                            .as_deref()
                            .unwrap_or("relay publish retryable"),
                        now_ms,
                    )
                    .await?;
            }
            _ => {
                outbox
                    .mark_relay_failed_terminal(
                        claimed.outbox_event_id,
                        claimed.claim_token.as_str(),
                        relay.relay_url.as_str(),
                        relay
                            .outcome
                            .message
                            .as_deref()
                            .unwrap_or("relay publish terminal"),
                        now_ms,
                    )
                    .await?;
            }
        }
    }

    outbox
        .complete_publish_attempt(
            claimed.outbox_event_id,
            claimed.claim_token.as_str(),
            "relay publish incomplete",
            "relay publish terminal",
            policy.next_attempt_after_ms,
            now_ms,
        )
        .await?;

    Ok(RadrootsOutboxPublishReceipt {
        local_ingest,
        publish,
    })
}

struct PublishableRelays {
    relays: Vec<String>,
    total_target_count: usize,
    accepted_count: usize,
}

async fn publishable_relays(
    outbox: &RadrootsOutbox,
    claimed: &RadrootsOutboxClaimedEvent,
    republish_accepted_relays: bool,
) -> Result<PublishableRelays, RadrootsRelayTransportError> {
    let statuses = outbox.relay_statuses(claimed.outbox_event_id).await?;
    let mut relays = Vec::new();
    let mut total_target_count = 0usize;
    let mut accepted_count = 0usize;
    for status in statuses {
        if !claimed
            .target_relays
            .iter()
            .any(|relay_url| relay_url == &status.relay_url)
        {
            continue;
        }
        total_target_count += 1;
        if status.status == RadrootsOutboxRelayStatus::Accepted {
            accepted_count += 1;
        }
        if republish_accepted_relays || status.status != RadrootsOutboxRelayStatus::Accepted {
            relays.push(status.relay_url);
        }
    }
    Ok(PublishableRelays {
        relays,
        total_target_count,
        accepted_count,
    })
}

async fn ingest_publish_observation(
    event_store: &RadrootsEventStore,
    signed_event: &RadrootsSignedNostrEvent,
    relay_url: &str,
    message: Option<&str>,
    observed_at_ms: i64,
) -> Result<(), RadrootsRelayTransportError> {
    let mut observation = RadrootsRelayObservation::new(
        relay_url,
        RadrootsRelayObservationType::PublishAck,
        observed_at_ms,
    );
    if let Some(message) = message {
        observation = observation.with_message(message);
    }
    let ingest = RadrootsEventIngest::new(event_from_signed(signed_event), observed_at_ms)
        .with_raw_json(signed_event.raw_json.clone())
        .with_observation(observation);
    event_store.ingest_event(ingest).await?;
    Ok(())
}

fn event_from_signed(signed_event: &RadrootsSignedNostrEvent) -> RadrootsNostrEvent {
    RadrootsNostrEvent {
        id: signed_event.id.clone(),
        author: signed_event.pubkey.clone(),
        created_at: signed_event.created_at,
        kind: signed_event.kind,
        tags: signed_event.tags.clone(),
        content: signed_event.content.clone(),
        sig: signed_event.sig.clone(),
    }
}
