use crate::auth::{RadrootsSimplexSmpEd25519Keypair, verify_signature};
use crate::error::RadrootsSimplexSmpCryptoError;
use crate::message::{
    RADROOTS_SIMPLEX_SMP_NONCE_LENGTH, RADROOTS_SIMPLEX_SMP_SHARED_SECRET_LENGTH, decrypt_padded,
    encrypt_padded, random_nonce,
};
use alloc::vec;
use alloc::vec::Vec;
use ed25519_dalek::Signer;
use hkdf::Hkdf;
use radroots_simplex_smp_proto::prelude::RadrootsSimplexSmpQueueLinkData;
use sha2::Sha512;
use sha3::{Digest, Sha3_256};

pub const RADROOTS_SIMPLEX_SMP_SHORT_LINK_ID_LENGTH: usize = 24;
pub const RADROOTS_SIMPLEX_SMP_SHORT_LINK_KEY_LENGTH: usize = 32;
pub const RADROOTS_SIMPLEX_SMP_SHORT_LINK_CONTACT_KDF_OUTPUT_LENGTH: usize = 56;
pub const RADROOTS_SIMPLEX_SMP_SHORT_LINK_FIXED_DATA_PADDED_LENGTH: usize = 2008;
pub const RADROOTS_SIMPLEX_SMP_SHORT_LINK_USER_DATA_PADDED_LENGTH: usize = 13784;
pub const RADROOTS_SIMPLEX_SMP_SHORT_LINK_CONTACT_INFO: &[u8] = b"SimpleXContactLink";
pub const RADROOTS_SIMPLEX_SMP_SHORT_LINK_INVITATION_INFO: &[u8] = b"SimpleXInvLink";
pub const RADROOTS_SIMPLEX_SMP_SHORT_LINK_SIGNATURE_LENGTH: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexSmpContactShortLinkKeyMaterial {
    pub link_id: Vec<u8>,
    pub link_data_key: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexSmpVerifiedShortLinkData {
    pub fixed_data: Vec<u8>,
    pub user_data: Vec<u8>,
}

pub fn derive_invitation_short_link_data_key(
    link_key: &[u8],
) -> Result<Vec<u8>, RadrootsSimplexSmpCryptoError> {
    validate_link_key(link_key)?;
    hkdf_expand(
        link_key,
        RADROOTS_SIMPLEX_SMP_SHORT_LINK_INVITATION_INFO,
        RADROOTS_SIMPLEX_SMP_SHORT_LINK_KEY_LENGTH,
    )
}

pub fn derive_contact_short_link_key_material(
    link_key: &[u8],
) -> Result<RadrootsSimplexSmpContactShortLinkKeyMaterial, RadrootsSimplexSmpCryptoError> {
    validate_link_key(link_key)?;
    let output = hkdf_expand(
        link_key,
        RADROOTS_SIMPLEX_SMP_SHORT_LINK_CONTACT_INFO,
        RADROOTS_SIMPLEX_SMP_SHORT_LINK_CONTACT_KDF_OUTPUT_LENGTH,
    )?;
    let (link_id, link_data_key) = output.split_at(RADROOTS_SIMPLEX_SMP_SHORT_LINK_ID_LENGTH);
    Ok(RadrootsSimplexSmpContactShortLinkKeyMaterial {
        link_id: link_id.to_vec(),
        link_data_key: link_data_key.to_vec(),
    })
}

pub fn sign_short_link_data(
    root_keypair: &RadrootsSimplexSmpEd25519Keypair,
    fixed_data: &[u8],
    user_data: &[u8],
) -> Result<(Vec<u8>, RadrootsSimplexSmpQueueLinkData), RadrootsSimplexSmpCryptoError> {
    let link_key = short_link_data_hash(fixed_data);
    let signing_key = root_keypair.signing_key()?;
    Ok((
        link_key,
        RadrootsSimplexSmpQueueLinkData {
            fixed_data: sign_payload(&signing_key, fixed_data),
            user_data: sign_payload(&signing_key, user_data),
        },
    ))
}

pub fn encrypt_short_link_data(
    link_data_key: &[u8],
    signed_data: &RadrootsSimplexSmpQueueLinkData,
) -> Result<RadrootsSimplexSmpQueueLinkData, RadrootsSimplexSmpCryptoError> {
    let fixed_nonce = random_nonce()?;
    let user_nonce = random_nonce()?;
    encrypt_short_link_data_with_nonces(link_data_key, signed_data, &fixed_nonce, &user_nonce)
}

pub fn encrypt_short_link_data_with_nonces(
    link_data_key: &[u8],
    signed_data: &RadrootsSimplexSmpQueueLinkData,
    fixed_nonce: &[u8],
    user_nonce: &[u8],
) -> Result<RadrootsSimplexSmpQueueLinkData, RadrootsSimplexSmpCryptoError> {
    validate_link_key(link_data_key)?;
    Ok(RadrootsSimplexSmpQueueLinkData {
        fixed_data: encrypt_link_data_part(
            link_data_key,
            fixed_nonce,
            &signed_data.fixed_data,
            RADROOTS_SIMPLEX_SMP_SHORT_LINK_FIXED_DATA_PADDED_LENGTH,
        )?,
        user_data: encrypt_link_data_part(
            link_data_key,
            user_nonce,
            &signed_data.user_data,
            RADROOTS_SIMPLEX_SMP_SHORT_LINK_USER_DATA_PADDED_LENGTH,
        )?,
    })
}

pub fn decrypt_short_link_data(
    link_data_key: &[u8],
    encrypted_data: &RadrootsSimplexSmpQueueLinkData,
) -> Result<RadrootsSimplexSmpQueueLinkData, RadrootsSimplexSmpCryptoError> {
    validate_link_key(link_data_key)?;
    Ok(RadrootsSimplexSmpQueueLinkData {
        fixed_data: decrypt_link_data_part(
            "fixed_data",
            link_data_key,
            &encrypted_data.fixed_data,
        )?,
        user_data: decrypt_link_data_part("user_data", link_data_key, &encrypted_data.user_data)?,
    })
}

pub fn verify_signed_short_link_data(
    link_key: &[u8],
    root_public_key: &[u8],
    signed_data: &RadrootsSimplexSmpQueueLinkData,
) -> Result<RadrootsSimplexSmpVerifiedShortLinkData, RadrootsSimplexSmpCryptoError> {
    validate_link_key(link_key)?;
    let fixed = split_signed_payload("fixed_data", &signed_data.fixed_data)?;
    let user = split_signed_payload("user_data", &signed_data.user_data)?;

    if short_link_data_hash(fixed.payload).as_slice() != link_key {
        return Err(RadrootsSimplexSmpCryptoError::ShortLinkDataHashMismatch);
    }
    verify_signature(fixed.payload, root_public_key, fixed.signature)?;
    verify_signature(user.payload, root_public_key, user.signature)?;
    Ok(RadrootsSimplexSmpVerifiedShortLinkData {
        fixed_data: fixed.payload.to_vec(),
        user_data: user.payload.to_vec(),
    })
}

pub fn decrypt_verify_short_link_data(
    link_key: &[u8],
    link_data_key: &[u8],
    root_public_key: &[u8],
    encrypted_data: &RadrootsSimplexSmpQueueLinkData,
) -> Result<RadrootsSimplexSmpVerifiedShortLinkData, RadrootsSimplexSmpCryptoError> {
    let signed_data = decrypt_short_link_data(link_data_key, encrypted_data)?;
    verify_signed_short_link_data(link_key, root_public_key, &signed_data)
}

fn short_link_data_hash(data: &[u8]) -> Vec<u8> {
    Sha3_256::digest(data).to_vec()
}

fn sign_payload(signing_key: &ed25519_dalek::SigningKey, payload: &[u8]) -> Vec<u8> {
    let signature = signing_key.sign(payload);
    let mut signed =
        Vec::with_capacity(RADROOTS_SIMPLEX_SMP_SHORT_LINK_SIGNATURE_LENGTH + payload.len());
    signed.extend_from_slice(&signature.to_bytes());
    signed.extend_from_slice(payload);
    signed
}

fn encrypt_link_data_part(
    link_data_key: &[u8],
    nonce: &[u8],
    data: &[u8],
    padded_len: usize,
) -> Result<Vec<u8>, RadrootsSimplexSmpCryptoError> {
    let mut encrypted = Vec::with_capacity(RADROOTS_SIMPLEX_SMP_NONCE_LENGTH + 16 + padded_len);
    let nonce_bytes: [u8; RADROOTS_SIMPLEX_SMP_NONCE_LENGTH] = nonce
        .try_into()
        .map_err(|_| RadrootsSimplexSmpCryptoError::InvalidNonceLength(nonce.len()))?;
    encrypted.extend_from_slice(&nonce_bytes);
    encrypted.extend_from_slice(&encrypt_padded(
        link_data_key,
        &nonce_bytes,
        data,
        padded_len,
    )?);
    Ok(encrypted)
}

fn decrypt_link_data_part(
    field: &'static str,
    link_data_key: &[u8],
    data: &[u8],
) -> Result<Vec<u8>, RadrootsSimplexSmpCryptoError> {
    if data.len() <= RADROOTS_SIMPLEX_SMP_NONCE_LENGTH {
        return Err(RadrootsSimplexSmpCryptoError::InvalidShortLinkDataLength {
            field,
            length: data.len(),
        });
    }
    let (nonce, ciphertext) = data.split_at(RADROOTS_SIMPLEX_SMP_NONCE_LENGTH);
    decrypt_padded(link_data_key, nonce, ciphertext)
}

struct SignedPayload<'a> {
    signature: &'a [u8],
    payload: &'a [u8],
}

fn split_signed_payload<'a>(
    field: &'static str,
    data: &'a [u8],
) -> Result<SignedPayload<'a>, RadrootsSimplexSmpCryptoError> {
    if data.len() <= RADROOTS_SIMPLEX_SMP_SHORT_LINK_SIGNATURE_LENGTH {
        return Err(RadrootsSimplexSmpCryptoError::InvalidShortLinkDataLength {
            field,
            length: data.len(),
        });
    }
    let (signature, payload) = data.split_at(RADROOTS_SIMPLEX_SMP_SHORT_LINK_SIGNATURE_LENGTH);
    Ok(SignedPayload { signature, payload })
}

fn validate_link_key(link_key: &[u8]) -> Result<(), RadrootsSimplexSmpCryptoError> {
    if link_key.len() != RADROOTS_SIMPLEX_SMP_SHARED_SECRET_LENGTH {
        return Err(RadrootsSimplexSmpCryptoError::InvalidShortLinkKeyLength(
            link_key.len(),
        ));
    }
    Ok(())
}

fn hkdf_expand(
    ikm: &[u8],
    info: &[u8],
    output_len: usize,
) -> Result<Vec<u8>, RadrootsSimplexSmpCryptoError> {
    let hkdf = Hkdf::<Sha512>::new(Some(b""), ikm);
    let mut output = vec![0_u8; output_len];
    hkdf.expand(info, &mut output)
        .map_err(|_| RadrootsSimplexSmpCryptoError::InvalidKeyDerivationLength(output_len))?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;

    fn keypair(seed: u8) -> RadrootsSimplexSmpEd25519Keypair {
        let private_key = [seed; 32];
        let signing_key = SigningKey::from_bytes(&private_key);
        RadrootsSimplexSmpEd25519Keypair {
            public_key: signing_key.verifying_key().to_bytes().to_vec(),
            private_key: private_key.to_vec(),
        }
    }

    #[test]
    fn derives_invitation_and_contact_short_link_keys() {
        let link_key = [7_u8; RADROOTS_SIMPLEX_SMP_SHORT_LINK_KEY_LENGTH];

        let invitation = derive_invitation_short_link_data_key(&link_key).unwrap();
        let contact = derive_contact_short_link_key_material(&link_key).unwrap();

        assert_eq!(invitation.len(), RADROOTS_SIMPLEX_SMP_SHORT_LINK_KEY_LENGTH);
        assert_eq!(
            contact.link_id.len(),
            RADROOTS_SIMPLEX_SMP_SHORT_LINK_ID_LENGTH
        );
        assert_eq!(
            contact.link_data_key.len(),
            RADROOTS_SIMPLEX_SMP_SHORT_LINK_KEY_LENGTH
        );
        assert_ne!(invitation, contact.link_data_key);
    }

    #[test]
    fn signs_encrypts_decrypts_and_verifies_short_link_data() {
        let root = keypair(11);
        let fixed_data = b"rr-synth-fixed-link-data".to_vec();
        let user_data = b"rr-synth-user-link-data".to_vec();
        let (link_key, signed_data) = sign_short_link_data(&root, &fixed_data, &user_data).unwrap();
        let link_data_key = derive_invitation_short_link_data_key(&link_key).unwrap();
        let fixed_nonce = [1_u8; RADROOTS_SIMPLEX_SMP_NONCE_LENGTH];
        let user_nonce = [2_u8; RADROOTS_SIMPLEX_SMP_NONCE_LENGTH];

        let encrypted = encrypt_short_link_data_with_nonces(
            &link_data_key,
            &signed_data,
            &fixed_nonce,
            &user_nonce,
        )
        .unwrap();

        assert_eq!(
            encrypted.fixed_data.len(),
            RADROOTS_SIMPLEX_SMP_NONCE_LENGTH
                + 16
                + RADROOTS_SIMPLEX_SMP_SHORT_LINK_FIXED_DATA_PADDED_LENGTH
        );
        assert_eq!(
            encrypted.user_data.len(),
            RADROOTS_SIMPLEX_SMP_NONCE_LENGTH
                + 16
                + RADROOTS_SIMPLEX_SMP_SHORT_LINK_USER_DATA_PADDED_LENGTH
        );
        let verified =
            decrypt_verify_short_link_data(&link_key, &link_data_key, &root.public_key, &encrypted)
                .unwrap();
        assert_eq!(verified.fixed_data, fixed_data);
        assert_eq!(verified.user_data, user_data);
    }

    #[test]
    fn rejects_short_link_hash_mismatch() {
        let root = keypair(13);
        let (link_key, signed_data) =
            sign_short_link_data(&root, b"rr-synth-fixed", b"rr-synth-user").unwrap();
        let mut wrong_link_key = link_key;
        wrong_link_key[0] ^= 0xff;

        let error = verify_signed_short_link_data(&wrong_link_key, &root.public_key, &signed_data)
            .unwrap_err();

        assert!(matches!(
            error,
            RadrootsSimplexSmpCryptoError::ShortLinkDataHashMismatch
        ));
    }

    #[test]
    fn rejects_tampered_signed_user_data() {
        let root = keypair(17);
        let (link_key, mut signed_data) =
            sign_short_link_data(&root, b"rr-synth-fixed", b"rr-synth-user").unwrap();
        let last = signed_data.user_data.last_mut().unwrap();
        *last ^= 0xff;

        let error =
            verify_signed_short_link_data(&link_key, &root.public_key, &signed_data).unwrap_err();

        assert!(matches!(
            error,
            RadrootsSimplexSmpCryptoError::SignatureVerificationFailed
        ));
    }
}
