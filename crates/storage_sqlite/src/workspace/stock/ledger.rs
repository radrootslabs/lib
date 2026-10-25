//! Bounded multipart operation framing for the multi-pool native ledger.
//! Framing authenticates original storage bytes; family-specific admission
//! must independently validate every business or physical effect.
use super::*;
use radroots_domain::{ActorRef, CheckpointDigest, CommandDigest, OperationId, PoolId};
use radroots_storage::command::OperationError;
use sha2::{Digest, Sha256};
use sqlx::Row;
use zeroize::Zeroizing;

pub(super) const BODY_MAX_BYTES: usize = 256 * 1024;
const PART_BYTES: usize = 16 * 1024;
const MAX_PARTS: usize = BODY_MAX_BYTES / PART_BYTES;

#[derive(Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Family {
    Inventory,
    Consent,
    Acceptance,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Header {
    pub version: u8,
    pub family: Family,
    #[serde(deserialize_with = "super::object")]
    pub namespace: SellerAuthoritySelection,
    pub actor: ActorRef,
    pub operation: OperationId,
    pub digest: CommandDigest,
    pub transaction: TransactionId,
    pub revision: Sequence,
    #[serde(deserialize_with = "super::required_nullable")]
    pub previous: Option<CheckpointDigest>,
    pub head: CheckpointDigest,
    #[serde(deserialize_with = "super::required_nullable")]
    pub pool: Option<PoolId>,
    #[serde(deserialize_with = "super::required_nullable")]
    pub order: Option<radroots_domain::OrderId>,
    #[serde(deserialize_with = "super::required_nullable")]
    pub related_operation: Option<OperationId>,
    #[serde(deserialize_with = "super::required_nullable")]
    pub subject_grant: Option<GrantId>,
    pub bytes: u32,
    pub parts: u8,
    pub body_digest: [u8; 32],
}
impl Header {
    pub fn validate(
        &self,
        input: &AttachmentInput,
        operation: OperationId,
    ) -> Result<(), OperationError> {
        if self.version != 5
            || !self.namespace.matches_attachment(input)
            || self.actor.participant() != input.participant()
            || self.actor.role() != radroots_domain::ActorRole::Seller
            || self.actor.organization().is_some()
            || self.actor.grant_id().is_none()
            || self.operation != operation
            || self.revision.get() == 0
            || (self.revision.get() == 1) != self.previous.is_none()
            || (self.family == Family::Inventory) != self.pool.is_some()
            || (self.family == Family::Inventory) != self.order.is_none()
            || (self.family == Family::Consent) != self.related_operation.is_some()
            || (self.family == Family::Consent) != self.subject_grant.is_some()
            || self.bytes == 0
            || self.bytes as usize > BODY_MAX_BYTES
            || usize::from(self.parts) != (self.bytes as usize).div_ceil(PART_BYTES)
        {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(())
    }
}

pub(super) struct Prepared {
    pub protected: Vec<u8>,
    parts: Vec<Vec<u8>>,
}
pub(super) struct Retained {
    pub header: Header,
    pub body: Zeroizing<Vec<u8>>,
    pub protected: Vec<u8>,
    pub parts_witness: [u8; 32],
}
fn part_id(actor: ActorRef, operation: OperationId, part: u8) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"radroots.seller_stock.operation_part_key.v1\0");
    hash.update(actor.participant().as_bytes());
    hash.update(operation.as_bytes());
    hash.update([part]);
    hash.finalize().into()
}
fn part_hash() -> Sha256 {
    let mut hash = Sha256::new();
    hash.update(b"radroots.seller_stock.operation_parts_snapshot.v1\0");
    hash
}
fn hash_part(hash: &mut Sha256, part: u8, grant: &[u8; 32], protected: &[u8]) {
    hash.update([part]);
    hash.update(grant);
    hash.update((protected.len() as u64).to_be_bytes());
    hash.update(protected);
}

pub(super) async fn prepare(
    crypto: &WorkspaceCrypto,
    input: &AttachmentInput,
    mut header: Header,
    body: &[u8],
) -> Result<Prepared, OperationError> {
    if body.is_empty() || body.len() > BODY_MAX_BYTES {
        return Err(WorkspaceError::Capacity.into());
    }
    header.bytes = body.len() as u32;
    header.parts = body.len().div_ceil(PART_BYTES) as u8;
    header.body_digest = Sha256::digest(body).into();
    header.validate(input, header.operation)?;
    let mut parts = Vec::with_capacity(usize::from(header.parts));
    for (index, clear) in body.chunks(PART_BYTES).enumerate() {
        parts.push(
            crypto
                .seal_stock_record(
                    input,
                    "operation_part",
                    &part_id(header.actor, header.operation, index as u8),
                    clear,
                )
                .await?,
        );
    }
    let clear =
        Zeroizing::new(serde_json::to_vec(&header).map_err(|_| OperationError::StorageFailure)?);
    if clear.len() > PART_BYTES {
        return Err(WorkspaceError::Capacity.into());
    }
    let protected = crypto
        .seal_stock_record_for_catalog(input, "operation", header.operation.as_bytes(), 7, &clear)
        .await?;
    Ok(Prepared { protected, parts })
}
impl Prepared {
    /// Called only within the existing owner's SQL transaction. Protection and
    /// provider callbacks have already completed before entering this method.
    pub async fn insert_parts(
        &self,
        connection: &mut SqliteConnection,
        actor: ActorRef,
        operation: OperationId,
    ) -> Result<(), OperationError> {
        let grant = actor.grant_id().ok_or(OperationError::RecoveryRequired)?;
        for (index, protected) in self.parts.iter().enumerate() {
            sqlx::query("INSERT INTO stock_operation_parts(actor,grant_id,operation_id,part,protected) VALUES(?,?,?,?,?)")
                .bind(actor.participant().as_bytes().as_slice())
                .bind(grant.as_bytes().as_slice())
                .bind(operation.as_bytes().as_slice())
                .bind(index as i64)
                .bind(protected)
                .execute(&mut *connection)
                .await
                .map_err(|_|OperationError::StorageFailure)?;
        }
        Ok(())
    }
}

pub(super) async fn read(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    operation: OperationId,
) -> Result<Option<Retained>, OperationError> {
    let row=sqlx::query("SELECT grant_id,command_digest,transaction_id,pool_id,revision,chain_head,CASE WHEN length(protected) BETWEEN 1 AND 262144 THEN protected END,family,order_id,related_operation,subject_grant FROM stock_operations WHERE actor=? AND operation_id=?")
        .bind(input.participant().as_bytes().as_slice()).bind(operation.as_bytes().as_slice())
        .fetch_optional(&mut *connection).await.map_err(|_|OperationError::RecoveryRequired)?;
    let Some(row) = row else { return Ok(None) };
    let protected: Vec<u8> = row
        .try_get::<Option<Vec<u8>>, _>(6)
        .map_err(|_| OperationError::RecoveryRequired)?
        .ok_or(OperationError::RecoveryRequired)?;
    let clear = crypto
        .open_stock_record_for_catalog(input, "operation", operation.as_bytes(), 7, &protected)
        .await?;
    let header: Header = clear.expose_secret(|bytes| super::closed_record(bytes))?;
    header.validate(input, operation)?;
    let grant: Vec<u8> = row
        .try_get(0)
        .map_err(|_| OperationError::RecoveryRequired)?;
    let digest: Vec<u8> = row
        .try_get(1)
        .map_err(|_| OperationError::RecoveryRequired)?;
    let transaction: Vec<u8> = row
        .try_get(2)
        .map_err(|_| OperationError::RecoveryRequired)?;
    let pool: Option<Vec<u8>> = row
        .try_get(3)
        .map_err(|_| OperationError::RecoveryRequired)?;
    let revision: i64 = row
        .try_get(4)
        .map_err(|_| OperationError::RecoveryRequired)?;
    let head: Vec<u8> = row
        .try_get(5)
        .map_err(|_| OperationError::RecoveryRequired)?;
    let family: i64 = row
        .try_get(7)
        .map_err(|_| OperationError::RecoveryRequired)?;
    let order: Option<Vec<u8>> = row
        .try_get(8)
        .map_err(|_| OperationError::RecoveryRequired)?;
    let related: Option<Vec<u8>> = row
        .try_get(9)
        .map_err(|_| OperationError::RecoveryRequired)?;
    let subject: Option<Vec<u8>> = row
        .try_get(10)
        .map_err(|_| OperationError::RecoveryRequired)?;
    if header
        .actor
        .grant_id()
        .map(|value| value.as_bytes().to_vec())
        != Some(grant)
        || digest != header.digest.as_bytes()
        || transaction != header.transaction.as_bytes()
        || pool != header.pool.map(|value| value.as_bytes().to_vec())
        || revision <= 0
        || revision as u64 != header.revision.get()
        || head != header.head.as_bytes()
        || family != header.family.code()
        || order != header.order.map(|value| value.as_bytes().to_vec())
        || subject != header.subject_grant.map(|value| value.as_bytes().to_vec())
        || related
            != header
                .related_operation
                .map(|value| value.as_bytes().to_vec())
    {
        return Err(OperationError::RecoveryRequired);
    }
    let mut body = Zeroizing::new(Vec::with_capacity(header.bytes as usize));
    let mut witness = part_hash();
    for index in 0..header.parts {
        let protected = read_part(connection, header.actor, operation, index).await?;
        hash_part(
            &mut witness,
            index,
            header
                .actor
                .grant_id()
                .ok_or(OperationError::RecoveryRequired)?
                .as_bytes(),
            &protected,
        );
        let clear = crypto
            .open_stock_record(
                input,
                "operation_part",
                &part_id(header.actor, operation, index),
                &protected,
            )
            .await?;
        clear.expose_secret(|bytes| {
            let expected = if index + 1 == header.parts {
                header.bytes as usize - usize::from(index) * PART_BYTES
            } else {
                PART_BYTES
            };
            if bytes.len() != expected {
                return Err(OperationError::RecoveryRequired);
            }
            body.extend_from_slice(bytes);
            Ok(())
        })?;
    }
    if body.len() != header.bytes as usize
        || <[u8; 32]>::from(Sha256::digest(body.as_slice())) != header.body_digest
    {
        return Err(OperationError::RecoveryRequired);
    }
    let parts_witness = witness.finalize().into();
    confirm_parts(
        connection,
        header.actor,
        operation,
        parts_witness,
        header.parts,
    )
    .await?;
    Ok(Some(Retained {
        header,
        body,
        protected,
        parts_witness,
    }))
}
impl Family {
    pub const fn code(self) -> i64 {
        match self {
            Self::Inventory => 0,
            Self::Consent => 1,
            Self::Acceptance => 2,
        }
    }
}
pub(super) async fn confirm_header(
    connection: &mut SqliteConnection,
    header: &Header,
    protected: &[u8],
) -> Result<(), OperationError> {
    let matched:i64=sqlx::query_scalar("SELECT count(*) FROM stock_operations WHERE actor=? AND grant_id=? AND operation_id=? AND command_digest=? AND transaction_id=? AND pool_id IS ? AND revision=? AND chain_head=? AND protected=? AND family=? AND order_id IS ? AND related_operation IS ? AND subject_grant IS ?")
        .bind(header.actor.participant().as_bytes().as_slice())
        .bind(header.actor.grant_id().ok_or(OperationError::RecoveryRequired)?.as_bytes().as_slice())
        .bind(header.operation.as_bytes().as_slice()).bind(header.digest.as_bytes().as_slice())
        .bind(header.transaction.as_bytes().as_slice()).bind(header.pool.map(|v|v.as_bytes().to_vec()))
        .bind(header.revision.get() as i64).bind(header.head.as_bytes().as_slice()).bind(protected)
        .bind(header.family.code()).bind(header.order.map(|v|v.as_bytes().to_vec()))
        .bind(header.related_operation.map(|v|v.as_bytes().to_vec())).bind(header.subject_grant.map(|v|v.as_bytes().to_vec())).fetch_one(connection).await.map_err(|_|OperationError::RecoveryRequired)?;
    if matched != 1 {
        return Err(OperationError::RecoveryRequired);
    }
    Ok(())
}
async fn read_part(
    connection: &mut SqliteConnection,
    actor: ActorRef,
    operation: OperationId,
    part: u8,
) -> Result<Vec<u8>, OperationError> {
    let row=sqlx::query("SELECT grant_id,protected FROM stock_operation_parts WHERE actor=? AND operation_id=? AND part=? AND length(protected) BETWEEN 1 AND 262144")
        .bind(actor.participant().as_bytes().as_slice()).bind(operation.as_bytes().as_slice()).bind(i64::from(part))
        .fetch_optional(connection).await.map_err(|_|OperationError::RecoveryRequired)?.ok_or(OperationError::RecoveryRequired)?;
    let grant: Vec<u8> = row
        .try_get(0)
        .map_err(|_| OperationError::RecoveryRequired)?;
    if grant
        != actor
            .grant_id()
            .ok_or(OperationError::RecoveryRequired)?
            .as_bytes()
    {
        return Err(OperationError::RecoveryRequired);
    }
    row.try_get(1).map_err(|_| OperationError::RecoveryRequired)
}
pub(super) async fn confirm_parts(
    connection: &mut SqliteConnection,
    actor: ActorRef,
    operation: OperationId,
    expected: [u8; 32],
    parts: u8,
) -> Result<(), OperationError> {
    if parts == 0 || usize::from(parts) > MAX_PARTS {
        return Err(OperationError::RecoveryRequired);
    }
    let (count, actual) = raw_parts_witness(
        connection,
        actor.participant().as_bytes(),
        operation.as_bytes(),
    )
    .await?;
    if count != parts || actual != expected {
        return Err(OperationError::RecoveryRequired);
    }
    Ok(())
}
pub(super) async fn raw_parts_witness(
    connection: &mut SqliteConnection,
    actor: &[u8; 32],
    operation: &[u8; 32],
) -> Result<(u8, [u8; 32]), OperationError> {
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM stock_operation_parts WHERE actor=? AND operation_id=?",
    )
    .bind(actor.as_slice())
    .bind(operation.as_slice())
    .fetch_one(&mut *connection)
    .await
    .map_err(|_| OperationError::RecoveryRequired)?;
    if count <= 0 || count > MAX_PARTS as i64 {
        return Err(OperationError::RecoveryRequired);
    }
    let mut hash = part_hash();
    for part in 0..count {
        let row=sqlx::query("SELECT grant_id,protected FROM stock_operation_parts WHERE actor=? AND operation_id=? AND part=? AND length(protected) BETWEEN 1 AND 262144")
            .bind(actor.as_slice()).bind(operation.as_slice()).bind(part).fetch_optional(&mut *connection).await.map_err(|_|OperationError::RecoveryRequired)?.ok_or(OperationError::RecoveryRequired)?;
        let grant: Vec<u8> = row
            .try_get(0)
            .map_err(|_| OperationError::RecoveryRequired)?;
        let grant: [u8; 32] = grant
            .try_into()
            .map_err(|_| OperationError::RecoveryRequired)?;
        let protected: Vec<u8> = row
            .try_get(1)
            .map_err(|_| OperationError::RecoveryRequired)?;
        hash_part(&mut hash, part as u8, &grant, &protected);
    }
    Ok((count as u8, hash.finalize().into()))
}

pub(super) struct MovementWitness {
    pub id: [u8; 32],
    pub pool: PoolId,
    pub grant: GrantId,
    pub protected: Vec<u8>,
}
/// A completely decoded original effect. Only the owning family decoder
/// supplies its pool effects and movement witnesses; SQL indexes do not.
pub(super) struct HistoryEntry {
    pub header: Header,
    pub protected: Vec<u8>,
    pub parts_witness: [u8; 32],
    pub pools: Vec<radroots_domain::stock::PoolSnapshot>,
    pub mapping: Option<radroots_domain::stock::OptionPoolMapping>,
    pub movements: Vec<MovementWitness>,
}
pub(super) async fn history(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    binding: &WorkspaceBinding,
    authority: &Authority,
    operation: OperationId,
) -> Result<Option<HistoryEntry>, OperationError> {
    let family: Option<i64> =
        sqlx::query_scalar("SELECT family FROM stock_operations WHERE actor=? AND operation_id=?")
            .bind(input.participant().as_bytes().as_slice())
            .bind(operation.as_bytes().as_slice())
            .fetch_optional(&mut *connection)
            .await
            .map_err(|_| OperationError::RecoveryRequired)?;
    match family {
        None => Ok(None),
        Some(0) => {
            let retained =
                super::inventory::read_operation(connection, input, crypto, binding, operation)
                    .await?
                    .ok_or(OperationError::RecoveryRequired)?;
            let (header, parts_witness) = retained.frame.ok_or(OperationError::RecoveryRequired)?;
            let movements = match (retained.movement, retained.movement_protected) {
                (Some(id), Some(protected)) => vec![MovementWitness {
                    id,
                    pool: retained.receipt.pool().id(),
                    grant: retained.original_grant,
                    protected,
                }],
                (None, None) => vec![],
                _ => return Err(OperationError::RecoveryRequired),
            };
            Ok(Some(HistoryEntry {
                header,
                protected: retained.protected,
                parts_witness,
                pools: vec![retained.receipt.pool()],
                mapping: retained.receipt.mapping(),
                movements,
            }))
        }
        Some(1) => {
            let retained = super::consent::read(
                connection,
                input,
                crypto,
                authority.input.grant_id(),
                operation,
            )
            .await?
            .ok_or(OperationError::RecoveryRequired)?;
            let count: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM stock_movements WHERE actor=? AND operation_id=?",
            )
            .bind(input.participant().as_bytes().as_slice())
            .bind(operation.as_bytes().as_slice())
            .fetch_one(connection)
            .await
            .map_err(|_| OperationError::RecoveryRequired)?;
            if count != 0 {
                return Err(OperationError::RecoveryRequired);
            }
            Ok(Some(HistoryEntry {
                header: retained.frame.header,
                protected: retained.frame.protected,
                parts_witness: retained.frame.parts_witness,
                pools: vec![],
                mapping: None,
                movements: vec![],
            }))
        }
        Some(2) => Err(OperationError::UnsupportedOperation),
        Some(_) => Err(OperationError::RecoveryRequired),
    }
}
pub(super) fn hash_optional(hash: &mut Sha256, value: Option<&[u8]>) -> Result<(), OperationError> {
    hash.update([u8::from(value.is_some())]);
    if let Some(bytes) = value {
        if bytes.len() != 32 {
            return Err(OperationError::RecoveryRequired);
        }
        hash.update(bytes);
    }
    Ok(())
}
impl HistoryEntry {
    pub fn hash(&self, hash: &mut Sha256) -> Result<(), OperationError> {
        let header = &self.header;
        hash.update(header.revision.get().to_be_bytes());
        hash.update(header.actor.participant().as_bytes());
        hash.update(
            header
                .actor
                .grant_id()
                .ok_or(OperationError::RecoveryRequired)?
                .as_bytes(),
        );
        hash.update(header.operation.as_bytes());
        hash.update(header.digest.as_bytes());
        hash.update(header.transaction.as_bytes());
        hash_optional(hash, header.pool.as_ref().map(|v| v.as_bytes().as_slice()))?;
        hash.update(header.head.as_bytes());
        hash.update((self.protected.len() as u64).to_be_bytes());
        hash.update(&self.protected);
        hash.update(header.family.code().to_be_bytes());
        hash_optional(hash, header.order.as_ref().map(|v| v.as_bytes().as_slice()))?;
        hash_optional(
            hash,
            header
                .related_operation
                .as_ref()
                .map(|v| v.as_bytes().as_slice()),
        )?;
        hash_optional(
            hash,
            header
                .subject_grant
                .as_ref()
                .map(|v| v.as_bytes().as_slice()),
        )?;
        hash.update([header.parts]);
        hash.update(self.parts_witness);
        if self.movements.len() > 64
            || self
                .movements
                .windows(2)
                .any(|pair| pair[0].id >= pair[1].id)
        {
            return Err(OperationError::RecoveryRequired);
        }
        hash.update([self.movements.len() as u8]);
        for movement in &self.movements {
            hash.update(movement.id);
            hash.update(movement.pool.as_bytes());
            hash.update(movement.grant.as_bytes());
            hash.update((movement.protected.len() as u64).to_be_bytes());
            hash.update(&movement.protected);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
