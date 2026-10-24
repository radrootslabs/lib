//! Caller-independent seller namespace metadata, without authority or custody.
use crate::workspace::{AttachmentInput, WorkspaceError};
use radroots_domain::{AuthorityRef, CheckpointRef, GrantId, PublicKey, RouteDigest, WorkspaceId};

/// The source-owned concrete local inventory contract supported by this host.
/// This is neither a public event-kind allocation nor remote pairing evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StockCapabilityContract;
impl StockCapabilityContract {
    pub const fn id(self) -> &'static str {
        "radroots.seller_stock.native_inventory"
    }
    pub const fn version(self) -> u16 {
        1
    }
}
#[cfg(feature = "serde")]
impl serde::Serialize for StockCapabilityContract {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut value = serializer.serialize_struct("StockCapabilityContract", 2)?;
        value.serialize_field("id", self.id())?;
        value.serialize_field("version", "1")?;
        value.end()
    }
}
#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for StockCapabilityContract {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            id: String,
            version: String,
        }
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = Wire;
            fn expecting(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.write_str("the supported native inventory contract object")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(self, map: A) -> Result<Wire, A::Error> {
                <Wire as serde::Deserialize>::deserialize(
                    serde::de::value::MapAccessDeserializer::new(map),
                )
            }
        }
        let v = deserializer.deserialize_map(Visitor)?;
        if v.id != Self.id() || v.version != "1" {
            return Err(serde::de::Error::custom(
                "unsupported native inventory contract",
            ));
        }
        Ok(Self)
    }
}
/// Exact namespace, authority and route selection. A caller grant is supplied
/// separately. This metadata authenticates neither an endpoint nor a writer,
/// and its checkpoint establishes no independently protected provenance.
#[derive(Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct SellerAuthoritySelection {
    capability_contract: StockCapabilityContract,
    workspace_id: WorkspaceId,
    authority: AuthorityRef,
    participant: PublicKey,
    endpoint: Box<str>,
    endpoint_identity: PublicKey,
    checkpoint: CheckpointRef,
    route_digest: RouteDigest,
}
impl SellerAuthoritySelection {
    /// Preserves all validated namespace fields while omitting the caller grant.
    pub fn from_attachment(input: &AttachmentInput) -> Self {
        Self {
            capability_contract: StockCapabilityContract,
            workspace_id: input.workspace_id(),
            authority: input.authority(),
            participant: input.participant(),
            endpoint: input.endpoint().into(),
            endpoint_identity: input.endpoint_identity(),
            checkpoint: input.checkpoint().clone(),
            route_digest: input.route_digest(),
        }
    }
    /// Selects a claimed caller grant; this creates no credential or permission.
    pub fn attachment(&self, grant: GrantId) -> Result<AttachmentInput, WorkspaceError> {
        AttachmentInput::new(
            self.workspace_id,
            self.authority,
            self.participant,
            self.endpoint.to_string(),
            self.endpoint_identity,
            grant,
            self.checkpoint.clone(),
            self.route_digest,
        )
    }
    /// Compares complete namespace metadata independently of the caller grant.
    pub fn matches_attachment(&self, input: &AttachmentInput) -> bool {
        self.workspace_id == input.workspace_id()
            && self.authority == input.authority()
            && self.participant == input.participant()
            && self.endpoint.as_ref() == input.endpoint()
            && self.endpoint_identity == input.endpoint_identity()
            && &self.checkpoint == input.checkpoint()
            && self.route_digest == input.route_digest()
    }
    pub const fn capability_contract(&self) -> StockCapabilityContract {
        self.capability_contract
    }
    /// Selected private namespace identity.
    pub const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }
    /// Selected executor and authority generation.
    pub const fn authority(&self) -> AuthorityRef {
        self.authority
    }
    /// Declared stock owner, without signer or actor proof.
    pub const fn participant(&self) -> PublicKey {
        self.participant
    }
    /// Explicit selected route endpoint, without pairing or endpoint trust.
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }
    /// Pinned claimed application identity.
    pub const fn endpoint_identity(&self) -> PublicKey {
        self.endpoint_identity
    }
    /// Required checkpoint claim, without custody or coverage provenance.
    pub const fn checkpoint(&self) -> &CheckpointRef {
        &self.checkpoint
    }
    /// Exact declared route commitment.
    pub const fn route_digest(&self) -> RouteDigest {
        self.route_digest
    }
}
impl core::fmt::Debug for SellerAuthoritySelection {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("SellerAuthoritySelection(<untrusted descriptor>)")
    }
}
#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for SellerAuthoritySelection {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            capability_contract: StockCapabilityContract,
            workspace_id: WorkspaceId,
            authority: AuthorityRef,
            participant: PublicKey,
            endpoint: String,
            endpoint_identity: PublicKey,
            checkpoint: CheckpointRef,
            route_digest: RouteDigest,
        }
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = Wire;
            fn expecting(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.write_str("a closed seller namespace object")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(self, map: A) -> Result<Wire, A::Error> {
                <Wire as serde::Deserialize>::deserialize(
                    serde::de::value::MapAccessDeserializer::new(map),
                )
            }
        }
        let v = deserializer.deserialize_map(Visitor)?;
        if v.endpoint.is_empty()
            || v.endpoint.len() > 2048
            || v.endpoint.chars().any(char::is_control)
        {
            return Err(serde::de::Error::custom(
                "invalid seller namespace selection",
            ));
        }
        Ok(Self {
            capability_contract: v.capability_contract,
            workspace_id: v.workspace_id,
            authority: v.authority,
            participant: v.participant,
            endpoint: v.endpoint.into_boxed_str(),
            endpoint_identity: v.endpoint_identity,
            checkpoint: v.checkpoint,
            route_digest: v.route_digest,
        })
    }
}
