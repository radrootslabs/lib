//! Same-owner connection recovery confers only original obtained-fact retention.
use super::{Database, OpenMode, State, WorkspaceError, namespace, stored};
use radroots_domain::OperationId;
use radroots_storage::command::{
    DeliveryObservation, DeliveryObservationKind, PublicationWork, SealEvidence,
};
use radroots_storage::workspace::{
    AttachmentInput, AttachmentProof, local_outbox::LocalOutboxGrant,
};
use radroots_transport::{sink::DeliveryRequestId, target::TargetFingerprint};

#[derive(Clone, Eq, PartialEq)]
pub(super) enum CaptureIdentity {
    Seal {
        operation: OperationId,
        digest: [u8; 32],
    },
    Observation {
        work: Box<PublicationWork>,
        request: DeliveryRequestId,
        target: TargetFingerprint,
        kind: DeliveryObservationKind,
    },
}
impl CaptureIdentity {
    pub fn seal(value: &SealEvidence) -> Self {
        Self::Seal {
            operation: value.operation_id(),
            digest: *value.digest(),
        }
    }
    pub fn observation(value: &DeliveryObservation) -> Self {
        Self::Observation {
            work: Box::new(value.work()),
            request: value.request_id().clone(),
            target: value.target_receipt().target().fingerprint().clone(),
            kind: value.kind(),
        }
    }
}

pub(super) struct Authorization {
    grant: LocalOutboxGrant,
    close_confirmed: bool,
}
impl State {
    pub(super) fn capture_confirmed(&self, identity: &CaptureIdentity) {
        let matches = self.capture_scope.borrow().as_ref() == Some(identity);
        if matches {
            self.uncertain.set(false);
            self.capture_scope.borrow_mut().take();
            self.recovery.borrow_mut().take();
        }
    }
    pub(super) async fn capture_retained(
        &self,
        input: &AttachmentInput,
        proof: &AttachmentProof,
    ) -> Result<(LocalOutboxGrant, Vec<u8>), WorkspaceError> {
        self.active()?;
        if input != &self.selected {
            return Err(WorkspaceError::Unauthorized);
        }
        if self.uncertain.get() {
            self.reopen_retained(input, proof).await?;
        }
        self.capture_grant(input, proof).await
    }
    async fn reopen_retained(
        &self,
        input: &AttachmentInput,
        proof: &AttachmentProof,
    ) -> Result<(), WorkspaceError> {
        // A live connection must authenticate its actual grant before disposal.
        // After confirmed closure, cached original authorization survives a
        // failed existing-only reopen. No raw proof is retained in this cache.
        if self
            .recovery
            .borrow()
            .as_ref()
            .is_none_or(|value| !value.close_confirmed)
        {
            let (grant, _) = self.capture_grant(input, proof).await?;
            *self.recovery.borrow_mut() = Some(Authorization {
                grant,
                close_confirmed: false,
            });
        }
        {
            let cached = self.recovery.borrow();
            cached
                .as_ref()
                .ok_or(WorkspaceError::RecoveryRequired)?
                .grant
                .authenticate_retained_capture(input, proof)?;
        }
        let closed = self
            .recovery
            .borrow()
            .as_ref()
            .ok_or(WorkspaceError::RecoveryRequired)?
            .close_confirmed;
        if !closed {
            self.database().close().await.map_err(stored)?;
            self.recovery
                .borrow_mut()
                .as_mut()
                .ok_or(WorkspaceError::RecoveryRequired)?
                .close_confirmed = true;
        }
        self.active()?;
        let replacement = Database::open(namespace(&self.selected)?, OpenMode::Existing)
            .await
            .map_err(stored)?;
        self.active()?;
        *self.db.borrow_mut() = replacement;
        self.recovery
            .borrow_mut()
            .as_mut()
            .ok_or(WorkspaceError::RecoveryRequired)?
            .close_confirmed = false;
        Ok(())
    }
}
