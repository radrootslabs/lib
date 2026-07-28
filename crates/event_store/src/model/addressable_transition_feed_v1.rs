use super::{
    RadrootsEventAdmissionStatus, RadrootsEventStoreSourceGeneration,
    RadrootsNip09SuppressionEvidenceV1,
};
use crate::RadrootsEventStoreError;
use radroots_event::ids::{RadrootsEventId, RadrootsPublicKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub const RADROOTS_ADDRESSABLE_TRANSITION_FEED_VERSION_V1: u32 = 1;
pub const RADROOTS_ADDRESSABLE_TRANSITION_SCOPE_KIND_MAX_V1: usize = 64;
pub const RADROOTS_ADDRESSABLE_TRANSITION_PAGE_LIMIT_MAX_V1: u32 = 64;
pub const RADROOTS_ADDRESSABLE_TRANSITION_PAGE_SCAN_MAX_V1: u32 = 1_024;
pub const RADROOTS_ADDRESSABLE_TRANSITION_PAGE_RAW_JSON_MAX_BYTES_V1: usize = 4 * 1024 * 1024;
pub const RADROOTS_ADDRESSABLE_TRANSITION_D_TAG_MAX_BYTES_V1: usize =
    radroots_event::wire::v1::DEFAULT_TAG_ELEMENT_MAX_BYTES;
pub const RADROOTS_ADDRESSABLE_TRANSITION_CURSOR_JSON_MAX_BYTES_V1: usize = 512;
const SCOPE_FINGERPRINT_DOMAIN_V1: &[u8] = b"radroots.addressable-transition-scope.v1\0";
const _: () = assert!(
    radroots_event::wire::v1::DEFAULT_RAW_JSON_MAX_BYTES
        <= RADROOTS_ADDRESSABLE_TRANSITION_PAGE_RAW_JSON_MAX_BYTES_V1
);
const _: () = assert!(
    RADROOTS_ADDRESSABLE_TRANSITION_PAGE_LIMIT_MAX_V1 as usize
        <= usize::MAX / radroots_event::wire::v1::DEFAULT_RAW_JSON_MAX_BYTES
);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RadrootsAddressableTransitionScopeFingerprintV1([u8; 32]);

impl RadrootsAddressableTransitionScopeFingerprintV1 {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_hex(self) -> String {
        hex::encode(self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsAddressableTransitionScopeV1 {
    kinds: Vec<u32>,
    fingerprint: RadrootsAddressableTransitionScopeFingerprintV1,
}

impl RadrootsAddressableTransitionScopeV1 {
    pub fn new(kinds: impl IntoIterator<Item = u32>) -> Result<Self, RadrootsEventStoreError> {
        let mut canonical_kinds = BTreeSet::new();
        let mut input_count = 0usize;
        for kind in kinds {
            input_count = input_count.saturating_add(1);
            if input_count > RADROOTS_ADDRESSABLE_TRANSITION_SCOPE_KIND_MAX_V1 {
                return Err(
                    RadrootsEventStoreError::AddressableTransitionScopeTooLarge {
                        max: RADROOTS_ADDRESSABLE_TRANSITION_SCOPE_KIND_MAX_V1,
                        actual: input_count,
                    },
                );
            }
            canonical_kinds.insert(kind);
        }
        let kinds = canonical_kinds;
        if kinds.is_empty() {
            return Err(RadrootsEventStoreError::AddressableTransitionScopeEmpty);
        }
        if let Some(kind) = kinds
            .iter()
            .copied()
            .find(|kind| !(30_000..=39_999).contains(kind))
        {
            return Err(RadrootsEventStoreError::AddressableTransitionScopeKindInvalid { kind });
        }
        let kinds = kinds.into_iter().collect::<Vec<_>>();
        let mut hasher = Sha256::new();
        hasher.update(SCOPE_FINGERPRINT_DOMAIN_V1);
        hasher.update(
            u32::try_from(kinds.len())
                .expect("scope maximum fits u32")
                .to_be_bytes(),
        );
        for kind in &kinds {
            hasher.update(kind.to_be_bytes());
        }
        let fingerprint =
            RadrootsAddressableTransitionScopeFingerprintV1::from_bytes(hasher.finalize().into());
        Ok(Self { kinds, fingerprint })
    }

    pub fn food_availability() -> Self {
        Self::new([30_402]).expect("the FoodAvailability kind is addressable")
    }

    pub fn kinds(&self) -> &[u32] {
        &self.kinds
    }

    pub const fn fingerprint(&self) -> RadrootsAddressableTransitionScopeFingerprintV1 {
        self.fingerprint
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsAddressableTransitionCursorV1 {
    source_generation: RadrootsEventStoreSourceGeneration,
    scope_fingerprint: RadrootsAddressableTransitionScopeFingerprintV1,
    last_transition_seq: i64,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AddressableTransitionCursorWireV1 {
    source_generation: String,
    feed_version: u32,
    scope_fingerprint: String,
    last_transition_seq: i64,
}

impl RadrootsAddressableTransitionCursorV1 {
    pub fn new(
        source_generation: RadrootsEventStoreSourceGeneration,
        scope_fingerprint: RadrootsAddressableTransitionScopeFingerprintV1,
        last_transition_seq: i64,
    ) -> Result<Self, RadrootsEventStoreError> {
        if last_transition_seq < 0 {
            return Err(
                RadrootsEventStoreError::AddressableTransitionCursorNegative {
                    value: last_transition_seq,
                },
            );
        }
        Ok(Self {
            source_generation,
            scope_fingerprint,
            last_transition_seq,
        })
    }

    pub const fn source_generation(&self) -> RadrootsEventStoreSourceGeneration {
        self.source_generation
    }

    pub const fn feed_version(&self) -> u32 {
        RADROOTS_ADDRESSABLE_TRANSITION_FEED_VERSION_V1
    }

    pub const fn scope_fingerprint(&self) -> RadrootsAddressableTransitionScopeFingerprintV1 {
        self.scope_fingerprint
    }

    pub const fn last_transition_seq(&self) -> i64 {
        self.last_transition_seq
    }

    pub fn to_json(&self) -> Result<String, RadrootsEventStoreError> {
        Ok(serde_json::to_string(&AddressableTransitionCursorWireV1 {
            source_generation: hex::encode(self.source_generation.as_bytes()),
            feed_version: RADROOTS_ADDRESSABLE_TRANSITION_FEED_VERSION_V1,
            scope_fingerprint: self.scope_fingerprint.to_hex(),
            last_transition_seq: self.last_transition_seq,
        })?)
    }

    pub fn from_json(value: &str) -> Result<Self, RadrootsEventStoreError> {
        if value.len() > RADROOTS_ADDRESSABLE_TRANSITION_CURSOR_JSON_MAX_BYTES_V1 {
            return Err(
                RadrootsEventStoreError::AddressableTransitionCursorTooLarge {
                    max: RADROOTS_ADDRESSABLE_TRANSITION_CURSOR_JSON_MAX_BYTES_V1,
                    actual: value.len(),
                },
            );
        }
        let cursor: AddressableTransitionCursorWireV1 = serde_json::from_str(value)?;
        if cursor.feed_version != RADROOTS_ADDRESSABLE_TRANSITION_FEED_VERSION_V1 {
            return Err(
                RadrootsEventStoreError::AddressableTransitionFeedVersionMismatch {
                    expected: RADROOTS_ADDRESSABLE_TRANSITION_FEED_VERSION_V1,
                    actual: cursor.feed_version,
                },
            );
        }
        Self::new(
            RadrootsEventStoreSourceGeneration::from_bytes(decode_cursor_hex(
                "source_generation",
                cursor.source_generation.as_str(),
            )?),
            RadrootsAddressableTransitionScopeFingerprintV1::from_bytes(decode_cursor_hex(
                "scope_fingerprint",
                cursor.scope_fingerprint.as_str(),
            )?),
            cursor.last_transition_seq,
        )
    }
}

fn decode_cursor_hex(
    field: &'static str,
    value: &str,
) -> Result<[u8; 32], RadrootsEventStoreError> {
    if value.len() != 64
        || value
            .bytes()
            .any(|byte| !byte.is_ascii_digit() && !(b'a'..=b'f').contains(&byte))
    {
        return Err(RadrootsEventStoreError::AddressableTransitionCursorEncoding { field });
    }
    let mut decoded = [0_u8; 32];
    hex::decode_to_slice(value, &mut decoded)
        .expect("the exact lowercase hexadecimal cursor encoding was validated");
    Ok(decoded)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsAddressableTransitionOriginV1 {
    Baseline,
    Incremental,
}

impl RadrootsAddressableTransitionOriginV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Baseline => "baseline",
            Self::Incremental => "incremental",
        }
    }

    pub(crate) fn parse(value: &str) -> Result<Self, RadrootsEventStoreError> {
        match value {
            "baseline" => Ok(Self::Baseline),
            "incremental" => Ok(Self::Incremental),
            _ => Err(RadrootsEventStoreError::InvalidStoredEnum {
                field: "addressable_transition.origin",
                value: value.to_owned(),
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsAddressableTransitionRawHeadDecisionV1 {
    BaselineRebuild,
    Applied,
    NotHeadSelected,
    SkippedOlder,
    SkippedSameTimestampHigherEventId,
    MalformedCoordinate,
}

impl RadrootsAddressableTransitionRawHeadDecisionV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BaselineRebuild => "baseline_rebuild",
            Self::Applied => "applied",
            Self::NotHeadSelected => "not_head_selected",
            Self::SkippedOlder => "skipped_older",
            Self::SkippedSameTimestampHigherEventId => "skipped_same_timestamp_higher_event_id",
            Self::MalformedCoordinate => "malformed_coordinate",
        }
    }

    pub(crate) fn parse(value: &str) -> Result<Self, RadrootsEventStoreError> {
        match value {
            "baseline_rebuild" => Ok(Self::BaselineRebuild),
            "applied" => Ok(Self::Applied),
            "not_head_selected" => Ok(Self::NotHeadSelected),
            "skipped_older" => Ok(Self::SkippedOlder),
            "skipped_same_timestamp_higher_event_id" => Ok(Self::SkippedSameTimestampHigherEventId),
            "malformed_coordinate" => Ok(Self::MalformedCoordinate),
            _ => Err(RadrootsEventStoreError::InvalidStoredEnum {
                field: "addressable_transition.raw_head_decision",
                value: value.to_owned(),
            }),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsAddressableTransitionEventReferenceV1 {
    pub(crate) event_id: RadrootsEventId,
    pub(crate) event_seq: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsAddressableTransitionCauseV1 {
    pub(crate) event: RadrootsAddressableTransitionEventReferenceV1,
    pub(crate) pubkey: RadrootsPublicKey,
    pub(crate) created_at: u64,
    pub(crate) kind: u32,
    pub(crate) admission_status: RadrootsEventAdmissionStatus,
    pub(crate) admission_code: Option<String>,
    pub(crate) contract_id: Option<String>,
}

impl RadrootsAddressableTransitionCauseV1 {
    pub const fn event(&self) -> &RadrootsAddressableTransitionEventReferenceV1 {
        &self.event
    }

    pub const fn pubkey(&self) -> &RadrootsPublicKey {
        &self.pubkey
    }

    pub const fn created_at(&self) -> u64 {
        self.created_at
    }

    pub const fn kind(&self) -> u32 {
        self.kind
    }

    pub const fn admission_status(&self) -> RadrootsEventAdmissionStatus {
        self.admission_status
    }

    pub fn admission_code(&self) -> Option<&str> {
        self.admission_code.as_deref()
    }

    pub fn contract_id(&self) -> Option<&str> {
        self.contract_id.as_deref()
    }
}

/// The exact addressable head identity retained by the event store.
///
/// This is intentionally not a [`radroots_event::ids::RadrootsNip01Coordinate`]:
/// an individually valid maximum-size `d` tag can make the combined NIP-01
/// coordinate too large for a wire tag element while still remaining valid raw
/// head identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsAddressableTransitionCoordinateV1 {
    pub(crate) kind: u32,
    pub(crate) pubkey: RadrootsPublicKey,
    pub(crate) d_tag: String,
}

impl RadrootsAddressableTransitionCoordinateV1 {
    pub const fn kind(&self) -> u32 {
        self.kind
    }

    pub const fn pubkey(&self) -> &RadrootsPublicKey {
        &self.pubkey
    }

    pub fn d_tag(&self) -> &str {
        self.d_tag.as_str()
    }
}

impl RadrootsAddressableTransitionEventReferenceV1 {
    pub const fn event_id(&self) -> &RadrootsEventId {
        &self.event_id
    }

    pub const fn event_seq(&self) -> i64 {
        self.event_seq
    }
}

/// A store-selected transition-time visible event with portable signed identity.
///
/// The opaque JSON has already passed id and signature verification. Local
/// database sequence and observation timestamps are deliberately excluded.
/// This snapshot is meaningful only in its containing ordered transition; it
/// is not proof that the event remains current when a historical page is read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsStoreProducedCanonicalEventV1 {
    pub(crate) event_id: RadrootsEventId,
    pub(crate) pubkey: RadrootsPublicKey,
    pub(crate) created_at: u64,
    pub(crate) kind: u32,
    pub(crate) raw_json: String,
}

impl RadrootsStoreProducedCanonicalEventV1 {
    pub const fn event_id(&self) -> &RadrootsEventId {
        &self.event_id
    }

    pub const fn pubkey(&self) -> &RadrootsPublicKey {
        &self.pubkey
    }

    pub const fn created_at(&self) -> u64 {
        self.created_at
    }

    pub const fn kind(&self) -> u32 {
        self.kind
    }

    pub fn raw_json(&self) -> &str {
        self.raw_json.as_str()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsAddressableTransitionVisibilityV1 {
    Visible,
    NotAdmitted,
    Suppressed,
}

impl RadrootsAddressableTransitionVisibilityV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Visible => "visible",
            Self::NotAdmitted => "not_admitted",
            Self::Suppressed => "suppressed",
        }
    }

    pub(crate) fn parse(value: &str) -> Result<Self, RadrootsEventStoreError> {
        match value {
            "visible" => Ok(Self::Visible),
            "not_admitted" => Ok(Self::NotAdmitted),
            "suppressed" => Ok(Self::Suppressed),
            _ => Err(RadrootsEventStoreError::InvalidStoredEnum {
                field: "addressable_transition.visibility",
                value: value.to_owned(),
            }),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsAddressableTransitionV1 {
    pub(crate) transition_seq: i64,
    pub(crate) source_generation: RadrootsEventStoreSourceGeneration,
    pub(crate) origin: RadrootsAddressableTransitionOriginV1,
    pub(crate) coordinate: RadrootsAddressableTransitionCoordinateV1,
    pub(crate) raw_head: RadrootsAddressableTransitionEventReferenceV1,
    pub(crate) raw_head_created_at: u64,
    pub(crate) visible_event: Option<RadrootsStoreProducedCanonicalEventV1>,
    pub(crate) retracted_event: Option<RadrootsAddressableTransitionEventReferenceV1>,
    pub(crate) admission_status: RadrootsEventAdmissionStatus,
    pub(crate) admission_code: Option<String>,
    pub(crate) contract_id: Option<String>,
    pub(crate) visibility: RadrootsAddressableTransitionVisibilityV1,
    pub(crate) suppression: Option<RadrootsNip09SuppressionEvidenceV1>,
    pub(crate) cause_event: Option<RadrootsAddressableTransitionCauseV1>,
    pub(crate) raw_head_decision: RadrootsAddressableTransitionRawHeadDecisionV1,
}

impl RadrootsAddressableTransitionV1 {
    pub const fn transition_seq(&self) -> i64 {
        self.transition_seq
    }

    pub const fn source_generation(&self) -> RadrootsEventStoreSourceGeneration {
        self.source_generation
    }

    pub const fn origin(&self) -> RadrootsAddressableTransitionOriginV1 {
        self.origin
    }

    pub const fn coordinate(&self) -> &RadrootsAddressableTransitionCoordinateV1 {
        &self.coordinate
    }

    pub const fn raw_head(&self) -> &RadrootsAddressableTransitionEventReferenceV1 {
        &self.raw_head
    }

    pub const fn raw_head_created_at(&self) -> u64 {
        self.raw_head_created_at
    }

    pub const fn visible_event(&self) -> Option<&RadrootsStoreProducedCanonicalEventV1> {
        self.visible_event.as_ref()
    }

    pub const fn retracted_event(&self) -> Option<&RadrootsAddressableTransitionEventReferenceV1> {
        self.retracted_event.as_ref()
    }

    pub const fn admission_status(&self) -> RadrootsEventAdmissionStatus {
        self.admission_status
    }

    pub fn admission_code(&self) -> Option<&str> {
        self.admission_code.as_deref()
    }

    pub fn contract_id(&self) -> Option<&str> {
        self.contract_id.as_deref()
    }

    pub const fn visibility(&self) -> RadrootsAddressableTransitionVisibilityV1 {
        self.visibility
    }

    pub const fn suppression(&self) -> Option<&RadrootsNip09SuppressionEvidenceV1> {
        self.suppression.as_ref()
    }

    pub const fn cause_event(&self) -> Option<&RadrootsAddressableTransitionCauseV1> {
        self.cause_event.as_ref()
    }

    pub const fn raw_head_decision(&self) -> RadrootsAddressableTransitionRawHeadDecisionV1 {
        self.raw_head_decision
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsAddressableTransitionPageV1 {
    pub(crate) source_high_water: i64,
    pub(crate) transitions: Vec<RadrootsAddressableTransitionV1>,
    pub(crate) next_cursor: RadrootsAddressableTransitionCursorV1,
    pub(crate) has_more: bool,
}

impl RadrootsAddressableTransitionPageV1 {
    pub const fn source_high_water(&self) -> i64 {
        self.source_high_water
    }

    pub fn transitions(&self) -> &[RadrootsAddressableTransitionV1] {
        &self.transitions
    }

    pub const fn next_cursor(&self) -> &RadrootsAddressableTransitionCursorV1 {
        &self.next_cursor
    }

    pub const fn has_more(&self) -> bool {
        self.has_more
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_is_canonical_bounded_and_fingerprinted() {
        let scope = RadrootsAddressableTransitionScopeV1::new([39_999, 30_402, 30_402])
            .expect("canonical scope");
        assert_eq!(scope.kinds(), &[30_402, 39_999]);
        let maximum_scope = RadrootsAddressableTransitionScopeV1::new(
            (0..RADROOTS_ADDRESSABLE_TRANSITION_SCOPE_KIND_MAX_V1)
                .map(|index| 30_000 + u32::try_from(index).expect("bounded index")),
        )
        .expect("maximum-size scope");
        assert_eq!(
            maximum_scope.kinds().len(),
            RADROOTS_ADDRESSABLE_TRANSITION_SCOPE_KIND_MAX_V1,
        );
        assert_eq!(
            RadrootsAddressableTransitionScopeV1::food_availability()
                .fingerprint()
                .to_hex(),
            "8b63c5ddc48a2cc7db69295238b96d5f814dba50427c80b4d0079f061e6d3de0"
        );
        assert!(matches!(
            RadrootsAddressableTransitionScopeV1::new([]),
            Err(RadrootsEventStoreError::AddressableTransitionScopeEmpty)
        ));
        assert!(matches!(
            RadrootsAddressableTransitionScopeV1::new([29_999]),
            Err(RadrootsEventStoreError::AddressableTransitionScopeKindInvalid { kind: 29_999 })
        ));
        assert!(matches!(
            RadrootsAddressableTransitionScopeV1::new(
                (0..=RADROOTS_ADDRESSABLE_TRANSITION_SCOPE_KIND_MAX_V1)
                    .map(|index| 30_000 + u32::try_from(index).expect("bounded index"))
            ),
            Err(RadrootsEventStoreError::AddressableTransitionScopeTooLarge {
                max: RADROOTS_ADDRESSABLE_TRANSITION_SCOPE_KIND_MAX_V1,
                actual
            }) if actual == RADROOTS_ADDRESSABLE_TRANSITION_SCOPE_KIND_MAX_V1 + 1
        ));
    }

    #[test]
    fn cursor_wire_round_trip_and_typed_failures_are_stable() {
        let scope = RadrootsAddressableTransitionScopeV1::food_availability();
        let cursor = RadrootsAddressableTransitionCursorV1::new(
            RadrootsEventStoreSourceGeneration::from_bytes([0x42; 32]),
            scope.fingerprint(),
            17,
        )
        .expect("cursor");
        let json = cursor.to_json().expect("cursor JSON");
        assert_eq!(
            RadrootsAddressableTransitionCursorV1::from_json(json.as_str()).expect("round trip"),
            cursor
        );

        let mut value: serde_json::Value = serde_json::from_str(&json).expect("wire object");
        value["feed_version"] = serde_json::json!(2);
        assert!(matches!(
            RadrootsAddressableTransitionCursorV1::from_json(
                serde_json::to_string(&value)
                    .expect("version JSON")
                    .as_str()
            ),
            Err(
                RadrootsEventStoreError::AddressableTransitionFeedVersionMismatch {
                    expected: RADROOTS_ADDRESSABLE_TRANSITION_FEED_VERSION_V1,
                    actual: 2,
                }
            )
        ));

        value["feed_version"] = serde_json::json!(RADROOTS_ADDRESSABLE_TRANSITION_FEED_VERSION_V1);
        value["source_generation"] = serde_json::json!("AA".repeat(32));
        assert!(matches!(
            RadrootsAddressableTransitionCursorV1::from_json(
                serde_json::to_string(&value)
                    .expect("encoding JSON")
                    .as_str()
            ),
            Err(
                RadrootsEventStoreError::AddressableTransitionCursorEncoding {
                    field: "source_generation"
                }
            )
        ));

        value["source_generation"] = serde_json::json!("42".repeat(32));
        value["scope_fingerprint"] = serde_json::json!("AA".repeat(32));
        assert!(matches!(
            RadrootsAddressableTransitionCursorV1::from_json(
                serde_json::to_string(&value)
                    .expect("scope encoding JSON")
                    .as_str()
            ),
            Err(
                RadrootsEventStoreError::AddressableTransitionCursorEncoding {
                    field: "scope_fingerprint"
                }
            )
        ));

        assert!(matches!(
            RadrootsAddressableTransitionCursorV1::new(
                cursor.source_generation(),
                cursor.scope_fingerprint(),
                -1,
            ),
            Err(RadrootsEventStoreError::AddressableTransitionCursorNegative { value: -1 })
        ));
        let oversized = " ".repeat(RADROOTS_ADDRESSABLE_TRANSITION_CURSOR_JSON_MAX_BYTES_V1 + 1);
        assert!(matches!(
            RadrootsAddressableTransitionCursorV1::from_json(&oversized),
            Err(RadrootsEventStoreError::AddressableTransitionCursorTooLarge {
                max: RADROOTS_ADDRESSABLE_TRANSITION_CURSOR_JSON_MAX_BYTES_V1,
                actual
            }) if actual == RADROOTS_ADDRESSABLE_TRANSITION_CURSOR_JSON_MAX_BYTES_V1 + 1
        ));
    }

    #[test]
    fn transition_storage_enums_round_trip_and_reject_unknown_values() {
        for origin in [
            RadrootsAddressableTransitionOriginV1::Baseline,
            RadrootsAddressableTransitionOriginV1::Incremental,
        ] {
            assert_eq!(
                RadrootsAddressableTransitionOriginV1::parse(origin.as_str()).expect("origin"),
                origin
            );
        }
        assert!(RadrootsAddressableTransitionOriginV1::parse("unknown").is_err());

        for decision in [
            RadrootsAddressableTransitionRawHeadDecisionV1::BaselineRebuild,
            RadrootsAddressableTransitionRawHeadDecisionV1::Applied,
            RadrootsAddressableTransitionRawHeadDecisionV1::NotHeadSelected,
            RadrootsAddressableTransitionRawHeadDecisionV1::SkippedOlder,
            RadrootsAddressableTransitionRawHeadDecisionV1::SkippedSameTimestampHigherEventId,
            RadrootsAddressableTransitionRawHeadDecisionV1::MalformedCoordinate,
        ] {
            assert_eq!(
                RadrootsAddressableTransitionRawHeadDecisionV1::parse(decision.as_str())
                    .expect("raw-head decision"),
                decision
            );
        }
        assert!(RadrootsAddressableTransitionRawHeadDecisionV1::parse("unknown").is_err());

        for visibility in [
            RadrootsAddressableTransitionVisibilityV1::Visible,
            RadrootsAddressableTransitionVisibilityV1::NotAdmitted,
            RadrootsAddressableTransitionVisibilityV1::Suppressed,
        ] {
            assert_eq!(
                RadrootsAddressableTransitionVisibilityV1::parse(visibility.as_str())
                    .expect("visibility"),
                visibility
            );
        }
        assert!(RadrootsAddressableTransitionVisibilityV1::parse("unknown").is_err());
    }
}
