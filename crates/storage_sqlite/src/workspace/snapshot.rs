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
use radroots_storage::command::SigningTarget;
use radroots_storage::command::{
    ArtifactEvidence, ArtifactSnapshot, BoundSigningPlan, ClaimSnapshot, DeliveryAttemptSummary,
    OperationError, OperationSnapshot, RetainedOperation, SigningAttempt,
};
use sqlx::{Connection, SqliteConnection};

struct SeenArtifact {
    report: ArtifactSnapshot,
    claim: Option<Vec<u8>>,
    evidence: Option<ProtectedEvidence>,
    obtained: Option<ArtifactEvidence>,
}
pub(super) struct RetainedGraph {
    retained: Option<(RetainedOperation, operation::ProtectedOperation)>,
    protected_plan: Option<ProtectedPlan>,
    artifacts: Vec<SeenArtifact>,
}
impl RetainedGraph {
    pub(super) fn obtained_artifact(&self, target: SigningTarget) -> Option<&ArtifactEvidence> {
        self.artifacts
            .iter()
            .find(|v| v.report.target() == target)
            .and_then(|v| v.obtained.as_ref())
    }
    pub(super) fn obtained(&self, recipient: PublicKey) -> Option<&ArtifactEvidence> {
        self.artifacts
            .iter()
            .find(|seen| seen.report.target().recipient() == Some(recipient))
            .and_then(|seen| seen.obtained.as_ref())
    }
    pub(super) fn bound_plan(&self) -> Option<&BoundSigningPlan> {
        self.retained
            .as_ref()
            .and_then(|(value, _)| value.bound_plan())
    }
    pub(super) fn reports(&self) -> impl Iterator<Item = ArtifactSnapshot> + '_ {
        self.artifacts.iter().map(|value| value.report)
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
        let mut artifacts = Vec::new();
        let mut protected_plan = None;
        let retained = if let Some((prepared, protected)) = original {
            operation::check_digest(&prepared, binding.actor(), expected_digest)?;
            same_binding(prepared.request().binding(), binding)?;
            let (prepared, bound) = plan::read(connection, selected, prepared, crypto).await?;
            let source = if let Some((plan, transaction, time, protected)) = bound {
                protected_plan = Some(protected);
                let mut bound =
                    BoundSigningPlan::from_committed_store(prepared, plan, transaction, time)?;
                let keys = match bound.plan().chat() {
                    Some(plan) => plan
                        .seals()
                        .iter()
                        .map(|seal| SigningTarget::BusinessSeal(seal.recipient()))
                        .collect::<Vec<_>>(),
                    None => vec![SigningTarget::PublicEvent],
                };
                for recipient in keys {
                    let (returned, claim) =
                        attempt::read(connection, selected, bound, recipient, crypto).await?;
                    let expected = EnvelopeEventId::from_bytes(
                        returned
                            .plan()
                            .artifact(recipient)
                            .map_err(|_| OperationError::RecoveryRequired)?
                            .event_id(),
                    );
                    if let Some((id, transaction, time, protected)) = claim {
                        let attempt = SigningAttempt::from_committed_store(
                            returned,
                            recipient,
                            id,
                            transaction,
                            time,
                        )?;
                        let captured = evidence::read(connection, &attempt, crypto).await?;
                        let claim = ClaimSnapshot::from_store_report(&attempt)?;
                        let (obtained, evidence, original) = match captured {
                            Some((value, protected)) => {
                                (Some(protected.receipt), Some(protected), Some(value))
                            }
                            None => (None, None, None),
                        };
                        artifacts.push(SeenArtifact {
                            report: ArtifactSnapshot::from_store_report(
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
                        artifacts.push(SeenArtifact {
                            report: ArtifactSnapshot::from_store_report(
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
                let mut keys = match prepared.request().command().chat() {
                    Some(command) => vec![
                        SigningTarget::BusinessSeal(command.context().actor().participant()),
                        SigningTarget::BusinessSeal(command.peer()),
                    ],
                    None => vec![SigningTarget::PublicEvent],
                };
                keys.sort_unstable_by_key(|target| target.key());
                keys.dedup();
                for recipient in keys {
                    artifacts.push(SeenArtifact {
                        report: ArtifactSnapshot::from_store_report(recipient, None, None, None),
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
            artifacts,
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
            for seen in &self.artifacts {
                if let (Some(claim), Some(protected)) = (seen.report.claim(), &seen.claim) {
                    attempt::unchanged_snapshot(connection, claim, protected).await?;
                    let scope = IssuanceScope {
                        workspace: claim.workspace_id(),
                        operation: claim.operation_id(),
                        plan_digest: *claim.plan_digest(),
                        attempt: claim.attempt_id(),
                        target: claim.target(),
                    };
                    evidence::unchanged_scope(connection, scope, seen.evidence.as_ref()).await?;
                } else {
                    absent_claim(connection, selected, operation_id, seen.report.target()).await?;
                }
            }
            graph_counts(
                connection,
                selected.workspace_id().as_bytes(),
                operation_id.as_bytes(),
                &self.artifacts,
            )
            .await?;
        } else {
            let count:i64=sqlx::query_scalar(attempt::sql(connection, "SELECT EXISTS(SELECT 1 FROM operations WHERE workspace=?1 AND operation_id=?2)+EXISTS(SELECT 1 FROM signing_plans WHERE workspace=?1 AND operation_id=?2)+EXISTS(SELECT 1 FROM signing_plan_parts WHERE workspace=?1 AND operation_id=?2)+EXISTS(SELECT 1 FROM signing_attempts WHERE workspace=?1 AND operation_id=?2)+EXISTS(SELECT 1 FROM seal_evidence WHERE workspace=?1 AND operation_id=?2)+EXISTS(SELECT 1 FROM seal_evidence_parts WHERE workspace=?1 AND operation_id=?2)").await?)
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
                self.artifacts.into_iter().map(|seen| seen.report).collect(),
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
    listing: super::listing_installation::Records,
    deliveries: DeliveryGraph,
    observations: Observations,
}
impl RetainedFacts {
    pub(super) fn graph(&self) -> &RetainedGraph {
        &self.graph
    }
    pub(super) fn listing(&self) -> &super::listing_installation::Records {
        &self.listing
    }

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
        Self::from_graph(connection, selected, operation, graph, crypto).await
    }
    pub(super) async fn from_graph(
        connection: &mut SqliteConnection,
        selected: &AttachmentInput,
        operation: OperationId,
        graph: RetainedGraph,
        crypto: &WorkspaceCrypto,
    ) -> Result<Self, OperationError> {
        let records = Records::read(connection, selected, operation, &graph, crypto).await?;
        let listing = super::listing_installation::Records::read(
            connection, selected, operation, &graph, crypto,
        )
        .await?;
        let deliveries = DeliveryGraph::read(
            connection, selected, operation, &graph, &records, &listing, crypto,
        )
        .await?;
        let observations =
            Observations::read(connection, &deliveries, selected, operation, crypto).await?;
        Ok(Self {
            graph,
            records,
            listing,
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
        self.listing
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
                    .with_public_installation(
                        self.listing.installation(),
                        self.listing.work().into_iter().collect(),
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
    artifacts: &[SeenArtifact],
) -> Result<(), OperationError> {
    // The valid graph has at most two claims, two obtained manifests and eight
    // data parts. One excess sentinel is sufficient to reject arbitrary extras.
    let counts:(i64,i64,i64)=sqlx::query_as(attempt::sql(connection, "SELECT (SELECT count(*) FROM (SELECT 1 FROM signing_attempts WHERE workspace=?1 AND operation_id=?2 LIMIT 3)),(SELECT count(*) FROM (SELECT 1 FROM seal_evidence WHERE workspace=?1 AND operation_id=?2 LIMIT 3)),(SELECT count(*) FROM (SELECT 1 FROM seal_evidence_parts WHERE workspace=?1 AND operation_id=?2 LIMIT 9))").await?)
        .bind(workspace.as_slice()).bind(operation.as_slice()).fetch_one(connection).await.map_err(|_|OperationError::RecoveryRequired)?;
    let claims = artifacts.iter().filter(|seen| seen.claim.is_some()).count() as i64;
    let evidence = artifacts
        .iter()
        .filter(|seen| seen.evidence.is_some())
        .count() as i64;
    let parts = artifacts
        .iter()
        .filter_map(|seen| seen.evidence.as_ref())
        .map(|value| value.parts.len() as i64)
        .sum::<i64>();
    if counts != (claims, evidence, parts) {
        return Err(OperationError::RecoveryRequired);
    }
    Ok(())
}
