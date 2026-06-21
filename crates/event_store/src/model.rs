use crate::RadrootsEventStoreError;
use radroots_events::RadrootsNostrEvent;
use radroots_events::contract::{
    RadrootsContractMatchError, RadrootsEventClass, RadrootsTagSemantic, RadrootsTagValueType,
};
use radroots_events::event_head::RadrootsEventHeadDecision;

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
pub enum RadrootsRelayObservationType {
    Fetch,
    Subscription,
    PublishAck,
    Import,
}

impl RadrootsRelayObservationType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fetch => "fetch",
            Self::Subscription => "subscription",
            Self::PublishAck => "publish_ack",
            Self::Import => "import",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsRelayObservation {
    pub relay_url: String,
    pub observation_type: RadrootsRelayObservationType,
    pub observed_at_ms: i64,
    pub message: Option<String>,
}

impl RadrootsRelayObservation {
    pub fn new(
        relay_url: impl Into<String>,
        observation_type: RadrootsRelayObservationType,
        observed_at_ms: i64,
    ) -> Self {
        Self {
            relay_url: relay_url.into(),
            observation_type,
            observed_at_ms,
            message: None,
        }
    }

    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsEventIngest {
    pub event: RadrootsNostrEvent,
    pub raw_json: Option<String>,
    pub observed_at_ms: i64,
    pub relay_observation: Option<RadrootsRelayObservation>,
}

impl RadrootsEventIngest {
    pub fn new(event: RadrootsNostrEvent, observed_at_ms: i64) -> Self {
        Self {
            event,
            raw_json: None,
            observed_at_ms,
            relay_observation: None,
        }
    }

    pub fn with_raw_json(mut self, raw_json: impl Into<String>) -> Self {
        self.raw_json = Some(raw_json.into());
        self
    }

    pub fn with_observation(mut self, observation: RadrootsRelayObservation) -> Self {
        self.relay_observation = Some(observation);
        self
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
    pub relay_observations: i64,
    pub last_event_seq: Option<i64>,
    pub last_event_updated_at_ms: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsStoredEvent {
    pub seq: i64,
    pub event_id: String,
    pub pubkey: String,
    pub created_at: u32,
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
    pub created_at: u32,
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
        RadrootsTagSemantic::Counterparty => "counterparty",
        RadrootsTagSemantic::EventPointer => "event_pointer",
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
        RadrootsTagSemantic::RootEvent => "root_event",
        RadrootsTagSemantic::ServiceInput => "service_input",
        RadrootsTagSemantic::ServiceOutput => "service_output",
        RadrootsTagSemantic::Status => "status",
        RadrootsTagSemantic::Summary => "summary",
        RadrootsTagSemantic::Title => "title",
        RadrootsTagSemantic::Url => "url",
    }
}

pub fn tag_value_type_name(value: RadrootsTagValueType) -> &'static str {
    match value {
        RadrootsTagValueType::AddressableCoordinate => "addressable_coordinate",
        RadrootsTagValueType::DTag => "d_tag",
        RadrootsTagValueType::EventId => "event_id",
        RadrootsTagValueType::EventPointer => "event_pointer",
        RadrootsTagValueType::Kind => "kind",
        RadrootsTagValueType::PublicKey => "public_key",
        RadrootsTagValueType::RelayUrl => "relay_url",
        RadrootsTagValueType::Text => "text",
        RadrootsTagValueType::UnixTimestamp => "unix_timestamp",
        RadrootsTagValueType::Url => "url",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use radroots_events::event_head::{
        RadrootsCurrentEventHead, RadrootsEventHeadCoordinate, RadrootsEventHeadDecision,
    };
    use radroots_events::ids::{RadrootsDTag, RadrootsEventId, RadrootsPublicKey};

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
            RadrootsRelayObservationType::Fetch,
            RadrootsRelayObservationType::Subscription,
            RadrootsRelayObservationType::PublishAck,
            RadrootsRelayObservationType::Import,
        ] {
            assert!(!observation_type.as_str().is_empty());
        }
        let observation = RadrootsRelayObservation::new(
            "wss://relay.example.test",
            RadrootsRelayObservationType::Fetch,
            1,
        )
        .with_message("seen");
        assert_eq!(observation.message.as_deref(), Some("seen"));
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
            (RadrootsTagSemantic::Counterparty, "counterparty"),
            (RadrootsTagSemantic::EventPointer, "event_pointer"),
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
            (RadrootsTagSemantic::RootEvent, "root_event"),
            (RadrootsTagSemantic::ServiceInput, "service_input"),
            (RadrootsTagSemantic::ServiceOutput, "service_output"),
            (RadrootsTagSemantic::Status, "status"),
            (RadrootsTagSemantic::Summary, "summary"),
            (RadrootsTagSemantic::Title, "title"),
            (RadrootsTagSemantic::Url, "url"),
        ] {
            assert_eq!(tag_semantic_name(semantic), expected);
        }

        for (value_type, expected) in [
            (
                RadrootsTagValueType::AddressableCoordinate,
                "addressable_coordinate",
            ),
            (RadrootsTagValueType::DTag, "d_tag"),
            (RadrootsTagValueType::EventId, "event_id"),
            (RadrootsTagValueType::EventPointer, "event_pointer"),
            (RadrootsTagValueType::Kind, "kind"),
            (RadrootsTagValueType::PublicKey, "public_key"),
            (RadrootsTagValueType::RelayUrl, "relay_url"),
            (RadrootsTagValueType::Text, "text"),
            (RadrootsTagValueType::UnixTimestamp, "unix_timestamp"),
            (RadrootsTagValueType::Url, "url"),
        ] {
            assert_eq!(tag_value_type_name(value_type), expected);
        }
    }
}
