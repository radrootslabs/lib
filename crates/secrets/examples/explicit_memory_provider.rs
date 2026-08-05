use futures_executor::block_on;
use radroots_secrets::EncryptedEnvelope;
use radroots_secrets::context::{
    EnvelopeContext, EnvelopePurpose, EnvelopeSubject, PayloadSchemaId,
};
use radroots_secrets::envelope::{Nonce, SealMaterial, SealRequest};
use radroots_secrets::id::{BackendKind, KeyVersion};
use radroots_secrets::memory::MemoryProvider;
use radroots_secrets::wrapping::SecretMaterial;
use radroots_secrets::{SecretId, SecretRef};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let reference = SecretRef::new(
        SecretId::parse("example-profile-key")?,
        BackendKind::Memory,
        KeyVersion::new(1)?,
    );
    let provider = MemoryProvider::new();
    provider.provision(&reference, SecretMaterial::from_slice(&[0x41; 32])?)?;

    let plaintext = SecretMaterial::from_slice(b"private profile value")?;
    let data_key = SecretMaterial::from_slice(&[0x41; 32])?;
    let context = EnvelopeContext::new(
        EnvelopePurpose::parse("radroots.private_profile")?,
        EnvelopeSubject::parse("profile", "example-profile")?,
        PayloadSchemaId::parse("radroots.private_profile.v1")?,
    );
    let request = SealRequest::new(
        reference,
        context.clone(),
        &plaintext,
        SealMaterial::new(data_key, Nonce::new([0x24; 24])),
    );
    let encoded = block_on(EncryptedEnvelope::seal(&provider, request))?.encode()?;
    let decoded = EncryptedEnvelope::decode(&encoded)?;
    let opened = block_on(decoded.open(&provider, &context))?;

    opened.expose_secret(|bytes| assert_eq!(bytes, b"private profile value"));
    println!("opened one explicitly provisioned in-memory envelope");
    Ok(())
}
