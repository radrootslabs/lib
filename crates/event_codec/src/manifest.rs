#[cfg(not(feature = "std"))]
use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};

use radroots_event::contract::VERSION;
use radroots_event::contract::{
    RADROOTS_EVENT_CONTRACT_REGISTRY_VERSION, RadrootsContentSchema, RadrootsContractFamily,
    RadrootsEventClass, RadrootsEventContract, RadrootsEventDiscriminator, RadrootsEventPrivacy,
    RadrootsEventStability, RadrootsNostrStandard, RadrootsReducer, RadrootsTagCardinality,
    RadrootsTagContract, RadrootsTagSemantic, RadrootsTagValueType, all_event_contracts,
    event_contract_family, kind_contract,
};
use serde::{Deserialize, Serialize};
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
pub struct RadrootsKnowledgeContractManifest {
    pub schema_version: u32,
    pub registry_version: u32,
    pub radroots_event_version: String,
    pub radroots_event_codec_version: String,
    pub contract_count: usize,
    pub contracts: Vec<RadrootsKnowledgeContractManifestEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
pub struct RadrootsKnowledgeManifestTagContract {
    pub name: String,
    pub cardinality: String,
    pub semantic: String,
    pub value_type: String,
    pub relay_indexed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RadrootsKnowledgeManifestCodecSupport {
    pub encode: bool,
    pub decode: bool,
    pub contract_validation: bool,
    pub verified_decode: bool,
    pub verified_decode_requires_nostr: bool,
}

pub fn knowledge_contract_manifest() -> RadrootsKnowledgeContractManifest {
    let mut contracts = all_event_contracts()
        .iter()
        .filter(|contract| {
            event_contract_family(contract) == Some(RadrootsContractFamily::Knowledge)
        })
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
    let mut json = serde_json::to_string_pretty(&knowledge_contract_manifest())?;
    json.push('\n');
    Ok(json)
}

pub fn contract_manifest_sha256() -> Result<String, serde_json::Error> {
    let json = contract_manifest_json()?;
    Ok(hex::encode(Sha256::digest(json.as_bytes())))
}

fn manifest_entry(contract: &RadrootsEventContract) -> RadrootsKnowledgeContractManifestEntry {
    let standard = kind_contract(contract.kind)
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
    discriminator: &RadrootsEventDiscriminator,
) -> RadrootsKnowledgeManifestDiscriminator {
    match discriminator {
        RadrootsEventDiscriminator::KindOnly => RadrootsKnowledgeManifestDiscriminator::KindOnly,
        RadrootsEventDiscriminator::AdmissionOnly => {
            RadrootsKnowledgeManifestDiscriminator::AdmissionOnly
        }
        RadrootsEventDiscriminator::ClassifiedListingPartition(value) => {
            RadrootsKnowledgeManifestDiscriminator::ClassifiedListingPartition {
                value: classified_listing_partition_label(*value).to_string(),
            }
        }
        RadrootsEventDiscriminator::DTagExact(value) => {
            RadrootsKnowledgeManifestDiscriminator::DTagExact {
                value: (*value).to_string(),
            }
        }
        RadrootsEventDiscriminator::DTagPrefix(prefix) => {
            RadrootsKnowledgeManifestDiscriminator::DTagPrefix {
                prefix: (*prefix).to_string(),
            }
        }
        RadrootsEventDiscriminator::DTagSuffix(suffix) => {
            RadrootsKnowledgeManifestDiscriminator::DTagSuffix {
                suffix: (*suffix).to_string(),
            }
        }
        RadrootsEventDiscriminator::TagEquals { name, value } => {
            RadrootsKnowledgeManifestDiscriminator::TagEquals {
                name: (*name).to_string(),
                value: (*value).to_string(),
            }
        }
        RadrootsEventDiscriminator::ContentJsonFieldEquals { field, value } => {
            RadrootsKnowledgeManifestDiscriminator::ContentJsonFieldEquals {
                field: (*field).to_string(),
                value: (*value).to_string(),
            }
        }
        RadrootsEventDiscriminator::EnvelopeType(value) => {
            RadrootsKnowledgeManifestDiscriminator::EnvelopeType {
                value: (*value).to_string(),
            }
        }
        RadrootsEventDiscriminator::Composite(parts) => {
            RadrootsKnowledgeManifestDiscriminator::Composite {
                parts: parts.iter().map(discriminator_manifest).collect(),
            }
        }
    }
}

fn classified_listing_partition_label(
    value: radroots_event::listing::classified::RadrootsClassifiedListingPartition,
) -> &'static str {
    use radroots_event::listing::classified::RadrootsClassifiedListingPartition;

    match value {
        RadrootsClassifiedListingPartition::FocusedFoodAvailability => "focused_food_availability",
        RadrootsClassifiedListingPartition::OperationalListing => "operational_listing",
        RadrootsClassifiedListingPartition::GenericNip99 => "generic_nip99",
        RadrootsClassifiedListingPartition::Ambiguous => "ambiguous",
    }
}

fn tag_contract_manifest(contract: &RadrootsTagContract) -> RadrootsKnowledgeManifestTagContract {
    RadrootsKnowledgeManifestTagContract {
        name: contract.name.to_string(),
        cardinality: tag_cardinality_label(contract.cardinality).to_string(),
        semantic: tag_semantic_label(contract.semantic).to_string(),
        value_type: tag_value_type_label(contract.value_type).to_string(),
        relay_indexed: contract.relay_indexed,
    }
}

fn class_label(value: RadrootsEventClass) -> &'static str {
    match value {
        RadrootsEventClass::Regular => "regular",
        RadrootsEventClass::Replaceable => "replaceable",
        RadrootsEventClass::Addressable => "addressable",
        RadrootsEventClass::Ephemeral => "ephemeral",
    }
}

fn standard_label(value: RadrootsNostrStandard) -> &'static str {
    match value {
        RadrootsNostrStandard::Nip01 => "nip01",
        RadrootsNostrStandard::Nip09 => "nip09",
        RadrootsNostrStandard::Nip17 => "nip17",
        RadrootsNostrStandard::Nip18 => "nip18",
        RadrootsNostrStandard::Nip22 => "nip22",
        RadrootsNostrStandard::Nip23 => "nip23",
        RadrootsNostrStandard::Nip25 => "nip25",
        RadrootsNostrStandard::Nip28 => "nip28",
        RadrootsNostrStandard::Nip29 => "nip29",
        RadrootsNostrStandard::Nip42 => "nip42",
        RadrootsNostrStandard::Nip51 => "nip51",
        RadrootsNostrStandard::Nip52 => "nip52",
        RadrootsNostrStandard::Nip53 => "nip53",
        RadrootsNostrStandard::Nip54 => "nip54",
        RadrootsNostrStandard::Nip56 => "nip56",
        RadrootsNostrStandard::Nip57 => "nip57",
        RadrootsNostrStandard::Nip78 => "nip78",
        RadrootsNostrStandard::Nip90 => "nip90",
        RadrootsNostrStandard::Nip94 => "nip94",
        RadrootsNostrStandard::Nip98 => "nip98",
        RadrootsNostrStandard::Nip99 => "nip99",
        RadrootsNostrStandard::Radroots => "radroots",
    }
}

fn stability_label(value: RadrootsEventStability) -> &'static str {
    match value {
        RadrootsEventStability::Stable => "stable",
        RadrootsEventStability::Experimental => "experimental",
    }
}

fn privacy_label(value: RadrootsEventPrivacy) -> &'static str {
    match value {
        RadrootsEventPrivacy::Public => "public",
        RadrootsEventPrivacy::Encrypted => "encrypted",
        RadrootsEventPrivacy::LocalOnly => "local_only",
        RadrootsEventPrivacy::Secret => "secret",
    }
}

fn content_schema_label(value: RadrootsContentSchema) -> &'static str {
    match value {
        RadrootsContentSchema::Empty => "empty",
        RadrootsContentSchema::JsonObject => "json_object",
        RadrootsContentSchema::PlainText => "plain_text",
        RadrootsContentSchema::Markdown => "markdown",
        RadrootsContentSchema::Djot => "djot",
        RadrootsContentSchema::Encrypted => "encrypted",
        RadrootsContentSchema::BinaryReference => "binary_reference",
    }
}

fn tag_cardinality_label(value: RadrootsTagCardinality) -> &'static str {
    match value {
        RadrootsTagCardinality::RequiredOne => "required_one",
        RadrootsTagCardinality::OptionalOne => "optional_one",
        RadrootsTagCardinality::OptionalMany => "optional_many",
        RadrootsTagCardinality::RequiredMany => "required_many",
    }
}

fn tag_semantic_label(value: RadrootsTagSemantic) -> &'static str {
    match value {
        RadrootsTagSemantic::AddressableCoordinate => "addressable_coordinate",
        RadrootsTagSemantic::CalendarEventAuthor => "calendar_event_author",
        RadrootsTagSemantic::CalendarEventReference => "calendar_event_reference",
        RadrootsTagSemantic::CalendarEventRevision => "calendar_event_revision",
        RadrootsTagSemantic::CalendarInclusionRequest => "calendar_inclusion_request",
        RadrootsTagSemantic::CalendarEnd => "calendar_end",
        RadrootsTagSemantic::CalendarStart => "calendar_start",
        RadrootsTagSemantic::Category => "category",
        RadrootsTagSemantic::Citation => "citation",
        RadrootsTagSemantic::Contract => "contract",
        RadrootsTagSemantic::Counterparty => "counterparty",
        RadrootsTagSemantic::Evidence => "evidence",
        RadrootsTagSemantic::EventPointer => "event_pointer",
        RadrootsTagSemantic::FreeBusy => "free_busy",
        RadrootsTagSemantic::Geohash => "geohash",
        RadrootsTagSemantic::GroupId => "group_id",
        RadrootsTagSemantic::Identifier => "identifier",
        RadrootsTagSemantic::Image => "image",
        RadrootsTagSemantic::Kind => "kind",
        RadrootsTagSemantic::ClassifiedListingAddress => "listing_address",
        RadrootsTagSemantic::OperationalListingSnapshot => "listing_snapshot",
        RadrootsTagSemantic::ListDescription => "list_description",
        RadrootsTagSemantic::Location => "location",
        RadrootsTagSemantic::Nip01Coordinate => "nip01_coordinate",
        RadrootsTagSemantic::Participant => "participant",
        RadrootsTagSemantic::PreviousEvent => "previous_event",
        RadrootsTagSemantic::Price => "price",
        RadrootsTagSemantic::PublishedAt => "published_at",
        RadrootsTagSemantic::Relay => "relay",
        RadrootsTagSemantic::Reference => "reference",
        RadrootsTagSemantic::ReviewTarget => "review_target",
        RadrootsTagSemantic::RootEvent => "root_event",
        RadrootsTagSemantic::ServiceInput => "service_input",
        RadrootsTagSemantic::ServiceOutput => "service_output",
        RadrootsTagSemantic::Source => "source",
        RadrootsTagSemantic::Status => "status",
        RadrootsTagSemantic::Summary => "summary",
        RadrootsTagSemantic::Title => "title",
        RadrootsTagSemantic::Topic => "topic",
        RadrootsTagSemantic::TimeZone => "time_zone",
        RadrootsTagSemantic::Url => "url",
        RadrootsTagSemantic::UtcDayCoverage => "utc_day_coverage",
    }
}

fn tag_value_type_label(value: RadrootsTagValueType) -> &'static str {
    match value {
        RadrootsTagValueType::AddressableCoordinate => "addressable_coordinate",
        RadrootsTagValueType::CalendarDate => "calendar_date",
        RadrootsTagValueType::CalendarEventCoordinate => "calendar_event_coordinate",
        RadrootsTagValueType::CalendarFreeBusy => "calendar_free_busy",
        RadrootsTagValueType::CalendarRsvpStatus => "calendar_rsvp_status",
        RadrootsTagValueType::CalendarUid => "calendar_uid",
        RadrootsTagValueType::ContractId => "contract_id",
        RadrootsTagValueType::DTag => "d_tag",
        RadrootsTagValueType::EventId => "event_id",
        RadrootsTagValueType::EventPointer => "event_pointer",
        RadrootsTagValueType::Geohash => "geohash",
        RadrootsTagValueType::IanaTimeZoneId => "iana_time_zone_id",
        RadrootsTagValueType::Kind => "kind",
        RadrootsTagValueType::Nip01Coordinate => "nip01_coordinate",
        RadrootsTagValueType::PublicKey => "public_key",
        RadrootsTagValueType::RelayUrl => "relay_url",
        RadrootsTagValueType::Sha256 => "sha256",
        RadrootsTagValueType::Text => "text",
        RadrootsTagValueType::UnixTimestamp => "unix_timestamp",
        RadrootsTagValueType::Uri => "uri",
        RadrootsTagValueType::Url => "url",
        RadrootsTagValueType::UtcDayIndex => "utc_day_index",
        RadrootsTagValueType::Uuid => "uuid",
    }
}

fn reducer_label(value: RadrootsReducer) -> &'static str {
    match value {
        RadrootsReducer::CalendarProjection => "calendar_projection",
        RadrootsReducer::FarmOpsProjection => "farm_ops_projection",
        RadrootsReducer::GroupProjection => "group_projection",
        RadrootsReducer::KnowledgeProjection => "knowledge_projection",
        RadrootsReducer::OperationalListingInventoryAccounting => {
            "operational_listing_inventory_accounting"
        }
        RadrootsReducer::OperationalListingProjection => "operational_listing_projection",
        RadrootsReducer::MarketProjection => "market_projection",
        RadrootsReducer::OrderProjection => "order_projection",
        RadrootsReducer::ProfileProjection => "profile_projection",
        RadrootsReducer::NostrRelayPolicyProjection => "nostr_relay_policy_projection",
        RadrootsReducer::SocialProjection => "social_projection",
        RadrootsReducer::TradeProjection => "trade_projection",
        RadrootsReducer::TradeValidation => "trade_validation",
    }
}

#[cfg(test)]
mod tests {
    use super::{discriminator_manifest, reducer_label, standard_label};
    use radroots_event::{
        contract::{RadrootsEventDiscriminator, RadrootsNostrStandard, RadrootsReducer},
        listing::classified::RadrootsClassifiedListingPartition,
    };

    #[test]
    fn classified_listing_standard_label_is_nip99() {
        assert_eq!(standard_label(RadrootsNostrStandard::Nip99), "nip99");
    }

    #[test]
    fn operational_listing_reducer_labels_are_unambiguous() {
        assert_eq!(
            reducer_label(RadrootsReducer::OperationalListingProjection),
            "operational_listing_projection"
        );
        assert_eq!(
            reducer_label(RadrootsReducer::OperationalListingInventoryAccounting),
            "operational_listing_inventory_accounting"
        );
    }

    #[test]
    fn classified_listing_partition_discriminators_render_exactly() {
        for (partition, expected) in [
            (
                RadrootsClassifiedListingPartition::FocusedFoodAvailability,
                "focused_food_availability",
            ),
            (
                RadrootsClassifiedListingPartition::OperationalListing,
                "operational_listing",
            ),
            (
                RadrootsClassifiedListingPartition::GenericNip99,
                "generic_nip99",
            ),
            (RadrootsClassifiedListingPartition::Ambiguous, "ambiguous"),
        ] {
            let manifest = discriminator_manifest(
                &RadrootsEventDiscriminator::ClassifiedListingPartition(partition),
            );
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
