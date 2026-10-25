use crate::nip01::{EventFields, WireError, signed_json_len};
use crate::nip59::{MAX_TIME, SIGNED_MAX_BYTES, nip44_v2_payload_len};
use alloc::{string::String, vec};
use radroots_domain::PublicKey;

/// Complete kind-1059 budget for an already obtained seal's exact UTF-8 bytes.
/// A canonical reserialization cannot stand in for the original representation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GiftWrapBudget {
    ciphertext_bytes: usize,
    signed_max_bytes: usize,
}

impl GiftWrapBudget {
    /// Counts padding, base64 and the complete one-recipient signed envelope.
    /// This validates size only, without authenticating the seal or recipient.
    pub fn for_seal(seal_bytes: usize, recipient: PublicKey) -> Result<Self, WireError> {
        if seal_bytes >= SIGNED_MAX_BYTES {
            return Err(WireError::LimitExceeded);
        }
        let ciphertext_bytes = nip44_v2_payload_len(seal_bytes)?;
        let tags = [vec![String::from("p"), recipient.to_hex()]];
        let fields = EventFields {
            author: recipient,
            created_at: MAX_TIME,
            kind: 1059,
            tags: &tags,
            content: "",
        };
        let signed_max_bytes = signed_json_len(&fields, SIGNED_MAX_BYTES)? + ciphertext_bytes;
        if signed_max_bytes > SIGNED_MAX_BYTES {
            return Err(WireError::LimitExceeded);
        }
        Ok(Self {
            ciphertext_bytes,
            signed_max_bytes,
        })
    }

    /// Exact encoded ciphertext size for this seal representation.
    pub const fn ciphertext_bytes(self) -> usize {
        self.ciphertext_bytes
    }

    /// Complete signed JSON upper bound using the largest supported timestamp.
    pub const fn signed_max_bytes(self) -> usize {
        self.signed_max_bytes
    }
}
