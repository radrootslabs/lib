#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![forbid(unsafe_code)]
#![deny(rustdoc::broken_intra_doc_links)]
#![doc = include_str!("../README.md")]

pub mod client;
pub mod error;
pub mod message;
pub mod method;
pub mod permission;
pub mod server;
pub mod uri;

pub use client::Client;
pub use error::RadrootsNostrConnectError as Error;
pub use message::{Request, Response};
pub use method::Method;
pub use permission::Permission;
pub use server::Server;
pub use uri::{BunkerUri, ClientUri};

/// Private migration surface for separate first-party repositories.
///
/// This module is excluded from the reviewed public API baseline, the package
/// is enabled only for package-realistic validation, consumers cut over in
/// Steps 271, 288, and 293, and Step 313 removes the shim in full.
#[doc(hidden)]
pub mod prelude {
    pub use crate::client::{
        CancellationPhase, CancellationToken, Client, ClientEvent, Completion, EventOutcome,
        Operation, Progress, RadrootsNostrConnectClientEventOutcome,
        RadrootsNostrConnectClientProgress, RadrootsNostrConnectClientRequest,
        RadrootsNostrConnectClientTarget, RadrootsNostrConnectClientTransport,
        RadrootsNostrConnectClientTransportFuture, Receive, Target, Transport, TransportFuture,
        build_request_event, execute_request_with_transport, parse_response_event,
    };
    pub use crate::error::RadrootsNostrConnectError;
    pub use crate::message::{
        PENDING_CONNECTION_ERROR, PendingConnectionOutcome, REMOTE_CAPABILITY_RELAY_COUNT_MAX,
        REQUEST_ID_MAX_BYTES, REQUEST_PARAM_COUNT_MAX, REQUEST_PARAM_MAX_BYTES,
        REQUEST_PARAMS_MAX_BYTES, RESPONSE_ERROR_MAX_BYTES, RESPONSE_RESULT_MAX_BYTES, RPC_KIND,
        RemoteSessionCapability, Request, RequestId, RequestMessage, Response, ResponseEnvelope,
        ResponseValidator, SignedEvent, UnsignedEvent,
    };
    pub use crate::message::{
        PENDING_CONNECTION_ERROR as RADROOTS_NOSTR_CONNECT_PENDING_CONNECTION_ERROR,
        PendingConnectionOutcome as RadrootsNostrConnectPendingConnectionPollOutcome,
        RPC_KIND as RADROOTS_NOSTR_CONNECT_RPC_KIND,
        RemoteSessionCapability as RadrootsNostrConnectRemoteSessionCapability,
        Request as RadrootsNostrConnectRequest,
        RequestMessage as RadrootsNostrConnectRequestMessage,
        Response as RadrootsNostrConnectResponse,
        ResponseEnvelope as RadrootsNostrConnectResponseEnvelope,
    };
    pub use crate::method::{Method, Method as RadrootsNostrConnectMethod};
    pub use crate::permission::{
        Permission, Permission as RadrootsNostrConnectPermission, Permissions,
        Permissions as RadrootsNostrConnectPermissions,
    };
    pub use crate::uri::{
        BUNKER_URI_SCHEME, BUNKER_URI_SCHEME as RADROOTS_NOSTR_CONNECT_BUNKER_URI_SCHEME,
        BunkerUri, BunkerUri as RadrootsNostrConnectBunkerUri, CLIENT_METADATA_JSON_MAX_BYTES,
        CLIENT_METADATA_JSON_MAX_BYTES as RADROOTS_NOSTR_CONNECT_CLIENT_METADATA_JSON_MAX_BYTES,
        CLIENT_NAME_MAX_BYTES,
        CLIENT_NAME_MAX_BYTES as RADROOTS_NOSTR_CONNECT_CLIENT_NAME_MAX_BYTES,
        CLIENT_URL_MAX_BYTES, CLIENT_URL_MAX_BYTES as RADROOTS_NOSTR_CONNECT_CLIENT_URL_MAX_BYTES,
        ClientMetadata, ClientMetadata as RadrootsNostrConnectClientMetadata, ClientUri,
        ClientUri as RadrootsNostrConnectClientUri, URI_SCHEME,
        URI_SCHEME as RADROOTS_NOSTR_CONNECT_URI_SCHEME, Uri, Uri as RadrootsNostrConnectUri,
    };
}
