#![forbid(unsafe_code)]

use crate::error::ensure_nonnegative_timestamp;
use crate::{
    RadrootsRelayOutcome, RadrootsRelayPublishAdapter, RadrootsRelayPublishReceipt,
    RadrootsRelayPublishRelayReceipt, RadrootsRelayPublishRequest, RadrootsRelayTargetSet,
    RadrootsRelayTransportError, RadrootsRelayUrlPolicy, publish_signed_event,
    verified_signed_event_payload,
};
use radroots_event::draft::RadrootsVerifiedSignedEvent;
use radroots_event_store::{
    RadrootsEventIngest, RadrootsEventStore, RadrootsTransportObservation,
    RadrootsTransportObservationType,
};
use radroots_outbox::{
    RadrootsOutbox, RadrootsOutboxClaimedEvent, RadrootsOutboxDeliveryTargetRecord,
    RadrootsOutboxDeliveryTargetStatus, RadrootsOutboxEventStoreIngestReceipt,
    RadrootsPhase1PublicationTargetClaim,
};
use radroots_transport::{
    RadrootsTransport, RadrootsTransportDeliveryReceipt, RadrootsTransportDeliveryRequest,
    RadrootsTransportDeliveryTargetStatus, RadrootsTransportError, RadrootsTransportKind,
    RadrootsTransportOutcome, RadrootsTransportOutcomeKind, RadrootsTransportSatisfactionClass,
    RadrootsTransportSatisfactionPolicy, RadrootsTransportTarget,
    RadrootsTransportTargetFingerprint, RadrootsTransportTargetSet,
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
    pub transport_status: RadrootsTransportDeliveryTargetStatus,
    pub outcome: RadrootsRelayOutcome,
}

pub fn phase1_publication_delivery_request(
    claim: &RadrootsPhase1PublicationTargetClaim,
    now_ms: i64,
) -> Result<RadrootsTransportDeliveryRequest, RadrootsRelayTransportError> {
    ensure_nonnegative_timestamp("now_ms", now_ms)?;
    let payload = verified_signed_event_payload(claim.signed_event())
        .map_err(transport_error_to_relay_error)?;
    let target = RadrootsTransportTarget::nostr_relay(claim.endpoint_uri())
        .map_err(transport_error_to_relay_error)?;
    let target_set =
        RadrootsTransportTargetSet::new(vec![target]).map_err(transport_error_to_relay_error)?;
    RadrootsTransportDeliveryRequest::new(
        hex::encode(claim.dispatch_digest()),
        payload,
        target_set,
        RadrootsTransportSatisfactionPolicy::all_accepted(),
    )
    .and_then(|request| request.try_with_now_ms(now_ms))
    .map_err(transport_error_to_relay_error)
}

pub async fn publish_claimed_phase1_publication_target_with_transport<T>(
    transport: &T,
    claim: &RadrootsPhase1PublicationTargetClaim,
    now_ms: i64,
) -> Result<RadrootsTransportDeliveryReceipt, RadrootsRelayTransportError>
where
    T: RadrootsTransport + ?Sized,
{
    let transport_kind = transport.transport_kind();
    if transport_kind != RadrootsTransportKind::Nostr {
        return Err(RadrootsRelayTransportError::UnexpectedTransportKind {
            expected: "nostr",
            actual: transport_kind.canonical_label(),
        });
    }
    let request = phase1_publication_delivery_request(claim, now_ms)?;
    let receipt = transport
        .deliver(request.clone())
        .await
        .map_err(transport_error_to_relay_error)?;
    receipt
        .validate_for_request(&request)
        .map_err(transport_error_to_relay_error)?;
    Ok(receipt)
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
    ensure_nonnegative_timestamp("now_ms", now_ms)?;
    ensure_nonnegative_timestamp("next_attempt_after_ms", policy.next_attempt_after_ms)?;
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
            event_id: signed_event.signed_event().id_str().to_owned(),
            attempted_count: 0,
            accepted_count: publishable.accepted_count,
            retryable_count: 0,
            terminal_count: 0,
            quorum: publishable.remaining_satisfaction_count,
            quorum_met: publishable.satisfied_count >= publishable.satisfaction_required_count,
            target_receipts: Vec::new(),
            relay_receipts: Vec::new(),
        });
    }
    let targets = RadrootsRelayTargetSet::new(
        unique_publishable_relay_urls(&publishable),
        policy.relay_url_policy,
    )?;
    let target_strings = targets.relay_strings();
    let active_delivery_plan_id = publishable.active_delivery_plan_id;
    let request = RadrootsRelayPublishRequest::new(signed_event.clone(), targets, now_ms)?
        .with_satisfaction_policy(RadrootsTransportSatisfactionPolicy::no_wait())
        .try_with_idempotency_key(outbox_publish_idempotency_key(
            claimed.outbox_event_id,
            claimed.attempt_count,
            signed_event.signed_event().id_str(),
            active_delivery_plan_id,
        ))?;
    let publish = match publish_signed_event(adapter, request).await {
        Ok(receipt) => receipt,
        Err(RadrootsRelayTransportError::Transport(message)) => adapter_transport_failure_receipt(
            signed_event.signed_event().id_str().to_owned(),
            target_strings,
            0,
            message,
        )?,
        Err(error) => return Err(error),
    };
    let target_receipts = target_receipts_from_relay_receipts(&publishable, &publish.relays);

    for target_receipt in &target_receipts {
        complete_outbox_delivery_target(outbox, claimed, target_receipt, now_ms).await?;
    }

    for relay in &publish.relays {
        if relay
            .outcome
            .kind()
            .transport_outcome_kind()
            .target_status()
            .counts_as_satisfied(RadrootsTransportSatisfactionClass::Accepted)
            && publishable
                .targets_for_relay(relay.relay_url.as_str())
                .next()
                .is_some()
        {
            ingest_publish_observation(
                event_store,
                &signed_event,
                relay.relay_url.as_str(),
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
        quorum: publishable.remaining_satisfaction_count,
        quorum_met: publishable.satisfied_count_after_receipts(&target_receipts)
            >= publishable.satisfaction_required_count,
        target_receipts,
        relay_receipts: publish.relays,
    })
}

pub async fn publish_claimed_outbox_event_with_transport<T>(
    outbox: &RadrootsOutbox,
    event_store: &RadrootsEventStore,
    transport: &T,
    claimed: &RadrootsOutboxClaimedEvent,
    policy: RadrootsOutboxPublishPolicy,
    now_ms: i64,
) -> Result<RadrootsOutboxPublishReceipt, RadrootsRelayTransportError>
where
    T: RadrootsTransport + ?Sized,
{
    ensure_nonnegative_timestamp("now_ms", now_ms)?;
    ensure_nonnegative_timestamp("next_attempt_after_ms", policy.next_attempt_after_ms)?;
    let transport_kind = transport.transport_kind();
    if transport_kind != RadrootsTransportKind::Nostr {
        return Err(RadrootsRelayTransportError::UnexpectedTransportKind {
            expected: "nostr",
            actual: transport_kind.canonical_label(),
        });
    }
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
            event_id: signed_event.signed_event().id_str().to_owned(),
            attempted_count: 0,
            accepted_count: publishable.accepted_count,
            retryable_count: 0,
            terminal_count: 0,
            quorum: publishable.remaining_satisfaction_count,
            quorum_met: publishable.satisfied_count >= publishable.satisfaction_required_count,
            target_receipts: Vec::new(),
            relay_receipts: Vec::new(),
        });
    }
    RadrootsRelayTargetSet::new(
        unique_publishable_relay_urls(&publishable),
        policy.relay_url_policy,
    )?;
    let transport_targets = publishable_transport_targets(&publishable)?;
    let target_set = RadrootsTransportTargetSet::new(transport_targets)?;
    let satisfaction_policy = transport_satisfaction_policy_for_publishable(&publishable);
    let request_id = outbox_publish_idempotency_key(
        claimed.outbox_event_id,
        claimed.attempt_count,
        signed_event.signed_event().id_str(),
        publishable.active_delivery_plan_id,
    );
    let payload =
        verified_signed_event_payload(&signed_event).map_err(transport_error_to_relay_error)?;
    let delivery_request =
        RadrootsTransportDeliveryRequest::new(request_id, payload, target_set, satisfaction_policy)
            .and_then(|request| request.try_with_now_ms(now_ms))
            .map_err(transport_error_to_relay_error)?;
    let delivery = transport
        .deliver(delivery_request.clone())
        .await
        .map_err(transport_error_to_relay_error)?;
    delivery
        .validate_for_request(&delivery_request)
        .map_err(transport_error_to_relay_error)?;
    let relay_receipts = relay_receipts_from_transport_receipts(&delivery)?;
    let target_receipts = target_receipts_from_transport_receipts(&publishable, &delivery)?;

    for target_receipt in &target_receipts {
        complete_outbox_delivery_target(outbox, claimed, target_receipt, now_ms).await?;
    }

    for relay in &relay_receipts {
        if relay
            .outcome
            .kind()
            .transport_outcome_kind()
            .target_status()
            .counts_as_satisfied(RadrootsTransportSatisfactionClass::Accepted)
            && publishable
                .targets_for_relay(relay.relay_url.as_str())
                .next()
                .is_some()
        {
            ingest_publish_observation(
                event_store,
                &signed_event,
                relay.relay_url.as_str(),
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
        event_id: signed_event.signed_event().id_str().to_owned(),
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
        quorum: publishable.remaining_satisfaction_count,
        quorum_met: publishable.satisfied_count_after_receipts(&target_receipts)
            >= publishable.satisfaction_required_count,
        target_receipts,
        relay_receipts,
    })
}

fn adapter_transport_failure_receipt(
    event_id: String,
    relay_urls: Vec<String>,
    quorum: usize,
    message: String,
) -> Result<RadrootsRelayPublishReceipt, RadrootsRelayTransportError> {
    let relays = relay_urls
        .into_iter()
        .map(|relay_url| {
            Ok(RadrootsRelayPublishRelayReceipt::attempted(
                relay_url,
                RadrootsRelayOutcome::connection_failed(message.clone())?,
            ))
        })
        .collect::<Result<Vec<_>, RadrootsRelayTransportError>>()?;
    Ok(RadrootsRelayPublishReceipt {
        event_id,
        attempted_count: relays.len(),
        accepted_count: 0,
        retryable_count: relays.len(),
        terminal_count: 0,
        quorum,
        quorum_met: false,
        relays,
    })
}

struct PublishableRelays {
    active_delivery_plan_id: i64,
    relays: Vec<PublishableRelay>,
    accepted_count: usize,
    satisfied_count: usize,
    satisfaction_required_count: usize,
    remaining_satisfaction_count: usize,
    satisfaction_class: RadrootsTransportSatisfactionClass,
    required_targets: Option<Vec<RadrootsTransportTargetFingerprint>>,
    remaining_required_targets: Option<Vec<RadrootsTransportTargetFingerprint>>,
}

impl PublishableRelays {
    fn targets_for_relay<'a>(
        &'a self,
        relay_url: &'a str,
    ) -> impl Iterator<Item = &'a PublishableRelay> + 'a {
        let canonical_relay_url = RadrootsTransportTarget::nostr_relay(relay_url)
            .ok()
            .map(|target| target.uri().as_str().to_owned());
        self.relays.iter().filter(move |target| {
            canonical_relay_url
                .as_deref()
                .is_some_and(|relay_url| target.relay_url == relay_url)
        })
    }

    fn satisfied_count_after_receipts(
        &self,
        target_receipts: &[RadrootsOutboxPublishTargetReceipt],
    ) -> usize {
        self.satisfied_count
            + target_receipts
                .iter()
                .filter(|receipt| {
                    receipt
                        .transport_status
                        .counts_as_satisfied(self.satisfaction_class)
                        && self
                            .required_targets
                            .as_ref()
                            .is_none_or(|required| required.contains(&receipt.endpoint_fingerprint))
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

fn unique_publishable_relay_urls(publishable: &PublishableRelays) -> Vec<&str> {
    let mut relay_urls = Vec::new();
    for target in &publishable.relays {
        let relay_url = target.relay_url.as_str();
        if !relay_urls.contains(&relay_url) {
            relay_urls.push(relay_url);
        }
    }
    relay_urls
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
                transport_status: relay_receipt
                    .outcome
                    .kind()
                    .transport_outcome_kind()
                    .target_status(),
                outcome: relay_receipt.outcome.clone(),
            });
        }
    }
    target_receipts
}

fn target_receipts_from_transport_receipts(
    publishable: &PublishableRelays,
    delivery: &RadrootsTransportDeliveryReceipt,
) -> Result<Vec<RadrootsOutboxPublishTargetReceipt>, RadrootsRelayTransportError> {
    let mut target_receipts = Vec::new();
    for receipt in delivery.target_receipts() {
        let Some(target) = publishable
            .relays
            .iter()
            .find(|target| target.endpoint_fingerprint == *receipt.target().fingerprint())
        else {
            continue;
        };
        target_receipts.push(RadrootsOutboxPublishTargetReceipt {
            delivery_target_id: target.delivery_target_id,
            endpoint_uri: target.relay_url.clone(),
            endpoint_fingerprint: target.endpoint_fingerprint.clone(),
            target_scope: target.target_scope.clone(),
            target_label: target.target_label.clone(),
            attempted: receipt.was_attempted(),
            transport_status: receipt.status(),
            outcome: relay_outcome_from_transport_outcome(receipt.outcome())?,
        });
    }
    Ok(target_receipts)
}

async fn complete_outbox_delivery_target(
    outbox: &RadrootsOutbox,
    claimed: &RadrootsOutboxClaimedEvent,
    receipt: &RadrootsOutboxPublishTargetReceipt,
    now_ms: i64,
) -> Result<(), RadrootsRelayTransportError> {
    match receipt.transport_status {
        RadrootsTransportDeliveryTargetStatus::Pending => {
            return Err(RadrootsRelayTransportError::TransportContract(
                "outbox publish receipt cannot complete a target with pending transport status"
                    .to_owned(),
            ));
        }
        RadrootsTransportDeliveryTargetStatus::Accepted => {
            outbox
                .mark_delivery_target_accepted(
                    claimed.outbox_event_id,
                    claimed.claim_token.as_str(),
                    receipt.delivery_target_id,
                    now_ms,
                )
                .await?;
        }
        RadrootsTransportDeliveryTargetStatus::Delivered => {
            outbox
                .mark_delivery_target_delivered(
                    claimed.outbox_event_id,
                    claimed.claim_token.as_str(),
                    receipt.delivery_target_id,
                    now_ms,
                )
                .await?;
        }
        RadrootsTransportDeliveryTargetStatus::Forwarded => {
            outbox
                .mark_delivery_target_forwarded(
                    claimed.outbox_event_id,
                    claimed.claim_token.as_str(),
                    receipt.delivery_target_id,
                    now_ms,
                )
                .await?;
        }
        RadrootsTransportDeliveryTargetStatus::StoredByGateway => {
            outbox
                .mark_delivery_target_stored_by_gateway(
                    claimed.outbox_event_id,
                    claimed.claim_token.as_str(),
                    receipt.delivery_target_id,
                    now_ms,
                )
                .await?;
        }
        RadrootsTransportDeliveryTargetStatus::Seen => {
            outbox
                .mark_delivery_target_seen(
                    claimed.outbox_event_id,
                    claimed.claim_token.as_str(),
                    receipt.delivery_target_id,
                    now_ms,
                )
                .await?;
        }
        RadrootsTransportDeliveryTargetStatus::DeferredUntilImplemented => {
            outbox
                .mark_delivery_target_deferred_until_implemented(
                    claimed.outbox_event_id,
                    claimed.claim_token.as_str(),
                    receipt.delivery_target_id,
                    receipt
                        .outcome
                        .message()
                        .unwrap_or("relay publish deferred until implemented"),
                    now_ms,
                )
                .await?;
        }
        RadrootsTransportDeliveryTargetStatus::SkippedPolicyDenied => {
            outbox
                .mark_delivery_target_skipped_policy_denied(
                    claimed.outbox_event_id,
                    claimed.claim_token.as_str(),
                    receipt.delivery_target_id,
                    receipt
                        .outcome
                        .message()
                        .unwrap_or("relay publish skipped by policy"),
                    now_ms,
                )
                .await?;
        }
        RadrootsTransportDeliveryTargetStatus::FailedRetryable => {
            outbox
                .mark_delivery_target_failed_retryable(
                    claimed.outbox_event_id,
                    claimed.claim_token.as_str(),
                    receipt.delivery_target_id,
                    receipt
                        .outcome
                        .message()
                        .unwrap_or("relay publish retryable"),
                    now_ms,
                )
                .await?;
        }
        RadrootsTransportDeliveryTargetStatus::FailedTerminal => {
            outbox
                .mark_delivery_target_failed_terminal(
                    claimed.outbox_event_id,
                    claimed.claim_token.as_str(),
                    receipt.delivery_target_id,
                    receipt
                        .outcome
                        .message()
                        .unwrap_or("relay publish terminal"),
                    now_ms,
                )
                .await?;
        }
    }
    Ok(())
}

fn relay_receipts_from_transport_receipts(
    delivery: &RadrootsTransportDeliveryReceipt,
) -> Result<Vec<RadrootsRelayPublishRelayReceipt>, RadrootsRelayTransportError> {
    let mut relay_receipts: Vec<RadrootsRelayPublishRelayReceipt> = Vec::new();
    for receipt in delivery.target_receipts() {
        let outcome = relay_outcome_from_transport_outcome(receipt.outcome())?;
        let relay_receipt = if receipt.was_attempted() {
            RadrootsRelayPublishRelayReceipt::attempted(receipt.target().uri().as_str(), outcome)
        } else {
            RadrootsRelayPublishRelayReceipt::skipped(receipt.target().uri().as_str(), outcome)
        };
        if let Some(existing) = relay_receipts
            .iter()
            .find(|existing| existing.relay_url == relay_receipt.relay_url)
        {
            if existing != &relay_receipt {
                return Err(
                    RadrootsRelayTransportError::ConflictingTransportReceiptRelayUrl {
                        url: relay_receipt.relay_url,
                    },
                );
            }
            continue;
        }
        relay_receipts.push(relay_receipt);
    }
    Ok(relay_receipts)
}

fn relay_outcome_from_transport_outcome(
    outcome: &RadrootsTransportOutcome,
) -> Result<RadrootsRelayOutcome, RadrootsRelayTransportError> {
    let kind = outcome
        .code()
        .and_then(relay_outcome_kind_from_code)
        .unwrap_or_else(|| relay_outcome_kind_from_transport_outcome(outcome.kind()));
    RadrootsRelayOutcome::try_new(kind, outcome.message().map(str::to_owned))
}

fn relay_outcome_kind_from_code(code: &str) -> Option<crate::RadrootsRelayOutcomeKind> {
    Some(match code {
        "accepted" => crate::RadrootsRelayOutcomeKind::Accepted,
        "duplicate_accepted" => crate::RadrootsRelayOutcomeKind::DuplicateAccepted,
        "blocked" => crate::RadrootsRelayOutcomeKind::Blocked,
        "rate_limited" => crate::RadrootsRelayOutcomeKind::RateLimited,
        "invalid" => crate::RadrootsRelayOutcomeKind::Invalid,
        "pow_required" => crate::RadrootsRelayOutcomeKind::PowRequired,
        "restricted" => crate::RadrootsRelayOutcomeKind::Restricted,
        "auth_required" => crate::RadrootsRelayOutcomeKind::AuthRequired,
        "muted" => crate::RadrootsRelayOutcomeKind::Muted,
        "unsupported" => crate::RadrootsRelayOutcomeKind::Unsupported,
        "payment_required" => crate::RadrootsRelayOutcomeKind::PaymentRequired,
        "error" => crate::RadrootsRelayOutcomeKind::Error,
        "timeout" => crate::RadrootsRelayOutcomeKind::Timeout,
        "connection_failed" => crate::RadrootsRelayOutcomeKind::ConnectionFailed,
        "relay_url_rejected" => crate::RadrootsRelayOutcomeKind::RelayUrlRejected,
        "skipped_already_accepted" => crate::RadrootsRelayOutcomeKind::SkippedAlreadyAccepted,
        "unknown" => crate::RadrootsRelayOutcomeKind::Unknown,
        _ => return None,
    })
}

fn relay_outcome_kind_from_transport_outcome(
    kind: RadrootsTransportOutcomeKind,
) -> crate::RadrootsRelayOutcomeKind {
    match kind {
        RadrootsTransportOutcomeKind::Accepted => crate::RadrootsRelayOutcomeKind::Accepted,
        RadrootsTransportOutcomeKind::DuplicateAccepted => {
            crate::RadrootsRelayOutcomeKind::DuplicateAccepted
        }
        RadrootsTransportOutcomeKind::Rejected => crate::RadrootsRelayOutcomeKind::Invalid,
        RadrootsTransportOutcomeKind::RouteUnavailable => {
            crate::RadrootsRelayOutcomeKind::RelayUrlRejected
        }
        RadrootsTransportOutcomeKind::PolicyDenied => crate::RadrootsRelayOutcomeKind::Restricted,
        RadrootsTransportOutcomeKind::Timeout => crate::RadrootsRelayOutcomeKind::Timeout,
        RadrootsTransportOutcomeKind::ConnectionFailed => {
            crate::RadrootsRelayOutcomeKind::ConnectionFailed
        }
        RadrootsTransportOutcomeKind::TransportUnavailable => {
            crate::RadrootsRelayOutcomeKind::Error
        }
        RadrootsTransportOutcomeKind::PayloadTooLarge => crate::RadrootsRelayOutcomeKind::Invalid,
        RadrootsTransportOutcomeKind::Delivered
        | RadrootsTransportOutcomeKind::Forwarded
        | RadrootsTransportOutcomeKind::StoredByGateway
        | RadrootsTransportOutcomeKind::Seen => crate::RadrootsRelayOutcomeKind::Accepted,
        RadrootsTransportOutcomeKind::DeferredUntilImplemented => {
            crate::RadrootsRelayOutcomeKind::Unsupported
        }
    }
}

fn publishable_transport_targets(
    publishable: &PublishableRelays,
) -> Result<Vec<RadrootsTransportTarget>, RadrootsRelayTransportError> {
    publishable
        .relays
        .iter()
        .map(|relay| {
            RadrootsTransportTarget::nostr_relay_with_metadata(
                relay.relay_url.as_str(),
                relay
                    .target_scope
                    .as_deref()
                    .map(radroots_transport::RadrootsTransportMeshScopeId::parse)
                    .transpose()
                    .map_err(transport_error_to_relay_error)?,
                relay
                    .target_label
                    .as_deref()
                    .map(radroots_transport::RadrootsTransportTargetLabel::parse)
                    .transpose()
                    .map_err(transport_error_to_relay_error)?,
            )
            .map_err(transport_error_to_relay_error)
        })
        .collect()
}

fn transport_satisfaction_policy_for_publishable(
    publishable: &PublishableRelays,
) -> RadrootsTransportSatisfactionPolicy {
    satisfaction_policy_for_remaining_count(
        publishable.satisfaction_class,
        publishable.remaining_satisfaction_count,
        publishable.relays.len(),
        publishable.remaining_required_targets.as_deref(),
    )
}

fn transport_error_to_relay_error(error: RadrootsTransportError) -> RadrootsRelayTransportError {
    match error {
        RadrootsTransportError::UnsupportedOperation
        | RadrootsTransportError::EmptyTransportKind
        | RadrootsTransportError::InvalidTransportKind
        | RadrootsTransportError::EmptyTargetScope
        | RadrootsTransportError::InvalidTargetScope
        | RadrootsTransportError::EmptyTargetLabel
        | RadrootsTransportError::InvalidTargetLabel
        | RadrootsTransportError::InvalidSatisfactionPolicy
        | RadrootsTransportError::EmptyRequiredTargetSet
        | RadrootsTransportError::DuplicateRequiredTargetFingerprint
        | RadrootsTransportError::RequiredTargetNotRequested
        | RadrootsTransportError::EmptyDeliveryRequestId
        | RadrootsTransportError::InvalidDeliveryRequestId
        | RadrootsTransportError::EmptyFetchRequestId
        | RadrootsTransportError::InvalidFetchRequestId
        | RadrootsTransportError::InvalidPayloadSignature
        | RadrootsTransportError::InvalidDeliveryTimestamp => {
            RadrootsRelayTransportError::Transport(error.to_string())
        }
        RadrootsTransportError::EmptyTargetUri
        | RadrootsTransportError::InvalidTargetUri
        | RadrootsTransportError::ResourceLimitExceeded { .. }
        | RadrootsTransportError::EmptyTargetSet
        | RadrootsTransportError::DuplicateTargetFingerprint
        | RadrootsTransportError::InvalidTargetFingerprint
        | RadrootsTransportError::UnexpectedDeliveryTargetReceipt
        | RadrootsTransportError::DuplicateDeliveryTargetReceipt
        | RadrootsTransportError::MissingDeliveryTargetReceipt
        | RadrootsTransportError::DeliveryTargetReceiptStatusMismatch
        | RadrootsTransportError::DeliveryTargetReceiptAttemptMismatch
        | RadrootsTransportError::TransportOutcomeStatusMismatch
        | RadrootsTransportError::DeliveryReceiptRequestIdMismatch
        | RadrootsTransportError::DeliveryReceiptTargetSetMismatch
        | RadrootsTransportError::UnexpectedFetchTargetReceipt
        | RadrootsTransportError::DuplicateFetchTargetReceipt
        | RadrootsTransportError::MissingFetchTargetReceipt
        | RadrootsTransportError::FetchReceiptRequestIdMismatch
        | RadrootsTransportError::FetchReceiptTargetSetMismatch => {
            RadrootsRelayTransportError::TransportContract(error.to_string())
        }
        RadrootsTransportError::EmptyPayloadId
        | RadrootsTransportError::InvalidPayloadId
        | RadrootsTransportError::EmptyPayloadLabel
        | RadrootsTransportError::InvalidPayloadLabel
        | RadrootsTransportError::EmptyPayloadBytes
        | RadrootsTransportError::InvalidPayloadBytes
        | RadrootsTransportError::InvalidPayloadDigest
        | RadrootsTransportError::PayloadDigestMismatch => {
            RadrootsRelayTransportError::NostrEventJson(error.to_string())
        }
    }
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
    let required_targets = plan
        .satisfaction_policy
        .required_target_fingerprints()
        .map(<[_]>::to_vec);
    let active_targets = targets
        .iter()
        .filter(|target| target.delivery_plan_id == active_delivery_plan_id)
        .collect::<Vec<_>>();
    let satisfaction_class = plan
        .satisfaction_policy
        .target_satisfaction_class()
        .unwrap_or(RadrootsTransportSatisfactionClass::Accepted);
    let satisfied_count = active_targets
        .iter()
        .filter(|target| {
            required_targets
                .as_ref()
                .is_none_or(|required| required.contains(&target.endpoint_fingerprint))
                && target
                    .status
                    .counts_as_transport_satisfaction(satisfaction_class)
        })
        .count();
    let remaining_satisfaction_count =
        (plan.required_success_count as usize).saturating_sub(satisfied_count);
    let remaining_required_targets = required_targets.as_ref().map(|required_targets| {
        required_targets
            .iter()
            .filter(|required| {
                !active_targets.iter().any(|target| {
                    target.endpoint_fingerprint == **required
                        && target
                            .status
                            .counts_as_transport_satisfaction(satisfaction_class)
                })
            })
            .cloned()
            .collect::<Vec<_>>()
    });
    let mut relays = Vec::new();
    let mut accepted_count = 0usize;
    for target in &active_targets {
        if !is_nostr_target(target) {
            continue;
        }
        let required_for_satisfaction = required_targets
            .as_ref()
            .is_some_and(|required| required.contains(&target.endpoint_fingerprint));
        if counts_as_accepted_for_plan(
            target.status,
            required_targets.is_some(),
            required_for_satisfaction,
        ) {
            accepted_count += 1;
        }
        let can_contribute_to_satisfaction =
            required_targets.is_none() || required_for_satisfaction;
        if remaining_satisfaction_count > 0
            && can_contribute_to_satisfaction
            && is_publishable_delivery_status(target.status, republish_accepted_relays)
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
                || !is_publishable_delivery_status(target.status, republish_accepted_relays)
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
        satisfied_count,
        satisfaction_required_count,
        remaining_satisfaction_count,
        satisfaction_class,
        required_targets,
        remaining_required_targets,
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

fn counts_as_accepted_for_plan(
    status: RadrootsOutboxDeliveryTargetStatus,
    has_required_targets: bool,
    required_for_satisfaction: bool,
) -> bool {
    status.counts_as_transport_satisfaction(RadrootsTransportSatisfactionClass::Accepted)
        && (!has_required_targets || required_for_satisfaction)
}

fn is_publishable_delivery_status(
    status: RadrootsOutboxDeliveryTargetStatus,
    republish_accepted_relays: bool,
) -> bool {
    status.is_ready_for_attempt()
        || (republish_accepted_relays && status == RadrootsOutboxDeliveryTargetStatus::Accepted)
}

fn is_nostr_target(target: &RadrootsOutboxDeliveryTargetRecord) -> bool {
    target.transport_kind == RadrootsTransportKind::Nostr
}

fn satisfaction_policy_for_remaining_count(
    satisfaction_class: RadrootsTransportSatisfactionClass,
    remaining_satisfaction_count: usize,
    target_count: usize,
    exact_required_targets: Option<&[RadrootsTransportTargetFingerprint]>,
) -> RadrootsTransportSatisfactionPolicy {
    if let Some(targets) = exact_required_targets {
        return RadrootsTransportSatisfactionPolicy::required_targets(
            satisfaction_class,
            targets.to_vec(),
        )
        .expect("remaining required targets retain a validated nonempty unique set");
    }
    if remaining_satisfaction_count >= target_count {
        return RadrootsTransportSatisfactionPolicy::all(satisfaction_class);
    }
    if remaining_satisfaction_count == 0 {
        return RadrootsTransportSatisfactionPolicy::no_wait();
    }
    if remaining_satisfaction_count == 1 {
        return RadrootsTransportSatisfactionPolicy::any(satisfaction_class);
    }
    let Ok(count) = u16::try_from(remaining_satisfaction_count) else {
        return RadrootsTransportSatisfactionPolicy::all(satisfaction_class);
    };
    RadrootsTransportSatisfactionPolicy::quorum(satisfaction_class, count)
        .expect("remaining quorum is bounded by the validated target set")
}

async fn ingest_publish_observation(
    event_store: &RadrootsEventStore,
    signed_event: &RadrootsVerifiedSignedEvent,
    relay_url: &str,
    observed_at_ms: i64,
) -> Result<(), RadrootsRelayTransportError> {
    let observation = RadrootsTransportObservation::new(
        RadrootsTransportKind::Nostr,
        relay_url,
        RadrootsTransportObservationType::PublishAck,
        observed_at_ms,
    )?;
    let ingest = RadrootsEventIngest::from_signed_event(
        signed_event.signed_event().clone(),
        observed_at_ms,
    )?
    .with_observation(observation);
    event_store.ingest_event(ingest).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        PublishableRelay, PublishableRelays, RadrootsOutboxDeliveryTargetStatus,
        adapter_transport_failure_receipt, counts_as_accepted_for_plan,
        is_publishable_delivery_status, publishable_transport_targets,
        relay_outcome_from_transport_outcome, relay_outcome_kind_from_code,
        relay_outcome_kind_from_transport_outcome, relay_receipts_from_transport_receipts,
        satisfaction_policy_for_remaining_count, target_receipts_from_relay_receipts,
        target_receipts_from_transport_receipts, transport_error_to_relay_error,
        transport_satisfaction_policy_for_publishable,
    };
    use crate::{
        RadrootsRelayOutcome, RadrootsRelayOutcomeKind, RadrootsRelayPublishRelayReceipt,
        RadrootsRelayTransportError,
    };
    use radroots_transport::{
        RadrootsTransportDeliveryReceipt, RadrootsTransportDeliveryTargetStatus,
        RadrootsTransportError, RadrootsTransportMeshScopeId, RadrootsTransportOutcome,
        RadrootsTransportOutcomeKind, RadrootsTransportSatisfactionClass,
        RadrootsTransportSatisfactionPolicy, RadrootsTransportTarget,
        RadrootsTransportTargetReceipt, RadrootsTransportTargetSet,
    };

    #[test]
    fn internal_outbox_publish_helpers_cover_policy_edges() {
        assert_eq!(
            satisfaction_policy_for_remaining_count(
                RadrootsTransportSatisfactionClass::Accepted,
                2,
                2,
                None
            ),
            RadrootsTransportSatisfactionPolicy::all_accepted()
        );
        assert_eq!(
            satisfaction_policy_for_remaining_count(
                RadrootsTransportSatisfactionClass::Accepted,
                1,
                3,
                None
            ),
            RadrootsTransportSatisfactionPolicy::any_accepted()
        );
        assert_eq!(
            satisfaction_policy_for_remaining_count(
                RadrootsTransportSatisfactionClass::Delivered,
                2,
                3,
                None
            ),
            RadrootsTransportSatisfactionPolicy::quorum_delivered(2).expect("valid quorum")
        );
        let required_target =
            RadrootsTransportTarget::nostr_relay("wss://relay.example").expect("required target");
        assert_eq!(
            satisfaction_policy_for_remaining_count(
                RadrootsTransportSatisfactionClass::Delivered,
                1,
                3,
                Some(core::slice::from_ref(required_target.fingerprint()))
            ),
            RadrootsTransportSatisfactionPolicy::required_targets(
                RadrootsTransportSatisfactionClass::Delivered,
                vec![required_target.fingerprint().clone()]
            )
            .expect("required target policy")
        );
        assert_eq!(
            satisfaction_policy_for_remaining_count(
                RadrootsTransportSatisfactionClass::Accepted,
                usize::from(u16::MAX) + 1,
                usize::from(u16::MAX) + 2,
                None,
            ),
            RadrootsTransportSatisfactionPolicy::all(RadrootsTransportSatisfactionClass::Accepted,)
        );
        assert_eq!(
            satisfaction_policy_for_remaining_count(
                RadrootsTransportSatisfactionClass::Accepted,
                0,
                3,
                None,
            ),
            RadrootsTransportSatisfactionPolicy::no_wait()
        );

        assert!(counts_as_accepted_for_plan(
            RadrootsOutboxDeliveryTargetStatus::Accepted,
            false,
            false,
        ));
        assert!(counts_as_accepted_for_plan(
            RadrootsOutboxDeliveryTargetStatus::Accepted,
            true,
            true,
        ));
        assert!(!counts_as_accepted_for_plan(
            RadrootsOutboxDeliveryTargetStatus::Accepted,
            true,
            false,
        ));
        assert!(!counts_as_accepted_for_plan(
            RadrootsOutboxDeliveryTargetStatus::FailedRetryable,
            false,
            false,
        ));

        assert!(is_publishable_delivery_status(
            RadrootsOutboxDeliveryTargetStatus::Pending,
            false,
        ));
        assert!(is_publishable_delivery_status(
            RadrootsOutboxDeliveryTargetStatus::FailedRetryable,
            false,
        ));
        assert!(is_publishable_delivery_status(
            RadrootsOutboxDeliveryTargetStatus::Accepted,
            true,
        ));
        assert!(!is_publishable_delivery_status(
            RadrootsOutboxDeliveryTargetStatus::Accepted,
            false,
        ));
        assert!(!is_publishable_delivery_status(
            RadrootsOutboxDeliveryTargetStatus::FailedTerminal,
            true,
        ));
    }

    #[test]
    fn outbox_publish_satisfaction_counts_use_active_transport_class() {
        let target = RadrootsTransportTarget::nostr_relay("wss://relay.example").expect("target");
        let publishable = PublishableRelays {
            active_delivery_plan_id: 7,
            relays: vec![PublishableRelay {
                delivery_target_id: 11,
                relay_url: target.uri().as_str().to_owned(),
                endpoint_fingerprint: target.fingerprint().clone(),
                target_scope: None,
                target_label: None,
            }],
            accepted_count: 0,
            satisfied_count: 0,
            satisfaction_required_count: 1,
            remaining_satisfaction_count: 1,
            satisfaction_class: RadrootsTransportSatisfactionClass::Delivered,
            required_targets: None,
            remaining_required_targets: None,
        };

        let accepted_relay_receipts = target_receipts_from_relay_receipts(
            &publishable,
            &[RadrootsRelayPublishRelayReceipt::attempted(
                target.uri().as_str(),
                RadrootsRelayOutcome::accepted(),
            )],
        );
        assert_eq!(
            accepted_relay_receipts[0].transport_status,
            RadrootsTransportDeliveryTargetStatus::Accepted
        );
        assert_eq!(
            publishable.satisfied_count_after_receipts(&accepted_relay_receipts),
            0
        );

        let delivery = RadrootsTransportDeliveryReceipt::new(
            "request-1",
            RadrootsTransportTargetSet::new(vec![target.clone()]).expect("target set"),
            vec![RadrootsTransportTargetReceipt::new(
                target,
                RadrootsTransportOutcome::new(RadrootsTransportOutcomeKind::Delivered),
            )],
        )
        .expect("delivery receipt");
        let delivered_transport_receipts =
            target_receipts_from_transport_receipts(&publishable, &delivery)
                .expect("bounded target receipts");
        assert_eq!(
            delivered_transport_receipts[0].transport_status,
            RadrootsTransportDeliveryTargetStatus::Delivered
        );
        assert_eq!(
            publishable.satisfied_count_after_receipts(&delivered_transport_receipts),
            1
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
        )
        .expect("bounded adapter failure receipt");

        assert_eq!(receipt.event_id, "event-1");
        assert_eq!(receipt.attempted_count, 2);
        assert_eq!(receipt.retryable_count, 2);
        assert_eq!(receipt.terminal_count, 0);
        assert_eq!(receipt.quorum, 2);
        assert!(!receipt.quorum_met);
        assert!(receipt.relays.iter().all(|relay| relay.attempted));
    }

    #[test]
    fn transport_outcomes_preserve_relay_semantics() {
        let cases = [
            (
                RadrootsTransportOutcomeKind::Accepted,
                RadrootsRelayOutcomeKind::Accepted,
            ),
            (
                RadrootsTransportOutcomeKind::DuplicateAccepted,
                RadrootsRelayOutcomeKind::DuplicateAccepted,
            ),
            (
                RadrootsTransportOutcomeKind::Delivered,
                RadrootsRelayOutcomeKind::Accepted,
            ),
            (
                RadrootsTransportOutcomeKind::Forwarded,
                RadrootsRelayOutcomeKind::Accepted,
            ),
            (
                RadrootsTransportOutcomeKind::StoredByGateway,
                RadrootsRelayOutcomeKind::Accepted,
            ),
            (
                RadrootsTransportOutcomeKind::Seen,
                RadrootsRelayOutcomeKind::Accepted,
            ),
            (
                RadrootsTransportOutcomeKind::DeferredUntilImplemented,
                RadrootsRelayOutcomeKind::Unsupported,
            ),
            (
                RadrootsTransportOutcomeKind::Rejected,
                RadrootsRelayOutcomeKind::Invalid,
            ),
            (
                RadrootsTransportOutcomeKind::RouteUnavailable,
                RadrootsRelayOutcomeKind::RelayUrlRejected,
            ),
            (
                RadrootsTransportOutcomeKind::PayloadTooLarge,
                RadrootsRelayOutcomeKind::Invalid,
            ),
            (
                RadrootsTransportOutcomeKind::PolicyDenied,
                RadrootsRelayOutcomeKind::Restricted,
            ),
            (
                RadrootsTransportOutcomeKind::Timeout,
                RadrootsRelayOutcomeKind::Timeout,
            ),
            (
                RadrootsTransportOutcomeKind::ConnectionFailed,
                RadrootsRelayOutcomeKind::ConnectionFailed,
            ),
            (
                RadrootsTransportOutcomeKind::TransportUnavailable,
                RadrootsRelayOutcomeKind::Error,
            ),
        ];
        for (transport_kind, relay_kind) in cases {
            assert_eq!(
                relay_outcome_kind_from_transport_outcome(transport_kind),
                relay_kind
            );
            let outcome = RadrootsTransportOutcome::new(transport_kind)
                .try_with_message(format!("{transport_kind:?}"))
                .expect("bounded test outcome message");
            let relay_outcome =
                relay_outcome_from_transport_outcome(&outcome).expect("bounded relay outcome");
            assert_eq!(relay_outcome.kind(), relay_kind);
            assert_eq!(relay_outcome.message(), outcome.message());
        }

        let code_cases = [
            ("accepted", RadrootsRelayOutcomeKind::Accepted),
            (
                "duplicate_accepted",
                RadrootsRelayOutcomeKind::DuplicateAccepted,
            ),
            ("blocked", RadrootsRelayOutcomeKind::Blocked),
            ("rate_limited", RadrootsRelayOutcomeKind::RateLimited),
            ("invalid", RadrootsRelayOutcomeKind::Invalid),
            ("pow_required", RadrootsRelayOutcomeKind::PowRequired),
            ("restricted", RadrootsRelayOutcomeKind::Restricted),
            ("auth_required", RadrootsRelayOutcomeKind::AuthRequired),
            ("muted", RadrootsRelayOutcomeKind::Muted),
            ("unsupported", RadrootsRelayOutcomeKind::Unsupported),
            (
                "payment_required",
                RadrootsRelayOutcomeKind::PaymentRequired,
            ),
            ("error", RadrootsRelayOutcomeKind::Error),
            ("timeout", RadrootsRelayOutcomeKind::Timeout),
            (
                "connection_failed",
                RadrootsRelayOutcomeKind::ConnectionFailed,
            ),
            (
                "relay_url_rejected",
                RadrootsRelayOutcomeKind::RelayUrlRejected,
            ),
            (
                "skipped_already_accepted",
                RadrootsRelayOutcomeKind::SkippedAlreadyAccepted,
            ),
            ("unknown", RadrootsRelayOutcomeKind::Unknown),
        ];
        for (code, relay_kind) in code_cases {
            assert_eq!(relay_outcome_kind_from_code(code), Some(relay_kind));
            assert_eq!(
                relay_outcome_from_transport_outcome(
                    &RadrootsTransportOutcome::new(RadrootsTransportOutcomeKind::Rejected)
                        .try_with_code(code)
                        .expect("bounded test outcome code")
                )
                .expect("bounded relay outcome")
                .kind(),
                relay_kind
            );
        }
        assert_eq!(relay_outcome_kind_from_code("unrecognized"), None);
        assert_eq!(
            relay_outcome_from_transport_outcome(
                &RadrootsTransportOutcome::new(RadrootsTransportOutcomeKind::Seen)
                    .try_with_code("unrecognized")
                    .expect("bounded test outcome code")
            )
            .expect("bounded relay outcome")
            .kind(),
            RadrootsRelayOutcomeKind::Accepted
        );

        for kind in [
            RadrootsRelayOutcomeKind::RateLimited,
            RadrootsRelayOutcomeKind::Error,
            RadrootsRelayOutcomeKind::Unknown,
        ] {
            assert_eq!(
                kind.transport_outcome_kind(),
                RadrootsTransportOutcomeKind::TransportUnavailable
            );
        }
    }

    #[test]
    fn transport_target_and_error_adapters_preserve_contract_categories() {
        let target = RadrootsTransportTarget::nostr_relay("wss://relay.example").expect("target");
        let publishable = PublishableRelays {
            active_delivery_plan_id: 7,
            relays: vec![PublishableRelay {
                delivery_target_id: 11,
                relay_url: target.uri().as_str().to_owned(),
                endpoint_fingerprint: target.fingerprint().clone(),
                target_scope: Some("foodshed.west".to_owned()),
                target_label: Some("primary relay".to_owned()),
            }],
            accepted_count: 0,
            satisfied_count: 0,
            satisfaction_required_count: 1,
            remaining_satisfaction_count: 1,
            satisfaction_class: RadrootsTransportSatisfactionClass::Accepted,
            required_targets: None,
            remaining_required_targets: None,
        };
        let targets = publishable_transport_targets(&publishable).expect("transport targets");
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].scope().expect("scope").as_str(), "foodshed.west");
        assert_eq!(targets[0].label().expect("label").as_str(), "primary relay");
        assert_eq!(
            transport_satisfaction_policy_for_publishable(&publishable),
            RadrootsTransportSatisfactionPolicy::all_accepted()
        );

        let mut invalid = publishable;
        invalid.relays[0].target_scope = Some("bad scope".to_owned());
        assert!(matches!(
            publishable_transport_targets(&invalid),
            Err(RadrootsRelayTransportError::Transport(_))
        ));
        invalid.relays[0].target_scope = Some("foodshed.west".to_owned());
        invalid.relays[0].target_label = Some("bad\0label".to_owned());
        assert!(matches!(
            publishable_transport_targets(&invalid),
            Err(RadrootsRelayTransportError::Transport(_))
        ));
        invalid.relays[0].target_label = Some("primary relay".to_owned());
        invalid.relays[0].relay_url = "not-a-relay".to_owned();
        assert!(matches!(
            publishable_transport_targets(&invalid),
            Err(RadrootsRelayTransportError::TransportContract(_))
        ));

        let generic_errors = [
            RadrootsTransportError::UnsupportedOperation,
            RadrootsTransportError::EmptyTransportKind,
            RadrootsTransportError::InvalidTransportKind,
            RadrootsTransportError::EmptyTargetScope,
            RadrootsTransportError::InvalidTargetScope,
            RadrootsTransportError::EmptyTargetLabel,
            RadrootsTransportError::InvalidTargetLabel,
            RadrootsTransportError::InvalidSatisfactionPolicy,
            RadrootsTransportError::EmptyRequiredTargetSet,
            RadrootsTransportError::DuplicateRequiredTargetFingerprint,
        ];
        for error in generic_errors {
            assert!(matches!(
                transport_error_to_relay_error(error),
                RadrootsRelayTransportError::Transport(_)
            ));
        }
        let target_errors = [
            RadrootsTransportError::EmptyTargetUri,
            RadrootsTransportError::InvalidTargetUri,
            RadrootsTransportError::EmptyTargetSet,
            RadrootsTransportError::DuplicateTargetFingerprint,
            RadrootsTransportError::InvalidTargetFingerprint,
        ];
        for error in target_errors {
            assert!(matches!(
                transport_error_to_relay_error(error),
                RadrootsRelayTransportError::TransportContract(_)
            ));
        }
        assert!(matches!(
            transport_error_to_relay_error(RadrootsTransportError::ResourceLimitExceeded {
                field: "fixture",
                max: 1,
                actual: 2,
            }),
            RadrootsRelayTransportError::TransportContract(_)
        ));
        let payload_errors = [
            RadrootsTransportError::EmptyPayloadId,
            RadrootsTransportError::InvalidPayloadId,
            RadrootsTransportError::EmptyPayloadLabel,
            RadrootsTransportError::InvalidPayloadLabel,
            RadrootsTransportError::EmptyPayloadBytes,
            RadrootsTransportError::InvalidPayloadBytes,
            RadrootsTransportError::InvalidPayloadDigest,
            RadrootsTransportError::PayloadDigestMismatch,
        ];
        for error in payload_errors {
            assert!(matches!(
                transport_error_to_relay_error(error),
                RadrootsRelayTransportError::NostrEventJson(_)
            ));
        }

        let unknown =
            RadrootsTransportTarget::nostr_relay("wss://unknown.example").expect("unknown target");
        let delivery = RadrootsTransportDeliveryReceipt::new(
            "unknown",
            RadrootsTransportTargetSet::new(vec![unknown.clone()]).expect("target set"),
            vec![RadrootsTransportTargetReceipt::new(
                unknown,
                RadrootsTransportOutcome::new(RadrootsTransportOutcomeKind::Accepted),
            )],
        )
        .expect("delivery receipt");
        assert!(
            target_receipts_from_transport_receipts(&invalid, &delivery)
                .expect("bounded target receipts")
                .is_empty()
        );
        assert_eq!(
            relay_receipts_from_transport_receipts(&delivery)
                .expect("relay receipts")
                .len(),
            1
        );

        let west = RadrootsTransportTarget::nostr_relay_with_metadata(
            "wss://scoped.example",
            Some(RadrootsTransportMeshScopeId::parse("foodshed.west").expect("west scope")),
            None,
        )
        .expect("west target");
        let east = RadrootsTransportTarget::nostr_relay_with_metadata(
            "wss://scoped.example",
            Some(RadrootsTransportMeshScopeId::parse("foodshed.east").expect("east scope")),
            None,
        )
        .expect("east target");
        let scoped_relay_uri = west.uri().as_str().to_owned();
        let conflicting = RadrootsTransportDeliveryReceipt::new(
            "conflicting",
            RadrootsTransportTargetSet::new(vec![west.clone(), east.clone()])
                .expect("scoped target set"),
            vec![
                RadrootsTransportTargetReceipt::new(
                    west,
                    RadrootsTransportOutcome::new(RadrootsTransportOutcomeKind::Accepted),
                ),
                RadrootsTransportTargetReceipt::new(
                    east,
                    RadrootsTransportOutcome::new(RadrootsTransportOutcomeKind::Timeout),
                ),
            ],
        )
        .expect("conflicting delivery receipt");
        assert!(matches!(
            relay_receipts_from_transport_receipts(&conflicting),
            Err(RadrootsRelayTransportError::ConflictingTransportReceiptRelayUrl { url })
                if url == scoped_relay_uri
        ));
    }
}
