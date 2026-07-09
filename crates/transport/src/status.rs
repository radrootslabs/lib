use crate::{
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
    PreviewUnavailable,
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
        matches!(
            self,
            Self::Delivered | Self::Forwarded | Self::StoredByGateway | Self::Seen
        )
    }

    pub fn counts_as_satisfied(
        self,
        satisfaction_class: RadrootsTransportSatisfactionClass,
    ) -> bool {
        match satisfaction_class {
            RadrootsTransportSatisfactionClass::Accepted => self.counts_as_accepted_satisfaction(),
            RadrootsTransportSatisfactionClass::Delivered => {
                self.counts_as_delivered_satisfaction()
            }
        }
    }

    pub fn is_retryable_failure(self) -> bool {
        matches!(self, Self::FailedRetryable)
    }

    pub fn is_terminal_failure(self) -> bool {
        matches!(self, Self::SkippedPolicyDenied | Self::FailedTerminal)
    }

    pub fn is_deferred_preview(self) -> bool {
        matches!(
            self,
            Self::DeferredUntilImplemented | Self::PreviewUnavailable
        )
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTransportOutcome {
    pub status: RadrootsTransportDeliveryTargetStatus,
    pub code: Option<String>,
    pub message: Option<String>,
}

impl RadrootsTransportOutcome {
    pub fn new(status: RadrootsTransportDeliveryTargetStatus) -> Self {
        Self {
            status,
            code: None,
            message: None,
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
    pub usable_for_delivery: bool,
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
            usable_for_delivery,
            message: message.into(),
        }
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
