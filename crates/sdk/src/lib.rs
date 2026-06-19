#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(feature = "runtime")]
mod actor_json;
#[cfg(any(
    feature = "radrootsd-client",
    feature = "signing",
    feature = "relay-client",
    feature = "signer-adapters"
))]
pub mod adapters;
#[cfg(feature = "runtime")]
mod error;
mod farm;
#[cfg(feature = "runtime")]
mod farms_runtime;
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

#[cfg(feature = "runtime")]
pub use crate::error::{
    RadrootsSdkError, RadrootsSdkErrorClass, RadrootsSdkPartialLocalMutationError,
    RadrootsSdkPartialLocalMutationFailure, RadrootsSdkRecoveryAction,
};
#[cfg(feature = "runtime")]
pub use crate::farms_runtime::{
    FARM_PUBLISH_OPERATION_KIND, FarmEnqueuePublishRequest, FarmEnqueueReceipt,
    FarmPreparePublishRequest, FarmPublishPlan,
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
    ORDER_CANCELLATION_OPERATION_KIND, ORDER_DECISION_OPERATION_KIND,
    ORDER_FULFILLMENT_UPDATE_OPERATION_KIND, ORDER_RECEIPT_RECORD_OPERATION_KIND,
    ORDER_REVISION_DECISION_OPERATION_KIND, ORDER_REVISION_PROPOSAL_OPERATION_KIND,
    ORDER_STATUS_DEFAULT_LIMIT, ORDER_STATUS_MAX_LIMIT, ORDER_SUBMIT_OPERATION_KIND,
    OrderCancellationEnqueueRequest, OrderCancellationPlan, OrderCancellationPrepareRequest,
    OrderCancellationReceipt, OrderDecisionEnqueueRequest, OrderDecisionPlan,
    OrderDecisionPrepareRequest, OrderDecisionReceipt, OrderEvidenceIngestReceipt,
    OrderEvidenceIngestRequest, OrderFulfillmentStatusKind, OrderFulfillmentUpdateEnqueueRequest,
    OrderFulfillmentUpdatePlan, OrderFulfillmentUpdatePrepareRequest,
    OrderFulfillmentUpdateReceipt, OrderPaymentStateKind, OrderReceiptRecordEnqueueRequest,
    OrderReceiptRecordPlan, OrderReceiptRecordPrepareRequest, OrderReceiptRecordReceipt,
    OrderRequestEvidenceIngestReceipt, OrderRequestEvidenceIngestRequest,
    OrderRevisionDecisionEnqueueRequest, OrderRevisionDecisionPlan,
    OrderRevisionDecisionPrepareRequest, OrderRevisionDecisionReceipt,
    OrderRevisionProposalEnqueueRequest, OrderRevisionProposalPlan,
    OrderRevisionProposalPrepareRequest, OrderRevisionProposalReceipt, OrderSettlementStateKind,
    OrderStatusKind, OrderStatusReceipt, OrderStatusRequest, OrderSubmitEnqueueRequest,
    OrderSubmitPlan, OrderSubmitPrepareRequest, OrderSubmitReceipt, SdkOrderStatusIssue,
    SdkOrderStatusIssueKind, SdkOrderStatusSource,
};
#[cfg(feature = "runtime")]
pub use crate::product_clients::{FarmsClient, ListingsClient, OrdersClient, SyncClient};
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
