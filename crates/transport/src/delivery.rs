use crate::{
    RadrootsTransportDeliveryTargetStatus, RadrootsTransportError, RadrootsTransportOutcome,
    RadrootsTransportTarget, RadrootsTransportTargetSet,
};
use alloc::string::String;
use alloc::vec::Vec;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsTransportSatisfactionPolicy {
    AllTargets,
    AnyTarget,
    AtLeast(u16),
}

impl RadrootsTransportSatisfactionPolicy {
    pub fn required_target_count(
        &self,
        total_targets: usize,
    ) -> Result<usize, RadrootsTransportError> {
        if total_targets == 0 {
            return Err(RadrootsTransportError::InvalidSatisfactionPolicy);
        }
        match self {
            Self::AllTargets => Ok(total_targets),
            Self::AnyTarget => Ok(1),
            Self::AtLeast(count) if *count > 0 && usize::from(*count) <= total_targets => {
                Ok(usize::from(*count))
            }
            Self::AtLeast(_) => Err(RadrootsTransportError::InvalidSatisfactionPolicy),
        }
    }

    pub fn is_satisfied_by(
        &self,
        total_targets: usize,
        satisfied_targets: usize,
    ) -> Result<bool, RadrootsTransportError> {
        let required = self.required_target_count(total_targets)?;
        Ok(satisfied_targets >= required)
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTransportDeliveryRequest {
    pub request_id: String,
    pub payload_digest: String,
    pub target_set: RadrootsTransportTargetSet,
    pub satisfaction_policy: RadrootsTransportSatisfactionPolicy,
}

impl RadrootsTransportDeliveryRequest {
    pub fn new(
        request_id: impl Into<String>,
        payload_digest: impl Into<String>,
        target_set: RadrootsTransportTargetSet,
        satisfaction_policy: RadrootsTransportSatisfactionPolicy,
    ) -> Self {
        Self {
            request_id: request_id.into(),
            payload_digest: payload_digest.into(),
            target_set,
            satisfaction_policy,
        }
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
    pub fn satisfied_target_count(&self) -> usize {
        self.target_receipts
            .iter()
            .filter(|receipt| receipt.status.counts_as_satisfied())
            .count()
    }
}
