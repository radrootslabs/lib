use super::*;
use sqlx::Row;
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Header {
    pub version: u8,
    #[serde(deserialize_with = "super::super::object")]
    pub manifest: Manifest,
    pub transaction: TransactionId,
    pub revision: Sequence,
    #[serde(deserialize_with = "super::super::required_nullable")]
    pub previous: Option<CheckpointDigest>,
    pub head: CheckpointDigest,
    pub grant: GrantId,
    pub principal: String,
    pub authorization: StockAuthorizationRef,
}
pub(super) struct RetainedHeader {
    pub header: Header,
    pub protected: Vec<u8>,
}
pub(super) fn encode(value: &impl Serialize) -> Result<Zeroizing<Vec<u8>>, OperationError> {
    let bytes =
        Zeroizing::new(serde_json::to_vec(value).map_err(|_| OperationError::StorageFailure)?);
    if bytes.len() > 16 * 1024 {
        return Err(OperationError::InvalidCommand);
    }
    Ok(bytes)
}
pub(super) fn head(
    input: &AttachmentInput,
    value: &Header,
) -> Result<CheckpointDigest, OperationError> {
    #[derive(Serialize)]
    struct Fields<'a> {
        version: u8,
        manifest: &'a Manifest,
        transaction: TransactionId,
        revision: Sequence,
        previous: Option<CheckpointDigest>,
        grant: GrantId,
        principal: &'a str,
        authorization: StockAuthorizationRef,
    }
    let fields = Fields {
        version: value.version,
        manifest: &value.manifest,
        transaction: value.transaction,
        revision: value.revision,
        previous: value.previous,
        grant: value.grant,
        principal: &value.principal,
        authorization: value.authorization,
    };
    let mut hash = Sha256::new();
    hash.update(b"radroots.seller_stock.source_chain.v1\0");
    hash.update(input.workspace_id().as_bytes());
    hash.update(input.authority().executor_id().as_bytes());
    hash.update(input.authority().generation().get().to_be_bytes());
    hash.update(input.route_digest().as_bytes());
    hash.update(encode(&fields)?.as_slice());
    Ok(CheckpointDigest::from_bytes(hash.finalize().into()))
}
impl Header {
    pub fn validate(&self, input: &AttachmentInput) -> Result<(), OperationError> {
        let known = self.manifest.profile == StockSourceProfile::Known;
        if self.version != 1
            || self.revision.get() == 0
            || (self.revision.get() == 1) != self.previous.is_none()
            || self.principal.is_empty()
            || self.principal.len() > 128
            || self.principal.chars().any(char::is_control)
            || known != self.manifest.record.is_some()
            || known != self.manifest.canonical_digest.is_some()
            || self.head != head(input, self)?
        {
            return Err(OperationError::RecoveryRequired);
        }
        for piece in self.manifest.pieces {
            if piece.bytes == 0
                || piece.bytes > 64 * 1024
                || u32::from(piece.parts) != piece.bytes.div_ceil(16 * 1024)
            {
                return Err(OperationError::RecoveryRequired);
            }
        }
        Ok(())
    }
    pub fn receipt(&self, input: &AttachmentInput) -> Result<StockSourceReceipt, OperationError> {
        Ok(StockSourceReceipt {
            transaction: self.transaction,
            envelope: self.manifest.envelope,
            record: self.manifest.record,
            profile: self.manifest.profile,
            checkpoint: StockSourceCheckpointRef::from_host_metadata(
                input.authority(),
                self.revision,
                Some(self.head),
            )?
            .with_authorization(self.authorization),
        })
    }
}
pub(super) async fn read(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    envelope: EnvelopeEventId,
) -> Result<Option<RetainedHeader>, OperationError> {
    let row=sqlx::query("SELECT record_id,transaction_id,revision,chain_head,CASE WHEN length(protected) BETWEEN 1 AND 262144 THEN protected END FROM stock_source_evidence WHERE envelope_id=?").bind(envelope.as_bytes().as_slice()).fetch_optional(connection).await.map_err(|_|OperationError::RecoveryRequired)?;
    let Some(row) = row else { return Ok(None) };
    let protected: Vec<u8> = row
        .try_get::<Option<Vec<u8>>, _>(4)
        .map_err(|_| OperationError::RecoveryRequired)?
        .ok_or(OperationError::RecoveryRequired)?;
    let clear = crypto
        .open_stock_record(input, "source_evidence", envelope.as_bytes(), &protected)
        .await?;
    let header: Header = clear
        .expose_secret(|bytes| super::super::closed_record(bytes))
        .map_err(|_| OperationError::RecoveryRequired)?;
    header.validate(input)?;
    let logical: Option<Vec<u8>> = row
        .try_get(0)
        .map_err(|_| OperationError::RecoveryRequired)?;
    let tx: Vec<u8> = row
        .try_get(1)
        .map_err(|_| OperationError::RecoveryRequired)?;
    let revision: i64 = row
        .try_get(2)
        .map_err(|_| OperationError::RecoveryRequired)?;
    let head: Vec<u8> = row
        .try_get(3)
        .map_err(|_| OperationError::RecoveryRequired)?;
    if header.manifest.envelope != envelope
        || logical != header.manifest.record.map(|id| id.as_bytes().to_vec())
        || tx != header.transaction.as_bytes()
        || revision <= 0
        || revision as u64 != header.revision.get()
        || head != header.head.as_bytes()
    {
        return Err(OperationError::RecoveryRequired);
    }
    Ok(Some(RetainedHeader { header, protected }))
}
pub(super) fn part_id(envelope: EnvelopeEventId, role: u8, part: u8) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"radroots.seller_stock.source_part_key.v1\0");
    hash.update(envelope.as_bytes());
    hash.update([role, part]);
    hash.finalize().into()
}
pub(super) async fn canonical(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    id: RecordId,
) -> Result<Option<(Zeroizing<Vec<u8>>, Vec<u8>)>, OperationError> {
    let row: Option<Option<Vec<u8>>> = sqlx::query_scalar("SELECT CASE WHEN length(protected) BETWEEN 1 AND 262144 THEN protected END FROM stock_source_records WHERE record_id=?")
        .bind(id.as_bytes().as_slice())
        .fetch_optional(connection)
        .await
        .map_err(|_| OperationError::RecoveryRequired)?;
    let Some(protected) = row else {
        return Ok(None);
    };
    let protected = protected.ok_or(OperationError::RecoveryRequired)?;
    let clear = crypto
        .open_stock_record(input, "source_record", id.as_bytes(), &protected)
        .await?;
    let canonical = clear.expose_secret(|bytes| {
        let decoded = radroots_protocol::commerce::decode(bytes)
            .map_err(|_| OperationError::RecoveryRequired)?;
        if decoded.record_id() != id {
            return Err(OperationError::RecoveryRequired);
        }
        let encoded = radroots_protocol::commerce::EncodedRecord::capture(decoded.content())
            .map_err(|_| OperationError::RecoveryRequired)?;
        if encoded.protected_wire() != bytes {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(Zeroizing::new(bytes.to_vec()))
    })?;
    Ok(Some((canonical, protected)))
}
pub(super) async fn pieces(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    header: &Header,
) -> Result<([Zeroizing<Vec<u8>>; 3], [u8; 32]), OperationError> {
    let mut pieces: [Zeroizing<Vec<u8>>; 3] = std::array::from_fn(|_| Zeroizing::new(Vec::new()));
    let mut witness = Sha256::new();
    witness.update(b"radroots.seller_stock.source_parts_snapshot.v1\0");
    let mut count = 0;
    for (role, piece) in header.manifest.pieces.iter().enumerate() {
        for part in 0..piece.parts {
            let protected:Vec<u8>=sqlx::query_scalar("SELECT protected FROM stock_source_parts WHERE envelope_id=? AND role=? AND part=? AND length(protected) BETWEEN 1 AND 262144").bind(header.manifest.envelope.as_bytes().as_slice()).bind(role as i64).bind(i64::from(part)).fetch_optional(&mut *connection).await.map_err(|_|OperationError::RecoveryRequired)?.ok_or(OperationError::RecoveryRequired)?;
            hash_part(&mut witness, role as u8, part, &protected);
            let clear = crypto
                .open_stock_record(
                    input,
                    "source_part",
                    &part_id(header.manifest.envelope, role as u8, part),
                    &protected,
                )
                .await?;
            clear.expose_secret(|bytes| {
                let expected = if part + 1 == piece.parts {
                    piece.bytes as usize - (usize::from(piece.parts) - 1) * 16 * 1024
                } else {
                    16 * 1024
                };
                if bytes.len() != expected {
                    return Err(OperationError::RecoveryRequired);
                }
                pieces[role].extend_from_slice(bytes);
                Ok(())
            })?;
            count += 1;
        }
        let actual: [u8; 32] = Sha256::digest(pieces[role].as_slice()).into();
        if pieces[role].len() != piece.bytes as usize || actual != piece.digest {
            return Err(OperationError::RecoveryRequired);
        }
    }
    let actual: i64 =
        sqlx::query_scalar("SELECT count(*) FROM stock_source_parts WHERE envelope_id=?")
            .bind(header.manifest.envelope.as_bytes().as_slice())
            .fetch_one(connection)
            .await
            .map_err(|_| OperationError::RecoveryRequired)?;
    if actual != count {
        return Err(OperationError::RecoveryRequired);
    }
    Ok((pieces, witness.finalize().into()))
}
fn hash_part(hash: &mut Sha256, role: u8, part: u8, protected: &[u8]) {
    hash.update([role, part]);
    hash.update((protected.len() as u64).to_be_bytes());
    hash.update(protected);
}
pub(super) async fn confirm_parts(
    connection: &mut SqliteConnection,
    header: &Header,
    expected: [u8; 32],
) -> Result<(), OperationError> {
    let mut hash = Sha256::new();
    hash.update(b"radroots.seller_stock.source_parts_snapshot.v1\0");
    let mut count = 0;
    for (role, piece) in header.manifest.pieces.iter().enumerate() {
        for part in 0..piece.parts {
            let protected:Vec<u8>=sqlx::query_scalar("SELECT protected FROM stock_source_parts WHERE envelope_id=? AND role=? AND part=? AND length(protected) BETWEEN 1 AND 262144").bind(header.manifest.envelope.as_bytes().as_slice()).bind(role as i64).bind(i64::from(part)).fetch_optional(&mut *connection).await.map_err(|_|OperationError::RecoveryRequired)?.ok_or(OperationError::RecoveryRequired)?;
            hash_part(&mut hash, role as u8, part, &protected);
            count += 1;
        }
    }
    let actual: i64 =
        sqlx::query_scalar("SELECT count(*) FROM stock_source_parts WHERE envelope_id=?")
            .bind(header.manifest.envelope.as_bytes().as_slice())
            .fetch_one(connection)
            .await
            .map_err(|_| OperationError::RecoveryRequired)?;
    let digest: [u8; 32] = hash.finalize().into();
    if digest != expected || actual != count {
        return Err(OperationError::RecoveryRequired);
    }
    Ok(())
}
