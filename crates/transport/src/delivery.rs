use crate::{
    RADROOTS_TRANSPORT_DIAGNOSTIC_MAX_BYTES, RADROOTS_TRANSPORT_IDENTIFIER_MAX_BYTES,
    RADROOTS_TRANSPORT_TARGET_MAX_COUNT, RadrootsTransportDeliveryTargetStatus,
    RadrootsTransportError, RadrootsTransportOutcome, RadrootsTransportOutcomeKind,
    RadrootsTransportPayload, RadrootsTransportTarget, RadrootsTransportTargetFingerprint,
    RadrootsTransportTargetSet,
};
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::String;
use alloc::vec::Vec;

pub const RADROOTS_TRANSPORT_DELIVERY_REQUEST_ID_MAX_BYTES: usize =
    RADROOTS_TRANSPORT_IDENTIFIER_MAX_BYTES;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RadrootsTransportSatisfactionClass {
    Accepted,
    Forwarded,
    Stored,
    Seen,
    Delivered,
    DurableOrObserved,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RadrootsTransportSatisfactionPolicyKind {
    NoWait,
    Any,
    All,
    Quorum,
    RequiredTargets,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTransportSatisfactionPolicy {
    body: RadrootsTransportSatisfactionPolicyBody,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
enum RadrootsTransportSatisfactionPolicyBody {
    NoWait,
    Any {
        class: RadrootsTransportSatisfactionClass,
    },
    All {
        class: RadrootsTransportSatisfactionClass,
    },
    Quorum {
        class: RadrootsTransportSatisfactionClass,
        threshold: u16,
    },
    RequiredTargets {
        class: RadrootsTransportSatisfactionClass,
        targets: Vec<RadrootsTransportTargetFingerprint>,
    },
}

impl RadrootsTransportSatisfactionPolicy {
    pub fn no_wait() -> Self {
        Self {
            body: RadrootsTransportSatisfactionPolicyBody::NoWait,
        }
    }

    pub fn any(class: RadrootsTransportSatisfactionClass) -> Self {
        Self {
            body: RadrootsTransportSatisfactionPolicyBody::Any { class },
        }
    }

    pub fn all(class: RadrootsTransportSatisfactionClass) -> Self {
        Self {
            body: RadrootsTransportSatisfactionPolicyBody::All { class },
        }
    }

    pub fn quorum(
        class: RadrootsTransportSatisfactionClass,
        threshold: u16,
    ) -> Result<Self, RadrootsTransportError> {
        if threshold == 0 || usize::from(threshold) > RADROOTS_TRANSPORT_TARGET_MAX_COUNT {
            return Err(RadrootsTransportError::InvalidSatisfactionPolicy);
        }
        Ok(Self {
            body: RadrootsTransportSatisfactionPolicyBody::Quorum { class, threshold },
        })
    }

    pub fn any_accepted() -> Self {
        Self::any(RadrootsTransportSatisfactionClass::Accepted)
    }

    pub fn all_accepted() -> Self {
        Self::all(RadrootsTransportSatisfactionClass::Accepted)
    }

    pub fn quorum_accepted(threshold: u16) -> Result<Self, RadrootsTransportError> {
        Self::quorum(RadrootsTransportSatisfactionClass::Accepted, threshold)
    }

    pub fn any_forwarded() -> Self {
        Self::any(RadrootsTransportSatisfactionClass::Forwarded)
    }

    pub fn all_forwarded() -> Self {
        Self::all(RadrootsTransportSatisfactionClass::Forwarded)
    }

    pub fn quorum_forwarded(threshold: u16) -> Result<Self, RadrootsTransportError> {
        Self::quorum(RadrootsTransportSatisfactionClass::Forwarded, threshold)
    }

    pub fn any_stored() -> Self {
        Self::any(RadrootsTransportSatisfactionClass::Stored)
    }

    pub fn all_stored() -> Self {
        Self::all(RadrootsTransportSatisfactionClass::Stored)
    }

    pub fn quorum_stored(threshold: u16) -> Result<Self, RadrootsTransportError> {
        Self::quorum(RadrootsTransportSatisfactionClass::Stored, threshold)
    }

    pub fn any_seen() -> Self {
        Self::any(RadrootsTransportSatisfactionClass::Seen)
    }

    pub fn all_seen() -> Self {
        Self::all(RadrootsTransportSatisfactionClass::Seen)
    }

    pub fn quorum_seen(threshold: u16) -> Result<Self, RadrootsTransportError> {
        Self::quorum(RadrootsTransportSatisfactionClass::Seen, threshold)
    }

    pub fn any_delivered() -> Self {
        Self::any(RadrootsTransportSatisfactionClass::Delivered)
    }

    pub fn all_delivered() -> Self {
        Self::all(RadrootsTransportSatisfactionClass::Delivered)
    }

    pub fn quorum_delivered(threshold: u16) -> Result<Self, RadrootsTransportError> {
        Self::quorum(RadrootsTransportSatisfactionClass::Delivered, threshold)
    }

    pub fn any_durable_or_observed() -> Self {
        Self::any(RadrootsTransportSatisfactionClass::DurableOrObserved)
    }

    pub fn all_durable_or_observed() -> Self {
        Self::all(RadrootsTransportSatisfactionClass::DurableOrObserved)
    }

    pub fn quorum_durable_or_observed(threshold: u16) -> Result<Self, RadrootsTransportError> {
        Self::quorum(
            RadrootsTransportSatisfactionClass::DurableOrObserved,
            threshold,
        )
    }

    pub fn required_targets(
        class: RadrootsTransportSatisfactionClass,
        mut targets: Vec<RadrootsTransportTargetFingerprint>,
    ) -> Result<Self, RadrootsTransportError> {
        validate_required_targets(&targets)?;
        targets.sort();
        Ok(Self {
            body: RadrootsTransportSatisfactionPolicyBody::RequiredTargets { class, targets },
        })
    }

    pub fn kind(&self) -> RadrootsTransportSatisfactionPolicyKind {
        match &self.body {
            RadrootsTransportSatisfactionPolicyBody::NoWait => {
                RadrootsTransportSatisfactionPolicyKind::NoWait
            }
            RadrootsTransportSatisfactionPolicyBody::Any { .. } => {
                RadrootsTransportSatisfactionPolicyKind::Any
            }
            RadrootsTransportSatisfactionPolicyBody::All { .. } => {
                RadrootsTransportSatisfactionPolicyKind::All
            }
            RadrootsTransportSatisfactionPolicyBody::Quorum { .. } => {
                RadrootsTransportSatisfactionPolicyKind::Quorum
            }
            RadrootsTransportSatisfactionPolicyBody::RequiredTargets { .. } => {
                RadrootsTransportSatisfactionPolicyKind::RequiredTargets
            }
        }
    }

    pub fn quorum_threshold(&self) -> Option<u16> {
        match &self.body {
            RadrootsTransportSatisfactionPolicyBody::Quorum { threshold, .. } => Some(*threshold),
            RadrootsTransportSatisfactionPolicyBody::NoWait
            | RadrootsTransportSatisfactionPolicyBody::Any { .. }
            | RadrootsTransportSatisfactionPolicyBody::All { .. }
            | RadrootsTransportSatisfactionPolicyBody::RequiredTargets { .. } => None,
        }
    }

    pub fn target_satisfaction_class(&self) -> Option<RadrootsTransportSatisfactionClass> {
        match &self.body {
            RadrootsTransportSatisfactionPolicyBody::NoWait => None,
            RadrootsTransportSatisfactionPolicyBody::Any { class }
            | RadrootsTransportSatisfactionPolicyBody::All { class }
            | RadrootsTransportSatisfactionPolicyBody::Quorum { class, .. }
            | RadrootsTransportSatisfactionPolicyBody::RequiredTargets { class, .. } => {
                Some(*class)
            }
        }
    }

    pub fn required_target_fingerprints(&self) -> Option<&[RadrootsTransportTargetFingerprint]> {
        match &self.body {
            RadrootsTransportSatisfactionPolicyBody::RequiredTargets { targets, .. } => {
                Some(targets)
            }
            RadrootsTransportSatisfactionPolicyBody::NoWait
            | RadrootsTransportSatisfactionPolicyBody::Any { .. }
            | RadrootsTransportSatisfactionPolicyBody::All { .. }
            | RadrootsTransportSatisfactionPolicyBody::Quorum { .. } => None,
        }
    }

    pub fn required_target_count(
        &self,
        total_targets: usize,
    ) -> Result<usize, RadrootsTransportError> {
        if total_targets == 0 && self.kind() != RadrootsTransportSatisfactionPolicyKind::NoWait {
            return Err(RadrootsTransportError::InvalidSatisfactionPolicy);
        }
        match &self.body {
            RadrootsTransportSatisfactionPolicyBody::NoWait => Ok(0),
            RadrootsTransportSatisfactionPolicyBody::All { .. } => Ok(total_targets),
            RadrootsTransportSatisfactionPolicyBody::Any { .. } => Ok(1),
            RadrootsTransportSatisfactionPolicyBody::Quorum { threshold, .. }
                if *threshold > 0 && usize::from(*threshold) <= total_targets =>
            {
                Ok(usize::from(*threshold))
            }
            RadrootsTransportSatisfactionPolicyBody::Quorum { .. } => {
                Err(RadrootsTransportError::InvalidSatisfactionPolicy)
            }
            RadrootsTransportSatisfactionPolicyBody::RequiredTargets { targets, .. } => {
                validate_required_targets(targets)?;
                if targets.len() > total_targets {
                    return Err(RadrootsTransportError::InvalidSatisfactionPolicy);
                }
                Ok(targets.len())
            }
        }
    }

    pub fn is_satisfied_by(
        &self,
        total_targets: usize,
        satisfied_targets: usize,
    ) -> Result<bool, RadrootsTransportError> {
        if self.kind() == RadrootsTransportSatisfactionPolicyKind::RequiredTargets {
            return Err(RadrootsTransportError::InvalidSatisfactionPolicy);
        }
        let required = self.required_target_count(total_targets)?;
        Ok(satisfied_targets >= required)
    }

    pub fn validate_for_target_set(
        &self,
        target_set: &RadrootsTransportTargetSet,
    ) -> Result<(), RadrootsTransportError> {
        self.required_target_count(target_set.len())?;
        if let RadrootsTransportSatisfactionPolicyBody::RequiredTargets { targets, .. } = &self.body
        {
            for required in targets {
                if !target_set
                    .targets()
                    .iter()
                    .any(|target| target.fingerprint() == required)
                {
                    return Err(RadrootsTransportError::RequiredTargetNotRequested);
                }
            }
        }
        Ok(())
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for RadrootsTransportSatisfactionPolicy {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.body.serialize(serializer)
    }
}

#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
enum RadrootsTransportSatisfactionPolicyWire {
    NoWait,
    Any {
        class: RadrootsTransportSatisfactionClass,
    },
    All {
        class: RadrootsTransportSatisfactionClass,
    },
    Quorum {
        class: RadrootsTransportSatisfactionClass,
        threshold: u16,
    },
    RequiredTargets {
        class: RadrootsTransportSatisfactionClass,
        #[serde(deserialize_with = "deserialize_required_target_fingerprints")]
        targets: Vec<RadrootsTransportTargetFingerprint>,
    },
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for RadrootsTransportSatisfactionPolicy {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        match RadrootsTransportSatisfactionPolicyWire::deserialize(deserializer)? {
            RadrootsTransportSatisfactionPolicyWire::NoWait => Ok(Self::no_wait()),
            RadrootsTransportSatisfactionPolicyWire::Any { class } => Ok(Self::any(class)),
            RadrootsTransportSatisfactionPolicyWire::All { class } => Ok(Self::all(class)),
            RadrootsTransportSatisfactionPolicyWire::Quorum { class, threshold } => {
                Self::quorum(class, threshold).map_err(serde::de::Error::custom)
            }
            RadrootsTransportSatisfactionPolicyWire::RequiredTargets { class, targets } => {
                Self::required_targets(class, targets).map_err(serde::de::Error::custom)
            }
        }
    }
}

#[cfg(feature = "serde")]
fn deserialize_required_target_fingerprints<'de, D>(
    deserializer: D,
) -> Result<Vec<RadrootsTransportTargetFingerprint>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    crate::serde_bounds::deserialize_vec(
        deserializer,
        "required_target_count",
        RADROOTS_TRANSPORT_TARGET_MAX_COUNT,
    )
}

fn validate_required_targets(
    targets: &[RadrootsTransportTargetFingerprint],
) -> Result<(), RadrootsTransportError> {
    if targets.is_empty() {
        return Err(RadrootsTransportError::EmptyRequiredTargetSet);
    }
    crate::limits::ensure_resource_limit(
        "required_target_count",
        targets.len(),
        RADROOTS_TRANSPORT_TARGET_MAX_COUNT,
    )?;
    let mut fingerprints = BTreeSet::new();
    for target in targets {
        if !fingerprints.insert(target.as_str()) {
            return Err(RadrootsTransportError::DuplicateRequiredTargetFingerprint);
        }
    }
    Ok(())
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTransportDeliveryRequest {
    request_id: String,
    payload: RadrootsTransportPayload,
    target_set: RadrootsTransportTargetSet,
    satisfaction_policy: RadrootsTransportSatisfactionPolicy,
    now_ms: i64,
}

impl RadrootsTransportDeliveryRequest {
    pub fn new(
        request_id: impl Into<String>,
        payload: RadrootsTransportPayload,
        target_set: RadrootsTransportTargetSet,
        satisfaction_policy: RadrootsTransportSatisfactionPolicy,
    ) -> Result<Self, RadrootsTransportError> {
        let request_id = request_id.into();
        validate_delivery_request_id(request_id.as_str())?;
        payload.validate()?;
        satisfaction_policy.validate_for_target_set(&target_set)?;
        Ok(Self {
            request_id,
            payload,
            target_set,
            satisfaction_policy,
            now_ms: 0,
        })
    }

    pub fn try_with_now_ms(mut self, now_ms: i64) -> Result<Self, RadrootsTransportError> {
        validate_delivery_timestamp(now_ms)?;
        self.now_ms = now_ms;
        Ok(self)
    }

    pub fn request_id(&self) -> &str {
        self.request_id.as_str()
    }

    pub fn payload(&self) -> &RadrootsTransportPayload {
        &self.payload
    }

    pub fn target_set(&self) -> &RadrootsTransportTargetSet {
        &self.target_set
    }

    pub fn satisfaction_policy(&self) -> &RadrootsTransportSatisfactionPolicy {
        &self.satisfaction_policy
    }

    pub fn now_ms(&self) -> i64 {
        self.now_ms
    }
}

fn validate_delivery_request_id(value: &str) -> Result<(), RadrootsTransportError> {
    if value.is_empty() {
        return Err(RadrootsTransportError::EmptyDeliveryRequestId);
    }
    crate::limits::ensure_resource_limit(
        "delivery_request_id",
        value.len(),
        RADROOTS_TRANSPORT_DELIVERY_REQUEST_ID_MAX_BYTES,
    )?;
    if value != value.trim() || value.chars().any(char::is_control) {
        return Err(RadrootsTransportError::InvalidDeliveryRequestId);
    }
    Ok(())
}

fn validate_delivery_timestamp(now_ms: i64) -> Result<(), RadrootsTransportError> {
    if now_ms < 0 {
        return Err(RadrootsTransportError::InvalidDeliveryTimestamp);
    }
    Ok(())
}

#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RadrootsTransportDeliveryRequestWire {
    #[serde(deserialize_with = "deserialize_delivery_request_id")]
    request_id: String,
    payload: RadrootsTransportPayload,
    target_set: RadrootsTransportTargetSet,
    satisfaction_policy: RadrootsTransportSatisfactionPolicy,
    now_ms: i64,
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for RadrootsTransportDeliveryRequest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = RadrootsTransportDeliveryRequestWire::deserialize(deserializer)?;
        Self::new(
            wire.request_id,
            wire.payload,
            wire.target_set,
            wire.satisfaction_policy,
        )
        .and_then(|request| request.try_with_now_ms(wire.now_ms))
        .map_err(serde::de::Error::custom)
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTransportTargetReceipt {
    target: RadrootsTransportTarget,
    attempted: bool,
    status: RadrootsTransportDeliveryTargetStatus,
    outcome: RadrootsTransportOutcome,
}

impl RadrootsTransportTargetReceipt {
    pub fn new(target: RadrootsTransportTarget, outcome: RadrootsTransportOutcome) -> Self {
        Self::attempted(target, outcome)
    }

    pub fn attempted(target: RadrootsTransportTarget, outcome: RadrootsTransportOutcome) -> Self {
        Self {
            target,
            attempted: true,
            status: outcome.status(),
            outcome,
        }
    }

    pub fn skipped(
        target: RadrootsTransportTarget,
        outcome: RadrootsTransportOutcome,
    ) -> Result<Self, RadrootsTransportError> {
        let receipt = Self {
            target,
            attempted: false,
            status: outcome.status(),
            outcome,
        };
        receipt.validate()?;
        Ok(receipt)
    }

    pub fn target(&self) -> &RadrootsTransportTarget {
        &self.target
    }

    pub fn was_attempted(&self) -> bool {
        self.attempted
    }

    pub fn status(&self) -> RadrootsTransportDeliveryTargetStatus {
        self.status
    }

    pub fn outcome(&self) -> &RadrootsTransportOutcome {
        &self.outcome
    }

    pub(crate) fn validate(&self) -> Result<(), RadrootsTransportError> {
        self.outcome.validate()?;
        if self.status != self.outcome.status() {
            return Err(RadrootsTransportError::DeliveryTargetReceiptStatusMismatch);
        }
        if !self.attempted
            && self.status.counts_as_accepted_satisfaction()
            && self.outcome.kind() != RadrootsTransportOutcomeKind::DuplicateAccepted
        {
            return Err(RadrootsTransportError::DeliveryTargetReceiptAttemptMismatch);
        }
        Ok(())
    }
}

#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RadrootsTransportTargetReceiptWire {
    target: RadrootsTransportTarget,
    attempted: bool,
    status: RadrootsTransportDeliveryTargetStatus,
    outcome: RadrootsTransportOutcome,
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for RadrootsTransportTargetReceipt {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = RadrootsTransportTargetReceiptWire::deserialize(deserializer)?;
        let receipt = Self {
            target: wire.target,
            attempted: wire.attempted,
            status: wire.status,
            outcome: wire.outcome,
        };
        receipt.validate().map_err(serde::de::Error::custom)?;
        Ok(receipt)
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTransportDeliveryReceipt {
    request_id: String,
    target_set: RadrootsTransportTargetSet,
    target_receipts: Vec<RadrootsTransportTargetReceipt>,
}

impl RadrootsTransportDeliveryReceipt {
    pub fn for_request(
        request: &RadrootsTransportDeliveryRequest,
        target_receipts: Vec<RadrootsTransportTargetReceipt>,
    ) -> Result<Self, RadrootsTransportError> {
        Self::new(
            request.request_id(),
            request.target_set().clone(),
            target_receipts,
        )
    }

    pub fn new(
        request_id: impl Into<String>,
        target_set: RadrootsTransportTargetSet,
        target_receipts: Vec<RadrootsTransportTargetReceipt>,
    ) -> Result<Self, RadrootsTransportError> {
        let request_id = request_id.into();
        validate_delivery_request_id(request_id.as_str())?;
        crate::limits::ensure_resource_limit(
            "delivery_target_receipt_count",
            target_receipts.len(),
            RADROOTS_TRANSPORT_TARGET_MAX_COUNT,
        )?;

        let mut receipt_fingerprints = BTreeSet::new();
        let mut diagnostic_bytes = 0usize;
        for receipt in &target_receipts {
            receipt.validate()?;
            diagnostic_bytes = diagnostic_bytes
                .checked_add(receipt.outcome().message().map_or(0, str::len))
                .ok_or(RadrootsTransportError::ResourceLimitExceeded {
                    field: "delivery_diagnostic_bytes",
                    max: RADROOTS_TRANSPORT_DIAGNOSTIC_MAX_BYTES,
                    actual: usize::MAX,
                })?;
            crate::limits::ensure_resource_limit(
                "delivery_diagnostic_bytes",
                diagnostic_bytes,
                RADROOTS_TRANSPORT_DIAGNOSTIC_MAX_BYTES,
            )?;
            let Some(requested_target) = target_set
                .targets()
                .iter()
                .find(|target| target.fingerprint() == receipt.target().fingerprint())
            else {
                return Err(RadrootsTransportError::UnexpectedDeliveryTargetReceipt);
            };
            if requested_target != receipt.target() {
                return Err(RadrootsTransportError::UnexpectedDeliveryTargetReceipt);
            }
            if !receipt_fingerprints.insert(receipt.target().fingerprint().as_str()) {
                return Err(RadrootsTransportError::DuplicateDeliveryTargetReceipt);
            }
        }

        if receipt_fingerprints.len() != target_set.len() {
            return Err(RadrootsTransportError::MissingDeliveryTargetReceipt);
        }
        let mut receipts_by_fingerprint = target_receipts
            .into_iter()
            .map(|receipt| {
                (
                    String::from(receipt.target().fingerprint().as_str()),
                    receipt,
                )
            })
            .collect::<BTreeMap<_, _>>();
        let target_receipts = target_set
            .targets()
            .iter()
            .map(|target| {
                receipts_by_fingerprint
                    .remove(target.fingerprint().as_str())
                    .ok_or(RadrootsTransportError::MissingDeliveryTargetReceipt)
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            request_id,
            target_set,
            target_receipts,
        })
    }

    pub fn request_id(&self) -> &str {
        self.request_id.as_str()
    }

    pub fn target_set(&self) -> &RadrootsTransportTargetSet {
        &self.target_set
    }

    pub fn target_receipts(&self) -> &[RadrootsTransportTargetReceipt] {
        &self.target_receipts
    }

    pub fn validate_for_request(
        &self,
        request: &RadrootsTransportDeliveryRequest,
    ) -> Result<(), RadrootsTransportError> {
        if self.request_id() != request.request_id() {
            return Err(RadrootsTransportError::DeliveryReceiptRequestIdMismatch);
        }
        if self.target_set() != request.target_set() {
            return Err(RadrootsTransportError::DeliveryReceiptTargetSetMismatch);
        }
        Ok(())
    }

    pub fn satisfied_target_count(
        &self,
        satisfaction_class: RadrootsTransportSatisfactionClass,
    ) -> usize {
        self.target_receipts
            .iter()
            .filter(|receipt| receipt.status().counts_as_satisfied(satisfaction_class))
            .count()
    }

    pub fn is_satisfied_by(
        &self,
        policy: &RadrootsTransportSatisfactionPolicy,
    ) -> Result<bool, RadrootsTransportError> {
        policy.validate_for_target_set(&self.target_set)?;
        if policy.kind() == RadrootsTransportSatisfactionPolicyKind::NoWait {
            return Ok(true);
        }
        let class = policy
            .target_satisfaction_class()
            .ok_or(RadrootsTransportError::InvalidSatisfactionPolicy)?;
        let Some(targets) = policy.required_target_fingerprints() else {
            return policy
                .is_satisfied_by(self.target_set.len(), self.satisfied_target_count(class));
        };
        validate_required_targets(targets)?;
        Ok(targets.iter().all(|required| {
            self.target_receipts.iter().any(|receipt| {
                receipt.target().fingerprint() == required
                    && receipt.status().counts_as_satisfied(class)
            })
        }))
    }
}

#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RadrootsTransportDeliveryReceiptWire {
    #[serde(deserialize_with = "deserialize_delivery_request_id")]
    request_id: String,
    target_set: RadrootsTransportTargetSet,
    #[serde(deserialize_with = "deserialize_delivery_target_receipts")]
    target_receipts: Vec<RadrootsTransportTargetReceipt>,
}

#[cfg(feature = "serde")]
fn deserialize_delivery_request_id<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    crate::serde_bounds::deserialize_string(
        deserializer,
        "delivery_request_id",
        RADROOTS_TRANSPORT_DELIVERY_REQUEST_ID_MAX_BYTES,
    )
}

#[cfg(feature = "serde")]
fn deserialize_delivery_target_receipts<'de, D>(
    deserializer: D,
) -> Result<Vec<RadrootsTransportTargetReceipt>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    crate::serde_bounds::deserialize_vec(
        deserializer,
        "delivery_target_receipt_count",
        RADROOTS_TRANSPORT_TARGET_MAX_COUNT,
    )
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for RadrootsTransportDeliveryReceipt {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = RadrootsTransportDeliveryReceiptWire::deserialize(deserializer)?;
        Self::new(wire.request_id, wire.target_set, wire.target_receipts)
            .map_err(serde::de::Error::custom)
    }
}
