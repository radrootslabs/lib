#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};
#[cfg(feature = "std")]
use std::{string::String, vec::Vec};

#[cfg(any(
    feature = "radrootsd-client",
    feature = "signing",
    feature = "relay-client",
    feature = "signer-adapters"
))]
pub mod adapters;
pub mod client;
pub mod config;
#[cfg(feature = "runtime")]
mod error;
pub mod farm;
#[cfg(feature = "identity-models")]
pub mod identity;
pub mod listing;
#[cfg(feature = "runtime")]
mod listings_runtime;
pub mod order;
#[cfg(feature = "runtime")]
mod product_clients;
pub mod profile;
#[cfg(feature = "runtime")]
mod receipt;
#[cfg(feature = "runtime")]
mod runtime;

#[cfg(feature = "radrootsd-client")]
pub use crate::adapters::radrootsd::{
    SdkRadrootsdBridgeDeliveryPolicy, SdkRadrootsdBridgeJobStatus,
    SdkRadrootsdBridgeRelayPublishResult, SdkRadrootsdSignerAuthority,
    SdkRadrootsdSignerSessionConnectRequest, SdkRadrootsdSignerSessionMode,
    SdkRadrootsdSignerSessionRole,
};
pub use crate::client::{
    FarmClient, ListingClient, ProfileClient, RadrootsSdkClient, SdkPublishError,
    SdkPublishReceipt, SdkRadrootsdPublishReceipt, SdkRelayFailure, SdkRelayPublishReceipt,
    SdkResolvedTransportTarget, SdkTransportReceipt, TradeClient,
};
#[cfg(feature = "radrootsd-client")]
pub use crate::client::{
    RadrootsdBridgeClient, RadrootsdClient, RadrootsdSignerSessionClient, SdkRadrootsdBridgeError,
    SdkRadrootsdBridgeJobRef, SdkRadrootsdBridgeJobView, SdkRadrootsdBridgeStatus,
    SdkRadrootsdFarmPublishOptions, SdkRadrootsdListingPublishOptions,
    SdkRadrootsdOrderRequestPublishOptions, SdkRadrootsdProfilePublishOptions,
    SdkRadrootsdSessionError, SdkRadrootsdSignerSessionAuthorizeResult,
    SdkRadrootsdSignerSessionCloseResult, SdkRadrootsdSignerSessionHandle,
    SdkRadrootsdSignerSessionPublicKeyResult, SdkRadrootsdSignerSessionRef,
    SdkRadrootsdSignerSessionRequireAuthResult, SdkRadrootsdSignerSessionView,
};
pub use crate::config::{
    NetworkConfig, RADROOTS_SDK_LOCAL_RADROOTSD_ENDPOINT, RADROOTS_SDK_LOCAL_RELAY_URL,
    RADROOTS_SDK_PRODUCTION_RADROOTSD_ENDPOINT, RADROOTS_SDK_PRODUCTION_RELAY_URL,
    RADROOTS_SDK_STAGING_RADROOTSD_ENDPOINT, RADROOTS_SDK_STAGING_RELAY_URL, RadrootsSdkConfig,
    RadrootsdAuth, RadrootsdConfig, RelayConfig, SdkConfigError, SdkEnvironment, SdkTransportMode,
    SignerConfig,
};
#[cfg(feature = "runtime")]
pub use crate::error::{
    RadrootsSdkError, RadrootsSdkPartialLocalMutationError, RadrootsSdkRecoveryAction,
};
#[cfg(feature = "runtime")]
pub use crate::listings_runtime::{
    ListingEnqueueReceipt, ListingPublishRequest, PreparedListingPublish,
};
#[cfg(feature = "runtime")]
pub use crate::product_clients::{ListingsClient, OrdersClient, SyncClient};
#[cfg(feature = "runtime")]
pub use crate::receipt::{RadrootsSdkEventReference, RadrootsSdkLocalMutationReceipt};
#[cfg(feature = "runtime")]
pub use crate::runtime::{
    RadrootsSdk, RadrootsSdkBuilder, RadrootsSdkClock, RadrootsSdkStorageConfig,
    RadrootsSdkStoragePaths, RadrootsSdkTimestamp,
};
pub use radroots_events::{
    RadrootsNostrEvent, RadrootsNostrEventPtr, RadrootsNostrEventRef,
    draft::{RadrootsFrozenEventDraft, RadrootsSignedNostrEvent},
    farm::RadrootsFarm,
    ids::{RadrootsIdParseError, RadrootsListingAddress},
    listing::RadrootsListing,
    profile::{RadrootsProfile, RadrootsProfileType},
};
#[cfg(feature = "serde_json")]
pub use radroots_events_codec::order::RadrootsOrderEnvelopeParseError;
pub use radroots_events_codec::wire::WireEventParts;
pub use radroots_trade::listing::validation::RadrootsTradeListing as TradeListingValidateResult;

pub type NostrTags = Vec<Vec<String>>;
