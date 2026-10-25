//! Fresh initial request/source inspection. Execution rights, installed order
//! state, pending fences and actual atomic stock effects remain separate.
use super::*;
use radroots_domain::{
    CheckpointRef, Terms, TermsDigest, UnixSeconds,
    causal::{CausalRecord, CausalScope, RequestClaimError},
};
use radroots_storage::workspace::seller_stock::StockListingCheckpointRef;
use std::collections::BTreeMap;

/// Explicit private request/source data under current administrative read access.
/// This does not authorize execution or promise a physical hold/agreement.
pub struct StockRequestSnapshot {
    pub(super) scope: CausalScope,
    pub(super) request: RecordId,
    pub(super) terms: Box<Terms>,
    pub(super) digest: TermsDigest,
    pub(super) expires: UnixSeconds,
    pub(super) quote: Option<RecordId>,
    pub(super) checkpoint: CheckpointRef,
    pub(super) sources: StockSourceCheckpointRef,
    pub(super) listings: StockListingCheckpointRef,
    pub(super) observed_at: UnixSeconds,
    pub(super) inventory: radroots_storage::inventory::InventoryCheckpointRef,
}
impl StockRequestSnapshot {
    /// Inventory progress included in the complete local causal read.
    pub const fn inventory_checkpoint(
        &self,
    ) -> radroots_storage::inventory::InventoryCheckpointRef {
        self.inventory
    }
    /// Exact selected scope derived from original authenticated request content.
    pub const fn scope(&self) -> CausalScope {
        self.scope
    }
    /// Exact original request identity, distinct from its transport envelopes.
    pub const fn request(&self) -> RecordId {
        self.request
    }
    /// Explicit private disclosure of complete original buyer-authorized terms.
    pub fn terms(&self) -> &Terms {
        &self.terms
    }
    /// Canonical complete terms identity; no approval is minted by this getter.
    pub const fn terms_digest(&self) -> TermsDigest {
        self.digest
    }
    /// Earliest request/quote deadline, excluding grant/approval/issuance bounds.
    pub const fn expires_at(&self) -> UnixSeconds {
        self.expires
    }
    /// Exact optional complete seller quote used for source permission.
    pub const fn quote(&self) -> Option<RecordId> {
        self.quote
    }
    /// Complete local causal data; not an installed order or decision fence.
    pub const fn checkpoint(&self) -> &CheckpointRef {
        &self.checkpoint
    }
    /// Independently acknowledged retained private source progress.
    pub const fn source_checkpoint(&self) -> StockSourceCheckpointRef {
        self.sources
    }
    /// Independently acknowledged retained public source progress.
    pub const fn listing_checkpoint(&self) -> StockListingCheckpointRef {
        self.listings
    }
    /// Final usable observation after transaction witnesses/current access checks.
    pub const fn observed_at(&self) -> UnixSeconds {
        self.observed_at
    }
}
impl std::fmt::Debug for StockRequestSnapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("StockRequestSnapshot(<private request>)")
    }
}
pub(in crate::workspace::stock) struct Prepared {
    pub scope: CausalScope,
    pub request: RecordId,
    pub digest: TermsDigest,
    pub expires: UnixSeconds,
    pub quote: Option<RecordId>,
    pub history: history::Prepared,
    lookup: checkpoint::Proof,
    views: BTreeMap<EnvelopeEventId, super::super::public_source::admission::ListingView>,
}
fn claim_error(error: RequestClaimError) -> OperationError {
    match error {
        RequestClaimError::MissingHistory => OperationError::MissingHistory,
        RequestClaimError::Uninterpreted => OperationError::UnsupportedOperation,
        RequestClaimError::Expired => OperationError::DeadlineExceeded,
        RequestClaimError::ClockAhead => WorkspaceError::Unavailable.into(),
        RequestClaimError::Conflicted
        | RequestClaimError::Withdrawn
        | RequestClaimError::Declined
        | RequestClaimError::ConflictingAuthorizations
        | RequestClaimError::RequiresReconciliation => OperationError::StaleState,
        _ => OperationError::InvalidEvidence,
    }
}
impl Prepared {
    pub fn terms(&self) -> Result<&Terms, OperationError> {
        match self
            .history
            .records
            .get(&self.request)
            .map(|record| record.content().payload())
        {
            Some(radroots_domain::CommercePayload::Request(request)) => Ok(request.terms()),
            _ => Err(OperationError::RecoveryRequired),
        }
    }
    pub fn validate_at(&self, at: UnixSeconds) -> Result<(), OperationError> {
        if at >= self.expires {
            return Err(OperationError::DeadlineExceeded);
        }
        for line in self.terms()?.lines() {
            self.views
                .get(&line.source().event_id())
                .ok_or(OperationError::RecoveryRequired)?
                .validate(line, self.quote.is_some(), at)?;
        }
        Ok(())
    }
    pub async fn confirm(&self, connection: &mut SqliteConnection) -> Result<(), OperationError> {
        self.lookup.confirm(connection).await?;
        self.history.confirm(connection).await?;
        for view in self.views.values() {
            view.confirm(connection).await?;
        }
        Ok(())
    }
}
pub(in crate::workspace::stock) async fn prepare(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    inventory_read: history::InventoryRead<'_>,
    request: RecordId,
    listing_minimum: StockListingCheckpointRef,
    observed_at: UnixSeconds,
) -> Result<Prepared, OperationError> {
    let authority = inventory_read.authority;
    let raw = record::canonical(connection, input, crypto, request).await?;
    let observed = raw
        .as_ref()
        .map(|(raw, _)| <[u8; 32]>::from(Sha256::digest(&**raw)));
    let lookup = checkpoint::authenticate(
        connection,
        input,
        crypto,
        authority
            .sources
            .ok_or(OperationError::UnsupportedOperation)?,
        checkpoint::Target::Record(request, observed),
    )
    .await?;
    let (raw, _) = raw.ok_or(OperationError::MissingHistory)?;
    let decoded =
        radroots_protocol::commerce::decode(&raw).map_err(|_| OperationError::RecoveryRequired)?;
    let header = decoded.content().header();
    if decoded.content().record_type() != radroots_domain::CommerceType::Request {
        return Err(OperationError::InvalidCommand);
    }
    let scope = CausalScope::new(
        header.thread_id(),
        header.order_id().ok_or(OperationError::InvalidEvidence)?,
        header.buyer(),
        header.seller(),
    )
    .map_err(|_| OperationError::InvalidEvidence)?;
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
    let claim = radroots_domain::causal::prepare_initial_request(
        scope,
        request,
        &nodes,
        &history.external,
        &history.uninterpreted,
        observed_at,
    )
    .map_err(claim_error)?;
    let digest = radroots_protocol::terms::EncodedTerms::capture(claim.terms())
        .map_err(|_| OperationError::InvalidEvidence)?
        .digest();
    let expires = claim.expires_at();
    let quote = claim.quote();
    let mut views = BTreeMap::new();
    for line in claim.terms().lines() {
        if let std::collections::btree_map::Entry::Vacant(entry) =
            views.entry(line.source().event_id())
        {
            let view = super::super::public_source::admission::prepare(
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
        views[&line.source().event_id()].validate(line, claim.quote().is_some(), observed_at)?;
    }
    Ok(Prepared {
        scope,
        request,
        digest,
        expires,
        quote,
        history,
        lookup,
        views,
    })
}
