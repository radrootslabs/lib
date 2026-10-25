//! Exact portable local coverage data. Neither a digest nor a complete supplied
//! graph proves authentic store coverage, participant permission or order state.
use super::{EncodedRecord, RecordWireError};
use alloc::{collections::BTreeSet, vec::Vec};
use core::fmt;
use radroots_domain::{
    CheckpointDigest, CheckpointRef, Coverage, Sequence, SourceId,
    causal::{self, CausalAnalysis, CausalRecord, CausalScope},
};
use sha2::{Digest, Sha256};

/// Checked canonical identity/graph data and an optional complete local scope
/// descriptor. Constructors never grant read, execute, signer or stock rights.
pub struct CapturedCausalCoverage {
    analysis: CausalAnalysis,
    checkpoint: Option<CheckpointRef>,
}
impl CapturedCausalCoverage {
    /// Checks every complete canonical record identity before deriving the
    /// descriptor. External identities and completeness are still caller claims;
    /// concrete stores must independently authenticate them and their coverage.
    pub fn capture(
        scope: CausalScope,
        records: &[CausalRecord<'_>],
        external: &[SourceId],
        uninterpreted: &[SourceId],
    ) -> Result<Self, RecordWireError> {
        let analysis = causal::analyze(scope, records, external, uninterpreted).map_err(
            |error| match error {
                causal::CausalError::LimitExceeded => RecordWireError::LimitExceeded,
                causal::CausalError::IdentityConflict => RecordWireError::IdentityMismatch,
                _ => RecordWireError::InvalidInput,
            },
        )?;
        let mut known = BTreeSet::new();
        let mut required = BTreeSet::<SourceId>::new();
        for record in records {
            if EncodedRecord::capture(record.content())?.record_id() != record.record_id() {
                return Err(RecordWireError::IdentityMismatch);
            }
            known.insert(SourceId::from_bytes(record.record_id().into_bytes()));
            required.extend(record.content().header().parents().iter().copied());
        }
        let checkpoint = if analysis.readiness() == causal::CausalReadiness::Complete {
            let external: BTreeSet<_> = external
                .iter()
                .copied()
                .filter(|id| required.contains(id) && !known.contains(id))
                .collect();
            let barriers: BTreeSet<_> = uninterpreted.iter().copied().collect();
            let mut hash = Sha256::new();
            hash.update(b"radroots.commerce.coverage.v1\0");
            // Fixed-width scope fields, then three length-prefixed sorted sets.
            // No host word size, JSON map ordering or transport encoding enters
            // this transcript. Public record IDs commit complete content.
            hash.update(scope.thread_id().as_bytes());
            hash.update(scope.order_id().as_bytes());
            hash.update(scope.buyer().as_bytes());
            hash.update(scope.seller().as_bytes());
            for identities in [&known, &external, &barriers] {
                hash.update((identities.len() as u64).to_be_bytes());
                for id in identities {
                    hash.update(id.as_bytes());
                }
            }
            let sequence = Sequence::new((known.len() + external.len() + barriers.len()) as u64)
                .map_err(|_| RecordWireError::LimitExceeded)?;
            Some(
                CheckpointRef::new(
                    CheckpointDigest::from_bytes(hash.finalize().into()),
                    sequence,
                    Coverage::ScopeSatisfied,
                    Vec::new(),
                )
                .map_err(|_| RecordWireError::InvalidInput)?,
            )
        } else {
            None
        };
        Ok(Self {
            analysis,
            checkpoint,
        })
    }
    /// Borrows exact known gaps/forks and local causal classification.
    pub const fn analysis(&self) -> &CausalAnalysis {
        &self.analysis
    }
    /// Returns a complete supplied scope descriptor; it grants no permission.
    pub const fn checkpoint(&self) -> Option<&CheckpointRef> {
        self.checkpoint.as_ref()
    }
    /// Moves the checked data without promoting it to authenticated evidence.
    pub fn into_parts(self) -> (CausalAnalysis, Option<CheckpointRef>) {
        (self.analysis, self.checkpoint)
    }
}
impl fmt::Debug for CapturedCausalCoverage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CapturedCausalCoverage(<local coverage claims>)")
    }
}
