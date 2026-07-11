#![forbid(unsafe_code)]

use crate::{
    RadrootsRelayOutcome, RadrootsRelayPublishAdapter, RadrootsRelayPublishReceipt,
    RadrootsRelayPublishRelayReceipt, RadrootsRelayPublishRequest, RadrootsRelayTargetSet,
    RadrootsRelayTransportError, RadrootsRelayUrlPolicy, publish_signed_event,
};
use radroots_event_store::{
    RadrootsEventIngest, RadrootsEventStore, RadrootsTransportObservation,
    RadrootsTransportObservationType,
};
use radroots_events::RadrootsEventEnvelope;
use radroots_events::draft::RadrootsSignedEvent;
use radroots_outbox::{
    RadrootsOutbox, RadrootsOutboxClaimedEvent, RadrootsOutboxDeliveryTargetRecord,
    RadrootsOutboxDeliveryTargetStatus, RadrootsOutboxEventStoreIngestReceipt,
};
use radroots_transport::{
    RadrootsTransportKind, RadrootsTransportSatisfactionClass, RadrootsTransportSatisfactionPolicy,
    RadrootsTransportTargetFingerprint,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOutboxPublishPolicy {
    pub next_attempt_after_ms: i64,
    pub republish_accepted_relays: bool,
    pub relay_url_policy: RadrootsRelayUrlPolicy,
}

impl RadrootsOutboxPublishPolicy {
    pub fn new(next_attempt_after_ms: i64) -> Self {
        Self {
            next_attempt_after_ms,
            republish_accepted_relays: false,
            relay_url_policy: RadrootsRelayUrlPolicy::Public,
        }
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
    pub event_id: String,
    pub attempted_count: usize,
    pub accepted_count: usize,
    pub retryable_count: usize,
    pub terminal_count: usize,
    pub quorum: usize,
    pub quorum_met: bool,
    pub target_receipts: Vec<RadrootsOutboxPublishTargetReceipt>,
    pub relay_receipts: Vec<RadrootsRelayPublishRelayReceipt>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOutboxPublishTargetReceipt {
    pub delivery_target_id: i64,
    pub endpoint_uri: String,
    pub endpoint_fingerprint: RadrootsTransportTargetFingerprint,
    pub target_scope: Option<String>,
    pub target_label: Option<String>,
    pub attempted: bool,
    pub outcome: RadrootsRelayOutcome,
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
    if publishable.relays.is_empty() {
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
        return Ok(RadrootsOutboxPublishReceipt {
            local_ingest,
            event_id: signed_event.id,
            attempted_count: 0,
            accepted_count: publishable.accepted_count,
            retryable_count: 0,
            terminal_count: 0,
            quorum: publishable.satisfaction_required_count,
            quorum_met: publishable.accepted_count >= publishable.satisfaction_required_count,
            target_receipts: Vec::new(),
            relay_receipts: Vec::new(),
        });
    }
    let targets = RadrootsRelayTargetSet::new(
        publishable
            .relays
            .iter()
            .map(|target| target.relay_url.as_str()),
        policy.relay_url_policy,
    )?;
    let target_strings = targets.relay_strings();
    let satisfaction_policy = satisfaction_policy_for_required_accept_count(
        publishable.required_accept_count,
        targets.len(),
        publishable.required_targets.is_some(),
    )?;
    let active_delivery_plan_id = publishable.active_delivery_plan_id;
    let request = RadrootsRelayPublishRequest::new(signed_event.clone(), targets, now_ms)
        .with_satisfaction_policy(satisfaction_policy)
        .with_idempotency_key(outbox_publish_idempotency_key(
            claimed.outbox_event_id,
            claimed.attempt_count,
            signed_event.id.as_str(),
            active_delivery_plan_id,
        ));
    let publish = match publish_signed_event(adapter, request).await {
        Ok(receipt) => receipt,
        Err(RadrootsRelayTransportError::Transport(message)) => adapter_transport_failure_receipt(
            signed_event.id.clone(),
            target_strings,
            publishable.required_accept_count,
            message,
        ),
        Err(error) => return Err(error),
    };
    let target_receipts = target_receipts_from_relay_receipts(&publishable, &publish.relays);

    for target_receipt in &target_receipts {
        if target_receipt.outcome.counts_toward_quorum() {
            outbox
                .mark_delivery_target_accepted(
                    claimed.outbox_event_id,
                    claimed.claim_token.as_str(),
                    target_receipt.delivery_target_id,
                    now_ms,
                )
                .await?;
        } else if target_receipt.outcome.is_retryable() {
            outbox
                .mark_delivery_target_failed_retryable(
                    claimed.outbox_event_id,
                    claimed.claim_token.as_str(),
                    target_receipt.delivery_target_id,
                    target_receipt
                        .outcome
                        .message
                        .as_deref()
                        .unwrap_or("relay publish retryable"),
                    now_ms,
                )
                .await?;
        } else {
            outbox
                .mark_delivery_target_failed_terminal(
                    claimed.outbox_event_id,
                    claimed.claim_token.as_str(),
                    target_receipt.delivery_target_id,
                    target_receipt
                        .outcome
                        .message
                        .as_deref()
                        .unwrap_or("relay publish terminal"),
                    now_ms,
                )
                .await?;
        }
    }

    for relay in &publish.relays {
        if relay.outcome.counts_toward_quorum()
            && publishable
                .targets_for_relay(relay.relay_url.as_str())
                .next()
                .is_some()
        {
            ingest_publish_observation(
                event_store,
                &signed_event,
                relay.relay_url.as_str(),
                relay.outcome.message.as_deref(),
                now_ms,
            )
            .await?;
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
        event_id: publish.event_id,
        attempted_count: target_receipts
            .iter()
            .filter(|receipt| receipt.attempted)
            .count(),
        accepted_count: target_receipts
            .iter()
            .filter(|receipt| receipt.outcome.counts_toward_quorum())
            .count(),
        retryable_count: target_receipts
            .iter()
            .filter(|receipt| receipt.outcome.is_retryable())
            .count(),
        terminal_count: target_receipts
            .iter()
            .filter(|receipt| receipt.outcome.is_terminal_failure())
            .count(),
        quorum: publishable.required_accept_count,
        quorum_met: publishable.satisfied_count_after_receipts(&target_receipts)
            >= publishable.satisfaction_required_count,
        target_receipts,
        relay_receipts: publish.relays,
    })
}

fn adapter_transport_failure_receipt(
    event_id: String,
    relay_urls: Vec<String>,
    quorum: usize,
    message: String,
) -> RadrootsRelayPublishReceipt {
    let relays = relay_urls
        .into_iter()
        .map(|relay_url| {
            RadrootsRelayPublishRelayReceipt::attempted(
                relay_url,
                RadrootsRelayOutcome::connection_failed(message.clone()),
            )
        })
        .collect::<Vec<_>>();
    RadrootsRelayPublishReceipt {
        event_id,
        attempted_count: relays.len(),
        accepted_count: 0,
        retryable_count: relays.len(),
        terminal_count: 0,
        quorum,
        quorum_met: false,
        relays,
    }
}

struct PublishableRelays {
    active_delivery_plan_id: i64,
    relays: Vec<PublishableRelay>,
    accepted_count: usize,
    satisfaction_required_count: usize,
    required_accept_count: usize,
    required_targets: Option<Vec<RadrootsTransportTargetFingerprint>>,
}

impl PublishableRelays {
    fn targets_for_relay<'a>(
        &'a self,
        relay_url: &'a str,
    ) -> impl Iterator<Item = &'a PublishableRelay> + 'a {
        self.relays
            .iter()
            .filter(move |target| target.relay_url == relay_url)
    }

    fn satisfied_count_after_receipts(
        &self,
        target_receipts: &[RadrootsOutboxPublishTargetReceipt],
    ) -> usize {
        self.accepted_count
            + target_receipts
                .iter()
                .filter(|receipt| {
                    receipt.outcome.counts_toward_quorum()
                        && self.required_targets.as_ref().is_none_or(|required| {
                            required
                                .iter()
                                .any(|fingerprint| receipt.endpoint_fingerprint == *fingerprint)
                        })
                })
                .count()
    }
}

struct PublishableRelay {
    delivery_target_id: i64,
    relay_url: String,
    endpoint_fingerprint: RadrootsTransportTargetFingerprint,
    target_scope: Option<String>,
    target_label: Option<String>,
}

fn target_receipts_from_relay_receipts(
    publishable: &PublishableRelays,
    relay_receipts: &[RadrootsRelayPublishRelayReceipt],
) -> Vec<RadrootsOutboxPublishTargetReceipt> {
    let mut target_receipts = Vec::new();
    for relay_receipt in relay_receipts {
        for target in publishable.targets_for_relay(relay_receipt.relay_url.as_str()) {
            target_receipts.push(RadrootsOutboxPublishTargetReceipt {
                delivery_target_id: target.delivery_target_id,
                endpoint_uri: target.relay_url.clone(),
                endpoint_fingerprint: target.endpoint_fingerprint.clone(),
                target_scope: target.target_scope.clone(),
                target_label: target.target_label.clone(),
                attempted: relay_receipt.attempted,
                outcome: relay_receipt.outcome.clone(),
            });
        }
    }
    target_receipts
}

async fn publishable_relays(
    outbox: &RadrootsOutbox,
    claimed: &RadrootsOutboxClaimedEvent,
    republish_accepted_relays: bool,
) -> Result<PublishableRelays, RadrootsRelayTransportError> {
    let active_delivery_plan_id = claimed.active_delivery_plan_id.ok_or_else(|| {
        RadrootsRelayTransportError::Transport(format!(
            "outbox event {} has no active delivery plan for Nostr publish",
            claimed.outbox_event_id
        ))
    })?;
    let targets = outbox.delivery_targets(claimed.outbox_event_id).await?;
    let plans = outbox.delivery_plans(claimed.outbox_event_id).await?;
    let plan = plans
        .iter()
        .find(|plan| plan.delivery_plan_id == active_delivery_plan_id)
        .ok_or_else(|| {
            RadrootsRelayTransportError::Transport(format!(
                "outbox event {} active delivery plan {} was not found for Nostr publish",
                claimed.outbox_event_id, active_delivery_plan_id
            ))
        })?;
    let satisfaction_required_count = plan.required_success_count as usize;
    let required_targets = match &plan.satisfaction_policy {
        RadrootsTransportSatisfactionPolicy::RequiredTargets { targets, .. } => {
            Some(targets.clone())
        }
        RadrootsTransportSatisfactionPolicy::NoWait
        | RadrootsTransportSatisfactionPolicy::Any { .. }
        | RadrootsTransportSatisfactionPolicy::All { .. }
        | RadrootsTransportSatisfactionPolicy::Quorum { .. } => None,
    };
    let active_targets = targets
        .iter()
        .filter(|target| target.delivery_plan_id == active_delivery_plan_id)
        .collect::<Vec<_>>();
    if let Some(target) = active_targets
        .iter()
        .find(|target| !is_nostr_target(target) && target.status.is_ready_for_attempt())
    {
        return Err(RadrootsRelayTransportError::Transport(format!(
            "direct Nostr outbox publish does not accept {} target {} in active delivery plan {}",
            target.transport_kind.canonical_label(),
            target.endpoint_uri.as_str(),
            active_delivery_plan_id
        )));
    }
    let satisfied_count = plan
        .satisfaction_policy
        .target_satisfaction_class()
        .map(|satisfaction_class| {
            active_targets
                .iter()
                .filter(|target| {
                    required_targets.as_ref().is_none_or(|required| {
                        required
                            .iter()
                            .any(|fingerprint| target.endpoint_fingerprint == *fingerprint)
                    }) && target
                        .status
                        .counts_as_transport_satisfaction(satisfaction_class)
                })
                .count()
        })
        .unwrap_or(0);
    let required_accept_count =
        (plan.required_success_count as usize).saturating_sub(satisfied_count);
    let mut relays = Vec::new();
    let mut accepted_count = 0usize;
    for target in &active_targets {
        if !is_nostr_target(target) {
            continue;
        }
        let required_for_satisfaction = required_targets.as_ref().is_some_and(|required| {
            required
                .iter()
                .any(|fingerprint| target.endpoint_fingerprint == *fingerprint)
        });
        if target
            .status
            .counts_as_transport_satisfaction(RadrootsTransportSatisfactionClass::Accepted)
            && (required_targets.is_none() || required_for_satisfaction)
        {
            accepted_count += 1;
        }
        let can_contribute_to_satisfaction =
            required_targets.is_none() || required_for_satisfaction;
        if required_accept_count > 0
            && can_contribute_to_satisfaction
            && (target.status.is_ready_for_attempt()
                || (republish_accepted_relays
                    && target.status == RadrootsOutboxDeliveryTargetStatus::Accepted))
        {
            relays.push(PublishableRelay {
                delivery_target_id: target.delivery_target_id,
                relay_url: target.endpoint_uri.as_str().to_owned(),
                endpoint_fingerprint: target.endpoint_fingerprint.clone(),
                target_scope: target
                    .target_scope
                    .as_ref()
                    .map(|scope| scope.as_str().to_owned()),
                target_label: target
                    .target_label
                    .as_ref()
                    .map(|label| label.as_str().to_owned()),
            });
        }
    }
    if required_targets.is_some() {
        let selected_relay_urls = relays
            .iter()
            .map(|relay| relay.relay_url.clone())
            .collect::<Vec<_>>();
        for target in &active_targets {
            if !is_nostr_target(target)
                || !selected_relay_urls
                    .iter()
                    .any(|relay_url| relay_url == target.endpoint_uri.as_str())
                || relays
                    .iter()
                    .any(|relay| relay.delivery_target_id == target.delivery_target_id)
                || !(target.status.is_ready_for_attempt()
                    || (republish_accepted_relays
                        && target.status == RadrootsOutboxDeliveryTargetStatus::Accepted))
            {
                continue;
            }
            relays.push(PublishableRelay {
                delivery_target_id: target.delivery_target_id,
                relay_url: target.endpoint_uri.as_str().to_owned(),
                endpoint_fingerprint: target.endpoint_fingerprint.clone(),
                target_scope: target
                    .target_scope
                    .as_ref()
                    .map(|scope| scope.as_str().to_owned()),
                target_label: target
                    .target_label
                    .as_ref()
                    .map(|label| label.as_str().to_owned()),
            });
        }
    }
    Ok(PublishableRelays {
        active_delivery_plan_id,
        relays,
        accepted_count,
        satisfaction_required_count,
        required_accept_count,
        required_targets,
    })
}

fn outbox_publish_idempotency_key(
    outbox_event_id: i64,
    attempt_count: i64,
    event_id: &str,
    active_delivery_plan_id: i64,
) -> String {
    format!(
        "radroots-nostr-outbox-{outbox_event_id}-{attempt_count}-{event_id}-{active_delivery_plan_id}"
    )
}

fn is_nostr_target(target: &RadrootsOutboxDeliveryTargetRecord) -> bool {
    target.transport_kind == RadrootsTransportKind::Nostr
}

fn satisfaction_policy_for_required_accept_count(
    required_accept_count: usize,
    target_count: usize,
    exact_required_targets: bool,
) -> Result<RadrootsTransportSatisfactionPolicy, RadrootsRelayTransportError> {
    if exact_required_targets {
        return Ok(RadrootsTransportSatisfactionPolicy::no_wait());
    }
    if required_accept_count >= target_count {
        return Ok(RadrootsTransportSatisfactionPolicy::all_accepted());
    }
    if required_accept_count == 0 {
        return Err(RadrootsRelayTransportError::Transport(
            "required Nostr relay acceptance count must be greater than zero".to_owned(),
        ));
    }
    if required_accept_count == 1 {
        return Ok(RadrootsTransportSatisfactionPolicy::any_accepted());
    }
    let count = u16::try_from(required_accept_count).map_err(|_| {
        RadrootsRelayTransportError::Transport(
            "required Nostr relay acceptance count exceeds supported transport policy range"
                .to_owned(),
        )
    })?;
    Ok(RadrootsTransportSatisfactionPolicy::quorum_accepted(count))
}

async fn ingest_publish_observation(
    event_store: &RadrootsEventStore,
    signed_event: &RadrootsSignedEvent,
    relay_url: &str,
    message: Option<&str>,
    observed_at_ms: i64,
) -> Result<(), RadrootsRelayTransportError> {
    let observation = RadrootsTransportObservation::new(
        RadrootsTransportKind::Nostr,
        relay_url,
        RadrootsTransportObservationType::PublishAck,
        observed_at_ms,
    );
    let mut observation = observation?;
    if let Some(message) = message {
        observation = observation.with_redacted_message(message);
    }
    let ingest = RadrootsEventIngest::new(event_from_signed(signed_event), observed_at_ms)
        .with_raw_json(signed_event.raw_json.clone())
        .with_observation(observation);
    event_store.ingest_event(ingest).await?;
    Ok(())
}

fn event_from_signed(signed_event: &RadrootsSignedEvent) -> RadrootsEventEnvelope {
    RadrootsEventEnvelope {
        id: signed_event.id.clone(),
        author: signed_event.pubkey.clone(),
        created_at: signed_event.created_at,
        kind: signed_event.kind,
        tags: signed_event.tags.clone(),
        content: signed_event.content.clone(),
        sig: signed_event.sig.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::{adapter_transport_failure_receipt, satisfaction_policy_for_required_accept_count};
    use radroots_transport::RadrootsTransportSatisfactionPolicy;

    #[test]
    fn internal_outbox_publish_helpers_cover_policy_edges() {
        assert_eq!(
            satisfaction_policy_for_required_accept_count(2, 2, false).expect("all targets"),
            RadrootsTransportSatisfactionPolicy::all_accepted()
        );
        assert_eq!(
            satisfaction_policy_for_required_accept_count(1, 3, false).expect("at least one"),
            RadrootsTransportSatisfactionPolicy::any_accepted()
        );
        assert_eq!(
            satisfaction_policy_for_required_accept_count(1, 3, true)
                .expect("exact required targets"),
            RadrootsTransportSatisfactionPolicy::no_wait()
        );
        assert!(
            satisfaction_policy_for_required_accept_count(
                usize::from(u16::MAX) + 1,
                usize::from(u16::MAX) + 2,
                false,
            )
            .is_err()
        );
    }

    #[test]
    fn adapter_transport_failure_receipts_preserve_each_target() {
        let receipt = adapter_transport_failure_receipt(
            "event-1".to_owned(),
            vec![
                "wss://relay-a.example".to_owned(),
                "wss://relay-b.example".to_owned(),
            ],
            2,
            "offline".to_owned(),
        );

        assert_eq!(receipt.event_id, "event-1");
        assert_eq!(receipt.attempted_count, 2);
        assert_eq!(receipt.retryable_count, 2);
        assert_eq!(receipt.terminal_count, 0);
        assert_eq!(receipt.quorum, 2);
        assert!(!receipt.quorum_met);
        assert!(receipt.relays.iter().all(|relay| relay.attempted));
    }
}
