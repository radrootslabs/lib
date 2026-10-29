//! Retention of original obtained stock material across uncertain persistence.
use super::{HostBound, Shared};
use crate::{Error, OperationContext};
use radroots_storage::{
    command::OperationError,
    inventory::{InventoryStore, StockEvidenceOutcome, StockEvidenceReceipt, StockEvidenceRequest},
    workspace::AttachmentProof,
};
use std::{sync::Mutex, vec::Vec};
#[path = "stock_obtained.rs"]
mod obtained;
pub(super) use obtained::{ObtainedContext, RetainedObtained};
#[path = "stock_unissued.rs"]
mod unissued;
pub(super) use unissued::RetainedUnissued;
#[path = "stock_delivery.rs"]
mod delivery;
pub(super) use delivery::{DeliveryContext, RetainedDelivery, RetainedEnvelope};

pub(super) struct Pending {
    store: Shared<dyn InventoryStore>,
    request: StockEvidenceRequest,
    proof: AttachmentProof,
    receipt: Mutex<Option<StockEvidenceReceipt>>,
    _lease: Shared<dyn HostBound>,
}
#[derive(Default)]
pub(super) struct Owner {
    unmapped: Mutex<Vec<Shared<RetainedObtained>>>,
    pending: Mutex<Vec<Shared<Pending>>>,
    unissued: Mutex<Vec<Shared<RetainedUnissued>>>,
    envelopes: Mutex<Vec<Shared<RetainedEnvelope>>>,
    deliveries: Mutex<Vec<Shared<RetainedDelivery>>>,
    pub(super) delivery_serial: futures::lock::Mutex<()>,
    serial: futures::lock::Mutex<()>,
}
impl Owner {
    pub(super) fn ensure_ready(&self) -> Result<(), OperationError> {
        // Hold both views together: conversion inserts pending before removing
        // unmapped material, so a split observation must not miss that transfer.
        let pending = self
            .pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let unmapped = self
            .unmapped
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let unissued = self
            .unissued
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let envelopes = self
            .envelopes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let deliveries = self
            .deliveries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if pending.is_empty()
            && unmapped.is_empty()
            && unissued.is_empty()
            && envelopes.is_empty()
            && deliveries.is_empty()
        {
            Ok(())
        } else {
            Err(OperationError::RecoveryRequired)
        }
    }
    pub(super) fn retain(
        &self,
        store: Shared<dyn InventoryStore>,
        request: StockEvidenceRequest,
        proof: AttachmentProof,
        lease: Shared<dyn HostBound>,
    ) -> Shared<Pending> {
        let entry = Shared::new(Pending {
            store,
            request,
            proof,
            receipt: Mutex::new(None),
            _lease: lease,
        });
        self.pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(entry.clone());
        entry
    }
    pub(super) async fn capture(
        &self,
        entry: &Shared<Pending>,
    ) -> crate::Result<StockEvidenceOutcome> {
        let _serial = self.serial.lock().await;
        self.append(entry, false).await
    }
    async fn append(
        &self,
        entry: &Shared<Pending>,
        reconciling: bool,
    ) -> crate::Result<StockEvidenceOutcome> {
        let error = |reason| {
            Error::capture_pending(
                reason,
                OperationContext::proposed(Some(entry.request.operation_id())),
            )
        };
        let committed = *entry
            .receipt
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(receipt) = committed {
            return Ok(StockEvidenceOutcome::Committed(receipt));
        }
        let outcome = if reconciling {
            entry
                .store
                .reconcile_stock_evidence(&entry.request, &entry.proof)
                .await
        } else {
            entry
                .store
                .capture_stock_evidence(&entry.request, &entry.proof)
                .await
        }
        .map_err(error)?;
        match outcome {
            StockEvidenceOutcome::Committed(receipt) => {
                let anchor = receipt.attempt();
                let original = entry.request.attempt();
                // Host reports cannot redirect ownership to a different original.
                // Actual authentication and durable append remain host obligations.
                if anchor.workspace_id() != original.workspace_id()
                    || anchor.operation_id() != original.operation_id()
                    || anchor.command_digest() != original.command_digest()
                    || anchor.transaction_id().is_none()
                    || receipt.signing_attempt_id() != entry.request.signing_attempt_id()
                    || receipt.recipient() != entry.request.recipient()
                    || receipt.digest() != &entry.request.evidence_digest()
                {
                    return Err(error(OperationError::RecoveryRequired));
                }
                *entry
                    .receipt
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(receipt);
                let removed = {
                    let mut pending = self
                        .pending
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    pending
                        .iter()
                        .position(|value| Shared::ptr_eq(value, entry))
                        .map(|index| pending.remove(index))
                };
                drop(removed);
                Ok(StockEvidenceOutcome::Committed(receipt))
            }
            StockEvidenceOutcome::Unknown(anchor) => {
                let original = entry.request.attempt();
                if anchor.workspace_id() != original.workspace_id()
                    || anchor.operation_id() != original.operation_id()
                    || anchor.command_digest() != original.command_digest()
                {
                    return Err(error(OperationError::RecoveryRequired));
                }
                // Retain exact bytes, original credentials and the resource/byte
                // lease. Neither an unknown result nor an error releases them.
                Ok(StockEvidenceOutcome::Unknown(anchor))
            }
        }
    }
    pub(super) async fn drain(&self) -> crate::Result<()> {
        let _serial = self.serial.lock().await;
        loop {
            let entry = self
                .unissued
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .first()
                .cloned();
            let Some(entry) = entry else { break };
            if matches!(
                self.append_unissued(&entry, true).await?,
                crate::session::commerce::AcceptanceUnissuedOutcome::Unknown(_)
            ) {
                return Err(Error::capture_pending(
                    OperationError::RecoveryRequired,
                    OperationContext::retained(entry.operation()),
                ));
            }
        }
        loop {
            let entry = self
                .unmapped
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .first()
                .cloned();
            let Some(entry) = entry else { break };
            self.map_obtained(&entry)?;
        }
        loop {
            let entry = self
                .pending
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .first()
                .cloned();
            let Some(entry) = entry else {
                return self.drain_delivery().await;
            };
            if let StockEvidenceOutcome::Unknown(_) = self.append(&entry, true).await? {
                return Err(Error::operation(
                    OperationError::RecoveryRequired,
                    OperationContext::proposed(Some(entry.request.operation_id())),
                ));
            }
        }
    }
}
