#![forbid(unsafe_code)]

pub mod client;
pub mod error;
pub mod message;
pub mod method;
pub mod permission;
pub mod uri;

pub mod prelude {
    pub use crate::client::{
        RadrootsNostrConnectClientEventOutcome, RadrootsNostrConnectClientProgress,
        RadrootsNostrConnectClientRequest, RadrootsNostrConnectClientTarget,
        RadrootsNostrConnectClientTransport, RadrootsNostrConnectClientTransportFuture,
        build_request_event, execute_request_with_transport, parse_response_event,
    };
    pub use crate::error::RadrootsNostrConnectError;
    pub use crate::message::{
        RADROOTS_NOSTR_CONNECT_PENDING_CONNECTION_ERROR, RADROOTS_NOSTR_CONNECT_RPC_KIND,
        RadrootsNostrConnectPendingConnectionPollOutcome,
        RadrootsNostrConnectRemoteSessionCapability, RadrootsNostrConnectRequest,
        RadrootsNostrConnectRequestMessage, RadrootsNostrConnectResponse,
        RadrootsNostrConnectResponseEnvelope,
    };
    pub use crate::method::RadrootsNostrConnectMethod;
    pub use crate::permission::{RadrootsNostrConnectPermission, RadrootsNostrConnectPermissions};
    pub use crate::uri::{
        RadrootsNostrConnectBunkerUri, RadrootsNostrConnectClientMetadata,
        RadrootsNostrConnectClientUri, RadrootsNostrConnectUri,
    };
}
