//! Original obtained outers and complete transport results share stock ownership.
use super::*;
use crate::session::commerce::AcceptanceSelection;
use alloc::boxed::Box;
use core::num::NonZeroU64;
use radroots_domain::PublicKey;
use radroots_storage::{command::PrivateEnvelopeCandidate, inventory::*};
use radroots_transport::{DeliveryReceipt, SinkFailure};

pub(in crate::session) struct DeliveryContext {
    pub selection: AcceptanceSelection,
    pub recipient: PublicKey,
    pub store: Shared<dyn InventoryStore>,
    pub proof: AttachmentProof,
    pub _lease: Shared<dyn HostBound>,
}
impl DeliveryContext {
    fn error(&self, reason: OperationError) -> Error {
        Error::capture_pending(reason, OperationContext::retained(self.selection.operation))
    }
    fn request(
        &self,
        observed: Option<NonZeroU64>,
    ) -> Result<StockEnvelopeRequest, OperationError> {
        StockEnvelopeRequest::new(
            self.selection.binding.clone(),
            self.selection.minimum,
            self.selection.operation,
            self.selection.digest,
            self.recipient,
            observed,
        )
    }
    fn proof(&self) -> Result<AttachmentProof, OperationError> {
        self.proof
            .with_secret(|bytes| AttachmentProof::from_protected_input(bytes))
            .map_err(Into::into)
    }
}
pub(in crate::session) struct RetainedEnvelope {
    context: DeliveryContext,
    candidate: PrivateEnvelopeCandidate,
    observed: Option<NonZeroU64>,
    receipt: Mutex<Option<StockEnvelopeReceipt>>,
}
pub(in crate::session) struct RetainedDelivery {
    context: DeliveryContext,
    attempt: Box<StockDeliveryAttempt>,
    result: Result<DeliveryReceipt, SinkFailure>,
    observed: Option<NonZeroU64>,
    complete: Mutex<bool>,
}
impl Owner {
    pub(in crate::session) fn retain_envelope(
        &self,
        context: DeliveryContext,
        candidate: PrivateEnvelopeCandidate,
        observed: Option<NonZeroU64>,
    ) -> Shared<RetainedEnvelope> {
        let entry = Shared::new(RetainedEnvelope {
            context,
            candidate,
            observed,
            receipt: Mutex::new(None),
        });
        self.envelopes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(entry.clone());
        entry
    }
    pub(in crate::session) async fn capture_envelope(
        &self,
        entry: &Shared<RetainedEnvelope>,
    ) -> crate::Result<StockEnvelopeReceipt> {
        let _serial = self.serial.lock().await;
        self.append_envelope(entry, false).await
    }
    async fn append_envelope(
        &self,
        entry: &Shared<RetainedEnvelope>,
        reconciling: bool,
    ) -> crate::Result<StockEnvelopeReceipt> {
        let c = &entry.context;
        if let Some(receipt) = *entry
            .receipt
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
        {
            return Ok(receipt);
        }
        let request = c.request(entry.observed).map_err(|e| c.error(e))?;
        let proof = c.proof().map_err(|e| c.error(e))?;
        let outcome = if reconciling {
            c.store
                .reconcile_stock_envelope(request, proof, &entry.candidate)
                .await
        } else {
            c.store
                .capture_stock_envelope(request, proof, &entry.candidate)
                .await
        }
        .map_err(|e| c.error(e))?;
        let StockEnvelopeOutcome::Committed(receipt) = outcome else {
            return Err(c.error(OperationError::RecoveryRequired));
        };
        let anchor = receipt.attempt();
        if anchor.workspace_id() != c.selection.binding.input().workspace_id()
            || anchor.operation_id() != c.selection.operation
            || anchor.command_digest() != c.selection.digest
            || anchor.transaction_id().is_none()
            || receipt.recipient() != c.recipient
            || receipt.event_id() != entry.candidate.event_id()
            || receipt.digest() != entry.candidate.digest()
            || receipt.checkpoint().authority() != c.selection.minimum.authority()
        {
            return Err(c.error(OperationError::RecoveryRequired));
        }
        *entry
            .receipt
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(*receipt);
        let removed = {
            let mut entries = self
                .envelopes
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            entries
                .iter()
                .position(|v| Shared::ptr_eq(v, entry))
                .map(|i| entries.remove(i))
        };
        drop(removed);
        Ok(*receipt)
    }
    pub(in crate::session) fn retain_delivery(
        &self,
        context: DeliveryContext,
        attempt: Box<StockDeliveryAttempt>,
        result: Result<DeliveryReceipt, SinkFailure>,
        observed: Option<NonZeroU64>,
    ) -> Shared<RetainedDelivery> {
        // No descriptor conversion or storage call precedes registration. Even
        // malformed adapter reports retain the original result and byte lease.
        let entry = Shared::new(RetainedDelivery {
            context,
            attempt,
            result,
            observed,
            complete: Mutex::new(false),
        });
        self.deliveries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(entry.clone());
        entry
    }
    pub(in crate::session) async fn capture_delivery(
        &self,
        entry: &Shared<RetainedDelivery>,
    ) -> crate::Result<()> {
        let _serial = self.serial.lock().await;
        self.append_delivery(entry, false).await
    }
    async fn append_delivery(
        &self,
        entry: &Shared<RetainedDelivery>,
        reconciling: bool,
    ) -> crate::Result<()> {
        let c = &entry.context;
        if *entry
            .complete
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
        {
            return Ok(());
        }
        let request = entry.attempt.transport_request().map_err(|e| c.error(e))?;
        let reports = match &entry.result {
            Ok(receipt) => {
                receipt
                    .validate_for_request(&request)
                    .map_err(|_| c.error(OperationError::InvalidEvidence))?;
                receipt.target_receipts()
            }
            Err(failure) => {
                failure
                    .validate_for_request(&request)
                    .map_err(|_| c.error(OperationError::InvalidEvidence))?;
                failure.partial_evidence()
            }
        };
        // Bound before allocating normalized copies and validate the entire
        // original result before the first write. Empty partial evidence stays
        // empty: a missing ACK never manufactures a target observation.
        let intent = entry.attempt.intent();
        if reports.len() > intent.targets().len()
            || reports.iter().any(|report| {
                report.was_attempted()
                    && !intent.selection().contains(report.target().fingerprint())
            })
        {
            return Err(c.error(OperationError::InvalidEvidence));
        }
        let observations = reports
            .iter()
            .filter(|report| intent.selection().contains(report.target().fingerprint()))
            .map(|report| {
                StockDeliveryObservation::from_store_report(&entry.attempt, report, entry.observed)
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| c.error(e))?;
        for observation in &observations {
            let result = if reconciling {
                c.store
                    .reconcile_stock_delivery_observation(
                        &c.selection.binding,
                        &c.proof,
                        observation,
                    )
                    .await
            } else {
                c.store
                    .record_stock_delivery_observation(&c.selection.binding, &c.proof, observation)
                    .await
            }
            .map_err(|e| c.error(e))?;
            match result {
                StockDeliveryObservationOutcome::Committed(receipt)
                    if receipt.observation() == observation => {}
                _ => return Err(c.error(OperationError::RecoveryRequired)),
            }
        }
        *entry
            .complete
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = true;
        let removed = {
            let mut entries = self
                .deliveries
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            entries
                .iter()
                .position(|v| Shared::ptr_eq(v, entry))
                .map(|i| entries.remove(i))
        };
        drop(removed);
        Ok(())
    }
    pub(super) async fn drain_delivery(&self) -> crate::Result<()> {
        loop {
            let entry = self
                .envelopes
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .first()
                .cloned();
            let Some(entry) = entry else { break };
            self.append_envelope(&entry, true).await?;
        }
        loop {
            let entry = self
                .deliveries
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .first()
                .cloned();
            let Some(entry) = entry else { return Ok(()) };
            self.append_delivery(&entry, true).await?;
        }
    }
}
