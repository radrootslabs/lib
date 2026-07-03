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
mod dvm_runtime;
#[cfg(feature = "runtime")]
mod error;
#[cfg(feature = "runtime")]
mod farm;
#[cfg(feature = "runtime")]
mod farms_runtime;
#[cfg(feature = "runtime")]
mod geonames;
#[cfg(feature = "runtime")]
mod idempotency;
#[cfg(feature = "identity-models")]
pub mod identity;
#[cfg(feature = "runtime")]
mod listings_runtime;
#[cfg(feature = "runtime")]
mod market_runtime;
#[cfg(feature = "runtime")]
mod order;
#[cfg(feature = "runtime")]
mod orders_runtime;
#[cfg(feature = "runtime")]
mod privacy;
#[cfg(feature = "runtime")]
mod private_store;
#[cfg(feature = "runtime")]
mod product_clients;
#[cfg(feature = "runtime")]
mod relay_targets;
#[cfg(feature = "runtime")]
mod runtime;
#[cfg(all(feature = "runtime", feature = "signer-adapters"))]
mod signer_provider;
#[cfg(feature = "runtime")]
mod sync_runtime;
#[cfg(feature = "runtime")]
mod trade_storage;
#[cfg(feature = "runtime")]
mod workflow_runtime;

#[cfg(feature = "runtime")]
pub use crate::dvm_runtime::{
    DVM_TRADE_TRANSITION_PROOF_REQUEST_CONTRACT_ID,
    DVM_TRADE_TRANSITION_PROOF_REQUEST_OPERATION_KIND, DvmProofMode,
    DvmTradeTransitionProofEnqueueRequest, DvmTradeTransitionProofPlan,
    DvmTradeTransitionProofPrepareRequest, DvmTradeTransitionProofReceipt,
    DvmTradeTransitionProofRequestPayload, DvmValidationReceiptIngestReceipt,
    DvmValidationReceiptIngestRequest,
};
#[cfg(feature = "runtime")]
pub use crate::error::{
    RadrootsSdkError, RadrootsSdkErrorClass, RadrootsSdkGeoNamesErrorKind,
    RadrootsSdkPartialLocalMutationError, RadrootsSdkPartialLocalMutationFailure,
    RadrootsSdkRecoveryAction,
};
#[cfg(feature = "runtime")]
pub use crate::farms_runtime::{
    FARM_PUBLISH_OPERATION_KIND, FarmEnqueuePublishRequest, FarmEnqueueReceipt,
    FarmPreparePublishRequest, FarmPrivateLocationClearReceipt, FarmPrivateLocationClearRequest,
    FarmPrivateLocationInput, FarmPrivateLocationLookupCandidate, FarmPrivateLocationLookupReceipt,
    FarmPrivateLocationReceipt, FarmPrivateLocationSetRequest, FarmPrivateLocationSetResult,
    FarmPrivateLocationUpsertRequest, FarmPublishPlan, SdkExactLocation, SdkPublicLocality,
};
#[cfg(feature = "runtime")]
pub use crate::geonames::{
    GEONAMES_1_0_ASSET, GEONAMES_ASSET_BYTE_SIZE, GEONAMES_ASSET_FILE_NAME, GEONAMES_ASSET_HOST,
    GEONAMES_ASSET_SHA256, GEONAMES_ASSET_URL, GEONAMES_ASSET_VERSION, GeoNamesAssetFetcher,
    GeoNamesAssetSpec, GeoNamesAssetState, GeoNamesAssetStatus, GeoNamesBlockingHttpFetcher,
    Geocoder, GeocoderCountryListResult, GeocoderError, GeocoderLocalityCandidate,
    GeocoderLocalityInput, GeocoderLocalityLookup, GeocoderLocalityQuery, GeocoderPoint,
    GeocoderReverseOptions, GeocoderReverseResult, GeocoderStructuredLocalityQuery,
    RadrootsGeoNamesConfig,
};
#[cfg(feature = "runtime")]
pub use crate::idempotency::{
    SDK_IDEMPOTENCY_KEY_MAX_LEN, SdkIdempotencyKey, SdkTradeIdempotencyRecord,
};
#[cfg(feature = "runtime")]
pub use crate::listings_runtime::{
    LISTING_PUBLISH_OPERATION_KIND, ListingEnqueuePublishRequest, ListingEnqueueReceipt,
    ListingPreparePublishRequest, ListingPublishPlan, SdkMutationState,
};
#[cfg(feature = "runtime")]
pub use crate::market_runtime::{
    MARKET_SEARCH_DEFAULT_LIMIT, MarketListingSearchRow, MarketSearchReceipt, MarketSearchRequest,
    MarketSearchSource,
};
#[cfg(feature = "runtime")]
pub use crate::orders_runtime::{
    SdkTradeStatusIssue, SdkTradeStatusIssueKind, SdkTradeStatusSource, TRADE_STATUS_DEFAULT_LIMIT,
    TRADE_STATUS_MAX_LIMIT, TradeEvidenceBranchReceipt, TradeEvidenceIngestReceipt,
    TradeEvidenceIngestRequest, TradeEvidenceQueryBranch, TradeEvidenceQueryBranchKind,
    TradeEvidenceQueryPlan, TradeEvidenceRelayFilter, TradeEvidenceRelayTagFilter,
    TradeRequestEvidenceIngestReceipt, TradeRequestEvidenceIngestRequest,
    TradeResyncEventImportReceipt, TradeResyncEvidenceReceipt, TradeResyncReceipt,
    TradeResyncRelayOutcomeKind, TradeResyncRelayOutcomeReceipt,
    TradeResyncRelayTransportOutcomeKind, TradeResyncRequest, TradeSellerInboxReceipt,
    TradeSellerInboxRequest, TradeStatusAmbiguityCandidate, TradeStatusEligibility,
    TradeStatusEvidenceSummary, TradeStatusKind, TradeStatusNextActionKind, TradeStatusReceipt,
    TradeStatusRequest, TradeValidationReceiptEvent, TradeValidationReceiptInspectReceipt,
    TradeValidationReceiptInspectRequest, TradeValidationReceiptInvalidCandidate,
    TradeValidationReceiptListReceipt, TradeValidationReceiptListRequest,
    TradeValidationReceiptRelayEvidenceReceipt, TradeValidationReceiptRelayOutcomeKind,
    TradeValidationReceiptRelayOutcomeReceipt, TradeValidationReceiptRelayTransportOutcomeKind,
    TradeValidationReceiptTags, TradeValidationReceiptVerifyRequest,
    TradeValidationReceiptWorkerEvidence, TradeValidationReceiptWorkerEvidenceSelection,
};
#[cfg(all(feature = "runtime", feature = "signer-adapters"))]
pub use crate::orders_runtime::{
    TRADE_CANCELLATION_OPERATION_KIND, TRADE_DECISION_OPERATION_KIND,
    TRADE_REVISION_DECISION_OPERATION_KIND, TRADE_REVISION_PROPOSAL_OPERATION_KIND,
    TRADE_SUBMIT_OPERATION_KIND, TradeAcceptRequest, TradeCancelRequest, TradeCancellationPlan,
    TradeCancellationReceipt, TradeDecisionPlan, TradeDecisionReceipt, TradeDeclineRequest,
    TradeMutationOutcome, TradeProposeRequest, TradeRevisionDecisionPlan,
    TradeRevisionDecisionReceipt, TradeRevisionDecisionRequest, TradeRevisionProposalPlan,
    TradeRevisionProposalReceipt, TradeRevisionProposalRequest, TradeSubmitPlan,
    TradeSubmitReceipt, TradeWorkflowEnqueueReceipt, TradeWorkflowIdempotencyReceipt,
    TradeWorkflowKind, TradeWorkflowPlan, TradeWorkflowRetryAdvice,
};
#[cfg(feature = "runtime")]
pub use crate::privacy::{
    PrivacyPreflightConfirmation, PrivacyPreflightReceipt, PrivacyPreflightStatus,
    ProductSensitivityField,
};
#[cfg(all(feature = "runtime", feature = "signer-adapters"))]
pub use crate::product_clients::TradeBuyerClient;
#[cfg(feature = "runtime")]
pub use crate::product_clients::{
    DvmClient, FarmsClient, GeoNamesClient, ListingsClient, MarketClient, SyncClient,
    TradeResyncClient, TradeSellerClient, TradeValidationReceiptsClient, TradesClient,
};
#[cfg(feature = "runtime")]
pub use crate::relay_targets::{
    AckPolicy, PublishMode, RelayResolutionPolicy, SDK_RELAY_TARGET_MAX_COUNT,
    SdkRelayTargetPolicy, SdkRelayTargetSet, SdkRelayUrlPolicy,
};
#[cfg(feature = "runtime")]
pub use crate::runtime::{
    BackupReceipt, BackupRequest, IntegrityReceipt, IntegrityRequest, RadrootsClient,
    RadrootsClientBuilder, RadrootsSdkClock, RadrootsSdkStorageConfig, RadrootsSdkStoragePaths,
    RadrootsSdkTimestamp, RestoreArchive, RestoreReceipt, RestoreRequest, SdkBackupManifest,
    SdkBackupManifestKind, SdkBackupState, SdkBackupVerification, SdkEventStoreStorageStatus,
    SdkOutboxStorageStatus, SdkPrivateStoreStorageStatus, SdkPublishTransport, SdkRestoreState,
    SdkSqliteStoreStatus, SdkStorageKind, StorageStatusReceipt, StorageStatusRequest,
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
    PushOutboxRequest, SYNC_PROJECTION_REFRESH_DEFAULT_LIMIT, SYNC_PROJECTION_REFRESH_MAX_LIMIT,
    SdkRelayAuthPolicy, SyncEventStoreStatus, SyncOutboxStatus, SyncProjectionRefreshReceipt,
    SyncProjectionRefreshRequest, SyncRelayTargetSummary, SyncStatusReceipt, SyncStatusRequest,
    SyncStatusSource,
};
#[cfg(feature = "runtime")]
pub use crate::trade_storage::{
    SDK_TRADE_PROJECTION_CACHE_VERSION, SdkTradeProjectionCache, SdkTradeProjectionCacheKey,
    SdkTradeProjectionCacheRecord,
};
#[cfg(feature = "runtime")]
pub use radroots_trade::dvm::RadrootsTradeInventoryBinWitnessDto;
#[cfg(feature = "runtime")]
pub use radroots_trade::validation_receipt::{
    RadrootsTradeCommitmentConfidence, RadrootsTradeValidationAuthority,
};
