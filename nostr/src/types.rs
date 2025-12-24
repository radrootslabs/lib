#![forbid(unsafe_code)]

pub type RadrootsNostrCoordinate = nostr::nips::nip01::Coordinate;
pub type RadrootsNostrEvent = nostr::Event;
pub type RadrootsNostrEventBuilder = nostr::EventBuilder;
pub type RadrootsNostrEventId = nostr::EventId;
pub type RadrootsNostrFilter = nostr::Filter;
pub type RadrootsNostrKind = nostr::Kind;
pub type RadrootsNostrKeys = nostr::Keys;
pub type RadrootsNostrMetadata = nostr::Metadata;
pub type RadrootsNostrPublicKey = nostr::PublicKey;
pub type RadrootsNostrRelayUrl = nostr::RelayUrl;
pub type RadrootsNostrSecretKey = nostr::SecretKey;
pub type RadrootsNostrSubscriptionId = nostr::SubscriptionId;
pub type RadrootsNostrTag = nostr::Tag;
pub type RadrootsNostrTagKind<'a> = nostr::TagKind<'a>;
pub type RadrootsNostrTagStandard = nostr::TagStandard;
pub type RadrootsNostrTimestamp = nostr::Timestamp;
pub type RadrootsNostrUrl = nostr::Url;

pub use nostr::nips::nip19::{
    FromBech32 as RadrootsNostrFromBech32,
    ToBech32 as RadrootsNostrToBech32,
};
pub use nostr::secp256k1::SecretKey as RadrootsNostrSecp256k1SecretKey;

#[cfg(feature = "client")]
pub type RadrootsNostrMonitor = nostr_sdk::prelude::Monitor;

#[cfg(feature = "client")]
pub type RadrootsNostrMonitorNotification = nostr_sdk::prelude::MonitorNotification;

#[cfg(feature = "client")]
pub type RadrootsNostrOutput<T> = nostr_sdk::prelude::Output<T>;

#[cfg(feature = "client")]
pub type RadrootsNostrRelay = nostr_sdk::Relay;

#[cfg(feature = "client")]
pub type RadrootsNostrRelayPoolNotification = nostr_sdk::RelayPoolNotification;

#[cfg(feature = "client")]
pub type RadrootsNostrRelayStatus = nostr_sdk::RelayStatus;

#[cfg(feature = "client")]
pub type RadrootsNostrSubscribeAutoCloseOptions = nostr_sdk::SubscribeAutoCloseOptions;
