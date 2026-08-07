#![forbid(unsafe_code)]

//! Frozen event-contract registry-v7 inventory and validation semantics.

#[cfg(not(feature = "std"))]
use alloc::{borrow::ToOwned, string::String, vec::Vec};

use crate::{
    calendar::{
        CalendarDate, CalendarUid, CalendarUri, IanaTimeZoneId, RADROOTS_CALENDAR_MAX_PARTICIPANTS,
        canonical_calendar_geohash_is_valid, canonical_calendar_tag_text_is_valid,
        covered_utc_days,
    },
    envelope::EventEnvelope,
    envelope::kind::*,
    id::{
        AddressableCoordinate, DTag, EventId, Nip01Coordinate, parse_public_key, relay_url_is_valid,
    },
    listing::classified::{
        ClassifiedListingPartition, TAG_RADROOTS_PRICE_UNIT, TAG_RADROOTS_QUANTITY,
    },
};
use radroots_blossom::BlobUrl;

pub const RADROOTS_EVENT_CONTRACT_REGISTRY_VERSION: u32 = 7;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventClass {
    Regular,
    Replaceable,
    Addressable,
    Ephemeral,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NostrStandard {
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
pub enum EventPrivacy {
    Public,
    Encrypted,
    LocalOnly,
    Secret,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventStability {
    Stable,
    Experimental,
}

/// Event-contract role required of an author.
///
/// This is event authoring policy, not signer provenance, authentication
/// state, account persistence, or proof that a host granted the role.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AuthorRole {
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

impl AuthorRole {
    pub const ALL: [Self; 9] = [
        Self::Any,
        Self::Application,
        Self::Buyer,
        Self::Farmer,
        Self::Member,
        Self::Moderator,
        Self::Relay,
        Self::Seller,
        Self::Service,
    ];

    /// Stable registry label used by versioned manifest encoders.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Any => "any",
            Self::Application => "application",
            Self::Buyer => "buyer",
            Self::Farmer => "farmer",
            Self::Member => "member",
            Self::Moderator => "moderator",
            Self::Relay => "relay",
            Self::Seller => "seller",
            Self::Service => "service",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reducer {
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
pub enum ContentSchema {
    Empty,
    JsonObject,
    PlainText,
    Markdown,
    Djot,
    Encrypted,
    BinaryReference,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TagCardinality {
    RequiredOne,
    OptionalOne,
    OptionalMany,
    RequiredMany,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TagSemantic {
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
pub enum TagValueType {
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
pub struct TagContract {
    pub name: &'static str,
    pub cardinality: TagCardinality,
    pub semantic: TagSemantic,
    pub value_type: TagValueType,
    pub relay_indexed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventDiscriminator {
    KindOnly,
    /// Exact profile selection is owned by a verified admission algorithm.
    AdmissionOnly,
    /// Exact NIP-99 profile selection uses the central raw marker partition.
    ClassifiedListingPartition(ClassifiedListingPartition),
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
    Composite(&'static [EventDiscriminator]),
}

/// Governs whether a contract may enter the generic frozen-draft boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventAuthoringPolicy {
    /// Generic contract validation is sufficient to construct a frozen draft.
    GenericDraft,
    /// Authoring requires a sealed typed API instead of generic draft parts.
    TypedOnly,
    /// The contract is an inbound/read boundary and cannot be authored.
    ReadOnly,
}

impl EventAuthoringPolicy {
    /// Returns whether untyped contract parts may construct a frozen draft.
    #[must_use]
    pub const fn permits_generic_draft(self) -> bool {
        matches!(self, Self::GenericDraft)
    }

    /// Returns whether a sealed typed authoring API may construct an event.
    #[must_use]
    pub const fn permits_typed_authoring(self) -> bool {
        !matches!(self, Self::ReadOnly)
    }

    /// Returns whether the contract is exclusively an inbound/read boundary.
    #[must_use]
    pub const fn is_read_only(self) -> bool {
        matches!(self, Self::ReadOnly)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContractMatchError {
    UnsupportedKind(u32),
    UnsupportedShape(u32),
    AmbiguousShape(u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContractFamily {
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
pub struct ContractFamilyMetadata {
    pub family: ContractFamily,
    pub id: &'static str,
    pub name: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContractValidationError {
    UnknownContract {
        contract_id: String,
    },
    AdmissionRequired {
        contract_id: &'static str,
    },
    ContractMatch {
        error: ContractMatchError,
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

impl ContractValidationError {
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
pub struct KindContract {
    pub kind: u32,
    pub canonical_constant: &'static str,
    pub name: &'static str,
    pub class: EventClass,
    pub standard: NostrStandard,
    pub accepted_event_contracts: &'static [&'static str],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EventContract {
    pub id: &'static str,
    pub kind: u32,
    pub name: &'static str,
    pub payload_type: &'static str,
    pub class: EventClass,
    pub stability: EventStability,
    pub privacy: EventPrivacy,
    required_author_role: AuthorRole,
    pub content_schema: ContentSchema,
    authoring_policy: EventAuthoringPolicy,
    pub discriminator: EventDiscriminator,
    pub tags: &'static [TagContract],
    pub reducers: &'static [Reducer],
}

impl EventContract {
    /// Returns the event-authoring role required by this contract.
    ///
    /// Hosts decide whether a concrete signer or actor has this role; the
    /// event package only owns the contract requirement.
    #[must_use]
    pub const fn required_author_role(&self) -> AuthorRole {
        self.required_author_role
    }

    /// Returns the single registry-owned policy governing authoring routes.
    #[must_use]
    pub const fn authoring_policy(&self) -> EventAuthoringPolicy {
        self.authoring_policy
    }
}

static CONTRACT_FAMILIES: &[ContractFamilyMetadata] = &[
    ContractFamilyMetadata {
        family: ContractFamily::Account,
        id: "account",
        name: "Account",
    },
    ContractFamilyMetadata {
        family: ContractFamily::Application,
        id: "application",
        name: "Application",
    },
    ContractFamilyMetadata {
        family: ContractFamily::Calendar,
        id: "calendar",
        name: "Calendar",
    },
    ContractFamilyMetadata {
        family: ContractFamily::Farm,
        id: "farm",
        name: "Farm",
    },
    ContractFamilyMetadata {
        family: ContractFamily::Group,
        id: "group",
        name: "Group",
    },
    ContractFamilyMetadata {
        family: ContractFamily::Http,
        id: "http",
        name: "HTTP",
    },
    ContractFamilyMetadata {
        family: ContractFamily::Job,
        id: "job",
        name: "Job",
    },
    ContractFamilyMetadata {
        family: ContractFamily::Knowledge,
        id: "knowledge",
        name: "Knowledge",
    },
    ContractFamilyMetadata {
        family: ContractFamily::List,
        id: "list",
        name: "List",
    },
    ContractFamilyMetadata {
        family: ContractFamily::Market,
        id: "market",
        name: "Market",
    },
    ContractFamilyMetadata {
        family: ContractFamily::Message,
        id: "message",
        name: "Message",
    },
    ContractFamilyMetadata {
        family: ContractFamily::Profile,
        id: "profile",
        name: "Profile",
    },
    ContractFamilyMetadata {
        family: ContractFamily::Relay,
        id: "relay",
        name: "Relay",
    },
    ContractFamilyMetadata {
        family: ContractFamily::Social,
        id: "social",
        name: "Social",
    },
    ContractFamilyMetadata {
        family: ContractFamily::Trade,
        id: "trade",
        name: "Trade",
    },
];

const fn tag(
    name: &'static str,
    cardinality: TagCardinality,
    semantic: TagSemantic,
    value_type: TagValueType,
    relay_indexed: bool,
) -> TagContract {
    TagContract {
        name,
        cardinality,
        semantic,
        value_type,
        relay_indexed,
    }
}

const TAG_D: TagContract = tag(
    "d",
    TagCardinality::RequiredOne,
    TagSemantic::Identifier,
    TagValueType::DTag,
    true,
);
const TAG_P_REQUIRED: TagContract = tag(
    "p",
    TagCardinality::RequiredOne,
    TagSemantic::Counterparty,
    TagValueType::PublicKey,
    true,
);
const TAG_P_MANY: TagContract = tag(
    "p",
    TagCardinality::OptionalMany,
    TagSemantic::Counterparty,
    TagValueType::PublicKey,
    true,
);
const TAG_CALENDAR_PARTICIPANT: TagContract = tag(
    "p",
    TagCardinality::OptionalMany,
    TagSemantic::Participant,
    TagValueType::PublicKey,
    true,
);
const TAG_A_ADDRESS_REQUIRED: TagContract = tag(
    "a",
    TagCardinality::RequiredOne,
    TagSemantic::AddressableCoordinate,
    TagValueType::AddressableCoordinate,
    true,
);
const TAG_A_OPTIONAL: TagContract = tag(
    "a",
    TagCardinality::OptionalOne,
    TagSemantic::AddressableCoordinate,
    TagValueType::AddressableCoordinate,
    true,
);
const TAG_A_MANY: TagContract = tag(
    "a",
    TagCardinality::OptionalMany,
    TagSemantic::AddressableCoordinate,
    TagValueType::AddressableCoordinate,
    true,
);
const TAG_CALENDAR_INCLUSION_REQUEST: TagContract = tag(
    "a",
    TagCardinality::OptionalMany,
    TagSemantic::CalendarInclusionRequest,
    TagValueType::AddressableCoordinate,
    true,
);
const TAG_E_ROOT: TagContract = tag(
    "e",
    TagCardinality::RequiredOne,
    TagSemantic::RootEvent,
    TagValueType::EventId,
    true,
);
const TAG_E_SOURCE_VERSION: TagContract = tag(
    "e",
    TagCardinality::RequiredOne,
    TagSemantic::Source,
    TagValueType::EventId,
    true,
);
const TAG_E_BASE_VERSION: TagContract = tag(
    "e",
    TagCardinality::OptionalOne,
    TagSemantic::PreviousEvent,
    TagValueType::EventId,
    true,
);
const TAG_E_MANY: TagContract = tag(
    "e",
    TagCardinality::OptionalMany,
    TagSemantic::EventPointer,
    TagValueType::EventId,
    true,
);
const TAG_NIP09_E_TARGET: TagContract = tag(
    "e",
    TagCardinality::OptionalMany,
    TagSemantic::EventPointer,
    TagValueType::EventId,
    true,
);
const TAG_NIP09_A_TARGET: TagContract = tag(
    "a",
    TagCardinality::OptionalMany,
    TagSemantic::Nip01Coordinate,
    TagValueType::Nip01Coordinate,
    true,
);
const TAG_NIP09_K_ADVISORY: TagContract = tag(
    "k",
    TagCardinality::OptionalMany,
    TagSemantic::Kind,
    TagValueType::Kind,
    true,
);
const TAG_NIP10_E_REQUIRED: TagContract = tag(
    "e",
    TagCardinality::RequiredMany,
    TagSemantic::EventPointer,
    TagValueType::EventId,
    true,
);
const TAG_NIP10_P_OPTIONAL: TagContract = tag(
    "p",
    TagCardinality::OptionalMany,
    TagSemantic::Participant,
    TagValueType::PublicKey,
    true,
);
const TAG_NIP22_E_ROOT: TagContract = tag(
    "E",
    TagCardinality::OptionalOne,
    TagSemantic::RootEvent,
    TagValueType::EventId,
    true,
);
const TAG_NIP22_A_ROOT: TagContract = tag(
    "A",
    TagCardinality::OptionalOne,
    TagSemantic::AddressableCoordinate,
    TagValueType::AddressableCoordinate,
    true,
);
const TAG_NIP22_K_ROOT: TagContract = tag(
    "K",
    TagCardinality::RequiredOne,
    TagSemantic::Kind,
    TagValueType::Kind,
    true,
);
const TAG_NIP22_P_ROOT: TagContract = tag(
    "P",
    TagCardinality::RequiredOne,
    TagSemantic::Participant,
    TagValueType::PublicKey,
    true,
);
const TAG_NIP22_A_PARENT: TagContract = tag(
    "a",
    TagCardinality::OptionalOne,
    TagSemantic::AddressableCoordinate,
    TagValueType::AddressableCoordinate,
    true,
);
const TAG_NIP22_E_PARENT: TagContract = tag(
    "e",
    TagCardinality::RequiredOne,
    TagSemantic::EventPointer,
    TagValueType::EventId,
    true,
);
const TAG_NIP22_K_PARENT: TagContract = tag(
    "k",
    TagCardinality::RequiredOne,
    TagSemantic::Kind,
    TagValueType::Kind,
    true,
);
const TAG_NIP22_P_PARENT: TagContract = tag(
    "p",
    TagCardinality::RequiredMany,
    TagSemantic::Participant,
    TagValueType::PublicKey,
    true,
);
const TAG_KIND: TagContract = tag(
    "k",
    TagCardinality::OptionalOne,
    TagSemantic::Kind,
    TagValueType::Kind,
    true,
);
const TAG_RELAY: TagContract = tag(
    "relay",
    TagCardinality::OptionalMany,
    TagSemantic::Relay,
    TagValueType::RelayUrl,
    false,
);
const TAG_GROUP: TagContract = tag(
    "h",
    TagCardinality::RequiredOne,
    TagSemantic::GroupId,
    TagValueType::DTag,
    true,
);
const TAG_TITLE: TagContract = tag(
    "title",
    TagCardinality::OptionalOne,
    TagSemantic::Title,
    TagValueType::Text,
    false,
);
const TAG_CALENDAR_TITLE: TagContract = tag(
    "title",
    TagCardinality::RequiredOne,
    TagSemantic::Title,
    TagValueType::Text,
    false,
);
const TAG_CALENDAR_LEGACY_NAME: TagContract = tag(
    "name",
    TagCardinality::OptionalOne,
    TagSemantic::Title,
    TagValueType::Text,
    false,
);
const TAG_CALENDAR_UID: TagContract = tag(
    "d",
    TagCardinality::RequiredOne,
    TagSemantic::Identifier,
    TagValueType::CalendarUid,
    true,
);
const TAG_CALENDAR_COLLECTION_DESCRIPTION: TagContract = tag(
    "description",
    TagCardinality::OptionalOne,
    TagSemantic::ListDescription,
    TagValueType::Text,
    false,
);
const TAG_CALENDAR_COLLECTION_EVENT: TagContract = tag(
    "a",
    TagCardinality::OptionalMany,
    TagSemantic::CalendarEventReference,
    TagValueType::CalendarEventCoordinate,
    true,
);
const TAG_CALENDAR_RSVP_EVENT: TagContract = tag(
    "a",
    TagCardinality::RequiredOne,
    TagSemantic::CalendarEventReference,
    TagValueType::CalendarEventCoordinate,
    true,
);
const TAG_CALENDAR_RSVP_REVISION: TagContract = tag(
    "e",
    TagCardinality::OptionalOne,
    TagSemantic::CalendarEventRevision,
    TagValueType::EventId,
    true,
);
const TAG_CALENDAR_RSVP_STATUS: TagContract = tag(
    "status",
    TagCardinality::RequiredOne,
    TagSemantic::Status,
    TagValueType::CalendarRsvpStatus,
    false,
);
const TAG_CALENDAR_RSVP_FREE_BUSY: TagContract = tag(
    "fb",
    TagCardinality::OptionalOne,
    TagSemantic::FreeBusy,
    TagValueType::CalendarFreeBusy,
    false,
);
const TAG_CALENDAR_RSVP_AUTHOR: TagContract = tag(
    "p",
    TagCardinality::OptionalOne,
    TagSemantic::CalendarEventAuthor,
    TagValueType::PublicKey,
    true,
);
const TAG_CALENDAR_DATE_START: TagContract = tag(
    "start",
    TagCardinality::RequiredOne,
    TagSemantic::CalendarStart,
    TagValueType::CalendarDate,
    false,
);
const TAG_CALENDAR_DATE_END: TagContract = tag(
    "end",
    TagCardinality::OptionalOne,
    TagSemantic::CalendarEnd,
    TagValueType::CalendarDate,
    false,
);
const TAG_CALENDAR_TIME_START: TagContract = tag(
    "start",
    TagCardinality::RequiredOne,
    TagSemantic::CalendarStart,
    TagValueType::UnixTimestamp,
    false,
);
const TAG_CALENDAR_TIME_END: TagContract = tag(
    "end",
    TagCardinality::OptionalOne,
    TagSemantic::CalendarEnd,
    TagValueType::UnixTimestamp,
    false,
);
const TAG_CALENDAR_COVERED_UTC_DAY: TagContract = tag(
    "D",
    TagCardinality::RequiredMany,
    TagSemantic::UtcDayCoverage,
    TagValueType::UtcDayIndex,
    true,
);
const TAG_CALENDAR_START_TZID: TagContract = tag(
    "start_tzid",
    TagCardinality::OptionalOne,
    TagSemantic::TimeZone,
    TagValueType::IanaTimeZoneId,
    false,
);
const TAG_CALENDAR_END_TZID: TagContract = tag(
    "end_tzid",
    TagCardinality::OptionalOne,
    TagSemantic::TimeZone,
    TagValueType::IanaTimeZoneId,
    false,
);
const TAG_SUMMARY: TagContract = tag(
    "summary",
    TagCardinality::OptionalOne,
    TagSemantic::Summary,
    TagValueType::Text,
    false,
);
const TAG_PUBLISHED_AT: TagContract = tag(
    "published_at",
    TagCardinality::OptionalOne,
    TagSemantic::PublishedAt,
    TagValueType::UnixTimestamp,
    false,
);
const TAG_LOCATION: TagContract = tag(
    "location",
    TagCardinality::OptionalMany,
    TagSemantic::Location,
    TagValueType::Text,
    false,
);
const TAG_CALENDAR_LOCATION: TagContract = tag(
    "location",
    TagCardinality::OptionalMany,
    TagSemantic::Location,
    TagValueType::Text,
    false,
);
const TAG_PRICE: TagContract = tag(
    "price",
    TagCardinality::OptionalMany,
    TagSemantic::Price,
    TagValueType::Text,
    false,
);
const TAG_STATUS: TagContract = tag(
    "status",
    TagCardinality::OptionalOne,
    TagSemantic::Status,
    TagValueType::Text,
    false,
);
const TAG_IMAGE: TagContract = tag(
    "image",
    TagCardinality::OptionalMany,
    TagSemantic::Image,
    TagValueType::Url,
    false,
);
const TAG_FOOD_TITLE: TagContract = tag(
    "title",
    TagCardinality::RequiredOne,
    TagSemantic::Title,
    TagValueType::Text,
    false,
);
const TAG_FOOD_SUMMARY: TagContract = tag(
    "summary",
    TagCardinality::RequiredOne,
    TagSemantic::Summary,
    TagValueType::Text,
    false,
);
const TAG_FOOD_PUBLISHED_AT: TagContract = tag(
    "published_at",
    TagCardinality::RequiredOne,
    TagSemantic::PublishedAt,
    TagValueType::UnixTimestamp,
    false,
);
const TAG_FOOD_LOCATION: TagContract = tag(
    "location",
    TagCardinality::RequiredOne,
    TagSemantic::Location,
    TagValueType::Text,
    false,
);
const TAG_FOOD_PRICE: TagContract = tag(
    "price",
    TagCardinality::RequiredOne,
    TagSemantic::Price,
    TagValueType::Text,
    false,
);
const TAG_FOOD_PRICE_UNIT: TagContract = tag(
    TAG_RADROOTS_PRICE_UNIT,
    TagCardinality::RequiredOne,
    TagSemantic::Price,
    TagValueType::Text,
    false,
);
const TAG_FOOD_QUANTITY: TagContract = tag(
    TAG_RADROOTS_QUANTITY,
    TagCardinality::OptionalOne,
    TagSemantic::Price,
    TagValueType::Text,
    false,
);
const TAG_FOOD_STATUS: TagContract = tag(
    "status",
    TagCardinality::RequiredOne,
    TagSemantic::Status,
    TagValueType::Text,
    false,
);
const TAG_OPERATIONAL_LISTING_FARM: TagContract = tag(
    "a",
    TagCardinality::RequiredOne,
    TagSemantic::AddressableCoordinate,
    TagValueType::AddressableCoordinate,
    true,
);
const TAG_OPERATIONAL_LISTING_PRODUCT_KEY: TagContract = tag(
    "key",
    TagCardinality::RequiredOne,
    TagSemantic::Category,
    TagValueType::Text,
    false,
);
const TAG_OPERATIONAL_LISTING_TITLE: TagContract = tag(
    "title",
    TagCardinality::RequiredOne,
    TagSemantic::Title,
    TagValueType::Text,
    false,
);
const TAG_OPERATIONAL_LISTING_CATEGORY: TagContract = tag(
    "category",
    TagCardinality::RequiredOne,
    TagSemantic::Category,
    TagValueType::Text,
    false,
);
const TAG_OPERATIONAL_LISTING_PRIMARY_BIN: TagContract = tag(
    "radroots:primary_bin",
    TagCardinality::RequiredOne,
    TagSemantic::OperationalListingSnapshot,
    TagValueType::Text,
    false,
);
const TAG_OPERATIONAL_LISTING_BIN: TagContract = tag(
    "radroots:bin",
    TagCardinality::RequiredMany,
    TagSemantic::OperationalListingSnapshot,
    TagValueType::Text,
    false,
);
const TAG_OPERATIONAL_LISTING_PRICE: TagContract = tag(
    "radroots:price",
    TagCardinality::RequiredMany,
    TagSemantic::Price,
    TagValueType::Text,
    false,
);
const TAG_OPERATIONAL_LISTING_DISCOUNT: TagContract = tag(
    "radroots:discount",
    TagCardinality::OptionalMany,
    TagSemantic::Price,
    TagValueType::Text,
    false,
);
const TAG_OPERATIONAL_LISTING_RESOURCE_AREA: TagContract = tag(
    "radroots:resource_area",
    TagCardinality::OptionalOne,
    TagSemantic::AddressableCoordinate,
    TagValueType::AddressableCoordinate,
    false,
);
const TAG_OPERATIONAL_LISTING_PLOT: TagContract = tag(
    "radroots:plot",
    TagCardinality::OptionalOne,
    TagSemantic::AddressableCoordinate,
    TagValueType::AddressableCoordinate,
    false,
);
const TAG_OPERATIONAL_LISTING_INVENTORY: TagContract = tag(
    "inventory",
    TagCardinality::OptionalOne,
    TagSemantic::OperationalListingSnapshot,
    TagValueType::Text,
    false,
);
const TAG_OPERATIONAL_LISTING_AVAILABILITY_START: TagContract = tag(
    "radroots:availability_start",
    TagCardinality::OptionalOne,
    TagSemantic::Status,
    TagValueType::UnixTimestamp,
    false,
);
const TAG_OPERATIONAL_LISTING_EXPIRES_AT: TagContract = tag(
    "expires_at",
    TagCardinality::OptionalOne,
    TagSemantic::Status,
    TagValueType::UnixTimestamp,
    false,
);
const TAG_OPERATIONAL_LISTING_DELIVERY: TagContract = tag(
    "delivery",
    TagCardinality::OptionalOne,
    TagSemantic::Reference,
    TagValueType::Text,
    false,
);
const TAG_OPERATIONAL_LISTING_PROCESS: TagContract = tag(
    "process",
    TagCardinality::OptionalOne,
    TagSemantic::Category,
    TagValueType::Text,
    false,
);
const TAG_OPERATIONAL_LISTING_LOT: TagContract = tag(
    "lot",
    TagCardinality::OptionalOne,
    TagSemantic::Reference,
    TagValueType::Text,
    false,
);
const TAG_OPERATIONAL_LISTING_PROFILE: TagContract = tag(
    "profile",
    TagCardinality::OptionalOne,
    TagSemantic::Category,
    TagValueType::Text,
    false,
);
const TAG_OPERATIONAL_LISTING_YEAR: TagContract = tag(
    "year",
    TagCardinality::OptionalOne,
    TagSemantic::Category,
    TagValueType::Text,
    false,
);
const TAG_CALENDAR_IMAGE: TagContract = tag(
    "image",
    TagCardinality::OptionalOne,
    TagSemantic::Image,
    TagValueType::Url,
    false,
);
const TAG_SERVICE_OUTPUT: TagContract = tag(
    "output",
    TagCardinality::RequiredOne,
    TagSemantic::ServiceOutput,
    TagValueType::Text,
    false,
);
const TAG_URL: TagContract = tag(
    "url",
    TagCardinality::OptionalOne,
    TagSemantic::Url,
    TagValueType::Url,
    false,
);
const TAG_CONTRACT_REQUIRED: TagContract = tag(
    "contract",
    TagCardinality::RequiredOne,
    TagSemantic::Contract,
    TagValueType::ContractId,
    false,
);
const TAG_TOPIC_MANY: TagContract = tag(
    "t",
    TagCardinality::OptionalMany,
    TagSemantic::Topic,
    TagValueType::Text,
    true,
);
const TAG_ASK_MARKER: TagContract = tag(
    "t",
    TagCardinality::RequiredOne,
    TagSemantic::Topic,
    TagValueType::Text,
    true,
);
const TAG_IMETA_REQUIRED_MANY: TagContract = tag(
    "imeta",
    TagCardinality::RequiredMany,
    TagSemantic::Image,
    TagValueType::Text,
    false,
);
const TAG_IMETA_OPTIONAL_MANY: TagContract = tag(
    "imeta",
    TagCardinality::OptionalMany,
    TagSemantic::Image,
    TagValueType::Text,
    false,
);
const TAG_CALENDAR_REFERENCE: TagContract = tag(
    "r",
    TagCardinality::OptionalMany,
    TagSemantic::Reference,
    TagValueType::Uri,
    true,
);
const TAG_GEOHASH_OPTIONAL: TagContract = tag(
    "g",
    TagCardinality::OptionalOne,
    TagSemantic::Geohash,
    TagValueType::Geohash,
    true,
);
const TAG_SOURCE_MANY: TagContract = tag(
    "source",
    TagCardinality::OptionalMany,
    TagSemantic::Source,
    TagValueType::EventPointer,
    false,
);
const TAG_CITATION_MANY: TagContract = tag(
    "citation",
    TagCardinality::OptionalMany,
    TagSemantic::Citation,
    TagValueType::Sha256,
    false,
);
const TAG_REVIEW_TARGET_REQUIRED: TagContract = tag(
    "review_target",
    TagCardinality::RequiredOne,
    TagSemantic::ReviewTarget,
    TagValueType::EventPointer,
    false,
);
const TAG_EVIDENCE_MANY: TagContract = tag(
    "evidence",
    TagCardinality::OptionalMany,
    TagSemantic::Evidence,
    TagValueType::EventPointer,
    false,
);

const NO_TAGS: &[TagContract] = &[];
const D_TAGS: &[TagContract] = &[TAG_D];
const P_TAGS: &[TagContract] = &[TAG_P_MANY];
const EVENT_POINTER_TAGS: &[TagContract] = &[TAG_E_MANY, TAG_P_MANY, TAG_KIND];
const NIP09_DELETION_TAGS: &[TagContract] =
    &[TAG_NIP09_E_TARGET, TAG_NIP09_A_TARGET, TAG_NIP09_K_ADVISORY];
const NIP22_COMMENT_TAGS: &[TagContract] = &[
    TAG_NIP22_E_ROOT,
    TAG_NIP22_A_ROOT,
    TAG_NIP22_K_ROOT,
    TAG_NIP22_P_ROOT,
    TAG_NIP22_A_PARENT,
    TAG_NIP22_E_PARENT,
    TAG_NIP22_K_PARENT,
    TAG_NIP22_P_PARENT,
];
const LIST_TAGS: &[TagContract] = &[TAG_E_MANY, TAG_A_OPTIONAL, TAG_P_MANY, TAG_RELAY];
const LIST_SET_TAGS: &[TagContract] = &[TAG_D, TAG_E_MANY, TAG_A_OPTIONAL, TAG_P_MANY];
const PROFILE_TAGS: &[TagContract] = &[TAG_P_MANY];
const GROUP_ACTION_TAGS: &[TagContract] = &[TAG_GROUP, TAG_P_MANY, TAG_E_MANY];
const GROUP_STATE_TAGS: &[TagContract] = &[TAG_D, TAG_P_MANY, TAG_E_MANY];
const FILE_METADATA_TAGS: &[TagContract] = &[TAG_URL, TAG_IMAGE];
const ARTICLE_TAGS: &[TagContract] = &[TAG_D, TAG_TITLE, TAG_SUMMARY, TAG_PUBLISHED_AT];
const WIKI_ARTICLE_TAGS: &[TagContract] = &[
    TAG_D,
    TAG_TITLE,
    TAG_SUMMARY,
    TAG_PUBLISHED_AT,
    TAG_TOPIC_MANY,
    TAG_SOURCE_MANY,
    TAG_A_MANY,
    TAG_E_MANY,
];
const WIKI_REDIRECT_TAGS: &[TagContract] = &[TAG_D, TAG_A_ADDRESS_REQUIRED];
const WIKI_MERGE_REQUEST_TAGS: &[TagContract] = &[
    TAG_A_ADDRESS_REQUIRED,
    TAG_P_REQUIRED,
    TAG_E_SOURCE_VERSION,
    TAG_E_BASE_VERSION,
];
const CALENDAR_DATE_EVENT_TAGS: &[TagContract] = &[
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
const CALENDAR_TIME_EVENT_TAGS: &[TagContract] = &[
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
const CALENDAR_COLLECTION_TAGS: &[TagContract] = &[
    TAG_CALENDAR_UID,
    TAG_CALENDAR_TITLE,
    TAG_CALENDAR_COLLECTION_DESCRIPTION,
    TAG_CALENDAR_IMAGE,
    TAG_CALENDAR_COLLECTION_EVENT,
];
const CALENDAR_RSVP_TAGS: &[TagContract] = &[
    TAG_CALENDAR_UID,
    TAG_CALENDAR_RSVP_EVENT,
    TAG_CALENDAR_RSVP_REVISION,
    TAG_CALENDAR_RSVP_STATUS,
    TAG_CALENDAR_RSVP_FREE_BUSY,
    TAG_CALENDAR_RSVP_AUTHOR,
];
const FARM_TAGS: &[TagContract] = &[TAG_D, TAG_TITLE, TAG_LOCATION, TAG_IMAGE];
const FOOD_AVAILABILITY_TAGS: &[TagContract] = &[
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
const OPERATIONAL_LISTING_TAGS: &[TagContract] = &[
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
const TRADE_MUTATION_TAGS: &[TagContract] =
    &[TAG_CONTRACT_REQUIRED, TAG_D, TAG_P_REQUIRED, TAG_E_MANY];
const TRADE_VALIDATION_RECEIPT_TAGS: &[TagContract] =
    &[TAG_E_ROOT, TAG_A_OPTIONAL, TAG_SERVICE_OUTPUT];
const KNOWLEDGE_SOURCE_TAGS: &[TagContract] = &[
    TAG_D,
    TAG_CONTRACT_REQUIRED,
    TAG_TOPIC_MANY,
    TAG_SOURCE_MANY,
];
const KNOWLEDGE_CLAIM_TAGS: &[TagContract] = &[
    TAG_CONTRACT_REQUIRED,
    TAG_TOPIC_MANY,
    TAG_SOURCE_MANY,
    TAG_CITATION_MANY,
];
const KNOWLEDGE_RELATION_TAGS: &[TagContract] =
    &[TAG_CONTRACT_REQUIRED, TAG_TOPIC_MANY, TAG_SOURCE_MANY];
const KNOWLEDGE_REVIEW_TAGS: &[TagContract] = &[
    TAG_CONTRACT_REQUIRED,
    TAG_REVIEW_TARGET_REQUIRED,
    TAG_EVIDENCE_MANY,
];
const KNOWLEDGE_FIELD_REPORT_TAGS: &[TagContract] = &[
    TAG_CONTRACT_REQUIRED,
    TAG_TOPIC_MANY,
    TAG_GEOHASH_OPTIONAL,
    TAG_EVIDENCE_MANY,
];
const KNOWLEDGE_CHANGE_PROPOSAL_TAGS: &[TagContract] = &[TAG_CONTRACT_REQUIRED, TAG_EVIDENCE_MANY];
const KNOWLEDGE_CONTRIBUTION_TAGS: &[TagContract] = &[TAG_CONTRACT_REQUIRED, TAG_EVIDENCE_MANY];
const EVIDENCE_BOUNTY_TAGS: &[TagContract] = &[
    TAG_D,
    TAG_CONTRACT_REQUIRED,
    TAG_TOPIC_MANY,
    TAG_EVIDENCE_MANY,
];

const SOCIAL_REDUCERS: &[Reducer] = &[Reducer::SocialProjection];
const PHOTO_UPDATE_TAGS: &[TagContract] = &[TAG_IMETA_REQUIRED_MANY];
const ASK_TAGS: &[TagContract] = &[TAG_ASK_MARKER, TAG_IMETA_OPTIONAL_MANY];
const NIP10_REPLY_TAGS: &[TagContract] = &[TAG_NIP10_E_REQUIRED, TAG_NIP10_P_OPTIONAL];
const PROFILE_REDUCERS: &[Reducer] = &[Reducer::ProfileProjection];
const FARM_OPS_REDUCERS: &[Reducer] = &[Reducer::FarmOpsProjection];
const GROUP_REDUCERS: &[Reducer] = &[Reducer::GroupProjection];
const CALENDAR_REDUCERS: &[Reducer] = &[Reducer::CalendarProjection];
const OPERATIONAL_LISTING_REDUCERS: &[Reducer] = &[
    Reducer::OperationalListingProjection,
    Reducer::MarketProjection,
    Reducer::OperationalListingInventoryAccounting,
];
const FOOD_AVAILABILITY_REDUCERS: &[Reducer] = &[Reducer::MarketProjection];
const TRADE_MUTATION_REDUCERS: &[Reducer] = &[
    Reducer::TradeProjection,
    Reducer::OperationalListingInventoryAccounting,
];
const TRADE_VALIDATION_REDUCERS: &[Reducer] = &[Reducer::TradeValidation];
const RELAY_REDUCERS: &[Reducer] = &[Reducer::NostrRelayPolicyProjection];
const KNOWLEDGE_REDUCERS: &[Reducer] = &[Reducer::KnowledgeProjection];

const FARM_MEMBERS_LIST_DISCRIMINATOR: &[EventDiscriminator] = &[
    EventDiscriminator::DTagPrefix("farm:"),
    EventDiscriminator::DTagSuffix(":members"),
];
const FARM_OWNERS_LIST_DISCRIMINATOR: &[EventDiscriminator] = &[
    EventDiscriminator::DTagPrefix("farm:"),
    EventDiscriminator::DTagSuffix(":members.owners"),
];
const FARM_WORKERS_LIST_DISCRIMINATOR: &[EventDiscriminator] = &[
    EventDiscriminator::DTagPrefix("farm:"),
    EventDiscriminator::DTagSuffix(":members.workers"),
];
const FARM_PLOTS_LIST_DISCRIMINATOR: &[EventDiscriminator] = &[
    EventDiscriminator::DTagPrefix("farm:"),
    EventDiscriminator::DTagSuffix(":plots"),
];
const FARM_LISTINGS_LIST_DISCRIMINATOR: &[EventDiscriminator] = &[
    EventDiscriminator::DTagPrefix("farm:"),
    EventDiscriminator::DTagSuffix(":listings"),
];

macro_rules! kind_contract {
    ($kind:expr, $constant:literal, $name:literal, $class:expr, $standard:expr, [$($contract:literal),+ $(,)?]) => {
        KindContract {
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
        $required_author_role:expr,
        $content_schema:expr,
        $discriminator:expr,
        $tags:expr,
        $reducers:expr,
        $stability:expr $(,)?
    ) => {
        EventContract {
            id: $id,
            kind: $kind,
            name: $name,
            payload_type: $payload_type,
            class: $class,
            stability: $stability,
            privacy: $standard_privacy,
            required_author_role: $required_author_role,
            content_schema: $content_schema,
            authoring_policy: EventAuthoringPolicy::GenericDraft,
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
        $required_author_role:expr,
        $content_schema:expr,
        $authoring_policy:expr,
        $discriminator:expr,
        $tags:expr,
        $reducers:expr $(,)?
    ) => {
        EventContract {
            authoring_policy: $authoring_policy,
            ..event_contract!(
                $id,
                $kind,
                $name,
                $payload_type,
                $class,
                $standard_privacy,
                $required_author_role,
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
        $required_author_role:expr,
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
            $required_author_role,
            $content_schema,
            $discriminator,
            $tags,
            $reducers,
            EventStability::Stable
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
        $required_author_role:expr,
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
            $required_author_role,
            $content_schema,
            $discriminator,
            $tags,
            $reducers,
            EventStability::Experimental
        )
    };
}

static KIND_CONTRACTS_REGISTRY_V7: &[KindContract] = &[
    kind_contract!(
        KIND_PROFILE,
        "KIND_PROFILE",
        "Profile Metadata",
        EventClass::Replaceable,
        NostrStandard::Nip01,
        ["radroots.profile.metadata.v1"]
    ),
    kind_contract!(
        KIND_POST,
        "KIND_POST",
        "Short Text Note",
        EventClass::Regular,
        NostrStandard::Nip01,
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
        EventClass::Replaceable,
        NostrStandard::Nip01,
        ["radroots.social.follow_list.v1"]
    ),
    kind_contract!(
        KIND_DELETION_REQUEST,
        "KIND_DELETION_REQUEST",
        "Deletion Request",
        EventClass::Regular,
        NostrStandard::Nip09,
        ["radroots.social.deletion_request.v1"]
    ),
    kind_contract!(
        KIND_REPOST,
        "KIND_REPOST",
        "Repost",
        EventClass::Regular,
        NostrStandard::Nip18,
        ["radroots.social.repost.v1"]
    ),
    kind_contract!(
        KIND_REACTION,
        "KIND_REACTION",
        "Reaction",
        EventClass::Regular,
        NostrStandard::Nip25,
        ["radroots.social.reaction.v1"]
    ),
    kind_contract!(
        KIND_SEAL,
        "KIND_SEAL",
        "Seal",
        EventClass::Regular,
        NostrStandard::Nip17,
        ["radroots.message.seal.v1"]
    ),
    kind_contract!(
        KIND_MESSAGE,
        "KIND_MESSAGE",
        "Direct Message",
        EventClass::Regular,
        NostrStandard::Nip17,
        ["radroots.message.private.v1"]
    ),
    kind_contract!(
        KIND_MESSAGE_FILE,
        "KIND_MESSAGE_FILE",
        "Direct Message File",
        EventClass::Regular,
        NostrStandard::Nip17,
        ["radroots.message.file.v1"]
    ),
    kind_contract!(
        KIND_GENERIC_REPOST,
        "KIND_GENERIC_REPOST",
        "Generic Repost",
        EventClass::Regular,
        NostrStandard::Nip18,
        ["radroots.social.generic_repost.v1"]
    ),
    kind_contract!(
        KIND_FARM_CRDT_CHANGE,
        "KIND_FARM_CRDT_CHANGE",
        "Farm CRDT Change",
        EventClass::Regular,
        NostrStandard::Radroots,
        ["radroots.farm.crdt_change.v1"]
    ),
    kind_contract!(
        KIND_GIFT_WRAP,
        "KIND_GIFT_WRAP",
        "Gift Wrap",
        EventClass::Regular,
        NostrStandard::Nip17,
        ["radroots.message.gift_wrap.v1"]
    ),
    kind_contract!(
        KIND_FILE_METADATA,
        "KIND_FILE_METADATA",
        "File Metadata",
        EventClass::Regular,
        NostrStandard::Nip94,
        ["radroots.file.metadata.v1"]
    ),
    kind_contract!(
        KIND_COMMENT,
        "KIND_COMMENT",
        "Comment",
        EventClass::Regular,
        NostrStandard::Nip22,
        ["radroots.social.comment.v1"]
    ),
    kind_contract!(
        KIND_REPORT,
        "KIND_REPORT",
        "Report",
        EventClass::Regular,
        NostrStandard::Nip56,
        ["radroots.social.report.v1"]
    ),
    kind_contract!(
        KIND_GROUP_PUT_USER,
        "KIND_GROUP_PUT_USER",
        "Group Put User",
        EventClass::Regular,
        NostrStandard::Nip29,
        ["radroots.group.put_user.v1"]
    ),
    kind_contract!(
        KIND_GROUP_REMOVE_USER,
        "KIND_GROUP_REMOVE_USER",
        "Group Remove User",
        EventClass::Regular,
        NostrStandard::Nip29,
        ["radroots.group.remove_user.v1"]
    ),
    kind_contract!(
        KIND_GROUP_EDIT_METADATA,
        "KIND_GROUP_EDIT_METADATA",
        "Group Edit Metadata",
        EventClass::Regular,
        NostrStandard::Nip29,
        ["radroots.group.edit_metadata.v1"]
    ),
    kind_contract!(
        KIND_GROUP_DELETE_EVENT,
        "KIND_GROUP_DELETE_EVENT",
        "Group Delete Event",
        EventClass::Regular,
        NostrStandard::Nip29,
        ["radroots.group.delete_event.v1"]
    ),
    kind_contract!(
        KIND_GROUP_CREATE_GROUP,
        "KIND_GROUP_CREATE_GROUP",
        "Group Create Group",
        EventClass::Regular,
        NostrStandard::Nip29,
        ["radroots.group.create_group.v1"]
    ),
    kind_contract!(
        KIND_GROUP_DELETE_GROUP,
        "KIND_GROUP_DELETE_GROUP",
        "Group Delete Group",
        EventClass::Regular,
        NostrStandard::Nip29,
        ["radroots.group.delete_group.v1"]
    ),
    kind_contract!(
        KIND_GROUP_CREATE_INVITE,
        "KIND_GROUP_CREATE_INVITE",
        "Group Create Invite",
        EventClass::Regular,
        NostrStandard::Nip29,
        ["radroots.group.create_invite.v1"]
    ),
    kind_contract!(
        KIND_GROUP_JOIN_REQUEST,
        "KIND_GROUP_JOIN_REQUEST",
        "Group Join Request",
        EventClass::Regular,
        NostrStandard::Nip29,
        ["radroots.group.join_request.v1"]
    ),
    kind_contract!(
        KIND_GROUP_LEAVE_REQUEST,
        "KIND_GROUP_LEAVE_REQUEST",
        "Group Leave Request",
        EventClass::Regular,
        NostrStandard::Nip29,
        ["radroots.group.leave_request.v1"]
    ),
    kind_contract!(
        KIND_GEOCHAT,
        "KIND_GEOCHAT",
        "Geochat",
        EventClass::Ephemeral,
        NostrStandard::Nip28,
        ["radroots.social.geochat.v1"]
    ),
    kind_contract!(
        KIND_RELAY_AUTH,
        "KIND_RELAY_AUTH",
        "Relay Auth",
        EventClass::Ephemeral,
        NostrStandard::Nip42,
        ["radroots.relay.auth.v1"]
    ),
    kind_contract!(
        KIND_HTTP_AUTH,
        "KIND_HTTP_AUTH",
        "HTTP Auth",
        EventClass::Ephemeral,
        NostrStandard::Nip98,
        ["radroots.http.auth.v1"]
    ),
    kind_contract!(
        KIND_LIST_MUTE,
        "KIND_LIST_MUTE",
        "Mute List",
        EventClass::Replaceable,
        NostrStandard::Nip51,
        ["radroots.list.mute.v1"]
    ),
    kind_contract!(
        KIND_LIST_PINNED_NOTES,
        "KIND_LIST_PINNED_NOTES",
        "Pinned Notes List",
        EventClass::Replaceable,
        NostrStandard::Nip51,
        ["radroots.list.pinned_notes.v1"]
    ),
    kind_contract!(
        KIND_LIST_READ_WRITE_RELAYS,
        "KIND_LIST_READ_WRITE_RELAYS",
        "Read Write Relays List",
        EventClass::Replaceable,
        NostrStandard::Nip51,
        ["radroots.list.read_write_relays.v1"]
    ),
    kind_contract!(
        KIND_LIST_BOOKMARKS,
        "KIND_LIST_BOOKMARKS",
        "Bookmarks List",
        EventClass::Replaceable,
        NostrStandard::Nip51,
        ["radroots.list.bookmarks.v1"]
    ),
    kind_contract!(
        KIND_LIST_COMMUNITIES,
        "KIND_LIST_COMMUNITIES",
        "Communities List",
        EventClass::Replaceable,
        NostrStandard::Nip51,
        ["radroots.list.communities.v1"]
    ),
    kind_contract!(
        KIND_LIST_PUBLIC_CHATS,
        "KIND_LIST_PUBLIC_CHATS",
        "Public Chats List",
        EventClass::Replaceable,
        NostrStandard::Nip51,
        ["radroots.list.public_chats.v1"]
    ),
    kind_contract!(
        KIND_LIST_BLOCKED_RELAYS,
        "KIND_LIST_BLOCKED_RELAYS",
        "Blocked Relays List",
        EventClass::Replaceable,
        NostrStandard::Nip51,
        ["radroots.list.blocked_relays.v1"]
    ),
    kind_contract!(
        KIND_LIST_SEARCH_RELAYS,
        "KIND_LIST_SEARCH_RELAYS",
        "Search Relays List",
        EventClass::Replaceable,
        NostrStandard::Nip51,
        ["radroots.list.search_relays.v1"]
    ),
    kind_contract!(
        KIND_LIST_SIMPLE_GROUPS,
        "KIND_LIST_SIMPLE_GROUPS",
        "Simple Groups List",
        EventClass::Replaceable,
        NostrStandard::Nip51,
        ["radroots.list.simple_groups.v1"]
    ),
    kind_contract!(
        KIND_LIST_RELAY_FEEDS,
        "KIND_LIST_RELAY_FEEDS",
        "Relay Feeds List",
        EventClass::Replaceable,
        NostrStandard::Nip51,
        ["radroots.list.relay_feeds.v1"]
    ),
    kind_contract!(
        KIND_LIST_INTERESTS,
        "KIND_LIST_INTERESTS",
        "Interests List",
        EventClass::Replaceable,
        NostrStandard::Nip51,
        ["radroots.list.interests.v1"]
    ),
    kind_contract!(
        KIND_LIST_MEDIA_FOLLOWS,
        "KIND_LIST_MEDIA_FOLLOWS",
        "Media Follows List",
        EventClass::Replaceable,
        NostrStandard::Nip51,
        ["radroots.list.media_follows.v1"]
    ),
    kind_contract!(
        KIND_LIST_EMOJIS,
        "KIND_LIST_EMOJIS",
        "Emojis List",
        EventClass::Replaceable,
        NostrStandard::Nip51,
        ["radroots.list.emojis.v1"]
    ),
    kind_contract!(
        KIND_LIST_DM_RELAYS,
        "KIND_LIST_DM_RELAYS",
        "DM Relays List",
        EventClass::Replaceable,
        NostrStandard::Nip51,
        ["radroots.list.dm_relays.v1"]
    ),
    kind_contract!(
        KIND_LIST_GOOD_WIKI_AUTHORS,
        "KIND_LIST_GOOD_WIKI_AUTHORS",
        "Good Wiki Authors List",
        EventClass::Replaceable,
        NostrStandard::Nip51,
        ["radroots.list.good_wiki_authors.v1"]
    ),
    kind_contract!(
        KIND_LIST_GOOD_WIKI_RELAYS,
        "KIND_LIST_GOOD_WIKI_RELAYS",
        "Good Wiki Relays List",
        EventClass::Replaceable,
        NostrStandard::Nip51,
        ["radroots.list.good_wiki_relays.v1"]
    ),
    kind_contract!(
        KIND_LIST_SET_FOLLOW,
        "KIND_LIST_SET_FOLLOW",
        "Follow Set",
        EventClass::Addressable,
        NostrStandard::Nip51,
        ["radroots.list_set.follow.v1"]
    ),
    kind_contract!(
        KIND_LIST_SET_GENERIC,
        "KIND_LIST_SET_GENERIC",
        "Generic List Set",
        EventClass::Addressable,
        NostrStandard::Nip51,
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
        EventClass::Addressable,
        NostrStandard::Nip51,
        ["radroots.list_set.relay.v1"]
    ),
    kind_contract!(
        KIND_LIST_SET_BOOKMARK,
        "KIND_LIST_SET_BOOKMARK",
        "Bookmark Set",
        EventClass::Addressable,
        NostrStandard::Nip51,
        ["radroots.list_set.bookmark.v1"]
    ),
    kind_contract!(
        KIND_LIST_SET_CURATION,
        "KIND_LIST_SET_CURATION",
        "Curation Set",
        EventClass::Addressable,
        NostrStandard::Nip51,
        ["radroots.list_set.curation.v1"]
    ),
    kind_contract!(
        KIND_LIST_SET_VIDEO,
        "KIND_LIST_SET_VIDEO",
        "Video Set",
        EventClass::Addressable,
        NostrStandard::Nip51,
        ["radroots.list_set.video.v1"]
    ),
    kind_contract!(
        KIND_LIST_SET_PICTURE,
        "KIND_LIST_SET_PICTURE",
        "Picture Set",
        EventClass::Addressable,
        NostrStandard::Nip51,
        ["radroots.list_set.picture.v1"]
    ),
    kind_contract!(
        KIND_LIST_SET_KIND_MUTE,
        "KIND_LIST_SET_KIND_MUTE",
        "Kind Mute Set",
        EventClass::Addressable,
        NostrStandard::Nip51,
        ["radroots.list_set.kind_mute.v1"]
    ),
    kind_contract!(
        KIND_LIST_SET_INTEREST,
        "KIND_LIST_SET_INTEREST",
        "Interest Set",
        EventClass::Addressable,
        NostrStandard::Nip51,
        ["radroots.list_set.interest.v1"]
    ),
    kind_contract!(
        KIND_LIST_SET_EMOJI,
        "KIND_LIST_SET_EMOJI",
        "Emoji Set",
        EventClass::Addressable,
        NostrStandard::Nip51,
        ["radroots.list_set.emoji.v1"]
    ),
    kind_contract!(
        KIND_LIST_SET_RELEASE_ARTIFACT,
        "KIND_LIST_SET_RELEASE_ARTIFACT",
        "Release Artifact Set",
        EventClass::Addressable,
        NostrStandard::Nip51,
        ["radroots.list_set.release_artifact.v1"]
    ),
    kind_contract!(
        KIND_LIST_SET_APP_CURATION,
        "KIND_LIST_SET_APP_CURATION",
        "App Curation Set",
        EventClass::Addressable,
        NostrStandard::Nip51,
        ["radroots.list_set.app_curation.v1"]
    ),
    kind_contract!(
        KIND_ARTICLE,
        "KIND_ARTICLE",
        "Long Form Article",
        EventClass::Addressable,
        NostrStandard::Nip23,
        ["radroots.social.article.v1"]
    ),
    kind_contract!(
        KIND_WIKI_MERGE_REQUEST,
        "KIND_WIKI_MERGE_REQUEST",
        "Wiki Merge Request",
        EventClass::Regular,
        NostrStandard::Nip54,
        ["radroots.wiki.merge_request.v1"]
    ),
    kind_contract!(
        KIND_WIKI_ARTICLE,
        "KIND_WIKI_ARTICLE",
        "Wiki Article",
        EventClass::Addressable,
        NostrStandard::Nip54,
        ["radroots.wiki.article.v1"]
    ),
    kind_contract!(
        KIND_WIKI_REDIRECT,
        "KIND_WIKI_REDIRECT",
        "Wiki Redirect",
        EventClass::Addressable,
        NostrStandard::Nip54,
        ["radroots.wiki.redirect.v1"]
    ),
    kind_contract!(
        KIND_CALENDAR_DATE_EVENT,
        "KIND_CALENDAR_DATE_EVENT",
        "Calendar Date Event",
        EventClass::Addressable,
        NostrStandard::Nip52,
        ["radroots.calendar.date_event.v1"]
    ),
    kind_contract!(
        KIND_CALENDAR_TIME_EVENT,
        "KIND_CALENDAR_TIME_EVENT",
        "Calendar Time Event",
        EventClass::Addressable,
        NostrStandard::Nip52,
        ["radroots.calendar.time_event.v1"]
    ),
    kind_contract!(
        KIND_CALENDAR,
        "KIND_CALENDAR",
        "Calendar Collection",
        EventClass::Addressable,
        NostrStandard::Nip52,
        ["radroots.calendar.collection.v1"]
    ),
    kind_contract!(
        KIND_CALENDAR_EVENT_RSVP,
        "KIND_CALENDAR_EVENT_RSVP",
        "Calendar RSVP",
        EventClass::Addressable,
        NostrStandard::Nip52,
        ["radroots.calendar.rsvp.v1"]
    ),
    kind_contract!(
        KIND_LIST_SET_STARTER_PACK,
        "KIND_LIST_SET_STARTER_PACK",
        "Starter Pack Set",
        EventClass::Addressable,
        NostrStandard::Nip51,
        ["radroots.list_set.starter_pack.v1"]
    ),
    kind_contract!(
        KIND_LIST_SET_MEDIA_STARTER_PACK,
        "KIND_LIST_SET_MEDIA_STARTER_PACK",
        "Media Starter Pack Set",
        EventClass::Addressable,
        NostrStandard::Nip51,
        ["radroots.list_set.media_starter_pack.v1"]
    ),
    kind_contract!(
        KIND_FARM,
        "KIND_FARM",
        "Farm",
        EventClass::Addressable,
        NostrStandard::Radroots,
        ["radroots.farm.profile.v1"]
    ),
    kind_contract!(
        KIND_PLOT,
        "KIND_PLOT",
        "Plot",
        EventClass::Addressable,
        NostrStandard::Radroots,
        ["radroots.farm.plot.v1"]
    ),
    kind_contract!(
        KIND_COOP,
        "KIND_COOP",
        "Coop",
        EventClass::Addressable,
        NostrStandard::Radroots,
        ["radroots.farm.coop.v1"]
    ),
    kind_contract!(
        KIND_DOCUMENT,
        "KIND_DOCUMENT",
        "Document",
        EventClass::Addressable,
        NostrStandard::Radroots,
        ["radroots.farm.document.v1"]
    ),
    kind_contract!(
        KIND_RESOURCE_AREA,
        "KIND_RESOURCE_AREA",
        "Resource Area",
        EventClass::Addressable,
        NostrStandard::Radroots,
        ["radroots.farm.resource_area.v1"]
    ),
    kind_contract!(
        KIND_RESOURCE_HARVEST_CAP,
        "KIND_RESOURCE_HARVEST_CAP",
        "Resource Harvest Capacity",
        EventClass::Addressable,
        NostrStandard::Radroots,
        ["radroots.farm.resource_harvest_cap.v1"]
    ),
    kind_contract!(
        KIND_ACCOUNT_CLAIM,
        "KIND_ACCOUNT_CLAIM",
        "Account Claim",
        EventClass::Addressable,
        NostrStandard::Radroots,
        ["radroots.account.claim.v1"]
    ),
    kind_contract!(
        KIND_FARM_WORKSPACE_MANIFEST,
        "KIND_FARM_WORKSPACE_MANIFEST",
        "Farm Workspace Manifest",
        EventClass::Addressable,
        NostrStandard::Nip78,
        ["radroots.farm.workspace_manifest.v1"]
    ),
    kind_contract!(
        KIND_CLASSIFIED_LISTING,
        "KIND_CLASSIFIED_LISTING",
        "Classified Listing",
        EventClass::Addressable,
        NostrStandard::Nip99,
        [
            "radroots.operational_listing.published.v1",
            "radroots.food.availability.v1"
        ]
    ),
    kind_contract!(
        KIND_KNOWLEDGE_SOURCE,
        "KIND_KNOWLEDGE_SOURCE",
        "Knowledge Source",
        EventClass::Addressable,
        NostrStandard::Radroots,
        ["radroots.knowledge.source.v1"]
    ),
    kind_contract!(
        KIND_EVIDENCE_BOUNTY,
        "KIND_EVIDENCE_BOUNTY",
        "Evidence Bounty",
        EventClass::Addressable,
        NostrStandard::Radroots,
        ["radroots.knowledge.evidence_bounty.v1"]
    ),
    kind_contract!(
        KIND_KNOWLEDGE_CLAIM,
        "KIND_KNOWLEDGE_CLAIM",
        "Knowledge Claim",
        EventClass::Regular,
        NostrStandard::Radroots,
        ["radroots.knowledge.claim.v1"]
    ),
    kind_contract!(
        KIND_KNOWLEDGE_RELATION,
        "KIND_KNOWLEDGE_RELATION",
        "Knowledge Relation",
        EventClass::Regular,
        NostrStandard::Radroots,
        ["radroots.knowledge.relation.v1"]
    ),
    kind_contract!(
        KIND_KNOWLEDGE_REVIEW,
        "KIND_KNOWLEDGE_REVIEW",
        "Knowledge Review",
        EventClass::Regular,
        NostrStandard::Radroots,
        ["radroots.knowledge.review.v1"]
    ),
    kind_contract!(
        KIND_KNOWLEDGE_FIELD_REPORT,
        "KIND_KNOWLEDGE_FIELD_REPORT",
        "Knowledge Field Report",
        EventClass::Regular,
        NostrStandard::Radroots,
        ["radroots.knowledge.field_report.v1"]
    ),
    kind_contract!(
        KIND_KNOWLEDGE_CHANGE_PROPOSAL,
        "KIND_KNOWLEDGE_CHANGE_PROPOSAL",
        "Knowledge Change Proposal",
        EventClass::Regular,
        NostrStandard::Radroots,
        ["radroots.knowledge.change_proposal.v1"]
    ),
    kind_contract!(
        KIND_CONTRIBUTION_ATTESTATION,
        "KIND_CONTRIBUTION_ATTESTATION",
        "Contribution Attestation",
        EventClass::Regular,
        NostrStandard::Radroots,
        ["radroots.knowledge.contribution_attestation.v1"]
    ),
    kind_contract!(
        KIND_APPLICATION_HANDLER,
        "KIND_APPLICATION_HANDLER",
        "Application Handler",
        EventClass::Addressable,
        NostrStandard::Radroots,
        ["radroots.application.handler.v1"]
    ),
    kind_contract!(
        KIND_GROUP_METADATA,
        "KIND_GROUP_METADATA",
        "Group Metadata",
        EventClass::Addressable,
        NostrStandard::Nip29,
        ["radroots.group.metadata.v1"]
    ),
    kind_contract!(
        KIND_GROUP_ADMINS,
        "KIND_GROUP_ADMINS",
        "Group Admins",
        EventClass::Addressable,
        NostrStandard::Nip29,
        ["radroots.group.admins.v1"]
    ),
    kind_contract!(
        KIND_GROUP_MEMBERS,
        "KIND_GROUP_MEMBERS",
        "Group Members",
        EventClass::Addressable,
        NostrStandard::Nip29,
        ["radroots.group.members.v1"]
    ),
    kind_contract!(
        KIND_GROUP_ROLES,
        "KIND_GROUP_ROLES",
        "Group Roles",
        EventClass::Addressable,
        NostrStandard::Nip29,
        ["radroots.group.roles.v1"]
    ),
    kind_contract!(
        KIND_TRADE_PROPOSAL,
        "KIND_TRADE_PROPOSAL",
        "Trade Proposal",
        EventClass::Regular,
        NostrStandard::Radroots,
        ["radroots.trade.proposal.v1"]
    ),
    kind_contract!(
        KIND_TRADE_DECISION,
        "KIND_TRADE_DECISION",
        "Trade Decision",
        EventClass::Regular,
        NostrStandard::Radroots,
        ["radroots.trade.decision.v1"]
    ),
    kind_contract!(
        KIND_TRADE_REVISION_PROPOSAL,
        "KIND_TRADE_REVISION_PROPOSAL",
        "Trade Revision Proposal",
        EventClass::Regular,
        NostrStandard::Radroots,
        ["radroots.trade.revision_proposal.v1"]
    ),
    kind_contract!(
        KIND_TRADE_REVISION_DECISION,
        "KIND_TRADE_REVISION_DECISION",
        "Trade Revision Decision",
        EventClass::Regular,
        NostrStandard::Radroots,
        ["radroots.trade.revision_decision.v1"]
    ),
    kind_contract!(
        KIND_TRADE_CANCELLATION,
        "KIND_TRADE_CANCELLATION",
        "Trade Cancellation",
        EventClass::Regular,
        NostrStandard::Radroots,
        ["radroots.trade.cancellation.v1"]
    ),
    kind_contract!(
        KIND_TRADE_VALIDATION_RECEIPT,
        "KIND_TRADE_VALIDATION_RECEIPT",
        "Trade Validation Receipt",
        EventClass::Regular,
        NostrStandard::Radroots,
        ["radroots.trade.validation_receipt.v1"]
    ),
];

static EVENT_CONTRACTS_REGISTRY_V7: &[EventContract] = &[
    event_contract_with_authoring_policy!(
        "radroots.profile.metadata.v1",
        KIND_PROFILE,
        "Profile Metadata",
        "RadrootsAuthoredProfile / RadrootsInboundProfileMetadata",
        EventClass::Replaceable,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::JsonObject,
        EventAuthoringPolicy::TypedOnly,
        EventDiscriminator::KindOnly,
        PROFILE_TAGS,
        PROFILE_REDUCERS
    ),
    event_contract_with_authoring_policy!(
        "radroots.social.post.v1",
        KIND_POST,
        "Short Text Note",
        "RadrootsPost",
        EventClass::Regular,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::PlainText,
        EventAuthoringPolicy::ReadOnly,
        EventDiscriminator::KindOnly,
        NO_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract_with_authoring_policy!(
        "radroots.social.update.v1",
        KIND_POST,
        "Root Text Update",
        "RadrootsAuthoredUpdate / RadrootsInboundPostProjection",
        EventClass::Regular,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::PlainText,
        EventAuthoringPolicy::TypedOnly,
        EventDiscriminator::AdmissionOnly,
        NO_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract_with_authoring_policy!(
        "radroots.social.photo_update.v1",
        KIND_POST,
        "NIP-92 Photo Update",
        "RadrootsAuthoredPhotoUpdate / RadrootsInboundPostProjection",
        EventClass::Regular,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::PlainText,
        EventAuthoringPolicy::TypedOnly,
        EventDiscriminator::AdmissionOnly,
        PHOTO_UPDATE_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract_with_authoring_policy!(
        "radroots.social.ask.v1",
        KIND_POST,
        "Root Ask",
        "RadrootsAuthoredAsk / RadrootsInboundPostProjection",
        EventClass::Regular,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::PlainText,
        EventAuthoringPolicy::TypedOnly,
        EventDiscriminator::AdmissionOnly,
        ASK_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract_with_authoring_policy!(
        "radroots.social.reply.v1",
        KIND_POST,
        "NIP-10 Reply",
        "RadrootsAuthoredNip10Reply / RadrootsInboundNip10ReplyProjection",
        EventClass::Regular,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::PlainText,
        EventAuthoringPolicy::TypedOnly,
        EventDiscriminator::AdmissionOnly,
        NIP10_REPLY_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.social.follow_list.v1",
        KIND_FOLLOW,
        "Contact List",
        "RadrootsFollowList",
        EventClass::Replaceable,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::JsonObject,
        EventDiscriminator::KindOnly,
        P_TAGS,
        PROFILE_REDUCERS
    ),
    event_contract_with_authoring_policy!(
        "radroots.social.deletion_request.v1",
        KIND_DELETION_REQUEST,
        "Deletion Request",
        "RadrootsAuthoredNip09DeletionRequest / RadrootsInboundNip09DeletionProjection",
        EventClass::Regular,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::PlainText,
        EventAuthoringPolicy::TypedOnly,
        EventDiscriminator::AdmissionOnly,
        NIP09_DELETION_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.social.repost.v1",
        KIND_REPOST,
        "Repost",
        "RadrootsRepost",
        EventClass::Regular,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::JsonObject,
        EventDiscriminator::KindOnly,
        EVENT_POINTER_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.social.reaction.v1",
        KIND_REACTION,
        "Reaction",
        "RadrootsReaction",
        EventClass::Regular,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::PlainText,
        EventDiscriminator::KindOnly,
        EVENT_POINTER_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.message.seal.v1",
        KIND_SEAL,
        "Seal",
        "RadrootsSeal",
        EventClass::Regular,
        EventPrivacy::Encrypted,
        AuthorRole::Any,
        ContentSchema::Encrypted,
        EventDiscriminator::KindOnly,
        NO_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.message.private.v1",
        KIND_MESSAGE,
        "Direct Message",
        "RadrootsMessage",
        EventClass::Regular,
        EventPrivacy::Encrypted,
        AuthorRole::Any,
        ContentSchema::Encrypted,
        EventDiscriminator::KindOnly,
        P_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.message.file.v1",
        KIND_MESSAGE_FILE,
        "Direct Message File",
        "RadrootsMessageFile",
        EventClass::Regular,
        EventPrivacy::Encrypted,
        AuthorRole::Any,
        ContentSchema::Encrypted,
        EventDiscriminator::KindOnly,
        P_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.social.generic_repost.v1",
        KIND_GENERIC_REPOST,
        "Generic Repost",
        "RadrootsGenericRepost",
        EventClass::Regular,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::JsonObject,
        EventDiscriminator::KindOnly,
        EVENT_POINTER_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.farm.crdt_change.v1",
        KIND_FARM_CRDT_CHANGE,
        "Farm CRDT Change",
        "RadrootsFarmCrdtChange",
        EventClass::Regular,
        EventPrivacy::Encrypted,
        AuthorRole::Farmer,
        ContentSchema::JsonObject,
        EventDiscriminator::KindOnly,
        NO_TAGS,
        FARM_OPS_REDUCERS
    ),
    event_contract!(
        "radroots.message.gift_wrap.v1",
        KIND_GIFT_WRAP,
        "Gift Wrap",
        "RadrootsGiftWrap",
        EventClass::Regular,
        EventPrivacy::Encrypted,
        AuthorRole::Any,
        ContentSchema::Encrypted,
        EventDiscriminator::KindOnly,
        P_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.file.metadata.v1",
        KIND_FILE_METADATA,
        "File Metadata",
        "RadrootsFileMetadata",
        EventClass::Regular,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::JsonObject,
        EventDiscriminator::KindOnly,
        FILE_METADATA_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract_with_authoring_policy!(
        "radroots.social.comment.v1",
        KIND_COMMENT,
        "Comment",
        "RadrootsAuthoredNip22Comment / RadrootsInboundNip22CommentProjection",
        EventClass::Regular,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::PlainText,
        EventAuthoringPolicy::TypedOnly,
        EventDiscriminator::AdmissionOnly,
        NIP22_COMMENT_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.social.report.v1",
        KIND_REPORT,
        "Report",
        "RadrootsReport",
        EventClass::Regular,
        EventPrivacy::Public,
        AuthorRole::Moderator,
        ContentSchema::PlainText,
        EventDiscriminator::KindOnly,
        EVENT_POINTER_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.group.put_user.v1",
        KIND_GROUP_PUT_USER,
        "Group Put User",
        "RadrootsGroupPutUser",
        EventClass::Regular,
        EventPrivacy::Public,
        AuthorRole::Moderator,
        ContentSchema::JsonObject,
        EventDiscriminator::KindOnly,
        GROUP_ACTION_TAGS,
        GROUP_REDUCERS
    ),
    event_contract!(
        "radroots.group.remove_user.v1",
        KIND_GROUP_REMOVE_USER,
        "Group Remove User",
        "RadrootsGroupRemoveUser",
        EventClass::Regular,
        EventPrivacy::Public,
        AuthorRole::Moderator,
        ContentSchema::JsonObject,
        EventDiscriminator::KindOnly,
        GROUP_ACTION_TAGS,
        GROUP_REDUCERS
    ),
    event_contract!(
        "radroots.group.edit_metadata.v1",
        KIND_GROUP_EDIT_METADATA,
        "Group Edit Metadata",
        "RadrootsGroupEditMetadata",
        EventClass::Regular,
        EventPrivacy::Public,
        AuthorRole::Moderator,
        ContentSchema::JsonObject,
        EventDiscriminator::KindOnly,
        GROUP_ACTION_TAGS,
        GROUP_REDUCERS
    ),
    event_contract!(
        "radroots.group.delete_event.v1",
        KIND_GROUP_DELETE_EVENT,
        "Group Delete Event",
        "RadrootsGroupDeleteEvent",
        EventClass::Regular,
        EventPrivacy::Public,
        AuthorRole::Moderator,
        ContentSchema::JsonObject,
        EventDiscriminator::KindOnly,
        GROUP_ACTION_TAGS,
        GROUP_REDUCERS
    ),
    event_contract!(
        "radroots.group.create_group.v1",
        KIND_GROUP_CREATE_GROUP,
        "Group Create Group",
        "RadrootsGroupCreateGroup",
        EventClass::Regular,
        EventPrivacy::Public,
        AuthorRole::Moderator,
        ContentSchema::JsonObject,
        EventDiscriminator::KindOnly,
        GROUP_ACTION_TAGS,
        GROUP_REDUCERS
    ),
    event_contract!(
        "radroots.group.delete_group.v1",
        KIND_GROUP_DELETE_GROUP,
        "Group Delete Group",
        "RadrootsGroupDeleteGroup",
        EventClass::Regular,
        EventPrivacy::Public,
        AuthorRole::Moderator,
        ContentSchema::JsonObject,
        EventDiscriminator::KindOnly,
        GROUP_ACTION_TAGS,
        GROUP_REDUCERS
    ),
    event_contract!(
        "radroots.group.create_invite.v1",
        KIND_GROUP_CREATE_INVITE,
        "Group Create Invite",
        "RadrootsGroupCreateInvite",
        EventClass::Regular,
        EventPrivacy::Public,
        AuthorRole::Moderator,
        ContentSchema::JsonObject,
        EventDiscriminator::KindOnly,
        GROUP_ACTION_TAGS,
        GROUP_REDUCERS
    ),
    event_contract!(
        "radroots.group.join_request.v1",
        KIND_GROUP_JOIN_REQUEST,
        "Group Join Request",
        "RadrootsGroupJoinRequest",
        EventClass::Regular,
        EventPrivacy::Public,
        AuthorRole::Member,
        ContentSchema::JsonObject,
        EventDiscriminator::KindOnly,
        GROUP_ACTION_TAGS,
        GROUP_REDUCERS
    ),
    event_contract!(
        "radroots.group.leave_request.v1",
        KIND_GROUP_LEAVE_REQUEST,
        "Group Leave Request",
        "RadrootsGroupLeaveRequest",
        EventClass::Regular,
        EventPrivacy::Public,
        AuthorRole::Member,
        ContentSchema::JsonObject,
        EventDiscriminator::KindOnly,
        GROUP_ACTION_TAGS,
        GROUP_REDUCERS
    ),
    event_contract!(
        "radroots.social.geochat.v1",
        KIND_GEOCHAT,
        "Geochat",
        "RadrootsGeochat",
        EventClass::Ephemeral,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::PlainText,
        EventDiscriminator::KindOnly,
        NO_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.relay.auth.v1",
        KIND_RELAY_AUTH,
        "Relay Auth",
        "RadrootsRelayAuth",
        EventClass::Ephemeral,
        EventPrivacy::Public,
        AuthorRole::Relay,
        ContentSchema::JsonObject,
        EventDiscriminator::KindOnly,
        NO_TAGS,
        RELAY_REDUCERS
    ),
    event_contract!(
        "radroots.http.auth.v1",
        KIND_HTTP_AUTH,
        "HTTP Auth",
        "RadrootsHttpAuth",
        EventClass::Ephemeral,
        EventPrivacy::Public,
        AuthorRole::Application,
        ContentSchema::JsonObject,
        EventDiscriminator::KindOnly,
        NO_TAGS,
        RELAY_REDUCERS
    ),
    event_contract!(
        "radroots.list.mute.v1",
        KIND_LIST_MUTE,
        "Mute List",
        "RadrootsList",
        EventClass::Replaceable,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::JsonObject,
        EventDiscriminator::KindOnly,
        LIST_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.list.pinned_notes.v1",
        KIND_LIST_PINNED_NOTES,
        "Pinned Notes List",
        "RadrootsList",
        EventClass::Replaceable,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::JsonObject,
        EventDiscriminator::KindOnly,
        LIST_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.list.read_write_relays.v1",
        KIND_LIST_READ_WRITE_RELAYS,
        "Read Write Relays List",
        "RadrootsList",
        EventClass::Replaceable,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::JsonObject,
        EventDiscriminator::KindOnly,
        LIST_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.list.bookmarks.v1",
        KIND_LIST_BOOKMARKS,
        "Bookmarks List",
        "RadrootsList",
        EventClass::Replaceable,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::JsonObject,
        EventDiscriminator::KindOnly,
        LIST_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.list.communities.v1",
        KIND_LIST_COMMUNITIES,
        "Communities List",
        "RadrootsList",
        EventClass::Replaceable,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::JsonObject,
        EventDiscriminator::KindOnly,
        LIST_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.list.public_chats.v1",
        KIND_LIST_PUBLIC_CHATS,
        "Public Chats List",
        "RadrootsList",
        EventClass::Replaceable,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::JsonObject,
        EventDiscriminator::KindOnly,
        LIST_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.list.blocked_relays.v1",
        KIND_LIST_BLOCKED_RELAYS,
        "Blocked Relays List",
        "RadrootsList",
        EventClass::Replaceable,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::JsonObject,
        EventDiscriminator::KindOnly,
        LIST_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.list.search_relays.v1",
        KIND_LIST_SEARCH_RELAYS,
        "Search Relays List",
        "RadrootsList",
        EventClass::Replaceable,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::JsonObject,
        EventDiscriminator::KindOnly,
        LIST_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.list.simple_groups.v1",
        KIND_LIST_SIMPLE_GROUPS,
        "Simple Groups List",
        "RadrootsList",
        EventClass::Replaceable,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::JsonObject,
        EventDiscriminator::KindOnly,
        LIST_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.list.relay_feeds.v1",
        KIND_LIST_RELAY_FEEDS,
        "Relay Feeds List",
        "RadrootsList",
        EventClass::Replaceable,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::JsonObject,
        EventDiscriminator::KindOnly,
        LIST_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.list.interests.v1",
        KIND_LIST_INTERESTS,
        "Interests List",
        "RadrootsList",
        EventClass::Replaceable,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::JsonObject,
        EventDiscriminator::KindOnly,
        LIST_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.list.media_follows.v1",
        KIND_LIST_MEDIA_FOLLOWS,
        "Media Follows List",
        "RadrootsList",
        EventClass::Replaceable,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::JsonObject,
        EventDiscriminator::KindOnly,
        LIST_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.list.emojis.v1",
        KIND_LIST_EMOJIS,
        "Emojis List",
        "RadrootsList",
        EventClass::Replaceable,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::JsonObject,
        EventDiscriminator::KindOnly,
        LIST_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.list.dm_relays.v1",
        KIND_LIST_DM_RELAYS,
        "DM Relays List",
        "RadrootsList",
        EventClass::Replaceable,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::JsonObject,
        EventDiscriminator::KindOnly,
        LIST_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.list.good_wiki_authors.v1",
        KIND_LIST_GOOD_WIKI_AUTHORS,
        "Good Wiki Authors List",
        "RadrootsList",
        EventClass::Replaceable,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::JsonObject,
        EventDiscriminator::KindOnly,
        LIST_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.list.good_wiki_relays.v1",
        KIND_LIST_GOOD_WIKI_RELAYS,
        "Good Wiki Relays List",
        "RadrootsList",
        EventClass::Replaceable,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::JsonObject,
        EventDiscriminator::KindOnly,
        LIST_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.list_set.follow.v1",
        KIND_LIST_SET_FOLLOW,
        "Follow Set",
        "RadrootsListSet",
        EventClass::Addressable,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::JsonObject,
        EventDiscriminator::KindOnly,
        LIST_SET_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.list_set.farm.members.v1",
        KIND_LIST_SET_GENERIC,
        "Farm Members List Set",
        "RadrootsListSet",
        EventClass::Addressable,
        EventPrivacy::Public,
        AuthorRole::Farmer,
        ContentSchema::JsonObject,
        EventDiscriminator::Composite(FARM_MEMBERS_LIST_DISCRIMINATOR),
        LIST_SET_TAGS,
        FARM_OPS_REDUCERS
    ),
    event_contract!(
        "radroots.list_set.farm.members.owners.v1",
        KIND_LIST_SET_GENERIC,
        "Farm Owners List Set",
        "RadrootsListSet",
        EventClass::Addressable,
        EventPrivacy::Public,
        AuthorRole::Farmer,
        ContentSchema::JsonObject,
        EventDiscriminator::Composite(FARM_OWNERS_LIST_DISCRIMINATOR),
        LIST_SET_TAGS,
        FARM_OPS_REDUCERS
    ),
    event_contract!(
        "radroots.list_set.farm.members.workers.v1",
        KIND_LIST_SET_GENERIC,
        "Farm Workers List Set",
        "RadrootsListSet",
        EventClass::Addressable,
        EventPrivacy::Public,
        AuthorRole::Farmer,
        ContentSchema::JsonObject,
        EventDiscriminator::Composite(FARM_WORKERS_LIST_DISCRIMINATOR),
        LIST_SET_TAGS,
        FARM_OPS_REDUCERS
    ),
    event_contract!(
        "radroots.list_set.farm.plots.v1",
        KIND_LIST_SET_GENERIC,
        "Farm Plots List Set",
        "RadrootsListSet",
        EventClass::Addressable,
        EventPrivacy::Public,
        AuthorRole::Farmer,
        ContentSchema::JsonObject,
        EventDiscriminator::Composite(FARM_PLOTS_LIST_DISCRIMINATOR),
        LIST_SET_TAGS,
        FARM_OPS_REDUCERS
    ),
    event_contract!(
        "radroots.list_set.farm.listings.v1",
        KIND_LIST_SET_GENERIC,
        "Farm Listings List Set",
        "RadrootsListSet",
        EventClass::Addressable,
        EventPrivacy::Public,
        AuthorRole::Farmer,
        ContentSchema::JsonObject,
        EventDiscriminator::Composite(FARM_LISTINGS_LIST_DISCRIMINATOR),
        LIST_SET_TAGS,
        FARM_OPS_REDUCERS
    ),
    event_contract!(
        "radroots.list_set.member_of.farms.v1",
        KIND_LIST_SET_GENERIC,
        "Member Of Farms List Set",
        "RadrootsListSet",
        EventClass::Addressable,
        EventPrivacy::Public,
        AuthorRole::Member,
        ContentSchema::JsonObject,
        EventDiscriminator::DTagExact("member_of.farms"),
        LIST_SET_TAGS,
        FARM_OPS_REDUCERS
    ),
    event_contract!(
        "radroots.list_set.relay.v1",
        KIND_LIST_SET_RELAY,
        "Relay Set",
        "RadrootsListSet",
        EventClass::Addressable,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::JsonObject,
        EventDiscriminator::KindOnly,
        LIST_SET_TAGS,
        RELAY_REDUCERS
    ),
    event_contract!(
        "radroots.list_set.bookmark.v1",
        KIND_LIST_SET_BOOKMARK,
        "Bookmark Set",
        "RadrootsListSet",
        EventClass::Addressable,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::JsonObject,
        EventDiscriminator::KindOnly,
        LIST_SET_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.list_set.curation.v1",
        KIND_LIST_SET_CURATION,
        "Curation Set",
        "RadrootsListSet",
        EventClass::Addressable,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::JsonObject,
        EventDiscriminator::KindOnly,
        LIST_SET_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.list_set.video.v1",
        KIND_LIST_SET_VIDEO,
        "Video Set",
        "RadrootsListSet",
        EventClass::Addressable,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::JsonObject,
        EventDiscriminator::KindOnly,
        LIST_SET_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.list_set.picture.v1",
        KIND_LIST_SET_PICTURE,
        "Picture Set",
        "RadrootsListSet",
        EventClass::Addressable,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::JsonObject,
        EventDiscriminator::KindOnly,
        LIST_SET_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.list_set.kind_mute.v1",
        KIND_LIST_SET_KIND_MUTE,
        "Kind Mute Set",
        "RadrootsListSet",
        EventClass::Addressable,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::JsonObject,
        EventDiscriminator::KindOnly,
        LIST_SET_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.list_set.interest.v1",
        KIND_LIST_SET_INTEREST,
        "Interest Set",
        "RadrootsListSet",
        EventClass::Addressable,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::JsonObject,
        EventDiscriminator::KindOnly,
        LIST_SET_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.list_set.emoji.v1",
        KIND_LIST_SET_EMOJI,
        "Emoji Set",
        "RadrootsListSet",
        EventClass::Addressable,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::JsonObject,
        EventDiscriminator::KindOnly,
        LIST_SET_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.list_set.release_artifact.v1",
        KIND_LIST_SET_RELEASE_ARTIFACT,
        "Release Artifact Set",
        "RadrootsListSet",
        EventClass::Addressable,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::JsonObject,
        EventDiscriminator::KindOnly,
        LIST_SET_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.list_set.app_curation.v1",
        KIND_LIST_SET_APP_CURATION,
        "App Curation Set",
        "RadrootsListSet",
        EventClass::Addressable,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::JsonObject,
        EventDiscriminator::KindOnly,
        LIST_SET_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.social.article.v1",
        KIND_ARTICLE,
        "Long Form Article",
        "RadrootsArticle",
        EventClass::Addressable,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::Markdown,
        EventDiscriminator::KindOnly,
        ARTICLE_TAGS,
        SOCIAL_REDUCERS
    ),
    experimental_event_contract!(
        "radroots.wiki.merge_request.v1",
        KIND_WIKI_MERGE_REQUEST,
        "Wiki Merge Request",
        "RadrootsWikiMergeRequest",
        EventClass::Regular,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::PlainText,
        EventDiscriminator::KindOnly,
        WIKI_MERGE_REQUEST_TAGS,
        KNOWLEDGE_REDUCERS
    ),
    experimental_event_contract!(
        "radroots.wiki.article.v1",
        KIND_WIKI_ARTICLE,
        "Wiki Article",
        "RadrootsWikiArticle",
        EventClass::Addressable,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::Djot,
        EventDiscriminator::KindOnly,
        WIKI_ARTICLE_TAGS,
        KNOWLEDGE_REDUCERS
    ),
    experimental_event_contract!(
        "radroots.wiki.redirect.v1",
        KIND_WIKI_REDIRECT,
        "Wiki Redirect",
        "RadrootsWikiRedirect",
        EventClass::Addressable,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::Empty,
        EventDiscriminator::KindOnly,
        WIKI_REDIRECT_TAGS,
        KNOWLEDGE_REDUCERS
    ),
    event_contract_with_authoring_policy!(
        "radroots.calendar.date_event.v1",
        KIND_CALENDAR_DATE_EVENT,
        "Calendar Date Event",
        "RadrootsAuthoredCalendarDateEvent / RadrootsParsedNip52CalendarDateEvent / RadrootsAdmittedCalendarDateEvent",
        EventClass::Addressable,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::PlainText,
        EventAuthoringPolicy::TypedOnly,
        EventDiscriminator::KindOnly,
        CALENDAR_DATE_EVENT_TAGS,
        CALENDAR_REDUCERS
    ),
    event_contract_with_authoring_policy!(
        "radroots.calendar.time_event.v1",
        KIND_CALENDAR_TIME_EVENT,
        "Calendar Time Event",
        "RadrootsAuthoredCalendarTimeEvent / RadrootsParsedNip52CalendarTimeEvent / RadrootsAdmittedCalendarTimeEvent",
        EventClass::Addressable,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::PlainText,
        EventAuthoringPolicy::TypedOnly,
        EventDiscriminator::KindOnly,
        CALENDAR_TIME_EVENT_TAGS,
        CALENDAR_REDUCERS
    ),
    event_contract!(
        "radroots.calendar.collection.v1",
        KIND_CALENDAR,
        "Calendar Collection",
        "RadrootsAdmittedCalendar",
        EventClass::Addressable,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::PlainText,
        EventDiscriminator::KindOnly,
        CALENDAR_COLLECTION_TAGS,
        CALENDAR_REDUCERS
    ),
    event_contract!(
        "radroots.calendar.rsvp.v1",
        KIND_CALENDAR_EVENT_RSVP,
        "Calendar RSVP",
        "RadrootsAdmittedCalendarEventRsvp",
        EventClass::Addressable,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::PlainText,
        EventDiscriminator::KindOnly,
        CALENDAR_RSVP_TAGS,
        CALENDAR_REDUCERS
    ),
    event_contract!(
        "radroots.list_set.starter_pack.v1",
        KIND_LIST_SET_STARTER_PACK,
        "Starter Pack Set",
        "RadrootsListSet",
        EventClass::Addressable,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::JsonObject,
        EventDiscriminator::KindOnly,
        LIST_SET_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.list_set.media_starter_pack.v1",
        KIND_LIST_SET_MEDIA_STARTER_PACK,
        "Media Starter Pack Set",
        "RadrootsListSet",
        EventClass::Addressable,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::JsonObject,
        EventDiscriminator::KindOnly,
        LIST_SET_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.farm.profile.v1",
        KIND_FARM,
        "Farm",
        "RadrootsFarm",
        EventClass::Addressable,
        EventPrivacy::Public,
        AuthorRole::Farmer,
        ContentSchema::JsonObject,
        EventDiscriminator::KindOnly,
        FARM_TAGS,
        FARM_OPS_REDUCERS
    ),
    event_contract!(
        "radroots.farm.plot.v1",
        KIND_PLOT,
        "Plot",
        "RadrootsPlot",
        EventClass::Addressable,
        EventPrivacy::Public,
        AuthorRole::Farmer,
        ContentSchema::JsonObject,
        EventDiscriminator::KindOnly,
        FARM_TAGS,
        FARM_OPS_REDUCERS
    ),
    event_contract!(
        "radroots.farm.coop.v1",
        KIND_COOP,
        "Coop",
        "RadrootsCoop",
        EventClass::Addressable,
        EventPrivacy::Public,
        AuthorRole::Farmer,
        ContentSchema::JsonObject,
        EventDiscriminator::KindOnly,
        FARM_TAGS,
        FARM_OPS_REDUCERS
    ),
    event_contract!(
        "radroots.farm.document.v1",
        KIND_DOCUMENT,
        "Document",
        "RadrootsDocument",
        EventClass::Addressable,
        EventPrivacy::Public,
        AuthorRole::Farmer,
        ContentSchema::JsonObject,
        EventDiscriminator::KindOnly,
        D_TAGS,
        FARM_OPS_REDUCERS
    ),
    event_contract!(
        "radroots.farm.resource_area.v1",
        KIND_RESOURCE_AREA,
        "Resource Area",
        "RadrootsResourceArea",
        EventClass::Addressable,
        EventPrivacy::Public,
        AuthorRole::Farmer,
        ContentSchema::JsonObject,
        EventDiscriminator::KindOnly,
        FARM_TAGS,
        FARM_OPS_REDUCERS
    ),
    event_contract!(
        "radroots.farm.resource_harvest_cap.v1",
        KIND_RESOURCE_HARVEST_CAP,
        "Resource Harvest Capacity",
        "RadrootsResourceHarvestCap",
        EventClass::Addressable,
        EventPrivacy::Public,
        AuthorRole::Farmer,
        ContentSchema::JsonObject,
        EventDiscriminator::KindOnly,
        FARM_TAGS,
        FARM_OPS_REDUCERS
    ),
    event_contract!(
        "radroots.account.claim.v1",
        KIND_ACCOUNT_CLAIM,
        "Account Claim",
        "RadrootsAccountClaim",
        EventClass::Addressable,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::JsonObject,
        EventDiscriminator::KindOnly,
        D_TAGS,
        PROFILE_REDUCERS
    ),
    event_contract!(
        "radroots.farm.workspace_manifest.v1",
        KIND_FARM_WORKSPACE_MANIFEST,
        "Farm Workspace Manifest",
        "RadrootsFarmWorkspaceManifest",
        EventClass::Addressable,
        EventPrivacy::Encrypted,
        AuthorRole::Farmer,
        ContentSchema::JsonObject,
        EventDiscriminator::KindOnly,
        D_TAGS,
        FARM_OPS_REDUCERS
    ),
    event_contract!(
        "radroots.operational_listing.published.v1",
        KIND_CLASSIFIED_LISTING,
        "Operational Listing",
        "RadrootsOperationalListing",
        EventClass::Addressable,
        EventPrivacy::Public,
        AuthorRole::Seller,
        ContentSchema::Markdown,
        EventDiscriminator::ClassifiedListingPartition(
            ClassifiedListingPartition::OperationalListing,
        ),
        OPERATIONAL_LISTING_TAGS,
        OPERATIONAL_LISTING_REDUCERS
    ),
    event_contract_with_authoring_policy!(
        "radroots.food.availability.v1",
        KIND_CLASSIFIED_LISTING,
        "Food Availability",
        "RadrootsFoodAvailabilityDetails / RadrootsInboundFoodAvailabilityProjection",
        EventClass::Addressable,
        EventPrivacy::Public,
        AuthorRole::Seller,
        ContentSchema::Markdown,
        EventAuthoringPolicy::TypedOnly,
        EventDiscriminator::AdmissionOnly,
        FOOD_AVAILABILITY_TAGS,
        FOOD_AVAILABILITY_REDUCERS
    ),
    experimental_event_contract!(
        "radroots.knowledge.source.v1",
        KIND_KNOWLEDGE_SOURCE,
        "Knowledge Source",
        "RadrootsKnowledgeSource",
        EventClass::Addressable,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::JsonObject,
        EventDiscriminator::TagEquals {
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
        EventClass::Addressable,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::JsonObject,
        EventDiscriminator::TagEquals {
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
        EventClass::Regular,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::JsonObject,
        EventDiscriminator::TagEquals {
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
        EventClass::Regular,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::JsonObject,
        EventDiscriminator::TagEquals {
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
        EventClass::Regular,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::JsonObject,
        EventDiscriminator::TagEquals {
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
        EventClass::Regular,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::JsonObject,
        EventDiscriminator::TagEquals {
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
        EventClass::Regular,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::JsonObject,
        EventDiscriminator::TagEquals {
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
        EventClass::Regular,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::JsonObject,
        EventDiscriminator::TagEquals {
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
        EventClass::Addressable,
        EventPrivacy::Public,
        AuthorRole::Application,
        ContentSchema::JsonObject,
        EventDiscriminator::KindOnly,
        D_TAGS,
        SOCIAL_REDUCERS
    ),
    event_contract!(
        "radroots.group.metadata.v1",
        KIND_GROUP_METADATA,
        "Group Metadata",
        "RadrootsGroupMetadata",
        EventClass::Addressable,
        EventPrivacy::Public,
        AuthorRole::Moderator,
        ContentSchema::JsonObject,
        EventDiscriminator::KindOnly,
        GROUP_STATE_TAGS,
        GROUP_REDUCERS
    ),
    event_contract!(
        "radroots.group.admins.v1",
        KIND_GROUP_ADMINS,
        "Group Admins",
        "RadrootsGroupAdmins",
        EventClass::Addressable,
        EventPrivacy::Public,
        AuthorRole::Moderator,
        ContentSchema::JsonObject,
        EventDiscriminator::KindOnly,
        GROUP_STATE_TAGS,
        GROUP_REDUCERS
    ),
    event_contract!(
        "radroots.group.members.v1",
        KIND_GROUP_MEMBERS,
        "Group Members",
        "RadrootsGroupMembers",
        EventClass::Addressable,
        EventPrivacy::Public,
        AuthorRole::Moderator,
        ContentSchema::JsonObject,
        EventDiscriminator::KindOnly,
        GROUP_STATE_TAGS,
        GROUP_REDUCERS
    ),
    event_contract!(
        "radroots.group.roles.v1",
        KIND_GROUP_ROLES,
        "Group Roles",
        "RadrootsGroupRoles",
        EventClass::Addressable,
        EventPrivacy::Public,
        AuthorRole::Moderator,
        ContentSchema::JsonObject,
        EventDiscriminator::KindOnly,
        GROUP_STATE_TAGS,
        GROUP_REDUCERS
    ),
    event_contract!(
        "radroots.trade.proposal.v1",
        KIND_TRADE_PROPOSAL,
        "Trade Proposal",
        "RadrootsTradeMutationEnvelopeV1",
        EventClass::Regular,
        EventPrivacy::Public,
        AuthorRole::Buyer,
        ContentSchema::JsonObject,
        EventDiscriminator::ContentJsonFieldEquals {
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
        EventClass::Regular,
        EventPrivacy::Public,
        AuthorRole::Seller,
        ContentSchema::JsonObject,
        EventDiscriminator::ContentJsonFieldEquals {
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
        EventClass::Regular,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::JsonObject,
        EventDiscriminator::ContentJsonFieldEquals {
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
        EventClass::Regular,
        EventPrivacy::Public,
        AuthorRole::Any,
        ContentSchema::JsonObject,
        EventDiscriminator::ContentJsonFieldEquals {
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
        EventClass::Regular,
        EventPrivacy::Public,
        AuthorRole::Buyer,
        ContentSchema::JsonObject,
        EventDiscriminator::ContentJsonFieldEquals {
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
        EventClass::Regular,
        EventPrivacy::Public,
        AuthorRole::Service,
        ContentSchema::JsonObject,
        EventDiscriminator::KindOnly,
        TRADE_VALIDATION_RECEIPT_TAGS,
        TRADE_VALIDATION_REDUCERS
    ),
];

pub fn all_kind_contracts() -> &'static [KindContract] {
    all_kind_contracts_registry_v7()
}

/// Returns the immutable kind-contract inventory for registry v7.
pub const fn all_kind_contracts_registry_v7() -> &'static [KindContract] {
    KIND_CONTRACTS_REGISTRY_V7
}

pub fn all_event_contracts() -> &'static [EventContract] {
    all_event_contracts_registry_v7()
}

/// Returns the immutable event-contract inventory for registry v7.
pub const fn all_event_contracts_registry_v7() -> &'static [EventContract] {
    EVENT_CONTRACTS_REGISTRY_V7
}

pub fn contract_families() -> &'static [ContractFamilyMetadata] {
    CONTRACT_FAMILIES
}

pub fn event_contract_family(contract: &EventContract) -> Option<ContractFamily> {
    contract_family_for_id(contract.id)
}

pub fn kind_contract_family(contract: &KindContract) -> Option<ContractFamily> {
    Some(match contract.kind {
        KIND_PROFILE | KIND_FOLLOW | KIND_ACCOUNT_CLAIM => ContractFamily::Profile,
        KIND_SEAL | KIND_MESSAGE | KIND_MESSAGE_FILE | KIND_GIFT_WRAP => ContractFamily::Message,
        KIND_COMMENT
        | KIND_DELETION_REQUEST
        | KIND_GEOCHAT
        | KIND_POST
        | KIND_REACTION
        | KIND_REPOST
        | KIND_GENERIC_REPOST
        | KIND_ARTICLE
        | KIND_FILE_METADATA => ContractFamily::Social,
        KIND_RELAY_AUTH | KIND_HTTP_AUTH => ContractFamily::Relay,
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
        | KIND_GROUP_ROLES => ContractFamily::Group,
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
        | KIND_LIST_SET_MEDIA_STARTER_PACK => ContractFamily::List,
        KIND_CALENDAR_DATE_EVENT
        | KIND_CALENDAR_TIME_EVENT
        | KIND_CALENDAR
        | KIND_CALENDAR_EVENT_RSVP => ContractFamily::Calendar,
        KIND_FARM
        | KIND_PLOT
        | KIND_COOP
        | KIND_DOCUMENT
        | KIND_RESOURCE_AREA
        | KIND_RESOURCE_HARVEST_CAP
        | KIND_FARM_WORKSPACE_MANIFEST
        | KIND_FARM_CRDT_CHANGE => ContractFamily::Farm,
        KIND_CLASSIFIED_LISTING => ContractFamily::Market,
        KIND_TRADE_VALIDATION_RECEIPT
        | KIND_TRADE_PROPOSAL
        | KIND_TRADE_DECISION
        | KIND_TRADE_REVISION_PROPOSAL
        | KIND_TRADE_REVISION_DECISION
        | KIND_TRADE_CANCELLATION => ContractFamily::Trade,
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
        | KIND_CONTRIBUTION_ATTESTATION => ContractFamily::Knowledge,
        KIND_JOB_FEEDBACK => ContractFamily::Job,
        _ if [
            is_request_kind(contract.kind),
            is_result_kind(contract.kind),
        ]
        .contains(&true) =>
        {
            ContractFamily::Job
        }
        _ => return None,
    })
}

pub fn kind_contract(kind: u32) -> Option<&'static KindContract> {
    kind_contract_registry_v7(kind)
}

/// Resolves a kind contract from the immutable registry-v7 inventory.
pub fn kind_contract_registry_v7(kind: u32) -> Option<&'static KindContract> {
    KIND_CONTRACTS_REGISTRY_V7
        .iter()
        .find(|contract| contract.kind == kind)
}

pub fn event_contract(id: &str) -> Option<&'static EventContract> {
    event_contract_registry_v7(id)
}

/// Resolves an event contract from the immutable registry-v7 inventory.
///
/// Event-store reconciliation v1 depends on this historical entry point.
/// Later registries must retain it and add a new versioned lookup.
pub fn event_contract_registry_v7(id: &str) -> Option<&'static EventContract> {
    EVENT_CONTRACTS_REGISTRY_V7
        .iter()
        .find(|contract| contract.id == id)
}

pub fn event_contracts_for_kind(kind: u32) -> impl Iterator<Item = &'static EventContract> {
    EVENT_CONTRACTS_REGISTRY_V7
        .iter()
        .filter(move |contract| contract.kind == kind)
}

pub fn identify_event_contract(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<&'static EventContract, ContractMatchError> {
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
    kind_contracts: &'static [KindContract],
    event_contracts: &'static [EventContract],
) -> Result<&'static EventContract, ContractMatchError> {
    crate::require_invariant(
        kind_contracts.iter().any(|contract| contract.kind == kind),
        &|| ContractMatchError::UnsupportedKind(kind),
    )?;
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
    event: &EventEnvelope,
) -> Result<&'static EventContract, ContractValidationError> {
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
    event: &EventEnvelope,
) -> Result<&'static EventContract, ContractValidationError> {
    validate_event_contract_in_registry(
        event,
        KIND_CONTRACTS_REGISTRY_V7,
        EVENT_CONTRACTS_REGISTRY_V7,
    )
}

fn validate_event_contract_in_registry(
    event: &EventEnvelope,
    kind_contracts: &'static [KindContract],
    event_contracts: &'static [EventContract],
) -> Result<&'static EventContract, ContractValidationError> {
    let tags = event.tags_as_vec();
    let contract = identify_event_contract_in_registry(
        event.kind_u32(),
        &tags,
        event.content(),
        kind_contracts,
        event_contracts,
    )
    .map_err(|error| ContractValidationError::ContractMatch { error })?;
    validate_event_contract_parts_in_registry(
        event.kind_u32(),
        &tags,
        event.content(),
        contract,
        event_contracts,
        false,
    )?;
    Ok(contract)
}

pub fn validate_event_contract_shape(
    event: &EventEnvelope,
    contract_id: &str,
) -> Result<(), ContractValidationError> {
    let tags = event.tags_as_vec();
    validate_event_contract_parts(event.kind_u32(), &tags, event.content(), contract_id)
}

/// Validates a contract selected explicitly by an admission boundary.
///
/// This is the required path for contracts whose discriminator is
/// [`EventDiscriminator::AdmissionOnly`]. The selected contract's kind,
/// content, tags, and custom invariants are still validated in full.
pub fn validate_event_contract_for_admission(
    event: &EventEnvelope,
    contract_id: &str,
) -> Result<&'static EventContract, ContractValidationError> {
    let contract =
        event_contract(contract_id).ok_or_else(|| ContractValidationError::UnknownContract {
            contract_id: contract_id.to_owned(),
        })?;
    let tags = event.tags_as_vec();
    validate_event_contract_parts_in_registry(
        event.kind_u32(),
        &tags,
        event.content(),
        contract,
        EVENT_CONTRACTS_REGISTRY_V7,
        true,
    )?;
    Ok(contract)
}

pub fn validate_event_contract_parts(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
    contract_id: &str,
) -> Result<(), ContractValidationError> {
    let contract =
        event_contract(contract_id).ok_or_else(|| ContractValidationError::UnknownContract {
            contract_id: contract_id.to_owned(),
        })?;
    validate_event_contract_parts_in_registry(
        kind,
        tags,
        content,
        contract,
        EVENT_CONTRACTS_REGISTRY_V7,
        false,
    )
}

fn validate_event_contract_parts_in_registry(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
    contract: &EventContract,
    event_contracts: &'static [EventContract],
    admission_selected: bool,
) -> Result<(), ContractValidationError> {
    crate::require_invariant(kind == contract.kind, &|| {
        ContractValidationError::KindMismatch {
            expected: contract.kind,
            actual: kind,
        }
    })?;
    crate::require_invariant(
        admission_selected || !matches!(contract.discriminator, EventDiscriminator::AdmissionOnly),
        &|| ContractValidationError::AdmissionRequired {
            contract_id: contract.id,
        },
    )?;
    validate_classified_listing_partition_parts(tags, contract)?;
    validate_content_shape_parts(content, contract)?;
    validate_contract_tags_parts_in_registry(tags, contract, event_contracts)?;
    if admission_selected {
        validate_admission_selected_contract_parts(kind, tags, contract)?;
    }
    validate_discriminator_parts(content, contract, admission_selected)?;
    validate_custom_calendar_contract_parts(tags, contract)?;
    validate_custom_knowledge_contract_parts(content, contract)?;
    Ok(())
}

fn validate_admission_selected_contract_parts(
    kind: u32,
    tags: &[Vec<String>],
    contract: &EventContract,
) -> Result<(), ContractValidationError> {
    if contract.id == "radroots.social.deletion_request.v1"
        && !tags.iter().any(|tag| {
            tag.first()
                .is_some_and(|name| matches!(name.as_str(), "e" | "a"))
        })
    {
        return Err(ContractValidationError::ContractMatch {
            error: ContractMatchError::UnsupportedShape(kind),
        });
    }
    Ok(())
}

fn validate_classified_listing_partition_parts(
    tags: &[Vec<String>],
    contract: &EventContract,
) -> Result<(), ContractValidationError> {
    let EventDiscriminator::ClassifiedListingPartition(expected) = contract.discriminator else {
        return Ok(());
    };
    if classify_classified_listing_raw_tags_registry_v7(tags) == expected {
        Ok(())
    } else {
        Err(ContractValidationError::ContractMatch {
            error: ContractMatchError::UnsupportedShape(contract.kind),
        })
    }
}

fn classify_classified_listing_raw_tags_registry_v7(
    tags: &[Vec<String>],
) -> ClassifiedListingPartition {
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
            return ClassifiedListingPartition::Ambiguous;
        }
    }

    match (has_focused_marker, has_operational_marker) {
        (true, false) => ClassifiedListingPartition::FocusedFoodAvailability,
        (false, true) => ClassifiedListingPartition::OperationalListing,
        (false, false) => ClassifiedListingPartition::GenericNip99,
        (true, true) => ClassifiedListingPartition::Ambiguous,
    }
}

fn identify_from_contracts<'a, I>(
    contracts: I,
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<&'a EventContract, ContractMatchError>
where
    I: IntoIterator<Item = &'a EventContract>,
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
        (None, _) => Err(ContractMatchError::UnsupportedShape(kind)),
        (Some(_), _) => Err(ContractMatchError::AmbiguousShape(kind)),
    }
}

fn contract_family_for_id(id: &str) -> Option<ContractFamily> {
    const PREFIX_FAMILIES: [(&str, ContractFamily); 18] = [
        ("radroots.account.", ContractFamily::Account),
        ("radroots.application.", ContractFamily::Application),
        ("radroots.calendar.", ContractFamily::Calendar),
        ("radroots.farm.", ContractFamily::Farm),
        ("radroots.group.", ContractFamily::Group),
        ("radroots.http.", ContractFamily::Http),
        ("radroots.job.", ContractFamily::Job),
        ("radroots.knowledge.", ContractFamily::Knowledge),
        ("radroots.wiki.", ContractFamily::Knowledge),
        ("radroots.list.", ContractFamily::List),
        ("radroots.list_set.", ContractFamily::List),
        ("radroots.operational_listing.", ContractFamily::Market),
        ("radroots.food.", ContractFamily::Market),
        ("radroots.message.", ContractFamily::Message),
        ("radroots.profile.", ContractFamily::Profile),
        ("radroots.relay.", ContractFamily::Relay),
        ("radroots.social.", ContractFamily::Social),
        ("radroots.trade.", ContractFamily::Trade),
    ];

    PREFIX_FAMILIES
        .iter()
        .find_map(|(prefix, family)| id.starts_with(prefix).then_some(*family))
}

fn validate_content_shape_parts(
    content: &str,
    contract: &EventContract,
) -> Result<(), ContractValidationError> {
    match contract.content_schema {
        ContentSchema::Empty => {
            if content.is_empty() {
                Ok(())
            } else {
                Err(ContractValidationError::ContentMustBeEmpty {
                    contract_id: contract.id,
                })
            }
        }
        ContentSchema::JsonObject => parse_content_object(content, contract.id).map(|_| ()),
        _ => Ok(()),
    }
}

#[cfg(test)]
fn validate_contract_tags_parts(
    tags: &[Vec<String>],
    contract: &EventContract,
) -> Result<(), ContractValidationError> {
    validate_contract_tags_parts_in_registry(tags, contract, EVENT_CONTRACTS_REGISTRY_V7)
}

fn validate_contract_tags_parts_in_registry(
    tags: &[Vec<String>],
    contract: &EventContract,
    event_contracts: &'static [EventContract],
) -> Result<(), ContractValidationError> {
    for tag_contract in contract.tags {
        let count = tag_count(tags, tag_contract.name);
        let has_multiple_contracts_for_name = contract
            .tags
            .iter()
            .filter(|candidate| candidate.name == tag_contract.name)
            .count()
            > 1;
        match tag_contract.cardinality {
            TagCardinality::RequiredOne => {
                crate::require_invariant(count != 0, &|| ContractValidationError::MissingTag {
                    contract_id: contract.id,
                    name: tag_contract.name,
                })?;
                crate::require_invariant(
                    [count == 1, has_multiple_contracts_for_name].contains(&true),
                    &|| ContractValidationError::TagCardinalityMismatch {
                        contract_id: contract.id,
                        name: tag_contract.name,
                    },
                )?;
            }
            TagCardinality::RequiredMany => {
                crate::require_invariant(count != 0, &|| ContractValidationError::MissingTag {
                    contract_id: contract.id,
                    name: tag_contract.name,
                })?;
            }
            TagCardinality::OptionalOne => {
                crate::require_invariant(
                    [count <= 1, has_multiple_contracts_for_name].contains(&true),
                    &|| ContractValidationError::TagCardinalityMismatch {
                        contract_id: contract.id,
                        name: tag_contract.name,
                    },
                )?;
            }
            TagCardinality::OptionalMany => {}
        }
        if tag_contract.name == "contract" {
            let actual = tag_value(tags, "contract").map(ToOwned::to_owned);
            crate::require_invariant(actual.as_deref() == Some(contract.id), &|| {
                ContractValidationError::TagValueMismatch {
                    contract_id: contract.id,
                    name: "contract",
                    expected: contract.id.to_owned(),
                    actual: actual.clone(),
                }
            })?;
        }
        validate_contract_tag_values_in_registry(tags, contract, tag_contract, event_contracts)?;
    }
    Ok(())
}

fn validate_contract_tag_values_in_registry(
    tags: &[Vec<String>],
    contract: &EventContract,
    tag_contract: &TagContract,
    event_contracts: &'static [EventContract],
) -> Result<(), ContractValidationError> {
    for tag in tags
        .iter()
        .filter(|tag| tag.first().map(|value| value.as_str()) == Some(tag_contract.name))
    {
        crate::require_invariant(
            tag_value_is_valid_in_registry(tag, tag_contract.value_type, event_contracts),
            &|| ContractValidationError::TagValueMismatch {
                contract_id: contract.id,
                name: tag_contract.name,
                expected: tag_value_type_expectation(tag_contract.value_type).to_owned(),
                actual: tag.get(1).cloned(),
            },
        )?;
    }
    Ok(())
}

#[cfg(test)]
fn tag_value_is_valid(tag: &[String], value_type: TagValueType) -> bool {
    tag_value_is_valid_in_registry(tag, value_type, EVENT_CONTRACTS_REGISTRY_V7)
}

fn tag_value_is_valid_in_registry(
    tag: &[String],
    value_type: TagValueType,
    event_contracts: &'static [EventContract],
) -> bool {
    let Some(value) = tag.get(1).map(String::as_str) else {
        return false;
    };
    match value_type {
        TagValueType::AddressableCoordinate => AddressableCoordinate::parse(value).is_ok(),
        TagValueType::CalendarDate => CalendarDate::parse(value).is_ok(),
        TagValueType::CalendarEventCoordinate => {
            canonical_calendar_event_coordinate_is_valid(value)
        }
        TagValueType::CalendarFreeBusy => matches!(value, "free" | "busy"),
        TagValueType::CalendarRsvpStatus => {
            matches!(value, "accepted" | "declined" | "tentative")
        }
        TagValueType::CalendarUid => CalendarUid::parse(value).is_ok(),
        TagValueType::ContractId => event_contracts.iter().any(|contract| contract.id == value),
        TagValueType::DTag => DTag::parse(value).is_ok(),
        TagValueType::EventId | TagValueType::Sha256 => EventId::parse(value).is_ok(),
        TagValueType::EventPointer => event_pointer_tag_is_valid(tag),
        TagValueType::Geohash => geohash_is_valid(value),
        TagValueType::IanaTimeZoneId => IanaTimeZoneId::parse(value).is_ok(),
        TagValueType::Kind => value.parse::<u32>().is_ok(),
        TagValueType::Nip01Coordinate => Nip01Coordinate::parse(value).is_ok(),
        TagValueType::PublicKey => parse_public_key(value).is_ok(),
        TagValueType::RelayUrl => relay_url_is_valid(value),
        TagValueType::Text => visible_text_is_valid(value),
        TagValueType::UnixTimestamp => value.parse::<u64>().is_ok(),
        TagValueType::Uri => CalendarUri::parse(value).is_ok(),
        TagValueType::Url => url_is_valid(value),
        TagValueType::UtcDayIndex => canonical_u64(value).is_some(),
        TagValueType::Uuid => uuid_is_valid(value),
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
    [
        EventId::parse(id).is_ok(),
        parse_public_key(author).is_ok(),
        kind.parse::<u32>().is_ok(),
        [d_tag.is_empty(), DTag::parse(d_tag).is_ok()].contains(&true),
        tag.iter()
            .skip(5)
            .all(|relay| relay_url_is_valid(relay.as_str())),
    ] == [true; 5]
}

fn visible_text_is_valid(value: &str) -> bool {
    [
        !value.trim().is_empty(),
        !value.chars().any(char::is_control),
    ] == [true; 2]
}

fn url_is_valid(value: &str) -> bool {
    [
        value
            .strip_prefix("https://")
            .or_else(|| value.strip_prefix("http://"))
            .is_some_and(|remainder| !remainder.is_empty()),
        value.trim() == value,
        !value.chars().any(char::is_control),
    ] == [true; 3]
}

fn geohash_is_valid(value: &str) -> bool {
    [
        !value.is_empty(),
        value.len() <= 12,
        value
            .bytes()
            .all(|byte| matches!(byte.to_ascii_lowercase(), b'0'..=b'9' | b'b'..=b'h' | b'j'..=b'k' | b'm'..=b'n' | b'p'..=b'z')),
    ] == [true; 3]
}

fn uuid_is_valid(value: &str) -> bool {
    let bytes = value.as_bytes();
    [
        bytes.len() == 36,
        bytes.iter().enumerate().all(|(index, byte)| match index {
            8 | 13 | 18 | 23 => *byte == b'-',
            _ => byte.is_ascii_hexdigit(),
        }),
    ] == [true; 2]
}

fn tag_value_type_expectation(value_type: TagValueType) -> &'static str {
    match value_type {
        TagValueType::AddressableCoordinate => "addressable_coordinate",
        TagValueType::CalendarDate => "calendar_date_yyyy_mm_dd",
        TagValueType::CalendarEventCoordinate => "canonical_kind_31922_or_31923_coordinate",
        TagValueType::CalendarFreeBusy => "calendar_free_or_busy",
        TagValueType::CalendarRsvpStatus => "calendar_rsvp_status",
        TagValueType::CalendarUid => "canonical_128_bit_base64url_calendar_uid",
        TagValueType::ContractId => "contract_id",
        TagValueType::DTag => "d_tag",
        TagValueType::EventId => "event_id",
        TagValueType::EventPointer => "event_pointer",
        TagValueType::Geohash => "geohash",
        TagValueType::IanaTimeZoneId => "canonical_iana_time_zone_id",
        TagValueType::Kind => "kind",
        TagValueType::Nip01Coordinate => "nip01_coordinate",
        TagValueType::PublicKey => "public_key",
        TagValueType::RelayUrl => "relay_url",
        TagValueType::Sha256 => "sha256",
        TagValueType::Text => "text",
        TagValueType::UnixTimestamp => "unix_timestamp",
        TagValueType::Uri => "absolute_uri",
        TagValueType::Url => "url",
        TagValueType::UtcDayIndex => "canonical_decimal_utc_day_index",
        TagValueType::Uuid => "uuid",
    }
}

fn canonical_u64(value: &str) -> Option<u64> {
    let valid = [
        !value.is_empty(),
        [value.len() > 1, value.starts_with('0')] != [true; 2],
        value.bytes().all(|byte| byte.is_ascii_digit()),
    ];
    (valid == [true; 3]).then_some(value.parse().ok()).flatten()
}

fn validate_custom_calendar_contract_parts(
    tags: &[Vec<String>],
    contract: &EventContract,
) -> Result<(), ContractValidationError> {
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
    contract: &EventContract,
) -> Result<(), ContractValidationError> {
    validate_exact_calendar_tags(tags, contract, &["d", "title", "description", "image"])?;
    validate_canonical_calendar_text_tags(tags, contract, &["title", "description"])?;
    validate_calendar_event_reference_tags(tags, contract)?;
    validate_calendar_blossom_image(tags, contract)?;

    let event_references = tags
        .iter()
        .filter(|tag| tag.first().map(String::as_str) == Some("a"))
        .collect::<Vec<_>>();
    for (index, reference) in event_references.iter().enumerate() {
        crate::require_invariant(
            !event_references
                .iter()
                .skip(index + 1)
                .any(|candidate| candidate.get(1) == reference.get(1)),
            &|| {
                calendar_tag_mismatch(
                    contract,
                    "a",
                    "duplicate_free_calendar_event_coordinates",
                    reference.get(1).cloned(),
                )
            },
        )?;
    }
    Ok(())
}

fn validate_calendar_rsvp_contract(
    tags: &[Vec<String>],
    contract: &EventContract,
) -> Result<(), ContractValidationError> {
    validate_exact_calendar_tags(tags, contract, &["d", "status", "fb"])?;
    validate_calendar_event_reference_tags(tags, contract)?;
    validate_calendar_rsvp_pointer_tag(tags, contract, "e", true)?;
    validate_calendar_rsvp_pointer_tag(tags, contract, "p", false)?;

    let event_author = tags
        .iter()
        .find(|tag| tag.first().map(String::as_str) == Some("a"))
        .and_then(|tag| tag.get(1))
        .and_then(|coordinate| crate::id::AddressableCoordinateParts::parse(coordinate).ok())
        .map(|parts| parts.pubkey);
    if let Some(author_hint) = tag_value(tags, "p") {
        let hint = parse_public_key(author_hint).ok();
        crate::require_invariant(
            [
                hint.as_ref() == event_author.as_ref(),
                hint.as_ref().is_some_and(|key| key.to_hex() == author_hint),
            ] == [true; 2],
            &|| {
                calendar_tag_mismatch(
                    contract,
                    "p",
                    "canonical_calendar_event_author_matching_a_coordinate",
                    Some(author_hint.to_owned()),
                )
            },
        )?;
    }
    Ok(())
}

fn validate_calendar_event_reference_tags(
    tags: &[Vec<String>],
    contract: &EventContract,
) -> Result<(), ContractValidationError> {
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
        crate::require_invariant(
            [
                (2..=3).contains(&tag.len()),
                coordinate_is_valid,
                relay_is_valid,
            ] == [true; 3],
            &|| {
                calendar_tag_mismatch(
                    contract,
                    "a",
                    "canonical_kind_31922_or_31923_coordinate_with_optional_relay",
                    tag.get(1).cloned(),
                )
            },
        )?;
    }
    Ok(())
}

fn validate_calendar_rsvp_pointer_tag(
    tags: &[Vec<String>],
    contract: &EventContract,
    name: &'static str,
    event_id: bool,
) -> Result<(), ContractValidationError> {
    for tag in tags
        .iter()
        .filter(|tag| tag.first().map(String::as_str) == Some(name))
    {
        let value_is_canonical = tag.get(1).is_some_and(|value| {
            if event_id {
                EventId::parse(value).is_ok_and(|parsed| parsed.to_hex() == *value)
            } else {
                parse_public_key(value).is_ok_and(|parsed| parsed.to_hex() == *value)
            }
        });
        let relay_is_valid = tag
            .get(2)
            .is_none_or(|relay| !relay.is_empty() && relay_url_is_valid(relay));
        crate::require_invariant(
            [
                (2..=3).contains(&tag.len()),
                value_is_canonical,
                relay_is_valid,
            ] == [true; 3],
            &|| {
                calendar_tag_mismatch(
                    contract,
                    name,
                    if event_id {
                        "canonical_event_id_with_optional_relay"
                    } else {
                        "canonical_public_key_with_optional_relay"
                    },
                    tag.get(1).cloned(),
                )
            },
        )?;
    }
    Ok(())
}

fn validate_canonical_calendar_text_tags(
    tags: &[Vec<String>],
    contract: &EventContract,
    names: &[&'static str],
) -> Result<(), ContractValidationError> {
    for name in names {
        for tag in tags
            .iter()
            .filter(|tag| tag.first().map(String::as_str) == Some(*name))
        {
            crate::require_invariant(
                tag.get(1)
                    .is_some_and(|value| canonical_calendar_tag_text_is_valid(value)),
                &|| {
                    calendar_tag_mismatch(
                        contract,
                        name,
                        "canonical_visible_calendar_text",
                        tag.get(1).cloned(),
                    )
                },
            )?;
        }
    }
    Ok(())
}

fn validate_calendar_blossom_image(
    tags: &[Vec<String>],
    contract: &EventContract,
) -> Result<(), ContractValidationError> {
    tag_value(tags, "image")
        .map(|image| {
            crate::require_invariant(BlobUrl::parse(image).is_ok(), &|| {
                calendar_tag_mismatch(
                    contract,
                    "image",
                    "structural_blossom_hash_path_url",
                    Some(image.to_owned()),
                )
            })
        })
        .transpose()
        .map(|_| ())
}

fn validate_calendar_date_contract(
    tags: &[Vec<String>],
    contract: &EventContract,
) -> Result<(), ContractValidationError> {
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

    let forbidden_day_tag = tags
        .iter()
        .find(|tag| tag.first().map(String::as_str) == Some("D"));
    crate::require_invariant(forbidden_day_tag.is_none(), &|| {
        calendar_tag_mismatch(
            contract,
            "D",
            "forbidden_on_calendar_date_event",
            forbidden_day_tag.and_then(|tag| tag.get(1)).cloned(),
        )
    })?;

    let start = calendar_date_tag(tags, contract, "start")?;
    optional_calendar_date_tag(tags, contract, "end")?
        .map(|end| {
            crate::require_invariant(end > start, &|| {
                calendar_tag_mismatch(
                    contract,
                    "end",
                    "gregorian_date_later_than_start",
                    Some(end.as_str().to_owned()),
                )
            })
        })
        .transpose()
        .map(|_| ())
}

fn validate_calendar_time_contract(
    tags: &[Vec<String>],
    contract: &EventContract,
) -> Result<(), ContractValidationError> {
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
    crate::require_invariant(!end.is_some_and(|end| end <= start), &|| {
        calendar_tag_mismatch(
            contract,
            "end",
            "canonical_unix_seconds_later_than_start",
            tag_value(tags, "end").map(ToOwned::to_owned),
        )
    })?;

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
    contract: &EventContract,
    names: &[&'static str],
) -> Result<(), ContractValidationError> {
    for name in names {
        for tag in tags
            .iter()
            .filter(|tag| tag.first().map(String::as_str) == Some(*name))
        {
            crate::require_invariant(tag.len() == 2, &|| {
                calendar_tag_mismatch(contract, name, "exact_two_element_tag", tag.get(1).cloned())
            })?;
        }
    }
    Ok(())
}

fn validate_calendar_participant_tags(
    tags: &[Vec<String>],
    contract: &EventContract,
) -> Result<(), ContractValidationError> {
    crate::require_invariant(
        tag_count(tags, "p") <= RADROOTS_CALENDAR_MAX_PARTICIPANTS,
        &|| calendar_tag_mismatch(contract, "p", "bounded_participant_count", None),
    )?;
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
        crate::require_invariant(
            [
                (2..=4).contains(&tag.len()),
                pubkey_is_canonical,
                relay_is_valid,
                role_is_valid,
                placeholder_is_canonical,
            ] == [true; 5],
            &|| {
                calendar_tag_mismatch(
                    contract,
                    "p",
                    "participant_pubkey_with_optional_relay_and_role",
                    tag.get(1).cloned(),
                )
            },
        )?;
    }
    Ok(())
}

fn validate_calendar_inclusion_request_tags(
    tags: &[Vec<String>],
    contract: &EventContract,
) -> Result<(), ContractValidationError> {
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
        crate::require_invariant(
            [
                (2..=3).contains(&tag.len()),
                coordinate_is_calendar,
                relay_is_valid,
            ] == [true; 3],
            &|| {
                calendar_tag_mismatch(
                    contract,
                    "a",
                    "kind_31924_coordinate_with_optional_relay",
                    tag.get(1).cloned(),
                )
            },
        )?;
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
    let Ok(parts) = crate::id::AddressableCoordinateParts::parse(value) else {
        return false;
    };
    [
        kind == "31924",
        pubkey == parts.pubkey.to_hex(),
        d_tag == parts.d_tag.as_str(),
    ] == [true; 3]
}

fn canonical_calendar_event_coordinate_is_valid(value: &str) -> bool {
    let Some((kind, remainder)) = value.split_once(':') else {
        return false;
    };
    let Some((pubkey, d_tag)) = remainder.split_once(':') else {
        return false;
    };
    let Ok(parts) = crate::id::AddressableCoordinateParts::parse(value) else {
        return false;
    };
    [
        matches!(
            parts.kind,
            KIND_CALENDAR_DATE_EVENT | KIND_CALENDAR_TIME_EVENT
        ),
        matches!(kind, "31922" | "31923"),
        pubkey == parts.pubkey.to_hex(),
        d_tag == parts.d_tag.as_str(),
    ] == [true; 4]
}

fn validate_canonical_calendar_common_tags(
    tags: &[Vec<String>],
    contract: &EventContract,
) -> Result<(), ContractValidationError> {
    for name in ["title", "location", "summary", "t", "name"] {
        for tag in tags
            .iter()
            .filter(|tag| tag.first().map(String::as_str) == Some(name))
        {
            crate::require_invariant(
                tag.get(1)
                    .is_some_and(|value| canonical_calendar_tag_text_is_valid(value)),
                &|| {
                    calendar_tag_mismatch(
                        contract,
                        name,
                        "canonical_visible_calendar_text",
                        tag.get(1).cloned(),
                    )
                },
            )?;
        }
    }
    tag_value(tags, "g")
        .map(|geohash| {
            crate::require_invariant(canonical_calendar_geohash_is_valid(geohash), &|| {
                calendar_tag_mismatch(
                    contract,
                    "g",
                    "canonical_lowercase_geohash",
                    Some(geohash.to_owned()),
                )
            })
        })
        .transpose()?;
    validate_calendar_blossom_image(tags, contract)?;
    Ok(())
}

fn calendar_date_tag(
    tags: &[Vec<String>],
    contract: &EventContract,
    name: &'static str,
) -> Result<CalendarDate, ContractValidationError> {
    let value = tag_value(tags, name).ok_or(ContractValidationError::MissingTag {
        contract_id: contract.id,
        name,
    })?;
    CalendarDate::parse(value).map_err(|_| {
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
    contract: &EventContract,
    name: &'static str,
) -> Result<Option<CalendarDate>, ContractValidationError> {
    tag_value(tags, name)
        .map(|_| calendar_date_tag(tags, contract, name))
        .transpose()
}

fn canonical_calendar_u64_tag(
    tags: &[Vec<String>],
    contract: &EventContract,
    name: &'static str,
) -> Result<u64, ContractValidationError> {
    let value = tag_value(tags, name).ok_or(ContractValidationError::MissingTag {
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
    contract: &EventContract,
    name: &'static str,
) -> Result<Option<u64>, ContractValidationError> {
    tag_value(tags, name)
        .map(|_| canonical_calendar_u64_tag(tags, contract, name))
        .transpose()
}

fn calendar_tag_mismatch(
    contract: &EventContract,
    name: &'static str,
    expected: &'static str,
    actual: Option<String>,
) -> ContractValidationError {
    ContractValidationError::TagValueMismatch {
        contract_id: contract.id,
        name,
        expected: expected.to_owned(),
        actual,
    }
}

fn validate_custom_knowledge_contract_parts(
    content: &str,
    contract: &EventContract,
) -> Result<(), ContractValidationError> {
    let Some(expected_schema) = custom_knowledge_schema(contract.id) else {
        return Ok(());
    };
    let object = parse_content_object(content, contract.id)?;
    reject_forbidden_knowledge_fields(&object, contract.id)?;

    match object.get("schema").and_then(|value| value.as_str()) {
        Some(actual) if actual == expected_schema => {}
        Some(_) => {
            return Err(ContractValidationError::ContentFieldMismatch {
                contract_id: contract.id,
                field: "schema",
                expected: expected_schema.to_owned(),
            });
        }
        None => {
            return Err(ContractValidationError::MissingContentField {
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
        Some(_) => Err(ContractValidationError::ContentFieldMismatch {
            contract_id: contract.id,
            field: "schema_version",
            expected: "1".to_owned(),
        }),
        None => Err(ContractValidationError::MissingContentField {
            contract_id: contract.id,
            field: "schema_version",
        }),
    }
}

fn validate_discriminator_parts(
    content: &str,
    contract: &EventContract,
    admission_selected: bool,
) -> Result<(), ContractValidationError> {
    crate::require_invariant(
        admission_selected || !matches!(contract.discriminator, EventDiscriminator::AdmissionOnly),
        &|| ContractValidationError::AdmissionRequired {
            contract_id: contract.id,
        },
    )?;
    let (field, value) = match &contract.discriminator {
        EventDiscriminator::ContentJsonFieldEquals { field, value } => (*field, *value),
        EventDiscriminator::EnvelopeType(value) => ("type", *value),
        _ => return Ok(()),
    };
    let object = parse_content_object(content, contract.id)?;
    match object.get(field).and_then(|actual| actual.as_str()) {
        Some(actual) if actual == value => Ok(()),
        Some(_) => Err(ContractValidationError::ContentFieldMismatch {
            contract_id: contract.id,
            field,
            expected: value.to_owned(),
        }),
        None => Err(ContractValidationError::MissingContentField {
            contract_id: contract.id,
            field,
        }),
    }
}

fn parse_content_object(
    content: &str,
    contract_id: &'static str,
) -> Result<serde_json::Map<String, serde_json::Value>, ContractValidationError> {
    match serde_json::from_str::<serde_json::Value>(content) {
        Ok(serde_json::Value::Object(object)) => Ok(object),
        _ => Err(ContractValidationError::InvalidJsonContent { contract_id }),
    }
}

fn reject_forbidden_knowledge_fields(
    object: &serde_json::Map<String, serde_json::Value>,
    contract_id: &'static str,
) -> Result<(), ContractValidationError> {
    for field in [
        "review_status",
        "canon_status",
        "approved_for_canon",
        "rights_status",
        "trust_status",
        "trusted",
    ] {
        crate::require_invariant(!object.contains_key(field), &|| {
            ContractValidationError::ForbiddenContentField { contract_id, field }
        })?;
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
    discriminator: &EventDiscriminator,
    tags: &[Vec<String>],
    content: &str,
) -> bool {
    match discriminator {
        EventDiscriminator::KindOnly => true,
        EventDiscriminator::AdmissionOnly => false,
        EventDiscriminator::ClassifiedListingPartition(expected) => {
            classify_classified_listing_raw_tags_registry_v7(tags) == *expected
        }
        EventDiscriminator::DTagExact(expected) => tag_value(tags, "d") == Some(*expected),
        EventDiscriminator::DTagPrefix(prefix) => tag_value(tags, "d")
            .map(|value| value.starts_with(prefix))
            .unwrap_or(false),
        EventDiscriminator::DTagSuffix(suffix) => tag_value(tags, "d")
            .map(|value| value.ends_with(suffix))
            .unwrap_or(false),
        EventDiscriminator::TagEquals { name, value } => tag_value(tags, name) == Some(*value),
        EventDiscriminator::ContentJsonFieldEquals { field, value } => {
            content_json_string_field_equals(content, field, value)
        }
        EventDiscriminator::EnvelopeType(expected) => {
            content_json_string_field_equals(content, "type", expected)
        }
        EventDiscriminator::Composite(parts) => parts
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
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests;
