//! One obtained private outer, independent of a business-work descriptor.
use super::{OperationError, outer};
use core::fmt;
use radroots_domain::{EnvelopeEventId, PublicKey, UnixSeconds};
use sha2::{Digest, Sha256};
use std::string::String;
use zeroize::Zeroizing;

/// Bounded signed NIP-59 outer and temporary wrapping material. A verified
/// carrier proves no inner containment, installation, grant or transport right.
/// Every concrete owner must verify these against its actual retained work.
pub struct PrivateEnvelopeCandidate {
    recipient: PublicKey,
    event: EnvelopeEventId,
    author: PublicKey,
    created_at: UnixSeconds,
    digest: [u8; 32],
    raw: Zeroizing<String>,
    key: Zeroizing<[u8; 32]>,
}
impl PrivateEnvelopeCandidate {
    /// Bounds before copying and verifies the exact one-recipient outer.
    /// The reported key must still be derived and used to verify actual inner
    /// containment by the qualified store; this constructor grants no authority.
    pub fn from_obtained(
        recipient: PublicKey,
        raw: &str,
        wrapping_key: &[u8; 32],
    ) -> Result<Self, OperationError> {
        let carrier = outer(raw, recipient)?;
        Ok(Self {
            recipient,
            event: EnvelopeEventId::from_bytes(*carrier.envelope().id().as_bytes()),
            author: *carrier.envelope().author(),
            created_at: UnixSeconds::new(carrier.envelope().created_at_u64())
                .map_err(|_| OperationError::InvalidEvidence)?,
            digest: Sha256::digest(raw.as_bytes()).into(),
            raw: Zeroizing::new(String::from(raw)),
            key: Zeroizing::new(*wrapping_key),
        })
    }
    /// Exact declared recipient authenticated by the outer's single p tag.
    pub const fn recipient(&self) -> PublicKey {
        self.recipient
    }
    /// Actual signed outer event identity, separate from the original seal.
    pub const fn event_id(&self) -> EnvelopeEventId {
        self.event
    }
    /// Actual outer author; the store must prove the temporary key matches.
    pub const fn author(&self) -> PublicKey {
        self.author
    }
    /// Signed randomized time, without a grant or trusted observation.
    pub const fn created_at(&self) -> UnixSeconds {
        self.created_at
    }
    /// SHA-256 of the exact original outer representation.
    pub const fn digest(&self) -> &[u8; 32] {
        &self.digest
    }
    /// Exact obtained bytes for protected handling, never diagnostic output.
    pub fn raw_json(&self) -> &str {
        &self.raw
    }
    /// Scoped borrow solely for qualified containment verification.
    pub fn with_wrapping_key<T>(&self, verify: impl FnOnce(&[u8; 32]) -> T) -> T {
        verify(&self.key)
    }
    /// Explicit bounded owner copy; retain the original until actual storage
    /// completion is reconciled. This creates no persistence or dispatch fact.
    pub fn retained_copy(&self) -> Self {
        Self {
            recipient: self.recipient,
            event: self.event,
            author: self.author,
            created_at: self.created_at,
            digest: self.digest,
            raw: Zeroizing::new(String::from(self.raw_json())),
            key: Zeroizing::new(*self.key),
        }
    }
}
impl fmt::Debug for PrivateEnvelopeCandidate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PrivateEnvelopeCandidate(<redacted>)")
    }
}
