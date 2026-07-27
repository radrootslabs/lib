#![forbid(unsafe_code)]

use crate::error::ensure_nonnegative_timestamp;
use crate::{RadrootsRelayOutcome, RadrootsRelayTargetSet, RadrootsRelayTransportError};
#[cfg(feature = "client")]
use core::time::Duration;
use futures::future::BoxFuture;
use radroots_event::{
    draft::{RadrootsSignedEvent, RadrootsVerifiedSignedEvent},
    ids::RadrootsEventId,
    wire::RadrootsNip01EventWire,
};
use radroots_transport::{
    RadrootsTransport, RadrootsTransportCapabilities, RadrootsTransportDeliveryReceipt,
    RadrootsTransportDeliveryRequest, RadrootsTransportError, RadrootsTransportFetchReceipt,
    RadrootsTransportFetchRequest, RadrootsTransportFuture, RadrootsTransportImplementationState,
    RadrootsTransportKind, RadrootsTransportOutcome, RadrootsTransportOutcomeKind,
    RadrootsTransportPayload, RadrootsTransportSatisfactionPolicy, RadrootsTransportStatus,
    RadrootsTransportTarget, RadrootsTransportTargetReceipt,
};
use serde::{Deserialize, Deserializer, Serialize, de};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::{Arc, Mutex, PoisonError};

use crate::RadrootsRelayOutcomeKind;
#[cfg(feature = "client")]
use nostr::JsonUtil;
#[cfg(feature = "client")]
use radroots_nostr::prelude::{RadrootsNostrClient, RadrootsNostrEvent};

#[cfg(feature = "client")]
const RELAY_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
pub const RADROOTS_RELAY_PUBLISH_IDEMPOTENCY_KEY_MAX_BYTES: usize = 256;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsRelayPublishRequest {
    signed_event: RadrootsVerifiedSignedEvent,
    targets: RadrootsRelayTargetSet,
    satisfaction_policy: RadrootsTransportSatisfactionPolicy,
    idempotency_key: Option<String>,
    now_ms: i64,
}

impl RadrootsRelayPublishRequest {
    pub fn new(
        signed_event: RadrootsVerifiedSignedEvent,
        targets: RadrootsRelayTargetSet,
        now_ms: i64,
    ) -> Result<Self, RadrootsRelayTransportError> {
        ensure_nonnegative_timestamp("now_ms", now_ms)?;
        Ok(Self {
            signed_event,
            targets,
            satisfaction_policy: RadrootsTransportSatisfactionPolicy::all_accepted(),
            idempotency_key: None,
            now_ms,
        })
    }

    pub fn with_satisfaction_policy(
        mut self,
        satisfaction_policy: RadrootsTransportSatisfactionPolicy,
    ) -> Self {
        self.satisfaction_policy = satisfaction_policy;
        self
    }

    pub fn try_with_idempotency_key(
        mut self,
        idempotency_key: impl Into<String>,
    ) -> Result<Self, RadrootsRelayTransportError> {
        let idempotency_key = idempotency_key.into();
        validate_publish_idempotency_key(idempotency_key.as_str())?;
        self.idempotency_key = Some(idempotency_key);
        Ok(self)
    }

    pub fn signed_event(&self) -> &RadrootsVerifiedSignedEvent {
        &self.signed_event
    }

    pub fn targets(&self) -> &RadrootsRelayTargetSet {
        &self.targets
    }

    pub fn satisfaction_policy(&self) -> &RadrootsTransportSatisfactionPolicy {
        &self.satisfaction_policy
    }

    pub fn idempotency_key(&self) -> Option<&str> {
        self.idempotency_key.as_deref()
    }

    pub fn now_ms(&self) -> i64 {
        self.now_ms
    }

    fn validate(&self) -> Result<(), RadrootsRelayTransportError> {
        ensure_nonnegative_timestamp("now_ms", self.now_ms)?;
        if let Some(idempotency_key) = self.idempotency_key.as_deref() {
            validate_publish_idempotency_key(idempotency_key)?;
        }
        let target_count = self.targets.len();
        self.satisfaction_policy
            .required_target_count(target_count)?;
        if let Some(required_targets) = self.satisfaction_policy.required_target_fingerprints() {
            let requested = self
                .targets
                .relays()
                .iter()
                .map(|relay| RadrootsTransportTarget::nostr_relay(relay.as_str()))
                .collect::<Result<Vec<_>, _>>()?;
            for required in required_targets {
                if !requested
                    .iter()
                    .any(|target| target.fingerprint() == required)
                {
                    return Err(RadrootsRelayTransportError::RequiredTargetNotRequested {
                        fingerprint: required.as_str().to_owned(),
                    });
                }
            }
        }
        Ok(())
    }
}

fn validate_publish_idempotency_key(value: &str) -> Result<(), RadrootsRelayTransportError> {
    let reason = if value.is_empty() {
        Some("key must not be empty")
    } else if value != value.trim() {
        Some("key must not have surrounding whitespace")
    } else if value.chars().any(char::is_control) {
        Some("key must not contain control characters")
    } else if value.len() > RADROOTS_RELAY_PUBLISH_IDEMPOTENCY_KEY_MAX_BYTES {
        Some("key exceeds the UTF-8 byte limit")
    } else {
        None
    };
    if let Some(reason) = reason {
        return Err(RadrootsRelayTransportError::InvalidIdempotencyKey {
            reason,
            actual_bytes: value.len(),
            max_bytes: RADROOTS_RELAY_PUBLISH_IDEMPOTENCY_KEY_MAX_BYTES,
        });
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RadrootsRelayPublishRelayReceipt {
    relay_url: String,
    outcome: RadrootsRelayOutcome,
    attempted: bool,
}

impl RadrootsRelayPublishRelayReceipt {
    pub fn attempted(
        relay_url: impl Into<String>,
        outcome: RadrootsRelayOutcome,
    ) -> Result<Self, RadrootsRelayTransportError> {
        Self::try_new(relay_url.into(), outcome, true)
    }

    pub fn skipped(
        relay_url: impl Into<String>,
        outcome: RadrootsRelayOutcome,
    ) -> Result<Self, RadrootsRelayTransportError> {
        Self::try_new(relay_url.into(), outcome, false)
    }

    fn try_new(
        relay_url: String,
        outcome: RadrootsRelayOutcome,
        attempted: bool,
    ) -> Result<Self, RadrootsRelayTransportError> {
        validate_publish_receipt_relay_url(relay_url.as_str())?;
        Ok(Self {
            relay_url,
            outcome,
            attempted,
        })
    }

    pub fn relay_url(&self) -> &str {
        self.relay_url.as_str()
    }

    pub fn outcome(&self) -> &RadrootsRelayOutcome {
        &self.outcome
    }

    pub fn was_attempted(&self) -> bool {
        self.attempted
    }
}

fn validate_publish_receipt_relay_url(relay_url: &str) -> Result<(), RadrootsRelayTransportError> {
    RadrootsTransportTarget::nostr_relay(relay_url).map_err(|error| {
        RadrootsRelayTransportError::InvalidPublishReceiptRelayUrl {
            url: if relay_url.len() <= radroots_transport::RADROOTS_TRANSPORT_ENDPOINT_URI_MAX_BYTES
            {
                relay_url.to_owned()
            } else {
                "<oversized>".to_owned()
            },
            reason: error.to_string(),
        }
    })?;
    Ok(())
}

struct BoundedPublishReceiptRelayUrl;

impl<'de> de::Visitor<'de> for BoundedPublishReceiptRelayUrl {
    type Value = String;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "a relay URL of at most {} UTF-8 bytes",
            radroots_transport::RADROOTS_TRANSPORT_ENDPOINT_URI_MAX_BYTES
        )
    }

    fn visit_borrowed_str<E>(self, value: &'de str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.visit_str(value)
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        validate_publish_receipt_relay_url(value)
            .map_err(E::custom)
            .map(|()| value.to_owned())
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        validate_publish_receipt_relay_url(value.as_str())
            .map_err(E::custom)
            .map(|()| value)
    }
}

fn deserialize_publish_receipt_relay_url<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_string(BoundedPublishReceiptRelayUrl)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RadrootsRelayPublishRelayReceiptWire {
    #[serde(deserialize_with = "deserialize_publish_receipt_relay_url")]
    relay_url: String,
    outcome: RadrootsRelayOutcome,
    attempted: bool,
}

impl<'de> Deserialize<'de> for RadrootsRelayPublishRelayReceipt {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = RadrootsRelayPublishRelayReceiptWire::deserialize(deserializer)?;
        Self::try_new(wire.relay_url, wire.outcome, wire.attempted).map_err(de::Error::custom)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RadrootsRelayPublishReceipt {
    event_id: RadrootsEventId,
    attempted_count: usize,
    accepted_count: usize,
    retryable_count: usize,
    terminal_count: usize,
    quorum: usize,
    quorum_met: bool,
    relays: Vec<RadrootsRelayPublishRelayReceipt>,
}

impl RadrootsRelayPublishReceipt {
    pub(crate) fn new(
        event_id: impl AsRef<str>,
        quorum: usize,
        quorum_met: bool,
        relays: Vec<RadrootsRelayPublishRelayReceipt>,
    ) -> Result<Self, RadrootsRelayTransportError> {
        let event_id = RadrootsEventId::parse(event_id).map_err(|error| {
            RadrootsRelayTransportError::InvalidPublishReceipt {
                field: "event_id",
                reason: error.to_string(),
            }
        })?;
        Self::from_validated_event_id(event_id, quorum, quorum_met, relays)
    }

    fn from_validated_event_id(
        event_id: RadrootsEventId,
        quorum: usize,
        quorum_met: bool,
        relays: Vec<RadrootsRelayPublishRelayReceipt>,
    ) -> Result<Self, RadrootsRelayTransportError> {
        if relays.is_empty() {
            return Err(RadrootsRelayTransportError::InvalidPublishReceipt {
                field: "relays",
                reason: "relay receipts must not be empty".to_owned(),
            });
        }
        if relays.len() > radroots_transport::RADROOTS_TRANSPORT_TARGET_MAX_COUNT {
            return Err(RadrootsRelayTransportError::InvalidPublishReceipt {
                field: "relays",
                reason: format!(
                    "relay receipt count {} exceeds maximum {}",
                    relays.len(),
                    radroots_transport::RADROOTS_TRANSPORT_TARGET_MAX_COUNT
                ),
            });
        }
        let mut canonical_relays = Vec::with_capacity(relays.len());
        for receipt in &relays {
            let canonical = RadrootsTransportTarget::nostr_relay(receipt.relay_url())?
                .uri()
                .as_str()
                .to_owned();
            if canonical_relays.contains(&canonical) {
                return Err(
                    RadrootsRelayTransportError::DuplicatePublishReceiptRelayUrl { url: canonical },
                );
            }
            canonical_relays.push(canonical);
        }
        if quorum > relays.len() {
            return Err(RadrootsRelayTransportError::InvalidPublishReceipt {
                field: "quorum",
                reason: format!(
                    "quorum {quorum} exceeds relay receipt count {}",
                    relays.len()
                ),
            });
        }
        let attempted_count = relays
            .iter()
            .filter(|receipt| receipt.was_attempted())
            .count();
        let accepted_count = relays
            .iter()
            .filter(|receipt| relay_receipt_counts_toward_quorum(receipt))
            .count();
        let retryable_count = relays
            .iter()
            .filter(|receipt| receipt.outcome().is_retryable())
            .count();
        let terminal_count = relays
            .iter()
            .filter(|receipt| receipt.outcome().is_terminal_failure())
            .count();
        if quorum_met && accepted_count < quorum {
            return Err(RadrootsRelayTransportError::InvalidPublishReceipt {
                field: "quorum_met",
                reason: format!("accepted count {accepted_count} cannot satisfy quorum {quorum}"),
            });
        }
        Ok(Self {
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

    pub fn event_id(&self) -> &str {
        self.event_id.as_str()
    }

    pub fn attempted_count(&self) -> usize {
        self.attempted_count
    }

    pub fn accepted_count(&self) -> usize {
        self.accepted_count
    }

    pub fn retryable_count(&self) -> usize {
        self.retryable_count
    }

    pub fn terminal_count(&self) -> usize {
        self.terminal_count
    }

    pub fn quorum(&self) -> usize {
        self.quorum
    }

    pub fn quorum_met(&self) -> bool {
        self.quorum_met
    }

    pub fn relays(&self) -> &[RadrootsRelayPublishRelayReceipt] {
        &self.relays
    }

    #[cfg(feature = "storage")]
    pub(crate) fn into_event_id_and_relays(
        self,
    ) -> (String, Vec<RadrootsRelayPublishRelayReceipt>) {
        (self.event_id.into_string(), self.relays)
    }
}

struct BoundedPublishRelayReceipts;

impl<'de> de::Visitor<'de> for BoundedPublishRelayReceipts {
    type Value = Vec<RadrootsRelayPublishRelayReceipt>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "one to {} relay publish receipts",
            radroots_transport::RADROOTS_TRANSPORT_TARGET_MAX_COUNT
        )
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: de::SeqAccess<'de>,
    {
        let mut relays = Vec::new();
        while let Some(receipt) = sequence.next_element()? {
            if relays.len() == radroots_transport::RADROOTS_TRANSPORT_TARGET_MAX_COUNT {
                return Err(de::Error::custom(format!(
                    "relay receipt count exceeds maximum {}",
                    radroots_transport::RADROOTS_TRANSPORT_TARGET_MAX_COUNT
                )));
            }
            relays.push(receipt);
        }
        Ok(relays)
    }
}

fn deserialize_publish_relay_receipts<'de, D>(
    deserializer: D,
) -> Result<Vec<RadrootsRelayPublishRelayReceipt>, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_seq(BoundedPublishRelayReceipts)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RadrootsRelayPublishReceiptWire {
    event_id: RadrootsEventId,
    attempted_count: usize,
    accepted_count: usize,
    retryable_count: usize,
    terminal_count: usize,
    quorum: usize,
    quorum_met: bool,
    #[serde(deserialize_with = "deserialize_publish_relay_receipts")]
    relays: Vec<RadrootsRelayPublishRelayReceipt>,
}

impl<'de> Deserialize<'de> for RadrootsRelayPublishReceipt {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = RadrootsRelayPublishReceiptWire::deserialize(deserializer)?;
        let receipt =
            Self::from_validated_event_id(wire.event_id, wire.quorum, wire.quorum_met, wire.relays)
                .map_err(de::Error::custom)?;
        for (field, expected, actual) in [
            (
                "attempted_count",
                receipt.attempted_count,
                wire.attempted_count,
            ),
            (
                "accepted_count",
                receipt.accepted_count,
                wire.accepted_count,
            ),
            (
                "retryable_count",
                receipt.retryable_count,
                wire.retryable_count,
            ),
            (
                "terminal_count",
                receipt.terminal_count,
                wire.terminal_count,
            ),
        ] {
            if expected != actual {
                return Err(de::Error::custom(format!(
                    "relay publish receipt {field} {actual} does not match derived count {expected}"
                )));
            }
        }
        Ok(receipt)
    }
}

pub trait RadrootsRelayPublishAdapter: Send + Sync {
    fn publish<'a>(
        &'a self,
        request: RadrootsRelayPublishRequest,
    ) -> BoxFuture<'a, Result<Vec<RadrootsRelayPublishRelayReceipt>, RadrootsRelayTransportError>>;
}

pub fn verified_signed_event_payload(
    signed_event: &RadrootsVerifiedSignedEvent,
) -> Result<RadrootsTransportPayload, RadrootsTransportError> {
    let signed_event = signed_event.signed_event();
    RadrootsTransportPayload::unchecked_signed_event_json(
        signed_event.id_str(),
        signed_event.raw_json(),
    )
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
            .expect("static Nostr transport status")
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
            let signed_event = signed_event_from_transport_payload(request.payload())?;
            let targets = relay_targets_from_transport_targets(request.target_set().targets())?;
            let publish_request =
                RadrootsRelayPublishRequest::new(signed_event, targets, request.now_ms())
                    .map_err(nostr_error_to_transport_error)?
                    .with_satisfaction_policy(RadrootsTransportSatisfactionPolicy::no_wait())
                    .try_with_idempotency_key(request.request_id())
                    .map_err(nostr_error_to_transport_error)?;
            let relay_receipts = match publish_signed_event(&self.adapter, publish_request).await {
                Ok(receipt) => receipt.relays,
                Err(RadrootsRelayTransportError::Transport(message)) => {
                    return RadrootsTransportDeliveryReceipt::for_request(
                        &request,
                        transport_failure_target_receipts(
                            request.target_set().targets(),
                            message.as_str(),
                        )?,
                    );
                }
                Err(error) => return Err(nostr_error_to_transport_error(error)),
            };
            RadrootsTransportDeliveryReceipt::for_request(
                &request,
                target_receipts_from_relay_receipts(
                    request.target_set().targets(),
                    relay_receipts.as_slice(),
                )?,
            )
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
        RadrootsRelayTransportError::ConflictingTransportReceiptRelayUrl { .. } => {
            RadrootsTransportError::DuplicateDeliveryTargetReceipt
        }
        RadrootsRelayTransportError::UnexpectedTransportKind { .. }
        | RadrootsRelayTransportError::DuplicateFetchTerminalRelayUrl { .. }
        | RadrootsRelayTransportError::ConflictingFetchTerminalRelayUrl { .. } => {
            RadrootsTransportError::InvalidTransportKind
        }
        RadrootsRelayTransportError::RelayUrlParse { .. }
        | RadrootsRelayTransportError::WsRequiresLocalhostPolicy { .. }
        | RadrootsRelayTransportError::UnsupportedRelayScheme { .. }
        | RadrootsRelayTransportError::EmptyRelayHost { .. }
        | RadrootsRelayTransportError::RelayUrlUserinfo { .. }
        | RadrootsRelayTransportError::RelayUrlQueryOrFragment { .. }
        | RadrootsRelayTransportError::RelayUrlForbiddenDestination { .. }
        | RadrootsRelayTransportError::RelayUrlResolvedForbiddenDestination { .. }
        | RadrootsRelayTransportError::RelayUrlResolvedNoAddresses { .. }
        | RadrootsRelayTransportError::EmptyTargetSet
        | RadrootsRelayTransportError::DuplicateRelayUrl { .. }
        | RadrootsRelayTransportError::InvalidFetchItemRelayUrl { .. }
        | RadrootsRelayTransportError::UnexpectedFetchItemRelayUrl { .. }
        | RadrootsRelayTransportError::InvalidPublishReceiptRelayUrl { .. }
        | RadrootsRelayTransportError::UnexpectedPublishReceiptRelayUrl { .. }
        | RadrootsRelayTransportError::DuplicatePublishReceiptRelayUrl { .. }
        | RadrootsRelayTransportError::InvalidPublishReceiptAttemptState { .. } => {
            RadrootsTransportError::InvalidTargetUri
        }
        RadrootsRelayTransportError::NostrEventJson(_) | RadrootsRelayTransportError::Json(_) => {
            RadrootsTransportError::InvalidPayloadBytes
        }
        RadrootsRelayTransportError::Transport(_) => RadrootsTransportError::InvalidTransportKind,
        RadrootsRelayTransportError::EmptyFetchFilters
        | RadrootsRelayTransportError::InvalidFetchLimit { .. }
        | RadrootsRelayTransportError::FetchLimitTooLarge { .. }
        | RadrootsRelayTransportError::InvalidFetchReceipt { .. }
        | RadrootsRelayTransportError::InvalidPublishReceipt { .. }
        | RadrootsRelayTransportError::InvalidTimestamp { .. }
        | RadrootsRelayTransportError::InvalidIdempotencyKey { .. }
        | RadrootsRelayTransportError::RequiredTargetNotRequested { .. } => {
            RadrootsTransportError::InvalidTransportKind
        }
        RadrootsRelayTransportError::DiagnosticLimitExceeded { field, max, actual } => {
            RadrootsTransportError::ResourceLimitExceeded { field, max, actual }
        }
        #[cfg(feature = "storage")]
        RadrootsRelayTransportError::EventStore(_)
        | RadrootsRelayTransportError::Outbox(_)
        | RadrootsRelayTransportError::MissingSignedOutboxEvent(_)
        | RadrootsRelayTransportError::MissingPersistedFetchReceiptEventId
        | RadrootsRelayTransportError::MissingStoredEventVisibility { .. }
        | RadrootsRelayTransportError::UnsupportedStoredEventVisibility { .. } => {
            RadrootsTransportError::InvalidTransportKind
        }
    }
}

#[cfg(test)]
mod contract_tests {
    use super::nostr_error_to_transport_error;
    use crate::RadrootsRelayTransportError;
    use radroots_transport::RadrootsTransportError;

    #[test]
    fn relay_errors_map_to_stable_transport_contract_categories() {
        assert_eq!(
            nostr_error_to_transport_error(RadrootsRelayTransportError::TransportContract(
                "contract".to_owned(),
            )),
            RadrootsTransportError::InvalidPayloadBytes
        );
        assert_eq!(
            nostr_error_to_transport_error(
                RadrootsRelayTransportError::ConflictingTransportReceiptRelayUrl {
                    url: "wss://relay.example".to_owned(),
                },
            ),
            RadrootsTransportError::DuplicateDeliveryTargetReceipt
        );
        for error in [
            RadrootsRelayTransportError::UnexpectedTransportKind {
                expected: "nostr",
                actual: "reticulum".to_owned(),
            },
            RadrootsRelayTransportError::DuplicateFetchTerminalRelayUrl {
                url: "wss://relay.example".to_owned(),
            },
            RadrootsRelayTransportError::ConflictingFetchTerminalRelayUrl {
                url: "wss://relay.example".to_owned(),
                first: "eose",
                next: "closed",
            },
        ] {
            assert_eq!(
                nostr_error_to_transport_error(error),
                RadrootsTransportError::InvalidTransportKind
            );
        }

        let target_errors = [
            RadrootsRelayTransportError::RelayUrlParse {
                url: "bad".to_owned(),
                reason: "parse".to_owned(),
            },
            RadrootsRelayTransportError::WsRequiresLocalhostPolicy {
                url: "ws://relay.example".to_owned(),
            },
            RadrootsRelayTransportError::UnsupportedRelayScheme {
                url: "https://relay.example".to_owned(),
                scheme: "https".to_owned(),
            },
            RadrootsRelayTransportError::EmptyRelayHost {
                url: "wss://".to_owned(),
            },
            RadrootsRelayTransportError::RelayUrlUserinfo {
                url: "wss://user@relay.example".to_owned(),
            },
            RadrootsRelayTransportError::RelayUrlQueryOrFragment {
                url: "wss://relay.example?x=1".to_owned(),
            },
            RadrootsRelayTransportError::RelayUrlForbiddenDestination {
                url: "wss://127.0.0.1".to_owned(),
                reason: "loopback".to_owned(),
            },
            RadrootsRelayTransportError::RelayUrlResolvedForbiddenDestination {
                url: "wss://relay.example".to_owned(),
                address: "127.0.0.1".to_owned(),
                reason: "loopback".to_owned(),
            },
            RadrootsRelayTransportError::RelayUrlResolvedNoAddresses {
                url: "wss://relay.example".to_owned(),
            },
            RadrootsRelayTransportError::EmptyTargetSet,
            RadrootsRelayTransportError::DuplicateRelayUrl {
                url: "wss://relay.example".to_owned(),
            },
            RadrootsRelayTransportError::InvalidFetchItemRelayUrl {
                url: "bad".to_owned(),
                reason: "parse".to_owned(),
            },
            RadrootsRelayTransportError::UnexpectedFetchItemRelayUrl {
                url: "wss://other.example".to_owned(),
            },
            RadrootsRelayTransportError::InvalidPublishReceiptRelayUrl {
                url: "bad".to_owned(),
                reason: "parse".to_owned(),
            },
            RadrootsRelayTransportError::UnexpectedPublishReceiptRelayUrl {
                url: "wss://other.example".to_owned(),
            },
            RadrootsRelayTransportError::DuplicatePublishReceiptRelayUrl {
                url: "wss://relay.example".to_owned(),
            },
        ];
        for error in target_errors {
            assert_eq!(
                nostr_error_to_transport_error(error),
                RadrootsTransportError::InvalidTargetUri
            );
        }

        assert_eq!(
            nostr_error_to_transport_error(RadrootsRelayTransportError::NostrEventJson(
                "event".to_owned(),
            )),
            RadrootsTransportError::InvalidPayloadBytes
        );
        let json_error = serde_json::from_str::<serde_json::Value>("{").expect_err("invalid json");
        assert_eq!(
            nostr_error_to_transport_error(RadrootsRelayTransportError::Json(json_error)),
            RadrootsTransportError::InvalidPayloadBytes
        );
        assert_eq!(
            nostr_error_to_transport_error(RadrootsRelayTransportError::Transport(
                "offline".to_owned(),
            )),
            RadrootsTransportError::InvalidTransportKind
        );
        assert_eq!(
            nostr_error_to_transport_error(RadrootsRelayTransportError::EmptyFetchFilters),
            RadrootsTransportError::InvalidTransportKind
        );
        assert_eq!(
            nostr_error_to_transport_error(RadrootsRelayTransportError::InvalidFetchLimit {
                field: "max_events",
            }),
            RadrootsTransportError::InvalidTransportKind
        );
        assert_eq!(
            nostr_error_to_transport_error(RadrootsRelayTransportError::FetchLimitTooLarge {
                field: "max_events",
                max: 1_000,
                actual: 1_001,
            }),
            RadrootsTransportError::InvalidTransportKind
        );
        assert_eq!(
            nostr_error_to_transport_error(RadrootsRelayTransportError::InvalidTimestamp {
                field: "now_ms",
                value: -1,
            }),
            RadrootsTransportError::InvalidTransportKind
        );
        assert_eq!(
            nostr_error_to_transport_error(RadrootsRelayTransportError::InvalidIdempotencyKey {
                reason: "empty",
                actual_bytes: 0,
                max_bytes: 256,
            }),
            RadrootsTransportError::InvalidTransportKind
        );
        assert_eq!(
            nostr_error_to_transport_error(
                RadrootsRelayTransportError::RequiredTargetNotRequested {
                    fingerprint: "sha256:missing".to_owned(),
                },
            ),
            RadrootsTransportError::InvalidTransportKind
        );

        #[cfg(feature = "storage")]
        {
            assert_eq!(
                nostr_error_to_transport_error(RadrootsRelayTransportError::EventStore(
                    radroots_event_store::RadrootsEventStoreError::MissingEvent(
                        "missing".to_owned(),
                    ),
                )),
                RadrootsTransportError::InvalidTransportKind
            );
            assert_eq!(
                nostr_error_to_transport_error(RadrootsRelayTransportError::Outbox(
                    radroots_outbox::RadrootsOutboxError::EventNotFound(1),
                )),
                RadrootsTransportError::InvalidTransportKind
            );
            assert_eq!(
                nostr_error_to_transport_error(
                    RadrootsRelayTransportError::MissingSignedOutboxEvent(1),
                ),
                RadrootsTransportError::InvalidTransportKind
            );
            assert_eq!(
                nostr_error_to_transport_error(
                    RadrootsRelayTransportError::MissingPersistedFetchReceiptEventId,
                ),
                RadrootsTransportError::InvalidTransportKind
            );
            assert_eq!(
                nostr_error_to_transport_error(
                    RadrootsRelayTransportError::MissingStoredEventVisibility {
                        event_id: "missing".to_owned(),
                    },
                ),
                RadrootsTransportError::InvalidTransportKind
            );
            assert_eq!(
                nostr_error_to_transport_error(
                    RadrootsRelayTransportError::UnsupportedStoredEventVisibility {
                        event_id: "unsupported".to_owned(),
                    },
                ),
                RadrootsTransportError::InvalidTransportKind
            );
        }
    }
}

fn signed_event_from_transport_payload(
    payload: &RadrootsTransportPayload,
) -> Result<RadrootsVerifiedSignedEvent, RadrootsTransportError> {
    let Some((event_id, raw_json)) = payload.signed_event_json_parts() else {
        return Err(RadrootsTransportError::InvalidPayloadBytes);
    };
    let wire = RadrootsNip01EventWire::parse_json(raw_json)
        .map_err(|_| RadrootsTransportError::InvalidPayloadBytes)?;
    if wire.id != event_id {
        return Err(RadrootsTransportError::InvalidPayloadId);
    }
    RadrootsSignedEvent::from_wire_verified_id(wire, raw_json.to_owned())
        .map_err(|_| RadrootsTransportError::InvalidPayloadBytes)?
        .verify_signature()
        .map_err(|_| RadrootsTransportError::InvalidPayloadSignature)
}

fn relay_targets_from_transport_targets(
    targets: &[RadrootsTransportTarget],
) -> Result<RadrootsRelayTargetSet, RadrootsTransportError> {
    let mut relays = Vec::new();
    for target in targets {
        if target.kind() != &RadrootsTransportKind::Nostr {
            return Err(RadrootsTransportError::InvalidTargetUri);
        }
        let policy = if target.uri().as_str().starts_with("ws://") {
            crate::RadrootsRelayUrlPolicy::Localhost
        } else {
            crate::RadrootsRelayUrlPolicy::Public
        };
        let relay = crate::RadrootsRelayUrl::parse(target.uri().as_str(), policy)
            .map_err(nostr_error_to_transport_error)?;
        if !relays.contains(&relay) {
            relays.push(relay);
        }
    }
    RadrootsRelayTargetSet::from_urls(relays).map_err(nostr_error_to_transport_error)
}

fn target_receipts_from_relay_receipts(
    targets: &[RadrootsTransportTarget],
    relay_receipts: &[RadrootsRelayPublishRelayReceipt],
) -> Result<Vec<RadrootsTransportTargetReceipt>, RadrootsTransportError> {
    targets
        .iter()
        .cloned()
        .map(|target| {
            let relay_receipt = relay_receipts
                .iter()
                .find(|receipt| relay_receipt_matches_target(receipt, &target));
            let receipt = match relay_receipt {
                Some(receipt) if receipt.attempted => RadrootsTransportTargetReceipt::attempted(
                    target,
                    receipt.outcome.to_transport_outcome()?,
                ),
                Some(receipt) => RadrootsTransportTargetReceipt::skipped(
                    target,
                    receipt.outcome.to_transport_outcome()?,
                )?,
                None => RadrootsTransportTargetReceipt::skipped(
                    target,
                    RadrootsTransportOutcome::new(RadrootsTransportOutcomeKind::RouteUnavailable)
                        .try_with_message("relay adapter omitted target receipt")?,
                )?,
            };
            Ok(receipt)
        })
        .collect()
}

fn transport_failure_target_receipts(
    targets: &[RadrootsTransportTarget],
    message: &str,
) -> Result<Vec<RadrootsTransportTargetReceipt>, RadrootsTransportError> {
    targets
        .iter()
        .cloned()
        .map(|target| {
            RadrootsTransportTargetReceipt::skipped(
                target,
                RadrootsTransportOutcome::new(RadrootsTransportOutcomeKind::ConnectionFailed)
                    .try_with_message(message.to_owned())?,
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
    request.validate()?;
    let event_id = request.signed_event.signed_event().id_str().to_owned();
    let satisfaction_policy = request.satisfaction_policy.clone();
    let requested_relays = request.targets.relay_strings();
    let target_count = request.targets.len();
    let quorum = satisfaction_policy.required_target_count(target_count)?;
    let relays =
        normalize_publish_receipts(requested_relays.as_slice(), adapter.publish(request).await?)?;
    let quorum_met = relay_publish_satisfies_policy(&satisfaction_policy, target_count, &relays)?;
    RadrootsRelayPublishReceipt::new(event_id, quorum, quorum_met, relays)
}

fn normalize_publish_receipts(
    requested_relays: &[String],
    receipts: Vec<RadrootsRelayPublishRelayReceipt>,
) -> Result<Vec<RadrootsRelayPublishRelayReceipt>, RadrootsRelayTransportError> {
    let requested = requested_relays.iter().cloned().collect::<BTreeSet<_>>();
    let mut by_relay = BTreeMap::new();
    for mut receipt in receipts {
        let target =
            RadrootsTransportTarget::nostr_relay(receipt.relay_url.as_str()).map_err(|error| {
                RadrootsRelayTransportError::InvalidPublishReceiptRelayUrl {
                    url: receipt.relay_url.clone(),
                    reason: error.to_string(),
                }
            })?;
        let canonical = target.uri().as_str().to_owned();
        if !requested.contains(&canonical) {
            return Err(
                RadrootsRelayTransportError::UnexpectedPublishReceiptRelayUrl { url: canonical },
            );
        }
        receipt.relay_url.clone_from(&canonical);
        if (!receipt.attempted
            && matches!(
                receipt.outcome.kind(),
                RadrootsRelayOutcomeKind::Accepted | RadrootsRelayOutcomeKind::DuplicateAccepted
            ))
            || (receipt.attempted
                && receipt.outcome.kind() == RadrootsRelayOutcomeKind::SkippedAlreadyAccepted)
        {
            return Err(
                RadrootsRelayTransportError::InvalidPublishReceiptAttemptState { url: canonical },
            );
        }
        if by_relay.insert(canonical.clone(), receipt).is_some() {
            return Err(
                RadrootsRelayTransportError::DuplicatePublishReceiptRelayUrl { url: canonical },
            );
        }
    }
    requested_relays
        .iter()
        .map(|relay_url| {
            if let Some(receipt) = by_relay.remove(relay_url) {
                Ok(receipt)
            } else {
                RadrootsRelayPublishRelayReceipt::skipped(
                    relay_url,
                    RadrootsRelayOutcome::unknown("relay adapter omitted target receipt")?,
                )
            }
        })
        .collect()
}

fn relay_receipt_counts_toward_quorum(receipt: &RadrootsRelayPublishRelayReceipt) -> bool {
    receipt.outcome.counts_toward_quorum()
        && (receipt.attempted
            || receipt.outcome.kind() == RadrootsRelayOutcomeKind::SkippedAlreadyAccepted)
}

fn relay_publish_satisfies_policy(
    policy: &RadrootsTransportSatisfactionPolicy,
    target_count: usize,
    relays: &[RadrootsRelayPublishRelayReceipt],
) -> Result<bool, RadrootsRelayTransportError> {
    let Some(class) = policy.target_satisfaction_class() else {
        return Ok(true);
    };
    let Some(targets) = policy.required_target_fingerprints() else {
        let satisfied_count = relays
            .iter()
            .filter(|receipt| {
                relay_receipt_counts_toward_quorum(receipt)
                    && receipt
                        .outcome
                        .kind()
                        .transport_outcome_kind()
                        .target_status()
                        .counts_as_satisfied(class)
            })
            .count();
        return Ok(policy.is_satisfied_by(target_count, satisfied_count)?);
    };
    policy.required_target_count(target_count)?;
    let mut satisfied_required_targets = BTreeSet::new();
    for receipt in relays {
        let target = RadrootsTransportTarget::nostr_relay(&receipt.relay_url)?;
        if targets.contains(target.fingerprint())
            && relay_receipt_counts_toward_quorum(receipt)
            && receipt
                .outcome
                .kind()
                .transport_outcome_kind()
                .target_status()
                .counts_as_satisfied(class)
        {
            satisfied_required_targets.insert(target.fingerprint().clone());
        }
    }
    Ok(targets
        .iter()
        .all(|target| satisfied_required_targets.contains(target)))
}

fn relay_receipt_matches_target(
    receipt: &RadrootsRelayPublishRelayReceipt,
    target: &RadrootsTransportTarget,
) -> bool {
    RadrootsTransportTarget::nostr_relay(receipt.relay_url.as_str())
        .is_ok_and(|receipt_target| receipt_target.uri() == target.uri())
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
                .push(request.signed_event.signed_event().raw_json().to_owned());
            request
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
                .collect()
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
            let event =
                RadrootsNostrEvent::from_json(request.signed_event.signed_event().raw_json())
                    .map_err(|error| {
                        RadrootsRelayTransportError::NostrEventJson(error.to_string())
                    })?;
            ensure_raw_event_matches_signed_event(&event, request.signed_event.signed_event())?;
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
                return target_strings
                    .into_iter()
                    .map(|relay_url| {
                        let target_url = relay_url.trim_end_matches('/');
                        let reason = connection_failures
                            .get(target_url)
                            .cloned()
                            .unwrap_or_else(|| "relay did not connect".to_owned());
                        RadrootsRelayPublishRelayReceipt::attempted(
                            relay_url,
                            RadrootsRelayOutcome::connection_failed(reason)?,
                        )
                    })
                    .collect();
            }
            let output = match self.client.send_event_to(connected_strings, &event).await {
                Ok(output) => output,
                Err(error) => {
                    let message = error.to_string();
                    return target_strings
                        .into_iter()
                        .map(|relay_url| {
                            RadrootsRelayPublishRelayReceipt::attempted(
                                relay_url,
                                RadrootsRelayOutcome::connection_failed(message.clone())?,
                            )
                        })
                        .collect();
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
                        RadrootsRelayOutcome::accepted_with_message(
                            "nostr-relay-pool-success-ok-message-unavailable",
                        )?,
                    )?);
                    continue;
                }
                if let Some(reason) = connection_failures.get(target_url) {
                    receipts.push(RadrootsRelayPublishRelayReceipt::attempted(
                        relay_url,
                        RadrootsRelayOutcome::connection_failed(reason.clone())?,
                    )?);
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
                    .transpose()?
                    .unwrap_or(RadrootsRelayOutcome::classify(
                        "error: relay output omitted target",
                    )?);
                receipts.push(RadrootsRelayPublishRelayReceipt::attempted(
                    relay_url, outcome,
                )?);
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
        ("id", event.id.to_hex(), signed_event.id_str().to_owned()),
        (
            "pubkey",
            event.pubkey.to_hex(),
            signed_event.pubkey_str().to_owned(),
        ),
        (
            "created_at",
            event.created_at.as_secs().to_string(),
            signed_event.created_at().to_string(),
        ),
        (
            "kind",
            (event.kind.as_u16() as u32).to_string(),
            signed_event.kind().to_string(),
        ),
        (
            "content",
            event.content.clone(),
            signed_event.content().to_owned(),
        ),
        (
            "sig",
            event.sig.to_string(),
            signed_event.sig_str().to_owned(),
        ),
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
    if raw_tags != signed_event.tags_as_vec() {
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
    use radroots_event::kinds::KIND_GEOCHAT;
    use radroots_event::wire::RadrootsNip01EventWire;
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
            "radroots.social.geochat.v1",
            KIND_GEOCHAT,
            1_700_000_000,
            vec![vec!["t".to_owned(), "soil".to_owned()]],
            content,
            FIXTURE_ALICE_PUBLIC_KEY_HEX,
        )
        .expect("draft");
        let signed_event = radroots_nostr_sign_frozen_draft(&keys, &draft).expect("signed event");
        let raw_event = RadrootsNostrEvent::from_json(signed_event.raw_json()).expect("raw event");
        (raw_event, signed_event)
    }

    fn assert_mismatch(raw_event: RadrootsNostrEvent, signed_event: &RadrootsSignedEvent) {
        assert!(ensure_raw_event_matches_signed_event(&raw_event, signed_event).is_err());
    }

    fn raw_event_from_wire(wire: RadrootsNip01EventWire) -> RadrootsNostrEvent {
        RadrootsNostrEvent::from_json(serde_json::to_string(&wire).expect("raw event json"))
            .expect("raw event")
    }

    #[test]
    fn raw_event_match_guard_accepts_exact_event_and_rejects_field_mismatches() {
        let (raw_event, signed_event) = signed_post("matched");
        ensure_raw_event_matches_signed_event(&raw_event, &signed_event).expect("matching event");

        let mut wire = signed_event.wire().clone();
        wire.id = "00".repeat(32);
        assert_mismatch(raw_event_from_wire(wire), &signed_event);

        let mut wire = signed_event.wire().clone();
        wire.pubkey = "11".repeat(32);
        assert_mismatch(raw_event_from_wire(wire), &signed_event);

        let mut wire = signed_event.wire().clone();
        wire.created_at += 1;
        assert_mismatch(raw_event_from_wire(wire), &signed_event);

        let mut wire = signed_event.wire().clone();
        wire.kind += 1;
        assert_mismatch(raw_event_from_wire(wire), &signed_event);

        let mut wire = signed_event.wire().clone();
        wire.content.push_str(" changed");
        assert_mismatch(raw_event_from_wire(wire), &signed_event);

        let mut wire = signed_event.wire().clone();
        wire.sig = "22".repeat(64);
        assert_mismatch(raw_event_from_wire(wire), &signed_event);

        let mut wire = signed_event.wire().clone();
        wire.tags.push(vec!["t".to_owned(), "compost".to_owned()]);
        assert_mismatch(raw_event_from_wire(wire), &signed_event);
    }
}
