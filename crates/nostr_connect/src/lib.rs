#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
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
        RADROOTS_NOSTR_CONNECT_CLIENT_METADATA_JSON_MAX_BYTES,
        RADROOTS_NOSTR_CONNECT_CLIENT_NAME_MAX_BYTES, RADROOTS_NOSTR_CONNECT_CLIENT_URL_MAX_BYTES,
        RadrootsNostrConnectBunkerUri, RadrootsNostrConnectClientMetadata,
        RadrootsNostrConnectClientUri, RadrootsNostrConnectUri,
    };
}
