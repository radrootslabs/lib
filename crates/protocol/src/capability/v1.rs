//! Capability catalog contract generation 1.

use alloc::string::{String, ToString};
use core::fmt;

use crate::schema::{Metadata, ModuleVersion, Registry};

/// Stable wire identity for a supported transport family.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TransportKind {
    /// Process-local transport.
    Local,
    /// Nostr relay transport.
    Nostr,
    /// Reticulum mesh transport.
    Reticulum,
}

impl TransportKind {
    /// Returns the stable serialized identity.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Nostr => "nostr",
            Self::Reticulum => "reticulum",
        }
    }

    /// Parses an exact stable transport identity.
    pub fn parse(value: &str) -> Result<Self, Error> {
        match value {
            "local" => Ok(Self::Local),
            "nostr" => Ok(Self::Nostr),
            "reticulum" => Ok(Self::Reticulum),
            _ => Err(Error::UnknownTransportKind {
                value: value.to_string(),
            }),
        }
    }
}

/// Product maturity of a capability.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Maturity {
    /// Supported as a preview contract.
    Preview,
    /// Supported as a stable contract.
    Stable,
}

/// Current availability of a capability.
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

/// Validated mesh-scope identifier.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MeshScopeId {
    value: String,
}

impl MeshScopeId {
    /// Parses the existing V1 mesh-scope grammar.
    pub fn parse(value: impl Into<String>) -> Result<Self, Error> {
        let value = value.into();
        if value.is_empty()
            || value != value.trim()
            || value.chars().any(|character| {
                !(character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.'))
            })
        {
            return Err(Error::InvalidMeshScopeId);
        }
        Ok(Self { value })
    }

    /// Returns the validated identifier.
    pub fn as_str(&self) -> &str {
        self.value.as_str()
    }
}

/// Validated Reticulum destination.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ReticulumDestination {
    canonical: String,
}

impl ReticulumDestination {
    /// Parses the existing V1 Reticulum destination grammar.
    pub fn parse(value: impl Into<String>) -> Result<Self, Error> {
        let value = value.into();
        if value.is_empty()
            || value != value.trim()
            || value
                .chars()
                .any(|character| character.is_ascii_control() || character.is_ascii_whitespace())
        {
            return Err(Error::InvalidReticulumDestination);
        }
        Ok(Self { canonical: value })
    }

    /// Returns the canonical destination text.
    pub fn as_str(&self) -> &str {
        self.canonical.as_str()
    }
}

/// Passive Reticulum target DTO.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReticulumTarget {
    /// Canonical destination.
    pub destination: ReticulumDestination,
    /// Optional mesh scope.
    pub mesh_scope: Option<MeshScopeId>,
}

/// Passive capability descriptor DTO.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransportDescriptor {
    /// Transport family.
    pub kind: TransportKind,
    /// Product maturity.
    pub maturity: Maturity,
    /// Current availability.
    pub availability: Availability,
    /// Whether delivery is defined.
    pub can_deliver: bool,
    /// Whether fetch is defined.
    pub can_fetch: bool,
    /// Whether discovery is defined.
    pub can_discover: bool,
    /// Whether gateway forwarding is defined.
    pub can_gateway_forward: bool,
    /// Whether delivery receipts are observable.
    pub can_observe_receipts: bool,
    /// Whether Release V1 requires the transport contract.
    pub required_for_v1: bool,
}

/// Exact Release V1 transport capability catalog.
pub const CATALOG: &[TransportDescriptor] = &[
    TransportDescriptor {
        kind: TransportKind::Local,
        maturity: Maturity::Stable,
        availability: Availability::Available,
        can_deliver: true,
        can_fetch: true,
        can_discover: false,
        can_gateway_forward: false,
        can_observe_receipts: true,
        required_for_v1: true,
    },
    TransportDescriptor {
        kind: TransportKind::Nostr,
        maturity: Maturity::Stable,
        availability: Availability::Available,
        can_deliver: true,
        can_fetch: true,
        can_discover: true,
        can_gateway_forward: false,
        can_observe_receipts: true,
        required_for_v1: true,
    },
    TransportDescriptor {
        kind: TransportKind::Reticulum,
        maturity: Maturity::Preview,
        availability: Availability::Unavailable,
        can_deliver: true,
        can_fetch: false,
        can_discover: true,
        can_gateway_forward: true,
        can_observe_receipts: true,
        required_for_v1: true,
    },
];

/// Exact schema identities retained from the predecessor package.
pub const SCHEMAS: &[Metadata] = &[
    Metadata {
        type_name: "TransportKindV1",
        schema_id: "radroots.protocol.transport_kind.v1",
        schema_version: 1,
    },
    Metadata {
        type_name: "TransportCapabilityDescriptorV1",
        schema_id: "radroots.protocol.transport_capability_descriptor.v1",
        schema_version: 1,
    },
    Metadata {
        type_name: "ReticulumTargetV1",
        schema_id: "radroots.protocol.reticulum_target.v1",
        schema_version: 1,
    },
];

/// Validates catalog uniqueness and required V1 membership.
pub fn validate_catalog(descriptors: &[TransportDescriptor]) -> Result<(), Error> {
    let mut seen = [false; 3];
    for descriptor in descriptors {
        let index = match descriptor.kind {
            TransportKind::Local => 0,
            TransportKind::Nostr => 1,
            TransportKind::Reticulum => 2,
        };
        if seen[index] {
            return Err(Error::DuplicateTransportKind {
                kind: descriptor.kind,
            });
        }
        seen[index] = true;
    }

    for (index, kind) in [
        TransportKind::Local,
        TransportKind::Nostr,
        TransportKind::Reticulum,
    ]
    .into_iter()
    .enumerate()
    {
        if !seen[index] {
            return Err(Error::MissingRequiredTransport { kind });
        }
    }
    Ok(())
}

/// Builds the validated capability schema registry.
pub fn schema_registry() -> Result<Registry, crate::schema::Error> {
    Registry::try_from_metadata(
        SCHEMAS
            .iter()
            .copied()
            .map(|metadata| (metadata, ModuleVersion::CapabilityV1)),
    )
}

/// Capability V1 validation failure.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Error {
    /// The transport identity is unknown.
    UnknownTransportKind {
        /// Rejected transport identity.
        value: String,
    },
    /// A mesh-scope identifier is malformed.
    InvalidMeshScopeId,
    /// A Reticulum destination is malformed.
    InvalidReticulumDestination,
    /// A transport appears more than once in a catalog.
    DuplicateTransportKind {
        /// Duplicated transport family.
        kind: TransportKind,
    },
    /// A required V1 transport is absent.
    MissingRequiredTransport {
        /// Missing transport family.
        kind: TransportKind,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownTransportKind { value } => {
                write!(formatter, "unknown transport kind {value}")
            }
            Self::InvalidMeshScopeId => formatter.write_str("invalid mesh scope id"),
            Self::InvalidReticulumDestination => {
                formatter.write_str("invalid Reticulum destination")
            }
            Self::DuplicateTransportKind { kind } => {
                write!(formatter, "duplicate transport kind {}", kind.as_str())
            }
            Self::MissingRequiredTransport { kind } => {
                write!(formatter, "missing required transport {}", kind.as_str())
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_and_schema_registry_validate() {
        validate_catalog(CATALOG).expect("catalog");
        let registry = schema_registry().expect("schema registry");
        assert_eq!(registry.len(), SCHEMAS.len());
        assert!(
            registry
                .descriptors()
                .iter()
                .all(|descriptor| descriptor.module() == ModuleVersion::CapabilityV1)
        );
    }

    #[test]
    fn parsers_preserve_v1_acceptance_and_diagnostics() {
        for (value, expected) in [
            ("local", TransportKind::Local),
            ("nostr", TransportKind::Nostr),
            ("reticulum", TransportKind::Reticulum),
        ] {
            assert_eq!(TransportKind::parse(value), Ok(expected));
            assert_eq!(expected.as_str(), value);
        }
        assert_eq!(
            TransportKind::parse("mesh")
                .expect_err("unknown")
                .to_string(),
            "unknown transport kind mesh"
        );
        assert_eq!(
            MeshScopeId::parse("local/scope")
                .expect_err("invalid scope")
                .to_string(),
            "invalid mesh scope id"
        );
        assert_eq!(
            ReticulumDestination::parse("reticulum:\nlocal")
                .expect_err("invalid destination")
                .to_string(),
            "invalid Reticulum destination"
        );
    }

    #[test]
    fn catalog_rejects_duplicates_and_missing_required_transports() {
        assert_eq!(
            validate_catalog(&[CATALOG[0], CATALOG[0]]),
            Err(Error::DuplicateTransportKind {
                kind: TransportKind::Local,
            })
        );
        assert_eq!(
            validate_catalog(&[CATALOG[1], CATALOG[2]]),
            Err(Error::MissingRequiredTransport {
                kind: TransportKind::Local,
            })
        );
    }
}
