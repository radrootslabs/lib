//! Original authenticated public listing observations. Local observation is not
//! a global head, supported profile, current business permission or stock hold.
use super::*;
use radroots_domain::{
    EnvelopeEventId,
    listing::{ListingCoordinate, ListingId, ListingRevision},
};
use radroots_event::envelope::event_head::EventHeadCoordinate;
use radroots_nostr::public_ingress::ObservedHead;
use zeroize::Zeroizing;
pub(in crate::workspace::stock) mod admission;
mod checkpoint;
pub(in crate::workspace::stock) mod history;
mod record;
#[cfg(test)]
mod tests;
mod transaction;
pub(in crate::workspace) use transaction::{Lookup, query, retain};

/// Captured before store IO, using only authenticated Nostr observation
/// evidence. Namespace comparison here is data validation, not store access.
pub struct StockListingCapture {
    namespace: SellerAuthoritySelection,
    transaction: TransactionId,
    listing: ListingRevision,
    created_at: u64,
    raw: Zeroizing<Vec<u8>>,
}
impl StockListingCapture {
    pub fn prepare(
        namespace: SellerAuthoritySelection,
        observed: &ObservedHead,
    ) -> Result<Self, WorkspaceError> {
        let selected = observed.selected().ok_or(WorkspaceError::InvalidInput)?;
        let EventHeadCoordinate::Addressable {
            kind,
            pubkey,
            d_tag,
        } = &selected.coordinate
        else {
            return Err(WorkspaceError::InvalidInput);
        };
        if *kind != radroots_protocol::listing::LISTING_KIND {
            return Err(WorkspaceError::InvalidInput);
        }
        let discriminator: ListingId = d_tag.parse().map_err(|_| WorkspaceError::InvalidInput)?;
        let coordinate = ListingCoordinate::new(*pubkey, discriminator);
        if coordinate.author() != namespace.participant() {
            return Err(WorkspaceError::WrongNamespace);
        }
        let raw = observed
            .raw_json()
            .ok_or(WorkspaceError::InvalidInput)?
            .as_bytes();
        if raw.is_empty() || raw.len() > 64 * 1024 {
            return Err(WorkspaceError::Capacity);
        }
        Ok(Self {
            namespace,
            transaction: TransactionId::from_bytes(super::super::random()?),
            listing: ListingRevision::new(
                coordinate,
                EnvelopeEventId::from_bytes(*selected.event_id.as_bytes()),
            ),
            // NIP01 permits the full unsigned timestamp range. Preserve an
            // authentic far-future head; clock readiness is evaluated separately.
            created_at: selected.created_at,
            raw: Zeroizing::new(raw.to_vec()),
        })
    }
    pub const fn transaction_id(&self) -> TransactionId {
        self.transaction
    }
    pub const fn listing_revision(&self) -> ListingRevision {
        self.listing
    }
    pub(super) fn retained_copy(&self) -> Self {
        Self {
            namespace: self.namespace.clone(),
            transaction: self.transaction,
            listing: self.listing,
            created_at: self.created_at,
            raw: Zeroizing::new(self.raw.to_vec()),
        }
    }
}
impl std::fmt::Debug for StockListingCapture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("StockListingCapture(<authenticated observation>)")
    }
}

/// Original persisted observation. Selection is described at its original
/// retention, not promised for a later lookup or globally across relays.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StockListingReceipt {
    transaction: TransactionId,
    listing: ListingRevision,
    selected_at_retention: bool,
    checkpoint: radroots_storage::workspace::seller_stock::StockListingCheckpointRef,
}
impl StockListingReceipt {
    pub const fn transaction_id(&self) -> TransactionId {
        self.transaction
    }
    pub const fn listing_revision(&self) -> ListingRevision {
        self.listing
    }
    pub const fn selected_at_retention(&self) -> bool {
        self.selected_at_retention
    }
    pub const fn checkpoint(
        &self,
    ) -> radroots_storage::workspace::seller_stock::StockListingCheckpointRef {
        self.checkpoint
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StockListingOutcome {
    Committed(Box<StockListingReceipt>),
    Unknown {
        transaction: TransactionId,
        listing: ListingRevision,
    },
}
/// Original public evidence plus the independently acknowledged scope in which
/// its current-observed flag was checked. Neither grants business permission.
pub struct StockListingSnapshot {
    receipt: StockListingReceipt,
    selection_checkpoint: radroots_storage::workspace::seller_stock::StockListingCheckpointRef,
    observed_at: radroots_domain::UnixSeconds,
    current: bool,
    raw: Zeroizing<String>,
}
impl StockListingSnapshot {
    pub const fn receipt(&self) -> &StockListingReceipt {
        &self.receipt
    }
    pub const fn selection_checkpoint(
        &self,
    ) -> radroots_storage::workspace::seller_stock::StockListingCheckpointRef {
        self.selection_checkpoint
    }
    pub const fn observed_at(&self) -> radroots_domain::UnixSeconds {
        self.observed_at
    }
    pub const fn is_current_observed(&self) -> bool {
        self.current
    }
    pub fn raw_json(&self) -> &str {
        &self.raw
    }
    /// Pure profile/time classification at the captured read observation. A
    /// historical or unsupported result cannot be substituted as a live offer.
    pub fn interpret_at_observation(
        &self,
    ) -> Result<
        radroots_nostr::public_ingress::Interpretation,
        radroots_storage::command::OperationError,
    > {
        use radroots_storage::command::OperationError;
        let coordinate = self.receipt.listing.coordinate();
        let mut observed = ObservedHead::listing(coordinate.author(), &coordinate.d().to_string())
            .map_err(|_| OperationError::RecoveryRequired)?;
        observed
            .observe(&self.raw)
            .map_err(|_| OperationError::RecoveryRequired)?;
        observed
            .interpret(
                radroots_protocol::advertisement::UnixSeconds::new(self.observed_at.get())
                    .map_err(|_| WorkspaceError::Unavailable)?,
            )
            .ok_or(OperationError::RecoveryRequired)
    }
}
impl std::fmt::Debug for StockListingSnapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("StockListingSnapshot(<original observation>)")
    }
}

impl NativeWorkspace {
    /// Retains original signed listing evidence under current administrative
    /// access. The borrowed capture keeps its original recovery identity if an
    /// observer is lost; this does not publish or admit a business command.
    pub async fn retain_stock_listing(
        &self,
        binding: WorkspaceBinding,
        minimum: radroots_storage::workspace::seller_stock::StockListingCheckpointRef,
        capture: &StockListingCapture,
        proof: &AttachmentProof,
    ) -> Result<StockListingOutcome, radroots_storage::command::OperationError> {
        let proof = proof.with_secret(|secret| AttachmentProof::from_protected_input(secret))?;
        self.worker
            .retain_stock_listing(binding, minimum, capture.retained_copy(), proof)
            .await
    }
    /// Reads one exact retained revision, including historical observations.
    pub async fn stock_listing_source(
        &self,
        binding: WorkspaceBinding,
        minimum: radroots_storage::workspace::seller_stock::StockListingCheckpointRef,
        revision: ListingRevision,
        proof: &AttachmentProof,
    ) -> Result<Option<StockListingSnapshot>, radroots_storage::command::OperationError> {
        let proof = proof.with_secret(|secret| AttachmentProof::from_protected_input(secret))?;
        self.worker
            .stock_listing(binding, minimum, Lookup::Revision(revision), proof)
            .await
    }
    /// Reads the protected current observed head without older-profile fallback
    /// or a claim of global relay completeness or business actionability.
    pub async fn stock_listing_head(
        &self,
        binding: WorkspaceBinding,
        minimum: radroots_storage::workspace::seller_stock::StockListingCheckpointRef,
        coordinate: ListingCoordinate,
        proof: &AttachmentProof,
    ) -> Result<Option<StockListingSnapshot>, radroots_storage::command::OperationError> {
        let proof = proof.with_secret(|secret| AttachmentProof::from_protected_input(secret))?;
        self.worker
            .stock_listing(binding, minimum, Lookup::Coordinate(coordinate), proof)
            .await
    }
}

pub(super) async fn counts(
    connection: &mut SqliteConnection,
    current: PublicSourceCheckpoint,
) -> Result<(), WorkspaceError> {
    use sqlx::Row;
    current.validate()?;
    let row=sqlx::query("SELECT count(*),coalesce(max(revision),0),coalesce(sum(CASE WHEN revision=? AND chain_head=? THEN 1 ELSE 0 END),0),(SELECT count(*) FROM stock_public_heads),(SELECT count(*) FROM stock_public_parts) FROM stock_public_events")
        .bind(current.revision.get() as i64).bind(current.head.map(|head|head.as_bytes().to_vec()))
        .fetch_one(connection).await.map_err(|_|WorkspaceError::RecoveryRequired)?;
    for (column, expected) in [
        (0, current.events.get() as i64),
        (1, current.revision.get() as i64),
        (2, i64::from(current.head.is_some())),
        (3, current.heads.get() as i64),
        (4, current.parts.get() as i64),
    ] {
        if row
            .try_get::<i64, _>(column)
            .map_err(|_| WorkspaceError::RecoveryRequired)?
            != expected
        {
            return Err(WorkspaceError::RecoveryRequired);
        }
    }
    Ok(())
}
