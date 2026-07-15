#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

use alloc::collections::BTreeSet;
use alloc::string::{String, ToString};

pub const PROTOCOL_CONTRACT_NAME_V1: &str = "radroots.protocol";
pub const PROTOCOL_CONTRACT_VERSION_V1: u16 = 1;

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "dto-bindgen", dto(as = "string_enum"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TransportKindV1 {
    #[cfg_attr(feature = "dto-bindgen", dto(rename = "local"))]
    Local,
    #[cfg_attr(feature = "dto-bindgen", dto(rename = "nostr"))]
    Nostr,
    #[cfg_attr(feature = "dto-bindgen", dto(rename = "reticulum"))]
    Reticulum,
}

impl TransportKindV1 {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Nostr => "nostr",
            Self::Reticulum => "reticulum",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ProtocolContractErrorV1> {
        match value {
            "local" => Ok(Self::Local),
            "nostr" => Ok(Self::Nostr),
            "reticulum" => Ok(Self::Reticulum),
            "reticulum_preview" | "mesh" | "proxy" | "radrootsd_proxy" | "hybrid" => {
                Err(ProtocolContractErrorV1::RetiredTransportIdentity {
                    identity: value.to_string(),
                })
            }
            _ => Err(ProtocolContractErrorV1::UnknownTransportKind {
                value: value.to_string(),
            }),
        }
    }
}

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "dto-bindgen", dto(as = "string_enum"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CapabilityMaturityV1 {
    #[cfg_attr(feature = "dto-bindgen", dto(rename = "preview"))]
    Preview,
    #[cfg_attr(feature = "dto-bindgen", dto(rename = "stable"))]
    Stable,
}

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "dto-bindgen", dto(as = "string_enum"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CapabilityAvailabilityV1 {
    #[cfg_attr(feature = "dto-bindgen", dto(rename = "available"))]
    Available,
    #[cfg_attr(feature = "dto-bindgen", dto(rename = "degraded"))]
    Degraded,
    #[cfg_attr(feature = "dto-bindgen", dto(rename = "unavailable"))]
    Unavailable,
}

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MeshScopeIdV1 {
    value: String,
}

impl MeshScopeIdV1 {
    pub fn parse(value: impl Into<String>) -> Result<Self, ProtocolContractErrorV1> {
        let value = value.into();
        if value.is_empty()
            || value != value.trim()
            || value
                .chars()
                .any(|ch| !(ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.')))
        {
            return Err(ProtocolContractErrorV1::InvalidMeshScopeId);
        }
        Ok(Self { value })
    }

    pub fn as_str(&self) -> &str {
        self.value.as_str()
    }
}

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReticulumDestinationV1 {
    canonical: String,
}

impl ReticulumDestinationV1 {
    pub fn parse(value: impl Into<String>) -> Result<Self, ProtocolContractErrorV1> {
        let value = value.into();
        if value.is_empty()
            || value != value.trim()
            || value
                .chars()
                .any(|ch| ch.is_ascii_control() || ch.is_ascii_whitespace())
        {
            return Err(ProtocolContractErrorV1::InvalidReticulumDestination);
        }
        Ok(Self { canonical: value })
    }

    pub fn as_str(&self) -> &str {
        self.canonical.as_str()
    }
}

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReticulumTargetV1 {
    pub destination: ReticulumDestinationV1,
    pub mesh_scope: Option<MeshScopeIdV1>,
}

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TransportCapabilityDescriptorV1 {
    pub kind: TransportKindV1,
    pub maturity: CapabilityMaturityV1,
    pub availability: CapabilityAvailabilityV1,
    pub can_deliver: bool,
    pub can_fetch: bool,
    pub can_discover: bool,
    pub can_gateway_forward: bool,
    pub can_observe_receipts: bool,
    pub required_for_v1: bool,
}

pub const TRANSPORT_CAPABILITY_CATALOG_V1: &[TransportCapabilityDescriptorV1] = &[
    TransportCapabilityDescriptorV1 {
        kind: TransportKindV1::Local,
        maturity: CapabilityMaturityV1::Stable,
        availability: CapabilityAvailabilityV1::Available,
        can_deliver: true,
        can_fetch: true,
        can_discover: false,
        can_gateway_forward: false,
        can_observe_receipts: true,
        required_for_v1: true,
    },
    TransportCapabilityDescriptorV1 {
        kind: TransportKindV1::Nostr,
        maturity: CapabilityMaturityV1::Stable,
        availability: CapabilityAvailabilityV1::Available,
        can_deliver: true,
        can_fetch: true,
        can_discover: true,
        can_gateway_forward: false,
        can_observe_receipts: true,
        required_for_v1: true,
    },
    TransportCapabilityDescriptorV1 {
        kind: TransportKindV1::Reticulum,
        maturity: CapabilityMaturityV1::Preview,
        availability: CapabilityAvailabilityV1::Unavailable,
        can_deliver: true,
        can_fetch: false,
        can_discover: true,
        can_gateway_forward: true,
        can_observe_receipts: true,
        required_for_v1: true,
    },
];

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "dto-bindgen", dto(as = "string_enum"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProtocolEventClassV1 {
    #[cfg_attr(feature = "dto-bindgen", dto(rename = "regular"))]
    Regular,
    #[cfg_attr(feature = "dto-bindgen", dto(rename = "replaceable"))]
    Replaceable,
    #[cfg_attr(feature = "dto-bindgen", dto(rename = "addressable"))]
    Addressable,
    #[cfg_attr(feature = "dto-bindgen", dto(rename = "unsigned_rumor"))]
    UnsignedRumor,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProtocolEventDescriptorV1 {
    pub name: &'static str,
    pub kind: u32,
    pub event_class: ProtocolEventClassV1,
    pub purpose: &'static str,
}

pub const PROTOCOL_EVENT_CATALOG_V1: &[ProtocolEventDescriptorV1] = &[
    ProtocolEventDescriptorV1 {
        name: "profile",
        kind: 0,
        event_class: ProtocolEventClassV1::Replaceable,
        purpose: "actor public profile/supporting discovery",
    },
    ProtocolEventDescriptorV1 {
        name: "deletion_request",
        kind: 5,
        event_class: ProtocolEventClassV1::Regular,
        purpose: "best-effort NIP-09 request; no global erasure guarantee",
    },
    ProtocolEventDescriptorV1 {
        name: "gift_wrap",
        kind: 1059,
        event_class: ProtocolEventClassV1::Regular,
        purpose: "NIP-59 encrypted private delivery wrapper",
    },
    ProtocolEventDescriptorV1 {
        name: "trade_private_coordination_rumor",
        kind: 3421,
        event_class: ProtocolEventClassV1::UnsignedRumor,
        purpose: "NIP-44 encrypted buyer/seller private coordination; never relay-published directly",
    },
    ProtocolEventDescriptorV1 {
        name: "trade_order_request",
        kind: 3422,
        event_class: ProtocolEventClassV1::Regular,
        purpose: "buyer request against exact listing/quote/validator set",
    },
    ProtocolEventDescriptorV1 {
        name: "trade_order_decision",
        kind: 3423,
        event_class: ProtocolEventClassV1::Regular,
        purpose: "seller accept or decline",
    },
    ProtocolEventDescriptorV1 {
        name: "trade_order_cancellation",
        kind: 3432,
        event_class: ProtocolEventClassV1::Regular,
        purpose: "authorized predecision cancellation",
    },
    ProtocolEventDescriptorV1 {
        name: "trade_validation_receipt",
        kind: 3440,
        event_class: ProtocolEventClassV1::Regular,
        purpose: "RHI validation result bound to root/target/listing/validator set",
    },
    ProtocolEventDescriptorV1 {
        name: "dm_relay_list",
        kind: 10050,
        event_class: ProtocolEventClassV1::Replaceable,
        purpose: "recipient private-message relay advertisement",
    },
    ProtocolEventDescriptorV1 {
        name: "relay_auth",
        kind: 22242,
        event_class: ProtocolEventClassV1::Regular,
        purpose: "NIP-42 relay authentication",
    },
    ProtocolEventDescriptorV1 {
        name: "farm",
        kind: 30340,
        event_class: ProtocolEventClassV1::Addressable,
        purpose: "public farm aggregate",
    },
    ProtocolEventDescriptorV1 {
        name: "validator_set",
        kind: 30381,
        event_class: ProtocolEventClassV1::Addressable,
        purpose: "immutable one-validator set artifact signed by network authority",
    },
    ProtocolEventDescriptorV1 {
        name: "listing",
        kind: 30402,
        event_class: ProtocolEventClassV1::Addressable,
        purpose: "public listing aggregate and revision",
    },
];

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProtocolSchemaMetadataV1 {
    pub type_name: &'static str,
    pub schema_id: &'static str,
    pub schema_version: u16,
}

pub const PROTOCOL_SCHEMA_METADATA_V1: &[ProtocolSchemaMetadataV1] = &[
    ProtocolSchemaMetadataV1 {
        type_name: "TransportKindV1",
        schema_id: "radroots.protocol.transport_kind.v1",
        schema_version: 1,
    },
    ProtocolSchemaMetadataV1 {
        type_name: "TransportCapabilityDescriptorV1",
        schema_id: "radroots.protocol.transport_capability_descriptor.v1",
        schema_version: 1,
    },
    ProtocolSchemaMetadataV1 {
        type_name: "ProtocolEventDescriptorV1",
        schema_id: "radroots.protocol.event_descriptor.v1",
        schema_version: 1,
    },
    ProtocolSchemaMetadataV1 {
        type_name: "ReticulumTargetV1",
        schema_id: "radroots.protocol.reticulum_target.v1",
        schema_version: 1,
    },
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProtocolContractErrorV1 {
    DuplicateTransportKind { kind: TransportKindV1 },
    DuplicateEventName { name: String },
    DuplicateEventKind { kind: u32 },
    DuplicateSchemaId { schema_id: String },
    MissingRequiredTransport { kind: TransportKindV1 },
    RetiredTransportIdentity { identity: String },
    UnknownTransportKind { value: String },
    InvalidMeshScopeId,
    InvalidReticulumDestination,
}

impl core::fmt::Display for ProtocolContractErrorV1 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::DuplicateTransportKind { kind } => {
                write!(f, "duplicate transport kind {}", kind.as_str())
            }
            Self::DuplicateEventName { name } => write!(f, "duplicate event name {name}"),
            Self::DuplicateEventKind { kind } => write!(f, "duplicate event kind {kind}"),
            Self::DuplicateSchemaId { schema_id } => write!(f, "duplicate schema id {schema_id}"),
            Self::MissingRequiredTransport { kind } => {
                write!(f, "missing required transport {}", kind.as_str())
            }
            Self::RetiredTransportIdentity { identity } => {
                write!(f, "retired transport identity {identity}")
            }
            Self::UnknownTransportKind { value } => write!(f, "unknown transport kind {value}"),
            Self::InvalidMeshScopeId => f.write_str("invalid mesh scope id"),
            Self::InvalidReticulumDestination => f.write_str("invalid Reticulum destination"),
        }
    }
}

pub fn validate_protocol_contract_v1() -> Result<(), ProtocolContractErrorV1> {
    validate_transport_capability_catalog(TRANSPORT_CAPABILITY_CATALOG_V1)?;
    validate_event_catalog(PROTOCOL_EVENT_CATALOG_V1)?;
    validate_schema_metadata(PROTOCOL_SCHEMA_METADATA_V1)
}

fn validate_transport_capability_catalog(
    descriptors: &[TransportCapabilityDescriptorV1],
) -> Result<(), ProtocolContractErrorV1> {
    let mut kinds = BTreeSet::new();
    for descriptor in descriptors {
        if !kinds.insert(descriptor.kind) {
            return Err(ProtocolContractErrorV1::DuplicateTransportKind {
                kind: descriptor.kind,
            });
        }
    }
    for required in [
        TransportKindV1::Local,
        TransportKindV1::Nostr,
        TransportKindV1::Reticulum,
    ] {
        if !kinds.contains(&required) {
            return Err(ProtocolContractErrorV1::MissingRequiredTransport { kind: required });
        }
    }
    Ok(())
}

fn validate_event_catalog(
    descriptors: &[ProtocolEventDescriptorV1],
) -> Result<(), ProtocolContractErrorV1> {
    let mut names = BTreeSet::new();
    let mut kinds = BTreeSet::new();
    for descriptor in descriptors {
        if !names.insert(descriptor.name) {
            return Err(ProtocolContractErrorV1::DuplicateEventName {
                name: descriptor.name.to_string(),
            });
        }
        if !kinds.insert(descriptor.kind) {
            return Err(ProtocolContractErrorV1::DuplicateEventKind {
                kind: descriptor.kind,
            });
        }
    }
    Ok(())
}

fn validate_schema_metadata(
    descriptors: &[ProtocolSchemaMetadataV1],
) -> Result<(), ProtocolContractErrorV1> {
    let mut schema_ids = BTreeSet::new();
    for descriptor in descriptors {
        if !schema_ids.insert(descriptor.schema_id) {
            return Err(ProtocolContractErrorV1::DuplicateSchemaId {
                schema_id: descriptor.schema_id.to_string(),
            });
        }
    }
    Ok(())
}

#[cfg(feature = "dto-bindgen")]
pub fn dto_roots() -> alloc::vec::Vec<dto_bindgen::export::RootDescriptor> {
    use dto_bindgen::export::RootDescriptor;
    alloc::vec![
        RootDescriptor::new::<TransportKindV1>(),
        RootDescriptor::new::<CapabilityMaturityV1>(),
        RootDescriptor::new::<CapabilityAvailabilityV1>(),
        RootDescriptor::new::<MeshScopeIdV1>(),
        RootDescriptor::new::<ReticulumDestinationV1>(),
        RootDescriptor::new::<ReticulumTargetV1>(),
        RootDescriptor::new::<TransportCapabilityDescriptorV1>(),
        RootDescriptor::new::<ProtocolEventClassV1>(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_contract_catalogs_validate() {
        validate_protocol_contract_v1().expect("protocol contract validates");
    }

    #[test]
    fn transport_kind_v1_rejects_retired_identities() {
        for identity in [
            "reticulum_preview",
            "mesh",
            "proxy",
            "radrootsd_proxy",
            "hybrid",
        ] {
            assert!(matches!(
                TransportKindV1::parse(identity),
                Err(ProtocolContractErrorV1::RetiredTransportIdentity { .. })
            ));
        }
    }

    #[test]
    fn reticulum_is_required_preview_v1_transport() {
        let reticulum = TRANSPORT_CAPABILITY_CATALOG_V1
            .iter()
            .find(|descriptor| descriptor.kind == TransportKindV1::Reticulum)
            .expect("reticulum descriptor");

        assert!(reticulum.required_for_v1);
        assert_eq!(reticulum.maturity, CapabilityMaturityV1::Preview);
        assert_eq!(
            reticulum.availability,
            CapabilityAvailabilityV1::Unavailable
        );
        assert!(reticulum.can_deliver);
        assert!(!reticulum.can_fetch);
    }

    #[test]
    fn reticulum_target_newtypes_validate() {
        let target = ReticulumTargetV1 {
            destination: ReticulumDestinationV1::parse("reticulum:local").expect("destination"),
            mesh_scope: Some(MeshScopeIdV1::parse("local_preview").expect("scope")),
        };

        assert_eq!(target.destination.as_str(), "reticulum:local");
        assert_eq!(
            target.mesh_scope.as_ref().expect("scope").as_str(),
            "local_preview"
        );
    }
}
