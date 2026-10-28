//! Mechanical outer containment only. Each owner authenticates original work.
use nostr::JsonUtil;
use radroots_domain::PublicKey;
use radroots_storage::command::OperationError;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;

pub(in crate::workspace) fn verify(
    raw: &str,
    author: PublicKey,
    wrapping_key: &[u8; 32],
    recipient: PublicKey,
    original: &str,
    original_digest: &[u8; 32],
) -> Result<(), OperationError> {
    let event = nostr::Event::from_json(raw).map_err(|_| OperationError::InvalidEvidence)?;
    event
        .verify()
        .map_err(|_| OperationError::InvalidEvidence)?;
    let recipient = nostr::PublicKey::from_slice(recipient.as_bytes())
        .map_err(|_| OperationError::InvalidEvidence)?;
    let secret =
        nostr::SecretKey::from_slice(wrapping_key).map_err(|_| OperationError::InvalidEvidence)?;
    let keys = nostr::Keys::new(secret);
    if keys.public_key().to_bytes() != *author.as_bytes() {
        return Err(OperationError::InvalidEvidence);
    }
    let opened = Zeroizing::new(
        nostr::nips::nip44::decrypt(keys.secret_key(), &recipient, &event.content)
            .map_err(|_| OperationError::InvalidEvidence)?,
    );
    let digest: [u8; 32] = Sha256::digest(opened.as_bytes()).into();
    if digest.ct_eq(original_digest).unwrap_u8() != 1 || opened.as_bytes() != original.as_bytes() {
        return Err(OperationError::InvalidEvidence);
    }
    Ok(())
}
