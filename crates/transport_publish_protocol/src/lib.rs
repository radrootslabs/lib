#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::{collections::BTreeSet, string::String, vec::Vec};
#[cfg(feature = "std")]
use std::{collections::BTreeSet, string::String, vec::Vec};

use core::fmt;
use radroots_protocol::radrootsd::transport_publish::v5::{
    RETICULUM_ENDPOINT_URI as RADROOTS_RETICULUM_ENDPOINT_URI,
    RETICULUM_UNAVAILABLE_MESSAGE as RADROOTS_RETICULUM_UNAVAILABLE_MESSAGE,
};
use radroots_transport::target::{TargetFingerprint, TargetLabel, TargetScope};
use radroots_transport::{RadrootsTransportError, Target, TransportId};

pub const API_VERSION: &str = "radrootsd.transport_publish.v5";
pub const DAEMON_NAME: &str = "radrootsd";
pub const METHOD_CAPABILITIES: &str = "transport.publish.capabilities";
pub const METHOD_EVENT: &str = "transport.publish.event";
pub const METHOD_JOB_GET: &str = "transport.publish.job.get";
pub const METHOD_JOB_LIST: &str = "transport.publish.job.list";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransportPublishProtocolError {
    InvalidHexField {
        field: &'static str,
        expected_len: usize,
    },
    EmptyRawEventJson,
    EmptyTag {
        index: usize,
    },
    EmptyIdempotencyKey,
    EmptyTransportKind {
        index: usize,
    },
    InvalidTransportKind {
        index: usize,
    },
    EmptyEndpointUri {
        index: usize,
    },
    InvalidEndpointUri {
        index: usize,
    },
    EmptyTargetScope {
        index: usize,
    },
    InvalidTargetScope {
        index: usize,
    },
    EmptyTargetLabel {
        index: usize,
    },
    InvalidTargetLabel {
        index: usize,
    },
    InvalidReticulumBehavior {
        index: usize,
    },
    InvalidTimeoutMs,
    InvalidReticulumEndpoint {
        index: usize,
    },
    DuplicateTarget {
        index: usize,
    },
    TargetLimitExceeded {
        max: usize,
        actual: usize,
    },
    EmptyTargetSet,
    InvalidQuorum,
    EmptyRequiredTargetSet,
    DuplicateRequiredTargetFingerprint {
        index: usize,
    },
    RequiredTargetNotInTargetSet {
        index: usize,
    },
    EmptyPrincipalId,
    EmptyJobId,
    InvalidJobTargetCount {
        expected: usize,
        actual: usize,
    },
    InvalidJobAcknowledgedCount {
        expected: usize,
        actual: usize,
    },
    InvalidJobRetryableCount {
        expected: usize,
        actual: usize,
    },
    InvalidJobTerminalCount {
        expected: usize,
        actual: usize,
    },
    InvalidJobTerminalState,
    InvalidJobDeliverySatisfiedState,
    InvalidJobCompletedAt,
    InvalidJobStatusState,
    InvalidExplicitTargetOutcome {
        index: usize,
    },
    InvalidTargetOutcomeKind {
        index: usize,
    },
    InvalidTargetSource {
        index: usize,
    },
    InvalidReticulumOutcome {
        index: usize,
    },
}

impl fmt::Display for TransportPublishProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidHexField {
                field,
                expected_len,
            } => write!(f, "{field} must be {expected_len} lowercase hex characters"),
            Self::EmptyRawEventJson => f.write_str("raw_event_json must not be empty"),
            Self::EmptyTag { index } => write!(f, "tag {index} must not be empty"),
            Self::EmptyIdempotencyKey => f.write_str("idempotency key must not be empty"),
            Self::EmptyTransportKind { index } => {
                write!(f, "transport target {index} kind must not be empty")
            }
            Self::InvalidTransportKind { index } => {
                write!(
                    f,
                    "transport target {index} kind must be canonical lowercase"
                )
            }
            Self::EmptyEndpointUri { index } => {
                write!(f, "transport target {index} endpoint_uri must not be empty")
            }
            Self::InvalidEndpointUri { index } => {
                write!(f, "transport target {index} endpoint_uri is invalid")
            }
            Self::EmptyTargetScope { index } => {
                write!(f, "transport target {index} target_scope must not be empty")
            }
            Self::InvalidTargetScope { index } => {
                write!(f, "transport target {index} target_scope must be canonical")
            }
            Self::EmptyTargetLabel { index } => {
                write!(f, "transport target {index} target_label must not be empty")
            }
            Self::InvalidTargetLabel { index } => {
                write!(f, "transport target {index} target_label is invalid")
            }
            Self::InvalidReticulumBehavior { index } => write!(
                f,
                "transport target {index} reticulum_behavior is only valid for Reticulum targets"
            ),
            Self::InvalidTimeoutMs => f.write_str("timeout_ms must be greater than zero"),
            Self::InvalidReticulumEndpoint { index } => write!(
                f,
                "transport target {index} Reticulum endpoint must be {RADROOTS_RETICULUM_ENDPOINT_URI}"
            ),
            Self::DuplicateTarget { index } => {
                write!(f, "transport target {index} duplicates an earlier target")
            }
            Self::TargetLimitExceeded { max, actual } => {
                write!(f, "transport target count {actual} exceeds limit {max}")
            }
            Self::EmptyTargetSet => f.write_str("transport publish target set must not be empty"),
            Self::InvalidQuorum => f.write_str("delivery quorum must be greater than zero"),
            Self::EmptyRequiredTargetSet => {
                f.write_str("delivery required target set must not be empty")
            }
            Self::DuplicateRequiredTargetFingerprint { index } => {
                write!(
                    f,
                    "delivery required target {index} duplicates an earlier fingerprint"
                )
            }
            Self::RequiredTargetNotInTargetSet { index } => {
                write!(
                    f,
                    "delivery required target {index} is not in the target set"
                )
            }
            Self::EmptyPrincipalId => f.write_str("principal id must not be empty"),
            Self::EmptyJobId => f.write_str("job id must not be empty"),
            Self::InvalidJobTargetCount { expected, actual } => write!(
                f,
                "job target_count {actual} does not match {expected} target outcomes"
            ),
            Self::InvalidJobAcknowledgedCount { expected, actual } => write!(
                f,
                "job acknowledged_count {actual} does not match {expected} target outcomes"
            ),
            Self::InvalidJobRetryableCount { expected, actual } => write!(
                f,
                "job retryable_count {actual} does not match {expected} target outcomes"
            ),
            Self::InvalidJobTerminalCount { expected, actual } => write!(
                f,
                "job terminal_count {actual} does not match {expected} target outcomes"
            ),
            Self::InvalidJobTerminalState => f.write_str("job terminal flag does not match status"),
            Self::InvalidJobDeliverySatisfiedState => {
                f.write_str("job delivery_satisfied flag does not match status")
            }
            Self::InvalidJobCompletedAt => {
                f.write_str("job completed_at_ms does not match status or request time")
            }
            Self::InvalidJobStatusState => f.write_str("job status does not match target outcomes"),
            Self::InvalidExplicitTargetOutcome { index } => write!(
                f,
                "transport target outcome {index} does not match explicit target policy"
            ),
            Self::InvalidTargetOutcomeKind { index } => {
                write!(
                    f,
                    "transport target outcome {index} kind is not valid for its transport"
                )
            }
            Self::InvalidTargetSource { index } => {
                write!(
                    f,
                    "transport target outcome {index} source does not match transport kind"
                )
            }
            Self::InvalidReticulumOutcome { index } => write!(
                f,
                "transport target outcome {index} Reticulum must be unavailable or deferred"
            ),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for TransportPublishProtocolError {}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TransportPublishReticulumBehavior {
    #[default]
    RejectDeliveryAttempts,
    DeferDeliveryPlans,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransportPublishTarget {
    pub transport_kind: String,
    pub endpoint_uri: String,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub target_scope: Option<String>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub target_label: Option<String>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub reticulum_behavior: Option<TransportPublishReticulumBehavior>,
}

impl TransportPublishTarget {
    pub fn nostr(endpoint_uri: impl Into<String>) -> Self {
        Self {
            transport_kind: "nostr".to_owned(),
            endpoint_uri: endpoint_uri.into(),
            target_scope: None,
            target_label: None,
            reticulum_behavior: None,
        }
    }

    pub fn reticulum(behavior: TransportPublishReticulumBehavior) -> Self {
        Self {
            transport_kind: "reticulum".to_owned(),
            endpoint_uri: RADROOTS_RETICULUM_ENDPOINT_URI.to_owned(),
            target_scope: None,
            target_label: None,
            reticulum_behavior: Some(behavior),
        }
    }

    pub fn with_scope(mut self, target_scope: impl Into<String>) -> Self {
        self.target_scope = Some(target_scope.into());
        self
    }

    pub fn with_label(mut self, target_label: impl Into<String>) -> Self {
        self.target_label = Some(target_label.into());
        self
    }

    fn validate(&self, index: usize) -> Result<(), TransportPublishProtocolError> {
        if self.transport_kind.trim().is_empty() {
            return Err(TransportPublishProtocolError::EmptyTransportKind { index });
        }
        let transport_kind = TransportId::parse_canonical(self.transport_kind.as_str())
            .map_err(|error| transport_kind_error(error, index))?;
        if self.endpoint_uri.trim().is_empty() {
            return Err(TransportPublishProtocolError::EmptyEndpointUri { index });
        }
        if transport_kind != TransportId::RETICULUM && self.reticulum_behavior.is_some() {
            return Err(TransportPublishProtocolError::InvalidReticulumBehavior { index });
        }
        if transport_kind == TransportId::RETICULUM
            && self.endpoint_uri != RADROOTS_RETICULUM_ENDPOINT_URI
        {
            return Err(TransportPublishProtocolError::InvalidReticulumEndpoint { index });
        }
        validate_target_metadata(
            self.target_scope.as_deref(),
            self.target_label.as_deref(),
            index,
        )?;
        self.canonical_target(index)?;
        Ok(())
    }

    fn fingerprint(
        &self,
        index: usize,
    ) -> Result<TargetFingerprint, TransportPublishProtocolError> {
        Ok(self.canonical_target(index)?.fingerprint().clone())
    }

    fn canonical_target(&self, index: usize) -> Result<Target, TransportPublishProtocolError> {
        let transport_kind = TransportId::parse_canonical(self.transport_kind.as_str())
            .map_err(|error| transport_kind_error(error, index))?;
        let scope = self
            .target_scope
            .as_deref()
            .map(TargetScope::parse)
            .transpose()
            .map_err(|error| target_metadata_error(error, index))?;
        let label = self
            .target_label
            .as_deref()
            .map(TargetLabel::parse)
            .transpose()
            .map_err(|error| target_metadata_error(error, index))?;
        let target =
            transport_target_from_parts(transport_kind, self.endpoint_uri.as_str(), scope, label)
                .map_err(|error| target_fingerprint_error(error, index))?;
        if target.uri().as_str() != self.endpoint_uri {
            return Err(TransportPublishProtocolError::InvalidEndpointUri { index });
        }
        Ok(target)
    }

    fn identity_eq(
        &self,
        target_index: usize,
        outcome: &TransportPublishTargetOutcome,
        outcome_index: usize,
    ) -> Result<bool, TransportPublishProtocolError> {
        if self.transport_kind != outcome.transport_kind
            || self.target_scope != outcome.target_scope
        {
            return Ok(false);
        }
        Ok(self.fingerprint(target_index)? == target_outcome_fingerprint(outcome, outcome_index)?)
    }
}

fn validate_target_metadata(
    target_scope: Option<&str>,
    target_label: Option<&str>,
    index: usize,
) -> Result<(), TransportPublishProtocolError> {
    if let Some(scope) = target_scope {
        TargetScope::parse(scope).map_err(|error| target_metadata_error(error, index))?;
    }
    if let Some(label) = target_label {
        TargetLabel::parse(label).map_err(|error| target_metadata_error(error, index))?;
    }
    Ok(())
}

fn transport_kind_error(
    error: RadrootsTransportError,
    index: usize,
) -> TransportPublishProtocolError {
    match error {
        RadrootsTransportError::EmptyTransportKind => {
            TransportPublishProtocolError::EmptyTransportKind { index }
        }
        _ => TransportPublishProtocolError::InvalidTransportKind { index },
    }
}

fn target_fingerprint_error(
    error: RadrootsTransportError,
    index: usize,
) -> TransportPublishProtocolError {
    match error {
        RadrootsTransportError::EmptyTargetUri => {
            TransportPublishProtocolError::EmptyEndpointUri { index }
        }
        _ => TransportPublishProtocolError::InvalidEndpointUri { index },
    }
}

fn target_metadata_error(
    error: RadrootsTransportError,
    index: usize,
) -> TransportPublishProtocolError {
    match error {
        RadrootsTransportError::EmptyTargetScope => {
            TransportPublishProtocolError::EmptyTargetScope { index }
        }
        RadrootsTransportError::InvalidTargetScope => {
            TransportPublishProtocolError::InvalidTargetScope { index }
        }
        RadrootsTransportError::EmptyTargetLabel => {
            TransportPublishProtocolError::EmptyTargetLabel { index }
        }
        RadrootsTransportError::InvalidTargetLabel => {
            TransportPublishProtocolError::InvalidTargetLabel { index }
        }
        _ => TransportPublishProtocolError::InvalidEndpointUri { index },
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NostrPublishTargetSourcePolicy {
    ExplicitOnly,
    RequestThenAuthorWriteThenDaemonDefault,
    AuthorWriteThenDaemonDefault,
    DaemonDefaultOnly,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "kind", rename_all = "snake_case"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransportPublishTargetPolicy {
    ExplicitTargets {
        targets: Vec<TransportPublishTarget>,
    },
    Nostr {
        source_policy: NostrPublishTargetSourcePolicy,
        #[cfg_attr(feature = "serde", serde(default))]
        relay_urls: Vec<String>,
    },
}

impl TransportPublishTargetPolicy {
    pub fn explicit_targets(targets: Vec<TransportPublishTarget>) -> Self {
        Self::ExplicitTargets { targets }
    }

    pub fn nostr(source_policy: NostrPublishTargetSourcePolicy, relay_urls: Vec<String>) -> Self {
        Self::Nostr {
            source_policy,
            relay_urls,
        }
    }

    pub fn request_target_count(&self) -> usize {
        match self {
            Self::ExplicitTargets { targets } => targets.len(),
            Self::Nostr { relay_urls, .. } => relay_urls.len(),
        }
    }

    fn validate(&self, max_targets: usize) -> Result<(), TransportPublishProtocolError> {
        match self {
            Self::ExplicitTargets { targets } => {
                validate_target_limit(targets.len(), max_targets)?;
                if targets.is_empty() {
                    return Err(TransportPublishProtocolError::EmptyTargetSet);
                }
                for (index, target) in targets.iter().enumerate() {
                    target.validate(index)?;
                }
                validate_explicit_target_uniqueness(targets)?;
            }
            Self::Nostr { relay_urls, .. } => {
                validate_target_limit(relay_urls.len(), max_targets)?;
                let mut fingerprints = BTreeSet::new();
                for (index, endpoint_uri) in relay_urls.iter().enumerate() {
                    if endpoint_uri.trim().is_empty() {
                        return Err(TransportPublishProtocolError::EmptyEndpointUri { index });
                    }
                    let target = Target::new(TransportId::NOSTR, endpoint_uri)
                        .map_err(|error| target_fingerprint_error(error, index))?;
                    if target.uri().as_str() != endpoint_uri {
                        return Err(TransportPublishProtocolError::InvalidEndpointUri { index });
                    }
                    if !fingerprints.insert(target.fingerprint().clone()) {
                        return Err(TransportPublishProtocolError::DuplicateTarget { index });
                    }
                }
            }
        }
        Ok(())
    }
}

fn validate_explicit_target_uniqueness(
    targets: &[TransportPublishTarget],
) -> Result<(), TransportPublishProtocolError> {
    let mut identities = Vec::new();
    for (index, target) in targets.iter().enumerate() {
        let fingerprint = target.fingerprint(index)?;
        let target_scope = target.target_scope.as_deref();
        if identities
            .iter()
            .any(|(existing_fingerprint, existing_scope)| {
                existing_fingerprint == &fingerprint && *existing_scope == target_scope
            })
        {
            return Err(TransportPublishProtocolError::DuplicateTarget { index });
        }
        identities.push((fingerprint, target_scope));
    }
    Ok(())
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "mode", rename_all = "snake_case"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransportPublishDeliveryPolicy {
    Any,
    All,
    Quorum { quorum: usize },
    RequiredTargets { targets: Vec<TargetFingerprint> },
}

impl TransportPublishDeliveryPolicy {
    pub fn required_targets(
        targets: Vec<TargetFingerprint>,
    ) -> Result<Self, TransportPublishProtocolError> {
        validate_required_target_fingerprints(&targets)?;
        Ok(Self::RequiredTargets { targets })
    }

    pub fn validate(&self) -> Result<(), TransportPublishProtocolError> {
        match self {
            Self::Quorum { quorum: 0 } => Err(TransportPublishProtocolError::InvalidQuorum),
            Self::RequiredTargets { targets } => validate_required_target_fingerprints(targets),
            Self::Any | Self::All | Self::Quorum { .. } => Ok(()),
        }
    }

    pub fn required_target_count(&self, target_count: usize) -> usize {
        match self {
            Self::Any => usize::from(target_count > 0),
            Self::All => target_count,
            Self::Quorum { quorum } => *quorum,
            Self::RequiredTargets { targets } => targets.len(),
        }
    }

    pub fn validate_target_membership(
        &self,
        target_fingerprints: &[TargetFingerprint],
    ) -> Result<(), TransportPublishProtocolError> {
        let Self::RequiredTargets { targets } = self else {
            return Ok(());
        };
        validate_required_target_fingerprints(targets)?;
        for (index, required) in targets.iter().enumerate() {
            if !target_fingerprints
                .iter()
                .any(|fingerprint| fingerprint == required)
            {
                return Err(TransportPublishProtocolError::RequiredTargetNotInTargetSet { index });
            }
        }
        Ok(())
    }
}

fn validate_required_target_fingerprints(
    targets: &[TargetFingerprint],
) -> Result<(), TransportPublishProtocolError> {
    if targets.is_empty() {
        return Err(TransportPublishProtocolError::EmptyRequiredTargetSet);
    }
    let mut seen = BTreeSet::new();
    for (index, target) in targets.iter().enumerate() {
        if !seen.insert(target.as_str()) {
            return Err(
                TransportPublishProtocolError::DuplicateRequiredTargetFingerprint { index },
            );
        }
    }
    Ok(())
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransportPublishEventRequest {
    pub raw_event_json: String,
    pub target_policy: TransportPublishTargetPolicy,
    pub delivery_policy: TransportPublishDeliveryPolicy,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub idempotency_key: Option<String>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub timeout_ms: Option<u64>,
}

impl TransportPublishEventRequest {
    pub fn validate(&self, max_targets: usize) -> Result<(), TransportPublishProtocolError> {
        if self.raw_event_json.is_empty() {
            return Err(TransportPublishProtocolError::EmptyRawEventJson);
        }
        self.target_policy.validate(max_targets)?;
        self.delivery_policy.validate()?;
        if let TransportPublishTargetPolicy::ExplicitTargets { targets } = &self.target_policy {
            let target_fingerprints = targets
                .iter()
                .enumerate()
                .map(|(index, target)| target.fingerprint(index))
                .collect::<Result<Vec<_>, _>>()?;
            self.delivery_policy
                .validate_target_membership(&target_fingerprints)?;
        }
        if self
            .idempotency_key
            .as_ref()
            .is_some_and(|key| key.trim().is_empty())
        {
            return Err(TransportPublishProtocolError::EmptyIdempotencyKey);
        }
        if self.timeout_ms == Some(0) {
            return Err(TransportPublishProtocolError::InvalidTimeoutMs);
        }
        Ok(())
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransportPublishJobStatus {
    Accepted,
    Publishing,
    DeliverySatisfied,
    DeliveryUnsatisfiedRetryable,
    DeliveryUnsatisfiedTerminal,
    DeliveryDeferred,
    DeliveryDeferredUntilImplemented,
    Rejected,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransportPublishOutcomeKind {
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
    TargetRejected,
    SkippedAlreadyAccepted,
    DeferredUntilImplemented,
    Unknown,
}

impl TransportPublishOutcomeKind {
    pub fn counts_toward_accepted_delivery(self) -> bool {
        matches!(
            self,
            Self::Accepted | Self::DuplicateAccepted | Self::SkippedAlreadyAccepted
        )
    }

    pub fn is_retryable(self) -> bool {
        matches!(
            self,
            Self::RateLimited
                | Self::PowRequired
                | Self::AuthRequired
                | Self::Error
                | Self::Timeout
                | Self::ConnectionFailed
                | Self::Unknown
        )
    }

    pub fn is_terminal_failure(self) -> bool {
        matches!(
            self,
            Self::Blocked
                | Self::Invalid
                | Self::Restricted
                | Self::Muted
                | Self::Unsupported
                | Self::PaymentRequired
                | Self::TargetRejected
        )
    }

    pub fn is_deferred_until_implemented(self) -> bool {
        matches!(self, Self::DeferredUntilImplemented)
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransportPublishTargetSource {
    Request,
    NostrAuthorWrite,
    DaemonDefault,
    Reticulum,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransportPublishTargetOutcome {
    pub transport_kind: String,
    pub endpoint_uri: String,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub target_scope: Option<String>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub target_label: Option<String>,
    pub source: TransportPublishTargetSource,
    pub attempted: bool,
    pub outcome_kind: TransportPublishOutcomeKind,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub message: Option<String>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub latency_ms: Option<u64>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransportPublishJobView {
    pub job_id: String,
    pub status: TransportPublishJobStatus,
    pub terminal: bool,
    pub delivery_satisfied: bool,
    pub event_id: String,
    pub pubkey: String,
    pub event_kind: u32,
    pub target_policy: TransportPublishTargetPolicy,
    pub delivery_policy: TransportPublishDeliveryPolicy,
    pub target_count: usize,
    pub acknowledged_count: usize,
    pub retryable_count: usize,
    pub terminal_count: usize,
    pub requested_at_ms: i64,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub completed_at_ms: Option<i64>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub last_error: Option<String>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub targets: Vec<TransportPublishTargetOutcome>,
}

impl TransportPublishJobView {
    pub fn validate(&self) -> Result<(), TransportPublishProtocolError> {
        if self.job_id.trim().is_empty() {
            return Err(TransportPublishProtocolError::EmptyJobId);
        }
        validate_lower_hex("event_id", self.event_id.as_str(), 64)?;
        validate_lower_hex("pubkey", self.pubkey.as_str(), 64)?;
        self.target_policy.validate(usize::MAX)?;
        self.delivery_policy.validate()?;
        if self.terminal != job_status_is_terminal(self.status) {
            return Err(TransportPublishProtocolError::InvalidJobTerminalState);
        }
        if self.delivery_satisfied != (self.status == TransportPublishJobStatus::DeliverySatisfied)
        {
            return Err(TransportPublishProtocolError::InvalidJobDeliverySatisfiedState);
        }
        let completed = job_status_has_completed_at(self.status);
        if self.completed_at_ms.is_some() != completed {
            return Err(TransportPublishProtocolError::InvalidJobCompletedAt);
        }
        if self
            .completed_at_ms
            .is_some_and(|completed_at_ms| completed_at_ms < self.requested_at_ms)
        {
            return Err(TransportPublishProtocolError::InvalidJobCompletedAt);
        }
        for (index, target) in self.targets.iter().enumerate() {
            validate_target_outcome(target, index)?;
        }
        validate_job_target_policy_outcomes(&self.target_policy, &self.targets)?;
        let has_outcomes = !self.targets.is_empty();
        if has_outcomes || completed {
            if self.target_count != self.targets.len() {
                return Err(TransportPublishProtocolError::InvalidJobTargetCount {
                    expected: self.targets.len(),
                    actual: self.target_count,
                });
            }
            if matches!(
                self.delivery_policy,
                TransportPublishDeliveryPolicy::RequiredTargets { .. }
            ) {
                let target_fingerprints = self
                    .targets
                    .iter()
                    .enumerate()
                    .map(|(index, target)| target_outcome_fingerprint(target, index))
                    .collect::<Result<Vec<_>, _>>()?;
                self.delivery_policy
                    .validate_target_membership(&target_fingerprints)?;
            }
        }
        let acknowledged_count = self
            .targets
            .iter()
            .filter(|target| target.outcome_kind.counts_toward_accepted_delivery())
            .count();
        let retryable_count = self
            .targets
            .iter()
            .filter(|target| target.outcome_kind.is_retryable())
            .count();
        let terminal_count = self
            .targets
            .iter()
            .filter(|target| target.outcome_kind.is_terminal_failure())
            .count();
        if self.acknowledged_count != acknowledged_count {
            return Err(TransportPublishProtocolError::InvalidJobAcknowledgedCount {
                expected: acknowledged_count,
                actual: self.acknowledged_count,
            });
        }
        if self.retryable_count != retryable_count {
            return Err(TransportPublishProtocolError::InvalidJobRetryableCount {
                expected: retryable_count,
                actual: self.retryable_count,
            });
        }
        if self.terminal_count != terminal_count {
            return Err(TransportPublishProtocolError::InvalidJobTerminalCount {
                expected: terminal_count,
                actual: self.terminal_count,
            });
        }
        validate_job_status_state(self, acknowledged_count, retryable_count, terminal_count)
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransportPublishEventResponse {
    pub deduplicated: bool,
    pub job: TransportPublishJobView,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransportPublishCapabilities {
    pub daemon: String,
    pub api_version: String,
    pub transports: Vec<String>,
    pub methods: Vec<String>,
    pub auth: TransportPublishAuthCapabilities,
    pub publish: TransportPublishSurfaceCapabilities,
}

impl TransportPublishCapabilities {
    pub fn v5(max_event_bytes: usize, max_targets_per_request: usize) -> Self {
        Self {
            daemon: DAEMON_NAME.to_owned(),
            api_version: API_VERSION.to_owned(),
            transports: vec!["jsonrpc_http".to_owned()],
            methods: vec![
                METHOD_CAPABILITIES.to_owned(),
                METHOD_EVENT.to_owned(),
                METHOD_JOB_GET.to_owned(),
                METHOD_JOB_LIST.to_owned(),
            ],
            auth: TransportPublishAuthCapabilities {
                mode: "scoped_bearer_token".to_owned(),
            },
            publish: TransportPublishSurfaceCapabilities {
                raw_event_json_ingress: true,
                server_side_user_signing: false,
                max_event_bytes,
                max_targets_per_request,
                delivery_policies: vec![
                    TransportPublishDeliveryPolicyName::Any,
                    TransportPublishDeliveryPolicyName::Quorum,
                    TransportPublishDeliveryPolicyName::All,
                    TransportPublishDeliveryPolicyName::RequiredTargets,
                ],
                target_policy_modes: vec![
                    TransportPublishTargetPolicyName::ExplicitTargets,
                    TransportPublishTargetPolicyName::Nostr,
                ],
                transports: vec![
                    TransportPublishTransportCapability {
                        transport: "nostr".to_owned(),
                        configured: true,
                        implementation: TransportPublishImplementation::Real,
                        maturity: TransportPublishCapabilityMaturity::Stable,
                        availability: TransportPublishCapabilityAvailability::Available,
                        usable_for_delivery: true,
                        capabilities: TransportPublishOperationCapabilities {
                            deliver: true,
                            fetch: false,
                            discovery: false,
                            gateway_forwarding: false,
                            receipt_observation: false,
                        },
                        reticulum_behavior: None,
                        message: "Nostr relay publish is available".to_owned(),
                    },
                    TransportPublishTransportCapability {
                        transport: "reticulum".to_owned(),
                        configured: true,
                        implementation: TransportPublishImplementation::Real,
                        maturity: TransportPublishCapabilityMaturity::Preview,
                        availability: TransportPublishCapabilityAvailability::Unavailable,
                        usable_for_delivery: false,
                        capabilities: TransportPublishOperationCapabilities {
                            deliver: false,
                            fetch: false,
                            discovery: false,
                            gateway_forwarding: false,
                            receipt_observation: false,
                        },
                        reticulum_behavior: Some(
                            TransportPublishReticulumBehavior::RejectDeliveryAttempts,
                        ),
                        message: RADROOTS_RETICULUM_UNAVAILABLE_MESSAGE.to_owned(),
                    },
                ],
            },
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransportPublishAuthCapabilities {
    pub mode: String,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransportPublishSurfaceCapabilities {
    pub raw_event_json_ingress: bool,
    pub server_side_user_signing: bool,
    pub max_event_bytes: usize,
    pub max_targets_per_request: usize,
    pub delivery_policies: Vec<TransportPublishDeliveryPolicyName>,
    pub target_policy_modes: Vec<TransportPublishTargetPolicyName>,
    pub transports: Vec<TransportPublishTransportCapability>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransportPublishImplementation {
    Real,
    Mock,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransportPublishCapabilityMaturity {
    Preview,
    Stable,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransportPublishCapabilityAvailability {
    Available,
    Degraded,
    Unavailable,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransportPublishTransportCapability {
    pub transport: String,
    pub configured: bool,
    pub implementation: TransportPublishImplementation,
    pub maturity: TransportPublishCapabilityMaturity,
    pub availability: TransportPublishCapabilityAvailability,
    pub usable_for_delivery: bool,
    pub capabilities: TransportPublishOperationCapabilities,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub reticulum_behavior: Option<TransportPublishReticulumBehavior>,
    pub message: String,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransportPublishOperationCapabilities {
    pub deliver: bool,
    pub fetch: bool,
    pub discovery: bool,
    pub gateway_forwarding: bool,
    pub receipt_observation: bool,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransportPublishDeliveryPolicyName {
    Any,
    Quorum,
    All,
    RequiredTargets,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransportPublishTargetPolicyName {
    ExplicitTargets,
    Nostr,
}

fn validate_target_limit(
    target_count: usize,
    max_targets: usize,
) -> Result<(), TransportPublishProtocolError> {
    if target_count > max_targets {
        Err(TransportPublishProtocolError::TargetLimitExceeded {
            max: max_targets,
            actual: target_count,
        })
    } else {
        Ok(())
    }
}

fn validate_lower_hex(
    field: &'static str,
    value: &str,
    expected_len: usize,
) -> Result<(), TransportPublishProtocolError> {
    if value.len() == expected_len
        && value
            .as_bytes()
            .iter()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    {
        Ok(())
    } else {
        Err(TransportPublishProtocolError::InvalidHexField {
            field,
            expected_len,
        })
    }
}

fn job_status_is_terminal(status: TransportPublishJobStatus) -> bool {
    matches!(
        status,
        TransportPublishJobStatus::DeliverySatisfied
            | TransportPublishJobStatus::DeliveryUnsatisfiedTerminal
            | TransportPublishJobStatus::DeliveryDeferred
            | TransportPublishJobStatus::DeliveryDeferredUntilImplemented
            | TransportPublishJobStatus::Rejected
    )
}

fn job_status_has_completed_at(status: TransportPublishJobStatus) -> bool {
    !matches!(
        status,
        TransportPublishJobStatus::Accepted | TransportPublishJobStatus::Publishing
    )
}

fn validate_target_outcome(
    target: &TransportPublishTargetOutcome,
    index: usize,
) -> Result<(), TransportPublishProtocolError> {
    if target.transport_kind.trim().is_empty() {
        return Err(TransportPublishProtocolError::EmptyTransportKind { index });
    }
    let transport_kind = TransportId::parse_canonical(target.transport_kind.as_str())
        .map_err(|error| transport_kind_error(error, index))?;
    if target.endpoint_uri.trim().is_empty() {
        return Err(TransportPublishProtocolError::EmptyEndpointUri { index });
    }
    validate_target_metadata(
        target.target_scope.as_deref(),
        target.target_label.as_deref(),
        index,
    )?;
    if transport_kind == TransportId::RETICULUM {
        if target.endpoint_uri != RADROOTS_RETICULUM_ENDPOINT_URI {
            return Err(TransportPublishProtocolError::InvalidReticulumEndpoint { index });
        }
        if target.source != TransportPublishTargetSource::Reticulum {
            return Err(TransportPublishProtocolError::InvalidTargetSource { index });
        }
        if target.attempted || !target.outcome_kind.is_deferred_until_implemented() {
            return Err(TransportPublishProtocolError::InvalidReticulumOutcome { index });
        }
        return Ok(());
    }
    if target.source == TransportPublishTargetSource::Reticulum {
        return Err(TransportPublishProtocolError::InvalidTargetSource { index });
    }
    if target.outcome_kind.is_deferred_until_implemented() {
        return Err(TransportPublishProtocolError::InvalidTargetOutcomeKind { index });
    }
    target_outcome_fingerprint(target, index)?;
    Ok(())
}

fn target_outcome_fingerprint(
    target: &TransportPublishTargetOutcome,
    index: usize,
) -> Result<TargetFingerprint, TransportPublishProtocolError> {
    let transport_kind = TransportId::parse_canonical(target.transport_kind.as_str())
        .map_err(|error| transport_kind_error(error, index))?;
    let scope = target
        .target_scope
        .as_deref()
        .map(TargetScope::parse)
        .transpose()
        .map_err(|error| target_metadata_error(error, index))?;
    let label = target
        .target_label
        .as_deref()
        .map(TargetLabel::parse)
        .transpose()
        .map_err(|error| target_metadata_error(error, index))?;
    let canonical_target =
        transport_target_from_parts(transport_kind, target.endpoint_uri.as_str(), scope, label)
            .map_err(|error| target_fingerprint_error(error, index))?;
    if canonical_target.uri().as_str() != target.endpoint_uri {
        return Err(TransportPublishProtocolError::InvalidEndpointUri { index });
    }
    Ok(canonical_target.fingerprint().clone())
}

fn transport_target_from_parts(
    transport_kind: TransportId,
    endpoint_uri: &str,
    scope: Option<TargetScope>,
    label: Option<TargetLabel>,
) -> Result<Target, RadrootsTransportError> {
    match transport_kind {
        TransportId::NOSTR => {
            Target::new_with_metadata(TransportId::NOSTR, endpoint_uri, scope, label)
        }
        TransportId::RETICULUM => {
            if endpoint_uri != RADROOTS_RETICULUM_ENDPOINT_URI {
                return Err(RadrootsTransportError::InvalidTargetUri);
            }
            Target::new_with_metadata(TransportId::RETICULUM, endpoint_uri, scope, label)
        }
        TransportId::LOCAL => {
            Target::new_with_metadata(TransportId::LOCAL, endpoint_uri, scope, label)
        }
        _ => Target::new_with_metadata(transport_kind, endpoint_uri, scope, label),
    }
}

fn required_policy_outcomes<'a>(
    required_targets: &[TargetFingerprint],
    outcomes: &'a [TransportPublishTargetOutcome],
) -> Result<Vec<&'a TransportPublishTargetOutcome>, TransportPublishProtocolError> {
    required_targets
        .iter()
        .enumerate()
        .map(|(required_index, required)| {
            outcomes
                .iter()
                .enumerate()
                .find_map(|(outcome_index, outcome)| {
                    let fingerprint = target_outcome_fingerprint(outcome, outcome_index).ok()?;
                    (fingerprint == *required).then_some(outcome)
                })
                .ok_or(
                    TransportPublishProtocolError::RequiredTargetNotInTargetSet {
                        index: required_index,
                    },
                )
        })
        .collect()
}

fn validate_job_target_policy_outcomes(
    target_policy: &TransportPublishTargetPolicy,
    outcomes: &[TransportPublishTargetOutcome],
) -> Result<(), TransportPublishProtocolError> {
    let TransportPublishTargetPolicy::ExplicitTargets { targets } = target_policy else {
        return Ok(());
    };
    if outcomes.is_empty() {
        return Ok(());
    }
    if targets.len() != outcomes.len() {
        return Err(
            TransportPublishProtocolError::InvalidExplicitTargetOutcome {
                index: outcomes.len().min(targets.len()),
            },
        );
    }
    let mut matched_targets = Vec::new();
    matched_targets.resize(targets.len(), false);
    for (outcome_index, outcome) in outcomes.iter().enumerate() {
        let mut matched_target_index = None;
        for (target_index, target) in targets.iter().enumerate() {
            if !matched_targets[target_index]
                && target.identity_eq(target_index, outcome, outcome_index)?
            {
                matched_target_index = Some(target_index);
                break;
            }
        }
        let Some(target_index) = matched_target_index else {
            return Err(
                TransportPublishProtocolError::InvalidExplicitTargetOutcome {
                    index: outcome_index,
                },
            );
        };
        matched_targets[target_index] = true;
    }
    Ok(())
}

fn validate_job_status_state(
    job: &TransportPublishJobView,
    acknowledged_count: usize,
    retryable_count: usize,
    terminal_count: usize,
) -> Result<(), TransportPublishProtocolError> {
    if matches!(
        job.status,
        TransportPublishJobStatus::Accepted | TransportPublishJobStatus::Publishing
    ) {
        return Ok(());
    }
    if job.status == TransportPublishJobStatus::Rejected {
        if job.target_count == 0
            && job.targets.is_empty()
            && acknowledged_count == 0
            && retryable_count == 0
            && terminal_count == 0
        {
            return Ok(());
        }
        return Err(TransportPublishProtocolError::InvalidJobStatusState);
    }
    if job.targets.is_empty() {
        return Err(TransportPublishProtocolError::InvalidJobStatusState);
    }
    let required_count = job.delivery_policy.required_target_count(job.target_count);
    let (satisfied, retryable_status_count, terminal_status_count, has_deferred) = match &job
        .delivery_policy
    {
        TransportPublishDeliveryPolicy::RequiredTargets { targets } => {
            let required_outcomes = required_policy_outcomes(targets, &job.targets)?;
            let satisfied = required_outcomes
                .iter()
                .all(|outcome| outcome.outcome_kind.counts_toward_accepted_delivery());
            (
                satisfied,
                required_outcomes
                    .iter()
                    .filter(|outcome| outcome.outcome_kind.is_retryable())
                    .count(),
                required_outcomes
                    .iter()
                    .filter(|outcome| outcome.outcome_kind.is_terminal_failure())
                    .count(),
                required_outcomes.iter().any(|outcome| {
                    outcome.outcome_kind == TransportPublishOutcomeKind::DeferredUntilImplemented
                }),
            )
        }
        TransportPublishDeliveryPolicy::Any
        | TransportPublishDeliveryPolicy::All
        | TransportPublishDeliveryPolicy::Quorum { .. } => (
            acknowledged_count >= required_count,
            retryable_count,
            terminal_count,
            job.targets.iter().any(|target| {
                target.outcome_kind == TransportPublishOutcomeKind::DeferredUntilImplemented
            }),
        ),
    };
    let status_matches = if satisfied {
        job.status == TransportPublishJobStatus::DeliverySatisfied
    } else if retryable_status_count > 0 {
        job.status == TransportPublishJobStatus::DeliveryUnsatisfiedRetryable
    } else if terminal_status_count > 0 {
        job.status == TransportPublishJobStatus::DeliveryUnsatisfiedTerminal
    } else if has_deferred {
        job.status == TransportPublishJobStatus::DeliveryDeferred
            || job.status == TransportPublishJobStatus::DeliveryDeferredUntilImplemented
    } else {
        false
    };
    if status_matches {
        Ok(())
    } else {
        Err(TransportPublishProtocolError::InvalidJobStatusState)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw_event_json() -> String {
        format!(
            r#"{{"id":"{}","pubkey":"{}","created_at":1700000000,"kind":30402,"tags":[["d","listing-1"]],"content":"{{}}","sig":"{}"}}"#,
            "0".repeat(64),
            "1".repeat(64),
            "2".repeat(128)
        )
    }

    fn nostr_outcome(outcome_kind: TransportPublishOutcomeKind) -> TransportPublishTargetOutcome {
        nostr_outcome_for("wss://relay.example.com", outcome_kind)
    }

    fn nostr_outcome_for(
        endpoint_uri: impl Into<String>,
        outcome_kind: TransportPublishOutcomeKind,
    ) -> TransportPublishTargetOutcome {
        TransportPublishTargetOutcome {
            transport_kind: "nostr".to_owned(),
            endpoint_uri: endpoint_uri.into(),
            target_scope: None,
            target_label: None,
            source: TransportPublishTargetSource::Request,
            attempted: true,
            outcome_kind,
            message: None,
            latency_ms: Some(7),
        }
    }

    fn nostr_outcome_for_scope(
        endpoint_uri: impl Into<String>,
        target_scope: impl Into<String>,
        outcome_kind: TransportPublishOutcomeKind,
    ) -> TransportPublishTargetOutcome {
        let mut outcome = nostr_outcome_for(endpoint_uri, outcome_kind);
        outcome.target_scope = Some(target_scope.into());
        outcome
    }

    fn reticulum_outcome(
        outcome_kind: TransportPublishOutcomeKind,
    ) -> TransportPublishTargetOutcome {
        TransportPublishTargetOutcome {
            transport_kind: "reticulum".to_owned(),
            endpoint_uri: RADROOTS_RETICULUM_ENDPOINT_URI.to_owned(),
            target_scope: None,
            target_label: None,
            source: TransportPublishTargetSource::Reticulum,
            attempted: false,
            outcome_kind,
            message: None,
            latency_ms: None,
        }
    }

    fn job_from_targets(
        status: TransportPublishJobStatus,
        target_policy: TransportPublishTargetPolicy,
        targets: Vec<TransportPublishTargetOutcome>,
    ) -> TransportPublishJobView {
        TransportPublishJobView {
            job_id: "job-1".to_owned(),
            status,
            terminal: job_status_is_terminal(status),
            delivery_satisfied: status == TransportPublishJobStatus::DeliverySatisfied,
            event_id: "0".repeat(64),
            pubkey: "1".repeat(64),
            event_kind: 30_402,
            target_policy,
            delivery_policy: TransportPublishDeliveryPolicy::Any,
            target_count: targets.len(),
            acknowledged_count: targets
                .iter()
                .filter(|target| target.outcome_kind.counts_toward_accepted_delivery())
                .count(),
            retryable_count: targets
                .iter()
                .filter(|target| target.outcome_kind.is_retryable())
                .count(),
            terminal_count: targets
                .iter()
                .filter(|target| target.outcome_kind.is_terminal_failure())
                .count(),
            requested_at_ms: 1,
            completed_at_ms: Some(2),
            last_error: None,
            targets,
        }
    }

    fn accepted_job() -> TransportPublishJobView {
        job_from_targets(
            TransportPublishJobStatus::DeliverySatisfied,
            TransportPublishTargetPolicy::explicit_targets(vec![TransportPublishTarget::nostr(
                "wss://relay.example.com",
            )]),
            vec![nostr_outcome(TransportPublishOutcomeKind::Accepted)],
        )
    }

    fn rejected_job() -> TransportPublishJobView {
        TransportPublishJobView {
            job_id: "job-1".to_owned(),
            status: TransportPublishJobStatus::Rejected,
            terminal: true,
            delivery_satisfied: false,
            event_id: "0".repeat(64),
            pubkey: "1".repeat(64),
            event_kind: 30_402,
            target_policy: TransportPublishTargetPolicy::nostr(
                NostrPublishTargetSourcePolicy::DaemonDefaultOnly,
                Vec::new(),
            ),
            delivery_policy: TransportPublishDeliveryPolicy::Any,
            target_count: 0,
            acknowledged_count: 0,
            retryable_count: 0,
            terminal_count: 0,
            requested_at_ms: 1,
            completed_at_ms: Some(2),
            last_error: Some("no_transport_publish_targets".to_owned()),
            targets: Vec::new(),
        }
    }

    #[test]
    fn transport_publish_capabilities_match_v5_surface() {
        let capabilities = TransportPublishCapabilities::v5(1024, 10);

        assert_eq!(capabilities.api_version, "radrootsd.transport_publish.v5");
        assert_eq!(
            capabilities.methods,
            vec![
                "transport.publish.capabilities".to_owned(),
                "transport.publish.event".to_owned(),
                "transport.publish.job.get".to_owned(),
                "transport.publish.job.list".to_owned()
            ]
        );
        assert_eq!(capabilities.publish.max_targets_per_request, 10);
        assert_eq!(
            capabilities.publish.target_policy_modes,
            vec![
                TransportPublishTargetPolicyName::ExplicitTargets,
                TransportPublishTargetPolicyName::Nostr
            ]
        );
        assert_eq!(capabilities.publish.transports.len(), 2);
        let nostr = capabilities
            .publish
            .transports
            .iter()
            .find(|transport| transport.transport == "nostr")
            .expect("nostr capability");
        assert!(nostr.configured);
        assert_eq!(nostr.implementation, TransportPublishImplementation::Real);
        assert_eq!(nostr.maturity, TransportPublishCapabilityMaturity::Stable);
        assert_eq!(
            nostr.availability,
            TransportPublishCapabilityAvailability::Available
        );
        assert!(nostr.usable_for_delivery);
        assert!(nostr.capabilities.deliver);
        assert!(!nostr.capabilities.fetch);
        assert!(!nostr.capabilities.discovery);
        assert!(!nostr.capabilities.gateway_forwarding);
        assert!(!nostr.capabilities.receipt_observation);
        let reticulum = capabilities
            .publish
            .transports
            .iter()
            .find(|transport| transport.transport == "reticulum")
            .expect("reticulum capability");
        assert!(reticulum.configured);
        assert_eq!(
            reticulum.implementation,
            TransportPublishImplementation::Real
        );
        assert_eq!(
            reticulum.maturity,
            TransportPublishCapabilityMaturity::Preview
        );
        assert_eq!(
            reticulum.availability,
            TransportPublishCapabilityAvailability::Unavailable
        );
        assert!(!reticulum.usable_for_delivery);
        assert!(!reticulum.capabilities.deliver);
        assert!(!reticulum.capabilities.fetch);
        assert!(!reticulum.capabilities.discovery);
        assert!(!reticulum.capabilities.gateway_forwarding);
        assert!(!reticulum.capabilities.receipt_observation);
        assert_eq!(
            reticulum.reticulum_behavior,
            Some(TransportPublishReticulumBehavior::RejectDeliveryAttempts)
        );
    }

    #[test]
    fn request_validation_covers_targets_and_policy() {
        let request = TransportPublishEventRequest {
            raw_event_json: raw_event_json(),
            target_policy: TransportPublishTargetPolicy::explicit_targets(vec![
                TransportPublishTarget::nostr("wss://relay.example.com"),
            ]),
            delivery_policy: TransportPublishDeliveryPolicy::Any,
            idempotency_key: Some("idem-1".to_owned()),
            timeout_ms: Some(5_000),
        };

        request.validate(1).expect("valid request");

        let mut too_many = request.clone();
        too_many.target_policy = TransportPublishTargetPolicy::explicit_targets(vec![
            TransportPublishTarget::nostr("wss://relay.example.com"),
            TransportPublishTarget::reticulum(
                TransportPublishReticulumBehavior::RejectDeliveryAttempts,
            ),
        ]);
        assert!(matches!(
            too_many.validate(1),
            Err(TransportPublishProtocolError::TargetLimitExceeded { max: 1, actual: 2 })
        ));

        let mut empty_endpoint = request.clone();
        empty_endpoint.target_policy =
            TransportPublishTargetPolicy::explicit_targets(vec![TransportPublishTarget::nostr(
                " ",
            )]);
        assert!(matches!(
            empty_endpoint.validate(1),
            Err(TransportPublishProtocolError::EmptyEndpointUri { index: 0 })
        ));

        let mut invalid_endpoint = request.clone();
        invalid_endpoint.target_policy =
            TransportPublishTargetPolicy::explicit_targets(vec![TransportPublishTarget::nostr(
                "wss://relay.example.com/has space",
            )]);
        assert_eq!(
            invalid_endpoint.validate(1),
            Err(TransportPublishProtocolError::InvalidEndpointUri { index: 0 })
        );
        let mut noncanonical_endpoint = request.clone();
        noncanonical_endpoint.target_policy =
            TransportPublishTargetPolicy::explicit_targets(vec![TransportPublishTarget::nostr(
                "WSS://RELAY.EXAMPLE.COM/",
            )]);
        assert_eq!(
            noncanonical_endpoint.validate(1),
            Err(TransportPublishProtocolError::InvalidEndpointUri { index: 0 })
        );

        let mut invalid_reticulum_endpoint = request.clone();
        invalid_reticulum_endpoint.target_policy =
            TransportPublishTargetPolicy::explicit_targets(vec![TransportPublishTarget {
                transport_kind: "reticulum".to_owned(),
                endpoint_uri: "reticulum:alternate".to_owned(),
                target_scope: None,
                target_label: None,
                reticulum_behavior: Some(TransportPublishReticulumBehavior::RejectDeliveryAttempts),
            }]);
        assert_eq!(
            invalid_reticulum_endpoint.validate(1),
            Err(TransportPublishProtocolError::InvalidReticulumEndpoint { index: 0 })
        );

        let mut noncanonical_reticulum_kind = request.clone();
        noncanonical_reticulum_kind.target_policy =
            TransportPublishTargetPolicy::explicit_targets(vec![TransportPublishTarget {
                transport_kind: "Reticulum".to_owned(),
                endpoint_uri: RADROOTS_RETICULUM_ENDPOINT_URI.to_owned(),
                target_scope: None,
                target_label: None,
                reticulum_behavior: Some(TransportPublishReticulumBehavior::RejectDeliveryAttempts),
            }]);
        assert_eq!(
            noncanonical_reticulum_kind.validate(1),
            Err(TransportPublishProtocolError::InvalidTransportKind { index: 0 })
        );

        let mut nostr_reticulum_behavior = request.clone();
        nostr_reticulum_behavior.target_policy =
            TransportPublishTargetPolicy::explicit_targets(vec![TransportPublishTarget {
                transport_kind: "nostr".to_owned(),
                endpoint_uri: "wss://relay.example.com".to_owned(),
                target_scope: None,
                target_label: None,
                reticulum_behavior: Some(TransportPublishReticulumBehavior::RejectDeliveryAttempts),
            }]);
        assert_eq!(
            nostr_reticulum_behavior.validate(1),
            Err(TransportPublishProtocolError::InvalidReticulumBehavior { index: 0 })
        );

        for invalid in [
            " reticulum:local",
            "reticulum:local ",
            "RETICULUM:local",
            "reticulum:Local",
            "reticulum:temporary",
            "reticulum:custom",
        ] {
            let mut invalid_reticulum_endpoint = request.clone();
            invalid_reticulum_endpoint.target_policy =
                TransportPublishTargetPolicy::explicit_targets(vec![TransportPublishTarget {
                    transport_kind: "reticulum".to_owned(),
                    endpoint_uri: invalid.to_owned(),
                    target_scope: None,
                    target_label: None,
                    reticulum_behavior: Some(
                        TransportPublishReticulumBehavior::RejectDeliveryAttempts,
                    ),
                }]);
            assert_eq!(
                invalid_reticulum_endpoint.validate(1),
                Err(TransportPublishProtocolError::InvalidReticulumEndpoint { index: 0 })
            );
        }

        let mut duplicate_targets = request.clone();
        duplicate_targets.target_policy = TransportPublishTargetPolicy::explicit_targets(vec![
            TransportPublishTarget::nostr("wss://relay.example.com/a"),
            TransportPublishTarget::nostr("wss://relay.example.com/a"),
        ]);
        assert_eq!(
            duplicate_targets.validate(2),
            Err(TransportPublishProtocolError::DuplicateTarget { index: 1 })
        );

        let mut scoped_targets = request.clone();
        scoped_targets.target_policy = TransportPublishTargetPolicy::explicit_targets(vec![
            TransportPublishTarget::nostr("wss://relay.example.com/a")
                .with_scope("farm.local")
                .with_label("Farm relay"),
            TransportPublishTarget::nostr("wss://relay.example.com/a")
                .with_scope("farm.remote")
                .with_label("Farm relay"),
        ]);
        scoped_targets
            .validate(2)
            .expect("scope participates in target identity");

        let mut relabeled_duplicate_targets = request.clone();
        relabeled_duplicate_targets.target_policy =
            TransportPublishTargetPolicy::explicit_targets(vec![
                TransportPublishTarget::nostr("wss://relay.example.com/a")
                    .with_scope("farm.local")
                    .with_label("Primary"),
                TransportPublishTarget::nostr("wss://relay.example.com/a")
                    .with_scope("farm.local")
                    .with_label("Secondary"),
            ]);
        assert_eq!(
            relabeled_duplicate_targets.validate(2),
            Err(TransportPublishProtocolError::DuplicateTarget { index: 1 })
        );

        let mut invalid_scope = request.clone();
        invalid_scope.target_policy = TransportPublishTargetPolicy::explicit_targets(vec![
            TransportPublishTarget::nostr("wss://relay.example.com").with_scope("bad scope"),
        ]);
        assert_eq!(
            invalid_scope.validate(1),
            Err(TransportPublishProtocolError::InvalidTargetScope { index: 0 })
        );

        let mut invalid_label = request.clone();
        invalid_label.target_policy = TransportPublishTargetPolicy::explicit_targets(vec![
            TransportPublishTarget::nostr("wss://relay.example.com").with_label("bad\nlabel"),
        ]);
        assert_eq!(
            invalid_label.validate(1),
            Err(TransportPublishProtocolError::InvalidTargetLabel { index: 0 })
        );

        let mut noncanonical_nostr_policy = request.clone();
        noncanonical_nostr_policy.target_policy = TransportPublishTargetPolicy::nostr(
            NostrPublishTargetSourcePolicy::ExplicitOnly,
            vec!["WSS://RELAY.EXAMPLE.COM/".to_owned()],
        );
        assert_eq!(
            noncanonical_nostr_policy.validate(1),
            Err(TransportPublishProtocolError::InvalidEndpointUri { index: 0 })
        );
        let mut duplicate_nostr_policy = request.clone();
        duplicate_nostr_policy.target_policy = TransportPublishTargetPolicy::nostr(
            NostrPublishTargetSourcePolicy::ExplicitOnly,
            vec![
                "wss://relay.example.com".to_owned(),
                "wss://relay.example.com".to_owned(),
            ],
        );
        assert_eq!(
            duplicate_nostr_policy.validate(2),
            Err(TransportPublishProtocolError::DuplicateTarget { index: 1 })
        );

        let mut empty_key = request.clone();
        empty_key.idempotency_key = Some(" ".to_owned());
        assert_eq!(
            empty_key.validate(1),
            Err(TransportPublishProtocolError::EmptyIdempotencyKey)
        );

        let mut zero_timeout = request;
        zero_timeout.timeout_ms = Some(0);
        assert_eq!(
            zero_timeout.validate(1),
            Err(TransportPublishProtocolError::InvalidTimeoutMs)
        );
    }

    #[test]
    fn outcome_kinds_classify_satisfaction_retry_and_terminal() {
        assert!(TransportPublishOutcomeKind::Accepted.counts_toward_accepted_delivery());
        assert!(
            TransportPublishOutcomeKind::SkippedAlreadyAccepted.counts_toward_accepted_delivery()
        );
        assert!(TransportPublishOutcomeKind::Timeout.is_retryable());
        assert!(
            TransportPublishOutcomeKind::DeferredUntilImplemented.is_deferred_until_implemented()
        );
        assert!(!TransportPublishOutcomeKind::DeferredUntilImplemented.is_terminal_failure());
    }

    #[test]
    fn job_view_validation_rejects_bad_identity() {
        let job = accepted_job();

        job.validate().expect("valid job");
        let mut invalid = job;
        invalid.event_id = "bad".to_owned();
        assert!(matches!(
            invalid.validate(),
            Err(TransportPublishProtocolError::InvalidHexField {
                field: "event_id",
                ..
            })
        ));
    }

    #[test]
    fn job_view_validation_accepts_valid_status_shapes() {
        accepted_job().validate().expect("accepted job");
        job_from_targets(
            TransportPublishJobStatus::DeliveryUnsatisfiedRetryable,
            TransportPublishTargetPolicy::explicit_targets(vec![TransportPublishTarget::nostr(
                "wss://relay.example.com",
            )]),
            vec![nostr_outcome(TransportPublishOutcomeKind::Timeout)],
        )
        .validate()
        .expect("retryable job");
        job_from_targets(
            TransportPublishJobStatus::DeliveryUnsatisfiedTerminal,
            TransportPublishTargetPolicy::explicit_targets(vec![TransportPublishTarget::nostr(
                "wss://relay.example.com",
            )]),
            vec![nostr_outcome(TransportPublishOutcomeKind::TargetRejected)],
        )
        .validate()
        .expect("terminal job");
        job_from_targets(
            TransportPublishJobStatus::DeliveryDeferredUntilImplemented,
            TransportPublishTargetPolicy::explicit_targets(vec![
                TransportPublishTarget::reticulum(
                    TransportPublishReticulumBehavior::RejectDeliveryAttempts,
                ),
            ]),
            vec![reticulum_outcome(
                TransportPublishOutcomeKind::DeferredUntilImplemented,
            )],
        )
        .validate()
        .expect("Reticulum unavailable job");
        rejected_job().validate().expect("rejected job");
    }

    #[test]
    fn required_target_job_status_uses_required_membership() {
        let required_target = TransportPublishTarget::nostr("wss://required.example");
        let optional_target = TransportPublishTarget::nostr("wss://optional.example");
        let required_fingerprint = required_target
            .fingerprint(0)
            .expect("required fingerprint");
        let target_policy = TransportPublishTargetPolicy::explicit_targets(vec![
            required_target.clone(),
            optional_target,
        ]);

        let mut optional_success = job_from_targets(
            TransportPublishJobStatus::DeliverySatisfied,
            target_policy.clone(),
            vec![
                nostr_outcome_for(
                    "wss://required.example",
                    TransportPublishOutcomeKind::ConnectionFailed,
                ),
                nostr_outcome_for(
                    "wss://optional.example",
                    TransportPublishOutcomeKind::Accepted,
                ),
            ],
        );
        optional_success.delivery_policy =
            TransportPublishDeliveryPolicy::required_targets(vec![required_fingerprint.clone()])
                .expect("required policy");
        optional_success.acknowledged_count = 1;
        optional_success.retryable_count = 1;
        assert_eq!(
            optional_success.validate(),
            Err(TransportPublishProtocolError::InvalidJobStatusState)
        );
        optional_success.status = TransportPublishJobStatus::DeliveryUnsatisfiedRetryable;
        optional_success.terminal = false;
        optional_success.delivery_satisfied = false;
        optional_success
            .validate()
            .expect("required target retryable remains unsatisfied");

        let mut optional_failure = job_from_targets(
            TransportPublishJobStatus::DeliverySatisfied,
            target_policy,
            vec![
                nostr_outcome_for(
                    "wss://required.example",
                    TransportPublishOutcomeKind::Accepted,
                ),
                nostr_outcome_for(
                    "wss://optional.example",
                    TransportPublishOutcomeKind::Timeout,
                ),
            ],
        );
        optional_failure.delivery_policy =
            TransportPublishDeliveryPolicy::required_targets(vec![required_fingerprint])
                .expect("required policy");
        optional_failure.acknowledged_count = 1;
        optional_failure.retryable_count = 1;
        optional_failure
            .validate()
            .expect("optional retryable target does not block required satisfaction");
    }

    #[test]
    fn job_view_validation_matches_explicit_target_outcomes_by_identity() {
        job_from_targets(
            TransportPublishJobStatus::DeliverySatisfied,
            TransportPublishTargetPolicy::explicit_targets(vec![
                TransportPublishTarget::nostr("wss://relay-a.example.com"),
                TransportPublishTarget::nostr("wss://relay-b.example.com"),
            ]),
            vec![
                nostr_outcome_for(
                    "wss://relay-b.example.com",
                    TransportPublishOutcomeKind::Accepted,
                ),
                nostr_outcome_for(
                    "wss://relay-a.example.com",
                    TransportPublishOutcomeKind::Accepted,
                ),
            ],
        )
        .validate()
        .expect("explicit target outcomes match regardless of order");

        let noncanonical_outcome = job_from_targets(
            TransportPublishJobStatus::DeliverySatisfied,
            TransportPublishTargetPolicy::explicit_targets(vec![TransportPublishTarget::nostr(
                "wss://relay-a.example.com",
            )]),
            vec![nostr_outcome_for(
                "wss://relay-a.example.com/",
                TransportPublishOutcomeKind::Accepted,
            )],
        );
        assert_eq!(
            noncanonical_outcome.validate(),
            Err(TransportPublishProtocolError::InvalidEndpointUri { index: 0 })
        );

        let mismatched_endpoint = job_from_targets(
            TransportPublishJobStatus::DeliverySatisfied,
            TransportPublishTargetPolicy::explicit_targets(vec![TransportPublishTarget::nostr(
                "wss://relay-a.example.com",
            )]),
            vec![nostr_outcome_for(
                "wss://relay-b.example.com",
                TransportPublishOutcomeKind::Accepted,
            )],
        );
        assert_eq!(
            mismatched_endpoint.validate(),
            Err(TransportPublishProtocolError::InvalidExplicitTargetOutcome { index: 0 })
        );

        let mismatched_count = job_from_targets(
            TransportPublishJobStatus::DeliverySatisfied,
            TransportPublishTargetPolicy::explicit_targets(vec![
                TransportPublishTarget::nostr("wss://relay-a.example.com"),
                TransportPublishTarget::nostr("wss://relay-b.example.com"),
            ]),
            vec![nostr_outcome_for(
                "wss://relay-a.example.com",
                TransportPublishOutcomeKind::Accepted,
            )],
        );
        assert_eq!(
            mismatched_count.validate(),
            Err(TransportPublishProtocolError::InvalidExplicitTargetOutcome { index: 1 })
        );

        job_from_targets(
            TransportPublishJobStatus::DeliverySatisfied,
            TransportPublishTargetPolicy::explicit_targets(vec![
                TransportPublishTarget::nostr("wss://relay.example.com")
                    .with_scope("farm.a")
                    .with_label("Farm A"),
                TransportPublishTarget::nostr("wss://relay.example.com")
                    .with_scope("farm.b")
                    .with_label("Farm B"),
            ]),
            vec![
                nostr_outcome_for_scope(
                    "wss://relay.example.com",
                    "farm.b",
                    TransportPublishOutcomeKind::Accepted,
                ),
                nostr_outcome_for_scope(
                    "wss://relay.example.com",
                    "farm.a",
                    TransportPublishOutcomeKind::Accepted,
                ),
            ],
        )
        .validate()
        .expect("explicit target outcomes match by scoped identity");

        let mismatched_scope = job_from_targets(
            TransportPublishJobStatus::DeliverySatisfied,
            TransportPublishTargetPolicy::explicit_targets(vec![
                TransportPublishTarget::nostr("wss://relay.example.com").with_scope("farm.a"),
            ]),
            vec![nostr_outcome_for_scope(
                "wss://relay.example.com",
                "farm.b",
                TransportPublishOutcomeKind::Accepted,
            )],
        );
        assert_eq!(
            mismatched_scope.validate(),
            Err(TransportPublishProtocolError::InvalidExplicitTargetOutcome { index: 0 })
        );
    }

    #[test]
    fn job_view_validation_rejects_inconsistent_counts_flags_and_status() {
        let mut target_count_mismatch = accepted_job();
        target_count_mismatch.target_count = 2;
        assert_eq!(
            target_count_mismatch.validate(),
            Err(TransportPublishProtocolError::InvalidJobTargetCount {
                expected: 1,
                actual: 2
            })
        );

        let mut acknowledged_mismatch = accepted_job();
        acknowledged_mismatch.acknowledged_count = 0;
        assert_eq!(
            acknowledged_mismatch.validate(),
            Err(TransportPublishProtocolError::InvalidJobAcknowledgedCount {
                expected: 1,
                actual: 0
            })
        );

        let mut terminal_flag_mismatch = accepted_job();
        terminal_flag_mismatch.terminal = false;
        assert_eq!(
            terminal_flag_mismatch.validate(),
            Err(TransportPublishProtocolError::InvalidJobTerminalState)
        );

        let mut satisfied_flag_mismatch = accepted_job();
        satisfied_flag_mismatch.delivery_satisfied = false;
        assert_eq!(
            satisfied_flag_mismatch.validate(),
            Err(TransportPublishProtocolError::InvalidJobDeliverySatisfiedState)
        );

        let mut completed_at_missing = accepted_job();
        completed_at_missing.completed_at_ms = None;
        assert_eq!(
            completed_at_missing.validate(),
            Err(TransportPublishProtocolError::InvalidJobCompletedAt)
        );

        let mut completed_at_before_request = accepted_job();
        completed_at_before_request.completed_at_ms = Some(0);
        assert_eq!(
            completed_at_before_request.validate(),
            Err(TransportPublishProtocolError::InvalidJobCompletedAt)
        );

        let mut terminal_status_with_retryable_outcome = job_from_targets(
            TransportPublishJobStatus::DeliveryUnsatisfiedTerminal,
            TransportPublishTargetPolicy::explicit_targets(vec![TransportPublishTarget::nostr(
                "wss://relay.example.com",
            )]),
            vec![nostr_outcome(TransportPublishOutcomeKind::Timeout)],
        );
        terminal_status_with_retryable_outcome.retryable_count = 1;
        assert_eq!(
            terminal_status_with_retryable_outcome.validate(),
            Err(TransportPublishProtocolError::InvalidJobStatusState)
        );
    }

    #[test]
    fn job_view_validation_rejects_reticulum_success_and_non_reticulum() {
        let mut reticulum_success = job_from_targets(
            TransportPublishJobStatus::DeliverySatisfied,
            TransportPublishTargetPolicy::explicit_targets(vec![
                TransportPublishTarget::reticulum(
                    TransportPublishReticulumBehavior::RejectDeliveryAttempts,
                ),
            ]),
            vec![reticulum_outcome(TransportPublishOutcomeKind::Accepted)],
        );
        reticulum_success.acknowledged_count = 1;
        assert_eq!(
            reticulum_success.validate(),
            Err(TransportPublishProtocolError::InvalidReticulumOutcome { index: 0 })
        );

        let non_reticulum = job_from_targets(
            TransportPublishJobStatus::DeliveryDeferredUntilImplemented,
            TransportPublishTargetPolicy::explicit_targets(vec![TransportPublishTarget::nostr(
                "wss://relay.example.com",
            )]),
            vec![nostr_outcome(
                TransportPublishOutcomeKind::DeferredUntilImplemented,
            )],
        );
        assert_eq!(
            non_reticulum.validate(),
            Err(TransportPublishProtocolError::InvalidTargetOutcomeKind { index: 0 })
        );

        let mut reticulum_wrong_source = job_from_targets(
            TransportPublishJobStatus::DeliveryDeferred,
            TransportPublishTargetPolicy::explicit_targets(vec![
                TransportPublishTarget::reticulum(
                    TransportPublishReticulumBehavior::DeferDeliveryPlans,
                ),
            ]),
            vec![reticulum_outcome(
                TransportPublishOutcomeKind::DeferredUntilImplemented,
            )],
        );
        reticulum_wrong_source.targets[0].source = TransportPublishTargetSource::Request;
        assert_eq!(
            reticulum_wrong_source.validate(),
            Err(TransportPublishProtocolError::InvalidTargetSource { index: 0 })
        );

        let reticulum_deferred = job_from_targets(
            TransportPublishJobStatus::DeliveryDeferred,
            TransportPublishTargetPolicy::explicit_targets(vec![
                TransportPublishTarget::reticulum(
                    TransportPublishReticulumBehavior::DeferDeliveryPlans,
                ),
            ]),
            vec![reticulum_outcome(
                TransportPublishOutcomeKind::DeferredUntilImplemented,
            )],
        );
        reticulum_deferred
            .validate()
            .expect("Reticulum deferred job");
    }

    #[test]
    fn serde_round_trip_preserves_reticulum_target() {
        let request = TransportPublishEventRequest {
            raw_event_json: raw_event_json(),
            target_policy: TransportPublishTargetPolicy::explicit_targets(vec![
                TransportPublishTarget::reticulum(
                    TransportPublishReticulumBehavior::DeferDeliveryPlans,
                ),
            ]),
            delivery_policy: TransportPublishDeliveryPolicy::All,
            idempotency_key: None,
            timeout_ms: None,
        };
        let encoded = serde_json::to_string(&request).expect("encode");
        assert!(encoded.contains("\"transport_kind\":\"reticulum\""));
        assert!(encoded.contains("\"reticulum_behavior\":\"defer_delivery_plans\""));
        let decoded: TransportPublishEventRequest =
            serde_json::from_str(encoded.as_str()).expect("decode");
        assert_eq!(decoded, request);
    }

    #[test]
    fn serde_round_trip_preserves_target_metadata() {
        let request = TransportPublishEventRequest {
            raw_event_json: raw_event_json(),
            target_policy: TransportPublishTargetPolicy::explicit_targets(vec![
                TransportPublishTarget::nostr("wss://relay.example.com")
                    .with_scope("farm.local")
                    .with_label("Farm local relay"),
            ]),
            delivery_policy: TransportPublishDeliveryPolicy::All,
            idempotency_key: None,
            timeout_ms: None,
        };
        let encoded = serde_json::to_string(&request).expect("encode");
        assert!(encoded.contains("\"target_scope\":\"farm.local\""));
        assert!(encoded.contains("\"target_label\":\"Farm local relay\""));
        let decoded: TransportPublishEventRequest =
            serde_json::from_str(encoded.as_str()).expect("decode");
        assert_eq!(decoded, request);
    }

    #[test]
    fn protocol_errors_have_stable_display_strings() {
        let cases = [
            (
                TransportPublishProtocolError::InvalidHexField {
                    field: "id",
                    expected_len: 64,
                },
                "id must be 64 lowercase hex characters",
            ),
            (
                TransportPublishProtocolError::EmptyRawEventJson,
                "raw_event_json must not be empty",
            ),
            (
                TransportPublishProtocolError::EmptyTag { index: 2 },
                "tag 2 must not be empty",
            ),
            (
                TransportPublishProtocolError::EmptyIdempotencyKey,
                "idempotency key must not be empty",
            ),
            (
                TransportPublishProtocolError::EmptyTransportKind { index: 1 },
                "transport target 1 kind must not be empty",
            ),
            (
                TransportPublishProtocolError::InvalidTransportKind { index: 2 },
                "transport target 2 kind must be canonical lowercase",
            ),
            (
                TransportPublishProtocolError::EmptyEndpointUri { index: 3 },
                "transport target 3 endpoint_uri must not be empty",
            ),
            (
                TransportPublishProtocolError::InvalidEndpointUri { index: 3 },
                "transport target 3 endpoint_uri is invalid",
            ),
            (
                TransportPublishProtocolError::EmptyTargetScope { index: 3 },
                "transport target 3 target_scope must not be empty",
            ),
            (
                TransportPublishProtocolError::InvalidTargetScope { index: 3 },
                "transport target 3 target_scope must be canonical",
            ),
            (
                TransportPublishProtocolError::EmptyTargetLabel { index: 3 },
                "transport target 3 target_label must not be empty",
            ),
            (
                TransportPublishProtocolError::InvalidTargetLabel { index: 3 },
                "transport target 3 target_label is invalid",
            ),
            (
                TransportPublishProtocolError::InvalidReticulumBehavior { index: 4 },
                "transport target 4 reticulum_behavior is only valid for Reticulum targets",
            ),
            (
                TransportPublishProtocolError::InvalidTimeoutMs,
                "timeout_ms must be greater than zero",
            ),
            (
                TransportPublishProtocolError::InvalidReticulumEndpoint { index: 5 },
                "transport target 5 Reticulum endpoint must be reticulum:local",
            ),
            (
                TransportPublishProtocolError::TargetLimitExceeded { max: 1, actual: 2 },
                "transport target count 2 exceeds limit 1",
            ),
            (
                TransportPublishProtocolError::DuplicateTarget { index: 1 },
                "transport target 1 duplicates an earlier target",
            ),
            (
                TransportPublishProtocolError::EmptyTargetSet,
                "transport publish target set must not be empty",
            ),
            (
                TransportPublishProtocolError::InvalidQuorum,
                "delivery quorum must be greater than zero",
            ),
            (
                TransportPublishProtocolError::EmptyRequiredTargetSet,
                "delivery required target set must not be empty",
            ),
            (
                TransportPublishProtocolError::DuplicateRequiredTargetFingerprint { index: 2 },
                "delivery required target 2 duplicates an earlier fingerprint",
            ),
            (
                TransportPublishProtocolError::RequiredTargetNotInTargetSet { index: 3 },
                "delivery required target 3 is not in the target set",
            ),
            (
                TransportPublishProtocolError::EmptyPrincipalId,
                "principal id must not be empty",
            ),
            (
                TransportPublishProtocolError::EmptyJobId,
                "job id must not be empty",
            ),
            (
                TransportPublishProtocolError::InvalidJobTargetCount {
                    expected: 1,
                    actual: 2,
                },
                "job target_count 2 does not match 1 target outcomes",
            ),
            (
                TransportPublishProtocolError::InvalidJobAcknowledgedCount {
                    expected: 1,
                    actual: 2,
                },
                "job acknowledged_count 2 does not match 1 target outcomes",
            ),
            (
                TransportPublishProtocolError::InvalidJobRetryableCount {
                    expected: 1,
                    actual: 2,
                },
                "job retryable_count 2 does not match 1 target outcomes",
            ),
            (
                TransportPublishProtocolError::InvalidJobTerminalCount {
                    expected: 1,
                    actual: 2,
                },
                "job terminal_count 2 does not match 1 target outcomes",
            ),
            (
                TransportPublishProtocolError::InvalidJobTerminalState,
                "job terminal flag does not match status",
            ),
            (
                TransportPublishProtocolError::InvalidJobDeliverySatisfiedState,
                "job delivery_satisfied flag does not match status",
            ),
            (
                TransportPublishProtocolError::InvalidJobCompletedAt,
                "job completed_at_ms does not match status or request time",
            ),
            (
                TransportPublishProtocolError::InvalidJobStatusState,
                "job status does not match target outcomes",
            ),
            (
                TransportPublishProtocolError::InvalidExplicitTargetOutcome { index: 6 },
                "transport target outcome 6 does not match explicit target policy",
            ),
            (
                TransportPublishProtocolError::InvalidTargetOutcomeKind { index: 7 },
                "transport target outcome 7 kind is not valid for its transport",
            ),
            (
                TransportPublishProtocolError::InvalidTargetSource { index: 8 },
                "transport target outcome 8 source does not match transport kind",
            ),
            (
                TransportPublishProtocolError::InvalidReticulumOutcome { index: 9 },
                "transport target outcome 9 Reticulum must be unavailable or deferred",
            ),
        ];

        for (error, message) in cases {
            assert_eq!(error.to_string(), message);
        }
    }

    #[test]
    fn publish_request_validation_rejects_empty_raw_event_json() {
        let empty_raw_event = TransportPublishEventRequest {
            raw_event_json: String::new(),
            target_policy: TransportPublishTargetPolicy::nostr(
                NostrPublishTargetSourcePolicy::DaemonDefaultOnly,
                Vec::new(),
            ),
            delivery_policy: TransportPublishDeliveryPolicy::Any,
            idempotency_key: None,
            timeout_ms: None,
        };
        assert_eq!(
            empty_raw_event.validate(10),
            Err(TransportPublishProtocolError::EmptyRawEventJson)
        );
    }

    #[test]
    fn target_and_delivery_policy_validation_cover_all_modes() {
        assert_eq!(
            TransportPublishReticulumBehavior::default(),
            TransportPublishReticulumBehavior::RejectDeliveryAttempts
        );
        let explicit = TransportPublishTargetPolicy::explicit_targets(vec![
            TransportPublishTarget::nostr("wss://relay.example"),
            TransportPublishTarget::reticulum(
                TransportPublishReticulumBehavior::DeferDeliveryPlans,
            ),
        ]);
        let nostr = TransportPublishTargetPolicy::nostr(
            NostrPublishTargetSourcePolicy::RequestThenAuthorWriteThenDaemonDefault,
            vec!["wss://relay.example".to_owned()],
        );
        assert_eq!(explicit.request_target_count(), 2);
        assert_eq!(nostr.request_target_count(), 1);

        let mut empty_targets = TransportPublishEventRequest {
            raw_event_json: raw_event_json(),
            target_policy: TransportPublishTargetPolicy::explicit_targets(Vec::new()),
            delivery_policy: TransportPublishDeliveryPolicy::Any,
            idempotency_key: None,
            timeout_ms: None,
        };
        assert_eq!(
            empty_targets.validate(10),
            Err(TransportPublishProtocolError::EmptyTargetSet)
        );

        empty_targets.target_policy =
            TransportPublishTargetPolicy::explicit_targets(vec![TransportPublishTarget {
                transport_kind: " ".to_owned(),
                endpoint_uri: "wss://relay.example".to_owned(),
                target_scope: None,
                target_label: None,
                reticulum_behavior: None,
            }]);
        assert_eq!(
            empty_targets.validate(10),
            Err(TransportPublishProtocolError::EmptyTransportKind { index: 0 })
        );

        empty_targets.target_policy =
            TransportPublishTargetPolicy::explicit_targets(vec![TransportPublishTarget {
                transport_kind: "Nostr".to_owned(),
                endpoint_uri: "wss://relay.example".to_owned(),
                target_scope: None,
                target_label: None,
                reticulum_behavior: None,
            }]);
        assert_eq!(
            empty_targets.validate(10),
            Err(TransportPublishProtocolError::InvalidTransportKind { index: 0 })
        );

        empty_targets.target_policy = TransportPublishTargetPolicy::nostr(
            NostrPublishTargetSourcePolicy::ExplicitOnly,
            vec![" ".to_owned()],
        );
        assert_eq!(
            empty_targets.validate(10),
            Err(TransportPublishProtocolError::EmptyEndpointUri { index: 0 })
        );

        empty_targets.delivery_policy = TransportPublishDeliveryPolicy::Quorum { quorum: 0 };
        empty_targets.target_policy = nostr;
        assert_eq!(
            empty_targets.validate(10),
            Err(TransportPublishProtocolError::InvalidQuorum)
        );

        assert_eq!(
            TransportPublishDeliveryPolicy::Any.required_target_count(0),
            0
        );
        assert_eq!(
            TransportPublishDeliveryPolicy::Any.required_target_count(3),
            1
        );
        assert_eq!(
            TransportPublishDeliveryPolicy::All.required_target_count(3),
            3
        );
        assert_eq!(
            TransportPublishDeliveryPolicy::Quorum { quorum: 2 }.required_target_count(3),
            2
        );

        let required_target =
            TransportPublishTarget::nostr("wss://relay.example").with_scope("farm.local");
        let required_fingerprint = required_target
            .fingerprint(0)
            .expect("required fingerprint");
        let required_policy =
            TransportPublishDeliveryPolicy::required_targets(vec![required_fingerprint.clone()])
                .expect("required policy");
        assert_eq!(required_policy.required_target_count(3), 1);
        let required_request = TransportPublishEventRequest {
            raw_event_json: raw_event_json(),
            target_policy: TransportPublishTargetPolicy::explicit_targets(vec![
                required_target.clone(),
                TransportPublishTarget::nostr("wss://relay.example").with_scope("farm.remote"),
            ]),
            delivery_policy: required_policy,
            idempotency_key: None,
            timeout_ms: None,
        };
        required_request
            .validate(2)
            .expect("required target belongs to scoped target set");

        let unscoped_fingerprint = TransportPublishTarget::nostr("wss://relay.example")
            .fingerprint(0)
            .expect("unscoped fingerprint");
        assert_ne!(required_fingerprint, unscoped_fingerprint);
        let mut stale_required = required_request;
        stale_required.delivery_policy =
            TransportPublishDeliveryPolicy::required_targets(vec![unscoped_fingerprint])
                .expect("stale required policy");
        assert_eq!(
            stale_required.validate(2),
            Err(TransportPublishProtocolError::RequiredTargetNotInTargetSet { index: 0 })
        );

        assert_eq!(
            TransportPublishDeliveryPolicy::required_targets(Vec::new()),
            Err(TransportPublishProtocolError::EmptyRequiredTargetSet)
        );
        assert_eq!(
            TransportPublishDeliveryPolicy::required_targets(vec![
                required_fingerprint.clone(),
                required_fingerprint
            ]),
            Err(TransportPublishProtocolError::DuplicateRequiredTargetFingerprint { index: 1 })
        );
    }

    #[test]
    fn outcome_kinds_cover_negative_classification_edges() {
        let satisfied = [
            TransportPublishOutcomeKind::Accepted,
            TransportPublishOutcomeKind::DuplicateAccepted,
            TransportPublishOutcomeKind::SkippedAlreadyAccepted,
        ];
        let retryable = [
            TransportPublishOutcomeKind::RateLimited,
            TransportPublishOutcomeKind::PowRequired,
            TransportPublishOutcomeKind::AuthRequired,
            TransportPublishOutcomeKind::Error,
            TransportPublishOutcomeKind::Timeout,
            TransportPublishOutcomeKind::ConnectionFailed,
            TransportPublishOutcomeKind::Unknown,
        ];
        let terminal = [
            TransportPublishOutcomeKind::Blocked,
            TransportPublishOutcomeKind::Invalid,
            TransportPublishOutcomeKind::Restricted,
            TransportPublishOutcomeKind::Muted,
            TransportPublishOutcomeKind::Unsupported,
            TransportPublishOutcomeKind::PaymentRequired,
            TransportPublishOutcomeKind::TargetRejected,
        ];
        let deferred_until_implemented = [TransportPublishOutcomeKind::DeferredUntilImplemented];

        for kind in satisfied {
            assert!(kind.counts_toward_accepted_delivery());
            assert!(!kind.is_retryable());
            assert!(!kind.is_terminal_failure());
        }
        for kind in retryable {
            assert!(!kind.counts_toward_accepted_delivery());
            assert!(kind.is_retryable());
            assert!(!kind.is_terminal_failure());
        }
        for kind in terminal {
            assert!(!kind.counts_toward_accepted_delivery());
            assert!(!kind.is_retryable());
            assert!(kind.is_terminal_failure());
        }
        for kind in deferred_until_implemented {
            assert!(!kind.counts_toward_accepted_delivery());
            assert!(!kind.is_retryable());
            assert!(!kind.is_terminal_failure());
            assert!(kind.is_deferred_until_implemented());
        }
    }

    #[test]
    fn job_validation_rejects_empty_job_invalid_kind_and_invalid_delivery_policy() {
        let base = TransportPublishJobView {
            job_id: "job-1".to_owned(),
            status: TransportPublishJobStatus::Accepted,
            terminal: false,
            delivery_satisfied: false,
            event_id: "0".repeat(64),
            pubkey: "1".repeat(64),
            event_kind: 30_402,
            target_policy: TransportPublishTargetPolicy::nostr(
                NostrPublishTargetSourcePolicy::DaemonDefaultOnly,
                Vec::new(),
            ),
            delivery_policy: TransportPublishDeliveryPolicy::Any,
            target_count: 0,
            acknowledged_count: 0,
            retryable_count: 0,
            terminal_count: 0,
            requested_at_ms: 1,
            completed_at_ms: None,
            last_error: None,
            targets: Vec::new(),
        };

        let mut empty_job = base.clone();
        empty_job.job_id = " ".to_owned();
        assert_eq!(
            empty_job.validate(),
            Err(TransportPublishProtocolError::EmptyJobId)
        );

        let mut invalid_pubkey = base.clone();
        invalid_pubkey.pubkey = "x".repeat(64);
        assert!(matches!(
            invalid_pubkey.validate(),
            Err(TransportPublishProtocolError::InvalidHexField {
                field: "pubkey",
                ..
            })
        ));

        let mut invalid_delivery = base;
        invalid_delivery.delivery_policy = TransportPublishDeliveryPolicy::Quorum { quorum: 0 };
        assert_eq!(
            invalid_delivery.validate(),
            Err(TransportPublishProtocolError::InvalidQuorum)
        );
    }

    #[test]
    fn low_level_target_validation_covers_defensive_mapping_boundaries() {
        assert_eq!(
            transport_kind_error(RadrootsTransportError::EmptyTransportKind, 1),
            TransportPublishProtocolError::EmptyTransportKind { index: 1 }
        );
        assert_eq!(
            transport_kind_error(RadrootsTransportError::InvalidTransportKind, 2),
            TransportPublishProtocolError::InvalidTransportKind { index: 2 }
        );
        assert_eq!(
            target_fingerprint_error(RadrootsTransportError::EmptyTargetUri, 3),
            TransportPublishProtocolError::EmptyEndpointUri { index: 3 }
        );
        assert_eq!(
            target_fingerprint_error(RadrootsTransportError::InvalidTargetUri, 4),
            TransportPublishProtocolError::InvalidEndpointUri { index: 4 }
        );
        for (error, expected) in [
            (
                RadrootsTransportError::EmptyTargetScope,
                TransportPublishProtocolError::EmptyTargetScope { index: 5 },
            ),
            (
                RadrootsTransportError::InvalidTargetScope,
                TransportPublishProtocolError::InvalidTargetScope { index: 5 },
            ),
            (
                RadrootsTransportError::EmptyTargetLabel,
                TransportPublishProtocolError::EmptyTargetLabel { index: 5 },
            ),
            (
                RadrootsTransportError::InvalidTargetLabel,
                TransportPublishProtocolError::InvalidTargetLabel { index: 5 },
            ),
            (
                RadrootsTransportError::InvalidTargetUri,
                TransportPublishProtocolError::InvalidEndpointUri { index: 5 },
            ),
        ] {
            assert_eq!(target_metadata_error(error, 5), expected);
        }

        assert!(serde_json::from_str::<TargetFingerprint>("\"invalid\"").is_err());

        assert!(
            transport_target_from_parts(TransportId::LOCAL, "local:publish", None, None,).is_ok()
        );
        assert_eq!(
            transport_target_from_parts(TransportId::RETICULUM, "reticulum:other", None, None,),
            Err(RadrootsTransportError::InvalidTargetUri)
        );

        let valid = nostr_outcome(TransportPublishOutcomeKind::Accepted);
        assert!(validate_target_outcome(&valid, 0).is_ok());
        assert!(target_outcome_fingerprint(&valid, 0).is_ok());

        let mut invalid_target = TransportPublishTarget::nostr("wss://relay.example");
        invalid_target.transport_kind = "Nostr".to_owned();
        assert_eq!(
            invalid_target.fingerprint(10),
            Err(TransportPublishProtocolError::InvalidTransportKind { index: 10 })
        );
        invalid_target = TransportPublishTarget::nostr("wss://relay.example");
        invalid_target.target_scope = Some(" ".to_owned());
        assert_eq!(
            invalid_target.fingerprint(11),
            Err(TransportPublishProtocolError::InvalidTargetScope { index: 11 })
        );
        invalid_target = TransportPublishTarget::nostr("wss://relay.example");
        invalid_target.target_label = Some(" ".to_owned());
        assert_eq!(
            invalid_target.fingerprint(12),
            Err(TransportPublishProtocolError::EmptyTargetLabel { index: 12 })
        );

        let target = TransportPublishTarget::nostr("wss://relay.example");
        let mut different_kind = valid.clone();
        different_kind.transport_kind = "local".to_owned();
        different_kind.endpoint_uri = "local:publish".to_owned();
        assert!(
            !target
                .identity_eq(0, &different_kind, 0)
                .expect("different transport identity")
        );

        let mut invalid = valid.clone();
        invalid.transport_kind = " ".to_owned();
        assert_eq!(
            validate_target_outcome(&invalid, 1),
            Err(TransportPublishProtocolError::EmptyTransportKind { index: 1 })
        );
        invalid.transport_kind = "Nostr".to_owned();
        assert_eq!(
            validate_target_outcome(&invalid, 2),
            Err(TransportPublishProtocolError::InvalidTransportKind { index: 2 })
        );
        assert_eq!(
            target_outcome_fingerprint(&invalid, 2),
            Err(TransportPublishProtocolError::InvalidTransportKind { index: 2 })
        );
        invalid = valid.clone();
        invalid.endpoint_uri = " ".to_owned();
        assert_eq!(
            validate_target_outcome(&invalid, 3),
            Err(TransportPublishProtocolError::EmptyEndpointUri { index: 3 })
        );
        invalid = valid.clone();
        invalid.target_scope = Some(" ".to_owned());
        assert_eq!(
            validate_target_outcome(&invalid, 4),
            Err(TransportPublishProtocolError::InvalidTargetScope { index: 4 })
        );
        assert_eq!(
            target_outcome_fingerprint(&invalid, 4),
            Err(TransportPublishProtocolError::InvalidTargetScope { index: 4 })
        );
        invalid = valid.clone();
        invalid.target_label = Some(" ".to_owned());
        assert_eq!(
            validate_target_outcome(&invalid, 5),
            Err(TransportPublishProtocolError::EmptyTargetLabel { index: 5 })
        );
        assert_eq!(
            target_outcome_fingerprint(&invalid, 5),
            Err(TransportPublishProtocolError::EmptyTargetLabel { index: 5 })
        );
        invalid = valid.clone();
        invalid.endpoint_uri = "not a URI".to_owned();
        assert_eq!(
            target_outcome_fingerprint(&invalid, 6),
            Err(TransportPublishProtocolError::InvalidEndpointUri { index: 6 })
        );

        let mut reticulum =
            reticulum_outcome(TransportPublishOutcomeKind::DeferredUntilImplemented);
        reticulum.endpoint_uri = "reticulum:other".to_owned();
        assert_eq!(
            validate_target_outcome(&reticulum, 7),
            Err(TransportPublishProtocolError::InvalidReticulumEndpoint { index: 7 })
        );
        reticulum.endpoint_uri = RADROOTS_RETICULUM_ENDPOINT_URI.to_owned();
        reticulum.attempted = true;
        assert_eq!(
            validate_target_outcome(&reticulum, 8),
            Err(TransportPublishProtocolError::InvalidReticulumOutcome { index: 8 })
        );
        let mut wrong_source = valid.clone();
        wrong_source.source = TransportPublishTargetSource::Reticulum;
        assert_eq!(
            validate_target_outcome(&wrong_source, 9),
            Err(TransportPublishProtocolError::InvalidTargetSource { index: 9 })
        );

        let required = TransportPublishTarget::nostr("wss://required.example")
            .fingerprint(0)
            .expect("required fingerprint");
        let mut unparseable = nostr_outcome(TransportPublishOutcomeKind::Accepted);
        unparseable.endpoint_uri = "not a URI".to_owned();
        assert_eq!(
            required_policy_outcomes(&[required], &[unparseable]),
            Err(TransportPublishProtocolError::RequiredTargetNotInTargetSet { index: 0 })
        );

        assert!(
            validate_job_target_policy_outcomes(
                &TransportPublishTargetPolicy::nostr(
                    NostrPublishTargetSourcePolicy::ExplicitOnly,
                    Vec::new(),
                ),
                &[],
            )
            .is_ok()
        );
        assert!(
            validate_job_target_policy_outcomes(
                &TransportPublishTargetPolicy::explicit_targets(vec![
                    TransportPublishTarget::nostr("wss://relay.example"),
                ]),
                &[],
            )
            .is_ok()
        );

        let nostr_request = TransportPublishEventRequest {
            raw_event_json: raw_event_json(),
            target_policy: TransportPublishTargetPolicy::nostr(
                NostrPublishTargetSourcePolicy::DaemonDefaultOnly,
                Vec::new(),
            ),
            delivery_policy: TransportPublishDeliveryPolicy::Any,
            idempotency_key: None,
            timeout_ms: None,
        };
        nostr_request
            .validate(10)
            .expect("valid Nostr policy request");

        let accepted_without_targets = TransportPublishJobView {
            job_id: "accepted-empty".to_owned(),
            status: TransportPublishJobStatus::Accepted,
            terminal: false,
            delivery_satisfied: false,
            event_id: "0".repeat(64),
            pubkey: "1".repeat(64),
            event_kind: 30_402,
            target_policy: TransportPublishTargetPolicy::nostr(
                NostrPublishTargetSourcePolicy::DaemonDefaultOnly,
                Vec::new(),
            ),
            delivery_policy: TransportPublishDeliveryPolicy::Any,
            target_count: 0,
            acknowledged_count: 0,
            retryable_count: 0,
            terminal_count: 0,
            requested_at_ms: 1,
            completed_at_ms: None,
            last_error: None,
            targets: Vec::new(),
        };
        accepted_without_targets
            .validate()
            .expect("accepted job may await target resolution");
    }

    #[test]
    fn job_count_and_status_guards_reject_every_inconsistent_shape() {
        let mut retryable_count_mismatch = job_from_targets(
            TransportPublishJobStatus::DeliveryUnsatisfiedRetryable,
            TransportPublishTargetPolicy::explicit_targets(vec![TransportPublishTarget::nostr(
                "wss://relay.example.com",
            )]),
            vec![nostr_outcome(TransportPublishOutcomeKind::Timeout)],
        );
        retryable_count_mismatch.retryable_count = 0;
        assert_eq!(
            retryable_count_mismatch.validate(),
            Err(TransportPublishProtocolError::InvalidJobRetryableCount {
                expected: 1,
                actual: 0,
            })
        );

        let mut terminal_count_mismatch = job_from_targets(
            TransportPublishJobStatus::DeliveryUnsatisfiedTerminal,
            TransportPublishTargetPolicy::explicit_targets(vec![TransportPublishTarget::nostr(
                "wss://relay.example.com",
            )]),
            vec![nostr_outcome(TransportPublishOutcomeKind::TargetRejected)],
        );
        terminal_count_mismatch.terminal_count = 0;
        assert_eq!(
            terminal_count_mismatch.validate(),
            Err(TransportPublishProtocolError::InvalidJobTerminalCount {
                expected: 1,
                actual: 0,
            })
        );

        let rejected = rejected_job();
        assert!(validate_job_status_state(&rejected, 0, 0, 0).is_ok());
        for (target_count, targets, acknowledged, retryable, terminal) in [
            (1, Vec::new(), 0, 0, 0),
            (
                0,
                vec![nostr_outcome(TransportPublishOutcomeKind::Accepted)],
                0,
                0,
                0,
            ),
            (0, Vec::new(), 1, 0, 0),
            (0, Vec::new(), 0, 1, 0),
            (0, Vec::new(), 0, 0, 1),
        ] {
            let mut invalid = rejected.clone();
            invalid.target_count = target_count;
            invalid.targets = targets;
            assert_eq!(
                validate_job_status_state(&invalid, acknowledged, retryable, terminal),
                Err(TransportPublishProtocolError::InvalidJobStatusState)
            );
        }

        let mut empty_terminal = rejected.clone();
        empty_terminal.status = TransportPublishJobStatus::DeliveryUnsatisfiedTerminal;
        assert_eq!(
            validate_job_status_state(&empty_terminal, 0, 0, 0),
            Err(TransportPublishProtocolError::InvalidJobStatusState)
        );
        let mut publishing = rejected.clone();
        publishing.status = TransportPublishJobStatus::Publishing;
        assert!(validate_job_status_state(&publishing, 0, 0, 0).is_ok());

        let accepted = accepted_job();
        assert!(validate_job_status_state(&accepted, 1, 0, 0).is_ok());
        assert_eq!(
            validate_job_status_state(&accepted, 0, 1, 0),
            Err(TransportPublishProtocolError::InvalidJobStatusState)
        );
        let retryable = job_from_targets(
            TransportPublishJobStatus::DeliveryUnsatisfiedRetryable,
            TransportPublishTargetPolicy::explicit_targets(vec![TransportPublishTarget::nostr(
                "wss://relay.example.com",
            )]),
            vec![nostr_outcome(TransportPublishOutcomeKind::Timeout)],
        );
        assert!(validate_job_status_state(&retryable, 0, 1, 0).is_ok());
        assert_eq!(
            validate_job_status_state(&retryable, 1, 1, 0),
            Err(TransportPublishProtocolError::InvalidJobStatusState)
        );
        assert_eq!(
            validate_job_status_state(&retryable, 0, 0, 0),
            Err(TransportPublishProtocolError::InvalidJobStatusState)
        );
        let terminal = job_from_targets(
            TransportPublishJobStatus::DeliveryUnsatisfiedTerminal,
            TransportPublishTargetPolicy::explicit_targets(vec![TransportPublishTarget::nostr(
                "wss://relay.example.com",
            )]),
            vec![nostr_outcome(TransportPublishOutcomeKind::TargetRejected)],
        );
        assert!(validate_job_status_state(&terminal, 0, 0, 1).is_ok());
        assert_eq!(
            validate_job_status_state(&terminal, 0, 1, 1),
            Err(TransportPublishProtocolError::InvalidJobStatusState)
        );
        assert_eq!(
            validate_job_status_state(&terminal, 0, 0, 0),
            Err(TransportPublishProtocolError::InvalidJobStatusState)
        );

        for status in [
            TransportPublishJobStatus::DeliveryDeferred,
            TransportPublishJobStatus::DeliveryDeferredUntilImplemented,
        ] {
            let deferred = job_from_targets(
                status,
                TransportPublishTargetPolicy::explicit_targets(vec![
                    TransportPublishTarget::reticulum(
                        TransportPublishReticulumBehavior::DeferDeliveryPlans,
                    ),
                ]),
                vec![reticulum_outcome(
                    TransportPublishOutcomeKind::DeferredUntilImplemented,
                )],
            );
            assert!(validate_job_status_state(&deferred, 0, 0, 0).is_ok());
            assert_eq!(
                validate_job_status_state(&deferred, 1, 0, 0),
                Err(TransportPublishProtocolError::InvalidJobStatusState)
            );
            assert_eq!(
                validate_job_status_state(&deferred, 0, 1, 0),
                Err(TransportPublishProtocolError::InvalidJobStatusState)
            );
            assert_eq!(
                validate_job_status_state(&deferred, 0, 0, 1),
                Err(TransportPublishProtocolError::InvalidJobStatusState)
            );
        }
    }
}
