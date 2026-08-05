use crate::{
    TransportId,
    capability::{Availability, Maturity, SinkCapabilities, SourceCapabilities},
};
use alloc::string::String;
use radroots_protocol::capability::v1::TransportDescriptor;

/// Runtime state shared by source and sink capability reports.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, Eq, PartialEq)]
struct RuntimeStatus {
    transport_id: TransportId,
    configured: bool,
    maturity: Maturity,
    availability: Availability,
    message: String,
}

impl RuntimeStatus {
    fn new(
        transport_id: TransportId,
        configured: bool,
        maturity: Maturity,
        availability: Availability,
        message: impl Into<String>,
    ) -> Self {
        Self {
            transport_id,
            configured,
            maturity,
            availability,
            message: message.into(),
        }
    }

    const fn transport_id(&self) -> TransportId {
        self.transport_id
    }

    const fn is_configured(&self) -> bool {
        self.configured
    }

    const fn maturity(&self) -> Maturity {
        self.maturity
    }

    const fn availability(&self) -> Availability {
        self.availability
    }

    fn message(&self) -> &str {
        self.message.as_str()
    }
}

/// Current state and operations supported by an event source.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceStatus {
    #[cfg_attr(feature = "serde", serde(flatten))]
    status: RuntimeStatus,
    capabilities: SourceCapabilities,
}

impl SourceStatus {
    /// Creates a source-specific status report.
    pub fn new(
        transport_id: TransportId,
        configured: bool,
        maturity: Maturity,
        availability: Availability,
        capabilities: SourceCapabilities,
        message: impl Into<String>,
    ) -> Self {
        Self {
            status: RuntimeStatus::new(transport_id, configured, maturity, availability, message),
            capabilities,
        }
    }

    /// Builds source status from the versioned passive capability catalog.
    pub fn from_descriptor(
        descriptor: &TransportDescriptor,
        configured: bool,
        message: impl Into<String>,
    ) -> Self {
        Self::new(
            descriptor.kind.into(),
            configured,
            descriptor.maturity.into(),
            descriptor.availability.into(),
            SourceCapabilities::from(descriptor),
            message,
        )
    }

    /// Returns the extensible transport identity.
    pub const fn transport_id(&self) -> TransportId {
        self.status.transport_id()
    }

    /// Whether the host has configured this source.
    pub const fn is_configured(&self) -> bool {
        self.status.is_configured()
    }

    /// Returns the source capability's product maturity.
    pub const fn maturity(&self) -> Maturity {
        self.status.maturity()
    }

    /// Returns current source availability.
    pub const fn availability(&self) -> Availability {
        self.status.availability()
    }

    /// Returns the operator-facing status detail.
    pub fn message(&self) -> &str {
        self.status.message()
    }

    /// Returns supported source operations.
    pub const fn capabilities(&self) -> SourceCapabilities {
        self.capabilities
    }
}

/// Current state and operations supported by an event sink.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SinkStatus {
    #[cfg_attr(feature = "serde", serde(flatten))]
    status: RuntimeStatus,
    capabilities: SinkCapabilities,
}

impl SinkStatus {
    /// Creates a sink-specific status report.
    pub fn new(
        transport_id: TransportId,
        configured: bool,
        maturity: Maturity,
        availability: Availability,
        capabilities: SinkCapabilities,
        message: impl Into<String>,
    ) -> Self {
        Self {
            status: RuntimeStatus::new(transport_id, configured, maturity, availability, message),
            capabilities,
        }
    }

    /// Builds sink status from the versioned passive capability catalog.
    pub fn from_descriptor(
        descriptor: &TransportDescriptor,
        configured: bool,
        message: impl Into<String>,
    ) -> Self {
        Self::new(
            descriptor.kind.into(),
            configured,
            descriptor.maturity.into(),
            descriptor.availability.into(),
            SinkCapabilities::from(descriptor),
            message,
        )
    }

    /// Returns the extensible transport identity.
    pub const fn transport_id(&self) -> TransportId {
        self.status.transport_id()
    }

    /// Whether the host has configured this sink.
    pub const fn is_configured(&self) -> bool {
        self.status.is_configured()
    }

    /// Returns the sink capability's product maturity.
    pub const fn maturity(&self) -> Maturity {
        self.status.maturity()
    }

    /// Returns current sink availability.
    pub const fn availability(&self) -> Availability {
        self.status.availability()
    }

    /// Returns the operator-facing status detail.
    pub fn message(&self) -> &str {
        self.status.message()
    }

    /// Returns supported sink operations.
    pub const fn capabilities(&self) -> SinkCapabilities {
        self.capabilities
    }
}
