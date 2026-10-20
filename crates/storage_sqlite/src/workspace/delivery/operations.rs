use super::super::{
    AttachmentProof, WorkspaceBinding,
    admission::{Journal, recheck},
    installation::Records,
    snapshot::RetainedGraph,
};
use super::{codec, graph::Graph, observation::Observations};
use nostr::JsonUtil;
use radroots_domain::{CommandDigest, OperationId, PublicKey, TransactionId};
use radroots_storage::command::{
    CommitAttempt, DeliveryAttempt, DeliveryAttemptOutcome, DeliveryAttemptRequest,
    EnvelopeCandidate, OperationError, PublicationWork,
};
use radroots_transport::sink::DeliveryRequestId;
use sha2::{Digest, Sha256};
use sqlx::Connection;
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;

fn contains_original(
    candidate: &EnvelopeCandidate,
    work: PublicationWork,
    original: &RetainedGraph,
) -> Result<(), OperationError> {
    let bound = original
        .bound_plan()
        .ok_or(OperationError::RecoveryRequired)?;
    let source = original
        .obtained(work.evidence().recipient())
        .ok_or(OperationError::RecoveryRequired)?;
    let actor = bound
        .prepared()
        .request()
        .command()
        .context()
        .actor()
        .participant();
    if candidate.work() != work
        || candidate.author() == actor
        || candidate.author() == bound.prepared().request().chat_command()?.peer()
        || candidate.author() == work.evidence().recipient()
        || source.digest() != work.evidence().digest()
    {
        return Err(OperationError::InvalidEvidence);
    }
    let event = nostr::Event::from_json(candidate.raw_json())
        .map_err(|_| OperationError::InvalidEvidence)?;
    event
        .verify()
        .map_err(|_| OperationError::InvalidEvidence)?;
    let recipient = nostr::PublicKey::from_slice(work.evidence().recipient().as_bytes())
        .map_err(|_| OperationError::InvalidEvidence)?;
    let opened = candidate.with_wrapping_key(|key| {
        let secret =
            nostr::SecretKey::from_slice(key).map_err(|_| OperationError::InvalidEvidence)?;
        let keys = nostr::Keys::new(secret);
        if keys.public_key().to_bytes() != *candidate.author().as_bytes() {
            return Err(OperationError::InvalidEvidence);
        }
        nostr::nips::nip44::decrypt(keys.secret_key(), &recipient, &event.content)
            .map(Zeroizing::new)
            .map_err(|_| OperationError::InvalidEvidence)
    })?;
    let digest: [u8; 32] = Sha256::digest(opened.as_bytes()).into();
    if digest.ct_eq(source.digest()).unwrap_u8() != 1
        || opened.as_bytes() != source.raw_json().as_bytes()
    {
        return Err(OperationError::InvalidEvidence);
    }
    Ok(())
}
fn work(records: &Records, requested: PublicationWork) -> Result<PublicationWork, OperationError> {
    let actual = records
        .work
        .iter()
        .find(|value| value.report.evidence().recipient() == requested.evidence().recipient())
        .ok_or(OperationError::InvalidEvidence)?
        .report;
    if actual != requested {
        return Err(OperationError::InvalidEvidence);
    }
    Ok(actual)
}
fn unknown(anchor: CommitAttempt, attempt: DeliveryRequestId) -> DeliveryAttemptOutcome {
    DeliveryAttemptOutcome::Unknown {
        operation: anchor,
        attempt,
    }
}

impl Journal<'_> {
    pub(in crate::workspace) async fn prepare_delivery(
        &mut self,
        request: DeliveryAttemptRequest,
        proof: AttachmentProof,
        candidate: Option<EnvelopeCandidate>,
    ) -> Result<DeliveryAttemptOutcome, OperationError> {
        let supplied = request.work();
        let evidence = supplied.evidence();
        let (record, grant, current) = self.access(request.binding(), &proof).await?;
        let original = RetainedGraph::read(
            self.connection,
            self.selected,
            request.binding(),
            evidence.operation_id(),
            Some(evidence.command_digest()),
            self.crypto,
        )
        .await?;
        let installed = Records::read(
            self.connection,
            self.selected,
            evidence.operation_id(),
            &original,
            self.crypto,
        )
        .await?;
        let actual = work(&installed, supplied)?;
        let mut deliveries = Graph::read(
            self.connection,
            self.selected,
            evidence.operation_id(),
            &original,
            &installed,
            self.crypto,
        )
        .await?;
        let existing = deliveries.attempts.iter().position(|value| {
            value.work().evidence().recipient() == evidence.recipient()
                && value.intent().request_id() == request.intent().request_id()
        });
        let observations = Observations::read(
            self.connection,
            &deliveries,
            self.selected,
            evidence.operation_id(),
            self.crypto,
        )
        .await?;
        if let Some(index) = existing {
            let value = &deliveries.attempts[index];
            if value.work() != actual
                || value.intent().digest() != request.intent().digest()
                || candidate.as_ref().is_some_and(|candidate| {
                    candidate.work() != actual || candidate.raw_json() != value.raw_json()
                })
            {
                return Err(OperationError::IdempotencyConflict);
            }
        }
        let fresh = existing.is_none();
        if deliveries.attempts.iter().any(|value| {
            value.work() == actual
                && value.intent().publication_policy_digest()
                    != request.intent().publication_policy_digest()
        }) {
            return Err(OperationError::IdempotencyConflict);
        }
        let prepared = if fresh {
            let bound = original
                .bound_plan()
                .ok_or(OperationError::RecoveryRequired)?;
            deliveries.authorize_routes(bound, evidence.recipient())?;
            let ordinals = deliveries.ordinals(evidence.recipient(), request.intent())?;
            if current.observed_at() < actual.queued_at()
                || deliveries
                    .attempts
                    .iter()
                    .any(|value| value.retained_at() > current.observed_at())
            {
                return Err(super::super::WorkspaceError::Unavailable.into());
            }
            if current
                .observed_at()
                .get()
                .checked_mul(1000)
                .is_none_or(|now| now >= request.intent().deadline_unix_ms().get())
            {
                return Err(OperationError::DeadlineExceeded);
            }
            let raw = match &candidate {
                Some(candidate) => {
                    if candidate.event_id() != request.intent().event_id() {
                        return Err(OperationError::InvalidEvidence);
                    }
                    contains_original(candidate, actual, &original)?;
                    Zeroizing::new(String::from(candidate.raw_json()))
                }
                None => {
                    let retained = deliveries
                        .attempts
                        .iter()
                        .find(|value| {
                            value.work() == actual
                                && value.intent().event_id() == request.intent().event_id()
                        })
                        .ok_or(OperationError::InvalidEvidence)?;
                    Zeroizing::new(String::from(retained.raw_json()))
                }
            };
            // A second representation of one outer ID cannot replace the first.
            if deliveries.attempts.iter().any(|value| {
                value.work() == actual
                    && value.intent().event_id() == request.intent().event_id()
                    && value.raw_json() != raw.as_str()
            }) {
                return Err(OperationError::IdempotencyConflict);
            }
            let value = DeliveryAttempt::from_store_report(
                actual,
                request.intent().clone(),
                &raw,
                TransactionId::from_bytes(super::super::random()?),
                current.observed_at(),
                ordinals,
            )?;
            Some(codec::protect(value, self.crypto).await?)
        } else {
            None
        };
        let retained = match (&prepared, existing) {
            (Some(value), _) => &value.report,
            (None, Some(index)) => &deliveries.attempts[index],
            _ => return Err(OperationError::RecoveryRequired),
        };
        let anchor = CommitAttempt::new(
            evidence.workspace_id(),
            evidence.operation_id(),
            evidence.command_digest(),
            Some(retained.transaction_id()),
        );
        let identity = request.intent().request_id().clone();
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
                request.binding(),
                &proof,
                self.clock,
            )
            .await?;
            original
                .unchanged(&mut transaction, self.selected, evidence.operation_id())
                .await?;
            installed
                .unchanged(&mut transaction, self.selected, evidence.operation_id())
                .await?;
            deliveries
                .unchanged(&mut transaction, self.selected, evidence.operation_id())
                .await?;
            observations
                .unchanged(&mut transaction, self.selected, evidence.operation_id())
                .await?;
            if fresh {
                if now.observed_at() < current.observed_at() {
                    return Err(super::super::WorkspaceError::Unavailable.into());
                }
                if now
                    .observed_at()
                    .get()
                    .checked_mul(1000)
                    .is_none_or(|now| now >= request.intent().deadline_unix_ms().get())
                {
                    return Err(OperationError::DeadlineExceeded);
                }
                codec::absent(
                    &mut transaction,
                    actual,
                    codec::request_key(identity.as_str()),
                )
                .await?;
                codec::insert(
                    &mut transaction,
                    prepared.as_ref().ok_or(OperationError::RecoveryRequired)?,
                )
                .await?;
            }
            Ok::<_, OperationError>(())
        }
        .await;
        if let Err(error) = changed {
            if transaction.rollback().await.is_err() {
                *self.uncertain = true;
                return Ok(unknown(anchor, identity));
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
            return Ok(unknown(anchor, identity));
        }
        match prepared {
            Some(value) => Ok(DeliveryAttemptOutcome::Claimed(Box::new(value.report))),
            None => Ok(DeliveryAttemptOutcome::Retained(Box::new(
                deliveries
                    .attempts
                    .remove(existing.ok_or(OperationError::RecoveryRequired)?),
            ))),
        }
    }

    pub(in crate::workspace) async fn query_delivery(
        &mut self,
        binding: WorkspaceBinding,
        operation: OperationId,
        digest: CommandDigest,
        recipient: PublicKey,
        request: DeliveryRequestId,
        proof: AttachmentProof,
    ) -> Result<Option<DeliveryAttempt>, OperationError> {
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
        let mut deliveries = Graph::read(
            self.connection,
            self.selected,
            operation,
            &original,
            &installed,
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
        Ok(deliveries
            .attempts
            .iter()
            .position(|value| {
                value.work().evidence().recipient() == recipient
                    && value.intent().request_id() == &request
            })
            .map(|index| deliveries.attempts.remove(index)))
    }
}
