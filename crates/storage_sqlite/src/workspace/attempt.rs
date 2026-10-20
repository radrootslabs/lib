//! Immutable backend-owned first-claim records; no signer is invoked here.
use super::{
    AttachmentInput,
    crypto::{IssuancePayload, IssuanceScope, WorkspaceCrypto},
};
use radroots_domain::{PublicKey, SigningAttemptId, TransactionId, UnixSeconds};
use radroots_storage::command::protected::issuance::ClaimRecord;
use radroots_storage::command::{BoundSigningPlan, ClaimSnapshot, OperationError, SigningAttempt};
use sqlx::{Row, SqliteConnection};

pub(super) const TABLE: &str = "CREATE TABLE signing_attempts (workspace BLOB NOT NULL, operation_id BLOB NOT NULL, recipient BLOB NOT NULL CHECK(length(recipient)=32), attempt_id BLOB NOT NULL CHECK(length(attempt_id)=32), transaction_id BLOB NOT NULL CHECK(length(transaction_id)=32), plan_digest BLOB NOT NULL CHECK(length(plan_digest)=32), intent_digest BLOB NOT NULL CHECK(length(intent_digest)=32), protected BLOB NOT NULL CHECK(length(protected)>0 AND length(protected)<=262144), PRIMARY KEY(workspace,operation_id,recipient), FOREIGN KEY(workspace,operation_id) REFERENCES signing_plans(workspace,operation_id)) STRICT, WITHOUT ROWID";
pub(super) const ID_INDEX: &str =
    "CREATE UNIQUE INDEX signing_attempt_identity ON signing_attempts(workspace,attempt_id)";
pub(super) const TX_INDEX: &str = "CREATE UNIQUE INDEX signing_attempt_transaction_identity ON signing_attempts(workspace,transaction_id)";

pub(super) fn scope(attempt: &SigningAttempt) -> Result<IssuanceScope, OperationError> {
    let identity = attempt.bound_plan().prepared().identity();
    Ok(IssuanceScope {
        workspace: identity.workspace_id(),
        operation: identity.operation_id(),
        plan_digest: *attempt.bound_plan().plan().digest(),
        attempt: attempt.attempt_id(),
        recipient: attempt.seal()?.recipient(),
    })
}
pub(super) async fn protect(
    attempt: &SigningAttempt,
    crypto: &WorkspaceCrypto,
) -> Result<Vec<u8>, OperationError> {
    let bytes = ClaimRecord::capture(attempt)?.encode()?;
    Ok(crypto
        .seal_issuance(scope(attempt)?, IssuancePayload::Attempt, 0, &bytes)
        .await?)
}
/// Returns the complete original plan on absence as well as a verified retained
/// claim on presence. Providers run before the caller's write transaction.
pub(super) async fn read(
    connection: &mut SqliteConnection,
    selected: &AttachmentInput,
    bound: BoundSigningPlan,
    recipient: PublicKey,
    crypto: &WorkspaceCrypto,
) -> Result<
    (
        BoundSigningPlan,
        Option<(SigningAttemptId, TransactionId, UnixSeconds, Vec<u8>)>,
    ),
    OperationError,
> {
    let original = bound.prepared().identity();
    let row=sqlx::query("SELECT CASE WHEN length(attempt_id)=32 THEN attempt_id END,CASE WHEN length(transaction_id)=32 THEN transaction_id END,CASE WHEN length(plan_digest)=32 THEN plan_digest END,CASE WHEN length(intent_digest)=32 THEN intent_digest END,CASE WHEN length(protected)>0 AND length(protected)<=262144 THEN protected END FROM signing_attempts WHERE workspace=? AND operation_id=? AND recipient=?")
        .bind(selected.workspace_id().as_bytes().as_slice()).bind(original.operation_id().as_bytes().as_slice()).bind(recipient.as_bytes().as_slice()).fetch_optional(&mut *connection).await.map_err(|_|OperationError::RecoveryRequired)?;
    let Some(row) = row else {
        return Ok((bound, None));
    };
    let bytes = |index| {
        row.try_get::<Option<Vec<u8>>, _>(index)
            .map_err(|_| OperationError::RecoveryRequired)?
            .ok_or(OperationError::RecoveryRequired)
    };
    let id = SigningAttemptId::from_bytes(
        bytes(0)?
            .try_into()
            .map_err(|_| OperationError::RecoveryRequired)?,
    );
    let transaction = TransactionId::from_bytes(
        bytes(1)?
            .try_into()
            .map_err(|_| OperationError::RecoveryRequired)?,
    );
    let plan_digest = bytes(2)?;
    let intent_digest = bytes(3)?;
    let protected = bytes(4)?;
    if plan_digest.as_slice() != bound.plan().digest() {
        return Err(OperationError::RecoveryRequired);
    }
    let issuance_scope = IssuanceScope {
        workspace: selected.workspace_id(),
        operation: original.operation_id(),
        plan_digest: *bound.plan().digest(),
        attempt: id,
        recipient,
    };
    let plaintext = crypto
        .open_issuance(issuance_scope, IssuancePayload::Attempt, 0, &protected)
        .await?;
    let attempt = plaintext.expose_secret(|raw| ClaimRecord::decode(raw, bound, recipient))?;
    if attempt.attempt_id() != id
        || attempt.transaction_id() != transaction
        || intent_digest.as_slice() != attempt.binding().digest()
    {
        return Err(OperationError::RecoveryRequired);
    }
    let time = attempt.claimed_at();
    Ok((
        attempt.into_bound_plan(),
        Some((id, transaction, time, protected)),
    ))
}

pub(super) async fn insert(
    connection: &mut SqliteConnection,
    attempt: &SigningAttempt,
    protected: &[u8],
) -> Result<(), OperationError> {
    let identity = attempt.bound_plan().prepared().identity();
    sqlx::query("INSERT INTO signing_attempts(workspace,operation_id,recipient,attempt_id,transaction_id,plan_digest,intent_digest,protected) VALUES(?,?,?,?,?,?,?,?)")
        .bind(identity.workspace_id().as_bytes().as_slice()).bind(identity.operation_id().as_bytes().as_slice()).bind(attempt.seal()?.recipient().as_bytes().as_slice()).bind(attempt.attempt_id().as_bytes().as_slice()).bind(attempt.transaction_id().as_bytes().as_slice()).bind(attempt.bound_plan().plan().digest().as_slice()).bind(attempt.binding().digest().as_slice()).bind(protected).execute(connection).await.map_err(|_|OperationError::StorageFailure)?;
    Ok(())
}
pub(super) async fn unchanged(
    connection: &mut SqliteConnection,
    attempt: &SigningAttempt,
    protected: &[u8],
) -> Result<(), OperationError> {
    unchanged_snapshot(
        connection,
        ClaimSnapshot::from_store_report(attempt)?,
        protected,
    )
    .await
}
pub(super) async fn unchanged_snapshot(
    connection: &mut SqliteConnection,
    snapshot: ClaimSnapshot,
    protected: &[u8],
) -> Result<(), OperationError> {
    let count:i64=sqlx::query_scalar("SELECT count(*) FROM signing_attempts WHERE workspace=? AND operation_id=? AND recipient=? AND attempt_id=? AND transaction_id=? AND plan_digest=? AND intent_digest=? AND protected=?")
        .bind(snapshot.workspace_id().as_bytes().as_slice()).bind(snapshot.operation_id().as_bytes().as_slice()).bind(snapshot.recipient().as_bytes().as_slice()).bind(snapshot.attempt_id().as_bytes().as_slice()).bind(snapshot.transaction_id().as_bytes().as_slice()).bind(snapshot.plan_digest().as_slice()).bind(snapshot.intent().digest().as_slice()).bind(protected).fetch_one(connection).await.map_err(|_|OperationError::RecoveryRequired)?;
    if count != 1 {
        return Err(OperationError::RecoveryRequired);
    }
    Ok(())
}
