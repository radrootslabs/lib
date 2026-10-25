//! Deterministic initial agreement intent from frozen preparation data claims.
use super::{DecodedRecord, EncodedCommerceRumor, RecordWireError};
use crate::{command::CapturedOrderAcceptance, terms::EncodedTerms};
use alloc::vec::Vec;
use radroots_domain::{
    ActorRole, AgreementCommitment, CommerceContent, CommerceHeader, CommercePayload, RecordId,
    Sequence, SourceId, UnixSeconds,
};

/// Constructs exact initial agreement/rumor data from a captured command,
/// original request and captured seller author head. These are data claims,
/// never authenticated permission or proof of T0. A concrete host must compare
/// all inputs with its actual retained preparation before binding T1. It must
/// not derive this head or time again from a later UI/history observation.
pub fn initial_commitment_rumor(
    command: &CapturedOrderAcceptance,
    request: &DecodedRecord,
    seller_head: Option<(RecordId, Sequence)>,
    created_at: UnixSeconds,
) -> Result<EncodedCommerceRumor, RecordWireError> {
    let header = request.content().header();
    let CommercePayload::Request(authorization) = request.content().payload() else {
        return Err(RecordWireError::InvalidInput);
    };
    let expected = command
        .context()
        .expected()
        .ok_or(RecordWireError::InvalidInput)?;
    if request.record_id() != command.input().request()
        || EncodedTerms::capture(authorization.terms())?.digest() != command.input().terms_digest()
        || command.context().actor().role() != ActorRole::Seller
        || command.context().actor().participant() != header.seller()
        || header.order_id() != Some(expected.order_id())
        || expected.agreement().is_some()
        || expected.seller_sequence().get() != 0
        || expected.buyer_head().is_none()
        || created_at.get() == 0
        || created_at < header.created_at()
        || created_at >= authorization.expires_at()
        || created_at >= command.context().not_after()
        || seller_head.is_some_and(|(_, sequence)| sequence.get() == 0)
    {
        return Err(RecordWireError::InvalidInput);
    }
    let sequence = Sequence::new(
        seller_head
            .map(|(_, sequence)| sequence.get())
            .unwrap_or(0)
            .checked_add(1)
            .ok_or(RecordWireError::LimitExceeded)?,
    )
    .map_err(|_| RecordWireError::LimitExceeded)?;
    let previous = seller_head.map(|(id, _)| id);
    let mut parents = Vec::new();
    parents.push(SourceId::from_bytes(request.record_id().into_bytes()));
    if let Some(head) = expected.buyer_head() {
        parents.push(SourceId::from_bytes(head.into_bytes()));
    }
    if let Some(head) = previous {
        parents.push(SourceId::from_bytes(head.into_bytes()));
    }
    parents.sort();
    parents.dedup();
    let header = CommerceHeader::new(
        header.thread_id(),
        header.order_id(),
        header.seller(),
        header.buyer(),
        header.seller(),
        created_at,
        sequence,
        previous,
        parents,
    )
    .map_err(|_| RecordWireError::InvalidInput)?;
    let payload = AgreementCommitment::new(
        None,
        request.record_id(),
        command.input().terms_digest(),
        command.context().authority(),
        Sequence::new(1).map_err(|_| RecordWireError::InvalidInput)?,
    )
    .map_err(|_| RecordWireError::InvalidInput)?;
    let record = CommerceContent::new(header, CommercePayload::AgreementCommitted(payload))
        .map_err(|_| RecordWireError::InvalidInput)?;
    EncodedCommerceRumor::capture(&record)
}
