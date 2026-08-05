use crate::SqliteStorage;
use radroots_storage::{
    Error,
    atomic::{AtomicCommitDigest, AtomicCommitDisposition, AtomicCommitId},
    authored::{
        AdmissionState, ArtifactOrigin, AuthoredArtifact, AuthoredArtifactId, AuthoredOperation,
        FailureClass, SigningState, WorkFailure, WorkPhase,
    },
    authored_atomic::{
        AuthoredAtomicCommand, AuthoredAtomicOutcome, AuthoredAtomicReceipt, AuthoredAtomicStorage,
        AuthoredWorkTarget, CancelAuthoredTarget, ClaimAuthoredTarget, WorkFence,
    },
    authored_delivery::{
        AuthoredDeliveryPlan, AuthoredDeliveryPlanId, AuthoredDeliveryState, DeliveryAttemptOutcome,
    },
    event::BoxFuture,
    journal::OperationInstanceId,
};
use radroots_transport::{SinkFailure, outcome::Retryability, policy::SatisfactionState};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sqlx::{Row, Sqlite, sqlite::SqliteRow};

const SNAPSHOT_MAX_BYTES: usize = 4 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
struct ReceiptSnapshot {
    outcome: AuthoredAtomicOutcome,
}

impl AuthoredAtomicStorage for SqliteStorage {
    fn execute_authored(
        &self,
        command: AuthoredAtomicCommand,
    ) -> BoxFuture<'_, Result<AuthoredAtomicReceipt, Error>> {
        Box::pin(async move {
            self.require_authored_writer()?;
            let mut transaction = self
                .pool()
                .begin_with("BEGIN IMMEDIATE")
                .await
                .map_err(map_backend)?;
            let result = execute_transaction(&mut transaction, &command).await;
            match result {
                Ok(receipt) => {
                    transaction.commit().await.map_err(map_backend)?;
                    Ok(receipt)
                }
                Err(primary) => {
                    let _ = transaction.rollback().await;
                    Err(primary)
                }
            }
        })
    }

    fn authored_receipt(
        &self,
        commit_id: AtomicCommitId,
    ) -> BoxFuture<'_, Result<Option<AuthoredAtomicReceipt>, Error>> {
        Box::pin(async move {
            sqlx::query(
                "SELECT commit_id, commit_digest, requested_at_unix_ms,
                        committed_at_unix_ms, receipt
                 FROM radroots_runtime_authored_atomic_commits WHERE commit_id = ?",
            )
            .bind(commit_id.as_bytes().as_slice())
            .fetch_optional(self.pool())
            .await
            .map_err(map_backend)?
            .as_ref()
            .map(decode_receipt_row)
            .transpose()
        })
    }

    fn authored_operation(
        &self,
        operation_id: OperationInstanceId,
    ) -> BoxFuture<'_, Result<Option<AuthoredOperation>, Error>> {
        Box::pin(async move {
            let Some(row) = sqlx::query(
                "SELECT operation_id, artifact_count, created_at_unix_ms,
                        updated_at_unix_ms, revision, snapshot
                 FROM radroots_runtime_authored_operations WHERE operation_id = ?",
            )
            .bind(operation_id.as_bytes().as_slice())
            .fetch_optional(self.pool())
            .await
            .map_err(map_backend)?
            else {
                return Ok(None);
            };
            let operation = decode_operation_row(&row)?;
            for (ordinal, artifact_id) in operation.artifact_ids().iter().enumerate() {
                let artifact = sqlx::query(
                    "SELECT * FROM radroots_runtime_authored_artifacts WHERE artifact_id = ?",
                )
                .bind(artifact_id.as_bytes().as_slice())
                .fetch_optional(self.pool())
                .await
                .map_err(map_backend)?
                .as_ref()
                .map(decode_artifact_row)
                .transpose()?
                .ok_or(Error::InvalidAuthoredOperation)?;
                if artifact.operation_id() != operation.operation_id()
                    || usize::from(artifact.ordinal()) != ordinal
                {
                    return Err(Error::InvalidAuthoredOperation);
                }
            }
            Ok(Some(operation))
        })
    }

    fn authored_artifact(
        &self,
        artifact_id: AuthoredArtifactId,
    ) -> BoxFuture<'_, Result<Option<AuthoredArtifact>, Error>> {
        Box::pin(async move {
            let Some(row) = sqlx::query(
                "SELECT * FROM radroots_runtime_authored_artifacts WHERE artifact_id = ?",
            )
            .bind(artifact_id.as_bytes().as_slice())
            .fetch_optional(self.pool())
            .await
            .map_err(map_backend)?
            else {
                return Ok(None);
            };
            let artifact = decode_artifact_row(&row)?;
            let operation_row = sqlx::query(
                "SELECT operation_id, artifact_count, created_at_unix_ms,
                        updated_at_unix_ms, revision, snapshot
                 FROM radroots_runtime_authored_operations WHERE operation_id = ?",
            )
            .bind(artifact.operation_id().as_bytes().as_slice())
            .fetch_optional(self.pool())
            .await
            .map_err(map_backend)?
            .ok_or(Error::InvalidAuthoredArtifact)?;
            let operation = decode_operation_row(&operation_row)?;
            if operation
                .artifact_ids()
                .get(usize::from(artifact.ordinal()))
                != Some(&artifact.artifact_id())
            {
                return Err(Error::InvalidAuthoredArtifact);
            }
            Ok(Some(artifact))
        })
    }

    fn authored_delivery_plan(
        &self,
        plan_id: AuthoredDeliveryPlanId,
    ) -> BoxFuture<'_, Result<Option<AuthoredDeliveryPlan>, Error>> {
        Box::pin(async move { load_plan_pool(self, plan_id).await })
    }
}

impl SqliteStorage {
    fn require_authored_writer(&self) -> Result<(), Error> {
        if self.event_mode() == radroots_storage::status::EventStoreMode::ReadOnly {
            return Err(Error::BackendUnavailable);
        }
        Ok(())
    }
}

async fn execute_transaction(
    transaction: &mut sqlx::Transaction<'_, Sqlite>,
    command: &AuthoredAtomicCommand,
) -> Result<AuthoredAtomicReceipt, Error> {
    if let Some(row) = sqlx::query(
        "SELECT commit_id, commit_digest, requested_at_unix_ms,
                committed_at_unix_ms, receipt
         FROM radroots_runtime_authored_atomic_commits WHERE commit_id = ?",
    )
    .bind(command.commit_id().as_bytes().as_slice())
    .fetch_optional(&mut **transaction)
    .await
    .map_err(map_backend)?
    {
        let committed = decode_receipt_row(&row)?;
        if committed.digest() != command.digest() {
            return Err(Error::AtomicCommitConflict);
        }
        return AuthoredAtomicReceipt::from_durable_parts(
            committed.commit_id(),
            committed.digest(),
            AtomicCommitDisposition::Replay,
            committed.committed_at_unix_ms(),
            committed.outcome().clone(),
        );
    }

    let outcome = execute_command(transaction, command.clone()).await?;
    let receipt = AuthoredAtomicReceipt::new(
        command,
        AtomicCommitDisposition::Committed,
        command.requested_at_unix_ms(),
        outcome,
    )?;
    sqlx::query(
        "INSERT INTO radroots_runtime_authored_atomic_commits (
           commit_id, commit_digest, phase, target_id, requested_at_unix_ms,
           committed_at_unix_ms, receipt
         ) VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(receipt.commit_id().as_bytes().as_slice())
    .bind(receipt.digest().as_bytes().as_slice())
    .bind(command_phase(command))
    .bind(command_target(command).as_slice())
    .bind(i64_from_u64(command.requested_at_unix_ms())?)
    .bind(i64_from_u64(receipt.committed_at_unix_ms())?)
    .bind(encode_snapshot(&ReceiptSnapshot {
        outcome: receipt.outcome().clone(),
    })?)
    .execute(&mut **transaction)
    .await
    .map_err(map_backend)?;
    Ok(receipt)
}

async fn execute_command(
    transaction: &mut sqlx::Transaction<'_, Sqlite>,
    command: AuthoredAtomicCommand,
) -> Result<AuthoredAtomicOutcome, Error> {
    match command {
        AuthoredAtomicCommand::Prepare(value) => {
            if row_exists(
                transaction,
                "SELECT 1 FROM radroots_runtime_authored_operations WHERE operation_id = ?",
                value.operation().operation_id().as_bytes(),
            )
            .await?
                || any_artifact_exists(transaction, value.artifacts()).await?
                || any_plan_exists(transaction, value.delivery_plans()).await?
            {
                return Err(Error::AtomicCommitConflict);
            }
            persist_operation(transaction, value.operation()).await?;
            for artifact in value.artifacts() {
                persist_artifact(transaction, artifact).await?;
            }
            for plan in value.delivery_plans() {
                persist_plan(transaction, plan).await?;
            }
            Ok(AuthoredAtomicOutcome::Prepared {
                operation: value.operation().clone(),
                artifacts: value.artifacts().to_vec(),
                delivery_plans: value.delivery_plans().to_vec(),
            })
        }
        AuthoredAtomicCommand::Claim(value) => match value.target() {
            ClaimAuthoredTarget::ArtifactSigning(id) => {
                let mut artifact = load_artifact_tx(transaction, *id).await?;
                artifact.set_signing_claim(
                    value.claim().clone(),
                    value.claim().acquired_at_unix_ms(),
                )?;
                persist_artifact(transaction, &artifact).await?;
                Ok(AuthoredAtomicOutcome::Artifact(artifact))
            }
            ClaimAuthoredTarget::ArtifactAdmission(id) => {
                let mut artifact = load_artifact_tx(transaction, *id).await?;
                artifact.set_admission_claim(
                    value.claim().clone(),
                    value.claim().acquired_at_unix_ms(),
                )?;
                persist_artifact(transaction, &artifact).await?;
                Ok(AuthoredAtomicOutcome::Artifact(artifact))
            }
            ClaimAuthoredTarget::DeliveryPlan(id) => {
                let mut plan = load_plan_tx(transaction, *id).await?;
                plan.claim(value.claim().clone(), value.claim().acquired_at_unix_ms())?;
                persist_plan(transaction, &plan).await?;
                Ok(AuthoredAtomicOutcome::DeliveryPlan(plan))
            }
        },
        AuthoredAtomicCommand::ApplySigned(value) => {
            let mut artifact = load_artifact_tx(transaction, value.artifact_id()).await?;
            require_artifact_claim(
                artifact.signing_claim(),
                value.fence(),
                value.applied_at_unix_ms(),
            )?;
            artifact.record_signed(value.event().clone(), value.applied_at_unix_ms())?;
            persist_artifact(transaction, &artifact).await?;
            let plan_ids = sqlx::query_scalar::<_, Vec<u8>>(
                "SELECT plan_id FROM radroots_runtime_authored_delivery_plans
                 WHERE artifact_id = ? ORDER BY plan_id",
            )
            .bind(value.artifact_id().as_bytes().as_slice())
            .fetch_all(&mut **transaction)
            .await
            .map_err(map_backend)?;
            for bytes in plan_ids {
                let id = AuthoredDeliveryPlanId::new(array(bytes)?)?;
                let mut plan = load_plan_tx(transaction, id).await?;
                plan.bind_signed_event(value.event().clone(), value.applied_at_unix_ms())?;
                persist_plan(transaction, &plan).await?;
            }
            Ok(AuthoredAtomicOutcome::Artifact(artifact))
        }
        AuthoredAtomicCommand::ApplyAdmission(value) => {
            let mut artifact = load_artifact_tx(transaction, value.artifact_id()).await?;
            require_artifact_claim(
                artifact.admission_claim(),
                value.fence(),
                value.applied_at_unix_ms(),
            )?;
            artifact.record_admission(
                value.state(),
                value.failure().cloned(),
                value.retry().cloned(),
                value.applied_at_unix_ms(),
            )?;
            persist_artifact(transaction, &artifact).await?;
            Ok(AuthoredAtomicOutcome::Artifact(artifact))
        }
        AuthoredAtomicCommand::ApplyDelivery(value) => {
            let mut plan = load_plan_tx(transaction, value.plan_id()).await?;
            match value.outcome().clone() {
                DeliveryAttemptOutcome::Receipt(receipt) => plan.apply_receipt(
                    value.fence().token(),
                    value.fence().generation(),
                    value.fence().row_revision(),
                    receipt,
                    value.retry().cloned(),
                    value.applied_at_unix_ms(),
                )?,
                DeliveryAttemptOutcome::SinkFailure(failure) => plan.apply_sink_failure(
                    value.fence().token(),
                    value.fence().generation(),
                    value.fence().row_revision(),
                    failure,
                    value.retry().cloned(),
                    value.applied_at_unix_ms(),
                )?,
            }
            persist_plan(transaction, &plan).await?;
            Ok(AuthoredAtomicOutcome::DeliveryPlan(plan))
        }
        AuthoredAtomicCommand::ApplyFailure(value) => match value.target() {
            AuthoredWorkTarget::Artifact(id) => {
                let mut artifact = load_artifact_tx(transaction, *id).await?;
                apply_artifact_failure(&mut artifact, &value)?;
                persist_artifact(transaction, &artifact).await?;
                Ok(AuthoredAtomicOutcome::Artifact(artifact))
            }
            AuthoredWorkTarget::DeliveryPlan(id) => {
                let mut plan = load_plan_tx(transaction, *id).await?;
                if value.failure().phase() != WorkPhase::Delivery
                    || value.failure().class() == FailureClass::Indeterminate
                {
                    return Err(Error::AtomicWorkflowMismatch);
                }
                let retryability = match value.failure().class() {
                    FailureClass::Retryable => Retryability::Retryable,
                    FailureClass::Terminal => Retryability::Terminal,
                    FailureClass::Indeterminate => unreachable!(),
                };
                let failure = SinkFailure::for_request(
                    plan.request().ok_or(Error::InvalidAuthoredDeliveryPlan)?,
                    value.failure().code(),
                    retryability,
                    value.failure().retry_after_unix_ms(),
                    value.failure().diagnostic().map(str::to_owned),
                    Vec::new(),
                )
                .map_err(|_| Error::AtomicWorkflowMismatch)?;
                plan.apply_sink_failure(
                    value.fence().token(),
                    value.fence().generation(),
                    value.fence().row_revision(),
                    failure,
                    value.retry().cloned(),
                    value.applied_at_unix_ms(),
                )?;
                persist_plan(transaction, &plan).await?;
                Ok(AuthoredAtomicOutcome::DeliveryPlan(plan))
            }
        },
        AuthoredAtomicCommand::Cancel(value) => match value.target() {
            CancelAuthoredTarget::ArtifactSigning(id) => {
                let mut artifact = load_artifact_tx(transaction, *id).await?;
                require_revision(artifact.revision().get(), value.expected_revision().get())?;
                artifact.cancel_signing(value.cancelled_at_unix_ms())?;
                persist_artifact(transaction, &artifact).await?;
                Ok(AuthoredAtomicOutcome::Artifact(artifact))
            }
            CancelAuthoredTarget::ArtifactAdmission(id) => {
                let mut artifact = load_artifact_tx(transaction, *id).await?;
                require_revision(artifact.revision().get(), value.expected_revision().get())?;
                artifact.record_admission(
                    AdmissionState::Cancelled,
                    Some(WorkFailure::new(
                        "cancelled",
                        WorkPhase::Admission,
                        FailureClass::Terminal,
                        None,
                        None,
                    )?),
                    None,
                    value.cancelled_at_unix_ms(),
                )?;
                persist_artifact(transaction, &artifact).await?;
                Ok(AuthoredAtomicOutcome::Artifact(artifact))
            }
            CancelAuthoredTarget::DeliveryPlan(id) => {
                let mut plan = load_plan_tx(transaction, *id).await?;
                require_revision(plan.revision().get(), value.expected_revision().get())?;
                plan.cancel(value.cancelled_at_unix_ms())?;
                persist_plan(transaction, &plan).await?;
                Ok(AuthoredAtomicOutcome::DeliveryPlan(plan))
            }
        },
    }
}

fn apply_artifact_failure(
    artifact: &mut AuthoredArtifact,
    value: &radroots_storage::authored_atomic::ApplyWorkFailure,
) -> Result<(), Error> {
    match value.failure().phase() {
        WorkPhase::Signing => {
            require_artifact_claim(
                artifact.signing_claim(),
                value.fence(),
                value.applied_at_unix_ms(),
            )?;
            artifact.record_signing_failure(
                value.failure().clone(),
                value.retry().cloned(),
                value.applied_at_unix_ms(),
            )
        }
        WorkPhase::Admission => {
            require_artifact_claim(
                artifact.admission_claim(),
                value.fence(),
                value.applied_at_unix_ms(),
            )?;
            let state = match value.failure().class() {
                FailureClass::Retryable => AdmissionState::Retryable,
                FailureClass::Terminal => AdmissionState::Rejected,
                FailureClass::Indeterminate => return Err(Error::InvalidAuthoredTransition),
            };
            artifact.record_admission(
                state,
                Some(value.failure().clone()),
                value.retry().cloned(),
                value.applied_at_unix_ms(),
            )
        }
        WorkPhase::Delivery => Err(Error::AtomicWorkflowMismatch),
    }
}

pub(crate) async fn persist_operation(
    transaction: &mut sqlx::Transaction<'_, Sqlite>,
    operation: &AuthoredOperation,
) -> Result<(), Error> {
    sqlx::query(
        "INSERT INTO radroots_runtime_authored_operations (
           operation_id, artifact_count, created_at_unix_ms, updated_at_unix_ms,
           revision, snapshot
         ) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(operation.operation_id().as_bytes().as_slice())
    .bind(i64::try_from(operation.artifact_ids().len()).map_err(|_| Error::AtomicCommitFailed)?)
    .bind(i64_from_u64(operation.created_at_unix_ms())?)
    .bind(i64_from_u64(operation.updated_at_unix_ms())?)
    .bind(i64_from_u64(operation.revision().get())?)
    .bind(encode_snapshot(operation)?)
    .execute(&mut **transaction)
    .await
    .map_err(map_backend)?;
    Ok(())
}

pub(crate) async fn persist_artifact(
    transaction: &mut sqlx::Transaction<'_, Sqlite>,
    artifact: &AuthoredArtifact,
) -> Result<(), Error> {
    let signing = artifact.signing_claim();
    let admission = artifact.admission_claim();
    let retry_not_before = artifact
        .signing_retry()
        .or_else(|| artifact.admission_retry())
        .map(|retry| retry.not_before_unix_ms());
    sqlx::query(
        "INSERT INTO radroots_runtime_authored_artifacts (
           artifact_id, operation_id, ordinal, origin, signing_state, admission_state,
           plan_wire, signed_raw_json, signed_raw_sha256,
           signing_claim_token, signing_claim_generation, signing_claim_revision,
           signing_claim_expires_at_unix_ms, admission_claim_token,
           admission_claim_generation, admission_claim_revision,
           admission_claim_expires_at_unix_ms, retry_not_before_unix_ms,
           last_failure_code, created_at_unix_ms, updated_at_unix_ms, revision, snapshot
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(artifact_id) DO UPDATE SET
           operation_id=excluded.operation_id, ordinal=excluded.ordinal, origin=excluded.origin,
           signing_state=excluded.signing_state, admission_state=excluded.admission_state,
           plan_wire=excluded.plan_wire, signed_raw_json=excluded.signed_raw_json,
           signed_raw_sha256=excluded.signed_raw_sha256,
           signing_claim_token=excluded.signing_claim_token,
           signing_claim_generation=excluded.signing_claim_generation,
           signing_claim_revision=excluded.signing_claim_revision,
           signing_claim_expires_at_unix_ms=excluded.signing_claim_expires_at_unix_ms,
           admission_claim_token=excluded.admission_claim_token,
           admission_claim_generation=excluded.admission_claim_generation,
           admission_claim_revision=excluded.admission_claim_revision,
           admission_claim_expires_at_unix_ms=excluded.admission_claim_expires_at_unix_ms,
           retry_not_before_unix_ms=excluded.retry_not_before_unix_ms,
           last_failure_code=excluded.last_failure_code,
           updated_at_unix_ms=excluded.updated_at_unix_ms, revision=excluded.revision,
           snapshot=excluded.snapshot",
    )
    .bind(artifact.artifact_id().as_bytes().as_slice())
    .bind(artifact.operation_id().as_bytes().as_slice())
    .bind(i64::from(artifact.ordinal()))
    .bind(origin_name(artifact.origin()))
    .bind(signing_name(artifact.signing_state()))
    .bind(admission_name(artifact.admission_state()))
    .bind(artifact.plan().map(|plan| plan.wire_json()))
    .bind(
        artifact
            .signed()
            .map(|signed| signed.event().raw_json().as_bytes()),
    )
    .bind(
        artifact
            .signed()
            .map(|signed| signed.raw_json_sha256().as_slice()),
    )
    .bind(signing.map(|claim| claim.token().as_slice()))
    .bind(
        signing
            .map(|claim| i64_from_u64(claim.generation().get()))
            .transpose()?,
    )
    .bind(
        signing
            .map(|claim| i64_from_u64(claim.row_revision().get()))
            .transpose()?,
    )
    .bind(
        signing
            .map(|claim| i64_from_u64(claim.expires_at_unix_ms()))
            .transpose()?,
    )
    .bind(admission.map(|claim| claim.token().as_slice()))
    .bind(
        admission
            .map(|claim| i64_from_u64(claim.generation().get()))
            .transpose()?,
    )
    .bind(
        admission
            .map(|claim| i64_from_u64(claim.row_revision().get()))
            .transpose()?,
    )
    .bind(
        admission
            .map(|claim| i64_from_u64(claim.expires_at_unix_ms()))
            .transpose()?,
    )
    .bind(retry_not_before.map(i64_from_u64).transpose()?)
    .bind(artifact.last_failure().map(WorkFailure::code))
    .bind(i64_from_u64(artifact.created_at_unix_ms())?)
    .bind(i64_from_u64(artifact.updated_at_unix_ms())?)
    .bind(i64_from_u64(artifact.revision().get())?)
    .bind(encode_snapshot(artifact)?)
    .execute(&mut **transaction)
    .await
    .map_err(map_backend)?;
    Ok(())
}

pub(crate) async fn persist_plan(
    transaction: &mut sqlx::Transaction<'_, Sqlite>,
    plan: &AuthoredDeliveryPlan,
) -> Result<(), Error> {
    let claim = plan.claim_evidence();
    sqlx::query(
        "INSERT INTO radroots_runtime_authored_delivery_plans (
           plan_id, artifact_id, request_digest, state, attempt_count,
           claim_token, claim_generation, claim_revision, claim_expires_at_unix_ms,
           retry_not_before_unix_ms, last_failure_code, created_at_unix_ms,
           updated_at_unix_ms, revision, snapshot
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(plan_id) DO UPDATE SET
           artifact_id=excluded.artifact_id, request_digest=excluded.request_digest,
           state=excluded.state, attempt_count=excluded.attempt_count,
           claim_token=excluded.claim_token, claim_generation=excluded.claim_generation,
           claim_revision=excluded.claim_revision,
           claim_expires_at_unix_ms=excluded.claim_expires_at_unix_ms,
           retry_not_before_unix_ms=excluded.retry_not_before_unix_ms,
           last_failure_code=excluded.last_failure_code,
           updated_at_unix_ms=excluded.updated_at_unix_ms, revision=excluded.revision,
           snapshot=excluded.snapshot",
    )
    .bind(plan.plan_id().as_bytes().as_slice())
    .bind(plan.artifact_id().as_bytes().as_slice())
    .bind(plan.request_digest().as_slice())
    .bind(delivery_state_name(plan.state()))
    .bind(i64::from(plan.attempt_count()))
    .bind(claim.map(|value| value.token().as_slice()))
    .bind(
        claim
            .map(|value| i64_from_u64(value.generation().get()))
            .transpose()?,
    )
    .bind(
        claim
            .map(|value| i64_from_u64(value.row_revision().get()))
            .transpose()?,
    )
    .bind(
        claim
            .map(|value| i64_from_u64(value.expires_at_unix_ms()))
            .transpose()?,
    )
    .bind(
        plan.retry()
            .map(|retry| i64_from_u64(retry.not_before_unix_ms()))
            .transpose()?,
    )
    .bind(plan.last_failure().map(WorkFailure::code))
    .bind(i64_from_u64(plan.created_at_unix_ms())?)
    .bind(i64_from_u64(plan.updated_at_unix_ms())?)
    .bind(i64_from_u64(plan.revision().get())?)
    .bind(encode_snapshot(plan)?)
    .execute(&mut **transaction)
    .await
    .map_err(map_backend)?;

    sqlx::query("DELETE FROM radroots_runtime_authored_delivery_targets WHERE plan_id = ?")
        .bind(plan.plan_id().as_bytes().as_slice())
        .execute(&mut **transaction)
        .await
        .map_err(map_backend)?;
    sqlx::query("DELETE FROM radroots_runtime_authored_delivery_attempts WHERE plan_id = ?")
        .bind(plan.plan_id().as_bytes().as_slice())
        .execute(&mut **transaction)
        .await
        .map_err(map_backend)?;
    for (ordinal, target) in plan.intent().target_set().targets().iter().enumerate() {
        sqlx::query(
            "INSERT INTO radroots_runtime_authored_delivery_targets (
               plan_id, ordinal, target_fingerprint, target_snapshot
             ) VALUES (?, ?, ?, ?)",
        )
        .bind(plan.plan_id().as_bytes().as_slice())
        .bind(i64::try_from(ordinal).map_err(|_| Error::InvalidAuthoredDeliveryPlan)?)
        .bind(target.fingerprint().as_str())
        .bind(encode_snapshot(target)?)
        .execute(&mut **transaction)
        .await
        .map_err(map_backend)?;
    }
    for attempt in plan.attempts() {
        sqlx::query(
            "INSERT INTO radroots_runtime_authored_delivery_attempts (
               plan_id, attempt, satisfaction, recorded_at_unix_ms, outcome_snapshot
             ) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(plan.plan_id().as_bytes().as_slice())
        .bind(i64::from(attempt.attempt().get()))
        .bind(satisfaction_name(attempt.satisfaction()))
        .bind(i64_from_u64(attempt.recorded_at_unix_ms())?)
        .bind(encode_snapshot(attempt.outcome())?)
        .execute(&mut **transaction)
        .await
        .map_err(map_backend)?;
    }
    Ok(())
}

async fn load_artifact_tx(
    transaction: &mut sqlx::Transaction<'_, Sqlite>,
    artifact_id: AuthoredArtifactId,
) -> Result<AuthoredArtifact, Error> {
    sqlx::query("SELECT * FROM radroots_runtime_authored_artifacts WHERE artifact_id = ?")
        .bind(artifact_id.as_bytes().as_slice())
        .fetch_optional(&mut **transaction)
        .await
        .map_err(map_backend)?
        .as_ref()
        .map(decode_artifact_row)
        .transpose()?
        .ok_or(Error::InvalidAuthoredArtifact)
}

async fn load_plan_tx(
    transaction: &mut sqlx::Transaction<'_, Sqlite>,
    plan_id: AuthoredDeliveryPlanId,
) -> Result<AuthoredDeliveryPlan, Error> {
    let plan =
        sqlx::query("SELECT * FROM radroots_runtime_authored_delivery_plans WHERE plan_id = ?")
            .bind(plan_id.as_bytes().as_slice())
            .fetch_optional(&mut **transaction)
            .await
            .map_err(map_backend)?
            .as_ref()
            .map(decode_plan_row)
            .transpose()?
            .ok_or(Error::InvalidAuthoredDeliveryPlan)?;
    validate_plan_children_tx(transaction, &plan).await?;
    Ok(plan)
}

async fn load_plan_pool(
    storage: &SqliteStorage,
    plan_id: AuthoredDeliveryPlanId,
) -> Result<Option<AuthoredDeliveryPlan>, Error> {
    let Some(row) =
        sqlx::query("SELECT * FROM radroots_runtime_authored_delivery_plans WHERE plan_id = ?")
            .bind(plan_id.as_bytes().as_slice())
            .fetch_optional(storage.pool())
            .await
            .map_err(map_backend)?
    else {
        return Ok(None);
    };
    let plan = decode_plan_row(&row)?;
    validate_plan_children_pool(storage, &plan).await?;
    Ok(Some(plan))
}

async fn validate_plan_children_tx(
    transaction: &mut sqlx::Transaction<'_, Sqlite>,
    plan: &AuthoredDeliveryPlan,
) -> Result<(), Error> {
    let targets = sqlx::query(
        "SELECT ordinal, target_fingerprint, target_snapshot
         FROM radroots_runtime_authored_delivery_targets WHERE plan_id = ? ORDER BY ordinal",
    )
    .bind(plan.plan_id().as_bytes().as_slice())
    .fetch_all(&mut **transaction)
    .await
    .map_err(map_backend)?;
    let attempts = sqlx::query(
        "SELECT attempt, satisfaction, recorded_at_unix_ms, outcome_snapshot
         FROM radroots_runtime_authored_delivery_attempts WHERE plan_id = ? ORDER BY attempt",
    )
    .bind(plan.plan_id().as_bytes().as_slice())
    .fetch_all(&mut **transaction)
    .await
    .map_err(map_backend)?;
    validate_plan_children(plan, &targets, &attempts)
}

async fn validate_plan_children_pool(
    storage: &SqliteStorage,
    plan: &AuthoredDeliveryPlan,
) -> Result<(), Error> {
    let targets = sqlx::query(
        "SELECT ordinal, target_fingerprint, target_snapshot
         FROM radroots_runtime_authored_delivery_targets WHERE plan_id = ? ORDER BY ordinal",
    )
    .bind(plan.plan_id().as_bytes().as_slice())
    .fetch_all(storage.pool())
    .await
    .map_err(map_backend)?;
    let attempts = sqlx::query(
        "SELECT attempt, satisfaction, recorded_at_unix_ms, outcome_snapshot
         FROM radroots_runtime_authored_delivery_attempts WHERE plan_id = ? ORDER BY attempt",
    )
    .bind(plan.plan_id().as_bytes().as_slice())
    .fetch_all(storage.pool())
    .await
    .map_err(map_backend)?;
    validate_plan_children(plan, &targets, &attempts)
}

fn validate_plan_children(
    plan: &AuthoredDeliveryPlan,
    targets: &[SqliteRow],
    attempts: &[SqliteRow],
) -> Result<(), Error> {
    if targets.len() != plan.intent().target_set().targets().len()
        || attempts.len() != plan.attempts().len()
    {
        return Err(Error::InvalidAuthoredDeliveryPlan);
    }
    for (ordinal, (row, expected)) in targets
        .iter()
        .zip(plan.intent().target_set().targets())
        .enumerate()
    {
        let decoded =
            decode_snapshot::<radroots_transport::Target>(column(row, "target_snapshot")?)?;
        if column::<i64>(row, "ordinal")? != i64::try_from(ordinal).unwrap_or(i64::MAX)
            || column::<String>(row, "target_fingerprint")? != expected.fingerprint().as_str()
            || decoded != *expected
        {
            return Err(Error::InvalidAuthoredDeliveryPlan);
        }
    }
    for (row, expected) in attempts.iter().zip(plan.attempts()) {
        let decoded = decode_snapshot::<DeliveryAttemptOutcome>(column(row, "outcome_snapshot")?)?;
        if column::<i64>(row, "attempt")? != i64::from(expected.attempt().get())
            || column::<String>(row, "satisfaction")? != satisfaction_name(expected.satisfaction())
            || u64_from_i64(column(row, "recorded_at_unix_ms")?)? != expected.recorded_at_unix_ms()
            || decoded != *expected.outcome()
        {
            return Err(Error::InvalidAuthoredDeliveryPlan);
        }
    }
    Ok(())
}

fn decode_operation_row(row: &SqliteRow) -> Result<AuthoredOperation, Error> {
    let value = decode_snapshot::<AuthoredOperation>(column(row, "snapshot")?)?;
    if column::<Vec<u8>>(row, "operation_id")?.as_slice() != value.operation_id().as_bytes()
        || column::<i64>(row, "artifact_count")?
            != i64::try_from(value.artifact_ids().len())
                .map_err(|_| Error::InvalidAuthoredOperation)?
        || u64_from_i64(column(row, "created_at_unix_ms")?)? != value.created_at_unix_ms()
        || u64_from_i64(column(row, "updated_at_unix_ms")?)? != value.updated_at_unix_ms()
        || u64_from_i64(column(row, "revision")?)? != value.revision().get()
    {
        return Err(Error::InvalidAuthoredOperation);
    }
    Ok(value)
}

fn decode_artifact_row(row: &SqliteRow) -> Result<AuthoredArtifact, Error> {
    let value = decode_snapshot::<AuthoredArtifact>(column(row, "snapshot")?)?;
    let plan_wire = column::<Option<Vec<u8>>>(row, "plan_wire")?;
    let raw = column::<Option<Vec<u8>>>(row, "signed_raw_json")?;
    let raw_digest = column::<Option<Vec<u8>>>(row, "signed_raw_sha256")?;
    let signing_claim = value.signing_claim();
    let admission_claim = value.admission_claim();
    let retry_not_before = value
        .signing_retry()
        .or_else(|| value.admission_retry())
        .map(|retry| retry.not_before_unix_ms());
    if column::<Vec<u8>>(row, "artifact_id")?.as_slice() != value.artifact_id().as_bytes()
        || column::<Vec<u8>>(row, "operation_id")?.as_slice() != value.operation_id().as_bytes()
        || column::<i64>(row, "ordinal")? != i64::from(value.ordinal())
        || column::<String>(row, "origin")? != origin_name(value.origin())
        || column::<String>(row, "signing_state")? != signing_name(value.signing_state())
        || column::<String>(row, "admission_state")? != admission_name(value.admission_state())
        || plan_wire.as_deref() != value.plan().map(|plan| plan.wire_json())
        || raw.as_deref()
            != value
                .signed()
                .map(|signed| signed.event().raw_json().as_bytes())
        || raw_digest.as_deref()
            != value
                .signed()
                .map(|signed| signed.raw_json_sha256().as_slice())
        || !claim_columns_match(row, "signing", signing_claim)?
        || !claim_columns_match(row, "admission", admission_claim)?
        || column::<Option<i64>>(row, "retry_not_before_unix_ms")?
            != retry_not_before.map(i64_from_u64).transpose()?
        || column::<Option<String>>(row, "last_failure_code")?.as_deref()
            != value.last_failure().map(WorkFailure::code)
        || u64_from_i64(column(row, "created_at_unix_ms")?)? != value.created_at_unix_ms()
        || u64_from_i64(column(row, "updated_at_unix_ms")?)? != value.updated_at_unix_ms()
        || u64_from_i64(column(row, "revision")?)? != value.revision().get()
    {
        return Err(Error::InvalidAuthoredArtifact);
    }
    Ok(value)
}

fn decode_plan_row(row: &SqliteRow) -> Result<AuthoredDeliveryPlan, Error> {
    let value = decode_snapshot::<AuthoredDeliveryPlan>(column(row, "snapshot")?)?;
    if column::<Vec<u8>>(row, "plan_id")?.as_slice() != value.plan_id().as_bytes()
        || column::<Vec<u8>>(row, "artifact_id")?.as_slice() != value.artifact_id().as_bytes()
        || column::<Vec<u8>>(row, "request_digest")?.as_slice() != value.request_digest()
        || column::<String>(row, "state")? != delivery_state_name(value.state())
        || column::<i64>(row, "attempt_count")? != i64::from(value.attempt_count())
        || !claim_columns_match(row, "claim", value.claim_evidence())?
        || column::<Option<i64>>(row, "retry_not_before_unix_ms")?
            != value
                .retry()
                .map(|retry| i64_from_u64(retry.not_before_unix_ms()))
                .transpose()?
        || column::<Option<String>>(row, "last_failure_code")?.as_deref()
            != value.last_failure().map(WorkFailure::code)
        || u64_from_i64(column(row, "created_at_unix_ms")?)? != value.created_at_unix_ms()
        || u64_from_i64(column(row, "updated_at_unix_ms")?)? != value.updated_at_unix_ms()
        || u64_from_i64(column(row, "revision")?)? != value.revision().get()
    {
        return Err(Error::InvalidAuthoredDeliveryPlan);
    }
    Ok(value)
}

fn claim_columns_match(
    row: &SqliteRow,
    prefix: &str,
    claim: Option<&radroots_storage::authored::WorkClaim>,
) -> Result<bool, Error> {
    let (token_column, generation_column, revision_column, expiry_column) = match prefix {
        "signing" => (
            "signing_claim_token",
            "signing_claim_generation",
            "signing_claim_revision",
            "signing_claim_expires_at_unix_ms",
        ),
        "admission" => (
            "admission_claim_token",
            "admission_claim_generation",
            "admission_claim_revision",
            "admission_claim_expires_at_unix_ms",
        ),
        "claim" => (
            "claim_token",
            "claim_generation",
            "claim_revision",
            "claim_expires_at_unix_ms",
        ),
        _ => return Err(Error::AtomicCommitFailed),
    };
    Ok(column::<Option<Vec<u8>>>(row, token_column)?.as_deref()
        == claim.map(|value| value.token().as_slice())
        && column::<Option<i64>>(row, generation_column)?
            == claim
                .map(|value| i64_from_u64(value.generation().get()))
                .transpose()?
        && column::<Option<i64>>(row, revision_column)?
            == claim
                .map(|value| i64_from_u64(value.row_revision().get()))
                .transpose()?
        && column::<Option<i64>>(row, expiry_column)?
            == claim
                .map(|value| i64_from_u64(value.expires_at_unix_ms()))
                .transpose()?)
}

fn decode_receipt_row(row: &SqliteRow) -> Result<AuthoredAtomicReceipt, Error> {
    let commit_id = AtomicCommitId::new(array(column(row, "commit_id")?)?)?;
    let digest = AtomicCommitDigest::new(array(column(row, "commit_digest")?)?);
    let requested = u64_from_i64(column(row, "requested_at_unix_ms")?)?;
    let committed = u64_from_i64(column(row, "committed_at_unix_ms")?)?;
    let snapshot = decode_snapshot::<ReceiptSnapshot>(column(row, "receipt")?)?;
    if committed < requested {
        return Err(Error::AtomicCommitFailed);
    }
    AuthoredAtomicReceipt::from_durable_parts(
        commit_id,
        digest,
        AtomicCommitDisposition::Committed,
        committed,
        snapshot.outcome,
    )
    .map_err(|_| Error::AtomicCommitFailed)
}

async fn any_artifact_exists(
    transaction: &mut sqlx::Transaction<'_, Sqlite>,
    artifacts: &[AuthoredArtifact],
) -> Result<bool, Error> {
    for artifact in artifacts {
        if row_exists(
            transaction,
            "SELECT 1 FROM radroots_runtime_authored_artifacts WHERE artifact_id = ?",
            artifact.artifact_id().as_bytes(),
        )
        .await?
        {
            return Ok(true);
        }
    }
    Ok(false)
}

async fn any_plan_exists(
    transaction: &mut sqlx::Transaction<'_, Sqlite>,
    plans: &[AuthoredDeliveryPlan],
) -> Result<bool, Error> {
    for plan in plans {
        if row_exists(
            transaction,
            "SELECT 1 FROM radroots_runtime_authored_delivery_plans WHERE plan_id = ?",
            plan.plan_id().as_bytes(),
        )
        .await?
        {
            return Ok(true);
        }
    }
    Ok(false)
}

async fn row_exists<const N: usize>(
    transaction: &mut sqlx::Transaction<'_, Sqlite>,
    query: &'static str,
    id: &[u8; N],
) -> Result<bool, Error> {
    Ok(sqlx::query_scalar::<_, i64>(query)
        .bind(id.as_slice())
        .fetch_optional(&mut **transaction)
        .await
        .map_err(map_backend)?
        .is_some())
}

fn require_artifact_claim(
    claim: Option<&radroots_storage::authored::WorkClaim>,
    fence: &WorkFence,
    now_unix_ms: u64,
) -> Result<(), Error> {
    if !claim.is_some_and(|claim| {
        claim.matches_fence(
            fence.token(),
            fence.generation(),
            fence.row_revision(),
            now_unix_ms,
        )
    }) {
        return Err(Error::DeliveryPlanClaimConflict);
    }
    Ok(())
}

fn require_revision(actual: u64, expected: u64) -> Result<(), Error> {
    if actual != expected {
        return Err(Error::InvalidAuthoredTransition);
    }
    Ok(())
}

fn encode_snapshot<T: Serialize>(value: &T) -> Result<Vec<u8>, Error> {
    let bytes = serde_json::to_vec(value).map_err(|_| Error::AtomicCommitFailed)?;
    if bytes.len() < 2 || bytes.len() > SNAPSHOT_MAX_BYTES {
        return Err(Error::AtomicCommitFailed);
    }
    Ok(bytes)
}

fn decode_snapshot<T: DeserializeOwned>(bytes: Vec<u8>) -> Result<T, Error> {
    if bytes.len() < 2 || bytes.len() > SNAPSHOT_MAX_BYTES {
        return Err(Error::AtomicCommitFailed);
    }
    serde_json::from_slice(&bytes).map_err(|_| Error::AtomicCommitFailed)
}

fn command_target(command: &AuthoredAtomicCommand) -> [u8; 16] {
    match command {
        AuthoredAtomicCommand::Prepare(value) => *value.operation().operation_id().as_bytes(),
        AuthoredAtomicCommand::Claim(value) => match value.target() {
            ClaimAuthoredTarget::ArtifactSigning(id)
            | ClaimAuthoredTarget::ArtifactAdmission(id) => *id.as_bytes(),
            ClaimAuthoredTarget::DeliveryPlan(id) => *id.as_bytes(),
        },
        AuthoredAtomicCommand::ApplySigned(value) => *value.artifact_id().as_bytes(),
        AuthoredAtomicCommand::ApplyAdmission(value) => *value.artifact_id().as_bytes(),
        AuthoredAtomicCommand::ApplyDelivery(value) => *value.plan_id().as_bytes(),
        AuthoredAtomicCommand::ApplyFailure(value) => match value.target() {
            AuthoredWorkTarget::Artifact(id) => *id.as_bytes(),
            AuthoredWorkTarget::DeliveryPlan(id) => *id.as_bytes(),
        },
        AuthoredAtomicCommand::Cancel(value) => match value.target() {
            CancelAuthoredTarget::ArtifactSigning(id)
            | CancelAuthoredTarget::ArtifactAdmission(id) => *id.as_bytes(),
            CancelAuthoredTarget::DeliveryPlan(id) => *id.as_bytes(),
        },
    }
}

fn command_phase(command: &AuthoredAtomicCommand) -> &'static str {
    match command {
        AuthoredAtomicCommand::Prepare(_) => "prepare",
        AuthoredAtomicCommand::Claim(_) => "claim",
        AuthoredAtomicCommand::ApplySigned(_) => "signing",
        AuthoredAtomicCommand::ApplyAdmission(_) => "admission",
        AuthoredAtomicCommand::ApplyDelivery(_) => "delivery",
        AuthoredAtomicCommand::ApplyFailure(value) => match value.failure().phase() {
            WorkPhase::Signing => "signing_failure",
            WorkPhase::Admission => "admission_failure",
            WorkPhase::Delivery => "delivery_failure",
        },
        AuthoredAtomicCommand::Cancel(_) => "cancel",
    }
}

const fn origin_name(value: ArtifactOrigin) -> &'static str {
    match value {
        ArtifactOrigin::Planned => "planned",
        ArtifactOrigin::ImportedSigned => "imported_signed",
    }
}

const fn signing_name(value: SigningState) -> &'static str {
    match value {
        SigningState::Planned => "planned",
        SigningState::Signed => "signed",
        SigningState::Retryable => "retryable",
        SigningState::Indeterminate => "indeterminate",
        SigningState::FailedTerminal => "failed_terminal",
        SigningState::Cancelled => "cancelled",
    }
}

const fn admission_name(value: AdmissionState) -> &'static str {
    match value {
        AdmissionState::Pending => "pending",
        AdmissionState::Inserted => "inserted",
        AdmissionState::Duplicate => "duplicate",
        AdmissionState::Retryable => "retryable",
        AdmissionState::Rejected => "rejected",
        AdmissionState::Cancelled => "cancelled",
    }
}

const fn delivery_state_name(value: AuthoredDeliveryState) -> &'static str {
    match value {
        AuthoredDeliveryState::Pending => "pending",
        AuthoredDeliveryState::Retryable => "retryable",
        AuthoredDeliveryState::Satisfied => "satisfied",
        AuthoredDeliveryState::Exhausted => "exhausted",
        AuthoredDeliveryState::FailedTerminal => "failed_terminal",
        AuthoredDeliveryState::Cancelled => "cancelled",
    }
}

const fn satisfaction_name(value: SatisfactionState) -> &'static str {
    match value {
        SatisfactionState::Satisfied => "satisfied",
        SatisfactionState::Pending => "pending",
        SatisfactionState::Exhausted => "exhausted",
    }
}

fn array<const N: usize>(bytes: Vec<u8>) -> Result<[u8; N], Error> {
    bytes.try_into().map_err(|_| Error::AtomicCommitFailed)
}

fn i64_from_u64(value: u64) -> Result<i64, Error> {
    i64::try_from(value).map_err(|_| Error::AtomicCommitFailed)
}

fn u64_from_i64(value: i64) -> Result<u64, Error> {
    u64::try_from(value).map_err(|_| Error::AtomicCommitFailed)
}

fn map_backend(_: sqlx::Error) -> Error {
    Error::BackendUnavailable
}

fn column<T>(row: &SqliteRow, name: &str) -> Result<T, Error>
where
    for<'decode> T: sqlx::Decode<'decode, Sqlite> + sqlx::Type<Sqlite>,
{
    row.try_get(name).map_err(|_| Error::AtomicCommitFailed)
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use crate::migration::runtime::{MIGRATIONS, migration_sql};
    use core::num::NonZeroU64;
    use radroots_event::{GenericEventDraft, SignedEvent, wire::v1::Nip01EventWire};
    use radroots_event_codec::authoring::AuthoredEventPlan;
    use radroots_storage::{
        authored::{AuthoredArtifact, WorkClaim},
        authored_atomic::{ApplySignedArtifact, ClaimAuthoredWork, PrepareAuthoredOperation},
        authored_delivery::{AuthoredDeliveryIntent, AuthoredDeliveryPlan},
        event::SourceGeneration,
        status::EventStoreMode,
    };
    use radroots_transport::{
        Target, TargetSet,
        policy::{SatisfactionClass, SatisfactionPolicy, TargetPolicy},
    };
    use sqlx::sqlite::SqlitePoolOptions;

    const AUTHOR: &str = "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df";

    async fn store(mode: EventStoreMode) -> SqliteStorage {
        let generation = SourceGeneration::new([91; 32]).expect("generation");
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("memory SQLite");
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await
            .expect("foreign keys");
        for migration in MIGRATIONS {
            sqlx::raw_sql(migration_sql(migration.version()).expect("registered SQL"))
                .execute(&pool)
                .await
                .expect("runtime migration");
        }
        sqlx::query(
            "INSERT INTO radroots_runtime_source_generations (
               generation, sequence_head, state, created_at_unix_ms, retired_at_unix_ms
             ) VALUES (?, 0, 'active', 1, NULL)",
        )
        .bind(generation.as_bytes().as_slice())
        .execute(&pool)
        .await
        .expect("source generation");
        SqliteStorage::new(pool, generation, mode)
    }

    fn plan() -> AuthoredEventPlan {
        AuthoredEventPlan::from_generic(
            GenericEventDraft::new(
                "radroots.social.geochat.v1",
                20_000,
                1_800_100_001,
                Vec::new(),
                "sqlite authored operation",
                AUTHOR,
            )
            .expect("generic draft"),
        )
        .expect("authored plan")
    }

    fn signed(plan: &AuthoredEventPlan) -> SignedEvent {
        let wire = Nip01EventWire {
            id: plan.expected_event_id().to_hex(),
            pubkey: plan.author().to_hex(),
            created_at: plan.created_at(),
            kind: plan.body().kind(),
            tags: plan.body().tags().to_vec(),
            content: plan.body().content().to_owned(),
            sig: "44".repeat(64),
            extra: Default::default(),
        };
        let raw = serde_json::to_string(&wire).expect("raw event");
        SignedEvent::from_wire_verified_id(wire, raw).expect("signed event")
    }

    fn ids() -> (
        OperationInstanceId,
        AuthoredArtifactId,
        AuthoredDeliveryPlanId,
    ) {
        (
            OperationInstanceId::new([1; 16]).expect("operation"),
            AuthoredArtifactId::new([2; 16]).expect("artifact"),
            AuthoredDeliveryPlanId::new([3; 16]).expect("plan"),
        )
    }

    fn prepare() -> (AuthoredAtomicCommand, AuthoredEventPlan) {
        let (operation_id, artifact_id, plan_id) = ids();
        let event_plan = plan();
        let artifact = AuthoredArtifact::planned(artifact_id, operation_id, 0, &event_plan, 10)
            .expect("artifact");
        let operation =
            AuthoredOperation::new(operation_id, vec![artifact_id], 10).expect("operation");
        let targets = TargetSet::new(vec![
            Target::nostr_relay("wss://one.sqlite.example").expect("first"),
            Target::nostr_relay("wss://two.sqlite.example").expect("second"),
        ])
        .expect("targets");
        let intent = AuthoredDeliveryIntent::new(
            "sqlite-authored-delivery",
            targets,
            SatisfactionPolicy::new(SatisfactionClass::Accepted, TargetPolicy::any()),
            10_000,
        )
        .expect("intent");
        let delivery =
            AuthoredDeliveryPlan::new(plan_id, artifact_id, intent, 10).expect("delivery plan");
        let command = PrepareAuthoredOperation::new(
            operation,
            vec![artifact],
            vec![delivery],
            AtomicCommitDigest::new([7; 32]),
            10,
        )
        .expect("prepare");
        (AuthoredAtomicCommand::Prepare(command), event_plan)
    }

    #[tokio::test]
    async fn authored_workflows_match_memory_replay_and_exact_binding_semantics() {
        let store = store(EventStoreMode::ReadWrite).await;
        let (prepare, event_plan) = prepare();
        let committed = store
            .execute_authored(prepare.clone())
            .await
            .expect("prepare");
        assert_eq!(committed.disposition(), AtomicCommitDisposition::Committed);
        assert_eq!(
            store
                .execute_authored(prepare.clone())
                .await
                .expect("replay")
                .disposition(),
            AtomicCommitDisposition::Replay
        );
        assert_eq!(
            store
                .authored_receipt(prepare.commit_id())
                .await
                .expect("receipt")
                .expect("stored receipt")
                .outcome(),
            committed.outcome()
        );

        let artifact = store
            .authored_artifact(ids().1)
            .await
            .expect("artifact query")
            .expect("artifact");
        let claim = WorkClaim::new(
            [4; 16],
            "sqlite-signer",
            NonZeroU64::MIN,
            11,
            50,
            artifact.revision(),
        )
        .expect("claim");
        store
            .execute_authored(AuthoredAtomicCommand::Claim(ClaimAuthoredWork::new(
                ClaimAuthoredTarget::ArtifactSigning(ids().1),
                claim.clone(),
            )))
            .await
            .expect("claim signing");
        store
            .execute_authored(AuthoredAtomicCommand::ApplySigned(
                ApplySignedArtifact::new(
                    ids().1,
                    WorkFence::new(*claim.token(), claim.generation(), claim.row_revision())
                        .expect("fence"),
                    signed(&event_plan),
                    12,
                )
                .expect("apply signed"),
            ))
            .await
            .expect("signed");
        let artifact = store
            .authored_artifact(ids().1)
            .await
            .expect("artifact query")
            .expect("artifact");
        let delivery = store
            .authored_delivery_plan(ids().2)
            .await
            .expect("delivery query")
            .expect("delivery");
        assert_eq!(artifact.signing_state(), SigningState::Signed);
        assert_eq!(
            delivery
                .request()
                .expect("bound request")
                .payload()
                .event()
                .raw_json(),
            artifact
                .signed()
                .expect("signed artifact")
                .event()
                .raw_json()
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM radroots_runtime_authored_delivery_targets",
            )
            .fetch_one(&store.pool)
            .await
            .expect("target count"),
            2
        );
    }

    #[tokio::test]
    async fn statement_failure_rolls_back_complete_preparation_and_receipt() {
        let store = store(EventStoreMode::ReadWrite).await;
        sqlx::query(
            "CREATE TEMP TRIGGER authored_target_fault
             BEFORE INSERT ON radroots_runtime_authored_delivery_targets
             BEGIN SELECT RAISE(ABORT, 'authored target fault'); END",
        )
        .execute(&store.pool)
        .await
        .expect("fault trigger");
        let (prepare, _) = prepare();
        assert_eq!(
            store.execute_authored(prepare).await,
            Err(Error::BackendUnavailable)
        );
        for (table, query) in [
            (
                "radroots_runtime_authored_operations",
                "SELECT COUNT(*) FROM radroots_runtime_authored_operations",
            ),
            (
                "radroots_runtime_authored_artifacts",
                "SELECT COUNT(*) FROM radroots_runtime_authored_artifacts",
            ),
            (
                "radroots_runtime_authored_delivery_plans",
                "SELECT COUNT(*) FROM radroots_runtime_authored_delivery_plans",
            ),
            (
                "radroots_runtime_authored_delivery_targets",
                "SELECT COUNT(*) FROM radroots_runtime_authored_delivery_targets",
            ),
            (
                "radroots_runtime_authored_atomic_commits",
                "SELECT COUNT(*) FROM radroots_runtime_authored_atomic_commits",
            ),
        ] {
            assert_eq!(
                sqlx::query_scalar::<_, i64>(query)
                    .fetch_one(&store.pool)
                    .await
                    .expect("row count"),
                0,
                "{table} retained partial state"
            );
        }
    }

    #[tokio::test]
    async fn corrupt_snapshots_children_claims_and_oversized_wires_fail_closed() {
        let store = store(EventStoreMode::ReadWrite).await;
        let (prepare, _) = prepare();
        store.execute_authored(prepare).await.expect("prepare");

        assert!(
            sqlx::query(
                "UPDATE radroots_runtime_authored_delivery_plans
                 SET claim_token = zeroblob(16) WHERE plan_id = ?",
            )
            .bind(ids().2.as_bytes().as_slice())
            .execute(&store.pool)
            .await
            .is_err()
        );
        assert!(
            sqlx::query(
                "UPDATE radroots_runtime_authored_artifacts
                 SET snapshot = zeroblob(4194305) WHERE artifact_id = ?",
            )
            .bind(ids().1.as_bytes().as_slice())
            .execute(&store.pool)
            .await
            .is_err()
        );
        sqlx::query(
            "UPDATE radroots_runtime_authored_delivery_targets
             SET target_fingerprint = 'forged' WHERE plan_id = ? AND ordinal = 0",
        )
        .bind(ids().2.as_bytes().as_slice())
        .execute(&store.pool)
        .await
        .expect("forge child row");
        assert_eq!(
            store.authored_delivery_plan(ids().2).await,
            Err(Error::InvalidAuthoredDeliveryPlan)
        );
        sqlx::query(
            "UPDATE radroots_runtime_authored_artifacts SET snapshot = x'7b7d'
             WHERE artifact_id = ?",
        )
        .bind(ids().1.as_bytes().as_slice())
        .execute(&store.pool)
        .await
        .expect("forge snapshot");
        assert_eq!(
            store.authored_artifact(ids().1).await,
            Err(Error::AtomicCommitFailed)
        );
    }

    #[tokio::test]
    async fn read_only_and_ready_query_plan_contracts_are_enforced() {
        let store = store(EventStoreMode::ReadOnly).await;
        assert_eq!(
            store.execute_authored(prepare().0).await,
            Err(Error::BackendUnavailable)
        );
        let plan = sqlx::query(
            "EXPLAIN QUERY PLAN
             SELECT plan_id FROM radroots_runtime_authored_delivery_plans
             WHERE state = 'retryable' AND retry_not_before_unix_ms <= 100
             ORDER BY state, retry_not_before_unix_ms, claim_expires_at_unix_ms,
                      updated_at_unix_ms, plan_id",
        )
        .fetch_all(&store.pool)
        .await
        .expect("query plan")
        .iter()
        .map(|row| row.get::<String, _>("detail"))
        .collect::<Vec<_>>()
        .join(" ");
        assert!(plan.contains("radroots_runtime_authored_delivery_ready_idx"));
    }
}
