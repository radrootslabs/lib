//! Protected original-source retention on the existing stock connection owner.
//! Recognized profiles remain claims until causal and business admission.
use super::*;
use radroots_domain::{CheckpointDigest, EnvelopeEventId, RecordId, RumorEventId};
use radroots_nostr::{commerce::RecordOpening, private_ingress::AuthenticatedCarrier};
use radroots_storage::{
    command::OperationError, workspace::seller_stock::StockSourceCheckpointRef,
};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;
pub(in crate::workspace::stock) mod checkpoint;
pub(in crate::workspace::stock) mod history;
pub(in crate::workspace::stock) mod request;
pub use history::StockHistorySnapshot;
pub use request::StockRequestSnapshot;
mod record;
mod transaction;
pub(in crate::workspace) use transaction::history as query_history;
pub(in crate::workspace) use transaction::inspect_request;
pub(in crate::workspace) use transaction::{query, retain};

/// Authenticated carrier/profile disposition, without causal actionability.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StockSourceProfile {
    Known,
    Unsupported,
    Rejected,
}
/// Original immutable source prepared before store IO. Only the Nostr layer's
/// authenticated opening types supply its evidence; metadata cannot mint it.
pub struct StockSourceCapture {
    namespace: SellerAuthoritySelection,
    transaction: TransactionId,
    envelope: EnvelopeEventId,
    seal: EnvelopeEventId,
    rumor: RumorEventId,
    profile: StockSourceProfile,
    canonical: Option<(RecordId, Zeroizing<Vec<u8>>)>,
    pieces: [Zeroizing<Vec<u8>>; 3],
    manifest: Manifest,
}
#[derive(Clone, Copy, Eq, PartialEq, Serialize)]
struct Piece {
    bytes: u32,
    digest: [u8; 32],
    parts: u8,
}
impl<'de> Deserialize<'de> for Piece {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            bytes: u32,
            digest: [u8; 32],
            parts: u8,
        }
        let wire: Wire = super::object(deserializer)?;
        Ok(Self {
            bytes: wire.bytes,
            digest: wire.digest,
            parts: wire.parts,
        })
    }
}
#[derive(Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    envelope: EnvelopeEventId,
    seal: EnvelopeEventId,
    rumor: RumorEventId,
    profile: StockSourceProfile,
    #[serde(deserialize_with = "super::required_nullable")]
    record: Option<RecordId>,
    #[serde(deserialize_with = "super::required_nullable")]
    canonical_digest: Option<[u8; 32]>,
    pieces: [Piece; 3],
}
impl StockSourceCapture {
    pub fn prepare(
        namespace: SellerAuthoritySelection,
        opening: &RecordOpening,
    ) -> Result<Self, WorkspaceError> {
        let (carrier, profile, canonical) = match opening {
            RecordOpening::Known(value) => {
                let encoded =
                    radroots_protocol::commerce::EncodedRecord::capture(value.record().content())
                        .map_err(|_| WorkspaceError::InvalidInput)?;
                (
                    value.carrier(),
                    StockSourceProfile::Known,
                    Some((
                        encoded.record_id(),
                        Zeroizing::new(encoded.protected_wire().to_vec()),
                    )),
                )
            }
            RecordOpening::Unsupported(value) => {
                (value.carrier(), StockSourceProfile::Unsupported, None)
            }
            RecordOpening::Rejected(value) => (value.carrier(), StockSourceProfile::Rejected, None),
        };
        if carrier.recipient() != namespace.participant() {
            return Err(WorkspaceError::WrongNamespace);
        }
        let pieces = source_pieces(carrier)?;
        let envelope = EnvelopeEventId::from_bytes(*carrier.envelope_id().as_bytes());
        let seal = EnvelopeEventId::from_bytes(*carrier.seal_id().as_bytes());
        let rumor = carrier.rumor().event_id();
        let descriptors = pieces.each_ref().map(|bytes| Piece {
            bytes: bytes.len() as u32,
            digest: Sha256::digest(bytes.as_slice()).into(),
            parts: bytes.len().div_ceil(16 * 1024) as u8,
        });
        let manifest = Manifest {
            envelope,
            seal,
            rumor,
            profile,
            record: canonical.as_ref().map(|value| value.0),
            canonical_digest: canonical
                .as_ref()
                .map(|value| Sha256::digest(value.1.as_slice()).into()),
            pieces: descriptors,
        };
        Ok(Self {
            namespace,
            transaction: TransactionId::from_bytes(super::super::random()?),
            envelope,
            seal,
            rumor,
            profile,
            canonical,
            pieces,
            manifest,
        })
    }
    pub const fn transaction_id(&self) -> TransactionId {
        self.transaction
    }
    pub const fn envelope_id(&self) -> EnvelopeEventId {
        self.envelope
    }
    pub const fn profile(&self) -> StockSourceProfile {
        self.profile
    }
    pub fn record_id(&self) -> Option<RecordId> {
        self.canonical.as_ref().map(|value| value.0)
    }
    pub(super) fn retained_copy(&self) -> Self {
        Self {
            namespace: self.namespace.clone(),
            transaction: self.transaction,
            envelope: self.envelope,
            seal: self.seal,
            rumor: self.rumor,
            profile: self.profile,
            canonical: self
                .canonical
                .as_ref()
                .map(|(id, bytes)| (*id, Zeroizing::new(bytes.to_vec()))),
            pieces: self
                .pieces
                .each_ref()
                .map(|value| Zeroizing::new(value.to_vec())),
            manifest: self.manifest,
        }
    }
}
fn source_pieces(
    carrier: &AuthenticatedCarrier,
) -> Result<[Zeroizing<Vec<u8>>; 3], WorkspaceError> {
    let texts = [
        carrier.protected_outer(),
        carrier.protected_seal(),
        carrier.protected_rumor(),
    ];
    if texts
        .iter()
        .any(|text| text.is_empty() || text.len() > 64 * 1024)
    {
        return Err(WorkspaceError::InvalidInput);
    }
    Ok(texts.map(|text| Zeroizing::new(text.as_bytes().to_vec())))
}
impl std::fmt::Debug for StockSourceCapture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("StockSourceCapture(<redacted>)")
    }
}
/// Confirmed original storage identity. Profile recognition is not an order
/// decision, source-completeness result or physical fact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StockSourceReceipt {
    transaction: TransactionId,
    envelope: EnvelopeEventId,
    record: Option<RecordId>,
    profile: StockSourceProfile,
    checkpoint: StockSourceCheckpointRef,
}
impl StockSourceReceipt {
    pub const fn transaction_id(&self) -> TransactionId {
        self.transaction
    }
    pub const fn envelope_id(&self) -> EnvelopeEventId {
        self.envelope
    }
    pub const fn record_id(&self) -> Option<RecordId> {
        self.record
    }
    pub const fn profile(&self) -> StockSourceProfile {
        self.profile
    }
    pub const fn checkpoint(&self) -> StockSourceCheckpointRef {
        self.checkpoint
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StockSourceOutcome {
    Committed(StockSourceReceipt),
    Unknown {
        transaction: TransactionId,
        envelope: EnvelopeEventId,
    },
}
/// Explicit private original bytes, returned only under current source access.
/// This never reconstructs an authenticated-opening token or business status.
pub struct StockSourceSnapshot {
    receipt: StockSourceReceipt,
    pieces: [Zeroizing<Vec<u8>>; 3],
    canonical: Option<Zeroizing<Vec<u8>>>,
}
impl StockSourceSnapshot {
    pub const fn receipt(&self) -> &StockSourceReceipt {
        &self.receipt
    }
    pub fn protected_outer(&self) -> &[u8] {
        &self.pieces[0]
    }
    pub fn protected_seal(&self) -> &[u8] {
        &self.pieces[1]
    }
    pub fn protected_rumor(&self) -> &[u8] {
        &self.pieces[2]
    }
    pub fn protected_canonical_record(&self) -> Option<&[u8]> {
        self.canonical.as_ref().map(|value| value.as_slice())
    }
}
impl std::fmt::Debug for StockSourceSnapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("StockSourceSnapshot(<redacted>)")
    }
}
impl NativeWorkspace {
    /// Inspects original initial request, quote, source snapshots and deadlines
    /// under current administrative read access. This acquires no execution
    /// permission, pending order fence, signature or physical stock hold.
    pub async fn stock_order_request(
        &self,
        binding: WorkspaceBinding,
        minimum: StockSourceCheckpointRef,
        listing_minimum: radroots_storage::workspace::seller_stock::StockListingCheckpointRef,
        request: RecordId,
        proof: &AttachmentProof,
    ) -> Result<StockRequestSnapshot, OperationError> {
        let proof = proof.with_secret(|bytes| AttachmentProof::from_protected_input(bytes))?;
        self.worker
            .stock_order_request(binding, minimum, listing_minimum, request, proof)
            .await
    }
    /// Reads complete retained local causal coverage for one bilateral scope.
    /// Current administrator access is required. This neither authorizes a new
    /// request nor reports an installed agreement, global history or stock hold.
    /// Uninterpreted commerce from either party conservatively blocks that pair
    /// because its future header cannot yet be scoped. Ordinary chat remains a
    /// separate source and never becomes a structured record by interpretation.
    pub async fn stock_order_history(
        &self,
        binding: WorkspaceBinding,
        minimum: StockSourceCheckpointRef,
        listing_minimum: radroots_storage::workspace::seller_stock::StockListingCheckpointRef,
        scope: radroots_domain::causal::CausalScope,
        proof: &AttachmentProof,
    ) -> Result<StockHistorySnapshot, OperationError> {
        let proof = proof.with_secret(|bytes| AttachmentProof::from_protected_input(bytes))?;
        self.worker
            .stock_order_history(binding, minimum, listing_minimum, scope, proof)
            .await
    }
    /// Retains original authenticated evidence on the selected owner worker.
    /// Borrowed capture/proof remain available after error or observer loss.
    pub async fn retain_stock_source(
        &self,
        binding: WorkspaceBinding,
        minimum: StockSourceCheckpointRef,
        capture: &StockSourceCapture,
        proof: &AttachmentProof,
    ) -> Result<StockSourceOutcome, OperationError> {
        let proof = proof.with_secret(|bytes| AttachmentProof::from_protected_input(bytes))?;
        self.worker
            .retain_stock_source(binding, minimum, capture.retained_copy(), proof)
            .await
    }
    pub async fn stock_source(
        &self,
        binding: WorkspaceBinding,
        minimum: StockSourceCheckpointRef,
        envelope: EnvelopeEventId,
        proof: &AttachmentProof,
    ) -> Result<Option<StockSourceSnapshot>, OperationError> {
        let proof = proof.with_secret(|bytes| AttachmentProof::from_protected_input(bytes))?;
        self.worker
            .stock_source(binding, minimum, envelope, proof)
            .await
    }
}
