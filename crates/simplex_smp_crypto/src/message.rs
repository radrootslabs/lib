use crate::error::RadrootsSimplexSmpCryptoError;
use alloc::vec::Vec;
use getrandom::getrandom;
use hkdf::Hkdf;
use sha2::{Digest, Sha256, Sha512};
use x25519_dalek::{PublicKey, StaticSecret};
use xsalsa20poly1305::aead::{AeadInPlace, KeyInit};
use xsalsa20poly1305::{Tag, XSalsa20Poly1305};

pub const RADROOTS_SIMPLEX_SMP_NONCE_LENGTH: usize = 24;
pub const RADROOTS_SIMPLEX_SMP_SHARED_SECRET_LENGTH: usize = 32;
const RADROOTS_SIMPLEX_SMP_AUTH_TAG_LENGTH: usize = 16;
const RADROOTS_SIMPLEX_SMP_SECRETBOX_CHAIN_INIT_INFO: &[u8] = b"SimpleXSbChainInit";
const RADROOTS_SIMPLEX_SMP_SECRETBOX_CHAIN_INFO: &[u8] = b"SimpleXSbChain";
const RADROOTS_SIMPLEX_SMP_SECRETBOX_CHAIN_KEY_LENGTH: usize = 32;
const RADROOTS_SIMPLEX_SMP_SECRETBOX_CHAIN_INIT_OUTPUT_LENGTH: usize = 64;
const RADROOTS_SIMPLEX_SMP_SECRETBOX_CHAIN_STEP_OUTPUT_LENGTH: usize = 88;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexSmpX25519Keypair {
    pub public_key: Vec<u8>,
    pub private_key: Vec<u8>,
}

impl RadrootsSimplexSmpX25519Keypair {
    pub fn generate() -> Result<Self, RadrootsSimplexSmpCryptoError> {
        let mut secret = [0_u8; RADROOTS_SIMPLEX_SMP_SHARED_SECRET_LENGTH];
        getrandom(&mut secret).map_err(|_| RadrootsSimplexSmpCryptoError::EntropyUnavailable)?;
        Ok(Self::from_secret_bytes(secret))
    }

    pub fn from_seed(seed: &[u8]) -> Self {
        let digest = Sha256::digest(seed);
        let mut secret = [0_u8; RADROOTS_SIMPLEX_SMP_SHARED_SECRET_LENGTH];
        secret.copy_from_slice(&digest[..RADROOTS_SIMPLEX_SMP_SHARED_SECRET_LENGTH]);
        Self::from_secret_bytes(secret)
    }

    pub fn public_key_from_private(
        private_key: &[u8],
    ) -> Result<Vec<u8>, RadrootsSimplexSmpCryptoError> {
        let private: [u8; RADROOTS_SIMPLEX_SMP_SHARED_SECRET_LENGTH] =
            private_key.try_into().map_err(|_| {
                RadrootsSimplexSmpCryptoError::InvalidPrivateKeyLength(private_key.len())
            })?;
        Ok(PublicKey::from(&StaticSecret::from(private))
            .as_bytes()
            .to_vec())
    }

    fn from_secret_bytes(secret: [u8; RADROOTS_SIMPLEX_SMP_SHARED_SECRET_LENGTH]) -> Self {
        let private = StaticSecret::from(secret);
        let public = PublicKey::from(&private);
        Self {
            public_key: public.as_bytes().to_vec(),
            private_key: private.to_bytes().to_vec(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexSmpSecretBoxChainKey {
    bytes: [u8; RADROOTS_SIMPLEX_SMP_SECRETBOX_CHAIN_KEY_LENGTH],
}

impl RadrootsSimplexSmpSecretBoxChainKey {
    fn from_slice(value: &[u8]) -> Result<Self, RadrootsSimplexSmpCryptoError> {
        let bytes: [u8; RADROOTS_SIMPLEX_SMP_SECRETBOX_CHAIN_KEY_LENGTH] =
            value.try_into().map_err(|_| {
                RadrootsSimplexSmpCryptoError::InvalidSecretBoxChainKeyLength(value.len())
            })?;
        Ok(Self { bytes })
    }
}

pub fn init_secretbox_chain(
    session_identifier: &[u8],
    shared_secret: &[u8],
) -> Result<
    (
        RadrootsSimplexSmpSecretBoxChainKey,
        RadrootsSimplexSmpSecretBoxChainKey,
    ),
    RadrootsSimplexSmpCryptoError,
> {
    let output = hkdf_expand(
        session_identifier,
        shared_secret,
        RADROOTS_SIMPLEX_SMP_SECRETBOX_CHAIN_INIT_INFO,
        RADROOTS_SIMPLEX_SMP_SECRETBOX_CHAIN_INIT_OUTPUT_LENGTH,
    )?;
    let (first, second) = output.split_at(RADROOTS_SIMPLEX_SMP_SECRETBOX_CHAIN_KEY_LENGTH);
    Ok((
        RadrootsSimplexSmpSecretBoxChainKey::from_slice(first)?,
        RadrootsSimplexSmpSecretBoxChainKey::from_slice(second)?,
    ))
}

pub fn advance_secretbox_chain(
    chain_key: &RadrootsSimplexSmpSecretBoxChainKey,
) -> Result<
    (
        (Vec<u8>, [u8; RADROOTS_SIMPLEX_SMP_NONCE_LENGTH]),
        RadrootsSimplexSmpSecretBoxChainKey,
    ),
    RadrootsSimplexSmpCryptoError,
> {
    let output = hkdf_expand(
        b"",
        &chain_key.bytes,
        RADROOTS_SIMPLEX_SMP_SECRETBOX_CHAIN_INFO,
        RADROOTS_SIMPLEX_SMP_SECRETBOX_CHAIN_STEP_OUTPUT_LENGTH,
    )?;
    let (next_chain_key, remainder) =
        output.split_at(RADROOTS_SIMPLEX_SMP_SECRETBOX_CHAIN_KEY_LENGTH);
    let (secretbox_key, nonce_bytes) =
        remainder.split_at(RADROOTS_SIMPLEX_SMP_SHARED_SECRET_LENGTH);
    Ok((
        (
            secretbox_key.to_vec(),
            nonce_bytes.try_into().map_err(|_| {
                RadrootsSimplexSmpCryptoError::InvalidNonceLength(nonce_bytes.len())
            })?,
        ),
        RadrootsSimplexSmpSecretBoxChainKey::from_slice(next_chain_key)?,
    ))
}

pub fn derive_shared_secret(
    private_key: &[u8],
    public_key: &[u8],
) -> Result<Vec<u8>, RadrootsSimplexSmpCryptoError> {
    let private: [u8; RADROOTS_SIMPLEX_SMP_SHARED_SECRET_LENGTH] = private_key
        .try_into()
        .map_err(|_| RadrootsSimplexSmpCryptoError::InvalidPrivateKeyLength(private_key.len()))?;
    let public: [u8; RADROOTS_SIMPLEX_SMP_SHARED_SECRET_LENGTH] = public_key
        .try_into()
        .map_err(|_| RadrootsSimplexSmpCryptoError::InvalidPublicKeyLength(public_key.len()))?;
    let secret = StaticSecret::from(private).diffie_hellman(&PublicKey::from(public));
    Ok(secret.as_bytes().to_vec())
}

pub fn random_nonce()
-> Result<[u8; RADROOTS_SIMPLEX_SMP_NONCE_LENGTH], RadrootsSimplexSmpCryptoError> {
    let mut nonce = [0_u8; RADROOTS_SIMPLEX_SMP_NONCE_LENGTH];
    getrandom(&mut nonce).map_err(|_| RadrootsSimplexSmpCryptoError::EntropyUnavailable)?;
    Ok(nonce)
}

pub fn encrypt_padded(
    shared_secret: &[u8],
    nonce: &[u8],
    plaintext: &[u8],
    padded_len: usize,
) -> Result<Vec<u8>, RadrootsSimplexSmpCryptoError> {
    if plaintext.len().saturating_add(2) > padded_len {
        return Err(RadrootsSimplexSmpCryptoError::InvalidMessageLength {
            actual: plaintext.len(),
            padded: padded_len,
        });
    }
    let mut padded = Vec::with_capacity(padded_len);
    padded.extend_from_slice(&(plaintext.len() as u16).to_be_bytes());
    padded.extend_from_slice(plaintext);
    padded.resize(padded_len, 0);
    encrypt_no_pad(shared_secret, nonce, &padded)
}

pub fn decrypt_padded(
    shared_secret: &[u8],
    nonce: &[u8],
    ciphertext: &[u8],
) -> Result<Vec<u8>, RadrootsSimplexSmpCryptoError> {
    let padded = decrypt_no_pad(shared_secret, nonce, ciphertext)?;
    if padded.len() < 2 {
        return Err(RadrootsSimplexSmpCryptoError::InvalidCiphertextLength(
            padded.len(),
        ));
    }
    let length = u16::from_be_bytes([padded[0], padded[1]]) as usize;
    if length > padded.len().saturating_sub(2) {
        return Err(RadrootsSimplexSmpCryptoError::InvalidCiphertextLength(
            padded.len(),
        ));
    }
    Ok(padded[2..2 + length].to_vec())
}

pub fn encrypt_no_pad(
    shared_secret: &[u8],
    nonce: &[u8],
    plaintext: &[u8],
) -> Result<Vec<u8>, RadrootsSimplexSmpCryptoError> {
    let cipher = cipher(shared_secret)?;
    let mut buffer = plaintext.to_vec();
    let tag = cipher
        .encrypt_in_place_detached(&nonce_array(nonce)?.into(), b"", &mut buffer)
        .map_err(|_| RadrootsSimplexSmpCryptoError::InvalidCiphertextLength(plaintext.len()))?;
    let mut encrypted = Vec::with_capacity(RADROOTS_SIMPLEX_SMP_AUTH_TAG_LENGTH + buffer.len());
    encrypted.extend_from_slice(&tag);
    encrypted.extend_from_slice(&buffer);
    Ok(encrypted)
}

pub fn decrypt_no_pad(
    shared_secret: &[u8],
    nonce: &[u8],
    ciphertext: &[u8],
) -> Result<Vec<u8>, RadrootsSimplexSmpCryptoError> {
    if ciphertext.len() < RADROOTS_SIMPLEX_SMP_AUTH_TAG_LENGTH {
        return Err(RadrootsSimplexSmpCryptoError::InvalidCiphertextLength(
            ciphertext.len(),
        ));
    }
    let cipher = cipher(shared_secret)?;
    let (tag_bytes, encrypted) = ciphertext.split_at(RADROOTS_SIMPLEX_SMP_AUTH_TAG_LENGTH);
    let tag = Tag::from_slice(tag_bytes);
    let mut buffer = encrypted.to_vec();
    cipher
        .decrypt_in_place_detached(&nonce_array(nonce)?.into(), b"", &mut buffer, tag)
        .map_err(|_| RadrootsSimplexSmpCryptoError::InvalidCiphertextLength(ciphertext.len()))?;
    Ok(buffer)
}

fn cipher(shared_secret: &[u8]) -> Result<XSalsa20Poly1305, RadrootsSimplexSmpCryptoError> {
    if shared_secret.len() != RADROOTS_SIMPLEX_SMP_SHARED_SECRET_LENGTH {
        return Err(RadrootsSimplexSmpCryptoError::InvalidSharedSecretLength(
            shared_secret.len(),
        ));
    }
    Ok(
        XSalsa20Poly1305::new_from_slice(shared_secret).map_err(|_| {
            RadrootsSimplexSmpCryptoError::InvalidSharedSecretLength(shared_secret.len())
        })?,
    )
}

fn nonce_array(
    nonce: &[u8],
) -> Result<[u8; RADROOTS_SIMPLEX_SMP_NONCE_LENGTH], RadrootsSimplexSmpCryptoError> {
    nonce
        .try_into()
        .map_err(|_| RadrootsSimplexSmpCryptoError::InvalidNonceLength(nonce.len()))
}

fn hkdf_expand(
    salt: &[u8],
    ikm: &[u8],
    info: &[u8],
    output_len: usize,
) -> Result<Vec<u8>, RadrootsSimplexSmpCryptoError> {
    let hkdf = Hkdf::<Sha512>::new(Some(salt), ikm);
    let mut output = vec![0_u8; output_len];
    hkdf.expand(info, &mut output)
        .map_err(|_| RadrootsSimplexSmpCryptoError::InvalidKeyDerivationLength(output_len))?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_repeatable_keypair_from_seed() {
        let first = RadrootsSimplexSmpX25519Keypair::from_seed(b"seed");
        let second = RadrootsSimplexSmpX25519Keypair::from_seed(b"seed");
        assert_eq!(first, second);
    }

    #[test]
    fn encrypts_and_decrypts_padded_message() {
        let alice = RadrootsSimplexSmpX25519Keypair::from_seed(b"alice");
        let bob = RadrootsSimplexSmpX25519Keypair::from_seed(b"bob");
        let alice_secret = derive_shared_secret(&alice.private_key, &bob.public_key).unwrap();
        let bob_secret = derive_shared_secret(&bob.private_key, &alice.public_key).unwrap();
        assert_eq!(alice_secret, bob_secret);

        let nonce = [5_u8; RADROOTS_SIMPLEX_SMP_NONCE_LENGTH];
        let ciphertext = encrypt_padded(&alice_secret, &nonce, b"hello", 32).unwrap();
        let plaintext = decrypt_padded(&bob_secret, &nonce, &ciphertext).unwrap();
        assert_eq!(plaintext, b"hello");
    }

    #[test]
    fn derives_repeatable_secretbox_chain_progression() {
        let (rcv_first, snd_first) = init_secretbox_chain(b"session", b"shared-secret").unwrap();
        let (rcv_second, snd_second) = init_secretbox_chain(b"session", b"shared-secret").unwrap();
        assert_eq!(rcv_first, rcv_second);
        assert_eq!(snd_first, snd_second);

        let ((send_key, send_nonce), next_send) = advance_secretbox_chain(&snd_first).unwrap();
        let ((recv_key, recv_nonce), next_recv) = advance_secretbox_chain(&rcv_first).unwrap();

        assert_eq!(send_key.len(), RADROOTS_SIMPLEX_SMP_SHARED_SECRET_LENGTH);
        assert_eq!(recv_key.len(), RADROOTS_SIMPLEX_SMP_SHARED_SECRET_LENGTH);
        assert_ne!(send_nonce, recv_nonce);
        assert_ne!(next_send, snd_first);
        assert_ne!(next_recv, rcv_first);
    }
}
