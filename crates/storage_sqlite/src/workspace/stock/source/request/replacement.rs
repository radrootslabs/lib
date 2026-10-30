//! Complete current local source admission for the selected replacement.
use super::*;
use radroots_domain::causal::{ReplacementTermsError, ReplacementTermsSelection};
use radroots_protocol::{command::CapturedTermsInstallation, commerce::DecodedRecord};

fn error(value: ReplacementTermsError) -> OperationError {
    match value {
        ReplacementTermsError::MissingHistory => OperationError::MissingHistory,
        ReplacementTermsError::Uninterpreted => OperationError::UnsupportedOperation,
        ReplacementTermsError::Expired => OperationError::DeadlineExceeded,
        ReplacementTermsError::ClockAhead => WorkspaceError::Unavailable.into(),
        ReplacementTermsError::Conflicted
        | ReplacementTermsError::Withdrawn
        | ReplacementTermsError::Rejected
        | ReplacementTermsError::ConflictingAuthorizations => OperationError::StaleState,
        _ => OperationError::InvalidEvidence,
    }
}
pub(in crate::workspace::stock) struct Selected<'a> {
    pub command: &'a CapturedTermsInstallation,
    pub base: &'a DecodedRecord,
}
pub(in crate::workspace::stock) async fn prepare(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    inventory_read: history::InventoryRead<'_>,
    selected: Selected<'_>,
    listing_minimum: StockListingCheckpointRef,
    observed_at: UnixSeconds,
) -> Result<Prepared, OperationError> {
    let Selected { command, base } = selected;
    let expected = command
        .context()
        .expected()
        .ok_or(OperationError::InvalidCommand)?;
    let original = base.content().header();
    if expected.agreement() != Some(base.record_id())
        || original.order_id() != Some(expected.order_id())
    {
        return Err(OperationError::StaleState);
    }
    let scope = CausalScope::new(
        original.thread_id(),
        expected.order_id(),
        original.buyer(),
        original.seller(),
    )
    .map_err(|_| OperationError::InvalidEvidence)?;
    let authority = inventory_read.authority;
    let history = Box::pin(history::prepare(
        connection,
        input,
        crypto,
        inventory_read,
        scope,
        listing_minimum,
    ))
    .await?;
    let nodes: Vec<_> = history
        .records
        .iter()
        .map(|(id, record)| CausalRecord::new(*id, record.content()))
        .collect();
    let selected = ReplacementTermsSelection::new(
        base.record_id(),
        command.input().proposal(),
        command.input().authorization(),
        command.input().terms_digest(),
    );
    let claim = radroots_domain::causal::prepare_replacement_terms(
        scope,
        selected,
        &nodes,
        &history.external,
        &history.uninterpreted,
        observed_at,
    )
    .map_err(error)?;
    if history
        .records
        .get(&base.record_id())
        .is_none_or(|record| record.content() != base.content())
        || claim.base_commitment().seller_sequence() != expected.seller_sequence()
        || expected.buyer_head() != history.analysis.buyer_head()
        || Some(expected.checkpoint()) != history.checkpoint.as_ref()
    {
        return Err(OperationError::StaleState);
    }
    let digest = radroots_protocol::terms::EncodedTerms::capture(claim.terms())
        .map_err(|_| OperationError::InvalidEvidence)?
        .digest();
    if digest != selected.terms_digest() {
        return Err(OperationError::InvalidEvidence);
    }
    let expires = claim.expires_at();
    let mut views = BTreeMap::new();
    for line in claim.terms().lines() {
        if let std::collections::btree_map::Entry::Vacant(entry) =
            views.entry(line.source().event_id())
        {
            let view = super::super::super::public_source::admission::prepare(
                connection,
                input,
                crypto,
                authority,
                line.source(),
                observed_at,
            )
            .await?;
            entry.insert(view);
        }
        views[&line.source().event_id()].validate(line, true, observed_at)?;
    }
    Ok(Prepared {
        scope,
        authorization: selected.authorization(),
        digest,
        expires,
        quote: None,
        replacement: Some(selected),
        history,
        lookup: None,
        views,
    })
}
