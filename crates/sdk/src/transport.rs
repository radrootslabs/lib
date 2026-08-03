//! Explicit user-facing transport profile composition.
//!
//! Profiles retain canonical `radroots_transport` identities, targets,
//! policies, and statuses. They select no adapter implicitly and never replace
//! an unavailable selection with another transport.

use radroots_transport::{
    Error, SinkStatus, SourceStatus, TargetSet, TransportId,
    capability::{Availability, Maturity, SinkCapabilities, SourceCapabilities},
    policy::SatisfactionPolicy,
};

const PREVIEW_UNAVAILABLE_MESSAGE: &str = "preview transport is unavailable in this SDK release";

/// A side-effect-free transport selection for a client operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Profile {
    selection: Selection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Selection {
    LocalOnly,
    Delivery {
        targets: TargetSet,
        satisfaction: SatisfactionPolicy,
    },
    UnavailablePreview {
        source: SourceStatus,
        sink: SinkStatus,
    },
}

impl Profile {
    /// Selects local persistence only, with no transport target or fallback.
    #[must_use]
    pub const fn local_only() -> Self {
        Self {
            selection: Selection::LocalOnly,
        }
    }

    /// Selects an exact bounded target set and canonical satisfaction policy.
    ///
    /// Impossible quorum and required-target policies are rejected here by the
    /// owning transport contract. Construction performs no network operation.
    pub fn delivery(targets: TargetSet, satisfaction: SatisfactionPolicy) -> Result<Self, Error> {
        satisfaction.validate_for(&targets)?;
        Ok(Self {
            selection: Selection::Delivery {
                targets,
                satisfaction,
            },
        })
    }

    /// Describes a preview transport that is intentionally not selectable.
    ///
    /// Both canonical capability directions remain explicitly unconfigured
    /// and unavailable. The profile has no targets and therefore cannot fall
    /// back to local, Nostr, daemon, or another transport.
    #[must_use]
    pub fn unavailable_preview(transport_id: TransportId) -> Self {
        Self {
            selection: Selection::UnavailablePreview {
                source: SourceStatus::new(
                    transport_id,
                    false,
                    Maturity::Preview,
                    Availability::Unavailable,
                    SourceCapabilities::NONE,
                    PREVIEW_UNAVAILABLE_MESSAGE,
                ),
                sink: SinkStatus::new(
                    transport_id,
                    false,
                    Maturity::Preview,
                    Availability::Unavailable,
                    SinkCapabilities::NONE,
                    PREVIEW_UNAVAILABLE_MESSAGE,
                ),
            },
        }
    }

    /// Returns whether this profile authorizes no transport operation.
    #[must_use]
    pub const fn is_local_only(&self) -> bool {
        matches!(self.selection, Selection::LocalOnly)
    }

    /// Returns the exact selected targets, if delivery is authorized.
    #[must_use]
    pub const fn targets(&self) -> Option<&TargetSet> {
        match &self.selection {
            Selection::Delivery { targets, .. } => Some(targets),
            Selection::LocalOnly | Selection::UnavailablePreview { .. } => None,
        }
    }

    /// Returns the exact selected satisfaction policy, if delivery is authorized.
    #[must_use]
    pub const fn satisfaction(&self) -> Option<&SatisfactionPolicy> {
        match &self.selection {
            Selection::Delivery { satisfaction, .. } => Some(satisfaction),
            Selection::LocalOnly | Selection::UnavailablePreview { .. } => None,
        }
    }

    /// Returns canonical source status for an unavailable preview.
    #[must_use]
    pub const fn source_status(&self) -> Option<&SourceStatus> {
        match &self.selection {
            Selection::UnavailablePreview { source, .. } => Some(source),
            Selection::LocalOnly | Selection::Delivery { .. } => None,
        }
    }

    /// Returns canonical sink status for an unavailable preview.
    #[must_use]
    pub const fn sink_status(&self) -> Option<&SinkStatus> {
        match &self.selection {
            Selection::UnavailablePreview { sink, .. } => Some(sink),
            Selection::LocalOnly | Selection::Delivery { .. } => None,
        }
    }
}

impl Default for Profile {
    fn default() -> Self {
        Self::local_only()
    }
}

#[cfg(test)]
mod tests {
    use radroots_transport::{
        Error, TARGET_SET_MAX_ITEMS, Target,
        capability::{Availability, Maturity},
        policy::{SatisfactionClass, SatisfactionPolicy, TargetPolicy},
        target::TargetFingerprint,
    };

    use super::*;

    fn target(index: usize) -> Target {
        Target::nostr_relay(format!("wss://relay-{index}.example")).expect("target")
    }

    #[test]
    fn delivery_profile_preserves_canonical_targets_and_policy() {
        let targets = TargetSet::new(vec![target(1), target(2)]).expect("target set");
        let policy = SatisfactionPolicy::new(SatisfactionClass::Accepted, TargetPolicy::all());
        let profile = Profile::delivery(targets.clone(), policy.clone()).expect("profile");

        assert_eq!(profile.targets(), Some(&targets));
        assert_eq!(profile.satisfaction(), Some(&policy));
        assert!(!profile.is_local_only());
        assert!(profile.source_status().is_none());
        assert!(profile.sink_status().is_none());
    }

    #[test]
    fn canonical_target_and_policy_bounds_fail_during_profile_construction() {
        assert_eq!(TargetSet::new(Vec::new()), Err(Error::EmptyTargetSet));
        assert_eq!(
            TargetSet::new((0..=TARGET_SET_MAX_ITEMS).map(target).collect()),
            Err(Error::TargetSetTooLarge)
        );

        let targets = TargetSet::new(vec![target(1)]).expect("target set");
        let quorum = SatisfactionPolicy::new(
            SatisfactionClass::Delivered,
            TargetPolicy::quorum(2).expect("non-zero quorum"),
        );
        assert_eq!(
            Profile::delivery(targets.clone(), quorum),
            Err(Error::InvalidSatisfactionPolicy)
        );

        let missing =
            TargetFingerprint::from_target(target(2).kind(), target(2).uri(), target(2).scope());
        let required = SatisfactionPolicy::new(
            SatisfactionClass::Accepted,
            TargetPolicy::required(vec![missing]).expect("required policy"),
        );
        assert_eq!(
            Profile::delivery(targets, required),
            Err(Error::RequiredTargetNotRequested)
        );
    }

    #[test]
    fn preview_transport_is_explicitly_unavailable_and_unselectable() {
        let profile = Profile::unavailable_preview(TransportId::RETICULUM);
        let source = profile.source_status().expect("source status");
        let sink = profile.sink_status().expect("sink status");

        assert_eq!(source.transport_id(), TransportId::RETICULUM);
        assert_eq!(sink.transport_id(), TransportId::RETICULUM);
        assert!(!source.is_configured());
        assert!(!sink.is_configured());
        assert_eq!(source.maturity(), Maturity::Preview);
        assert_eq!(sink.maturity(), Maturity::Preview);
        assert_eq!(source.availability(), Availability::Unavailable);
        assert_eq!(sink.availability(), Availability::Unavailable);
        assert!(!source.capabilities().can_fetch());
        assert!(!sink.capabilities().can_deliver());
        assert!(profile.targets().is_none());
        assert!(profile.satisfaction().is_none());
    }

    #[test]
    fn local_and_preview_profiles_never_substitute_fallback_targets() {
        let local = Profile::local_only();
        let preview = Profile::unavailable_preview(TransportId::RETICULUM);
        assert!(local.is_local_only());
        assert!(local.targets().is_none());
        assert!(preview.targets().is_none());

        let selected = TargetSet::new(vec![target(7)]).expect("selected targets");
        let profile = Profile::delivery(
            selected.clone(),
            SatisfactionPolicy::new(SatisfactionClass::Accepted, TargetPolicy::any()),
        )
        .expect("profile");
        assert_eq!(profile.targets(), Some(&selected));
        assert!(
            profile
                .targets()
                .expect("targets")
                .targets()
                .iter()
                .all(|target| *target.kind() == TransportId::NOSTR)
        );
    }
}
