//! Immutable backend-owned first-claim records; no signer is invoked here.
use super::{
    AttachmentInput,
    crypto::{IssuancePayload, IssuanceScope, WorkspaceCrypto},
};
use radroots_domain::{
    CommandDigest, EnvelopeEventId, GrantId, OperationId, PublicKey, SigningAttemptId,
    TransactionId, UnixSeconds, WorkspaceId,
};
use radroots_storage::command::{BoundSigningPlan, ClaimSnapshot, OperationError, SigningAttempt};
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqliteConnection};
use zeroize::Zeroizing;

pub(super) const TABLE: &str = "CREATE TABLE signing_attempts (workspace BLOB NOT NULL, operation_id BLOB NOT NULL, recipient BLOB NOT NULL CHECK(length(recipient)=32), attempt_id BLOB NOT NULL CHECK(length(attempt_id)=32), transaction_id BLOB NOT NULL CHECK(length(transaction_id)=32), plan_digest BLOB NOT NULL CHECK(length(plan_digest)=32), intent_digest BLOB NOT NULL CHECK(length(intent_digest)=32), protected BLOB NOT NULL CHECK(length(protected)>0 AND length(protected)<=262144), PRIMARY KEY(workspace,operation_id,recipient), FOREIGN KEY(workspace,operation_id) REFERENCES signing_plans(workspace,operation_id)) STRICT, WITHOUT ROWID";
pub(super) const ID_INDEX: &str =
    "CREATE UNIQUE INDEX signing_attempt_identity ON signing_attempts(workspace,attempt_id)";
pub(super) const TX_INDEX: &str = "CREATE UNIQUE INDEX signing_attempt_transaction_identity ON signing_attempts(workspace,transaction_id)";

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Encoded {
    version: u8,
    workspace: WorkspaceId,
    operation_id: OperationId,
    command_digest: CommandDigest,
    plan_digest: [u8; 32],
    recipient: PublicKey,
    attempt_id: SigningAttemptId,
    transaction_id: TransactionId,
    claimed_at: UnixSeconds,
    intent_digest: [u8; 32],
    deadline_unix_ms: u64,
    preparation_transaction: TransactionId,
    plan_transaction: TransactionId,
    grant_id: GrantId,
    expected_event: EnvelopeEventId,
}
impl Encoded {
    fn new(attempt: &SigningAttempt) -> Self {
        let plan = attempt.bound_plan();
        let prepared = plan.prepared();
        let identity = prepared.identity();
        Self {
            version: 1,
            workspace: identity.workspace_id(),
            operation_id: identity.operation_id(),
            command_digest: identity.command_digest(),
            plan_digest: *plan.plan().digest(),
            recipient: attempt.seal().recipient(),
            attempt_id: attempt.attempt_id(),
            transaction_id: attempt.transaction_id(),
            claimed_at: attempt.claimed_at(),
            intent_digest: *attempt.binding().digest(),
            deadline_unix_ms: attempt.deadline_unix_ms().get(),
            preparation_transaction: prepared.transaction_id(),
            plan_transaction: plan.transaction_id(),
            grant_id: prepared.request().binding().input().grant_id(),
            expected_event: EnvelopeEventId::from_bytes(attempt.seal().event_id()),
        }
    }
}
pub(super) fn scope(attempt: &SigningAttempt) -> IssuanceScope {
    let identity = attempt.bound_plan().prepared().identity();
    IssuanceScope {
        workspace: identity.workspace_id(),
        operation: identity.operation_id(),
        plan_digest: *attempt.bound_plan().plan().digest(),
        attempt: attempt.attempt_id(),
        recipient: attempt.seal().recipient(),
    }
}
pub(super) async fn protect(
    attempt: &SigningAttempt,
    crypto: &WorkspaceCrypto,
) -> Result<Vec<u8>, OperationError> {
    let bytes = Zeroizing::new(
        serde_json::to_vec(&Encoded::new(attempt)).map_err(|_| OperationError::InvalidCommand)?,
    );
    Ok(crypto
        .seal_issuance(scope(attempt), IssuancePayload::Attempt, 0, &bytes)
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
    let encoded: Encoded = plaintext
        .expose_secret(|bytes| serde_json::from_slice(bytes))
        .map_err(|_| OperationError::RecoveryRequired)?;
    if encoded.version != 1
        || encoded.workspace != selected.workspace_id()
        || encoded.operation_id != original.operation_id()
        || encoded.command_digest != original.command_digest()
        || &encoded.plan_digest != bound.plan().digest()
        || encoded.recipient != recipient
        || encoded.attempt_id != id
        || encoded.transaction_id != transaction
        || encoded.preparation_transaction != bound.prepared().transaction_id()
        || encoded.plan_transaction != bound.transaction_id()
        || encoded.grant_id != bound.prepared().request().binding().input().grant_id()
    {
        return Err(OperationError::RecoveryRequired);
    }
    let attempt = SigningAttempt::from_committed_store(
        bound,
        recipient,
        id,
        transaction,
        encoded.claimed_at,
    )?;
    if encoded.intent_digest != *attempt.binding().digest()
        || intent_digest.as_slice() != attempt.binding().digest()
        || encoded.deadline_unix_ms != attempt.deadline_unix_ms().get()
        || encoded.expected_event.as_bytes() != &attempt.seal().event_id()
    {
        return Err(OperationError::RecoveryRequired);
    }
    Ok((
        attempt.into_bound_plan(),
        Some((id, transaction, encoded.claimed_at, protected)),
    ))
}
pub(super) async fn insert(
    connection: &mut SqliteConnection,
    attempt: &SigningAttempt,
    protected: &[u8],
) -> Result<(), OperationError> {
    let identity = attempt.bound_plan().prepared().identity();
    sqlx::query("INSERT INTO signing_attempts(workspace,operation_id,recipient,attempt_id,transaction_id,plan_digest,intent_digest,protected) VALUES(?,?,?,?,?,?,?,?)")
        .bind(identity.workspace_id().as_bytes().as_slice()).bind(identity.operation_id().as_bytes().as_slice()).bind(attempt.seal().recipient().as_bytes().as_slice()).bind(attempt.attempt_id().as_bytes().as_slice()).bind(attempt.transaction_id().as_bytes().as_slice()).bind(attempt.bound_plan().plan().digest().as_slice()).bind(attempt.binding().digest().as_slice()).bind(protected).execute(connection).await.map_err(|_|OperationError::StorageFailure)?;
    Ok(())
}
pub(super) async fn unchanged(
    connection: &mut SqliteConnection,
    attempt: &SigningAttempt,
    protected: &[u8],
) -> Result<(), OperationError> {
    unchanged_snapshot(
        connection,
        ClaimSnapshot::from_store_report(attempt),
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
