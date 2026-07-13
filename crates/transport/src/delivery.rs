use crate::{
    RadrootsTransportDeliveryTargetStatus, RadrootsTransportError, RadrootsTransportOutcome,
    RadrootsTransportPayload, RadrootsTransportTarget, RadrootsTransportTargetFingerprint,
    RadrootsTransportTargetSet,
};
use alloc::collections::BTreeSet;
use alloc::string::String;
use alloc::vec::Vec;

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

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
        targets: Vec<RadrootsTransportTargetFingerprint>,
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
        mut targets: Vec<RadrootsTransportTargetFingerprint>,
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

    pub fn required_target_fingerprints(&self) -> Option<&[RadrootsTransportTargetFingerprint]> {
        match self {
            Self::RequiredTargets { targets, .. } => Some(targets),
            Self::NoWait | Self::Any { .. } | Self::All { .. } | Self::Quorum { .. } => None,
        }
    }

    pub fn required_target_count(
        &self,
        total_targets: usize,
    ) -> Result<usize, RadrootsTransportError> {
        if matches!(self, Self::NoWait) {
            return Ok(0);
        }
        if total_targets == 0 {
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
}

fn validate_required_targets(
    targets: &[RadrootsTransportTargetFingerprint],
) -> Result<(), RadrootsTransportError> {
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

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTransportDeliveryRequest {
    pub request_id: String,
    pub payload: RadrootsTransportPayload,
    pub target_set: RadrootsTransportTargetSet,
    pub satisfaction_policy: RadrootsTransportSatisfactionPolicy,
    pub now_ms: i64,
}

impl RadrootsTransportDeliveryRequest {
    pub fn new(
        request_id: impl Into<String>,
        payload: RadrootsTransportPayload,
        target_set: RadrootsTransportTargetSet,
        satisfaction_policy: RadrootsTransportSatisfactionPolicy,
    ) -> Self {
        Self {
            request_id: request_id.into(),
            payload,
            target_set,
            satisfaction_policy,
            now_ms: 0,
        }
    }

    pub fn with_now_ms(mut self, now_ms: i64) -> Self {
        self.now_ms = now_ms;
        self
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTransportTargetReceipt {
    pub target: RadrootsTransportTarget,
    pub status: RadrootsTransportDeliveryTargetStatus,
    pub outcome: RadrootsTransportOutcome,
}

impl RadrootsTransportTargetReceipt {
    pub fn new(target: RadrootsTransportTarget, outcome: RadrootsTransportOutcome) -> Self {
        Self {
            target,
            status: outcome.status,
            outcome,
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTransportDeliveryReceipt {
    pub request_id: String,
    pub target_receipts: Vec<RadrootsTransportTargetReceipt>,
}

impl RadrootsTransportDeliveryReceipt {
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
        match policy {
            RadrootsTransportSatisfactionPolicy::NoWait => Ok(true),
            RadrootsTransportSatisfactionPolicy::Any { class }
            | RadrootsTransportSatisfactionPolicy::All { class }
            | RadrootsTransportSatisfactionPolicy::Quorum { class, .. } => policy.is_satisfied_by(
                self.target_receipts.len(),
                self.satisfied_target_count(*class),
            ),
            RadrootsTransportSatisfactionPolicy::RequiredTargets { class, targets } => {
                validate_required_targets(targets)?;
                Ok(targets.iter().all(|required| {
                    self.target_receipts.iter().any(|receipt| {
                        receipt.target.fingerprint == *required
                            && receipt.status.counts_as_satisfied(*class)
                    })
                }))
            }
        }
    }
}
