#[cfg(not(feature = "std"))]
use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};

use radroots_event::{
    contract::{
        AuthorRole, RadrootsContentSchema, RadrootsEventAuthoringPolicy, RadrootsEventClass,
        RadrootsEventContract, RadrootsEventDiscriminator, RadrootsEventPrivacy,
        RadrootsEventStability, RadrootsKindContract, RadrootsNostrStandard, RadrootsReducer,
        RadrootsTagCardinality, RadrootsTagContract, RadrootsTagSemantic, RadrootsTagValueType,
        all_event_contracts_registry_v7, all_kind_contracts_registry_v7,
    },
    listing::classified::RadrootsClassifiedListingPartition,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const RADROOTS_EVENT_CONTRACT_REGISTRY_V7_INVENTORY_SCHEMA_VERSION: u32 = 1;
pub const RADROOTS_EVENT_CONTRACT_REGISTRY_V7_VERSION: u32 = 7;
pub const RADROOTS_EVENT_CONTRACT_REGISTRY_V7_KIND_COUNT: usize = 93;
pub const RADROOTS_EVENT_CONTRACT_REGISTRY_V7_EVENT_COUNT: usize = 103;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RadrootsEventContractRegistryV7Inventory {
    pub schema_version: u32,
    pub event_contract_registry_version: u32,
    pub kind_contracts: Vec<RadrootsKindContractRegistryV7InventoryEntry>,
    pub event_contracts: Vec<RadrootsEventContractRegistryV7InventoryEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RadrootsKindContractRegistryV7InventoryEntry {
    pub ordinal: usize,
    pub kind: u32,
    pub canonical_constant: String,
    pub name: String,
    pub class: RadrootsEventContractRegistryV7Class,
    pub standard: RadrootsEventContractRegistryV7Standard,
    pub accepted_event_contracts: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RadrootsEventContractRegistryV7InventoryEntry {
    pub ordinal: usize,
    pub contract_id: String,
    pub kind: u32,
    pub name: String,
    pub payload_type: String,
    pub class: RadrootsEventContractRegistryV7Class,
    pub stability: RadrootsEventContractRegistryV7Stability,
    pub privacy: RadrootsEventContractRegistryV7Privacy,
    pub author_role: RadrootsEventContractRegistryV7ActorRole,
    pub content_schema: RadrootsEventContractRegistryV7ContentSchema,
    pub authoring_policy: RadrootsEventContractRegistryV7AuthoringPolicy,
    pub discriminator: RadrootsEventContractRegistryV7Discriminator,
    pub tags: Vec<RadrootsEventContractRegistryV7Tag>,
    pub reducers: Vec<RadrootsEventContractRegistryV7Reducer>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RadrootsEventContractRegistryV7Class {
    Regular,
    Replaceable,
    Addressable,
    Ephemeral,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RadrootsEventContractRegistryV7Standard {
    Nip01,
    Nip09,
    Nip17,
    Nip18,
    Nip22,
    Nip23,
    Nip25,
    Nip28,
    Nip29,
    Nip42,
    Nip51,
    Nip52,
    Nip53,
    Nip54,
    Nip56,
    Nip57,
    Nip78,
    Nip90,
    Nip94,
    Nip98,
    Nip99,
    Radroots,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RadrootsEventContractRegistryV7Stability {
    Stable,
    Experimental,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RadrootsEventContractRegistryV7Privacy {
    Public,
    Encrypted,
    LocalOnly,
    Secret,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RadrootsEventContractRegistryV7ActorRole {
    Any,
    Application,
    Buyer,
    Farmer,
    Member,
    Moderator,
    Relay,
    Seller,
    Service,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RadrootsEventContractRegistryV7ContentSchema {
    Empty,
    JsonObject,
    PlainText,
    Markdown,
    Djot,
    Encrypted,
    BinaryReference,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RadrootsEventContractRegistryV7AuthoringPolicy {
    GenericDraft,
    TypedOnly,
    ReadOnly,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RadrootsEventContractRegistryV7Discriminator {
    KindOnly,
    AdmissionOnly,
    ClassifiedListingPartition {
        value: RadrootsEventContractRegistryV7ClassifiedListingPartition,
    },
    DTagExact {
        value: String,
    },
    DTagPrefix {
        prefix: String,
    },
    DTagSuffix {
        suffix: String,
    },
    TagEquals {
        name: String,
        value: String,
    },
    ContentJsonFieldEquals {
        field: String,
        value: String,
    },
    EnvelopeType {
        value: String,
    },
    Composite {
        parts: Vec<RadrootsEventContractRegistryV7Discriminator>,
    },
}

impl<'de> Deserialize<'de> for RadrootsEventContractRegistryV7Discriminator {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        discriminator_from_json_value(&value).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RadrootsEventContractRegistryV7ClassifiedListingPartition {
    FocusedFoodAvailability,
    OperationalListing,
    GenericNip99,
    Ambiguous,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RadrootsEventContractRegistryV7Tag {
    pub name: String,
    pub cardinality: RadrootsEventContractRegistryV7TagCardinality,
    pub semantic: RadrootsEventContractRegistryV7TagSemantic,
    pub value_type: RadrootsEventContractRegistryV7TagValueType,
    pub relay_indexed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RadrootsEventContractRegistryV7TagCardinality {
    RequiredOne,
    OptionalOne,
    OptionalMany,
    RequiredMany,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RadrootsEventContractRegistryV7TagSemantic {
    AddressableCoordinate,
    CalendarEventAuthor,
    CalendarEventReference,
    CalendarEventRevision,
    CalendarInclusionRequest,
    CalendarEnd,
    CalendarStart,
    Category,
    Citation,
    Contract,
    Counterparty,
    Evidence,
    EventPointer,
    FreeBusy,
    Geohash,
    GroupId,
    Identifier,
    Image,
    Kind,
    ClassifiedListingAddress,
    OperationalListingSnapshot,
    ListDescription,
    Location,
    Nip01Coordinate,
    Participant,
    PreviousEvent,
    Price,
    PublishedAt,
    Relay,
    Reference,
    ReviewTarget,
    RootEvent,
    ServiceInput,
    ServiceOutput,
    Source,
    Status,
    Summary,
    Title,
    Topic,
    TimeZone,
    Url,
    UtcDayCoverage,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RadrootsEventContractRegistryV7TagValueType {
    AddressableCoordinate,
    CalendarDate,
    CalendarEventCoordinate,
    CalendarFreeBusy,
    CalendarRsvpStatus,
    CalendarUid,
    ContractId,
    DTag,
    EventId,
    EventPointer,
    Geohash,
    IanaTimeZoneId,
    Kind,
    Nip01Coordinate,
    PublicKey,
    RelayUrl,
    Sha256,
    Text,
    UnixTimestamp,
    Uri,
    Url,
    UtcDayIndex,
    Uuid,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RadrootsEventContractRegistryV7Reducer {
    CalendarProjection,
    FarmOpsProjection,
    GroupProjection,
    KnowledgeProjection,
    OperationalListingInventoryAccounting,
    OperationalListingProjection,
    MarketProjection,
    OrderProjection,
    ProfileProjection,
    NostrRelayPolicyProjection,
    SocialProjection,
    TradeProjection,
    TradeValidation,
}

pub fn event_contract_registry_v7_inventory() -> RadrootsEventContractRegistryV7Inventory {
    RadrootsEventContractRegistryV7Inventory {
        schema_version: RADROOTS_EVENT_CONTRACT_REGISTRY_V7_INVENTORY_SCHEMA_VERSION,
        event_contract_registry_version: RADROOTS_EVENT_CONTRACT_REGISTRY_V7_VERSION,
        kind_contracts: all_kind_contracts_registry_v7()
            .iter()
            .enumerate()
            .map(|(ordinal, contract)| kind_inventory_entry(ordinal, contract))
            .collect(),
        event_contracts: all_event_contracts_registry_v7()
            .iter()
            .enumerate()
            .map(|(ordinal, contract)| event_inventory_entry(ordinal, contract))
            .collect(),
    }
}

pub fn event_contract_registry_v7_inventory_json() -> Result<String, serde_json::Error> {
    let mut json = serde_json::to_string_pretty(&event_contract_registry_v7_inventory())?;
    json.push('\n');
    Ok(json)
}

pub fn event_contract_registry_v7_inventory_sha256() -> Result<String, serde_json::Error> {
    let json = event_contract_registry_v7_inventory_json()?;
    Ok(hex::encode(Sha256::digest(json.as_bytes())))
}

pub fn parse_event_contract_registry_v7_inventory_json(
    json: &str,
) -> Result<RadrootsEventContractRegistryV7Inventory, serde_json::Error> {
    serde_json::from_str(json)
}

fn kind_inventory_entry(
    ordinal: usize,
    contract: &RadrootsKindContract,
) -> RadrootsKindContractRegistryV7InventoryEntry {
    RadrootsKindContractRegistryV7InventoryEntry {
        ordinal,
        kind: contract.kind,
        canonical_constant: contract.canonical_constant.to_string(),
        name: contract.name.to_string(),
        class: class_inventory(contract.class),
        standard: standard_inventory(contract.standard),
        accepted_event_contracts: contract
            .accepted_event_contracts
            .iter()
            .map(|contract_id| (*contract_id).to_string())
            .collect(),
    }
}

fn event_inventory_entry(
    ordinal: usize,
    contract: &RadrootsEventContract,
) -> RadrootsEventContractRegistryV7InventoryEntry {
    RadrootsEventContractRegistryV7InventoryEntry {
        ordinal,
        contract_id: contract.id.to_string(),
        kind: contract.kind,
        name: contract.name.to_string(),
        payload_type: contract.payload_type.to_string(),
        class: class_inventory(contract.class),
        stability: stability_inventory(contract.stability),
        privacy: privacy_inventory(contract.privacy),
        author_role: author_role_inventory(contract.required_author_role()),
        content_schema: content_schema_inventory(contract.content_schema),
        authoring_policy: authoring_policy_inventory(contract.authoring_policy()),
        discriminator: discriminator_inventory(&contract.discriminator),
        tags: contract.tags.iter().map(tag_inventory).collect(),
        reducers: contract
            .reducers
            .iter()
            .copied()
            .map(reducer_inventory)
            .collect(),
    }
}

fn class_inventory(value: RadrootsEventClass) -> RadrootsEventContractRegistryV7Class {
    match value {
        RadrootsEventClass::Regular => RadrootsEventContractRegistryV7Class::Regular,
        RadrootsEventClass::Replaceable => RadrootsEventContractRegistryV7Class::Replaceable,
        RadrootsEventClass::Addressable => RadrootsEventContractRegistryV7Class::Addressable,
        RadrootsEventClass::Ephemeral => RadrootsEventContractRegistryV7Class::Ephemeral,
    }
}

fn standard_inventory(value: RadrootsNostrStandard) -> RadrootsEventContractRegistryV7Standard {
    match value {
        RadrootsNostrStandard::Nip01 => RadrootsEventContractRegistryV7Standard::Nip01,
        RadrootsNostrStandard::Nip09 => RadrootsEventContractRegistryV7Standard::Nip09,
        RadrootsNostrStandard::Nip17 => RadrootsEventContractRegistryV7Standard::Nip17,
        RadrootsNostrStandard::Nip18 => RadrootsEventContractRegistryV7Standard::Nip18,
        RadrootsNostrStandard::Nip22 => RadrootsEventContractRegistryV7Standard::Nip22,
        RadrootsNostrStandard::Nip23 => RadrootsEventContractRegistryV7Standard::Nip23,
        RadrootsNostrStandard::Nip25 => RadrootsEventContractRegistryV7Standard::Nip25,
        RadrootsNostrStandard::Nip28 => RadrootsEventContractRegistryV7Standard::Nip28,
        RadrootsNostrStandard::Nip29 => RadrootsEventContractRegistryV7Standard::Nip29,
        RadrootsNostrStandard::Nip42 => RadrootsEventContractRegistryV7Standard::Nip42,
        RadrootsNostrStandard::Nip51 => RadrootsEventContractRegistryV7Standard::Nip51,
        RadrootsNostrStandard::Nip52 => RadrootsEventContractRegistryV7Standard::Nip52,
        RadrootsNostrStandard::Nip53 => RadrootsEventContractRegistryV7Standard::Nip53,
        RadrootsNostrStandard::Nip54 => RadrootsEventContractRegistryV7Standard::Nip54,
        RadrootsNostrStandard::Nip56 => RadrootsEventContractRegistryV7Standard::Nip56,
        RadrootsNostrStandard::Nip57 => RadrootsEventContractRegistryV7Standard::Nip57,
        RadrootsNostrStandard::Nip78 => RadrootsEventContractRegistryV7Standard::Nip78,
        RadrootsNostrStandard::Nip90 => RadrootsEventContractRegistryV7Standard::Nip90,
        RadrootsNostrStandard::Nip94 => RadrootsEventContractRegistryV7Standard::Nip94,
        RadrootsNostrStandard::Nip98 => RadrootsEventContractRegistryV7Standard::Nip98,
        RadrootsNostrStandard::Nip99 => RadrootsEventContractRegistryV7Standard::Nip99,
        RadrootsNostrStandard::Radroots => RadrootsEventContractRegistryV7Standard::Radroots,
    }
}

fn stability_inventory(value: RadrootsEventStability) -> RadrootsEventContractRegistryV7Stability {
    match value {
        RadrootsEventStability::Stable => RadrootsEventContractRegistryV7Stability::Stable,
        RadrootsEventStability::Experimental => {
            RadrootsEventContractRegistryV7Stability::Experimental
        }
    }
}

fn privacy_inventory(value: RadrootsEventPrivacy) -> RadrootsEventContractRegistryV7Privacy {
    match value {
        RadrootsEventPrivacy::Public => RadrootsEventContractRegistryV7Privacy::Public,
        RadrootsEventPrivacy::Encrypted => RadrootsEventContractRegistryV7Privacy::Encrypted,
        RadrootsEventPrivacy::LocalOnly => RadrootsEventContractRegistryV7Privacy::LocalOnly,
        RadrootsEventPrivacy::Secret => RadrootsEventContractRegistryV7Privacy::Secret,
    }
}

fn author_role_inventory(value: AuthorRole) -> RadrootsEventContractRegistryV7ActorRole {
    match value {
        AuthorRole::Any => RadrootsEventContractRegistryV7ActorRole::Any,
        AuthorRole::Application => RadrootsEventContractRegistryV7ActorRole::Application,
        AuthorRole::Buyer => RadrootsEventContractRegistryV7ActorRole::Buyer,
        AuthorRole::Farmer => RadrootsEventContractRegistryV7ActorRole::Farmer,
        AuthorRole::Member => RadrootsEventContractRegistryV7ActorRole::Member,
        AuthorRole::Moderator => RadrootsEventContractRegistryV7ActorRole::Moderator,
        AuthorRole::Relay => RadrootsEventContractRegistryV7ActorRole::Relay,
        AuthorRole::Seller => RadrootsEventContractRegistryV7ActorRole::Seller,
        AuthorRole::Service => RadrootsEventContractRegistryV7ActorRole::Service,
    }
}

fn content_schema_inventory(
    value: RadrootsContentSchema,
) -> RadrootsEventContractRegistryV7ContentSchema {
    match value {
        RadrootsContentSchema::Empty => RadrootsEventContractRegistryV7ContentSchema::Empty,
        RadrootsContentSchema::JsonObject => {
            RadrootsEventContractRegistryV7ContentSchema::JsonObject
        }
        RadrootsContentSchema::PlainText => RadrootsEventContractRegistryV7ContentSchema::PlainText,
        RadrootsContentSchema::Markdown => RadrootsEventContractRegistryV7ContentSchema::Markdown,
        RadrootsContentSchema::Djot => RadrootsEventContractRegistryV7ContentSchema::Djot,
        RadrootsContentSchema::Encrypted => RadrootsEventContractRegistryV7ContentSchema::Encrypted,
        RadrootsContentSchema::BinaryReference => {
            RadrootsEventContractRegistryV7ContentSchema::BinaryReference
        }
    }
}

fn authoring_policy_inventory(
    value: RadrootsEventAuthoringPolicy,
) -> RadrootsEventContractRegistryV7AuthoringPolicy {
    match value {
        RadrootsEventAuthoringPolicy::GenericDraft => {
            RadrootsEventContractRegistryV7AuthoringPolicy::GenericDraft
        }
        RadrootsEventAuthoringPolicy::TypedOnly => {
            RadrootsEventContractRegistryV7AuthoringPolicy::TypedOnly
        }
        RadrootsEventAuthoringPolicy::ReadOnly => {
            RadrootsEventContractRegistryV7AuthoringPolicy::ReadOnly
        }
    }
}

fn discriminator_inventory(
    value: &RadrootsEventDiscriminator,
) -> RadrootsEventContractRegistryV7Discriminator {
    match value {
        RadrootsEventDiscriminator::KindOnly => {
            RadrootsEventContractRegistryV7Discriminator::KindOnly
        }
        RadrootsEventDiscriminator::AdmissionOnly => {
            RadrootsEventContractRegistryV7Discriminator::AdmissionOnly
        }
        RadrootsEventDiscriminator::ClassifiedListingPartition(value) => {
            RadrootsEventContractRegistryV7Discriminator::ClassifiedListingPartition {
                value: classified_listing_partition_inventory(*value),
            }
        }
        RadrootsEventDiscriminator::DTagExact(value) => {
            RadrootsEventContractRegistryV7Discriminator::DTagExact {
                value: (*value).to_string(),
            }
        }
        RadrootsEventDiscriminator::DTagPrefix(prefix) => {
            RadrootsEventContractRegistryV7Discriminator::DTagPrefix {
                prefix: (*prefix).to_string(),
            }
        }
        RadrootsEventDiscriminator::DTagSuffix(suffix) => {
            RadrootsEventContractRegistryV7Discriminator::DTagSuffix {
                suffix: (*suffix).to_string(),
            }
        }
        RadrootsEventDiscriminator::TagEquals { name, value } => {
            RadrootsEventContractRegistryV7Discriminator::TagEquals {
                name: (*name).to_string(),
                value: (*value).to_string(),
            }
        }
        RadrootsEventDiscriminator::ContentJsonFieldEquals { field, value } => {
            RadrootsEventContractRegistryV7Discriminator::ContentJsonFieldEquals {
                field: (*field).to_string(),
                value: (*value).to_string(),
            }
        }
        RadrootsEventDiscriminator::EnvelopeType(value) => {
            RadrootsEventContractRegistryV7Discriminator::EnvelopeType {
                value: (*value).to_string(),
            }
        }
        RadrootsEventDiscriminator::Composite(parts) => {
            RadrootsEventContractRegistryV7Discriminator::Composite {
                parts: parts.iter().map(discriminator_inventory).collect(),
            }
        }
    }
}

fn discriminator_from_json_value(
    value: &serde_json::Value,
) -> Result<RadrootsEventContractRegistryV7Discriminator, String> {
    let object = value
        .as_object()
        .ok_or_else(|| "registry-v7 discriminator must be an object".to_string())?;
    let discriminator_type = object
        .get("type")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "registry-v7 discriminator type must be a string".to_string())?;

    match discriminator_type {
        "kind_only" => {
            require_exact_discriminator_fields(object, &["type"])?;
            Ok(RadrootsEventContractRegistryV7Discriminator::KindOnly)
        }
        "admission_only" => {
            require_exact_discriminator_fields(object, &["type"])?;
            Ok(RadrootsEventContractRegistryV7Discriminator::AdmissionOnly)
        }
        "classified_listing_partition" => {
            require_exact_discriminator_fields(object, &["type", "value"])?;
            let value = serde_json::from_value(
                object
                    .get("value")
                    .cloned()
                    .ok_or_else(|| "registry-v7 discriminator is missing value".to_string())?,
            )
            .map_err(|error| {
                format!("invalid registry-v7 classified-listing discriminator value: {error}")
            })?;
            Ok(RadrootsEventContractRegistryV7Discriminator::ClassifiedListingPartition { value })
        }
        "d_tag_exact" => {
            require_exact_discriminator_fields(object, &["type", "value"])?;
            Ok(RadrootsEventContractRegistryV7Discriminator::DTagExact {
                value: discriminator_string_field(object, "value")?,
            })
        }
        "d_tag_prefix" => {
            require_exact_discriminator_fields(object, &["type", "prefix"])?;
            Ok(RadrootsEventContractRegistryV7Discriminator::DTagPrefix {
                prefix: discriminator_string_field(object, "prefix")?,
            })
        }
        "d_tag_suffix" => {
            require_exact_discriminator_fields(object, &["type", "suffix"])?;
            Ok(RadrootsEventContractRegistryV7Discriminator::DTagSuffix {
                suffix: discriminator_string_field(object, "suffix")?,
            })
        }
        "tag_equals" => {
            require_exact_discriminator_fields(object, &["type", "name", "value"])?;
            Ok(RadrootsEventContractRegistryV7Discriminator::TagEquals {
                name: discriminator_string_field(object, "name")?,
                value: discriminator_string_field(object, "value")?,
            })
        }
        "content_json_field_equals" => {
            require_exact_discriminator_fields(object, &["type", "field", "value"])?;
            Ok(
                RadrootsEventContractRegistryV7Discriminator::ContentJsonFieldEquals {
                    field: discriminator_string_field(object, "field")?,
                    value: discriminator_string_field(object, "value")?,
                },
            )
        }
        "envelope_type" => {
            require_exact_discriminator_fields(object, &["type", "value"])?;
            Ok(RadrootsEventContractRegistryV7Discriminator::EnvelopeType {
                value: discriminator_string_field(object, "value")?,
            })
        }
        "composite" => {
            require_exact_discriminator_fields(object, &["type", "parts"])?;
            let parts = serde_json::from_value(
                object
                    .get("parts")
                    .cloned()
                    .ok_or_else(|| "registry-v7 discriminator is missing parts".to_string())?,
            )
            .map_err(|error| format!("invalid registry-v7 composite discriminator: {error}"))?;
            Ok(RadrootsEventContractRegistryV7Discriminator::Composite { parts })
        }
        unknown => Err(format!(
            "unknown registry-v7 discriminator type {unknown:?}"
        )),
    }
}

fn require_exact_discriminator_fields(
    object: &serde_json::Map<String, serde_json::Value>,
    expected: &[&str],
) -> Result<(), String> {
    if object.len() != expected.len()
        || object
            .keys()
            .any(|field| !expected.contains(&field.as_str()))
    {
        return Err("registry-v7 discriminator contains unknown or missing fields".to_string());
    }
    Ok(())
}

fn discriminator_string_field(
    object: &serde_json::Map<String, serde_json::Value>,
    field: &str,
) -> Result<String, String> {
    object
        .get(field)
        .and_then(serde_json::Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| format!("registry-v7 discriminator {field} must be a string"))
}

fn classified_listing_partition_inventory(
    value: RadrootsClassifiedListingPartition,
) -> RadrootsEventContractRegistryV7ClassifiedListingPartition {
    match value {
        RadrootsClassifiedListingPartition::FocusedFoodAvailability => {
            RadrootsEventContractRegistryV7ClassifiedListingPartition::FocusedFoodAvailability
        }
        RadrootsClassifiedListingPartition::OperationalListing => {
            RadrootsEventContractRegistryV7ClassifiedListingPartition::OperationalListing
        }
        RadrootsClassifiedListingPartition::GenericNip99 => {
            RadrootsEventContractRegistryV7ClassifiedListingPartition::GenericNip99
        }
        RadrootsClassifiedListingPartition::Ambiguous => {
            RadrootsEventContractRegistryV7ClassifiedListingPartition::Ambiguous
        }
    }
}

fn tag_inventory(contract: &RadrootsTagContract) -> RadrootsEventContractRegistryV7Tag {
    RadrootsEventContractRegistryV7Tag {
        name: contract.name.to_string(),
        cardinality: tag_cardinality_inventory(contract.cardinality),
        semantic: tag_semantic_inventory(contract.semantic),
        value_type: tag_value_type_inventory(contract.value_type),
        relay_indexed: contract.relay_indexed,
    }
}

fn tag_cardinality_inventory(
    value: RadrootsTagCardinality,
) -> RadrootsEventContractRegistryV7TagCardinality {
    match value {
        RadrootsTagCardinality::RequiredOne => {
            RadrootsEventContractRegistryV7TagCardinality::RequiredOne
        }
        RadrootsTagCardinality::OptionalOne => {
            RadrootsEventContractRegistryV7TagCardinality::OptionalOne
        }
        RadrootsTagCardinality::OptionalMany => {
            RadrootsEventContractRegistryV7TagCardinality::OptionalMany
        }
        RadrootsTagCardinality::RequiredMany => {
            RadrootsEventContractRegistryV7TagCardinality::RequiredMany
        }
    }
}

fn tag_semantic_inventory(
    value: RadrootsTagSemantic,
) -> RadrootsEventContractRegistryV7TagSemantic {
    match value {
        RadrootsTagSemantic::AddressableCoordinate => {
            RadrootsEventContractRegistryV7TagSemantic::AddressableCoordinate
        }
        RadrootsTagSemantic::CalendarEventAuthor => {
            RadrootsEventContractRegistryV7TagSemantic::CalendarEventAuthor
        }
        RadrootsTagSemantic::CalendarEventReference => {
            RadrootsEventContractRegistryV7TagSemantic::CalendarEventReference
        }
        RadrootsTagSemantic::CalendarEventRevision => {
            RadrootsEventContractRegistryV7TagSemantic::CalendarEventRevision
        }
        RadrootsTagSemantic::CalendarInclusionRequest => {
            RadrootsEventContractRegistryV7TagSemantic::CalendarInclusionRequest
        }
        RadrootsTagSemantic::CalendarEnd => RadrootsEventContractRegistryV7TagSemantic::CalendarEnd,
        RadrootsTagSemantic::CalendarStart => {
            RadrootsEventContractRegistryV7TagSemantic::CalendarStart
        }
        RadrootsTagSemantic::Category => RadrootsEventContractRegistryV7TagSemantic::Category,
        RadrootsTagSemantic::Citation => RadrootsEventContractRegistryV7TagSemantic::Citation,
        RadrootsTagSemantic::Contract => RadrootsEventContractRegistryV7TagSemantic::Contract,
        RadrootsTagSemantic::Counterparty => {
            RadrootsEventContractRegistryV7TagSemantic::Counterparty
        }
        RadrootsTagSemantic::Evidence => RadrootsEventContractRegistryV7TagSemantic::Evidence,
        RadrootsTagSemantic::EventPointer => {
            RadrootsEventContractRegistryV7TagSemantic::EventPointer
        }
        RadrootsTagSemantic::FreeBusy => RadrootsEventContractRegistryV7TagSemantic::FreeBusy,
        RadrootsTagSemantic::Geohash => RadrootsEventContractRegistryV7TagSemantic::Geohash,
        RadrootsTagSemantic::GroupId => RadrootsEventContractRegistryV7TagSemantic::GroupId,
        RadrootsTagSemantic::Identifier => RadrootsEventContractRegistryV7TagSemantic::Identifier,
        RadrootsTagSemantic::Image => RadrootsEventContractRegistryV7TagSemantic::Image,
        RadrootsTagSemantic::Kind => RadrootsEventContractRegistryV7TagSemantic::Kind,
        RadrootsTagSemantic::ClassifiedListingAddress => {
            RadrootsEventContractRegistryV7TagSemantic::ClassifiedListingAddress
        }
        RadrootsTagSemantic::OperationalListingSnapshot => {
            RadrootsEventContractRegistryV7TagSemantic::OperationalListingSnapshot
        }
        RadrootsTagSemantic::ListDescription => {
            RadrootsEventContractRegistryV7TagSemantic::ListDescription
        }
        RadrootsTagSemantic::Location => RadrootsEventContractRegistryV7TagSemantic::Location,
        RadrootsTagSemantic::Nip01Coordinate => {
            RadrootsEventContractRegistryV7TagSemantic::Nip01Coordinate
        }
        RadrootsTagSemantic::Participant => RadrootsEventContractRegistryV7TagSemantic::Participant,
        RadrootsTagSemantic::PreviousEvent => {
            RadrootsEventContractRegistryV7TagSemantic::PreviousEvent
        }
        RadrootsTagSemantic::Price => RadrootsEventContractRegistryV7TagSemantic::Price,
        RadrootsTagSemantic::PublishedAt => RadrootsEventContractRegistryV7TagSemantic::PublishedAt,
        RadrootsTagSemantic::Relay => RadrootsEventContractRegistryV7TagSemantic::Relay,
        RadrootsTagSemantic::Reference => RadrootsEventContractRegistryV7TagSemantic::Reference,
        RadrootsTagSemantic::ReviewTarget => {
            RadrootsEventContractRegistryV7TagSemantic::ReviewTarget
        }
        RadrootsTagSemantic::RootEvent => RadrootsEventContractRegistryV7TagSemantic::RootEvent,
        RadrootsTagSemantic::ServiceInput => {
            RadrootsEventContractRegistryV7TagSemantic::ServiceInput
        }
        RadrootsTagSemantic::ServiceOutput => {
            RadrootsEventContractRegistryV7TagSemantic::ServiceOutput
        }
        RadrootsTagSemantic::Source => RadrootsEventContractRegistryV7TagSemantic::Source,
        RadrootsTagSemantic::Status => RadrootsEventContractRegistryV7TagSemantic::Status,
        RadrootsTagSemantic::Summary => RadrootsEventContractRegistryV7TagSemantic::Summary,
        RadrootsTagSemantic::Title => RadrootsEventContractRegistryV7TagSemantic::Title,
        RadrootsTagSemantic::Topic => RadrootsEventContractRegistryV7TagSemantic::Topic,
        RadrootsTagSemantic::TimeZone => RadrootsEventContractRegistryV7TagSemantic::TimeZone,
        RadrootsTagSemantic::Url => RadrootsEventContractRegistryV7TagSemantic::Url,
        RadrootsTagSemantic::UtcDayCoverage => {
            RadrootsEventContractRegistryV7TagSemantic::UtcDayCoverage
        }
    }
}

fn tag_value_type_inventory(
    value: RadrootsTagValueType,
) -> RadrootsEventContractRegistryV7TagValueType {
    match value {
        RadrootsTagValueType::AddressableCoordinate => {
            RadrootsEventContractRegistryV7TagValueType::AddressableCoordinate
        }
        RadrootsTagValueType::CalendarDate => {
            RadrootsEventContractRegistryV7TagValueType::CalendarDate
        }
        RadrootsTagValueType::CalendarEventCoordinate => {
            RadrootsEventContractRegistryV7TagValueType::CalendarEventCoordinate
        }
        RadrootsTagValueType::CalendarFreeBusy => {
            RadrootsEventContractRegistryV7TagValueType::CalendarFreeBusy
        }
        RadrootsTagValueType::CalendarRsvpStatus => {
            RadrootsEventContractRegistryV7TagValueType::CalendarRsvpStatus
        }
        RadrootsTagValueType::CalendarUid => {
            RadrootsEventContractRegistryV7TagValueType::CalendarUid
        }
        RadrootsTagValueType::ContractId => RadrootsEventContractRegistryV7TagValueType::ContractId,
        RadrootsTagValueType::DTag => RadrootsEventContractRegistryV7TagValueType::DTag,
        RadrootsTagValueType::EventId => RadrootsEventContractRegistryV7TagValueType::EventId,
        RadrootsTagValueType::EventPointer => {
            RadrootsEventContractRegistryV7TagValueType::EventPointer
        }
        RadrootsTagValueType::Geohash => RadrootsEventContractRegistryV7TagValueType::Geohash,
        RadrootsTagValueType::IanaTimeZoneId => {
            RadrootsEventContractRegistryV7TagValueType::IanaTimeZoneId
        }
        RadrootsTagValueType::Kind => RadrootsEventContractRegistryV7TagValueType::Kind,
        RadrootsTagValueType::Nip01Coordinate => {
            RadrootsEventContractRegistryV7TagValueType::Nip01Coordinate
        }
        RadrootsTagValueType::PublicKey => RadrootsEventContractRegistryV7TagValueType::PublicKey,
        RadrootsTagValueType::RelayUrl => RadrootsEventContractRegistryV7TagValueType::RelayUrl,
        RadrootsTagValueType::Sha256 => RadrootsEventContractRegistryV7TagValueType::Sha256,
        RadrootsTagValueType::Text => RadrootsEventContractRegistryV7TagValueType::Text,
        RadrootsTagValueType::UnixTimestamp => {
            RadrootsEventContractRegistryV7TagValueType::UnixTimestamp
        }
        RadrootsTagValueType::Uri => RadrootsEventContractRegistryV7TagValueType::Uri,
        RadrootsTagValueType::Url => RadrootsEventContractRegistryV7TagValueType::Url,
        RadrootsTagValueType::UtcDayIndex => {
            RadrootsEventContractRegistryV7TagValueType::UtcDayIndex
        }
        RadrootsTagValueType::Uuid => RadrootsEventContractRegistryV7TagValueType::Uuid,
    }
}

fn reducer_inventory(value: RadrootsReducer) -> RadrootsEventContractRegistryV7Reducer {
    match value {
        RadrootsReducer::CalendarProjection => {
            RadrootsEventContractRegistryV7Reducer::CalendarProjection
        }
        RadrootsReducer::FarmOpsProjection => {
            RadrootsEventContractRegistryV7Reducer::FarmOpsProjection
        }
        RadrootsReducer::GroupProjection => RadrootsEventContractRegistryV7Reducer::GroupProjection,
        RadrootsReducer::KnowledgeProjection => {
            RadrootsEventContractRegistryV7Reducer::KnowledgeProjection
        }
        RadrootsReducer::OperationalListingInventoryAccounting => {
            RadrootsEventContractRegistryV7Reducer::OperationalListingInventoryAccounting
        }
        RadrootsReducer::OperationalListingProjection => {
            RadrootsEventContractRegistryV7Reducer::OperationalListingProjection
        }
        RadrootsReducer::MarketProjection => {
            RadrootsEventContractRegistryV7Reducer::MarketProjection
        }
        RadrootsReducer::OrderProjection => RadrootsEventContractRegistryV7Reducer::OrderProjection,
        RadrootsReducer::ProfileProjection => {
            RadrootsEventContractRegistryV7Reducer::ProfileProjection
        }
        RadrootsReducer::NostrRelayPolicyProjection => {
            RadrootsEventContractRegistryV7Reducer::NostrRelayPolicyProjection
        }
        RadrootsReducer::SocialProjection => {
            RadrootsEventContractRegistryV7Reducer::SocialProjection
        }
        RadrootsReducer::TradeProjection => RadrootsEventContractRegistryV7Reducer::TradeProjection,
        RadrootsReducer::TradeValidation => RadrootsEventContractRegistryV7Reducer::TradeValidation,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inventory_pins_runtime_order_and_counts() {
        let inventory = event_contract_registry_v7_inventory();

        assert_eq!(
            inventory.schema_version,
            RADROOTS_EVENT_CONTRACT_REGISTRY_V7_INVENTORY_SCHEMA_VERSION
        );
        assert_eq!(
            inventory.event_contract_registry_version,
            RADROOTS_EVENT_CONTRACT_REGISTRY_V7_VERSION
        );
        assert_eq!(
            inventory.kind_contracts.len(),
            RADROOTS_EVENT_CONTRACT_REGISTRY_V7_KIND_COUNT
        );
        assert_eq!(
            inventory.event_contracts.len(),
            RADROOTS_EVENT_CONTRACT_REGISTRY_V7_EVENT_COUNT
        );
        assert!(
            inventory
                .kind_contracts
                .iter()
                .enumerate()
                .all(|(ordinal, contract)| contract.ordinal == ordinal)
        );
        assert!(
            inventory
                .event_contracts
                .iter()
                .enumerate()
                .all(|(ordinal, contract)| contract.ordinal == ordinal)
        );
    }

    #[test]
    fn inventory_json_is_canonical_and_round_trips_strictly() {
        let json = event_contract_registry_v7_inventory_json().expect("serialize inventory");

        assert!(json.ends_with('\n'));
        assert!(!json.ends_with("\n\n"));
        assert!(!json.contains("\r\n"));
        assert!(json.contains("\n  \"schema_version\": 1,"));
        assert_eq!(
            parse_event_contract_registry_v7_inventory_json(&json).expect("parse inventory"),
            event_contract_registry_v7_inventory()
        );
    }

    #[test]
    fn inventory_parser_rejects_unknown_fields_and_enum_values() {
        let mut top_level =
            serde_json::to_value(event_contract_registry_v7_inventory()).expect("inventory value");
        top_level
            .as_object_mut()
            .expect("inventory object")
            .insert("unknown".to_string(), serde_json::Value::Bool(true));
        assert!(
            serde_json::from_value::<RadrootsEventContractRegistryV7Inventory>(top_level).is_err()
        );

        let mut nested =
            serde_json::to_value(event_contract_registry_v7_inventory()).expect("inventory value");
        nested["event_contracts"][0]["discriminator"]["unknown"] = serde_json::Value::Bool(true);
        assert!(
            serde_json::from_value::<RadrootsEventContractRegistryV7Inventory>(nested).is_err()
        );

        let mut enum_value =
            serde_json::to_value(event_contract_registry_v7_inventory()).expect("inventory value");
        enum_value["kind_contracts"][0]["class"] =
            serde_json::Value::String("future_class".to_string());
        assert!(
            serde_json::from_value::<RadrootsEventContractRegistryV7Inventory>(enum_value).is_err()
        );
    }

    #[test]
    fn inventory_digest_is_deterministic_lowercase_hex() {
        let first = event_contract_registry_v7_inventory_sha256().expect("hash inventory");
        let second = event_contract_registry_v7_inventory_sha256().expect("hash inventory");

        assert_eq!(first, second);
        assert_eq!(first.len(), 64);
        assert!(
            first
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        );
    }
}
