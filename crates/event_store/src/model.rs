use crate::RadrootsEventStoreError;
use radroots_event::contract::{RadrootsTagSemantic, RadrootsTagValueType};
use radroots_event::draft::RadrootsSignedEvent;
use radroots_event::event_head::RadrootsEventHeadDecision;
use radroots_event::ids::{
    RadrootsDTag, RadrootsEventId, RadrootsInventoryBinId, RadrootsPublicKey,
    RadrootsTradeCandidateId, RadrootsTradeId, RadrootsTradeMutationId,
};
use radroots_event::trade::RadrootsTradeMutationKindV1;
use radroots_event::wire::RadrootsNip01EventWire;
use radroots_event::{RadrootsEventEnvelope, RadrootsEventKind, RadrootsEventKindClass};
use radroots_event_codec::verification::{RadrootsSignatureVerifiedEvent, verify_nip01_event};
use radroots_transport::{
    RadrootsTransportKind, RadrootsTransportTargetFingerprint, RadrootsTransportTargetUri,
};

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsTransportObservationType {
    Fetch,
    Subscription,
    PublishAck,
    LocalImport,
    MeshHeard,
    MeshForwarded,
    GatewayStored,
    GatewayRepublished,
    DeliveryAck,
    Other,
}

impl RadrootsTransportObservationType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fetch => "fetch",
            Self::Subscription => "subscription",
            Self::PublishAck => "publish_ack",
            Self::LocalImport => "local_import",
            Self::MeshHeard => "mesh_heard",
            Self::MeshForwarded => "mesh_forwarded",
            Self::GatewayStored => "gateway_stored",
            Self::GatewayRepublished => "gateway_republished",
            Self::DeliveryAck => "delivery_ack",
            Self::Other => "other",
        }
    }

    pub fn parse(value: &str) -> Result<Self, RadrootsEventStoreError> {
        match value {
            "fetch" => Ok(Self::Fetch),
            "subscription" => Ok(Self::Subscription),
            "publish_ack" => Ok(Self::PublishAck),
            "local_import" => Ok(Self::LocalImport),
            "mesh_heard" => Ok(Self::MeshHeard),
            "mesh_forwarded" => Ok(Self::MeshForwarded),
            "gateway_stored" => Ok(Self::GatewayStored),
            "gateway_republished" => Ok(Self::GatewayRepublished),
            "delivery_ack" => Ok(Self::DeliveryAck),
            "other" => Ok(Self::Other),
            _ => Err(RadrootsEventStoreError::InvalidStoredEnum {
                field: "observation_type",
                value: value.to_owned(),
            }),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTransportObservation {
    pub transport_kind: RadrootsTransportKind,
    pub endpoint_uri: RadrootsTransportTargetUri,
    pub endpoint_fingerprint: RadrootsTransportTargetFingerprint,
    pub observation_type: RadrootsTransportObservationType,
    pub observed_at_ms: i64,
    pub redacted_message: Option<String>,
}

impl RadrootsTransportObservation {
    pub fn new(
        transport_kind: RadrootsTransportKind,
        endpoint_uri: impl AsRef<str>,
        observation_type: RadrootsTransportObservationType,
        observed_at_ms: i64,
    ) -> Result<Self, RadrootsEventStoreError> {
        let endpoint_uri = RadrootsTransportTargetUri::parse(endpoint_uri)?;
        let endpoint_fingerprint =
            RadrootsTransportTargetFingerprint::from_target(&transport_kind, &endpoint_uri, None);
        Ok(Self {
            transport_kind,
            endpoint_uri,
            endpoint_fingerprint,
            observation_type,
            observed_at_ms,
            redacted_message: None,
        })
    }

    pub fn with_redacted_message(mut self, message: impl Into<String>) -> Self {
        self.redacted_message = Some(message.into());
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsEventIngest {
    verified_event: RadrootsSignatureVerifiedEvent,
    raw_json: String,
    observed_at_ms: i64,
    transport_observation: Option<RadrootsTransportObservation>,
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
        let verified_event = verify_nip01_event(signed_event.envelope().clone())?;
        Ok(Self {
            verified_event,
            raw_json: signed_event.raw_json().to_owned(),
            observed_at_ms,
            transport_observation: None,
        })
    }

    pub fn from_raw_json(
        raw_json: impl Into<String>,
        observed_at_ms: i64,
    ) -> Result<Self, RadrootsEventStoreError> {
        let raw_json = raw_json.into();
        let wire = RadrootsNip01EventWire::parse_json(raw_json.as_str())?;
        let signed_event = RadrootsSignedEvent::from_wire_verified_id(wire, raw_json)?;
        Self::from_signed_event(signed_event, observed_at_ms)
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
pub struct RadrootsEventStoreStatusSummary {
    pub total_events: i64,
    pub valid_stream_events: i64,
    pub transport_observations: i64,
    pub last_event_seq: Option<i64>,
    pub last_event_updated_at_ms: Option<i64>,
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
pub struct RadrootsStoredValidEvent {
    raw_event: RadrootsStoredRawEvent,
}

impl RadrootsStoredValidEvent {
    pub(crate) fn try_from_raw(
        raw_event: RadrootsStoredRawEvent,
    ) -> Result<Self, RadrootsEventStoreError> {
        let expected_class =
            StoredEventClass::from_event_kind_class(RadrootsEventKind::new(raw_event.kind).class());
        if raw_event.admission_status != RadrootsEventAdmissionStatus::Admitted
            || raw_event.event_class != expected_class
            || raw_event.event_class == StoredEventClass::Ephemeral
            || !raw_event.valid_stream_eligible
        {
            return Err(
                RadrootsEventStoreError::StoredRawEventClassificationInconsistent {
                    event_id: raw_event.event_id,
                },
            );
        }
        Ok(Self { raw_event })
    }

    pub fn raw_event(&self) -> &RadrootsStoredRawEvent {
        &self.raw_event
    }

    pub fn into_raw_event(self) -> RadrootsStoredRawEvent {
        self.raw_event
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsStoredVisibleEvent {
    valid_event: RadrootsStoredValidEvent,
}

impl RadrootsStoredVisibleEvent {
    pub(crate) fn new(valid_event: RadrootsStoredValidEvent) -> Self {
        Self { valid_event }
    }

    pub fn valid_event(&self) -> &RadrootsStoredValidEvent {
        &self.valid_event
    }

    pub fn into_valid_event(self) -> RadrootsStoredValidEvent {
        self.valid_event
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsStoredEventTag {
    pub event_id: String,
    pub tag_index: u32,
    pub tag_name: String,
    pub tag_value: Option<String>,
    pub tag_json: String,
    pub contract_semantic: Option<String>,
    pub contract_value_type: Option<String>,
    pub relay_indexed: bool,
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
pub struct RadrootsStoredVisibleEventHead {
    raw_head: RadrootsStoredRawEventHead,
    event: RadrootsStoredVisibleEvent,
}

impl RadrootsStoredVisibleEventHead {
    pub(crate) fn new(
        raw_head: RadrootsStoredRawEventHead,
        event: RadrootsStoredVisibleEvent,
    ) -> Self {
        Self { raw_head, event }
    }

    pub fn raw_head(&self) -> &RadrootsStoredRawEventHead {
        &self.raw_head
    }

    pub fn event(&self) -> &RadrootsStoredVisibleEvent {
        &self.event
    }
}

#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsEventVisibility {
    Visible,
    NotAdmitted,
    NotCurrent { raw_head_event_id: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsProjectionCursor {
    pub projection_id: String,
    pub projection_version: u32,
    pub last_event_seq: i64,
    pub updated_at_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsStoredTradeMutation {
    pub mutation_id: RadrootsTradeMutationId,
    pub trade_id: RadrootsTradeId,
    pub root_mutation_id: Option<RadrootsTradeMutationId>,
    pub contract_id: String,
    pub mutation_kind: RadrootsTradeMutationKindV1,
    pub schema_version: u16,
    pub candidate_id: Option<RadrootsTradeCandidateId>,
    pub proposal_mutation_id: Option<RadrootsTradeMutationId>,
    pub target_claim_mutation_id: Option<RadrootsTradeMutationId>,
    pub author_pubkey: RadrootsPublicKey,
    pub counterparty_pubkey: RadrootsPublicKey,
    pub buyer_pubkey: RadrootsPublicKey,
    pub seller_pubkey: RadrootsPublicKey,
    pub farm_id: RadrootsDTag,
    pub authored_at_unix_s: u64,
    pub canonical_payload_bytes: Vec<u8>,
    pub payload_sha256: String,
    pub first_event_seq: i64,
    pub first_transport_event_id: RadrootsEventId,
    pub inserted_at_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsStoredTradeMutationParent {
    pub mutation_id: RadrootsTradeMutationId,
    pub parent_mutation_id: RadrootsTradeMutationId,
    pub parent_index: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsStoredTradeMissingParent {
    pub trade_id: RadrootsTradeId,
    pub mutation_id: RadrootsTradeMutationId,
    pub missing_parent_mutation_id: RadrootsTradeMutationId,
    pub first_transport_event_id: RadrootsEventId,
    pub first_seen_at_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsStoredTradeTransportEnvelope {
    pub transport_event_id: RadrootsEventId,
    pub mutation_id: RadrootsTradeMutationId,
    pub trade_id: RadrootsTradeId,
    pub transport_kind: String,
    pub pubkey: RadrootsPublicKey,
    pub created_at: u64,
    pub event_seq: i64,
    pub payload_sha256: String,
    pub observed_at_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsStoredSellerReservation {
    pub reservation_id: RadrootsDTag,
    pub trade_id: RadrootsTradeId,
    pub candidate_id: RadrootsTradeCandidateId,
    pub claim_mutation_id: RadrootsTradeMutationId,
    pub inventory_authority_pubkey: RadrootsPublicKey,
    pub inventory_epoch: u64,
    pub assertion_commitment: String,
    pub reservation_expires_at_unix_s: u64,
    pub reservation_json: String,
    pub inserted_at_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsStoredSellerReservationLine {
    pub reservation_id: RadrootsDTag,
    pub line_id: RadrootsDTag,
    pub bin_id: RadrootsInventoryBinId,
    pub quantity_mantissa: String,
    pub quantity_scale: u8,
    pub unit_code: String,
    pub line_index: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeProjectionCheckpoint {
    pub trade_id: RadrootsTradeId,
    pub reducer_contract_id: String,
    pub reducer_version: u16,
    pub projection_digest: String,
    pub root_mutation_id: Option<RadrootsTradeMutationId>,
    pub negotiation_state: String,
    pub agreement_state: String,
    pub evidence_state: String,
    pub conflict_state: String,
    pub private_terms_state: String,
    pub attestation_state: String,
    pub fulfillment_state: String,
    pub payment_state: String,
    pub projection_json: String,
    pub last_mutation_id: Option<RadrootsTradeMutationId>,
    pub last_transport_event_seq: Option<i64>,
    pub updated_at_ms: i64,
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

#[cfg(test)]
mod tests {
    use super::*;
    use radroots_event::event_head::{
        RadrootsCurrentEventHead, RadrootsEventHeadCoordinate, RadrootsEventHeadDecision,
    };
    use radroots_event::ids::{RadrootsEventId, RadrootsPublicKey};

    #[test]
    fn admission_status_event_class_and_observation_values_roundtrip() {
        for (status, expected) in [
            (RadrootsEventAdmissionStatus::Admitted, "admitted"),
            (RadrootsEventAdmissionStatus::Unsupported, "unsupported"),
            (RadrootsEventAdmissionStatus::Invalid, "invalid"),
        ] {
            assert_eq!(status.as_str(), expected);
            assert_eq!(
                RadrootsEventAdmissionStatus::parse(expected).expect("status"),
                status
            );
        }
        for legacy in [
            "supported",
            "unsupported_kind",
            "unsupported_shape",
            "ambiguous_shape",
        ] {
            assert!(RadrootsEventAdmissionStatus::parse(legacy).is_err());
        }
        assert!(RadrootsEventAdmissionStatus::parse("bad").is_err());

        for class in [
            StoredEventClass::Regular,
            StoredEventClass::Replaceable,
            StoredEventClass::Addressable,
            StoredEventClass::Ephemeral,
        ] {
            assert_eq!(
                StoredEventClass::parse(class.as_str()).expect("class"),
                class
            );
        }
        assert_eq!(
            StoredEventClass::from_event_kind_class(RadrootsEventKindClass::Regular),
            StoredEventClass::Regular
        );
        assert_eq!(
            StoredEventClass::from_event_kind_class(RadrootsEventKindClass::Replaceable),
            StoredEventClass::Replaceable
        );
        assert_eq!(
            StoredEventClass::from_event_kind_class(RadrootsEventKindClass::Addressable),
            StoredEventClass::Addressable
        );
        assert_eq!(
            StoredEventClass::from_event_kind_class(RadrootsEventKindClass::Ephemeral),
            StoredEventClass::Ephemeral
        );
        assert!(StoredEventClass::parse("bad").is_err());

        let inserted = RadrootsEventPersistence::Inserted { seq: 7 };
        assert_eq!(inserted.sequence(), Some(7));
        assert!(inserted.is_inserted());
        assert!(!inserted.is_duplicate());
        let duplicate = RadrootsEventPersistence::Duplicate { seq: 7 };
        assert_eq!(duplicate.sequence(), Some(7));
        assert!(!duplicate.is_inserted());
        assert!(duplicate.is_duplicate());
        let not_persisted = RadrootsEventPersistence::NotPersisted;
        assert_eq!(not_persisted.sequence(), None);
        assert!(!not_persisted.is_inserted());
        assert!(!not_persisted.is_duplicate());

        for observation_type in [
            RadrootsTransportObservationType::Fetch,
            RadrootsTransportObservationType::Subscription,
            RadrootsTransportObservationType::PublishAck,
            RadrootsTransportObservationType::LocalImport,
            RadrootsTransportObservationType::MeshHeard,
            RadrootsTransportObservationType::MeshForwarded,
            RadrootsTransportObservationType::GatewayStored,
            RadrootsTransportObservationType::GatewayRepublished,
            RadrootsTransportObservationType::DeliveryAck,
            RadrootsTransportObservationType::Other,
        ] {
            assert!(!observation_type.as_str().is_empty());
            assert_eq!(
                RadrootsTransportObservationType::parse(observation_type.as_str())
                    .expect("observation type"),
                observation_type
            );
        }
        assert!(RadrootsTransportObservationType::parse("bad").is_err());
        let observation = RadrootsTransportObservation::new(
            RadrootsTransportKind::Nostr,
            "wss://relay.example.test",
            RadrootsTransportObservationType::Fetch,
            1,
        )
        .expect("observation")
        .with_redacted_message("seen");
        assert_eq!(observation.redacted_message.as_deref(), Some("seen"));
        assert_eq!(
            observation.endpoint_uri.as_str(),
            "wss://relay.example.test"
        );
        assert!(
            RadrootsTransportObservation::new(
                RadrootsTransportKind::Nostr,
                "not a URI",
                RadrootsTransportObservationType::Fetch,
                1,
            )
            .is_err()
        );
    }

    #[test]
    fn head_decisions_and_tag_metadata_names_cover_all_variants() {
        let coordinate = RadrootsEventHeadCoordinate::Addressable {
            kind: 30_023,
            pubkey: RadrootsPublicKey::parse(
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            )
            .expect("pubkey"),
            d_tag: "opaque d value".to_owned(),
        };
        let current = RadrootsCurrentEventHead {
            coordinate,
            event_id: RadrootsEventId::parse(
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            )
            .expect("event id"),
            created_at: 10,
        };

        assert_eq!(
            RadrootsRawHeadDecision::from_protocol(&RadrootsEventHeadDecision::Applied(current)),
            RadrootsRawHeadDecision::Applied
        );
        assert_eq!(
            RadrootsRawHeadDecision::from_protocol(&RadrootsEventHeadDecision::SkippedDuplicate),
            RadrootsRawHeadDecision::SkippedDuplicate
        );
        assert_eq!(
            RadrootsRawHeadDecision::from_protocol(&RadrootsEventHeadDecision::SkippedOlder),
            RadrootsRawHeadDecision::SkippedOlder
        );
        assert_eq!(
            RadrootsRawHeadDecision::from_protocol(
                &RadrootsEventHeadDecision::SkippedSameTimestampHigherEventId
            ),
            RadrootsRawHeadDecision::SkippedSameTimestampHigherEventId
        );
        assert_eq!(
            RadrootsRawHeadDecision::from_protocol(&RadrootsEventHeadDecision::CoordinateMismatch),
            RadrootsRawHeadDecision::MalformedCoordinate
        );

        for (semantic, expected) in [
            (
                RadrootsTagSemantic::AddressableCoordinate,
                "addressable_coordinate",
            ),
            (
                RadrootsTagSemantic::CalendarEventAuthor,
                "calendar_event_author",
            ),
            (
                RadrootsTagSemantic::CalendarEventReference,
                "calendar_event_reference",
            ),
            (
                RadrootsTagSemantic::CalendarEventRevision,
                "calendar_event_revision",
            ),
            (
                RadrootsTagSemantic::CalendarInclusionRequest,
                "calendar_inclusion_request",
            ),
            (RadrootsTagSemantic::CalendarEnd, "calendar_end"),
            (RadrootsTagSemantic::CalendarStart, "calendar_start"),
            (RadrootsTagSemantic::Category, "category"),
            (RadrootsTagSemantic::Citation, "citation"),
            (RadrootsTagSemantic::Contract, "contract"),
            (RadrootsTagSemantic::Counterparty, "counterparty"),
            (RadrootsTagSemantic::Evidence, "evidence"),
            (RadrootsTagSemantic::EventPointer, "event_pointer"),
            (RadrootsTagSemantic::FreeBusy, "free_busy"),
            (RadrootsTagSemantic::Geohash, "geohash"),
            (RadrootsTagSemantic::GroupId, "group_id"),
            (RadrootsTagSemantic::Identifier, "identifier"),
            (RadrootsTagSemantic::Image, "image"),
            (RadrootsTagSemantic::Kind, "kind"),
            (
                RadrootsTagSemantic::ClassifiedListingAddress,
                "listing_address",
            ),
            (
                RadrootsTagSemantic::OperationalListingSnapshot,
                "listing_snapshot",
            ),
            (RadrootsTagSemantic::ListDescription, "list_description"),
            (RadrootsTagSemantic::Location, "location"),
            (RadrootsTagSemantic::Nip01Coordinate, "nip01_coordinate"),
            (RadrootsTagSemantic::Participant, "participant"),
            (RadrootsTagSemantic::PreviousEvent, "previous_event"),
            (RadrootsTagSemantic::Price, "price"),
            (RadrootsTagSemantic::PublishedAt, "published_at"),
            (RadrootsTagSemantic::Relay, "relay"),
            (RadrootsTagSemantic::Reference, "reference"),
            (RadrootsTagSemantic::ReviewTarget, "review_target"),
            (RadrootsTagSemantic::RootEvent, "root_event"),
            (RadrootsTagSemantic::ServiceInput, "service_input"),
            (RadrootsTagSemantic::ServiceOutput, "service_output"),
            (RadrootsTagSemantic::Source, "source"),
            (RadrootsTagSemantic::Status, "status"),
            (RadrootsTagSemantic::Summary, "summary"),
            (RadrootsTagSemantic::Title, "title"),
            (RadrootsTagSemantic::Topic, "topic"),
            (RadrootsTagSemantic::TimeZone, "time_zone"),
            (RadrootsTagSemantic::Url, "url"),
            (RadrootsTagSemantic::UtcDayCoverage, "utc_day_coverage"),
        ] {
            assert_eq!(tag_semantic_name(semantic), expected);
        }

        for (value_type, expected) in [
            (
                RadrootsTagValueType::AddressableCoordinate,
                "addressable_coordinate",
            ),
            (RadrootsTagValueType::CalendarDate, "calendar_date"),
            (
                RadrootsTagValueType::CalendarEventCoordinate,
                "calendar_event_coordinate",
            ),
            (RadrootsTagValueType::CalendarFreeBusy, "calendar_free_busy"),
            (
                RadrootsTagValueType::CalendarRsvpStatus,
                "calendar_rsvp_status",
            ),
            (RadrootsTagValueType::CalendarUid, "calendar_uid"),
            (RadrootsTagValueType::ContractId, "contract_id"),
            (RadrootsTagValueType::DTag, "d_tag"),
            (RadrootsTagValueType::EventId, "event_id"),
            (RadrootsTagValueType::EventPointer, "event_pointer"),
            (RadrootsTagValueType::Geohash, "geohash"),
            (RadrootsTagValueType::IanaTimeZoneId, "iana_time_zone_id"),
            (RadrootsTagValueType::Kind, "kind"),
            (RadrootsTagValueType::Nip01Coordinate, "nip01_coordinate"),
            (RadrootsTagValueType::PublicKey, "public_key"),
            (RadrootsTagValueType::RelayUrl, "relay_url"),
            (RadrootsTagValueType::Sha256, "sha256"),
            (RadrootsTagValueType::Text, "text"),
            (RadrootsTagValueType::UnixTimestamp, "unix_timestamp"),
            (RadrootsTagValueType::Uri, "uri"),
            (RadrootsTagValueType::Url, "url"),
            (RadrootsTagValueType::UtcDayIndex, "utc_day_index"),
            (RadrootsTagValueType::Uuid, "uuid"),
        ] {
            assert_eq!(tag_value_type_name(value_type), expected);
        }
    }
}
