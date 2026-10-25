//! Explicit protected stock grants and authorization checkpoint metadata.
use super::{SellerAuthoritySelection, StockGrantScope};
use crate::workspace::{
    AttachmentInput, AttachmentProof, WorkspaceBinding, WorkspaceError, WorkspaceScope,
    grant::WorkspaceGrant,
};
use radroots_domain::{ActorRef, CheckpointDigest, Sequence, UnixSeconds};
use serde::{Deserialize, Serialize};

/// A claimed protected grant-set observation. It never authenticates a caller.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StockAuthorizationRef {
    generation: Sequence,
    digest: CheckpointDigest,
}
impl<'de> Deserialize<'de> for StockAuthorizationRef {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            generation: Sequence,
            digest: CheckpointDigest,
        }
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = Wire;
            fn expecting(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.write_str("a closed authorization checkpoint object")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(self, map: A) -> Result<Wire, A::Error> {
                Wire::deserialize(serde::de::value::MapAccessDeserializer::new(map))
            }
        }
        let v = deserializer.deserialize_map(Visitor)?;
        Self::from_host_metadata(v.generation, v.digest).map_err(serde::de::Error::custom)
    }
}
impl StockAuthorizationRef {
    pub fn from_host_metadata(
        generation: Sequence,
        digest: CheckpointDigest,
    ) -> Result<Self, WorkspaceError> {
        if generation.get() == 0 {
            return Err(WorkspaceError::InvalidInput);
        }
        Ok(Self { generation, digest })
    }
    pub const fn generation(self) -> Sequence {
        self.generation
    }
    pub const fn digest(self) -> CheckpointDigest {
        self.digest
    }
}

/// Closed stock-only grant. The envelope and current namespace/grant-set must
/// be authenticated by the concrete host before these claims are consumed.
pub struct StockGrantRecord {
    credential: WorkspaceGrant,
    permissions: StockGrantScope,
    administrator: bool,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire<'a> {
    version: u8,
    #[serde(borrow)]
    credential: &'a serde_json::value::RawValue,
    permissions: StockGrantScope,
    administrator: bool,
}
impl StockGrantRecord {
    pub fn new(
        input: AttachmentInput,
        principal: String,
        expires_at: UnixSeconds,
        permissions: StockGrantScope,
        proof: &AttachmentProof,
    ) -> Result<Self, WorkspaceError> {
        let actor = ActorRef::new(
            input.participant(),
            WorkspaceScope::SellerStockOwner.actor_role(),
            None,
            Some(input.grant_id()),
        );
        let credential = WorkspaceGrant::with_scope(
            input,
            WorkspaceScope::SellerStockOwner,
            actor,
            principal,
            expires_at,
            proof,
        )?;
        Ok(Self {
            credential,
            permissions,
            administrator: false,
        })
    }
    /// Only fresh authority enrollment supplies the original administrative grant.
    /// Hosts must never treat construction of this metadata as authorization.
    pub fn from_enrollment(credential: WorkspaceGrant) -> Result<Self, WorkspaceError> {
        if credential.scope() != WorkspaceScope::SellerStockOwner {
            return Err(WorkspaceError::InvalidInput);
        }
        Ok(Self {
            credential,
            permissions: StockGrantScope::full_inventory(),
            administrator: true,
        })
    }
    pub fn encode(&self) -> Result<zeroize::Zeroizing<Vec<u8>>, WorkspaceError> {
        let clear = self.credential.encode()?;
        let credential: &serde_json::value::RawValue =
            serde_json::from_slice(&clear).map_err(|_| WorkspaceError::StorageFailure)?;
        let bytes = zeroize::Zeroizing::new(
            serde_json::to_vec(&Wire {
                version: 2,
                credential,
                permissions: self.permissions.clone(),
                administrator: self.administrator,
            })
            .map_err(|_| WorkspaceError::StorageFailure)?,
        );
        if bytes.len() > 16 * 1024 {
            return Err(WorkspaceError::Capacity);
        }
        Ok(bytes)
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, WorkspaceError> {
        if bytes.len() > 16 * 1024
            || bytes.iter().copied().find(|b| !b.is_ascii_whitespace()) != Some(b'{')
        {
            return Err(WorkspaceError::RecoveryRequired);
        }
        let wire: Wire<'_> =
            serde_json::from_slice(bytes).map_err(|_| WorkspaceError::RecoveryRequired)?;
        if wire.version != 2 {
            return Err(WorkspaceError::RecoveryRequired);
        }
        if !wire.credential.get().starts_with('{') {
            return Err(WorkspaceError::RecoveryRequired);
        }
        let credential = WorkspaceGrant::decode(wire.credential.get().as_bytes())?;
        if credential.scope() != WorkspaceScope::SellerStockOwner
            || (wire.administrator && wire.permissions != StockGrantScope::full_inventory())
        {
            return Err(WorkspaceError::RecoveryRequired);
        }
        Ok(Self {
            credential,
            permissions: wire.permissions,
            administrator: wire.administrator,
        })
    }
    pub fn input(&self) -> &AttachmentInput {
        self.credential.input()
    }
    pub fn expires_at(&self) -> UnixSeconds {
        self.credential.expires_at()
    }
    /// Returns the decoded revocation claim. Current protected provenance and
    /// grant-set membership must be authenticated by the concrete owner.
    pub fn is_revoked(&self) -> bool {
        self.credential.is_revoked()
    }
    pub fn permissions(&self) -> &StockGrantScope {
        &self.permissions
    }
    pub const fn administrator(&self) -> bool {
        self.administrator
    }
    pub fn authenticate(
        &self,
        input: &AttachmentInput,
        proof: &AttachmentProof,
        now: UnixSeconds,
    ) -> Result<WorkspaceBinding, WorkspaceError> {
        self.credential.authenticate(input, proof, now)
    }
    pub fn authenticate_retained_capture(
        &self,
        input: &AttachmentInput,
        proof: &AttachmentProof,
    ) -> Result<(), WorkspaceError> {
        self.credential.authenticate_retained_capture(input, proof)
    }
    pub fn revoke(&mut self) -> Result<(), WorkspaceError> {
        self.credential.revoke()
    }
    pub fn belongs_to(&self, namespace: &SellerAuthoritySelection) -> Result<bool, WorkspaceError> {
        Ok(namespace.matches_attachment(self.credential.input()))
    }
}
impl core::fmt::Debug for StockGrantRecord {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("StockGrantRecord(<redacted>)")
    }
}
