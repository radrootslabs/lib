use crate::{
    RadrootsTransportDeliveryTargetStatus, RadrootsTransportError, RadrootsTransportOutcome,
    RadrootsTransportOutcomeKind, RadrootsTransportPayload, Target, TargetSet,
    target::TargetFingerprint,
};
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::String;
use alloc::vec::Vec;

pub const RADROOTS_TRANSPORT_DELIVERY_REQUEST_ID_MAX_BYTES: usize = 256;

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

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsTransportSatisfactionPolicy {
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
        targets: Vec<TargetFingerprint>,
    },
}

impl RadrootsTransportSatisfactionPolicy {
    pub fn no_wait() -> Self {
        Self::NoWait
    }

    pub fn any_accepted() -> Self {
        Self::Any {
            class: RadrootsTransportSatisfactionClass::Accepted,
        }
    }

    pub fn all_accepted() -> Self {
        Self::All {
            class: RadrootsTransportSatisfactionClass::Accepted,
        }
    }

    pub fn quorum_accepted(threshold: u16) -> Self {
        Self::Quorum {
            class: RadrootsTransportSatisfactionClass::Accepted,
            threshold,
        }
    }

    pub fn any_forwarded() -> Self {
        Self::Any {
            class: RadrootsTransportSatisfactionClass::Forwarded,
        }
    }

    pub fn all_forwarded() -> Self {
        Self::All {
            class: RadrootsTransportSatisfactionClass::Forwarded,
        }
    }

    pub fn quorum_forwarded(threshold: u16) -> Self {
        Self::Quorum {
            class: RadrootsTransportSatisfactionClass::Forwarded,
            threshold,
        }
    }

    pub fn any_stored() -> Self {
        Self::Any {
            class: RadrootsTransportSatisfactionClass::Stored,
        }
    }

    pub fn all_stored() -> Self {
        Self::All {
            class: RadrootsTransportSatisfactionClass::Stored,
        }
    }

    pub fn quorum_stored(threshold: u16) -> Self {
        Self::Quorum {
            class: RadrootsTransportSatisfactionClass::Stored,
            threshold,
        }
    }

    pub fn any_seen() -> Self {
        Self::Any {
            class: RadrootsTransportSatisfactionClass::Seen,
        }
    }

    pub fn all_seen() -> Self {
        Self::All {
            class: RadrootsTransportSatisfactionClass::Seen,
        }
    }

    pub fn quorum_seen(threshold: u16) -> Self {
        Self::Quorum {
            class: RadrootsTransportSatisfactionClass::Seen,
            threshold,
        }
    }

    pub fn any_delivered() -> Self {
        Self::Any {
            class: RadrootsTransportSatisfactionClass::Delivered,
        }
    }

    pub fn all_delivered() -> Self {
        Self::All {
            class: RadrootsTransportSatisfactionClass::Delivered,
        }
    }

    pub fn quorum_delivered(threshold: u16) -> Self {
        Self::Quorum {
            class: RadrootsTransportSatisfactionClass::Delivered,
            threshold,
        }
    }

    pub fn any_durable_or_observed() -> Self {
        Self::Any {
            class: RadrootsTransportSatisfactionClass::DurableOrObserved,
        }
    }

    pub fn all_durable_or_observed() -> Self {
        Self::All {
            class: RadrootsTransportSatisfactionClass::DurableOrObserved,
        }
    }

    pub fn quorum_durable_or_observed(threshold: u16) -> Self {
        Self::Quorum {
            class: RadrootsTransportSatisfactionClass::DurableOrObserved,
            threshold,
        }
    }

    pub fn required_targets(
        class: RadrootsTransportSatisfactionClass,
        mut targets: Vec<TargetFingerprint>,
    ) -> Result<Self, RadrootsTransportError> {
        validate_required_targets(&targets)?;
        targets.sort();
        Ok(Self::RequiredTargets { class, targets })
    }

    pub fn target_satisfaction_class(&self) -> Option<RadrootsTransportSatisfactionClass> {
        match self {
            Self::NoWait => None,
            Self::Any { class }
            | Self::All { class }
            | Self::Quorum { class, .. }
            | Self::RequiredTargets { class, .. } => Some(*class),
        }
    }

    pub fn required_target_fingerprints(&self) -> Option<&[TargetFingerprint]> {
        match self {
            Self::RequiredTargets { targets, .. } => Some(targets),
            Self::NoWait | Self::Any { .. } | Self::All { .. } | Self::Quorum { .. } => None,
        }
    }

    pub fn required_target_count(
        &self,
        total_targets: usize,
    ) -> Result<usize, RadrootsTransportError> {
        if total_targets == 0 && !matches!(self, Self::NoWait) {
            return Err(RadrootsTransportError::InvalidSatisfactionPolicy);
        }
        match self {
            Self::NoWait => Ok(0),
            Self::All { .. } => Ok(total_targets),
            Self::Any { .. } => Ok(1),
            Self::Quorum { threshold, .. }
                if *threshold > 0 && usize::from(*threshold) <= total_targets =>
            {
                Ok(usize::from(*threshold))
            }
            Self::Quorum { .. } => Err(RadrootsTransportError::InvalidSatisfactionPolicy),
            Self::RequiredTargets { targets, .. } => {
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
        if matches!(self, Self::RequiredTargets { .. }) {
            return Err(RadrootsTransportError::InvalidSatisfactionPolicy);
        }
        let required = self.required_target_count(total_targets)?;
        Ok(satisfied_targets >= required)
    }

    pub fn validate_for_target_set(
        &self,
        target_set: &TargetSet,
    ) -> Result<(), RadrootsTransportError> {
        self.required_target_count(target_set.len())?;
        if let Self::RequiredTargets { targets, .. } = self {
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
#[derive(serde::Deserialize)]
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
        targets: Vec<TargetFingerprint>,
    },
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for RadrootsTransportSatisfactionPolicy {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        match RadrootsTransportSatisfactionPolicyWire::deserialize(deserializer)? {
            RadrootsTransportSatisfactionPolicyWire::NoWait => Ok(Self::NoWait),
            RadrootsTransportSatisfactionPolicyWire::Any { class } => Ok(Self::Any { class }),
            RadrootsTransportSatisfactionPolicyWire::All { class } => Ok(Self::All { class }),
            RadrootsTransportSatisfactionPolicyWire::Quorum { class, threshold }
                if threshold > 0 =>
            {
                Ok(Self::Quorum { class, threshold })
            }
            RadrootsTransportSatisfactionPolicyWire::Quorum { .. } => Err(
                serde::de::Error::custom(RadrootsTransportError::InvalidSatisfactionPolicy),
            ),
            RadrootsTransportSatisfactionPolicyWire::RequiredTargets { class, targets } => {
                Self::required_targets(class, targets).map_err(serde::de::Error::custom)
            }
        }
    }
}

fn validate_required_targets(targets: &[TargetFingerprint]) -> Result<(), RadrootsTransportError> {
    if targets.is_empty() {
        return Err(RadrootsTransportError::EmptyRequiredTargetSet);
    }
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
    target_set: TargetSet,
    satisfaction_policy: RadrootsTransportSatisfactionPolicy,
    now_ms: i64,
}

impl RadrootsTransportDeliveryRequest {
    pub fn new(
        request_id: impl Into<String>,
        payload: RadrootsTransportPayload,
        target_set: TargetSet,
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

    pub fn target_set(&self) -> &TargetSet {
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
    if value != value.trim()
        || value.chars().any(char::is_control)
        || value.len() > RADROOTS_TRANSPORT_DELIVERY_REQUEST_ID_MAX_BYTES
    {
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
    request_id: String,
    payload: RadrootsTransportPayload,
    target_set: TargetSet,
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
    pub target: Target,
    pub attempted: bool,
    pub status: RadrootsTransportDeliveryTargetStatus,
    pub outcome: RadrootsTransportOutcome,
}

impl RadrootsTransportTargetReceipt {
    pub fn new(target: Target, outcome: RadrootsTransportOutcome) -> Self {
        Self::attempted(target, outcome)
    }

    pub fn attempted(target: Target, outcome: RadrootsTransportOutcome) -> Self {
        Self {
            target,
            attempted: true,
            status: outcome.status,
            outcome,
        }
    }

    pub fn skipped(target: Target, outcome: RadrootsTransportOutcome) -> Self {
        Self {
            target,
            attempted: false,
            status: outcome.status,
            outcome,
        }
    }

    fn validate(&self) -> Result<(), RadrootsTransportError> {
        self.outcome.validate()?;
        if self.status != self.outcome.status {
            return Err(RadrootsTransportError::DeliveryTargetReceiptStatusMismatch);
        }
        if !self.attempted
            && self.status.counts_as_accepted_satisfaction()
            && self.outcome.kind != RadrootsTransportOutcomeKind::DuplicateAccepted
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
    target: Target,
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
    target_set: TargetSet,
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
        target_set: TargetSet,
        target_receipts: Vec<RadrootsTransportTargetReceipt>,
    ) -> Result<Self, RadrootsTransportError> {
        let request_id = request_id.into();
        validate_delivery_request_id(request_id.as_str())?;

        let mut receipts_by_fingerprint: BTreeMap<String, RadrootsTransportTargetReceipt> =
            BTreeMap::new();
        for receipt in target_receipts {
            receipt.validate()?;
            let Some(requested_target) = target_set
                .targets()
                .iter()
                .find(|target| target.fingerprint() == receipt.target.fingerprint())
            else {
                return Err(RadrootsTransportError::UnexpectedDeliveryTargetReceipt);
            };
            if requested_target != &receipt.target {
                return Err(RadrootsTransportError::UnexpectedDeliveryTargetReceipt);
            }
            if receipts_by_fingerprint
                .insert(receipt.target.fingerprint().as_str().into(), receipt)
                .is_some()
            {
                return Err(RadrootsTransportError::DuplicateDeliveryTargetReceipt);
            }
        }

        if receipts_by_fingerprint.len() != target_set.len() {
            return Err(RadrootsTransportError::MissingDeliveryTargetReceipt);
        }
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

    pub fn target_set(&self) -> &TargetSet {
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
            .filter(|receipt| receipt.status.counts_as_satisfied(satisfaction_class))
            .count()
    }

    pub fn is_satisfied_by(
        &self,
        policy: &RadrootsTransportSatisfactionPolicy,
    ) -> Result<bool, RadrootsTransportError> {
        policy.validate_for_target_set(&self.target_set)?;
        match policy {
            RadrootsTransportSatisfactionPolicy::NoWait => Ok(true),
            RadrootsTransportSatisfactionPolicy::Any { class }
            | RadrootsTransportSatisfactionPolicy::All { class }
            | RadrootsTransportSatisfactionPolicy::Quorum { class, .. } => {
                policy.is_satisfied_by(self.target_set.len(), self.satisfied_target_count(*class))
            }
            RadrootsTransportSatisfactionPolicy::RequiredTargets { class, targets } => {
                validate_required_targets(targets)?;
                Ok(targets.iter().all(|required| {
                    self.target_receipts.iter().any(|receipt| {
                        receipt.target.fingerprint() == required
                            && receipt.status.counts_as_satisfied(*class)
                    })
                }))
            }
        }
    }
}

#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RadrootsTransportDeliveryReceiptWire {
    request_id: String,
    target_set: TargetSet,
    target_receipts: Vec<RadrootsTransportTargetReceipt>,
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
