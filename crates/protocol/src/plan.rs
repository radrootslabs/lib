//! Common immutable signing-plan data, with explicit artifact families.
use crate::{
    command::CapturedCommand,
    intent::{ArtifactRole, SigningReplay},
    nip01::{EventFields, WireError},
};
use alloc::vec::Vec;
use core::fmt;
use radroots_domain::{CommandDigest, OperationId, PublicKey, UnixSeconds};

/// Exact T1 artifact selection. Public events have no invented private peer.
/// Selection is data only and establishes neither authority nor an invocation.
#[derive(Clone, Copy, Eq, PartialEq, Hash)]
pub enum SigningTarget {
    /// The single exact public event in a public plan.
    PublicEvent,
    /// One exact recipient's business seal in a private plan.
    BusinessSeal(PublicKey),
}
impl SigningTarget {
    /// Role bound by the selected plan artifact.
    pub const fn role(self) -> ArtifactRole {
        match self {
            Self::PublicEvent => ArtifactRole::PublicEvent,
            Self::BusinessSeal(_) => ArtifactRole::BusinessSeal,
        }
    }
    /// No public event is represented by a participant key.
    pub const fn recipient(self) -> Option<PublicKey> {
        match self {
            Self::PublicEvent => None,
            Self::BusinessSeal(peer) => Some(peer),
        }
    }
    /// Bounded unambiguous storage selector. Its public tag is not a recipient.
    pub fn key(self) -> Vec<u8> {
        match self {
            Self::PublicEvent => alloc::vec![0],
            Self::BusinessSeal(peer) => peer.as_bytes().to_vec(),
        }
    }
    /// Decodes only the exact supported tagged selector profiles.
    pub fn from_key(key: &[u8]) -> Result<Self, WireError> {
        match key {
            [0] => Ok(Self::PublicEvent),
            rest if rest.len() == 32 => {
                PublicKey::from_bytes(rest.try_into().map_err(|_| WireError::InvalidInput)?)
                    .map(Self::BusinessSeal)
                    .map_err(|_| WireError::InvalidInput)
            }
            _ => Err(WireError::InvalidInput),
        }
    }
}
impl From<PublicKey> for SigningTarget {
    fn from(recipient: PublicKey) -> Self {
        Self::BusinessSeal(recipient)
    }
}
impl fmt::Debug for SigningTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::PublicEvent => "SigningTarget::PublicEvent",
            Self::BusinessSeal(_) => "SigningTarget::BusinessSeal(<redacted>)",
        })
    }
}

/// Borrowed exact known artifact. This authenticates no signer or store fact.
pub enum SigningArtifact<'a> {
    /// Exact assigned-kind public listing.
    #[cfg(feature = "listing")]
    Public(&'a crate::listing::ListingEvent),
    /// Exact private business seal, with its actual recipient.
    #[cfg(feature = "chat")]
    Seal(&'a crate::chat::ChatSeal),
}
impl SigningArtifact<'_> {
    /// Borrows original NIP-01 fields without rebuilding content or tags.
    pub fn fields(&self) -> EventFields<'_> {
        match self {
            #[cfg(feature = "listing")]
            Self::Public(event) => event.fields(),
            #[cfg(feature = "chat")]
            Self::Seal(seal) => seal.fields(),
        }
    }
    /// Original exact event ID.
    pub fn event_id(&self) -> [u8; 32] {
        match self {
            #[cfg(feature = "listing")]
            Self::Public(event) => *event.event_id(),
            #[cfg(feature = "chat")]
            Self::Seal(seal) => seal.event_id(),
        }
    }
    /// Exact role/recipient pair without a public-peer fallback.
    pub fn target(&self) -> SigningTarget {
        match self {
            #[cfg(feature = "listing")]
            Self::Public(_) => SigningTarget::PublicEvent,
            #[cfg(feature = "chat")]
            Self::Seal(seal) => SigningTarget::BusinessSeal(seal.recipient()),
        }
    }
}
impl fmt::Debug for SigningArtifact<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SigningArtifact(<redacted>)")
    }
}

/// Exact complete plan data. No public artifact is represented as a chat seal,
/// and no optional recipient is filled with a synthetic participant.
pub enum CapturedSigningPlan {
    /// Original private profile, preserving its established binary bytes.
    #[cfg(feature = "chat")]
    Chat(crate::chat::ChatSigningPlan),
    /// Exact known-kind public listing preimage and selected target descriptors.
    #[cfg(feature = "listing")]
    Listing(crate::listing::PublicSigningPlan),
}
impl CapturedSigningPlan {
    /// Resolves only an actual retained artifact in this exact plan family.
    pub fn artifact(&self, target: SigningTarget) -> Result<SigningArtifact<'_>, WireError> {
        match (self, target) {
            #[cfg(feature = "listing")]
            (Self::Listing(plan), SigningTarget::PublicEvent) => {
                Ok(SigningArtifact::Public(plan.event()))
            }
            #[cfg(feature = "chat")]
            (Self::Chat(plan), SigningTarget::BusinessSeal(peer)) => plan
                .seals()
                .iter()
                .find(|seal| seal.recipient() == peer)
                .map(SigningArtifact::Seal)
                .ok_or(WireError::InvalidInput),
            _ => Err(WireError::InvalidInput),
        }
    }
    /// Reconstructs only the family of the independently retained command.
    pub fn parse(command: &CapturedCommand, wire: &[u8]) -> Result<Self, WireError> {
        match command {
            #[cfg(feature = "chat")]
            CapturedCommand::Chat(command) => {
                crate::chat::ChatSigningPlan::parse(command, wire).map(Self::Chat)
            }
            #[cfg(feature = "listing")]
            CapturedCommand::Listing(command) => {
                crate::listing::PublicSigningPlan::parse(command, wire).map(Self::Listing)
            }
            #[cfg(not(feature = "chat"))]
            CapturedCommand::Chat(_) => Err(WireError::InvalidInput),
        }
    }
    /// Original operation identity.
    pub const fn operation_id(&self) -> OperationId {
        match self {
            #[cfg(feature = "chat")]
            Self::Chat(p) => p.operation_id(),
            #[cfg(feature = "listing")]
            Self::Listing(p) => p.operation_id(),
        }
    }
    /// Exact command commitment.
    pub const fn command_digest(&self) -> CommandDigest {
        match self {
            #[cfg(feature = "chat")]
            Self::Chat(p) => p.command_digest(),
            #[cfg(feature = "listing")]
            Self::Listing(p) => p.command_digest(),
        }
    }
    /// Original business event time.
    pub const fn created_at(&self) -> UnixSeconds {
        match self {
            #[cfg(feature = "chat")]
            Self::Chat(p) => p.created_at(),
            #[cfg(feature = "listing")]
            Self::Listing(p) => p.created_at(),
        }
    }
    /// Historical declaration, without permission to reissue.
    pub const fn replay(&self) -> SigningReplay {
        match self {
            #[cfg(feature = "chat")]
            Self::Chat(p) => p.replay(),
            #[cfg(feature = "listing")]
            Self::Listing(p) => p.replay(),
        }
    }
    /// Complete immutable protected representation.
    pub fn protected_wire(&self) -> &[u8] {
        match self {
            #[cfg(feature = "chat")]
            Self::Chat(p) => p.protected_wire(),
            #[cfg(feature = "listing")]
            Self::Listing(p) => p.protected_wire(),
        }
    }
    /// Commitment to every retained field.
    pub const fn digest(&self) -> &[u8; 32] {
        match self {
            #[cfg(feature = "chat")]
            Self::Chat(p) => p.digest(),
            #[cfg(feature = "listing")]
            Self::Listing(p) => p.digest(),
        }
    }
    /// Borrows actual private preimages only.
    #[cfg(feature = "chat")]
    pub const fn chat(&self) -> Option<&crate::chat::ChatSigningPlan> {
        match self {
            Self::Chat(p) => Some(p),
            #[cfg(feature = "listing")]
            Self::Listing(_) => None,
        }
    }
    /// Borrows actual public data only.
    #[cfg(feature = "listing")]
    pub const fn listing(&self) -> Option<&crate::listing::PublicSigningPlan> {
        match self {
            Self::Listing(p) => Some(p),
            #[cfg(feature = "chat")]
            Self::Chat(_) => None,
        }
    }
}
#[cfg(feature = "chat")]
impl From<crate::chat::ChatSigningPlan> for CapturedSigningPlan {
    fn from(p: crate::chat::ChatSigningPlan) -> Self {
        Self::Chat(p)
    }
}
#[cfg(feature = "listing")]
impl From<crate::listing::PublicSigningPlan> for CapturedSigningPlan {
    fn from(p: crate::listing::PublicSigningPlan) -> Self {
        Self::Listing(p)
    }
}
impl fmt::Debug for CapturedSigningPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CapturedSigningPlan(<redacted>)")
    }
}
