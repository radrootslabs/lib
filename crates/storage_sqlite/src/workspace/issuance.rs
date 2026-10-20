//! First-claim and append-only obtained-evidence turns on the existing owner.
//! This layer never invokes a signer or network capability.
use super::{
    AttachmentInput, AttachmentProof, WorkspaceError,
    admission::{Journal, recheck, same_binding},
    attempt, evidence, operation, plan,
    record::GrantRecord,
    snapshot::RetainedFacts,
    worker,
};
use radroots_domain::{CommandDigest, OperationId, SigningAttemptId, TransactionId};
use radroots_storage::command::SigningTarget;
use radroots_storage::command::{
    ArtifactEvidence, BoundSigningPlan, CapturedArtifact, ClaimAttemptOutcome, ClaimAttemptRequest,
    CommitAttempt, EvidenceCaptureOutcome, OperationError, SigningAttempt,
};
use sqlx::{Connection, SqliteConnection};

struct Loaded {
    bound: BoundSigningPlan,
    operation: Vec<u8>,
    plan: plan::ProtectedPlan,
}
impl Journal<'_> {
    async fn load_issuance_plan(
        &mut self,
        operation_id: OperationId,
        digest: CommandDigest,
    ) -> Result<Option<Loaded>, OperationError> {
        let Some((prepared, operation)) =
            operation::read(self.connection, self.selected, operation_id, self.crypto).await?
        else {
            return Ok(None);
        };
        if prepared.request().command().context().actor().participant()
            != self.selected.participant()
        {
            return Err(OperationError::Unauthorized);
        }
        operation::check_digest(
            &prepared,
            prepared.request().command().context().actor(),
            Some(digest),
        )?;
        let (prepared, value) =
            plan::read(self.connection, self.selected, prepared, self.crypto).await?;
        let Some((plan, transaction, time, protected)) = value else {
            return Ok(None);
        };
        let bound = BoundSigningPlan::from_committed_store(prepared, plan, transaction, time)?;
        Ok(Some(Loaded {
            bound,
            operation,
            plan: protected,
        }))
    }
    pub(super) async fn claim_issuance(
        &mut self,
        request: ClaimAttemptRequest,
        proof: AttachmentProof,
    ) -> Result<ClaimAttemptOutcome, OperationError> {
        let (grant_record, grant, current) = self.access(request.binding(), &proof).await?;
        attempt::require_target(self.connection, request.target()).await?;
        let loaded = self
            .load_issuance_plan(request.operation_id(), request.command_digest())
            .await?
            .ok_or(OperationError::RecoveryRequired)?;
        same_binding(
            loaded.bound.prepared().request().binding(),
            request.binding(),
        )?;
        if loaded.bound.plan().digest() != request.plan_digest() {
            return Err(OperationError::IdempotencyConflict);
        }
        let Loaded {
            bound,
            operation,
            plan,
        } = loaded;
        let (bound, existing) = attempt::read(
            self.connection,
            self.selected,
            bound,
            request.target(),
            self.crypto,
        )
        .await?;
        let absence = if existing.is_none() {
            Some(
                RetainedFacts::read(
                    self.connection,
                    self.selected,
                    request.binding(),
                    request.operation_id(),
                    Some(request.command_digest()),
                    self.crypto,
                )
                .await?,
            )
        } else {
            None
        };
        let (snapshot, protected, fresh) = if let Some((id, transaction, time, protected)) =
            existing
        {
            (
                SigningAttempt::from_committed_store(
                    bound,
                    request.target(),
                    id,
                    transaction,
                    time,
                )?,
                protected,
                false,
            )
        } else {
            if current.observed_at() < bound.bound_at() {
                return Err(WorkspaceError::Unavailable.into());
            }
            if current.observed_at() >= bound.prepared().issuance_window().first_issue_not_after() {
                return Err(OperationError::DeadlineExceeded);
            }
            let snapshot = SigningAttempt::from_committed_store(
                bound,
                request.target(),
                SigningAttemptId::from_bytes(super::random()?),
                TransactionId::from_bytes(super::random()?),
                current.observed_at(),
            )?;
            let protected = attempt::protect(&snapshot, self.crypto).await?;
            (snapshot, protected, true)
        };
        let anchor = snapshot.identity();
        let mut transaction = self
            .connection
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(|_| OperationError::StorageFailure)?;
        let checked: Result<(), OperationError> = async {
            let now = recheck(
                &mut transaction,
                self.namespace,
                self.selected,
                &grant,
                &grant_record,
                request.binding(),
                &proof,
                self.clock,
            )
            .await?;
            operation::unchanged(
                &mut transaction,
                snapshot.bound_plan().prepared(),
                &operation,
            )
            .await?;
            plan::unchanged(
                &mut transaction,
                snapshot.bound_plan().prepared(),
                Some(&plan),
            )
            .await?;
            if fresh {
                absence
                    .as_ref()
                    .ok_or(OperationError::RecoveryRequired)?
                    .unchanged(&mut transaction, self.selected, request.operation_id())
                    .await?;
                if now.observed_at() < snapshot.claimed_at() {
                    return Err(WorkspaceError::Unavailable.into());
                }
                if now.observed_at()
                    >= snapshot
                        .bound_plan()
                        .prepared()
                        .issuance_window()
                        .first_issue_not_after()
                {
                    return Err(OperationError::DeadlineExceeded);
                }
                absent_claim(
                    &mut transaction,
                    self.selected,
                    request.operation_id(),
                    request.target(),
                )
                .await?;
                attempt::insert(&mut transaction, &snapshot, &protected).await?;
            } else {
                attempt::unchanged(&mut transaction, &snapshot, &protected).await?;
            }
            Ok(())
        }
        .await;
        if let Err(error) = checked {
            if transaction.rollback().await.is_err() {
                *self.uncertain = true;
                return Ok(ClaimAttemptOutcome::Unknown(anchor));
            }
            return Err(error);
        }
        let completed = if fresh {
            transaction.commit().await
        } else {
            transaction.rollback().await
        };
        if completed.is_err() {
            *self.uncertain = true;
            return Ok(ClaimAttemptOutcome::Unknown(anchor));
        }
        Ok(if fresh {
            ClaimAttemptOutcome::Claimed(snapshot)
        } else {
            ClaimAttemptOutcome::Retained(snapshot)
        })
    }
    pub(super) async fn query_issuance(
        &mut self,
        request: ClaimAttemptRequest,
        proof: AttachmentProof,
    ) -> Result<Option<SigningAttempt>, OperationError> {
        let (record, grant, _) = self.access(request.binding(), &proof).await?;
        let loaded = self
            .load_issuance_plan(request.operation_id(), request.command_digest())
            .await?;
        let retained = if let Some(loaded) = loaded {
            same_binding(
                loaded.bound.prepared().request().binding(),
                request.binding(),
            )?;
            if loaded.bound.plan().digest() != request.plan_digest() {
                return Err(OperationError::IdempotencyConflict);
            }
            let (bound, value) = attempt::read(
                self.connection,
                self.selected,
                loaded.bound,
                request.target(),
                self.crypto,
            )
            .await?;
            match value {
                Some((id, transaction, time, protected)) => Some((
                    SigningAttempt::from_committed_store(
                        bound,
                        request.target(),
                        id,
                        transaction,
                        time,
                    )?,
                    protected,
                    loaded.operation,
                    loaded.plan,
                )),
                None => None,
            }
        } else {
            None
        };
        let absence = if retained.is_none() {
            Some(
                RetainedFacts::read(
                    self.connection,
                    self.selected,
                    request.binding(),
                    request.operation_id(),
                    Some(request.command_digest()),
                    self.crypto,
                )
                .await?,
            )
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
                request.binding(),
                &proof,
                self.clock,
            )
            .await?;
            if let Some((snapshot, protected, operation, plan)) = &retained {
                operation::unchanged(
                    &mut transaction,
                    snapshot.bound_plan().prepared(),
                    operation,
                )
                .await?;
                plan::unchanged(
                    &mut transaction,
                    snapshot.bound_plan().prepared(),
                    Some(plan),
                )
                .await?;
                attempt::unchanged(&mut transaction, snapshot, protected).await?;
            } else {
                absence
                    .as_ref()
                    .ok_or(OperationError::RecoveryRequired)?
                    .unchanged(&mut transaction, self.selected, request.operation_id())
                    .await?;
                absent_claim(
                    &mut transaction,
                    self.selected,
                    request.operation_id(),
                    request.target(),
                )
                .await?;
            }
            Ok(())
        }
        .await;
        if transaction.rollback().await.is_err() {
            *self.uncertain = true;
            return Err(OperationError::RecoveryRequired);
        }
        checked?;
        Ok(retained.map(|value| value.0))
    }
    /// Original-scope append admission deliberately does not consult a clock,
    /// revocation flag or fresh command permission. The actual existing claim,
    /// immutable plan and authenticated exact bytes are independently required.
    pub(super) async fn capture_access(
        &mut self,
        input: &AttachmentInput,
        proof: &AttachmentProof,
    ) -> Result<(GrantRecord, Vec<u8>), OperationError> {
        if *self.uncertain {
            return Err(OperationError::RecoveryRequired);
        }
        if input != self.selected {
            return Err(OperationError::Unauthorized);
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
        record.authenticate_retained_capture(input, proof)?;
        self.namespace.validate()?;
        Ok((record, protected))
    }
    pub(super) async fn capture_issuance(
        &mut self,
        input: AttachmentInput,
        proof: AttachmentProof,
        supplied: ArtifactEvidence,
    ) -> Result<EvidenceCaptureOutcome, OperationError> {
        let (record, grant) = self.capture_access(&input, &proof).await?;
        if supplied.workspace_id() != self.selected.workspace_id() {
            return Err(OperationError::InvalidEvidence);
        }
        let Loaded {
            bound,
            operation,
            plan,
        } = self
            .load_issuance_plan(supplied.operation_id(), supplied.command_digest())
            .await?
            .ok_or(OperationError::RecoveryRequired)?;
        let (bound, claim) = attempt::read(
            self.connection,
            self.selected,
            bound,
            supplied.target(),
            self.crypto,
        )
        .await?;
        let (id, claim_transaction, claimed_at, claim_protected) =
            claim.ok_or(OperationError::RecoveryRequired)?;
        let snapshot = SigningAttempt::from_committed_store(
            bound,
            supplied.target(),
            id,
            claim_transaction,
            claimed_at,
        )?;
        if !supplied.matches_attempt(&snapshot) {
            return Err(OperationError::InvalidEvidence);
        }
        // The supplied typed value was already independently verified. Recheck
        // it against the backend's actual immutable claim, not its descriptor.
        let checked = ArtifactEvidence::verify(
            &snapshot,
            supplied.raw_json(),
            supplied.observed_at_unix_ms(),
        )?;
        let existing = evidence::read(self.connection, &snapshot, self.crypto).await?;
        let (protected, fresh) = if let Some((original, protected)) = existing {
            if original.raw_json() != checked.raw_json() || original.digest() != checked.digest() {
                return Err(OperationError::IdempotencyConflict);
            }
            (protected, false)
        } else {
            (
                evidence::protect(
                    &snapshot,
                    &checked,
                    TransactionId::from_bytes(super::random()?),
                    self.crypto,
                )
                .await?,
                true,
            )
        };
        let receipt = protected.receipt;
        let anchor = CommitAttempt::new(
            receipt.workspace_id(),
            receipt.operation_id(),
            receipt.command_digest(),
            Some(receipt.transaction_id()),
        );
        let mut transaction = self
            .connection
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(|_| OperationError::StorageFailure)?;
        let captured = async {
            self.namespace.validate()?;
            if worker::read(&mut transaction, self.selected).await? != grant {
                return Err(OperationError::RecoveryRequired);
            }
            record.authenticate_retained_capture(&input, &proof)?;
            operation::unchanged(
                &mut transaction,
                snapshot.bound_plan().prepared(),
                &operation,
            )
            .await?;
            plan::unchanged(
                &mut transaction,
                snapshot.bound_plan().prepared(),
                Some(&plan),
            )
            .await?;
            attempt::unchanged(&mut transaction, &snapshot, &claim_protected).await?;
            if fresh {
                evidence::unchanged(&mut transaction, &snapshot, None).await?;
                evidence::insert(&mut transaction, &protected).await?;
            } else {
                evidence::unchanged(&mut transaction, &snapshot, Some(&protected)).await?;
            }
            Ok(())
        }
        .await;
        if let Err(error) = captured {
            if transaction.rollback().await.is_err() {
                *self.uncertain = true;
                return Ok(EvidenceCaptureOutcome::Unknown(anchor));
            }
            return Err(error);
        }
        let completed = if fresh {
            transaction.commit().await
        } else {
            transaction.rollback().await
        };
        if completed.is_err() {
            *self.uncertain = true;
            return Ok(EvidenceCaptureOutcome::Unknown(anchor));
        }
        Ok(EvidenceCaptureOutcome::Committed(receipt))
    }
    pub(super) async fn query_captured(
        &mut self,
        request: ClaimAttemptRequest,
        proof: AttachmentProof,
    ) -> Result<Option<CapturedArtifact>, OperationError> {
        let (record, grant, _) = self.access(request.binding(), &proof).await?;
        let loaded = self
            .load_issuance_plan(request.operation_id(), request.command_digest())
            .await?;
        let retained = if let Some(loaded) = loaded {
            same_binding(
                loaded.bound.prepared().request().binding(),
                request.binding(),
            )?;
            if loaded.bound.plan().digest() != request.plan_digest() {
                return Err(OperationError::IdempotencyConflict);
            }
            let (bound, value) = attempt::read(
                self.connection,
                self.selected,
                loaded.bound,
                request.target(),
                self.crypto,
            )
            .await?;
            if let Some((id, transaction, time, protected)) = value {
                let snapshot = SigningAttempt::from_committed_store(
                    bound,
                    request.target(),
                    id,
                    transaction,
                    time,
                )?;
                let captured = evidence::read(self.connection, &snapshot, self.crypto).await?;
                Some((snapshot, protected, loaded.operation, loaded.plan, captured))
            } else {
                None
            }
        } else {
            None
        };
        let absence = if retained.as_ref().is_none_or(|value| value.4.is_none()) {
            Some(
                RetainedFacts::read(
                    self.connection,
                    self.selected,
                    request.binding(),
                    request.operation_id(),
                    Some(request.command_digest()),
                    self.crypto,
                )
                .await?,
            )
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
                request.binding(),
                &proof,
                self.clock,
            )
            .await?;
            if let Some(absence) = &absence {
                absence
                    .unchanged(&mut transaction, self.selected, request.operation_id())
                    .await?;
            }
            if let Some((snapshot, protected, operation, plan, captured)) = &retained {
                operation::unchanged(
                    &mut transaction,
                    snapshot.bound_plan().prepared(),
                    operation,
                )
                .await?;
                plan::unchanged(
                    &mut transaction,
                    snapshot.bound_plan().prepared(),
                    Some(plan),
                )
                .await?;
                attempt::unchanged(&mut transaction, snapshot, protected).await?;
                evidence::unchanged(
                    &mut transaction,
                    snapshot,
                    captured.as_ref().map(|value| &value.1),
                )
                .await?;
            } else {
                absent_claim(
                    &mut transaction,
                    self.selected,
                    request.operation_id(),
                    request.target(),
                )
                .await?;
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
            Some((attempt, _, _, _, Some((evidence, protected)))) => {
                Ok(Some(CapturedArtifact::from_committed_store(
                    attempt,
                    evidence,
                    protected.receipt.transaction_id(),
                )?))
            }
            _ => Ok(None),
        }
    }
}
pub(super) async fn absent_claim(
    connection: &mut SqliteConnection,
    selected: &AttachmentInput,
    operation: OperationId,
    target: impl Into<SigningTarget>,
) -> Result<(), OperationError> {
    let target = target.into();
    let count:i64=sqlx::query_scalar(attempt::sql(connection, "SELECT (SELECT count(*) FROM signing_attempts WHERE workspace=?1 AND operation_id=?2 AND recipient=?3)+(SELECT count(*) FROM seal_evidence WHERE workspace=?1 AND operation_id=?2 AND recipient=?3)+(SELECT count(*) FROM seal_evidence_parts WHERE workspace=?1 AND operation_id=?2 AND recipient=?3)").await?)
        .bind(selected.workspace_id().as_bytes().as_slice()).bind(operation.as_bytes().as_slice()).bind(target.key().as_slice()).fetch_one(connection).await.map_err(|_|OperationError::RecoveryRequired)?;
    if count != 0 {
        return Err(OperationError::RecoveryRequired);
    }
    Ok(())
}
