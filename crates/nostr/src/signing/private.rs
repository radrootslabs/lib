use radroots_identity::PublicKey;
use radroots_signing::{
    provider::{
        NonceSource, ParticipantProvider, PrivateCiphertext, PrivateCryptoCapability, PrivateText,
        ProviderCall, ProviderError, ProviderState,
    },
    signer::BoxFuture,
};

use super::LocalSigner;

/// Explicit OS CSPRNG nonce source for native host composition. Constructing
/// it performs no work; errors never return a partially filled nonce.
pub struct OsNonceSource;

impl NonceSource for OsNonceSource {
    fn next_nonce(&self) -> Result<[u8; 32], ProviderError> {
        use nostr::secp256k1::rand::{RngCore, rngs::OsRng};
        let mut nonce = [0; 32];
        OsRng
            .try_fill_bytes(&mut nonce)
            .map_err(|_| ProviderError::CapabilityUnavailable)?;
        Ok(nonce)
    }
}

impl LocalSigner {
    fn admit_crypto(&self, call: &ProviderCall) -> Result<(), ProviderError> {
        if call.identity() != self.public_key {
            return Err(ProviderError::IdentityChanged);
        }
        let now = (self.clock)().map_err(|_| ProviderError::ProviderFailure)?;
        call.check_before_effect(self.public_key, now)
    }
}

impl ParticipantProvider for LocalSigner {
    fn provider_state(&self) -> BoxFuture<'_, Result<ProviderState, ProviderError>> {
        Box::pin(async move {
            let decryption = if cfg!(feature = "nip17") {
                PrivateCryptoCapability::Nip44V2
            } else {
                PrivateCryptoCapability::Unavailable
            };
            let encryption = if cfg!(feature = "nip17") && self.nonce_source.is_some() {
                PrivateCryptoCapability::Nip44V2
            } else {
                PrivateCryptoCapability::Unavailable
            };
            Ok(ProviderState::new(
                self.public_key,
                encryption,
                decryption,
                radroots_signing::recovery::ReplayCapability::LocalReplaySafe,
            )
            .with_exact_issuance(Some(
                radroots_signing::recovery::ReplayCapability::NonReplayable,
            )))
        })
    }

    fn sign_exact(
        &self,
        intent: radroots_signing::issuance::SigningIntent,
    ) -> BoxFuture<'_, radroots_signing::issuance::SigningOutcome> {
        self.issue_exact(intent)
    }

    fn encrypt_private(
        &self,
        call: ProviderCall,
        recipient: PublicKey,
        plaintext: PrivateText,
    ) -> BoxFuture<'_, Result<PrivateCiphertext, ProviderError>> {
        Box::pin(async move {
            self.admit_crypto(&call)?;
            #[cfg(feature = "nip17")]
            {
                encrypt(self, recipient, &plaintext)
            }
            #[cfg(not(feature = "nip17"))]
            {
                let _ = (recipient, plaintext);
                Err(ProviderError::CapabilityUnavailable)
            }
        })
    }

    fn decrypt_private(
        &self,
        call: ProviderCall,
        sender: PublicKey,
        ciphertext: PrivateCiphertext,
    ) -> BoxFuture<'_, Result<PrivateText, ProviderError>> {
        Box::pin(async move {
            self.admit_crypto(&call)?;
            #[cfg(feature = "nip17")]
            {
                decrypt(self, sender, &ciphertext)
            }
            #[cfg(not(feature = "nip17"))]
            {
                let _ = (sender, ciphertext);
                Err(ProviderError::CapabilityUnavailable)
            }
        })
    }
}

#[cfg(feature = "nip17")]
fn encrypt(
    signer: &LocalSigner,
    recipient: PublicKey,
    plaintext: &PrivateText,
) -> Result<PrivateCiphertext, ProviderError> {
    use base64::Engine;
    use radroots_signing::provider::PRIVATE_CRYPTO_MAX_BYTES;

    let peer =
        crate::key::public_key_to_nostr(recipient).map_err(|_| ProviderError::InvalidInput)?;
    // The pinned codec uses a two-byte length prefix. Keep all intermediate
    // allocations within the public-event boundary before obtaining entropy.
    let length = plaintext.expose().len();
    if length > 65_408 {
        return Err(ProviderError::LimitExceeded);
    }
    let padded = if length <= 32 {
        32
    } else {
        let power = length.next_power_of_two();
        let chunk = if power <= 256 { 32 } else { power / 8 };
        length.div_ceil(chunk) * chunk
    };
    let encoded_length = (padded + 67).div_ceil(3) * 4;
    if encoded_length > PRIVATE_CRYPTO_MAX_BYTES {
        return Err(ProviderError::LimitExceeded);
    }
    let source = signer
        .nonce_source
        .as_ref()
        .ok_or(ProviderError::CapabilityUnavailable)?;
    let nonce = source.next_nonce()?;
    let payload = encrypt_payload(signer.keys.secret_key(), &peer, plaintext.expose(), nonce)?;
    PrivateCiphertext::parse(&base64::engine::general_purpose::STANDARD.encode(payload))
}

#[cfg(feature = "nip17")]
pub(super) fn encrypt_payload(
    secret: &nostr::SecretKey,
    recipient: &nostr::PublicKey,
    plaintext: &str,
    nonce: [u8; 32],
) -> Result<alloc::vec::Vec<u8>, ProviderError> {
    use nostr::nips::nip44::v2::{ConversationKey, encrypt_to_bytes_with_rng};
    let key = ConversationKey::derive(secret, recipient).map_err(normalize_crypto)?;
    let mut transcript = NonceTranscript {
        nonce,
        consumed: false,
        invalid: false,
    };
    let payload = encrypt_to_bytes_with_rng(&mut transcript, &key, plaintext.as_bytes())
        .map_err(normalize_crypto)?;
    if transcript.invalid || !transcript.consumed {
        return Err(ProviderError::ProviderFailure);
    }
    Ok(payload)
}

#[cfg(feature = "nip17")]
fn decrypt(
    signer: &LocalSigner,
    sender: PublicKey,
    ciphertext: &PrivateCiphertext,
) -> Result<PrivateText, ProviderError> {
    use base64::Engine;
    use nostr::nips::nip44::v2::{ConversationKey, decrypt_to_bytes};
    use zeroize::Zeroizing;

    let peer = crate::key::public_key_to_nostr(sender).map_err(|_| ProviderError::InvalidInput)?;
    let payload = base64::engine::general_purpose::STANDARD
        .decode(ciphertext.as_str())
        .map_err(|_| ProviderError::InvalidInput)?;
    if payload.len() < 99 {
        return Err(ProviderError::InvalidInput);
    }
    if payload.first() != Some(&2) {
        return Err(ProviderError::UnsupportedVersion);
    }
    let key = ConversationKey::derive(signer.keys.secret_key(), &peer).map_err(normalize_crypto)?;
    let plaintext = Zeroizing::new(decrypt_to_bytes(&key, &payload).map_err(normalize_crypto)?);
    let text = core::str::from_utf8(&plaintext).map_err(|_| ProviderError::InvalidInput)?;
    PrivateText::new(text)
}

#[cfg(feature = "nip17")]
fn normalize_crypto(error: nostr::nips::nip44::Error) -> ProviderError {
    use nostr::nips::nip44::{Error, v2::ErrorV2};
    match error {
        Error::UnknownVersion(_) => ProviderError::UnsupportedVersion,
        Error::V2(ErrorV2::InvalidHmac) => ProviderError::AuthenticationFailed,
        Error::V2(ErrorV2::MessageTooLong) | Error::MessageTooLong => ProviderError::LimitExceeded,
        _ => ProviderError::InvalidInput,
    }
}

// Transcribes one nonce already obtained from the injected source into the
// pinned codec's RngCore API. This is not an RNG and implements no CryptoRng
// claim. Any different consumption pattern is refused before returning output.
#[cfg(feature = "nip17")]
struct NonceTranscript {
    nonce: [u8; 32],
    consumed: bool,
    invalid: bool,
}

#[cfg(feature = "nip17")]
impl Drop for NonceTranscript {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.nonce.zeroize();
    }
}

#[cfg(feature = "nip17")]
impl nostr::secp256k1::rand::RngCore for NonceTranscript {
    fn next_u32(&mut self) -> u32 {
        self.invalid = true;
        0
    }
    fn next_u64(&mut self) -> u64 {
        self.invalid = true;
        0
    }
    fn fill_bytes(&mut self, output: &mut [u8]) {
        if self.consumed || output.len() != 32 {
            self.invalid = true;
            output.fill(0);
            return;
        }
        output.copy_from_slice(&self.nonce);
        self.consumed = true;
    }
    fn try_fill_bytes(&mut self, output: &mut [u8]) -> Result<(), nostr::secp256k1::rand::Error> {
        self.fill_bytes(output);
        if self.invalid {
            Err(nostr::secp256k1::rand::Error::from(
                core::num::NonZeroU32::MIN,
            ))
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests;
