use super::WorkspaceError;
use radroots_domain::{GrantId, OperationId, WorkspaceId};
use radroots_secrets::{
    EncryptedEnvelope, SecretRef,
    context::{EnvelopeContext, EnvelopePurpose, EnvelopeSubject, PayloadSchemaId},
    envelope::{ENVELOPE_MAX_BYTES, Nonce, SealMaterial, SealRequest},
    provider::{AccessPolicy, ResidencySupport, SecretProvider, SelectionPolicy},
    wrapping::SecretMaterial,
};
use std::{fmt, sync::Arc};

pub(super) const PRIVATE_PAYLOAD_MAX_BYTES: usize = 16 * 1024;
pub(super) const PROTECTED_MAX_BYTES: usize = ENVELOPE_MAX_BYTES;

/// Explicit host-owned encryption material. This is a data-encryption key,
/// independent of the participant signer and the scoped bearer credential.
/// The host chooses its qualified provider and access policy; no fallback or
/// credential installation occurs here.
pub struct WorkspaceCrypto {
    provider: Arc<dyn SecretProvider>,
    reference: SecretRef,
    key: SecretMaterial,
}
impl WorkspaceCrypto {
    /// Binds protected host input to one exact persistent provider/ref. Actual
    /// provider access and context authentication still execute at seal/open.
    pub fn new(
        provider: Arc<dyn SecretProvider>,
        reference: SecretRef,
        key: SecretMaterial,
        policy: AccessPolicy,
    ) -> Result<Self, WorkspaceError> {
        if key.len() != 32 || provider.capabilities().residency() == ResidencySupport::Volatile {
            return Err(WorkspaceError::Unavailable);
        }
        SelectionPolicy::new(reference.backend(), policy)
            .select(&[provider.as_ref()])
            .map_err(|_| WorkspaceError::Unavailable)?;
        Ok(Self {
            provider,
            reference,
            key,
        })
    }
    pub(super) async fn seal(
        &self,
        workspace: WorkspaceId,
        grant: GrantId,
        plaintext: &[u8],
    ) -> Result<Vec<u8>, WorkspaceError> {
        self.seal_bound(
            context(workspace, grant)?,
            PRIVATE_PAYLOAD_MAX_BYTES,
            plaintext,
        )
        .await
    }
    pub(super) async fn seal_operation(
        &self,
        workspace: WorkspaceId,
        operation: OperationId,
        plaintext: &[u8],
    ) -> Result<Vec<u8>, WorkspaceError> {
        self.seal_bound(operation_context(workspace, operation)?, 65_536, plaintext)
            .await
    }
    async fn seal_bound(
        &self,
        context: EnvelopeContext,
        max_bytes: usize,
        plaintext: &[u8],
    ) -> Result<Vec<u8>, WorkspaceError> {
        if plaintext.len() > max_bytes {
            return Err(WorkspaceError::InvalidInput);
        }
        let plaintext =
            SecretMaterial::from_slice(plaintext).map_err(|_| WorkspaceError::InvalidInput)?;
        let key = self
            .key
            .expose_secret(SecretMaterial::from_slice)
            .map_err(|_| WorkspaceError::Unavailable)?;
        let mut nonce = [0; 24];
        getrandom::getrandom(&mut nonce).map_err(|_| WorkspaceError::Unavailable)?;
        let envelope = EncryptedEnvelope::seal(
            self.provider.as_ref(),
            SealRequest::new(
                SecretRef::new(
                    self.reference.id().clone(),
                    self.reference.backend(),
                    self.reference.key_version(),
                ),
                context,
                &plaintext,
                SealMaterial::new(key, Nonce::new(nonce)),
            ),
        )
        .await
        .map_err(|_| WorkspaceError::Unavailable)?;
        envelope
            .encode()
            .map_err(|_| WorkspaceError::StorageFailure)
    }
    pub(super) async fn open(
        &self,
        workspace: WorkspaceId,
        grant: GrantId,
        protected: &[u8],
    ) -> Result<SecretMaterial, WorkspaceError> {
        self.open_bound(
            context(workspace, grant)?,
            PRIVATE_PAYLOAD_MAX_BYTES,
            protected,
        )
        .await
    }
    pub(super) async fn open_operation(
        &self,
        workspace: WorkspaceId,
        operation: OperationId,
        protected: &[u8],
    ) -> Result<SecretMaterial, WorkspaceError> {
        self.open_bound(operation_context(workspace, operation)?, 65_536, protected)
            .await
    }
    async fn open_bound(
        &self,
        context: EnvelopeContext,
        max_bytes: usize,
        protected: &[u8],
    ) -> Result<SecretMaterial, WorkspaceError> {
        if protected.len() > PROTECTED_MAX_BYTES {
            return Err(WorkspaceError::RecoveryRequired);
        }
        // A wrong explicitly supplied key cannot silently fall back to a
        // different provider-held key, including on a read-only observation.
        self.provider
            .wrap(radroots_secrets::wrapping::WrapRequest::new(
                &self.reference,
                &context,
                &self.key,
            ))
            .await
            .map_err(|_| WorkspaceError::Unavailable)?;
        let envelope =
            EncryptedEnvelope::decode(protected).map_err(|_| WorkspaceError::RecoveryRequired)?;
        if envelope.reference().id() != self.reference.id()
            || envelope.reference().backend() != self.reference.backend()
            || envelope.reference().key_version() != self.reference.key_version()
        {
            return Err(WorkspaceError::RecoveryRequired);
        }
        let plaintext = envelope
            .open(self.provider.as_ref(), &context)
            .await
            .map_err(|_| WorkspaceError::RecoveryRequired)?;
        if plaintext.len() > max_bytes {
            return Err(WorkspaceError::RecoveryRequired);
        }
        Ok(plaintext)
    }
}
impl fmt::Debug for WorkspaceCrypto {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("WorkspaceCrypto(<redacted>)")
    }
}

fn context(workspace: WorkspaceId, grant: GrantId) -> Result<EnvelopeContext, WorkspaceError> {
    // Both values are fixed-width lowercase hex; their 128-byte concatenation
    // is unambiguous and binds the complete IDs without truncation.
    Ok(EnvelopeContext::new(
        EnvelopePurpose::parse("radroots.workspace.grant")
            .map_err(|_| WorkspaceError::InvalidInput)?,
        EnvelopeSubject::parse("workspace_grant", format!("{workspace}{grant}"))
            .map_err(|_| WorkspaceError::InvalidInput)?,
        PayloadSchemaId::parse("radroots.workspace.grant.v1")
            .map_err(|_| WorkspaceError::InvalidInput)?,
    ))
}

fn operation_context(
    workspace: WorkspaceId,
    operation: OperationId,
) -> Result<EnvelopeContext, WorkspaceError> {
    Ok(EnvelopeContext::new(
        EnvelopePurpose::parse("radroots.workspace.operation")
            .map_err(|_| WorkspaceError::InvalidInput)?,
        EnvelopeSubject::parse("workspace_operation", format!("{workspace}{operation}"))
            .map_err(|_| WorkspaceError::InvalidInput)?,
        PayloadSchemaId::parse("radroots.workspace.operation.v1")
            .map_err(|_| WorkspaceError::InvalidInput)?,
    ))
}
