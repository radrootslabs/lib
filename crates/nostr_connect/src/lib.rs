#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![forbid(unsafe_code)]

pub mod client;
pub mod error;
pub mod message;
pub mod method;
pub mod permission;
pub mod server;
pub mod uri;

pub use error::RadrootsNostrConnectError as Error;
pub use method::Method;
pub use permission::Permission;
pub use uri::{BunkerUri, ClientUri};

// Transitional compatibility surface for consumers migrated in Step 141.
// Publication remains disabled, and Step 143 removes this module.
#[doc(hidden)]
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
    pub use crate::method::Method;
    pub use crate::method::Method as RadrootsNostrConnectMethod;
    pub use crate::permission::{Permission, Permissions};
    pub use crate::permission::{
        Permission as RadrootsNostrConnectPermission,
        Permissions as RadrootsNostrConnectPermissions,
    };
    pub use crate::uri::{
        BUNKER_URI_SCHEME, BunkerUri, CLIENT_METADATA_JSON_MAX_BYTES, CLIENT_NAME_MAX_BYTES,
        CLIENT_URL_MAX_BYTES, ClientMetadata, ClientUri, URI_SCHEME, Uri,
    };
    pub use crate::uri::{
        BUNKER_URI_SCHEME as RADROOTS_NOSTR_CONNECT_BUNKER_URI_SCHEME,
        BunkerUri as RadrootsNostrConnectBunkerUri,
        CLIENT_METADATA_JSON_MAX_BYTES as RADROOTS_NOSTR_CONNECT_CLIENT_METADATA_JSON_MAX_BYTES,
        CLIENT_NAME_MAX_BYTES as RADROOTS_NOSTR_CONNECT_CLIENT_NAME_MAX_BYTES,
        CLIENT_URL_MAX_BYTES as RADROOTS_NOSTR_CONNECT_CLIENT_URL_MAX_BYTES,
        ClientMetadata as RadrootsNostrConnectClientMetadata,
        ClientUri as RadrootsNostrConnectClientUri,
        URI_SCHEME as RADROOTS_NOSTR_CONNECT_URI_SCHEME, Uri as RadrootsNostrConnectUri,
    };
}
