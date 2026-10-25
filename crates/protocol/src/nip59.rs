//! Shared bounded NIP44/NIP59 authoring data. No signature, encryption MAC,
//! participant authority or completed store transaction is proved here.
use crate::nip01::{EventFields, WireError};
use alloc::{string::String, vec};
use core::fmt;
use radroots_domain::{PublicKey, UnixSeconds};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;
pub(crate) const MAX_TIME: u64 = 253_402_300_799;
const RANDOMIZED_PAST_SECONDS: u64 = 172_800;
/// Complete unsigned-rumor bound shared by supported private profiles.
pub const RUMOR_MAX_BYTES: usize = crate::profile::UNSIGNED_RUMOR_MAX_BYTES;
/// Complete signed seal/gift-wrap bound, including final JSON.
pub const SIGNED_MAX_BYTES: usize = crate::profile::SIGNED_EVENT_MAX_BYTES;
fn payload_shape(plaintext_bytes: usize) -> Result<(usize, usize), WireError> {
    // The qualified small-payload profile uses the two-byte length prefix.
    // This does not claim support for the newer extended NIP-44 prefix.
    if !(1..=65_535).contains(&plaintext_bytes) {
        return Err(WireError::InvalidInput);
    }
    let padded = if plaintext_bytes <= 32 {
        32
    } else {
        let power = plaintext_bytes.next_power_of_two();
        let chunk = if power <= 256 { 32 } else { power / 8 };
        plaintext_bytes.div_ceil(chunk) * chunk
    };
    let raw = padded + 67;
    Ok((raw.div_ceil(3) * 4, (3 - raw % 3) % 3))
}
/// Exact encoded size of the qualified two-byte-prefix NIP-44 v2 payload.
/// This is a size calculation, without encryption or a MAC-validity claim.
pub fn nip44_v2_payload_len(plaintext_bytes: usize) -> Result<usize, WireError> {
    Ok(payload_shape(plaintext_bytes)?.0)
}

/// Conservative complete nested sizes using the largest supported timestamp.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PrivateEnvelopeBudget {
    rumor_ciphertext_bytes: usize,
    signed_seal_max_bytes: usize,
    envelope_ciphertext_max_bytes: usize,
    signed_envelope_max_bytes: usize,
}
impl PrivateEnvelopeBudget {
    /// Measures both nesting layers before any business seal signing.
    pub fn for_rumor(rumor_bytes: usize, participant: PublicKey) -> Result<Self, WireError> {
        if rumor_bytes > RUMOR_MAX_BYTES {
            return Err(WireError::LimitExceeded);
        }
        let rumor_ciphertext_bytes = nip44_v2_payload_len(rumor_bytes)?;
        let fields = EventFields {
            author: participant,
            created_at: MAX_TIME,
            kind: 13,
            tags: &[],
            content: "",
        };
        let signed_seal_max_bytes =
            crate::nip01::signed_json_len(&fields, SIGNED_MAX_BYTES)? + rumor_ciphertext_bytes;
        let envelope_ciphertext_max_bytes = nip44_v2_payload_len(signed_seal_max_bytes)?;
        let tags = [vec![String::from("p"), participant.to_hex()]];
        let fields = EventFields {
            author: participant,
            created_at: MAX_TIME,
            kind: 1059,
            tags: &tags,
            content: "",
        };
        let signed_envelope_max_bytes = crate::nip01::signed_json_len(&fields, SIGNED_MAX_BYTES)?
            + envelope_ciphertext_max_bytes;
        if signed_seal_max_bytes > SIGNED_MAX_BYTES || signed_envelope_max_bytes > SIGNED_MAX_BYTES
        {
            return Err(WireError::LimitExceeded);
        }
        Ok(Self {
            rumor_ciphertext_bytes,
            signed_seal_max_bytes,
            envelope_ciphertext_max_bytes,
            signed_envelope_max_bytes,
        })
    }
    /// Exact encoded rumor ciphertext size.
    pub const fn rumor_ciphertext_bytes(self) -> usize {
        self.rumor_ciphertext_bytes
    }
    /// Maximum complete seal JSON size.
    pub const fn signed_seal_max_bytes(self) -> usize {
        self.signed_seal_max_bytes
    }
    /// Maximum outer encoded ciphertext size.
    pub const fn envelope_ciphertext_max_bytes(self) -> usize {
        self.envelope_ciphertext_max_bytes
    }
    /// Maximum complete signed gift-wrap size.
    pub const fn signed_envelope_max_bytes(self) -> usize {
        self.signed_envelope_max_bytes
    }
}

fn sextet(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}
fn ciphertext_shape(value: &str, plaintext_bytes: usize) -> Result<(), WireError> {
    let (size, padding) = payload_shape(plaintext_bytes)?;
    if value.len() != size {
        return Err(WireError::InvalidInput);
    }
    let bytes = value.as_bytes();
    let split = size - padding;
    if !bytes[..split].iter().all(|b| sextet(*b).is_some())
        || !bytes[split..].iter().all(|b| *b == b'=')
        || bytes[0] != b'A'
        || !(b'g'..=b'v').contains(&bytes[1])
    {
        return Err(WireError::InvalidInput);
    }
    let last = sextet(bytes[split - 1]).ok_or(WireError::InvalidInput)?;
    if (padding == 1 && last & 3 != 0) || (padding == 2 && last & 15 != 0) {
        return Err(WireError::InvalidInput);
    }
    Ok(())
}

/// Exact bounded kind13 preimage for one declared private recipient. This is
/// structural data: the caller separately proves plaintext/encryption binding,
/// recipient membership, current permission and actual durable T1 retention.
pub struct PrivateSeal {
    author: PublicKey,
    recipient: PublicKey,
    created_at: UnixSeconds,
    ciphertext: Zeroizing<String>,
    event_id: [u8; 32],
}
impl PrivateSeal {
    /// Validates the initial-authoring timestamp and both complete nesting
    /// budgets before copying. A supplied ciphertext remains a claim until
    /// qualified encryption verifies its exact original plaintext/recipient.
    pub fn new(
        author: PublicKey,
        recipient: PublicKey,
        created_at: UnixSeconds,
        rumor_created_at: UnixSeconds,
        rumor_bytes: usize,
        ciphertext: &str,
    ) -> Result<Self, WireError> {
        if created_at.get() == 0
            || rumor_created_at.get() == 0
            || created_at > rumor_created_at
            || rumor_created_at.get() - created_at.get() > RANDOMIZED_PAST_SECONDS
        {
            return Err(WireError::InvalidInput);
        }
        PrivateEnvelopeBudget::for_rumor(rumor_bytes, author)?;
        ciphertext_shape(ciphertext, rumor_bytes)?;
        let fields = EventFields {
            author,
            created_at: created_at.get(),
            kind: 13,
            tags: &[],
            content: ciphertext,
        };
        crate::nip01::signed_json_len(&fields, SIGNED_MAX_BYTES)?;
        let preimage = crate::nip01::bounded_preimage(&fields, SIGNED_MAX_BYTES)?;
        let event_id = Sha256::digest(preimage.as_bytes()).into();
        Ok(Self {
            author,
            recipient,
            created_at,
            ciphertext: Zeroizing::new(String::from(ciphertext)),
            event_id,
        })
    }
    /// Borrows the exact immutable preimage fields, with an empty tag set.
    pub fn fields(&self) -> EventFields<'_> {
        EventFields {
            author: self.author,
            created_at: self.created_at.get(),
            kind: 13,
            tags: &[],
            content: &self.ciphertext,
        }
    }
    /// Exact declared private recipient, distinct from public seal tags.
    pub const fn recipient(&self) -> PublicKey {
        self.recipient
    }
    /// NIP01 identity of these exact fields, without a signature claim.
    pub const fn event_id(&self) -> [u8; 32] {
        self.event_id
    }
}
impl fmt::Debug for PrivateSeal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PrivateSeal(<redacted>)")
    }
}

mod routes;
pub use routes::{CapturedDmRoute, DmRelayAdvertisement};
