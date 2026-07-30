//! Capability catalog contract generation 1.

use alloc::string::{String, ToString};
use core::fmt;

use crate::schema::{Metadata, ModuleVersion, Registry};

/// Maximum encoded length of a capability transport identity.
pub const MAX_TRANSPORT_KIND_BYTES: usize = 64;

/// Stable wire identity for a transport family.
///
/// The representation is intentionally open so adding a transport does not
/// require adding an enum variant to this versioned wire contract.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TransportKind {
    bytes: [u8; MAX_TRANSPORT_KIND_BYTES],
    len: u8,
}

impl TransportKind {
    /// Process-local transport.
    pub const LOCAL: Self = Self::from_static(b"local");
    /// Nostr relay transport.
    pub const NOSTR: Self = Self::from_static(b"nostr");
    /// Reticulum mesh transport.
    pub const RETICULUM: Self = Self::from_static(b"reticulum");
    /// Daemon-mediated transport.
    pub const RADROOTSD: Self = Self::from_static(b"radrootsd");

    const fn from_static(value: &[u8]) -> Self {
        let mut bytes = [0; MAX_TRANSPORT_KIND_BYTES];
        let mut index = 0;
        while index < value.len() {
            bytes[index] = value[index];
            index += 1;
        }
        Self {
            bytes,
            len: value.len() as u8,
        }
    }

    /// Parses an exact canonical transport identity.
    ///
    /// Identities contain 1-64 lowercase ASCII bytes. They begin and end with
    /// an ASCII letter or digit and may use single `-` separators internally.
    pub fn parse(value: &str) -> Result<Self, Error> {
        if value.is_empty() {
            return Err(Error::EmptyTransportKind);
        }
        let raw = value.as_bytes();
        let valid_edge = |byte: u8| byte.is_ascii_lowercase() || byte.is_ascii_digit();
        let valid = raw.len() <= MAX_TRANSPORT_KIND_BYTES
            && valid_edge(raw[0])
            && valid_edge(raw[raw.len() - 1])
            && raw.iter().enumerate().all(|(index, byte)| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || (*byte == b'-' && index > 0 && raw[index - 1] != b'-')
            });
        if !valid {
            return Err(Error::InvalidTransportKind {
                value: value.to_string(),
            });
        }

        let mut bytes = [0; MAX_TRANSPORT_KIND_BYTES];
        bytes[..raw.len()].copy_from_slice(raw);
        Ok(Self {
            bytes,
            len: raw.len() as u8,
        })
    }

    /// Returns the validated wire identity.
    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.bytes[..usize::from(self.len)])
            .expect("TransportKind stores validated ASCII")
    }
}

impl fmt::Display for TransportKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for TransportKind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for TransportKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = <String as serde::Deserialize>::deserialize(deserializer)?;
        Self::parse(value.as_str()).map_err(serde::de::Error::custom)
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
        kind: TransportKind::LOCAL,
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
        kind: TransportKind::NOSTR,
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
        kind: TransportKind::RETICULUM,
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
    for (index, descriptor) in descriptors.iter().enumerate() {
        if descriptors[..index]
            .iter()
            .any(|candidate| candidate.kind == descriptor.kind)
        {
            return Err(Error::DuplicateTransportKind {
                kind: descriptor.kind,
            });
        }
    }

    for kind in [
        TransportKind::LOCAL,
        TransportKind::NOSTR,
        TransportKind::RETICULUM,
    ] {
        if !descriptors.iter().any(|descriptor| descriptor.kind == kind) {
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
    /// The transport identity is empty.
    EmptyTransportKind,
    /// The transport identity is not canonical or exceeds its bound.
    InvalidTransportKind {
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
            Self::EmptyTransportKind => formatter.write_str("transport kind is empty"),
            Self::InvalidTransportKind { value } => {
                write!(formatter, "invalid transport kind {value}")
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
    fn transport_kind_accepts_built_ins_and_forward_compatible_values() {
        for (value, expected) in [
            ("local", TransportKind::LOCAL),
            ("nostr", TransportKind::NOSTR),
            ("reticulum", TransportKind::RETICULUM),
            ("radrootsd", TransportKind::RADROOTSD),
        ] {
            assert_eq!(TransportKind::parse(value), Ok(expected));
            assert_eq!(expected.as_str(), value);
        }
        assert_eq!(
            TransportKind::parse("fieldbus-v2").unwrap().as_str(),
            "fieldbus-v2"
        );
        for invalid in ["", "NOSTR", " fieldbus", "fieldbus_2", "fieldbus--2"] {
            assert!(
                TransportKind::parse(invalid).is_err(),
                "accepted {invalid:?}"
            );
        }
        assert!(TransportKind::parse(&"a".repeat(MAX_TRANSPORT_KIND_BYTES + 1)).is_err());
    }

    #[test]
    fn other_capability_parsers_preserve_v1_diagnostics() {
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
                kind: TransportKind::LOCAL,
            })
        );
        assert_eq!(
            validate_catalog(&[CATALOG[1], CATALOG[2]]),
            Err(Error::MissingRequiredTransport {
                kind: TransportKind::LOCAL,
            })
        );
    }
}
