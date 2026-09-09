use radroots_transport::{
    outcome::{FetchTargetOutcome, FetchTargetState},
    target::{TargetFingerprint, TargetSet},
};

/// Bounded evidence for one requested target across all returned pull pages.
///
/// Counts describe returned, validated pages only. A source failure returning no
/// page adds no target observation; the pull termination retains that failure.
/// No diagnostic messages, event bodies or per-page history are retained here.
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PullTargetSummary {
    target: TargetFingerprint,
    pages_observed: u16,
    incomplete_pages: u16,
    missing_outcome_pages: u16,
    last_incomplete: Option<FetchTargetState>,
}

impl PullTargetSummary {
    /// Exact requested target identity.
    pub const fn target(&self) -> &TargetFingerprint {
        &self.target
    }

    /// Number of validated pages returned during this pull.
    pub const fn pages_observed(&self) -> u16 {
        self.pages_observed
    }

    /// Pages that supplied an explicit non-complete outcome for this target.
    pub const fn incomplete_pages(&self) -> u16 {
        self.incomplete_pages
    }

    /// Pages that supplied no outcome for this requested target.
    pub const fn missing_outcome_pages(&self) -> u16 {
        self.missing_outcome_pages
    }

    /// Last actual non-complete state, preserved across later success or omission.
    pub const fn last_incomplete(&self) -> Option<FetchTargetState> {
        self.last_incomplete
    }

    /// Whether every returned page positively reported this target complete.
    ///
    /// False when no page returned. The caller must also inspect pull termination
    /// and request bounds; this is not a claim about complete global history.
    pub const fn all_pages_complete(&self) -> bool {
        self.pages_observed > 0 && self.incomplete_pages == 0 && self.missing_outcome_pages == 0
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PullTargetSummaries(Vec<PullTargetSummary>);

impl PullTargetSummaries {
    pub(super) fn new(targets: &TargetSet) -> Self {
        Self(
            targets
                .targets()
                .iter()
                .map(|target| PullTargetSummary {
                    target: target.fingerprint().clone(),
                    pages_observed: 0,
                    incomplete_pages: 0,
                    missing_outcome_pages: 0,
                    last_incomplete: None,
                })
                .collect(),
        )
    }

    pub(super) fn as_slice(&self) -> &[PullTargetSummary] {
        &self.0
    }

    pub(super) fn observe(&mut self, outcomes: &[FetchTargetOutcome]) {
        // Only the validated pull loop calls this, at most PULL_MAX_PAGES times.
        for summary in &mut self.0 {
            summary.pages_observed += 1;
            match outcomes
                .iter()
                .find(|outcome| outcome.target() == summary.target())
            {
                Some(outcome) if outcome.state() != FetchTargetState::Complete => {
                    summary.incomplete_pages += 1;
                    summary.last_incomplete = Some(outcome.state());
                }
                Some(_) => {}
                None => summary.missing_outcome_pages += 1,
            }
        }
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for PullTargetSummary {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            target: TargetFingerprint,
            pages_observed: u16,
            incomplete_pages: u16,
            missing_outcome_pages: u16,
            last_incomplete: Option<FetchTargetState>,
        }
        let wire = Wire::deserialize(deserializer)?;
        if wire.pages_observed > super::PULL_MAX_PAGES
            || u32::from(wire.incomplete_pages) + u32::from(wire.missing_outcome_pages)
                > u32::from(wire.pages_observed)
            || (wire.incomplete_pages > 0) != wire.last_incomplete.is_some()
            || wire.last_incomplete == Some(FetchTargetState::Complete)
        {
            return Err(serde::de::Error::custom(
                "invalid pull target summary counts or state",
            ));
        }
        Ok(Self {
            target: wire.target,
            pages_observed: wire.pages_observed,
            incomplete_pages: wire.incomplete_pages,
            missing_outcome_pages: wire.missing_outcome_pages,
            last_incomplete: wire.last_incomplete,
        })
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for PullTargetSummaries {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct BoundedSummaries;
        impl<'de> serde::de::Visitor<'de> for BoundedSummaries {
            type Value = PullTargetSummaries;

            fn expecting(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                formatter.write_str("a nonempty bounded list of unique pull target summaries")
            }

            fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let mut summaries: Vec<PullTargetSummary> = Vec::new();
                while summaries.len() < radroots_transport::target::TARGET_SET_MAX_ITEMS {
                    let Some(summary) = sequence.next_element::<PullTargetSummary>()? else {
                        if summaries.is_empty() {
                            return Err(serde::de::Error::custom("empty pull target summaries"));
                        }
                        return Ok(PullTargetSummaries(summaries));
                    };
                    if summaries
                        .iter()
                        .any(|prior| prior.target() == summary.target())
                    {
                        return Err(serde::de::Error::custom("duplicate pull summary target"));
                    }
                    summaries.push(summary);
                }
                // Skip an extra item without deserializing/retaining its fields.
                if sequence.next_element::<serde::de::IgnoredAny>()?.is_some() {
                    return Err(serde::de::Error::custom("too many pull target summaries"));
                }
                Ok(PullTargetSummaries(summaries))
            }
        }
        deserializer.deserialize_seq(BoundedSummaries)
    }
}
