//! Daemon transport-publish wire contract generation 5.
//!
//! The types in this module are passive serialized DTOs. Transport-native
//! targets, relay clients, delivery execution, and conversions belong to their
//! owning transport and host packages.

use alloc::{borrow::ToOwned, collections::BTreeSet, string::String, vec, vec::Vec};
use core::fmt;

use crate::schema::{Descriptor as SchemaDescriptor, ModuleVersion, Registry};

/// Stable daemon API/schema identity.
pub const API_VERSION: &str = "radrootsd.transport_publish.v5";
/// Stable daemon identity.
pub const DAEMON_NAME: &str = "radrootsd";
/// Capabilities RPC method.
pub const METHOD_CAPABILITIES: &str = "transport.publish.capabilities";
/// Event publication RPC method.
pub const METHOD_EVENT: &str = "transport.publish.event";
/// Job lookup RPC method.
pub const METHOD_JOB_GET: &str = "transport.publish.job.get";
/// Job listing RPC method.
pub const METHOD_JOB_LIST: &str = "transport.publish.job.list";
/// Canonical V5 Reticulum endpoint identity.
pub const RETICULUM_ENDPOINT_URI: &str = "reticulum:local";
/// Stable V5 Reticulum unavailability message.
pub const RETICULUM_UNAVAILABLE_MESSAGE: &str = concat!(
    "Reticulum transport is configured, ",
    "but this build does not implement Reticulum delivery."
);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
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

impl fmt::Display for Error {
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
                "transport target {index} Reticulum endpoint must be {RETICULUM_ENDPOINT_URI}"
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
impl std::error::Error for Error {}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ReticulumBehavior {
    #[default]
    RejectDeliveryAttempts,
    DeferDeliveryPlans,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
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
    pub reticulum_behavior: Option<ReticulumBehavior>,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NostrTargetSourcePolicy {
    ExplicitOnly,
    RequestThenAuthorWriteThenDaemonDefault,
    AuthorWriteThenDaemonDefault,
    DaemonDefaultOnly,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "kind", rename_all = "snake_case"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TargetPolicy {
    ExplicitTargets {
        targets: Vec<Target>,
    },
    Nostr {
        source_policy: NostrTargetSourcePolicy,
        #[cfg_attr(feature = "serde", serde(default))]
        relay_urls: Vec<String>,
    },
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "mode", rename_all = "snake_case"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeliveryPolicy {
    Any,
    All,
    Quorum { quorum: usize },
    RequiredTargets { targets: Vec<TargetFingerprint> },
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventRequest {
    pub raw_event_json: String,
    pub target_policy: TargetPolicy,
    pub delivery_policy: DeliveryPolicy,
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
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JobStatus {
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
pub enum OutcomeKind {
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

impl OutcomeKind {
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
pub enum TargetSource {
    Request,
    NostrAuthorWrite,
    DaemonDefault,
    Reticulum,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TargetOutcome {
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
    pub source: TargetSource,
    pub attempted: bool,
    pub outcome_kind: OutcomeKind,
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
pub struct Job {
    pub job_id: String,
    pub status: JobStatus,
    pub terminal: bool,
    pub delivery_satisfied: bool,
    pub event_id: String,
    pub pubkey: String,
    pub event_kind: u32,
    pub target_policy: TargetPolicy,
    pub delivery_policy: DeliveryPolicy,
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
    pub targets: Vec<TargetOutcome>,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventResponse {
    pub deduplicated: bool,
    pub job: Job,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Capabilities {
    pub daemon: String,
    pub api_version: String,
    pub transports: Vec<String>,
    pub methods: Vec<String>,
    pub auth: AuthCapabilities,
    pub publish: SurfaceCapabilities,
}

impl Capabilities {
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
            auth: AuthCapabilities {
                mode: "scoped_bearer_token".to_owned(),
            },
            publish: SurfaceCapabilities {
                raw_event_json_ingress: true,
                server_side_user_signing: false,
                max_event_bytes,
                max_targets_per_request,
                delivery_policies: vec![
                    DeliveryPolicyName::Any,
                    DeliveryPolicyName::Quorum,
                    DeliveryPolicyName::All,
                    DeliveryPolicyName::RequiredTargets,
                ],
                target_policy_modes: vec![
                    TargetPolicyName::ExplicitTargets,
                    TargetPolicyName::Nostr,
                ],
                transports: vec![
                    TransportCapability {
                        transport: "nostr".to_owned(),
                        configured: true,
                        implementation: Implementation::Real,
                        maturity: CapabilityMaturity::Stable,
                        availability: CapabilityAvailability::Available,
                        usable_for_delivery: true,
                        capabilities: OperationCapabilities {
                            deliver: true,
                            fetch: false,
                            discovery: false,
                            gateway_forwarding: false,
                            receipt_observation: false,
                        },
                        reticulum_behavior: None,
                        message: "Nostr relay publish is available".to_owned(),
                    },
                    TransportCapability {
                        transport: "reticulum".to_owned(),
                        configured: true,
                        implementation: Implementation::Real,
                        maturity: CapabilityMaturity::Preview,
                        availability: CapabilityAvailability::Unavailable,
                        usable_for_delivery: false,
                        capabilities: OperationCapabilities {
                            deliver: false,
                            fetch: false,
                            discovery: false,
                            gateway_forwarding: false,
                            receipt_observation: false,
                        },
                        reticulum_behavior: Some(ReticulumBehavior::RejectDeliveryAttempts),
                        message: RETICULUM_UNAVAILABLE_MESSAGE.to_owned(),
                    },
                ],
            },
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthCapabilities {
    pub mode: String,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SurfaceCapabilities {
    pub raw_event_json_ingress: bool,
    pub server_side_user_signing: bool,
    pub max_event_bytes: usize,
    pub max_targets_per_request: usize,
    pub delivery_policies: Vec<DeliveryPolicyName>,
    pub target_policy_modes: Vec<TargetPolicyName>,
    pub transports: Vec<TransportCapability>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Implementation {
    Real,
    Mock,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapabilityMaturity {
    Preview,
    Stable,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapabilityAvailability {
    Available,
    Degraded,
    Unavailable,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransportCapability {
    pub transport: String,
    pub configured: bool,
    pub implementation: Implementation,
    pub maturity: CapabilityMaturity,
    pub availability: CapabilityAvailability,
    pub usable_for_delivery: bool,
    pub capabilities: OperationCapabilities,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub reticulum_behavior: Option<ReticulumBehavior>,
    pub message: String,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OperationCapabilities {
    pub deliver: bool,
    pub fetch: bool,
    pub discovery: bool,
    pub gateway_forwarding: bool,
    pub receipt_observation: bool,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeliveryPolicyName {
    Any,
    Quorum,
    All,
    RequiredTargets,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetPolicyName {
    ExplicitTargets,
    Nostr,
}

/// Canonical serialized target fingerprint.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TargetFingerprint(String);

impl TargetFingerprint {
    /// Parses the exact lowercase 64-hex wire representation.
    pub fn parse(value: impl Into<String>) -> Result<Self, Error> {
        let value = value.into();
        validate_lower_hex("target_fingerprint", value.as_str(), 64)?;
        Ok(Self(value))
    }

    /// Returns the canonical wire representation.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for TargetFingerprint {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for TargetFingerprint {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = <String as serde::Deserialize>::deserialize(deserializer)?;
        Self::parse(value).map_err(serde::de::Error::custom)
    }
}

impl Target {
    /// Creates a passive Nostr target DTO.
    pub fn nostr(endpoint_uri: impl Into<String>) -> Self {
        Self {
            transport_kind: "nostr".to_owned(),
            endpoint_uri: endpoint_uri.into(),
            target_scope: None,
            target_label: None,
            reticulum_behavior: None,
        }
    }

    /// Creates the canonical passive Reticulum target DTO.
    pub fn reticulum(behavior: ReticulumBehavior) -> Self {
        Self {
            transport_kind: "reticulum".to_owned(),
            endpoint_uri: RETICULUM_ENDPOINT_URI.to_owned(),
            target_scope: None,
            target_label: None,
            reticulum_behavior: Some(behavior),
        }
    }

    /// Adds a serialized mesh scope.
    pub fn with_scope(mut self, target_scope: impl Into<String>) -> Self {
        self.target_scope = Some(target_scope.into());
        self
    }

    /// Adds a serialized display label.
    pub fn with_label(mut self, target_label: impl Into<String>) -> Self {
        self.target_label = Some(target_label.into());
        self
    }

    fn validate_structure(&self, index: usize) -> Result<(), Error> {
        match self.transport_kind.as_str() {
            "" => return Err(Error::EmptyTransportKind { index }),
            "local" | "nostr" | "reticulum" => {}
            _ if self.transport_kind.trim().is_empty() => {
                return Err(Error::EmptyTransportKind { index });
            }
            _ => return Err(Error::InvalidTransportKind { index }),
        }
        validate_endpoint(
            self.transport_kind.as_str(),
            self.endpoint_uri.as_str(),
            index,
        )?;
        validate_target_metadata(
            self.target_scope.as_deref(),
            self.target_label.as_deref(),
            index,
        )?;
        if self.transport_kind != "reticulum" && self.reticulum_behavior.is_some() {
            return Err(Error::InvalidReticulumBehavior { index });
        }
        if self.transport_kind == "reticulum" && self.endpoint_uri != RETICULUM_ENDPOINT_URI {
            return Err(Error::InvalidReticulumEndpoint { index });
        }
        Ok(())
    }

    fn same_wire_identity(&self, outcome: &TargetOutcome) -> bool {
        self.transport_kind == outcome.transport_kind
            && self.endpoint_uri == outcome.endpoint_uri
            && self.target_scope == outcome.target_scope
    }
}

impl TargetPolicy {
    /// Creates an explicit-target policy.
    pub fn explicit_targets(targets: Vec<Target>) -> Self {
        Self::ExplicitTargets { targets }
    }

    /// Creates a Nostr relay-source policy.
    pub fn nostr(source_policy: NostrTargetSourcePolicy, relay_urls: Vec<String>) -> Self {
        Self::Nostr {
            source_policy,
            relay_urls,
        }
    }

    /// Returns the number of request-declared targets.
    pub fn request_target_count(&self) -> usize {
        match self {
            Self::ExplicitTargets { targets } => targets.len(),
            Self::Nostr { relay_urls, .. } => relay_urls.len(),
        }
    }

    fn validate_structure(&self, max_targets: usize) -> Result<(), Error> {
        match self {
            Self::ExplicitTargets { targets } => {
                validate_target_limit(targets.len(), max_targets)?;
                if targets.is_empty() {
                    return Err(Error::EmptyTargetSet);
                }
                for (index, target) in targets.iter().enumerate() {
                    target.validate_structure(index)?;
                    if targets[..index].iter().any(|prior| {
                        prior.transport_kind == target.transport_kind
                            && prior.endpoint_uri == target.endpoint_uri
                            && prior.target_scope == target.target_scope
                    }) {
                        return Err(Error::DuplicateTarget { index });
                    }
                }
            }
            Self::Nostr { relay_urls, .. } => {
                validate_target_limit(relay_urls.len(), max_targets)?;
                for (index, endpoint) in relay_urls.iter().enumerate() {
                    validate_endpoint("nostr", endpoint, index)?;
                    if relay_urls[..index].contains(endpoint) {
                        return Err(Error::DuplicateTarget { index });
                    }
                }
            }
        }
        Ok(())
    }
}

impl DeliveryPolicy {
    /// Creates a required-target policy after structural validation.
    pub fn required_targets(targets: Vec<TargetFingerprint>) -> Result<Self, Error> {
        validate_required_target_fingerprints(targets.as_slice())?;
        Ok(Self::RequiredTargets { targets })
    }

    /// Validates quorum and required-target structure.
    pub fn validate(&self) -> Result<(), Error> {
        match self {
            Self::Quorum { quorum: 0 } => Err(Error::InvalidQuorum),
            Self::RequiredTargets { targets } => {
                validate_required_target_fingerprints(targets.as_slice())
            }
            Self::Any | Self::All | Self::Quorum { .. } => Ok(()),
        }
    }

    /// Returns the count required for delivery satisfaction.
    pub fn required_target_count(&self, target_count: usize) -> usize {
        match self {
            Self::Any => usize::from(target_count > 0),
            Self::All => target_count,
            Self::Quorum { quorum } => *quorum,
            Self::RequiredTargets { targets } => targets.len(),
        }
    }
}

impl EventRequest {
    /// Performs wire-structural validation without creating native targets.
    pub fn validate(&self, max_targets: usize) -> Result<(), Error> {
        if self.raw_event_json.is_empty() {
            return Err(Error::EmptyRawEventJson);
        }
        self.target_policy.validate_structure(max_targets)?;
        self.delivery_policy.validate()?;
        if self
            .idempotency_key
            .as_ref()
            .is_some_and(|key| key.trim().is_empty())
        {
            return Err(Error::EmptyIdempotencyKey);
        }
        if self.timeout_ms == Some(0) {
            return Err(Error::InvalidTimeoutMs);
        }
        Ok(())
    }
}

impl Job {
    /// Performs transport-neutral structural validation of a job receipt.
    pub fn validate(&self) -> Result<(), Error> {
        if self.job_id.trim().is_empty() {
            return Err(Error::EmptyJobId);
        }
        validate_lower_hex("event_id", self.event_id.as_str(), 64)?;
        validate_lower_hex("pubkey", self.pubkey.as_str(), 64)?;
        self.target_policy.validate_structure(usize::MAX)?;
        self.delivery_policy.validate()?;
        if self.terminal != job_status_is_terminal(self.status) {
            return Err(Error::InvalidJobTerminalState);
        }
        if self.delivery_satisfied != (self.status == JobStatus::DeliverySatisfied) {
            return Err(Error::InvalidJobDeliverySatisfiedState);
        }
        let completed = job_status_has_completed_at(self.status);
        if self.completed_at_ms.is_some() != completed
            || self
                .completed_at_ms
                .is_some_and(|completed_at| completed_at < self.requested_at_ms)
        {
            return Err(Error::InvalidJobCompletedAt);
        }
        for (index, target) in self.targets.iter().enumerate() {
            validate_target_outcome(target, index)?;
        }
        validate_explicit_outcomes(&self.target_policy, self.targets.as_slice())?;
        if (!self.targets.is_empty() || completed) && self.target_count != self.targets.len() {
            return Err(Error::InvalidJobTargetCount {
                expected: self.targets.len(),
                actual: self.target_count,
            });
        }

        let acknowledged = self
            .targets
            .iter()
            .filter(|target| target.outcome_kind.counts_toward_accepted_delivery())
            .count();
        let retryable = self
            .targets
            .iter()
            .filter(|target| target.outcome_kind.is_retryable())
            .count();
        let terminal = self
            .targets
            .iter()
            .filter(|target| target.outcome_kind.is_terminal_failure())
            .count();
        if self.acknowledged_count != acknowledged {
            return Err(Error::InvalidJobAcknowledgedCount {
                expected: acknowledged,
                actual: self.acknowledged_count,
            });
        }
        if self.retryable_count != retryable {
            return Err(Error::InvalidJobRetryableCount {
                expected: retryable,
                actual: self.retryable_count,
            });
        }
        if self.terminal_count != terminal {
            return Err(Error::InvalidJobTerminalCount {
                expected: terminal,
                actual: self.terminal_count,
            });
        }
        validate_job_status(self, acknowledged, retryable, terminal)
    }
}

fn validate_endpoint(kind: &str, endpoint: &str, index: usize) -> Result<(), Error> {
    if endpoint.trim().is_empty() {
        return Err(Error::EmptyEndpointUri { index });
    }
    if endpoint != endpoint.trim()
        || endpoint
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace())
    {
        return Err(Error::InvalidEndpointUri { index });
    }
    if kind == "reticulum" && endpoint != RETICULUM_ENDPOINT_URI {
        return Err(Error::InvalidReticulumEndpoint { index });
    }
    if kind == "nostr" && !(endpoint.starts_with("wss://") || endpoint.starts_with("ws://")) {
        return Err(Error::InvalidEndpointUri { index });
    }
    Ok(())
}

fn validate_target_metadata(
    scope: Option<&str>,
    label: Option<&str>,
    index: usize,
) -> Result<(), Error> {
    if let Some(scope) = scope {
        if scope.is_empty() {
            return Err(Error::EmptyTargetScope { index });
        }
        if scope != scope.trim()
            || scope
                .bytes()
                .any(|byte| !(byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.')))
        {
            return Err(Error::InvalidTargetScope { index });
        }
    }
    if let Some(label) = label {
        if label.trim().is_empty() {
            return Err(Error::EmptyTargetLabel { index });
        }
        if label != label.trim() || label.chars().any(char::is_control) {
            return Err(Error::InvalidTargetLabel { index });
        }
    }
    Ok(())
}

fn validate_required_target_fingerprints(targets: &[TargetFingerprint]) -> Result<(), Error> {
    if targets.is_empty() {
        return Err(Error::EmptyRequiredTargetSet);
    }
    let mut seen = BTreeSet::new();
    for (index, target) in targets.iter().enumerate() {
        if !seen.insert(target.as_str()) {
            return Err(Error::DuplicateRequiredTargetFingerprint { index });
        }
    }
    Ok(())
}

fn validate_target_limit(target_count: usize, max_targets: usize) -> Result<(), Error> {
    if target_count > max_targets {
        Err(Error::TargetLimitExceeded {
            max: max_targets,
            actual: target_count,
        })
    } else {
        Ok(())
    }
}

fn validate_lower_hex(field: &'static str, value: &str, expected_len: usize) -> Result<(), Error> {
    if value.len() == expected_len
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    {
        Ok(())
    } else {
        Err(Error::InvalidHexField {
            field,
            expected_len,
        })
    }
}

fn job_status_is_terminal(status: JobStatus) -> bool {
    matches!(
        status,
        JobStatus::DeliverySatisfied
            | JobStatus::DeliveryUnsatisfiedTerminal
            | JobStatus::DeliveryDeferred
            | JobStatus::DeliveryDeferredUntilImplemented
            | JobStatus::Rejected
    )
}

fn job_status_has_completed_at(status: JobStatus) -> bool {
    !matches!(status, JobStatus::Accepted | JobStatus::Publishing)
}

fn validate_target_outcome(target: &TargetOutcome, index: usize) -> Result<(), Error> {
    Target {
        transport_kind: target.transport_kind.clone(),
        endpoint_uri: target.endpoint_uri.clone(),
        target_scope: target.target_scope.clone(),
        target_label: target.target_label.clone(),
        reticulum_behavior: None,
    }
    .validate_structure(index)?;

    if target.transport_kind == "reticulum" {
        if target.source != TargetSource::Reticulum
            || target.attempted
            || !target.outcome_kind.is_deferred_until_implemented()
        {
            return Err(Error::InvalidReticulumOutcome { index });
        }
    } else {
        if target.source == TargetSource::Reticulum {
            return Err(Error::InvalidTargetSource { index });
        }
        if target.outcome_kind.is_deferred_until_implemented() {
            return Err(Error::InvalidTargetOutcomeKind { index });
        }
    }
    Ok(())
}

fn validate_explicit_outcomes(
    policy: &TargetPolicy,
    outcomes: &[TargetOutcome],
) -> Result<(), Error> {
    let TargetPolicy::ExplicitTargets { targets } = policy else {
        return Ok(());
    };
    if outcomes.is_empty() {
        return Ok(());
    }
    if targets.len() != outcomes.len() {
        return Err(Error::InvalidExplicitTargetOutcome {
            index: outcomes.len().min(targets.len()),
        });
    }
    let mut matched = vec![false; targets.len()];
    for (outcome_index, outcome) in outcomes.iter().enumerate() {
        let Some(index) = targets.iter().enumerate().find_map(|(index, target)| {
            (!matched[index] && target.same_wire_identity(outcome)).then_some(index)
        }) else {
            return Err(Error::InvalidExplicitTargetOutcome {
                index: outcome_index,
            });
        };
        matched[index] = true;
    }
    Ok(())
}

fn validate_job_status(
    job: &Job,
    acknowledged: usize,
    retryable: usize,
    terminal: usize,
) -> Result<(), Error> {
    if matches!(job.status, JobStatus::Accepted | JobStatus::Publishing) {
        return Ok(());
    }
    if job.status == JobStatus::Rejected {
        return (job.target_count == 0
            && job.targets.is_empty()
            && acknowledged == 0
            && retryable == 0
            && terminal == 0)
            .then_some(())
            .ok_or(Error::InvalidJobStatusState);
    }
    if job.targets.is_empty() {
        return Err(Error::InvalidJobStatusState);
    }
    if matches!(job.delivery_policy, DeliveryPolicy::RequiredTargets { .. }) {
        // Matching fingerprints to native targets is intentionally deferred to
        // the transport conversion boundary. All other job invariants remain
        // structural and are enforced above.
        return Ok(());
    }

    let satisfied = acknowledged >= job.delivery_policy.required_target_count(job.target_count);
    let deferred = job
        .targets
        .iter()
        .any(|target| target.outcome_kind == OutcomeKind::DeferredUntilImplemented);
    let matches = if satisfied {
        job.status == JobStatus::DeliverySatisfied
    } else if retryable > 0 {
        job.status == JobStatus::DeliveryUnsatisfiedRetryable
    } else if terminal > 0 {
        job.status == JobStatus::DeliveryUnsatisfiedTerminal
    } else if deferred {
        matches!(
            job.status,
            JobStatus::DeliveryDeferred | JobStatus::DeliveryDeferredUntilImplemented
        )
    } else {
        false
    };
    matches.then_some(()).ok_or(Error::InvalidJobStatusState)
}

/// Builds the schema registry for the V5 daemon transport-publish contract.
pub fn schema_registry() -> Result<Registry, crate::schema::Error> {
    Registry::try_new([SchemaDescriptor::try_new(
        API_VERSION,
        ModuleVersion::RadrootsdTransportPublishV5,
    )?])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> EventRequest {
        EventRequest {
            raw_event_json: "{\"id\":\"event\"}".to_owned(),
            target_policy: TargetPolicy::explicit_targets(vec![Target::nostr(
                "wss://relay.example.com",
            )]),
            delivery_policy: DeliveryPolicy::Any,
            idempotency_key: Some("idem-1".to_owned()),
            timeout_ms: Some(5_000),
        }
    }

    fn accepted_job() -> Job {
        Job {
            job_id: "job-1".to_owned(),
            status: JobStatus::DeliverySatisfied,
            terminal: true,
            delivery_satisfied: true,
            event_id: "0".repeat(64),
            pubkey: "1".repeat(64),
            event_kind: 30_402,
            target_policy: TargetPolicy::explicit_targets(vec![Target::nostr(
                "wss://relay.example.com",
            )]),
            delivery_policy: DeliveryPolicy::Any,
            target_count: 1,
            acknowledged_count: 1,
            retryable_count: 0,
            terminal_count: 0,
            requested_at_ms: 1,
            completed_at_ms: Some(2),
            last_error: None,
            targets: vec![TargetOutcome {
                transport_kind: "nostr".to_owned(),
                endpoint_uri: "wss://relay.example.com".to_owned(),
                target_scope: None,
                target_label: None,
                source: TargetSource::Request,
                attempted: true,
                outcome_kind: OutcomeKind::Accepted,
                message: None,
                latency_ms: Some(7),
            }],
        }
    }

    #[test]
    fn request_job_and_schema_registry_validate() {
        request().validate(1).expect("request");
        accepted_job().validate().expect("job");
        let registry = schema_registry().expect("schema registry");
        assert_eq!(registry.len(), 1);
        assert_eq!(
            registry.descriptors()[0].module(),
            ModuleVersion::RadrootsdTransportPublishV5
        );
    }

    #[test]
    fn structural_validation_rejects_invalid_fields() {
        let mut invalid_timeout = request();
        invalid_timeout.timeout_ms = Some(0);
        assert_eq!(invalid_timeout.validate(1), Err(Error::InvalidTimeoutMs));

        let mut invalid_endpoint = request();
        invalid_endpoint.target_policy =
            TargetPolicy::explicit_targets(vec![Target::nostr("WSS://relay.example.com")]);
        assert_eq!(
            invalid_endpoint.validate(1),
            Err(Error::InvalidEndpointUri { index: 0 })
        );

        let mut job = accepted_job();
        job.event_id = "ABC".repeat(21);
        assert_eq!(
            job.validate(),
            Err(Error::InvalidHexField {
                field: "event_id",
                expected_len: 64,
            })
        );
    }

    #[cfg(feature = "serde")]
    #[test]
    fn json_vectors_preserve_v5_names_and_unknown_fields_fail_closed() {
        let encoded = serde_json::to_value(request()).expect("request JSON");
        assert_eq!(encoded["target_policy"]["kind"], "explicit_targets");
        assert_eq!(encoded["delivery_policy"]["mode"], "any");
        let mut object = encoded.as_object().expect("request object").clone();
        object.insert("unknown".to_owned(), serde_json::json!(true));
        assert!(serde_json::from_value::<EventRequest>(object.into()).is_err());

        assert_eq!(
            serde_json::to_string(&ReticulumBehavior::RejectDeliveryAttempts)
                .expect("behavior JSON"),
            "\"reject_delivery_attempts\""
        );
        let capabilities = Capabilities::v5(1_024, 10);
        assert_eq!(capabilities.api_version, API_VERSION);
        assert_eq!(
            capabilities.publish.transports[1].message,
            RETICULUM_UNAVAILABLE_MESSAGE
        );
    }
}
