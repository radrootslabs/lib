//! Exact v1 signing commitment data, without signer or execution authority.
use core::num::NonZeroU64;
use radroots_domain::{OperationId, PublicKey};
use sha2::{Digest, Sha256};

const DOMAIN: &[u8] = b"radroots.exact_signing_intent.v1\0";
const MAX_OBSERVATION_UNIX_MS: u64 = 253_402_300_799_999;

/// Purpose of an exact transport artifact, separate from business permission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ArtifactRole {
    /// A public authored event, excluding private carrier and rumor kinds.
    PublicEvent,
    /// One encrypted kind-13 business seal for an exact recipient.
    BusinessSeal,
    /// One outer kind-1059 gift wrap for an exact recipient.
    GiftWrap,
}
/// Invalid descriptor data; no provider diagnostic or private input is retained.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidIntentBinding;
impl core::fmt::Display for InvalidIntentBinding {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("invalid exact intent binding")
    }
}
impl core::error::Error for InvalidIntentBinding {}

/// Immutable v1 commitment, with no signature, qualified replay guarantee,
/// command grant, dispatch permission or proof of a retained transaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IntentBinding {
    operation_id: OperationId,
    command_digest: [u8; 32],
    digest: [u8; 32],
}
impl IntentBinding {
    /// Hashes exact already-captured data. The caller independently validates
    /// the plan, role, current access, provider and durable execution boundary.
    /// This preserves the existing v1 byte layout and domain separator.
    pub fn from_components(
        operation_id: OperationId,
        command_digest: [u8; 32],
        plan_digest: [u8; 32],
        role: ArtifactRole,
        recipient: Option<PublicKey>,
        deadline_unix_ms: NonZeroU64,
    ) -> Result<Self, InvalidIntentBinding> {
        if deadline_unix_ms.get() > MAX_OBSERVATION_UNIX_MS
            || matches!(role, ArtifactRole::PublicEvent) != recipient.is_none()
        {
            return Err(InvalidIntentBinding);
        }
        let mut digest = Sha256::new();
        digest.update(DOMAIN);
        digest.update(operation_id.as_bytes());
        digest.update(command_digest);
        digest.update(plan_digest);
        digest.update([match role {
            ArtifactRole::PublicEvent => 0,
            ArtifactRole::BusinessSeal => 1,
            ArtifactRole::GiftWrap => 2,
        }]);
        match recipient {
            Some(peer) => {
                digest.update([1]);
                digest.update(peer.as_bytes());
            }
            None => digest.update([0]),
        }
        digest.update(deadline_unix_ms.get().to_be_bytes());
        Ok(Self {
            operation_id,
            command_digest,
            digest: digest.finalize().into(),
        })
    }
    /// Returns the full original operation identity, without truncation.
    pub const fn operation_id(self) -> OperationId {
        self.operation_id
    }
    /// Borrows the original semantic command commitment.
    pub const fn command_digest(&self) -> &[u8; 32] {
        &self.command_digest
    }
    /// Borrows the exact domain-separated v1 commitment.
    pub const fn digest(&self) -> &[u8; 32] {
        &self.digest
    }
}
