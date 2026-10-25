//! Shared bounded workspace grant codec; concrete host admission remains separate.
use super::{AttachmentInput, AttachmentProof, WorkspaceBinding, WorkspaceError, WorkspaceScope};
use radroots_domain::{ActorRef, GrantId, Sequence, UnixSeconds, WorkspaceId};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use zeroize::{Zeroize, Zeroizing};
/// Maximum cleartext grant record, checked before decoding.
pub const GRANT_BYTES_MAX: usize = 16 * 1024;
/// Retained protected outbox context. A stock host needs its own qualified context.
pub const GRANT_ENVELOPE_PURPOSE: &str = "radroots.workspace.grant";
pub const GRANT_PAYLOAD_SCHEMA: &str = "radroots.workspace.grant.v1";
pub fn grant_envelope_subject(workspace: WorkspaceId, grant: GrantId) -> String {
    // Both IDs have fixed-width canonical hexadecimal representations.
    format!("{workspace}{grant}")
}
/// Validated shared grant metadata and current credential/authorization rules.
/// Hosts authenticate its envelope before decoding; this value alone is no authority.
/// Raw Serde construction cannot bypass the bounded validating codec.
pub struct WorkspaceGrant {
    wire: GrantWire,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct GrantWire {
    version: u8,
    scope: WorkspaceScope,
    input: AttachmentInput,
    actor: ActorRef,
    principal: String,
    revision: Sequence,
    expires_at: UnixSeconds,
    revoked: bool,
    verifier: [u8; 32],
}
impl Drop for WorkspaceGrant {
    fn drop(&mut self) {
        self.wire.verifier.zeroize();
    }
}
impl WorkspaceGrant {
    pub(super) const fn is_revoked(&self) -> bool {
        self.wire.revoked
    }
    pub(super) fn expires_at(&self) -> UnixSeconds {
        self.wire.expires_at
    }
    pub(super) fn input(&self) -> &AttachmentInput {
        &self.wire.input
    }
    pub fn new(
        input: AttachmentInput,
        actor: ActorRef,
        principal: String,
        expires_at: UnixSeconds,
        proof: &AttachmentProof,
    ) -> Result<Self, WorkspaceError> {
        Self::with_scope(
            input,
            WorkspaceScope::NewBuyerOutbox,
            actor,
            principal,
            expires_at,
            proof,
        )
    }
    pub(super) fn with_scope(
        input: AttachmentInput,
        scope: WorkspaceScope,
        actor: ActorRef,
        principal: String,
        expires_at: UnixSeconds,
        proof: &AttachmentProof,
    ) -> Result<Self, WorkspaceError> {
        let verifier = verifier(&input, scope, proof);
        let record = Self {
            wire: GrantWire {
                version: 1,
                scope,
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
    /// Decodes bounded metadata only, without authenticating an envelope or
    /// qualifying a host's supported scopes. Outbox hosts use `decode_outbox`.
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
    /// Admits only the retained outbox scopes after envelope authentication.
    /// A structurally valid stock-owner claim cannot upgrade an outbox host.
    pub fn decode_outbox(bytes: &[u8]) -> Result<Self, WorkspaceError> {
        let record = Self::decode(bytes)?;
        if record.wire.scope == WorkspaceScope::SellerStockOwner {
            return Err(WorkspaceError::RecoveryRequired);
        }
        Ok(record)
    }
    /// Declared authenticated metadata; the concrete host still qualifies its scope.
    pub const fn scope(&self) -> WorkspaceScope {
        self.wire.scope
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
            || self.wire.actor.role() != self.wire.scope.actor_role()
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
            self.wire.scope,
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
        let observed = Zeroizing::new(verifier(&self.wire.input, self.wire.scope, proof));
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
impl WorkspaceScope {
    pub(super) fn route_domain(self) -> &'static [u8] {
        match self {
            Self::NewBuyerOutbox => b"radroots.local_buyer_outbox.route.v1\0",
            Self::NewSellerOutbox => b"radroots.local_seller_outbox.route.v1\0",
            Self::SellerStockOwner => b"radroots.seller_stock.route.v1\0",
        }
    }
    pub(super) fn genesis_domain(self) -> &'static [u8] {
        match self {
            Self::NewBuyerOutbox => b"radroots.local_buyer_outbox.genesis.v1\0",
            Self::NewSellerOutbox => b"radroots.local_seller_outbox.genesis.v1\0",
            Self::SellerStockOwner => b"radroots.seller_stock.genesis.v1\0",
        }
    }
    fn credential_domain(self) -> &'static [u8] {
        match self {
            Self::NewBuyerOutbox => b"radroots.local_buyer_outbox.credential.v1\0",
            Self::NewSellerOutbox => b"radroots.local_seller_outbox.credential.v1\0",
            Self::SellerStockOwner => b"radroots.seller_stock.credential.v1\0",
        }
    }
}
fn verifier(input: &AttachmentInput, scope: WorkspaceScope, proof: &AttachmentProof) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(scope.credential_domain());
    digest.update(input.workspace_id().as_bytes());
    digest.update(input.grant_id().as_bytes());
    proof.with_secret(|secret| digest.update(secret));
    digest.finalize().into()
}
