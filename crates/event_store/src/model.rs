use crate::RadrootsEventStoreError;
use radroots_event::RadrootsEventEnvelope;
use radroots_event::contract::{
    RadrootsContractMatchError, RadrootsEventClass, RadrootsTagSemantic, RadrootsTagValueType,
};
use radroots_event::draft::RadrootsSignedEvent;
use radroots_event::event_head::RadrootsEventHeadDecision;
use radroots_event::wire::RadrootsNip01EventWire;
use radroots_transport::{
    RadrootsTransportKind, RadrootsTransportTargetFingerprint, RadrootsTransportTargetUri,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsEventVerificationStatus {
    NotChecked,
    IdVerified,
    Verified,
    IdMismatch,
    SignatureInvalid,
    MalformedEnvelope,
}

impl RadrootsEventVerificationStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotChecked => "not_checked",
            Self::IdVerified => "id_verified",
            Self::Verified => "verified",
            Self::IdMismatch => "id_mismatch",
            Self::SignatureInvalid => "signature_invalid",
            Self::MalformedEnvelope => "malformed_envelope",
        }
    }

    pub fn parse(value: &str) -> Result<Self, RadrootsEventStoreError> {
        match value {
            "not_checked" => Ok(Self::NotChecked),
            "id_verified" => Ok(Self::IdVerified),
            "verified" => Ok(Self::Verified),
            "id_mismatch" => Ok(Self::IdMismatch),
            "signature_invalid" => Ok(Self::SignatureInvalid),
            "malformed_envelope" => Ok(Self::MalformedEnvelope),
            _ => Err(RadrootsEventStoreError::InvalidStoredEnum {
                field: "verification_status",
                value: value.to_owned(),
            }),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsEventContractStatus {
    Supported,
    UnsupportedKind(u32),
    UnsupportedShape(u32),
    AmbiguousShape(u32),
}

impl RadrootsEventContractStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Supported => "supported",
            Self::UnsupportedKind(_) => "unsupported_kind",
            Self::UnsupportedShape(_) => "unsupported_shape",
            Self::AmbiguousShape(_) => "ambiguous_shape",
        }
    }

    pub fn from_match_error(error: RadrootsContractMatchError) -> Self {
        match error {
            RadrootsContractMatchError::UnsupportedKind(kind) => Self::UnsupportedKind(kind),
            RadrootsContractMatchError::UnsupportedShape(kind) => Self::UnsupportedShape(kind),
            RadrootsContractMatchError::AmbiguousShape(kind) => Self::AmbiguousShape(kind),
        }
    }

    pub fn parse(value: &str, kind: u32) -> Result<Self, RadrootsEventStoreError> {
        match value {
            "supported" => Ok(Self::Supported),
            "unsupported_kind" => Ok(Self::UnsupportedKind(kind)),
            "unsupported_shape" => Ok(Self::UnsupportedShape(kind)),
            "ambiguous_shape" => Ok(Self::AmbiguousShape(kind)),
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

    pub fn from_event_class(value: RadrootsEventClass) -> Self {
        match value {
            RadrootsEventClass::Regular => Self::Regular,
            RadrootsEventClass::Replaceable => Self::Replaceable,
            RadrootsEventClass::Addressable => Self::Addressable,
            RadrootsEventClass::Ephemeral => Self::Ephemeral,
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
    pub signed_event: RadrootsSignedEvent,
    pub observed_at_ms: i64,
    pub transport_observation: Option<RadrootsTransportObservation>,
}

impl RadrootsEventIngest {
    pub fn new(signed_event: RadrootsSignedEvent, observed_at_ms: i64) -> Self {
        Self {
            signed_event,
            observed_at_ms,
            transport_observation: None,
        }
    }

    pub fn from_raw_json(
        raw_json: impl Into<String>,
        observed_at_ms: i64,
    ) -> Result<Self, RadrootsEventStoreError> {
        let raw_json = raw_json.into();
        let wire = RadrootsNip01EventWire::parse_json(raw_json.as_str())?;
        let signed_event = RadrootsSignedEvent::from_wire_verified_id(wire, raw_json)?;
        Ok(Self::new(signed_event, observed_at_ms))
    }

    pub fn with_observation(mut self, observation: RadrootsTransportObservation) -> Self {
        self.transport_observation = Some(observation);
        self
    }

    pub fn event(&self) -> &RadrootsEventEnvelope {
        self.signed_event.envelope()
    }

    pub fn raw_json(&self) -> &str {
        self.signed_event.raw_json()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsEventHeadStoreDecision {
    Applied,
    NotHeadSelected,
    NotPersisted,
    NotProjectionEligible,
    SkippedDuplicate,
    SkippedOlder,
    SkippedSameTimestampHigherEventId,
    Malformed,
    Unsupported,
}

impl RadrootsEventHeadStoreDecision {
    pub fn from_protocol(value: &RadrootsEventHeadDecision) -> Self {
        match value {
            RadrootsEventHeadDecision::Applied(_) => Self::Applied,
            RadrootsEventHeadDecision::SkippedDuplicate => Self::SkippedDuplicate,
            RadrootsEventHeadDecision::SkippedOlder => Self::SkippedOlder,
            RadrootsEventHeadDecision::SkippedSameTimestampHigherEventId => {
                Self::SkippedSameTimestampHigherEventId
            }
            RadrootsEventHeadDecision::CoordinateMismatch => Self::Malformed,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsEventIngestReceipt {
    pub seq: i64,
    pub event_id: String,
    pub inserted: bool,
    pub verification_status: RadrootsEventVerificationStatus,
    pub contract_status: RadrootsEventContractStatus,
    pub contract_id: Option<String>,
    pub projection_eligible: bool,
    pub head_decision: RadrootsEventHeadStoreDecision,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsEventStoreStatusSummary {
    pub total_events: i64,
    pub projection_eligible_events: i64,
    pub transport_observations: i64,
    pub last_event_seq: Option<i64>,
    pub last_event_updated_at_ms: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsStoredEvent {
    pub seq: i64,
    pub event_id: String,
    pub pubkey: String,
    pub created_at: u64,
    pub kind: u32,
    pub tags_json: String,
    pub content: String,
    pub sig: String,
    pub raw_json: String,
    pub verification_status: RadrootsEventVerificationStatus,
    pub contract_status: RadrootsEventContractStatus,
    pub contract_id: Option<String>,
    pub event_class: Option<StoredEventClass>,
    pub projection_eligible: bool,
    pub inserted_at_ms: i64,
    pub updated_at_ms: i64,
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
pub struct RadrootsStoredEventHead {
    pub coordinate_type: StoredEventClass,
    pub kind: u32,
    pub pubkey: String,
    pub d_tag: Option<String>,
    pub event_id: String,
    pub created_at: u64,
    pub updated_at_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsProjectionCursor {
    pub projection_id: String,
    pub projection_version: u32,
    pub last_event_seq: i64,
    pub updated_at_ms: i64,
}

pub fn tag_semantic_name(value: RadrootsTagSemantic) -> &'static str {
    match value {
        RadrootsTagSemantic::AddressableCoordinate => "addressable_coordinate",
        RadrootsTagSemantic::Category => "category",
        RadrootsTagSemantic::Citation => "citation",
        RadrootsTagSemantic::Contract => "contract",
        RadrootsTagSemantic::Counterparty => "counterparty",
        RadrootsTagSemantic::Evidence => "evidence",
        RadrootsTagSemantic::EventPointer => "event_pointer",
        RadrootsTagSemantic::Geohash => "geohash",
        RadrootsTagSemantic::GroupId => "group_id",
        RadrootsTagSemantic::Identifier => "identifier",
        RadrootsTagSemantic::Image => "image",
        RadrootsTagSemantic::Kind => "kind",
        RadrootsTagSemantic::ListingAddress => "listing_address",
        RadrootsTagSemantic::ListingSnapshot => "listing_snapshot",
        RadrootsTagSemantic::Location => "location",
        RadrootsTagSemantic::PreviousEvent => "previous_event",
        RadrootsTagSemantic::Price => "price",
        RadrootsTagSemantic::PublishedAt => "published_at",
        RadrootsTagSemantic::Relay => "relay",
        RadrootsTagSemantic::ReviewTarget => "review_target",
        RadrootsTagSemantic::RootEvent => "root_event",
        RadrootsTagSemantic::ServiceInput => "service_input",
        RadrootsTagSemantic::ServiceOutput => "service_output",
        RadrootsTagSemantic::Source => "source",
        RadrootsTagSemantic::Status => "status",
        RadrootsTagSemantic::Summary => "summary",
        RadrootsTagSemantic::Title => "title",
        RadrootsTagSemantic::Topic => "topic",
        RadrootsTagSemantic::Url => "url",
    }
}

pub fn tag_value_type_name(value: RadrootsTagValueType) -> &'static str {
    match value {
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

#[cfg(test)]
mod tests {
    use super::*;
    use radroots_event::event_head::{
        RadrootsCurrentEventHead, RadrootsEventHeadCoordinate, RadrootsEventHeadDecision,
    };
    use radroots_event::ids::{RadrootsDTag, RadrootsEventId, RadrootsPublicKey};

    #[test]
    fn contract_status_event_class_and_observation_values_roundtrip() {
        assert_eq!(
            RadrootsEventContractStatus::from_match_error(
                RadrootsContractMatchError::UnsupportedKind(7)
            ),
            RadrootsEventContractStatus::UnsupportedKind(7)
        );
        assert_eq!(
            RadrootsEventContractStatus::from_match_error(
                RadrootsContractMatchError::UnsupportedShape(8)
            ),
            RadrootsEventContractStatus::UnsupportedShape(8)
        );
        assert_eq!(
            RadrootsEventContractStatus::from_match_error(
                RadrootsContractMatchError::AmbiguousShape(9)
            ),
            RadrootsEventContractStatus::AmbiguousShape(9)
        );

        for (status, expected) in [
            (RadrootsEventContractStatus::Supported, "supported"),
            (
                RadrootsEventContractStatus::UnsupportedKind(1),
                "unsupported_kind",
            ),
            (
                RadrootsEventContractStatus::UnsupportedShape(2),
                "unsupported_shape",
            ),
            (
                RadrootsEventContractStatus::AmbiguousShape(3),
                "ambiguous_shape",
            ),
        ] {
            assert_eq!(status.as_str(), expected);
            assert_eq!(
                RadrootsEventContractStatus::parse(expected, 99).expect("status"),
                match status {
                    RadrootsEventContractStatus::Supported =>
                        RadrootsEventContractStatus::Supported,
                    RadrootsEventContractStatus::UnsupportedKind(_) => {
                        RadrootsEventContractStatus::UnsupportedKind(99)
                    }
                    RadrootsEventContractStatus::UnsupportedShape(_) => {
                        RadrootsEventContractStatus::UnsupportedShape(99)
                    }
                    RadrootsEventContractStatus::AmbiguousShape(_) => {
                        RadrootsEventContractStatus::AmbiguousShape(99)
                    }
                }
            );
        }
        assert!(RadrootsEventContractStatus::parse("bad", 1).is_err());

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
            StoredEventClass::from_event_class(RadrootsEventClass::Regular),
            StoredEventClass::Regular
        );
        assert_eq!(
            StoredEventClass::from_event_class(RadrootsEventClass::Replaceable),
            StoredEventClass::Replaceable
        );
        assert_eq!(
            StoredEventClass::from_event_class(RadrootsEventClass::Addressable),
            StoredEventClass::Addressable
        );
        assert_eq!(
            StoredEventClass::from_event_class(RadrootsEventClass::Ephemeral),
            StoredEventClass::Ephemeral
        );
        assert!(StoredEventClass::parse("bad").is_err());

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
    }

    #[test]
    fn head_decisions_and_tag_metadata_names_cover_all_variants() {
        let coordinate = RadrootsEventHeadCoordinate::Addressable {
            kind: 30_023,
            pubkey: RadrootsPublicKey::parse(
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            )
            .expect("pubkey"),
            d_tag: RadrootsDTag::parse("AAAAAAAAAAAAAAAAAAAAAA").expect("d tag"),
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
            RadrootsEventHeadStoreDecision::from_protocol(&RadrootsEventHeadDecision::Applied(
                current
            )),
            RadrootsEventHeadStoreDecision::Applied
        );
        assert_eq!(
            RadrootsEventHeadStoreDecision::from_protocol(
                &RadrootsEventHeadDecision::SkippedDuplicate
            ),
            RadrootsEventHeadStoreDecision::SkippedDuplicate
        );
        assert_eq!(
            RadrootsEventHeadStoreDecision::from_protocol(&RadrootsEventHeadDecision::SkippedOlder),
            RadrootsEventHeadStoreDecision::SkippedOlder
        );
        assert_eq!(
            RadrootsEventHeadStoreDecision::from_protocol(
                &RadrootsEventHeadDecision::SkippedSameTimestampHigherEventId
            ),
            RadrootsEventHeadStoreDecision::SkippedSameTimestampHigherEventId
        );
        assert_eq!(
            RadrootsEventHeadStoreDecision::from_protocol(
                &RadrootsEventHeadDecision::CoordinateMismatch
            ),
            RadrootsEventHeadStoreDecision::Malformed
        );

        for (semantic, expected) in [
            (
                RadrootsTagSemantic::AddressableCoordinate,
                "addressable_coordinate",
            ),
            (RadrootsTagSemantic::Category, "category"),
            (RadrootsTagSemantic::Citation, "citation"),
            (RadrootsTagSemantic::Contract, "contract"),
            (RadrootsTagSemantic::Counterparty, "counterparty"),
            (RadrootsTagSemantic::Evidence, "evidence"),
            (RadrootsTagSemantic::EventPointer, "event_pointer"),
            (RadrootsTagSemantic::Geohash, "geohash"),
            (RadrootsTagSemantic::GroupId, "group_id"),
            (RadrootsTagSemantic::Identifier, "identifier"),
            (RadrootsTagSemantic::Image, "image"),
            (RadrootsTagSemantic::Kind, "kind"),
            (RadrootsTagSemantic::ListingAddress, "listing_address"),
            (RadrootsTagSemantic::ListingSnapshot, "listing_snapshot"),
            (RadrootsTagSemantic::Location, "location"),
            (RadrootsTagSemantic::PreviousEvent, "previous_event"),
            (RadrootsTagSemantic::Price, "price"),
            (RadrootsTagSemantic::PublishedAt, "published_at"),
            (RadrootsTagSemantic::Relay, "relay"),
            (RadrootsTagSemantic::ReviewTarget, "review_target"),
            (RadrootsTagSemantic::RootEvent, "root_event"),
            (RadrootsTagSemantic::ServiceInput, "service_input"),
            (RadrootsTagSemantic::ServiceOutput, "service_output"),
            (RadrootsTagSemantic::Source, "source"),
            (RadrootsTagSemantic::Status, "status"),
            (RadrootsTagSemantic::Summary, "summary"),
            (RadrootsTagSemantic::Title, "title"),
            (RadrootsTagSemantic::Topic, "topic"),
            (RadrootsTagSemantic::Url, "url"),
        ] {
            assert_eq!(tag_semantic_name(semantic), expected);
        }

        for (value_type, expected) in [
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
        ] {
            assert_eq!(tag_value_type_name(value_type), expected);
        }
    }
}
