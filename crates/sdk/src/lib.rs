#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};
#[cfg(feature = "std")]
use std::{string::String, vec::Vec};

pub(crate) use radroots_events::{
    RadrootsNostrEvent, RadrootsNostrEventPtr,
    profile::{RadrootsProfile, RadrootsProfileType},
};
pub(crate) use radroots_events_codec::wire::WireEventParts;
pub(crate) use radroots_trade::listing::validation::RadrootsTradeListing as TradeListingValidateResult;

#[cfg(any(
    feature = "radrootsd-client",
    feature = "signing",
    feature = "relay-client",
    feature = "signer-adapters"
))]
mod adapters;
mod client;
mod config;
#[cfg(feature = "runtime")]
mod error;
mod farm;
#[cfg(feature = "identity-models")]
mod identity;
mod listing;
#[cfg(feature = "runtime")]
mod listings_runtime;
mod order;
#[cfg(feature = "runtime")]
mod orders_runtime;
#[cfg(feature = "runtime")]
mod product_clients;
mod profile;
pub mod protocol;
#[cfg(feature = "runtime")]
mod receipt;
#[cfg(feature = "runtime")]
mod runtime;
#[cfg(feature = "runtime")]
mod runtime_targets;
#[cfg(feature = "runtime")]
mod sync_runtime;

#[cfg(feature = "runtime")]
pub use crate::error::{
    RadrootsSdkError, RadrootsSdkPartialLocalMutationError, RadrootsSdkRecoveryAction,
};
#[cfg(feature = "runtime")]
pub use crate::listings_runtime::{
    ListingEnqueueReceipt, ListingPublishRequest, PreparedListingPublish,
};
#[cfg(feature = "runtime")]
pub use crate::orders_runtime::{
    ORDER_STATUS_DEFAULT_LIMIT, ORDER_STATUS_MAX_LIMIT, OrderFulfillmentStatusKind,
    OrderPaymentStateKind, OrderSettlementStateKind, OrderStatusKind, OrderStatusReceipt,
    OrderStatusRequest,
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
#[cfg(feature = "runtime")]
pub use crate::runtime_targets::{
    SDK_IDEMPOTENCY_KEY_MAX_LEN, SDK_RELAY_TARGET_MAX_COUNT, SdkIdempotencyKey,
    SdkRelayTargetPolicy, SdkRelayTargetSet,
};
#[cfg(feature = "runtime")]
pub use crate::sync_runtime::{
    PUSH_OUTBOX_DEFAULT_LIMIT, PUSH_OUTBOX_MAX_LIMIT, PushOutboxEventReceipt, PushOutboxEventState,
    PushOutboxReceipt, PushOutboxRelayOutcomeKind, PushOutboxRelayReceipt, PushOutboxRequest,
};

pub(crate) type NostrTags = Vec<Vec<String>>;
