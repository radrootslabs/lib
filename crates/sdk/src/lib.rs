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
mod workflow_runtime;

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
    NetworkConfig, RADROOTS_SDK_DEFAULT_TIMEOUT_MS, RADROOTS_SDK_LOCAL_RADROOTSD_ENDPOINT,
    RADROOTS_SDK_LOCAL_RELAY_URL, RADROOTS_SDK_PRODUCTION_RADROOTSD_ENDPOINT,
    RADROOTS_SDK_PRODUCTION_RELAY_URL, RADROOTS_SDK_STAGING_RADROOTSD_ENDPOINT,
    RADROOTS_SDK_STAGING_RELAY_URL, RadrootsSdkConfig, RadrootsdAuth, RadrootsdConfig, RelayConfig,
    SdkConfigError, SdkEnvironment, SdkTransportMode, SignerConfig,
};
#[cfg(feature = "runtime")]
pub use crate::error::{
    RadrootsSdkError, RadrootsSdkErrorClass, RadrootsSdkPartialLocalMutationError,
    RadrootsSdkPartialLocalMutationFailure, RadrootsSdkRecoveryAction,
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
    BackupReceipt, BackupRequest, IntegrityReceipt, IntegrityRequest, RadrootsSdk,
    RadrootsSdkBuilder, RadrootsSdkClock, RadrootsSdkStorageConfig, RadrootsSdkStoragePaths,
    RadrootsSdkTimestamp, RestoreArchive, RestoreReceipt, RestoreRequest, SdkBackupManifest,
    SdkBackupManifestKind, SdkBackupState, SdkBackupVerification, SdkEventStoreStorageStatus,
    SdkOutboxStorageStatus, SdkRestoreState, SdkSqliteStoreStatus, SdkStorageKind,
    StorageStatusReceipt, StorageStatusRequest,
};
#[cfg(feature = "runtime")]
pub use crate::sync_runtime::{
    PUSH_OUTBOX_DEFAULT_CLAIM_TTL_MS, PUSH_OUTBOX_DEFAULT_LIMIT,
    PUSH_OUTBOX_DEFAULT_NEXT_ATTEMPT_DELAY_MS, PUSH_OUTBOX_MAX_LIMIT, PushOutboxEventReceipt,
    PushOutboxEventState, PushOutboxReceipt, PushOutboxRelayOutcomeKind, PushOutboxRelayReceipt,
    PushOutboxRequest, SdkRelayAuthPolicy, SyncEventStoreStatus, SyncOutboxStatus,
    SyncRelayTargetSummary, SyncStatusReceipt, SyncStatusRequest, SyncStatusSource,
};
