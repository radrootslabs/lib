//! Deterministic replacement intent from captured original-source claims.
use super::{DecodedRecord, EncodedCommerceRumor, RecordWireError};
use crate::{command::CapturedTermsInstallation, terms::EncodedTerms};
use alloc::vec;
use radroots_domain::{
    ActorRole, AgreementCommitment, CommerceContent, CommerceHeader, CommercePayload, RecordId,
    Sequence, SourceId, UnixSeconds,
};

/// Builds the exact next seller commitment from complete mutually authorized
/// replacement data. This does not authenticate the original records or prove
/// that the base, residual, author heads, authority or stock are current. The
/// executor must independently admit and freeze those inputs before T1.
pub fn replacement_commitment_rumor(
    command: &CapturedTermsInstallation,
    base: &DecodedRecord,
    proposal: &DecodedRecord,
    authorization: &DecodedRecord,
    seller_head: (RecordId, Sequence),
    created_at: UnixSeconds,
) -> Result<EncodedCommerceRumor, RecordWireError> {
    let invalid = RecordWireError::InvalidInput;
    let expected = command.context().expected().ok_or(invalid)?;
    let CommercePayload::AgreementCommitted(base_commitment) = base.content().payload() else {
        return Err(invalid);
    };
    let CommercePayload::TermsProposed(proposed) = proposal.content().payload() else {
        return Err(invalid);
    };
    let CommercePayload::TermsAuthorized(authorized) = authorization.content().payload() else {
        return Err(invalid);
    };
    let original = base.content().header();
    if command.context().actor().role() != ActorRole::Seller
        || command.context().actor().participant() != original.seller()
        || original.author() != original.seller()
        || original.order_id() != Some(expected.order_id())
        || expected.agreement() != Some(base.record_id())
        || expected.seller_sequence() != base_commitment.seller_sequence()
        || base_commitment.authority() != command.context().authority()
        || proposed.base_agreement() != Some(base.record_id())
        || proposal.record_id() != command.input().proposal()
        || authorization.record_id() != command.input().authorization()
        || authorized.proposal() != proposal.record_id()
        || authorized.terms_digest() != command.input().terms_digest()
        || EncodedTerms::capture(proposed.terms())?.digest() != command.input().terms_digest()
        || proposal.content().header().author() == authorization.content().header().author()
        || expected.buyer_head().is_none()
        || seller_head.1.get() < original.author_sequence().get()
        || created_at.get() == 0
        || created_at >= command.context().not_after()
        || created_at >= proposed.expires_at()
        || created_at >= authorized.expires_at()
    {
        return Err(invalid);
    }
    for record in [base, proposal, authorization] {
        let header = record.content().header();
        if header.thread_id() != original.thread_id()
            || header.order_id() != original.order_id()
            || header.buyer() != original.buyer()
            || header.seller() != original.seller()
            || header.created_at() > created_at
            || (header.author() != original.buyer() && header.author() != original.seller())
            || (header.author() == original.seller() && header.author_sequence() > seller_head.1)
            || (record.record_id() == seller_head.0
                && (header.author() != original.seller()
                    || header.author_sequence() != seller_head.1))
            || (expected.buyer_head() == Some(record.record_id())
                && header.author() != original.buyer())
        {
            return Err(invalid);
        }
    }
    let author_sequence = Sequence::new(
        seller_head
            .1
            .get()
            .checked_add(1)
            .ok_or(RecordWireError::LimitExceeded)?,
    )
    .map_err(|_| RecordWireError::LimitExceeded)?;
    let seller_sequence = Sequence::new(
        base_commitment
            .seller_sequence()
            .get()
            .checked_add(1)
            .ok_or(RecordWireError::LimitExceeded)?,
    )
    .map_err(|_| RecordWireError::LimitExceeded)?;
    let mut parents = vec![
        base.record_id(),
        proposal.record_id(),
        authorization.record_id(),
        seller_head.0,
    ];
    parents.extend(expected.buyer_head());
    parents.sort();
    parents.dedup();
    let header = CommerceHeader::new(
        original.thread_id(),
        original.order_id(),
        original.seller(),
        original.buyer(),
        original.seller(),
        created_at,
        author_sequence,
        Some(seller_head.0),
        parents
            .into_iter()
            .map(|id| SourceId::from_bytes(id.into_bytes()))
            .collect(),
    )
    .map_err(|_| invalid)?;
    let commitment = AgreementCommitment::new(
        Some(base.record_id()),
        authorization.record_id(),
        command.input().terms_digest(),
        command.context().authority(),
        seller_sequence,
    )
    .map_err(|_| invalid)?;
    let content = CommerceContent::new(header, CommercePayload::AgreementCommitted(commitment))
        .map_err(|_| invalid)?;
    EncodedCommerceRumor::capture(&content)
}
