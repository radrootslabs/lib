//! Transport-neutral delivery satisfaction policy.

use crate::{
    Error,
    target::{TargetFingerprint, TargetSet},
};
use alloc::{collections::BTreeSet, vec::Vec};

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
                let requested: BTreeSet<&str> = targets
                    .targets()
                    .iter()
                    .map(|target| target.fingerprint().as_str())
                    .collect();
                if required
                    .iter()
                    .any(|fingerprint| !requested.contains(fingerprint.as_str()))
                {
                    Err(Error::RequiredTargetNotRequested)
                } else {
                    Ok(())
                }
            }
        }
    }

    pub(crate) fn is_satisfied(&self, total_targets: usize, satisfied: &BTreeSet<&str>) -> bool {
        match &self.kind {
            TargetPolicyKind::Any => !satisfied.is_empty(),
            TargetPolicyKind::All => satisfied.len() == total_targets,
            TargetPolicyKind::Quorum(threshold) => satisfied.len() >= usize::from(*threshold),
            TargetPolicyKind::Required(required) => required
                .iter()
                .all(|fingerprint| satisfied.contains(fingerprint.as_str())),
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

    pub(crate) fn validate_for(&self, targets: &TargetSet) -> Result<(), Error> {
        self.targets.validate_for(targets)
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
