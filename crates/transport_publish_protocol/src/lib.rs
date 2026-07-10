#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::{collections::BTreeSet, string::String, vec::Vec};
#[cfg(feature = "std")]
use std::{collections::BTreeSet, string::String, vec::Vec};

use core::fmt;
use radroots_transport::{
    RADROOTS_RETICULUM_PREVIEW_ENDPOINT_URI, RADROOTS_RETICULUM_UNAVAILABLE_MESSAGE,
    RadrootsTransportError, RadrootsTransportKind, RadrootsTransportMeshScopeId,
    RadrootsTransportTarget, RadrootsTransportTargetFingerprint, RadrootsTransportTargetLabel,
};

pub const API_VERSION: &str = "radrootsd.transport_publish.v4";
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
    InvalidKind(u32),
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
    InvalidPreviewBehavior {
        index: usize,
    },
    InvalidTimeoutMs,
    InvalidReticulumPreviewEndpoint {
        index: usize,
    },
    ExplicitProxyTarget {
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
    InvalidRequiredTargetFingerprint {
        index: usize,
    },
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
            Self::InvalidKind(kind) => {
                write!(f, "event kind {kind} exceeds transport publish range")
            }
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
            Self::InvalidPreviewBehavior { index } => write!(
                f,
                "transport target {index} preview_behavior is only valid for Reticulum targets"
            ),
            Self::InvalidTimeoutMs => f.write_str("timeout_ms must be greater than zero"),
            Self::InvalidReticulumPreviewEndpoint { index } => write!(
                f,
                "transport target {index} Reticulum preview endpoint must be {RADROOTS_RETICULUM_PREVIEW_ENDPOINT_URI}"
            ),
            Self::ExplicitProxyTarget { index } => write!(
                f,
                "transport target {index} proxy is an SDK delegation target and cannot be used as a daemon explicit target"
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
            Self::InvalidRequiredTargetFingerprint { index } => {
                write!(f, "delivery required target {index} fingerprint is invalid")
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
                "transport target outcome {index} Reticulum preview must be unavailable or deferred"
            ),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for TransportPublishProtocolError {}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignedNostrEventWire {
    pub id: String,
    pub pubkey: String,
    pub created_at: u64,
    pub kind: u32,
    pub tags: Vec<Vec<String>>,
    pub content: String,
    pub sig: String,
}

impl SignedNostrEventWire {
    pub fn validate(&self) -> Result<(), TransportPublishProtocolError> {
        validate_lower_hex("id", self.id.as_str(), 64)?;
        validate_lower_hex("pubkey", self.pubkey.as_str(), 64)?;
        validate_lower_hex("sig", self.sig.as_str(), 128)?;
        if self.kind > u16::MAX as u32 {
            return Err(TransportPublishProtocolError::InvalidKind(self.kind));
        }
        for (index, tag) in self.tags.iter().enumerate() {
            if tag.is_empty() {
                return Err(TransportPublishProtocolError::EmptyTag { index });
            }
        }
        Ok(())
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransportPublishPreviewBehavior {
    RejectDeliveryAttempts,
    DeferDeliveryPlans,
}

impl Default for TransportPublishPreviewBehavior {
    fn default() -> Self {
        Self::RejectDeliveryAttempts
    }
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
    pub preview_behavior: Option<TransportPublishPreviewBehavior>,
}

impl TransportPublishTarget {
    pub fn nostr(endpoint_uri: impl Into<String>) -> Self {
        Self {
            transport_kind: "nostr".to_owned(),
            endpoint_uri: endpoint_uri.into(),
            target_scope: None,
            target_label: None,
            preview_behavior: None,
        }
    }

    pub fn reticulum_preview(behavior: TransportPublishPreviewBehavior) -> Self {
        Self {
            transport_kind: "reticulum".to_owned(),
            endpoint_uri: RADROOTS_RETICULUM_PREVIEW_ENDPOINT_URI.to_owned(),
            target_scope: None,
            target_label: None,
            preview_behavior: Some(behavior),
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
        let transport_kind = RadrootsTransportKind::parse_canonical(self.transport_kind.as_str())
            .map_err(|error| transport_kind_error(error, index))?;
        if transport_kind == RadrootsTransportKind::Proxy {
            return Err(TransportPublishProtocolError::ExplicitProxyTarget { index });
        }
        if self.endpoint_uri.trim().is_empty() {
            return Err(TransportPublishProtocolError::EmptyEndpointUri { index });
        }
        if transport_kind != RadrootsTransportKind::Reticulum && self.preview_behavior.is_some() {
            return Err(TransportPublishProtocolError::InvalidPreviewBehavior { index });
        }
        if transport_kind == RadrootsTransportKind::Reticulum
            && self.endpoint_uri != RADROOTS_RETICULUM_PREVIEW_ENDPOINT_URI
        {
            return Err(TransportPublishProtocolError::InvalidReticulumPreviewEndpoint { index });
        }
        validate_target_metadata(
            self.target_scope.as_deref(),
            self.target_label.as_deref(),
            index,
        )?;
        Ok(())
    }

    fn fingerprint(
        &self,
        index: usize,
    ) -> Result<RadrootsTransportTargetFingerprint, TransportPublishProtocolError> {
        let transport_kind = RadrootsTransportKind::parse_canonical(self.transport_kind.as_str())
            .map_err(|error| transport_kind_error(error, index))?;
        let scope = self
            .target_scope
            .as_deref()
            .map(RadrootsTransportMeshScopeId::parse)
            .transpose()
            .map_err(|error| target_metadata_error(error, index))?;
        let label = self
            .target_label
            .as_deref()
            .map(RadrootsTransportTargetLabel::parse)
            .transpose()
            .map_err(|error| target_metadata_error(error, index))?;
        let target = RadrootsTransportTarget::new_with_metadata(
            transport_kind,
            self.endpoint_uri.as_str(),
            scope,
            label,
        )
        .map_err(|error| target_fingerprint_error(error, index))?;
        Ok(target.fingerprint)
    }

    fn identity_eq(&self, outcome: &TransportPublishTargetOutcome) -> bool {
        self.transport_kind == outcome.transport_kind
            && self.endpoint_uri == outcome.endpoint_uri
            && self.target_scope == outcome.target_scope
    }
}

fn validate_target_metadata(
    target_scope: Option<&str>,
    target_label: Option<&str>,
    index: usize,
) -> Result<(), TransportPublishProtocolError> {
    if let Some(scope) = target_scope {
        RadrootsTransportMeshScopeId::parse(scope)
            .map_err(|error| target_metadata_error(error, index))?;
    }
    if let Some(label) = target_label {
        RadrootsTransportTargetLabel::parse(label)
            .map_err(|error| target_metadata_error(error, index))?;
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
                for (index, endpoint_uri) in relay_urls.iter().enumerate() {
                    if endpoint_uri.trim().is_empty() {
                        return Err(TransportPublishProtocolError::EmptyEndpointUri { index });
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
    Quorum {
        quorum: usize,
    },
    RequiredTargets {
        targets: Vec<RadrootsTransportTargetFingerprint>,
    },
}

impl TransportPublishDeliveryPolicy {
    pub fn required_targets(
        targets: Vec<RadrootsTransportTargetFingerprint>,
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
        target_fingerprints: &[RadrootsTransportTargetFingerprint],
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
    targets: &[RadrootsTransportTargetFingerprint],
) -> Result<(), TransportPublishProtocolError> {
    if targets.is_empty() {
        return Err(TransportPublishProtocolError::EmptyRequiredTargetSet);
    }
    let mut seen = BTreeSet::new();
    for (index, target) in targets.iter().enumerate() {
        if RadrootsTransportTargetFingerprint::parse(target.as_str()).is_err() {
            return Err(TransportPublishProtocolError::InvalidRequiredTargetFingerprint { index });
        }
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
    pub event: SignedNostrEventWire,
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
        self.event.validate()?;
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
    DeliveryPreviewUnavailable,
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
    PreviewUnavailable,
    Unknown,
}

impl TransportPublishOutcomeKind {
    pub fn counts_toward_satisfaction(self) -> bool {
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

    pub fn is_deferred_preview(self) -> bool {
        matches!(
            self,
            Self::DeferredUntilImplemented | Self::PreviewUnavailable
        )
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransportPublishTargetSource {
    Request,
    NostrAuthorWrite,
    DaemonDefault,
    ReticulumPreview,
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
        if self.event_kind > u16::MAX as u32 {
            return Err(TransportPublishProtocolError::InvalidKind(self.event_kind));
        }
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
            .filter(|target| target.outcome_kind.counts_toward_satisfaction())
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
    pub fn v4(max_event_bytes: usize, max_targets_per_request: usize) -> Self {
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
                signed_event_ingress: true,
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
                        usable_for_delivery: true,
                        preview_behavior: None,
                        message: "Nostr relay publish is available".to_owned(),
                    },
                    TransportPublishTransportCapability {
                        transport: "reticulum".to_owned(),
                        configured: true,
                        implementation: TransportPublishImplementation::PreviewUnavailable,
                        usable_for_delivery: false,
                        preview_behavior: Some(
                            TransportPublishPreviewBehavior::RejectDeliveryAttempts,
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
    pub signed_event_ingress: bool,
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
    PreviewUnavailable,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransportPublishTransportCapability {
    pub transport: String,
    pub configured: bool,
    pub implementation: TransportPublishImplementation,
    pub usable_for_delivery: bool,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub preview_behavior: Option<TransportPublishPreviewBehavior>,
    pub message: String,
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
            | TransportPublishJobStatus::DeliveryPreviewUnavailable
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
    let transport_kind = RadrootsTransportKind::parse_canonical(target.transport_kind.as_str())
        .map_err(|error| transport_kind_error(error, index))?;
    if target.endpoint_uri.trim().is_empty() {
        return Err(TransportPublishProtocolError::EmptyEndpointUri { index });
    }
    validate_target_metadata(
        target.target_scope.as_deref(),
        target.target_label.as_deref(),
        index,
    )?;
    if transport_kind == RadrootsTransportKind::Reticulum {
        if target.endpoint_uri != RADROOTS_RETICULUM_PREVIEW_ENDPOINT_URI {
            return Err(TransportPublishProtocolError::InvalidReticulumPreviewEndpoint { index });
        }
        if target.source != TransportPublishTargetSource::ReticulumPreview {
            return Err(TransportPublishProtocolError::InvalidTargetSource { index });
        }
        if target.attempted || !target.outcome_kind.is_deferred_preview() {
            return Err(TransportPublishProtocolError::InvalidReticulumOutcome { index });
        }
        return Ok(());
    }
    if target.source == TransportPublishTargetSource::ReticulumPreview {
        return Err(TransportPublishProtocolError::InvalidTargetSource { index });
    }
    if target.outcome_kind.is_deferred_preview() {
        return Err(TransportPublishProtocolError::InvalidTargetOutcomeKind { index });
    }
    Ok(())
}

fn target_outcome_fingerprint(
    target: &TransportPublishTargetOutcome,
    index: usize,
) -> Result<RadrootsTransportTargetFingerprint, TransportPublishProtocolError> {
    let transport_kind = RadrootsTransportKind::parse_canonical(target.transport_kind.as_str())
        .map_err(|error| transport_kind_error(error, index))?;
    let scope = target
        .target_scope
        .as_deref()
        .map(RadrootsTransportMeshScopeId::parse)
        .transpose()
        .map_err(|error| target_metadata_error(error, index))?;
    let label = target
        .target_label
        .as_deref()
        .map(RadrootsTransportTargetLabel::parse)
        .transpose()
        .map_err(|error| target_metadata_error(error, index))?;
    let target = RadrootsTransportTarget::new_with_metadata(
        transport_kind,
        target.endpoint_uri.as_str(),
        scope,
        label,
    )
    .map_err(|error| target_fingerprint_error(error, index))?;
    Ok(target.fingerprint)
}

fn required_policy_outcomes<'a>(
    required_targets: &[RadrootsTransportTargetFingerprint],
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
        let Some((target_index, _)) = targets.iter().enumerate().find(|(target_index, target)| {
            !matched_targets[*target_index] && target.identity_eq(outcome)
        }) else {
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
    let (satisfied, retryable_status_count, terminal_status_count, has_deferred, has_preview) =
        match &job.delivery_policy {
            TransportPublishDeliveryPolicy::RequiredTargets { targets } => {
                let required_outcomes = required_policy_outcomes(targets, &job.targets)?;
                let satisfied = targets.iter().all(|required| {
                    required_outcomes.iter().any(|outcome| {
                        target_outcome_fingerprint(outcome, 0).is_ok_and(|fingerprint| {
                            fingerprint == *required
                                && outcome.outcome_kind.counts_toward_satisfaction()
                        })
                    })
                });
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
                        outcome.outcome_kind
                            == TransportPublishOutcomeKind::DeferredUntilImplemented
                    }),
                    required_outcomes.iter().any(|outcome| {
                        outcome.outcome_kind == TransportPublishOutcomeKind::PreviewUnavailable
                    }),
                )
            }
            TransportPublishDeliveryPolicy::Any
            | TransportPublishDeliveryPolicy::All
            | TransportPublishDeliveryPolicy::Quorum { .. } => (
                required_count > 0 && acknowledged_count >= required_count,
                retryable_count,
                terminal_count,
                job.targets.iter().any(|target| {
                    target.outcome_kind == TransportPublishOutcomeKind::DeferredUntilImplemented
                }),
                job.targets.iter().any(|target| {
                    target.outcome_kind == TransportPublishOutcomeKind::PreviewUnavailable
                }),
            ),
        };
    match job.status {
        TransportPublishJobStatus::DeliverySatisfied if satisfied => Ok(()),
        TransportPublishJobStatus::DeliveryUnsatisfiedRetryable
            if !satisfied && retryable_status_count > 0 =>
        {
            Ok(())
        }
        TransportPublishJobStatus::DeliveryUnsatisfiedTerminal
            if !satisfied && retryable_status_count == 0 && terminal_status_count > 0 =>
        {
            Ok(())
        }
        TransportPublishJobStatus::DeliveryDeferred
            if !satisfied
                && terminal_status_count == 0
                && retryable_status_count == 0
                && has_deferred =>
        {
            Ok(())
        }
        TransportPublishJobStatus::DeliveryPreviewUnavailable
            if !satisfied
                && terminal_status_count == 0
                && retryable_status_count == 0
                && has_preview =>
        {
            Ok(())
        }
        _ => Err(TransportPublishProtocolError::InvalidJobStatusState),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event() -> SignedNostrEventWire {
        SignedNostrEventWire {
            id: "0".repeat(64),
            pubkey: "1".repeat(64),
            created_at: 1_700_000_000,
            kind: 30_402,
            tags: vec![vec!["d".to_owned(), "listing-1".to_owned()]],
            content: "{}".to_owned(),
            sig: "2".repeat(128),
        }
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
            endpoint_uri: RADROOTS_RETICULUM_PREVIEW_ENDPOINT_URI.to_owned(),
            target_scope: None,
            target_label: None,
            source: TransportPublishTargetSource::ReticulumPreview,
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
                .filter(|target| target.outcome_kind.counts_toward_satisfaction())
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
    fn transport_publish_capabilities_match_v4_surface() {
        let capabilities = TransportPublishCapabilities::v4(1024, 10);

        assert_eq!(capabilities.api_version, "radrootsd.transport_publish.v4");
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
        assert!(nostr.usable_for_delivery);
        let reticulum = capabilities
            .publish
            .transports
            .iter()
            .find(|transport| transport.transport == "reticulum")
            .expect("reticulum capability");
        assert!(reticulum.configured);
        assert_eq!(
            reticulum.implementation,
            TransportPublishImplementation::PreviewUnavailable
        );
        assert!(!reticulum.usable_for_delivery);
        assert_eq!(
            reticulum.preview_behavior,
            Some(TransportPublishPreviewBehavior::RejectDeliveryAttempts)
        );
    }

    #[test]
    fn request_validation_covers_targets_and_policy() {
        let request = TransportPublishEventRequest {
            event: event(),
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
            TransportPublishTarget::reticulum_preview(
                TransportPublishPreviewBehavior::RejectDeliveryAttempts,
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

        let mut invalid_reticulum_endpoint = request.clone();
        invalid_reticulum_endpoint.target_policy =
            TransportPublishTargetPolicy::explicit_targets(vec![TransportPublishTarget {
                transport_kind: "reticulum".to_owned(),
                endpoint_uri: "reticulum:preview-unavailable-alt".to_owned(),
                target_scope: None,
                target_label: None,
                preview_behavior: Some(TransportPublishPreviewBehavior::RejectDeliveryAttempts),
            }]);
        assert_eq!(
            invalid_reticulum_endpoint.validate(1),
            Err(TransportPublishProtocolError::InvalidReticulumPreviewEndpoint { index: 0 })
        );

        let mut noncanonical_reticulum_kind = request.clone();
        noncanonical_reticulum_kind.target_policy =
            TransportPublishTargetPolicy::explicit_targets(vec![TransportPublishTarget {
                transport_kind: "Reticulum".to_owned(),
                endpoint_uri: RADROOTS_RETICULUM_PREVIEW_ENDPOINT_URI.to_owned(),
                target_scope: None,
                target_label: None,
                preview_behavior: Some(TransportPublishPreviewBehavior::RejectDeliveryAttempts),
            }]);
        assert_eq!(
            noncanonical_reticulum_kind.validate(1),
            Err(TransportPublishProtocolError::InvalidTransportKind { index: 0 })
        );

        let mut nostr_preview_behavior = request.clone();
        nostr_preview_behavior.target_policy =
            TransportPublishTargetPolicy::explicit_targets(vec![TransportPublishTarget {
                transport_kind: "nostr".to_owned(),
                endpoint_uri: "wss://relay.example.com".to_owned(),
                target_scope: None,
                target_label: None,
                preview_behavior: Some(TransportPublishPreviewBehavior::RejectDeliveryAttempts),
            }]);
        assert_eq!(
            nostr_preview_behavior.validate(1),
            Err(TransportPublishProtocolError::InvalidPreviewBehavior { index: 0 })
        );

        for invalid in [
            " reticulum:preview-unavailable",
            "reticulum:preview-unavailable ",
            "RETICULUM:preview-unavailable",
            "reticulum:Preview-Unavailable",
            "reticulum:preview",
            "reticulum:custom",
        ] {
            let mut invalid_reticulum_endpoint = request.clone();
            invalid_reticulum_endpoint.target_policy =
                TransportPublishTargetPolicy::explicit_targets(vec![TransportPublishTarget {
                    transport_kind: "reticulum".to_owned(),
                    endpoint_uri: invalid.to_owned(),
                    target_scope: None,
                    target_label: None,
                    preview_behavior: Some(TransportPublishPreviewBehavior::RejectDeliveryAttempts),
                }]);
            assert_eq!(
                invalid_reticulum_endpoint.validate(1),
                Err(TransportPublishProtocolError::InvalidReticulumPreviewEndpoint { index: 0 })
            );
        }

        let mut removed_proxy_kind = request.clone();
        removed_proxy_kind.target_policy =
            TransportPublishTargetPolicy::explicit_targets(vec![TransportPublishTarget {
                transport_kind: removed_proxy_kind_string(),
                endpoint_uri: "radrootsd-proxy:publish".to_owned(),
                target_scope: None,
                target_label: None,
                preview_behavior: None,
            }]);
        assert_eq!(
            removed_proxy_kind.validate(1),
            Err(TransportPublishProtocolError::InvalidTransportKind { index: 0 })
        );

        let mut explicit_proxy_target = request.clone();
        explicit_proxy_target.target_policy =
            TransportPublishTargetPolicy::explicit_targets(vec![TransportPublishTarget {
                transport_kind: "proxy".to_owned(),
                endpoint_uri: "radrootsd-proxy:publish".to_owned(),
                target_scope: None,
                target_label: None,
                preview_behavior: None,
            }]);
        assert_eq!(
            explicit_proxy_target.validate(1),
            Err(TransportPublishProtocolError::ExplicitProxyTarget { index: 0 })
        );

        let mut duplicate_targets = request.clone();
        duplicate_targets.target_policy = TransportPublishTargetPolicy::explicit_targets(vec![
            TransportPublishTarget::nostr("wss://relay.example.com/a"),
            TransportPublishTarget::nostr("WSS://RELAY.EXAMPLE.COM/a"),
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
            TransportPublishTarget::nostr("WSS://RELAY.EXAMPLE.COM/a")
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
                TransportPublishTarget::nostr("WSS://RELAY.EXAMPLE.COM/a")
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

    fn removed_proxy_kind_string() -> String {
        ["radrootsd", "_proxy"].concat()
    }

    #[test]
    fn outcome_kinds_classify_satisfaction_retry_and_terminal() {
        assert!(TransportPublishOutcomeKind::Accepted.counts_toward_satisfaction());
        assert!(TransportPublishOutcomeKind::SkippedAlreadyAccepted.counts_toward_satisfaction());
        assert!(TransportPublishOutcomeKind::Timeout.is_retryable());
        assert!(TransportPublishOutcomeKind::PreviewUnavailable.is_deferred_preview());
        assert!(TransportPublishOutcomeKind::DeferredUntilImplemented.is_deferred_preview());
        assert!(!TransportPublishOutcomeKind::PreviewUnavailable.is_terminal_failure());
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
            TransportPublishJobStatus::DeliveryPreviewUnavailable,
            TransportPublishTargetPolicy::explicit_targets(vec![
                TransportPublishTarget::reticulum_preview(
                    TransportPublishPreviewBehavior::RejectDeliveryAttempts,
                ),
            ]),
            vec![reticulum_outcome(
                TransportPublishOutcomeKind::PreviewUnavailable,
            )],
        )
        .validate()
        .expect("preview unavailable job");
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
    fn job_view_validation_rejects_reticulum_success_and_non_reticulum_preview() {
        let mut reticulum_success = job_from_targets(
            TransportPublishJobStatus::DeliverySatisfied,
            TransportPublishTargetPolicy::explicit_targets(vec![
                TransportPublishTarget::reticulum_preview(
                    TransportPublishPreviewBehavior::RejectDeliveryAttempts,
                ),
            ]),
            vec![reticulum_outcome(TransportPublishOutcomeKind::Accepted)],
        );
        reticulum_success.acknowledged_count = 1;
        assert_eq!(
            reticulum_success.validate(),
            Err(TransportPublishProtocolError::InvalidReticulumOutcome { index: 0 })
        );

        let non_reticulum_preview = job_from_targets(
            TransportPublishJobStatus::DeliveryPreviewUnavailable,
            TransportPublishTargetPolicy::explicit_targets(vec![TransportPublishTarget::nostr(
                "wss://relay.example.com",
            )]),
            vec![nostr_outcome(
                TransportPublishOutcomeKind::PreviewUnavailable,
            )],
        );
        assert_eq!(
            non_reticulum_preview.validate(),
            Err(TransportPublishProtocolError::InvalidTargetOutcomeKind { index: 0 })
        );

        let mut reticulum_wrong_source = job_from_targets(
            TransportPublishJobStatus::DeliveryDeferred,
            TransportPublishTargetPolicy::explicit_targets(vec![
                TransportPublishTarget::reticulum_preview(
                    TransportPublishPreviewBehavior::DeferDeliveryPlans,
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
                TransportPublishTarget::reticulum_preview(
                    TransportPublishPreviewBehavior::DeferDeliveryPlans,
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
    fn serde_round_trip_preserves_preview_target() {
        let request = TransportPublishEventRequest {
            event: event(),
            target_policy: TransportPublishTargetPolicy::explicit_targets(vec![
                TransportPublishTarget::reticulum_preview(
                    TransportPublishPreviewBehavior::DeferDeliveryPlans,
                ),
            ]),
            delivery_policy: TransportPublishDeliveryPolicy::All,
            idempotency_key: None,
            timeout_ms: None,
        };
        let encoded = serde_json::to_string(&request).expect("encode");
        assert!(encoded.contains("\"transport_kind\":\"reticulum\""));
        assert!(encoded.contains("\"preview_behavior\":\"defer_delivery_plans\""));
        let decoded: TransportPublishEventRequest =
            serde_json::from_str(encoded.as_str()).expect("decode");
        assert_eq!(decoded, request);
    }

    #[test]
    fn serde_round_trip_preserves_target_metadata() {
        let request = TransportPublishEventRequest {
            event: event(),
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
                TransportPublishProtocolError::InvalidKind(70_000),
                "event kind 70000 exceeds transport publish range",
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
                TransportPublishProtocolError::InvalidPreviewBehavior { index: 4 },
                "transport target 4 preview_behavior is only valid for Reticulum targets",
            ),
            (
                TransportPublishProtocolError::InvalidTimeoutMs,
                "timeout_ms must be greater than zero",
            ),
            (
                TransportPublishProtocolError::InvalidReticulumPreviewEndpoint { index: 5 },
                "transport target 5 Reticulum preview endpoint must be reticulum:preview-unavailable",
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
                TransportPublishProtocolError::EmptyPrincipalId,
                "principal id must not be empty",
            ),
            (
                TransportPublishProtocolError::EmptyJobId,
                "job id must not be empty",
            ),
            (
                TransportPublishProtocolError::InvalidExplicitTargetOutcome { index: 6 },
                "transport target outcome 6 does not match explicit target policy",
            ),
        ];

        for (error, message) in cases {
            assert_eq!(error.to_string(), message);
        }
    }

    #[test]
    fn signed_event_validation_rejects_each_invalid_event_shape() {
        let mut invalid_id = event();
        invalid_id.id = "A".repeat(64);
        assert!(matches!(
            invalid_id.validate(),
            Err(TransportPublishProtocolError::InvalidHexField { field: "id", .. })
        ));

        let mut invalid_pubkey = event();
        invalid_pubkey.pubkey = "g".repeat(64);
        assert!(matches!(
            invalid_pubkey.validate(),
            Err(TransportPublishProtocolError::InvalidHexField {
                field: "pubkey",
                ..
            })
        ));

        let mut invalid_sig = event();
        invalid_sig.sig = "2".repeat(127);
        assert!(matches!(
            invalid_sig.validate(),
            Err(TransportPublishProtocolError::InvalidHexField { field: "sig", .. })
        ));

        let mut invalid_kind = event();
        invalid_kind.kind = u16::MAX as u32 + 1;
        assert_eq!(
            invalid_kind.validate(),
            Err(TransportPublishProtocolError::InvalidKind(
                u16::MAX as u32 + 1
            ))
        );

        let mut empty_tag = event();
        empty_tag.tags.push(Vec::new());
        assert_eq!(
            empty_tag.validate(),
            Err(TransportPublishProtocolError::EmptyTag { index: 1 })
        );
    }

    #[test]
    fn target_and_delivery_policy_validation_cover_all_modes() {
        assert_eq!(
            TransportPublishPreviewBehavior::default(),
            TransportPublishPreviewBehavior::RejectDeliveryAttempts
        );
        let explicit = TransportPublishTargetPolicy::explicit_targets(vec![
            TransportPublishTarget::nostr("wss://relay.example"),
            TransportPublishTarget::reticulum_preview(
                TransportPublishPreviewBehavior::DeferDeliveryPlans,
            ),
        ]);
        let nostr = TransportPublishTargetPolicy::nostr(
            NostrPublishTargetSourcePolicy::RequestThenAuthorWriteThenDaemonDefault,
            vec!["wss://relay.example".to_owned()],
        );
        assert_eq!(explicit.request_target_count(), 2);
        assert_eq!(nostr.request_target_count(), 1);

        let mut empty_targets = TransportPublishEventRequest {
            event: event(),
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
                preview_behavior: None,
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
                preview_behavior: None,
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
            event: event(),
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
        let deferred_preview = [
            TransportPublishOutcomeKind::DeferredUntilImplemented,
            TransportPublishOutcomeKind::PreviewUnavailable,
        ];

        for kind in satisfied {
            assert!(kind.counts_toward_satisfaction());
            assert!(!kind.is_retryable());
            assert!(!kind.is_terminal_failure());
        }
        for kind in retryable {
            assert!(!kind.counts_toward_satisfaction());
            assert!(kind.is_retryable());
            assert!(!kind.is_terminal_failure());
        }
        for kind in terminal {
            assert!(!kind.counts_toward_satisfaction());
            assert!(!kind.is_retryable());
            assert!(kind.is_terminal_failure());
        }
        for kind in deferred_preview {
            assert!(!kind.counts_toward_satisfaction());
            assert!(!kind.is_retryable());
            assert!(!kind.is_terminal_failure());
            assert!(kind.is_deferred_preview());
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

        let mut invalid_kind = base.clone();
        invalid_kind.event_kind = u16::MAX as u32 + 1;
        assert_eq!(
            invalid_kind.validate(),
            Err(TransportPublishProtocolError::InvalidKind(
                u16::MAX as u32 + 1
            ))
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
}
