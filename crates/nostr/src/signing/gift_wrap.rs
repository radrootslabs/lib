//! Wrap an already obtained seal without invoking a participant signer.
//! The caller must retain the result before transport; construction proves no
//! admission, recipient authorization, route approval or durable capture.

use alloc::vec;
use base64::Engine;
use core::{fmt, num::NonZeroU64};
use nostr::{
    JsonUtil,
    hashes::{Hash, sha256},
};
use radroots_event::{
    SignedEvent,
    wire::{EventWireLimits, Nip01EventWire},
};
use radroots_identity::PublicKey;
use radroots_protocol::chat::{GiftWrapBudget, SIGNED_MAX_BYTES};
use radroots_signing::provider::NonceSource;
use zeroize::Zeroizing;

const MAX_TIME: u64 = 253_402_300_799;
const PAST_SECONDS: u64 = 172_800;
const ENTROPY_DRAWS: usize = 16;

/// Fixed failures without input, ciphertext, key or dependency diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WrapError {
    /// The original representation is not an authentic supported kind-13 seal.
    InvalidSeal,
    /// The exact nested representation exceeds the supported event budget.
    LimitExceeded,
    /// The supplied current time is outside the supported positive range.
    InvalidTime,
    /// The injected entropy source failed or exhausted bounded rejection draws.
    EntropyUnavailable,
    /// Cryptography or exact output validation failed.
    CryptoFailure,
}
impl fmt::Display for WrapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidSeal => "invalid retained seal",
            Self::LimitExceeded => "gift wrap byte limit exceeded",
            Self::InvalidTime => "invalid wrapping time",
            Self::EntropyUnavailable => "wrapping entropy unavailable",
            Self::CryptoFailure => "gift wrapping failed",
        })
    }
}
impl core::error::Error for WrapError {}

/// One obtained outer representation awaiting protected durable capture.
/// It is neither cloneable nor automatically serializable. Temporary wrapping
/// material belongs to this attempt, never to a participant identity.
pub struct PendingGiftWrap {
    event: SignedEvent,
    recipient: PublicKey,
    seal_digest: [u8; 32],
    seal_event_id: [u8; 32],
    wrapping_key: Zeroizing<[u8; 32]>,
}
impl fmt::Debug for PendingGiftWrap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PendingGiftWrap(<redacted>)")
    }
}
impl PendingGiftWrap {
    /// Exact complete signed bytes; retry these bytes after durable capture.
    pub fn signed_json(&self) -> &str {
        self.event.raw_json()
    }
    /// Actual outer event ID, distinct from seal and logical message IDs.
    pub fn event_id(&self) -> [u8; 32] {
        *self.event.envelope().id().as_bytes()
    }
    /// Selected recipient. This is not proof of the original operation binding.
    pub const fn recipient(&self) -> PublicKey {
        self.recipient
    }
    /// SHA-256 of the exact original seal representation, including whitespace.
    pub const fn seal_digest(&self) -> [u8; 32] {
        self.seal_digest
    }
    /// Original independently verified seal event ID.
    pub const fn seal_event_id(&self) -> [u8; 32] {
        self.seal_event_id
    }
    /// Borrows this attempt's temporary key for independent capture verification.
    /// The callback must not log, publish or retain this key as participant custody.
    pub fn with_wrapping_key<T>(&self, verify: impl FnOnce(&[u8; 32]) -> T) -> T {
        verify(&self.wrapping_key)
    }
}

fn entropy(source: &dyn NonceSource) -> Result<Zeroizing<[u8; 32]>, WrapError> {
    source
        .next_nonce()
        .map(Zeroizing::new)
        .map_err(|_| WrapError::EntropyUnavailable)
}

fn past_time(now: u64, source: &dyn NonceSource) -> Result<u64, WrapError> {
    let range = now.min(PAST_SECONDS + 1);
    let threshold = range.wrapping_neg() % range;
    for _ in 0..ENTROPY_DRAWS {
        let bytes = entropy(source)?;
        let mut selected = [0; 8];
        selected.copy_from_slice(&bytes[..8]);
        let draw = u64::from_be_bytes(selected);
        if draw >= threshold {
            return Ok(now - draw % range);
        }
    }
    Err(WrapError::EntropyUnavailable)
}

fn wrapping_keys(
    source: &dyn NonceSource,
    author: PublicKey,
    recipient: PublicKey,
) -> Result<(Zeroizing<[u8; 32]>, nostr::Keys), WrapError> {
    for _ in 0..ENTROPY_DRAWS {
        let bytes = entropy(source)?;
        let Ok(secret) = nostr::SecretKey::from_slice(bytes.as_ref()) else {
            continue;
        };
        let keys = nostr::Keys::new(secret);
        let public = keys.public_key().to_bytes();
        if public != *author.as_bytes() && public != *recipient.as_bytes() {
            return Ok((bytes, keys));
        }
    }
    Err(WrapError::EntropyUnavailable)
}

fn exact_event(raw: &str) -> Result<SignedEvent, WrapError> {
    let limits = EventWireLimits {
        max_raw_json_bytes: SIGNED_MAX_BYTES,
        ..Default::default()
    };
    let wire = Nip01EventWire::parse_json_exact_with_limits(raw, limits)
        .map_err(|_| WrapError::InvalidSeal)?;
    let event =
        SignedEvent::from_wire_verified_id(wire, raw).map_err(|_| WrapError::InvalidSeal)?;
    if crate::event_verify::verify_event(event.envelope())
        != crate::event_verify::Verification::Verified
    {
        return Err(WrapError::InvalidSeal);
    }
    Ok(event)
}

/// Authenticates the bounded original seal and encrypts its exact bytes into a
/// signed kind-1059 envelope with one `p` tag. No participant signer, ambient
/// clock, transport, database or background task is accessed. Envelope key,
/// timestamp, nonce and signing auxiliary randomness use the injected source;
/// upstream secp256k1 context blinding retains its own initialization contract.
/// The source must provide qualified CSPRNG output; deterministic sources are test-only.
pub fn wrap_retained_seal(
    raw_seal: &str,
    recipient: PublicKey,
    now: NonZeroU64,
    source: &dyn NonceSource,
) -> Result<PendingGiftWrap, WrapError> {
    if now.get() > MAX_TIME {
        return Err(WrapError::InvalidTime);
    }
    if raw_seal.len() > SIGNED_MAX_BYTES {
        return Err(WrapError::LimitExceeded);
    }
    let budget = GiftWrapBudget::for_seal(raw_seal.len(), recipient).map_err(|e| match e {
        radroots_protocol::nip01::WireError::LimitExceeded => WrapError::LimitExceeded,
        radroots_protocol::nip01::WireError::InvalidInput => WrapError::InvalidSeal,
    })?;
    let seal = exact_event(raw_seal)?;
    let fields = seal.envelope();
    if fields.kind_u32() != 13
        || !fields.tags_as_vec().is_empty()
        || fields.created_at_u64() == 0
        || fields.created_at_u64() > now.get()
        || fields.created_at_u64() > MAX_TIME
    {
        return Err(WrapError::InvalidSeal);
    }
    let sealed_payload = base64::engine::general_purpose::STANDARD
        .decode(fields.content())
        .map_err(|_| WrapError::InvalidSeal)?;
    if sealed_payload.len() < 99
        || sealed_payload.first() != Some(&2)
        || (sealed_payload.len() - 67) % 32 != 0
    {
        return Err(WrapError::InvalidSeal);
    }
    let created_at = past_time(now.get(), source)?;
    let (wrapping_key, keys) = wrapping_keys(source, *fields.author(), recipient)?;
    let peer = crate::key::public_key_to_nostr(recipient).map_err(|_| WrapError::InvalidSeal)?;
    let nonce = entropy(source)?;
    let payload = super::private::encrypt_payload(keys.secret_key(), &peer, raw_seal, *nonce)
        .map_err(|_| WrapError::CryptoFailure)?;
    let content = base64::engine::general_purpose::STANDARD.encode(payload);
    if content.len() != budget.ciphertext_bytes() {
        return Err(WrapError::CryptoFailure);
    }
    let tags = nostr::Tags::from_list(vec![nostr::Tag::public_key(peer)]);
    let mut unsigned = nostr::UnsignedEvent {
        id: None,
        pubkey: keys.public_key(),
        created_at: nostr::Timestamp::from_secs(created_at),
        kind: nostr::Kind::GiftWrap,
        tags,
        content,
    };
    let aux = entropy(source)?;
    let message = nostr::secp256k1::Message::from_digest(unsigned.id().to_bytes());
    let mut signing_key =
        nostr::secp256k1::Keypair::from_secret_key(nostr::SECP256K1, keys.secret_key());
    let signature = nostr::SECP256K1.sign_schnorr_with_aux_rand(&message, &signing_key, &aux);
    signing_key.non_secure_erase();
    let outer = unsigned
        .add_signature(signature)
        .map_err(|_| WrapError::CryptoFailure)?;
    let raw = Zeroizing::new(outer.as_json());
    if raw.len() > budget.signed_max_bytes() {
        return Err(WrapError::CryptoFailure);
    }
    let event = exact_event(&raw).map_err(|_| WrapError::CryptoFailure)?;
    Ok(PendingGiftWrap {
        event,
        recipient,
        seal_digest: sha256::Hash::hash(raw_seal.as_bytes()).to_byte_array(),
        seal_event_id: *fields.id().as_bytes(),
        wrapping_key,
    })
}

#[cfg(test)]
mod tests;
