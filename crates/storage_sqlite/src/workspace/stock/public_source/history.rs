//! Batched historical identity checks for causal reads. Historical existence
//! never grants permission to request against an old or unsupported offer.
use super::*;
use checkpoint::{Proof, Target};
use radroots_domain::{Sequence, SourceId};
use radroots_storage::{
    command::OperationError, workspace::seller_stock::StockListingCheckpointRef,
};
use std::collections::BTreeSet;

pub(in crate::workspace::stock) struct Witness {
    minimum: Proof,
    all: Proof,
    progress: PublicSourceCheckpoint,
    originals: Vec<(EnvelopeEventId, u8, [u8; 32])>,
    present: Vec<SourceId>,
}
impl Witness {
    pub(in crate::workspace::stock) fn present(&self) -> &[SourceId] {
        &self.present
    }
    pub(in crate::workspace::stock) async fn confirm(
        &self,
        connection: &mut SqliteConnection,
    ) -> Result<(), OperationError> {
        counts(connection, self.progress).await?;
        self.minimum.confirm(connection).await?;
        self.all.confirm(connection).await?;
        for (event, count, digest) in &self.originals {
            if record::parts_witness(connection, *event).await? != (*count, *digest) {
                return Err(OperationError::RecoveryRequired);
            }
        }
        Ok(())
    }
}
pub(in crate::workspace::stock) async fn prepare(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    authority: &Authority,
    minimum: StockListingCheckpointRef,
    wanted: &BTreeSet<SourceId>,
) -> Result<Witness, OperationError> {
    if wanted.len() > radroots_domain::causal::MAX_CAUSAL_SOURCES {
        return Err(WorkspaceError::Capacity.into());
    }
    if minimum.authorization() != authority.authorization {
        return Err(WorkspaceError::StaleCheckpoint.into());
    }
    let progress = authority
        .public_sources
        .ok_or(OperationError::UnsupportedOperation)?;
    let minimum = checkpoint::authenticate(
        connection,
        input,
        crypto,
        authority,
        Target::Minimum(minimum),
    )
    .await?;
    // One complete protected walk proves all requested presence/absence claims.
    // Repeating a full walk independently for every missing parent would make
    // scoped reads multiply unrelated public-history cost by the parent count.
    let genesis = StockListingCheckpointRef::from_host_metadata(
        input.authority(),
        Sequence::new(0).map_err(|_| OperationError::RecoveryRequired)?,
        None,
    )?;
    let all = checkpoint::authenticate(
        connection,
        input,
        crypto,
        authority,
        Target::Minimum(genesis),
    )
    .await?;
    let mut originals = Vec::new();
    let mut present = Vec::new();
    for source in wanted {
        let event = EnvelopeEventId::from_bytes(source.into_bytes());
        if let Some(retained) =
            record::read(connection, input, crypto, authority.input.grant_id(), event).await?
        {
            let parts = record::read_parts(connection, input, crypto, &retained.header).await?;
            originals.push((event, retained.header.manifest.parts, parts.witness));
            present.push(*source);
        }
    }
    Ok(Witness {
        minimum,
        all,
        progress,
        originals,
        present,
    })
}
