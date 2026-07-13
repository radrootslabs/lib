#![forbid(unsafe_code)]

use crate::{RadrootsRelayOutcome, RadrootsRelayTargetSet, RadrootsRelayTransportError};
#[cfg(feature = "client")]
use core::time::Duration;
use futures::future::BoxFuture;
use radroots_event::draft::{RadrootsSignedEvent, RadrootsSignedEventParts};
use radroots_transport::{
    RadrootsTransport, RadrootsTransportCapabilities, RadrootsTransportDeliveryReceipt,
    RadrootsTransportDeliveryRequest, RadrootsTransportError, RadrootsTransportFetchReceipt,
    RadrootsTransportFetchRequest, RadrootsTransportFuture, RadrootsTransportImplementationState,
    RadrootsTransportKind, RadrootsTransportOutcome, RadrootsTransportOutcomeKind,
    RadrootsTransportPayload, RadrootsTransportSatisfactionPolicy, RadrootsTransportStatus,
    RadrootsTransportTarget, RadrootsTransportTargetReceipt,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, PoisonError};

#[cfg(feature = "client")]
use crate::RadrootsRelayOutcomeKind;
#[cfg(feature = "client")]
use nostr::JsonUtil;
#[cfg(feature = "client")]
use radroots_nostr::prelude::{RadrootsNostrClient, RadrootsNostrEvent};

#[cfg(feature = "client")]
const RELAY_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsRelayPublishRequest {
    pub signed_event: RadrootsSignedEvent,
    pub targets: RadrootsRelayTargetSet,
    pub satisfaction_policy: RadrootsTransportSatisfactionPolicy,
    pub idempotency_key: Option<String>,
    pub now_ms: i64,
}

impl RadrootsRelayPublishRequest {
    pub fn new(
        signed_event: RadrootsSignedEvent,
        targets: RadrootsRelayTargetSet,
        now_ms: i64,
    ) -> Self {
        Self {
            signed_event,
            targets,
            satisfaction_policy: RadrootsTransportSatisfactionPolicy::all_accepted(),
            idempotency_key: None,
            now_ms,
        }
    }

    pub fn with_satisfaction_policy(
        mut self,
        satisfaction_policy: RadrootsTransportSatisfactionPolicy,
    ) -> Self {
        self.satisfaction_policy = satisfaction_policy;
        self
    }

    pub fn with_idempotency_key(mut self, idempotency_key: impl Into<String>) -> Self {
        self.idempotency_key = Some(idempotency_key.into());
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RadrootsRelayPublishRelayReceipt {
    pub relay_url: String,
    pub outcome: RadrootsRelayOutcome,
    pub attempted: bool,
}

impl RadrootsRelayPublishRelayReceipt {
    pub fn attempted(relay_url: impl Into<String>, outcome: RadrootsRelayOutcome) -> Self {
        Self {
            relay_url: relay_url.into(),
            outcome,
            attempted: true,
        }
    }

    pub fn skipped(relay_url: impl Into<String>, outcome: RadrootsRelayOutcome) -> Self {
        Self {
            relay_url: relay_url.into(),
            outcome,
            attempted: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RadrootsRelayPublishReceipt {
    pub event_id: String,
    pub attempted_count: usize,
    pub accepted_count: usize,
    pub retryable_count: usize,
    pub terminal_count: usize,
    pub quorum: usize,
    pub quorum_met: bool,
    pub relays: Vec<RadrootsRelayPublishRelayReceipt>,
}

pub trait RadrootsRelayPublishAdapter: Send + Sync {
    fn publish<'a>(
        &'a self,
        request: RadrootsRelayPublishRequest,
    ) -> BoxFuture<'a, Result<Vec<RadrootsRelayPublishRelayReceipt>, RadrootsRelayTransportError>>;
}

pub fn verified_signed_event_payload(
    signed_event: &RadrootsSignedEvent,
) -> Result<RadrootsTransportPayload, RadrootsTransportError> {
    verify_signed_event_raw_json_matches_event(signed_event)?;
    RadrootsTransportPayload::unchecked_signed_event_json(
        signed_event.id.as_str(),
        signed_event.raw_json.as_str(),
    )
}

fn verify_signed_event_raw_json_matches_event(
    signed_event: &RadrootsSignedEvent,
) -> Result<(), RadrootsTransportError> {
    let wire: SignedEventJsonWire = serde_json::from_str(signed_event.raw_json.as_str())
        .map_err(|_| RadrootsTransportError::InvalidPayloadBytes)?;
    if wire.id != signed_event.id {
        return Err(RadrootsTransportError::InvalidPayloadId);
    }
    if wire.pubkey != signed_event.pubkey
        || wire.created_at != signed_event.created_at
        || wire.kind != signed_event.kind
        || wire.tags != signed_event.tags
        || wire.content != signed_event.content
        || wire.sig != signed_event.sig
    {
        return Err(RadrootsTransportError::InvalidPayloadBytes);
    }
    Ok(())
}

impl<A> RadrootsRelayPublishAdapter for &A
where
    A: RadrootsRelayPublishAdapter + ?Sized,
{
    fn publish<'a>(
        &'a self,
        request: RadrootsRelayPublishRequest,
    ) -> BoxFuture<'a, Result<Vec<RadrootsRelayPublishRelayReceipt>, RadrootsRelayTransportError>>
    {
        (*self).publish(request)
    }
}

#[derive(Clone)]
pub struct RadrootsNostrTransport<A> {
    adapter: A,
    status: RadrootsTransportStatus,
}

impl<A> RadrootsNostrTransport<A> {
    pub fn new(adapter: A) -> Self {
        Self {
            adapter,
            status: RadrootsTransportStatus::new(
                RadrootsTransportKind::Nostr,
                true,
                RadrootsTransportImplementationState::Real,
                true,
                "ready",
            )
            .with_capabilities(RadrootsTransportCapabilities::deliver_only()),
        }
    }

    pub fn with_status(mut self, status: RadrootsTransportStatus) -> Self {
        self.status = status;
        self
    }

    pub fn adapter(&self) -> &A {
        &self.adapter
    }
}

impl<A> RadrootsTransport for RadrootsNostrTransport<A>
where
    A: RadrootsRelayPublishAdapter,
{
    fn transport_kind(&self) -> RadrootsTransportKind {
        RadrootsTransportKind::Nostr
    }

    fn status<'a>(&'a self) -> RadrootsTransportFuture<'a, RadrootsTransportStatus> {
        Box::pin(async move { Ok(self.status.clone()) })
    }

    fn deliver<'a>(
        &'a self,
        request: RadrootsTransportDeliveryRequest,
    ) -> RadrootsTransportFuture<'a, RadrootsTransportDeliveryReceipt> {
        Box::pin(async move {
            let signed_event = signed_event_from_transport_payload(&request.payload)?;
            let targets = relay_targets_from_transport_targets(request.target_set.targets())?;
            let relay_receipts = match self
                .adapter
                .publish(
                    RadrootsRelayPublishRequest::new(signed_event, targets, request.now_ms)
                        .with_satisfaction_policy(request.satisfaction_policy.clone())
                        .with_idempotency_key(request.request_id.clone()),
                )
                .await
            {
                Ok(receipts) => receipts,
                Err(RadrootsRelayTransportError::Transport(message)) => {
                    return Ok(RadrootsTransportDeliveryReceipt {
                        request_id: request.request_id,
                        target_receipts: transport_failure_target_receipts(
                            request.target_set.targets(),
                            message.as_str(),
                        ),
                    });
                }
                Err(error) => return Err(nostr_error_to_transport_error(error)),
            };
            Ok(RadrootsTransportDeliveryReceipt {
                request_id: request.request_id,
                target_receipts: target_receipts_from_relay_receipts(
                    request.target_set.targets(),
                    relay_receipts.as_slice(),
                ),
            })
        })
    }

    fn fetch<'a>(
        &'a self,
        _request: RadrootsTransportFetchRequest,
    ) -> RadrootsTransportFuture<'a, RadrootsTransportFetchReceipt> {
        Box::pin(async move { Err(RadrootsTransportError::UnsupportedOperation) })
    }
}

fn nostr_error_to_transport_error(error: RadrootsRelayTransportError) -> RadrootsTransportError {
    match error {
        RadrootsRelayTransportError::TransportContract(_) => {
            RadrootsTransportError::InvalidPayloadBytes
        }
        RadrootsRelayTransportError::RelayUrlParse { .. }
        | RadrootsRelayTransportError::WsRequiresLocalhostPolicy { .. }
        | RadrootsRelayTransportError::UnsupportedRelayScheme { .. }
        | RadrootsRelayTransportError::EmptyRelayHost { .. }
        | RadrootsRelayTransportError::RelayUrlUserinfo { .. }
        | RadrootsRelayTransportError::RelayUrlQueryOrFragment { .. }
        | RadrootsRelayTransportError::RelayUrlForbiddenDestination { .. }
        | RadrootsRelayTransportError::RelayUrlResolvedForbiddenDestination { .. }
        | RadrootsRelayTransportError::EmptyTargetSet => RadrootsTransportError::InvalidTargetUri,
        RadrootsRelayTransportError::NostrEventJson(_) | RadrootsRelayTransportError::Json(_) => {
            RadrootsTransportError::InvalidPayloadBytes
        }
        RadrootsRelayTransportError::Transport(_) => RadrootsTransportError::InvalidTransportKind,
        RadrootsRelayTransportError::EmptyFetchFilters
        | RadrootsRelayTransportError::InvalidFetchLimit { .. } => {
            RadrootsTransportError::InvalidTransportKind
        }
        #[cfg(feature = "storage")]
        RadrootsRelayTransportError::EventStore(_)
        | RadrootsRelayTransportError::Outbox(_)
        | RadrootsRelayTransportError::MissingSignedOutboxEvent(_) => {
            RadrootsTransportError::InvalidTransportKind
        }
    }
}

#[derive(Deserialize)]
struct SignedEventJsonWire {
    id: String,
    pubkey: String,
    created_at: u32,
    kind: u32,
    tags: Vec<Vec<String>>,
    content: String,
    sig: String,
}

fn signed_event_from_transport_payload(
    payload: &RadrootsTransportPayload,
) -> Result<RadrootsSignedEvent, RadrootsTransportError> {
    let RadrootsTransportPayload::SignedEventJson {
        event_id, raw_json, ..
    } = payload
    else {
        return Err(RadrootsTransportError::InvalidPayloadBytes);
    };
    let wire: SignedEventJsonWire =
        serde_json::from_str(raw_json).map_err(|_| RadrootsTransportError::InvalidPayloadBytes)?;
    if wire.id != *event_id {
        return Err(RadrootsTransportError::InvalidPayloadId);
    }
    RadrootsSignedEvent::new(RadrootsSignedEventParts {
        id: wire.id,
        pubkey: wire.pubkey,
        created_at: wire.created_at,
        kind: wire.kind,
        tags: wire.tags,
        content: wire.content,
        sig: wire.sig,
        raw_json: raw_json.clone(),
    })
    .map_err(|_| RadrootsTransportError::InvalidPayloadBytes)
}

fn relay_targets_from_transport_targets(
    targets: &[RadrootsTransportTarget],
) -> Result<RadrootsRelayTargetSet, RadrootsTransportError> {
    let relays = targets
        .iter()
        .map(|target| {
            if target.kind != RadrootsTransportKind::Nostr {
                return Err(RadrootsTransportError::InvalidTargetUri);
            }
            let policy = if target.uri.as_str().starts_with("ws://") {
                crate::RadrootsRelayUrlPolicy::Localhost
            } else {
                crate::RadrootsRelayUrlPolicy::Public
            };
            crate::RadrootsRelayUrl::parse(target.uri.as_str(), policy)
                .map_err(nostr_error_to_transport_error)
        })
        .collect::<Result<Vec<_>, _>>()?;
    RadrootsRelayTargetSet::from_urls(relays).map_err(nostr_error_to_transport_error)
}

fn target_receipts_from_relay_receipts(
    targets: &[RadrootsTransportTarget],
    relay_receipts: &[RadrootsRelayPublishRelayReceipt],
) -> Vec<RadrootsTransportTargetReceipt> {
    targets
        .iter()
        .cloned()
        .map(|target| {
            let outcome = relay_receipts
                .iter()
                .find(|receipt| relay_receipt_matches_target(receipt, &target))
                .map(|receipt| receipt.outcome.to_transport_outcome())
                .unwrap_or_else(|| {
                    RadrootsTransportOutcome::new(RadrootsTransportOutcomeKind::RouteUnavailable)
                        .with_message("relay adapter omitted target receipt")
                });
            RadrootsTransportTargetReceipt::new(target, outcome)
        })
        .collect()
}

fn transport_failure_target_receipts(
    targets: &[RadrootsTransportTarget],
    message: &str,
) -> Vec<RadrootsTransportTargetReceipt> {
    targets
        .iter()
        .cloned()
        .map(|target| {
            RadrootsTransportTargetReceipt::new(
                target,
                RadrootsTransportOutcome::new(RadrootsTransportOutcomeKind::ConnectionFailed)
                    .with_message(message.to_owned()),
            )
        })
        .collect()
}

pub async fn publish_signed_event<A>(
    adapter: &A,
    request: RadrootsRelayPublishRequest,
) -> Result<RadrootsRelayPublishReceipt, RadrootsRelayTransportError>
where
    A: RadrootsRelayPublishAdapter,
{
    let event_id = request.signed_event.id.clone();
    let satisfaction_policy = request.satisfaction_policy.clone();
    let target_count = request.targets.len();
    let quorum = satisfaction_policy.required_target_count(target_count)?;
    let relays = adapter.publish(request).await?;
    let attempted_count = relays.iter().filter(|receipt| receipt.attempted).count();
    let accepted_count = relays
        .iter()
        .filter(|receipt| receipt.outcome.counts_toward_quorum())
        .count();
    let retryable_count = relays
        .iter()
        .filter(|receipt| receipt.outcome.is_retryable())
        .count();
    let terminal_count = relays
        .iter()
        .filter(|receipt| receipt.outcome.is_terminal_failure())
        .count();
    let quorum_met = relay_publish_satisfies_policy(&satisfaction_policy, target_count, &relays)?;
    Ok(RadrootsRelayPublishReceipt {
        event_id,
        attempted_count,
        accepted_count,
        retryable_count,
        terminal_count,
        quorum,
        quorum_met,
        relays,
    })
}

fn relay_publish_satisfies_policy(
    policy: &RadrootsTransportSatisfactionPolicy,
    target_count: usize,
    relays: &[RadrootsRelayPublishRelayReceipt],
) -> Result<bool, RadrootsRelayTransportError> {
    match policy {
        RadrootsTransportSatisfactionPolicy::NoWait => Ok(true),
        RadrootsTransportSatisfactionPolicy::Any { class }
        | RadrootsTransportSatisfactionPolicy::All { class }
        | RadrootsTransportSatisfactionPolicy::Quorum { class, .. } => {
            let satisfied_count = relays
                .iter()
                .filter(|receipt| {
                    receipt
                        .outcome
                        .to_transport_outcome()
                        .status
                        .counts_as_satisfied(*class)
                })
                .count();
            Ok(policy.is_satisfied_by(target_count, satisfied_count)?)
        }
        RadrootsTransportSatisfactionPolicy::RequiredTargets { class, targets } => {
            policy.required_target_count(target_count)?;
            let mut satisfied_required_targets = BTreeSet::new();
            for receipt in relays {
                let target = RadrootsTransportTarget::nostr_relay(&receipt.relay_url)?;
                if targets.contains(&target.fingerprint)
                    && receipt
                        .outcome
                        .to_transport_outcome()
                        .status
                        .counts_as_satisfied(*class)
                {
                    satisfied_required_targets.insert(target.fingerprint);
                }
            }
            Ok(targets
                .iter()
                .all(|target| satisfied_required_targets.contains(target)))
        }
    }
}

fn relay_receipt_matches_target(
    receipt: &RadrootsRelayPublishRelayReceipt,
    target: &RadrootsTransportTarget,
) -> bool {
    RadrootsTransportTarget::nostr_relay(receipt.relay_url.as_str())
        .is_ok_and(|receipt_target| receipt_target.uri == target.uri)
}

#[derive(Clone, Default)]
pub struct RadrootsMockRelayPublishAdapter {
    outcomes: BTreeMap<String, RadrootsRelayOutcome>,
    captured_raw_events: Arc<Mutex<Vec<String>>>,
}

impl RadrootsMockRelayPublishAdapter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_outcome(
        mut self,
        relay_url: impl Into<String>,
        outcome: RadrootsRelayOutcome,
    ) -> Self {
        self.outcomes.insert(relay_url.into(), outcome);
        self
    }

    pub fn captured_raw_events(&self) -> Vec<String> {
        self.captured_raw_events
            .lock()
            .expect("captured raw event lock")
            .clone()
    }
}

impl RadrootsRelayPublishAdapter for RadrootsMockRelayPublishAdapter {
    fn publish<'a>(
        &'a self,
        request: RadrootsRelayPublishRequest,
    ) -> BoxFuture<'a, Result<Vec<RadrootsRelayPublishRelayReceipt>, RadrootsRelayTransportError>>
    {
        Box::pin(async move {
            self.captured_raw_events
                .lock()
                .map_err(captured_raw_event_lock_error)?
                .push(request.signed_event.raw_json.clone());
            Ok(request
                .targets
                .relays()
                .iter()
                .map(|relay| {
                    let outcome = self
                        .outcomes
                        .get(relay.as_str())
                        .cloned()
                        .unwrap_or_else(RadrootsRelayOutcome::accepted);
                    RadrootsRelayPublishRelayReceipt::attempted(relay.as_str(), outcome)
                })
                .collect())
        })
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn captured_raw_event_lock_error<T>(_error: PoisonError<T>) -> RadrootsRelayTransportError {
    RadrootsRelayTransportError::Transport("captured raw event lock poisoned".to_owned())
}

#[cfg(feature = "client")]
#[derive(Clone)]
pub struct RadrootsNostrClientPublishAdapter {
    client: RadrootsNostrClient,
}

#[cfg(feature = "client")]
impl RadrootsNostrClientPublishAdapter {
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn new(client: RadrootsNostrClient) -> Self {
        Self { client }
    }
}

#[cfg(feature = "client")]
impl RadrootsRelayPublishAdapter for RadrootsNostrClientPublishAdapter {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn publish<'a>(
        &'a self,
        request: RadrootsRelayPublishRequest,
    ) -> BoxFuture<'a, Result<Vec<RadrootsRelayPublishRelayReceipt>, RadrootsRelayTransportError>>
    {
        Box::pin(async move {
            let event = RadrootsNostrEvent::from_json(request.signed_event.raw_json.as_str())
                .map_err(|error| RadrootsRelayTransportError::NostrEventJson(error.to_string()))?;
            ensure_raw_event_matches_signed_event(&event, &request.signed_event)?;
            let target_strings = request.targets.relay_strings();
            for relay_url in &target_strings {
                self.client
                    .add_write_relay(relay_url.as_str())
                    .await
                    .map_err(|error| RadrootsRelayTransportError::Transport(error.to_string()))?;
            }
            let connection_output = self.client.try_connect(RELAY_CONNECT_TIMEOUT).await;
            let target_url_set = target_strings
                .iter()
                .map(|relay_url| relay_url.trim_end_matches('/').to_owned())
                .collect::<BTreeSet<_>>();
            let connected_strings = self
                .client
                .relays()
                .await
                .into_values()
                .filter(|relay| relay.is_connected())
                .map(|relay| relay.url().to_string())
                .filter(|relay_url| target_url_set.contains(relay_url.trim_end_matches('/')))
                .collect::<Vec<_>>();
            let connection_failures = connection_output
                .failed
                .iter()
                .map(|(relay, reason)| {
                    (
                        relay.to_string().trim_end_matches('/').to_owned(),
                        reason.clone(),
                    )
                })
                .collect::<BTreeMap<_, _>>();
            if connected_strings.is_empty() {
                return Ok(target_strings
                    .into_iter()
                    .map(|relay_url| {
                        let target_url = relay_url.trim_end_matches('/');
                        let reason = connection_failures
                            .get(target_url)
                            .cloned()
                            .unwrap_or_else(|| "relay did not connect".to_owned());
                        RadrootsRelayPublishRelayReceipt::attempted(
                            relay_url,
                            RadrootsRelayOutcome::connection_failed(reason),
                        )
                    })
                    .collect());
            }
            let output = match self.client.send_event_to(connected_strings, &event).await {
                Ok(output) => output,
                Err(error) => {
                    let message = error.to_string();
                    return Ok(target_strings
                        .into_iter()
                        .map(|relay_url| {
                            RadrootsRelayPublishRelayReceipt::attempted(
                                relay_url,
                                RadrootsRelayOutcome::connection_failed(message.clone()),
                            )
                        })
                        .collect());
                }
            };
            let mut receipts = Vec::new();
            for relay_url in &target_strings {
                let target_url = relay_url.trim_end_matches('/');
                let success = output
                    .success
                    .iter()
                    .any(|success_url| success_url.to_string().trim_end_matches('/') == target_url);
                if success {
                    receipts.push(RadrootsRelayPublishRelayReceipt::attempted(
                        relay_url,
                        RadrootsRelayOutcome {
                            kind: RadrootsRelayOutcomeKind::Accepted,
                            message: Some(
                                "nostr-relay-pool-success-ok-message-unavailable".to_owned(),
                            ),
                        },
                    ));
                    continue;
                }
                if let Some(reason) = connection_failures.get(target_url) {
                    receipts.push(RadrootsRelayPublishRelayReceipt::attempted(
                        relay_url,
                        RadrootsRelayOutcome::connection_failed(reason.clone()),
                    ));
                    continue;
                }
                let failed = output.failed.iter().find_map(|(failed_url, message)| {
                    if failed_url.to_string().trim_end_matches('/') == target_url {
                        Some(message.clone())
                    } else {
                        None
                    }
                });
                let outcome = failed
                    .map(RadrootsRelayOutcome::classify)
                    .unwrap_or_else(|| {
                        RadrootsRelayOutcome::classify("error: relay output omitted target")
                    });
                receipts.push(RadrootsRelayPublishRelayReceipt::attempted(
                    relay_url, outcome,
                ));
            }
            Ok(receipts)
        })
    }
}

#[cfg(feature = "client")]
fn ensure_raw_event_matches_signed_event(
    event: &RadrootsNostrEvent,
    signed_event: &RadrootsSignedEvent,
) -> Result<(), RadrootsRelayTransportError> {
    let mismatches = [
        ("id", event.id.to_hex(), signed_event.id.clone()),
        ("pubkey", event.pubkey.to_hex(), signed_event.pubkey.clone()),
        (
            "created_at",
            event.created_at.as_secs().to_string(),
            signed_event.created_at.to_string(),
        ),
        (
            "kind",
            (event.kind.as_u16() as u32).to_string(),
            signed_event.kind.to_string(),
        ),
        (
            "content",
            event.content.clone(),
            signed_event.content.clone(),
        ),
        ("sig", event.sig.to_string(), signed_event.sig.clone()),
    ];
    for (field, raw, wrapped) in mismatches {
        if raw != wrapped {
            return Err(RadrootsRelayTransportError::NostrEventJson(format!(
                "raw event JSON {field} does not match signed event {field}"
            )));
        }
    }
    let raw_tags = event
        .tags
        .iter()
        .map(|tag| tag.as_slice().to_vec())
        .collect::<Vec<_>>();
    if raw_tags != signed_event.tags {
        return Err(RadrootsRelayTransportError::NostrEventJson(
            "raw event JSON tags do not match signed event tags".to_owned(),
        ));
    }
    Ok(())
}

#[cfg(all(test, feature = "client"))]
mod tests {
    use super::{RadrootsNostrEvent, ensure_raw_event_matches_signed_event};
    use nostr::JsonUtil;
    use radroots_event::draft::{RadrootsEventDraft, RadrootsSignedEvent};
    use radroots_event::kinds::KIND_POST;
    use radroots_nostr::prelude::{
        RadrootsNostrKeys, RadrootsNostrSecretKey, radroots_nostr_sign_frozen_draft,
    };

    const FIXTURE_ALICE_SECRET_KEY_HEX: &str =
        "10c5304d6c9ae3a1a16f7860f1cc8f5e3a76225a2663b3a989a0d775919b7df5";
    const FIXTURE_ALICE_PUBLIC_KEY_HEX: &str =
        "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df";

    fn signed_post(content: &str) -> (RadrootsNostrEvent, RadrootsSignedEvent) {
        let secret_key =
            RadrootsNostrSecretKey::from_hex(FIXTURE_ALICE_SECRET_KEY_HEX).expect("secret key");
        let keys = RadrootsNostrKeys::new(secret_key);
        let draft = RadrootsEventDraft::new(
            "radroots.social.post.v1",
            KIND_POST,
            1_700_000_000,
            vec![vec!["t".to_owned(), "soil".to_owned()]],
            content,
            FIXTURE_ALICE_PUBLIC_KEY_HEX,
        )
        .expect("draft");
        let signed_event = radroots_nostr_sign_frozen_draft(&keys, &draft).expect("signed event");
        let raw_event =
            RadrootsNostrEvent::from_json(signed_event.raw_json.as_str()).expect("raw event");
        (raw_event, signed_event)
    }

    fn assert_mismatch(raw_event: &RadrootsNostrEvent, signed_event: RadrootsSignedEvent) {
        assert!(ensure_raw_event_matches_signed_event(raw_event, &signed_event).is_err());
    }

    #[test]
    fn raw_event_match_guard_accepts_exact_event_and_rejects_field_mismatches() {
        let (raw_event, signed_event) = signed_post("matched");
        ensure_raw_event_matches_signed_event(&raw_event, &signed_event).expect("matching event");

        let mut mismatched = signed_event.clone();
        mismatched.id = "00".repeat(32);
        assert_mismatch(&raw_event, mismatched);

        let mut mismatched = signed_event.clone();
        mismatched.pubkey = "11".repeat(32);
        assert_mismatch(&raw_event, mismatched);

        let mut mismatched = signed_event.clone();
        mismatched.created_at += 1;
        assert_mismatch(&raw_event, mismatched);

        let mut mismatched = signed_event.clone();
        mismatched.kind += 1;
        assert_mismatch(&raw_event, mismatched);

        let mut mismatched = signed_event.clone();
        mismatched.content.push_str(" changed");
        assert_mismatch(&raw_event, mismatched);

        let mut mismatched = signed_event.clone();
        mismatched.sig = "22".repeat(64);
        assert_mismatch(&raw_event, mismatched);

        let mut mismatched = signed_event;
        mismatched
            .tags
            .push(vec!["t".to_owned(), "compost".to_owned()]);
        assert_mismatch(&raw_event, mismatched);
    }
}
