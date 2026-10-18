//! Shared pure enrollment and protected grant rules for concrete local hosts.
//! No database, runtime, signer or provider is acquired here.
use super::{AttachmentInput, AttachmentProof, WorkspaceBinding, WorkspaceError};
use radroots_domain::{
    ActorRef, ActorRole, AuthorityRef, CheckpointDigest, CheckpointRef, Coverage, ExecutorId,
    Generation, GrantId, PublicKey, RouteDigest, Sequence, UnixSeconds, WorkspaceId,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use zeroize::{Zeroize, Zeroizing};
/// Maximum cleartext grant record, checked before decoding.
pub const GRANT_BYTES_MAX: usize = 16 * 1024;
/// Exact protected grant context, shared by concrete storage hosts.
pub const GRANT_ENVELOPE_PURPOSE: &str = "radroots.workspace.grant";
pub const GRANT_PAYLOAD_SCHEMA: &str = "radroots.workspace.grant.v1";
pub fn grant_envelope_subject(workspace: WorkspaceId, grant: GrantId) -> String {
    // Both IDs have fixed-width canonical hexadecimal representations.
    format!("{workspace}{grant}")
}
/// Explicit local enrollment inputs, supplied by a trusted host interaction.
/// Creation is limited to an unrelated new buyer outbox, never seller stock.
pub struct LocalOutboxConfig {
    participant: PublicKey,
    endpoint_identity: PublicKey,
    principal: String,
    expires_at: UnixSeconds,
}
impl LocalOutboxConfig {
    /// Captures the participant, local application identity, device audit label
    /// and positive expiry. This performs no authentication or filesystem work.
    pub fn new(
        participant: PublicKey,
        endpoint_identity: PublicKey,
        principal: impl AsRef<str>,
        expires_at: UnixSeconds,
    ) -> Result<Self, WorkspaceError> {
        let principal = principal.as_ref();
        if principal.is_empty()
            || principal.len() > 128
            || principal.chars().any(char::is_control)
            || expires_at.get() == 0
        {
            return Err(WorkspaceError::InvalidInput);
        }
        Ok(Self {
            participant,
            endpoint_identity,
            principal: principal.to_owned(),
            expires_at,
        })
    }
}
impl std::fmt::Debug for LocalOutboxConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("LocalOutboxConfig(<redacted>)")
    }
}

impl LocalOutboxConfig {
    /// Checks enrollment time without acquiring entropy or a host resource.
    pub fn validate_at(&self, now: UnixSeconds) -> Result<(), WorkspaceError> {
        if now.get() == 0 {
            return Err(WorkspaceError::Unavailable);
        }
        if self.expires_at <= now {
            return Err(WorkspaceError::GrantExpired);
        }
        Ok(())
    }
    /// Pure trusted-host enrollment. Concrete hosts supply qualified fresh IDs
    /// and a protected proof; this does not acquire or persist any authority.
    pub fn enroll(
        self,
        now: UnixSeconds,
        workspace: WorkspaceId,
        executor: ExecutorId,
        grant: GrantId,
        proof: AttachmentProof,
    ) -> Result<(AttachmentInput, AttachmentProof, LocalOutboxGrant), WorkspaceError> {
        self.validate_at(now)?;
        let authority = AuthorityRef::new(
            executor,
            Generation::new(1).map_err(|_| WorkspaceError::InvalidInput)?,
        );
        let endpoint = format!("local://workspace/{workspace}");
        let mut route = Sha256::new();
        route.update(b"radroots.local_buyer_outbox.route.v1\0");
        route.update(workspace.as_bytes());
        route.update(executor.as_bytes());
        route.update(self.endpoint_identity.as_bytes());
        route.update(endpoint.as_bytes());
        let route = RouteDigest::from_bytes(route.finalize().into());
        let mut checkpoint = Sha256::new();
        checkpoint.update(b"radroots.local_buyer_outbox.genesis.v1\0");
        checkpoint.update(workspace.as_bytes());
        checkpoint.update(executor.as_bytes());
        checkpoint.update(grant.as_bytes());
        // Genesis certifies only a fresh unrelated outbox. Partial coverage is
        // deliberate: no old order, private history or stock has been recovered.
        let checkpoint = CheckpointRef::new(
            CheckpointDigest::from_bytes(checkpoint.finalize().into()),
            Sequence::new(0).map_err(|_| WorkspaceError::InvalidInput)?,
            Coverage::Partial,
            vec![],
        )
        .map_err(|_| WorkspaceError::InvalidInput)?;
        let input = AttachmentInput::new(
            workspace,
            authority,
            self.participant,
            endpoint,
            self.endpoint_identity,
            grant,
            checkpoint,
            route,
        )?;
        let actor = ActorRef::new(self.participant, ActorRole::Buyer, None, Some(grant));
        let record = LocalOutboxGrant::new(
            input.clone(),
            actor,
            self.principal,
            self.expires_at,
            &proof,
        )?;
        Ok((input, proof, record))
    }
}
/// Validated protected local-outbox grant codec and current authorization rules.
/// Hosts authenticate its envelope before decoding; this value alone is no authority.
/// Raw Serde construction cannot bypass the bounded validating codec.
pub struct LocalOutboxGrant {
    wire: GrantWire,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct GrantWire {
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
impl Drop for LocalOutboxGrant {
    fn drop(&mut self) {
        self.wire.verifier.zeroize();
    }
}
impl LocalOutboxGrant {
    pub fn new(
        input: AttachmentInput,
        actor: ActorRef,
        principal: String,
        expires_at: UnixSeconds,
        proof: &AttachmentProof,
    ) -> Result<Self, WorkspaceError> {
        let verifier = verifier(&input, proof);
        let record = Self {
            wire: GrantWire {
                version: 1,
                scope: Scope::NewBuyerOutbox,
                input,
                actor,
                principal,
                revision: Sequence::new(1).map_err(|_| WorkspaceError::InvalidInput)?,
                expires_at,
                revoked: false,
                verifier,
            },
        };
        record.validate()?;
        Ok(record)
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, WorkspaceError> {
        if bytes.len() > GRANT_BYTES_MAX {
            return Err(WorkspaceError::RecoveryRequired);
        }
        let record = Self {
            wire: serde_json::from_slice(bytes).map_err(|_| WorkspaceError::RecoveryRequired)?,
        };
        record
            .validate()
            .map_err(|_| WorkspaceError::RecoveryRequired)?;
        Ok(record)
    }
    pub fn encode(&self) -> Result<Zeroizing<Vec<u8>>, WorkspaceError> {
        self.validate()?;
        let bytes = Zeroizing::new(
            serde_json::to_vec(&self.wire).map_err(|_| WorkspaceError::StorageFailure)?,
        );
        if bytes.len() > GRANT_BYTES_MAX {
            return Err(WorkspaceError::InvalidInput);
        }
        Ok(bytes)
    }
    fn validate(&self) -> Result<(), WorkspaceError> {
        if self.wire.version != 1
            || self.wire.actor.role() != ActorRole::Buyer
            || self.wire.actor.organization().is_some()
            || self.wire.actor.participant() != self.wire.input.participant()
            || self.wire.actor.grant_id() != Some(self.wire.input.grant_id())
            || self.wire.principal.is_empty()
            || self.wire.principal.len() > 128
            || self.wire.principal.chars().any(char::is_control)
            || self.wire.revision.get() == 0
            || self.wire.expires_at.get() == 0
        {
            return Err(WorkspaceError::InvalidInput);
        }
        Ok(())
    }
    pub fn authenticate(
        &self,
        input: &AttachmentInput,
        proof: &AttachmentProof,
        now: UnixSeconds,
    ) -> Result<WorkspaceBinding, WorkspaceError> {
        self.authenticate_retained_capture(input, proof)?;
        if self.wire.revoked {
            return Err(WorkspaceError::GrantRevoked);
        }
        if now.get() == 0 {
            return Err(WorkspaceError::Unavailable);
        }
        if self.wire.expires_at <= now {
            return Err(WorkspaceError::GrantExpired);
        }
        self.validate()?;
        WorkspaceBinding::from_authenticated_host(
            self.wire.input.clone(),
            self.wire.actor,
            self.wire.principal.clone(),
            self.wire.revision,
            self.wire.expires_at,
            now,
        )
    }
    /// Checks only the original protected credential and exact namespace scope.
    /// This path is exclusively for appending a matching already obtained
    /// artifact to an actual existing claim, never for new issuance or reads.
    pub fn authenticate_retained_capture(
        &self,
        input: &AttachmentInput,
        proof: &AttachmentProof,
    ) -> Result<(), WorkspaceError> {
        // Check the complete secret commitment before projecting detailed state.
        let observed = Zeroizing::new(verifier(&self.wire.input, proof));
        if !bool::from(observed.as_ref().ct_eq(&self.wire.verifier)) {
            return Err(WorkspaceError::Unauthorized);
        }
        if self.wire.input.workspace_id() != input.workspace_id() {
            return Err(WorkspaceError::WrongNamespace);
        }
        if self.wire.input.authority() != input.authority() {
            return Err(WorkspaceError::StaleAuthority);
        }
        if self.wire.input.checkpoint() != input.checkpoint() {
            return Err(WorkspaceError::StaleCheckpoint);
        }
        if &self.wire.input != input {
            return Err(WorkspaceError::Unauthorized);
        }
        self.validate()
    }
    pub fn revoke(&mut self) -> Result<(), WorkspaceError> {
        self.wire.revoked = true;
        self.wire.revision = Sequence::new(
            self.wire
                .revision
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
