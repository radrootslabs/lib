#![forbid(unsafe_code)]

//! Frozen event-contract registry-v7 inventory and validation semantics.

#[cfg(not(feature = "std"))]
use alloc::{borrow::ToOwned, string::String, vec::Vec};

use crate::{
    calendar::{
        RADROOTS_CALENDAR_MAX_PARTICIPANTS, RadrootsCalendarDate, RadrootsCalendarUid,
        RadrootsCalendarUri, RadrootsIanaTimeZoneId, canonical_calendar_geohash_is_valid,
        canonical_calendar_tag_text_is_valid, covered_utc_days,
    },
    classified_listing::{
        RadrootsClassifiedListingPartition, TAG_RADROOTS_PRICE_UNIT, TAG_RADROOTS_QUANTITY,
    },
    envelope::RadrootsEventEnvelope,
    ids::{
        RadrootsAddressableCoordinate, RadrootsDTag, RadrootsEventId, RadrootsNip01Coordinate,
        parse_public_key, relay_url_is_valid,
    },
    kinds::*,
};
use radroots_blossom::url::BlobUrl;

pub const RADROOTS_EVENT_CONTRACT_REGISTRY_VERSION: u32 = 7;

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
    Nip99,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsTagValueType {
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
    /// Exact profile selection is owned by a verified admission algorithm.
    AdmissionOnly,
    /// Exact NIP-99 profile selection uses the central raw marker partition.
    ClassifiedListingPartition(RadrootsClassifiedListingPartition),
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

/// Governs whether a contract may enter the generic frozen-draft boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsEventAuthoringPolicy {
    /// Generic contract validation is sufficient to construct a frozen draft.
    GenericDraft,
    /// Authoring requires a sealed typed API instead of generic draft parts.
    TypedOnly,
    /// The contract is an inbound/read boundary and cannot be authored.
    ReadOnly,
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
    AdmissionRequired {
        contract_id: &'static str,
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
            Self::AdmissionRequired { .. } => "admission_required",
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
    pub authoring_policy: RadrootsEventAuthoringPolicy,
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
const TAG_CALENDAR_PARTICIPANT: RadrootsTagContract = tag(
    "p",
    RadrootsTagCardinality::OptionalMany,
    RadrootsTagSemantic::Participant,
    RadrootsTagValueType::PublicKey,
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
const TAG_CALENDAR_INCLUSION_REQUEST: RadrootsTagContract = tag(
    "a",
    RadrootsTagCardinality::OptionalMany,
    RadrootsTagSemantic::CalendarInclusionRequest,
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
const TAG_NIP09_E_TARGET: RadrootsTagContract = tag(
    "e",
    RadrootsTagCardinality::OptionalMany,
    RadrootsTagSemantic::EventPointer,
    RadrootsTagValueType::EventId,
    true,
);
const TAG_NIP09_A_TARGET: RadrootsTagContract = tag(
    "a",
    RadrootsTagCardinality::OptionalMany,
    RadrootsTagSemantic::Nip01Coordinate,
    RadrootsTagValueType::Nip01Coordinate,
    true,
);
const TAG_NIP09_K_ADVISORY: RadrootsTagContract = tag(
    "k",
    RadrootsTagCardinality::OptionalMany,
    RadrootsTagSemantic::Kind,
    RadrootsTagValueType::Kind,
    true,
);
const TAG_NIP10_E_REQUIRED: RadrootsTagContract = tag(
    "e",
    RadrootsTagCardinality::RequiredMany,
    RadrootsTagSemantic::EventPointer,
    RadrootsTagValueType::EventId,
    true,
);
const TAG_NIP10_P_OPTIONAL: RadrootsTagContract = tag(
    "p",
    RadrootsTagCardinality::OptionalMany,
    RadrootsTagSemantic::Participant,
    RadrootsTagValueType::PublicKey,
    true,
);
const TAG_NIP22_E_ROOT: RadrootsTagContract = tag(
    "E",
    RadrootsTagCardinality::OptionalOne,
    RadrootsTagSemantic::RootEvent,
    RadrootsTagValueType::EventId,
    true,
);
const TAG_NIP22_A_ROOT: RadrootsTagContract = tag(
    "A",
    RadrootsTagCardinality::OptionalOne,
    RadrootsTagSemantic::AddressableCoordinate,
    RadrootsTagValueType::AddressableCoordinate,
    true,
);
const TAG_NIP22_K_ROOT: RadrootsTagContract = tag(
    "K",
    RadrootsTagCardinality::RequiredOne,
    RadrootsTagSemantic::Kind,
    RadrootsTagValueType::Kind,
    true,
);
const TAG_NIP22_P_ROOT: RadrootsTagContract = tag(
    "P",
    RadrootsTagCardinality::RequiredOne,
    RadrootsTagSemantic::Participant,
    RadrootsTagValueType::PublicKey,
    true,
);
const TAG_NIP22_A_PARENT: RadrootsTagContract = tag(
    "a",
    RadrootsTagCardinality::OptionalOne,
    RadrootsTagSemantic::AddressableCoordinate,
    RadrootsTagValueType::AddressableCoordinate,
    true,
);
const TAG_NIP22_E_PARENT: RadrootsTagContract = tag(
    "e",
    RadrootsTagCardinality::RequiredOne,
    RadrootsTagSemantic::EventPointer,
    RadrootsTagValueType::EventId,
    true,
);
const TAG_NIP22_K_PARENT: RadrootsTagContract = tag(
    "k",
    RadrootsTagCardinality::RequiredOne,
    RadrootsTagSemantic::Kind,
    RadrootsTagValueType::Kind,
    true,
);
const TAG_NIP22_P_PARENT: RadrootsTagContract = tag(
    "p",
    RadrootsTagCardinality::RequiredMany,
    RadrootsTagSemantic::Participant,
    RadrootsTagValueType::PublicKey,
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
const TAG_CALENDAR_TITLE: RadrootsTagContract = tag(
    "title",
    RadrootsTagCardinality::RequiredOne,
    RadrootsTagSemantic::Title,
    RadrootsTagValueType::Text,
    false,
);
const TAG_CALENDAR_LEGACY_NAME: RadrootsTagContract = tag(
    "name",
    RadrootsTagCardinality::OptionalOne,
    RadrootsTagSemantic::Title,
    RadrootsTagValueType::Text,
    false,
);
const TAG_CALENDAR_UID: RadrootsTagContract = tag(
    "d",
    RadrootsTagCardinality::RequiredOne,
    RadrootsTagSemantic::Identifier,
    RadrootsTagValueType::CalendarUid,
    true,
);
const TAG_CALENDAR_COLLECTION_DESCRIPTION: RadrootsTagContract = tag(
    "description",
    RadrootsTagCardinality::OptionalOne,
    RadrootsTagSemantic::ListDescription,
    RadrootsTagValueType::Text,
    false,
);
const TAG_CALENDAR_COLLECTION_EVENT: RadrootsTagContract = tag(
    "a",
    RadrootsTagCardinality::OptionalMany,
    RadrootsTagSemantic::CalendarEventReference,
    RadrootsTagValueType::CalendarEventCoordinate,
    true,
);
const TAG_CALENDAR_RSVP_EVENT: RadrootsTagContract = tag(
    "a",
    RadrootsTagCardinality::RequiredOne,
    RadrootsTagSemantic::CalendarEventReference,
    RadrootsTagValueType::CalendarEventCoordinate,
    true,
);
const TAG_CALENDAR_RSVP_REVISION: RadrootsTagContract = tag(
    "e",
    RadrootsTagCardinality::OptionalOne,
    RadrootsTagSemantic::CalendarEventRevision,
    RadrootsTagValueType::EventId,
    true,
);
const TAG_CALENDAR_RSVP_STATUS: RadrootsTagContract = tag(
    "status",
    RadrootsTagCardinality::RequiredOne,
    RadrootsTagSemantic::Status,
    RadrootsTagValueType::CalendarRsvpStatus,
    false,
);
const TAG_CALENDAR_RSVP_FREE_BUSY: RadrootsTagContract = tag(
    "fb",
    RadrootsTagCardinality::OptionalOne,
    RadrootsTagSemantic::FreeBusy,
    RadrootsTagValueType::CalendarFreeBusy,
    false,
);
const TAG_CALENDAR_RSVP_AUTHOR: RadrootsTagContract = tag(
    "p",
    RadrootsTagCardinality::OptionalOne,
    RadrootsTagSemantic::CalendarEventAuthor,
    RadrootsTagValueType::PublicKey,
    true,
);
const TAG_CALENDAR_DATE_START: RadrootsTagContract = tag(
    "start",
    RadrootsTagCardinality::RequiredOne,
    RadrootsTagSemantic::CalendarStart,
    RadrootsTagValueType::CalendarDate,
    false,
);
const TAG_CALENDAR_DATE_END: RadrootsTagContract = tag(
    "end",
    RadrootsTagCardinality::OptionalOne,
    RadrootsTagSemantic::CalendarEnd,
    RadrootsTagValueType::CalendarDate,
    false,
);
const TAG_CALENDAR_TIME_START: RadrootsTagContract = tag(
    "start",
    RadrootsTagCardinality::RequiredOne,
    RadrootsTagSemantic::CalendarStart,
    RadrootsTagValueType::UnixTimestamp,
    false,
);
const TAG_CALENDAR_TIME_END: RadrootsTagContract = tag(
    "end",
    RadrootsTagCardinality::OptionalOne,
    RadrootsTagSemantic::CalendarEnd,
    RadrootsTagValueType::UnixTimestamp,
    false,
);
const TAG_CALENDAR_COVERED_UTC_DAY: RadrootsTagContract = tag(
    "D",
    RadrootsTagCardinality::RequiredMany,
    RadrootsTagSemantic::UtcDayCoverage,
    RadrootsTagValueType::UtcDayIndex,
    true,
);
const TAG_CALENDAR_START_TZID: RadrootsTagContract = tag(
    "start_tzid",
    RadrootsTagCardinality::OptionalOne,
    RadrootsTagSemantic::TimeZone,
    RadrootsTagValueType::IanaTimeZoneId,
    false,
);
const TAG_CALENDAR_END_TZID: RadrootsTagContract = tag(
    "end_tzid",
    RadrootsTagCardinality::OptionalOne,
    RadrootsTagSemantic::TimeZone,
    RadrootsTagValueType::IanaTimeZoneId,
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
const TAG_CALENDAR_LOCATION: RadrootsTagContract = tag(
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
const TAG_IMAGE: RadrootsTagContract = tag(
    "image",
    RadrootsTagCardinality::OptionalMany,
    RadrootsTagSemantic::Image,
    RadrootsTagValueType::Url,
    false,
);
const TAG_FOOD_TITLE: RadrootsTagContract = tag(
    "title",
    RadrootsTagCardinality::RequiredOne,
    RadrootsTagSemantic::Title,
    RadrootsTagValueType::Text,
    false,
);
const TAG_FOOD_SUMMARY: RadrootsTagContract = tag(
    "summary",
    RadrootsTagCardinality::RequiredOne,
    RadrootsTagSemantic::Summary,
    RadrootsTagValueType::Text,
    false,
);
const TAG_FOOD_PUBLISHED_AT: RadrootsTagContract = tag(
    "published_at",
    RadrootsTagCardinality::RequiredOne,
    RadrootsTagSemantic::PublishedAt,
    RadrootsTagValueType::UnixTimestamp,
    false,
);
const TAG_FOOD_LOCATION: RadrootsTagContract = tag(
    "location",
    RadrootsTagCardinality::RequiredOne,
    RadrootsTagSemantic::Location,
    RadrootsTagValueType::Text,
    false,
);
const TAG_FOOD_PRICE: RadrootsTagContract = tag(
    "price",
    RadrootsTagCardinality::RequiredOne,
    RadrootsTagSemantic::Price,
    RadrootsTagValueType::Text,
    false,
);
const TAG_FOOD_PRICE_UNIT: RadrootsTagContract = tag(
    TAG_RADROOTS_PRICE_UNIT,
    RadrootsTagCardinality::RequiredOne,
    RadrootsTagSemantic::Price,
    RadrootsTagValueType::Text,
    false,
);
const TAG_FOOD_QUANTITY: RadrootsTagContract = tag(
    TAG_RADROOTS_QUANTITY,
    RadrootsTagCardinality::OptionalOne,
    RadrootsTagSemantic::Price,
    RadrootsTagValueType::Text,
    false,
);
const TAG_FOOD_STATUS: RadrootsTagContract = tag(
    "status",
    RadrootsTagCardinality::RequiredOne,
    RadrootsTagSemantic::Status,
    RadrootsTagValueType::Text,
    false,
);
const TAG_OPERATIONAL_LISTING_FARM: RadrootsTagContract = tag(
    "a",
    RadrootsTagCardinality::RequiredOne,
    RadrootsTagSemantic::AddressableCoordinate,
    RadrootsTagValueType::AddressableCoordinate,
    true,
);
const TAG_OPERATIONAL_LISTING_PRODUCT_KEY: RadrootsTagContract = tag(
    "key",
    RadrootsTagCardinality::RequiredOne,
    RadrootsTagSemantic::Category,
    RadrootsTagValueType::Text,
    false,
);
const TAG_OPERATIONAL_LISTING_TITLE: RadrootsTagContract = tag(
    "title",
    RadrootsTagCardinality::RequiredOne,
    RadrootsTagSemantic::Title,
    RadrootsTagValueType::Text,
    false,
);
const TAG_OPERATIONAL_LISTING_CATEGORY: RadrootsTagContract = tag(
    "category",
    RadrootsTagCardinality::RequiredOne,
    RadrootsTagSemantic::Category,
    RadrootsTagValueType::Text,
    false,
);
const TAG_OPERATIONAL_LISTING_PRIMARY_BIN: RadrootsTagContract = tag(
    "radroots:primary_bin",
    RadrootsTagCardinality::RequiredOne,
    RadrootsTagSemantic::OperationalListingSnapshot,
    RadrootsTagValueType::Text,
    false,
);
const TAG_OPERATIONAL_LISTING_BIN: RadrootsTagContract = tag(
    "radroots:bin",
    RadrootsTagCardinality::RequiredMany,
    RadrootsTagSemantic::OperationalListingSnapshot,
    RadrootsTagValueType::Text,
    false,
);
const TAG_OPERATIONAL_LISTING_PRICE: RadrootsTagContract = tag(
    "radroots:price",
    RadrootsTagCardinality::RequiredMany,
    RadrootsTagSemantic::Price,
    RadrootsTagValueType::Text,
    false,
);
const TAG_OPERATIONAL_LISTING_DISCOUNT: RadrootsTagContract = tag(
    "radroots:discount",
    RadrootsTagCardinality::OptionalMany,
    RadrootsTagSemantic::Price,
    RadrootsTagValueType::Text,
    false,
);
const TAG_OPERATIONAL_LISTING_RESOURCE_AREA: RadrootsTagContract = tag(
    "radroots:resource_area",
    RadrootsTagCardinality::OptionalOne,
    RadrootsTagSemantic::AddressableCoordinate,
    RadrootsTagValueType::AddressableCoordinate,
    false,
);
const TAG_OPERATIONAL_LISTING_PLOT: RadrootsTagContract = tag(
    "radroots:plot",
    RadrootsTagCardinality::OptionalOne,
    RadrootsTagSemantic::AddressableCoordinate,
    RadrootsTagValueType::AddressableCoordinate,
    false,
);
const TAG_OPERATIONAL_LISTING_INVENTORY: RadrootsTagContract = tag(
    "inventory",
    RadrootsTagCardinality::OptionalOne,
    RadrootsTagSemantic::OperationalListingSnapshot,
    RadrootsTagValueType::Text,
    false,
);
const TAG_OPERATIONAL_LISTING_AVAILABILITY_START: RadrootsTagContract = tag(
    "radroots:availability_start",
    RadrootsTagCardinality::OptionalOne,
    RadrootsTagSemantic::Status,
    RadrootsTagValueType::UnixTimestamp,
    false,
);
const TAG_OPERATIONAL_LISTING_EXPIRES_AT: RadrootsTagContract = tag(
    "expires_at",
    RadrootsTagCardinality::OptionalOne,
    RadrootsTagSemantic::Status,
    RadrootsTagValueType::UnixTimestamp,
    false,
);
const TAG_OPERATIONAL_LISTING_DELIVERY: RadrootsTagContract = tag(
    "delivery",
    RadrootsTagCardinality::OptionalOne,
    RadrootsTagSemantic::Reference,
    RadrootsTagValueType::Text,
    false,
);
const TAG_OPERATIONAL_LISTING_PROCESS: RadrootsTagContract = tag(
    "process",
    RadrootsTagCardinality::OptionalOne,
    RadrootsTagSemantic::Category,
    RadrootsTagValueType::Text,
    false,
);
const TAG_OPERATIONAL_LISTING_LOT: RadrootsTagContract = tag(
    "lot",
    RadrootsTagCardinality::OptionalOne,
    RadrootsTagSemantic::Reference,
    RadrootsTagValueType::Text,
    false,
);
const TAG_OPERATIONAL_LISTING_PROFILE: RadrootsTagContract = tag(
    "profile",
    RadrootsTagCardinality::OptionalOne,
    RadrootsTagSemantic::Category,
    RadrootsTagValueType::Text,
    false,
);
const TAG_OPERATIONAL_LISTING_YEAR: RadrootsTagContract = tag(
    "year",
    RadrootsTagCardinality::OptionalOne,
    RadrootsTagSemantic::Category,
    RadrootsTagValueType::Text,
    false,
);
const TAG_CALENDAR_IMAGE: RadrootsTagContract = tag(
    "image",
    RadrootsTagCardinality::OptionalOne,
    RadrootsTagSemantic::Image,
    RadrootsTagValueType::Url,
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
const TAG_ASK_MARKER: RadrootsTagContract = tag(
    "t",
    RadrootsTagCardinality::RequiredOne,
    RadrootsTagSemantic::Topic,
    RadrootsTagValueType::Text,
    true,
);
const TAG_IMETA_REQUIRED_MANY: RadrootsTagContract = tag(
    "imeta",
    RadrootsTagCardinality::RequiredMany,
    RadrootsTagSemantic::Image,
    RadrootsTagValueType::Text,
    false,
);
const TAG_IMETA_OPTIONAL_MANY: RadrootsTagContract = tag(
    "imeta",
    RadrootsTagCardinality::OptionalMany,
    RadrootsTagSemantic::Image,
    RadrootsTagValueType::Text,
    false,
);
const TAG_CALENDAR_REFERENCE: RadrootsTagContract = tag(
    "r",
    RadrootsTagCardinality::OptionalMany,
    RadrootsTagSemantic::Reference,
    RadrootsTagValueType::Uri,
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
const NIP09_DELETION_TAGS: &[RadrootsTagContract] =
    &[TAG_NIP09_E_TARGET, TAG_NIP09_A_TARGET, TAG_NIP09_K_ADVISORY];
const NIP22_COMMENT_TAGS: &[RadrootsTagContract] = &[
    TAG_NIP22_E_ROOT,
    TAG_NIP22_A_ROOT,
    TAG_NIP22_K_ROOT,
    TAG_NIP22_P_ROOT,
    TAG_NIP22_A_PARENT,
    TAG_NIP22_E_PARENT,
    TAG_NIP22_K_PARENT,
    TAG_NIP22_P_PARENT,
];
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
const CALENDAR_DATE_EVENT_TAGS: &[RadrootsTagContract] = &[
    TAG_D,
    TAG_CALENDAR_TITLE,
    TAG_CALENDAR_LEGACY_NAME,
    TAG_CALENDAR_DATE_START,
    TAG_CALENDAR_DATE_END,
    TAG_CALENDAR_LOCATION,
    TAG_GEOHASH_OPTIONAL,
    TAG_SUMMARY,
    TAG_CALENDAR_IMAGE,
    TAG_CALENDAR_PARTICIPANT,
    TAG_TOPIC_MANY,
    TAG_CALENDAR_REFERENCE,
    TAG_CALENDAR_INCLUSION_REQUEST,
];
const CALENDAR_TIME_EVENT_TAGS: &[RadrootsTagContract] = &[
    TAG_D,
    TAG_CALENDAR_TITLE,
    TAG_CALENDAR_LEGACY_NAME,
    TAG_CALENDAR_TIME_START,
    TAG_CALENDAR_TIME_END,
    TAG_CALENDAR_COVERED_UTC_DAY,
    TAG_CALENDAR_START_TZID,
    TAG_CALENDAR_END_TZID,
    TAG_CALENDAR_LOCATION,
    TAG_GEOHASH_OPTIONAL,
    TAG_SUMMARY,
    TAG_CALENDAR_IMAGE,
    TAG_CALENDAR_PARTICIPANT,
    TAG_TOPIC_MANY,
    TAG_CALENDAR_REFERENCE,
    TAG_CALENDAR_INCLUSION_REQUEST,
];
const CALENDAR_COLLECTION_TAGS: &[RadrootsTagContract] = &[
    TAG_CALENDAR_UID,
    TAG_CALENDAR_TITLE,
    TAG_CALENDAR_COLLECTION_DESCRIPTION,
    TAG_CALENDAR_IMAGE,
    TAG_CALENDAR_COLLECTION_EVENT,
];
const CALENDAR_RSVP_TAGS: &[RadrootsTagContract] = &[
    TAG_CALENDAR_UID,
    TAG_CALENDAR_RSVP_EVENT,
    TAG_CALENDAR_RSVP_REVISION,
    TAG_CALENDAR_RSVP_STATUS,
    TAG_CALENDAR_RSVP_FREE_BUSY,
    TAG_CALENDAR_RSVP_AUTHOR,
];
const FARM_TAGS: &[RadrootsTagContract] = &[TAG_D, TAG_TITLE, TAG_LOCATION, TAG_IMAGE];
const FOOD_AVAILABILITY_TAGS: &[RadrootsTagContract] = &[
    TAG_D,
    TAG_FOOD_TITLE,
    TAG_FOOD_SUMMARY,
    TAG_FOOD_PUBLISHED_AT,
    TAG_FOOD_LOCATION,
    TAG_FOOD_PRICE,
    TAG_FOOD_PRICE_UNIT,
    TAG_FOOD_QUANTITY,
    TAG_FOOD_STATUS,
    TAG_IMAGE,
];
const OPERATIONAL_LISTING_TAGS: &[RadrootsTagContract] = &[
    TAG_D,
    TAG_P_REQUIRED,
    TAG_OPERATIONAL_LISTING_FARM,
    TAG_OPERATIONAL_LISTING_PRODUCT_KEY,
    TAG_OPERATIONAL_LISTING_TITLE,
    TAG_OPERATIONAL_LISTING_CATEGORY,
    TAG_SUMMARY,
    TAG_PUBLISHED_AT,
    TAG_OPERATIONAL_LISTING_PROCESS,
    TAG_OPERATIONAL_LISTING_LOT,
    TAG_OPERATIONAL_LISTING_PROFILE,
    TAG_OPERATIONAL_LISTING_YEAR,
    TAG_LOCATION,
    TAG_PRICE,
    TAG_STATUS,
    TAG_IMAGE,
    TAG_GEOHASH_OPTIONAL,
    TAG_OPERATIONAL_LISTING_PRIMARY_BIN,
    TAG_OPERATIONAL_LISTING_BIN,
    TAG_OPERATIONAL_LISTING_PRICE,
    TAG_OPERATIONAL_LISTING_DISCOUNT,
    TAG_OPERATIONAL_LISTING_RESOURCE_AREA,
    TAG_OPERATIONAL_LISTING_PLOT,
    TAG_OPERATIONAL_LISTING_INVENTORY,
    TAG_OPERATIONAL_LISTING_AVAILABILITY_START,
    TAG_OPERATIONAL_LISTING_EXPIRES_AT,
    TAG_OPERATIONAL_LISTING_DELIVERY,
];
const TRADE_MUTATION_TAGS: &[RadrootsTagContract] =
    &[TAG_CONTRACT_REQUIRED, TAG_D, TAG_P_REQUIRED, TAG_E_MANY];
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
const PHOTO_UPDATE_TAGS: &[RadrootsTagContract] = &[TAG_IMETA_REQUIRED_MANY];
const ASK_TAGS: &[RadrootsTagContract] = &[TAG_ASK_MARKER, TAG_IMETA_OPTIONAL_MANY];
const NIP10_REPLY_TAGS: &[RadrootsTagContract] = &[TAG_NIP10_E_REQUIRED, TAG_NIP10_P_OPTIONAL];
const PROFILE_REDUCERS: &[RadrootsReducer] = &[RadrootsReducer::ProfileProjection];
const FARM_OPS_REDUCERS: &[RadrootsReducer] = &[RadrootsReducer::FarmOpsProjection];
const GROUP_REDUCERS: &[RadrootsReducer] = &[RadrootsReducer::GroupProjection];
const CALENDAR_REDUCERS: &[RadrootsReducer] = &[RadrootsReducer::CalendarProjection];
const OPERATIONAL_LISTING_REDUCERS: &[RadrootsReducer] = &[
    RadrootsReducer::OperationalListingProjection,
    RadrootsReducer::MarketProjection,
    RadrootsReducer::OperationalListingInventoryAccounting,
];
const FOOD_AVAILABILITY_REDUCERS: &[RadrootsReducer] = &[RadrootsReducer::MarketProjection];
const TRADE_MUTATION_REDUCERS: &[RadrootsReducer] = &[
    RadrootsReducer::TradeProjection,
    RadrootsReducer::OperationalListingInventoryAccounting,
];
const TRADE_VALIDATION_REDUCERS: &[RadrootsReducer] = &[RadrootsReducer::TradeValidation];
const RELAY_REDUCERS: &[RadrootsReducer] = &[RadrootsReducer::NostrRelayPolicyProjection];
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
            authoring_policy: RadrootsEventAuthoringPolicy::GenericDraft,
            discriminator: $discriminator,
            tags: $tags,
            reducers: $reducers,
        }
    };
}

macro_rules! event_contract_with_authoring_policy {
    (
        $id:literal,
        $kind:expr,
        $name:literal,
        $payload_type:literal,
        $class:expr,
        $standard_privacy:expr,
        $author_role:expr,
        $content_schema:expr,
        $authoring_policy:expr,
        $discriminator:expr,
        $tags:expr,
        $reducers:expr $(,)?
    ) => {
        RadrootsEventContract {
            authoring_policy: $authoring_policy,
            ..event_contract!(
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
                $reducers
            )
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

static KIND_CONTRACTS_REGISTRY_V7: &[RadrootsKindContract] = &[
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
        [
            "radroots.social.post.v1",
            "radroots.social.update.v1",
            "radroots.social.photo_update.v1",
            "radroots.social.ask.v1",
            "radroots.social.reply.v1"
        ]
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
        KIND_DELETION_REQUEST,
        "KIND_DELETION_REQUEST",
        "Deletion Request",
        RadrootsEventClass::Regular,
        RadrootsNostrStandard::Nip09,
        ["radroots.social.deletion_request.v1"]
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
        KIND_CLASSIFIED_LISTING,
        "KIND_CLASSIFIED_LISTING",
        "Classified Listing",
        RadrootsEventClass::Addressable,
        RadrootsNostrStandard::Nip99,
        [
            "radroots.operational_listing.published.v1",
            "radroots.food.availability.v1"
        ]
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
        KIND_TRADE_PROPOSAL,
        "KIND_TRADE_PROPOSAL",
        "Trade Proposal",
        RadrootsEventClass::Regular,
        RadrootsNostrStandard::Radroots,
        ["radroots.trade.proposal.v1"]
    ),
    kind_contract!(
        KIND_TRADE_DECISION,
        "KIND_TRADE_DECISION",
        "Trade Decision",
        RadrootsEventClass::Regular,
        RadrootsNostrStandard::Radroots,
        ["radroots.trade.decision.v1"]
    ),
    kind_contract!(
        KIND_TRADE_REVISION_PROPOSAL,
        "KIND_TRADE_REVISION_PROPOSAL",
        "Trade Revision Proposal",
        RadrootsEventClass::Regular,
        RadrootsNostrStandard::Radroots,
        ["radroots.trade.revision_proposal.v1"]
    ),
    kind_contract!(
        KIND_TRADE_REVISION_DECISION,
        "KIND_TRADE_REVISION_DECISION",
        "Trade Revision Decision",
        RadrootsEventClass::Regular,
        RadrootsNostrStandard::Radroots,
        ["radroots.trade.revision_decision.v1"]
    ),
    kind_contract!(
        KIND_TRADE_CANCELLATION,
        "KIND_TRADE_CANCELLATION",
        "Trade Cancellation",
        RadrootsEventClass::Regular,
        RadrootsNostrStandard::Radroots,
        ["radroots.trade.cancellation.v1"]
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

static EVENT_CONTRACTS_REGISTRY_V7: &[RadrootsEventContract] = &[
    event_contract_with_authoring_policy!(
        "radroots.profile.metadata.v1",
        KIND_PROFILE,
        "Profile Metadata",
        "RadrootsAuthoredProfile / RadrootsInboundProfileMetadata",
        RadrootsEventClass::Replaceable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::JsonObject,
        RadrootsEventAuthoringPolicy::TypedOnly,
        RadrootsEventDiscriminator::KindOnly,
        PROFILE_TAGS,
        PROFILE_REDUCERS
    ),
    event_contract_with_authoring_policy!(
        "radroots.social.post.v1",
        KIND_POST,
        "Short Text Note",
        "RadrootsPost",
        RadrootsEventClass::Regular,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::PlainText,
        RadrootsEventAuthoringPolicy::ReadOnly,
        RadrootsEventDiscriminator::KindOnly,
        NO_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract_with_authoring_policy!(
        "radroots.social.update.v1",
        KIND_POST,
        "Root Text Update",
        "RadrootsAuthoredUpdate / RadrootsInboundPostProjection",
        RadrootsEventClass::Regular,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::PlainText,
        RadrootsEventAuthoringPolicy::TypedOnly,
        RadrootsEventDiscriminator::AdmissionOnly,
        NO_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract_with_authoring_policy!(
        "radroots.social.photo_update.v1",
        KIND_POST,
        "NIP-92 Photo Update",
        "RadrootsAuthoredPhotoUpdate / RadrootsInboundPostProjection",
        RadrootsEventClass::Regular,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::PlainText,
        RadrootsEventAuthoringPolicy::TypedOnly,
        RadrootsEventDiscriminator::AdmissionOnly,
        PHOTO_UPDATE_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract_with_authoring_policy!(
        "radroots.social.ask.v1",
        KIND_POST,
        "Root Ask",
        "RadrootsAuthoredAsk / RadrootsInboundPostProjection",
        RadrootsEventClass::Regular,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::PlainText,
        RadrootsEventAuthoringPolicy::TypedOnly,
        RadrootsEventDiscriminator::AdmissionOnly,
        ASK_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract_with_authoring_policy!(
        "radroots.social.reply.v1",
        KIND_POST,
        "NIP-10 Reply",
        "RadrootsAuthoredNip10Reply / RadrootsInboundNip10ReplyProjection",
        RadrootsEventClass::Regular,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::PlainText,
        RadrootsEventAuthoringPolicy::TypedOnly,
        RadrootsEventDiscriminator::AdmissionOnly,
        NIP10_REPLY_TAGS,
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
    event_contract_with_authoring_policy!(
        "radroots.social.deletion_request.v1",
        KIND_DELETION_REQUEST,
        "Deletion Request",
        "RadrootsAuthoredNip09DeletionRequest / RadrootsInboundNip09DeletionProjection",
        RadrootsEventClass::Regular,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::PlainText,
        RadrootsEventAuthoringPolicy::TypedOnly,
        RadrootsEventDiscriminator::AdmissionOnly,
        NIP09_DELETION_TAGS,
        SOCIAL_REDUCERS
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
    event_contract_with_authoring_policy!(
        "radroots.social.comment.v1",
        KIND_COMMENT,
        "Comment",
        "RadrootsAuthoredNip22Comment / RadrootsInboundNip22CommentProjection",
        RadrootsEventClass::Regular,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::PlainText,
        RadrootsEventAuthoringPolicy::TypedOnly,
        RadrootsEventDiscriminator::AdmissionOnly,
        NIP22_COMMENT_TAGS,
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
        "RadrootsAdmittedCalendarDateEvent",
        RadrootsEventClass::Addressable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::PlainText,
        RadrootsEventDiscriminator::KindOnly,
        CALENDAR_DATE_EVENT_TAGS,
        CALENDAR_REDUCERS
    ),
    event_contract!(
        "radroots.calendar.time_event.v1",
        KIND_CALENDAR_TIME_EVENT,
        "Calendar Time Event",
        "RadrootsAdmittedCalendarTimeEvent",
        RadrootsEventClass::Addressable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::PlainText,
        RadrootsEventDiscriminator::KindOnly,
        CALENDAR_TIME_EVENT_TAGS,
        CALENDAR_REDUCERS
    ),
    event_contract!(
        "radroots.calendar.collection.v1",
        KIND_CALENDAR,
        "Calendar Collection",
        "RadrootsAdmittedCalendar",
        RadrootsEventClass::Addressable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::PlainText,
        RadrootsEventDiscriminator::KindOnly,
        CALENDAR_COLLECTION_TAGS,
        CALENDAR_REDUCERS
    ),
    event_contract!(
        "radroots.calendar.rsvp.v1",
        KIND_CALENDAR_EVENT_RSVP,
        "Calendar RSVP",
        "RadrootsAdmittedCalendarEventRsvp",
        RadrootsEventClass::Addressable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::PlainText,
        RadrootsEventDiscriminator::KindOnly,
        CALENDAR_RSVP_TAGS,
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
        "radroots.operational_listing.published.v1",
        KIND_CLASSIFIED_LISTING,
        "Operational Listing",
        "RadrootsOperationalListing",
        RadrootsEventClass::Addressable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Seller,
        RadrootsContentSchema::Markdown,
        RadrootsEventDiscriminator::ClassifiedListingPartition(
            RadrootsClassifiedListingPartition::OperationalListing,
        ),
        OPERATIONAL_LISTING_TAGS,
        OPERATIONAL_LISTING_REDUCERS
    ),
    event_contract_with_authoring_policy!(
        "radroots.food.availability.v1",
        KIND_CLASSIFIED_LISTING,
        "Food Availability",
        "RadrootsFoodAvailabilityDetails / RadrootsInboundFoodAvailabilityProjection",
        RadrootsEventClass::Addressable,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Seller,
        RadrootsContentSchema::Markdown,
        RadrootsEventAuthoringPolicy::TypedOnly,
        RadrootsEventDiscriminator::AdmissionOnly,
        FOOD_AVAILABILITY_TAGS,
        FOOD_AVAILABILITY_REDUCERS
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
        "radroots.trade.proposal.v1",
        KIND_TRADE_PROPOSAL,
        "Trade Proposal",
        "RadrootsTradeMutationEnvelopeV1",
        RadrootsEventClass::Regular,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Buyer,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::ContentJsonFieldEquals {
            field: "contract_id",
            value: "radroots.trade.proposal.v1",
        },
        TRADE_MUTATION_TAGS,
        TRADE_MUTATION_REDUCERS
    ),
    event_contract!(
        "radroots.trade.decision.v1",
        KIND_TRADE_DECISION,
        "Trade Decision",
        "RadrootsTradeMutationEnvelopeV1",
        RadrootsEventClass::Regular,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Seller,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::ContentJsonFieldEquals {
            field: "contract_id",
            value: "radroots.trade.decision.v1",
        },
        TRADE_MUTATION_TAGS,
        TRADE_MUTATION_REDUCERS
    ),
    event_contract!(
        "radroots.trade.revision_proposal.v1",
        KIND_TRADE_REVISION_PROPOSAL,
        "Trade Revision Proposal",
        "RadrootsTradeMutationEnvelopeV1",
        RadrootsEventClass::Regular,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::ContentJsonFieldEquals {
            field: "contract_id",
            value: "radroots.trade.revision_proposal.v1",
        },
        TRADE_MUTATION_TAGS,
        TRADE_MUTATION_REDUCERS
    ),
    event_contract!(
        "radroots.trade.revision_decision.v1",
        KIND_TRADE_REVISION_DECISION,
        "Trade Revision Decision",
        "RadrootsTradeMutationEnvelopeV1",
        RadrootsEventClass::Regular,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Any,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::ContentJsonFieldEquals {
            field: "contract_id",
            value: "radroots.trade.revision_decision.v1",
        },
        TRADE_MUTATION_TAGS,
        TRADE_MUTATION_REDUCERS
    ),
    event_contract!(
        "radroots.trade.cancellation.v1",
        KIND_TRADE_CANCELLATION,
        "Trade Cancellation",
        "RadrootsTradeMutationEnvelopeV1",
        RadrootsEventClass::Regular,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Buyer,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::ContentJsonFieldEquals {
            field: "contract_id",
            value: "radroots.trade.cancellation.v1",
        },
        TRADE_MUTATION_TAGS,
        TRADE_MUTATION_REDUCERS
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
    all_kind_contracts_registry_v7()
}

/// Returns the immutable kind-contract inventory for registry v7.
pub const fn all_kind_contracts_registry_v7() -> &'static [RadrootsKindContract] {
    KIND_CONTRACTS_REGISTRY_V7
}

pub fn all_event_contracts() -> &'static [RadrootsEventContract] {
    all_event_contracts_registry_v7()
}

/// Returns the immutable event-contract inventory for registry v7.
pub const fn all_event_contracts_registry_v7() -> &'static [RadrootsEventContract] {
    EVENT_CONTRACTS_REGISTRY_V7
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
        KIND_COMMENT
        | KIND_DELETION_REQUEST
        | KIND_GEOCHAT
        | KIND_POST
        | KIND_REACTION
        | KIND_REPOST
        | KIND_GENERIC_REPOST
        | KIND_ARTICLE
        | KIND_FILE_METADATA => RadrootsContractFamily::Social,
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
        KIND_CLASSIFIED_LISTING => RadrootsContractFamily::Market,
        KIND_TRADE_VALIDATION_RECEIPT
        | KIND_TRADE_PROPOSAL
        | KIND_TRADE_DECISION
        | KIND_TRADE_REVISION_PROPOSAL
        | KIND_TRADE_REVISION_DECISION
        | KIND_TRADE_CANCELLATION => RadrootsContractFamily::Trade,
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
    kind_contract_registry_v7(kind)
}

/// Resolves a kind contract from the immutable registry-v7 inventory.
pub fn kind_contract_registry_v7(kind: u32) -> Option<&'static RadrootsKindContract> {
    KIND_CONTRACTS_REGISTRY_V7
        .iter()
        .find(|contract| contract.kind == kind)
}

pub fn event_contract(id: &str) -> Option<&'static RadrootsEventContract> {
    event_contract_registry_v7(id)
}

/// Resolves an event contract from the immutable registry-v7 inventory.
///
/// Event-store reconciliation v1 depends on this historical entry point.
/// Later registries must retain it and add a new versioned lookup.
pub fn event_contract_registry_v7(id: &str) -> Option<&'static RadrootsEventContract> {
    EVENT_CONTRACTS_REGISTRY_V7
        .iter()
        .find(|contract| contract.id == id)
}

pub fn event_contracts_for_kind(kind: u32) -> impl Iterator<Item = &'static RadrootsEventContract> {
    EVENT_CONTRACTS_REGISTRY_V7
        .iter()
        .filter(move |contract| contract.kind == kind)
}

pub fn identify_event_contract(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<&'static RadrootsEventContract, RadrootsContractMatchError> {
    identify_event_contract_in_registry(
        kind,
        tags,
        content,
        KIND_CONTRACTS_REGISTRY_V7,
        EVENT_CONTRACTS_REGISTRY_V7,
    )
}

fn identify_event_contract_in_registry(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
    kind_contracts: &'static [RadrootsKindContract],
    event_contracts: &'static [RadrootsEventContract],
) -> Result<&'static RadrootsEventContract, RadrootsContractMatchError> {
    if !kind_contracts.iter().any(|contract| contract.kind == kind) {
        return Err(RadrootsContractMatchError::UnsupportedKind(kind));
    }
    identify_from_contracts(
        event_contracts
            .iter()
            .filter(move |contract| contract.kind == kind),
        kind,
        tags,
        content,
    )
}

pub fn validate_event_contract(
    event: &RadrootsEventEnvelope,
) -> Result<&'static RadrootsEventContract, RadrootsContractValidationError> {
    validate_event_contract_in_registry(
        event,
        KIND_CONTRACTS_REGISTRY_V7,
        EVENT_CONTRACTS_REGISTRY_V7,
    )
}

/// Validates against the immutable event-contract inventory used by registry 7.
///
/// Event-store migration 0002 depends on this historical entry point. Later
/// registries must retain it and add a new versioned validator.
pub fn validate_event_contract_registry_v7(
    event: &RadrootsEventEnvelope,
) -> Result<&'static RadrootsEventContract, RadrootsContractValidationError> {
    validate_event_contract_in_registry(
        event,
        KIND_CONTRACTS_REGISTRY_V7,
        EVENT_CONTRACTS_REGISTRY_V7,
    )
}

fn validate_event_contract_in_registry(
    event: &RadrootsEventEnvelope,
    kind_contracts: &'static [RadrootsKindContract],
    event_contracts: &'static [RadrootsEventContract],
) -> Result<&'static RadrootsEventContract, RadrootsContractValidationError> {
    let tags = event.tags_as_vec();
    let contract = identify_event_contract_in_registry(
        event.kind_u32(),
        &tags,
        event.content(),
        kind_contracts,
        event_contracts,
    )
    .map_err(|error| RadrootsContractValidationError::ContractMatch { error })?;
    validate_event_contract_parts_in_registry(
        event.kind_u32(),
        &tags,
        event.content(),
        contract,
        event_contracts,
    )?;
    Ok(contract)
}

pub fn validate_event_contract_shape(
    event: &RadrootsEventEnvelope,
    contract_id: &str,
) -> Result<(), RadrootsContractValidationError> {
    let tags = event.tags_as_vec();
    validate_event_contract_parts(event.kind_u32(), &tags, event.content(), contract_id)
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
    validate_event_contract_parts_in_registry(
        kind,
        tags,
        content,
        contract,
        EVENT_CONTRACTS_REGISTRY_V7,
    )
}

fn validate_event_contract_parts_in_registry(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
    contract: &RadrootsEventContract,
    event_contracts: &'static [RadrootsEventContract],
) -> Result<(), RadrootsContractValidationError> {
    if kind != contract.kind {
        return Err(RadrootsContractValidationError::KindMismatch {
            expected: contract.kind,
            actual: kind,
        });
    }
    if matches!(
        contract.discriminator,
        RadrootsEventDiscriminator::AdmissionOnly
    ) {
        return Err(RadrootsContractValidationError::AdmissionRequired {
            contract_id: contract.id,
        });
    }
    validate_classified_listing_partition_parts(tags, contract)?;
    validate_content_shape_parts(content, contract)?;
    validate_contract_tags_parts_in_registry(tags, contract, event_contracts)?;
    validate_discriminator_parts(content, contract)?;
    validate_custom_calendar_contract_parts(tags, contract)?;
    validate_custom_knowledge_contract_parts(content, contract)?;
    Ok(())
}

fn validate_classified_listing_partition_parts(
    tags: &[Vec<String>],
    contract: &RadrootsEventContract,
) -> Result<(), RadrootsContractValidationError> {
    let RadrootsEventDiscriminator::ClassifiedListingPartition(expected) = contract.discriminator
    else {
        return Ok(());
    };
    if classify_classified_listing_raw_tags_registry_v7(tags) == expected {
        Ok(())
    } else {
        Err(RadrootsContractValidationError::ContractMatch {
            error: RadrootsContractMatchError::UnsupportedShape(contract.kind),
        })
    }
}

fn classify_classified_listing_raw_tags_registry_v7(
    tags: &[Vec<String>],
) -> RadrootsClassifiedListingPartition {
    let mut has_focused_marker = false;
    let mut has_operational_marker = false;

    for name in tags
        .iter()
        .filter_map(|tag| tag.first().map(String::as_str))
    {
        match name {
            "radroots:price_unit" | "radroots:quantity" => has_focused_marker = true,
            "radroots:primary_bin" | "radroots:bin" | "radroots:price" => {
                has_operational_marker = true;
            }
            _ => {}
        }

        if has_focused_marker && has_operational_marker {
            return RadrootsClassifiedListingPartition::Ambiguous;
        }
    }

    match (has_focused_marker, has_operational_marker) {
        (true, false) => RadrootsClassifiedListingPartition::FocusedFoodAvailability,
        (false, true) => RadrootsClassifiedListingPartition::OperationalListing,
        (false, false) => RadrootsClassifiedListingPartition::GenericNip99,
        (true, true) => RadrootsClassifiedListingPartition::Ambiguous,
    }
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
    } else if id.starts_with("radroots.operational_listing.") || id.starts_with("radroots.food.") {
        Some(RadrootsContractFamily::Market)
    } else if id.starts_with("radroots.message.") {
        Some(RadrootsContractFamily::Message)
    } else if id.starts_with("radroots.profile.") {
        Some(RadrootsContractFamily::Profile)
    } else if id.starts_with("radroots.relay.") {
        Some(RadrootsContractFamily::Relay)
    } else if id.starts_with("radroots.social.") {
        Some(RadrootsContractFamily::Social)
    } else if id.starts_with("radroots.trade.") {
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

#[cfg(test)]
fn validate_contract_tags_parts(
    tags: &[Vec<String>],
    contract: &RadrootsEventContract,
) -> Result<(), RadrootsContractValidationError> {
    validate_contract_tags_parts_in_registry(tags, contract, EVENT_CONTRACTS_REGISTRY_V7)
}

fn validate_contract_tags_parts_in_registry(
    tags: &[Vec<String>],
    contract: &RadrootsEventContract,
    event_contracts: &'static [RadrootsEventContract],
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
        validate_contract_tag_values_in_registry(tags, contract, tag_contract, event_contracts)?;
    }
    Ok(())
}

fn validate_contract_tag_values_in_registry(
    tags: &[Vec<String>],
    contract: &RadrootsEventContract,
    tag_contract: &RadrootsTagContract,
    event_contracts: &'static [RadrootsEventContract],
) -> Result<(), RadrootsContractValidationError> {
    for tag in tags
        .iter()
        .filter(|tag| tag.first().map(|value| value.as_str()) == Some(tag_contract.name))
    {
        if !tag_value_is_valid_in_registry(tag, tag_contract.value_type, event_contracts) {
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

#[cfg(test)]
fn tag_value_is_valid(tag: &[String], value_type: RadrootsTagValueType) -> bool {
    tag_value_is_valid_in_registry(tag, value_type, EVENT_CONTRACTS_REGISTRY_V7)
}

fn tag_value_is_valid_in_registry(
    tag: &[String],
    value_type: RadrootsTagValueType,
    event_contracts: &'static [RadrootsEventContract],
) -> bool {
    let Some(value) = tag.get(1).map(String::as_str) else {
        return false;
    };
    match value_type {
        RadrootsTagValueType::AddressableCoordinate => {
            RadrootsAddressableCoordinate::parse(value).is_ok()
        }
        RadrootsTagValueType::CalendarDate => RadrootsCalendarDate::parse(value).is_ok(),
        RadrootsTagValueType::CalendarEventCoordinate => {
            canonical_calendar_event_coordinate_is_valid(value)
        }
        RadrootsTagValueType::CalendarFreeBusy => matches!(value, "free" | "busy"),
        RadrootsTagValueType::CalendarRsvpStatus => {
            matches!(value, "accepted" | "declined" | "tentative")
        }
        RadrootsTagValueType::CalendarUid => RadrootsCalendarUid::parse(value).is_ok(),
        RadrootsTagValueType::ContractId => {
            event_contracts.iter().any(|contract| contract.id == value)
        }
        RadrootsTagValueType::DTag => RadrootsDTag::parse(value).is_ok(),
        RadrootsTagValueType::EventId | RadrootsTagValueType::Sha256 => {
            RadrootsEventId::parse(value).is_ok()
        }
        RadrootsTagValueType::EventPointer => event_pointer_tag_is_valid(tag),
        RadrootsTagValueType::Geohash => geohash_is_valid(value),
        RadrootsTagValueType::IanaTimeZoneId => RadrootsIanaTimeZoneId::parse(value).is_ok(),
        RadrootsTagValueType::Kind => value.parse::<u32>().is_ok(),
        RadrootsTagValueType::Nip01Coordinate => RadrootsNip01Coordinate::parse(value).is_ok(),
        RadrootsTagValueType::PublicKey => parse_public_key(value).is_ok(),
        RadrootsTagValueType::RelayUrl => relay_url_is_valid(value),
        RadrootsTagValueType::Text => visible_text_is_valid(value),
        RadrootsTagValueType::UnixTimestamp => value.parse::<u64>().is_ok(),
        RadrootsTagValueType::Uri => RadrootsCalendarUri::parse(value).is_ok(),
        RadrootsTagValueType::Url => url_is_valid(value),
        RadrootsTagValueType::UtcDayIndex => canonical_u64(value).is_some(),
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
        && parse_public_key(author).is_ok()
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
    value
        .strip_prefix("https://")
        .or_else(|| value.strip_prefix("http://"))
        .is_some_and(|remainder| !remainder.is_empty())
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
        RadrootsTagValueType::CalendarDate => "calendar_date_yyyy_mm_dd",
        RadrootsTagValueType::CalendarEventCoordinate => "canonical_kind_31922_or_31923_coordinate",
        RadrootsTagValueType::CalendarFreeBusy => "calendar_free_or_busy",
        RadrootsTagValueType::CalendarRsvpStatus => "calendar_rsvp_status",
        RadrootsTagValueType::CalendarUid => "canonical_128_bit_base64url_calendar_uid",
        RadrootsTagValueType::ContractId => "contract_id",
        RadrootsTagValueType::DTag => "d_tag",
        RadrootsTagValueType::EventId => "event_id",
        RadrootsTagValueType::EventPointer => "event_pointer",
        RadrootsTagValueType::Geohash => "geohash",
        RadrootsTagValueType::IanaTimeZoneId => "canonical_iana_time_zone_id",
        RadrootsTagValueType::Kind => "kind",
        RadrootsTagValueType::Nip01Coordinate => "nip01_coordinate",
        RadrootsTagValueType::PublicKey => "public_key",
        RadrootsTagValueType::RelayUrl => "relay_url",
        RadrootsTagValueType::Sha256 => "sha256",
        RadrootsTagValueType::Text => "text",
        RadrootsTagValueType::UnixTimestamp => "unix_timestamp",
        RadrootsTagValueType::Uri => "absolute_uri",
        RadrootsTagValueType::Url => "url",
        RadrootsTagValueType::UtcDayIndex => "canonical_decimal_utc_day_index",
        RadrootsTagValueType::Uuid => "uuid",
    }
}

fn canonical_u64(value: &str) -> Option<u64> {
    if value.is_empty()
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    value.parse().ok()
}

fn validate_custom_calendar_contract_parts(
    tags: &[Vec<String>],
    contract: &RadrootsEventContract,
) -> Result<(), RadrootsContractValidationError> {
    match contract.id {
        "radroots.calendar.date_event.v1" => validate_calendar_date_contract(tags, contract),
        "radroots.calendar.time_event.v1" => validate_calendar_time_contract(tags, contract),
        "radroots.calendar.collection.v1" => validate_calendar_collection_contract(tags, contract),
        "radroots.calendar.rsvp.v1" => validate_calendar_rsvp_contract(tags, contract),
        _ => Ok(()),
    }
}

fn validate_calendar_collection_contract(
    tags: &[Vec<String>],
    contract: &RadrootsEventContract,
) -> Result<(), RadrootsContractValidationError> {
    validate_exact_calendar_tags(tags, contract, &["d", "title", "description", "image"])?;
    validate_canonical_calendar_text_tags(tags, contract, &["title", "description"])?;
    validate_calendar_event_reference_tags(tags, contract)?;
    validate_calendar_blossom_image(tags, contract)?;

    let event_references = tags
        .iter()
        .filter(|tag| tag.first().map(String::as_str) == Some("a"))
        .collect::<Vec<_>>();
    for (index, reference) in event_references.iter().enumerate() {
        if event_references
            .iter()
            .skip(index + 1)
            .any(|candidate| candidate.get(1) == reference.get(1))
        {
            return Err(calendar_tag_mismatch(
                contract,
                "a",
                "duplicate_free_calendar_event_coordinates",
                reference.get(1).cloned(),
            ));
        }
    }
    Ok(())
}

fn validate_calendar_rsvp_contract(
    tags: &[Vec<String>],
    contract: &RadrootsEventContract,
) -> Result<(), RadrootsContractValidationError> {
    validate_exact_calendar_tags(tags, contract, &["d", "status", "fb"])?;
    validate_calendar_event_reference_tags(tags, contract)?;
    validate_calendar_rsvp_pointer_tag(tags, contract, "e", true)?;
    validate_calendar_rsvp_pointer_tag(tags, contract, "p", false)?;

    let event_author = tags
        .iter()
        .find(|tag| tag.first().map(String::as_str) == Some("a"))
        .and_then(|tag| tag.get(1))
        .and_then(|coordinate| {
            crate::ids::RadrootsAddressableCoordinateParts::parse(coordinate).ok()
        })
        .map(|parts| parts.pubkey);
    if let Some(author_hint) = tag_value(tags, "p") {
        let hint = parse_public_key(author_hint).ok();
        if hint.as_ref() != event_author.as_ref()
            || hint.as_ref().is_none_or(|key| key.to_hex() != author_hint)
        {
            return Err(calendar_tag_mismatch(
                contract,
                "p",
                "canonical_calendar_event_author_matching_a_coordinate",
                Some(author_hint.to_owned()),
            ));
        }
    }
    Ok(())
}

fn validate_calendar_event_reference_tags(
    tags: &[Vec<String>],
    contract: &RadrootsEventContract,
) -> Result<(), RadrootsContractValidationError> {
    for tag in tags
        .iter()
        .filter(|tag| tag.first().map(String::as_str) == Some("a"))
    {
        let coordinate_is_valid = tag
            .get(1)
            .is_some_and(|value| canonical_calendar_event_coordinate_is_valid(value));
        let relay_is_valid = tag
            .get(2)
            .is_none_or(|relay| !relay.is_empty() && relay_url_is_valid(relay));
        if !(2..=3).contains(&tag.len()) || !coordinate_is_valid || !relay_is_valid {
            return Err(calendar_tag_mismatch(
                contract,
                "a",
                "canonical_kind_31922_or_31923_coordinate_with_optional_relay",
                tag.get(1).cloned(),
            ));
        }
    }
    Ok(())
}

fn validate_calendar_rsvp_pointer_tag(
    tags: &[Vec<String>],
    contract: &RadrootsEventContract,
    name: &'static str,
    event_id: bool,
) -> Result<(), RadrootsContractValidationError> {
    for tag in tags
        .iter()
        .filter(|tag| tag.first().map(String::as_str) == Some(name))
    {
        let value_is_canonical = tag.get(1).is_some_and(|value| {
            if event_id {
                RadrootsEventId::parse(value).is_ok_and(|parsed| parsed.as_str() == value)
            } else {
                parse_public_key(value).is_ok_and(|parsed| parsed.to_hex() == *value)
            }
        });
        let relay_is_valid = tag
            .get(2)
            .is_none_or(|relay| !relay.is_empty() && relay_url_is_valid(relay));
        if !(2..=3).contains(&tag.len()) || !value_is_canonical || !relay_is_valid {
            return Err(calendar_tag_mismatch(
                contract,
                name,
                if event_id {
                    "canonical_event_id_with_optional_relay"
                } else {
                    "canonical_public_key_with_optional_relay"
                },
                tag.get(1).cloned(),
            ));
        }
    }
    Ok(())
}

fn validate_canonical_calendar_text_tags(
    tags: &[Vec<String>],
    contract: &RadrootsEventContract,
    names: &[&'static str],
) -> Result<(), RadrootsContractValidationError> {
    for name in names {
        for tag in tags
            .iter()
            .filter(|tag| tag.first().map(String::as_str) == Some(*name))
        {
            if !tag
                .get(1)
                .is_some_and(|value| canonical_calendar_tag_text_is_valid(value))
            {
                return Err(calendar_tag_mismatch(
                    contract,
                    name,
                    "canonical_visible_calendar_text",
                    tag.get(1).cloned(),
                ));
            }
        }
    }
    Ok(())
}

fn validate_calendar_blossom_image(
    tags: &[Vec<String>],
    contract: &RadrootsEventContract,
) -> Result<(), RadrootsContractValidationError> {
    if let Some(image) = tag_value(tags, "image")
        && BlobUrl::parse(image).is_err()
    {
        return Err(calendar_tag_mismatch(
            contract,
            "image",
            "structural_blossom_hash_path_url",
            Some(image.to_owned()),
        ));
    }
    Ok(())
}

fn validate_calendar_date_contract(
    tags: &[Vec<String>],
    contract: &RadrootsEventContract,
) -> Result<(), RadrootsContractValidationError> {
    validate_exact_calendar_tags(
        tags,
        contract,
        &[
            "d", "title", "start", "end", "location", "g", "summary", "image", "t", "r", "name",
        ],
    )?;
    validate_calendar_participant_tags(tags, contract)?;
    validate_calendar_inclusion_request_tags(tags, contract)?;
    validate_canonical_calendar_common_tags(tags, contract)?;

    if let Some(tag) = tags
        .iter()
        .find(|tag| tag.first().map(String::as_str) == Some("D"))
    {
        return Err(calendar_tag_mismatch(
            contract,
            "D",
            "forbidden_on_calendar_date_event",
            tag.get(1).cloned(),
        ));
    }

    let start = calendar_date_tag(tags, contract, "start")?;
    if let Some(end) = optional_calendar_date_tag(tags, contract, "end")?
        && end <= start
    {
        return Err(calendar_tag_mismatch(
            contract,
            "end",
            "gregorian_date_later_than_start",
            Some(end.as_str().to_owned()),
        ));
    }
    Ok(())
}

fn validate_calendar_time_contract(
    tags: &[Vec<String>],
    contract: &RadrootsEventContract,
) -> Result<(), RadrootsContractValidationError> {
    validate_exact_calendar_tags(
        tags,
        contract,
        &[
            "d",
            "title",
            "start",
            "end",
            "start_tzid",
            "end_tzid",
            "location",
            "g",
            "summary",
            "image",
            "D",
            "t",
            "r",
            "name",
        ],
    )?;
    validate_calendar_participant_tags(tags, contract)?;
    validate_calendar_inclusion_request_tags(tags, contract)?;
    validate_canonical_calendar_common_tags(tags, contract)?;

    let start = canonical_calendar_u64_tag(tags, contract, "start")?;
    let end = optional_canonical_calendar_u64_tag(tags, contract, "end")?;
    if end.is_some_and(|end| end <= start) {
        return Err(calendar_tag_mismatch(
            contract,
            "end",
            "canonical_unix_seconds_later_than_start",
            tag_value(tags, "end").map(ToOwned::to_owned),
        ));
    }

    let expected_days = covered_utc_days(start, end).map_err(|_| {
        calendar_tag_mismatch(
            contract,
            "D",
            "complete_ascending_utc_day_coverage_with_maximum_366_days",
            None,
        )
    })?;
    let mut expected_days = expected_days.into_iter();
    let mut actual_days = tags
        .iter()
        .filter(|tag| tag.first().map(String::as_str) == Some("D"));
    loop {
        match (expected_days.next(), actual_days.next()) {
            (Some(expected), Some(actual))
                if actual.get(1).and_then(|value| canonical_u64(value)) == Some(expected) => {}
            (None, None) => break,
            (_, actual) => {
                return Err(calendar_tag_mismatch(
                    contract,
                    "D",
                    "complete_ascending_utc_day_coverage",
                    actual.and_then(|tag| tag.get(1)).cloned(),
                ));
            }
        }
    }
    Ok(())
}

fn validate_exact_calendar_tags(
    tags: &[Vec<String>],
    contract: &RadrootsEventContract,
    names: &[&'static str],
) -> Result<(), RadrootsContractValidationError> {
    for name in names {
        for tag in tags
            .iter()
            .filter(|tag| tag.first().map(String::as_str) == Some(*name))
        {
            if tag.len() != 2 {
                return Err(calendar_tag_mismatch(
                    contract,
                    name,
                    "exact_two_element_tag",
                    tag.get(1).cloned(),
                ));
            }
        }
    }
    Ok(())
}

fn validate_calendar_participant_tags(
    tags: &[Vec<String>],
    contract: &RadrootsEventContract,
) -> Result<(), RadrootsContractValidationError> {
    if tag_count(tags, "p") > RADROOTS_CALENDAR_MAX_PARTICIPANTS {
        return Err(calendar_tag_mismatch(
            contract,
            "p",
            "bounded_participant_count",
            None,
        ));
    }
    for tag in tags
        .iter()
        .filter(|tag| tag.first().map(String::as_str) == Some("p"))
    {
        let pubkey_is_canonical = tag.get(1).is_some_and(|value| {
            parse_public_key(value).is_ok_and(|pubkey| pubkey.to_hex() == *value)
        });
        let relay_is_valid = tag
            .get(2)
            .map(|relay| relay.is_empty() || relay_url_is_valid(relay))
            .unwrap_or(true);
        let role_is_valid = tag
            .get(3)
            .map(|role| canonical_calendar_tag_text_is_valid(role))
            .unwrap_or(true);
        let placeholder_is_canonical = !(tag.len() == 3 && tag[2].is_empty());
        if !(2..=4).contains(&tag.len())
            || !pubkey_is_canonical
            || !relay_is_valid
            || !role_is_valid
            || !placeholder_is_canonical
        {
            return Err(calendar_tag_mismatch(
                contract,
                "p",
                "participant_pubkey_with_optional_relay_and_role",
                tag.get(1).cloned(),
            ));
        }
    }
    Ok(())
}

fn validate_calendar_inclusion_request_tags(
    tags: &[Vec<String>],
    contract: &RadrootsEventContract,
) -> Result<(), RadrootsContractValidationError> {
    for tag in tags
        .iter()
        .filter(|tag| tag.first().map(String::as_str) == Some("a"))
    {
        let coordinate_is_calendar = tag
            .get(1)
            .is_some_and(|value| canonical_calendar_coordinate_is_valid(value));
        let relay_is_valid = tag
            .get(2)
            .is_none_or(|relay| !relay.is_empty() && relay_url_is_valid(relay));
        if !(2..=3).contains(&tag.len()) || !coordinate_is_calendar || !relay_is_valid {
            return Err(calendar_tag_mismatch(
                contract,
                "a",
                "kind_31924_coordinate_with_optional_relay",
                tag.get(1).cloned(),
            ));
        }
    }
    Ok(())
}

fn canonical_calendar_coordinate_is_valid(value: &str) -> bool {
    let Some((kind, remainder)) = value.split_once(':') else {
        return false;
    };
    let Some((pubkey, d_tag)) = remainder.split_once(':') else {
        return false;
    };
    let Ok(parts) = crate::ids::RadrootsAddressableCoordinateParts::parse(value) else {
        return false;
    };
    kind == "31924" && pubkey == parts.pubkey.to_hex() && d_tag == parts.d_tag.as_str()
}

fn canonical_calendar_event_coordinate_is_valid(value: &str) -> bool {
    let Some((kind, remainder)) = value.split_once(':') else {
        return false;
    };
    let Some((pubkey, d_tag)) = remainder.split_once(':') else {
        return false;
    };
    let Ok(parts) = crate::ids::RadrootsAddressableCoordinateParts::parse(value) else {
        return false;
    };
    matches!(
        parts.kind,
        KIND_CALENDAR_DATE_EVENT | KIND_CALENDAR_TIME_EVENT
    ) && matches!(kind, "31922" | "31923")
        && pubkey == parts.pubkey.to_hex()
        && d_tag == parts.d_tag.as_str()
}

fn validate_canonical_calendar_common_tags(
    tags: &[Vec<String>],
    contract: &RadrootsEventContract,
) -> Result<(), RadrootsContractValidationError> {
    for name in ["title", "location", "summary", "t", "name"] {
        for tag in tags
            .iter()
            .filter(|tag| tag.first().map(String::as_str) == Some(name))
        {
            if !tag
                .get(1)
                .is_some_and(|value| canonical_calendar_tag_text_is_valid(value))
            {
                return Err(calendar_tag_mismatch(
                    contract,
                    name,
                    "canonical_visible_calendar_text",
                    tag.get(1).cloned(),
                ));
            }
        }
    }
    if let Some(geohash) = tag_value(tags, "g")
        && !canonical_calendar_geohash_is_valid(geohash)
    {
        return Err(calendar_tag_mismatch(
            contract,
            "g",
            "canonical_lowercase_geohash",
            Some(geohash.to_owned()),
        ));
    }
    if let Some(image) = tag_value(tags, "image")
        && BlobUrl::parse(image).is_err()
    {
        return Err(calendar_tag_mismatch(
            contract,
            "image",
            "structural_blossom_hash_path_url",
            Some(image.to_owned()),
        ));
    }
    Ok(())
}

fn calendar_date_tag(
    tags: &[Vec<String>],
    contract: &RadrootsEventContract,
    name: &'static str,
) -> Result<RadrootsCalendarDate, RadrootsContractValidationError> {
    let value = tag_value(tags, name).ok_or(RadrootsContractValidationError::MissingTag {
        contract_id: contract.id,
        name,
    })?;
    RadrootsCalendarDate::parse(value).map_err(|_| {
        calendar_tag_mismatch(
            contract,
            name,
            "calendar_date_yyyy_mm_dd",
            Some(value.to_owned()),
        )
    })
}

fn optional_calendar_date_tag(
    tags: &[Vec<String>],
    contract: &RadrootsEventContract,
    name: &'static str,
) -> Result<Option<RadrootsCalendarDate>, RadrootsContractValidationError> {
    tag_value(tags, name)
        .map(|_| calendar_date_tag(tags, contract, name))
        .transpose()
}

fn canonical_calendar_u64_tag(
    tags: &[Vec<String>],
    contract: &RadrootsEventContract,
    name: &'static str,
) -> Result<u64, RadrootsContractValidationError> {
    let value = tag_value(tags, name).ok_or(RadrootsContractValidationError::MissingTag {
        contract_id: contract.id,
        name,
    })?;
    canonical_u64(value).ok_or_else(|| {
        calendar_tag_mismatch(
            contract,
            name,
            "canonical_decimal_u64",
            Some(value.to_owned()),
        )
    })
}

fn optional_canonical_calendar_u64_tag(
    tags: &[Vec<String>],
    contract: &RadrootsEventContract,
    name: &'static str,
) -> Result<Option<u64>, RadrootsContractValidationError> {
    tag_value(tags, name)
        .map(|_| canonical_calendar_u64_tag(tags, contract, name))
        .transpose()
}

fn calendar_tag_mismatch(
    contract: &RadrootsEventContract,
    name: &'static str,
    expected: &'static str,
    actual: Option<String>,
) -> RadrootsContractValidationError {
    RadrootsContractValidationError::TagValueMismatch {
        contract_id: contract.id,
        name,
        expected: expected.to_owned(),
        actual,
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

fn validate_discriminator_parts(
    content: &str,
    contract: &RadrootsEventContract,
) -> Result<(), RadrootsContractValidationError> {
    if matches!(
        contract.discriminator,
        RadrootsEventDiscriminator::AdmissionOnly
    ) {
        return Err(RadrootsContractValidationError::AdmissionRequired {
            contract_id: contract.id,
        });
    }
    let (field, value) = match &contract.discriminator {
        RadrootsEventDiscriminator::ContentJsonFieldEquals { field, value } => (*field, *value),
        RadrootsEventDiscriminator::EnvelopeType(value) => ("type", *value),
        _ => return Ok(()),
    };
    let object = parse_content_object(content, contract.id)?;
    match object.get(field).and_then(|actual| actual.as_str()) {
        Some(actual) if actual == value => Ok(()),
        Some(_) => Err(RadrootsContractValidationError::ContentFieldMismatch {
            contract_id: contract.id,
            field,
            expected: value.to_owned(),
        }),
        None => Err(RadrootsContractValidationError::MissingContentField {
            contract_id: contract.id,
            field,
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
        RadrootsEventDiscriminator::AdmissionOnly => false,
        RadrootsEventDiscriminator::ClassifiedListingPartition(expected) => {
            classify_classified_listing_raw_tags_registry_v7(tags) == *expected
        }
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
mod tests;
