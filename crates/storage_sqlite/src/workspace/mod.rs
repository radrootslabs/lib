//! Explicit protected local buyer-outbox attachment.
//!
//! This host cannot acquire seller stock authority, attach a remote endpoint,
//! continue an existing order or restore state from a participant key. Its
//! single connection and persistent writer lock belong to one bounded worker.
//! Losing an observer never drops that lock ahead of the SQLite connection.

mod admission;
mod crypto;
mod fs;
mod operation;
mod plan;
mod record;
mod worker;

#[cfg(test)]
mod tests;

pub use crypto::WorkspaceCrypto;
pub use radroots_storage::workspace::{
    AttachmentInput, AttachmentProof, WorkspaceBinding, WorkspaceClock, WorkspaceError,
};

use radroots_domain::{
    ActorRef, ActorRole, AuthorityRef, CheckpointDigest, CheckpointRef, Coverage, ExecutorId,
    Generation, GrantId, PublicKey, RouteDigest, Sequence, UnixSeconds, WorkspaceId,
};
use radroots_storage::workspace::WorkspaceStore;
use radroots_transport::BoxFuture;
use sha2::{Digest, Sha256};
use std::{path::Path, sync::Arc};

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
        principal: impl Into<String>,
        expires_at: UnixSeconds,
    ) -> Result<Self, WorkspaceError> {
        let principal = principal.into();
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
            principal,
            expires_at,
        })
    }
}
impl std::fmt::Debug for LocalOutboxConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("LocalOutboxConfig(<redacted>)")
    }
}

/// One native local-outbox connection, with an owned bounded request worker.
/// Clones refer to the same admission, writer authority and close completion.
#[derive(Clone)]
pub struct NativeWorkspace {
    worker: Arc<worker::Handle>,
}
impl NativeWorkspace {
    /// Creates a new owner-only namespace and persisted protected grant. All
    /// workspace/executor/grant IDs and the returned credential use OS entropy.
    /// The caller must retain both returned values through protected handling.
    pub async fn initialize_local_outbox(
        path: &Path,
        config: LocalOutboxConfig,
        crypto: WorkspaceCrypto,
        clock: Arc<dyn WorkspaceClock>,
    ) -> Result<(Self, AttachmentInput, AttachmentProof), WorkspaceError> {
        let now = clock.now()?;
        if now.get() == 0 {
            return Err(WorkspaceError::Unavailable);
        }
        if config.expires_at <= now {
            return Err(WorkspaceError::GrantExpired);
        }
        let workspace = WorkspaceId::from_bytes(random()?);
        let executor = ExecutorId::from_bytes(random()?);
        let grant = GrantId::from_bytes(random()?);
        let authority = AuthorityRef::new(
            executor,
            Generation::new(1).map_err(|_| WorkspaceError::InvalidInput)?,
        );
        let endpoint = format!("local://workspace/{workspace}");
        let mut route = Sha256::new();
        route.update(b"radroots.local_buyer_outbox.route.v1\0");
        route.update(workspace.as_bytes());
        route.update(executor.as_bytes());
        route.update(config.endpoint_identity.as_bytes());
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
            config.participant,
            endpoint,
            config.endpoint_identity,
            grant,
            checkpoint,
            route,
        )?;
        let secret = zeroize::Zeroizing::new(random()?);
        let proof = AttachmentProof::from_protected_input(secret.as_ref())?;
        let actor = ActorRef::new(config.participant, ActorRole::Buyer, None, Some(grant));
        let record = record::GrantRecord::new(
            input.clone(),
            actor,
            config.principal,
            config.expires_at,
            &proof,
        )?;
        let worker = worker::Handle::launch(
            path.to_path_buf(),
            input.clone(),
            Some(record),
            crypto,
            clock,
        )
        .await?;
        Ok((
            Self {
                worker: Arc::new(worker),
            },
            input,
            proof,
        ))
    }
    /// Opens only an existing exact namespace. The independently retained
    /// selection supplies the expected envelope context; no grant is created.
    pub async fn open_existing(
        path: &Path,
        expected: AttachmentInput,
        crypto: WorkspaceCrypto,
        clock: Arc<dyn WorkspaceClock>,
    ) -> Result<Self, WorkspaceError> {
        let worker =
            worker::Handle::launch(path.to_path_buf(), expected, None, crypto, clock).await?;
        Ok(Self {
            worker: Arc::new(worker),
        })
    }
    /// Authenticates and revokes this exact local grant. A cancelled observer
    /// must query its retained identity; it cannot presume the write aborted.
    pub async fn revoke(
        &self,
        input: AttachmentInput,
        proof: &AttachmentProof,
    ) -> Result<(), WorkspaceError> {
        self.worker.revoke(input, proof).await
    }
    /// Copies an explicitly borrowed protected credential into one owned
    /// bounded authentication request. The caller retains its original input.
    pub async fn authenticate(
        &self,
        input: AttachmentInput,
        proof: &AttachmentProof,
    ) -> Result<WorkspaceBinding, WorkspaceError> {
        let proof = proof.with_secret(|secret| AttachmentProof::from_protected_input(secret))?;
        self.worker.authenticate(input, proof).await
    }
    /// Permanently closes admission, drains admitted requests and explicitly
    /// closes SQLite before releasing the persistent writer authority.
    pub async fn close(&self) -> Result<(), WorkspaceError> {
        self.worker.close().await
    }
}
impl std::fmt::Debug for NativeWorkspace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("NativeWorkspace(<redacted>)")
    }
}
impl WorkspaceStore for NativeWorkspace {
    fn authenticate(
        &self,
        input: AttachmentInput,
        proof: AttachmentProof,
    ) -> BoxFuture<'_, Result<WorkspaceBinding, WorkspaceError>> {
        Box::pin(self.worker.authenticate(input, proof))
    }
    fn close_workspace(&self) -> BoxFuture<'_, Result<(), WorkspaceError>> {
        Box::pin(self.close())
    }
}
impl radroots_storage::command::OperationJournal for NativeWorkspace {
    fn bind_signing_intents(
        &self,
        binding: WorkspaceBinding,
        plan: radroots_storage::command::ChatSigningPlan,
        proof: AttachmentProof,
    ) -> BoxFuture<
        '_,
        Result<
            radroots_storage::command::PlanBindingOutcome,
            radroots_storage::command::OperationError,
        >,
    > {
        Box::pin(self.worker.bind_plan(binding, plan, proof))
    }
    fn signing_plan(
        &self,
        binding: WorkspaceBinding,
        operation_id: radroots_domain::OperationId,
        digest: radroots_domain::CommandDigest,
        proof: AttachmentProof,
    ) -> BoxFuture<
        '_,
        Result<
            Option<radroots_storage::command::BoundSigningPlan>,
            radroots_storage::command::OperationError,
        >,
    > {
        Box::pin(
            self.worker
                .signing_plan(binding, operation_id, digest, proof),
        )
    }
    fn admit_command_and_hold(
        &self,
        request: radroots_storage::command::AdmissionRequest,
        proof: AttachmentProof,
    ) -> BoxFuture<
        '_,
        Result<
            radroots_storage::command::AdmissionOutcome,
            radroots_storage::command::OperationError,
        >,
    > {
        Box::pin(self.worker.admit(request, proof))
    }
    fn operation(
        &self,
        binding: WorkspaceBinding,
        operation_id: radroots_domain::OperationId,
        digest: Option<radroots_domain::CommandDigest>,
        proof: AttachmentProof,
    ) -> BoxFuture<
        '_,
        Result<
            Option<radroots_storage::command::PreparedOperation>,
            radroots_storage::command::OperationError,
        >,
    > {
        Box::pin(self.worker.operation(binding, operation_id, digest, proof))
    }
}
fn random() -> Result<[u8; 32], WorkspaceError> {
    let mut value = [0; 32];
    getrandom::getrandom(&mut value).map_err(|_| WorkspaceError::Unavailable)?;
    Ok(value)
}
