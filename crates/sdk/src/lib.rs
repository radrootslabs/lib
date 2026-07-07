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
#[cfg(feature = "knowledge")]
pub mod knowledge;
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
mod runtime;
#[cfg(all(feature = "runtime", feature = "signer-adapters"))]
mod signer_provider;
#[cfg(feature = "runtime")]
mod sync_runtime;
#[cfg(feature = "runtime")]
mod trade_storage;
#[cfg(feature = "runtime")]
pub mod transport;
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
#[cfg(feature = "knowledge")]
pub use crate::knowledge::{
    KIND_FILE_METADATA, KIND_KNOWLEDGE_CLAIM, KIND_KNOWLEDGE_FIELD_REPORT, KIND_KNOWLEDGE_RELATION,
    KIND_KNOWLEDGE_REVIEW, KIND_KNOWLEDGE_SOURCE, KIND_WIKI_ARTICLE, KIND_WIKI_MERGE_REQUEST,
    KIND_WIKI_REDIRECT, KNOWLEDGE_CLAIM_CONTRACT_ID, KNOWLEDGE_FIELD_REPORT_CONTRACT_ID,
    KNOWLEDGE_RELATION_CONTRACT_ID, KNOWLEDGE_REVIEW_CONTRACT_ID, KNOWLEDGE_SOURCE_CONTRACT_ID,
    KnowledgeCodec, KnowledgeDraftBuilder, KnowledgeEventBuilder,
    RADROOTS_CONTRIBUTION_ATTESTATION_SCHEMA, RADROOTS_EVIDENCE_BOUNTY_SCHEMA,
    RADROOTS_KNOWLEDGE_CHANGE_PROPOSAL_SCHEMA, RADROOTS_KNOWLEDGE_CLAIM_SCHEMA,
    RADROOTS_KNOWLEDGE_CONTRACT_MANIFEST_SCHEMA_VERSION, RADROOTS_KNOWLEDGE_FIELD_REPORT_SCHEMA,
    RADROOTS_KNOWLEDGE_RELATION_SCHEMA, RADROOTS_KNOWLEDGE_REVIEW_SCHEMA,
    RADROOTS_KNOWLEDGE_SCHEMA_VERSION, RADROOTS_KNOWLEDGE_SOURCE_SCHEMA,
    RADROOTS_WIKI_D_TAG_MAX_LEN, RadrootsAddressableRef, RadrootsContractValidatedEvent,
    RadrootsDecodeError, RadrootsDecodedEvent, RadrootsDraftError, RadrootsEncodeError,
    RadrootsFrozenEventDraft, RadrootsIdVerifiedEvent, RadrootsKnowledgeBuilderError,
    RadrootsKnowledgeChangeProposal, RadrootsKnowledgeCitationSpan, RadrootsKnowledgeClaim,
    RadrootsKnowledgeClaimBuilder, RadrootsKnowledgeContractManifest,
    RadrootsKnowledgeContractManifestEntry, RadrootsKnowledgeFieldContext,
    RadrootsKnowledgeFieldReport, RadrootsKnowledgeFieldReportBuilder, RadrootsKnowledgeLocation,
    RadrootsKnowledgeLocationPrecision, RadrootsKnowledgeManifestCodecSupport,
    RadrootsKnowledgeManifestDiscriminator, RadrootsKnowledgeManifestTagContract,
    RadrootsKnowledgeNodeRef, RadrootsKnowledgeObservation, RadrootsKnowledgeObservationValue,
    RadrootsKnowledgeRelation, RadrootsKnowledgeRelationBuilder, RadrootsKnowledgeReview,
    RadrootsKnowledgeReviewBuilder, RadrootsKnowledgeReviewScope, RadrootsKnowledgeReviewScore,
    RadrootsKnowledgeReviewTarget, RadrootsKnowledgeSource, RadrootsKnowledgeSourceBuilder,
    RadrootsNip01VerificationError, RadrootsNostrEvent, RadrootsNostrEventRef,
    RadrootsRightsAssertion, RadrootsSdkKnowledgeError, RadrootsSignatureVerifiedEvent,
    RadrootsWikiArticle, RadrootsWikiArticleBuilder, RadrootsWikiArticleVersionRef,
    RadrootsWikiDTagError, RadrootsWikiMergeRequest, RadrootsWikiMergeRequestBuilder,
    RadrootsWikiRedirect, RadrootsWikiRedirectBuilder, WIKI_ARTICLE_CONTRACT_ID,
    WIKI_MERGE_REQUEST_CONTRACT_ID, WIKI_REDIRECT_CONTRACT_ID, WireEventParts,
    build_knowledge_claim_event, build_knowledge_field_report_event,
    build_knowledge_relation_event, build_knowledge_review_event, build_knowledge_source_event,
    build_wiki_article_event, build_wiki_merge_request_event, build_wiki_redirect_event,
    contract_manifest, contract_manifest_json, contract_manifest_sha256, normalize_wiki_d_tag,
    prepare_knowledge_claim_draft, prepare_knowledge_field_report_draft,
    prepare_knowledge_relation_draft, prepare_knowledge_review_draft,
    prepare_knowledge_source_draft, prepare_wiki_article_draft, prepare_wiki_merge_request_draft,
    prepare_wiki_redirect_draft, validate_wiki_d_tag, verify_and_decode_radroots_event,
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
    TRADE_STATUS_MAX_LIMIT, TRADE_STATUS_ROOT_SELECTOR_SEPARATOR,
    TRADE_STATUS_WATCH_DEFAULT_CAPACITY, TRADE_STATUS_WATCH_DEFAULT_REFRESH_INTERVAL_MS,
    TRADE_STATUS_WATCH_MAX_CAPACITY, TRADE_STATUS_WATCH_MAX_REFRESH_INTERVAL_MS,
    TradeEvidenceBranchReceipt, TradeEvidenceIngestReceipt, TradeEvidenceIngestRequest,
    TradeEvidenceQueryBranch, TradeEvidenceQueryBranchKind, TradeEvidenceQueryPlan,
    TradeEvidenceRelayFilter, TradeEvidenceRelayTagFilter, TradeRequestEvidenceIngestReceipt,
    TradeRequestEvidenceIngestRequest, TradeResyncEventImportReceipt, TradeResyncEvidenceReceipt,
    TradeResyncReceipt, TradeResyncRelayOutcomeKind, TradeResyncRelayOutcomeReceipt,
    TradeResyncRelayTransportOutcomeKind, TradeResyncRequest, TradeSellerInboxReceipt,
    TradeSellerInboxRequest, TradeStatusAmbiguityCandidate, TradeStatusEligibility,
    TradeStatusEvidenceSummary, TradeStatusKind, TradeStatusNextActionKind, TradeStatusReceipt,
    TradeStatusRequest, TradeStatusWatch, TradeStatusWatchCancelReceipt,
    TradeStatusWatchCancelState, TradeStatusWatchRequest, TradeStatusWatchUpdate,
    TradeValidationReceiptEvent, TradeValidationReceiptInspectReceipt,
    TradeValidationReceiptInspectRequest, TradeValidationReceiptInvalidCandidate,
    TradeValidationReceiptListReceipt, TradeValidationReceiptListRequest,
    TradeValidationReceiptRelayEvidenceReceipt, TradeValidationReceiptRelayOutcomeKind,
    TradeValidationReceiptRelayOutcomeReceipt, TradeValidationReceiptRelayTransportOutcomeKind,
    TradeValidationReceiptTags, TradeValidationReceiptVerifyRequest,
    TradeValidationReceiptWorkerEvidence, TradeValidationReceiptWorkerEvidenceSelection,
    TradeValidationTrustDecision,
};
#[cfg(all(feature = "runtime", feature = "signer-adapters"))]
pub use crate::orders_runtime::{
    TRADE_CANCELLATION_OPERATION_KIND, TRADE_DECISION_OPERATION_KIND,
    TRADE_REVISION_DECISION_OPERATION_KIND, TRADE_REVISION_PROPOSAL_OPERATION_KIND,
    TRADE_SUBMIT_OPERATION_KIND, TradeAcceptRequest, TradeCancelRequest, TradeCancellationPlan,
    TradeCancellationReceipt, TradeDecisionPlan, TradeDecisionReceipt, TradeDeclineRequest,
    TradeEvidenceMode, TradeMutationOutcome, TradeProposeRequest, TradeRevisionDecisionPlan,
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
pub use crate::runtime::{
    BackupReceipt, BackupRequest, IntegrityReceipt, IntegrityRequest, RadrootsClient,
    RadrootsClientBuilder, RadrootsSdkClock, RadrootsSdkStorageConfig, RadrootsSdkStoragePaths,
    RadrootsSdkTimestamp, RestoreArchive, RestoreReceipt, RestoreRequest, SdkBackupManifest,
    SdkBackupManifestKind, SdkBackupState, SdkBackupVerification, SdkEventStoreStorageStatus,
    SdkOutboxStorageStatus, SdkPrivateStoreStorageStatus, SdkRestoreState, SdkSqliteStoreStatus,
    SdkSqliteWalCheckpointReceipt, SdkSqliteWalStatus, SdkStorageKind, StorageCheckpointReceipt,
    StorageCheckpointRequest, StorageStatusReceipt, StorageStatusRequest,
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
    PushOutboxEventState, PushOutboxReceipt, PushOutboxRequest, PushOutboxTargetOutcomeKind,
    PushOutboxTargetReceipt, SYNC_PROJECTION_REFRESH_DEFAULT_LIMIT,
    SYNC_PROJECTION_REFRESH_MAX_LIMIT, SdkRelayAuthPolicy, SyncEventStoreStatus, SyncOutboxStatus,
    SyncProjectionRefreshReceipt, SyncProjectionRefreshRequest, SyncStatusReceipt,
    SyncStatusRequest, SyncStatusSource, SyncTransportProfileSummary,
};
#[cfg(feature = "runtime")]
pub use crate::trade_storage::{
    SDK_TRADE_PROJECTION_CACHE_VERSION, SdkTradeProjectionCache, SdkTradeProjectionCacheKey,
    SdkTradeProjectionCacheRecord,
};
#[cfg(feature = "runtime")]
pub use crate::transport::{
    HybridProfile, NostrProfile, NostrRelayUrlPolicy, ProxyAuth, ProxyProfile, PublishMode,
    ReticulumPreviewBehavior, ReticulumPreviewProfile, SDK_TRANSPORT_TARGET_MAX_COUNT,
    SatisfactionPolicy, TargetPolicy, TargetSet, TransportDeliveryReceipt,
    TransportDeliveryTargetStatus, TransportKind, TransportOutcome, TransportProfile,
    TransportReceipt, TransportTargetReceipt,
};
#[cfg(feature = "runtime")]
pub use radroots_trade::dvm::RadrootsTradeInventoryBinWitnessDto;
#[cfg(feature = "runtime")]
pub use radroots_trade::validation_receipt::{
    RadrootsTradeCommitmentConfidence, RadrootsTradeValidationAuthority,
    RadrootsTradeValidationTrustPolicy, RadrootsTradeValidationTrustState,
};
