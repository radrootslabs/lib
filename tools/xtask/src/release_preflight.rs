//! Closed, non-short-circuit aggregation for the release preflight.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum LaneId {
    Catalog,
    ServiceSourceLockContract,
    ServiceBuildQualificationContract,
    ServiceReleaseArtifactsContract,
    PublicNativeGroup,
    PreviewGroup,
    ToolsGroup,
    DtoRoots,
    ProtocolFreshness,
    ArtifactContracts,
    ReleaseContracts,
}

impl LaneId {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Catalog => "catalog",
            Self::ServiceSourceLockContract => "service_source_lock_contract",
            Self::ServiceBuildQualificationContract => "service_build_qualification_contract",
            Self::ServiceReleaseArtifactsContract => "service_release_artifacts_contract",
            Self::PublicNativeGroup => "public_native_group",
            Self::PreviewGroup => "preview_group",
            Self::ToolsGroup => "tools_group",
            Self::DtoRoots => "dto_roots",
            Self::ProtocolFreshness => "protocol_freshness",
            Self::ArtifactContracts => "artifact_contracts",
            Self::ReleaseContracts => "release_contracts",
        }
    }
}

pub(crate) const REQUIRED_LANES: [LaneId; 11] = [
    LaneId::Catalog,
    LaneId::ServiceSourceLockContract,
    LaneId::ServiceBuildQualificationContract,
    LaneId::ServiceReleaseArtifactsContract,
    LaneId::PublicNativeGroup,
    LaneId::PreviewGroup,
    LaneId::ToolsGroup,
    LaneId::DtoRoots,
    LaneId::ProtocolFreshness,
    LaneId::ArtifactContracts,
    LaneId::ReleaseContracts,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LaneState {
    Failed,
    Interrupted,
    Pass,
    Skipped,
    Unavailable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LaneOutcome {
    id: String,
    state: LaneState,
}

impl LaneOutcome {
    pub(crate) fn required(id: LaneId, state: LaneState) -> Self {
        Self {
            id: id.as_str().to_owned(),
            state,
        }
    }

    fn named(id: &str, state: LaneState) -> Self {
        Self {
            id: id.to_owned(),
            state,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreflightReport {
    outcomes: Vec<LaneOutcome>,
    missing: Vec<&'static str>,
    duplicate: Vec<String>,
    unexpected: Vec<String>,
    ordered: bool,
}

impl PreflightReport {
    pub(crate) fn outcomes(&self) -> &[LaneOutcome] {
        &self.outcomes
    }

    pub(crate) fn is_pass(&self) -> bool {
        self.missing.is_empty()
            && self.duplicate.is_empty()
            && self.unexpected.is_empty()
            && self.ordered
            && self.outcomes.len() == REQUIRED_LANES.len()
            && self
                .outcomes
                .iter()
                .all(|outcome| outcome.state == LaneState::Pass)
    }
}

#[derive(Debug)]
pub(crate) struct PreflightError {
    report: PreflightReport,
}

impl PreflightError {
    pub(crate) fn report(&self) -> &PreflightReport {
        &self.report
    }
}

impl fmt::Display for PreflightError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("release preflight required lanes did not all pass")
    }
}

impl std::error::Error for PreflightError {}

pub(crate) fn execute_all<F>(mut execute: F) -> Result<PreflightReport, PreflightError>
where
    F: FnMut(LaneId) -> LaneState,
{
    let outcomes = REQUIRED_LANES
        .into_iter()
        .map(|lane| LaneOutcome::required(lane, execute(lane)))
        .collect();
    close(outcomes)
}

pub(crate) fn close(outcomes: Vec<LaneOutcome>) -> Result<PreflightReport, PreflightError> {
    let required = REQUIRED_LANES
        .into_iter()
        .map(LaneId::as_str)
        .collect::<BTreeSet<_>>();
    let mut counts = BTreeMap::<String, usize>::new();
    for outcome in &outcomes {
        *counts.entry(outcome.id.clone()).or_default() += 1;
    }
    let missing = required
        .iter()
        .copied()
        .filter(|lane| !counts.contains_key(*lane))
        .collect::<Vec<_>>();
    let duplicate = counts
        .iter()
        .filter(|(_, count)| **count > 1)
        .map(|(lane, _)| lane.clone())
        .collect::<Vec<_>>();
    let unexpected = counts
        .keys()
        .filter(|lane| !required.contains(lane.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    let ordered = outcomes
        .iter()
        .map(|outcome| outcome.id.as_str())
        .eq(REQUIRED_LANES.into_iter().map(LaneId::as_str));
    let report = PreflightReport {
        outcomes,
        missing,
        duplicate,
        unexpected,
        ordered,
    };
    if report.is_pass() {
        Ok(report)
    } else {
        Err(PreflightError { report })
    }
}

pub(crate) fn self_test() -> Result<(), String> {
    let mut attempted = Vec::new();
    execute_all(|lane| {
        attempted.push(lane);
        LaneState::Pass
    })
    .map_err(|error| error.to_string())?;
    if attempted != REQUIRED_LANES {
        return Err("release preflight lane inventory self-test failed".to_owned());
    }

    for state in [
        LaneState::Failed,
        LaneState::Interrupted,
        LaneState::Skipped,
        LaneState::Unavailable,
    ] {
        let mut attempted = Vec::new();
        let result = execute_all(|lane| {
            attempted.push(lane);
            if lane == LaneId::Catalog {
                state
            } else {
                LaneState::Pass
            }
        });
        let error = result
            .err()
            .ok_or_else(|| "release preflight exhaustion self-test failed".to_owned())?;
        if attempted != REQUIRED_LANES || error.report().outcomes().len() != REQUIRED_LANES.len() {
            return Err("release preflight exhaustion self-test failed".to_owned());
        }
    }

    let all_pass = || {
        REQUIRED_LANES
            .into_iter()
            .map(|lane| LaneOutcome::required(lane, LaneState::Pass))
            .collect::<Vec<_>>()
    };
    let mut missing = all_pass();
    missing.pop();
    let mut duplicate = all_pass();
    duplicate.push(LaneOutcome::required(LaneId::Catalog, LaneState::Pass));
    let mut unexpected = all_pass();
    unexpected.push(LaneOutcome::named("not_governed", LaneState::Pass));
    let mut reordered = all_pass();
    reordered.swap(0, 1);
    if [missing, duplicate, unexpected, reordered]
        .into_iter()
        .any(|outcomes| close(outcomes).is_ok())
    {
        return Err("release preflight closure self-test failed".to_owned());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_pass() -> Vec<LaneOutcome> {
        REQUIRED_LANES
            .into_iter()
            .map(|lane| LaneOutcome::required(lane, LaneState::Pass))
            .collect()
    }

    #[test]
    fn exact_inventory_and_order_are_closed() {
        assert_eq!(
            REQUIRED_LANES.map(LaneId::as_str),
            [
                "catalog",
                "service_source_lock_contract",
                "service_build_qualification_contract",
                "service_release_artifacts_contract",
                "public_native_group",
                "preview_group",
                "tools_group",
                "dto_roots",
                "protocol_freshness",
                "artifact_contracts",
                "release_contracts",
            ]
        );
        assert!(close(all_pass()).expect("all pass").is_pass());
    }

    #[test]
    fn every_nonpass_state_fails_closed_without_short_circuiting() {
        for state in [
            LaneState::Failed,
            LaneState::Interrupted,
            LaneState::Skipped,
            LaneState::Unavailable,
        ] {
            let mut attempted = Vec::new();
            let error = execute_all(|lane| {
                attempted.push(lane);
                if lane == LaneId::Catalog {
                    state
                } else {
                    LaneState::Pass
                }
            })
            .expect_err("nonpass lane");
            assert_eq!(attempted, REQUIRED_LANES);
            assert_eq!(error.report().outcomes().len(), REQUIRED_LANES.len());
        }
    }

    #[test]
    fn missing_duplicate_and_unexpected_lanes_fail_closed() {
        let mut missing = all_pass();
        missing.pop();
        assert!(close(missing).is_err());

        let mut duplicate = all_pass();
        duplicate.push(LaneOutcome::required(LaneId::Catalog, LaneState::Pass));
        assert!(close(duplicate).is_err());

        let mut unexpected = all_pass();
        unexpected.push(LaneOutcome::named("not_governed", LaneState::Pass));
        assert!(close(unexpected).is_err());

        let mut reordered = all_pass();
        reordered.swap(0, 1);
        assert!(close(reordered).is_err());
    }

    #[test]
    fn aggregate_diagnostics_are_static() {
        let error = close(vec![LaneOutcome::named(
            "secret-path-or-command",
            LaneState::Failed,
        )])
        .expect_err("invalid aggregate");
        let diagnostic = error.to_string();
        assert_eq!(
            diagnostic,
            "release preflight required lanes did not all pass"
        );
        assert!(!diagnostic.contains("secret"));
    }
}
