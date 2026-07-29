use super::RadrootsTransportObservation;
use crate::RadrootsEventStoreError;
use radroots_event::contract::registry_v7::{TagSemantic, TagValueType};
use radroots_event::draft::SignedEvent;
use radroots_event::envelope::event_head::v1::EventHeadDecision;
use radroots_event::envelope::{EventEnvelope, EventKindClass};
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

    pub fn from_event_kind_class(value: EventKindClass) -> Self {
        match value {
            EventKindClass::Regular => Self::Regular,
            EventKindClass::Replaceable => Self::Replaceable,
            EventKindClass::Ephemeral => Self::Ephemeral,
            EventKindClass::Addressable => Self::Addressable,
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
    pub(crate) fn new(signed_event: SignedEvent, observed_at_ms: i64) -> Self {
        Self::from_signed_event(signed_event, observed_at_ms)
            .expect("test event must have a valid NIP-01 signature")
    }

    pub fn from_signed_event(
        signed_event: SignedEvent,
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

    pub fn event(&self) -> &EventEnvelope {
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
    pub fn from_protocol(value: &EventHeadDecision) -> Self {
        match value {
            EventHeadDecision::Applied(_) => Self::Applied,
            EventHeadDecision::SkippedDuplicate => Self::SkippedDuplicate,
            EventHeadDecision::SkippedOlder => Self::SkippedOlder,
            EventHeadDecision::SkippedSameTimestampHigherEventId => {
                Self::SkippedSameTimestampHigherEventId
            }
            EventHeadDecision::CoordinateMismatch => Self::MalformedCoordinate,
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

pub fn tag_semantic_name(value: TagSemantic) -> &'static str {
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

pub fn tag_value_type_name(value: TagValueType) -> &'static str {
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
