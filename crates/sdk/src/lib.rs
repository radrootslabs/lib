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
pub mod farm;
#[cfg(feature = "identity-models")]
pub mod identity;
pub mod listing;
pub mod profile;
pub mod trade;

#[cfg(feature = "radrootsd-client")]
pub use crate::adapters::radrootsd::{
    SdkRadrootsdBridgeDeliveryPolicy, SdkRadrootsdBridgeJobStatus,
    SdkRadrootsdBridgeRelayPublishResult, SdkRadrootsdPublicTradePublishValidationError,
    SdkRadrootsdPublicTradeRoute, SdkRadrootsdSignerAuthority,
    SdkRadrootsdSignerSessionConnectRequest, SdkRadrootsdSignerSessionMode,
    SdkRadrootsdSignerSessionRole, SdkRadrootsdTradeChain,
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
    SdkRadrootsdPublicTradeMessage, SdkRadrootsdPublicTradePublishOptions,
    SdkRadrootsdSessionError, SdkRadrootsdSignerSessionAuthorizeResult,
    SdkRadrootsdSignerSessionCloseResult, SdkRadrootsdSignerSessionHandle,
    SdkRadrootsdSignerSessionRef, SdkRadrootsdSignerSessionRequireAuthResult,
    SdkRadrootsdSignerSessionView,
};
pub use crate::config::{
    NetworkConfig, RADROOTS_SDK_LOCAL_RADROOTSD_ENDPOINT, RADROOTS_SDK_LOCAL_RELAY_URL,
    RADROOTS_SDK_PRODUCTION_RADROOTSD_ENDPOINT, RADROOTS_SDK_PRODUCTION_RELAY_URL,
    RADROOTS_SDK_STAGING_RADROOTSD_ENDPOINT, RADROOTS_SDK_STAGING_RELAY_URL, RadrootsSdkConfig,
    RadrootsdAuth, RadrootsdConfig, RelayConfig, SdkConfigError, SdkEnvironment, SdkTransportMode,
    SignerConfig,
};
pub use radroots_events::{
    RadrootsNostrEvent, RadrootsNostrEventPtr, RadrootsNostrEventRef,
    farm::RadrootsFarm,
    listing::RadrootsListing,
    profile::{RadrootsProfile, RadrootsProfileType},
    trade::{RadrootsTradeMessagePayload, RadrootsTradeMessageType},
};
#[cfg(feature = "serde_json")]
pub use radroots_events_codec::trade::{
    RadrootsTradeEnvelopeParseError, RadrootsTradeListingAddress, RadrootsTradeListingAddressError,
};
pub use radroots_events_codec::wire::{EventDraft as UnsignedEventDraft, WireEventParts};
pub use radroots_trade::listing::validation::RadrootsTradeListing as TradeListingValidateResult;

pub type NostrTags = Vec<Vec<String>>;
pub type RadrootsTradeEnvelope =
    radroots_events::trade::RadrootsTradeEnvelope<RadrootsTradeMessagePayload>;
