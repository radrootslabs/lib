//! Original public snapshots and protected current-observed source checks.
//! The caller independently proves request/quote and current execution rights.
use super::*;
use radroots_domain::{AgreementLine, UnixSeconds, listing::ListingState};
use radroots_storage::command::OperationError;

pub(in crate::workspace::stock) struct ListingView {
    source: ListingRevision,
    original: checkpoint::Proof,
    projection: checkpoint::Proof,
    head: Option<record::RetainedHead>,
    parts: u8,
    parts_witness: [u8; 32],
    listing: Box<radroots_protocol::listing::DecodedListing>,
}
pub(in crate::workspace::stock) async fn prepare(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    authority: &Authority,
    source: ListingRevision,
    observed_at: UnixSeconds,
) -> Result<ListingView, OperationError> {
    let retained = record::read(
        connection,
        input,
        crypto,
        authority.input.grant_id(),
        source.event_id(),
    )
    .await?;
    let original = checkpoint::authenticate(
        connection,
        input,
        crypto,
        authority,
        checkpoint::Target::Event(
            source.event_id(),
            retained
                .as_ref()
                .map(|value| value.identity())
                .transpose()?,
        ),
    )
    .await?;
    let retained = retained.ok_or(OperationError::MissingHistory)?;
    if retained.header.manifest.listing != source {
        return Err(OperationError::InvalidEvidence);
    }
    let parts = record::read_parts(connection, input, crypto, &retained.header).await?;
    let head = record::read_head(connection, input, crypto, source.coordinate()).await?;
    let projection = checkpoint::authenticate(
        connection,
        input,
        crypto,
        authority,
        checkpoint::Target::Coordinate(
            source.coordinate(),
            head.as_ref().map(|value| value.head.clone()),
        ),
    )
    .await?;
    let raw = std::str::from_utf8(&parts.raw).map_err(|_| OperationError::RecoveryRequired)?;
    let mut observed = ObservedHead::listing(
        source.coordinate().author(),
        &source.coordinate().d().to_string(),
    )
    .map_err(|_| OperationError::RecoveryRequired)?;
    observed
        .observe(raw)
        .map_err(|_| OperationError::RecoveryRequired)?;
    let meaning = observed
        .interpret(
            radroots_protocol::advertisement::UnixSeconds::new(observed_at.get())
                .map_err(|_| WorkspaceError::Unavailable)?,
        )
        .ok_or(OperationError::RecoveryRequired)?;
    let listing = match meaning {
        radroots_nostr::public_ingress::Interpretation::Listing(listing) => listing,
        radroots_nostr::public_ingress::Interpretation::ClockAhead => {
            return Err(WorkspaceError::Unavailable.into());
        }
        radroots_nostr::public_ingress::Interpretation::Unsupported => {
            return Err(OperationError::UnsupportedOperation);
        }
        _ => return Err(OperationError::InvalidEvidence),
    };
    Ok(ListingView {
        source,
        original,
        projection,
        head,
        parts: retained.header.manifest.parts,
        parts_witness: parts.witness,
        listing,
    })
}
impl ListingView {
    pub(in crate::workspace::stock) fn validate(
        &self,
        line: &AgreementLine,
        explicit_quote: bool,
        observed_at: UnixSeconds,
    ) -> Result<(), OperationError> {
        let listing = self.listing.listing();
        let terms = listing.terms();
        if listing.created_at() > observed_at {
            return Err(WorkspaceError::Unavailable.into());
        }
        if line.source() != self.source
            || listing.source() != self.source
            || line.product() != terms.product()
            || terms
                .options()
                .iter()
                .find(|option| option.id() == line.option_id())
                != Some(line.option())
        {
            return Err(OperationError::InvalidEvidence);
        }
        if !explicit_quote
            && (terms.state() != ListingState::Active
                || !self
                    .head
                    .as_ref()
                    .is_some_and(|head| head.head.listing == self.source))
        {
            return Err(OperationError::StaleState);
        }
        Ok(())
    }
    pub(in crate::workspace::stock) async fn confirm(
        &self,
        connection: &mut SqliteConnection,
    ) -> Result<(), OperationError> {
        self.original.confirm(connection).await?;
        self.projection.confirm(connection).await?;
        record::confirm_head(connection, self.source.coordinate(), self.head.as_ref()).await?;
        if record::parts_witness(connection, self.source.event_id()).await?
            != (self.parts, self.parts_witness)
        {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(())
    }
}
