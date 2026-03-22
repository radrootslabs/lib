#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod error;
pub mod uri;
pub mod version;
pub mod wire;

pub mod prelude {
    pub use crate::error::RadrootsSimplexSmpProtoError;
    pub use crate::uri::{
        RADROOTS_SIMPLEX_SMP_DEFAULT_PORT, RADROOTS_SIMPLEX_SMP_URI_SCHEME,
        RadrootsSimplexSmpQueueMode, RadrootsSimplexSmpQueueUri, RadrootsSimplexSmpServerAddress,
    };
    pub use crate::version::{
        RADROOTS_SIMPLEX_SMP_AUTH_COMMANDS_TRANSPORT_VERSION,
        RADROOTS_SIMPLEX_SMP_BLOCKED_ENTITY_TRANSPORT_VERSION,
        RADROOTS_SIMPLEX_SMP_CURRENT_CLIENT_VERSION,
        RADROOTS_SIMPLEX_SMP_CURRENT_TRANSPORT_VERSION,
        RADROOTS_SIMPLEX_SMP_DELETED_EVENT_TRANSPORT_VERSION,
        RADROOTS_SIMPLEX_SMP_ENCRYPTED_BLOCK_TRANSPORT_VERSION,
        RADROOTS_SIMPLEX_SMP_INITIAL_CLIENT_VERSION,
        RADROOTS_SIMPLEX_SMP_INITIAL_TRANSPORT_VERSION,
        RADROOTS_SIMPLEX_SMP_NEW_NOTIFIER_CREDENTIALS_TRANSPORT_VERSION,
        RADROOTS_SIMPLEX_SMP_PROXY_SERVER_HANDSHAKE_TRANSPORT_VERSION,
        RADROOTS_SIMPLEX_SMP_SENDER_AUTH_KEY_CLIENT_VERSION,
        RADROOTS_SIMPLEX_SMP_SENDER_AUTH_KEY_TRANSPORT_VERSION,
        RADROOTS_SIMPLEX_SMP_SENDING_PROXY_TRANSPORT_VERSION,
        RADROOTS_SIMPLEX_SMP_SERVER_HOSTNAMES_CLIENT_VERSION,
        RADROOTS_SIMPLEX_SMP_SERVICE_CERTS_TRANSPORT_VERSION,
        RADROOTS_SIMPLEX_SMP_SHORT_LINKS_CLIENT_VERSION,
        RADROOTS_SIMPLEX_SMP_SHORT_LINKS_TRANSPORT_VERSION, RadrootsSimplexSmpVersionRange,
    };
    pub use crate::wire::{
        RadrootsSimplexSmpBlockingInfo, RadrootsSimplexSmpBlockingReason,
        RadrootsSimplexSmpBrokerError, RadrootsSimplexSmpBrokerMessage,
        RadrootsSimplexSmpBrokerTransmission, RadrootsSimplexSmpCertChainPublicKey,
        RadrootsSimplexSmpCommand, RadrootsSimplexSmpCommandError,
        RadrootsSimplexSmpCommandTransmission, RadrootsSimplexSmpContactQueueRequest,
        RadrootsSimplexSmpCorrelationId, RadrootsSimplexSmpError, RadrootsSimplexSmpHandshakeError,
        RadrootsSimplexSmpKeyList, RadrootsSimplexSmpMessageFlags,
        RadrootsSimplexSmpMessagingQueueRequest, RadrootsSimplexSmpNetworkError,
        RadrootsSimplexSmpNewNotifierCredentials, RadrootsSimplexSmpNewQueueRequest,
        RadrootsSimplexSmpNotifierIdsResponse, RadrootsSimplexSmpProtocolServer,
        RadrootsSimplexSmpProxyError, RadrootsSimplexSmpQueueIdsResponse,
        RadrootsSimplexSmpQueueLinkData, RadrootsSimplexSmpQueueRequestData,
        RadrootsSimplexSmpReceivedMessage, RadrootsSimplexSmpSendCommand,
        RadrootsSimplexSmpServerNotifierCredentials, RadrootsSimplexSmpSubscriptionMode,
        RadrootsSimplexSmpTransportError,
    };
}
