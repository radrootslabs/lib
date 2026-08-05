//! Transport-neutral delivery satisfaction policy.

use crate::{
    Error,
    outcome::DeliveryOutcome,
    target::{TargetFingerprint, TargetSet},
};
use alloc::{collections::BTreeMap, vec::Vec};

/// Current result of evaluating delivery evidence against one exact policy.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SatisfactionState {
    /// The evidence already satisfies the policy.
    Satisfied,
    /// The policy is not satisfied, but unattempted or retryable work can satisfy it.
    Pending,
    /// The available evidence proves that the policy can no longer be satisfied.
    Exhausted,
}

/// Success level a caller requires from selected targets.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SatisfactionClass {
    /// The target accepted responsibility for the event.
    Accepted,
    /// The target confirmed final delivery.
    Delivered,
}

/// Which requested targets must reach the satisfaction class.
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TargetPolicy {
    kind: TargetPolicyKind,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Debug, Eq, PartialEq)]
enum TargetPolicyKind {
    Any,
    All,
    Quorum(u16),
    Required(Vec<TargetFingerprint>),
}

impl TargetPolicy {
    /// Any one requested target must satisfy the class.
    pub const fn any() -> Self {
        Self {
            kind: TargetPolicyKind::Any,
        }
    }

    /// Every requested target must satisfy the class.
    pub const fn all() -> Self {
        Self {
            kind: TargetPolicyKind::All,
        }
    }

    /// At least this non-zero count of requested targets must satisfy the class.
    pub const fn quorum(threshold: u16) -> Result<Self, Error> {
        if threshold == 0 {
            return Err(Error::InvalidSatisfactionPolicy);
        }
        Ok(Self {
            kind: TargetPolicyKind::Quorum(threshold),
        })
    }

    /// These exact, unique target fingerprints must satisfy the class.
    pub fn required(mut targets: Vec<TargetFingerprint>) -> Result<Self, Error> {
        if targets.is_empty() {
            return Err(Error::EmptyRequiredTargetSet);
        }
        targets.sort();
        if targets.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(Error::DuplicateRequiredTargetFingerprint);
        }
        Ok(Self {
            kind: TargetPolicyKind::Required(targets),
        })
    }

    /// Returns exact required fingerprints for a required-target policy.
    pub fn required_targets(&self) -> Option<&[TargetFingerprint]> {
        match &self.kind {
            TargetPolicyKind::Required(targets) => Some(targets.as_slice()),
            TargetPolicyKind::Any | TargetPolicyKind::All | TargetPolicyKind::Quorum(_) => None,
        }
    }

    /// Returns whether any requested target may satisfy this policy.
    pub const fn is_any(&self) -> bool {
        matches!(self.kind, TargetPolicyKind::Any)
    }

    /// Returns whether every requested target must satisfy this policy.
    pub const fn is_all(&self) -> bool {
        matches!(self.kind, TargetPolicyKind::All)
    }

    /// Returns the threshold for a quorum policy.
    pub const fn quorum_threshold(&self) -> Option<u16> {
        match self.kind {
            TargetPolicyKind::Quorum(threshold) => Some(threshold),
            TargetPolicyKind::Any | TargetPolicyKind::All | TargetPolicyKind::Required(_) => None,
        }
    }

    pub(crate) fn validate_for(&self, targets: &TargetSet) -> Result<(), Error> {
        match &self.kind {
            TargetPolicyKind::Any | TargetPolicyKind::All => Ok(()),
            TargetPolicyKind::Quorum(threshold) => {
                if usize::from(*threshold) > targets.len() {
                    Err(Error::InvalidSatisfactionPolicy)
                } else {
                    Ok(())
                }
            }
            TargetPolicyKind::Required(required) => {
                if required
                    .iter()
                    .any(|fingerprint| !targets.contains(fingerprint))
                {
                    Err(Error::RequiredTargetNotRequested)
                } else {
                    Ok(())
                }
            }
        }
    }

    fn is_satisfied(&self, total_targets: usize, satisfied: usize, fingerprints: &[&str]) -> bool {
        match &self.kind {
            TargetPolicyKind::Any => satisfied != 0,
            TargetPolicyKind::All => satisfied == total_targets,
            TargetPolicyKind::Quorum(threshold) => satisfied >= usize::from(*threshold),
            TargetPolicyKind::Required(required) => required
                .iter()
                .all(|fingerprint| fingerprints.contains(&fingerprint.as_str())),
        }
    }
}

/// Required success level and target selection for one delivery request.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SatisfactionPolicy {
    class: SatisfactionClass,
    targets: TargetPolicy,
}

impl SatisfactionPolicy {
    /// Creates an explicit satisfaction policy.
    pub const fn new(class: SatisfactionClass, targets: TargetPolicy) -> Self {
        Self { class, targets }
    }

    /// Returns the accepted or delivered success level.
    pub const fn class(&self) -> SatisfactionClass {
        self.class
    }

    /// Returns the selected target policy.
    pub const fn targets(&self) -> &TargetPolicy {
        &self.targets
    }

    /// Validates this policy against an exact bounded target set.
    ///
    /// This permits higher-level composition layers to reject impossible
    /// quorum and required-target profiles before constructing a delivery
    /// request, without reproducing transport policy law.
    pub fn validate_for(&self, targets: &TargetSet) -> Result<(), Error> {
        self.targets.validate_for(targets)
    }
}

/// Evaluates target evidence using the transport-owned satisfaction law.
///
/// Evidence is ordered from oldest to newest when a target occurs more than
/// once. A prior success remains authoritative; otherwise the newest outcome
/// determines whether the target can be retried. Targets without evidence are
/// pending. Evidence for a target outside `targets` is rejected.
pub fn evaluate_satisfaction<'a, I>(
    policy: &SatisfactionPolicy,
    targets: &TargetSet,
    evidence: I,
) -> Result<SatisfactionState, Error>
where
    I: IntoIterator<Item = (&'a TargetFingerprint, &'a DeliveryOutcome)>,
{
    policy.validate_for(targets)?;
    let mut states: BTreeMap<&str, (bool, bool)> = targets
        .targets()
        .iter()
        .map(|target| (target.fingerprint().as_str(), (false, true)))
        .collect();

    for (target, outcome) in evidence {
        outcome.validate()?;
        let Some((satisfied, retryable)) = states.get_mut(target.as_str()) else {
            return Err(Error::UnexpectedDeliveryTargetReceipt);
        };
        if outcome.satisfies(policy.class()) {
            *satisfied = true;
            *retryable = false;
        } else if !*satisfied {
            *retryable = outcome.is_retryable();
        }
    }

    let satisfied_targets: Vec<&str> = states
        .iter()
        .filter_map(|(target, (satisfied, _))| satisfied.then_some(*target))
        .collect();
    if policy.targets.is_satisfied(
        targets.len(),
        satisfied_targets.len(),
        satisfied_targets.as_slice(),
    ) {
        return Ok(SatisfactionState::Satisfied);
    }

    let possible_targets: Vec<&str> = states
        .iter()
        .filter_map(|(target, (satisfied, retryable))| {
            (*satisfied || *retryable).then_some(*target)
        })
        .collect();
    if policy.targets.is_satisfied(
        targets.len(),
        possible_targets.len(),
        possible_targets.as_slice(),
    ) {
        Ok(SatisfactionState::Pending)
    } else {
        Ok(SatisfactionState::Exhausted)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for TargetPolicy {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        match TargetPolicyKind::deserialize(deserializer)? {
            TargetPolicyKind::Any => Ok(Self::any()),
            TargetPolicyKind::All => Ok(Self::all()),
            TargetPolicyKind::Quorum(threshold) => {
                Self::quorum(threshold).map_err(serde::de::Error::custom)
            }
            TargetPolicyKind::Required(targets) => {
                Self::required(targets).map_err(serde::de::Error::custom)
            }
        }
    }
}
