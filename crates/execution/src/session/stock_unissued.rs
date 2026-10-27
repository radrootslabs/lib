//! Retain original invocation material before any fallible capture or await.
use super::*;
use crate::session::commerce::{
    AcceptanceClaim, AcceptanceSelection, AcceptanceUnissued, AcceptanceUnissuedOutcome,
    AcceptanceUnissuedReceipt,
};
use alloc::boxed::Box;
use radroots_domain::PublicKey;

pub(in crate::session) struct RetainedUnissued {
    selection: AcceptanceSelection,
    claim: AcceptanceClaim,
    recipient: PublicKey,
    observation: Box<dyn AcceptanceUnissued>,
    receipt: Mutex<Option<AcceptanceUnissuedReceipt>>,
    _lease: Shared<dyn HostBound>,
}
impl RetainedUnissued {
    pub(super) fn operation(&self) -> radroots_domain::OperationId {
        self.selection.operation
    }
}
impl Owner {
    pub(in crate::session) fn retain_unissued(
        &self,
        selection: AcceptanceSelection,
        claim: AcceptanceClaim,
        recipient: PublicKey,
        observation: Box<dyn AcceptanceUnissued>,
        lease: Shared<dyn HostBound>,
    ) -> Shared<RetainedUnissued> {
        let entry = Shared::new(RetainedUnissued {
            selection,
            claim,
            recipient,
            observation,
            receipt: Mutex::new(None),
            _lease: lease,
        });
        self.unissued
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(entry.clone());
        entry
    }
    pub(in crate::session) async fn capture_unissued(
        &self,
        entry: &Shared<RetainedUnissued>,
    ) -> crate::Result<AcceptanceUnissuedOutcome> {
        let _serial = self.serial.lock().await;
        self.append_unissued(entry, false).await
    }
    pub(super) async fn append_unissued(
        &self,
        entry: &Shared<RetainedUnissued>,
        reconciling: bool,
    ) -> crate::Result<AcceptanceUnissuedOutcome> {
        let error = |reason| {
            Error::capture_pending(
                reason,
                OperationContext::retained(entry.selection.operation),
            )
        };
        let committed = *entry
            .receipt
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(receipt) = committed {
            return Ok(AcceptanceUnissuedOutcome::Committed(Box::new(receipt)));
        }
        // A malformed report must never cause material to leave this owner.
        if entry.observation.claim() != entry.claim {
            return Err(error(OperationError::RecoveryRequired));
        }
        let outcome = if reconciling {
            entry.observation.reconcile().await
        } else {
            entry.observation.capture().await
        }
        .map_err(error)?;
        let attempt = match &outcome {
            AcceptanceUnissuedOutcome::Committed(value) => value.attempt,
            AcceptanceUnissuedOutcome::Unknown(value) => *value,
        };
        if attempt.workspace_id() != entry.selection.binding.input().workspace_id()
            || attempt.operation_id() != entry.selection.operation
            || attempt.command_digest() != entry.selection.digest
        {
            return Err(error(OperationError::RecoveryRequired));
        }
        if let AcceptanceUnissuedOutcome::Committed(value) = &outcome {
            if attempt.transaction_id().is_none()
                || value.claim != entry.claim.id
                || value.recipient != entry.recipient
                || value.reason != entry.observation.reason()
                || value.checkpoint.authority() != entry.selection.minimum.authority()
            {
                return Err(error(OperationError::RecoveryRequired));
            }
            *entry
                .receipt
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(**value);
            let removed = {
                let mut pending = self
                    .unissued
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                pending
                    .iter()
                    .position(|v| Shared::ptr_eq(v, entry))
                    .map(|i| pending.remove(i))
            };
            drop(removed);
        }
        Ok(outcome)
    }
}
