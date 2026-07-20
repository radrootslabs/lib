use super::RadrootsTransportObservation;
use crate::RadrootsEventStoreError;
use radroots_event::contract::registry_v7::{RadrootsTagSemantic, RadrootsTagValueType};
use radroots_event::draft::RadrootsSignedEvent;
use radroots_event::envelope::{RadrootsEventEnvelope, RadrootsEventKindClass};
use radroots_event::event_head::v1::RadrootsEventHeadDecision;
use radroots_event_codec::verification::v1::RadrootsSignatureVerifiedEvent;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsEventAdmissionStatus {
    Admitted,
    Unsupported,
    Invalid,
}

impl RadrootsEventAdmissionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Admitted => "admitted",
            Self::Unsupported => "unsupported",
            Self::Invalid => "invalid",
        }
    }

    pub fn parse(value: &str) -> Result<Self, RadrootsEventStoreError> {
        match value {
            "admitted" => Ok(Self::Admitted),
            "unsupported" => Ok(Self::Unsupported),
            "invalid" => Ok(Self::Invalid),
            _ => Err(RadrootsEventStoreError::InvalidStoredEnum {
                field: "contract_status",
                value: value.to_owned(),
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StoredEventClass {
    Regular,
    Replaceable,
    Addressable,
    Ephemeral,
}

impl StoredEventClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Regular => "regular",
            Self::Replaceable => "replaceable",
            Self::Addressable => "addressable",
            Self::Ephemeral => "ephemeral",
        }
    }

    pub fn from_event_kind_class(value: RadrootsEventKindClass) -> Self {
        match value {
            RadrootsEventKindClass::Regular => Self::Regular,
            RadrootsEventKindClass::Replaceable => Self::Replaceable,
            RadrootsEventKindClass::Ephemeral => Self::Ephemeral,
            RadrootsEventKindClass::Addressable => Self::Addressable,
        }
    }

    pub fn parse(value: &str) -> Result<Self, RadrootsEventStoreError> {
        match value {
            "regular" => Ok(Self::Regular),
            "replaceable" => Ok(Self::Replaceable),
            "addressable" => Ok(Self::Addressable),
            "ephemeral" => Ok(Self::Ephemeral),
            _ => Err(RadrootsEventStoreError::InvalidStoredEnum {
                field: "event_class",
                value: value.to_owned(),
            }),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsEventPersistence {
    Inserted { seq: i64 },
    Duplicate { seq: i64 },
    NotPersisted,
}

impl RadrootsEventPersistence {
    pub const fn sequence(&self) -> Option<i64> {
        match self {
            Self::Inserted { seq } | Self::Duplicate { seq } => Some(*seq),
            Self::NotPersisted => None,
        }
    }

    pub const fn is_inserted(&self) -> bool {
        matches!(self, Self::Inserted { .. })
    }

    pub const fn is_duplicate(&self) -> bool {
        matches!(self, Self::Duplicate { .. })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsEventIngestReceipt {
    pub persistence: RadrootsEventPersistence,
    pub event_id: String,
    pub admission_status: RadrootsEventAdmissionStatus,
    pub admission_code: Option<String>,
    pub contract_id: Option<String>,
    pub valid_stream_eligible: bool,
    pub raw_head_decision: RadrootsRawHeadDecision,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsStoredRawEvent {
    pub seq: i64,
    pub event_id: String,
    pub pubkey: String,
    pub created_at: u64,
    pub kind: u32,
    pub tags_json: String,
    pub content: String,
    pub sig: String,
    pub raw_json: String,
    pub admission_status: RadrootsEventAdmissionStatus,
    pub contract_id: Option<String>,
    pub event_class: StoredEventClass,
    pub valid_stream_eligible: bool,
    pub inserted_at_ms: i64,
    pub updated_at_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsStoredRawEventHead {
    pub coordinate_type: StoredEventClass,
    pub kind: u32,
    pub pubkey: String,
    pub d_tag: Option<String>,
    pub event_id: String,
    pub created_at: u64,
    pub updated_at_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsEventIngest {
    pub(super) verified_event: RadrootsSignatureVerifiedEvent,
    pub(super) raw_json: String,
    pub(super) observed_at_ms: i64,
    pub(super) transport_observation: Option<RadrootsTransportObservation>,
}

impl RadrootsEventIngest {
    #[cfg(test)]
    pub(crate) fn new(signed_event: RadrootsSignedEvent, observed_at_ms: i64) -> Self {
        Self::from_signed_event(signed_event, observed_at_ms)
            .expect("test event must have a valid NIP-01 signature")
    }

    pub fn from_signed_event(
        signed_event: RadrootsSignedEvent,
        observed_at_ms: i64,
    ) -> Result<Self, RadrootsEventStoreError> {
        Self::from_signed_event_reconciliation_v1(signed_event, observed_at_ms)
    }

    pub fn from_raw_json(
        raw_json: impl Into<String>,
        observed_at_ms: i64,
    ) -> Result<Self, RadrootsEventStoreError> {
        Self::from_raw_json_reconciliation_v1(raw_json, observed_at_ms)
    }

    pub fn with_observation(mut self, observation: RadrootsTransportObservation) -> Self {
        self.transport_observation = Some(observation);
        self
    }

    pub fn event(&self) -> &RadrootsEventEnvelope {
        self.verified_event.event()
    }

    pub fn verified_event(&self) -> &RadrootsSignatureVerifiedEvent {
        &self.verified_event
    }

    pub fn raw_json(&self) -> &str {
        self.raw_json.as_str()
    }

    pub fn observed_at_ms(&self) -> i64 {
        self.observed_at_ms
    }

    pub fn transport_observation(&self) -> Option<&RadrootsTransportObservation> {
        self.transport_observation.as_ref()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsRawHeadDecision {
    Applied,
    NotHeadSelected,
    NotPersisted,
    SkippedDuplicate,
    SkippedOlder,
    SkippedSameTimestampHigherEventId,
    MalformedCoordinate,
}

impl RadrootsRawHeadDecision {
    pub fn from_protocol(value: &RadrootsEventHeadDecision) -> Self {
        match value {
            RadrootsEventHeadDecision::Applied(_) => Self::Applied,
            RadrootsEventHeadDecision::SkippedDuplicate => Self::SkippedDuplicate,
            RadrootsEventHeadDecision::SkippedOlder => Self::SkippedOlder,
            RadrootsEventHeadDecision::SkippedSameTimestampHigherEventId => {
                Self::SkippedSameTimestampHigherEventId
            }
            RadrootsEventHeadDecision::CoordinateMismatch => Self::MalformedCoordinate,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RadrootsEventStoreSourceGeneration([u8; 32]);

impl RadrootsEventStoreSourceGeneration {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

pub fn tag_semantic_name(value: RadrootsTagSemantic) -> &'static str {
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

pub fn tag_value_type_name(value: RadrootsTagValueType) -> &'static str {
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
