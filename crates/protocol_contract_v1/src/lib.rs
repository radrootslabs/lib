#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

use alloc::collections::BTreeSet;
use alloc::string::{String, ToString};

pub const PROTOCOL_CONTRACT_NAME_V1: &str = "radroots.protocol";
pub const PROTOCOL_CONTRACT_VERSION_V1: u16 = 1;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TransportKindV1 {
    Local,
    Nostr,
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
            _ => Err(ProtocolContractErrorV1::UnknownTransportKind {
                value: value.to_string(),
            }),
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CapabilityMaturityV1 {
    Preview,
    Stable,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CapabilityAvailabilityV1 {
    Available,
    Degraded,
    Unavailable,
}

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

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReticulumTargetV1 {
    pub destination: ReticulumDestinationV1,
    pub mesh_scope: Option<MeshScopeIdV1>,
}

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

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProtocolEventClassV1 {
    Regular,
    Replaceable,
    Addressable,
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
        name: "classified_listing",
        kind: 30402,
        event_class: ProtocolEventClassV1::Addressable,
        purpose: "NIP-99 classified listing",
    },
];

pub const RETIRED_PROTOCOL_EVENT_KINDS_V1: &[u32] = &[
    3424, 3425, 3426, 3427, 3428, 3429, 3430, 3433, 3434, 5321, 5322, 6321, 6322, 30403,
];

pub const RETIRED_PROTOCOL_EVENT_NAMES_V1: &[&str] = &[
    "listing_draft",
    "trade_answer",
    "trade_discount_accept",
    "trade_discount_offer",
    "trade_discount_request",
    "trade_fulfillment_update",
    "trade_listing_validation_request",
    "trade_listing_validation_result",
    "trade_order_revision_decision",
    "trade_order_revision_proposal",
    "trade_question",
    "trade_receipt",
    "trade_transition_proof_request",
    "trade_transition_proof_result",
];

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProtocolTradeStateV1 {
    Missing,
    Requested,
    AgreedPendingValidation,
    Committed,
    Declined,
    Cancelled,
    ValidationExpired,
    Invalid,
}

impl ProtocolTradeStateV1 {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Requested => "requested",
            Self::AgreedPendingValidation => "agreed_pending_validation",
            Self::Committed => "committed",
            Self::Declined => "declined",
            Self::Cancelled => "cancelled",
            Self::ValidationExpired => "validation_expired",
            Self::Invalid => "invalid",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ProtocolContractErrorV1> {
        match value {
            "missing" => Ok(Self::Missing),
            "requested" => Ok(Self::Requested),
            "agreed_pending_validation" => Ok(Self::AgreedPendingValidation),
            "committed" => Ok(Self::Committed),
            "declined" => Ok(Self::Declined),
            "cancelled" => Ok(Self::Cancelled),
            "validation_expired" => Ok(Self::ValidationExpired),
            "invalid" => Ok(Self::Invalid),
            "revision_proposed" | "agreed_pending_rhi" | "pending_rhi" | "pending_validation" => {
                Err(ProtocolContractErrorV1::RetiredTradeState {
                    state: value.to_string(),
                })
            }
            _ => Err(ProtocolContractErrorV1::UnknownTradeState {
                value: value.to_string(),
            }),
        }
    }
}

pub const PROTOCOL_TRADE_STATE_VOCABULARY_V1: &[ProtocolTradeStateV1] = &[
    ProtocolTradeStateV1::Missing,
    ProtocolTradeStateV1::Requested,
    ProtocolTradeStateV1::AgreedPendingValidation,
    ProtocolTradeStateV1::Committed,
    ProtocolTradeStateV1::Declined,
    ProtocolTradeStateV1::Cancelled,
    ProtocolTradeStateV1::ValidationExpired,
    ProtocolTradeStateV1::Invalid,
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
        type_name: "ProtocolTradeStateV1",
        schema_id: "radroots.protocol.trade_state.v1",
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
    DuplicateTradeState { state: ProtocolTradeStateV1 },
    MissingRequiredTransport { kind: TransportKindV1 },
    RetiredEventKind { kind: u32 },
    RetiredEventName { name: String },
    RetiredTradeState { state: String },
    UnknownTradeState { value: String },
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
            Self::DuplicateTradeState { state } => {
                write!(f, "duplicate trade state {}", state.as_str())
            }
            Self::MissingRequiredTransport { kind } => {
                write!(f, "missing required transport {}", kind.as_str())
            }
            Self::RetiredEventKind { kind } => write!(f, "retired event kind {kind}"),
            Self::RetiredEventName { name } => write!(f, "retired event name {name}"),
            Self::RetiredTradeState { state } => write!(f, "retired trade state {state}"),
            Self::UnknownTradeState { value } => write!(f, "unknown trade state {value}"),
            Self::UnknownTransportKind { value } => write!(f, "unknown transport kind {value}"),
            Self::InvalidMeshScopeId => f.write_str("invalid mesh scope id"),
            Self::InvalidReticulumDestination => f.write_str("invalid Reticulum destination"),
        }
    }
}

pub fn validate_protocol_contract_v1() -> Result<(), ProtocolContractErrorV1> {
    validate_transport_capability_catalog(TRANSPORT_CAPABILITY_CATALOG_V1)?;
    validate_event_catalog(PROTOCOL_EVENT_CATALOG_V1)?;
    validate_trade_state_vocabulary(PROTOCOL_TRADE_STATE_VOCABULARY_V1)?;
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
        if RETIRED_PROTOCOL_EVENT_NAMES_V1.contains(&descriptor.name) {
            return Err(ProtocolContractErrorV1::RetiredEventName {
                name: descriptor.name.to_string(),
            });
        }
        if RETIRED_PROTOCOL_EVENT_KINDS_V1.contains(&descriptor.kind) {
            return Err(ProtocolContractErrorV1::RetiredEventKind {
                kind: descriptor.kind,
            });
        }
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

fn validate_trade_state_vocabulary(
    states: &[ProtocolTradeStateV1],
) -> Result<(), ProtocolContractErrorV1> {
    let mut seen = BTreeSet::new();
    for state in states {
        if !seen.insert(*state) {
            return Err(ProtocolContractErrorV1::DuplicateTradeState { state: *state });
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_contract_catalogs_validate() {
        validate_protocol_contract_v1().expect("protocol contract validates");
    }

    #[test]
    fn transport_kind_v1_parses_current_and_unknown_identities() {
        for (raw, expected) in [
            ("local", TransportKindV1::Local),
            ("nostr", TransportKindV1::Nostr),
            ("reticulum", TransportKindV1::Reticulum),
        ] {
            let parsed = TransportKindV1::parse(raw).expect("current transport parses");
            assert_eq!(parsed, expected);
            assert_eq!(parsed.as_str(), raw);
        }

        assert_eq!(
            TransportKindV1::parse("unknown_transport")
                .expect_err("unknown transport")
                .to_string(),
            "unknown transport kind unknown_transport"
        );
    }

    #[test]
    fn transport_kind_v1_rejects_unrecognized_identities() {
        for identity in [
            concat!("reticulum", "_preview"),
            "mesh",
            concat!("pro", "xy"),
            concat!("radrootsd", "_", "pro", "xy"),
            concat!("hy", "brid"),
        ] {
            assert!(matches!(
                TransportKindV1::parse(identity),
                Err(ProtocolContractErrorV1::UnknownTransportKind { .. })
            ));
            assert_eq!(
                TransportKindV1::parse(identity)
                    .expect_err("unknown transport")
                    .to_string(),
                alloc::format!("unknown transport kind {identity}")
            );
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

    #[test]
    fn reticulum_target_newtypes_reject_invalid_values() {
        for value in ["", " local", "local ", "local/scope"] {
            assert_eq!(
                MeshScopeIdV1::parse(value)
                    .expect_err("invalid mesh scope")
                    .to_string(),
                "invalid mesh scope id"
            );
        }

        for value in [
            "",
            " reticulum:local",
            "reticulum:local ",
            "reticulum:\nlocal",
        ] {
            assert_eq!(
                ReticulumDestinationV1::parse(value)
                    .expect_err("invalid destination")
                    .to_string(),
                "invalid Reticulum destination"
            );
        }
    }

    #[test]
    fn trade_state_v1_vocabulary_is_exact() {
        let states = PROTOCOL_TRADE_STATE_VOCABULARY_V1
            .iter()
            .map(|state| state.as_str())
            .collect::<alloc::vec::Vec<_>>();
        assert_eq!(
            states,
            alloc::vec![
                "missing",
                "requested",
                "agreed_pending_validation",
                "committed",
                "declined",
                "cancelled",
                "validation_expired",
                "invalid",
            ]
        );

        for state in PROTOCOL_TRADE_STATE_VOCABULARY_V1 {
            assert_eq!(
                ProtocolTradeStateV1::parse(state.as_str()).expect("state parses"),
                *state
            );
        }
    }

    #[test]
    fn trade_state_v1_rejects_retired_and_unknown_states() {
        for state in [
            "revision_proposed",
            "agreed_pending_rhi",
            "pending_rhi",
            "pending_validation",
        ] {
            assert_eq!(
                ProtocolTradeStateV1::parse(state)
                    .expect_err("retired state")
                    .to_string(),
                alloc::format!("retired trade state {state}")
            );
        }

        assert_eq!(
            ProtocolTradeStateV1::parse("fulfilled")
                .expect_err("unknown state")
                .to_string(),
            "unknown trade state fulfilled"
        );
    }

    #[test]
    fn retired_protocol_event_catalog_entries_fail_closed() {
        for name in RETIRED_PROTOCOL_EVENT_NAMES_V1 {
            let event = ProtocolEventDescriptorV1 {
                name,
                kind: u32::MAX,
                event_class: ProtocolEventClassV1::Regular,
                purpose: "retired",
            };
            assert_eq!(
                validate_event_catalog(&[event])
                    .expect_err("retired event name")
                    .to_string(),
                alloc::format!("retired event name {name}")
            );
            let successor = radroots_protocol::event::v1::EventDescriptor {
                name,
                kind: u32::MAX,
                event_class: radroots_protocol::event::v1::EventClass::Regular,
                purpose: "retired",
            };
            assert_eq!(
                radroots_protocol::event::v1::validate_catalog(&[successor])
                    .expect_err("successor retired event name")
                    .to_string(),
                alloc::format!("retired event name {name}")
            );
        }

        for kind in RETIRED_PROTOCOL_EVENT_KINDS_V1 {
            let event = ProtocolEventDescriptorV1 {
                name: "synthetic_current_name",
                kind: *kind,
                event_class: ProtocolEventClassV1::Regular,
                purpose: "retired",
            };
            assert_eq!(
                validate_event_catalog(&[event])
                    .expect_err("retired event kind")
                    .to_string(),
                alloc::format!("retired event kind {kind}")
            );
            let successor = radroots_protocol::event::v1::EventDescriptor {
                name: "synthetic_current_name",
                kind: *kind,
                event_class: radroots_protocol::event::v1::EventClass::Regular,
                purpose: "retired",
            };
            assert_eq!(
                radroots_protocol::event::v1::validate_catalog(&[successor])
                    .expect_err("successor retired event kind")
                    .to_string(),
                alloc::format!("retired event kind {kind}")
            );
        }
    }

    #[test]
    fn classified_listing_catalog_entry_is_exact_nip_99_contract() {
        let classified_listing = PROTOCOL_EVENT_CATALOG_V1
            .iter()
            .find(|event| event.kind == 30402)
            .expect("kind 30402 catalog entry");

        assert_eq!(
            *classified_listing,
            ProtocolEventDescriptorV1 {
                name: "classified_listing",
                kind: 30402,
                event_class: ProtocolEventClassV1::Addressable,
                purpose: "NIP-99 classified listing",
            }
        );
    }

    #[test]
    fn validation_reports_transport_catalog_errors() {
        let local = TRANSPORT_CAPABILITY_CATALOG_V1[0];
        let nostr = TRANSPORT_CAPABILITY_CATALOG_V1[1];
        let reticulum = TRANSPORT_CAPABILITY_CATALOG_V1[2];

        assert_eq!(
            validate_transport_capability_catalog(&[local, local])
                .expect_err("duplicate kind")
                .to_string(),
            "duplicate transport kind local"
        );
        assert_eq!(
            validate_transport_capability_catalog(&[nostr, reticulum])
                .expect_err("missing required kind")
                .to_string(),
            "missing required transport local"
        );
    }

    #[test]
    fn validation_reports_event_and_schema_catalog_errors() {
        let event = PROTOCOL_EVENT_CATALOG_V1[0];
        let duplicate_name = ProtocolEventDescriptorV1 {
            name: event.name,
            kind: u32::MAX,
            event_class: ProtocolEventClassV1::Regular,
            purpose: "duplicate name",
        };
        let duplicate_kind = ProtocolEventDescriptorV1 {
            name: "duplicate_kind",
            kind: event.kind,
            event_class: ProtocolEventClassV1::Regular,
            purpose: "duplicate kind",
        };
        let schema = PROTOCOL_SCHEMA_METADATA_V1[0];
        let duplicate_schema = ProtocolSchemaMetadataV1 {
            type_name: "Duplicate",
            schema_id: schema.schema_id,
            schema_version: schema.schema_version,
        };

        assert_eq!(
            validate_event_catalog(&[event, duplicate_name])
                .expect_err("duplicate event name")
                .to_string(),
            alloc::format!("duplicate event name {}", event.name)
        );
        assert_eq!(
            validate_event_catalog(&[event, duplicate_kind])
                .expect_err("duplicate event kind")
                .to_string(),
            alloc::format!("duplicate event kind {}", event.kind)
        );
        assert_eq!(
            validate_schema_metadata(&[schema, duplicate_schema])
                .expect_err("duplicate schema id")
                .to_string(),
            alloc::format!("duplicate schema id {}", schema.schema_id)
        );
        assert_eq!(
            validate_trade_state_vocabulary(&[
                ProtocolTradeStateV1::Missing,
                ProtocolTradeStateV1::Missing,
            ])
            .expect_err("duplicate trade state")
            .to_string(),
            "duplicate trade state missing"
        );
    }
}
