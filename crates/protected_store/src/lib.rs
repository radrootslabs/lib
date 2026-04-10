#![forbid(unsafe_code)]
#![no_std]

extern crate alloc;
#[cfg(any(feature = "std", test))]
extern crate std;

pub mod error;

use alloc::string::String;
use alloc::vec::Vec;
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use error::RadrootsProtectedStoreError;
use getrandom::getrandom;
use radroots_secret_vault::RadrootsSecretKeyWrapping;
use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

pub const RADROOTS_PROTECTED_STORE_ENVELOPE_VERSION: u8 = 1;
pub const RADROOTS_PROTECTED_STORE_KEY_LENGTH: usize = 32;
pub const RADROOTS_PROTECTED_STORE_NONCE_LENGTH: usize = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RadrootsProtectedStoreCipher {
    XChaCha20Poly1305,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RadrootsProtectedStoreKeySource {
    SecretVaultWrapped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RadrootsProtectedStoreHeader {
    pub version: u8,
    pub cipher: RadrootsProtectedStoreCipher,
    pub key_source: RadrootsProtectedStoreKeySource,
    pub key_slot: String,
    pub nonce: [u8; RADROOTS_PROTECTED_STORE_NONCE_LENGTH],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RadrootsProtectedStoreEnvelope {
    pub header: RadrootsProtectedStoreHeader,
    pub wrapped_key: Vec<u8>,
    pub ciphertext: Vec<u8>,
}

#[derive(Debug, Serialize)]
struct RadrootsProtectedStoreAad<'a> {
    version: u8,
    cipher: RadrootsProtectedStoreCipher,
    key_source: RadrootsProtectedStoreKeySource,
    key_slot: &'a str,
    nonce: &'a [u8; RADROOTS_PROTECTED_STORE_NONCE_LENGTH],
    wrapped_key: &'a [u8],
}

impl RadrootsProtectedStoreEnvelope {
    pub fn seal_with_wrapped_key<V>(
        vault: &V,
        key_slot: &str,
        plaintext: &[u8],
    ) -> Result<Self, RadrootsProtectedStoreError>
    where
        V: RadrootsSecretKeyWrapping,
    {
        Self::seal_with_wrapped_key_with_entropy(vault, key_slot, plaintext, fill_random_bytes)
    }

    fn seal_with_wrapped_key_with_entropy<V, F>(
        vault: &V,
        key_slot: &str,
        plaintext: &[u8],
        mut fill_entropy: F,
    ) -> Result<Self, RadrootsProtectedStoreError>
    where
        V: RadrootsSecretKeyWrapping,
        F: FnMut(&mut [u8]) -> Result<(), RadrootsProtectedStoreError>,
    {
        let mut store_key = [0_u8; RADROOTS_PROTECTED_STORE_KEY_LENGTH];
        let mut nonce = [0_u8; RADROOTS_PROTECTED_STORE_NONCE_LENGTH];
        fill_entropy(&mut store_key)?;
        fill_entropy(&mut nonce)?;
        let result =
            Self::seal_with_wrapped_key_and_material(vault, key_slot, plaintext, store_key, nonce);
        store_key.zeroize();
        result
    }

    pub fn seal_with_wrapped_key_and_material<V>(
        vault: &V,
        key_slot: &str,
        plaintext: &[u8],
        mut store_key: [u8; RADROOTS_PROTECTED_STORE_KEY_LENGTH],
        nonce: [u8; RADROOTS_PROTECTED_STORE_NONCE_LENGTH],
    ) -> Result<Self, RadrootsProtectedStoreError>
    where
        V: RadrootsSecretKeyWrapping,
    {
        let wrapped_key = vault
            .wrap_data_key(key_slot, &store_key)
            .map_err(|_| RadrootsProtectedStoreError::KeyWrapFailed)?;

        let header = RadrootsProtectedStoreHeader {
            version: RADROOTS_PROTECTED_STORE_ENVELOPE_VERSION,
            cipher: RadrootsProtectedStoreCipher::XChaCha20Poly1305,
            key_source: RadrootsProtectedStoreKeySource::SecretVaultWrapped,
            key_slot: String::from(key_slot),
            nonce,
        };

        let aad = envelope_aad(&header, &wrapped_key)?;
        let cipher = XChaCha20Poly1305::new(Key::from_slice(&store_key));
        let ciphertext = cipher
            .encrypt(
                XNonce::from_slice(&header.nonce),
                Payload {
                    msg: plaintext,
                    aad: &aad,
                },
            )
            .map_err(|_| RadrootsProtectedStoreError::EncryptFailed)?;
        store_key.zeroize();

        Ok(Self {
            header,
            wrapped_key,
            ciphertext,
        })
    }

    pub fn open_with_wrapped_key<V>(
        &self,
        vault: &V,
    ) -> Result<Vec<u8>, RadrootsProtectedStoreError>
    where
        V: RadrootsSecretKeyWrapping,
    {
        self.validate_header()?;
        let mut store_key = vault
            .unwrap_data_key(&self.header.key_slot, &self.wrapped_key)
            .map_err(|_| RadrootsProtectedStoreError::KeyUnwrapFailed)?;

        if store_key.len() != RADROOTS_PROTECTED_STORE_KEY_LENGTH {
            let length = store_key.len();
            store_key.zeroize();
            return Err(RadrootsProtectedStoreError::InvalidStoreKeyLength(length));
        }

        let mut store_key_bytes = [0_u8; RADROOTS_PROTECTED_STORE_KEY_LENGTH];
        store_key_bytes.copy_from_slice(&store_key);
        store_key.zeroize();

        let aad = envelope_aad(&self.header, &self.wrapped_key)?;
        let cipher = XChaCha20Poly1305::new(Key::from_slice(&store_key_bytes));
        let decrypted = cipher
            .decrypt(
                XNonce::from_slice(&self.header.nonce),
                Payload {
                    msg: &self.ciphertext,
                    aad: &aad,
                },
            )
            .map_err(|_| RadrootsProtectedStoreError::DecryptFailed)?;
        store_key_bytes.zeroize();
        Ok(decrypted)
    }

    pub fn encode_json(&self) -> Result<Vec<u8>, RadrootsProtectedStoreError> {
        serde_json::to_vec(self).map_err(|_| RadrootsProtectedStoreError::EnvelopeEncodeFailed)
    }

    pub fn decode_json(json: &[u8]) -> Result<Self, RadrootsProtectedStoreError> {
        let envelope: Self = serde_json::from_slice(json)
            .map_err(|_| RadrootsProtectedStoreError::EnvelopeDecodeFailed)?;
        envelope.validate_header()?;
        Ok(envelope)
    }

    fn validate_header(&self) -> Result<(), RadrootsProtectedStoreError> {
        if self.header.version != RADROOTS_PROTECTED_STORE_ENVELOPE_VERSION {
            return Err(RadrootsProtectedStoreError::UnsupportedEnvelopeVersion(
                self.header.version,
            ));
        }

        Ok(())
    }
}

fn envelope_aad(
    header: &RadrootsProtectedStoreHeader,
    wrapped_key: &[u8],
) -> Result<Vec<u8>, RadrootsProtectedStoreError> {
    serde_json::to_vec(&RadrootsProtectedStoreAad {
        version: header.version,
        cipher: header.cipher,
        key_source: header.key_source,
        key_slot: &header.key_slot,
        nonce: &header.nonce,
        wrapped_key,
    })
    .map_err(|_| RadrootsProtectedStoreError::EnvelopeEncodeFailed)
}

fn fill_random_bytes(bytes: &mut [u8]) -> Result<(), RadrootsProtectedStoreError> {
    getrandom(bytes).map_err(|_| RadrootsProtectedStoreError::EntropyUnavailable)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::String;
    use alloc::vec;
    use core::cell::{Cell, RefCell};

    struct FakeVault {
        wrap_calls: Cell<usize>,
        unwrap_calls: Cell<usize>,
        fail_wrap: bool,
        fail_unwrap: bool,
        last_slot: RefCell<Option<String>>,
    }

    impl FakeVault {
        fn new() -> Self {
            Self {
                wrap_calls: Cell::new(0),
                unwrap_calls: Cell::new(0),
                fail_wrap: false,
                fail_unwrap: false,
                last_slot: RefCell::new(None),
            }
        }

        fn with_wrap_failure() -> Self {
            Self {
                fail_wrap: true,
                ..Self::new()
            }
        }

        fn with_unwrap_failure() -> Self {
            Self {
                fail_unwrap: true,
                ..Self::new()
            }
        }
    }

    impl RadrootsSecretKeyWrapping for FakeVault {
        type Error = ();

        fn wrap_data_key(
            &self,
            key_slot: &str,
            plaintext_key: &[u8],
        ) -> Result<Vec<u8>, Self::Error> {
            if self.fail_wrap {
                return Err(());
            }
            self.wrap_calls.set(self.wrap_calls.get() + 1);
            self.last_slot.replace(Some(String::from(key_slot)));
            let mut wrapped = key_slot.as_bytes().to_vec();
            wrapped.push(0);
            wrapped.extend(plaintext_key.iter().map(|byte| byte ^ 0x5a));
            Ok(wrapped)
        }

        fn unwrap_data_key(
            &self,
            key_slot: &str,
            wrapped_key: &[u8],
        ) -> Result<Vec<u8>, Self::Error> {
            if self.fail_unwrap {
                return Err(());
            }
            self.unwrap_calls.set(self.unwrap_calls.get() + 1);
            self.last_slot.replace(Some(String::from(key_slot)));

            let separator = wrapped_key.iter().position(|byte| *byte == 0).ok_or(())?;
            if &wrapped_key[..separator] != key_slot.as_bytes() {
                return Err(());
            }

            Ok(wrapped_key[separator + 1..]
                .iter()
                .map(|byte| byte ^ 0x5a)
                .collect())
        }
    }

    #[test]
    fn wrapped_key_roundtrip_uses_secret_vault_and_stable_envelope() {
        let vault = FakeVault::new();
        let envelope = RadrootsProtectedStoreEnvelope::seal_with_wrapped_key_and_material(
            &vault,
            "drafts/default",
            b"secret draft body",
            [7_u8; RADROOTS_PROTECTED_STORE_KEY_LENGTH],
            [9_u8; RADROOTS_PROTECTED_STORE_NONCE_LENGTH],
        )
        .expect("seal succeeds");

        assert_eq!(vault.wrap_calls.get(), 1);
        assert_eq!(
            envelope.header.version,
            RADROOTS_PROTECTED_STORE_ENVELOPE_VERSION
        );
        assert_eq!(
            envelope.header.cipher,
            RadrootsProtectedStoreCipher::XChaCha20Poly1305
        );
        assert_eq!(
            envelope.header.key_source,
            RadrootsProtectedStoreKeySource::SecretVaultWrapped
        );
        assert_eq!(envelope.header.key_slot, "drafts/default");

        let encoded = envelope.encode_json().expect("encode succeeds");
        let decoded =
            RadrootsProtectedStoreEnvelope::decode_json(&encoded).expect("decode succeeds");
        let plaintext = decoded
            .open_with_wrapped_key(&vault)
            .expect("open succeeds");

        assert_eq!(vault.unwrap_calls.get(), 1);
        assert_eq!(plaintext, b"secret draft body");
    }

    #[test]
    fn seal_with_wrapped_key_uses_runtime_entropy_and_roundtrips() {
        let vault = FakeVault::new();
        let envelope = RadrootsProtectedStoreEnvelope::seal_with_wrapped_key(
            &vault,
            "drafts/default",
            b"runtime entropy body",
        )
        .expect("seal succeeds");

        assert_eq!(vault.wrap_calls.get(), 1);
        assert_eq!(envelope.header.key_slot, "drafts/default");

        let plaintext = envelope
            .open_with_wrapped_key(&vault)
            .expect("open succeeds");

        assert_eq!(vault.unwrap_calls.get(), 1);
        assert_eq!(plaintext, b"runtime entropy body");
    }

    #[test]
    fn seal_with_wrapped_key_reports_entropy_failure() {
        let vault = FakeVault::new();
        let mut attempts = 0usize;
        let err = RadrootsProtectedStoreEnvelope::seal_with_wrapped_key_with_entropy(
            &vault,
            "drafts/default",
            b"secret draft body",
            |_bytes| {
                attempts += 1;
                Err(RadrootsProtectedStoreError::EntropyUnavailable)
            },
        )
        .expect_err("entropy failure must surface");

        assert_eq!(attempts, 1);
        assert_eq!(err, RadrootsProtectedStoreError::EntropyUnavailable);
        assert_eq!(vault.wrap_calls.get(), 0);
    }

    #[test]
    fn tampered_wrapped_key_fails_authentication() {
        let vault = FakeVault::new();
        let mut envelope = RadrootsProtectedStoreEnvelope::seal_with_wrapped_key_and_material(
            &vault,
            "drafts/default",
            b"secret draft body",
            [3_u8; RADROOTS_PROTECTED_STORE_KEY_LENGTH],
            [4_u8; RADROOTS_PROTECTED_STORE_NONCE_LENGTH],
        )
        .expect("seal succeeds");

        let last = envelope.wrapped_key.len() - 1;
        envelope.wrapped_key[last] ^= 0x01;

        let err = envelope
            .open_with_wrapped_key(&vault)
            .expect_err("tampered wrapped key must fail");
        assert_eq!(err, RadrootsProtectedStoreError::DecryptFailed);
    }

    #[test]
    fn unsupported_version_is_rejected() {
        let envelope = RadrootsProtectedStoreEnvelope {
            header: RadrootsProtectedStoreHeader {
                version: 2,
                cipher: RadrootsProtectedStoreCipher::XChaCha20Poly1305,
                key_source: RadrootsProtectedStoreKeySource::SecretVaultWrapped,
                key_slot: String::from("drafts/default"),
                nonce: [0_u8; RADROOTS_PROTECTED_STORE_NONCE_LENGTH],
            },
            wrapped_key: vec![1, 2, 3],
            ciphertext: vec![4, 5, 6],
        };

        let encoded = envelope.encode_json().expect("encode succeeds");
        let err = RadrootsProtectedStoreEnvelope::decode_json(&encoded)
            .expect_err("unsupported version must fail");
        assert_eq!(
            err,
            RadrootsProtectedStoreError::UnsupportedEnvelopeVersion(2)
        );
    }

    #[test]
    fn wrap_failures_are_delegated_to_secret_vault() {
        let vault = FakeVault::with_wrap_failure();
        let err = RadrootsProtectedStoreEnvelope::seal_with_wrapped_key_and_material(
            &vault,
            "drafts/default",
            b"secret draft body",
            [7_u8; RADROOTS_PROTECTED_STORE_KEY_LENGTH],
            [9_u8; RADROOTS_PROTECTED_STORE_NONCE_LENGTH],
        )
        .expect_err("wrap failure must surface");

        assert_eq!(err, RadrootsProtectedStoreError::KeyWrapFailed);
    }

    #[test]
    fn unwrap_failures_are_delegated_to_secret_vault() {
        let seal_vault = FakeVault::new();
        let open_vault = FakeVault::with_unwrap_failure();
        let envelope = RadrootsProtectedStoreEnvelope::seal_with_wrapped_key_and_material(
            &seal_vault,
            "drafts/default",
            b"secret draft body",
            [7_u8; RADROOTS_PROTECTED_STORE_KEY_LENGTH],
            [9_u8; RADROOTS_PROTECTED_STORE_NONCE_LENGTH],
        )
        .expect("seal succeeds");

        let err = envelope
            .open_with_wrapped_key(&open_vault)
            .expect_err("unwrap failure must surface");
        assert_eq!(err, RadrootsProtectedStoreError::KeyUnwrapFailed);
    }
}
