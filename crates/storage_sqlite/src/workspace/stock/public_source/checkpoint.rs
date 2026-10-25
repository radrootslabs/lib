use super::*;
use radroots_domain::{CheckpointDigest, Sequence};
use radroots_storage::{
    command::OperationError, workspace::seller_stock::StockListingCheckpointRef,
};
use sha2::{Digest, Sha256};
use sqlx::Row;

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) struct Identity {
    pub revision: Sequence,
    pub head: CheckpointDigest,
    pub protected: [u8; 32],
}
pub(super) enum Target {
    Minimum(StockListingCheckpointRef),
    Event(EnvelopeEventId, Option<Identity>),
    Coordinate(ListingCoordinate, Option<record::Head>),
}
pub(super) struct Proof {
    lower: u64,
    upper: u64,
    digest: [u8; 32],
}
fn hash() -> Sha256 {
    let mut h = Sha256::new();
    h.update(b"radroots.seller_stock.public_source_ancestry_snapshot.v1\0");
    h
}
fn hash_retained(hash: &mut Sha256, value: &record::RetainedHeader) -> Result<(), OperationError> {
    let header = &value.header;
    let listing = header.manifest.listing;
    hash.update(header.revision.get().to_be_bytes());
    hash.update(listing.event_id().as_bytes());
    hash.update(listing.coordinate().author().as_bytes());
    hash.update(listing.coordinate().d().as_bytes());
    hash.update(record::time(&header.manifest.created_at)?.to_be_bytes());
    hash.update(header.transaction.as_bytes());
    hash.update(
        header
            .head
            .ok_or(OperationError::RecoveryRequired)?
            .as_bytes(),
    );
    hash.update((value.protected.len() as u64).to_be_bytes());
    hash.update(&value.protected);
    Ok(())
}
impl record::RetainedHeader {
    pub(super) fn identity(&self) -> Result<Identity, OperationError> {
        Ok(Identity {
            revision: self.header.revision,
            head: self.header.head.ok_or(OperationError::RecoveryRequired)?,
            protected: Sha256::digest(&self.protected).into(),
        })
    }
}
impl Proof {
    /// Exact bounded-row witnesses are checked in the actual transaction,
    /// after every provider callback and before reporting authenticated state.
    pub async fn confirm(&self, connection: &mut SqliteConnection) -> Result<(), OperationError> {
        let mut hash = hash();
        for revision in (self.lower..=self.upper).rev() {
            let row=sqlx::query("SELECT event_id,listing_author,listing_d,created_at,transaction_id,chain_head,CASE WHEN length(protected) BETWEEN 1 AND 262144 THEN protected END FROM stock_public_events WHERE revision=?")
                .bind(revision as i64).fetch_optional(&mut *connection).await.map_err(|_|OperationError::RecoveryRequired)?.ok_or(OperationError::RecoveryRequired)?;
            hash.update(revision.to_be_bytes());
            for (column, length) in [(0, 32), (1, 32), (2, 16), (3, 8), (4, 32), (5, 32)] {
                let value: Vec<u8> = row
                    .try_get(column)
                    .map_err(|_| OperationError::RecoveryRequired)?;
                if value.len() != length {
                    return Err(OperationError::RecoveryRequired);
                }
                hash.update(value);
            }
            let protected: Vec<u8> = row
                .try_get::<Option<Vec<u8>>, _>(6)
                .map_err(|_| OperationError::RecoveryRequired)?
                .ok_or(OperationError::RecoveryRequired)?;
            hash.update((protected.len() as u64).to_be_bytes());
            hash.update(protected);
        }
        if <[u8; 32]>::from(hash.finalize()) != self.digest {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(())
    }
}
pub(super) async fn authenticate(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    authority: &Authority,
    target: Target,
) -> Result<Proof, OperationError> {
    let current = authority
        .public_sources
        .ok_or(OperationError::UnsupportedOperation)?;
    if let Target::Minimum(minimum) = &target {
        if minimum.authority() != input.authority() {
            return Err(WorkspaceError::StaleAuthority.into());
        }
        if minimum.revision() > current.revision {
            return Err(WorkspaceError::StaleCheckpoint.into());
        }
    }
    let upper = current.revision.get();
    let mut previous = current.head;
    let mut hash = hash();
    for revision in (1..=upper).rev() {
        let id: Vec<u8> = sqlx::query_scalar(
            "SELECT event_id FROM stock_public_events WHERE revision=? AND length(event_id)=32",
        )
        .bind(revision as i64)
        .fetch_optional(&mut *connection)
        .await
        .map_err(|_| OperationError::RecoveryRequired)?
        .ok_or(OperationError::RecoveryRequired)?;
        let event = EnvelopeEventId::from_bytes(
            id.try_into()
                .map_err(|_| OperationError::RecoveryRequired)?,
        );
        let original = record::read(connection, input, crypto, authority.input.grant_id(), event)
            .await?
            .ok_or(OperationError::RecoveryRequired)?;
        let header = &original.header;
        if header.revision.get() != revision || header.head != previous {
            return Err(OperationError::RecoveryRequired);
        }
        hash_retained(&mut hash, &original)?;
        let matched = match &target {
            Target::Minimum(minimum) if minimum.revision().get() == revision => {
                if minimum.head() != header.head {
                    return Err(WorkspaceError::StaleCheckpoint.into());
                }
                true
            }
            Target::Event(wanted, expected) if *wanted == event => {
                if *expected != Some(original.identity()?) {
                    return Err(OperationError::RecoveryRequired);
                }
                true
            }
            Target::Coordinate(coordinate, expected)
                if *coordinate == header.manifest.listing.coordinate() =>
            {
                if expected.as_ref() != Some(&header.after) {
                    return Err(OperationError::RecoveryRequired);
                }
                true
            }
            _ => false,
        };
        if matched {
            return Ok(Proof {
                lower: revision,
                upper,
                digest: hash.finalize().into(),
            });
        }
        previous = header.previous;
    }
    if previous.is_some() {
        return Err(OperationError::RecoveryRequired);
    }
    match target {
        Target::Minimum(value) if value.revision().get() == 0 => {}
        Target::Event(_, None) | Target::Coordinate(_, None) => {}
        _ => return Err(OperationError::RecoveryRequired),
    }
    Ok(Proof {
        lower: 1,
        upper,
        digest: hash.finalize().into(),
    })
}
