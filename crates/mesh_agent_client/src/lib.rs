#![forbid(unsafe_code)]

use radroots_mesh::{
    RADROOTS_MESH_RETICULUM_POLICY_ID, RADROOTS_MESH_UNAVAILABLE_MESSAGE, RadrootsMeshPayloadPolicy,
};
use radroots_mesh_agent_proto::{
    RADROOTS_MESH_AGENT_SCHEMA_ID, RADROOTS_MESH_AGENT_SCHEMA_NAMESPACE, schema_sha256_hex,
};
use radroots_transport::{
    RADROOTS_RETICULUM_ENDPOINT_URI, RadrootsTransportKind, RadrootsTransportMeshScopeId,
};

pub const RADROOTS_MESH_AGENT_CLIENT_SCHEMA_ID: &str = RADROOTS_MESH_AGENT_SCHEMA_ID;
pub const RADROOTS_MESH_AGENT_CLIENT_SCHEMA_NAMESPACE: &str = RADROOTS_MESH_AGENT_SCHEMA_NAMESPACE;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MeshAgentStatusRequest {
    pub include_transports: bool,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MeshAgentStatusResponse {
    pub transports: Vec<MeshAgentTransportStatus>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MeshAgentTransportStatus {
    pub transport: MeshAgentTransportKind,
    pub profile_id: String,
    pub endpoint_uri: String,
    pub configured: bool,
    pub implementation: MeshAgentImplementation,
    pub maturity: MeshAgentCapabilityMaturity,
    pub availability: MeshAgentCapabilityAvailability,
    pub usable_for_delivery: bool,
    pub message: String,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MeshAgentImplementation {
    Real,
    Mock,
}

impl MeshAgentImplementation {
    pub fn schema_name(self) -> &'static str {
        match self {
            Self::Real => "real",
            Self::Mock => "mock",
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MeshAgentCapabilityMaturity {
    Preview,
    Stable,
}

impl MeshAgentCapabilityMaturity {
    pub fn schema_name(self) -> &'static str {
        match self {
            Self::Preview => "preview",
            Self::Stable => "stable",
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MeshAgentCapabilityAvailability {
    Available,
    Degraded,
    Unavailable,
}

impl MeshAgentCapabilityAvailability {
    pub fn schema_name(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::Degraded => "degraded",
            Self::Unavailable => "unavailable",
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MeshAgentTransportKind {
    Reticulum,
}

impl MeshAgentTransportKind {
    pub fn schema_name(self) -> &'static str {
        match self {
            Self::Reticulum => "reticulum",
        }
    }

    pub fn transport_kind(self) -> RadrootsTransportKind {
        match self {
            Self::Reticulum => RadrootsTransportKind::Reticulum,
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MeshAgentPublishRequest {
    pub publish_request_id: String,
    pub payload_cbor: Vec<u8>,
    pub event_id: String,
    pub target_fingerprint: String,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MeshAgentPublishResponse {
    pub publish_request_id: String,
    pub status: MeshAgentResponseStatus,
    pub transport_receipts: Vec<MeshAgentTransportReceipt>,
    pub event_id: String,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MeshAgentResponseStatus {
    Accepted,
    Deferred,
    Rejected,
}

impl MeshAgentResponseStatus {
    pub fn schema_name(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::Deferred => "deferred",
            Self::Rejected => "rejected",
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MeshAgentTransportReceipt {
    pub transport_kind: MeshAgentTransportKind,
    pub endpoint_uri: String,
    pub outcome: MeshAgentTransportOutcome,
    pub message: String,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MeshAgentTransportOutcome {
    Accepted,
    Delivered,
    Forwarded,
    StoredByGateway,
    DeferredUntilImplemented,
    Rejected,
    RouteUnavailable,
    Timeout,
    TransportUnavailable,
}

impl MeshAgentTransportOutcome {
    pub fn schema_name(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::Delivered => "delivered",
            Self::Forwarded => "forwarded",
            Self::StoredByGateway => "storedByGateway",
            Self::DeferredUntilImplemented => "deferredUntilImplemented",
            Self::Rejected => "rejected",
            Self::RouteUnavailable => "routeUnavailable",
            Self::Timeout => "timeout",
            Self::TransportUnavailable => "transportUnavailable",
        }
    }

    pub fn is_success(self) -> bool {
        matches!(
            self,
            Self::Accepted | Self::Delivered | Self::Forwarded | Self::StoredByGateway
        )
    }
}

pub trait RadrootsMeshAgentClient {
    fn status(&self, request: MeshAgentStatusRequest) -> MeshAgentStatusResponse;
    fn publish(&self, request: MeshAgentPublishRequest) -> MeshAgentPublishResponse;
    fn schema_sha256_hex(&self) -> String;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsMockMeshAgentClient {
    profile_id: String,
    endpoint_uri: String,
    scope: RadrootsTransportMeshScopeId,
    policy: RadrootsMeshPayloadPolicy,
}

impl RadrootsMockMeshAgentClient {
    pub fn reticulum_unavailable() -> Self {
        Self {
            profile_id: RADROOTS_MESH_RETICULUM_POLICY_ID.to_owned(),
            endpoint_uri: RADROOTS_RETICULUM_ENDPOINT_URI.to_owned(),
            scope: RadrootsTransportMeshScopeId::local_reticulum(),
            policy: RadrootsMeshPayloadPolicy::reticulum_unavailable(),
        }
    }

    pub fn scope(&self) -> &RadrootsTransportMeshScopeId {
        &self.scope
    }

    pub fn policy(&self) -> &RadrootsMeshPayloadPolicy {
        &self.policy
    }
}

impl Default for RadrootsMockMeshAgentClient {
    fn default() -> Self {
        Self::reticulum_unavailable()
    }
}

impl RadrootsMeshAgentClient for RadrootsMockMeshAgentClient {
    fn status(&self, request: MeshAgentStatusRequest) -> MeshAgentStatusResponse {
        let transports = if request.include_transports {
            vec![MeshAgentTransportStatus {
                transport: MeshAgentTransportKind::Reticulum,
                profile_id: self.profile_id.clone(),
                endpoint_uri: self.endpoint_uri.clone(),
                configured: true,
                implementation: MeshAgentImplementation::Real,
                maturity: MeshAgentCapabilityMaturity::Preview,
                availability: MeshAgentCapabilityAvailability::Unavailable,
                usable_for_delivery: false,
                message: RADROOTS_MESH_UNAVAILABLE_MESSAGE.to_owned(),
            }]
        } else {
            Vec::new()
        };
        MeshAgentStatusResponse { transports }
    }

    fn publish(&self, request: MeshAgentPublishRequest) -> MeshAgentPublishResponse {
        MeshAgentPublishResponse {
            publish_request_id: request.publish_request_id,
            status: MeshAgentResponseStatus::Rejected,
            transport_receipts: vec![MeshAgentTransportReceipt {
                transport_kind: MeshAgentTransportKind::Reticulum,
                endpoint_uri: self.endpoint_uri.clone(),
                outcome: MeshAgentTransportOutcome::TransportUnavailable,
                message: RADROOTS_MESH_UNAVAILABLE_MESSAGE.to_owned(),
            }],
            event_id: request.event_id,
        }
    }

    fn schema_sha256_hex(&self) -> String {
        schema_sha256_hex()
    }
}
