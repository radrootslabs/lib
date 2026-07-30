//! Transport-neutral capability vocabulary.

use radroots_protocol::capability::v1 as protocol;

/// Product maturity of a transport capability.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Maturity {
    /// Contract may change without compatibility guarantees.
    Experimental,
    /// Contract is available for preview use.
    Preview,
    /// Contract is supported as stable.
    Stable,
}

/// Current runtime availability of a transport capability.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Availability {
    /// Fully available.
    Available,
    /// Available with reduced functionality.
    Degraded,
    /// Not currently available.
    Unavailable,
}

impl From<protocol::Maturity> for Maturity {
    fn from(value: protocol::Maturity) -> Self {
        match value {
            protocol::Maturity::Experimental => Self::Experimental,
            protocol::Maturity::Preview => Self::Preview,
            protocol::Maturity::Stable => Self::Stable,
        }
    }
}

impl From<Maturity> for protocol::Maturity {
    fn from(value: Maturity) -> Self {
        match value {
            Maturity::Experimental => Self::Experimental,
            Maturity::Preview => Self::Preview,
            Maturity::Stable => Self::Stable,
        }
    }
}

impl From<protocol::Availability> for Availability {
    fn from(value: protocol::Availability) -> Self {
        match value {
            protocol::Availability::Available => Self::Available,
            protocol::Availability::Degraded => Self::Degraded,
            protocol::Availability::Unavailable => Self::Unavailable,
        }
    }
}

impl From<Availability> for protocol::Availability {
    fn from(value: Availability) -> Self {
        match value {
            Availability::Available => Self::Available,
            Availability::Degraded => Self::Degraded,
            Availability::Unavailable => Self::Unavailable,
        }
    }
}

/// Operations supported by an event source.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SourceCapabilities {
    fetch: bool,
    discovery: bool,
}

impl SourceCapabilities {
    /// No source operations are supported.
    pub const NONE: Self = Self {
        fetch: false,
        discovery: false,
    };

    /// Bounded fetch is supported.
    pub const FETCH: Self = Self {
        fetch: true,
        discovery: false,
    };

    /// Sets discovery support.
    #[must_use]
    pub const fn with_discovery(mut self, discovery: bool) -> Self {
        self.discovery = discovery;
        self
    }

    /// Whether bounded fetch is supported.
    pub const fn can_fetch(self) -> bool {
        self.fetch
    }

    /// Whether target discovery is supported.
    pub const fn can_discover(self) -> bool {
        self.discovery
    }
}

impl From<&protocol::TransportDescriptor> for SourceCapabilities {
    fn from(value: &protocol::TransportDescriptor) -> Self {
        Self {
            fetch: value.can_fetch,
            discovery: value.can_discover,
        }
    }
}

/// Operations supported by an event sink.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SinkCapabilities {
    deliver: bool,
    gateway_forwarding: bool,
    receipt_observation: bool,
}

impl SinkCapabilities {
    /// No sink operations are supported.
    pub const NONE: Self = Self {
        deliver: false,
        gateway_forwarding: false,
        receipt_observation: false,
    };

    /// Delivery is supported.
    pub const DELIVER: Self = Self {
        deliver: true,
        gateway_forwarding: false,
        receipt_observation: false,
    };

    /// Sets gateway-forwarding support.
    #[must_use]
    pub const fn with_gateway_forwarding(mut self, supported: bool) -> Self {
        self.gateway_forwarding = supported;
        self
    }

    /// Sets receipt-observation support.
    #[must_use]
    pub const fn with_receipt_observation(mut self, supported: bool) -> Self {
        self.receipt_observation = supported;
        self
    }

    /// Whether delivery is supported.
    pub const fn can_deliver(self) -> bool {
        self.deliver
    }

    /// Whether gateway forwarding is supported.
    pub const fn can_gateway_forward(self) -> bool {
        self.gateway_forwarding
    }

    /// Whether delivery receipt observation is supported.
    pub const fn can_observe_receipts(self) -> bool {
        self.receipt_observation
    }
}

impl From<&protocol::TransportDescriptor> for SinkCapabilities {
    fn from(value: &protocol::TransportDescriptor) -> Self {
        Self {
            deliver: value.can_deliver,
            gateway_forwarding: value.can_gateway_forward,
            receipt_observation: value.can_observe_receipts,
        }
    }
}
