#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(feature = "runtime")]
mod actor_json;
#[cfg(any(
    feature = "radrootsd-proxy",
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
mod geonames;
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
#[cfg(all(feature = "runtime", feature = "signer-adapters"))]
mod signer_provider;
#[cfg(feature = "runtime")]
mod sync_runtime;
#[cfg(feature = "runtime")]
mod workflow_runtime;

#[cfg(feature = "runtime")]
pub use crate::error::{
    RadrootsSdkError, RadrootsSdkErrorClass, RadrootsSdkGeoNamesErrorKind,
    RadrootsSdkPartialLocalMutationError, RadrootsSdkPartialLocalMutationFailure,
    RadrootsSdkRecoveryAction,
};
#[cfg(feature = "runtime")]
pub use crate::farms_runtime::{
    FARM_PUBLISH_OPERATION_KIND, FarmEnqueuePublishRequest, FarmEnqueueReceipt,
    FarmPreparePublishRequest, FarmPublishPlan,
};
#[cfg(feature = "runtime")]
pub use crate::geonames::{
    GEONAMES_1_0_ASSET, GEONAMES_ASSET_BYTE_SIZE, GEONAMES_ASSET_FILE_NAME, GEONAMES_ASSET_HOST,
    GEONAMES_ASSET_SHA256, GEONAMES_ASSET_URL, GEONAMES_ASSET_VERSION, GeoNamesAssetFetcher,
    GeoNamesAssetSpec, GeoNamesAssetState, GeoNamesAssetStatus, GeoNamesBlockingHttpFetcher,
    Geocoder, GeocoderCountryListResult, GeocoderError, GeocoderPoint, GeocoderReverseOptions,
    GeocoderReverseResult, RadrootsGeoNamesConfig,
};
#[cfg(feature = "runtime")]
pub use crate::idempotency::{SDK_IDEMPOTENCY_KEY_MAX_LEN, SdkIdempotencyKey};
#[cfg(feature = "runtime")]
pub use crate::listings_runtime::{
    LISTING_PUBLISH_OPERATION_KIND, ListingEnqueuePublishRequest, ListingEnqueueReceipt,
    ListingPreparePublishRequest, ListingPublishPlan, SdkMutationState,
};
#[cfg(feature = "runtime")]
pub use crate::orders_runtime::{
    ORDER_CANCELLATION_OPERATION_KIND, ORDER_DECISION_OPERATION_KIND,
    ORDER_REVISION_DECISION_OPERATION_KIND, ORDER_REVISION_PROPOSAL_OPERATION_KIND,
    ORDER_STATUS_DEFAULT_LIMIT, ORDER_STATUS_MAX_LIMIT, ORDER_SUBMIT_OPERATION_KIND,
    OrderCancellationEnqueueRequest, OrderCancellationPlan, OrderCancellationPrepareRequest,
    OrderCancellationReceipt, OrderDecisionEnqueueRequest, OrderDecisionPlan,
    OrderDecisionPrepareRequest, OrderDecisionReceipt, OrderEvidenceIngestReceipt,
    OrderEvidenceIngestRequest, OrderRequestEvidenceIngestReceipt,
    OrderRequestEvidenceIngestRequest, OrderRevisionDecisionEnqueueRequest,
    OrderRevisionDecisionPlan, OrderRevisionDecisionPrepareRequest, OrderRevisionDecisionReceipt,
    OrderRevisionProposalEnqueueRequest, OrderRevisionProposalPlan,
    OrderRevisionProposalPrepareRequest, OrderRevisionProposalReceipt, OrderStatusEligibility,
    OrderStatusEvidenceSummary, OrderStatusKind, OrderStatusNextActionKind, OrderStatusReceipt,
    OrderStatusRequest, OrderSubmitEnqueueRequest, OrderSubmitPlan, OrderSubmitPrepareRequest,
    OrderSubmitReceipt, OrderWorkflowEnqueueReceipt, OrderWorkflowIdempotencyReceipt,
    OrderWorkflowKind, OrderWorkflowPlan, OrderWorkflowRetryAdvice, SdkOrderStatusIssue,
    SdkOrderStatusIssueKind, SdkOrderStatusSource,
};
#[cfg(feature = "runtime")]
pub use crate::product_clients::{
    DvmClient, FarmsClient, GeoNamesClient, ListingsClient, MarketClient, SyncClient, TradesClient,
};
#[cfg(feature = "runtime")]
pub use crate::relay_targets::{
    SDK_RELAY_TARGET_MAX_COUNT, SdkRelayTargetPolicy, SdkRelayTargetSet, SdkRelayUrlPolicy,
};
#[cfg(feature = "runtime")]
pub use crate::runtime::{
    BackupReceipt, BackupRequest, IntegrityReceipt, IntegrityRequest, RadrootsClient,
    RadrootsClientBuilder, RadrootsSdkClock, RadrootsSdkStorageConfig, RadrootsSdkStoragePaths,
    RadrootsSdkTimestamp, RestoreArchive, RestoreReceipt, RestoreRequest, SdkBackupManifest,
    SdkBackupManifestKind, SdkBackupState, SdkBackupVerification, SdkEventStoreStorageStatus,
    SdkOutboxStorageStatus, SdkPublishTransport, SdkRestoreState, SdkSqliteStoreStatus,
    SdkStorageKind, StorageStatusReceipt, StorageStatusRequest,
};
#[cfg(all(feature = "runtime", feature = "signer-adapters"))]
pub use crate::signer_provider::{
    RADROOTS_SDK_MYC_NIP46_DEFAULT_REQUEST_TIMEOUT_MS,
    RADROOTS_SDK_MYC_NIP46_PRODUCT_SIGN_EVENT_KINDS, RadrootsSdkLocalKeySigner,
    RadrootsSdkMycNip46RequestPolicy, RadrootsSdkMycNip46Signer, RadrootsSdkNip46Transport,
    RadrootsSdkNip46TransportFuture, RadrootsSdkSignReceipt, RadrootsSdkSignRequest,
    RadrootsSdkSignerCapability, RadrootsSdkSignerMode, RadrootsSdkSignerProgress,
    RadrootsSdkSignerProgressSink, RadrootsSdkSignerProvider, RadrootsSdkSignerState,
    RadrootsSdkSignerStatus, radroots_sdk_myc_nip46_product_permission_strings,
    radroots_sdk_myc_nip46_product_permissions,
};
#[cfg(feature = "runtime")]
pub use crate::sync_runtime::{
    PUSH_OUTBOX_DEFAULT_CLAIM_TTL_MS, PUSH_OUTBOX_DEFAULT_LIMIT,
    PUSH_OUTBOX_DEFAULT_NEXT_ATTEMPT_DELAY_MS, PUSH_OUTBOX_MAX_LIMIT, PushOutboxEventReceipt,
    PushOutboxEventState, PushOutboxReceipt, PushOutboxRelayOutcomeKind, PushOutboxRelayReceipt,
    PushOutboxRequest, SdkRelayAuthPolicy, SyncEventStoreStatus, SyncOutboxStatus,
    SyncRelayTargetSummary, SyncStatusReceipt, SyncStatusRequest, SyncStatusSource,
};
