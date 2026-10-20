//! Protected complete plan storage; all crypto runs outside write transactions.
use super::{
    AttachmentInput,
    crypto::{PRIVATE_PAYLOAD_MAX_BYTES, WorkspaceCrypto},
};
use radroots_domain::{TransactionId, UnixSeconds};
use radroots_storage::command::protected::plan::PlanManifest;
use radroots_storage::command::{CapturedSigningPlan, OperationError, PreparedOperation};
use sqlx::{Row, SqliteConnection};
use zeroize::Zeroizing;

pub(super) const TABLE: &str = "CREATE TABLE signing_plans (workspace BLOB NOT NULL, operation_id BLOB NOT NULL, plan_digest BLOB NOT NULL CHECK(length(plan_digest)=32), transaction_id BLOB NOT NULL CHECK(length(transaction_id)=32), part_count INTEGER NOT NULL CHECK(part_count BETWEEN 1 AND 24), byte_count INTEGER NOT NULL CHECK(byte_count BETWEEN 1 AND 393216), protected BLOB NOT NULL CHECK(length(protected)>0 AND length(protected)<=262144), PRIMARY KEY(workspace,operation_id), FOREIGN KEY(workspace,operation_id) REFERENCES operations(workspace,operation_id)) STRICT, WITHOUT ROWID";
pub(super) const PARTS: &str = "CREATE TABLE signing_plan_parts (workspace BLOB NOT NULL, operation_id BLOB NOT NULL, part INTEGER NOT NULL CHECK(part BETWEEN 1 AND 24), protected BLOB NOT NULL CHECK(length(protected)>0 AND length(protected)<=262144), PRIMARY KEY(workspace,operation_id,part), FOREIGN KEY(workspace,operation_id) REFERENCES signing_plans(workspace,operation_id)) STRICT, WITHOUT ROWID";
const MAX_PROTECTED_TOTAL: usize = 2 * 1024 * 1024;
pub(super) struct ProtectedPlan {
    pub digest: [u8; 32],
    pub transaction: TransactionId,
    pub manifest: Vec<u8>,
    pub parts: Vec<Vec<u8>>,
    pub bytes: usize,
}
pub(super) async fn protect(
    prepared: &PreparedOperation,
    plan: &CapturedSigningPlan,
    transaction: TransactionId,
    bound_at: UnixSeconds,
    crypto: &WorkspaceCrypto,
) -> Result<ProtectedPlan, OperationError> {
    let anchor = prepared.identity();
    let chunks = plan.protected_wire().chunks(PRIVATE_PAYLOAD_MAX_BYTES);
    let manifest = PlanManifest::capture(prepared, plan, transaction, bound_at)?;
    let plaintext = manifest.encode()?;
    let protected = crypto
        .seal_plan_part(
            anchor.workspace_id(),
            anchor.operation_id(),
            plan.digest(),
            0,
            &plaintext,
        )
        .await?;
    let mut total = protected.len();
    let mut parts = Vec::new();
    for (index, chunk) in chunks.enumerate() {
        let part = crypto
            .seal_plan_part(
                anchor.workspace_id(),
                anchor.operation_id(),
                plan.digest(),
                (index + 1) as u8,
                chunk,
            )
            .await?;
        total = total
            .checked_add(part.len())
            .filter(|size| *size <= MAX_PROTECTED_TOTAL)
            .ok_or(OperationError::InvalidCommand)?;
        parts.push(part);
    }
    Ok(ProtectedPlan {
        digest: *plan.digest(),
        transaction,
        manifest: protected,
        parts,
        bytes: manifest.byte_count(),
    })
}

pub(super) async fn read(
    connection: &mut SqliteConnection,
    selected: &AttachmentInput,
    prepared: PreparedOperation,
    crypto: &WorkspaceCrypto,
) -> Result<
    (
        PreparedOperation,
        Option<(
            CapturedSigningPlan,
            TransactionId,
            UnixSeconds,
            ProtectedPlan,
        )>,
    ),
    OperationError,
> {
    let anchor = prepared.identity();
    let row = sqlx::query("SELECT CASE WHEN length(plan_digest)=32 THEN plan_digest END,CASE WHEN length(transaction_id)=32 THEN transaction_id END,part_count,byte_count,CASE WHEN length(protected)>0 AND length(protected)<=262144 THEN protected END FROM signing_plans WHERE workspace=? AND operation_id=?")
        .bind(selected.workspace_id().as_bytes().as_slice()).bind(anchor.operation_id().as_bytes().as_slice()).fetch_optional(&mut *connection).await.map_err(|_| OperationError::RecoveryRequired)?;
    let Some(row) = row else {
        return Ok((prepared, None));
    };
    let digest: Vec<u8> = row
        .try_get::<Option<Vec<u8>>, _>(0)
        .map_err(|_| OperationError::RecoveryRequired)?
        .ok_or(OperationError::RecoveryRequired)?;
    let digest: [u8; 32] = digest
        .try_into()
        .map_err(|_| OperationError::RecoveryRequired)?;
    let transaction: Vec<u8> = row
        .try_get::<Option<Vec<u8>>, _>(1)
        .map_err(|_| OperationError::RecoveryRequired)?
        .ok_or(OperationError::RecoveryRequired)?;
    let transaction = TransactionId::from_bytes(
        transaction
            .try_into()
            .map_err(|_| OperationError::RecoveryRequired)?,
    );
    let part_count: i64 = row
        .try_get(2)
        .map_err(|_| OperationError::RecoveryRequired)?;
    let byte_count: i64 = row
        .try_get(3)
        .map_err(|_| OperationError::RecoveryRequired)?;
    if !(1..=24).contains(&part_count)
        || !(1..=393216).contains(&byte_count)
        || (byte_count as usize).div_ceil(PRIVATE_PAYLOAD_MAX_BYTES) != part_count as usize
    {
        return Err(OperationError::RecoveryRequired);
    }
    let manifest: Vec<u8> = row
        .try_get::<Option<Vec<u8>>, _>(4)
        .map_err(|_| OperationError::RecoveryRequired)?
        .ok_or(OperationError::RecoveryRequired)?;
    let decoded = crypto
        .open_plan_part(
            selected.workspace_id(),
            anchor.operation_id(),
            &digest,
            0,
            &manifest,
        )
        .await?;
    let decoded = decoded.expose_secret(|bytes| PlanManifest::decode(bytes, &prepared))?;
    if decoded.plan_digest().as_bytes() != &digest
        || decoded.transaction_id() != transaction
        || decoded.part_count() != part_count as usize
        || decoded.byte_count() != byte_count as usize
    {
        return Err(OperationError::RecoveryRequired);
    }
    // SQL bounds aggregate/count before materializing any untrusted part set.
    let (count, total): (i64, i64) = sqlx::query_as("SELECT count(*),coalesce(sum(length(protected)),0) FROM signing_plan_parts WHERE workspace=? AND operation_id=?")
        .bind(selected.workspace_id().as_bytes().as_slice()).bind(anchor.operation_id().as_bytes().as_slice()).fetch_one(&mut *connection).await.map_err(|_| OperationError::RecoveryRequired)?;
    if count != part_count || total <= 0 || total > (MAX_PROTECTED_TOTAL - manifest.len()) as i64 {
        return Err(OperationError::RecoveryRequired);
    }
    let rows = sqlx::query("SELECT part,CASE WHEN length(protected)>0 AND length(protected)<=262144 AND (SELECT coalesce(sum(length(p.protected)),0) FROM signing_plan_parts p WHERE p.workspace=signing_plan_parts.workspace AND p.operation_id=signing_plan_parts.operation_id)<=2097152 THEN protected END FROM signing_plan_parts WHERE workspace=? AND operation_id=? ORDER BY part LIMIT 25")
        .bind(selected.workspace_id().as_bytes().as_slice()).bind(anchor.operation_id().as_bytes().as_slice()).fetch_all(&mut *connection).await.map_err(|_| OperationError::RecoveryRequired)?;
    if rows.len() != part_count as usize {
        return Err(OperationError::RecoveryRequired);
    }
    let mut wire = Zeroizing::new(Vec::with_capacity(byte_count as usize));
    let mut parts = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        let part: i64 = row
            .try_get(0)
            .map_err(|_| OperationError::RecoveryRequired)?;
        if part != (index + 1) as i64 {
            return Err(OperationError::RecoveryRequired);
        }
        let protected: Vec<u8> = row
            .try_get::<Option<Vec<u8>>, _>(1)
            .map_err(|_| OperationError::RecoveryRequired)?
            .ok_or(OperationError::RecoveryRequired)?;
        let plaintext = crypto
            .open_plan_part(
                selected.workspace_id(),
                anchor.operation_id(),
                &digest,
                part as u8,
                &protected,
            )
            .await?;
        let expected = if index + 1 == rows.len() {
            byte_count as usize - index * PRIVATE_PAYLOAD_MAX_BYTES
        } else {
            PRIVATE_PAYLOAD_MAX_BYTES
        };
        plaintext.expose_secret(|bytes| {
            if bytes.len() != expected {
                return Err(OperationError::RecoveryRequired);
            }
            wire.extend_from_slice(bytes);
            Ok(())
        })?;
        parts.push(protected);
    }
    let plan = CapturedSigningPlan::parse(prepared.request().command(), &wire)
        .map_err(|_| OperationError::RecoveryRequired)?;
    if plan.digest() != &digest {
        return Err(OperationError::RecoveryRequired);
    }
    let protected = ProtectedPlan {
        digest,
        transaction,
        manifest,
        parts,
        bytes: byte_count as usize,
    };
    // Validate the original T1 observation before returning even to internal callers.
    let bound_at = decoded.bound_at();
    let bound = decoded.bind(prepared, plan)?;
    let (prepared, plan) = bound.into_parts();
    Ok((prepared, Some((plan, transaction, bound_at, protected))))
}

pub(super) async fn insert(
    connection: &mut SqliteConnection,
    prepared: &PreparedOperation,
    value: &ProtectedPlan,
) -> Result<(), OperationError> {
    let anchor = prepared.identity();
    sqlx::query("INSERT INTO signing_plans(workspace,operation_id,plan_digest,transaction_id,part_count,byte_count,protected) VALUES(?,?,?,?,?,?,?)")
        .bind(anchor.workspace_id().as_bytes().as_slice()).bind(anchor.operation_id().as_bytes().as_slice()).bind(value.digest.as_slice()).bind(value.transaction.as_bytes().as_slice()).bind(value.parts.len() as i64).bind(value.bytes as i64).bind(&value.manifest).execute(&mut *connection).await.map_err(|_| OperationError::StorageFailure)?;
    for (index, part) in value.parts.iter().enumerate() {
        sqlx::query(
            "INSERT INTO signing_plan_parts(workspace,operation_id,part,protected) VALUES(?,?,?,?)",
        )
        .bind(anchor.workspace_id().as_bytes().as_slice())
        .bind(anchor.operation_id().as_bytes().as_slice())
        .bind((index + 1) as i64)
        .bind(part)
        .execute(&mut *connection)
        .await
        .map_err(|_| OperationError::StorageFailure)?;
    }
    Ok(())
}
pub(super) async fn unchanged(
    connection: &mut SqliteConnection,
    prepared: &PreparedOperation,
    value: Option<&ProtectedPlan>,
) -> Result<(), OperationError> {
    let anchor = prepared.identity();
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM signing_plans WHERE workspace=? AND operation_id=?",
    )
    .bind(anchor.workspace_id().as_bytes().as_slice())
    .bind(anchor.operation_id().as_bytes().as_slice())
    .fetch_one(&mut *connection)
    .await
    .map_err(|_| OperationError::RecoveryRequired)?;
    let Some(value) = value else {
        let parts: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM signing_plan_parts WHERE workspace=? AND operation_id=?",
        )
        .bind(anchor.workspace_id().as_bytes().as_slice())
        .bind(anchor.operation_id().as_bytes().as_slice())
        .fetch_one(&mut *connection)
        .await
        .map_err(|_| OperationError::RecoveryRequired)?;
        return if count == 0 && parts == 0 {
            Ok(())
        } else {
            Err(OperationError::RecoveryRequired)
        };
    };
    let matched: i64 = sqlx::query_scalar("SELECT count(*) FROM signing_plans WHERE workspace=? AND operation_id=? AND plan_digest=? AND transaction_id=? AND part_count=? AND byte_count=? AND protected=?")
        .bind(anchor.workspace_id().as_bytes().as_slice()).bind(anchor.operation_id().as_bytes().as_slice()).bind(value.digest.as_slice()).bind(value.transaction.as_bytes().as_slice()).bind(value.parts.len() as i64).bind(value.bytes as i64).bind(&value.manifest).fetch_one(&mut *connection).await.map_err(|_| OperationError::RecoveryRequired)?;
    let parts: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM signing_plan_parts WHERE workspace=? AND operation_id=?",
    )
    .bind(anchor.workspace_id().as_bytes().as_slice())
    .bind(anchor.operation_id().as_bytes().as_slice())
    .fetch_one(&mut *connection)
    .await
    .map_err(|_| OperationError::RecoveryRequired)?;
    if count != 1 || matched != 1 || parts != value.parts.len() as i64 {
        return Err(OperationError::RecoveryRequired);
    }
    for (index, part) in value.parts.iter().enumerate() {
        let matched: i64 = sqlx::query_scalar("SELECT count(*) FROM signing_plan_parts WHERE workspace=? AND operation_id=? AND part=? AND protected=?")
            .bind(anchor.workspace_id().as_bytes().as_slice()).bind(anchor.operation_id().as_bytes().as_slice()).bind((index + 1) as i64).bind(part).fetch_one(&mut *connection).await.map_err(|_| OperationError::RecoveryRequired)?;
        if matched != 1 {
            return Err(OperationError::RecoveryRequired);
        }
    }
    Ok(())
}
