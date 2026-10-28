//! Constant-memory reduction of bounded original per-recipient transport history.
use super::*;

struct Endpoint {
    fingerprint: TargetFingerprint,
    attempts: u8,
    accepted: bool,
    last: Option<DeliveryObservationKind>,
    observed: Option<NonZeroU64>,
}
struct Recipient {
    participant: PublicKey,
    source: Option<envelope::Retained>,
    first: Option<DeliveryIntent>,
    attempts: u16,
    endpoints: Vec<Endpoint>,
}
impl Recipient {
    fn observe_attempt(&mut self, value: &StockDeliveryAttempt) -> Result<(), OperationError> {
        if self
            .source
            .as_ref()
            .is_none_or(|source| source.source.receipt() != value.source().receipt())
        {
            return Err(OperationError::RecoveryRequired);
        }
        if let Some(original) = &self.first {
            if original.publication_policy_digest() != value.intent().publication_policy_digest() {
                return Err(OperationError::RecoveryRequired);
            }
        } else {
            self.first = Some(value.intent().clone());
            self.endpoints = value
                .intent()
                .targets()
                .targets()
                .iter()
                .map(|target| Endpoint {
                    fingerprint: target.fingerprint().clone(),
                    attempts: 0,
                    accepted: false,
                    last: None,
                    observed: None,
                })
                .collect();
        }
        for (fingerprint, ordinal) in value
            .intent()
            .selection()
            .iter()
            .zip(value.target_ordinals())
        {
            let endpoint = self
                .endpoints
                .iter_mut()
                .find(|endpoint| &endpoint.fingerprint == fingerprint)
                .ok_or(OperationError::RecoveryRequired)?;
            if endpoint.attempts >= 12 || *ordinal != endpoint.attempts + 1 {
                return Err(OperationError::RecoveryRequired);
            }
            endpoint.attempts = *ordinal;
        }
        self.attempts = self
            .attempts
            .checked_add(1)
            .ok_or(OperationError::RecoveryRequired)?;
        if self.attempts > 16 * 12 {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(())
    }
    fn observe(&mut self, value: &StockDeliveryObservation) -> Result<(), OperationError> {
        let endpoint = self
            .endpoints
            .iter_mut()
            .find(|endpoint| endpoint.fingerprint == *value.target_receipt().target().fingerprint())
            .ok_or(OperationError::RecoveryRequired)?;
        if value.target_ordinal() == 0 || value.target_ordinal() > endpoint.attempts {
            return Err(OperationError::RecoveryRequired);
        }
        endpoint.accepted |= value.kind() == DeliveryObservationKind::Accepted;
        endpoint.last = Some(value.kind());
        endpoint.observed = value.observed_at_unix_ms();
        Ok(())
    }
}
pub(super) struct SelectObservation<'a> {
    pub request: &'a DeliveryRequestId,
    pub target: &'a TargetFingerprint,
    pub kind: DeliveryObservationKind,
}
pub(super) struct Graph {
    recipients: [Recipient; 2],
    pub found_attempt: Option<attempt::Retained>,
    pub found_observation: Option<observation::Retained>,
    pub last_at: Option<UnixSeconds>,
}
impl Graph {
    pub(super) async fn read(
        turn: &mut Turn<'_>,
        request: &StockEnvelopeRequest,
        authority: &Authority,
        wanted: Option<&DeliveryRequestId>,
        observed: Option<SelectObservation<'_>>,
    ) -> Result<Self, OperationError> {
        let original = Box::pin(inventory::acceptance::codec::read(
            turn.connection,
            turn.selected,
            turn.crypto,
            request.operation_id(),
        ))
        .await?
        .ok_or(OperationError::MissingHistory)?;
        if original.receipt.attempt().command_digest() != request.command_digest()
            || !original
                .receipt
                .rumor()
                .recipients()
                .contains(&request.recipient())
        {
            return Err(OperationError::InvalidEvidence);
        }
        let mut recipients = Vec::with_capacity(2);
        for participant in original.receipt.rumor().recipients() {
            let selector = StockEnvelopeRequest::new(
                request.binding().clone(),
                request.minimum_checkpoint(),
                request.operation_id(),
                request.command_digest(),
                *participant,
                None,
            )?;
            let source = Box::pin(envelope::lookup(turn, &selector)).await?;
            recipients.push(Recipient {
                participant: *participant,
                source,
                first: None,
                attempts: 0,
                endpoints: vec![],
            });
        }
        let mut result = Self {
            recipients: recipients
                .try_into()
                .map_err(|_| OperationError::RecoveryRequired)?,
            found_attempt: None,
            found_observation: None,
            last_at: None,
        };
        let rows = sqlx::query("SELECT d.entry_id FROM stock_delivery_attempts d JOIN stock_operations o ON o.actor=d.actor AND o.operation_id=d.entry_id WHERE d.actor=? AND d.operation_id=? ORDER BY o.revision LIMIT 385")
            .bind(turn.selected.participant().as_bytes().as_slice()).bind(request.operation_id().as_bytes().as_slice())
            .fetch_all(&mut *turn.connection).await.map_err(|_|OperationError::RecoveryRequired)?;
        if rows.len() > ATTEMPT_COUNT_MAX {
            return Err(OperationError::RecoveryRequired);
        }
        // Exact natural-key constraints and each body-derived projection prove
        // uniqueness; retaining another history-sized identity set is unnecessary.
        for row in rows {
            let entry = entry(&row)?;
            let value = Box::pin(attempt::read_entry(
                turn.connection,
                turn.selected,
                turn.crypto,
                request.binding(),
                authority.input.grant_id(),
                entry,
            ))
            .await?
            .ok_or(OperationError::RecoveryRequired)?;
            let source = value.report.source().receipt();
            if source.attempt().operation_id() != request.operation_id()
                || source.attempt().command_digest() != request.command_digest()
                || result
                    .last_at
                    .is_some_and(|at| value.report.retained_at() < at)
            {
                return Err(OperationError::RecoveryRequired);
            }
            result.last_at = Some(value.report.retained_at());
            result
                .recipient_mut(source.recipient())?
                .observe_attempt(&value.report)?;
            if source.recipient() == request.recipient()
                && wanted.is_some_and(|wanted| wanted == value.report.intent().request_id())
            {
                result.found_attempt = Some(value);
            }
        }
        let rows = sqlx::query("SELECT d.entry_id FROM stock_delivery_observations d JOIN stock_operations o ON o.actor=d.actor AND o.operation_id=d.entry_id WHERE d.actor=? AND d.operation_id=? ORDER BY o.revision LIMIT 2305")
            .bind(turn.selected.participant().as_bytes().as_slice()).bind(request.operation_id().as_bytes().as_slice())
            .fetch_all(&mut *turn.connection).await.map_err(|_|OperationError::RecoveryRequired)?;
        if rows.len() > OBSERVATION_COUNT_MAX {
            return Err(OperationError::RecoveryRequired);
        }
        for row in rows {
            let value = Box::pin(observation::read_entry(
                turn.connection,
                turn.selected,
                turn.crypto,
                request.binding(),
                authority.input.grant_id(),
                entry(&row)?,
            ))
            .await?
            .ok_or(OperationError::RecoveryRequired)?;
            let report = value.receipt.observation();
            let source = report.source();
            let target = report.target_receipt().target().fingerprint();
            if source.attempt().operation_id() != request.operation_id()
                || source.attempt().command_digest() != request.command_digest()
            {
                return Err(OperationError::RecoveryRequired);
            }
            result.recipient_mut(source.recipient())?.observe(report)?;
            if source.recipient() == request.recipient()
                && observed.as_ref().is_some_and(|selected| {
                    selected.request == report.request_id()
                        && selected.target == target
                        && selected.kind == report.kind()
                })
            {
                result.found_observation = Some(value);
            }
        }
        Ok(result)
    }
    fn recipient(&self, participant: PublicKey) -> Result<&Recipient, OperationError> {
        self.recipients
            .iter()
            .find(|value| value.participant == participant)
            .ok_or(OperationError::InvalidEvidence)
    }
    fn recipient_mut(&mut self, participant: PublicKey) -> Result<&mut Recipient, OperationError> {
        self.recipients
            .iter_mut()
            .find(|value| value.participant == participant)
            .ok_or(OperationError::RecoveryRequired)
    }
    pub(super) fn source(
        &self,
        recipient: PublicKey,
    ) -> Result<&envelope::Retained, OperationError> {
        self.recipient(recipient)?
            .source
            .as_ref()
            .ok_or(OperationError::MissingHistory)
    }
    pub(super) fn ordinals(
        &self,
        recipient: PublicKey,
        intent: &DeliveryIntent,
    ) -> Result<Vec<u8>, OperationError> {
        let original = self.recipient(recipient)?;
        if original.first.as_ref().is_some_and(|first| {
            first.publication_policy_digest() != intent.publication_policy_digest()
        }) {
            return Err(OperationError::IdempotencyConflict);
        }
        intent
            .selection()
            .iter()
            .map(|fingerprint| {
                let count = original
                    .endpoints
                    .iter()
                    .find(|value| &value.fingerprint == fingerprint)
                    .map_or(0, |value| value.attempts);
                if count >= 12 {
                    Err(OperationError::DeliveryAttemptLimit)
                } else {
                    Ok(count + 1)
                }
            })
            .collect()
    }
    pub(super) fn summary(
        &self,
        request: &StockEnvelopeRequest,
        checkpoint: InventoryCheckpointRef,
    ) -> Result<StockDeliverySummary, OperationError> {
        let value = self.recipient(request.recipient())?;
        let targets = value
            .endpoints
            .iter()
            .map(|target| {
                StockDeliveryTargetProgress::from_store_report(
                    target.fingerprint.clone(),
                    target.attempts,
                    target.accepted,
                    target.last,
                    target.observed,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        StockDeliverySummary::from_store_report(
            request,
            value.source.as_ref().map(|source| source.source.receipt()),
            value.first.clone(),
            value.attempts,
            targets,
            checkpoint,
        )
    }
}
fn entry(row: &sqlx::sqlite::SqliteRow) -> Result<OperationId, OperationError> {
    let bytes: Vec<u8> = row
        .try_get(0)
        .map_err(|_| OperationError::RecoveryRequired)?;
    Ok(OperationId::from_bytes(
        bytes
            .try_into()
            .map_err(|_| OperationError::RecoveryRequired)?,
    ))
}
