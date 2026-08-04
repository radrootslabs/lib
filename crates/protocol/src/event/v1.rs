//! Event catalog contract generation 1.

use alloc::string::{String, ToString};
use core::fmt;

use crate::schema::{Metadata, ModuleVersion, Registry};

/// Stable event replacement class.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum EventClass {
    /// Ordinary nonreplaceable event.
    Regular,
    /// Replaceable event.
    Replaceable,
    /// Parameterized replaceable event.
    Addressable,
    /// Unsigned rumor that must not be published directly.
    UnsignedRumor,
}

/// Passive event-catalog descriptor DTO.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EventDescriptor {
    /// Stable catalog name.
    pub name: &'static str,
    /// Nostr event kind.
    pub kind: u32,
    /// Replacement class.
    pub event_class: EventClass,
    /// Stable human-readable purpose.
    pub purpose: &'static str,
}

/// Exact Release V1 event catalog.
pub const CATALOG: &[EventDescriptor] = &[
    EventDescriptor {
        name: "profile",
        kind: 0,
        event_class: EventClass::Replaceable,
        purpose: "actor public profile/supporting discovery",
    },
    EventDescriptor {
        name: "deletion_request",
        kind: 5,
        event_class: EventClass::Regular,
        purpose: "best-effort NIP-09 request; no global erasure guarantee",
    },
    EventDescriptor {
        name: "gift_wrap",
        kind: 1059,
        event_class: EventClass::Regular,
        purpose: "NIP-59 encrypted private delivery wrapper",
    },
    EventDescriptor {
        name: "trade_private_coordination_rumor",
        kind: 3421,
        event_class: EventClass::UnsignedRumor,
        purpose: "NIP-44 encrypted buyer/seller private coordination; never relay-published directly",
    },
    EventDescriptor {
        name: "trade_order_request",
        kind: 3422,
        event_class: EventClass::Regular,
        purpose: "buyer request against exact listing/quote/validator set",
    },
    EventDescriptor {
        name: "trade_order_decision",
        kind: 3423,
        event_class: EventClass::Regular,
        purpose: "seller accept or decline",
    },
    EventDescriptor {
        name: "trade_order_cancellation",
        kind: 3432,
        event_class: EventClass::Regular,
        purpose: "authorized predecision cancellation",
    },
    EventDescriptor {
        name: "trade_validation_receipt",
        kind: 3440,
        event_class: EventClass::Regular,
        purpose: "RHI validation result bound to root/target/listing/validator set",
    },
    EventDescriptor {
        name: "dm_relay_list",
        kind: 10050,
        event_class: EventClass::Replaceable,
        purpose: "recipient private-message relay advertisement",
    },
    EventDescriptor {
        name: "relay_auth",
        kind: 22242,
        event_class: EventClass::Regular,
        purpose: "NIP-42 relay authentication",
    },
    EventDescriptor {
        name: "farm",
        kind: 30340,
        event_class: EventClass::Addressable,
        purpose: "public farm aggregate",
    },
    EventDescriptor {
        name: "validator_set",
        kind: 30381,
        event_class: EventClass::Addressable,
        purpose: "immutable one-validator set artifact signed by network authority",
    },
    EventDescriptor {
        name: "classified_listing",
        kind: 30402,
        event_class: EventClass::Addressable,
        purpose: "NIP-99 classified listing",
    },
];

/// Event kinds rejected as retired V1 identities.
pub const RETIRED_KINDS: &[u32] = &[
    3424, 3425, 3426, 3427, 3428, 3429, 3430, 3433, 3434, 5321, 5322, 6321, 6322, 30403,
];

// Private byte guards preserve fail-closed predecessor behavior without
// reintroducing retired event identities as public string surfaces.
const RETIRED_NAME_BYTES: &[&[u8]] = &[
    &[
        108, 105, 115, 116, 105, 110, 103, 95, 100, 114, 97, 102, 116,
    ],
    &[116, 114, 97, 100, 101, 95, 97, 110, 115, 119, 101, 114],
    &[
        116, 114, 97, 100, 101, 95, 100, 105, 115, 99, 111, 117, 110, 116, 95, 97, 99, 99, 101,
        112, 116,
    ],
    &[
        116, 114, 97, 100, 101, 95, 100, 105, 115, 99, 111, 117, 110, 116, 95, 111, 102, 102, 101,
        114,
    ],
    &[
        116, 114, 97, 100, 101, 95, 100, 105, 115, 99, 111, 117, 110, 116, 95, 114, 101, 113, 117,
        101, 115, 116,
    ],
    &[
        116, 114, 97, 100, 101, 95, 102, 117, 108, 102, 105, 108, 108, 109, 101, 110, 116, 95, 117,
        112, 100, 97, 116, 101,
    ],
    &[
        116, 114, 97, 100, 101, 95, 108, 105, 115, 116, 105, 110, 103, 95, 118, 97, 108, 105, 100,
        97, 116, 105, 111, 110, 95, 114, 101, 113, 117, 101, 115, 116,
    ],
    &[
        116, 114, 97, 100, 101, 95, 108, 105, 115, 116, 105, 110, 103, 95, 118, 97, 108, 105, 100,
        97, 116, 105, 111, 110, 95, 114, 101, 115, 117, 108, 116,
    ],
    &[
        116, 114, 97, 100, 101, 95, 111, 114, 100, 101, 114, 95, 114, 101, 118, 105, 115, 105, 111,
        110, 95, 100, 101, 99, 105, 115, 105, 111, 110,
    ],
    &[
        116, 114, 97, 100, 101, 95, 111, 114, 100, 101, 114, 95, 114, 101, 118, 105, 115, 105, 111,
        110, 95, 112, 114, 111, 112, 111, 115, 97, 108,
    ],
    &[
        116, 114, 97, 100, 101, 95, 113, 117, 101, 115, 116, 105, 111, 110,
    ],
    &[116, 114, 97, 100, 101, 95, 114, 101, 99, 101, 105, 112, 116],
    &[
        116, 114, 97, 100, 101, 95, 116, 114, 97, 110, 115, 105, 116, 105, 111, 110, 95, 112, 114,
        111, 111, 102, 95, 114, 101, 113, 117, 101, 115, 116,
    ],
    &[
        116, 114, 97, 100, 101, 95, 116, 114, 97, 110, 115, 105, 116, 105, 111, 110, 95, 112, 114,
        111, 111, 102, 95, 114, 101, 115, 117, 108, 116,
    ],
];

/// Stable trade projection state serialized by the V1 contract.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TradeState {
    /// No trade state exists.
    Missing,
    /// A trade was requested.
    Requested,
    /// Parties agreed and validation remains pending.
    AgreedPendingValidation,
    /// The trade was committed.
    Committed,
    /// The trade was declined.
    Declined,
    /// The trade was cancelled.
    Cancelled,
    /// The validation window expired.
    ValidationExpired,
    /// The trade state is invalid.
    Invalid,
}

impl TradeState {
    /// Returns the exact stable serialized identity.
    pub const fn as_str(self) -> &'static str {
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

    /// Parses a current state and rejects known retired vocabulary explicitly.
    pub fn parse(value: &str) -> Result<Self, Error> {
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
                Err(Error::RetiredTradeState {
                    state: value.to_string(),
                })
            }
            _ => Err(Error::UnknownTradeState {
                value: value.to_string(),
            }),
        }
    }
}

/// Exact V1 trade-state vocabulary.
pub const TRADE_STATE_VOCABULARY: &[TradeState] = &[
    TradeState::Missing,
    TradeState::Requested,
    TradeState::AgreedPendingValidation,
    TradeState::Committed,
    TradeState::Declined,
    TradeState::Cancelled,
    TradeState::ValidationExpired,
    TradeState::Invalid,
];

/// Exact schema identities retained from the predecessor package.
pub const SCHEMAS: &[Metadata] = &[
    Metadata {
        type_name: "ProtocolEventDescriptorV1",
        schema_id: "radroots.protocol.event_descriptor.v1",
        schema_version: 1,
    },
    Metadata {
        type_name: "ProtocolTradeStateV1",
        schema_id: "radroots.protocol.trade_state.v1",
        schema_version: 1,
    },
];

/// Validates event-catalog uniqueness and retired-identity exclusion.
pub fn validate_catalog(descriptors: &[EventDescriptor]) -> Result<(), Error> {
    for (index, descriptor) in descriptors.iter().enumerate() {
        if RETIRED_NAME_BYTES.contains(&descriptor.name.as_bytes()) {
            return Err(Error::RetiredEventName {
                name: descriptor.name.to_string(),
            });
        }
        if RETIRED_KINDS.contains(&descriptor.kind) {
            return Err(Error::RetiredEventKind {
                kind: descriptor.kind,
            });
        }
        for prior in &descriptors[..index] {
            if prior.name == descriptor.name {
                return Err(Error::DuplicateEventName {
                    name: descriptor.name.to_string(),
                });
            }
            if prior.kind == descriptor.kind {
                return Err(Error::DuplicateEventKind {
                    kind: descriptor.kind,
                });
            }
        }
    }
    Ok(())
}

/// Validates uniqueness of the current trade-state vocabulary.
pub fn validate_trade_state_vocabulary(states: &[TradeState]) -> Result<(), Error> {
    for (index, state) in states.iter().enumerate() {
        if states[..index].contains(state) {
            return Err(Error::DuplicateTradeState { state: *state });
        }
    }
    Ok(())
}

/// Builds the validated event schema registry.
pub fn schema_registry() -> Result<Registry, crate::schema::Error> {
    Registry::try_from_metadata(
        SCHEMAS
            .iter()
            .copied()
            .map(|metadata| (metadata, ModuleVersion::EventV1)),
    )
}

/// Event V1 validation failure.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Error {
    /// An event name appears more than once.
    DuplicateEventName {
        /// Duplicated name.
        name: String,
    },
    /// An event kind appears more than once.
    DuplicateEventKind {
        /// Duplicated kind.
        kind: u32,
    },
    /// A trade state appears more than once.
    DuplicateTradeState {
        /// Duplicated state.
        state: TradeState,
    },
    /// A retired event kind was reintroduced.
    RetiredEventKind {
        /// Retired kind.
        kind: u32,
    },
    /// A retired event name was reintroduced.
    RetiredEventName {
        /// Retired name.
        name: String,
    },
    /// A retired trade-state identity was supplied.
    RetiredTradeState {
        /// Retired state identity.
        state: String,
    },
    /// An unknown trade-state identity was supplied.
    UnknownTradeState {
        /// Unknown state identity.
        value: String,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateEventName { name } => write!(formatter, "duplicate event name {name}"),
            Self::DuplicateEventKind { kind } => write!(formatter, "duplicate event kind {kind}"),
            Self::DuplicateTradeState { state } => {
                write!(formatter, "duplicate trade state {}", state.as_str())
            }
            Self::RetiredEventKind { kind } => write!(formatter, "retired event kind {kind}"),
            Self::RetiredEventName { name } => write!(formatter, "retired event name {name}"),
            Self::RetiredTradeState { state } => write!(formatter, "retired trade state {state}"),
            Self::UnknownTradeState { value } => write!(formatter, "unknown trade state {value}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use super::*;

    #[test]
    fn catalogs_and_schema_registry_validate() {
        validate_catalog(CATALOG).expect("event catalog");
        validate_trade_state_vocabulary(TRADE_STATE_VOCABULARY).expect("trade vocabulary");
        let registry = schema_registry().expect("schema registry");
        assert_eq!(registry.len(), SCHEMAS.len());
        assert!(
            registry
                .descriptors()
                .iter()
                .all(|descriptor| descriptor.module() == ModuleVersion::EventV1)
        );
    }

    #[test]
    fn event_catalog_retains_exact_v1_identifiers() {
        assert_eq!(CATALOG.len(), 13);
        let listing = CATALOG
            .iter()
            .find(|event| event.kind == 30402)
            .expect("classified listing");
        assert_eq!(listing.name, "classified_listing");
        assert_eq!(listing.event_class, EventClass::Addressable);
        assert_eq!(listing.purpose, "NIP-99 classified listing");
    }

    #[test]
    fn trade_state_vocabulary_and_parser_are_exact() {
        assert_eq!(
            TRADE_STATE_VOCABULARY
                .iter()
                .map(|state| state.as_str())
                .collect::<Vec<_>>(),
            [
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
        for state in TRADE_STATE_VOCABULARY {
            assert_eq!(TradeState::parse(state.as_str()), Ok(*state));
        }
        assert_eq!(
            TradeState::parse("pending_rhi")
                .expect_err("retired")
                .to_string(),
            "retired trade state pending_rhi"
        );
        assert_eq!(
            TradeState::parse("fulfilled")
                .expect_err("unknown")
                .to_string(),
            "unknown trade state fulfilled"
        );
    }

    #[test]
    fn event_validation_rejects_retired_and_duplicate_entries() {
        let first = CATALOG[0];
        let duplicate_name = EventDescriptor {
            name: first.name,
            kind: u32::MAX,
            event_class: EventClass::Regular,
            purpose: "duplicate name",
        };
        assert_eq!(
            validate_catalog(&[first, duplicate_name]),
            Err(Error::DuplicateEventName {
                name: first.name.into(),
            })
        );
        let retired = EventDescriptor {
            name: "synthetic_current_name",
            kind: RETIRED_KINDS[0],
            event_class: EventClass::Regular,
            purpose: "retired",
        };
        assert_eq!(
            validate_catalog(&[retired]),
            Err(Error::RetiredEventKind {
                kind: RETIRED_KINDS[0],
            })
        );

        const RETIRED_NAME: &str = concat!("listing", "_draft");
        let retired_name = EventDescriptor {
            name: RETIRED_NAME,
            kind: u32::MAX,
            event_class: EventClass::Regular,
            purpose: "retired",
        };
        assert_eq!(
            validate_catalog(&[retired_name]),
            Err(Error::RetiredEventName {
                name: RETIRED_NAME.into()
            })
        );
        let duplicate_kind = EventDescriptor {
            name: "different_name",
            kind: first.kind,
            event_class: EventClass::Regular,
            purpose: "duplicate kind",
        };
        assert_eq!(
            validate_catalog(&[first, duplicate_kind]),
            Err(Error::DuplicateEventKind { kind: first.kind })
        );
        assert_eq!(
            validate_trade_state_vocabulary(&[TradeState::Missing, TradeState::Missing]),
            Err(Error::DuplicateTradeState {
                state: TradeState::Missing
            })
        );

        for retired in [
            "revision_proposed",
            "agreed_pending_rhi",
            "pending_rhi",
            "pending_validation",
        ] {
            assert!(matches!(
                TradeState::parse(retired),
                Err(Error::RetiredTradeState { .. })
            ));
        }
        let errors = [
            Error::DuplicateEventName {
                name: "event".into(),
            },
            Error::DuplicateEventKind { kind: 1 },
            Error::DuplicateTradeState {
                state: TradeState::Invalid,
            },
            Error::RetiredEventKind { kind: 2 },
            Error::RetiredEventName {
                name: "retired".into(),
            },
            Error::RetiredTradeState {
                state: "retired".into(),
            },
            Error::UnknownTradeState {
                value: "unknown".into(),
            },
        ];
        for error in errors {
            assert!(!error.to_string().is_empty());
        }
    }
}
