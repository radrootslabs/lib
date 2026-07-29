mod addressable_transition_feed_v1;
mod current_visibility_v1;
mod food_availability_projection_v1;
mod ingest_reconciliation_v1;
pub(crate) mod reconciliation_v1;

pub use addressable_transition_feed_v1::{
    RADROOTS_ADDRESSABLE_TRANSITION_CURSOR_JSON_MAX_BYTES_V1,
    RADROOTS_ADDRESSABLE_TRANSITION_D_TAG_MAX_BYTES_V1,
    RADROOTS_ADDRESSABLE_TRANSITION_FEED_VERSION_V1,
    RADROOTS_ADDRESSABLE_TRANSITION_PAGE_LIMIT_MAX_V1,
    RADROOTS_ADDRESSABLE_TRANSITION_PAGE_RAW_JSON_MAX_BYTES_V1,
    RADROOTS_ADDRESSABLE_TRANSITION_PAGE_SCAN_MAX_V1,
    RADROOTS_ADDRESSABLE_TRANSITION_SCOPE_KIND_MAX_V1, RadrootsAddressableTransitionCauseV1,
    RadrootsAddressableTransitionCoordinateV1, RadrootsAddressableTransitionCursorV1,
    RadrootsAddressableTransitionEventReferenceV1, RadrootsAddressableTransitionOriginV1,
    RadrootsAddressableTransitionPageV1, RadrootsAddressableTransitionRawHeadDecisionV1,
    RadrootsAddressableTransitionScopeFingerprintV1, RadrootsAddressableTransitionScopeV1,
    RadrootsAddressableTransitionV1, RadrootsAddressableTransitionVisibilityV1,
    RadrootsStoreProducedCanonicalEventV1,
};
pub use current_visibility_v1::{
    RadrootsCurrentEventVisibilityV1, RadrootsCurrentVisibilityDecisionV1,
    RadrootsNip09SuppressionEvidenceV1, RadrootsNip09SuppressionOutcome,
    RadrootsNip09SuppressionReason,
};
pub use food_availability_projection_v1::{
    RADROOTS_FOOD_AVAILABILITY_PROJECTION_APPLY_PAGE_LIMIT_V1,
    RADROOTS_FOOD_AVAILABILITY_PROJECTION_VERSION_V1,
    RADROOTS_FOOD_AVAILABILITY_SEARCH_QUERY_MAX_BYTES_V1,
    RADROOTS_FOOD_AVAILABILITY_SEARCH_QUERY_MAX_TERMS_V1, RadrootsFoodAvailabilitySearchQueryV1,
    RadrootsFoodAvailabilityStatusFilterV1, RadrootsStoredFoodAvailabilityImageV1,
    RadrootsStoredFoodAvailabilityV1,
};

use crate::RadrootsEventStoreError;
use radroots_event::envelope::EventKind;
use radroots_event::id::{CandidateId, DTag, EventId, InventoryBinId, MutationId, TradeId};
use radroots_event::trade::TradeMutationKindV1;
use radroots_identity::PublicKey;
use radroots_transport::{
    RadrootsTransportKind, RadrootsTransportTarget, RadrootsTransportTargetFingerprint,
    RadrootsTransportTargetUri,
};
pub use reconciliation_v1::{
    RadrootsEventAdmissionStatus, RadrootsEventIngest, RadrootsEventIngestReceipt,
    RadrootsEventPersistence, RadrootsEventStoreSourceGeneration, RadrootsRawHeadDecision,
    RadrootsStoredRawEvent, RadrootsStoredRawEventHead, StoredEventClass,
};

pub const RADROOTS_TRANSPORT_OBSERVATION_MESSAGE_MAX_BYTES: usize = 4_096;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTransportObservationMessage(String);

impl RadrootsTransportObservationMessage {
    pub fn parse(value: impl Into<String>) -> Result<Self, RadrootsEventStoreError> {
        let value = value.into();
        validate_transport_observation_message(value.as_str()).map_err(|reason| {
            RadrootsEventStoreError::InvalidTransportObservationMessage {
                reason,
                actual_bytes: value.len(),
                max_bytes: RADROOTS_TRANSPORT_OBSERVATION_MESSAGE_MAX_BYTES,
            }
        })?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub(crate) fn parse_stored(
        event_id: &str,
        value: String,
    ) -> Result<Self, RadrootsEventStoreError> {
        validate_transport_observation_message(value.as_str()).map_err(|reason| {
            RadrootsEventStoreError::InvalidStoredTransportObservationMessage {
                event_id: event_id.to_owned(),
                reason,
                actual_bytes: value.len(),
                max_bytes: RADROOTS_TRANSPORT_OBSERVATION_MESSAGE_MAX_BYTES,
            }
        })?;
        Ok(Self(value))
    }
}

impl AsRef<str> for RadrootsTransportObservationMessage {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl core::ops::Deref for RadrootsTransportObservationMessage {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

fn validate_transport_observation_message(value: &str) -> Result<(), &'static str> {
    if value.is_empty() {
        return Err("message must not be empty");
    }
    if value != value.trim() {
        return Err("message must not have surrounding whitespace");
    }
    if value.chars().any(char::is_control) {
        return Err("message must not contain control characters");
    }
    if value.len() > RADROOTS_TRANSPORT_OBSERVATION_MESSAGE_MAX_BYTES {
        return Err("message exceeds the UTF-8 byte limit");
    }
    Ok(())
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
    transport_kind: RadrootsTransportKind,
    endpoint_uri: RadrootsTransportTargetUri,
    endpoint_fingerprint: RadrootsTransportTargetFingerprint,
    observation_type: RadrootsTransportObservationType,
    observed_at_ms: i64,
    caller_redacted_message: Option<RadrootsTransportObservationMessage>,
}

impl RadrootsTransportObservation {
    /// Creates endpoint-level transport evidence.
    ///
    /// Logical target scope and label are intentionally not part of this v1
    /// observation identity. Use transport delivery receipts when scoped
    /// Reticulum or local-target evidence must be preserved.
    pub fn new(
        transport_kind: RadrootsTransportKind,
        endpoint_uri: impl AsRef<str>,
        observation_type: RadrootsTransportObservationType,
        observed_at_ms: i64,
    ) -> Result<Self, RadrootsEventStoreError> {
        if observed_at_ms < 0 {
            return Err(
                RadrootsEventStoreError::InvalidTransportObservationTimestamp {
                    value: observed_at_ms,
                },
            );
        }
        let target = RadrootsTransportTarget::new(transport_kind, endpoint_uri)?;
        Ok(Self {
            transport_kind: target.kind().clone(),
            endpoint_uri: target.uri().clone(),
            endpoint_fingerprint: target.fingerprint().clone(),
            observation_type,
            observed_at_ms,
            caller_redacted_message: None,
        })
    }

    pub fn transport_kind(&self) -> &RadrootsTransportKind {
        &self.transport_kind
    }

    pub fn endpoint_uri(&self) -> &RadrootsTransportTargetUri {
        &self.endpoint_uri
    }

    pub fn endpoint_fingerprint(&self) -> &RadrootsTransportTargetFingerprint {
        &self.endpoint_fingerprint
    }

    pub fn observation_type(&self) -> RadrootsTransportObservationType {
        self.observation_type
    }

    pub fn observed_at_ms(&self) -> i64 {
        self.observed_at_ms
    }

    pub fn caller_redacted_message(&self) -> Option<&str> {
        self.caller_redacted_message.as_deref()
    }

    pub fn try_with_caller_redacted_message(
        mut self,
        message: impl Into<String>,
    ) -> Result<Self, RadrootsEventStoreError> {
        self.caller_redacted_message = Some(RadrootsTransportObservationMessage::parse(message)?);
        Ok(self)
    }

    pub(crate) fn validate_endpoint_for_event(
        &self,
        event_id: &str,
    ) -> Result<(), RadrootsEventStoreError> {
        let target =
            RadrootsTransportTarget::new(self.transport_kind.clone(), self.endpoint_uri.as_str())?;
        if target.uri() != &self.endpoint_uri || target.fingerprint() != &self.endpoint_fingerprint
        {
            return Err(
                RadrootsEventStoreError::InvalidStoredTransportEndpointFingerprint {
                    event_id: event_id.to_owned(),
                    transport_kind: self.transport_kind.canonical_label(),
                    endpoint_uri: self.endpoint_uri.as_str().to_owned(),
                    endpoint_fingerprint: self.endpoint_fingerprint.as_str().to_owned(),
                },
            );
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn from_unchecked_parts_for_test(
        transport_kind: RadrootsTransportKind,
        endpoint_uri: RadrootsTransportTargetUri,
        endpoint_fingerprint: RadrootsTransportTargetFingerprint,
        observation_type: RadrootsTransportObservationType,
        observed_at_ms: i64,
    ) -> Self {
        Self {
            transport_kind,
            endpoint_uri,
            endpoint_fingerprint,
            observation_type,
            observed_at_ms,
            caller_redacted_message: None,
        }
    }
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
pub struct RadrootsStoredValidEvent {
    raw_event: RadrootsStoredRawEvent,
}

impl RadrootsStoredValidEvent {
    pub(crate) fn try_from_raw(
        raw_event: RadrootsStoredRawEvent,
    ) -> Result<Self, RadrootsEventStoreError> {
        let expected_class =
            StoredEventClass::from_event_kind_class(EventKind::new(raw_event.kind).class());
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
    NotCurrent {
        raw_head_event_id: String,
    },
    Suppressed {
        reason: RadrootsNip09SuppressionReason,
        event_reference_request_id: Option<EventId>,
        address_reference_request_id: Option<EventId>,
        address_reference_cutoff: Option<u64>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsProjectionCursor {
    pub(crate) projection_id: String,
    pub(crate) projection_version: u32,
    pub(crate) source_generation: RadrootsEventStoreSourceGeneration,
    pub(crate) last_event_seq: i64,
    pub(crate) updated_at_ms: i64,
}

impl RadrootsProjectionCursor {
    pub fn new(
        projection_id: impl Into<String>,
        projection_version: u32,
        source_generation: RadrootsEventStoreSourceGeneration,
        last_event_seq: i64,
        updated_at_ms: i64,
    ) -> Result<Self, crate::RadrootsEventStoreError> {
        let projection_id = projection_id.into();
        if projection_id.is_empty() {
            return Err(crate::RadrootsEventStoreError::InvalidProjectionId);
        }
        if projection_version == 0 {
            return Err(crate::RadrootsEventStoreError::InvalidProjectionVersion {
                projection_id,
                value: 0,
            });
        }
        if last_event_seq < 0 {
            return Err(crate::RadrootsEventStoreError::InvalidProjectionCursor {
                projection_id,
                value: last_event_seq,
            });
        }
        Ok(Self {
            projection_id,
            projection_version,
            source_generation,
            last_event_seq,
            updated_at_ms,
        })
    }

    pub fn projection_id(&self) -> &str {
        self.projection_id.as_str()
    }

    pub const fn projection_version(&self) -> u32 {
        self.projection_version
    }

    pub const fn source_generation(&self) -> RadrootsEventStoreSourceGeneration {
        self.source_generation
    }

    pub const fn last_event_seq(&self) -> i64 {
        self.last_event_seq
    }

    pub const fn updated_at_ms(&self) -> i64 {
        self.updated_at_ms
    }
}

#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsProjectionRebuildPrior {
    Missing,
    Cursor {
        source_generation: Option<RadrootsEventStoreSourceGeneration>,
        source_revision: u64,
        projection_version: u32,
        last_event_seq: i64,
        updated_at_ms: i64,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsProjectionRebuildTicket {
    pub(crate) projection_id: String,
    pub(crate) target_projection_version: u32,
    pub(crate) target_source_generation: RadrootsEventStoreSourceGeneration,
    pub(crate) target_raw_high_water_seq: i64,
    pub(crate) prior: RadrootsProjectionRebuildPrior,
}

impl RadrootsProjectionRebuildTicket {
    pub fn projection_id(&self) -> &str {
        self.projection_id.as_str()
    }

    pub const fn target_projection_version(&self) -> u32 {
        self.target_projection_version
    }

    pub const fn target_source_generation(&self) -> RadrootsEventStoreSourceGeneration {
        self.target_source_generation
    }

    pub const fn target_raw_high_water_seq(&self) -> i64 {
        self.target_raw_high_water_seq
    }

    pub const fn prior(&self) -> &RadrootsProjectionRebuildPrior {
        &self.prior
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsStoredTradeMutation {
    pub mutation_id: MutationId,
    pub trade_id: TradeId,
    pub root_mutation_id: Option<MutationId>,
    pub contract_id: String,
    pub mutation_kind: TradeMutationKindV1,
    pub schema_version: u16,
    pub candidate_id: Option<CandidateId>,
    pub proposal_mutation_id: Option<MutationId>,
    pub target_claim_mutation_id: Option<MutationId>,
    pub author_pubkey: PublicKey,
    pub counterparty_pubkey: PublicKey,
    pub buyer_pubkey: PublicKey,
    pub seller_pubkey: PublicKey,
    pub farm_id: DTag,
    pub authored_at_unix_s: u64,
    pub canonical_payload_bytes: Vec<u8>,
    pub payload_sha256: String,
    pub first_event_seq: i64,
    pub first_transport_event_id: EventId,
    pub inserted_at_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsStoredTradeMutationParent {
    pub mutation_id: MutationId,
    pub parent_mutation_id: MutationId,
    pub parent_index: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsStoredTradeMissingParent {
    pub trade_id: TradeId,
    pub mutation_id: MutationId,
    pub missing_parent_mutation_id: MutationId,
    pub first_transport_event_id: EventId,
    pub first_seen_at_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsStoredTradeTransportEnvelope {
    pub transport_event_id: EventId,
    pub mutation_id: MutationId,
    pub trade_id: TradeId,
    pub transport_kind: String,
    pub pubkey: PublicKey,
    pub created_at: u64,
    pub event_seq: i64,
    pub payload_sha256: String,
    pub observed_at_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsStoredSellerReservation {
    pub reservation_id: DTag,
    pub trade_id: TradeId,
    pub candidate_id: CandidateId,
    pub claim_mutation_id: MutationId,
    pub inventory_authority_pubkey: PublicKey,
    pub inventory_epoch: u64,
    pub assertion_commitment: String,
    pub reservation_expires_at_unix_s: u64,
    pub reservation_json: String,
    pub inserted_at_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsStoredSellerReservationLine {
    pub reservation_id: DTag,
    pub line_id: DTag,
    pub bin_id: InventoryBinId,
    pub quantity_mantissa: String,
    pub quantity_scale: u8,
    pub unit_code: String,
    pub line_index: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeProjectionCheckpoint {
    pub trade_id: TradeId,
    pub reducer_contract_id: String,
    pub reducer_version: u16,
    pub projection_digest: String,
    pub root_mutation_id: Option<MutationId>,
    pub negotiation_state: String,
    pub agreement_state: String,
    pub evidence_state: String,
    pub conflict_state: String,
    pub private_terms_state: String,
    pub attestation_state: String,
    pub fulfillment_state: String,
    pub payment_state: String,
    pub projection_json: String,
    pub last_mutation_id: Option<MutationId>,
    pub last_transport_event_seq: Option<i64>,
    pub updated_at_ms: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::reconciliation_v1::{tag_semantic_name, tag_value_type_name};
    use radroots_event::contract::{TagSemantic, TagValueType};
    use radroots_event::envelope::EventKindClass;
    use radroots_event::envelope::event_head::{
        CurrentEventHead, EventHeadCoordinate, EventHeadDecision,
    };
    use radroots_event::id::EventId;
    use radroots_identity::PublicKey;

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
            StoredEventClass::from_event_kind_class(EventKindClass::Regular),
            StoredEventClass::Regular
        );
        assert_eq!(
            StoredEventClass::from_event_kind_class(EventKindClass::Replaceable),
            StoredEventClass::Replaceable
        );
        assert_eq!(
            StoredEventClass::from_event_kind_class(EventKindClass::Addressable),
            StoredEventClass::Addressable
        );
        assert_eq!(
            StoredEventClass::from_event_kind_class(EventKindClass::Ephemeral),
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
        .try_with_caller_redacted_message("seen")
        .expect("caller-redacted message");
        assert_eq!(observation.caller_redacted_message(), Some("seen"));
        assert_eq!(
            observation.endpoint_uri().as_str(),
            "wss://relay.example.test"
        );
        let canonical_relay = RadrootsTransportObservation::new(
            RadrootsTransportKind::Nostr,
            "WSS://RELAY.EXAMPLE.TEST/",
            RadrootsTransportObservationType::Fetch,
            1,
        )
        .expect("canonical relay observation");
        assert_eq!(
            canonical_relay.endpoint_uri().as_str(),
            "wss://relay.example.test"
        );
        for invalid_relay in [
            "https://relay.example.test",
            "wss://relay.example.test?query=1",
            "wss://:443",
            "ws://relay.example.test",
        ] {
            assert!(
                RadrootsTransportObservation::new(
                    RadrootsTransportKind::Nostr,
                    invalid_relay,
                    RadrootsTransportObservationType::Fetch,
                    1,
                )
                .is_err(),
                "accepted invalid relay `{invalid_relay}`"
            );
        }
        let reticulum = RadrootsTransportObservation::new(
            RadrootsTransportKind::Reticulum,
            "reticulum:local",
            RadrootsTransportObservationType::MeshHeard,
            1,
        )
        .expect("Reticulum observation");
        let expected_reticulum =
            RadrootsTransportTarget::reticulum().expect("canonical Reticulum target");
        assert_eq!(reticulum.endpoint_uri(), expected_reticulum.uri());
        assert_eq!(
            reticulum.endpoint_fingerprint(),
            expected_reticulum.fingerprint()
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
        assert!(matches!(
            RadrootsTransportObservation::new(
                RadrootsTransportKind::Nostr,
                "wss://relay.example.test",
                RadrootsTransportObservationType::Fetch,
                -1,
            ),
            Err(RadrootsEventStoreError::InvalidTransportObservationTimestamp { value: -1 })
        ));
        for invalid_message in [
            "",
            " ",
            " leading",
            "trailing ",
            "line\nbreak",
            "tab\tseparated",
        ] {
            assert!(
                observation
                    .clone()
                    .try_with_caller_redacted_message(invalid_message)
                    .is_err(),
                "accepted invalid caller-redacted message {invalid_message:?}"
            );
        }
        assert!(
            observation
                .clone()
                .try_with_caller_redacted_message(
                    "x".repeat(RADROOTS_TRANSPORT_OBSERVATION_MESSAGE_MAX_BYTES + 1),
                )
                .is_err()
        );
    }

    #[test]
    fn head_decisions_and_tag_metadata_names_cover_all_variants() {
        let coordinate = EventHeadCoordinate::Addressable {
            kind: 30_023,
            pubkey: PublicKey::from_hex(
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            )
            .expect("pubkey"),
            d_tag: "opaque d value".to_owned(),
        };
        let current = CurrentEventHead {
            coordinate,
            event_id: EventId::parse(
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            )
            .expect("event id"),
            created_at: 10,
        };

        assert_eq!(
            RadrootsRawHeadDecision::from_protocol(&EventHeadDecision::Applied(current)),
            RadrootsRawHeadDecision::Applied
        );
        assert_eq!(
            RadrootsRawHeadDecision::from_protocol(&EventHeadDecision::SkippedDuplicate),
            RadrootsRawHeadDecision::SkippedDuplicate
        );
        assert_eq!(
            RadrootsRawHeadDecision::from_protocol(&EventHeadDecision::SkippedOlder),
            RadrootsRawHeadDecision::SkippedOlder
        );
        assert_eq!(
            RadrootsRawHeadDecision::from_protocol(
                &EventHeadDecision::SkippedSameTimestampHigherEventId
            ),
            RadrootsRawHeadDecision::SkippedSameTimestampHigherEventId
        );
        assert_eq!(
            RadrootsRawHeadDecision::from_protocol(&EventHeadDecision::CoordinateMismatch),
            RadrootsRawHeadDecision::MalformedCoordinate
        );

        for (semantic, expected) in [
            (TagSemantic::AddressableCoordinate, "addressable_coordinate"),
            (TagSemantic::CalendarEventAuthor, "calendar_event_author"),
            (
                TagSemantic::CalendarEventReference,
                "calendar_event_reference",
            ),
            (
                TagSemantic::CalendarEventRevision,
                "calendar_event_revision",
            ),
            (
                TagSemantic::CalendarInclusionRequest,
                "calendar_inclusion_request",
            ),
            (TagSemantic::CalendarEnd, "calendar_end"),
            (TagSemantic::CalendarStart, "calendar_start"),
            (TagSemantic::Category, "category"),
            (TagSemantic::Citation, "citation"),
            (TagSemantic::Contract, "contract"),
            (TagSemantic::Counterparty, "counterparty"),
            (TagSemantic::Evidence, "evidence"),
            (TagSemantic::EventPointer, "event_pointer"),
            (TagSemantic::FreeBusy, "free_busy"),
            (TagSemantic::Geohash, "geohash"),
            (TagSemantic::GroupId, "group_id"),
            (TagSemantic::Identifier, "identifier"),
            (TagSemantic::Image, "image"),
            (TagSemantic::Kind, "kind"),
            (TagSemantic::ClassifiedListingAddress, "listing_address"),
            (TagSemantic::OperationalListingSnapshot, "listing_snapshot"),
            (TagSemantic::ListDescription, "list_description"),
            (TagSemantic::Location, "location"),
            (TagSemantic::Nip01Coordinate, "nip01_coordinate"),
            (TagSemantic::Participant, "participant"),
            (TagSemantic::PreviousEvent, "previous_event"),
            (TagSemantic::Price, "price"),
            (TagSemantic::PublishedAt, "published_at"),
            (TagSemantic::Relay, "relay"),
            (TagSemantic::Reference, "reference"),
            (TagSemantic::ReviewTarget, "review_target"),
            (TagSemantic::RootEvent, "root_event"),
            (TagSemantic::ServiceInput, "service_input"),
            (TagSemantic::ServiceOutput, "service_output"),
            (TagSemantic::Source, "source"),
            (TagSemantic::Status, "status"),
            (TagSemantic::Summary, "summary"),
            (TagSemantic::Title, "title"),
            (TagSemantic::Topic, "topic"),
            (TagSemantic::TimeZone, "time_zone"),
            (TagSemantic::Url, "url"),
            (TagSemantic::UtcDayCoverage, "utc_day_coverage"),
        ] {
            assert_eq!(tag_semantic_name(semantic), expected);
        }

        for (value_type, expected) in [
            (
                TagValueType::AddressableCoordinate,
                "addressable_coordinate",
            ),
            (TagValueType::CalendarDate, "calendar_date"),
            (
                TagValueType::CalendarEventCoordinate,
                "calendar_event_coordinate",
            ),
            (TagValueType::CalendarFreeBusy, "calendar_free_busy"),
            (TagValueType::CalendarRsvpStatus, "calendar_rsvp_status"),
            (TagValueType::CalendarUid, "calendar_uid"),
            (TagValueType::ContractId, "contract_id"),
            (TagValueType::DTag, "d_tag"),
            (TagValueType::EventId, "event_id"),
            (TagValueType::EventPointer, "event_pointer"),
            (TagValueType::Geohash, "geohash"),
            (TagValueType::IanaTimeZoneId, "iana_time_zone_id"),
            (TagValueType::Kind, "kind"),
            (TagValueType::Nip01Coordinate, "nip01_coordinate"),
            (TagValueType::PublicKey, "public_key"),
            (TagValueType::RelayUrl, "relay_url"),
            (TagValueType::Sha256, "sha256"),
            (TagValueType::Text, "text"),
            (TagValueType::UnixTimestamp, "unix_timestamp"),
            (TagValueType::Uri, "uri"),
            (TagValueType::Url, "url"),
            (TagValueType::UtcDayIndex, "utc_day_index"),
            (TagValueType::Uuid, "uuid"),
        ] {
            assert_eq!(tag_value_type_name(value_type), expected);
        }
    }
}
