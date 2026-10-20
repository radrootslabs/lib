//! One authenticated coherent observation of the complete retained T0/T1/S graph.
use super::{
    AttachmentInput, AttachmentProof, WorkspaceBinding, WorkspaceCrypto,
    admission::{Journal, recheck, same_binding},
    attempt,
    crypto::IssuanceScope,
    delivery::{graph::Graph as DeliveryGraph, observation::Observations},
    evidence::{self, ProtectedEvidence},
    installation::Records,
    issuance::absent_claim,
    operation, plan,
    plan::ProtectedPlan,
};
use radroots_domain::{CommandDigest, EnvelopeEventId, OperationId, PublicKey, UnixSeconds};
use radroots_storage::command::{
    BoundSigningPlan, ClaimSnapshot, DeliveryAttemptSummary, OperationError, OperationSnapshot,
    RecipientSnapshot, RetainedOperation, SealEvidence, SigningAttempt,
};
use sqlx::{Connection, SqliteConnection};

struct SeenRecipient {
    report: RecipientSnapshot,
    claim: Option<Vec<u8>>,
    evidence: Option<ProtectedEvidence>,
    obtained: Option<SealEvidence>,
}
pub(super) struct RetainedGraph {
    retained: Option<(RetainedOperation, Vec<u8>)>,
    protected_plan: Option<ProtectedPlan>,
    recipients: Vec<SeenRecipient>,
}
impl RetainedGraph {
    pub(super) fn obtained(&self, recipient: PublicKey) -> Option<&SealEvidence> {
        self.recipients
            .iter()
            .find(|seen| seen.report.recipient() == recipient)
            .and_then(|seen| seen.obtained.as_ref())
    }
    pub(super) fn bound_plan(&self) -> Option<&BoundSigningPlan> {
        self.retained
            .as_ref()
            .and_then(|(value, _)| value.bound_plan())
    }
    pub(super) fn reports(&self) -> impl Iterator<Item = RecipientSnapshot> + '_ {
        self.recipients.iter().map(|value| value.report)
    }
    pub(super) async fn read(
        connection: &mut SqliteConnection,
        selected: &AttachmentInput,
        binding: &WorkspaceBinding,
        operation_id: OperationId,
        expected_digest: Option<CommandDigest>,
        crypto: &WorkspaceCrypto,
    ) -> Result<Self, OperationError> {
        let original = operation::read(connection, selected, operation_id, crypto).await?;
        let mut recipients = Vec::new();
        let mut protected_plan = None;
        let retained = if let Some((prepared, protected)) = original {
            operation::check_digest(&prepared, binding.actor(), expected_digest)?;
            same_binding(prepared.request().binding(), binding)?;
            let (prepared, bound) = plan::read(connection, selected, prepared, crypto).await?;
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
                    let (returned, claim) =
                        attempt::read(connection, selected, bound, recipient, crypto).await?;
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
                        let captured = evidence::read(connection, &attempt, crypto).await?;
                        let claim = ClaimSnapshot::from_store_report(&attempt);
                        let (obtained, evidence, original) = match captured {
                            Some((value, protected)) => {
                                (Some(protected.receipt), Some(protected), Some(value))
                            }
                            None => (None, None, None),
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
                            obtained: original,
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
                            obtained: None,
                        });
                        bound = returned;
                    }
                }
                RetainedOperation::Planned(Box::new(bound))
            } else {
                let command = prepared.request().chat_command()?;
                let mut keys = vec![command.context().actor().participant(), command.peer()];
                keys.sort_unstable();
                keys.dedup();
                for recipient in keys {
                    recipients.push(SeenRecipient {
                        report: RecipientSnapshot::from_store_report(recipient, None, None, None),
                        claim: None,
                        evidence: None,
                        obtained: None,
                    });
                }
                RetainedOperation::Prepared(prepared)
            };
            Some((source, protected))
        } else {
            None
        };
        Ok(Self {
            retained,
            protected_plan,
            recipients,
        })
    }
    pub(super) async fn unchanged(
        &self,
        connection: &mut SqliteConnection,
        selected: &AttachmentInput,
        operation_id: OperationId,
    ) -> Result<(), OperationError> {
        if let Some((source, protected)) = &self.retained {
            operation::unchanged(connection, source.prepared(), protected).await?;
            plan::unchanged(connection, source.prepared(), self.protected_plan.as_ref()).await?;
            for seen in &self.recipients {
                if let (Some(claim), Some(protected)) = (seen.report.claim(), &seen.claim) {
                    attempt::unchanged_snapshot(connection, claim, protected).await?;
                    let scope = IssuanceScope {
                        workspace: claim.workspace_id(),
                        operation: claim.operation_id(),
                        plan_digest: *claim.plan_digest(),
                        attempt: claim.attempt_id(),
                        recipient: claim.recipient(),
                    };
                    evidence::unchanged_scope(connection, scope, seen.evidence.as_ref()).await?;
                } else {
                    absent_claim(connection, selected, operation_id, seen.report.recipient())
                        .await?;
                }
            }
            graph_counts(
                connection,
                selected.workspace_id().as_bytes(),
                operation_id.as_bytes(),
                &self.recipients,
            )
            .await?;
        } else {
            let count:i64=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM operations WHERE workspace=?1 AND operation_id=?2)+EXISTS(SELECT 1 FROM signing_plans WHERE workspace=?1 AND operation_id=?2)+EXISTS(SELECT 1 FROM signing_plan_parts WHERE workspace=?1 AND operation_id=?2)+EXISTS(SELECT 1 FROM signing_attempts WHERE workspace=?1 AND operation_id=?2)+EXISTS(SELECT 1 FROM seal_evidence WHERE workspace=?1 AND operation_id=?2)+EXISTS(SELECT 1 FROM seal_evidence_parts WHERE workspace=?1 AND operation_id=?2)")
                    .bind(selected.workspace_id().as_bytes().as_slice()).bind(operation_id.as_bytes().as_slice()).fetch_one(connection).await.map_err(|_|OperationError::RecoveryRequired)?;
            if count != 0 {
                return Err(OperationError::RecoveryRequired);
            }
        }
        Ok(())
    }
    pub(super) fn into_snapshot(
        self,
        observed: UnixSeconds,
    ) -> Result<Option<OperationSnapshot>, OperationError> {
        match self.retained {
            Some((source, _)) => OperationSnapshot::from_store_report(
                source,
                self.recipients
                    .into_iter()
                    .map(|seen| seen.report)
                    .collect(),
                observed,
            )
            .map(Some),
            None => Ok(None),
        }
    }
}
/// Complete authenticated retained facts, read outside a transaction and
/// compared together inside it. Absence of an earlier phase is not safe to
/// report or replace while any later phase still depends on that history.
pub(super) struct RetainedFacts {
    graph: RetainedGraph,
    records: Records,
    deliveries: DeliveryGraph,
    observations: Observations,
}
impl RetainedFacts {
    pub(super) async fn read(
        connection: &mut SqliteConnection,
        selected: &AttachmentInput,
        binding: &WorkspaceBinding,
        operation: OperationId,
        digest: Option<CommandDigest>,
        crypto: &WorkspaceCrypto,
    ) -> Result<Self, OperationError> {
        let graph =
            RetainedGraph::read(connection, selected, binding, operation, digest, crypto).await?;
        let records = Records::read(connection, selected, operation, &graph, crypto).await?;
        let deliveries =
            DeliveryGraph::read(connection, selected, operation, &graph, &records, crypto).await?;
        let observations =
            Observations::read(connection, &deliveries, selected, operation, crypto).await?;
        Ok(Self {
            graph,
            records,
            deliveries,
            observations,
        })
    }
    pub(super) async fn unchanged(
        &self,
        connection: &mut SqliteConnection,
        selected: &AttachmentInput,
        operation: OperationId,
    ) -> Result<(), OperationError> {
        self.graph
            .unchanged(connection, selected, operation)
            .await?;
        self.records
            .unchanged(connection, selected, operation)
            .await?;
        self.deliveries
            .unchanged(connection, selected, operation)
            .await?;
        self.observations
            .unchanged(connection, selected, operation)
            .await
    }
    fn into_snapshot(
        self,
        observed: UnixSeconds,
    ) -> Result<Option<OperationSnapshot>, OperationError> {
        self.graph
            .into_snapshot(observed)?
            .map(|snapshot| {
                snapshot
                    .with_installation(
                        self.records.message.map(|value| value.report),
                        self.records
                            .work
                            .into_iter()
                            .map(|value| value.report)
                            .collect(),
                    )?
                    .with_delivery(
                        self.deliveries
                            .attempts
                            .iter()
                            .map(DeliveryAttemptSummary::from_attempt)
                            .collect(),
                        self.observations.reports,
                    )
            })
            .transpose()
    }
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
        let facts = RetainedFacts::read(
            self.connection,
            self.selected,
            &binding,
            operation_id,
            expected_digest,
            self.crypto,
        )
        .await?;
        // Provider and signature verification work precedes the transaction.
        let mut transaction = self
            .connection
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(|_| OperationError::StorageFailure)?;
        let checked = async {
            let observed = recheck(
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
            facts
                .unchanged(&mut transaction, self.selected, operation_id)
                .await?;
            Ok::<_, OperationError>(observed.observed_at())
        }
        .await;
        if transaction.rollback().await.is_err() {
            *self.uncertain = true;
            return Err(OperationError::RecoveryRequired);
        }
        facts.into_snapshot(checked?)
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
