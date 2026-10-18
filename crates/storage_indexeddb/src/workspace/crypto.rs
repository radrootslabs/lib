use js_sys::Reflect;
use radroots_domain::{GrantId, WorkspaceId};
use radroots_secrets::{
    EncryptedEnvelope, SecretRef,
    context::{EnvelopeContext, EnvelopePurpose, EnvelopeSubject, PayloadSchemaId},
    envelope::{Nonce, SealMaterial, SealRequest},
    provider::SecretProvider,
    wrapping::SecretMaterial,
};
use radroots_storage::workspace::local_outbox::{
    GRANT_ENVELOPE_PURPOSE, GRANT_PAYLOAD_SCHEMA, grant_envelope_subject,
};
use radroots_storage::workspace::{WorkspaceError, local_outbox::GRANT_BYTES_MAX};
use wasm_bindgen::{JsCast, JsValue};
use zeroize::Zeroizing;

pub(super) const PROTECTED_MAX: usize = 32 * 1024;
pub(super) fn random<const N: usize>() -> Result<Zeroizing<[u8; N]>, WorkspaceError> {
    let crypto = Reflect::get(&js_sys::global(), &JsValue::from_str("crypto"))
        .map_err(|_| WorkspaceError::Unavailable)?
        .dyn_into::<web_sys::Crypto>()
        .map_err(|_| WorkspaceError::Unavailable)?;
    let mut bytes = Zeroizing::new([0; N]);
    crypto
        .get_random_values_with_u8_array(bytes.as_mut())
        .map_err(|_| WorkspaceError::Unavailable)?;
    Ok(bytes)
}
fn context(workspace: WorkspaceId, grant: GrantId) -> Result<EnvelopeContext, WorkspaceError> {
    Ok(EnvelopeContext::new(
        EnvelopePurpose::parse(GRANT_ENVELOPE_PURPOSE).map_err(|_| WorkspaceError::InvalidInput)?,
        EnvelopeSubject::parse("workspace_grant", grant_envelope_subject(workspace, grant))
            .map_err(|_| WorkspaceError::InvalidInput)?,
        PayloadSchemaId::parse(GRANT_PAYLOAD_SCHEMA).map_err(|_| WorkspaceError::InvalidInput)?,
    ))
}
pub(super) async fn seal(
    provider: &dyn SecretProvider,
    reference: &SecretRef,
    workspace: WorkspaceId,
    grant: GrantId,
    plaintext: &[u8],
) -> Result<Vec<u8>, WorkspaceError> {
    if plaintext.len() > GRANT_BYTES_MAX {
        return Err(WorkspaceError::InvalidInput);
    }
    let key = random::<32>()?;
    let material =
        SecretMaterial::from_slice(key.as_ref()).map_err(|_| WorkspaceError::Unavailable)?;
    let plaintext =
        SecretMaterial::from_slice(plaintext).map_err(|_| WorkspaceError::InvalidInput)?;
    let nonce = random::<24>()?;
    let result = EncryptedEnvelope::seal(
        provider,
        SealRequest::new(
            SecretRef::new(
                reference.id().clone(),
                reference.backend(),
                reference.key_version(),
            ),
            context(workspace, grant)?,
            &plaintext,
            SealMaterial::new(material, Nonce::new(*nonce)),
        ),
    )
    .await
    .map_err(|_| WorkspaceError::Unavailable)?
    .encode()
    .map_err(|_| WorkspaceError::StorageFailure)?;
    if result.len() > PROTECTED_MAX {
        return Err(WorkspaceError::InvalidInput);
    }
    Ok(result)
}
pub(super) async fn open(
    provider: &dyn SecretProvider,
    reference: &SecretRef,
    workspace: WorkspaceId,
    grant: GrantId,
    protected: &[u8],
) -> Result<SecretMaterial, WorkspaceError> {
    if protected.len() > PROTECTED_MAX {
        return Err(WorkspaceError::RecoveryRequired);
    }
    let envelope =
        EncryptedEnvelope::decode(protected).map_err(|_| WorkspaceError::RecoveryRequired)?;
    if envelope.reference().backend() != reference.backend()
        || envelope.reference().id() != reference.id()
        || envelope.reference().key_version() != reference.key_version()
    {
        return Err(WorkspaceError::RecoveryRequired);
    }
    let plaintext = envelope
        .open(provider, &context(workspace, grant)?)
        .await
        .map_err(|_| WorkspaceError::RecoveryRequired)?;
    if plaintext.len() > GRANT_BYTES_MAX {
        return Err(WorkspaceError::RecoveryRequired);
    }
    Ok(plaintext)
}
