#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use crate::kinds::*;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
    Counterparty,
    EventPointer,
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
    RootEvent,
    ServiceInput,
    ServiceOutput,
    Status,
    Summary,
    Title,
    Url,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsTagValueType {
    AddressableCoordinate,
    DTag,
    EventId,
    Kind,
    PublicKey,
    RelayUrl,
    Text,
    UnixTimestamp,
    Url,
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
const TAG_A_OPTIONAL: RadrootsTagContract = tag(
    "a",
    RadrootsTagCardinality::OptionalOne,
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
        RadrootsEventContract {
            id: $id,
            kind: $kind,
            name: $name,
            payload_type: $payload_type,
            class: $class,
            stability: RadrootsEventStability::Stable,
            privacy: $standard_privacy,
            author_role: $author_role,
            content_schema: $content_schema,
            discriminator: $discriminator,
            tags: $tags,
            reducers: $reducers,
        }
    };
}

static LIST_SET_GENERIC_EVENT_CONTRACTS: &[RadrootsEventContract] = &[
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
        FARM_OPS_REDUCERS,
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
        FARM_OPS_REDUCERS,
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
        FARM_OPS_REDUCERS,
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
        FARM_OPS_REDUCERS,
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
        FARM_OPS_REDUCERS,
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
        FARM_OPS_REDUCERS,
    ),
];

static ALL_KIND_CONTRACTS: &[RadrootsKindContract] = &[
    kind_contract!(
        KIND_PROFILE,
        "KIND_PROFILE",
        "Profile Metadata",
        RadrootsEventClass::Regular,
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
        KIND_ORDER_FULFILLMENT_UPDATE,
        "KIND_ORDER_FULFILLMENT_UPDATE",
        "Order Fulfillment Update",
        RadrootsEventClass::Regular,
        RadrootsNostrStandard::Radroots,
        ["radroots.order.fulfillment_update.v1"]
    ),
    kind_contract!(
        KIND_ORDER_RECEIPT,
        "KIND_ORDER_RECEIPT",
        "Order Receipt",
        RadrootsEventClass::Regular,
        RadrootsNostrStandard::Radroots,
        ["radroots.order.receipt.v1"]
    ),
    kind_contract!(
        KIND_ORDER_PAYMENT_RECORD,
        "KIND_ORDER_PAYMENT_RECORD",
        "Order Payment Record",
        RadrootsEventClass::Regular,
        RadrootsNostrStandard::Radroots,
        ["radroots.order.payment_record.v1"]
    ),
    kind_contract!(
        KIND_ORDER_SETTLEMENT_DECISION,
        "KIND_ORDER_SETTLEMENT_DECISION",
        "Order Settlement Decision",
        RadrootsEventClass::Regular,
        RadrootsNostrStandard::Radroots,
        ["radroots.order.settlement_decision.v1"]
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
        RadrootsEventClass::Regular,
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
        RadrootsActorRole::Buyer,
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
        RadrootsActorRole::Seller,
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
        "radroots.order.fulfillment_update.v1",
        KIND_ORDER_FULFILLMENT_UPDATE,
        "Order Fulfillment Update",
        "RadrootsOrderFulfillmentUpdate",
        RadrootsEventClass::Regular,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Seller,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        CHAINED_ORDER_TAGS,
        ORDER_REDUCERS
    ),
    event_contract!(
        "radroots.order.receipt.v1",
        KIND_ORDER_RECEIPT,
        "Order Receipt",
        "RadrootsOrderReceipt",
        RadrootsEventClass::Regular,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Buyer,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        CHAINED_ORDER_TAGS,
        ORDER_REDUCERS
    ),
    event_contract!(
        "radroots.order.payment_record.v1",
        KIND_ORDER_PAYMENT_RECORD,
        "Order Payment Record",
        "RadrootsOrderPaymentRecord",
        RadrootsEventClass::Regular,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Buyer,
        RadrootsContentSchema::JsonObject,
        RadrootsEventDiscriminator::KindOnly,
        CHAINED_ORDER_TAGS,
        ORDER_REDUCERS
    ),
    event_contract!(
        "radroots.order.settlement_decision.v1",
        KIND_ORDER_SETTLEMENT_DECISION,
        "Order Settlement Decision",
        "RadrootsOrderSettlementDecision",
        RadrootsEventClass::Regular,
        RadrootsEventPrivacy::Public,
        RadrootsActorRole::Seller,
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

pub fn kind_contract(kind: u32) -> Option<&'static RadrootsKindContract> {
    ALL_KIND_CONTRACTS
        .iter()
        .find(|contract| contract.kind == kind)
}

pub fn event_contract(id: &str) -> Option<&'static RadrootsEventContract> {
    ALL_EVENT_CONTRACTS
        .iter()
        .find(|contract| contract.id == id)
        .or_else(|| {
            LIST_SET_GENERIC_EVENT_CONTRACTS
                .iter()
                .find(|contract| contract.id == id)
        })
}

pub fn event_contracts_for_kind(kind: u32) -> &'static [RadrootsEventContract] {
    if kind == KIND_LIST_SET_GENERIC {
        return LIST_SET_GENERIC_EVENT_CONTRACTS;
    }

    match ALL_EVENT_CONTRACTS
        .iter()
        .find(|contract| contract.kind == kind)
    {
        Some(contract) => core::slice::from_ref(contract),
        None => &[],
    }
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

fn identify_from_contracts(
    contracts: &'static [RadrootsEventContract],
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<&'static RadrootsEventContract, RadrootsContractMatchError> {
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

fn content_json_string_field_equals(content: &str, field: &str, value: &str) -> bool {
    let mut quoted = content.split('"');
    while let Some(token) = quoted.next() {
        if token == field {
            if let Some(separator) = quoted.next() {
                if separator.trim_start().starts_with(':') {
                    return quoted.next() == Some(value);
                }
            }
        }
    }
    false
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
        for contract in all_event_contracts()
            .iter()
            .chain(LIST_SET_GENERIC_EVENT_CONTRACTS.iter())
        {
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
    fn covers_public_kind_arrays() {
        for kind in COMMERCIAL_EVENT_KINDS
            .iter()
            .chain(PUBLIC_SOCIAL_KINDS.iter())
            .chain(PRIVATE_FARM_OPS_KINDS.iter())
            .chain(NIP29_GROUP_KINDS.iter())
        {
            assert!(kind_contract(*kind).is_some(), "missing kind {kind}");
        }
    }

    #[test]
    fn event_contract_lookup_supports_many_contracts_per_kind() {
        let contracts = event_contracts_for_kind(KIND_LIST_SET_GENERIC);
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
            identify_from_contracts(AMBIGUOUS_TEST_CONTRACTS, KIND_POST, &[], ""),
            Err(RadrootsContractMatchError::AmbiguousShape(KIND_POST))
        );
    }

    #[test]
    fn supports_content_field_discriminators_without_json_dependency() {
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
    fn relay_indexed_tags_are_single_letter() {
        for contract in all_event_contracts()
            .iter()
            .chain(LIST_SET_GENERIC_EVENT_CONTRACTS.iter())
        {
            for tag in contract.tags {
                if tag.relay_indexed {
                    assert_eq!(tag.name.len(), 1, "{}:{}", contract.id, tag.name);
                }
            }
        }
    }

    #[test]
    fn addressable_event_contracts_require_d_tags() {
        for contract in all_event_contracts()
            .iter()
            .chain(LIST_SET_GENERIC_EVENT_CONTRACTS.iter())
        {
            if contract.class == RadrootsEventClass::Addressable {
                assert!(
                    contract.tags.iter().any(|tag| tag.name == "d"
                        && tag.cardinality == RadrootsTagCardinality::RequiredOne),
                    "{}",
                    contract.id
                );
            }
        }
    }
}
