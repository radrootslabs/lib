//! Complete protected local scoped causal coverage. This read supplies no
//! request permission, current-offer permission, installed order or stock hold.
use super::*;
use radroots_domain::{
    SourceId, UnixSeconds,
    causal::{self, CausalAnalysis, CausalRecord, CausalScope},
};
use radroots_protocol::{commerce::DecodedRecord, private_rumor::DecodedRumor};
use radroots_storage::workspace::seller_stock::StockListingCheckpointRef;
use sqlx::Row;
use std::collections::{BTreeMap, BTreeSet};

/// Checked retained local causal coverage at independently acknowledged private
/// and public checkpoints. This grants no request, agreement or stock authority.
pub struct StockHistorySnapshot {
    pub(super) scope: CausalScope,
    pub(super) analysis: CausalAnalysis,
    pub(super) sources: StockSourceCheckpointRef,
    pub(super) listings: StockListingCheckpointRef,
    pub(super) observed_at: UnixSeconds,
    pub(super) records: usize,
    pub(super) inventory: radroots_storage::inventory::InventoryCheckpointRef,
    pub(super) checkpoint: Option<radroots_domain::CheckpointRef>,
}
impl StockHistorySnapshot {
    /// Independently acknowledged inventory history containing local issued records.
    pub const fn inventory_checkpoint(
        &self,
    ) -> radroots_storage::inventory::InventoryCheckpointRef {
        self.inventory
    }
    /// Borrows complete local causal coverage data. This is not an installed
    /// order, pending-decision fence or permission to accept the request.
    pub fn checkpoint(&self) -> Option<&radroots_domain::CheckpointRef> {
        self.checkpoint.as_ref()
    }
    /// Returns the exact selected participant/thread/order scope.
    pub const fn scope(&self) -> CausalScope {
        self.scope
    }
    /// Borrows the deterministic local graph classification and exact gaps.
    pub const fn analysis(&self) -> &CausalAnalysis {
        &self.analysis
    }
    /// Returns all private-source progress included in this checked snapshot.
    pub const fn source_checkpoint(&self) -> StockSourceCheckpointRef {
        self.sources
    }
    /// Returns public-source progress checked for original source existence.
    /// This is not current-head or offer actionability permission.
    pub const fn listing_checkpoint(&self) -> StockListingCheckpointRef {
        self.listings
    }
    /// Returns the usable clock observation of the final transaction access check.
    pub const fn observed_at(&self) -> UnixSeconds {
        self.observed_at
    }
    /// Returns unique known records in the selected order and null quote scopes.
    pub const fn known_records(&self) -> usize {
        self.records
    }
}
impl std::fmt::Debug for StockHistorySnapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("StockHistorySnapshot(<private coverage>)")
    }
}
/// Current verified access plus caller checkpoint data; not fresh authority.
pub(in crate::workspace::stock) struct InventoryRead<'a> {
    pub binding: &'a WorkspaceBinding,
    pub authority: &'a Authority,
    pub minimum: radroots_storage::inventory::InventoryCheckpointRef,
}
pub(in crate::workspace::stock) struct Prepared {
    pub records: BTreeMap<RecordId, DecodedRecord>,
    pub inventory: radroots_storage::inventory::InventoryCheckpointRef,
    inventory_proof: super::super::inventory::checkpoint::MinimumProof,
    inventory_state: super::super::InventoryCheckpoint,
    pub analysis: CausalAnalysis,
    pub checkpoint: Option<radroots_domain::CheckpointRef>,
    pub external: Vec<SourceId>,
    pub uninterpreted: Vec<SourceId>,
    ancestry: checkpoint::Proof,
    public: super::super::public_source::history::Witness,
    revision: u64,
    known: [u8; 32],
    unknown: [u8; 32],
}
fn hash(label: &[u8]) -> Sha256 {
    let mut value = Sha256::new();
    value.update(label);
    value
}
fn known_hash() -> Sha256 {
    hash(b"radroots.seller_stock.causal_known_snapshot.v1\0")
}
fn unknown_hash() -> Sha256 {
    hash(b"radroots.seller_stock.causal_unknown_snapshot.v1\0")
}
fn known_row(hash: &mut Sha256, revision: u64, id: RecordId, protected: &[u8]) {
    hash.update(revision.to_be_bytes());
    hash.update(id.as_bytes());
    hash.update((protected.len() as u64).to_be_bytes());
    hash.update(protected);
}
fn unknown_row(
    hash: &mut Sha256,
    revision: u64,
    envelope: EnvelopeEventId,
    part: u8,
    protected: &[u8],
) {
    hash.update(revision.to_be_bytes());
    hash.update(envelope.as_bytes());
    hash.update([part]);
    hash.update((protected.len() as u64).to_be_bytes());
    hash.update(protected);
}
async fn header_at(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    revision: u64,
) -> Result<record::RetainedHeader, OperationError> {
    let id: Vec<u8> = sqlx::query_scalar(
        "SELECT envelope_id FROM stock_source_evidence WHERE revision=? AND length(envelope_id)=32",
    )
    .bind(revision as i64)
    .fetch_optional(&mut *connection)
    .await
    .map_err(|_| OperationError::RecoveryRequired)?
    .ok_or(OperationError::RecoveryRequired)?;
    let envelope = EnvelopeEventId::from_bytes(
        id.try_into()
            .map_err(|_| OperationError::RecoveryRequired)?,
    );
    let header = record::read(connection, input, crypto, envelope)
        .await?
        .ok_or(OperationError::RecoveryRequired)?;
    if header.header.revision.get() != revision {
        return Err(OperationError::RecoveryRequired);
    }
    Ok(header)
}
async fn rumor_part(
    connection: &mut SqliteConnection,
    envelope: EnvelopeEventId,
    part: u8,
) -> Result<Option<Vec<u8>>, OperationError> {
    sqlx::query_scalar::<_,Option<Vec<u8>>>("SELECT CASE WHEN length(protected) BETWEEN 1 AND 262144 THEN protected END FROM stock_source_parts WHERE envelope_id=? AND role=2 AND part=?")
        .bind(envelope.as_bytes().as_slice()).bind(i64::from(part)).fetch_optional(connection).await.map_err(|_|OperationError::RecoveryRequired)?
        .map(|value|value.ok_or(OperationError::RecoveryRequired)).transpose()
}
async fn original_rumor(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    header: &record::Header,
    hash: &mut Sha256,
) -> Result<DecodedRumor, OperationError> {
    let envelope = header.manifest.envelope;
    let piece = header.manifest.pieces[2];
    let mut raw = Zeroizing::new(Vec::new());
    for part in 0..piece.parts {
        let protected = rumor_part(connection, envelope, part)
            .await?
            .ok_or(OperationError::RecoveryRequired)?;
        unknown_row(hash, header.revision.get(), envelope, part, &protected);
        let clear = crypto
            .open_stock_record(
                input,
                "source_part",
                &record::part_id(envelope, 2, part),
                &protected,
            )
            .await?;
        clear.expose_secret(|bytes| {
            let length = if part + 1 == piece.parts {
                piece.bytes as usize - (usize::from(piece.parts) - 1) * 16 * 1024
            } else {
                16 * 1024
            };
            if bytes.len() != length {
                return Err(OperationError::RecoveryRequired);
            }
            raw.extend_from_slice(bytes);
            Ok(())
        })?;
    }
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM stock_source_parts WHERE envelope_id=? AND role=2",
    )
    .bind(envelope.as_bytes().as_slice())
    .fetch_one(connection)
    .await
    .map_err(|_| OperationError::RecoveryRequired)?;
    if count != i64::from(piece.parts)
        || raw.len() != piece.bytes as usize
        || <[u8; 32]>::from(Sha256::digest(&*raw)) != piece.digest
    {
        return Err(OperationError::RecoveryRequired);
    }
    let rumor = DecodedRumor::decode(&raw).map_err(|_| OperationError::RecoveryRequired)?;
    if rumor.event_id() != header.manifest.rumor {
        return Err(OperationError::RecoveryRequired);
    }
    Ok(rumor)
}
pub(in crate::workspace::stock) async fn prepare(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    inventory_read: InventoryRead<'_>,
    scope: CausalScope,
    listing_minimum: StockListingCheckpointRef,
) -> Result<Prepared, OperationError> {
    let authority = inventory_read.authority;
    if inventory_read.minimum.authorization() != authority.authorization {
        return Err(WorkspaceError::StaleCheckpoint.into());
    }
    if scope.seller() != input.participant() {
        return Err(OperationError::Unauthorized);
    }
    let progress = authority
        .sources
        .ok_or(OperationError::UnsupportedOperation)?;
    let revision = progress.revision.get();
    let genesis = StockSourceCheckpointRef::from_host_metadata(
        input.authority(),
        Sequence::new(0).map_err(|_| OperationError::RecoveryRequired)?,
        None,
    )?;
    let ancestry = checkpoint::authenticate(
        connection,
        input,
        crypto,
        progress,
        checkpoint::Target::Minimum(genesis),
    )
    .await?;
    let mut records = BTreeMap::new();
    let mut known = known_hash();
    for revision in 1..=revision {
        let retained = header_at(connection, input, crypto, revision).await?;
        if let Some(id) = retained.header.manifest.record {
            let (raw, protected) = record::canonical(connection, input, crypto, id)
                .await?
                .ok_or(OperationError::RecoveryRequired)?;
            if Some(<[u8; 32]>::from(Sha256::digest(&*raw)))
                != retained.header.manifest.canonical_digest
            {
                return Err(OperationError::RecoveryRequired);
            }
            known_row(&mut known, revision, id, &protected);
            let decoded = radroots_protocol::commerce::decode(&raw)
                .map_err(|_| OperationError::RecoveryRequired)?;
            if scope.contains(decoded.content()) {
                records.insert(id, decoded);
                if records.len() > causal::MAX_CAUSAL_RECORDS {
                    return Err(WorkspaceError::Capacity.into());
                }
            }
        }
    }
    let inventory_state = authority
        .inventory
        .ok_or(OperationError::UnsupportedOperation)?;
    super::super::inventory::check_head(connection, inventory_state).await?;
    let inventory_proof = Box::pin(
        super::super::inventory::checkpoint::authenticate_minimum_with_local(
            connection,
            input,
            crypto,
            inventory_read.binding,
            authority,
            inventory_read.minimum,
            Some(super::super::inventory::checkpoint::LocalSources {
                scope,
                records: &mut records,
            }),
        ),
    )
    .await?;
    let inventory = radroots_storage::inventory::InventoryCheckpointRef::from_host_metadata(
        input.authority(),
        inventory_state.revision,
        inventory_state.head,
    )?
    .with_authorization(
        authority
            .authorization
            .ok_or(OperationError::RecoveryRequired)?,
    );
    let mut wanted: BTreeSet<_> = records
        .values()
        .flat_map(|record| record.content().header().parents().iter().copied())
        .collect();
    for id in records.keys() {
        wanted.remove(&SourceId::from_bytes(id.into_bytes()));
    }
    let mut external = BTreeSet::new();
    let mut uninterpreted = BTreeSet::new();
    let mut unknown = unknown_hash();
    for revision in 1..=revision {
        let retained = header_at(connection, input, crypto, revision).await?;
        if let Some(id) = retained.header.manifest.record {
            if wanted.contains(&SourceId::from_bytes(id.into_bytes())) {
                return Err(OperationError::InvalidEvidence);
            }
            continue;
        }
        let rumor =
            original_rumor(connection, input, crypto, &retained.header, &mut unknown).await?;
        if rumor.author() != scope.buyer() && rumor.author() != scope.seller() {
            continue;
        }
        let source = SourceId::from_bytes(rumor.event_id().into_bytes());
        if rumor.kind() == radroots_protocol::profile::COMMERCE_KIND {
            // Unknown content cannot supply trustworthy thread/order semantics.
            // Conservatively block this participant pair rather than assuming
            // it is unrelated and resurrecting an older actionable record.
            uninterpreted.insert(source);
            if uninterpreted.len() > causal::MAX_CAUSAL_SOURCES {
                return Err(WorkspaceError::Capacity.into());
            }
        } else if rumor.kind() == 14 && wanted.contains(&source) {
            external.insert(source);
        }
    }
    for source in &external {
        wanted.remove(source);
    }
    let public = super::super::public_source::history::prepare(
        connection,
        input,
        crypto,
        authority,
        listing_minimum,
        &wanted,
    )
    .await?;
    external.extend(public.present());
    let nodes: Vec<_> = records
        .iter()
        .map(|(id, record)| CausalRecord::new(*id, record.content()))
        .collect();
    let external: Vec<_> = external.into_iter().collect();
    let uninterpreted: Vec<_> = uninterpreted.into_iter().collect();
    let coverage = radroots_protocol::commerce::CapturedCausalCoverage::capture(
        scope,
        &nodes,
        &external,
        &uninterpreted,
    )
    .map_err(|error| match error {
        radroots_protocol::commerce::RecordWireError::LimitExceeded => {
            OperationError::Workspace(WorkspaceError::Capacity)
        }
        radroots_protocol::commerce::RecordWireError::InvalidInput => {
            OperationError::InvalidEvidence
        }
        _ => OperationError::RecoveryRequired,
    })?;
    let (analysis, checkpoint) = coverage.into_parts();
    Ok(Prepared {
        inventory,
        inventory_state,
        inventory_proof,
        records,
        analysis,
        checkpoint,
        external,
        uninterpreted,
        ancestry,
        public,
        revision,
        known: known.finalize().into(),
        unknown: unknown.finalize().into(),
    })
}
impl Prepared {
    pub(in crate::workspace::stock) async fn confirm(
        &self,
        connection: &mut SqliteConnection,
    ) -> Result<(), OperationError> {
        self.ancestry.confirm(connection).await?;
        super::super::inventory::check_head(connection, self.inventory_state).await?;
        self.inventory_proof.confirm(connection).await?;
        let mut known = known_hash();
        let mut unknown = unknown_hash();
        for revision in 1..=self.revision {
            let row = sqlx::query(
                "SELECT envelope_id,record_id FROM stock_source_evidence WHERE revision=?",
            )
            .bind(revision as i64)
            .fetch_optional(&mut *connection)
            .await
            .map_err(|_| OperationError::RecoveryRequired)?
            .ok_or(OperationError::RecoveryRequired)?;
            let envelope: Vec<u8> = row
                .try_get(0)
                .map_err(|_| OperationError::RecoveryRequired)?;
            let envelope = EnvelopeEventId::from_bytes(
                envelope
                    .try_into()
                    .map_err(|_| OperationError::RecoveryRequired)?,
            );
            let id: Option<Vec<u8>> = row
                .try_get(1)
                .map_err(|_| OperationError::RecoveryRequired)?;
            if let Some(id) = id {
                let id = RecordId::from_bytes(
                    id.try_into()
                        .map_err(|_| OperationError::RecoveryRequired)?,
                );
                let protected:Option<Vec<u8>>=sqlx::query_scalar("SELECT CASE WHEN length(protected) BETWEEN 1 AND 262144 THEN protected END FROM stock_source_records WHERE record_id=?").bind(id.as_bytes().as_slice()).fetch_optional(&mut *connection).await.map_err(|_|OperationError::RecoveryRequired)?.flatten();
                known_row(
                    &mut known,
                    revision,
                    id,
                    &protected.ok_or(OperationError::RecoveryRequired)?,
                );
            } else {
                let mut count = 0;
                for part in 0..4 {
                    if let Some(protected) = rumor_part(connection, envelope, part).await? {
                        unknown_row(&mut unknown, revision, envelope, part, &protected);
                        count += 1;
                    }
                }
                let actual: i64 = sqlx::query_scalar(
                    "SELECT count(*) FROM stock_source_parts WHERE envelope_id=? AND role=2",
                )
                .bind(envelope.as_bytes().as_slice())
                .fetch_one(&mut *connection)
                .await
                .map_err(|_| OperationError::RecoveryRequired)?;
                if count == 0 || count != actual {
                    return Err(OperationError::RecoveryRequired);
                }
            }
        }
        if <[u8; 32]>::from(known.finalize()) != self.known
            || <[u8; 32]>::from(unknown.finalize()) != self.unknown
        {
            return Err(OperationError::RecoveryRequired);
        }
        self.public.confirm(connection).await
    }
}
