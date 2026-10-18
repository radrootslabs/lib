//! One authenticated coherent observation of the complete retained T0/T1/S graph.
use super::{
    AttachmentProof, WorkspaceBinding,
    admission::{Journal, recheck, same_binding},
    attempt,
    crypto::IssuanceScope,
    evidence::{self, ProtectedEvidence},
    issuance::absent_claim,
    operation, plan,
};
use radroots_domain::{CommandDigest, EnvelopeEventId, OperationId, PublicKey};
use radroots_storage::command::{
    BoundSigningPlan, ClaimSnapshot, OperationError, OperationSnapshot, RecipientSnapshot,
    RetainedOperation, SigningAttempt,
};
use sqlx::{Connection, SqliteConnection};

struct SeenRecipient {
    report: RecipientSnapshot,
    claim: Option<Vec<u8>>,
    evidence: Option<ProtectedEvidence>,
}
impl Journal<'_> {
    pub(super) async fn query_snapshot(
        &mut self,
        binding: WorkspaceBinding,
        operation_id: OperationId,
        expected_digest: Option<CommandDigest>,
        proof: AttachmentProof,
    ) -> Result<Option<OperationSnapshot>, OperationError> {
        let (record, grant, _) = self.access(&binding, &proof).await?;
        let original =
            operation::read(self.connection, self.selected, operation_id, self.crypto).await?;
        let mut recipients = Vec::new();
        let mut protected_plan = None;
        let retained = if let Some((prepared, protected)) = original {
            operation::check_digest(&prepared, binding.actor(), expected_digest)?;
            same_binding(prepared.request().binding(), &binding)?;
            let (prepared, bound) =
                plan::read(self.connection, self.selected, prepared, self.crypto).await?;
            let source = if let Some((plan, transaction, time, protected)) = bound {
                protected_plan = Some(protected);
                let mut bound =
                    BoundSigningPlan::from_committed_store(prepared, plan, transaction, time)?;
                let keys: Vec<PublicKey> = bound
                    .plan()
                    .seals()
                    .iter()
                    .map(|seal| seal.recipient())
                    .collect();
                for recipient in keys {
                    let (returned, claim) = attempt::read(
                        self.connection,
                        self.selected,
                        bound,
                        recipient,
                        self.crypto,
                    )
                    .await?;
                    let expected = returned
                        .plan()
                        .seals()
                        .iter()
                        .find(|seal| seal.recipient() == recipient)
                        .map(|seal| EnvelopeEventId::from_bytes(seal.event_id()))
                        .ok_or(OperationError::RecoveryRequired)?;
                    if let Some((id, transaction, time, protected)) = claim {
                        let attempt = SigningAttempt::from_committed_store(
                            returned,
                            recipient,
                            id,
                            transaction,
                            time,
                        )?;
                        let captured =
                            evidence::read(self.connection, &attempt, self.crypto).await?;
                        let claim = ClaimSnapshot::from_store_report(&attempt);
                        let (obtained, evidence) = match captured {
                            Some((_, protected)) => (Some(protected.receipt), Some(protected)),
                            None => (None, None),
                        };
                        recipients.push(SeenRecipient {
                            report: RecipientSnapshot::from_store_report(
                                recipient,
                                Some(expected),
                                Some(claim),
                                obtained,
                            ),
                            claim: Some(protected),
                            evidence,
                        });
                        bound = attempt.into_bound_plan();
                    } else {
                        recipients.push(SeenRecipient {
                            report: RecipientSnapshot::from_store_report(
                                recipient,
                                Some(expected),
                                None,
                                None,
                            ),
                            claim: None,
                            evidence: None,
                        });
                        bound = returned;
                    }
                }
                RetainedOperation::Planned(Box::new(bound))
            } else {
                let command = prepared.request().command();
                let mut keys = vec![command.context().actor().participant(), command.peer()];
                keys.sort_unstable();
                keys.dedup();
                for recipient in keys {
                    recipients.push(SeenRecipient {
                        report: RecipientSnapshot::from_store_report(recipient, None, None, None),
                        claim: None,
                        evidence: None,
                    });
                }
                RetainedOperation::Prepared(prepared)
            };
            Some((source, protected))
        } else {
            None
        };
        // No provider or signature verification work occurs under this lock.
        let mut transaction = self
            .connection
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(|_| OperationError::StorageFailure)?;
        let checked = async {
            let observed = recheck(&mut transaction, self.namespace, self.selected, &grant, &record, &binding, &proof, self.clock).await?;
            if let Some((source, protected)) = &retained {
                operation::unchanged(&mut transaction, source.prepared(), protected).await?;
                plan::unchanged(&mut transaction, source.prepared(), protected_plan.as_ref()).await?;
                for seen in &recipients {
                    if let (Some(claim), Some(protected)) = (seen.report.claim(), &seen.claim) {
                        attempt::unchanged_snapshot(&mut transaction, claim, protected).await?;
                        let scope = IssuanceScope {
                            workspace: claim.workspace_id(), operation: claim.operation_id(),
                            plan_digest: *claim.plan_digest(), attempt: claim.attempt_id(), recipient: claim.recipient(),
                        };
                        evidence::unchanged_scope(&mut transaction, scope, seen.evidence.as_ref()).await?;
                    } else {
                        absent_claim(&mut transaction, self.selected, operation_id, seen.report.recipient()).await?;
                    }
                }
                graph_counts(&mut transaction, self.selected.workspace_id().as_bytes(), operation_id.as_bytes(), &recipients).await?;
            } else {
                let count:i64=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM operations WHERE workspace=?1 AND operation_id=?2)+EXISTS(SELECT 1 FROM signing_plans WHERE workspace=?1 AND operation_id=?2)+EXISTS(SELECT 1 FROM signing_plan_parts WHERE workspace=?1 AND operation_id=?2)+EXISTS(SELECT 1 FROM signing_attempts WHERE workspace=?1 AND operation_id=?2)+EXISTS(SELECT 1 FROM seal_evidence WHERE workspace=?1 AND operation_id=?2)+EXISTS(SELECT 1 FROM seal_evidence_parts WHERE workspace=?1 AND operation_id=?2)")
                    .bind(self.selected.workspace_id().as_bytes().as_slice()).bind(operation_id.as_bytes().as_slice()).fetch_one(&mut *transaction).await.map_err(|_|OperationError::RecoveryRequired)?;
                if count != 0 { return Err(OperationError::RecoveryRequired); }
            }
            Ok::<_,OperationError>(observed.observed_at())
        }.await;
        if transaction.rollback().await.is_err() {
            *self.uncertain = true;
            return Err(OperationError::RecoveryRequired);
        }
        let observed = checked?;
        match retained {
            Some((source, _)) => OperationSnapshot::from_store_report(
                source,
                recipients.into_iter().map(|seen| seen.report).collect(),
                observed,
            )
            .map(Some),
            None => Ok(None),
        }
    }
}
async fn graph_counts(
    connection: &mut SqliteConnection,
    workspace: &[u8; 32],
    operation: &[u8; 32],
    recipients: &[SeenRecipient],
) -> Result<(), OperationError> {
    // The valid graph has at most two claims, two obtained manifests and eight
    // data parts. One excess sentinel is sufficient to reject arbitrary extras.
    let counts:(i64,i64,i64)=sqlx::query_as("SELECT (SELECT count(*) FROM (SELECT 1 FROM signing_attempts WHERE workspace=?1 AND operation_id=?2 LIMIT 3)),(SELECT count(*) FROM (SELECT 1 FROM seal_evidence WHERE workspace=?1 AND operation_id=?2 LIMIT 3)),(SELECT count(*) FROM (SELECT 1 FROM seal_evidence_parts WHERE workspace=?1 AND operation_id=?2 LIMIT 9))")
        .bind(workspace.as_slice()).bind(operation.as_slice()).fetch_one(connection).await.map_err(|_|OperationError::RecoveryRequired)?;
    let claims = recipients
        .iter()
        .filter(|seen| seen.claim.is_some())
        .count() as i64;
    let evidence = recipients
        .iter()
        .filter(|seen| seen.evidence.is_some())
        .count() as i64;
    let parts = recipients
        .iter()
        .filter_map(|seen| seen.evidence.as_ref())
        .map(|value| value.parts.len() as i64)
        .sum::<i64>();
    if counts != (claims, evidence, parts) {
        return Err(OperationError::RecoveryRequired);
    }
    Ok(())
}
