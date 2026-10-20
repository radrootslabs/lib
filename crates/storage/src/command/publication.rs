//! Exact publication scope. Values describe retained work; they do not grant
//! a writer lease, transport permission or proof of a committed row.
use super::{
    EvidenceReceipt, OperationError, PublicPublicationWork, PublicationWork, SigningTarget,
};
use core::fmt;
use radroots_domain::{PublicKey, TransactionId, UnixSeconds};

/// One actual installed artifact's pending publication work. A public event has
/// no private participant or wrapping identity.
#[derive(Clone, Copy, Eq, PartialEq)]
pub enum PublicationArtifact {
    /// Original private seal work, published through its verified outer carrier.
    Private(PublicationWork),
    /// Original public signed event, published directly without another signer.
    Public(PublicPublicationWork),
}
impl PublicationArtifact {
    /// Actual original captured artifact selection.
    pub const fn target(self) -> SigningTarget {
        self.evidence().target()
    }
    /// Actual private participant, absent for public work.
    pub const fn recipient(self) -> Option<PublicKey> {
        self.target().recipient()
    }
    /// Private-only projection; absence never becomes a synthetic participant.
    pub fn private(self) -> Result<PublicationWork, OperationError> {
        match self {
            Self::Private(v) => Ok(v),
            Self::Public(_) => Err(OperationError::UnsupportedOperation),
        }
    }
    /// Public-only projection without a private carrier fallback.
    pub fn public(self) -> Result<PublicPublicationWork, OperationError> {
        match self {
            Self::Public(v) => Ok(v),
            Self::Private(_) => Err(OperationError::UnsupportedOperation),
        }
    }
    /// Original installed business transaction, separate from transport.
    pub const fn installation_transaction_id(self) -> TransactionId {
        match self {
            Self::Private(v) => v.installation_transaction_id(),
            Self::Public(v) => v.installation_transaction_id(),
        }
    }
    /// Original complete signing plan and its captured publication inputs.
    pub const fn plan_digest(&self) -> &[u8; 32] {
        match self {
            Self::Private(v) => v.plan_digest(),
            Self::Public(v) => v.plan_digest(),
        }
    }
    /// Original exact obtained artifact reference.
    pub const fn evidence(self) -> EvidenceReceipt {
        match self {
            Self::Private(v) => v.evidence(),
            Self::Public(v) => v.evidence(),
        }
    }
    /// Original pending-work retention transaction.
    pub const fn transaction_id(self) -> TransactionId {
        match self {
            Self::Private(v) => v.transaction_id(),
            Self::Public(v) => v.transaction_id(),
        }
    }
    /// Original queued observation, never a renewed dispatch deadline.
    pub const fn queued_at(self) -> UnixSeconds {
        match self {
            Self::Private(v) => v.queued_at(),
            Self::Public(v) => v.queued_at(),
        }
    }
}
impl From<PublicationWork> for PublicationArtifact {
    fn from(v: PublicationWork) -> Self {
        Self::Private(v)
    }
}
impl From<PublicPublicationWork> for PublicationArtifact {
    fn from(v: PublicPublicationWork) -> Self {
        Self::Public(v)
    }
}
impl fmt::Debug for PublicationArtifact {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PublicationArtifact(<redacted>)")
    }
}
