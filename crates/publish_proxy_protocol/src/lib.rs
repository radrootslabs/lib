#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};
#[cfg(feature = "std")]
use std::{string::String, vec::Vec};

use core::fmt;

pub const API_VERSION: &str = "radrootsd.publish_proxy.v1";
pub const DAEMON_NAME: &str = "radrootsd";
pub const METHOD_CAPABILITIES: &str = "publish.capabilities";
pub const METHOD_EVENT: &str = "publish.event";
pub const METHOD_JOB_GET: &str = "publish.job.get";
pub const METHOD_JOB_LIST: &str = "publish.job.list";
pub const METHOD_RELAYS_RESOLVE: &str = "publish.relays.resolve";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PublishProxyProtocolError {
    InvalidHexField {
        field: &'static str,
        expected_len: usize,
    },
    InvalidKind(u32),
    EmptyTag {
        index: usize,
    },
    EmptyIdempotencyKey,
    EmptyRelayUrl {
        index: usize,
    },
    RelayLimitExceeded {
        max: usize,
        actual: usize,
    },
    InvalidQuorum,
    EmptyPrincipalId,
    EmptyJobId,
}

impl fmt::Display for PublishProxyProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidHexField {
                field,
                expected_len,
            } => write!(f, "{field} must be {expected_len} lowercase hex characters"),
            Self::InvalidKind(kind) => write!(f, "event kind {kind} exceeds publish proxy range"),
            Self::EmptyTag { index } => write!(f, "tag {index} must not be empty"),
            Self::EmptyIdempotencyKey => f.write_str("idempotency key must not be empty"),
            Self::EmptyRelayUrl { index } => write!(f, "relay URL {index} must not be empty"),
            Self::RelayLimitExceeded { max, actual } => {
                write!(f, "relay count {actual} exceeds limit {max}")
            }
            Self::InvalidQuorum => f.write_str("delivery quorum must be greater than zero"),
            Self::EmptyPrincipalId => f.write_str("principal id must not be empty"),
            Self::EmptyJobId => f.write_str("job id must not be empty"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for PublishProxyProtocolError {}

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
    pub fn validate(&self) -> Result<(), PublishProxyProtocolError> {
        validate_lower_hex("id", self.id.as_str(), 64)?;
        validate_lower_hex("pubkey", self.pubkey.as_str(), 64)?;
        validate_lower_hex("sig", self.sig.as_str(), 128)?;
        if self.kind > u16::MAX as u32 {
            return Err(PublishProxyProtocolError::InvalidKind(self.kind));
        }
        for (index, tag) in self.tags.iter().enumerate() {
            if tag.is_empty() {
                return Err(PublishProxyProtocolError::EmptyTag { index });
            }
        }
        Ok(())
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PublishRelayPolicy {
    ExplicitOnly,
    RequestThenAuthorWriteThenDaemonDefault,
    AuthorWriteThenDaemonDefault,
    DaemonDefaultOnly,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "mode", rename_all = "snake_case"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PublishDeliveryPolicy {
    Any,
    All,
    Quorum { quorum: usize },
}

impl PublishDeliveryPolicy {
    pub fn validate(&self) -> Result<(), PublishProxyProtocolError> {
        if matches!(self, Self::Quorum { quorum: 0 }) {
            Err(PublishProxyProtocolError::InvalidQuorum)
        } else {
            Ok(())
        }
    }

    pub fn required_ack_count(&self, relay_count: usize) -> usize {
        match self {
            Self::Any => usize::from(relay_count > 0),
            Self::All => relay_count,
            Self::Quorum { quorum } => *quorum,
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublishEventRequest {
    pub event: SignedNostrEventWire,
    #[cfg_attr(feature = "serde", serde(default))]
    pub relays: Vec<String>,
    pub relay_policy: PublishRelayPolicy,
    pub delivery_policy: PublishDeliveryPolicy,
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

impl PublishEventRequest {
    pub fn validate(&self, max_relays: usize) -> Result<(), PublishProxyProtocolError> {
        self.event.validate()?;
        self.delivery_policy.validate()?;
        if self.relays.len() > max_relays {
            return Err(PublishProxyProtocolError::RelayLimitExceeded {
                max: max_relays,
                actual: self.relays.len(),
            });
        }
        for (index, relay) in self.relays.iter().enumerate() {
            if relay.trim().is_empty() {
                return Err(PublishProxyProtocolError::EmptyRelayUrl { index });
            }
        }
        if self
            .idempotency_key
            .as_ref()
            .is_some_and(|key| key.trim().is_empty())
        {
            return Err(PublishProxyProtocolError::EmptyIdempotencyKey);
        }
        Ok(())
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PublishJobStatus {
    Accepted,
    Publishing,
    DeliverySatisfied,
    DeliveryUnsatisfiedRetryable,
    DeliveryUnsatisfiedTerminal,
    Rejected,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PublishRelayOutcomeKind {
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
    RelayUrlRejected,
    SkippedAlreadyAccepted,
    Unknown,
}

impl PublishRelayOutcomeKind {
    pub fn counts_toward_quorum(self) -> bool {
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
                | Self::RelayUrlRejected
        )
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublishRelayOutcome {
    pub relay_url: String,
    pub source: PublishRelaySource,
    pub attempted: bool,
    pub outcome_kind: PublishRelayOutcomeKind,
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
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PublishRelaySource {
    Request,
    AuthorWrite,
    DaemonDefault,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublishJobView {
    pub job_id: String,
    pub status: PublishJobStatus,
    pub terminal: bool,
    pub delivery_satisfied: bool,
    pub event_id: String,
    pub pubkey: String,
    pub event_kind: u32,
    pub relay_policy: PublishRelayPolicy,
    pub delivery_policy: PublishDeliveryPolicy,
    pub relay_count: usize,
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
    pub relays: Vec<PublishRelayOutcome>,
}

impl PublishJobView {
    pub fn validate(&self) -> Result<(), PublishProxyProtocolError> {
        if self.job_id.trim().is_empty() {
            return Err(PublishProxyProtocolError::EmptyJobId);
        }
        validate_lower_hex("event_id", self.event_id.as_str(), 64)?;
        validate_lower_hex("pubkey", self.pubkey.as_str(), 64)?;
        if self.event_kind > u16::MAX as u32 {
            return Err(PublishProxyProtocolError::InvalidKind(self.event_kind));
        }
        self.delivery_policy.validate()
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublishEventResponse {
    pub deduplicated: bool,
    pub job: PublishJobView,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublishCapabilities {
    pub daemon: String,
    pub api_version: String,
    pub transports: Vec<String>,
    pub methods: Vec<String>,
    pub auth: PublishAuthCapabilities,
    pub publish: PublishSurfaceCapabilities,
}

impl PublishCapabilities {
    pub fn v1(max_event_bytes: usize, max_relays_per_request: usize) -> Self {
        Self {
            daemon: DAEMON_NAME.to_owned(),
            api_version: API_VERSION.to_owned(),
            transports: vec!["jsonrpc_http".to_owned()],
            methods: vec![
                METHOD_CAPABILITIES.to_owned(),
                METHOD_EVENT.to_owned(),
                METHOD_JOB_GET.to_owned(),
                METHOD_JOB_LIST.to_owned(),
                METHOD_RELAYS_RESOLVE.to_owned(),
            ],
            auth: PublishAuthCapabilities {
                mode: "scoped_bearer_token".to_owned(),
            },
            publish: PublishSurfaceCapabilities {
                signed_event_ingress: true,
                server_side_user_signing: false,
                max_event_bytes,
                max_relays_per_request,
                delivery_policies: vec![
                    PublishDeliveryPolicyName::Any,
                    PublishDeliveryPolicyName::Quorum,
                    PublishDeliveryPolicyName::All,
                ],
                relay_policies: vec![
                    PublishRelayPolicy::ExplicitOnly,
                    PublishRelayPolicy::RequestThenAuthorWriteThenDaemonDefault,
                    PublishRelayPolicy::AuthorWriteThenDaemonDefault,
                    PublishRelayPolicy::DaemonDefaultOnly,
                ],
            },
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublishAuthCapabilities {
    pub mode: String,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublishSurfaceCapabilities {
    pub signed_event_ingress: bool,
    pub server_side_user_signing: bool,
    pub max_event_bytes: usize,
    pub max_relays_per_request: usize,
    pub delivery_policies: Vec<PublishDeliveryPolicyName>,
    pub relay_policies: Vec<PublishRelayPolicy>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PublishDeliveryPolicyName {
    Any,
    Quorum,
    All,
}

fn validate_lower_hex(
    field: &'static str,
    value: &str,
    expected_len: usize,
) -> Result<(), PublishProxyProtocolError> {
    if value.len() == expected_len
        && value
            .as_bytes()
            .iter()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    {
        Ok(())
    } else {
        Err(PublishProxyProtocolError::InvalidHexField {
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
            content: "{\"name\":\"carrots\"}".to_owned(),
            sig: "2".repeat(128),
        }
    }

    #[test]
    fn signed_event_wire_uses_pubkey_and_rejects_author() {
        let value = serde_json::to_value(event()).expect("serialize");
        assert!(value.get("pubkey").is_some());
        assert!(value.get("author").is_none());

        let err = serde_json::from_value::<SignedNostrEventWire>(serde_json::json!({
            "id": "0".repeat(64),
            "author": "1".repeat(64),
            "created_at": 1_700_000_000u64,
            "kind": 30402u32,
            "tags": [["d", "listing-1"]],
            "content": "{}",
            "sig": "2".repeat(128)
        }))
        .expect_err("author must not be accepted");
        let message = err.to_string();
        assert!(message.contains("author"));
        assert!(message.contains("pubkey"));
    }

    #[test]
    fn signed_event_validation_rejects_malformed_fields() {
        let mut invalid_id = event();
        invalid_id.id = "A".repeat(64);
        assert!(matches!(
            invalid_id.validate(),
            Err(PublishProxyProtocolError::InvalidHexField { field: "id", .. })
        ));

        let mut invalid_kind = event();
        invalid_kind.kind = u16::MAX as u32 + 1;
        assert!(matches!(
            invalid_kind.validate(),
            Err(PublishProxyProtocolError::InvalidKind(_))
        ));

        let mut empty_tag = event();
        empty_tag.tags = vec![Vec::new()];
        assert!(matches!(
            empty_tag.validate(),
            Err(PublishProxyProtocolError::EmptyTag { index: 0 })
        ));
    }

    #[test]
    fn publish_request_validation_covers_policy_and_relay_limits() {
        let request = PublishEventRequest {
            event: event(),
            relays: vec!["wss://relay.example.com".to_owned()],
            relay_policy: PublishRelayPolicy::RequestThenAuthorWriteThenDaemonDefault,
            delivery_policy: PublishDeliveryPolicy::Quorum { quorum: 1 },
            idempotency_key: Some("key-1".to_owned()),
            timeout_ms: Some(10_000),
        };
        request.validate(1).expect("valid request");
        assert_eq!(request.delivery_policy.required_ack_count(3), 1);

        let mut too_many = request.clone();
        too_many.relays.push("wss://relay-2.example.com".to_owned());
        assert!(matches!(
            too_many.validate(1),
            Err(PublishProxyProtocolError::RelayLimitExceeded { max: 1, actual: 2 })
        ));

        let mut invalid_quorum = request;
        invalid_quorum.delivery_policy = PublishDeliveryPolicy::Quorum { quorum: 0 };
        assert!(matches!(
            invalid_quorum.validate(1),
            Err(PublishProxyProtocolError::InvalidQuorum)
        ));
    }

    #[test]
    fn capabilities_match_publish_proxy_v1_surface() {
        let capabilities = PublishCapabilities::v1(65_536, 20);
        let value = serde_json::to_value(&capabilities).expect("capabilities");
        assert_eq!(value["daemon"], DAEMON_NAME);
        assert_eq!(value["api_version"], API_VERSION);
        assert_eq!(value["auth"]["mode"], "scoped_bearer_token");
        assert_eq!(value["publish"]["server_side_user_signing"], false);
        assert!(
            value["methods"]
                .as_array()
                .expect("methods")
                .iter()
                .any(|method| method == METHOD_EVENT)
        );
    }

    #[test]
    fn outcome_kind_semantics_cover_daemon_results() {
        assert!(PublishRelayOutcomeKind::SkippedAlreadyAccepted.counts_toward_quorum());
        assert!(PublishRelayOutcomeKind::AuthRequired.is_retryable());
        assert!(PublishRelayOutcomeKind::RelayUrlRejected.is_terminal_failure());
        assert!(PublishRelayOutcomeKind::Muted.is_terminal_failure());
        assert!(PublishRelayOutcomeKind::PaymentRequired.is_terminal_failure());
    }
}
