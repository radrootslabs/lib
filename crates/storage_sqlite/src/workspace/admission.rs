use super::{
    AttachmentInput, AttachmentProof, WorkspaceBinding, WorkspaceClock, WorkspaceCrypto,
    fs::Namespace, operation, record::GrantRecord, worker,
};
use radroots_domain::{CommandDigest, IssuanceWindow, OperationId, TransactionId};
use radroots_storage::command::{
    AdmissionOutcome, AdmissionRequest, CommitAttempt, OperationError, PreparedOperation,
};
use sqlx::{Connection, Sqlite, SqliteConnection, Transaction};

/// One borrowed turn on the existing connection owner, never a second runtime.
pub(super) struct Journal<'a> {
    pub connection: &'a mut SqliteConnection,
    pub namespace: &'a Namespace,
    pub selected: &'a AttachmentInput,
    pub crypto: &'a WorkspaceCrypto,
    pub clock: &'a dyn WorkspaceClock,
    pub uncertain: &'a mut bool,
}
impl Journal<'_> {
    async fn access(
        &mut self,
        binding: &WorkspaceBinding,
        proof: &AttachmentProof,
    ) -> Result<(GrantRecord, Vec<u8>, WorkspaceBinding), OperationError> {
        if *self.uncertain {
            return Err(OperationError::RecoveryRequired);
        }
        self.namespace.validate()?;
        let protected = worker::read(self.connection, self.selected).await?;
        let plaintext = self
            .crypto
            .open(
                self.selected.workspace_id(),
                self.selected.grant_id(),
                &protected,
            )
            .await?;
        let record = plaintext.expose_secret(GrantRecord::decode)?;
        let current = record.authenticate(binding.input(), proof, self.clock.now()?)?;
        same_binding(binding, &current)?;
        self.namespace.validate()?;
        Ok((record, protected, current))
    }

    pub(super) async fn admit(
        &mut self,
        request: AdmissionRequest,
        proof: AttachmentProof,
    ) -> Result<AdmissionOutcome, OperationError> {
        let (record, grant, observed) = self.access(request.binding(), &proof).await?;
        let context = request.command().context();
        let workspace = context.workspace_id();
        let operation_id = context.operation_id();
        let digest = request.command().digest();
        // Exact replay precedes fresh deadline, allocation, entropy and encoding.
        let existing =
            operation::read(self.connection, self.selected, operation_id, self.crypto).await?;
        if let Some((snapshot, protected)) = existing {
            operation::check_digest(&snapshot, context.actor(), Some(digest))?;
            let anchor = snapshot.identity();
            let mut transaction = self
                .connection
                .begin_with("BEGIN IMMEDIATE")
                .await
                .map_err(|_| OperationError::StorageFailure)?;
            let checked = async {
                recheck(
                    &mut transaction,
                    self.namespace,
                    self.selected,
                    &grant,
                    &record,
                    request.binding(),
                    &proof,
                    self.clock,
                )
                .await?;
                operation::unchanged(&mut transaction, &snapshot, &protected).await
            }
            .await;
            if transaction.rollback().await.is_err() {
                *self.uncertain = true;
                return Ok(AdmissionOutcome::Unknown(anchor));
            }
            checked?;
            return Ok(AdmissionOutcome::Committed(snapshot));
        }

        // This fresh local buyer namespace has no authenticated parent/order
        // history or protected approval origin. Selectors cannot create either.
        if request.command().parent().is_some() || context.expected().is_some() {
            return Err(OperationError::MissingHistory);
        }
        if context.approval().is_some() {
            return Err(OperationError::UnverifiedApproval);
        }
        if observed.observed_at() < request.binding().observed_at() {
            return Err(super::WorkspaceError::Unavailable.into());
        }
        let window = IssuanceWindow::capture(observed.observed_at(), context.not_after())
            .map_err(|_| OperationError::DeadlineExceeded)?;
        let transaction_id = TransactionId::from_bytes(super::random()?);
        let anchor = CommitAttempt::new(workspace, operation_id, digest, Some(transaction_id));
        // Encryption/provider work finishes before opening the write transaction.
        let protected = operation::protect(&request, transaction_id, window, self.crypto).await?;
        let mut transaction = self
            .connection
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(|_| OperationError::StorageFailure)?;
        let inserted = async {
            let current = recheck(
                &mut transaction,
                self.namespace,
                self.selected,
                &grant,
                &record,
                request.binding(),
                &proof,
                self.clock,
            )
            .await?;
            if current.observed_at() < window.prepared_at() {
                return Err(super::WorkspaceError::Unavailable.into());
            }
            if window.first_issue_not_after() <= current.observed_at()
                || context.not_after() > current.expires_at()
            {
                return Err(OperationError::DeadlineExceeded);
            }
            operation::insert(&mut transaction, &request, transaction_id, &protected).await
        }
        .await;
        if let Err(error) = inserted {
            if transaction.rollback().await.is_err() {
                *self.uncertain = true;
                return Ok(AdmissionOutcome::Unknown(anchor));
            }
            return Err(error);
        }
        if transaction.commit().await.is_err() {
            *self.uncertain = true;
            return Ok(AdmissionOutcome::Unknown(anchor));
        }
        Ok(AdmissionOutcome::Committed(
            PreparedOperation::from_committed_store(
                request,
                transaction_id,
                radroots_domain::Sequence::new(1).map_err(|_| OperationError::RecoveryRequired)?,
                window,
            )?,
        ))
    }

    pub(super) async fn query(
        &mut self,
        binding: WorkspaceBinding,
        operation_id: OperationId,
        digest: Option<CommandDigest>,
        proof: AttachmentProof,
    ) -> Result<Option<PreparedOperation>, OperationError> {
        let (record, grant, _) = self.access(&binding, &proof).await?;
        let result =
            operation::read(self.connection, self.selected, operation_id, self.crypto).await?;
        if let Some((snapshot, _)) = &result {
            operation::check_digest(snapshot, binding.actor(), digest)?;
        }
        let mut transaction = self
            .connection
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(|_| OperationError::StorageFailure)?;
        let checked = async {
            recheck(
                &mut transaction,
                self.namespace,
                self.selected,
                &grant,
                &record,
                &binding,
                &proof,
                self.clock,
            )
            .await?;
            if let Some((snapshot, protected)) = &result {
                operation::unchanged(&mut transaction, snapshot, protected).await?;
            } else {
                let count: i64 = sqlx::query_scalar(
                    "SELECT count(*) FROM operations WHERE workspace=? AND operation_id=?",
                )
                .bind(self.selected.workspace_id().as_bytes().as_slice())
                .bind(operation_id.as_bytes().as_slice())
                .fetch_one(&mut *transaction)
                .await
                .map_err(|_| OperationError::RecoveryRequired)?;
                if count != 0 {
                    return Err(OperationError::RecoveryRequired);
                }
            }
            Ok(())
        }
        .await;
        if transaction.rollback().await.is_err() {
            *self.uncertain = true;
            return Err(OperationError::RecoveryRequired);
        }
        checked?;
        Ok(result.map(|(snapshot, _)| snapshot))
    }
}

fn same_binding(
    captured: &WorkspaceBinding,
    current: &WorkspaceBinding,
) -> Result<(), OperationError> {
    if captured.input() != current.input()
        || captured.actor() != current.actor()
        || captured.principal_id() != current.principal_id()
        || captured.grant_revision() != current.grant_revision()
        || captured.expires_at() != current.expires_at()
    {
        return Err(OperationError::Unauthorized);
    }
    Ok(())
}

#[expect(
    clippy::too_many_arguments,
    reason = "Private transaction recheck borrows only the existing owner inputs"
)]
async fn recheck(
    transaction: &mut Transaction<'_, Sqlite>,
    namespace: &Namespace,
    selected: &AttachmentInput,
    protected: &[u8],
    record: &GrantRecord,
    binding: &WorkspaceBinding,
    proof: &AttachmentProof,
    clock: &dyn WorkspaceClock,
) -> Result<WorkspaceBinding, OperationError> {
    namespace.validate()?;
    if worker::read(transaction, selected).await? != protected {
        return Err(OperationError::RecoveryRequired);
    }
    // No provider await occurs under this transaction: the exact protected row
    // has just matched the authenticated pre-transaction plaintext.
    let current = record.authenticate(binding.input(), proof, clock.now()?)?;
    same_binding(binding, &current)?;
    Ok(current)
}
