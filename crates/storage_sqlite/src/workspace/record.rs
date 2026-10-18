use super::{AttachmentInput, AttachmentProof, WorkspaceBinding, WorkspaceError};
use radroots_domain::{ActorRef, ActorRole, Sequence, UnixSeconds};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use zeroize::{Zeroize, Zeroizing};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GrantRecord {
    version: u8,
    scope: Scope,
    input: AttachmentInput,
    actor: ActorRef,
    principal: String,
    revision: Sequence,
    expires_at: UnixSeconds,
    revoked: bool,
    verifier: [u8; 32],
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Scope {
    NewBuyerOutbox,
}
impl Drop for GrantRecord {
    fn drop(&mut self) {
        self.verifier.zeroize();
    }
}
impl GrantRecord {
    pub(super) fn new(
        input: AttachmentInput,
        actor: ActorRef,
        principal: String,
        expires_at: UnixSeconds,
        proof: &AttachmentProof,
    ) -> Result<Self, WorkspaceError> {
        let verifier = verifier(&input, proof);
        let record = Self {
            version: 1,
            scope: Scope::NewBuyerOutbox,
            input,
            actor,
            principal,
            revision: Sequence::new(1).map_err(|_| WorkspaceError::InvalidInput)?,
            expires_at,
            revoked: false,
            verifier,
        };
        record.validate()?;
        Ok(record)
    }
    pub(super) fn decode(bytes: &[u8]) -> Result<Self, WorkspaceError> {
        let record: Self =
            serde_json::from_slice(bytes).map_err(|_| WorkspaceError::RecoveryRequired)?;
        record
            .validate()
            .map_err(|_| WorkspaceError::RecoveryRequired)?;
        Ok(record)
    }
    pub(super) fn encode(&self) -> Result<Zeroizing<Vec<u8>>, WorkspaceError> {
        serde_json::to_vec(self)
            .map(Zeroizing::new)
            .map_err(|_| WorkspaceError::StorageFailure)
    }
    fn validate(&self) -> Result<(), WorkspaceError> {
        if self.version != 1
            || self.actor.role() != ActorRole::Buyer
            || self.actor.organization().is_some()
            || self.actor.participant() != self.input.participant()
            || self.actor.grant_id() != Some(self.input.grant_id())
            || self.principal.is_empty()
            || self.principal.len() > 128
            || self.principal.chars().any(char::is_control)
            || self.revision.get() == 0
            || self.expires_at.get() == 0
        {
            return Err(WorkspaceError::InvalidInput);
        }
        Ok(())
    }
    pub(super) fn authenticate(
        &self,
        input: &AttachmentInput,
        proof: &AttachmentProof,
        now: UnixSeconds,
    ) -> Result<WorkspaceBinding, WorkspaceError> {
        self.authenticate_retained_capture(input, proof)?;
        if self.revoked {
            return Err(WorkspaceError::GrantRevoked);
        }
        if now.get() == 0 {
            return Err(WorkspaceError::Unavailable);
        }
        if self.expires_at <= now {
            return Err(WorkspaceError::GrantExpired);
        }
        self.validate()?;
        WorkspaceBinding::from_authenticated_host(
            self.input.clone(),
            self.actor,
            self.principal.clone(),
            self.revision,
            self.expires_at,
            now,
        )
    }
    /// Checks only the original protected credential and exact namespace scope.
    /// This path is exclusively for appending a matching already obtained
    /// artifact to an actual existing claim, never for new issuance or reads.
    pub(super) fn authenticate_retained_capture(
        &self,
        input: &AttachmentInput,
        proof: &AttachmentProof,
    ) -> Result<(), WorkspaceError> {
        // Check the complete secret commitment before projecting detailed state.
        let observed = Zeroizing::new(verifier(&self.input, proof));
        if !bool::from(observed.as_ref().ct_eq(&self.verifier)) {
            return Err(WorkspaceError::Unauthorized);
        }
        if self.input.workspace_id() != input.workspace_id() {
            return Err(WorkspaceError::WrongNamespace);
        }
        if self.input.authority() != input.authority() {
            return Err(WorkspaceError::StaleAuthority);
        }
        if self.input.checkpoint() != input.checkpoint() {
            return Err(WorkspaceError::StaleCheckpoint);
        }
        if &self.input != input {
            return Err(WorkspaceError::Unauthorized);
        }
        self.validate()
    }
    pub(super) fn revoke(&mut self) -> Result<(), WorkspaceError> {
        self.revoked = true;
        self.revision = Sequence::new(
            self.revision
                .get()
                .checked_add(1)
                .ok_or(WorkspaceError::RecoveryRequired)?,
        )
        .map_err(|_| WorkspaceError::RecoveryRequired)?;
        Ok(())
    }
}
fn verifier(input: &AttachmentInput, proof: &AttachmentProof) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"radroots.local_buyer_outbox.credential.v1\0");
    digest.update(input.workspace_id().as_bytes());
    digest.update(input.grant_id().as_bytes());
    proof.with_secret(|secret| digest.update(secret));
    digest.finalize().into()
}
