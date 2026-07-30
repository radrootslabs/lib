//! Governed event-contract manifest generation and validation.
//!
//! Manifest operations derive deterministic JSON and SHA-256 evidence from
//! versioned contract authority. They perform no filesystem writes or release
//! publication themselves.

#[cfg(not(feature = "std"))]
use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};

use radroots_event::contract::VERSION;
use radroots_event::contract::{
    ContentSchema, ContractFamily, EventClass, EventContract, EventDiscriminator, EventPrivacy,
    EventStability, NostrStandard, RADROOTS_EVENT_CONTRACT_REGISTRY_VERSION, Reducer,
    TagCardinality, TagContract, TagSemantic, TagValueType, all_event_contracts_registry_v7,
    event_contract_family, kind_contract_registry_v7,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};

pub mod registry_v7;

pub const RADROOTS_KNOWLEDGE_CONTRACT_MANIFEST_SCHEMA_VERSION: u32 = 2;

const HISTORICAL_KNOWLEDGE_CONTRACT_INTRODUCTIONS: [(&str, &str); 11] = [
    ("radroots.knowledge.change_proposal.v1", "0.1.0-alpha.2"),
    ("radroots.knowledge.claim.v1", "0.1.0-alpha.2"),
    (
        "radroots.knowledge.contribution_attestation.v1",
        "0.1.0-alpha.2",
    ),
    ("radroots.knowledge.evidence_bounty.v1", "0.1.0-alpha.2"),
    ("radroots.knowledge.field_report.v1", "0.1.0-alpha.2"),
    ("radroots.knowledge.relation.v1", "0.1.0-alpha.2"),
    ("radroots.knowledge.review.v1", "0.1.0-alpha.2"),
    ("radroots.knowledge.source.v1", "0.1.0-alpha.2"),
    ("radroots.wiki.article.v1", "0.1.0-alpha.2"),
    ("radroots.wiki.merge_request.v1", "0.1.0-alpha.2"),
    ("radroots.wiki.redirect.v1", "0.1.0-alpha.2"),
];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RadrootsKnowledgeContractManifest {
    pub schema_version: u32,
    pub registry_version: u32,
    pub radroots_event_version: String,
    pub radroots_event_codec_version: String,
    pub contract_count: usize,
    pub contracts: Vec<RadrootsKnowledgeContractManifestEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RadrootsKnowledgeContractManifestEntry {
    pub contract_id: String,
    pub kind: u32,
    pub class: String,
    pub standard: String,
    pub stability: String,
    pub privacy: String,
    pub author_role: String,
    pub content_schema: String,
    pub payload_type: String,
    pub discriminators: Vec<RadrootsKnowledgeManifestDiscriminator>,
    pub tag_contracts: Vec<RadrootsKnowledgeManifestTagContract>,
    pub reducers: Vec<String>,
    pub codec_support: RadrootsKnowledgeManifestCodecSupport,
    pub sdk_builder_support: bool,
    pub sdk_draft_support: bool,
    pub wasm_tag_builder_support: bool,
    pub wasm_verified_decode_support: bool,
    pub deprecated: bool,
    pub replaced_by: Option<String>,
    pub introduced_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RadrootsKnowledgeManifestDiscriminator {
    KindOnly,
    AdmissionOnly,
    ClassifiedListingPartition {
        value: String,
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
        parts: Vec<RadrootsKnowledgeManifestDiscriminator>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RadrootsKnowledgeManifestTagContract {
    pub name: String,
    pub cardinality: String,
    pub semantic: String,
    pub value_type: String,
    pub relay_indexed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RadrootsKnowledgeManifestCodecSupport {
    pub encode: bool,
    pub decode: bool,
    pub contract_validation: bool,
    pub verified_decode: bool,
    pub verified_decode_requires_nostr: bool,
}

pub fn knowledge_contract_manifest() -> RadrootsKnowledgeContractManifest {
    let mut contracts = all_event_contracts_registry_v7()
        .iter()
        .filter(|contract| event_contract_family(contract) == Some(ContractFamily::Knowledge))
        .map(manifest_entry)
        .collect::<Vec<_>>();
    contracts.sort_by(|left, right| left.contract_id.cmp(&right.contract_id));

    RadrootsKnowledgeContractManifest {
        schema_version: RADROOTS_KNOWLEDGE_CONTRACT_MANIFEST_SCHEMA_VERSION,
        registry_version: RADROOTS_EVENT_CONTRACT_REGISTRY_VERSION,
        radroots_event_version: VERSION.to_string(),
        radroots_event_codec_version: env!("CARGO_PKG_VERSION").to_string(),
        contract_count: contracts.len(),
        contracts,
    }
}

pub fn contract_manifest_json() -> Result<String, serde_json::Error> {
    canonical_manifest_json(&knowledge_contract_manifest())
}

pub fn contract_manifest_sha256() -> Result<String, serde_json::Error> {
    let json = contract_manifest_json()?;
    Ok(manifest_sha256(&json))
}

pub fn parse_knowledge_contract_manifest_json(
    json: &str,
) -> Result<RadrootsKnowledgeContractManifest, serde_json::Error> {
    serde_json::from_str(json)
}

pub(super) fn canonical_manifest_json<T>(manifest: &T) -> Result<String, serde_json::Error>
where
    T: Serialize,
{
    let mut json = serde_json::to_string_pretty(manifest)?;
    json.push('\n');
    Ok(json)
}

pub(super) fn parse_manifest_json<T>(json: &str) -> Result<T, serde_json::Error>
where
    T: DeserializeOwned,
{
    serde_json::from_str(json)
}

pub(super) fn manifest_sha256(json: &str) -> String {
    hex::encode(Sha256::digest(json.as_bytes()))
}

fn manifest_entry(contract: &EventContract) -> RadrootsKnowledgeContractManifestEntry {
    let standard = kind_contract_registry_v7(contract.kind)
        .map(|contract| standard_label(contract.standard))
        .unwrap_or("unknown");
    let mvp_support = mvp_sdk_and_wasm_tag_support(contract.id);

    RadrootsKnowledgeContractManifestEntry {
        contract_id: contract.id.to_string(),
        kind: contract.kind,
        class: class_label(contract.class).to_string(),
        standard: standard.to_string(),
        stability: stability_label(contract.stability).to_string(),
        privacy: privacy_label(contract.privacy).to_string(),
        author_role: contract.required_author_role().as_str().to_string(),
        content_schema: content_schema_label(contract.content_schema).to_string(),
        payload_type: contract.payload_type.to_string(),
        discriminators: vec![discriminator_manifest(&contract.discriminator)],
        tag_contracts: contract.tags.iter().map(tag_contract_manifest).collect(),
        reducers: contract
            .reducers
            .iter()
            .copied()
            .map(reducer_label)
            .map(ToString::to_string)
            .collect(),
        codec_support: RadrootsKnowledgeManifestCodecSupport {
            encode: true,
            decode: true,
            contract_validation: true,
            verified_decode: true,
            verified_decode_requires_nostr: true,
        },
        sdk_builder_support: mvp_support,
        sdk_draft_support: mvp_support,
        wasm_tag_builder_support: mvp_support,
        wasm_verified_decode_support: true,
        deprecated: false,
        replaced_by: None,
        introduced_at: knowledge_contract_introduced_at(contract.id).to_string(),
    }
}

fn knowledge_contract_introduced_at(contract_id: &str) -> &'static str {
    HISTORICAL_KNOWLEDGE_CONTRACT_INTRODUCTIONS
        .iter()
        .find_map(|(historical_id, version)| (*historical_id == contract_id).then_some(*version))
        .unwrap_or(env!("CARGO_PKG_VERSION"))
}

fn mvp_sdk_and_wasm_tag_support(contract_id: &str) -> bool {
    matches!(
        contract_id,
        "radroots.wiki.article.v1"
            | "radroots.wiki.redirect.v1"
            | "radroots.wiki.merge_request.v1"
            | "radroots.knowledge.source.v1"
            | "radroots.knowledge.claim.v1"
            | "radroots.knowledge.relation.v1"
            | "radroots.knowledge.review.v1"
            | "radroots.knowledge.field_report.v1"
    )
}

fn discriminator_manifest(
    discriminator: &EventDiscriminator,
) -> RadrootsKnowledgeManifestDiscriminator {
    match discriminator {
        EventDiscriminator::KindOnly => RadrootsKnowledgeManifestDiscriminator::KindOnly,
        EventDiscriminator::AdmissionOnly => RadrootsKnowledgeManifestDiscriminator::AdmissionOnly,
        EventDiscriminator::ClassifiedListingPartition(value) => {
            RadrootsKnowledgeManifestDiscriminator::ClassifiedListingPartition {
                value: classified_listing_partition_label(*value).to_string(),
            }
        }
        EventDiscriminator::DTagExact(value) => RadrootsKnowledgeManifestDiscriminator::DTagExact {
            value: (*value).to_string(),
        },
        EventDiscriminator::DTagPrefix(prefix) => {
            RadrootsKnowledgeManifestDiscriminator::DTagPrefix {
                prefix: (*prefix).to_string(),
            }
        }
        EventDiscriminator::DTagSuffix(suffix) => {
            RadrootsKnowledgeManifestDiscriminator::DTagSuffix {
                suffix: (*suffix).to_string(),
            }
        }
        EventDiscriminator::TagEquals { name, value } => {
            RadrootsKnowledgeManifestDiscriminator::TagEquals {
                name: (*name).to_string(),
                value: (*value).to_string(),
            }
        }
        EventDiscriminator::ContentJsonFieldEquals { field, value } => {
            RadrootsKnowledgeManifestDiscriminator::ContentJsonFieldEquals {
                field: (*field).to_string(),
                value: (*value).to_string(),
            }
        }
        EventDiscriminator::EnvelopeType(value) => {
            RadrootsKnowledgeManifestDiscriminator::EnvelopeType {
                value: (*value).to_string(),
            }
        }
        EventDiscriminator::Composite(parts) => RadrootsKnowledgeManifestDiscriminator::Composite {
            parts: parts.iter().map(discriminator_manifest).collect(),
        },
    }
}

fn classified_listing_partition_label(
    value: radroots_event::listing::classified::ClassifiedListingPartition,
) -> &'static str {
    use radroots_event::listing::classified::ClassifiedListingPartition;

    match value {
        ClassifiedListingPartition::FocusedFoodAvailability => "focused_food_availability",
        ClassifiedListingPartition::OperationalListing => "operational_listing",
        ClassifiedListingPartition::GenericNip99 => "generic_nip99",
        ClassifiedListingPartition::Ambiguous => "ambiguous",
    }
}

fn tag_contract_manifest(contract: &TagContract) -> RadrootsKnowledgeManifestTagContract {
    RadrootsKnowledgeManifestTagContract {
        name: contract.name.to_string(),
        cardinality: tag_cardinality_label(contract.cardinality).to_string(),
        semantic: tag_semantic_label(contract.semantic).to_string(),
        value_type: tag_value_type_label(contract.value_type).to_string(),
        relay_indexed: contract.relay_indexed,
    }
}

fn class_label(value: EventClass) -> &'static str {
    match value {
        EventClass::Regular => "regular",
        EventClass::Replaceable => "replaceable",
        EventClass::Addressable => "addressable",
        EventClass::Ephemeral => "ephemeral",
    }
}

fn standard_label(value: NostrStandard) -> &'static str {
    match value {
        NostrStandard::Nip01 => "nip01",
        NostrStandard::Nip09 => "nip09",
        NostrStandard::Nip17 => "nip17",
        NostrStandard::Nip18 => "nip18",
        NostrStandard::Nip22 => "nip22",
        NostrStandard::Nip23 => "nip23",
        NostrStandard::Nip25 => "nip25",
        NostrStandard::Nip28 => "nip28",
        NostrStandard::Nip29 => "nip29",
        NostrStandard::Nip42 => "nip42",
        NostrStandard::Nip51 => "nip51",
        NostrStandard::Nip52 => "nip52",
        NostrStandard::Nip53 => "nip53",
        NostrStandard::Nip54 => "nip54",
        NostrStandard::Nip56 => "nip56",
        NostrStandard::Nip57 => "nip57",
        NostrStandard::Nip78 => "nip78",
        NostrStandard::Nip90 => "nip90",
        NostrStandard::Nip94 => "nip94",
        NostrStandard::Nip98 => "nip98",
        NostrStandard::Nip99 => "nip99",
        NostrStandard::Radroots => "radroots",
    }
}

fn stability_label(value: EventStability) -> &'static str {
    match value {
        EventStability::Stable => "stable",
        EventStability::Experimental => "experimental",
    }
}

fn privacy_label(value: EventPrivacy) -> &'static str {
    match value {
        EventPrivacy::Public => "public",
        EventPrivacy::Encrypted => "encrypted",
        EventPrivacy::LocalOnly => "local_only",
        EventPrivacy::Secret => "secret",
    }
}

fn content_schema_label(value: ContentSchema) -> &'static str {
    match value {
        ContentSchema::Empty => "empty",
        ContentSchema::JsonObject => "json_object",
        ContentSchema::PlainText => "plain_text",
        ContentSchema::Markdown => "markdown",
        ContentSchema::Djot => "djot",
        ContentSchema::Encrypted => "encrypted",
        ContentSchema::BinaryReference => "binary_reference",
    }
}

fn tag_cardinality_label(value: TagCardinality) -> &'static str {
    match value {
        TagCardinality::RequiredOne => "required_one",
        TagCardinality::OptionalOne => "optional_one",
        TagCardinality::OptionalMany => "optional_many",
        TagCardinality::RequiredMany => "required_many",
    }
}

fn tag_semantic_label(value: TagSemantic) -> &'static str {
    match value {
        TagSemantic::AddressableCoordinate => "addressable_coordinate",
        TagSemantic::CalendarEventAuthor => "calendar_event_author",
        TagSemantic::CalendarEventReference => "calendar_event_reference",
        TagSemantic::CalendarEventRevision => "calendar_event_revision",
        TagSemantic::CalendarInclusionRequest => "calendar_inclusion_request",
        TagSemantic::CalendarEnd => "calendar_end",
        TagSemantic::CalendarStart => "calendar_start",
        TagSemantic::Category => "category",
        TagSemantic::Citation => "citation",
        TagSemantic::Contract => "contract",
        TagSemantic::Counterparty => "counterparty",
        TagSemantic::Evidence => "evidence",
        TagSemantic::EventPointer => "event_pointer",
        TagSemantic::FreeBusy => "free_busy",
        TagSemantic::Geohash => "geohash",
        TagSemantic::GroupId => "group_id",
        TagSemantic::Identifier => "identifier",
        TagSemantic::Image => "image",
        TagSemantic::Kind => "kind",
        TagSemantic::ClassifiedListingAddress => "listing_address",
        TagSemantic::OperationalListingSnapshot => "listing_snapshot",
        TagSemantic::ListDescription => "list_description",
        TagSemantic::Location => "location",
        TagSemantic::Nip01Coordinate => "nip01_coordinate",
        TagSemantic::Participant => "participant",
        TagSemantic::PreviousEvent => "previous_event",
        TagSemantic::Price => "price",
        TagSemantic::PublishedAt => "published_at",
        TagSemantic::Relay => "relay",
        TagSemantic::Reference => "reference",
        TagSemantic::ReviewTarget => "review_target",
        TagSemantic::RootEvent => "root_event",
        TagSemantic::ServiceInput => "service_input",
        TagSemantic::ServiceOutput => "service_output",
        TagSemantic::Source => "source",
        TagSemantic::Status => "status",
        TagSemantic::Summary => "summary",
        TagSemantic::Title => "title",
        TagSemantic::Topic => "topic",
        TagSemantic::TimeZone => "time_zone",
        TagSemantic::Url => "url",
        TagSemantic::UtcDayCoverage => "utc_day_coverage",
    }
}

fn tag_value_type_label(value: TagValueType) -> &'static str {
    match value {
        TagValueType::AddressableCoordinate => "addressable_coordinate",
        TagValueType::CalendarDate => "calendar_date",
        TagValueType::CalendarEventCoordinate => "calendar_event_coordinate",
        TagValueType::CalendarFreeBusy => "calendar_free_busy",
        TagValueType::CalendarRsvpStatus => "calendar_rsvp_status",
        TagValueType::CalendarUid => "calendar_uid",
        TagValueType::ContractId => "contract_id",
        TagValueType::DTag => "d_tag",
        TagValueType::EventId => "event_id",
        TagValueType::EventPointer => "event_pointer",
        TagValueType::Geohash => "geohash",
        TagValueType::IanaTimeZoneId => "iana_time_zone_id",
        TagValueType::Kind => "kind",
        TagValueType::Nip01Coordinate => "nip01_coordinate",
        TagValueType::PublicKey => "public_key",
        TagValueType::RelayUrl => "relay_url",
        TagValueType::Sha256 => "sha256",
        TagValueType::Text => "text",
        TagValueType::UnixTimestamp => "unix_timestamp",
        TagValueType::Uri => "uri",
        TagValueType::Url => "url",
        TagValueType::UtcDayIndex => "utc_day_index",
        TagValueType::Uuid => "uuid",
    }
}

fn reducer_label(value: Reducer) -> &'static str {
    match value {
        Reducer::CalendarProjection => "calendar_projection",
        Reducer::FarmOpsProjection => "farm_ops_projection",
        Reducer::GroupProjection => "group_projection",
        Reducer::KnowledgeProjection => "knowledge_projection",
        Reducer::OperationalListingInventoryAccounting => {
            "operational_listing_inventory_accounting"
        }
        Reducer::OperationalListingProjection => "operational_listing_projection",
        Reducer::MarketProjection => "market_projection",
        Reducer::OrderProjection => "order_projection",
        Reducer::ProfileProjection => "profile_projection",
        Reducer::NostrRelayPolicyProjection => "nostr_relay_policy_projection",
        Reducer::SocialProjection => "social_projection",
        Reducer::TradeProjection => "trade_projection",
        Reducer::TradeValidation => "trade_validation",
    }
}

#[cfg(test)]
mod tests {
    use super::{
        contract_manifest_json, contract_manifest_sha256, discriminator_manifest,
        knowledge_contract_manifest, parse_knowledge_contract_manifest_json, reducer_label,
        standard_label,
    };
    use radroots_event::{
        contract::{
            ContractFamily, EventDiscriminator, NostrStandard, Reducer,
            all_event_contracts_registry_v7, event_contract_family,
        },
        listing::classified::ClassifiedListingPartition,
    };

    #[test]
    fn knowledge_manifest_is_derived_from_authority_with_stable_order_and_count() {
        let manifest = knowledge_contract_manifest();
        let authority_count = all_event_contracts_registry_v7()
            .iter()
            .filter(|contract| event_contract_family(contract) == Some(ContractFamily::Knowledge))
            .count();

        assert_eq!(manifest.contract_count, authority_count);
        assert_eq!(manifest.contract_count, manifest.contracts.len());
        assert!(
            manifest
                .contracts
                .windows(2)
                .all(|pair| pair[0].contract_id < pair[1].contract_id)
        );
    }

    #[test]
    fn knowledge_manifest_json_is_canonical_repeatable_and_strictly_parsed() {
        let first = contract_manifest_json().expect("serialize manifest");
        let second = contract_manifest_json().expect("serialize manifest again");

        assert_eq!(first, second);
        assert!(first.ends_with('\n'));
        assert!(!first.ends_with("\n\n"));
        assert!(!first.contains("\r\n"));
        assert_eq!(
            parse_knowledge_contract_manifest_json(&first).expect("parse manifest"),
            knowledge_contract_manifest()
        );
        assert_eq!(
            contract_manifest_sha256().expect("hash manifest"),
            contract_manifest_sha256().expect("hash manifest again")
        );

        let mut unknown =
            serde_json::to_value(knowledge_contract_manifest()).expect("manifest value");
        unknown
            .as_object_mut()
            .expect("manifest object")
            .insert("unknown".to_string(), serde_json::Value::Bool(true));
        assert!(
            serde_json::from_value::<super::RadrootsKnowledgeContractManifest>(unknown).is_err()
        );
    }

    #[test]
    fn classified_listing_standard_label_is_nip99() {
        assert_eq!(standard_label(NostrStandard::Nip99), "nip99");
    }

    #[test]
    fn operational_listing_reducer_labels_are_unambiguous() {
        assert_eq!(
            reducer_label(Reducer::OperationalListingProjection),
            "operational_listing_projection"
        );
        assert_eq!(
            reducer_label(Reducer::OperationalListingInventoryAccounting),
            "operational_listing_inventory_accounting"
        );
    }

    #[test]
    fn classified_listing_partition_discriminators_render_exactly() {
        for (partition, expected) in [
            (
                ClassifiedListingPartition::FocusedFoodAvailability,
                "focused_food_availability",
            ),
            (
                ClassifiedListingPartition::OperationalListing,
                "operational_listing",
            ),
            (ClassifiedListingPartition::GenericNip99, "generic_nip99"),
            (ClassifiedListingPartition::Ambiguous, "ambiguous"),
        ] {
            let manifest =
                discriminator_manifest(&EventDiscriminator::ClassifiedListingPartition(partition));
            assert_eq!(
                serde_json::to_value(manifest).expect("serialized discriminator"),
                serde_json::json!({
                    "type": "classified_listing_partition",
                    "value": expected,
                })
            );
        }
    }
}
