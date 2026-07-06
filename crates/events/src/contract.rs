#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
use alloc::{borrow::ToOwned, string::String, vec::Vec};

use crate::{
    RadrootsNostrEvent,
    ids::{
        RadrootsAddressableCoordinate, RadrootsDTag, RadrootsEventId, RadrootsPublicKey,
        relay_url_is_valid,
    },
    kinds::*,
};

pub const RADROOTS_EVENT_CONTRACT_REGISTRY_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsEventClass {
    Regular,
    Replaceable,
    Addressable,
    Ephemeral,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsNostrStandard {
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
    Radroots,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsEventPrivacy {
    Public,
    Encrypted,
    LocalOnly,
    Secret,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsEventStability {
    Stable,
    Experimental,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RadrootsActorRole {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsReducer {
    CalendarProjection,
    FarmOpsProjection,
    GroupProjection,
    KnowledgeProjection,
    ListingInventoryAccounting,
    ListingProjection,
    MarketProjection,
    OrderProjection,
    ProfileProjection,
    RelayPolicyProjection,
    SocialProjection,
    TradeValidation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsContentSchema {
    Empty,
    JsonObject,
    PlainText,
    Markdown,
    Djot,
    Encrypted,
    BinaryReference,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsTagCardinality {
    RequiredOne,
    OptionalOne,
    OptionalMany,
    RequiredMany,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsTagSemantic {
    AddressableCoordinate,
    Category,
    Citation,
    Contract,
    Counterparty,
    Evidence,
    EventPointer,
    Geohash,
    GroupId,
    Identifier,
    Image,
    Kind,
    ListingAddress,
    ListingSnapshot,
    Location,
    PreviousEvent,
    Price,
    PublishedAt,
    Relay,
    ReviewTarget,
    RootEvent,
    ServiceInput,
    ServiceOutput,
    Source,
    Status,
    Summary,
    Title,
    Topic,
    Url,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsTagValueType {
    AddressableCoordinate,
    ContractId,
    DTag,
    EventId,
    EventPointer,
    Geohash,
    Kind,
    PublicKey,
    RelayUrl,
    Sha256,
    Text,
    UnixTimestamp,
    Url,
    Uuid,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RadrootsTagContract {
    pub name: &'static str,
    pub cardinality: RadrootsTagCardinality,
    pub semantic: RadrootsTagSemantic,
    pub value_type: RadrootsTagValueType,
    pub relay_indexed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsEventDiscriminator {
    KindOnly,
    DTagExact(&'static str),
    DTagPrefix(&'static str),
    DTagSuffix(&'static str),
    TagEquals {
        name: &'static str,
        value: &'static str,
    },
    ContentJsonFieldEquals {
        field: &'static str,
        value: &'static str,
    },
    EnvelopeType(&'static str),
    Composite(&'static [RadrootsEventDiscriminator]),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsContractMatchError {
    UnsupportedKind(u32),
    UnsupportedShape(u32),
    AmbiguousShape(u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsContractFamily {
    Account,
    Application,
    Calendar,
    Farm,
    Group,
    Http,
    Job,
    Knowledge,
    List,
    Market,
    Message,
    Profile,
    Relay,
    Social,
    Trade,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RadrootsContractFamilyMetadata {
    pub family: RadrootsContractFamily,
    pub id: &'static str,
    pub name: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsContractValidationError {
    UnknownContract {
        contract_id: String,
    },
    ContractMatch {
        error: RadrootsContractMatchError,
    },
    KindMismatch {
        expected: u32,
        actual: u32,
    },
    ContentMustBeEmpty {
        contract_id: &'static str,
    },
    InvalidJsonContent {
        contract_id: &'static str,
    },
    MissingTag {
        contract_id: &'static str,
        name: &'static str,
    },
    TagCardinalityMismatch {
        contract_id: &'static str,
        name: &'static str,
    },
    TagValueMismatch {
        contract_id: &'static str,
        name: &'static str,
        expected: String,
        actual: Option<String>,
    },
    MissingContentField {
        contract_id: &'static str,
        field: &'static str,
    },
    ContentFieldMismatch {
        contract_id: &'static str,
        field: &'static str,
        expected: String,
    },
    ForbiddenContentField {
        contract_id: &'static str,
        field: &'static str,
    },
}

impl RadrootsContractValidationError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::UnknownContract { .. } => "unknown_contract",
            Self::ContractMatch { .. } => "contract_match",
            Self::KindMismatch { .. } => "kind_mismatch",
            Self::ContentMustBeEmpty { .. } => "content_must_be_empty",
            Self::InvalidJsonContent { .. } => "invalid_json_content",
            Self::MissingTag { .. } => "missing_tag",
            Self::TagCardinalityMismatch { .. } => "tag_cardinality_mismatch",
            Self::TagValueMismatch { .. } => "tag_value_mismatch",
            Self::MissingContentField { .. } => "missing_content_field",
            Self::ContentFieldMismatch { .. } => "content_field_mismatch",
            Self::ForbiddenContentField { .. } => "forbidden_content_field",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RadrootsKindContract {
    pub kind: u32,
    pub canonical_constant: &'static str,
    pub name: &'static str,
    pub class: RadrootsEventClass,
    pub standard: RadrootsNostrStandard,
    pub accepted_event_contracts: &'static [&'static str],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RadrootsEventContract {
    pub id: &'static str,
    pub kind: u32,
    pub name: &'static str,
    pub payload_type: &'static str,
    pub class: RadrootsEventClass,
    pub stability: RadrootsEventStability,
    pub privacy: RadrootsEventPrivacy,
    pub author_role: RadrootsActorRole,
    pub content_schema: RadrootsContentSchema,
    pub discriminator: RadrootsEventDiscriminator,
    pub tags: &'static [RadrootsTagContract],
    pub reducers: &'static [RadrootsReducer],
}

static CONTRACT_FAMILIES: &[RadrootsContractFamilyMetadata] = &[
    RadrootsContractFamilyMetadata {
        family: RadrootsContractFamily::Account,
        id: "account",
        name: "Account",
    },
    RadrootsContractFamilyMetadata {
        family: RadrootsContractFamily::Application,
        id: "application",
        name: "Application",
    },
    RadrootsContractFamilyMetadata {
        family: RadrootsContractFamily::Calendar,
        id: "calendar",
        name: "Calendar",
    },
    RadrootsContractFamilyMetadata {
        family: RadrootsContractFamily::Farm,
        id: "farm",
        name: "Farm",
    },
    RadrootsContractFamilyMetadata {
        family: RadrootsContractFamily::Group,
        id: "group",
        name: "Group",
    },
    RadrootsContractFamilyMetadata {
        family: RadrootsContractFamily::Http,
        id: "http",
        name: "HTTP",
    },
    RadrootsContractFamilyMetadata {
        family: RadrootsContractFamily::Job,
        id: "job",
        name: "Job",
    },
    RadrootsContractFamilyMetadata {
        family: RadrootsContractFamily::Knowledge,
        id: "knowledge",
        name: "Knowledge",
    },
    RadrootsContractFamilyMetadata {
        family: RadrootsContractFamily::List,
        id: "list",
        name: "List",
    },
    RadrootsContractFamilyMetadata {
        family: RadrootsContractFamily::Market,
        id: "market",
        name: "Market",
    },
    RadrootsContractFamilyMetadata {
        family: RadrootsContractFamily::Message,
        id: "message",
        name: "Message",
    },
    RadrootsContractFamilyMetadata {
        family: RadrootsContractFamily::Profile,
        id: "profile",
        name: "Profile",
    },
    RadrootsContractFamilyMetadata {
        family: RadrootsContractFamily::Relay,
        id: "relay",
        name: "Relay",
    },
    RadrootsContractFamilyMetadata {
        family: RadrootsContractFamily::Social,
        id: "social",
        name: "Social",
    },
    RadrootsContractFamilyMetadata {
        family: RadrootsContractFamily::Trade,
        id: "trade",
        name: "Trade",
    },
];

const fn tag(
    name: &'static str,
    cardinality: RadrootsTagCardinality,
    semantic: RadrootsTagSemantic,
    value_type: RadrootsTagValueType,
    relay_indexed: bool,
) -> RadrootsTagContract {
    RadrootsTagContract {
        name,
        cardinality,
        semantic,
        value_type,
        relay_indexed,
    }
}

const TAG_D: RadrootsTagContract = tag(
    "d",
    RadrootsTagCardinality::RequiredOne,
    RadrootsTagSemantic::Identifier,
    RadrootsTagValueType::DTag,
    true,
);
const TAG_P_REQUIRED: RadrootsTagContract = tag(
    "p",
    RadrootsTagCardinality::RequiredOne,
    RadrootsTagSemantic::Counterparty,
    RadrootsTagValueType::PublicKey,
    true,
);
const TAG_P_MANY: RadrootsTagContract = tag(
    "p",
    RadrootsTagCardinality::OptionalMany,
    RadrootsTagSemantic::Counterparty,
    RadrootsTagValueType::PublicKey,
    true,
);
const TAG_A_REQUIRED: RadrootsTagContract = tag(
    "a",
    RadrootsTagCardinality::RequiredOne,
    RadrootsTagSemantic::ListingAddress,
    RadrootsTagValueType::AddressableCoordinate,
    true,
);
const TAG_A_ADDRESS_REQUIRED: RadrootsTagContract = tag(
    "a",
    RadrootsTagCardinality::RequiredOne,
    RadrootsTagSemantic::AddressableCoordinate,
    RadrootsTagValueType::AddressableCoordinate,
    true,
);
const TAG_A_OPTIONAL: RadrootsTagContract = tag(
    "a",
    RadrootsTagCardinality::OptionalOne,
    RadrootsTagSemantic::AddressableCoordinate,
    RadrootsTagValueType::AddressableCoordinate,
    true,
);
const TAG_A_MANY: RadrootsTagContract = tag(
    "a",
    RadrootsTagCardinality::OptionalMany,
    RadrootsTagSemantic::AddressableCoordinate,
    RadrootsTagValueType::AddressableCoordinate,
    true,
);
const TAG_E_ROOT: RadrootsTagContract = tag(
    "e",
    RadrootsTagCardinality::RequiredOne,
    RadrootsTagSemantic::RootEvent,
    RadrootsTagValueType::EventId,
    true,
);
const TAG_E_PREVIOUS: RadrootsTagContract = tag(
    "e",
    RadrootsTagCardinality::RequiredOne,
    RadrootsTagSemantic::PreviousEvent,
    RadrootsTagValueType::EventId,
    true,
);
const TAG_E_SOURCE_VERSION: RadrootsTagContract = tag(
    "e",
    RadrootsTagCardinality::RequiredOne,
    RadrootsTagSemantic::Source,
    RadrootsTagValueType::EventId,
    true,
);
const TAG_E_BASE_VERSION: RadrootsTagContract = tag(
    "e",
    RadrootsTagCardinality::OptionalOne,
    RadrootsTagSemantic::PreviousEvent,
    RadrootsTagValueType::EventId,
    true,
);
const TAG_E_MANY: RadrootsTagContract = tag(
    "e",
    RadrootsTagCardinality::OptionalMany,
    RadrootsTagSemantic::EventPointer,
    RadrootsTagValueType::EventId,
    true,
);
const TAG_KIND: RadrootsTagContract = tag(
    "k",
    RadrootsTagCardinality::OptionalOne,
    RadrootsTagSemantic::Kind,
    RadrootsTagValueType::Kind,
    true,
);
const TAG_RELAY: RadrootsTagContract = tag(
    "relay",
    RadrootsTagCardinality::OptionalMany,
    RadrootsTagSemantic::Relay,
    RadrootsTagValueType::RelayUrl,
    false,
);
const TAG_GROUP: RadrootsTagContract = tag(
    "h",
    RadrootsTagCardinality::RequiredOne,
    RadrootsTagSemantic::GroupId,
    RadrootsTagValueType::DTag,
    true,
);
const TAG_TITLE: RadrootsTagContract = tag(
    "title",
    RadrootsTagCardinality::OptionalOne,
    RadrootsTagSemantic::Title,
    RadrootsTagValueType::Text,
    false,
);
const TAG_SUMMARY: RadrootsTagContract = tag(
    "summary",
    RadrootsTagCardinality::OptionalOne,
    RadrootsTagSemantic::Summary,
    RadrootsTagValueType::Text,
    false,
);
const TAG_PUBLISHED_AT: RadrootsTagContract = tag(
    "published_at",
    RadrootsTagCardinality::OptionalOne,
    RadrootsTagSemantic::PublishedAt,
    RadrootsTagValueType::UnixTimestamp,
    false,
);
const TAG_LOCATION: RadrootsTagContract = tag(
    "location",
    RadrootsTagCardinality::OptionalMany,
    RadrootsTagSemantic::Location,
    RadrootsTagValueType::Text,
    false,
);
const TAG_PRICE: RadrootsTagContract = tag(
    "price",
    RadrootsTagCardinality::OptionalMany,
    RadrootsTagSemantic::Price,
    RadrootsTagValueType::Text,
    false,
);
const TAG_STATUS: RadrootsTagContract = tag(
    "status",
    RadrootsTagCardinality::OptionalOne,
    RadrootsTagSemantic::Status,
    RadrootsTagValueType::Text,
    false,
);
const TAG_CATEGORY: RadrootsTagContract = tag(
    "category",
    RadrootsTagCardinality::OptionalMany,
    RadrootsTagSemantic::Category,
    RadrootsTagValueType::Text,
    false,
);
const TAG_IMAGE: RadrootsTagContract = tag(
    "image",
    RadrootsTagCardinality::OptionalMany,
    RadrootsTagSemantic::Image,
    RadrootsTagValueType::Url,
    false,
);
const TAG_LISTING_EVENT: RadrootsTagContract = tag(
    "listing_event",
    RadrootsTagCardinality::RequiredOne,
    RadrootsTagSemantic::ListingSnapshot,
    RadrootsTagValueType::EventId,
    false,
);
const TAG_SERVICE_INPUT: RadrootsTagContract = tag(
    "i",
    RadrootsTagCardinality::RequiredOne,
    RadrootsTagSemantic::ServiceInput,
    RadrootsTagValueType::Text,
    true,
);
const TAG_SERVICE_REQUEST: RadrootsTagContract = tag(
    "request",
    RadrootsTagCardinality::RequiredOne,
    RadrootsTagSemantic::ServiceInput,
    RadrootsTagValueType::EventId,
    false,
);
const TAG_SERVICE_OUTPUT: RadrootsTagContract = tag(
    "output",
    RadrootsTagCardinality::RequiredOne,
    RadrootsTagSemantic::ServiceOutput,
    RadrootsTagValueType::Text,
    false,
);
const TAG_URL: RadrootsTagContract = tag(
    "url",
    RadrootsTagCardinality::OptionalOne,
    RadrootsTagSemantic::Url,
    RadrootsTagValueType::Url,
    false,
);
const TAG_CONTRACT_REQUIRED: RadrootsTagContract = tag(
    "contract",
    RadrootsTagCardinality::RequiredOne,
    RadrootsTagSemantic::Contract,
    RadrootsTagValueType::ContractId,
    false,
);
const TAG_TOPIC_MANY: RadrootsTagContract = tag(
    "t",
    RadrootsTagCardinality::OptionalMany,
    RadrootsTagSemantic::Topic,
    RadrootsTagValueType::Text,
    true,
);
const TAG_GEOHASH_OPTIONAL: RadrootsTagContract = tag(
    "g",
    RadrootsTagCardinality::OptionalOne,
    RadrootsTagSemantic::Geohash,
    RadrootsTagValueType::Geohash,
    true,
);
const TAG_SOURCE_MANY: RadrootsTagContract = tag(
    "source",
    RadrootsTagCardinality::OptionalMany,
    RadrootsTagSemantic::Source,
    RadrootsTagValueType::EventPointer,
    false,
);
const TAG_CITATION_MANY: RadrootsTagContract = tag(
    "citation",
    RadrootsTagCardinality::OptionalMany,
    RadrootsTagSemantic::Citation,
    RadrootsTagValueType::Sha256,
    false,
);
const TAG_REVIEW_TARGET_REQUIRED: RadrootsTagContract = tag(
    "review_target",
    RadrootsTagCardinality::RequiredOne,
    RadrootsTagSemantic::ReviewTarget,
    RadrootsTagValueType::EventPointer,
    false,
);
const TAG_EVIDENCE_MANY: RadrootsTagContract = tag(
    "evidence",
    RadrootsTagCardinality::OptionalMany,
    RadrootsTagSemantic::Evidence,
    RadrootsTagValueType::EventPointer,
    false,
);

const NO_TAGS: &[RadrootsTagContract] = &[];
const D_TAGS: &[RadrootsTagContract] = &[TAG_D];
const P_TAGS: &[RadrootsTagContract] = &[TAG_P_MANY];
const EVENT_POINTER_TAGS: &[RadrootsTagContract] = &[TAG_E_MANY, TAG_P_MANY, TAG_KIND];
const LIST_TAGS: &[RadrootsTagContract] = &[TAG_E_MANY, TAG_A_OPTIONAL, TAG_P_MANY, TAG_RELAY];
const LIST_SET_TAGS: &[RadrootsTagContract] = &[TAG_D, TAG_E_MANY, TAG_A_OPTIONAL, TAG_P_MANY];
const PROFILE_TAGS: &[RadrootsTagContract] = &[TAG_P_MANY];
const GROUP_ACTION_TAGS: &[RadrootsTagContract] = &[TAG_GROUP, TAG_P_MANY, TAG_E_MANY];
const GROUP_STATE_TAGS: &[RadrootsTagContract] = &[TAG_D, TAG_P_MANY, TAG_E_MANY];
const FILE_METADATA_TAGS: &[RadrootsTagContract] = &[TAG_URL, TAG_IMAGE];
const ARTICLE_TAGS: &[RadrootsTagContract] = &[TAG_D, TAG_TITLE, TAG_SUMMARY, TAG_PUBLISHED_AT];
const WIKI_ARTICLE_TAGS: &[RadrootsTagContract] = &[
    TAG_D,
    TAG_TITLE,
    TAG_SUMMARY,
    TAG_PUBLISHED_AT,
    TAG_TOPIC_MANY,
    TAG_SOURCE_MANY,
    TAG_A_MANY,
    TAG_E_MANY,
];
const WIKI_REDIRECT_TAGS: &[RadrootsTagContract] = &[TAG_D, TAG_A_ADDRESS_REQUIRED];
const WIKI_MERGE_REQUEST_TAGS: &[RadrootsTagContract] = &[
    TAG_A_ADDRESS_REQUIRED,
    TAG_P_REQUIRED,
    TAG_E_SOURCE_VERSION,
    TAG_E_BASE_VERSION,
];
const CALENDAR_EVENT_TAGS: &[RadrootsTagContract] =
    &[TAG_D, TAG_TITLE, TAG_LOCATION, TAG_PUBLISHED_AT];
const FARM_TAGS: &[RadrootsTagContract] = &[TAG_D, TAG_TITLE, TAG_LOCATION, TAG_IMAGE];
const LISTING_TAGS: &[RadrootsTagContract] = &[
    TAG_D,
    TAG_TITLE,
    TAG_SUMMARY,
    TAG_PUBLISHED_AT,
    TAG_LOCATION,
    TAG_PRICE,
    TAG_STATUS,
    TAG_CATEGORY,
    TAG_IMAGE,
];
const ORDER_REQUEST_TAGS: &[RadrootsTagContract] =
    &[TAG_D, TAG_P_REQUIRED, TAG_A_REQUIRED, TAG_LISTING_EVENT];
const CHAINED_ORDER_TAGS: &[RadrootsTagContract] = &[
    TAG_D,
    TAG_P_REQUIRED,
    TAG_A_REQUIRED,
    TAG_E_ROOT,
    TAG_E_PREVIOUS,
];
const TRADE_VALIDATION_REQUEST_TAGS: &[RadrootsTagContract] = &[TAG_SERVICE_INPUT, TAG_A_REQUIRED];
const TRADE_VALIDATION_RESULT_TAGS: &[RadrootsTagContract] =
    &[TAG_SERVICE_REQUEST, TAG_SERVICE_OUTPUT];
const TRADE_VALIDATION_RECEIPT_TAGS: &[RadrootsTagContract] =
    &[TAG_E_ROOT, TAG_A_OPTIONAL, TAG_SERVICE_OUTPUT];
const KNOWLEDGE_SOURCE_TAGS: &[RadrootsTagContract] = &[
    TAG_D,
    TAG_CONTRACT_REQUIRED,
    TAG_TOPIC_MANY,
    TAG_SOURCE_MANY,
];
const KNOWLEDGE_CLAIM_TAGS: &[RadrootsTagContract] = &[
    TAG_CONTRACT_REQUIRED,
    TAG_TOPIC_MANY,
    TAG_SOURCE_MANY,
    TAG_CITATION_MANY,
];
const KNOWLEDGE_RELATION_TAGS: &[RadrootsTagContract] =
    &[TAG_CONTRACT_REQUIRED, TAG_TOPIC_MANY, TAG_SOURCE_MANY];
const KNOWLEDGE_REVIEW_TAGS: &[RadrootsTagContract] = &[
    TAG_CONTRACT_REQUIRED,
    TAG_REVIEW_TARGET_REQUIRED,
    TAG_EVIDENCE_MANY,
];
const KNOWLEDGE_FIELD_REPORT_TAGS: &[RadrootsTagContract] = &[
    TAG_CONTRACT_REQUIRED,
    TAG_TOPIC_MANY,
    TAG_GEOHASH_OPTIONAL,
    TAG_EVIDENCE_MANY,
];
const KNOWLEDGE_CHANGE_PROPOSAL_TAGS: &[RadrootsTagContract] =
    &[TAG_CONTRACT_REQUIRED, TAG_EVIDENCE_MANY];
const KNOWLEDGE_CONTRIBUTION_TAGS: &[RadrootsTagContract] =
    &[TAG_CONTRACT_REQUIRED, TAG_EVIDENCE_MANY];
const EVIDENCE_BOUNTY_TAGS: &[RadrootsTagContract] = &[
    TAG_D,
    TAG_CONTRACT_REQUIRED,
    TAG_TOPIC_MANY,
    TAG_EVIDENCE_MANY,
];

const SOCIAL_REDUCERS: &[RadrootsReducer] = &[RadrootsReducer::SocialProjection];
const PROFILE_REDUCERS: &[RadrootsReducer] = &[RadrootsReducer::ProfileProjection];
const FARM_OPS_REDUCERS: &[RadrootsReducer] = &[RadrootsReducer::FarmOpsProjection];
const GROUP_REDUCERS: &[RadrootsReducer] = &[RadrootsReducer::GroupProjection];
const CALENDAR_REDUCERS: &[RadrootsReducer] = &[RadrootsReducer::CalendarProjection];
const LISTING_REDUCERS: &[RadrootsReducer] = &[
    RadrootsReducer::ListingProjection,
    RadrootsReducer::MarketProjection,
    RadrootsReducer::ListingInventoryAccounting,
];
const ORDER_REDUCERS: &[RadrootsReducer] = &[
    RadrootsReducer::OrderProjection,
    RadrootsReducer::ListingInventoryAccounting,
];
const TRADE_VALIDATION_REDUCERS: &[RadrootsReducer] = &[RadrootsReducer::TradeValidation];
const RELAY_REDUCERS: &[RadrootsReducer] = &[RadrootsReducer::RelayPolicyProjection];
const KNOWLEDGE_REDUCERS: &[RadrootsReducer] = &[RadrootsReducer::KnowledgeProjection];

const FARM_MEMBERS_LIST_DISCRIMINATOR: &[RadrootsEventDiscriminator] = &[
    RadrootsEventDiscriminator::DTagPrefix("farm:"),
    RadrootsEventDiscriminator::DTagSuffix(":members"),
];
const FARM_OWNERS_LIST_DISCRIMINATOR: &[RadrootsEventDiscriminator] = &[
    RadrootsEventDiscriminator::DTagPrefix("farm:"),
    RadrootsEventDiscriminator::DTagSuffix(":members.owners"),
];
const FARM_WORKERS_LIST_DISCRIMINATOR: &[RadrootsEventDiscriminator] = &[
    RadrootsEventDiscriminator::DTagPrefix("farm:"),
    RadrootsEventDiscriminator::DTagSuffix(":members.workers"),
];
const FARM_PLOTS_LIST_DISCRIMINATOR: &[RadrootsEventDiscriminator] = &[
    RadrootsEventDiscriminator::DTagPrefix("farm:"),
    RadrootsEventDiscriminator::DTagSuffix(":plots"),
];
const FARM_LISTINGS_LIST_DISCRIMINATOR: &[RadrootsEventDiscriminator] = &[
    RadrootsEventDiscriminator::DTagPrefix("farm:"),
    RadrootsEventDiscriminator::DTagSuffix(":listings"),
];

macro_rules! kind_contract {
    ($kind:expr, $constant:literal, $name:literal, $class:expr, $standard:expr, [$($contract:literal),+ $(,)?]) => {
        RadrootsKindContract {
            kind: $kind,
            canonical_constant: $constant,
            name: $name,
            class: $class,
            standard: $standard,
            accepted_event_contracts: &[$($contract),+],
        }
    };
}

macro_rules! event_contract_with_stability {
    (
        $id:literal,
        $kind:expr,
        $name:literal,
        $payload_type:literal,
        $class:expr,
        $standard_privacy:expr,
        $author_role:expr,
        $content_schema:expr,
        $discriminator:expr,
        $tags:expr,
        $reducers:expr,
        $stability:expr $(,)?
    ) => {
        RadrootsEventContract {
            id: $id,
            kind: $kind,
            name: $name,
            payload_type: $payload_type,
            class: $class,
            stability: $stability,
            privacy: $standard_privacy,
            author_role: $author_role,
            content_schema: $content_schema,
            discriminator: $discriminator,
            tags: $tags,
            reducers: $reducers,
        }
    };
}

macro_rules! event_contract {
    (
        $id:literal,
        $kind:expr,
        $name:literal,
        $payload_type:literal,
        $class:expr,
        $standard_privacy:expr,
        $author_role:expr,
        $content_schema:expr,
        $discriminator:expr,
        $tags:expr,
        $reducers:expr $(,)?
    ) => {
        event_contract_with_stability!(
            $id,
            $kind,
            $name,
            $payload_type,
            $class,
            $standard_privacy,
            $author_role,
            $content_schema,
            $discriminator,
            $tags,
            $reducers,
            RadrootsEventStability::Stable
        )
    };
}

macro_rules! experimental_event_contract {
    (
        $id:literal,
        $kind:expr,
        $name:literal,
        $payload_type:literal,
        $class:expr,
        $standard_privacy:expr,
        $author_role:expr,
        $content_schema:expr,
        $discriminator:expr,
        $tags:expr,
        $reducers:expr $(,)?
    ) => {
        event_contract_with_stability!(
            $id,
            $kind,
            $name,
            $payload_type,
            $class,
            $standard_privacy,
            $author_role,
            $content_schema,
            $discriminator,
            $tags,
            $reducers,
            RadrootsEventStability::Experimental
        )
    };
}

static ALL_KIND_CONTRACTS: &[RadrootsKindContract] = &[
    kind_contract!(
        KIND_PROFILE,
        "KIND_PROFILE",
        "Profile Metadata",
        RadrootsEventClass::Replaceable,
        RadrootsNostrStandard::Nip01,
        ["radroots.profile.metadata.v1"]
    ),
    kind_contract!(
        KIND_POST,
        "KIND_POST",
        "Short Text Note",
        RadrootsEventClass::Regular,
        RadrootsNostrStandard::Nip01,
        ["radroots.social.post.v1"]
    ),
    kind_contract!(
        KIND_FOLLOW,
        "KIND_FOLLOW",
        "Contact List",
        RadrootsEventClass::Replaceable,
        RadrootsNostrStandard::Nip01,
        ["radroots.social.follow_list.v1"]
    ),
    kind_contract!(
        KIND_REPOST,
        "KIND_REPOST",
        "Repost",
        RadrootsEventClass::Regular,
        RadrootsNostrStandard::Nip18,
        ["radroots.social.repost.v1"]
    ),
    kind_contract!(
        KIND_REACTION,
        "KIND_REACTION",
        "Reaction",
        RadrootsEventClass::Regular,
        RadrootsNostrStandard::Nip25,
        ["radroots.social.reaction.v1"]
    ),
    kind_contract!(
        KIND_SEAL,
        "KIND_SEAL",
        "Seal",
        RadrootsEventClass::Regular,
        RadrootsNostrStandard::Nip17,
        ["radroots.message.seal.v1"]
    ),
    kind_contract!(
        KIND_MESSAGE,
        "KIND_MESSAGE",
        "Direct Message",
        RadrootsEventClass::Regular,
        RadrootsNostrStandard::Nip17,
        ["radroots.message.private.v1"]
    ),
    kind_contract!(
        KIND_MESSAGE_FILE,
        "KIND_MESSAGE_FILE",
        "Direct Message File",
        RadrootsEventClass::Regular,
        RadrootsNostrStandard::Nip17,
        ["radroots.message.file.v1"]
    ),
    kind_contract!(
        KIND_GENERIC_REPOST,
        "KIND_GENERIC_REPOST",
        "Generic Repost",
        RadrootsEventClass::Regular,
        RadrootsNostrStandard::Nip18,
        ["radroots.social.generic_repost.v1"]
    ),
    kind_contract!(
        KIND_FARM_CRDT_CHANGE,
        "KIND_FARM_CRDT_CHANGE",
        "Farm CRDT Change",
        RadrootsEventClass::Regular,
        RadrootsNostrStandard::Radroots,
        ["radroots.farm.crdt_change.v1"]
    ),
    kind_contract!(
        KIND_GIFT_WRAP,
        "KIND_GIFT_WRAP",
        "Gift Wrap",
        RadrootsEventClass::Regular,
        RadrootsNostrStandard::Nip17,
        ["radroots.message.gift_wrap.v1"]
    ),
    kind_contract!(
        KIND_FILE_METADATA,
        "KIND_FILE_METADATA",
        "File Metadata",
        RadrootsEventClass::Regular,
        RadrootsNostrStandard::Nip94,
        ["radroots.file.metadata.v1"]
    ),
    kind_contract!(
        KIND_COMMENT,
        "KIND_COMMENT",
        "Comment",
        RadrootsEventClass::Regular,
        RadrootsNostrStandard::Nip22,
        ["radroots.social.comment.v1"]
    ),
    kind_contract!(
        KIND_REPORT,
        "KIND_REPORT",
        "Report",
        RadrootsEventClass::Regular,
        RadrootsNostrStandard::Nip56,
        ["radroots.social.report.v1"]
    ),
    kind_contract!(
        KIND_GROUP_PUT_USER,
        "KIND_GROUP_PUT_USER",
        "Group Put User",
        RadrootsEventClass::Regular,
        RadrootsNostrStandard::Nip29,
        ["radroots.group.put_user.v1"]
    ),
    kind_contract!(
        KIND_GROUP_REMOVE_USER,
        "KIND_GROUP_REMOVE_USER",
        "Group Remove User",
        RadrootsEventClass::Regular,
        RadrootsNostrStandard::Nip29,
        ["radroots.group.remove_user.v1"]
    ),
    kind_contract!(
        KIND_GROUP_EDIT_METADATA,
        "KIND_GROUP_EDIT_METADATA",
        "Group Edit Metadata",
        RadrootsEventClass::Regular,
        RadrootsNostrStandard::Nip29,
        ["radroots.group.edit_metadata.v1"]
    ),
    kind_contract!(
        KIND_GROUP_DELETE_EVENT,
        "KIND_GROUP_DELETE_EVENT",
        "Group Delete Event",
        RadrootsEventClass::Regular,
        RadrootsNostrStandard::Nip29,
        ["radroots.group.delete_event.v1"]
    ),
    kind_contract!(
        KIND_GROUP_CREATE_GROUP,
        "KIND_GROUP_CREATE_GROUP",
        "Group Create Group",
        RadrootsEventClass::Regular,
        RadrootsNostrStandard::Nip29,
        ["radroots.group.create_group.v1"]
    ),
    kind_contract!(
        KIND_GROUP_DELETE_GROUP,
        "KIND_GROUP_DELETE_GROUP",
        "Group Delete Group",
        RadrootsEventClass::Regular,
        RadrootsNostrStandard::Nip29,
        ["radroots.group.delete_group.v1"]
    ),
    kind_contract!(
        KIND_GROUP_CREATE_INVITE,
        "KIND_GROUP_CREATE_INVITE",
        "Group Create Invite",
        RadrootsEventClass::Regular,
        RadrootsNostrStandard::Nip29,
        ["radroots.group.create_invite.v1"]
    ),
    kind_contract!(
        KIND_GROUP_JOIN_REQUEST,
        "KIND_GROUP_JOIN_REQUEST",
        "Group Join Request",
        RadrootsEventClass::Regular,
        RadrootsNostrStandard::Nip29,
        ["radroots.group.join_request.v1"]
    ),
    kind_contract!(
        KIND_GROUP_LEAVE_REQUEST,
        "KIND_GROUP_LEAVE_REQUEST",
        "Group Leave Request",
        RadrootsEventClass::Regular,
        RadrootsNostrStandard::Nip29,
        ["radroots.group.leave_request.v1"]
    ),
    kind_contract!(
        KIND_GEOCHAT,
        "KIND_GEOCHAT",
        "Geochat",
        RadrootsEventClass::Ephemeral,
        RadrootsNostrStandard::Nip28,
        ["radroots.social.geochat.v1"]
    ),
    kind_contract!(
        KIND_RELAY_AUTH,
        "KIND_RELAY_AUTH",
        "Relay Auth",
        RadrootsEventClass::Ephemeral,
        RadrootsNostrStandard::Nip42,
        ["radroots.relay.auth.v1"]
    ),
    kind_contract!(
        KIND_HTTP_AUTH,
        "KIND_HTTP_AUTH",
        "HTTP Auth",
        RadrootsEventClass::Ephemeral,
        RadrootsNostrStandard::Nip98,
        ["radroots.http.auth.v1"]
    ),
    kind_contract!(
        KIND_LIST_MUTE,
        "KIND_LIST_MUTE",
        "Mute List",
        RadrootsEventClass::Replaceable,
        RadrootsNostrStandard::Nip51,
        ["radroots.list.mute.v1"]
    ),
    kind_contract!(
        KIND_LIST_PINNED_NOTES,
        "KIND_LIST_PINNED_NOTES",
        "Pinned Notes List",
        RadrootsEventClass::Replaceable,
        RadrootsNostrStandard::Nip51,
        ["radroots.list.pinned_notes.v1"]
    ),
    kind_contract!(
        KIND_LIST_READ_WRITE_RELAYS,
        "KIND_LIST_READ_WRITE_RELAYS",
        "Read Write Relays List",
        RadrootsEventClass::Replaceable,
        RadrootsNostrStandard::Nip51,
        ["radroots.list.read_write_relays.v1"]
    ),
    kind_contract!(
        KIND_LIST_BOOKMARKS,
        "KIND_LIST_BOOKMARKS",
        "Bookmarks List",
        RadrootsEventClass::Replaceable,
        RadrootsNostrStandard::Nip51,
        ["radroots.list.bookmarks.v1"]
    ),
    kind_contract!(
        KIND_LIST_COMMUNITIES,
        "KIND_LIST_COMMUNITIES",
        "Communities List",
        RadrootsEventClass::Replaceable,
        RadrootsNostrStandard::Nip51,
        ["radroots.list.communities.v1"]
    ),
    kind_contract!(
        KIND_LIST_PUBLIC_CHATS,
        "KIND_LIST_PUBLIC_CHATS",
        "Public Chats List",
        RadrootsEventClass::Replaceable,
        RadrootsNostrStandard::Nip51,
        ["radroots.list.public_chats.v1"]
    ),
    kind_contract!(
        KIND_LIST_BLOCKED_RELAYS,
        "KIND_LIST_BLOCKED_RELAYS",
        "Blocked Relays List",
        RadrootsEventClass::Replaceable,
        RadrootsNostrStandard::Nip51,
        ["radroots.list.blocked_relays.v1"]
    ),
    kind_contract!(
        KIND_LIST_SEARCH_RELAYS,
        "KIND_LIST_SEARCH_RELAYS",
        "Search Relays List",
        RadrootsEventClass::Replaceable,
        RadrootsNostrStandard::Nip51,
        ["radroots.list.search_relays.v1"]
    ),
    kind_contract!(
        KIND_LIST_SIMPLE_GROUPS,
        "KIND_LIST_SIMPLE_GROUPS",
        "Simple Groups List",
        RadrootsEventClass::Replaceable,
        RadrootsNostrStandard::Nip51,
        ["radroots.list.simple_groups.v1"]
    ),
    kind_contract!(
        KIND_LIST_RELAY_FEEDS,
        "KIND_LIST_RELAY_FEEDS",
        "Relay Feeds List",
        RadrootsEventClass::Replaceable,
        RadrootsNostrStandard::Nip51,
        ["radroots.list.relay_feeds.v1"]
    ),
    kind_contract!(
        KIND_LIST_INTERESTS,
        "KIND_LIST_INTERESTS",
        "Interests List",
        RadrootsEventClass::Replaceable,
        RadrootsNostrStandard::Nip51,
        ["radroots.list.interests.v1"]
    ),
    kind_contract!(
        KIND_LIST_MEDIA_FOLLOWS,
        "KIND_LIST_MEDIA_FOLLOWS",
        "Media Follows List",
        RadrootsEventClass::Replaceable,
        RadrootsNostrStandard::Nip51,
        ["radroots.list.media_follows.v1"]
    ),
    kind_contract!(
        KIND_LIST_EMOJIS,
        "KIND_LIST_EMOJIS",
        "Emojis List",
        RadrootsEventClass::Replaceable,
        RadrootsNostrStandard::Nip51,
        ["radroots.list.emojis.v1"]
    ),
    kind_contract!(
        KIND_LIST_DM_RELAYS,
        "KIND_LIST_DM_RELAYS",
        "DM Relays List",
        RadrootsEventClass::Replaceable,
        RadrootsNostrStandard::Nip51,
        ["radroots.list.dm_relays.v1"]
    ),
    kind_contract!(
        KIND_LIST_GOOD_WIKI_AUTHORS,
        "KIND_LIST_GOOD_WIKI_AUTHORS",
        "Good Wiki Authors List",
        RadrootsEventClass::Replaceable,
        RadrootsNostrStandard::Nip51,
        ["radroots.list.good_wiki_authors.v1"]
    ),
    kind_contract!(
        KIND_LIST_GOOD_WIKI_RELAYS,
        "KIND_LIST_GOOD_WIKI_RELAYS",
        "Good Wiki Relays List",
        RadrootsEventClass::Replaceable,
        RadrootsNostrStandard::Nip51,
        ["radroots.list.good_wiki_relays.v1"]
    ),
    kind_contract!(
        KIND_LIST_SET_FOLLOW,
        "KIND_LIST_SET_FOLLOW",
        "Follow Set",
        RadrootsEventClass::Addressable,
        RadrootsNostrStandard::Nip51,
        ["radroots.list_set.follow.v1"]
    ),
    kind_contract!(
        KIND_LIST_SET_GENERIC,
        "KIND_LIST_SET_GENERIC",
        "Generic List Set",
        RadrootsEventClass::Addressable,
        RadrootsNostrStandard::Nip51,
        [
            "radroots.list_set.farm.members.v1",
            "radroots.list_set.farm.members.owners.v1",
            "radroots.list_set.farm.members.workers.v1",
            "radroots.list_set.farm.plots.v1",
            "radroots.list_set.farm.listings.v1",
            "radroots.list_set.member_of.farms.v1"
        ]
    ),
    kind_contract!(
        KIND_LIST_SET_RELAY,
        "KIND_LIST_SET_RELAY",
        "Relay Set",
        RadrootsEventClass::Addressable,
        RadrootsNostrStandard::Nip51,
        ["radroots.list_set.relay.v1"]
    ),
    kind_contract!(
        KIND_LIST_SET_BOOKMARK,
        "KIND_LIST_SET_BOOKMARK",
        "Bookmark Set",
        RadrootsEventClass::Addressable,
        RadrootsNostrStandard::Nip51,
        ["radroots.list_set.bookmark.v1"]
    ),
    kind_contract!(
        KIND_LIST_SET_CURATION,
        "KIND_LIST_SET_CURATION",
        "Curation Set",
        RadrootsEventClass::Addressable,
        RadrootsNostrStandard::Nip51,
        ["radroots.list_set.curation.v1"]
    ),
    kind_contract!(
        KIND_LIST_SET_VIDEO,
        "KIND_LIST_SET_VIDEO",
        "Video Set",
        RadrootsEventClass::Addressable,
        RadrootsNostrStandard::Nip51,
        ["radroots.list_set.video.v1"]
    ),
    kind_contract!(
        KIND_LIST_SET_PICTURE,
        "KIND_LIST_SET_PICTURE",
        "Picture Set",
        RadrootsEventClass::Addressable,
        RadrootsNostrStandard::Nip51,
        ["radroots.list_set.picture.v1"]
    ),
    kind_contract!(
        KIND_LIST_SET_KIND_MUTE,
        "KIND_LIST_SET_KIND_MUTE",
        "Kind Mute Set",
        RadrootsEventClass::Addressable,
        RadrootsNostrStandard::Nip51,
        ["radroots.list_set.kind_mute.v1"]
    ),
    kind_contract!(
        KIND_LIST_SET_INTEREST,
        "KIND_LIST_SET_INTEREST",
        "Interest Set",
        RadrootsEventClass::Addressable,
        RadrootsNostrStandard::Nip51,
        ["radroots.list_set.interest.v1"]
    ),
    kind_contract!(
        KIND_LIST_SET_EMOJI,
        "KIND_LIST_SET_EMOJI",
        "Emoji Set",
        RadrootsEventClass::Addressable,
        RadrootsNostrStandard::Nip51,
        ["radroots.list_set.emoji.v1"]
    ),
    kind_contract!(
        KIND_LIST_SET_RELEASE_ARTIFACT,
        "KIND_LIST_SET_RELEASE_ARTIFACT",
        "Release Artifact Set",
        RadrootsEventClass::Addressable,
        RadrootsNostrStandard::Nip51,
        ["radroots.list_set.release_artifact.v1"]
    ),
    kind_contract!(
        KIND_LIST_SET_APP_CURATION,
        "KIND_LIST_SET_APP_CURATION",
        "App Curation Set",
        RadrootsEventClass::Addressable,
        RadrootsNostrStandard::Nip51,
        ["radroots.list_set.app_curation.v1"]
    ),
    kind_contract!(
        KIND_ARTICLE,
        "KIND_ARTICLE",
        "Long Form Article",
        RadrootsEventClass::Addressable,
        RadrootsNostrStandard::Nip23,
        ["radroots.social.article.v1"]
    ),
    kind_contract!(
        KIND_WIKI_MERGE_REQUEST,
        "KIND_WIKI_MERGE_REQUEST",
        "Wiki Merge Request",
        RadrootsEventClass::Regular,
        RadrootsNostrStandard::Nip54,
        ["radroots.wiki.merge_request.v1"]
    ),
    kind_contract!(
        KIND_WIKI_ARTICLE,
        "KIND_WIKI_ARTICLE",
        "Wiki Article",
        RadrootsEventClass::Addressable,
        RadrootsNostrStandard::Nip54,
        ["radroots.wiki.article.v1"]
    ),
    kind_contract!(
        KIND_WIKI_REDIRECT,
        "KIND_WIKI_REDIRECT",
        "Wiki Redirect",
        RadrootsEventClass::Addressable,
        RadrootsNostrStandard::Nip54,
        ["radroots.wiki.redirect.v1"]
    ),
    kind_contract!(
        KIND_CALENDAR_DATE_EVENT,
        "KIND_CALENDAR_DATE_EVENT",
        "Calendar Date Event",
        RadrootsEventClass::Addressable,
        RadrootsNostrStandard::Nip52,
        ["radroots.calendar.date_event.v1"]
    ),
    kind_contract!(
        KIND_CALENDAR_TIME_EVENT,
        "KIND_CALENDAR_TIME_EVENT",
        "Calendar Time Event",
        RadrootsEventClass::Addressable,
        RadrootsNostrStandard::Nip52,
        ["radroots.calendar.time_event.v1"]
    ),
    kind_contract!(
        KIND_CALENDAR,
        "KIND_CALENDAR",
        "Calendar Collection",
        RadrootsEventClass::Addressable,
        RadrootsNostrStandard::Nip52,
        ["radroots.calendar.collection.v1"]
    ),
    kind_contract!(
        KIND_CALENDAR_EVENT_RSVP,
        "KIND_CALENDAR_EVENT_RSVP",
        "Calendar RSVP",
        RadrootsEventClass::Addressable,
        RadrootsNostrStandard::Nip52,
        ["radroots.calendar.rsvp.v1"]
    ),
    kind_contract!(
        KIND_LIST_SET_STARTER_PACK,
        "KIND_LIST_SET_STARTER_PACK",
        "Starter Pack Set",
        RadrootsEventClass::Addressable,
        RadrootsNostrStandard::Nip51,
        ["radroots.list_set.starter_pack.v1"]
    ),
    kind_contract!(
        KIND_LIST_SET_MEDIA_STARTER_PACK,
        "KIND_LIST_SET_MEDIA_STARTER_PACK",
        "Media Starter Pack Set",
        RadrootsEventClass::Addressable,
        RadrootsNostrStandard::Nip51,
        ["radroots.list_set.media_starter_pack.v1"]
    ),
    kind_contract!(
        KIND_FARM,
        "KIND_FARM",
        "Farm",
        RadrootsEventClass::Addressable,
        RadrootsNostrStandard::Radroots,
        ["radroots.farm.profile.v1"]
    ),
    kind_contract!(
        KIND_PLOT,
        "KIND_PLOT",
        "Plot",
        RadrootsEventClass::Addressable,
        RadrootsNostrStandard::Radroots,
        ["radroots.farm.plot.v1"]
    ),
    kind_contract!(
        KIND_COOP,
        "KIND_COOP",
        "Coop",
        RadrootsEventClass::Addressable,
        RadrootsNostrStandard::Radroots,
        ["radroots.farm.coop.v1"]
    ),
    kind_contract!(
        KIND_DOCUMENT,
        "KIND_DOCUMENT",
        "Document",
        RadrootsEventClass::Addressable,
        RadrootsNostrStandard::Radroots,
        ["radroots.farm.document.v1"]
    ),
    kind_contract!(
        KIND_RESOURCE_AREA,
        "KIND_RESOURCE_AREA",
        "Resource Area",
        RadrootsEventClass::Addressable,
        RadrootsNostrStandard::Radroots,
        ["radroots.farm.resource_area.v1"]
    ),
    kind_contract!(
        KIND_RESOURCE_HARVEST_CAP,
        "KIND_RESOURCE_HARVEST_CAP",
        "Resource Harvest Capacity",
        RadrootsEventClass::Addressable,
        RadrootsNostrStandard::Radroots,
        ["radroots.farm.resource_harvest_cap.v1"]
    ),
    kind_contract!(
        KIND_ACCOUNT_CLAIM,
        "KIND_ACCOUNT_CLAIM",
        "Account Claim",
        RadrootsEventClass::Addressable,
        RadrootsNostrStandard::Radroots,
        ["radroots.account.claim.v1"]
    ),
    kind_contract!(
        KIND_FARM_WORKSPACE_MANIFEST,
        "KIND_FARM_WORKSPACE_MANIFEST",
        "Farm Workspace Manifest",
        RadrootsEventClass::Addressable,
        RadrootsNostrStandard::Nip78,
        ["radroots.farm.workspace_manifest.v1"]
    ),
    kind_contract!(
        KIND_LISTING,
        "KIND_LISTING",
        "Listing",
        RadrootsEventClass::Addressable,
        RadrootsNostrStandard::Radroots,
        ["radroots.listing.published.v1"]
    ),
    kind_contract!(
        KIND_LISTING_DRAFT,
        "KIND_LISTING_DRAFT",
        "Listing Draft",
        RadrootsEventClass::Addressable,
        RadrootsNostrStandard::Radroots,
        ["radroots.listing.draft.v1"]
    ),
    kind_contract!(
        KIND_KNOWLEDGE_SOURCE,
        "KIND_KNOWLEDGE_SOURCE",
        "Knowledge Source",
        RadrootsEventClass::Addressable,
        RadrootsNostrStandard::Radroots,
        ["radroots.knowledge.source.v1"]
    ),
    kind_contract!(
        KIND_EVIDENCE_BOUNTY,
        "KIND_EVIDENCE_BOUNTY",
        "Evidence Bounty",
        RadrootsEventClass::Addressable,
        RadrootsNostrStandard::Radroots,
        ["radroots.knowledge.evidence_bounty.v1"]
    ),
    kind_contract!(
        KIND_KNOWLEDGE_CLAIM,
        "KIND_KNOWLEDGE_CLAIM",
        "Knowledge Claim",
        RadrootsEventClass::Regular,
        RadrootsNostrStandard::Radroots,
        ["radroots.knowledge.claim.v1"]
    ),
    kind_contract!(
        KIND_KNOWLEDGE_RELATION,
        "KIND_KNOWLEDGE_RELATION",
        "Knowledge Relation",
        RadrootsEventClass::Regular,
        RadrootsNostrStandard::Radroots,
        ["radroots.knowledge.relation.v1"]
    ),
    kind_contract!(
        KIND_KNOWLEDGE_REVIEW,
        "KIND_KNOWLEDGE_REVIEW",
        "Knowledge Review",
        RadrootsEventClass::Regular,
        RadrootsNostrStandard::Radroots,
        ["radroots.knowledge.review.v1"]
    ),
    kind_contract!(
        KIND_KNOWLEDGE_FIELD_REPORT,
        "KIND_KNOWLEDGE_FIELD_REPORT",
        "Knowledge Field Report",
        RadrootsEventClass::Regular,
        RadrootsNostrStandard::Radroots,
        ["radroots.knowledge.field_report.v1"]
    ),
    kind_contract!(
        KIND_KNOWLEDGE_CHANGE_PROPOSAL,
        "KIND_KNOWLEDGE_CHANGE_PROPOSAL",
        "Knowledge Change Proposal",
        RadrootsEventClass::Regular,
        RadrootsNostrStandard::Radroots,
        ["radroots.knowledge.change_proposal.v1"]
    ),
    kind_contract!(
        KIND_CONTRIBUTION_ATTESTATION,
        "KIND_CONTRIBUTION_ATTESTATION",
        "Contribution Attestation",
        RadrootsEventClass::Regular,
        RadrootsNostrStandard::Radroots,
        ["radroots.knowledge.contribution_attestation.v1"]
    ),
    kind_contract!(
        KIND_APPLICATION_HANDLER,
        "KIND_APPLICATION_HANDLER",
        "Application Handler",
        RadrootsEventClass::Addressable,
        RadrootsNostrStandard::Radroots,
        ["radroots.application.handler.v1"]
    ),
    kind_contract!(
        KIND_GROUP_METADATA,
        "KIND_GROUP_METADATA",
        "Group Metadata",
        RadrootsEventClass::Addressable,
        RadrootsNostrStandard::Nip29,
        ["radroots.group.metadata.v1"]
    ),
    kind_contract!(
        KIND_GROUP_ADMINS,
        "KIND_GROUP_ADMINS",
        "Group Admins",
        RadrootsEventClass::Addressable,
        RadrootsNostrStandard::Nip29,
        ["radroots.group.admins.v1"]
    ),
    kind_contract!(
        KIND_GROUP_MEMBERS,
        "KIND_GROUP_MEMBERS",
        "Group Members",
        RadrootsEventClass::Addressable,
        RadrootsNostrStandard::Nip29,
        ["radroots.group.members.v1"]
    ),
    kind_contract!(
        KIND_GROUP_ROLES,
        "KIND_GROUP_ROLES",
        "Group Roles",
        RadrootsEventClass::Addressable,
        RadrootsNostrStandard::Nip29,
        ["radroots.group.roles.v1"]
    ),
    kind_contract!(
        KIND_TRADE_LISTING_VALIDATION_REQUEST,
        "KIND_TRADE_LISTING_VALIDATION_REQUEST",
        "Trade Listing Validation Request",
        RadrootsEventClass::Regular,
        RadrootsNostrStandard::Nip90,
        ["radroots.trade.listing_validation.request.v1"]
    ),
    kind_contract!(
        KIND_TRADE_LISTING_VALIDATION_RESULT,
        "KIND_TRADE_LISTING_VALIDATION_RESULT",
        "Trade Listing Validation Result",
        RadrootsEventClass::Regular,
        RadrootsNostrStandard::Nip90,
        ["radroots.trade.listing_validation.result.v1"]
    ),
    kind_contract!(
        KIND_TRADE_TRANSITION_PROOF_REQUEST,
        "KIND_TRADE_TRANSITION_PROOF_REQUEST",
        "Trade Transition Proof Request",
        RadrootsEventClass::Regular,
        RadrootsNostrStandard::Nip90,
        ["radroots.trade.transition_proof.request.v1"]
    ),
    kind_contract!(
        KIND_TRADE_TRANSITION_PROOF_RESULT,
        "KIND_TRADE_TRANSITION_PROOF_RESULT",
        "Trade Transition Proof Result",
        RadrootsEventClass::Regular,
        RadrootsNostrStandard::Nip90,
        ["radroots.trade.transition_proof.result.v1"]
    ),
    kind_contract!(
        KIND_ORDER_REQUEST,
        "KIND_ORDER_REQUEST",
        "Order Request",
        RadrootsEventClass::Regular,
        RadrootsNostrStandard::Radroots,
        ["radroots.order.request.v1"]
    ),
    kind_contract!(
        KIND_ORDER_DECISION,
        "KIND_ORDER_DECISION",
        "Order Decision",
        RadrootsEventClass::Regular,
        RadrootsNostrStandard::Radroots,
        ["radroots.order.decision.v1"]
    ),
    kind_contract!(
        KIND_ORDER_REVISION_PROPOSAL,
        "KIND_ORDER_REVISION_PROPOSAL",
        "Order Revision Proposal",
        RadrootsEventClass::Regular,
        RadrootsNostrStandard::Radroots,
        ["radroots.order.revision_proposal.v1"]
    ),
    kind_contract!(
        KIND_ORDER_REVISION_DECISION,
        "KIND_ORDER_REVISION_DECISION",
        "Order Revision Decision",
        RadrootsEventClass::Regular,
        RadrootsNostrStandard::Radroots,
        ["radroots.order.revision_decision.v1"]
    ),
    kind_contract!(
        KIND_ORDER_CANCELLATION,
        "KIND_ORDER_CANCELLATION",
        "Order Cancellation",
        RadrootsEventClass::Regular,
        RadrootsNostrStandard::Radroots,
        ["radroots.order.cancellation.v1"]
    ),
    kind_contract!(
        KIND_TRADE_VALIDATION_RECEIPT,
        "KIND_TRADE_VALIDATION_RECEIPT",
        "Trade Validation Receipt",
        RadrootsEventClass::Regular,
        RadrootsNostrStandard::Radroots,
        ["radroots.trade.validation_receipt.v1"]
    ),
];

static ALL_EVENT_CONTRACTS: &[RadrootsEventContract] = &[
    event_contract!(
        "radroots.profile.metadata.v1",
        KIND_PROFILE,
        "Profile Metadata",
        "RadrootsProfile",
        RadrootsEventClass::Replaceable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        PROFILE_TAGS,
        PROFILE_REDUCERS
    ),
    event_contract!(
        "radroots.social.post.v1",
        KIND_POST,
        "Short Text Note",
        "RadrootsPost",
        RadrootsEventClass::Regular,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::PlainText,
        RadrootsEventDiscriminator::KindOnly,
        NO_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.social.follow_list.v1",
        KIND_FOLLOW,
        "Contact List",
        "RadrootsFollowList",
        RadrootsEventClass::Replaceable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        P_TAGS,
        PROFILE_REDUCERS
    ),
    event_contract!(
        "radroots.social.repost.v1",
        KIND_REPOST,
        "Repost",
        "RadrootsRepost",
        RadrootsEventClass::Regular,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        EVENT_POINTER_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.social.reaction.v1",
        KIND_REACTION,
        "Reaction",
        "RadrootsReaction",
        RadrootsEventClass::Regular,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::PlainText,
        RadrootsEventDiscriminator::KindOnly,
        EVENT_POINTER_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.message.seal.v1",
        KIND_SEAL,
        "Seal",
        "RadrootsSeal",
        RadrootsEventClass::Regular,
        RadrootsEventPrivacy::Encrypted,
        RadrootsActorRole::Any,
        RadrootsContentSchema::Encrypted,
        RadrootsEventDiscriminator::KindOnly,
        NO_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.message.private.v1",
        KIND_MESSAGE,
        "Direct Message",
        "RadrootsMessage",
        RadrootsEventClass::Regular,
        RadrootsEventPrivacy::Encrypted,
        RadrootsActorRole::Any,
        RadrootsContentSchema::Encrypted,
        RadrootsEventDiscriminator::KindOnly,
        P_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.message.file.v1",
        KIND_MESSAGE_FILE,
        "Direct Message File",
        "RadrootsMessageFile",
        RadrootsEventClass::Regular,
        RadrootsEventPrivacy::Encrypted,
        RadrootsActorRole::Any,
        RadrootsContentSchema::Encrypted,
        RadrootsEventDiscriminator::KindOnly,
        P_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.social.generic_repost.v1",
        KIND_GENERIC_REPOST,
        "Generic Repost",
        "RadrootsGenericRepost",
        RadrootsEventClass::Regular,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        EVENT_POINTER_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.farm.crdt_change.v1",
        KIND_FARM_CRDT_CHANGE,
        "Farm CRDT Change",
        "RadrootsFarmCrdtChange",
        RadrootsEventClass::Regular,
        RadrootsEventPrivacy::Encrypted,
        RadrootsActorRole::Farmer,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        NO_TAGS,
        FARM_OPS_REDUCERS
    ),
    event_contract!(
        "radroots.message.gift_wrap.v1",
        KIND_GIFT_WRAP,
        "Gift Wrap",
        "RadrootsGiftWrap",
        RadrootsEventClass::Regular,
        RadrootsEventPrivacy::Encrypted,
        RadrootsActorRole::Any,
        RadrootsContentSchema::Encrypted,
        RadrootsEventDiscriminator::KindOnly,
        P_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.file.metadata.v1",
        KIND_FILE_METADATA,
        "File Metadata",
        "RadrootsFileMetadata",
        RadrootsEventClass::Regular,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        FILE_METADATA_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.social.comment.v1",
        KIND_COMMENT,
        "Comment",
        "RadrootsComment",
        RadrootsEventClass::Regular,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::PlainText,
        RadrootsEventDiscriminator::KindOnly,
        EVENT_POINTER_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.social.report.v1",
        KIND_REPORT,
        "Report",
        "RadrootsReport",
        RadrootsEventClass::Regular,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Moderator,
        RadrootsContentSchema::PlainText,
        RadrootsEventDiscriminator::KindOnly,
        EVENT_POINTER_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.group.put_user.v1",
        KIND_GROUP_PUT_USER,
        "Group Put User",
        "RadrootsGroupPutUser",
        RadrootsEventClass::Regular,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Moderator,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        GROUP_ACTION_TAGS,
        GROUP_REDUCERS
    ),
    event_contract!(
        "radroots.group.remove_user.v1",
        KIND_GROUP_REMOVE_USER,
        "Group Remove User",
        "RadrootsGroupRemoveUser",
        RadrootsEventClass::Regular,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Moderator,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        GROUP_ACTION_TAGS,
        GROUP_REDUCERS
    ),
    event_contract!(
        "radroots.group.edit_metadata.v1",
        KIND_GROUP_EDIT_METADATA,
        "Group Edit Metadata",
        "RadrootsGroupEditMetadata",
        RadrootsEventClass::Regular,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Moderator,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        GROUP_ACTION_TAGS,
        GROUP_REDUCERS
    ),
    event_contract!(
        "radroots.group.delete_event.v1",
        KIND_GROUP_DELETE_EVENT,
        "Group Delete Event",
        "RadrootsGroupDeleteEvent",
        RadrootsEventClass::Regular,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Moderator,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        GROUP_ACTION_TAGS,
        GROUP_REDUCERS
    ),
    event_contract!(
        "radroots.group.create_group.v1",
        KIND_GROUP_CREATE_GROUP,
        "Group Create Group",
        "RadrootsGroupCreateGroup",
        RadrootsEventClass::Regular,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Moderator,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        GROUP_ACTION_TAGS,
        GROUP_REDUCERS
    ),
    event_contract!(
        "radroots.group.delete_group.v1",
        KIND_GROUP_DELETE_GROUP,
        "Group Delete Group",
        "RadrootsGroupDeleteGroup",
        RadrootsEventClass::Regular,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Moderator,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        GROUP_ACTION_TAGS,
        GROUP_REDUCERS
    ),
    event_contract!(
        "radroots.group.create_invite.v1",
        KIND_GROUP_CREATE_INVITE,
        "Group Create Invite",
        "RadrootsGroupCreateInvite",
        RadrootsEventClass::Regular,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Moderator,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        GROUP_ACTION_TAGS,
        GROUP_REDUCERS
    ),
    event_contract!(
        "radroots.group.join_request.v1",
        KIND_GROUP_JOIN_REQUEST,
        "Group Join Request",
        "RadrootsGroupJoinRequest",
        RadrootsEventClass::Regular,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Member,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        GROUP_ACTION_TAGS,
        GROUP_REDUCERS
    ),
    event_contract!(
        "radroots.group.leave_request.v1",
        KIND_GROUP_LEAVE_REQUEST,
        "Group Leave Request",
        "RadrootsGroupLeaveRequest",
        RadrootsEventClass::Regular,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Member,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        GROUP_ACTION_TAGS,
        GROUP_REDUCERS
    ),
    event_contract!(
        "radroots.social.geochat.v1",
        KIND_GEOCHAT,
        "Geochat",
        "RadrootsGeochat",
        RadrootsEventClass::Ephemeral,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::PlainText,
        RadrootsEventDiscriminator::KindOnly,
        NO_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.relay.auth.v1",
        KIND_RELAY_AUTH,
        "Relay Auth",
        "RadrootsRelayAuth",
        RadrootsEventClass::Ephemeral,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Relay,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        NO_TAGS,
        RELAY_REDUCERS
    ),
    event_contract!(
        "radroots.http.auth.v1",
        KIND_HTTP_AUTH,
        "HTTP Auth",
        "RadrootsHttpAuth",
        RadrootsEventClass::Ephemeral,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Application,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        NO_TAGS,
        RELAY_REDUCERS
    ),
    event_contract!(
        "radroots.list.mute.v1",
        KIND_LIST_MUTE,
        "Mute List",
        "RadrootsList",
        RadrootsEventClass::Replaceable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        LIST_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.list.pinned_notes.v1",
        KIND_LIST_PINNED_NOTES,
        "Pinned Notes List",
        "RadrootsList",
        RadrootsEventClass::Replaceable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        LIST_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.list.read_write_relays.v1",
        KIND_LIST_READ_WRITE_RELAYS,
        "Read Write Relays List",
        "RadrootsList",
        RadrootsEventClass::Replaceable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        LIST_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.list.bookmarks.v1",
        KIND_LIST_BOOKMARKS,
        "Bookmarks List",
        "RadrootsList",
        RadrootsEventClass::Replaceable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        LIST_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.list.communities.v1",
        KIND_LIST_COMMUNITIES,
        "Communities List",
        "RadrootsList",
        RadrootsEventClass::Replaceable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        LIST_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.list.public_chats.v1",
        KIND_LIST_PUBLIC_CHATS,
        "Public Chats List",
        "RadrootsList",
        RadrootsEventClass::Replaceable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        LIST_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.list.blocked_relays.v1",
        KIND_LIST_BLOCKED_RELAYS,
        "Blocked Relays List",
        "RadrootsList",
        RadrootsEventClass::Replaceable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        LIST_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.list.search_relays.v1",
        KIND_LIST_SEARCH_RELAYS,
        "Search Relays List",
        "RadrootsList",
        RadrootsEventClass::Replaceable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        LIST_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.list.simple_groups.v1",
        KIND_LIST_SIMPLE_GROUPS,
        "Simple Groups List",
        "RadrootsList",
        RadrootsEventClass::Replaceable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        LIST_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.list.relay_feeds.v1",
        KIND_LIST_RELAY_FEEDS,
        "Relay Feeds List",
        "RadrootsList",
        RadrootsEventClass::Replaceable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        LIST_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.list.interests.v1",
        KIND_LIST_INTERESTS,
        "Interests List",
        "RadrootsList",
        RadrootsEventClass::Replaceable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        LIST_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.list.media_follows.v1",
        KIND_LIST_MEDIA_FOLLOWS,
        "Media Follows List",
        "RadrootsList",
        RadrootsEventClass::Replaceable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        LIST_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.list.emojis.v1",
        KIND_LIST_EMOJIS,
        "Emojis List",
        "RadrootsList",
        RadrootsEventClass::Replaceable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        LIST_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.list.dm_relays.v1",
        KIND_LIST_DM_RELAYS,
        "DM Relays List",
        "RadrootsList",
        RadrootsEventClass::Replaceable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        LIST_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.list.good_wiki_authors.v1",
        KIND_LIST_GOOD_WIKI_AUTHORS,
        "Good Wiki Authors List",
        "RadrootsList",
        RadrootsEventClass::Replaceable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        LIST_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.list.good_wiki_relays.v1",
        KIND_LIST_GOOD_WIKI_RELAYS,
        "Good Wiki Relays List",
        "RadrootsList",
        RadrootsEventClass::Replaceable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        LIST_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.list_set.follow.v1",
        KIND_LIST_SET_FOLLOW,
        "Follow Set",
        "RadrootsListSet",
        RadrootsEventClass::Addressable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        LIST_SET_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.list_set.farm.members.v1",
        KIND_LIST_SET_GENERIC,
        "Farm Members List Set",
        "RadrootsListSet",
        RadrootsEventClass::Addressable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Farmer,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::Composite(FARM_MEMBERS_LIST_DISCRIMINATOR),
        LIST_SET_TAGS,
        FARM_OPS_REDUCERS
    ),
    event_contract!(
        "radroots.list_set.farm.members.owners.v1",
        KIND_LIST_SET_GENERIC,
        "Farm Owners List Set",
        "RadrootsListSet",
        RadrootsEventClass::Addressable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Farmer,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::Composite(FARM_OWNERS_LIST_DISCRIMINATOR),
        LIST_SET_TAGS,
        FARM_OPS_REDUCERS
    ),
    event_contract!(
        "radroots.list_set.farm.members.workers.v1",
        KIND_LIST_SET_GENERIC,
        "Farm Workers List Set",
        "RadrootsListSet",
        RadrootsEventClass::Addressable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Farmer,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::Composite(FARM_WORKERS_LIST_DISCRIMINATOR),
        LIST_SET_TAGS,
        FARM_OPS_REDUCERS
    ),
    event_contract!(
        "radroots.list_set.farm.plots.v1",
        KIND_LIST_SET_GENERIC,
        "Farm Plots List Set",
        "RadrootsListSet",
        RadrootsEventClass::Addressable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Farmer,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::Composite(FARM_PLOTS_LIST_DISCRIMINATOR),
        LIST_SET_TAGS,
        FARM_OPS_REDUCERS
    ),
    event_contract!(
        "radroots.list_set.farm.listings.v1",
        KIND_LIST_SET_GENERIC,
        "Farm Listings List Set",
        "RadrootsListSet",
        RadrootsEventClass::Addressable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Farmer,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::Composite(FARM_LISTINGS_LIST_DISCRIMINATOR),
        LIST_SET_TAGS,
        FARM_OPS_REDUCERS
    ),
    event_contract!(
        "radroots.list_set.member_of.farms.v1",
        KIND_LIST_SET_GENERIC,
        "Member Of Farms List Set",
        "RadrootsListSet",
        RadrootsEventClass::Addressable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Member,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::DTagExact("member_of.farms"),
        LIST_SET_TAGS,
        FARM_OPS_REDUCERS
    ),
    event_contract!(
        "radroots.list_set.relay.v1",
        KIND_LIST_SET_RELAY,
        "Relay Set",
        "RadrootsListSet",
        RadrootsEventClass::Addressable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        LIST_SET_TAGS,
        RELAY_REDUCERS
    ),
    event_contract!(
        "radroots.list_set.bookmark.v1",
        KIND_LIST_SET_BOOKMARK,
        "Bookmark Set",
        "RadrootsListSet",
        RadrootsEventClass::Addressable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        LIST_SET_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.list_set.curation.v1",
        KIND_LIST_SET_CURATION,
        "Curation Set",
        "RadrootsListSet",
        RadrootsEventClass::Addressable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        LIST_SET_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.list_set.video.v1",
        KIND_LIST_SET_VIDEO,
        "Video Set",
        "RadrootsListSet",
        RadrootsEventClass::Addressable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        LIST_SET_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.list_set.picture.v1",
        KIND_LIST_SET_PICTURE,
        "Picture Set",
        "RadrootsListSet",
        RadrootsEventClass::Addressable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        LIST_SET_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.list_set.kind_mute.v1",
        KIND_LIST_SET_KIND_MUTE,
        "Kind Mute Set",
        "RadrootsListSet",
        RadrootsEventClass::Addressable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        LIST_SET_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.list_set.interest.v1",
        KIND_LIST_SET_INTEREST,
        "Interest Set",
        "RadrootsListSet",
        RadrootsEventClass::Addressable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        LIST_SET_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.list_set.emoji.v1",
        KIND_LIST_SET_EMOJI,
        "Emoji Set",
        "RadrootsListSet",
        RadrootsEventClass::Addressable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        LIST_SET_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.list_set.release_artifact.v1",
        KIND_LIST_SET_RELEASE_ARTIFACT,
        "Release Artifact Set",
        "RadrootsListSet",
        RadrootsEventClass::Addressable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        LIST_SET_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.list_set.app_curation.v1",
        KIND_LIST_SET_APP_CURATION,
        "App Curation Set",
        "RadrootsListSet",
        RadrootsEventClass::Addressable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        LIST_SET_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.social.article.v1",
        KIND_ARTICLE,
        "Long Form Article",
        "RadrootsArticle",
        RadrootsEventClass::Addressable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::Markdown,
        RadrootsEventDiscriminator::KindOnly,
        ARTICLE_TAGS,
        SOCIAL_REDUCERS
    ),
    experimental_event_contract!(
        "radroots.wiki.merge_request.v1",
        KIND_WIKI_MERGE_REQUEST,
        "Wiki Merge Request",
        "RadrootsWikiMergeRequest",
        RadrootsEventClass::Regular,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::PlainText,
        RadrootsEventDiscriminator::KindOnly,
        WIKI_MERGE_REQUEST_TAGS,
        KNOWLEDGE_REDUCERS
    ),
    experimental_event_contract!(
        "radroots.wiki.article.v1",
        KIND_WIKI_ARTICLE,
        "Wiki Article",
        "RadrootsWikiArticle",
        RadrootsEventClass::Addressable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::Djot,
        RadrootsEventDiscriminator::KindOnly,
        WIKI_ARTICLE_TAGS,
        KNOWLEDGE_REDUCERS
    ),
    experimental_event_contract!(
        "radroots.wiki.redirect.v1",
        KIND_WIKI_REDIRECT,
        "Wiki Redirect",
        "RadrootsWikiRedirect",
        RadrootsEventClass::Addressable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::Empty,
        RadrootsEventDiscriminator::KindOnly,
        WIKI_REDIRECT_TAGS,
        KNOWLEDGE_REDUCERS
    ),
    event_contract!(
        "radroots.calendar.date_event.v1",
        KIND_CALENDAR_DATE_EVENT,
        "Calendar Date Event",
        "RadrootsCalendarDateEvent",
        RadrootsEventClass::Addressable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        CALENDAR_EVENT_TAGS,
        CALENDAR_REDUCERS
    ),
    event_contract!(
        "radroots.calendar.time_event.v1",
        KIND_CALENDAR_TIME_EVENT,
        "Calendar Time Event",
        "RadrootsCalendarTimeEvent",
        RadrootsEventClass::Addressable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        CALENDAR_EVENT_TAGS,
        CALENDAR_REDUCERS
    ),
    event_contract!(
        "radroots.calendar.collection.v1",
        KIND_CALENDAR,
        "Calendar Collection",
        "RadrootsCalendar",
        RadrootsEventClass::Addressable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        LIST_SET_TAGS,
        CALENDAR_REDUCERS
    ),
    event_contract!(
        "radroots.calendar.rsvp.v1",
        KIND_CALENDAR_EVENT_RSVP,
        "Calendar RSVP",
        "RadrootsCalendarRsvp",
        RadrootsEventClass::Addressable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        CALENDAR_EVENT_TAGS,
        CALENDAR_REDUCERS
    ),
    event_contract!(
        "radroots.list_set.starter_pack.v1",
        KIND_LIST_SET_STARTER_PACK,
        "Starter Pack Set",
        "RadrootsListSet",
        RadrootsEventClass::Addressable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        LIST_SET_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.list_set.media_starter_pack.v1",
        KIND_LIST_SET_MEDIA_STARTER_PACK,
        "Media Starter Pack Set",
        "RadrootsListSet",
        RadrootsEventClass::Addressable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        LIST_SET_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.farm.profile.v1",
        KIND_FARM,
        "Farm",
        "RadrootsFarm",
        RadrootsEventClass::Addressable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Farmer,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        FARM_TAGS,
        FARM_OPS_REDUCERS
    ),
    event_contract!(
        "radroots.farm.plot.v1",
        KIND_PLOT,
        "Plot",
        "RadrootsPlot",
        RadrootsEventClass::Addressable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Farmer,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        FARM_TAGS,
        FARM_OPS_REDUCERS
    ),
    event_contract!(
        "radroots.farm.coop.v1",
        KIND_COOP,
        "Coop",
        "RadrootsCoop",
        RadrootsEventClass::Addressable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Farmer,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        FARM_TAGS,
        FARM_OPS_REDUCERS
    ),
    event_contract!(
        "radroots.farm.document.v1",
        KIND_DOCUMENT,
        "Document",
        "RadrootsDocument",
        RadrootsEventClass::Addressable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Farmer,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        D_TAGS,
        FARM_OPS_REDUCERS
    ),
    event_contract!(
        "radroots.farm.resource_area.v1",
        KIND_RESOURCE_AREA,
        "Resource Area",
        "RadrootsResourceArea",
        RadrootsEventClass::Addressable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Farmer,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        FARM_TAGS,
        FARM_OPS_REDUCERS
    ),
    event_contract!(
        "radroots.farm.resource_harvest_cap.v1",
        KIND_RESOURCE_HARVEST_CAP,
        "Resource Harvest Capacity",
        "RadrootsResourceHarvestCap",
        RadrootsEventClass::Addressable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Farmer,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        FARM_TAGS,
        FARM_OPS_REDUCERS
    ),
    event_contract!(
        "radroots.account.claim.v1",
        KIND_ACCOUNT_CLAIM,
        "Account Claim",
        "RadrootsAccountClaim",
        RadrootsEventClass::Addressable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        D_TAGS,
        PROFILE_REDUCERS
    ),
    event_contract!(
        "radroots.farm.workspace_manifest.v1",
        KIND_FARM_WORKSPACE_MANIFEST,
        "Farm Workspace Manifest",
        "RadrootsFarmWorkspaceManifest",
        RadrootsEventClass::Addressable,
        RadrootsEventPrivacy::Encrypted,
        RadrootsActorRole::Farmer,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        D_TAGS,
        FARM_OPS_REDUCERS
    ),
    event_contract!(
        "radroots.listing.published.v1",
        KIND_LISTING,
        "Listing",
        "RadrootsListing",
        RadrootsEventClass::Addressable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Seller,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        LISTING_TAGS,
        LISTING_REDUCERS
    ),
    event_contract!(
        "radroots.listing.draft.v1",
        KIND_LISTING_DRAFT,
        "Listing Draft",
        "RadrootsListing",
        RadrootsEventClass::Addressable,
        RadrootsEventPrivacy::Secret,
        RadrootsActorRole::Seller,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        LISTING_TAGS,
        LISTING_REDUCERS
    ),
    experimental_event_contract!(
        "radroots.knowledge.source.v1",
        KIND_KNOWLEDGE_SOURCE,
        "Knowledge Source",
        "RadrootsKnowledgeSource",
        RadrootsEventClass::Addressable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::TagEquals {
            name: "contract",
            value: "radroots.knowledge.source.v1",
        },
        KNOWLEDGE_SOURCE_TAGS,
        KNOWLEDGE_REDUCERS
    ),
    experimental_event_contract!(
        "radroots.knowledge.evidence_bounty.v1",
        KIND_EVIDENCE_BOUNTY,
        "Evidence Bounty",
        "RadrootsEvidenceBounty",
        RadrootsEventClass::Addressable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::TagEquals {
            name: "contract",
            value: "radroots.knowledge.evidence_bounty.v1",
        },
        EVIDENCE_BOUNTY_TAGS,
        KNOWLEDGE_REDUCERS
    ),
    experimental_event_contract!(
        "radroots.knowledge.claim.v1",
        KIND_KNOWLEDGE_CLAIM,
        "Knowledge Claim",
        "RadrootsKnowledgeClaim",
        RadrootsEventClass::Regular,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::TagEquals {
            name: "contract",
            value: "radroots.knowledge.claim.v1",
        },
        KNOWLEDGE_CLAIM_TAGS,
        KNOWLEDGE_REDUCERS
    ),
    experimental_event_contract!(
        "radroots.knowledge.relation.v1",
        KIND_KNOWLEDGE_RELATION,
        "Knowledge Relation",
        "RadrootsKnowledgeRelation",
        RadrootsEventClass::Regular,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::TagEquals {
            name: "contract",
            value: "radroots.knowledge.relation.v1",
        },
        KNOWLEDGE_RELATION_TAGS,
        KNOWLEDGE_REDUCERS
    ),
    experimental_event_contract!(
        "radroots.knowledge.review.v1",
        KIND_KNOWLEDGE_REVIEW,
        "Knowledge Review",
        "RadrootsKnowledgeReview",
        RadrootsEventClass::Regular,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::TagEquals {
            name: "contract",
            value: "radroots.knowledge.review.v1",
        },
        KNOWLEDGE_REVIEW_TAGS,
        KNOWLEDGE_REDUCERS
    ),
    experimental_event_contract!(
        "radroots.knowledge.field_report.v1",
        KIND_KNOWLEDGE_FIELD_REPORT,
        "Knowledge Field Report",
        "RadrootsKnowledgeFieldReport",
        RadrootsEventClass::Regular,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::TagEquals {
            name: "contract",
            value: "radroots.knowledge.field_report.v1",
        },
        KNOWLEDGE_FIELD_REPORT_TAGS,
        KNOWLEDGE_REDUCERS
    ),
    experimental_event_contract!(
        "radroots.knowledge.change_proposal.v1",
        KIND_KNOWLEDGE_CHANGE_PROPOSAL,
        "Knowledge Change Proposal",
        "RadrootsKnowledgeChangeProposal",
        RadrootsEventClass::Regular,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::TagEquals {
            name: "contract",
            value: "radroots.knowledge.change_proposal.v1",
        },
        KNOWLEDGE_CHANGE_PROPOSAL_TAGS,
        KNOWLEDGE_REDUCERS
    ),
    experimental_event_contract!(
        "radroots.knowledge.contribution_attestation.v1",
        KIND_CONTRIBUTION_ATTESTATION,
        "Contribution Attestation",
        "RadrootsContributionAttestation",
        RadrootsEventClass::Regular,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::TagEquals {
            name: "contract",
            value: "radroots.knowledge.contribution_attestation.v1",
        },
        KNOWLEDGE_CONTRIBUTION_TAGS,
        KNOWLEDGE_REDUCERS
    ),
    event_contract!(
        "radroots.application.handler.v1",
        KIND_APPLICATION_HANDLER,
        "Application Handler",
        "RadrootsApplicationHandler",
        RadrootsEventClass::Addressable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Application,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        D_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.group.metadata.v1",
        KIND_GROUP_METADATA,
        "Group Metadata",
        "RadrootsGroupMetadata",
        RadrootsEventClass::Addressable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Moderator,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        GROUP_STATE_TAGS,
        GROUP_REDUCERS
    ),
    event_contract!(
        "radroots.group.admins.v1",
        KIND_GROUP_ADMINS,
        "Group Admins",
        "RadrootsGroupAdmins",
        RadrootsEventClass::Addressable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Moderator,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        GROUP_STATE_TAGS,
        GROUP_REDUCERS
    ),
    event_contract!(
        "radroots.group.members.v1",
        KIND_GROUP_MEMBERS,
        "Group Members",
        "RadrootsGroupMembers",
        RadrootsEventClass::Addressable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Moderator,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        GROUP_STATE_TAGS,
        GROUP_REDUCERS
    ),
    event_contract!(
        "radroots.group.roles.v1",
        KIND_GROUP_ROLES,
        "Group Roles",
        "RadrootsGroupRoles",
        RadrootsEventClass::Addressable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Moderator,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        GROUP_STATE_TAGS,
        GROUP_REDUCERS
    ),
    event_contract!(
        "radroots.trade.listing_validation.request.v1",
        KIND_TRADE_LISTING_VALIDATION_REQUEST,
        "Trade Listing Validation Request",
        "RadrootsTradeValidationListingRequest",
        RadrootsEventClass::Regular,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Service,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        TRADE_VALIDATION_REQUEST_TAGS,
        TRADE_VALIDATION_REDUCERS
    ),
    event_contract!(
        "radroots.trade.listing_validation.result.v1",
        KIND_TRADE_LISTING_VALIDATION_RESULT,
        "Trade Listing Validation Result",
        "RadrootsTradeValidationListingResult",
        RadrootsEventClass::Regular,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Service,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        TRADE_VALIDATION_RESULT_TAGS,
        TRADE_VALIDATION_REDUCERS
    ),
    event_contract!(
        "radroots.trade.transition_proof.request.v1",
        KIND_TRADE_TRANSITION_PROOF_REQUEST,
        "Trade Transition Proof Request",
        "RadrootsTradeTransitionProofRequest",
        RadrootsEventClass::Regular,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Service,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        TRADE_VALIDATION_REQUEST_TAGS,
        TRADE_VALIDATION_REDUCERS
    ),
    event_contract!(
        "radroots.trade.transition_proof.result.v1",
        KIND_TRADE_TRANSITION_PROOF_RESULT,
        "Trade Transition Proof Result",
        "RadrootsTradeTransitionProofResult",
        RadrootsEventClass::Regular,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Service,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        TRADE_VALIDATION_RESULT_TAGS,
        TRADE_VALIDATION_REDUCERS
    ),
    event_contract!(
        "radroots.order.request.v1",
        KIND_ORDER_REQUEST,
        "Order Request",
        "RadrootsOrderRequest",
        RadrootsEventClass::Regular,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Buyer,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        ORDER_REQUEST_TAGS,
        ORDER_REDUCERS
    ),
    event_contract!(
        "radroots.order.decision.v1",
        KIND_ORDER_DECISION,
        "Order Decision",
        "RadrootsOrderDecision",
        RadrootsEventClass::Regular,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Seller,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        CHAINED_ORDER_TAGS,
        ORDER_REDUCERS
    ),
    event_contract!(
        "radroots.order.revision_proposal.v1",
        KIND_ORDER_REVISION_PROPOSAL,
        "Order Revision Proposal",
        "RadrootsOrderRevisionProposal",
        RadrootsEventClass::Regular,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Seller,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        CHAINED_ORDER_TAGS,
        ORDER_REDUCERS
    ),
    event_contract!(
        "radroots.order.revision_decision.v1",
        KIND_ORDER_REVISION_DECISION,
        "Order Revision Decision",
        "RadrootsOrderRevisionDecision",
        RadrootsEventClass::Regular,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Buyer,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        CHAINED_ORDER_TAGS,
        ORDER_REDUCERS
    ),
    event_contract!(
        "radroots.order.cancellation.v1",
        KIND_ORDER_CANCELLATION,
        "Order Cancellation",
        "RadrootsOrderCancellation",
        RadrootsEventClass::Regular,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Buyer,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        CHAINED_ORDER_TAGS,
        ORDER_REDUCERS
    ),
    event_contract!(
        "radroots.trade.validation_receipt.v1",
        KIND_TRADE_VALIDATION_RECEIPT,
        "Trade Validation Receipt",
        "RadrootsTradeValidationReceipt",
        RadrootsEventClass::Regular,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Service,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        TRADE_VALIDATION_RECEIPT_TAGS,
        TRADE_VALIDATION_REDUCERS
    ),
];

pub fn all_kind_contracts() -> &'static [RadrootsKindContract] {
    ALL_KIND_CONTRACTS
}

pub fn all_event_contracts() -> &'static [RadrootsEventContract] {
    ALL_EVENT_CONTRACTS
}

pub fn contract_families() -> &'static [RadrootsContractFamilyMetadata] {
    CONTRACT_FAMILIES
}

pub fn event_contract_family(contract: &RadrootsEventContract) -> Option<RadrootsContractFamily> {
    contract_family_for_id(contract.id)
}

pub fn kind_contract_family(contract: &RadrootsKindContract) -> Option<RadrootsContractFamily> {
    Some(match contract.kind {
        KIND_PROFILE | KIND_FOLLOW | KIND_ACCOUNT_CLAIM => RadrootsContractFamily::Profile,
        KIND_SEAL | KIND_MESSAGE | KIND_MESSAGE_FILE | KIND_GIFT_WRAP => {
            RadrootsContractFamily::Message
        }
        KIND_COMMENT | KIND_GEOCHAT | KIND_POST | KIND_REACTION | KIND_REPOST
        | KIND_GENERIC_REPOST | KIND_ARTICLE | KIND_FILE_METADATA => RadrootsContractFamily::Social,
        KIND_RELAY_AUTH | KIND_HTTP_AUTH => RadrootsContractFamily::Relay,
        KIND_GROUP_PUT_USER
        | KIND_GROUP_REMOVE_USER
        | KIND_GROUP_EDIT_METADATA
        | KIND_GROUP_DELETE_EVENT
        | KIND_GROUP_CREATE_GROUP
        | KIND_GROUP_DELETE_GROUP
        | KIND_GROUP_CREATE_INVITE
        | KIND_GROUP_JOIN_REQUEST
        | KIND_GROUP_LEAVE_REQUEST
        | KIND_GROUP_METADATA
        | KIND_GROUP_ADMINS
        | KIND_GROUP_MEMBERS
        | KIND_GROUP_ROLES => RadrootsContractFamily::Group,
        KIND_LIST_MUTE
        | KIND_LIST_PINNED_NOTES
        | KIND_LIST_READ_WRITE_RELAYS
        | KIND_LIST_BOOKMARKS
        | KIND_LIST_COMMUNITIES
        | KIND_LIST_PUBLIC_CHATS
        | KIND_LIST_BLOCKED_RELAYS
        | KIND_LIST_SEARCH_RELAYS
        | KIND_LIST_SIMPLE_GROUPS
        | KIND_LIST_RELAY_FEEDS
        | KIND_LIST_INTERESTS
        | KIND_LIST_MEDIA_FOLLOWS
        | KIND_LIST_EMOJIS
        | KIND_LIST_DM_RELAYS
        | KIND_LIST_GOOD_WIKI_AUTHORS
        | KIND_LIST_GOOD_WIKI_RELAYS
        | KIND_LIST_SET_FOLLOW
        | KIND_LIST_SET_GENERIC
        | KIND_LIST_SET_RELAY
        | KIND_LIST_SET_BOOKMARK
        | KIND_LIST_SET_CURATION
        | KIND_LIST_SET_VIDEO
        | KIND_LIST_SET_PICTURE
        | KIND_LIST_SET_KIND_MUTE
        | KIND_LIST_SET_INTEREST
        | KIND_LIST_SET_EMOJI
        | KIND_LIST_SET_RELEASE_ARTIFACT
        | KIND_LIST_SET_APP_CURATION
        | KIND_LIST_SET_STARTER_PACK
        | KIND_LIST_SET_MEDIA_STARTER_PACK => RadrootsContractFamily::List,
        KIND_CALENDAR_DATE_EVENT
        | KIND_CALENDAR_TIME_EVENT
        | KIND_CALENDAR
        | KIND_CALENDAR_EVENT_RSVP => RadrootsContractFamily::Calendar,
        KIND_FARM
        | KIND_PLOT
        | KIND_COOP
        | KIND_DOCUMENT
        | KIND_RESOURCE_AREA
        | KIND_RESOURCE_HARVEST_CAP
        | KIND_FARM_WORKSPACE_MANIFEST
        | KIND_FARM_CRDT_CHANGE => RadrootsContractFamily::Farm,
        KIND_LISTING | KIND_LISTING_DRAFT => RadrootsContractFamily::Market,
        KIND_TRADE_LISTING_VALIDATION_REQUEST
        | KIND_TRADE_LISTING_VALIDATION_RESULT
        | KIND_TRADE_TRANSITION_PROOF_REQUEST
        | KIND_TRADE_TRANSITION_PROOF_RESULT
        | KIND_TRADE_VALIDATION_RECEIPT
        | KIND_ORDER_REQUEST
        | KIND_ORDER_DECISION
        | KIND_ORDER_REVISION_PROPOSAL
        | KIND_ORDER_REVISION_DECISION
        | KIND_ORDER_CANCELLATION => RadrootsContractFamily::Trade,
        KIND_WIKI_MERGE_REQUEST
        | KIND_WIKI_ARTICLE
        | KIND_WIKI_REDIRECT
        | KIND_KNOWLEDGE_SOURCE
        | KIND_EVIDENCE_BOUNTY
        | KIND_KNOWLEDGE_CLAIM
        | KIND_KNOWLEDGE_RELATION
        | KIND_KNOWLEDGE_REVIEW
        | KIND_KNOWLEDGE_FIELD_REPORT
        | KIND_KNOWLEDGE_CHANGE_PROPOSAL
        | KIND_CONTRIBUTION_ATTESTATION => RadrootsContractFamily::Knowledge,
        KIND_JOB_FEEDBACK => RadrootsContractFamily::Job,
        _ if is_request_kind(contract.kind) || is_result_kind(contract.kind) => {
            RadrootsContractFamily::Job
        }
        _ => return None,
    })
}

pub fn kind_contract(kind: u32) -> Option<&'static RadrootsKindContract> {
    ALL_KIND_CONTRACTS
        .iter()
        .find(|contract| contract.kind == kind)
}

pub fn event_contract(id: &str) -> Option<&'static RadrootsEventContract> {
    ALL_EVENT_CONTRACTS
        .iter()
        .find(|contract| contract.id == id)
}

pub fn event_contracts_for_kind(kind: u32) -> impl Iterator<Item = &'static RadrootsEventContract> {
    ALL_EVENT_CONTRACTS
        .iter()
        .filter(move |contract| contract.kind == kind)
}

pub fn identify_event_contract(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<&'static RadrootsEventContract, RadrootsContractMatchError> {
    if kind_contract(kind).is_none() {
        return Err(RadrootsContractMatchError::UnsupportedKind(kind));
    }

    identify_from_contracts(event_contracts_for_kind(kind), kind, tags, content)
}

pub fn validate_event_contract(
    event: &RadrootsNostrEvent,
) -> Result<&'static RadrootsEventContract, RadrootsContractValidationError> {
    let contract = match identify_event_contract(event.kind, &event.tags, &event.content) {
        Ok(contract) => contract,
        Err(error) => return Err(RadrootsContractValidationError::ContractMatch { error }),
    };
    validate_event_contract_shape(event, contract.id)?;
    Ok(contract)
}

pub fn validate_event_contract_shape(
    event: &RadrootsNostrEvent,
    contract_id: &str,
) -> Result<(), RadrootsContractValidationError> {
    validate_event_contract_parts(event.kind, &event.tags, event.content.as_str(), contract_id)
}

pub fn validate_event_contract_parts(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
    contract_id: &str,
) -> Result<(), RadrootsContractValidationError> {
    let contract = event_contract(contract_id).ok_or_else(|| {
        RadrootsContractValidationError::UnknownContract {
            contract_id: contract_id.to_owned(),
        }
    })?;
    if kind != contract.kind {
        return Err(RadrootsContractValidationError::KindMismatch {
            expected: contract.kind,
            actual: kind,
        });
    }
    validate_content_shape_parts(content, contract)?;
    validate_contract_tags_parts(tags, contract)?;
    validate_custom_knowledge_contract_parts(content, contract)?;
    Ok(())
}

fn identify_from_contracts<'a, I>(
    contracts: I,
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<&'a RadrootsEventContract, RadrootsContractMatchError>
where
    I: IntoIterator<Item = &'a RadrootsEventContract>,
{
    let mut matched = None;
    let mut matched_count = 0;

    for contract in contracts {
        if discriminator_matches(&contract.discriminator, tags, content) {
            matched = Some(contract);
            matched_count += 1;
        }
    }

    match (matched, matched_count) {
        (Some(contract), 1) => Ok(contract),
        (None, _) => Err(RadrootsContractMatchError::UnsupportedShape(kind)),
        (Some(_), _) => Err(RadrootsContractMatchError::AmbiguousShape(kind)),
    }
}

fn contract_family_for_id(id: &str) -> Option<RadrootsContractFamily> {
    if id.starts_with("radroots.account.") {
        Some(RadrootsContractFamily::Account)
    } else if id.starts_with("radroots.application.") {
        Some(RadrootsContractFamily::Application)
    } else if id.starts_with("radroots.calendar.") {
        Some(RadrootsContractFamily::Calendar)
    } else if id.starts_with("radroots.farm.") {
        Some(RadrootsContractFamily::Farm)
    } else if id.starts_with("radroots.group.") {
        Some(RadrootsContractFamily::Group)
    } else if id.starts_with("radroots.http.") {
        Some(RadrootsContractFamily::Http)
    } else if id.starts_with("radroots.job.") {
        Some(RadrootsContractFamily::Job)
    } else if id.starts_with("radroots.knowledge.") || id.starts_with("radroots.wiki.") {
        Some(RadrootsContractFamily::Knowledge)
    } else if id.starts_with("radroots.list.") || id.starts_with("radroots.list_set.") {
        Some(RadrootsContractFamily::List)
    } else if id.starts_with("radroots.listing.") {
        Some(RadrootsContractFamily::Market)
    } else if id.starts_with("radroots.message.") {
        Some(RadrootsContractFamily::Message)
    } else if id.starts_with("radroots.profile.") {
        Some(RadrootsContractFamily::Profile)
    } else if id.starts_with("radroots.relay.") {
        Some(RadrootsContractFamily::Relay)
    } else if id.starts_with("radroots.trade.") || id.starts_with("radroots.order.") {
        Some(RadrootsContractFamily::Trade)
    } else {
        None
    }
}

fn validate_content_shape_parts(
    content: &str,
    contract: &RadrootsEventContract,
) -> Result<(), RadrootsContractValidationError> {
    match contract.content_schema {
        RadrootsContentSchema::Empty => {
            if content.is_empty() {
                Ok(())
            } else {
                Err(RadrootsContractValidationError::ContentMustBeEmpty {
                    contract_id: contract.id,
                })
            }
        }
        RadrootsContentSchema::JsonObject => parse_content_object(content, contract.id).map(|_| ()),
        _ => Ok(()),
    }
}

fn validate_contract_tags_parts(
    tags: &[Vec<String>],
    contract: &RadrootsEventContract,
) -> Result<(), RadrootsContractValidationError> {
    for tag_contract in contract.tags {
        let count = tag_count(tags, tag_contract.name);
        let has_multiple_contracts_for_name = contract
            .tags
            .iter()
            .filter(|candidate| candidate.name == tag_contract.name)
            .count()
            > 1;
        match tag_contract.cardinality {
            RadrootsTagCardinality::RequiredOne => {
                if count == 0 {
                    return Err(RadrootsContractValidationError::MissingTag {
                        contract_id: contract.id,
                        name: tag_contract.name,
                    });
                }
                if count != 1 && !has_multiple_contracts_for_name {
                    return Err(RadrootsContractValidationError::TagCardinalityMismatch {
                        contract_id: contract.id,
                        name: tag_contract.name,
                    });
                }
            }
            RadrootsTagCardinality::RequiredMany => {
                if count == 0 {
                    return Err(RadrootsContractValidationError::MissingTag {
                        contract_id: contract.id,
                        name: tag_contract.name,
                    });
                }
            }
            RadrootsTagCardinality::OptionalOne => {
                if count > 1 && !has_multiple_contracts_for_name {
                    return Err(RadrootsContractValidationError::TagCardinalityMismatch {
                        contract_id: contract.id,
                        name: tag_contract.name,
                    });
                }
            }
            RadrootsTagCardinality::OptionalMany => {}
        }
        if tag_contract.name == "contract" {
            let actual = tag_value(tags, "contract").map(ToOwned::to_owned);
            if actual.as_deref() != Some(contract.id) {
                return Err(RadrootsContractValidationError::TagValueMismatch {
                    contract_id: contract.id,
                    name: "contract",
                    expected: contract.id.to_owned(),
                    actual,
                });
            }
        }
        validate_contract_tag_values(tags, contract, tag_contract)?;
    }
    Ok(())
}

fn validate_contract_tag_values(
    tags: &[Vec<String>],
    contract: &RadrootsEventContract,
    tag_contract: &RadrootsTagContract,
) -> Result<(), RadrootsContractValidationError> {
    for tag in tags
        .iter()
        .filter(|tag| tag.first().map(|value| value.as_str()) == Some(tag_contract.name))
    {
        if !tag_value_is_valid(tag, tag_contract.value_type) {
            return Err(RadrootsContractValidationError::TagValueMismatch {
                contract_id: contract.id,
                name: tag_contract.name,
                expected: tag_value_type_expectation(tag_contract.value_type).to_owned(),
                actual: tag.get(1).cloned(),
            });
        }
    }
    Ok(())
}

fn tag_value_is_valid(tag: &[String], value_type: RadrootsTagValueType) -> bool {
    let Some(value) = tag.get(1).map(String::as_str) else {
        return false;
    };
    match value_type {
        RadrootsTagValueType::AddressableCoordinate => {
            RadrootsAddressableCoordinate::parse(value).is_ok()
        }
        RadrootsTagValueType::ContractId => all_event_contracts()
            .iter()
            .any(|contract| contract.id == value),
        RadrootsTagValueType::DTag => RadrootsDTag::parse(value).is_ok(),
        RadrootsTagValueType::EventId | RadrootsTagValueType::Sha256 => {
            RadrootsEventId::parse(value).is_ok()
        }
        RadrootsTagValueType::EventPointer => event_pointer_tag_is_valid(tag),
        RadrootsTagValueType::Geohash => geohash_is_valid(value),
        RadrootsTagValueType::Kind => value.parse::<u32>().is_ok(),
        RadrootsTagValueType::PublicKey => RadrootsPublicKey::parse(value).is_ok(),
        RadrootsTagValueType::RelayUrl => relay_url_is_valid(value),
        RadrootsTagValueType::Text => visible_text_is_valid(value),
        RadrootsTagValueType::UnixTimestamp => value.parse::<u64>().is_ok(),
        RadrootsTagValueType::Url => url_is_valid(value),
        RadrootsTagValueType::Uuid => uuid_is_valid(value),
    }
}

fn event_pointer_tag_is_valid(tag: &[String]) -> bool {
    if tag.len() < 5 {
        return false;
    }
    let id = tag[1].as_str();
    let author = tag[2].as_str();
    let kind = tag[3].as_str();
    let d_tag = tag[4].as_str();
    RadrootsEventId::parse(id).is_ok()
        && RadrootsPublicKey::parse(author).is_ok()
        && kind.parse::<u32>().is_ok()
        && (d_tag.is_empty() || RadrootsDTag::parse(d_tag).is_ok())
        && tag
            .iter()
            .skip(5)
            .all(|relay| relay_url_is_valid(relay.as_str()))
}

fn visible_text_is_valid(value: &str) -> bool {
    !value.trim().is_empty() && !value.chars().any(char::is_control)
}

fn url_is_valid(value: &str) -> bool {
    (value.starts_with("http://") || value.starts_with("https://"))
        && value.len() > "http://".len()
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

fn geohash_is_valid(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 12
        && value
            .bytes()
            .all(|byte| matches!(byte.to_ascii_lowercase(), b'0'..=b'9' | b'b'..=b'h' | b'j'..=b'k' | b'm'..=b'n' | b'p'..=b'z'))
}

fn uuid_is_valid(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 36 {
        return false;
    }
    bytes.iter().enumerate().all(|(index, byte)| match index {
        8 | 13 | 18 | 23 => *byte == b'-',
        _ => byte.is_ascii_hexdigit(),
    })
}

fn tag_value_type_expectation(value_type: RadrootsTagValueType) -> &'static str {
    match value_type {
        RadrootsTagValueType::AddressableCoordinate => "addressable_coordinate",
        RadrootsTagValueType::ContractId => "contract_id",
        RadrootsTagValueType::DTag => "d_tag",
        RadrootsTagValueType::EventId => "event_id",
        RadrootsTagValueType::EventPointer => "event_pointer",
        RadrootsTagValueType::Geohash => "geohash",
        RadrootsTagValueType::Kind => "kind",
        RadrootsTagValueType::PublicKey => "public_key",
        RadrootsTagValueType::RelayUrl => "relay_url",
        RadrootsTagValueType::Sha256 => "sha256",
        RadrootsTagValueType::Text => "text",
        RadrootsTagValueType::UnixTimestamp => "unix_timestamp",
        RadrootsTagValueType::Url => "url",
        RadrootsTagValueType::Uuid => "uuid",
    }
}

fn validate_custom_knowledge_contract_parts(
    content: &str,
    contract: &RadrootsEventContract,
) -> Result<(), RadrootsContractValidationError> {
    let Some(expected_schema) = custom_knowledge_schema(contract.id) else {
        return Ok(());
    };
    let object = parse_content_object(content, contract.id)?;
    reject_forbidden_knowledge_fields(&object, contract.id)?;

    match object.get("schema").and_then(|value| value.as_str()) {
        Some(actual) if actual == expected_schema => {}
        Some(_) => {
            return Err(RadrootsContractValidationError::ContentFieldMismatch {
                contract_id: contract.id,
                field: "schema",
                expected: expected_schema.to_owned(),
            });
        }
        None => {
            return Err(RadrootsContractValidationError::MissingContentField {
                contract_id: contract.id,
                field: "schema",
            });
        }
    }

    match object
        .get("schema_version")
        .and_then(|value| value.as_u64())
    {
        Some(1) => Ok(()),
        Some(_) => Err(RadrootsContractValidationError::ContentFieldMismatch {
            contract_id: contract.id,
            field: "schema_version",
            expected: "1".to_owned(),
        }),
        None => Err(RadrootsContractValidationError::MissingContentField {
            contract_id: contract.id,
            field: "schema_version",
        }),
    }
}

fn parse_content_object(
    content: &str,
    contract_id: &'static str,
) -> Result<serde_json::Map<String, serde_json::Value>, RadrootsContractValidationError> {
    match serde_json::from_str::<serde_json::Value>(content) {
        Ok(serde_json::Value::Object(object)) => Ok(object),
        _ => Err(RadrootsContractValidationError::InvalidJsonContent { contract_id }),
    }
}

fn reject_forbidden_knowledge_fields(
    object: &serde_json::Map<String, serde_json::Value>,
    contract_id: &'static str,
) -> Result<(), RadrootsContractValidationError> {
    for field in [
        "review_status",
        "canon_status",
        "approved_for_canon",
        "rights_status",
        "trust_status",
        "trusted",
    ] {
        if object.contains_key(field) {
            return Err(RadrootsContractValidationError::ForbiddenContentField {
                contract_id,
                field,
            });
        }
    }
    Ok(())
}

fn custom_knowledge_schema(contract_id: &str) -> Option<&'static str> {
    match contract_id {
        "radroots.knowledge.source.v1" => Some("radroots.knowledge.source.v1"),
        "radroots.knowledge.evidence_bounty.v1" => Some("radroots.knowledge.evidence_bounty.v1"),
        "radroots.knowledge.claim.v1" => Some("radroots.knowledge.claim.v1"),
        "radroots.knowledge.relation.v1" => Some("radroots.knowledge.relation.v1"),
        "radroots.knowledge.review.v1" => Some("radroots.knowledge.review.v1"),
        "radroots.knowledge.field_report.v1" => Some("radroots.knowledge.field_report.v1"),
        "radroots.knowledge.change_proposal.v1" => Some("radroots.knowledge.change_proposal.v1"),
        "radroots.knowledge.contribution_attestation.v1" => {
            Some("radroots.knowledge.contribution_attestation.v1")
        }
        _ => None,
    }
}

fn discriminator_matches(
    discriminator: &RadrootsEventDiscriminator,
    tags: &[Vec<String>],
    content: &str,
) -> bool {
    match discriminator {
        RadrootsEventDiscriminator::KindOnly => true,
        RadrootsEventDiscriminator::DTagExact(expected) => tag_value(tags, "d") == Some(*expected),
        RadrootsEventDiscriminator::DTagPrefix(prefix) => tag_value(tags, "d")
            .map(|value| value.starts_with(prefix))
            .unwrap_or(false),
        RadrootsEventDiscriminator::DTagSuffix(suffix) => tag_value(tags, "d")
            .map(|value| value.ends_with(suffix))
            .unwrap_or(false),
        RadrootsEventDiscriminator::TagEquals { name, value } => {
            tag_value(tags, name) == Some(*value)
        }
        RadrootsEventDiscriminator::ContentJsonFieldEquals { field, value } => {
            content_json_string_field_equals(content, field, value)
        }
        RadrootsEventDiscriminator::EnvelopeType(expected) => {
            content_json_string_field_equals(content, "type", expected)
        }
        RadrootsEventDiscriminator::Composite(parts) => parts
            .iter()
            .all(|part| discriminator_matches(part, tags, content)),
    }
}

fn tag_value<'a>(tags: &'a [Vec<String>], name: &str) -> Option<&'a str> {
    tags.iter().find_map(|tag| {
        if tag.first().map(|value| value.as_str()) == Some(name) {
            tag.get(1).map(|value| value.as_str())
        } else {
            None
        }
    })
}

fn tag_count(tags: &[Vec<String>], name: &str) -> usize {
    tags.iter()
        .filter(|tag| tag.first().map(|value| value.as_str()) == Some(name))
        .count()
}

fn content_json_string_field_equals(content: &str, field: &str, value: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(content)
        .ok()
        .and_then(|json| {
            json.get(field)
                .and_then(|field| field.as_str())
                .map(|field| field == value)
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    static AMBIGUOUS_TEST_CONTRACTS: &[RadrootsEventContract] = &[
        event_contract!(
            "radroots.test.one.v1",
            KIND_POST,
            "Test One",
            "Test",
            RadrootsEventClass::Regular,
            RadrootsEventPrivacy::Public,
            RadrootsActorRole::Any,
            RadrootsContentSchema::PlainText,
            RadrootsEventDiscriminator::KindOnly,
            NO_TAGS,
            SOCIAL_REDUCERS,
        ),
        event_contract!(
            "radroots.test.two.v1",
            KIND_POST,
            "Test Two",
            "Test",
            RadrootsEventClass::Regular,
            RadrootsEventPrivacy::Public,
            RadrootsActorRole::Any,
            RadrootsContentSchema::PlainText,
            RadrootsEventDiscriminator::KindOnly,
            NO_TAGS,
            SOCIAL_REDUCERS,
        ),
    ];

    static REQUIRED_MANY_TEST_TAGS: &[RadrootsTagContract] = &[tag(
        "test_many",
        RadrootsTagCardinality::RequiredMany,
        RadrootsTagSemantic::Topic,
        RadrootsTagValueType::Text,
        false,
    )];

    static OPTIONAL_ONE_TEST_TAGS: &[RadrootsTagContract] = &[tag(
        "test_optional",
        RadrootsTagCardinality::OptionalOne,
        RadrootsTagSemantic::Topic,
        RadrootsTagValueType::Text,
        false,
    )];

    static DUPLICATE_REQUIRED_TEST_TAGS: &[RadrootsTagContract] = &[
        tag(
            "test_required",
            RadrootsTagCardinality::RequiredOne,
            RadrootsTagSemantic::Topic,
            RadrootsTagValueType::Text,
            false,
        ),
        tag(
            "test_required",
            RadrootsTagCardinality::RequiredOne,
            RadrootsTagSemantic::Category,
            RadrootsTagValueType::Text,
            false,
        ),
    ];

    static DUPLICATE_OPTIONAL_TEST_TAGS: &[RadrootsTagContract] = &[
        tag(
            "test_optional",
            RadrootsTagCardinality::OptionalOne,
            RadrootsTagSemantic::Topic,
            RadrootsTagValueType::Text,
            false,
        ),
        tag(
            "test_optional",
            RadrootsTagCardinality::OptionalOne,
            RadrootsTagSemantic::Category,
            RadrootsTagValueType::Text,
            false,
        ),
    ];

    fn synthetic_event_contract(
        id: &'static str,
        tags: &'static [RadrootsTagContract],
    ) -> RadrootsEventContract {
        RadrootsEventContract {
            id,
            kind: KIND_POST,
            name: "Test",
            payload_type: "Test",
            class: RadrootsEventClass::Regular,
            stability: RadrootsEventStability::Experimental,
            privacy: RadrootsEventPrivacy::Public,
            author_role: RadrootsActorRole::Any,
            content_schema: RadrootsContentSchema::PlainText,
            discriminator: RadrootsEventDiscriminator::KindOnly,
            tags,
            reducers: SOCIAL_REDUCERS,
        }
    }

    fn synthetic_kind_contract(kind: u32) -> RadrootsKindContract {
        RadrootsKindContract {
            kind,
            canonical_constant: "KIND_TEST",
            name: "Test",
            class: RadrootsEventClass::Regular,
            standard: RadrootsNostrStandard::Radroots,
            accepted_event_contracts: &[],
        }
    }

    fn unsigned_event(kind: u32, tags: Vec<Vec<&str>>, content: &str) -> RadrootsNostrEvent {
        RadrootsNostrEvent {
            id: "0".repeat(64),
            author: "1".repeat(64),
            created_at: 1_700_000_000,
            kind,
            tags: tags
                .into_iter()
                .map(|tag| tag.into_iter().map(ToOwned::to_owned).collect())
                .collect(),
            content: content.to_owned(),
            sig: "2".repeat(128),
        }
    }

    fn unsigned_event_owned(
        kind: u32,
        tags: Vec<Vec<String>>,
        content: &str,
    ) -> RadrootsNostrEvent {
        RadrootsNostrEvent {
            id: "0".repeat(64),
            author: "1".repeat(64),
            created_at: 1_700_000_000,
            kind,
            tags,
            content: content.to_owned(),
            sig: "2".repeat(128),
        }
    }

    fn hex_64(character: char) -> String {
        core::iter::repeat_n(character, 64).collect()
    }

    fn event_ref_tag(name: &str, event_id: &str, author: &str, kind: u32) -> Vec<String> {
        vec![
            name.to_owned(),
            event_id.to_owned(),
            author.to_owned(),
            kind.to_string(),
            String::new(),
        ]
    }

    fn owned_tag(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn exposes_one_kind_contract_per_supported_kind() {
        let mut kinds = BTreeSet::new();
        for contract in all_kind_contracts() {
            assert!(
                kinds.insert(contract.kind),
                "duplicate kind {}",
                contract.kind
            );
            assert!(!contract.accepted_event_contracts.is_empty());
        }
    }

    #[test]
    fn exposes_unique_event_contract_ids() {
        let mut ids = BTreeSet::new();
        for contract in all_event_contracts() {
            assert!(
                ids.insert(contract.id),
                "duplicate event contract {}",
                contract.id
            );
            assert!(kind_contract(contract.kind).is_some());
        }
    }

    #[test]
    fn every_kind_references_known_matching_event_contracts() {
        for kind in all_kind_contracts() {
            for id in kind.accepted_event_contracts {
                let event = event_contract(id).expect("accepted event contract");
                assert_eq!(event.kind, kind.kind, "{}", id);
            }
        }
    }

    #[test]
    fn event_contract_classes_match_kind_contracts() {
        for contract in all_event_contracts() {
            let kind = kind_contract(contract.kind).expect("event kind contract");
            assert_eq!(contract.class, kind.class, "{}", contract.id);
        }
    }

    #[test]
    fn every_event_contract_is_listed_by_its_kind_contract() {
        for contract in all_event_contracts() {
            let kind = kind_contract(contract.kind).expect("event kind contract");
            assert!(
                kind.accepted_event_contracts.contains(&contract.id),
                "{}",
                contract.id
            );
        }
    }

    #[test]
    fn order_request_listing_event_contract_is_event_id() {
        let contract = event_contract("radroots.order.request.v1").expect("order request");
        let tag = contract
            .tags
            .iter()
            .find(|tag| tag.name == "listing_event")
            .expect("listing event tag");

        assert_eq!(tag.semantic, RadrootsTagSemantic::ListingSnapshot);
        assert_eq!(tag.value_type, RadrootsTagValueType::EventId);
        assert!(!tag.relay_indexed);
    }

    #[test]
    fn covers_public_kind_arrays() {
        for kind in COMMERCIAL_EVENT_KINDS
            .iter()
            .chain(PUBLIC_SOCIAL_KINDS.iter())
            .chain(PRIVATE_FARM_OPS_KINDS.iter())
            .chain(NIP29_GROUP_KINDS.iter())
            .chain(KNOWLEDGE_EVENT_KINDS.iter())
        {
            assert!(kind_contract(*kind).is_some(), "missing kind {kind}");
        }
    }

    #[test]
    fn event_contract_lookup_supports_many_contracts_per_kind() {
        let contracts = event_contracts_for_kind(KIND_LIST_SET_GENERIC).collect::<Vec<_>>();
        assert_eq!(contracts.len(), 6);
        assert!(
            contracts
                .iter()
                .any(|contract| contract.id == "radroots.list_set.farm.members.v1")
        );
        assert_eq!(
            event_contract("radroots.list_set.member_of.farms.v1").map(|contract| contract.kind),
            Some(KIND_LIST_SET_GENERIC)
        );
        assert!(event_contracts_for_kind(999_999).next().is_none());
    }

    #[test]
    fn event_contract_lookup_supports_knowledge_contract_kinds() {
        let contracts = event_contracts_for_kind(KIND_WIKI_ARTICLE).collect::<Vec<_>>();
        assert_eq!(contracts.len(), 1);
        assert_eq!(contracts[0].id, "radroots.wiki.article.v1");
        assert_eq!(
            identify_event_contract(KIND_WIKI_ARTICLE, &[], "# Soil")
                .expect("wiki article contract")
                .id,
            "radroots.wiki.article.v1"
        );
    }

    #[test]
    fn exposes_contract_family_metadata() {
        assert!(
            contract_families()
                .iter()
                .any(|family| family.family == RadrootsContractFamily::Knowledge
                    && family.id == "knowledge")
        );
        assert_eq!(
            event_contract_family(event_contract("radroots.wiki.article.v1").expect("wiki")),
            Some(RadrootsContractFamily::Knowledge)
        );
        assert_eq!(
            kind_contract_family(kind_contract(KIND_KNOWLEDGE_CLAIM).expect("claim kind")),
            Some(RadrootsContractFamily::Knowledge)
        );
        assert_eq!(
            kind_contract_family(kind_contract(KIND_LIST_SET_GENERIC).expect("list kind")),
            Some(RadrootsContractFamily::List)
        );
    }

    #[test]
    fn contract_family_helpers_cover_prefixes_and_kind_branches() {
        for (id, family) in [
            (
                "radroots.account.test.v1",
                Some(RadrootsContractFamily::Account),
            ),
            (
                "radroots.application.test.v1",
                Some(RadrootsContractFamily::Application),
            ),
            (
                "radroots.calendar.test.v1",
                Some(RadrootsContractFamily::Calendar),
            ),
            ("radroots.farm.test.v1", Some(RadrootsContractFamily::Farm)),
            (
                "radroots.group.test.v1",
                Some(RadrootsContractFamily::Group),
            ),
            ("radroots.http.test.v1", Some(RadrootsContractFamily::Http)),
            ("radroots.job.test.v1", Some(RadrootsContractFamily::Job)),
            (
                "radroots.knowledge.test.v1",
                Some(RadrootsContractFamily::Knowledge),
            ),
            (
                "radroots.wiki.test.v1",
                Some(RadrootsContractFamily::Knowledge),
            ),
            ("radroots.list.test.v1", Some(RadrootsContractFamily::List)),
            (
                "radroots.list_set.test.v1",
                Some(RadrootsContractFamily::List),
            ),
            (
                "radroots.listing.test.v1",
                Some(RadrootsContractFamily::Market),
            ),
            (
                "radroots.message.test.v1",
                Some(RadrootsContractFamily::Message),
            ),
            (
                "radroots.profile.test.v1",
                Some(RadrootsContractFamily::Profile),
            ),
            (
                "radroots.relay.test.v1",
                Some(RadrootsContractFamily::Relay),
            ),
            (
                "radroots.trade.test.v1",
                Some(RadrootsContractFamily::Trade),
            ),
            (
                "radroots.order.test.v1",
                Some(RadrootsContractFamily::Trade),
            ),
            ("radroots.test.unknown.v1", None),
        ] {
            assert_eq!(contract_family_for_id(id), family, "{id}");
        }

        for (kind, family) in [
            (KIND_PROFILE, RadrootsContractFamily::Profile),
            (KIND_MESSAGE, RadrootsContractFamily::Message),
            (KIND_POST, RadrootsContractFamily::Social),
            (KIND_RELAY_AUTH, RadrootsContractFamily::Relay),
            (KIND_GROUP_ROLES, RadrootsContractFamily::Group),
            (KIND_LIST_SET_GENERIC, RadrootsContractFamily::List),
            (KIND_CALENDAR_EVENT_RSVP, RadrootsContractFamily::Calendar),
            (KIND_FARM_CRDT_CHANGE, RadrootsContractFamily::Farm),
            (KIND_LISTING, RadrootsContractFamily::Market),
            (KIND_ORDER_CANCELLATION, RadrootsContractFamily::Trade),
            (KIND_KNOWLEDGE_CLAIM, RadrootsContractFamily::Knowledge),
            (KIND_JOB_FEEDBACK, RadrootsContractFamily::Job),
            (KIND_JOB_REQUEST_MIN, RadrootsContractFamily::Job),
            (KIND_JOB_RESULT_MIN, RadrootsContractFamily::Job),
        ] {
            assert_eq!(
                kind_contract_family(&synthetic_kind_contract(kind)),
                Some(family),
                "{kind}"
            );
        }

        assert_eq!(
            kind_contract_family(&synthetic_kind_contract(999_999)),
            None
        );
    }

    #[test]
    fn exposes_knowledge_contracts() {
        let wiki_article = event_contract("radroots.wiki.article.v1").expect("wiki article");
        assert_eq!(wiki_article.kind, KIND_WIKI_ARTICLE);
        assert_eq!(wiki_article.stability, RadrootsEventStability::Experimental);
        assert_eq!(
            kind_contract(KIND_WIKI_ARTICLE)
                .expect("wiki kind")
                .standard,
            RadrootsNostrStandard::Nip54
        );
        assert_eq!(wiki_article.content_schema, RadrootsContentSchema::Djot);

        let wiki_merge_request =
            event_contract("radroots.wiki.merge_request.v1").expect("wiki merge request");
        assert_eq!(
            wiki_merge_request.stability,
            RadrootsEventStability::Experimental
        );
        assert_eq!(
            wiki_merge_request.content_schema,
            RadrootsContentSchema::PlainText
        );

        let wiki_redirect = event_contract("radroots.wiki.redirect.v1").expect("wiki redirect");
        assert_eq!(wiki_redirect.kind, KIND_WIKI_REDIRECT);
        assert_eq!(
            wiki_redirect.stability,
            RadrootsEventStability::Experimental
        );
        assert_eq!(wiki_redirect.content_schema, RadrootsContentSchema::Empty);

        for id in [
            "radroots.knowledge.source.v1",
            "radroots.knowledge.evidence_bounty.v1",
            "radroots.knowledge.claim.v1",
            "radroots.knowledge.relation.v1",
            "radroots.knowledge.review.v1",
            "radroots.knowledge.field_report.v1",
            "radroots.knowledge.change_proposal.v1",
            "radroots.knowledge.contribution_attestation.v1",
        ] {
            let contract = event_contract(id).expect(id);
            assert_eq!(contract.stability, RadrootsEventStability::Experimental);
            assert_eq!(
                event_contract_family(contract),
                Some(RadrootsContractFamily::Knowledge)
            );
            let contract_tag = contract
                .tags
                .iter()
                .find(|tag| tag.name == "contract")
                .expect("contract tag");
            assert_eq!(contract_tag.semantic, RadrootsTagSemantic::Contract);
            assert_eq!(contract_tag.value_type, RadrootsTagValueType::ContractId);
        }
    }

    #[test]
    fn custom_knowledge_schema_lookup_covers_registered_ids() {
        for id in [
            "radroots.knowledge.source.v1",
            "radroots.knowledge.evidence_bounty.v1",
            "radroots.knowledge.claim.v1",
            "radroots.knowledge.relation.v1",
            "radroots.knowledge.review.v1",
            "radroots.knowledge.field_report.v1",
            "radroots.knowledge.change_proposal.v1",
            "radroots.knowledge.contribution_attestation.v1",
        ] {
            assert_eq!(custom_knowledge_schema(id), Some(id), "{id}");
        }
        assert_eq!(custom_knowledge_schema("radroots.wiki.article.v1"), None);
    }

    #[test]
    fn identifies_exact_list_set_shape() {
        let tags = vec![vec!["d".to_owned(), "member_of.farms".to_owned()]];
        let contract = identify_event_contract(KIND_LIST_SET_GENERIC, &tags, "{}")
            .expect("member_of farms contract");
        assert_eq!(contract.id, "radroots.list_set.member_of.farms.v1");
    }

    #[test]
    fn identifies_composite_list_set_shape() {
        let tags = vec![vec![
            "d".to_owned(),
            "farm:farm_01:members.workers".to_owned(),
        ]];
        let contract = identify_event_contract(KIND_LIST_SET_GENERIC, &tags, "{}")
            .expect("farm workers contract");
        assert_eq!(contract.id, "radroots.list_set.farm.members.workers.v1");
    }

    #[test]
    fn rejects_unknown_or_unsupported_shapes() {
        assert_eq!(
            identify_event_contract(999_999, &[], "{}"),
            Err(RadrootsContractMatchError::UnsupportedKind(999_999))
        );
        assert_eq!(
            validate_event_contract(&unsigned_event(999_999, Vec::new(), "{}")),
            Err(RadrootsContractValidationError::ContractMatch {
                error: RadrootsContractMatchError::UnsupportedKind(999_999),
            })
        );

        let tags = vec![vec!["d".to_owned(), "unknown".to_owned()]];
        assert_eq!(
            identify_event_contract(KIND_LIST_SET_GENERIC, &tags, "{}"),
            Err(RadrootsContractMatchError::UnsupportedShape(
                KIND_LIST_SET_GENERIC
            ))
        );
    }

    #[test]
    fn rejects_ambiguous_shapes() {
        assert_eq!(
            identify_from_contracts(AMBIGUOUS_TEST_CONTRACTS.iter(), KIND_POST, &[], ""),
            Err(RadrootsContractMatchError::AmbiguousShape(KIND_POST))
        );
    }

    #[test]
    fn supports_content_field_discriminators() {
        assert!(discriminator_matches(
            &RadrootsEventDiscriminator::EnvelopeType("order_request"),
            &[],
            r#"{"domain":"radroots.order","type":"order_request"}"#
        ));
        assert!(discriminator_matches(
            &RadrootsEventDiscriminator::ContentJsonFieldEquals {
                field: "domain",
                value: "radroots.order"
            },
            &[],
            r#"{"domain": "radroots.order", "type": "order_request"}"#
        ));
    }

    #[test]
    fn supports_tag_equals_discriminators() {
        let tags = vec![vec!["status".to_owned(), "accepted".to_owned()]];

        assert!(discriminator_matches(
            &RadrootsEventDiscriminator::TagEquals {
                name: "status",
                value: "accepted",
            },
            &tags,
            "{}"
        ));
        assert!(!discriminator_matches(
            &RadrootsEventDiscriminator::TagEquals {
                name: "status",
                value: "declined",
            },
            &tags,
            "{}"
        ));
    }

    #[test]
    fn validates_custom_knowledge_contract_shape() {
        let event = unsigned_event(
            KIND_KNOWLEDGE_CLAIM,
            vec![vec!["contract", "radroots.knowledge.claim.v1"]],
            r#"{"schema":"radroots.knowledge.claim.v1","schema_version":1,"text":"soil improves with cover crops"}"#,
        );

        assert_eq!(
            validate_event_contract_shape(&event, "radroots.knowledge.claim.v1"),
            Ok(())
        );
        assert_eq!(
            validate_event_contract(&event).expect("validated").id,
            "radroots.knowledge.claim.v1"
        );
    }

    #[test]
    fn rejects_custom_knowledge_contract_tag_mismatch() {
        let event = unsigned_event(
            KIND_KNOWLEDGE_CLAIM,
            vec![vec!["contract", "radroots.knowledge.relation.v1"]],
            r#"{"schema":"radroots.knowledge.claim.v1","schema_version":1}"#,
        );

        assert_eq!(
            validate_event_contract_shape(&event, "radroots.knowledge.claim.v1"),
            Err(RadrootsContractValidationError::TagValueMismatch {
                contract_id: "radroots.knowledge.claim.v1",
                name: "contract",
                expected: "radroots.knowledge.claim.v1".to_owned(),
                actual: Some("radroots.knowledge.relation.v1".to_owned()),
            })
        );
    }

    #[test]
    fn rejects_custom_knowledge_schema_mismatch() {
        let event = unsigned_event(
            KIND_KNOWLEDGE_CLAIM,
            vec![vec!["contract", "radroots.knowledge.claim.v1"]],
            r#"{"schema":"radroots.knowledge.relation.v1","schema_version":1}"#,
        );

        assert_eq!(
            validate_event_contract_shape(&event, "radroots.knowledge.claim.v1"),
            Err(RadrootsContractValidationError::ContentFieldMismatch {
                contract_id: "radroots.knowledge.claim.v1",
                field: "schema",
                expected: "radroots.knowledge.claim.v1".to_owned(),
            })
        );
    }

    #[test]
    fn rejects_custom_knowledge_missing_schema_version() {
        let event = unsigned_event(
            KIND_KNOWLEDGE_CLAIM,
            vec![vec!["contract", "radroots.knowledge.claim.v1"]],
            r#"{"schema":"radroots.knowledge.claim.v1"}"#,
        );

        assert_eq!(
            validate_event_contract_shape(&event, "radroots.knowledge.claim.v1"),
            Err(RadrootsContractValidationError::MissingContentField {
                contract_id: "radroots.knowledge.claim.v1",
                field: "schema_version",
            })
        );
    }

    #[test]
    fn rejects_authoritative_knowledge_status_fields() {
        let event = unsigned_event(
            KIND_KNOWLEDGE_REVIEW,
            vec![
                vec!["contract", "radroots.knowledge.review.v1"],
                vec![
                    "review_target",
                    "0000000000000000000000000000000000000000000000000000000000000000",
                    "1111111111111111111111111111111111111111111111111111111111111111",
                    "30818",
                    "soil",
                ],
            ],
            r#"{"schema":"radroots.knowledge.review.v1","schema_version":1,"canon_status":"approved"}"#,
        );

        assert_eq!(
            validate_event_contract_shape(&event, "radroots.knowledge.review.v1"),
            Err(RadrootsContractValidationError::ForbiddenContentField {
                contract_id: "radroots.knowledge.review.v1",
                field: "canon_status",
            })
        );
    }

    #[test]
    fn validate_event_contract_shape_reports_registry_kind_and_content_errors() {
        let event = unsigned_event(KIND_POST, Vec::new(), "hello");
        assert_eq!(
            validate_event_contract_shape(&event, "missing.contract.v1"),
            Err(RadrootsContractValidationError::UnknownContract {
                contract_id: "missing.contract.v1".to_owned(),
            })
        );
        assert_eq!(
            validate_event_contract_shape(&event, "radroots.profile.metadata.v1"),
            Err(RadrootsContractValidationError::KindMismatch {
                expected: KIND_PROFILE,
                actual: KIND_POST,
            })
        );

        let invalid_json = unsigned_event(
            KIND_KNOWLEDGE_CLAIM,
            vec![vec!["contract", "radroots.knowledge.claim.v1"]],
            "not-json",
        );
        assert_eq!(
            validate_event_contract_shape(&invalid_json, "radroots.knowledge.claim.v1"),
            Err(RadrootsContractValidationError::InvalidJsonContent {
                contract_id: "radroots.knowledge.claim.v1",
            })
        );

        assert_eq!(
            validate_event_contract_shape(
                &unsigned_event(KIND_POST, Vec::new(), "plain text"),
                "radroots.social.post.v1",
            ),
            Ok(())
        );
    }

    #[test]
    fn validate_contract_tags_reports_cardinality_errors() {
        let missing_required_one = unsigned_event(
            KIND_KNOWLEDGE_CLAIM,
            Vec::new(),
            r#"{"schema":"radroots.knowledge.claim.v1","schema_version":1}"#,
        );
        assert_eq!(
            validate_event_contract_shape(&missing_required_one, "radroots.knowledge.claim.v1"),
            Err(RadrootsContractValidationError::MissingTag {
                contract_id: "radroots.knowledge.claim.v1",
                name: "contract",
            })
        );

        let duplicate_required_one = unsigned_event(
            KIND_KNOWLEDGE_CLAIM,
            vec![
                vec!["contract", "radroots.knowledge.claim.v1"],
                vec!["contract", "radroots.knowledge.claim.v1"],
            ],
            r#"{"schema":"radroots.knowledge.claim.v1","schema_version":1}"#,
        );
        assert_eq!(
            validate_event_contract_shape(&duplicate_required_one, "radroots.knowledge.claim.v1"),
            Err(RadrootsContractValidationError::TagCardinalityMismatch {
                contract_id: "radroots.knowledge.claim.v1",
                name: "contract",
            })
        );

        let required_many =
            synthetic_event_contract("radroots.test.required_many.v1", REQUIRED_MANY_TEST_TAGS);
        assert_eq!(
            validate_contract_tags_parts(&[], &required_many),
            Err(RadrootsContractValidationError::MissingTag {
                contract_id: "radroots.test.required_many.v1",
                name: "test_many",
            })
        );
        assert_eq!(
            validate_contract_tags_parts(
                &vec![vec!["test_many".to_owned(), "one".to_owned()]],
                &required_many
            ),
            Ok(())
        );

        let optional_one =
            synthetic_event_contract("radroots.test.optional_one.v1", OPTIONAL_ONE_TEST_TAGS);
        assert_eq!(validate_contract_tags_parts(&[], &optional_one), Ok(()));
        assert_eq!(
            validate_contract_tags_parts(
                &vec![
                    vec!["test_optional".to_owned(), "one".to_owned()],
                    vec!["test_optional".to_owned(), "two".to_owned()],
                ],
                &optional_one,
            ),
            Err(RadrootsContractValidationError::TagCardinalityMismatch {
                contract_id: "radroots.test.optional_one.v1",
                name: "test_optional",
            })
        );

        let duplicate_required = synthetic_event_contract(
            "radroots.test.duplicate_required.v1",
            DUPLICATE_REQUIRED_TEST_TAGS,
        );
        assert_eq!(
            validate_contract_tags_parts(
                &vec![
                    vec!["test_required".to_owned(), "one".to_owned()],
                    vec!["test_required".to_owned(), "two".to_owned()],
                ],
                &duplicate_required,
            ),
            Ok(())
        );

        let duplicate_optional = synthetic_event_contract(
            "radroots.test.duplicate_optional.v1",
            DUPLICATE_OPTIONAL_TEST_TAGS,
        );
        assert_eq!(
            validate_contract_tags_parts(
                &vec![
                    vec!["test_optional".to_owned(), "one".to_owned()],
                    vec!["test_optional".to_owned(), "two".to_owned()],
                ],
                &duplicate_optional,
            ),
            Ok(())
        );
    }

    #[test]
    fn validate_contract_tags_enforces_declared_value_types() {
        let claim_content = r#"{"schema":"radroots.knowledge.claim.v1","schema_version":1}"#;
        let invalid_source = unsigned_event_owned(
            KIND_KNOWLEDGE_CLAIM,
            vec![
                vec![
                    "contract".to_owned(),
                    "radroots.knowledge.claim.v1".to_owned(),
                ],
                vec!["source".to_owned(), "not-an-event-id".to_owned()],
            ],
            claim_content,
        );
        assert_eq!(
            validate_event_contract_shape(&invalid_source, "radroots.knowledge.claim.v1"),
            Err(RadrootsContractValidationError::TagValueMismatch {
                contract_id: "radroots.knowledge.claim.v1",
                name: "source",
                expected: "event_pointer".to_owned(),
                actual: Some("not-an-event-id".to_owned()),
            })
        );

        let invalid_citation = unsigned_event(
            KIND_KNOWLEDGE_CLAIM,
            vec![
                vec!["contract", "radroots.knowledge.claim.v1"],
                vec!["citation", "not-hex"],
            ],
            claim_content,
        );
        assert_eq!(
            validate_event_contract_shape(&invalid_citation, "radroots.knowledge.claim.v1"),
            Err(RadrootsContractValidationError::TagValueMismatch {
                contract_id: "radroots.knowledge.claim.v1",
                name: "citation",
                expected: "sha256".to_owned(),
                actual: Some("not-hex".to_owned()),
            })
        );

        let invalid_review = unsigned_event(
            KIND_KNOWLEDGE_REVIEW,
            vec![
                vec!["contract", "radroots.knowledge.review.v1"],
                vec!["review_target", "not-an-event-id"],
            ],
            r#"{"schema":"radroots.knowledge.review.v1","schema_version":1}"#,
        );
        assert_eq!(
            validate_event_contract_shape(&invalid_review, "radroots.knowledge.review.v1"),
            Err(RadrootsContractValidationError::TagValueMismatch {
                contract_id: "radroots.knowledge.review.v1",
                name: "review_target",
                expected: "event_pointer".to_owned(),
                actual: Some("not-an-event-id".to_owned()),
            })
        );

        let invalid_geohash = unsigned_event(
            KIND_KNOWLEDGE_FIELD_REPORT,
            vec![
                vec!["contract", "radroots.knowledge.field_report.v1"],
                vec!["g", "invalid-a"],
            ],
            r#"{"schema":"radroots.knowledge.field_report.v1","schema_version":1}"#,
        );
        assert_eq!(
            validate_event_contract_shape(&invalid_geohash, "radroots.knowledge.field_report.v1"),
            Err(RadrootsContractValidationError::TagValueMismatch {
                contract_id: "radroots.knowledge.field_report.v1",
                name: "g",
                expected: "geohash".to_owned(),
                actual: Some("invalid-a".to_owned()),
            })
        );

        let invalid_address = unsigned_event(
            KIND_WIKI_REDIRECT,
            vec![vec!["d", "soil"], vec!["a", "30818:not-hex:soil"]],
            "",
        );
        assert_eq!(
            validate_event_contract_shape(&invalid_address, "radroots.wiki.redirect.v1"),
            Err(RadrootsContractValidationError::TagValueMismatch {
                contract_id: "radroots.wiki.redirect.v1",
                name: "a",
                expected: "addressable_coordinate".to_owned(),
                actual: Some("30818:not-hex:soil".to_owned()),
            })
        );

        let invalid_event_id = unsigned_event(
            KIND_WIKI_MERGE_REQUEST,
            vec![
                vec![
                    "a",
                    "30818:0000000000000000000000000000000000000000000000000000000000000000:soil",
                ],
                vec![
                    "p",
                    "1111111111111111111111111111111111111111111111111111111111111111",
                ],
                vec!["e", "not-hex"],
            ],
            "",
        );
        assert_eq!(
            validate_event_contract_shape(&invalid_event_id, "radroots.wiki.merge_request.v1"),
            Err(RadrootsContractValidationError::TagValueMismatch {
                contract_id: "radroots.wiki.merge_request.v1",
                name: "e",
                expected: "event_id".to_owned(),
                actual: Some("not-hex".to_owned()),
            })
        );

        let valid_source = unsigned_event_owned(
            KIND_KNOWLEDGE_CLAIM,
            vec![
                vec![
                    "contract".to_owned(),
                    "radroots.knowledge.claim.v1".to_owned(),
                ],
                event_ref_tag(
                    "source",
                    hex_64('a').as_str(),
                    hex_64('b').as_str(),
                    KIND_KNOWLEDGE_SOURCE,
                ),
                vec!["citation".to_owned(), hex_64('c')],
            ],
            claim_content,
        );
        assert_eq!(
            validate_event_contract_shape(&valid_source, "radroots.knowledge.claim.v1"),
            Ok(())
        );
    }

    #[test]
    fn tag_value_shape_helpers_cover_contract_registry_value_types() {
        let event_id = hex_64('a');
        let public_key = hex_64('b');
        let coordinate = format!("{KIND_WIKI_ARTICLE}:{public_key}:soil");
        let valid_pointer = vec![
            "source".to_owned(),
            event_id.clone(),
            public_key.clone(),
            KIND_KNOWLEDGE_SOURCE.to_string(),
            "soil".to_owned(),
            "ws://relay.example.com".to_owned(),
            "wss://relay.example.net".to_owned(),
        ];
        let empty_d_pointer = vec![
            "source".to_owned(),
            event_id.clone(),
            public_key.clone(),
            KIND_KNOWLEDGE_SOURCE.to_string(),
            String::new(),
        ];

        assert!(!tag_value_is_valid(
            &owned_tag(&["source"]),
            RadrootsTagValueType::EventPointer
        ));
        assert!(tag_value_is_valid(
            &owned_tag(&["a", coordinate.as_str()]),
            RadrootsTagValueType::AddressableCoordinate
        ));
        assert!(!tag_value_is_valid(
            &owned_tag(&["a", "30818:not-hex:soil"]),
            RadrootsTagValueType::AddressableCoordinate
        ));
        assert!(tag_value_is_valid(
            &owned_tag(&["contract", "radroots.knowledge.claim.v1"]),
            RadrootsTagValueType::ContractId
        ));
        assert!(!tag_value_is_valid(
            &owned_tag(&["contract", "radroots.unknown.v1"]),
            RadrootsTagValueType::ContractId
        ));
        assert!(tag_value_is_valid(
            &owned_tag(&["d", "soil"]),
            RadrootsTagValueType::DTag
        ));
        assert!(!tag_value_is_valid(
            &owned_tag(&["d", ""]),
            RadrootsTagValueType::DTag
        ));
        assert!(tag_value_is_valid(
            &owned_tag(&["e", event_id.as_str()]),
            RadrootsTagValueType::EventId
        ));
        assert!(tag_value_is_valid(
            &owned_tag(&["citation", event_id.as_str()]),
            RadrootsTagValueType::Sha256
        ));
        assert!(!tag_value_is_valid(
            &owned_tag(&["e", "not-hex"]),
            RadrootsTagValueType::EventId
        ));
        assert!(tag_value_is_valid(
            &valid_pointer,
            RadrootsTagValueType::EventPointer
        ));
        assert!(tag_value_is_valid(
            &empty_d_pointer,
            RadrootsTagValueType::EventPointer
        ));
        assert!(!event_pointer_tag_is_valid(&owned_tag(&[
            "source",
            "not-hex",
            public_key.as_str(),
            "1",
            ""
        ])));
        assert!(!event_pointer_tag_is_valid(&owned_tag(&[
            "source",
            event_id.as_str(),
            "not-hex",
            "1",
            ""
        ])));
        assert!(!event_pointer_tag_is_valid(&owned_tag(&[
            "source",
            event_id.as_str(),
            public_key.as_str(),
            "not-a-kind",
            ""
        ])));
        assert!(!event_pointer_tag_is_valid(&owned_tag(&[
            "source",
            event_id.as_str(),
            public_key.as_str(),
            "1"
        ])));
        assert!(!event_pointer_tag_is_valid(&owned_tag(&[
            "source",
            event_id.as_str(),
            public_key.as_str(),
            "1",
            "bad tag"
        ])));
        assert!(!event_pointer_tag_is_valid(&owned_tag(&[
            "source",
            event_id.as_str(),
            public_key.as_str(),
            "1",
            "",
            "https://relay.example.com"
        ])));
        assert!(tag_value_is_valid(
            &owned_tag(&["g", "9q8yy"]),
            RadrootsTagValueType::Geohash
        ));
        assert!(tag_value_is_valid(
            &owned_tag(&["g", "9Q8YY"]),
            RadrootsTagValueType::Geohash
        ));
        assert!(!tag_value_is_valid(
            &owned_tag(&["g", ""]),
            RadrootsTagValueType::Geohash
        ));
        assert!(!tag_value_is_valid(
            &owned_tag(&["g", "1234567890123"]),
            RadrootsTagValueType::Geohash
        ));
        assert!(!tag_value_is_valid(
            &owned_tag(&["g", "aaaaa"]),
            RadrootsTagValueType::Geohash
        ));
        assert!(tag_value_is_valid(
            &owned_tag(&["k", "30818"]),
            RadrootsTagValueType::Kind
        ));
        assert!(!tag_value_is_valid(
            &owned_tag(&["k", "not-a-kind"]),
            RadrootsTagValueType::Kind
        ));
        assert!(tag_value_is_valid(
            &owned_tag(&["p", public_key.as_str()]),
            RadrootsTagValueType::PublicKey
        ));
        assert!(!tag_value_is_valid(
            &owned_tag(&["p", "not-hex"]),
            RadrootsTagValueType::PublicKey
        ));
        assert!(tag_value_is_valid(
            &owned_tag(&["relay", "ws://relay.example.com"]),
            RadrootsTagValueType::RelayUrl
        ));
        assert!(tag_value_is_valid(
            &owned_tag(&["relay", "wss://relay.example.com"]),
            RadrootsTagValueType::RelayUrl
        ));
        assert!(!tag_value_is_valid(
            &owned_tag(&["relay", "http://relay.example.com"]),
            RadrootsTagValueType::RelayUrl
        ));
        assert!(relay_url_is_valid("ws://relay.example.com"));
        assert!(relay_url_is_valid("wss://relay.example.com"));
        assert!(!relay_url_is_valid("ws://"));
        assert!(!relay_url_is_valid("http://relay.example.com"));
        assert!(!relay_url_is_valid(" wss://relay.example.com"));
        assert!(!relay_url_is_valid("wss://relay.example.com "));
        assert!(!relay_url_is_valid("wss://relay.example.com\nmiddle"));
        assert!(tag_value_is_valid(
            &owned_tag(&["title", "Soil Guide"]),
            RadrootsTagValueType::Text
        ));
        assert!(!tag_value_is_valid(
            &owned_tag(&["title", "   "]),
            RadrootsTagValueType::Text
        ));
        assert!(!tag_value_is_valid(
            &owned_tag(&["title", "Soil\nGuide"]),
            RadrootsTagValueType::Text
        ));
        assert!(tag_value_is_valid(
            &owned_tag(&["expiration", "1700000000"]),
            RadrootsTagValueType::UnixTimestamp
        ));
        assert!(!tag_value_is_valid(
            &owned_tag(&["expiration", "not-time"]),
            RadrootsTagValueType::UnixTimestamp
        ));
        assert!(tag_value_is_valid(
            &owned_tag(&["image", "https://example.com"]),
            RadrootsTagValueType::Url
        ));
        assert!(!tag_value_is_valid(
            &owned_tag(&["image", "wss://example.com"]),
            RadrootsTagValueType::Url
        ));
        assert!(url_is_valid("http://example.com"));
        assert!(url_is_valid("https://example.com"));
        assert!(!url_is_valid("http://"));
        assert!(!url_is_valid("wss://example.com"));
        assert!(!url_is_valid(" https://example.com"));
        assert!(!url_is_valid("https://example.com "));
        assert!(!url_is_valid("https://example.com\nmiddle"));
        assert!(tag_value_is_valid(
            &owned_tag(&["uuid", "123e4567-e89b-12d3-a456-426614174000"]),
            RadrootsTagValueType::Uuid
        ));
        assert!(!tag_value_is_valid(
            &owned_tag(&["uuid", "123e4567-e89b-12d3-a456-42661417400"]),
            RadrootsTagValueType::Uuid
        ));
        assert!(uuid_is_valid("123e4567-e89b-12d3-a456-426614174000"));
        assert!(!uuid_is_valid("123e4567-e89b-12d3-a456-42661417400"));
        assert!(!uuid_is_valid("123e4567xe89b-12d3-a456-426614174000"));
        assert!(!uuid_is_valid("123e4567-e89b-12d3-a456-42661417400x"));

        let expectations = [
            (
                RadrootsTagValueType::AddressableCoordinate,
                "addressable_coordinate",
            ),
            (RadrootsTagValueType::ContractId, "contract_id"),
            (RadrootsTagValueType::DTag, "d_tag"),
            (RadrootsTagValueType::EventId, "event_id"),
            (RadrootsTagValueType::EventPointer, "event_pointer"),
            (RadrootsTagValueType::Geohash, "geohash"),
            (RadrootsTagValueType::Kind, "kind"),
            (RadrootsTagValueType::PublicKey, "public_key"),
            (RadrootsTagValueType::RelayUrl, "relay_url"),
            (RadrootsTagValueType::Sha256, "sha256"),
            (RadrootsTagValueType::Text, "text"),
            (RadrootsTagValueType::UnixTimestamp, "unix_timestamp"),
            (RadrootsTagValueType::Url, "url"),
            (RadrootsTagValueType::Uuid, "uuid"),
        ];
        for (value_type, expected) in expectations {
            assert_eq!(tag_value_type_expectation(value_type), expected);
        }
    }

    #[test]
    fn validate_custom_knowledge_contract_rejects_missing_schema_and_bad_version() {
        let missing_schema = unsigned_event(
            KIND_KNOWLEDGE_CLAIM,
            vec![vec!["contract", "radroots.knowledge.claim.v1"]],
            r#"{"schema_version":1}"#,
        );
        assert_eq!(
            validate_event_contract_shape(&missing_schema, "radroots.knowledge.claim.v1"),
            Err(RadrootsContractValidationError::MissingContentField {
                contract_id: "radroots.knowledge.claim.v1",
                field: "schema",
            })
        );

        let bad_version = unsigned_event(
            KIND_KNOWLEDGE_CLAIM,
            vec![vec!["contract", "radroots.knowledge.claim.v1"]],
            r#"{"schema":"radroots.knowledge.claim.v1","schema_version":2}"#,
        );
        assert_eq!(
            validate_event_contract_shape(&bad_version, "radroots.knowledge.claim.v1"),
            Err(RadrootsContractValidationError::ContentFieldMismatch {
                contract_id: "radroots.knowledge.claim.v1",
                field: "schema_version",
                expected: "1".to_owned(),
            })
        );
    }

    #[test]
    fn validates_nip54_empty_redirect_content() {
        let event = unsigned_event(
            KIND_WIKI_REDIRECT,
            vec![
                vec!["d", "soil"],
                vec![
                    "a",
                    "30818:0000000000000000000000000000000000000000000000000000000000000000:soil",
                ],
            ],
            "",
        );

        assert_eq!(
            validate_event_contract_shape(&event, "radroots.wiki.redirect.v1"),
            Ok(())
        );

        let invalid = unsigned_event(
            KIND_WIKI_REDIRECT,
            vec![
                vec!["d", "soil"],
                vec![
                    "a",
                    "30818:0000000000000000000000000000000000000000000000000000000000000000:soil",
                ],
            ],
            "{}",
        );
        assert_eq!(
            validate_event_contract_shape(&invalid, "radroots.wiki.redirect.v1"),
            Err(RadrootsContractValidationError::ContentMustBeEmpty {
                contract_id: "radroots.wiki.redirect.v1",
            })
        );
    }

    #[test]
    fn exposes_validation_error_codes() {
        for (error, code) in [
            (
                RadrootsContractValidationError::UnknownContract {
                    contract_id: "missing".to_owned(),
                },
                "unknown_contract",
            ),
            (
                RadrootsContractValidationError::ContractMatch {
                    error: RadrootsContractMatchError::UnsupportedKind(999_999),
                },
                "contract_match",
            ),
            (
                RadrootsContractValidationError::KindMismatch {
                    expected: KIND_PROFILE,
                    actual: KIND_POST,
                },
                "kind_mismatch",
            ),
            (
                RadrootsContractValidationError::ContentMustBeEmpty {
                    contract_id: "radroots.wiki.redirect.v1",
                },
                "content_must_be_empty",
            ),
            (
                RadrootsContractValidationError::InvalidJsonContent {
                    contract_id: "radroots.knowledge.claim.v1",
                },
                "invalid_json_content",
            ),
            (
                RadrootsContractValidationError::MissingTag {
                    contract_id: "radroots.knowledge.claim.v1",
                    name: "contract",
                },
                "missing_tag",
            ),
            (
                RadrootsContractValidationError::TagCardinalityMismatch {
                    contract_id: "radroots.knowledge.claim.v1",
                    name: "contract",
                },
                "tag_cardinality_mismatch",
            ),
            (
                RadrootsContractValidationError::TagValueMismatch {
                    contract_id: "radroots.knowledge.claim.v1",
                    name: "contract",
                    expected: "radroots.knowledge.claim.v1".to_owned(),
                    actual: None,
                },
                "tag_value_mismatch",
            ),
            (
                RadrootsContractValidationError::MissingContentField {
                    contract_id: "radroots.knowledge.claim.v1",
                    field: "schema",
                },
                "missing_content_field",
            ),
            (
                RadrootsContractValidationError::ContentFieldMismatch {
                    contract_id: "radroots.knowledge.claim.v1",
                    field: "schema",
                    expected: "radroots.knowledge.claim.v1".to_owned(),
                },
                "content_field_mismatch",
            ),
            (
                RadrootsContractValidationError::ForbiddenContentField {
                    contract_id: "radroots.knowledge.claim.v1",
                    field: "review_status",
                },
                "forbidden_content_field",
            ),
        ] {
            assert_eq!(error.code(), code);
        }
    }

    #[test]
    fn tag_helpers_cover_missing_names_and_cardinality_mismatches() {
        let tags = vec![
            vec!["p".to_owned(), "counterparty".to_owned()],
            vec!["d".to_owned()],
        ];

        assert_eq!(tag_value(&tags, "d"), None);
        assert_eq!(tag_value(&tags, "p"), Some("counterparty"));

        let malformed = [
            tag(
                "d",
                RadrootsTagCardinality::OptionalOne,
                RadrootsTagSemantic::Identifier,
                RadrootsTagValueType::DTag,
                true,
            ),
            tag(
                "p",
                RadrootsTagCardinality::RequiredOne,
                RadrootsTagSemantic::Counterparty,
                RadrootsTagValueType::PublicKey,
                true,
            ),
        ];

        assert!(
            !malformed.iter().any(
                |tag| tag.name == "d" && tag.cardinality == RadrootsTagCardinality::RequiredOne
            )
        );
    }

    #[test]
    fn relay_indexed_tags_are_single_letter() {
        for contract in all_event_contracts() {
            for tag in contract.tags {
                if tag.relay_indexed {
                    assert_eq!(tag.name.len(), 1, "{}:{}", contract.id, tag.name);
                }
            }
        }
    }

    #[test]
    fn addressable_event_contracts_require_d_tags() {
        for contract in all_event_contracts() {
            if contract.class == RadrootsEventClass::Addressable {
                let d_tag_cardinality = contract
                    .tags
                    .iter()
                    .find(|tag| tag.name == "d")
                    .map(|tag| tag.cardinality);
                assert_eq!(
                    d_tag_cardinality,
                    Some(RadrootsTagCardinality::RequiredOne),
                    "{}",
                    contract.id
                );
            }
        }
    }
}
