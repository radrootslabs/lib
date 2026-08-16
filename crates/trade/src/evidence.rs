//! Evidence supplied explicitly to deterministic trade reduction.
//!
//! These values describe observed mutation, private-term, and attestation
//! records. They perform no retrieval, signature verification, or decryption.

use core::fmt;

/// Maximum number of evidence sources admitted by the v1 coverage evaluator.
pub const RADROOTS_TRADE_EVIDENCE_MAXIMUM_SOURCE_COUNT: usize = 16;

/// Maximum admitted event count represented for one v1 evidence source.
pub const RADROOTS_TRADE_EVIDENCE_MAXIMUM_EVENTS_PER_SOURCE: u32 = 4_096;

/// Evidence coverage established for one governed reconciliation scope.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum RadrootsTradeEvidenceCoverageV1 {
    /// No source completed and no relevant evidence was admitted.
    #[default]
    Missing,
    /// Some relevant completion or evidence exists, but required scope is not satisfied.
    Partial,
    /// Every required source completed and every required scope prerequisite is satisfied.
    ScopeSatisfied,
    /// At least one required source cannot evaluate the governed scope.
    Unsupported,
}

impl RadrootsTradeEvidenceCoverageV1 {
    /// Returns whether this coverage permits the requested outcome.
    pub const fn permits(self, outcome: RadrootsTradeEvidenceOutcomeV1) -> bool {
        matches!(outcome, RadrootsTradeEvidenceOutcomeV1::Indeterminate)
            || matches!(self, Self::ScopeSatisfied)
    }
}

/// Result of evaluating a claim against one immutable evidence scope.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum RadrootsTradeEvidenceOutcomeV1 {
    Valid,
    Invalid,
    #[default]
    Indeterminate,
}

/// Completion evidence retained for one configured source.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum RadrootsTradeEvidenceSourceCompletionV1 {
    Complete,
    #[default]
    Incomplete,
    Unsupported,
}

/// Whether one configured evidence source is required by the governed scope.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum RadrootsTradeEvidenceSourceRequirementV1 {
    Required,
    Optional,
}

/// Whether every non-source prerequisite for the governed scope is satisfied.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum RadrootsTradeEvidenceScopePrerequisitesV1 {
    Satisfied,
    Unsatisfied,
}

/// Bounded source facts consumed by the v1 coverage evaluator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RadrootsTradeEvidenceSourceResultV1 {
    requirement: RadrootsTradeEvidenceSourceRequirementV1,
    completion: RadrootsTradeEvidenceSourceCompletionV1,
    admitted_event_count: u32,
}

impl RadrootsTradeEvidenceSourceResultV1 {
    pub const fn new(
        requirement: RadrootsTradeEvidenceSourceRequirementV1,
        completion: RadrootsTradeEvidenceSourceCompletionV1,
        admitted_event_count: u32,
    ) -> Result<Self, RadrootsTradeEvidenceCoverageError> {
        if admitted_event_count > RADROOTS_TRADE_EVIDENCE_MAXIMUM_EVENTS_PER_SOURCE {
            return Err(RadrootsTradeEvidenceCoverageError::AdmittedEventCountOutOfRange);
        }
        Ok(Self {
            requirement,
            completion,
            admitted_event_count,
        })
    }

    pub const fn requirement(&self) -> RadrootsTradeEvidenceSourceRequirementV1 {
        self.requirement
    }

    pub const fn completion(&self) -> RadrootsTradeEvidenceSourceCompletionV1 {
        self.completion
    }

    pub const fn admitted_event_count(&self) -> u32 {
        self.admitted_event_count
    }
}

/// Stable failures produced before coverage can be classified.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsTradeEvidenceCoverageError {
    SourceCountOutOfRange,
    NoRequiredSource,
    AdmittedEventCountOutOfRange,
}

impl fmt::Display for RadrootsTradeEvidenceCoverageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::SourceCountOutOfRange => "evidence source count is out of range",
            Self::NoRequiredSource => "evidence scope has no required source",
            Self::AdmittedEventCountOutOfRange => {
                "evidence source admitted event count is out of range"
            }
        })
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsTradeEvidenceCoverageError {}

/// Classifies bounded source results using the governed v1 coverage precedence.
///
/// The iterator is consumed only through the maximum source count plus one, so
/// an excessive or infinite input terminates with `SourceCountOutOfRange`.
pub fn classify_trade_evidence_coverage_v1(
    sources: impl IntoIterator<Item = RadrootsTradeEvidenceSourceResultV1>,
    scope_prerequisites: RadrootsTradeEvidenceScopePrerequisitesV1,
) -> Result<RadrootsTradeEvidenceCoverageV1, RadrootsTradeEvidenceCoverageError> {
    let mut source_count = 0_usize;
    let mut has_required_source = false;
    let mut required_source_unsupported = false;
    let mut every_required_source_complete = true;
    let mut any_source_complete = false;
    let mut any_relevant_evidence = false;

    for source in sources {
        source_count += 1;
        if source_count > RADROOTS_TRADE_EVIDENCE_MAXIMUM_SOURCE_COUNT {
            return Err(RadrootsTradeEvidenceCoverageError::SourceCountOutOfRange);
        }

        if matches!(
            source.requirement,
            RadrootsTradeEvidenceSourceRequirementV1::Required
        ) {
            has_required_source = true;
            required_source_unsupported |= matches!(
                source.completion,
                RadrootsTradeEvidenceSourceCompletionV1::Unsupported
            );
            every_required_source_complete &= matches!(
                source.completion,
                RadrootsTradeEvidenceSourceCompletionV1::Complete
            );
        }
        any_source_complete |= matches!(
            source.completion,
            RadrootsTradeEvidenceSourceCompletionV1::Complete
        );
        any_relevant_evidence |= source.admitted_event_count > 0;
    }

    if source_count == 0 {
        return Err(RadrootsTradeEvidenceCoverageError::SourceCountOutOfRange);
    }
    if !has_required_source {
        return Err(RadrootsTradeEvidenceCoverageError::NoRequiredSource);
    }

    Ok(if required_source_unsupported {
        RadrootsTradeEvidenceCoverageV1::Unsupported
    } else if every_required_source_complete
        && matches!(
            scope_prerequisites,
            RadrootsTradeEvidenceScopePrerequisitesV1::Satisfied
        )
    {
        RadrootsTradeEvidenceCoverageV1::ScopeSatisfied
    } else if any_source_complete || any_relevant_evidence {
        RadrootsTradeEvidenceCoverageV1::Partial
    } else {
        RadrootsTradeEvidenceCoverageV1::Missing
    })
}

pub use crate::trade_contract_v1::{
    RadrootsTradeAttestationRecordV1, RadrootsTradeAttestationResultV1,
    RadrootsTradeEvidenceStateV1, RadrootsTradeMutationRecordV1,
    RadrootsTradePrivateTermsEvidenceV1,
};

#[cfg(feature = "json")]
pub use crate::evidence_manifest::{
    RADROOTS_TRADE_EVIDENCE_MANIFEST_CONTRACT_ID,
    RADROOTS_TRADE_EVIDENCE_MANIFEST_CONTRACT_VERSION,
    RADROOTS_TRADE_EVIDENCE_MANIFEST_MAXIMUM_BYTES,
    RADROOTS_TRADE_EVIDENCE_MANIFEST_MAXIMUM_OBSERVATIONS,
    RADROOTS_TRADE_EVIDENCE_SOURCE_ID_MAXIMUM_BYTES, RadrootsTradeEvidenceManifestDigestV1,
    RadrootsTradeEvidenceManifestError, RadrootsTradeEvidenceManifestObservationV1,
    RadrootsTradeEvidenceManifestSourceResultV1, RadrootsTradeEvidenceManifestV1,
    RadrootsTradeEvidencePolicyDigestV1, RadrootsTradeEvidenceProvenanceDigestV1,
    RadrootsTradeEvidenceSourceIdV1, RadrootsTradeEvidenceSourceResultDigestV1,
    RadrootsTradeSignedEventDigestV1,
};

#[cfg(feature = "json")]
pub use crate::evidence_report::{
    RADROOTS_RHI_EVIDENCE_ATTESTATION_METHOD, RADROOTS_RHI_EVIDENCE_REPORT_CONTRACT_ID,
    RADROOTS_RHI_EVIDENCE_REPORT_CONTRACT_VERSION,
    RADROOTS_RHI_EVIDENCE_REPORT_MAXIMUM_CANONICAL_BYTES,
    RADROOTS_RHI_EVIDENCE_REPORT_MAXIMUM_REASON_CODES,
    RADROOTS_RHI_EVIDENCE_REPORT_REASON_CODE_MAXIMUM_BYTES, RadrootsRhiEvidenceReasonCodeV1,
    RadrootsRhiEvidenceReportError, RadrootsRhiEvidenceReportV1,
    RadrootsRhiEvidenceStatementDigestV1, RadrootsRhiEvidenceSupersessionV1,
    RadrootsTradeEvidenceProjectionDigestV1,
};

#[cfg(test)]
mod tests {
    #[cfg(not(feature = "std"))]
    use alloc::{format, string::ToString, vec};

    use super::*;

    fn source(
        required: bool,
        completion: RadrootsTradeEvidenceSourceCompletionV1,
        admitted_event_count: u32,
    ) -> RadrootsTradeEvidenceSourceResultV1 {
        let requirement = if required {
            RadrootsTradeEvidenceSourceRequirementV1::Required
        } else {
            RadrootsTradeEvidenceSourceRequirementV1::Optional
        };
        RadrootsTradeEvidenceSourceResultV1::new(requirement, completion, admitted_event_count)
            .expect("valid source result")
    }

    #[test]
    fn coverage_precedence_matches_the_governed_vectors() {
        use RadrootsTradeEvidenceCoverageV1::{Missing, Partial, ScopeSatisfied, Unsupported};
        use RadrootsTradeEvidenceSourceCompletionV1::{
            Complete, Incomplete, Unsupported as SourceUnsupported,
        };

        let vectors = [
            (vec![source(true, SourceUnsupported, 0)], false, Unsupported),
            (vec![source(true, Incomplete, 0)], false, Missing),
            (
                vec![source(true, Incomplete, 0), source(false, Complete, 1)],
                false,
                Partial,
            ),
            (vec![source(true, Complete, 1)], false, Partial),
            (
                vec![source(true, Complete, 1), source(false, Incomplete, 0)],
                true,
                ScopeSatisfied,
            ),
        ];

        for (sources, prerequisites, expected) in vectors {
            let prerequisites = if prerequisites {
                RadrootsTradeEvidenceScopePrerequisitesV1::Satisfied
            } else {
                RadrootsTradeEvidenceScopePrerequisitesV1::Unsatisfied
            };
            assert_eq!(
                classify_trade_evidence_coverage_v1(sources, prerequisites),
                Ok(expected)
            );
        }
    }

    #[test]
    fn optional_sources_cannot_substitute_for_required_completion() {
        use RadrootsTradeEvidenceCoverageV1::{Missing, Partial};
        use RadrootsTradeEvidenceSourceCompletionV1::{Complete, Incomplete, Unsupported};

        assert_eq!(
            classify_trade_evidence_coverage_v1(
                [source(true, Incomplete, 0), source(false, Unsupported, 0)],
                RadrootsTradeEvidenceScopePrerequisitesV1::Satisfied,
            ),
            Ok(Missing)
        );
        assert_eq!(
            classify_trade_evidence_coverage_v1(
                [source(true, Incomplete, 0), source(false, Complete, 0)],
                RadrootsTradeEvidenceScopePrerequisitesV1::Satisfied,
            ),
            Ok(Partial)
        );
    }

    #[test]
    fn coverage_and_outcome_matrix_fails_closed() {
        use RadrootsTradeEvidenceCoverageV1::{Missing, Partial, ScopeSatisfied, Unsupported};
        use RadrootsTradeEvidenceOutcomeV1::{Indeterminate, Invalid, Valid};

        for coverage in [Missing, Partial, Unsupported] {
            assert!(coverage.permits(Indeterminate));
            assert!(!coverage.permits(Valid));
            assert!(!coverage.permits(Invalid));
        }
        for outcome in [Valid, Invalid, Indeterminate] {
            assert!(ScopeSatisfied.permits(outcome));
        }
        assert_eq!(RadrootsTradeEvidenceCoverageV1::default(), Missing);
        assert_eq!(RadrootsTradeEvidenceOutcomeV1::default(), Indeterminate);
    }

    #[test]
    fn source_and_event_bounds_are_enforced_before_unbounded_ingestion() {
        use RadrootsTradeEvidenceCoverageError::{
            AdmittedEventCountOutOfRange, NoRequiredSource, SourceCountOutOfRange,
        };
        use RadrootsTradeEvidenceSourceCompletionV1::Complete;

        let maximum = source(
            true,
            Complete,
            RADROOTS_TRADE_EVIDENCE_MAXIMUM_EVENTS_PER_SOURCE,
        );
        assert_eq!(maximum.admitted_event_count(), 4_096);
        assert_eq!(
            RadrootsTradeEvidenceSourceResultV1::new(
                RadrootsTradeEvidenceSourceRequirementV1::Required,
                Complete,
                4_097,
            ),
            Err(AdmittedEventCountOutOfRange)
        );

        assert_eq!(
            classify_trade_evidence_coverage_v1(
                core::iter::empty(),
                RadrootsTradeEvidenceScopePrerequisitesV1::Satisfied,
            ),
            Err(SourceCountOutOfRange)
        );
        assert_eq!(
            classify_trade_evidence_coverage_v1(
                [source(false, Complete, 0)],
                RadrootsTradeEvidenceScopePrerequisitesV1::Satisfied,
            ),
            Err(NoRequiredSource)
        );
        assert_eq!(
            classify_trade_evidence_coverage_v1(
                core::iter::repeat(source(true, Complete, 0)),
                RadrootsTradeEvidenceScopePrerequisitesV1::Satisfied,
            ),
            Err(SourceCountOutOfRange)
        );
        assert_eq!(
            classify_trade_evidence_coverage_v1(
                core::iter::repeat_n(
                    source(true, Complete, 0),
                    RADROOTS_TRADE_EVIDENCE_MAXIMUM_SOURCE_COUNT,
                ),
                RadrootsTradeEvidenceScopePrerequisitesV1::Satisfied,
            ),
            Ok(RadrootsTradeEvidenceCoverageV1::ScopeSatisfied)
        );
    }

    #[cfg(feature = "json")]
    #[test]
    fn coverage_and_outcome_wire_values_are_exact() {
        use RadrootsTradeEvidenceCoverageV1::{Missing, Partial, ScopeSatisfied, Unsupported};
        use RadrootsTradeEvidenceOutcomeV1::{Indeterminate, Invalid, Valid};

        for (value, wire) in [
            (Missing, "\"missing\""),
            (Partial, "\"partial\""),
            (ScopeSatisfied, "\"scope_satisfied\""),
            (Unsupported, "\"unsupported\""),
        ] {
            assert_eq!(serde_json::to_string(&value).unwrap(), wire);
            assert_eq!(
                serde_json::from_str::<RadrootsTradeEvidenceCoverageV1>(wire).unwrap(),
                value
            );
        }
        for (value, wire) in [
            (Valid, "\"valid\""),
            (Invalid, "\"invalid\""),
            (Indeterminate, "\"indeterminate\""),
        ] {
            assert_eq!(serde_json::to_string(&value).unwrap(), wire);
            assert_eq!(
                serde_json::from_str::<RadrootsTradeEvidenceOutcomeV1>(wire).unwrap(),
                value
            );
        }
        for rejected in ["\"complete\"", "\"query_partial\"", "null", "{}"] {
            assert!(serde_json::from_str::<RadrootsTradeEvidenceCoverageV1>(rejected).is_err());
        }
    }

    #[test]
    fn coverage_errors_are_stable_and_source_free() {
        for (error, message) in [
            (
                RadrootsTradeEvidenceCoverageError::SourceCountOutOfRange,
                "evidence source count is out of range",
            ),
            (
                RadrootsTradeEvidenceCoverageError::NoRequiredSource,
                "evidence scope has no required source",
            ),
            (
                RadrootsTradeEvidenceCoverageError::AdmittedEventCountOutOfRange,
                "evidence source admitted event count is out of range",
            ),
        ] {
            assert_eq!(error.to_string(), message);
            assert!(!format!("{error:?}").contains("source_id"));
            #[cfg(feature = "std")]
            assert!(std::error::Error::source(&error).is_none());
        }
    }
}
