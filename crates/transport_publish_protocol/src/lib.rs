#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};
#[cfg(feature = "std")]
use std::{string::String, vec::Vec};

use core::fmt;

pub const API_VERSION: &str = "radrootsd.transport_publish.v2";
pub const DAEMON_NAME: &str = "radrootsd";
pub const METHOD_CAPABILITIES: &str = "transport.publish.capabilities";
pub const METHOD_EVENT: &str = "transport.publish.event";
pub const METHOD_JOB_GET: &str = "transport.publish.job.get";
pub const METHOD_JOB_LIST: &str = "transport.publish.job.list";
pub const RETICULUM_PREVIEW_ENDPOINT_URI: &str = "reticulum:preview-unavailable";

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
    EmptyEndpointUri {
        index: usize,
    },
    InvalidReticulumPreviewEndpoint {
        index: usize,
    },
    TargetLimitExceeded {
        max: usize,
        actual: usize,
    },
    EmptyTargetSet,
    InvalidQuorum,
    EmptyPrincipalId,
    EmptyJobId,
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
            Self::EmptyEndpointUri { index } => {
                write!(f, "transport target {index} endpoint_uri must not be empty")
            }
            Self::InvalidReticulumPreviewEndpoint { index } => write!(
                f,
                "transport target {index} Reticulum preview endpoint must be {RETICULUM_PREVIEW_ENDPOINT_URI}"
            ),
            Self::TargetLimitExceeded { max, actual } => {
                write!(f, "transport target count {actual} exceeds limit {max}")
            }
            Self::EmptyTargetSet => f.write_str("transport publish target set must not be empty"),
            Self::InvalidQuorum => f.write_str("delivery quorum must be greater than zero"),
            Self::EmptyPrincipalId => f.write_str("principal id must not be empty"),
            Self::EmptyJobId => f.write_str("job id must not be empty"),
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
    pub preview_behavior: Option<TransportPublishPreviewBehavior>,
}

impl TransportPublishTarget {
    pub fn nostr(endpoint_uri: impl Into<String>) -> Self {
        Self {
            transport_kind: "nostr".to_owned(),
            endpoint_uri: endpoint_uri.into(),
            preview_behavior: None,
        }
    }

    pub fn reticulum_preview(behavior: TransportPublishPreviewBehavior) -> Self {
        Self {
            transport_kind: "reticulum".to_owned(),
            endpoint_uri: RETICULUM_PREVIEW_ENDPOINT_URI.to_owned(),
            preview_behavior: Some(behavior),
        }
    }

    fn validate(&self, index: usize) -> Result<(), TransportPublishProtocolError> {
        if self.transport_kind.trim().is_empty() {
            return Err(TransportPublishProtocolError::EmptyTransportKind { index });
        }
        if self.endpoint_uri.trim().is_empty() {
            return Err(TransportPublishProtocolError::EmptyEndpointUri { index });
        }
        if self.transport_kind.trim() == "reticulum"
            && self.endpoint_uri.trim() != RETICULUM_PREVIEW_ENDPOINT_URI
        {
            return Err(TransportPublishProtocolError::InvalidReticulumPreviewEndpoint { index });
        }
        Ok(())
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

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "mode", rename_all = "snake_case"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransportPublishDeliveryPolicy {
    Any,
    All,
    Quorum { quorum: usize },
}

impl TransportPublishDeliveryPolicy {
    pub fn validate(&self) -> Result<(), TransportPublishProtocolError> {
        if matches!(self, Self::Quorum { quorum: 0 }) {
            Err(TransportPublishProtocolError::InvalidQuorum)
        } else {
            Ok(())
        }
    }

    pub fn required_target_count(&self, target_count: usize) -> usize {
        match self {
            Self::Any => usize::from(target_count > 0),
            Self::All => target_count,
            Self::Quorum { quorum } => *quorum,
        }
    }
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
        if self
            .idempotency_key
            .as_ref()
            .is_some_and(|key| key.trim().is_empty())
        {
            return Err(TransportPublishProtocolError::EmptyIdempotencyKey);
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
        self.delivery_policy.validate()
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
    pub fn v2(max_event_bytes: usize, max_targets_per_request: usize) -> Self {
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
                ],
                target_policy_modes: vec![
                    TransportPublishTargetPolicyName::ExplicitTargets,
                    TransportPublishTargetPolicyName::Nostr,
                ],
                transports: vec![
                    TransportPublishTransportCapability {
                        transport_kind: "nostr".to_owned(),
                        implementation_state: TransportPublishImplementationState::Available,
                        usable_for_delivery: true,
                        preview_behavior: None,
                        message: "Nostr relay publish is available".to_owned(),
                    },
                    TransportPublishTransportCapability {
                        transport_kind: "reticulum".to_owned(),
                        implementation_state:
                            TransportPublishImplementationState::PreviewUnavailable,
                        usable_for_delivery: false,
                        preview_behavior: Some(
                            TransportPublishPreviewBehavior::RejectDeliveryAttempts,
                        ),
                        message: "Reticulum preview is explicit and unavailable for real delivery"
                            .to_owned(),
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
pub enum TransportPublishImplementationState {
    Available,
    Disabled,
    Misconfigured,
    PreviewUnavailable,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransportPublishTransportCapability {
    pub transport_kind: String,
    pub implementation_state: TransportPublishImplementationState,
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

    #[test]
    fn transport_publish_capabilities_match_v2_surface() {
        let capabilities = TransportPublishCapabilities::v2(1024, 10);

        assert_eq!(capabilities.api_version, "radrootsd.transport_publish.v2");
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
            .find(|transport| transport.transport_kind == "nostr")
            .expect("nostr capability");
        assert_eq!(
            nostr.implementation_state,
            TransportPublishImplementationState::Available
        );
        assert!(nostr.usable_for_delivery);
        let reticulum = capabilities
            .publish
            .transports
            .iter()
            .find(|transport| transport.transport_kind == "reticulum")
            .expect("reticulum capability");
        assert_eq!(
            reticulum.implementation_state,
            TransportPublishImplementationState::PreviewUnavailable
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

        let mut invalid_reticulum_endpoint = request.clone();
        invalid_reticulum_endpoint.target_policy =
            TransportPublishTargetPolicy::explicit_targets(vec![TransportPublishTarget {
                transport_kind: "reticulum".to_owned(),
                endpoint_uri: "reticulum:preview-unavailable-alt".to_owned(),
                preview_behavior: Some(TransportPublishPreviewBehavior::RejectDeliveryAttempts),
            }]);
        assert_eq!(
            invalid_reticulum_endpoint.validate(1),
            Err(TransportPublishProtocolError::InvalidReticulumPreviewEndpoint { index: 0 })
        );

        let mut empty_key = request.clone();
        empty_key.idempotency_key = Some(" ".to_owned());
        assert_eq!(
            empty_key.validate(1),
            Err(TransportPublishProtocolError::EmptyIdempotencyKey)
        );
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
        let job = TransportPublishJobView {
            job_id: "job-1".to_owned(),
            status: TransportPublishJobStatus::DeliverySatisfied,
            terminal: true,
            delivery_satisfied: true,
            event_id: "0".repeat(64),
            pubkey: "1".repeat(64),
            event_kind: 30_402,
            target_policy: TransportPublishTargetPolicy::explicit_targets(vec![
                TransportPublishTarget::nostr("wss://relay.example.com"),
            ]),
            delivery_policy: TransportPublishDeliveryPolicy::Any,
            target_count: 1,
            acknowledged_count: 1,
            retryable_count: 0,
            terminal_count: 0,
            requested_at_ms: 1,
            completed_at_ms: Some(2),
            last_error: None,
            targets: Vec::new(),
        };

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
                TransportPublishProtocolError::EmptyEndpointUri { index: 3 },
                "transport target 3 endpoint_uri must not be empty",
            ),
            (
                TransportPublishProtocolError::InvalidReticulumPreviewEndpoint { index: 4 },
                "transport target 4 Reticulum preview endpoint must be reticulum:preview-unavailable",
            ),
            (
                TransportPublishProtocolError::TargetLimitExceeded { max: 1, actual: 2 },
                "transport target count 2 exceeds limit 1",
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
                preview_behavior: None,
            }]);
        assert_eq!(
            empty_targets.validate(10),
            Err(TransportPublishProtocolError::EmptyTransportKind { index: 0 })
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
