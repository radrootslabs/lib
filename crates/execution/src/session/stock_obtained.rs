//! Register obtained bytes before any fallible storage-descriptor conversion.
use super::*;
use crate::session::commerce::{AcceptanceClaim, AcceptanceSelection};
use radroots_domain::PublicKey;
use radroots_signing::issuance::SignedArtifact;

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "stock_obtained_tests.rs"]
mod tests;

pub(in crate::session) struct ObtainedContext {
    pub selection: AcceptanceSelection,
    pub claim: AcceptanceClaim,
    pub recipient: PublicKey,
    pub store: Shared<dyn InventoryStore>,
    pub proof: AttachmentProof,
    pub lease: Shared<dyn HostBound>,
}
pub(in crate::session) struct RetainedObtained {
    context: ObtainedContext,
    artifact: SignedArtifact,
    mapped: Mutex<Option<Shared<Pending>>>,
}
impl Owner {
    pub(in crate::session) fn retain_obtained(
        &self,
        context: ObtainedContext,
        artifact: SignedArtifact,
    ) -> Shared<RetainedObtained> {
        let entry = Shared::new(RetainedObtained {
            context,
            artifact,
            mapped: Mutex::new(None),
        });
        self.unmapped
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(entry.clone());
        entry
    }
    pub(super) fn map_obtained(
        &self,
        entry: &Shared<RetainedObtained>,
    ) -> crate::Result<Shared<Pending>> {
        let mut mapped = entry
            .mapped
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(pending) = mapped.as_ref() {
            return Ok(pending.clone());
        }
        let context = &entry.context;
        let error = |reason| {
            Error::capture_pending(
                reason,
                OperationContext::retained(context.selection.operation),
            )
        };
        let request = entry
            .artifact
            .with_json(|raw| {
                StockEvidenceRequest::from_obtained_wire(
                    context.selection.binding.clone(),
                    context.selection.operation,
                    context.selection.digest,
                    context.claim.id,
                    context.recipient,
                    *context.claim.binding.digest(),
                    raw,
                    entry.artifact.observed_at_unix_ms(),
                )
            })
            .map_err(error)?;
        let proof = context
            .proof
            .with_secret(|bytes| AttachmentProof::from_protected_input(bytes))
            .map_err(|e| error(e.into()))?;
        let pending = self.retain(context.store.clone(), request, proof, context.lease.clone());
        *mapped = Some(pending.clone());
        let removed = {
            let mut raw = self
                .unmapped
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            raw.iter()
                .position(|v| Shared::ptr_eq(v, entry))
                .map(|index| raw.remove(index))
        };
        drop(removed);
        Ok(pending)
    }
    pub(in crate::session) async fn capture_obtained(
        &self,
        entry: &Shared<RetainedObtained>,
    ) -> crate::Result<StockEvidenceOutcome> {
        let _serial = self.serial.lock().await;
        let pending = self.map_obtained(entry)?;
        self.append(&pending, false).await
    }
}
