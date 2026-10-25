use super::*;
use sqlx::Row;
pub(in crate::workspace::stock) enum Target {
    Minimum(StockSourceCheckpointRef),
    Envelope(
        EnvelopeEventId,
        Option<(Sequence, CheckpointDigest, [u8; 32])>,
    ),
    Record(RecordId, Option<[u8; 32]>),
}
pub(in crate::workspace::stock) struct Proof {
    lower: u64,
    upper: u64,
    digest: [u8; 32],
}
fn hash() -> Sha256 {
    let mut value = Sha256::new();
    value.update(b"radroots.seller_stock.source_ancestry_snapshot.v1\0");
    value
}
fn hash_row(
    hash: &mut Sha256,
    revision: u64,
    envelope: &[u8],
    record: Option<&[u8]>,
    transaction: &[u8],
    head: &[u8],
    protected: &[u8],
) {
    hash.update(revision.to_be_bytes());
    hash.update(envelope);
    hash.update([u8::from(record.is_some())]);
    if let Some(record) = record {
        hash.update(record);
    }
    hash.update(transaction);
    hash.update(head);
    hash.update((protected.len() as u64).to_be_bytes());
    hash.update(protected);
}
impl Proof {
    pub async fn confirm(&self, connection: &mut SqliteConnection) -> Result<(), OperationError> {
        let mut hash = hash();
        for revision in (self.lower..=self.upper).rev() {
            let row=sqlx::query("SELECT envelope_id,record_id,transaction_id,chain_head,CASE WHEN length(protected) BETWEEN 1 AND 262144 THEN protected END FROM stock_source_evidence WHERE revision=?").bind(revision as i64).fetch_optional(&mut *connection).await.map_err(|_|OperationError::RecoveryRequired)?.ok_or(OperationError::RecoveryRequired)?;
            let envelope: Vec<u8> = row
                .try_get(0)
                .map_err(|_| OperationError::RecoveryRequired)?;
            let record: Option<Vec<u8>> = row
                .try_get(1)
                .map_err(|_| OperationError::RecoveryRequired)?;
            let tx: Vec<u8> = row
                .try_get(2)
                .map_err(|_| OperationError::RecoveryRequired)?;
            let head: Vec<u8> = row
                .try_get(3)
                .map_err(|_| OperationError::RecoveryRequired)?;
            let protected: Vec<u8> = row
                .try_get::<Option<Vec<u8>>, _>(4)
                .map_err(|_| OperationError::RecoveryRequired)?
                .ok_or(OperationError::RecoveryRequired)?;
            if envelope.len() != 32
                || tx.len() != 32
                || head.len() != 32
                || record.as_ref().is_some_and(|value| value.len() != 32)
            {
                return Err(OperationError::RecoveryRequired);
            }
            hash_row(
                &mut hash,
                revision,
                &envelope,
                record.as_deref(),
                &tx,
                &head,
                &protected,
            );
        }
        let actual: [u8; 32] = hash.finalize().into();
        if actual != self.digest {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(())
    }
}
pub(in crate::workspace::stock) async fn authenticate(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    current: SourceCheckpoint,
    target: Target,
) -> Result<Proof, OperationError> {
    let upper = current.revision.get();
    if let Target::Minimum(minimum) = target
        && (minimum.authority() != input.authority() || minimum.revision().get() > upper)
    {
        return Err(WorkspaceError::StaleCheckpoint.into());
    }
    let mut expected = current.head;
    let mut hash = hash();
    for revision in (1..=upper).rev() {
        let id:Vec<u8>=sqlx::query_scalar("SELECT envelope_id FROM stock_source_evidence WHERE revision=? AND length(envelope_id)=32").bind(revision as i64).fetch_optional(&mut *connection).await.map_err(|_|OperationError::RecoveryRequired)?.ok_or(OperationError::RecoveryRequired)?;
        let envelope = EnvelopeEventId::from_bytes(
            id.try_into()
                .map_err(|_| OperationError::RecoveryRequired)?,
        );
        let retained = record::read(connection, input, crypto, envelope)
            .await?
            .ok_or(OperationError::RecoveryRequired)?;
        let header = &retained.header;
        if header.revision.get() != revision || Some(header.head) != expected {
            return Err(OperationError::RecoveryRequired);
        }
        hash_row(
            &mut hash,
            revision,
            envelope.as_bytes(),
            header
                .manifest
                .record
                .as_ref()
                .map(|id| id.as_bytes().as_slice()),
            header.transaction.as_bytes(),
            header.head.as_bytes(),
            &retained.protected,
        );
        let matched = match &target {
            Target::Minimum(minimum) if minimum.revision().get() == revision => {
                if minimum.head() != Some(header.head) {
                    return Err(WorkspaceError::StaleCheckpoint.into());
                }
                true
            }
            Target::Envelope(id, observed) if *id == envelope => {
                let digest: [u8; 32] = Sha256::digest(&retained.protected).into();
                if *observed != Some((header.revision, header.head, digest)) {
                    return Err(OperationError::RecoveryRequired);
                }
                true
            }
            Target::Record(id, observed) if header.manifest.record == Some(*id) => {
                if *observed != header.manifest.canonical_digest {
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
        expected = header.previous;
    }
    let absent = match target {
        Target::Minimum(value) => value.revision().get() == 0 && value.head().is_none(),
        Target::Envelope(_, value) => value.is_none(),
        Target::Record(_, value) => value.is_none(),
    };
    if expected.is_some() || !absent {
        return Err(OperationError::RecoveryRequired);
    }
    Ok(Proof {
        lower: 1,
        upper,
        digest: hash.finalize().into(),
    })
}
