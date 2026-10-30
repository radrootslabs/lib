//! Replacement authorization claims. Actual source authentication, canonical
//! terms hashing, current installed state and stock effects belong to execution.
use super::*;
use crate::{AgreementCommitment, CommercePayload, Terms, TermsDigest, UnixSeconds};

/// Explicit existing-agreement/proposal/authorization selectors and an
/// independently computed complete terms digest. Construction proves no claim.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct ReplacementTermsSelection {
    base: RecordId,
    proposal: RecordId,
    authorization: RecordId,
    digest: TermsDigest,
}
impl ReplacementTermsSelection {
    /// Captures exact identities only. The consumer must recompute the digest
    /// from the actual complete proposal and compare the live installed head.
    pub const fn new(
        base: RecordId,
        proposal: RecordId,
        authorization: RecordId,
        digest: TermsDigest,
    ) -> Self {
        Self {
            base,
            proposal,
            authorization,
            digest,
        }
    }
    /// Explicit existing agreement, not proof that it is the current head.
    pub const fn base_agreement(self) -> RecordId {
        self.base
    }
    /// Exact selected complete replacement proposal.
    pub const fn proposal(self) -> RecordId {
        self.proposal
    }
    /// Exact selected counterparty authorization.
    pub const fn authorization(self) -> RecordId {
        self.authorization
    }
    /// Declared expected digest; this value performs no canonical hashing.
    pub const fn terms_digest(self) -> TermsDigest {
        self.digest
    }
}
impl fmt::Debug for ReplacementTermsSelection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ReplacementTermsSelection(<private identities>)")
    }
}

/// Borrowed replacement claims after bounded local causal/time checks. This
/// grants no authenticity, consent, current authority, residual or stock effect.
pub struct ReplacementTermsClaim<'a> {
    selection: ReplacementTermsSelection,
    base: AgreementCommitment,
    terms: &'a Terms,
    expires: UnixSeconds,
}
impl ReplacementTermsClaim<'_> {
    /// Exact selected identities and declared terms digest.
    pub const fn selection(&self) -> ReplacementTermsSelection {
        self.selection
    }
    /// Original base commitment claim, not current executor authority.
    pub const fn base_commitment(&self) -> AgreementCommitment {
        self.base
    }
    /// Complete proposed terms; execution must independently hash these bytes.
    pub const fn terms(&self) -> &Terms {
        self.terms
    }
    /// Earliest proposal/authorization deadline, excluding executor bounds.
    pub const fn expires_at(&self) -> UnixSeconds {
        self.expires
    }
}
impl fmt::Debug for ReplacementTermsClaim<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ReplacementTermsClaim(<private data>)")
    }
}

/// Bounded input-free failures of the selected replacement claims.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ReplacementTermsError {
    /// Source bounds, identities or the declared bilateral scope differ.
    InvalidGraph,
    /// Required original records or causal predecessors are absent.
    MissingHistory,
    /// Known author forks or cycles prevent a complete scoped claim.
    Conflicted,
    /// Retained authenticated uninterpreted evidence prevents reduction.
    Uninterpreted,
    /// The selected base is not a seller agreement commitment in this scope.
    WrongBase,
    /// The proposal does not identify the exact selected existing agreement.
    WrongProposal,
    /// The authorization names another proposal or is authored by its proposer.
    WrongAuthorization,
    /// The authorization differs from the supplied complete terms digest.
    TermsDigestMismatch,
    /// The current observation is at or beyond either selected deadline.
    Expired,
    /// A known scoped author time is ahead of the supplied observation.
    ClockAhead,
    /// The author withdrew the selected authorization.
    Withdrawn,
    /// The counterparty rejected the selected proposal.
    Rejected,
    /// Another live authorization for this base declares incompatible terms.
    ConflictingAuthorizations,
}
impl fmt::Display for ReplacementTermsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidGraph => "invalid replacement graph",
            Self::MissingHistory => "replacement history missing",
            Self::Conflicted => "replacement history conflicted",
            Self::Uninterpreted => "replacement history uninterpreted",
            Self::WrongBase => "replacement base differs",
            Self::WrongProposal => "replacement proposal differs",
            Self::WrongAuthorization => "replacement authorization differs",
            Self::TermsDigestMismatch => "replacement terms digest differs",
            Self::Expired => "replacement deadline expired",
            Self::ClockAhead => "replacement clock observation insufficient",
            Self::Withdrawn => "replacement authorization withdrawn",
            Self::Rejected => "replacement proposal rejected",
            Self::ConflictingAuthorizations => "replacement authorizations conflict",
        })
    }
}
impl core::error::Error for ReplacementTermsError {}

/// Checks one explicit replacement proposal/authorization against the complete
/// supplied local graph, with either participant permitted to propose. Generic
/// external sources cannot substitute for the three selected original records.
/// Losing observation or expiry does not erase an already prepared operation;
/// recover such an operation before using this fresh-claim planner.
///
/// The caller must authenticate all sources, independently recompute the actual
/// complete proposal digest, and revalidate current installed agreement, residual,
/// permissions, original allocations and pending-operation fences in its native
/// transaction. This pure result does not select a current agreement or acquire,
/// release, consume, restore or resolve any physical backing or deficit.
pub fn prepare_replacement_terms<'a>(
    scope: CausalScope,
    selected: ReplacementTermsSelection,
    records: &[CausalRecord<'a>],
    external: &[SourceId],
    uninterpreted: &[SourceId],
    observed_at: UnixSeconds,
) -> Result<ReplacementTermsClaim<'a>, ReplacementTermsError> {
    use ReplacementTermsError as E;
    let analysis = analyze(scope, records, external, uninterpreted).map_err(|_| E::InvalidGraph)?;
    match analysis.readiness() {
        CausalReadiness::Complete => {}
        CausalReadiness::MissingHistory => return Err(E::MissingHistory),
        CausalReadiness::Conflicted => return Err(E::Conflicted),
        CausalReadiness::Uninterpreted => return Err(E::Uninterpreted),
    }
    let known: BTreeMap<_, _> = records
        .iter()
        .map(|record| (record.id, record.content))
        .collect();
    let base = known
        .get(&selected.base)
        .copied()
        .ok_or(E::MissingHistory)?;
    let CommercePayload::AgreementCommitted(base) = base.payload() else {
        return Err(E::WrongBase);
    };
    let proposal = known
        .get(&selected.proposal)
        .copied()
        .ok_or(E::MissingHistory)?;
    let CommercePayload::TermsProposed(terms) = proposal.payload() else {
        return Err(E::WrongProposal);
    };
    if terms.base_agreement() != Some(selected.base) {
        return Err(E::WrongProposal);
    }
    let authorization = known
        .get(&selected.authorization)
        .copied()
        .ok_or(E::MissingHistory)?;
    let CommercePayload::TermsAuthorized(authorized) = authorization.payload() else {
        return Err(E::WrongAuthorization);
    };
    if authorized.proposal() != selected.proposal
        || authorization.header().author() == proposal.header().author()
    {
        return Err(E::WrongAuthorization);
    }
    if authorized.terms_digest() != selected.digest {
        return Err(E::TermsDigestMismatch);
    }
    if records
        .iter()
        .any(|record| record.content.header().created_at() > observed_at)
    {
        return Err(E::ClockAhead);
    }
    let expires = terms.expires_at().min(authorized.expires_at());
    if observed_at >= expires {
        return Err(E::Expired);
    }
    let withdrawn = |id, author| {
        known.values().any(|record| {
        record.header().author() == author && matches!(record.payload(), CommercePayload::WithdrawalRequested(value) if value.target() == id)
    })
    };
    let rejected = |id, author| {
        known.values().any(|record| {
        record.header().author() != author && matches!(record.payload(), CommercePayload::TermsRejected(value) if value.target() == id)
    })
    };
    if withdrawn(selected.authorization, authorization.header().author()) {
        return Err(E::Withdrawn);
    }
    if rejected(selected.proposal, proposal.header().author()) {
        return Err(E::Rejected);
    }
    for (id, other) in &known {
        let CommercePayload::TermsAuthorized(other_authorization) = other.payload() else {
            continue;
        };
        if *id == selected.authorization
            || other_authorization.expires_at() <= observed_at
            || withdrawn(*id, other.header().author())
        {
            continue;
        }
        let other_proposal = known
            .get(&other_authorization.proposal())
            .ok_or(E::MissingHistory)?;
        let CommercePayload::TermsProposed(other_terms) = other_proposal.payload() else {
            return Err(E::WrongAuthorization);
        };
        if other_proposal.header().author() == other.header().author() {
            return Err(E::WrongAuthorization);
        }
        if other_terms.base_agreement() != Some(selected.base)
            || other_terms.expires_at() <= observed_at
            || rejected(
                other_authorization.proposal(),
                other_proposal.header().author(),
            )
        {
            continue;
        }
        if other_terms.terms() != terms.terms()
            || other_authorization.terms_digest() != selected.digest
        {
            return Err(E::ConflictingAuthorizations);
        }
    }
    Ok(ReplacementTermsClaim {
        selection: selected,
        base: *base,
        terms: terms.terms(),
        expires,
    })
}
