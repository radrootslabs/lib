use crate::{
    RADROOTS_TRANSPORT_OUTCOME_CODE_MAX_BYTES, RADROOTS_TRANSPORT_OUTCOME_MESSAGE_MAX_BYTES,
    RadrootsTransportCapabilityAvailability, RadrootsTransportCapabilityMaturity,
    RadrootsTransportImplementationState, RadrootsTransportKind,
    delivery::RadrootsTransportSatisfactionClass,
};
use alloc::string::String;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RadrootsTransportDeliveryTargetStatus {
    Pending,
    Accepted,
    Delivered,
    Forwarded,
    StoredByGateway,
    Seen,
    DeferredUntilImplemented,
    SkippedPolicyDenied,
    FailedRetryable,
    FailedTerminal,
}

impl RadrootsTransportDeliveryTargetStatus {
    pub fn is_ready_for_attempt(self) -> bool {
        matches!(self, Self::Pending | Self::FailedRetryable)
    }

    pub fn counts_as_accepted_satisfaction(self) -> bool {
        matches!(
            self,
            Self::Accepted | Self::Delivered | Self::Forwarded | Self::StoredByGateway | Self::Seen
        )
    }

    pub fn counts_as_delivered_satisfaction(self) -> bool {
        matches!(self, Self::Delivered)
    }

    pub fn counts_as_satisfied(
        self,
        satisfaction_class: RadrootsTransportSatisfactionClass,
    ) -> bool {
        match satisfaction_class {
            RadrootsTransportSatisfactionClass::Accepted => self.counts_as_accepted_satisfaction(),
            RadrootsTransportSatisfactionClass::Forwarded => {
                matches!(self, Self::Forwarded | Self::Delivered)
            }
            RadrootsTransportSatisfactionClass::Stored => matches!(self, Self::StoredByGateway),
            RadrootsTransportSatisfactionClass::Seen => {
                matches!(self, Self::Seen | Self::Delivered)
            }
            RadrootsTransportSatisfactionClass::Delivered => {
                self.counts_as_delivered_satisfaction()
            }
            RadrootsTransportSatisfactionClass::DurableOrObserved => {
                matches!(self, Self::StoredByGateway | Self::Seen | Self::Delivered)
            }
        }
    }

    pub fn is_retryable_failure(self) -> bool {
        matches!(self, Self::FailedRetryable)
    }

    pub fn is_terminal_failure(self) -> bool {
        matches!(self, Self::SkippedPolicyDenied | Self::FailedTerminal)
    }

    pub fn is_deferred_until_implemented(self) -> bool {
        matches!(self, Self::DeferredUntilImplemented)
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RadrootsTransportOutcomeKind {
    Accepted,
    DuplicateAccepted,
    Delivered,
    Forwarded,
    StoredByGateway,
    Seen,
    DeferredUntilImplemented,
    Rejected,
    RouteUnavailable,
    PayloadTooLarge,
    PolicyDenied,
    Timeout,
    ConnectionFailed,
    TransportUnavailable,
}

impl RadrootsTransportOutcomeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::DuplicateAccepted => "duplicate_accepted",
            Self::Delivered => "delivered",
            Self::Forwarded => "forwarded",
            Self::StoredByGateway => "stored_by_gateway",
            Self::Seen => "seen",
            Self::DeferredUntilImplemented => "deferred_until_implemented",
            Self::Rejected => "rejected",
            Self::RouteUnavailable => "route_unavailable",
            Self::PayloadTooLarge => "payload_too_large",
            Self::PolicyDenied => "policy_denied",
            Self::Timeout => "timeout",
            Self::ConnectionFailed => "connection_failed",
            Self::TransportUnavailable => "transport_unavailable",
        }
    }

    pub fn target_status(self) -> RadrootsTransportDeliveryTargetStatus {
        match self {
            Self::Accepted | Self::DuplicateAccepted => {
                RadrootsTransportDeliveryTargetStatus::Accepted
            }
            Self::Delivered => RadrootsTransportDeliveryTargetStatus::Delivered,
            Self::Forwarded => RadrootsTransportDeliveryTargetStatus::Forwarded,
            Self::StoredByGateway => RadrootsTransportDeliveryTargetStatus::StoredByGateway,
            Self::Seen => RadrootsTransportDeliveryTargetStatus::Seen,
            Self::DeferredUntilImplemented => {
                RadrootsTransportDeliveryTargetStatus::DeferredUntilImplemented
            }
            Self::PolicyDenied => RadrootsTransportDeliveryTargetStatus::SkippedPolicyDenied,
            Self::Timeout | Self::ConnectionFailed | Self::TransportUnavailable => {
                RadrootsTransportDeliveryTargetStatus::FailedRetryable
            }
            Self::Rejected | Self::RouteUnavailable | Self::PayloadTooLarge => {
                RadrootsTransportDeliveryTargetStatus::FailedTerminal
            }
        }
    }

    pub fn counts_as_satisfied(
        self,
        satisfaction_class: RadrootsTransportSatisfactionClass,
    ) -> bool {
        self.target_status().counts_as_satisfied(satisfaction_class)
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTransportOutcome {
    kind: RadrootsTransportOutcomeKind,
    status: RadrootsTransportDeliveryTargetStatus,
    code: Option<String>,
    message: Option<String>,
}

impl RadrootsTransportOutcome {
    pub fn new(kind: RadrootsTransportOutcomeKind) -> Self {
        Self {
            kind,
            status: kind.target_status(),
            code: None,
            message: None,
        }
    }

    pub fn try_with_message(
        mut self,
        message: impl Into<String>,
    ) -> Result<Self, crate::RadrootsTransportError> {
        let message = message.into();
        crate::limits::ensure_resource_limit(
            "transport_outcome_message",
            message.len(),
            RADROOTS_TRANSPORT_OUTCOME_MESSAGE_MAX_BYTES,
        )?;
        self.message = Some(message);
        Ok(self)
    }

    pub fn try_with_code(
        mut self,
        code: impl Into<String>,
    ) -> Result<Self, crate::RadrootsTransportError> {
        let code = code.into();
        crate::limits::ensure_resource_limit(
            "transport_outcome_code",
            code.len(),
            RADROOTS_TRANSPORT_OUTCOME_CODE_MAX_BYTES,
        )?;
        self.code = Some(code);
        Ok(self)
    }

    pub fn kind(&self) -> RadrootsTransportOutcomeKind {
        self.kind
    }

    pub fn status(&self) -> RadrootsTransportDeliveryTargetStatus {
        self.status
    }

    pub fn code(&self) -> Option<&str> {
        self.code.as_deref()
    }

    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    pub fn validate(&self) -> Result<(), crate::RadrootsTransportError> {
        if self.status != self.kind.target_status() {
            return Err(crate::RadrootsTransportError::TransportOutcomeStatusMismatch);
        }
        if let Some(code) = &self.code {
            crate::limits::ensure_resource_limit(
                "transport_outcome_code",
                code.len(),
                RADROOTS_TRANSPORT_OUTCOME_CODE_MAX_BYTES,
            )?;
        }
        if let Some(message) = &self.message {
            crate::limits::ensure_resource_limit(
                "transport_outcome_message",
                message.len(),
                RADROOTS_TRANSPORT_OUTCOME_MESSAGE_MAX_BYTES,
            )?;
        }
        Ok(())
    }
}

#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RadrootsTransportOutcomeWire {
    kind: RadrootsTransportOutcomeKind,
    status: RadrootsTransportDeliveryTargetStatus,
    #[serde(deserialize_with = "deserialize_outcome_code")]
    code: Option<String>,
    #[serde(deserialize_with = "deserialize_outcome_message")]
    message: Option<String>,
}

#[cfg(feature = "serde")]
fn deserialize_outcome_code<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    crate::serde_bounds::deserialize_option_string(
        deserializer,
        "transport_outcome_code",
        RADROOTS_TRANSPORT_OUTCOME_CODE_MAX_BYTES,
    )
}

#[cfg(feature = "serde")]
fn deserialize_outcome_message<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    crate::serde_bounds::deserialize_option_string(
        deserializer,
        "transport_outcome_message",
        RADROOTS_TRANSPORT_OUTCOME_MESSAGE_MAX_BYTES,
    )
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for RadrootsTransportOutcome {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = RadrootsTransportOutcomeWire::deserialize(deserializer)?;
        let outcome = Self {
            kind: wire.kind,
            status: wire.status,
            code: wire.code,
            message: wire.message,
        };
        outcome.validate().map_err(serde::de::Error::custom)?;
        Ok(outcome)
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTransportCapabilities {
    pub deliver: bool,
    pub fetch: bool,
    pub discovery: bool,
    pub gateway_forwarding: bool,
    pub receipt_observation: bool,
}

impl RadrootsTransportCapabilities {
    pub const fn none() -> Self {
        Self {
            deliver: false,
            fetch: false,
            discovery: false,
            gateway_forwarding: false,
            receipt_observation: false,
        }
    }

    pub const fn deliver_only() -> Self {
        Self {
            deliver: true,
            fetch: false,
            discovery: false,
            gateway_forwarding: false,
            receipt_observation: false,
        }
    }

    pub const fn fetch_only() -> Self {
        Self {
            deliver: false,
            fetch: true,
            discovery: false,
            gateway_forwarding: false,
            receipt_observation: false,
        }
    }

    pub const fn deliver_and_fetch() -> Self {
        Self {
            deliver: true,
            fetch: true,
            discovery: false,
            gateway_forwarding: false,
            receipt_observation: false,
        }
    }

    pub const fn with_discovery(mut self, discovery: bool) -> Self {
        self.discovery = discovery;
        self
    }

    pub const fn with_gateway_forwarding(mut self, gateway_forwarding: bool) -> Self {
        self.gateway_forwarding = gateway_forwarding;
        self
    }

    pub const fn with_receipt_observation(mut self, receipt_observation: bool) -> Self {
        self.receipt_observation = receipt_observation;
        self
    }

    pub const fn reticulum_unavailable() -> Self {
        Self {
            deliver: false,
            fetch: false,
            discovery: false,
            gateway_forwarding: false,
            receipt_observation: false,
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTransportStatus {
    #[cfg_attr(feature = "serde", serde(rename = "transport"))]
    pub kind: RadrootsTransportKind,
    pub profile_id: Option<String>,
    pub endpoint_uri: Option<String>,
    pub configured: bool,
    pub implementation: RadrootsTransportImplementationState,
    pub maturity: RadrootsTransportCapabilityMaturity,
    pub availability: RadrootsTransportCapabilityAvailability,
    pub usable_for_delivery: bool,
    pub capabilities: RadrootsTransportCapabilities,
    pub message: String,
}

impl RadrootsTransportStatus {
    pub fn new(
        kind: RadrootsTransportKind,
        configured: bool,
        implementation: RadrootsTransportImplementationState,
        usable_for_delivery: bool,
        message: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            profile_id: None,
            endpoint_uri: None,
            configured,
            implementation,
            maturity: RadrootsTransportCapabilityMaturity::Stable,
            availability: if usable_for_delivery {
                RadrootsTransportCapabilityAvailability::Available
            } else {
                RadrootsTransportCapabilityAvailability::Unavailable
            },
            usable_for_delivery,
            capabilities: if usable_for_delivery {
                RadrootsTransportCapabilities::deliver_only()
            } else {
                RadrootsTransportCapabilities::none()
            },
            message: message.into(),
        }
    }

    pub fn with_capabilities(mut self, capabilities: RadrootsTransportCapabilities) -> Self {
        self.capabilities = capabilities;
        self
    }

    pub fn with_maturity(mut self, maturity: RadrootsTransportCapabilityMaturity) -> Self {
        self.maturity = maturity;
        self
    }

    pub fn with_availability(
        mut self,
        availability: RadrootsTransportCapabilityAvailability,
    ) -> Self {
        self.availability = availability;
        self
    }

    pub fn with_profile_id(mut self, profile_id: impl Into<String>) -> Self {
        self.profile_id = Some(profile_id.into());
        self
    }

    pub fn with_endpoint_uri(mut self, endpoint_uri: impl Into<String>) -> Self {
        self.endpoint_uri = Some(endpoint_uri.into());
        self
    }
}
