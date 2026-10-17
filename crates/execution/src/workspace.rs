//! Authenticated attachment and immutable captured command context.
//!
//! Descriptors and observed signer keys cannot construct these capabilities.
//! Current grant verification remains mandatory at authoritative admission;
//! attachment and context construction are not durable command submission.

use crate::session::host::Shared;
use radroots_domain::{
    Approval, CommandContextData, CorrelationId, ExpectedOrder, OperationId, UnixSeconds,
};
use radroots_storage::workspace::{
    AttachmentInput, AttachmentProof, WorkspaceBinding, WorkspaceError, WorkspaceStore,
};
use std::{
    fmt,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

struct Attached {
    store: Shared<dyn WorkspaceStore>,
    proof: Mutex<Option<AttachmentProof>>,
    binding: WorkspaceBinding,
    closed: AtomicBool,
}
impl Attached {
    async fn current(&self) -> Result<WorkspaceBinding, WorkspaceError> {
        if self.closed.load(Ordering::Acquire) {
            return Err(WorkspaceError::Closed);
        }
        let proof = self
            .proof
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .ok_or(WorkspaceError::Closed)?
            .with_secret(|secret| AttachmentProof::from_protected_input(secret))?;
        if self.closed.load(Ordering::Acquire) {
            return Err(WorkspaceError::Closed);
        }
        let current = self
            .store
            .authenticate(self.binding.input().clone(), proof)
            .await?;
        if self.closed.load(Ordering::Acquire) {
            return Err(WorkspaceError::Closed);
        }
        if current.input() != self.binding.input()
            || current.actor() != self.binding.actor()
            || current.principal_id() != self.binding.principal_id()
            || current.grant_revision() != self.binding.grant_revision()
            || current.expires_at() != self.binding.expires_at()
        {
            return Err(WorkspaceError::GrantRevoked);
        }
        Ok(current)
    }
}

/// An owned authenticated attachment to the explicitly configured workspace.
/// Clones retain one captured grant and permanently shared close gate.
#[derive(Clone)]
pub struct WorkspaceSession {
    attached: Shared<Attached>,
}
impl WorkspaceSession {
    /// Authenticates the protected credential through a qualified host adapter.
    /// Supplying a custom adapter is an explicit host trust boundary; metadata
    /// returned by an unqualified adapter cannot qualify native SQL authority.
    pub async fn attach(
        store: Shared<dyn WorkspaceStore>,
        input: AttachmentInput,
        proof: AttachmentProof,
    ) -> crate::Result<Self> {
        let request_proof = proof
            .with_secret(|secret| AttachmentProof::from_protected_input(secret))
            .map_err(|error| crate::Error::workspace(error, None))?;
        let binding = store
            .authenticate(input.clone(), request_proof)
            .await
            .map_err(|error| crate::Error::workspace(error, None))?;
        if binding.input() != &input {
            return Err(crate::Error::workspace(WorkspaceError::Unauthorized, None));
        }
        Ok(Self {
            attached: Shared::new(Attached {
                store,
                proof: Mutex::new(Some(proof)),
                binding,
                closed: AtomicBool::new(false),
            }),
        })
    }
    /// Borrows the captured grant observation, without claiming it is current.
    pub fn binding(&self) -> &WorkspaceBinding {
        &self.attached.binding
    }
    /// Purely captures the complete context from the authenticated attachment.
    /// This performs no clock, storage, signer or network operation. Current
    /// rights and deadline must be revalidated at authoritative admission.
    /// Approval remains a descriptor; authoritative commands must additionally
    /// verify its protected consent origin and exact command intent.
    pub fn command_context(
        &self,
        operation_id: OperationId,
        expected: Option<ExpectedOrder>,
        approval: Option<Approval>,
        not_after: UnixSeconds,
    ) -> crate::Result<CommandContext> {
        self.command_context_with_correlation(operation_id, expected, approval, not_after, None)
    }
    /// Captures an optional bounded diagnostic correlation value. Correlation
    /// supplies neither authorization nor semantic idempotency.
    pub fn command_context_with_correlation(
        &self,
        operation_id: OperationId,
        expected: Option<ExpectedOrder>,
        approval: Option<Approval>,
        not_after: UnixSeconds,
        correlation: Option<CorrelationId>,
    ) -> crate::Result<CommandContext> {
        let failure = |error| crate::Error::workspace(error, Some(operation_id));
        if self.attached.closed.load(Ordering::Acquire) {
            return Err(failure(WorkspaceError::Closed));
        }
        let current = &self.attached.binding;
        let input = current.input();
        let data = CommandContextData::new(
            operation_id,
            current.actor(),
            input.workspace_id(),
            input.authority(),
            input.route_digest(),
            approval,
            expected,
            not_after,
            correlation,
        )
        .map_err(|_| failure(WorkspaceError::InvalidInput))?;
        if not_after <= current.observed_at() || not_after > current.expires_at() {
            return Err(failure(WorkspaceError::GrantExpired));
        }
        Ok(CommandContext {
            data,
            attached: self.attached.clone(),
        })
    }
    /// Permanently closes this attachment and every context it has issued.
    /// Releases its credential root; owned in-flight requests retain only their
    /// own zeroizing input until the adapter's safe completion boundary.
    /// It does not revoke the persisted grant or close other workspace sessions.
    pub fn close(&self) {
        self.attached.closed.store(true, Ordering::Release);
        self.attached
            .proof
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
    }
    pub(crate) fn is_closed(&self) -> bool {
        self.attached.closed.load(Ordering::Acquire)
    }
}
impl fmt::Debug for WorkspaceSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("WorkspaceSession(<redacted>)")
    }
}

/// Authenticated captured context. There is no public field constructor or
/// deserializer from the corresponding untrusted Domain DTO.
///
/// ```compile_fail
/// use radroots_domain::CommandContextData;
/// use radroots_execution::workspace::CommandContext;
/// fn trust_unchecked(data: CommandContextData) -> CommandContext { data }
/// ```
pub struct CommandContext {
    data: CommandContextData,
    attached: Shared<Attached>,
}
impl CommandContext {
    /// Borrows the exact captured descriptor without transferring authority.
    pub const fn data(&self) -> &CommandContextData {
        &self.data
    }
    /// Revalidates current grant and deadline before an executor prepares its
    /// authoritative transaction. The transaction itself must check them again;
    /// this observation is not T0, a retained operation or stock admission.
    pub async fn revalidate(&self) -> crate::Result<WorkspaceBinding> {
        let failure = |error| crate::Error::workspace(error, Some(self.data.operation_id()));
        let current = self.attached.current().await.map_err(failure)?;
        if self.data.not_after() <= current.observed_at() {
            return Err(failure(WorkspaceError::GrantExpired));
        }
        Ok(current)
    }
}
impl fmt::Debug for CommandContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CommandContext(<redacted>)")
    }
}
