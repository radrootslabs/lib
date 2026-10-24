//! Explicit pure enrollment policy for a fresh seller stock authority.
//! No database, runtime, signer or provider is acquired here.
use super::{
    AttachmentInput, AttachmentProof, WorkspaceError, WorkspaceScope, grant::WorkspaceGrant,
};
use radroots_domain::{
    ActorRef, AuthorityRef, CheckpointDigest, CheckpointRef, Coverage, ExecutorId, Generation,
    GrantId, PublicKey, RouteDigest, Sequence, UnixSeconds, WorkspaceId,
};
use sha2::{Digest, Sha256};
/// Explicit local enrollment inputs, supplied by a trusted host interaction.
/// Only a qualified stock host may persist the resulting authority.
pub struct SellerStockConfig {
    scope: WorkspaceScope,
    participant: PublicKey,
    endpoint_identity: PublicKey,
    principal: String,
    expires_at: UnixSeconds,
}
impl SellerStockConfig {
    /// Selects fresh seller stock with participant, local application identity,
    /// device audit label and positive expiry. This performs no authentication
    /// or filesystem work. It never recovers existing obligations.
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
            scope: WorkspaceScope::SellerStockOwner,
            participant,
            endpoint_identity,
            principal: principal.to_owned(),
            expires_at,
        })
    }
}
impl std::fmt::Debug for SellerStockConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SellerStockConfig(<redacted>)")
    }
}

impl SellerStockConfig {
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
    ) -> Result<(AttachmentInput, AttachmentProof, WorkspaceGrant), WorkspaceError> {
        self.validate_at(now)?;
        let authority = AuthorityRef::new(
            executor,
            Generation::new(1).map_err(|_| WorkspaceError::InvalidInput)?,
        );
        let endpoint = format!("local://seller-stock/{workspace}");
        let mut route = Sha256::new();
        route.update(self.scope.route_domain());
        route.update(workspace.as_bytes());
        route.update(executor.as_bytes());
        route.update(self.endpoint_identity.as_bytes());
        route.update(endpoint.as_bytes());
        let route = RouteDigest::from_bytes(route.finalize().into());
        let mut checkpoint = Sha256::new();
        checkpoint.update(self.scope.genesis_domain());
        checkpoint.update(workspace.as_bytes());
        checkpoint.update(executor.as_bytes());
        checkpoint.update(grant.as_bytes());
        // Genesis certifies only a fresh unrelated stock namespace. Partial coverage is
        // deliberate: no old order, private history or inventory has been recovered.
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
        let actor = ActorRef::new(self.participant, self.scope.actor_role(), None, Some(grant));
        let record = WorkspaceGrant::with_scope(
            input.clone(),
            self.scope,
            actor,
            self.principal,
            self.expires_at,
            &proof,
        )?;
        Ok((input, proof, record))
    }
}
