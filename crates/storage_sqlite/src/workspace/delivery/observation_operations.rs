use super::super::{
    AttachmentProof, WorkspaceBinding,
    admission::{Journal, recheck},
    installation::Records,
    snapshot::RetainedGraph,
    worker,
};
use super::{
    graph::Graph,
    observation::{self, Observations},
};
use radroots_domain::{CommandDigest, OperationId};
use radroots_storage::command::{
    DeliveryObservation, DeliveryObservationOutcome, OperationError, SigningTarget,
};
use radroots_transport::sink::DeliveryRequestId;
use sqlx::Connection;

impl Journal<'_> {
    pub(in crate::workspace) async fn capture_delivery_observation(
        &mut self,
        binding: WorkspaceBinding,
        proof: AttachmentProof,
        supplied: DeliveryObservation,
    ) -> Result<DeliveryObservationOutcome, OperationError> {
        let (record, grant) = self.capture_access(binding.input(), &proof).await?;
        let work = supplied.work();
        let e = work.evidence();
        let original = RetainedGraph::read(
            self.connection,
            self.selected,
            &binding,
            e.operation_id(),
            Some(e.command_digest()),
            self.crypto,
        )
        .await?;
        let installed = Records::read(
            self.connection,
            self.selected,
            e.operation_id(),
            &original,
            self.crypto,
        )
        .await?;
        let public = super::super::listing_installation::Records::read(
            self.connection,
            self.selected,
            e.operation_id(),
            &original,
            self.crypto,
        )
        .await?;
        let deliveries = Graph::read(
            self.connection,
            self.selected,
            e.operation_id(),
            &original,
            &installed,
            &public,
            self.crypto,
        )
        .await?;
        let observations = Observations::read(
            self.connection,
            &deliveries,
            self.selected,
            e.operation_id(),
            self.crypto,
        )
        .await?;
        let actual = deliveries
            .attempts
            .iter()
            .find(|value| {
                value.work() == work && value.intent().request_id() == supplied.request_id()
            })
            .ok_or(OperationError::InvalidEvidence)?;
        let checked = DeliveryObservation::from_store_report(
            actual,
            supplied.target_receipt(),
            supplied.observed_at_unix_ms(),
        )?;
        if checked != supplied {
            return Err(OperationError::InvalidEvidence);
        }
        let existing = observations.reports.iter().find(|value| {
            value.request_id() == supplied.request_id()
                && value.work() == work
                && value.target_receipt().target().fingerprint()
                    == supplied.target_receipt().target().fingerprint()
                && value.kind() == supplied.kind()
        });
        // The first original outcome of each class, including absent time, is
        // immutable. Further classes append; they do not replace earlier facts.
        let retained = existing.cloned().unwrap_or(supplied);
        let fresh = existing.is_none();
        let protected = if fresh {
            Some(observation::protect(retained.clone(), self.crypto).await?)
        } else {
            None
        };
        let mut transaction = self
            .connection
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(|_| OperationError::StorageFailure)?;
        let changed = async {
            self.namespace.validate()?;
            if worker::read(&mut transaction, self.selected).await? != grant {
                return Err(OperationError::RecoveryRequired);
            }
            record.authenticate_retained_capture(binding.input(), &proof)?;
            original
                .unchanged(&mut transaction, self.selected, e.operation_id())
                .await?;
            installed
                .unchanged(&mut transaction, self.selected, e.operation_id())
                .await?;
            public
                .unchanged(&mut transaction, self.selected, e.operation_id())
                .await?;
            deliveries
                .unchanged(&mut transaction, self.selected, e.operation_id())
                .await?;
            observations
                .unchanged(&mut transaction, self.selected, e.operation_id())
                .await?;
            if let Some(value) = &protected {
                value.insert(&mut transaction).await?;
            }
            Ok::<_, OperationError>(())
        }
        .await;
        if let Err(error) = changed {
            if transaction.rollback().await.is_err() {
                *self.uncertain = true;
                return Ok(DeliveryObservationOutcome::Unknown(retained));
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
            return Ok(DeliveryObservationOutcome::Unknown(retained));
        }
        Ok(DeliveryObservationOutcome::Committed(retained))
    }

    pub(in crate::workspace) async fn query_delivery_observations(
        &mut self,
        binding: WorkspaceBinding,
        operation: OperationId,
        digest: CommandDigest,
        target: SigningTarget,
        request: DeliveryRequestId,
        proof: AttachmentProof,
    ) -> Result<Vec<DeliveryObservation>, OperationError> {
        if request.as_str().len() != 64
            || !request
                .as_str()
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(OperationError::InvalidCommand);
        }
        let (record, grant, _) = self.access(&binding, &proof).await?;
        let original = RetainedGraph::read(
            self.connection,
            self.selected,
            &binding,
            operation,
            Some(digest),
            self.crypto,
        )
        .await?;
        let installed = Records::read(
            self.connection,
            self.selected,
            operation,
            &original,
            self.crypto,
        )
        .await?;
        let public = super::super::listing_installation::Records::read(
            self.connection,
            self.selected,
            operation,
            &original,
            self.crypto,
        )
        .await?;
        let deliveries = Graph::read(
            self.connection,
            self.selected,
            operation,
            &original,
            &installed,
            &public,
            self.crypto,
        )
        .await?;
        let observations = Observations::read(
            self.connection,
            &deliveries,
            self.selected,
            operation,
            self.crypto,
        )
        .await?;
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
            original
                .unchanged(&mut transaction, self.selected, operation)
                .await?;
            installed
                .unchanged(&mut transaction, self.selected, operation)
                .await?;
            public
                .unchanged(&mut transaction, self.selected, operation)
                .await?;
            deliveries
                .unchanged(&mut transaction, self.selected, operation)
                .await?;
            observations
                .unchanged(&mut transaction, self.selected, operation)
                .await?;
            Ok::<_, OperationError>(())
        }
        .await;
        if transaction.rollback().await.is_err() {
            *self.uncertain = true;
            return Err(OperationError::RecoveryRequired);
        }
        checked?;
        Ok(observations
            .reports
            .into_iter()
            .filter(|value| value.request_id() == &request && value.work().target() == target)
            .collect())
    }
}
