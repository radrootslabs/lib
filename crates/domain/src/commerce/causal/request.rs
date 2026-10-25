//! Initial request claims only. Concrete adapters must authenticate complete
//! coverage, current public/quote source permission and actual execution rights.
use super::*;
use crate::{CommercePayload, Terms, TermsRequest, UnixSeconds};

/// Borrowed complete request data after self-contained local causal/time checks.
/// This value supplies no signature, current-source or execution capability.
pub struct InitialRequestClaim<'a> {
    request: RecordId,
    terms: &'a Terms,
    expires: UnixSeconds,
    quote: Option<RecordId>,
}
impl InitialRequestClaim<'_> {
    /// Exact selected request claim; original carrier authentication is separate.
    pub const fn request(&self) -> RecordId {
        self.request
    }
    /// Complete private terms claim; no stock is acquired by borrowing it.
    pub const fn terms(&self) -> &Terms {
        self.terms
    }
    /// Earliest current request/explicit-quote deadline, excluding execution scope.
    pub const fn expires_at(&self) -> UnixSeconds {
        self.expires
    }
    /// Exact optional seller quote whose complete terms equal this request.
    pub const fn quote(&self) -> Option<RecordId> {
        self.quote
    }
}
impl fmt::Debug for InitialRequestClaim<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("InitialRequestClaim(<private data>)")
    }
}
/// Bounded initial-request claim failures, without private source values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RequestClaimError {
    /// Source count or identity/scope declarations are invalid.
    InvalidGraph,
    /// Required original parents or the selected request are absent.
    MissingHistory,
    /// The known scoped graph contains an incompatible author fork or cycle.
    Conflicted,
    /// Authenticated uninterpreted evidence prevents a complete local reduction.
    Uninterpreted,
    /// The selected record is not a buyer request in this exact scope.
    WrongRequest,
    /// The request or exact quote no longer admits a fresh decision.
    Expired,
    /// Known scoped author time is ahead of the supplied usable observation.
    ClockAhead,
    /// An observed buyer withdrawal names the selected authorization.
    Withdrawn,
    /// An observed seller decline names the selected request.
    Declined,
    /// The exact quoted record has another family or different complete terms.
    QuoteMismatch,
    /// Another unexpired unretracted buyer request authorizes incompatible terms.
    ConflictingAuthorizations,
    /// Advanced agreement/negotiation state needs its corresponding operation.
    RequiresReconciliation,
}
impl fmt::Display for RequestClaimError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidGraph => "invalid initial request graph",
            Self::MissingHistory => "initial request history missing",
            Self::Conflicted => "initial request history conflicted",
            Self::Uninterpreted => "initial request history uninterpreted",
            Self::WrongRequest => "wrong initial request",
            Self::Expired => "initial request deadline expired",
            Self::ClockAhead => "initial request clock observation insufficient",
            Self::Withdrawn => "initial request withdrawn",
            Self::Declined => "initial request declined",
            Self::QuoteMismatch => "initial request quote differs",
            Self::ConflictingAuthorizations => "initial request authorizations conflict",
            Self::RequiresReconciliation => "initial request state needs reconciliation",
        })
    }
}
impl core::error::Error for RequestClaimError {}
fn deadline(
    request: &TermsRequest,
    records: &[CausalRecord<'_>],
) -> Result<UnixSeconds, RequestClaimError> {
    let mut expires = request.expires_at();
    if let Some(id) = request.quote() {
        let quoted = records
            .iter()
            .find(|record| record.id == id)
            .ok_or(RequestClaimError::MissingHistory)?;
        let CommercePayload::Quote(quote) = quoted.content.payload() else {
            return Err(RequestClaimError::QuoteMismatch);
        };
        if quote.terms() != request.terms() {
            return Err(RequestClaimError::QuoteMismatch);
        }
        expires = expires.min(quote.expires_at());
    }
    Ok(expires)
}

/// Prepares only an initial quote/request/decline/withdrawal claim. Broader
/// negotiation and installed agreement records require their own stateful
/// operation; they are preserved, never treated as an empty initial order.
/// An original retained operation must be recovered before calling this fresh
/// planner. Neither expiry nor withdrawal erases an already admitted operation.
pub fn prepare_initial_request<'a>(
    scope: CausalScope,
    selected: RecordId,
    records: &[CausalRecord<'a>],
    external: &[SourceId],
    uninterpreted: &[SourceId],
    observed_at: UnixSeconds,
) -> Result<InitialRequestClaim<'a>, RequestClaimError> {
    let analysis = analyze(scope, records, external, uninterpreted)
        .map_err(|_| RequestClaimError::InvalidGraph)?;
    match analysis.readiness() {
        CausalReadiness::Complete => {}
        CausalReadiness::MissingHistory => return Err(RequestClaimError::MissingHistory),
        CausalReadiness::Conflicted => return Err(RequestClaimError::Conflicted),
        CausalReadiness::Uninterpreted => return Err(RequestClaimError::Uninterpreted),
    }
    let selected_content = records
        .iter()
        .find(|record| record.id == selected)
        .ok_or(RequestClaimError::MissingHistory)?
        .content;
    let CommercePayload::Request(request) = selected_content.payload() else {
        return Err(RequestClaimError::WrongRequest);
    };
    if selected_content.header().order_id() != Some(scope.order)
        || selected_content.header().author() != scope.buyer
    {
        return Err(RequestClaimError::WrongRequest);
    }
    if records
        .iter()
        .any(|record| record.content.header().created_at() > observed_at)
    {
        return Err(RequestClaimError::ClockAhead);
    }
    let expires = deadline(request, records)?;
    if expires <= observed_at {
        return Err(RequestClaimError::Expired);
    }
    let withdrawn = |id| {
        records.iter().any(|record|matches!(record.content.payload(),CommercePayload::WithdrawalRequested(value) if value.target()==id))
    };
    let declined = |id| {
        records.iter().any(|record|matches!(record.content.payload(),CommercePayload::Decline(value) if value.target()==id))
    };
    if withdrawn(selected) {
        return Err(RequestClaimError::Withdrawn);
    }
    if declined(selected) {
        return Err(RequestClaimError::Declined);
    }
    for record in records {
        match record.content.payload() {
            CommercePayload::Request(other) => {
                let other_expires = deadline(other, records)?;
                if record.id != selected
                    && other_expires > observed_at
                    && !withdrawn(record.id)
                    && !declined(record.id)
                    && other.terms() != request.terms()
                {
                    return Err(RequestClaimError::ConflictingAuthorizations);
                }
            }
            CommercePayload::Quote(_)
            | CommercePayload::Decline(_)
            | CommercePayload::WithdrawalRequested(_) => {}
            _ => return Err(RequestClaimError::RequiresReconciliation),
        }
    }
    Ok(InitialRequestClaim {
        request: selected,
        terms: request.terms(),
        expires,
        quote: request.quote(),
    })
}
