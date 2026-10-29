use super::WorkspaceError;
use radroots_domain::{GrantId, OperationId, WorkspaceId};
use radroots_secrets::{
    EncryptedEnvelope, SecretRef,
    context::{EnvelopeContext, EnvelopePurpose, EnvelopeSubject, PayloadSchemaId},
    envelope::{ENVELOPE_MAX_BYTES, Nonce, SealMaterial, SealRequest},
    provider::{AccessPolicy, ResidencySupport, SecretProvider, SelectionPolicy},
    wrapping::SecretMaterial,
};
use radroots_storage::workspace::grant::{
    GRANT_ENVELOPE_PURPOSE, GRANT_PAYLOAD_SCHEMA, grant_envelope_subject,
};
use std::{fmt, sync::Arc};

pub(super) const PRIVATE_PAYLOAD_MAX_BYTES: usize = 16 * 1024;
pub(super) const PROTECTED_MAX_BYTES: usize = ENVELOPE_MAX_BYTES;

#[derive(Clone, Copy)]
pub(super) struct IssuanceScope {
    pub workspace: WorkspaceId,
    pub operation: OperationId,
    pub plan_digest: [u8; 32],
    pub attempt: radroots_domain::SigningAttemptId,
    pub target: radroots_storage::command::SigningTarget,
}
#[derive(Clone, Copy)]
pub(super) enum IssuancePayload {
    Attempt,
    Evidence,
    Publication,
    ListingInstallation,
}

#[derive(Clone, Copy)]
pub(super) struct DeliveryScope {
    pub workspace: WorkspaceId,
    pub operation: OperationId,
    pub target: radroots_storage::command::SigningTarget,
    pub request: [u8; 32],
}
#[derive(Clone, Copy)]
pub(super) enum DeliveryPayload {
    Attempt,
    Observation,
}

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
    pub(super) async fn seal_delivery(
        &self,
        scope: DeliveryScope,
        payload: DeliveryPayload,
        part: u8,
        plaintext: &[u8],
    ) -> Result<Vec<u8>, WorkspaceError> {
        self.seal_bound(
            delivery_context(scope, payload, part)?,
            PRIVATE_PAYLOAD_MAX_BYTES,
            plaintext,
        )
        .await
    }
    pub(super) async fn open_delivery(
        &self,
        scope: DeliveryScope,
        payload: DeliveryPayload,
        part: u8,
        protected: &[u8],
    ) -> Result<SecretMaterial, WorkspaceError> {
        self.open_bound(
            delivery_context(scope, payload, part)?,
            PRIVATE_PAYLOAD_MAX_BYTES,
            protected,
        )
        .await
    }
    pub(super) async fn seal_message(
        &self,
        workspace: WorkspaceId,
        operation: OperationId,
        plan: &[u8; 32],
        plaintext: &[u8],
    ) -> Result<Vec<u8>, WorkspaceError> {
        self.seal_bound(
            message_context(workspace, operation, plan)?,
            PRIVATE_PAYLOAD_MAX_BYTES,
            plaintext,
        )
        .await
    }
    pub(super) async fn open_message(
        &self,
        workspace: WorkspaceId,
        operation: OperationId,
        plan: &[u8; 32],
        protected: &[u8],
    ) -> Result<SecretMaterial, WorkspaceError> {
        self.open_bound(
            message_context(workspace, operation, plan)?,
            PRIVATE_PAYLOAD_MAX_BYTES,
            protected,
        )
        .await
    }
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
    pub(super) async fn seal_stock_root(
        &self,
        input: &super::AttachmentInput,
        version: i64,
        clear: &[u8],
    ) -> Result<Vec<u8>, WorkspaceError> {
        self.seal_bound(
            stock_root_context(input, version)?,
            PRIVATE_PAYLOAD_MAX_BYTES,
            clear,
        )
        .await
    }
    pub(super) async fn open_stock_root(
        &self,
        input: &super::AttachmentInput,
        version: i64,
        protected: &[u8],
    ) -> Result<SecretMaterial, WorkspaceError> {
        self.open_bound(
            stock_root_context(input, version)?,
            PRIVATE_PAYLOAD_MAX_BYTES,
            protected,
        )
        .await
    }
    pub(super) async fn seal_stock_record_for_catalog(
        &self,
        input: &super::AttachmentInput,
        kind: &str,
        id: &[u8; 32],
        catalog: i64,
        clear: &[u8],
    ) -> Result<Vec<u8>, WorkspaceError> {
        self.seal_bound(
            stock_record_context_for_catalog(input, kind, id, catalog)?,
            PRIVATE_PAYLOAD_MAX_BYTES,
            clear,
        )
        .await
    }
    pub(super) async fn open_stock_record_for_catalog(
        &self,
        input: &super::AttachmentInput,
        kind: &str,
        id: &[u8; 32],
        catalog: i64,
        protected: &[u8],
    ) -> Result<SecretMaterial, WorkspaceError> {
        self.open_bound(
            stock_record_context_for_catalog(input, kind, id, catalog)?,
            PRIVATE_PAYLOAD_MAX_BYTES,
            protected,
        )
        .await
    }
    pub(super) async fn seal_stock_record(
        &self,
        input: &super::AttachmentInput,
        kind: &str,
        id: &[u8; 32],
        clear: &[u8],
    ) -> Result<Vec<u8>, WorkspaceError> {
        self.seal_bound(
            stock_record_context(input, kind, id)?,
            PRIVATE_PAYLOAD_MAX_BYTES,
            clear,
        )
        .await
    }
    pub(super) async fn open_stock_record(
        &self,
        input: &super::AttachmentInput,
        kind: &str,
        id: &[u8; 32],
        protected: &[u8],
    ) -> Result<SecretMaterial, WorkspaceError> {
        self.open_bound(
            stock_record_context(input, kind, id)?,
            PRIVATE_PAYLOAD_MAX_BYTES,
            protected,
        )
        .await
    }
    pub(super) async fn seal_stock(
        &self,
        input: &super::AttachmentInput,
        authority: bool,
        plaintext: &[u8],
    ) -> Result<Vec<u8>, WorkspaceError> {
        self.seal_bound(
            stock_context(input, authority)?,
            PRIVATE_PAYLOAD_MAX_BYTES,
            plaintext,
        )
        .await
    }
    pub(super) async fn open_stock(
        &self,
        input: &super::AttachmentInput,
        authority: bool,
        protected: &[u8],
    ) -> Result<SecretMaterial, WorkspaceError> {
        self.open_bound(
            stock_context(input, authority)?,
            PRIVATE_PAYLOAD_MAX_BYTES,
            protected,
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
    pub(super) async fn seal_plan_part(
        &self,
        workspace: WorkspaceId,
        operation: OperationId,
        digest: &[u8; 32],
        part: u8,
        plaintext: &[u8],
    ) -> Result<Vec<u8>, WorkspaceError> {
        self.seal_bound(
            plan_context(workspace, operation, digest, part)?,
            PRIVATE_PAYLOAD_MAX_BYTES,
            plaintext,
        )
        .await
    }
    pub(super) async fn open_plan_part(
        &self,
        workspace: WorkspaceId,
        operation: OperationId,
        digest: &[u8; 32],
        part: u8,
        protected: &[u8],
    ) -> Result<SecretMaterial, WorkspaceError> {
        self.open_bound(
            plan_context(workspace, operation, digest, part)?,
            PRIVATE_PAYLOAD_MAX_BYTES,
            protected,
        )
        .await
    }
    pub(super) async fn seal_listing(
        &self,
        scope: ListingScope,
        plaintext: &[u8],
    ) -> Result<Vec<u8>, WorkspaceError> {
        self.seal_bound(
            listing_context(scope)?,
            PRIVATE_PAYLOAD_MAX_BYTES,
            plaintext,
        )
        .await
    }
    pub(super) async fn open_listing(
        &self,
        scope: ListingScope,
        protected: &[u8],
    ) -> Result<SecretMaterial, WorkspaceError> {
        self.open_bound(
            listing_context(scope)?,
            PRIVATE_PAYLOAD_MAX_BYTES,
            protected,
        )
        .await
    }
    pub(super) async fn seal_issuance(
        &self,
        scope: IssuanceScope,
        payload: IssuancePayload,
        part: u8,
        plaintext: &[u8],
    ) -> Result<Vec<u8>, WorkspaceError> {
        self.seal_bound(
            issuance_context(scope, payload, part)?,
            PRIVATE_PAYLOAD_MAX_BYTES,
            plaintext,
        )
        .await
    }
    pub(super) async fn open_issuance(
        &self,
        scope: IssuanceScope,
        payload: IssuancePayload,
        part: u8,
        protected: &[u8],
    ) -> Result<SecretMaterial, WorkspaceError> {
        self.open_bound(
            issuance_context(scope, payload, part)?,
            PRIVATE_PAYLOAD_MAX_BYTES,
            protected,
        )
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

fn delivery_context(
    scope: DeliveryScope,
    payload: DeliveryPayload,
    part: u8,
) -> Result<EnvelopeContext, WorkspaceError> {
    use sha2::{Digest, Sha256};
    let (tag, mut purpose, mut schema) = match payload {
        DeliveryPayload::Attempt => (
            0u8,
            "radroots.workspace.delivery_attempt",
            "radroots.workspace.delivery_attempt.v1",
        ),
        DeliveryPayload::Observation => (
            1u8,
            "radroots.workspace.delivery_observation",
            "radroots.workspace.delivery_observation.v1",
        ),
    };
    let mut hash = Sha256::new();
    hash.update(b"radroots.workspace.delivery.context.v1\0");
    hash.update([tag, part]);
    hash.update(scope.workspace.as_bytes());
    hash.update(scope.operation.as_bytes());
    match scope.target {
        radroots_storage::command::SigningTarget::BusinessSeal(peer) => {
            hash.update(peer.as_bytes())
        }
        radroots_storage::command::SigningTarget::PublicEvent => {
            hash.update(b"public_event.no_recipient.v1\0");
            (purpose, schema) = match payload {
                DeliveryPayload::Attempt => (
                    "radroots.workspace.public_delivery_attempt",
                    "radroots.workspace.public_delivery_attempt.v1",
                ),
                DeliveryPayload::Observation => (
                    "radroots.workspace.public_delivery_observation",
                    "radroots.workspace.public_delivery_observation.v1",
                ),
            };
        }
    }
    hash.update(scope.request);
    let subject = radroots_domain::CommandDigest::from_bytes(hash.finalize().into());
    Ok(EnvelopeContext::new(
        EnvelopePurpose::parse(purpose).map_err(|_| WorkspaceError::InvalidInput)?,
        EnvelopeSubject::parse("workspace_delivery", subject.to_string())
            .map_err(|_| WorkspaceError::InvalidInput)?,
        PayloadSchemaId::parse(schema).map_err(|_| WorkspaceError::InvalidInput)?,
    ))
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
        EnvelopePurpose::parse(GRANT_ENVELOPE_PURPOSE).map_err(|_| WorkspaceError::InvalidInput)?,
        EnvelopeSubject::parse("workspace_grant", grant_envelope_subject(workspace, grant))
            .map_err(|_| WorkspaceError::InvalidInput)?,
        PayloadSchemaId::parse(GRANT_PAYLOAD_SCHEMA).map_err(|_| WorkspaceError::InvalidInput)?,
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
        PayloadSchemaId::parse("radroots.workspace.operation.v2")
            .map_err(|_| WorkspaceError::InvalidInput)?,
    ))
}

fn plan_context(
    workspace: WorkspaceId,
    operation: OperationId,
    digest: &[u8; 32],
    part: u8,
) -> Result<EnvelopeContext, WorkspaceError> {
    use sha2::{Digest, Sha256};
    let mut hash = Sha256::new();
    hash.update(b"radroots.workspace.signing_plan.part.v1\0");
    hash.update(workspace.as_bytes());
    hash.update(operation.as_bytes());
    hash.update(digest);
    hash.update([part]);
    let subject = radroots_domain::CommandDigest::from_bytes(hash.finalize().into());
    Ok(EnvelopeContext::new(
        EnvelopePurpose::parse("radroots.workspace.signing_plan")
            .map_err(|_| WorkspaceError::InvalidInput)?,
        EnvelopeSubject::parse("workspace_plan_part", subject.to_string())
            .map_err(|_| WorkspaceError::InvalidInput)?,
        PayloadSchemaId::parse("radroots.workspace.signing_plan.v1")
            .map_err(|_| WorkspaceError::InvalidInput)?,
    ))
}

fn issuance_context(
    scope: IssuanceScope,
    payload: IssuancePayload,
    part: u8,
) -> Result<EnvelopeContext, WorkspaceError> {
    use sha2::{Digest, Sha256};
    let (tag, mut purpose, mut schema) = match payload {
        IssuancePayload::Attempt => (
            0u8,
            "radroots.workspace.signing_attempt",
            "radroots.workspace.signing_attempt.v1",
        ),
        IssuancePayload::Evidence => (
            1u8,
            "radroots.workspace.seal_evidence",
            "radroots.workspace.seal_evidence.v1",
        ),
        IssuancePayload::Publication => (
            2u8,
            "radroots.workspace.chat_publication_work",
            "radroots.workspace.chat_publication_work.v1",
        ),
        IssuancePayload::ListingInstallation => (
            3u8,
            "radroots.workspace.listing_installation",
            "radroots.workspace.listing_installation.v1",
        ),
    };
    let mut hash = Sha256::new();
    hash.update(b"radroots.workspace.issuance.context.v1\0");
    hash.update([tag, part]);
    hash.update(scope.workspace.as_bytes());
    hash.update(scope.operation.as_bytes());
    hash.update(scope.plan_digest);
    hash.update(scope.attempt.as_bytes());
    match scope.target {
        radroots_storage::command::SigningTarget::BusinessSeal(recipient) => {
            if matches!(payload, IssuancePayload::ListingInstallation) {
                return Err(WorkspaceError::InvalidInput);
            }
            hash.update(recipient.as_bytes())
        }
        radroots_storage::command::SigningTarget::PublicEvent => {
            (purpose, schema) = match payload {
                IssuancePayload::Attempt => (
                    "radroots.workspace.public_signing_attempt",
                    "radroots.workspace.public_signing_attempt.v1",
                ),
                IssuancePayload::Evidence => (
                    "radroots.workspace.public_artifact_evidence",
                    "radroots.workspace.public_artifact_evidence.v1",
                ),
                IssuancePayload::Publication => (
                    "radroots.workspace.public_publication_work",
                    "radroots.workspace.public_publication_work.v1",
                ),
                IssuancePayload::ListingInstallation => (
                    "radroots.workspace.listing_installation",
                    "radroots.workspace.listing_installation.v1",
                ),
            };
            hash.update(b"public_event.no_recipient.v1\0");
        }
    }
    let subject = radroots_domain::CommandDigest::from_bytes(hash.finalize().into());
    Ok(EnvelopeContext::new(
        EnvelopePurpose::parse(purpose).map_err(|_| WorkspaceError::InvalidInput)?,
        EnvelopeSubject::parse("workspace_issuance", subject.to_string())
            .map_err(|_| WorkspaceError::InvalidInput)?,
        PayloadSchemaId::parse(schema).map_err(|_| WorkspaceError::InvalidInput)?,
    ))
}

fn message_context(
    workspace: WorkspaceId,
    operation: OperationId,
    plan: &[u8; 32],
) -> Result<EnvelopeContext, WorkspaceError> {
    use sha2::{Digest, Sha256};
    let mut hash = Sha256::new();
    hash.update(b"radroots.workspace.chat_message.context.v1\0");
    hash.update(workspace.as_bytes());
    hash.update(operation.as_bytes());
    hash.update(plan);
    let subject = radroots_domain::CommandDigest::from_bytes(hash.finalize().into());
    Ok(EnvelopeContext::new(
        EnvelopePurpose::parse("radroots.workspace.chat_message")
            .map_err(|_| WorkspaceError::InvalidInput)?,
        EnvelopeSubject::parse("workspace_chat_message", subject.to_string())
            .map_err(|_| WorkspaceError::InvalidInput)?,
        PayloadSchemaId::parse("radroots.workspace.chat_message.v1")
            .map_err(|_| WorkspaceError::InvalidInput)?,
    ))
}

#[derive(Clone, Copy)]
pub(super) enum ListingScope {
    Decision {
        workspace: WorkspaceId,
        operation: OperationId,
    },
}
fn listing_context(scope: ListingScope) -> Result<EnvelopeContext, WorkspaceError> {
    use sha2::{Digest, Sha256};
    let mut hash = Sha256::new();
    hash.update(b"radroots.workspace.listing.state.context.v1\0");
    let (purpose, schema) = match scope {
        ListingScope::Decision {
            workspace,
            operation,
        } => {
            hash.update([0]);
            hash.update(workspace.as_bytes());
            hash.update(operation.as_bytes());
            (
                "radroots.workspace.listing_decision",
                "radroots.workspace.listing_decision.v1",
            )
        }
    };
    let digest = radroots_domain::CommandDigest::from_bytes(hash.finalize().into());
    Ok(EnvelopeContext::new(
        EnvelopePurpose::parse(purpose).map_err(|_| WorkspaceError::InvalidInput)?,
        EnvelopeSubject::parse("workspace_listing", digest.to_string())
            .map_err(|_| WorkspaceError::InvalidInput)?,
        PayloadSchemaId::parse(schema).map_err(|_| WorkspaceError::InvalidInput)?,
    ))
}

fn stock_context(
    input: &super::AttachmentInput,
    authority: bool,
) -> Result<EnvelopeContext, WorkspaceError> {
    let (purpose, schema) = if authority {
        (
            "radroots.seller_stock.authority",
            "radroots.seller_stock.authority.v1",
        )
    } else {
        (
            "radroots.seller_stock.grant",
            "radroots.seller_stock.grant.v1",
        )
    };
    use sha2::{Digest, Sha256};
    let mut subject = Sha256::new();
    subject.update(b"radroots.seller_stock.context.v1\0");
    subject.update(input.workspace_id().as_bytes());
    subject.update(input.authority().executor_id().as_bytes());
    subject.update(input.authority().generation().get().to_be_bytes());
    subject.update(input.grant_id().as_bytes());
    let subject = radroots_domain::CommandDigest::from_bytes(subject.finalize().into());
    Ok(EnvelopeContext::new(
        EnvelopePurpose::parse(purpose).map_err(|_| WorkspaceError::InvalidInput)?,
        EnvelopeSubject::parse("seller_stock", subject.to_string())
            .map_err(|_| WorkspaceError::InvalidInput)?,
        PayloadSchemaId::parse(schema).map_err(|_| WorkspaceError::InvalidInput)?,
    ))
}

fn stock_root_context(
    input: &super::AttachmentInput,
    version: i64,
) -> Result<EnvelopeContext, WorkspaceError> {
    if matches!(version, 4..=18) {
        use sha2::{Digest, Sha256};
        let namespace =
            radroots_storage::workspace::seller_stock::SellerAuthoritySelection::from_attachment(
                input,
            );
        let wire = zeroize::Zeroizing::new(
            serde_json::to_vec(&namespace).map_err(|_| WorkspaceError::InvalidInput)?,
        );
        if wire.len() > PRIVATE_PAYLOAD_MAX_BYTES {
            return Err(WorkspaceError::InvalidInput);
        }
        let mut hash = Sha256::new();
        hash.update(if version == 4 {
            b"radroots.seller_stock.authority_context.v4\0"
        } else if version == 5 {
            b"radroots.seller_stock.authority_context.v5\0"
        } else if version == 6 {
            b"radroots.seller_stock.authority_context.v6\0"
        } else if version == 7 {
            b"radroots.seller_stock.authority_context.v7\0"
        } else if version == 8 {
            b"radroots.seller_stock.authority_context.v8\0"
        } else if version == 9 {
            b"radroots.seller_stock.authority_context.v9\0"
        } else if version == 10 {
            b"radroots.seller_stock.authority_context.v10\0".as_slice()
        } else if version == 11 {
            b"radroots.seller_stock.authority_context.v11\0".as_slice()
        } else if version == 12 {
            b"radroots.seller_stock.authority_context.v12\0".as_slice()
        } else if version == 13 {
            b"radroots.seller_stock.authority_context.v13\0".as_slice()
        } else if version == 14 {
            b"radroots.seller_stock.authority_context.v14\0".as_slice()
        } else if version == 15 {
            b"radroots.seller_stock.authority_context.v15\0".as_slice()
        } else if version == 16 {
            b"radroots.seller_stock.authority_context.v16\0".as_slice()
        } else if version == 17 {
            b"radroots.seller_stock.authority_context.v17\0".as_slice()
        } else {
            b"radroots.seller_stock.authority_context.v18\0".as_slice()
        });
        hash.update(wire.as_slice());
        let subject = radroots_domain::CommandDigest::from_bytes(hash.finalize().into());
        return Ok(EnvelopeContext::new(
            EnvelopePurpose::parse("radroots.seller_stock.authority")
                .map_err(|_| WorkspaceError::InvalidInput)?,
            EnvelopeSubject::parse("seller_stock", subject.to_string())
                .map_err(|_| WorkspaceError::InvalidInput)?,
            PayloadSchemaId::parse(if version == 4 {
                "radroots.seller_stock.authority.v4"
            } else if version == 5 {
                "radroots.seller_stock.authority.v5"
            } else if version == 6 {
                "radroots.seller_stock.authority.v6"
            } else if version == 7 {
                "radroots.seller_stock.authority.v7"
            } else if version == 8 {
                "radroots.seller_stock.authority.v8"
            } else if version == 9 {
                "radroots.seller_stock.authority.v9"
            } else if version == 10 {
                "radroots.seller_stock.authority.v10"
            } else if version == 11 {
                "radroots.seller_stock.authority.v11"
            } else if version == 12 {
                "radroots.seller_stock.authority.v12"
            } else if version == 13 {
                "radroots.seller_stock.authority.v13"
            } else if version == 14 {
                "radroots.seller_stock.authority.v14"
            } else if version == 15 {
                "radroots.seller_stock.authority.v15"
            } else if version == 16 {
                "radroots.seller_stock.authority.v16"
            } else if version == 17 {
                "radroots.seller_stock.authority.v17"
            } else {
                "radroots.seller_stock.authority.v18"
            })
            .map_err(|_| WorkspaceError::InvalidInput)?,
        ));
    }
    let schema = match version {
        1 => "radroots.seller_stock.authority.v1",
        2 => "radroots.seller_stock.authority.v2",
        3 => "radroots.seller_stock.authority.v3",
        _ => return Err(WorkspaceError::RecoveryRequired),
    };
    let original = stock_context(input, true)?;
    Ok(EnvelopeContext::new(
        original.purpose().clone(),
        original.subject().clone(),
        PayloadSchemaId::parse(schema).map_err(|_| WorkspaceError::InvalidInput)?,
    ))
}
fn stock_record_context(
    input: &super::AttachmentInput,
    kind: &str,
    id: &[u8; 32],
) -> Result<EnvelopeContext, WorkspaceError> {
    use sha2::{Digest, Sha256};
    let (purpose, schema) = match kind {
        "grant_change" => (
            "radroots.seller_stock.grant_change",
            "radroots.seller_stock.grant_change.v1",
        ),
        "mapping" => (
            "radroots.seller_stock.mapping",
            "radroots.seller_stock.mapping.v1",
        ),
        "source_record" => (
            "radroots.seller_stock.source_record",
            "radroots.seller_stock.source_record.v1",
        ),
        "public_event" => (
            "radroots.seller_stock.public_source_event",
            "radroots.seller_stock.public_source_event.v1",
        ),
        "public_head" => (
            "radroots.seller_stock.public_source_head",
            "radroots.seller_stock.public_source_head.v1",
        ),
        "public_part" => (
            "radroots.seller_stock.public_source_part",
            "radroots.seller_stock.public_source_part.v1",
        ),
        "order" => (
            "radroots.seller_stock.order",
            "radroots.seller_stock.order.v1",
        ),
        "abort_movement" => (
            "radroots.seller_stock.abort_movement",
            "radroots.seller_stock.abort_movement.v1",
        ),
        "installation_movement" => (
            "radroots.seller_stock.installation_movement",
            "radroots.seller_stock.installation_movement.v1",
        ),
        "acceptance_movement" => (
            "radroots.seller_stock.acceptance_movement",
            "radroots.seller_stock.acceptance_movement.v1",
        ),
        "operation_part" => (
            "radroots.seller_stock.operation_part",
            "radroots.seller_stock.operation_part.v1",
        ),
        "source_evidence" => (
            "radroots.seller_stock.source_evidence",
            "radroots.seller_stock.source_evidence.v1",
        ),
        "source_part" => (
            "radroots.seller_stock.source_part",
            "radroots.seller_stock.source_part.v1",
        ),
        "grant" => (
            "radroots.seller_stock.grant",
            "radroots.seller_stock.grant.v2",
        ),
        "pool" => (
            "radroots.seller_stock.pool",
            "radroots.seller_stock.pool.v1",
        ),
        "operation" => (
            "radroots.seller_stock.operation",
            "radroots.seller_stock.operation.v1",
        ),
        "movement" => (
            "radroots.seller_stock.movement",
            "radroots.seller_stock.movement.v1",
        ),
        _ => return Err(WorkspaceError::InvalidInput),
    };
    let mut h = Sha256::new();
    h.update(b"radroots.seller_stock.record_context.v1\0");
    h.update(input.workspace_id().as_bytes());
    h.update(input.authority().executor_id().as_bytes());
    h.update(input.authority().generation().get().to_be_bytes());
    h.update(id);
    let id = radroots_domain::CommandDigest::from_bytes(h.finalize().into());
    Ok(EnvelopeContext::new(
        EnvelopePurpose::parse(purpose).map_err(|_| WorkspaceError::InvalidInput)?,
        EnvelopeSubject::parse("stock_record", id.to_string())
            .map_err(|_| WorkspaceError::InvalidInput)?,
        PayloadSchemaId::parse(schema).map_err(|_| WorkspaceError::InvalidInput)?,
    ))
}

fn stock_record_context_for_catalog(
    input: &super::AttachmentInput,
    kind: &str,
    id: &[u8; 32],
    catalog: i64,
) -> Result<EnvelopeContext, WorkspaceError> {
    let original = stock_record_context(input, kind, id)?;
    let schema = match (catalog, kind) {
        (2, _) | (3..=18, "pool") => return Ok(original),
        (3, "operation") => "radroots.seller_stock.operation.v2",
        (4, "operation") => "radroots.seller_stock.operation.v3",
        (5 | 6, "operation") => "radroots.seller_stock.operation.v4",
        (7..=18, "operation") => "radroots.seller_stock.operation.v5",
        (3..=18, "movement") => "radroots.seller_stock.movement.v2",
        _ => return Err(WorkspaceError::RecoveryRequired),
    };
    Ok(EnvelopeContext::new(
        original.purpose().clone(),
        original.subject().clone(),
        PayloadSchemaId::parse(schema).map_err(|_| WorkspaceError::RecoveryRequired)?,
    ))
}
