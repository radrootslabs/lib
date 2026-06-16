#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
extern crate alloc;

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
mod farm;
#[cfg(feature = "runtime")]
mod idempotency;
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
mod relay_targets;
#[cfg(feature = "runtime")]
mod runtime;
#[cfg(feature = "runtime")]
mod sync_runtime;

#[cfg(feature = "runtime")]
pub use crate::error::{
    RadrootsSdkError, RadrootsSdkPartialLocalMutationError, RadrootsSdkPartialLocalMutationFailure,
    RadrootsSdkRecoveryAction,
};
#[cfg(feature = "runtime")]
pub use crate::idempotency::{SDK_IDEMPOTENCY_KEY_MAX_LEN, SdkIdempotencyKey};
#[cfg(feature = "runtime")]
pub use crate::listings_runtime::{
    ListingEnqueuePublishRequest, ListingEnqueueReceipt, ListingPreparePublishRequest,
    ListingPublishPlan, SdkMutationState,
};
#[cfg(feature = "runtime")]
pub use crate::orders_runtime::{
    ORDER_STATUS_DEFAULT_LIMIT, ORDER_STATUS_MAX_LIMIT, OrderFulfillmentStatusKind,
    OrderPaymentStateKind, OrderSettlementStateKind, OrderStatusKind, OrderStatusReceipt,
    OrderStatusRequest, SdkOrderStatusIssue, SdkOrderStatusIssueKind, SdkOrderStatusSource,
};
#[cfg(feature = "runtime")]
pub use crate::product_clients::{ListingsClient, OrdersClient, SyncClient};
#[cfg(feature = "runtime")]
pub use crate::relay_targets::{
    SDK_RELAY_TARGET_MAX_COUNT, SdkRelayTargetPolicy, SdkRelayTargetSet, SdkRelayUrlPolicy,
};
#[cfg(feature = "runtime")]
pub use crate::runtime::{
    RadrootsSdk, RadrootsSdkBuilder, RadrootsSdkClock, RadrootsSdkStorageConfig,
    RadrootsSdkStoragePaths, RadrootsSdkTimestamp,
};
#[cfg(feature = "runtime")]
pub use crate::sync_runtime::{
    PUSH_OUTBOX_DEFAULT_LIMIT, PUSH_OUTBOX_MAX_LIMIT, PushOutboxEventReceipt, PushOutboxEventState,
    PushOutboxReceipt, PushOutboxRelayOutcomeKind, PushOutboxRelayReceipt, PushOutboxRequest,
};
