//! Original admission credentials survive uncertainty; repair grants no invocation.
use super::delivery::DeliveryContext;
use super::*;
use radroots_domain::OperationId;
use radroots_storage::inventory::{StockEnvelopeReceipt, StockRewrapOutcome, StockRewrapRequest};

pub(in crate::session) struct RetainedAdmission {
    context: DeliveryContext,
    original: StockEnvelopeReceipt,
    id: OperationId,
}
impl Owner {
    pub(in crate::session) fn retain_rewrap_admission(
        &self,
        context: DeliveryContext,
        original: StockEnvelopeReceipt,
        id: OperationId,
    ) -> Shared<RetainedAdmission> {
        let entry = Shared::new(RetainedAdmission {
            context,
            original,
            id,
        });
        self.rewraps
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(entry.clone());
        entry
    }
    pub(in crate::session) async fn prepare_rewrap(
        &self,
        entry: &Shared<RetainedAdmission>,
    ) -> crate::Result<StockRewrapOutcome> {
        let _serial = self.serial.lock().await;
        let c = &entry.context;
        let request = StockRewrapRequest::new(c.request(None).map_err(|e| c.error(e))?, entry.id);
        let result = c
            .store
            .prepare_stock_rewrap(request, c.proof().map_err(|e| c.error(e))?)
            .await
            .map_err(|e| c.error(e))?;
        match &result {
            StockRewrapOutcome::Claimed(admitted) | StockRewrapOutcome::Retained(admitted) => {
                if admitted.rewrap_id() != entry.id
                    || admitted.original() != entry.original
                    || admitted.checkpoint().authority() != c.selection.minimum.authority()
                    || admitted.checkpoint().revision() <= entry.original.checkpoint().revision()
                    || (matches!(&result, StockRewrapOutcome::Claimed(_))
                        && admitted.checkpoint().revision() <= c.selection.minimum.revision())
                {
                    return Err(c.error(OperationError::RecoveryRequired));
                }
                self.finish_rewrap_admission(entry);
            }
            StockRewrapOutcome::Unknown(attempt) => {
                if attempt.workspace_id() != c.selection.binding.input().workspace_id()
                    || attempt.operation_id() != c.selection.operation
                    || attempt.command_digest() != c.selection.digest
                {
                    return Err(c.error(OperationError::RecoveryRequired));
                }
                // Neither an unknown write nor a lost waiter may create keys.
                // Keep original proof, store and task/byte lease until repair.
            }
        }
        Ok(result)
    }
    fn finish_rewrap_admission(&self, entry: &Shared<RetainedAdmission>) {
        let removed = {
            let mut entries = self
                .rewraps
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            entries
                .iter()
                .position(|v| Shared::ptr_eq(v, entry))
                .map(|index| entries.remove(index))
        };
        drop(removed);
    }
    /// Called under the existing capture serial lock. This repairs the writer
    /// fence only; status reads still require current access and consent.
    pub(super) async fn drain_rewrap_admissions(&self) -> crate::Result<()> {
        loop {
            let entry = self
                .rewraps
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .first()
                .cloned();
            let Some(entry) = entry else { return Ok(()) };
            let c = &entry.context;
            c.store
                .reconcile_stock_rewrap_admission(
                    StockRewrapRequest::new(c.request(None).map_err(|e| c.error(e))?, entry.id),
                    c.proof().map_err(|e| c.error(e))?,
                )
                .await
                .map_err(|e| c.error(e))?;
            self.finish_rewrap_admission(&entry);
        }
    }
}
