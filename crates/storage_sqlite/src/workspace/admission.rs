use super::{
    AttachmentInput, AttachmentProof, WorkspaceBinding, WorkspaceClock, WorkspaceCrypto,
    fs::Namespace, operation, plan, record::GrantRecord, worker,
};
use radroots_domain::{CommandDigest, IssuanceWindow, OperationId, TransactionId};
use radroots_storage::command::{
    AdmissionOutcome, AdmissionRequest, BoundSigningPlan, CapturedSigningPlan, CommitAttempt,
    OperationError, PlanBindingOutcome, PreparedOperation,
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
    pub(super) async fn access(
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
        let record = plaintext.expose_secret(GrantRecord::decode_outbox)?;
        let current = record.authenticate(binding.input(), proof, self.clock.now()?)?;
        same_binding(binding, &current)?;
        self.namespace.validate()?;
        Ok((record, protected, current))
    }

    pub(super) async fn bind_plan(
        &mut self,
        binding: WorkspaceBinding,
        supplied: CapturedSigningPlan,
        proof: AttachmentProof,
    ) -> Result<PlanBindingOutcome, OperationError> {
        let (record, grant, current) = self.access(&binding, &proof).await?;
        let (prepared, operation_protected) = operation::read(
            self.connection,
            self.selected,
            supplied.operation_id(),
            self.crypto,
        )
        .await?
        .ok_or(OperationError::RecoveryRequired)?;
        operation::check_digest(&prepared, binding.actor(), Some(supplied.command_digest()))?;
        same_binding(prepared.request().binding(), &binding)?;
        let anchor = prepared.identity();
        let (prepared, existing) =
            plan::read(self.connection, self.selected, prepared, self.crypto).await?;
        let (plan, transaction_id, bound_at, protected, fresh) =
            if let Some((plan, transaction, bound_at, protected)) = existing {
                if plan.protected_wire() != supplied.protected_wire() {
                    return Err(OperationError::IdempotencyConflict);
                }
                (plan, transaction, bound_at, protected, false)
            } else {
                if supplied.listing().is_some() && !super::listing::current(self.connection).await?
                {
                    return Err(OperationError::UnsupportedOperation);
                }
                let window = prepared.issuance_window();
                if current.observed_at() < window.prepared_at() {
                    return Err(super::WorkspaceError::Unavailable.into());
                }
                if current.observed_at() >= window.first_issue_not_after() {
                    return Err(OperationError::DeadlineExceeded);
                }
                let supplied = CapturedSigningPlan::parse(
                    prepared.request().command(),
                    supplied.protected_wire(),
                )
                .map_err(|_| OperationError::InvalidCommand)?;
                if supplied.created_at() < window.prepared_at()
                    || supplied.created_at() > current.observed_at()
                {
                    return Err(OperationError::InvalidCommand);
                }
                let transaction = TransactionId::from_bytes(super::random()?);
                let protected = plan::protect(
                    &prepared,
                    &supplied,
                    transaction,
                    current.observed_at(),
                    self.crypto,
                )
                .await?;
                (
                    supplied,
                    transaction,
                    current.observed_at(),
                    protected,
                    true,
                )
            };
        let attempt = CommitAttempt::new(
            anchor.workspace_id(),
            anchor.operation_id(),
            anchor.command_digest(),
            Some(transaction_id),
        );
        let mut transaction = self
            .connection
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(|_| OperationError::StorageFailure)?;
        let changed = async {
            let now = recheck(
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
            operation::unchanged(&mut transaction, &prepared, &operation_protected).await?;
            if fresh {
                if now.observed_at() < bound_at {
                    return Err(super::WorkspaceError::Unavailable.into());
                }
                if now.observed_at() >= prepared.issuance_window().first_issue_not_after() {
                    return Err(OperationError::DeadlineExceeded);
                }
                plan::unchanged(&mut transaction, &prepared, None).await?;
                plan::insert(&mut transaction, &prepared, &protected).await?;
            } else {
                plan::unchanged(&mut transaction, &prepared, Some(&protected)).await?;
            }
            Ok(())
        }
        .await;
        if let Err(error) = changed {
            if transaction.rollback().await.is_err() {
                *self.uncertain = true;
                return Ok(PlanBindingOutcome::Unknown(attempt));
            }
            return Err(error);
        }
        let finished = if fresh {
            transaction.commit().await
        } else {
            transaction.rollback().await
        };
        if finished.is_err() {
            *self.uncertain = true;
            return Ok(PlanBindingOutcome::Unknown(attempt));
        }
        Ok(PlanBindingOutcome::Committed(
            BoundSigningPlan::from_committed_store(prepared, plan, transaction_id, bound_at)?,
        ))
    }

    pub(super) async fn query_plan(
        &mut self,
        binding: WorkspaceBinding,
        operation_id: OperationId,
        digest: CommandDigest,
        proof: AttachmentProof,
    ) -> Result<Option<BoundSigningPlan>, OperationError> {
        let (record, grant, _) = self.access(&binding, &proof).await?;
        let operation =
            operation::read(self.connection, self.selected, operation_id, self.crypto).await?;
        let retained = if let Some((prepared, ciphertext)) = operation {
            operation::check_digest(&prepared, binding.actor(), Some(digest))?;
            same_binding(prepared.request().binding(), &binding)?;
            let (prepared, plan) =
                plan::read(self.connection, self.selected, prepared, self.crypto).await?;
            Some((prepared, ciphertext, plan))
        } else {
            None
        };
        let mut transaction = self
            .connection
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(|_| OperationError::StorageFailure)?;
        let checked: Result<(), OperationError> = async {
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
            if let Some((prepared, ciphertext, plan)) = &retained {
                operation::unchanged(&mut transaction, prepared, ciphertext).await?;
                plan::unchanged(
                    &mut transaction,
                    prepared,
                    plan.as_ref().map(|entry| &entry.3),
                )
                .await?;
            } else {
                operation::absent(&mut transaction, self.selected, operation_id).await?;
            }
            Ok(())
        }
        .await;
        if transaction.rollback().await.is_err() {
            *self.uncertain = true;
            return Err(OperationError::RecoveryRequired);
        }
        checked?;
        match retained {
            Some((prepared, _, Some((plan, transaction, bound_at, _)))) => Ok(Some(
                BoundSigningPlan::from_committed_store(prepared, plan, transaction, bound_at)?,
            )),
            _ => Ok(None),
        }
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
            same_binding(snapshot.request().binding(), request.binding())?;
            let anchor = snapshot.identity();
            let mut transaction = self
                .connection
                .begin_with("BEGIN IMMEDIATE")
                .await
                .map_err(|_| OperationError::StorageFailure)?;
            let checked: Result<(), OperationError> = async {
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

        if request.command().listing().is_some()
            && !super::listing::current(self.connection).await?
        {
            return Err(OperationError::UnsupportedOperation);
        }

        // This fresh local outbox has no authenticated parent/order
        // history or protected approval origin. Selectors cannot create either.
        request.validate_fresh(observed.observed_at())?;
        if observed.observed_at() < request.binding().observed_at() {
            return Err(super::WorkspaceError::Unavailable.into());
        }
        let window = IssuanceWindow::capture(observed.observed_at(), context.not_after())
            .map_err(|_| OperationError::DeadlineExceeded)?;
        let transaction_id = TransactionId::from_bytes(super::random()?);
        let anchor = CommitAttempt::new(workspace, operation_id, digest, Some(transaction_id));
        // Encryption/provider work finishes before opening the write transaction.
        let protected = operation::protect(&request, transaction_id, window, self.crypto).await?;
        let creation =
            super::listing::Creation::protect(&request, transaction_id, self.crypto).await?;
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
            operation::absent(&mut transaction, self.selected, operation_id).await?;
            operation::insert(&mut transaction, &request, transaction_id, &protected).await?;
            if let Some(creation) = &creation {
                creation.insert(&mut transaction).await?;
            }
            Ok(())
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
            same_binding(snapshot.request().binding(), &binding)?;
        }
        let mut transaction = self
            .connection
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(|_| OperationError::StorageFailure)?;
        let checked: Result<(), OperationError> = async {
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
                operation::absent(&mut transaction, self.selected, operation_id).await?;
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

pub(super) use radroots_storage::command::protected::same_binding;

#[expect(
    clippy::too_many_arguments,
    reason = "Private transaction recheck borrows only the existing owner inputs"
)]
pub(super) async fn recheck(
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
