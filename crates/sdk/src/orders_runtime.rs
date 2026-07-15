#[cfg(feature = "signer-adapters")]
use crate::TradeBuyerClient;
#[cfg(all(feature = "runtime", feature = "transport-nostr-runtime"))]
use crate::runtime::sdk_now_ms;
#[cfg(feature = "runtime")]
use crate::sync_runtime::SyncProjectionRefreshReceipt;
#[cfg(all(feature = "runtime", feature = "transport-nostr-runtime"))]
use crate::sync_runtime::{SyncProjectionRefreshRequest, refresh_product_projections_for_sdk};
#[cfg(feature = "signer-adapters")]
use crate::workflow_runtime::enqueue_configured_signed_workflow;
#[cfg(feature = "signer-adapters")]
use crate::{
    PrivacyPreflightConfirmation, PrivacyPreflightReceipt, ProductSensitivityField, PublishMode,
    PushOutboxReceipt, PushOutboxRequest, RadrootsSdkRecoveryAction, SatisfactionPolicy,
    SdkIdempotencyKey, SdkMutationState, TargetPolicy, workflow_runtime::SdkWorkflowEnqueueRequest,
};
#[cfg(feature = "runtime")]
use crate::{
    RadrootsSdkError, RadrootsSdkTimestamp, TradeResyncClient, TradeSellerClient,
    TradeValidationReceiptsClient, TradesClient, order,
};
#[cfg(feature = "runtime")]
use radroots_authority::RadrootsActorContext;
#[cfg(feature = "signer-adapters")]
use radroots_event::wire::RadrootsNip01EventWireParts;
#[cfg(feature = "runtime")]
use radroots_event::{
    RadrootsEventEnvelope, RadrootsEventEnvelopeParts,
    contract::RadrootsActorRole,
    draft::RadrootsSignedEvent,
    ids::RadrootsEventId,
    kinds::{
        KIND_ORDER_CANCELLATION, KIND_ORDER_DECISION, KIND_ORDER_REQUEST,
        KIND_TRADE_VALIDATION_RECEIPT,
    },
    tags::TAG_P,
};
#[cfg(feature = "signer-adapters")]
use radroots_event::{
    RadrootsEventPtr,
    draft::RadrootsEventDraft,
    order::{
        RadrootsOrderCancellation, RadrootsOrderDecision, RadrootsOrderDecisionOutcome,
        RadrootsOrderInventoryCommitment, RadrootsOrderItem, RadrootsOrderRequest,
    },
};
#[cfg(any(feature = "runtime", feature = "signer-adapters", test))]
use radroots_event::{
    ids::{RadrootsListingAddress, RadrootsOrderId, RadrootsPublicKey},
    order::RadrootsOrderEconomics,
};
#[cfg(all(feature = "runtime", feature = "transport-nostr-runtime"))]
use radroots_event::{
    kinds::KIND_LISTING,
    tags::{TAG_D, TAG_E},
};
#[cfg(feature = "runtime")]
use radroots_event_codec::order::{
    order_cancellation_from_event, order_decision_from_event, order_request_from_event,
};
#[cfg(all(feature = "runtime", feature = "transport-nostr-runtime"))]
use radroots_event_store::RadrootsStoredEventTag;
#[cfg(feature = "runtime")]
use radroots_event_store::{RadrootsEventIngest, RadrootsStoredEvent};
#[cfg(all(feature = "runtime", feature = "transport-nostr-runtime"))]
use radroots_nostr::prelude::{
    RadrootsNostrEventId, RadrootsNostrFilter, RadrootsNostrKind, RadrootsNostrPublicKey,
    radroots_nostr_filter_tag,
};
#[cfg(feature = "runtime")]
use radroots_trade::identity::{RadrootsTradeLocator, RadrootsTradeLocatorCandidate};
#[cfg(all(feature = "runtime", feature = "transport-nostr-runtime"))]
use radroots_trade::listing::parse_listing_address;
#[cfg(feature = "signer-adapters")]
use radroots_trade::order::{
    RadrootsOrderCanonicalizationError, RadrootsOrderProjectionQueryResult,
    canonicalize_order_decision_for_signer, canonicalize_order_request_for_signer,
    order_projection_query_for_order_id,
};
#[cfg(feature = "runtime")]
use radroots_trade::order::{
    RadrootsOrderEventRecord, RadrootsOrderIssue, RadrootsOrderProjection,
    RadrootsOrderStoreQueryError, RadrootsTradeLocatorProjectionQueryResult,
    RadrootsTradeLocatorProjectionResolution, order_event_record_from_event,
    order_projection_query_for_trade_locator,
};
#[cfg(all(feature = "runtime", feature = "transport-nostr-runtime"))]
use radroots_trade::validation_receipt::RadrootsValidationReceiptError;
#[cfg(feature = "runtime")]
use radroots_trade::validation_receipt::{
    RadrootsTradeCommitmentConfidence, RadrootsTradeValidationAuthority,
    RadrootsTradeValidationReceipt, RadrootsTradeValidationTrustPolicy,
    RadrootsTradeValidationTrustState, RadrootsValidationReceiptExpectedBinding,
    RadrootsValidationReceiptProofSystem, RadrootsValidationReceiptResult,
    RadrootsValidationReceiptTags, verify_validation_receipt_event,
};
#[cfg(feature = "runtime")]
use radroots_trade::workflow::RadrootsTradeWorkflowState;
#[cfg(all(feature = "runtime", feature = "transport-nostr-runtime"))]
use radroots_transport_nostr::{
    RadrootsNostrClientFetchAdapter, RadrootsRelayFetchAdapter, RadrootsRelayFetchEventReceipt,
    RadrootsRelayFetchOutcomeKind, RadrootsRelayFetchReceipt, RadrootsRelayFetchRelayOutcome,
    RadrootsRelayFetchRequest, RadrootsRelayOutcomeKind, fetch_and_ingest_relay_events,
};
#[cfg(feature = "runtime")]
use serde::ser::SerializeStruct;
#[cfg(all(feature = "runtime", feature = "transport-nostr-runtime"))]
use std::collections::{BTreeMap, BTreeSet};
#[cfg(feature = "runtime")]
use std::time::Duration;
#[cfg(feature = "runtime")]
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinHandle,
};
#[cfg(feature = "runtime")]
pub const TRADE_STATUS_DEFAULT_LIMIT: u32 = 500;
#[cfg(feature = "runtime")]
pub const TRADE_STATUS_MAX_LIMIT: u32 = 1_000;
#[cfg(feature = "runtime")]
pub const TRADE_STATUS_ROOT_SELECTOR_SEPARATOR: char = '@';
#[cfg(feature = "runtime")]
pub const TRADE_STATUS_WATCH_DEFAULT_CAPACITY: usize = 8;
#[cfg(feature = "runtime")]
pub const TRADE_STATUS_WATCH_MAX_CAPACITY: usize = 128;
#[cfg(feature = "runtime")]
pub const TRADE_STATUS_WATCH_DEFAULT_REFRESH_INTERVAL_MS: u64 = 1_000;
#[cfg(feature = "runtime")]
pub const TRADE_STATUS_WATCH_MAX_REFRESH_INTERVAL_MS: u64 = 60_000;
#[cfg(feature = "signer-adapters")]
pub const TRADE_SUBMIT_OPERATION_KIND: &str = "trade.submit.v1";
#[cfg(feature = "signer-adapters")]
pub const TRADE_DECISION_OPERATION_KIND: &str = "trade.decision.v1";
#[cfg(feature = "signer-adapters")]
pub const TRADE_CANCELLATION_OPERATION_KIND: &str = "trade.cancellation.v1";

#[cfg(feature = "signer-adapters")]
const TRADE_SUBMIT_CONTRACT_ID: &str = "radroots.order.request.v1";
#[cfg(feature = "signer-adapters")]
const TRADE_DECISION_CONTRACT_ID: &str = "radroots.order.decision.v1";
#[cfg(feature = "signer-adapters")]
const TRADE_CANCELLATION_CONTRACT_ID: &str = "radroots.order.cancellation.v1";

#[cfg(feature = "signer-adapters")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum TradeWorkflowKind {
    Submit,
    Decision,
    Cancellation,
}

#[cfg(feature = "signer-adapters")]
impl TradeWorkflowKind {
    pub fn operation_kind(self) -> &'static str {
        match self {
            Self::Submit => TRADE_SUBMIT_OPERATION_KIND,
            Self::Decision => TRADE_DECISION_OPERATION_KIND,
            Self::Cancellation => TRADE_CANCELLATION_OPERATION_KIND,
        }
    }

    pub fn contract_id(self) -> &'static str {
        match self {
            Self::Submit => TRADE_SUBMIT_CONTRACT_ID,
            Self::Decision => TRADE_DECISION_CONTRACT_ID,
            Self::Cancellation => TRADE_CANCELLATION_CONTRACT_ID,
        }
    }
}

#[cfg(feature = "signer-adapters")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct TradeWorkflowPlan {
    pub kind: TradeWorkflowKind,
    pub operation_kind: &'static str,
    pub contract_id: &'static str,
    pub expected_event_id: RadrootsEventId,
    pub created_at: RadrootsSdkTimestamp,
}

#[cfg(feature = "signer-adapters")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct TradeWorkflowEnqueueReceipt {
    pub kind: TradeWorkflowKind,
    pub operation_kind: &'static str,
    pub expected_event_id: RadrootsEventId,
    pub signed_event_id: RadrootsEventId,
    pub local_event_seq: i64,
    pub outbox_operation_id: i64,
    pub outbox_event_id: i64,
    pub state: SdkMutationState,
    pub idempotency_digest_prefix: Option<String>,
    pub idempotency: TradeWorkflowIdempotencyReceipt,
    pub retry: TradeWorkflowRetryAdvice,
}

#[cfg(feature = "signer-adapters")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct TradeWorkflowIdempotencyReceipt {
    pub digest_prefix: Option<String>,
    pub replayed_existing_operation: bool,
    pub safe_to_retry_with_same_idempotency_key: bool,
}

#[cfg(feature = "signer-adapters")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct TradeWorkflowRetryAdvice {
    pub retryable_after_error: bool,
    pub safe_to_retry_enqueue_with_same_idempotency_key: bool,
    pub recovery_actions: Vec<RadrootsSdkRecoveryAction>,
}

#[cfg(feature = "signer-adapters")]
#[derive(Clone, Debug, serde::Serialize)]
#[non_exhaustive]
pub struct TradeSubmitPrepareRequest {
    #[serde(serialize_with = "crate::actor_json::serialize_actor_context")]
    pub actor: RadrootsActorContext,
    pub listing_event: RadrootsEventPtr,
    pub order: RadrootsOrderRequest,
    pub created_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(feature = "signer-adapters")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct TradeSubmitPlan {
    pub workflow: TradeWorkflowPlan,
    pub order_id: RadrootsOrderId,
    pub listing_addr: RadrootsListingAddress,
    pub buyer_pubkey: RadrootsPublicKey,
    pub seller_pubkey: RadrootsPublicKey,
    pub listing_event_id: RadrootsEventId,
    pub expected_event_id: RadrootsEventId,
    pub frozen_draft: RadrootsEventDraft,
    pub created_at: RadrootsSdkTimestamp,
}

#[cfg(feature = "signer-adapters")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct TradeSubmitReceipt {
    pub workflow: TradeWorkflowEnqueueReceipt,
    pub locator: RadrootsTradeLocator,
    pub order_id: RadrootsOrderId,
    pub listing_addr: RadrootsListingAddress,
    pub buyer_pubkey: RadrootsPublicKey,
    pub seller_pubkey: RadrootsPublicKey,
    pub listing_event_id: RadrootsEventId,
    pub expected_event_id: RadrootsEventId,
    pub signed_event_id: RadrootsEventId,
    pub local_event_seq: i64,
    pub outbox_operation_id: i64,
    pub outbox_event_id: i64,
    pub state: SdkMutationState,
    pub idempotency_digest_prefix: Option<String>,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, serde::Serialize)]
#[non_exhaustive]
pub struct TradeRequestEvidenceIngestRequest {
    pub event: RadrootsSignedEvent,
    pub observed_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(feature = "runtime")]
impl TradeRequestEvidenceIngestRequest {
    pub fn new(event: RadrootsSignedEvent) -> Self {
        Self {
            event,
            observed_at: None,
        }
    }

    pub fn with_observed_at(mut self, observed_at: RadrootsSdkTimestamp) -> Self {
        self.observed_at = Some(observed_at);
        self
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct TradeRequestEvidenceIngestReceipt {
    pub order_id: RadrootsOrderId,
    pub listing_addr: RadrootsListingAddress,
    pub buyer_pubkey: RadrootsPublicKey,
    pub seller_pubkey: RadrootsPublicKey,
    pub request_event_id: RadrootsEventId,
    pub local_event_seq: i64,
    pub inserted: bool,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, serde::Serialize)]
#[non_exhaustive]
pub struct TradeEvidenceIngestRequest {
    pub event: RadrootsSignedEvent,
    pub observed_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(feature = "runtime")]
impl TradeEvidenceIngestRequest {
    pub fn new(event: RadrootsSignedEvent) -> Self {
        Self {
            event,
            observed_at: None,
        }
    }

    pub fn with_observed_at(mut self, observed_at: RadrootsSdkTimestamp) -> Self {
        self.observed_at = Some(observed_at);
        self
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct TradeEvidenceIngestReceipt {
    pub order_id: RadrootsOrderId,
    pub listing_addr: RadrootsListingAddress,
    pub event_id: RadrootsEventId,
    pub event_kind: u32,
    pub local_event_seq: i64,
    pub inserted: bool,
}

#[cfg(feature = "signer-adapters")]
#[derive(Clone, Debug, serde::Serialize)]
#[non_exhaustive]
pub struct TradeDecisionPrepareRequest {
    #[serde(serialize_with = "crate::actor_json::serialize_actor_context")]
    pub actor: RadrootsActorContext,
    pub request_event: RadrootsEventPtr,
    pub decision: RadrootsOrderDecision,
    pub created_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(feature = "signer-adapters")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct TradeDecisionPlan {
    pub workflow: TradeWorkflowPlan,
    pub order_id: RadrootsOrderId,
    pub listing_addr: RadrootsListingAddress,
    pub buyer_pubkey: RadrootsPublicKey,
    pub seller_pubkey: RadrootsPublicKey,
    pub request_event_id: RadrootsEventId,
    pub expected_event_id: RadrootsEventId,
    pub frozen_draft: RadrootsEventDraft,
    pub created_at: RadrootsSdkTimestamp,
}

#[cfg(feature = "signer-adapters")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct TradeDecisionReceipt {
    pub workflow: TradeWorkflowEnqueueReceipt,
    pub locator: RadrootsTradeLocator,
    pub order_id: RadrootsOrderId,
    pub listing_addr: RadrootsListingAddress,
    pub buyer_pubkey: RadrootsPublicKey,
    pub seller_pubkey: RadrootsPublicKey,
    pub request_event_id: RadrootsEventId,
    pub expected_event_id: RadrootsEventId,
    pub signed_event_id: RadrootsEventId,
    pub local_event_seq: i64,
    pub outbox_operation_id: i64,
    pub outbox_event_id: i64,
    pub state: SdkMutationState,
    pub idempotency_digest_prefix: Option<String>,
}

#[cfg(feature = "signer-adapters")]
#[derive(Clone, Debug, serde::Serialize)]
#[non_exhaustive]
pub struct TradeCancellationPrepareRequest {
    #[serde(serialize_with = "crate::actor_json::serialize_actor_context")]
    pub actor: RadrootsActorContext,
    pub root_event: RadrootsEventPtr,
    pub previous_event: RadrootsEventPtr,
    pub cancellation: RadrootsOrderCancellation,
    pub created_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(feature = "signer-adapters")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct TradeCancellationPlan {
    pub workflow: TradeWorkflowPlan,
    pub order_id: RadrootsOrderId,
    pub listing_addr: RadrootsListingAddress,
    pub buyer_pubkey: RadrootsPublicKey,
    pub seller_pubkey: RadrootsPublicKey,
    pub root_event_id: RadrootsEventId,
    pub previous_event_id: RadrootsEventId,
    pub expected_event_id: RadrootsEventId,
    pub frozen_draft: RadrootsEventDraft,
    pub created_at: RadrootsSdkTimestamp,
}

#[cfg(feature = "signer-adapters")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct TradeCancellationReceipt {
    pub workflow: TradeWorkflowEnqueueReceipt,
    pub locator: RadrootsTradeLocator,
    pub order_id: RadrootsOrderId,
    pub listing_addr: RadrootsListingAddress,
    pub buyer_pubkey: RadrootsPublicKey,
    pub seller_pubkey: RadrootsPublicKey,
    pub root_event_id: RadrootsEventId,
    pub previous_event_id: RadrootsEventId,
    pub expected_event_id: RadrootsEventId,
    pub signed_event_id: RadrootsEventId,
    pub local_event_seq: i64,
    pub outbox_operation_id: i64,
    pub outbox_event_id: i64,
    pub state: SdkMutationState,
    pub idempotency_digest_prefix: Option<String>,
}

#[cfg(feature = "signer-adapters")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case", tag = "mode")]
pub enum TradeMutationOutcome<Plan, Receipt> {
    DryRun {
        plan: Plan,
    },
    Enqueued {
        receipt: Receipt,
    },
    Published {
        receipt: Receipt,
        publish: PushOutboxReceipt,
    },
}

#[cfg(feature = "signer-adapters")]
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum TradeEvidenceMode {
    LocalOnly,
    ResyncBeforeMutation,
    RequireExplicitEvidence {
        evidence: Vec<TradeEvidenceIngestRequest>,
    },
}

#[cfg(feature = "signer-adapters")]
impl TradeEvidenceMode {
    pub fn require_explicit_evidence(
        evidence: impl IntoIterator<Item = TradeEvidenceIngestRequest>,
    ) -> Self {
        Self::RequireExplicitEvidence {
            evidence: evidence.into_iter().collect(),
        }
    }
}

#[cfg(feature = "signer-adapters")]
#[derive(Clone, Debug, serde::Serialize)]
#[non_exhaustive]
pub struct TradeProposeRequest {
    #[serde(serialize_with = "crate::actor_json::serialize_actor_context")]
    pub actor: RadrootsActorContext,
    pub listing_event: RadrootsEventPtr,
    pub order_id: RadrootsOrderId,
    pub listing_addr: RadrootsListingAddress,
    pub seller_pubkey: RadrootsPublicKey,
    pub items: Vec<RadrootsOrderItem>,
    pub economics: RadrootsOrderEconomics,
    pub public_note: Option<String>,
    pub target_policy: TargetPolicy,
    pub publish_mode: PublishMode,
    pub satisfaction_policy: SatisfactionPolicy,
    pub privacy_confirmation: PrivacyPreflightConfirmation,
    pub idempotency_key: Option<SdkIdempotencyKey>,
    pub created_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(feature = "signer-adapters")]
impl TradeProposeRequest {
    pub fn new(
        actor: RadrootsActorContext,
        listing_event: RadrootsEventPtr,
        order: RadrootsOrderRequest,
        target_policy: TargetPolicy,
        publish_mode: PublishMode,
        satisfaction_policy: SatisfactionPolicy,
    ) -> Self {
        Self {
            actor,
            listing_event,
            order_id: order.order_id,
            listing_addr: order.listing_addr,
            seller_pubkey: order.seller_pubkey,
            items: order.items,
            economics: order.economics,
            public_note: None,
            target_policy,
            publish_mode,
            satisfaction_policy,
            privacy_confirmation: PrivacyPreflightConfirmation::new(),
            idempotency_key: None,
            created_at: None,
        }
    }

    pub fn with_public_note(mut self, public_note: impl Into<String>) -> Self {
        self.public_note = Some(public_note.into());
        self
    }

    pub fn with_optional_public_note(mut self, public_note: Option<String>) -> Self {
        self.public_note = public_note;
        self
    }

    pub fn with_privacy_confirmation(
        mut self,
        privacy_confirmation: PrivacyPreflightConfirmation,
    ) -> Self {
        self.privacy_confirmation = privacy_confirmation;
        self
    }

    pub fn try_with_idempotency_key(
        mut self,
        idempotency_key: impl AsRef<str>,
    ) -> Result<Self, RadrootsSdkError> {
        self.idempotency_key = Some(SdkIdempotencyKey::new(idempotency_key)?);
        Ok(self)
    }

    pub fn with_created_at(mut self, created_at: RadrootsSdkTimestamp) -> Self {
        self.created_at = Some(created_at);
        self
    }
}

#[cfg(feature = "signer-adapters")]
#[derive(Clone, Debug, serde::Serialize)]
#[non_exhaustive]
pub struct TradeAcceptRequest {
    #[serde(serialize_with = "crate::actor_json::serialize_actor_context")]
    pub actor: RadrootsActorContext,
    pub locator: RadrootsTradeLocator,
    pub inventory_commitments: Vec<RadrootsOrderInventoryCommitment>,
    pub target_policy: TargetPolicy,
    pub publish_mode: PublishMode,
    pub satisfaction_policy: SatisfactionPolicy,
    pub evidence_mode: TradeEvidenceMode,
    pub privacy_confirmation: PrivacyPreflightConfirmation,
    pub idempotency_key: Option<SdkIdempotencyKey>,
    pub created_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(feature = "signer-adapters")]
impl TradeAcceptRequest {
    pub fn new(
        actor: RadrootsActorContext,
        locator: RadrootsTradeLocator,
        inventory_commitments: Vec<RadrootsOrderInventoryCommitment>,
        target_policy: TargetPolicy,
        publish_mode: PublishMode,
        satisfaction_policy: SatisfactionPolicy,
        evidence_mode: TradeEvidenceMode,
    ) -> Self {
        Self {
            actor,
            locator,
            inventory_commitments,
            target_policy,
            publish_mode,
            satisfaction_policy,
            evidence_mode,
            privacy_confirmation: PrivacyPreflightConfirmation::new(),
            idempotency_key: None,
            created_at: None,
        }
    }

    pub fn with_privacy_confirmation(
        mut self,
        privacy_confirmation: PrivacyPreflightConfirmation,
    ) -> Self {
        self.privacy_confirmation = privacy_confirmation;
        self
    }

    pub fn try_with_idempotency_key(
        mut self,
        idempotency_key: impl AsRef<str>,
    ) -> Result<Self, RadrootsSdkError> {
        self.idempotency_key = Some(SdkIdempotencyKey::new(idempotency_key)?);
        Ok(self)
    }

    pub fn with_created_at(mut self, created_at: RadrootsSdkTimestamp) -> Self {
        self.created_at = Some(created_at);
        self
    }
}

#[cfg(feature = "signer-adapters")]
#[derive(Clone, Debug, serde::Serialize)]
#[non_exhaustive]
pub struct TradeDeclineRequest {
    #[serde(serialize_with = "crate::actor_json::serialize_actor_context")]
    pub actor: RadrootsActorContext,
    pub locator: RadrootsTradeLocator,
    pub reason: String,
    pub target_policy: TargetPolicy,
    pub publish_mode: PublishMode,
    pub satisfaction_policy: SatisfactionPolicy,
    pub evidence_mode: TradeEvidenceMode,
    pub privacy_confirmation: PrivacyPreflightConfirmation,
    pub idempotency_key: Option<SdkIdempotencyKey>,
    pub created_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(feature = "signer-adapters")]
impl TradeDeclineRequest {
    pub fn new(
        actor: RadrootsActorContext,
        locator: RadrootsTradeLocator,
        reason: impl Into<String>,
        target_policy: TargetPolicy,
        publish_mode: PublishMode,
        satisfaction_policy: SatisfactionPolicy,
        evidence_mode: TradeEvidenceMode,
    ) -> Self {
        Self {
            actor,
            locator,
            reason: reason.into(),
            target_policy,
            publish_mode,
            satisfaction_policy,
            evidence_mode,
            privacy_confirmation: PrivacyPreflightConfirmation::new(),
            idempotency_key: None,
            created_at: None,
        }
    }

    pub fn with_privacy_confirmation(
        mut self,
        privacy_confirmation: PrivacyPreflightConfirmation,
    ) -> Self {
        self.privacy_confirmation = privacy_confirmation;
        self
    }

    pub fn try_with_idempotency_key(
        mut self,
        idempotency_key: impl AsRef<str>,
    ) -> Result<Self, RadrootsSdkError> {
        self.idempotency_key = Some(SdkIdempotencyKey::new(idempotency_key)?);
        Ok(self)
    }

    pub fn with_created_at(mut self, created_at: RadrootsSdkTimestamp) -> Self {
        self.created_at = Some(created_at);
        self
    }
}

#[cfg(feature = "signer-adapters")]
#[derive(Clone, Debug, serde::Serialize)]
#[non_exhaustive]
pub struct TradeCancelRequest {
    #[serde(serialize_with = "crate::actor_json::serialize_actor_context")]
    pub actor: RadrootsActorContext,
    pub locator: RadrootsTradeLocator,
    pub reason: String,
    pub target_policy: TargetPolicy,
    pub publish_mode: PublishMode,
    pub satisfaction_policy: SatisfactionPolicy,
    pub evidence_mode: TradeEvidenceMode,
    pub privacy_confirmation: PrivacyPreflightConfirmation,
    pub idempotency_key: Option<SdkIdempotencyKey>,
    pub created_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(feature = "signer-adapters")]
impl TradeCancelRequest {
    pub fn new(
        actor: RadrootsActorContext,
        locator: RadrootsTradeLocator,
        reason: impl Into<String>,
        target_policy: TargetPolicy,
        publish_mode: PublishMode,
        satisfaction_policy: SatisfactionPolicy,
        evidence_mode: TradeEvidenceMode,
    ) -> Self {
        Self {
            actor,
            locator,
            reason: reason.into(),
            target_policy,
            publish_mode,
            satisfaction_policy,
            evidence_mode,
            privacy_confirmation: PrivacyPreflightConfirmation::new(),
            idempotency_key: None,
            created_at: None,
        }
    }

    pub fn with_privacy_confirmation(
        mut self,
        privacy_confirmation: PrivacyPreflightConfirmation,
    ) -> Self {
        self.privacy_confirmation = privacy_confirmation;
        self
    }

    pub fn try_with_idempotency_key(
        mut self,
        idempotency_key: impl AsRef<str>,
    ) -> Result<Self, RadrootsSdkError> {
        self.idempotency_key = Some(SdkIdempotencyKey::new(idempotency_key)?);
        Ok(self)
    }

    pub fn with_created_at(mut self, created_at: RadrootsSdkTimestamp) -> Self {
        self.created_at = Some(created_at);
        self
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, serde::Serialize)]
#[non_exhaustive]
pub struct TradeSellerInboxRequest {
    #[serde(serialize_with = "crate::actor_json::serialize_actor_context")]
    pub actor: RadrootsActorContext,
    pub limit: u32,
}

#[cfg(feature = "runtime")]
impl TradeSellerInboxRequest {
    pub fn new(actor: RadrootsActorContext) -> Self {
        Self {
            actor,
            limit: TRADE_STATUS_DEFAULT_LIMIT,
        }
    }

    pub fn with_limit(mut self, limit: u32) -> Self {
        self.limit = limit;
        self
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct TradeSellerInboxReceipt {
    pub seller_pubkey: RadrootsPublicKey,
    pub statuses: Vec<TradeStatusReceipt>,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, serde::Serialize)]
#[non_exhaustive]
pub struct TradeResyncRequest {
    pub locator: RadrootsTradeLocator,
    pub limit: u32,
    pub validation_trust_policy: RadrootsTradeValidationTrustPolicy,
}

#[cfg(feature = "runtime")]
impl TradeResyncRequest {
    pub fn new(locator: RadrootsTradeLocator) -> Self {
        Self {
            locator,
            limit: TRADE_STATUS_DEFAULT_LIMIT,
            validation_trust_policy: RadrootsTradeValidationTrustPolicy::production(),
        }
    }

    pub fn with_limit(mut self, limit: u32) -> Self {
        self.limit = limit;
        self
    }

    pub fn with_validation_trust_policy(
        mut self,
        policy: RadrootsTradeValidationTrustPolicy,
    ) -> Self {
        self.validation_trust_policy = policy;
        self
    }

    pub fn try_with_trusted_rhi_pubkeys<I, S>(
        mut self,
        pubkeys: I,
    ) -> Result<Self, RadrootsSdkError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.validation_trust_policy.trusted_rhi_pubkeys = parse_worker_pubkeys(pubkeys)?;
        Ok(self)
    }

    #[cfg(feature = "transport-nostr-runtime")]
    fn validate(&self) -> Result<(), RadrootsSdkError> {
        if self.limit == 0 || self.limit > TRADE_STATUS_MAX_LIMIT {
            return Err(RadrootsSdkError::trade_status_limit_invalid(
                self.limit,
                1,
                TRADE_STATUS_MAX_LIMIT,
            ));
        }
        Ok(())
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct TradeResyncReceipt {
    pub nostr_relay_urls: Vec<String>,
    pub evidence: TradeResyncEvidenceReceipt,
    pub refresh: SyncProjectionRefreshReceipt,
    pub status: TradeStatusReceipt,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct TradeResyncEvidenceReceipt {
    pub query_plan: TradeEvidenceQueryPlan,
    pub inserted_count: usize,
    pub duplicate_count: usize,
    pub malformed_count: usize,
    pub out_of_filter_count: usize,
    pub skipped_over_limit_count: usize,
    pub unsupported_count: usize,
    pub eose_count: usize,
    pub closed_count: usize,
    pub notice_count: usize,
    pub branches: Vec<TradeEvidenceBranchReceipt>,
    pub events: Vec<TradeResyncEventImportReceipt>,
    pub nostr_relay_outcomes: Vec<TradeResyncNostrRelayOutcomeReceipt>,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct TradeEvidenceQueryPlan {
    pub locator: RadrootsTradeLocator,
    pub limit: u32,
    pub branches: Vec<TradeEvidenceQueryBranch>,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct TradeEvidenceQueryBranch {
    pub kind: TradeEvidenceQueryBranchKind,
    pub filter: TradeEvidenceNostrRelayFilter,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum TradeEvidenceQueryBranchKind {
    RequestRoots,
    LifecycleChain,
    ValidationReceipts,
    ListingSnapshot,
    RejectedEvidence,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct TradeEvidenceNostrRelayFilter {
    pub active: bool,
    pub event_kinds: Vec<u32>,
    pub author_pubkey: Option<String>,
    pub tag: Option<TradeEvidenceNostrRelayTagFilter>,
    pub limit: u32,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct TradeEvidenceNostrRelayTagFilter {
    pub tag_name: String,
    pub values: Vec<String>,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct TradeEvidenceBranchReceipt {
    pub branch: TradeEvidenceQueryBranchKind,
    pub accepted_count: usize,
    pub inserted_count: usize,
    pub duplicate_count: usize,
    pub malformed_count: usize,
    pub out_of_filter_count: usize,
    pub skipped_over_limit_count: usize,
    pub unsupported_count: usize,
    pub nostr_relay_failure_count: usize,
    pub empty_result: bool,
    pub events: Vec<TradeResyncEventImportReceipt>,
    pub nostr_relay_outcomes: Vec<TradeResyncNostrRelayOutcomeReceipt>,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct TradeResyncEventImportReceipt {
    pub nostr_relay_url: String,
    pub event_id: Option<String>,
    pub inserted: bool,
    pub duplicate: bool,
    pub unsupported: bool,
    pub malformed: bool,
    pub out_of_filter: bool,
    pub skipped_over_limit: bool,
    pub projection_eligible: bool,
    pub verification_status: Option<String>,
    pub message: Option<String>,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct TradeResyncNostrRelayOutcomeReceipt {
    pub nostr_relay_url: String,
    pub outcome_kind: TradeResyncNostrRelayOutcomeKind,
    pub transport_outcome_kind: Option<TradeResyncNostrRelayTransportOutcomeKind>,
    pub message: Option<String>,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum TradeResyncNostrRelayOutcomeKind {
    Eose,
    Closed,
    Notice,
}

#[cfg(feature = "runtime")]
impl TradeResyncNostrRelayOutcomeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Eose => "eose",
            Self::Closed => "closed",
            Self::Notice => "notice",
        }
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum TradeResyncNostrRelayTransportOutcomeKind {
    Accepted,
    DuplicateAccepted,
    Blocked,
    RateLimited,
    Invalid,
    PowRequired,
    Restricted,
    AuthRequired,
    Muted,
    Unsupported,
    PaymentRequired,
    Error,
    Timeout,
    ConnectionFailed,
    RelayUrlRejected,
    SkippedAlreadyAccepted,
    Unknown,
}

#[cfg(feature = "runtime")]
impl TradeResyncNostrRelayTransportOutcomeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::DuplicateAccepted => "duplicate_accepted",
            Self::Blocked => "blocked",
            Self::RateLimited => "rate_limited",
            Self::Invalid => "invalid",
            Self::PowRequired => "pow_required",
            Self::Restricted => "restricted",
            Self::AuthRequired => "auth_required",
            Self::Muted => "muted",
            Self::Unsupported => "unsupported",
            Self::PaymentRequired => "payment_required",
            Self::Error => "error",
            Self::Timeout => "timeout",
            Self::ConnectionFailed => "connection_failed",
            Self::RelayUrlRejected => "relay_url_rejected",
            Self::SkippedAlreadyAccepted => "skipped_already_accepted",
            Self::Unknown => "unknown",
        }
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[non_exhaustive]
pub struct TradeValidationReceiptListRequest {
    pub order_id: RadrootsOrderId,
    pub limit: u32,
}

#[cfg(feature = "runtime")]
impl TradeValidationReceiptListRequest {
    pub fn new(order_id: RadrootsOrderId) -> Self {
        Self {
            order_id,
            limit: TRADE_STATUS_DEFAULT_LIMIT,
        }
    }

    pub fn parse(order_id: &str) -> Result<Self, RadrootsSdkError> {
        RadrootsOrderId::parse(order_id)
            .map(Self::new)
            .map_err(|error| RadrootsSdkError::invalid_trade_id(order_id, error.to_string()))
    }

    pub fn with_limit(mut self, limit: u32) -> Self {
        self.limit = limit;
        self
    }

    #[cfg(feature = "transport-nostr-runtime")]
    fn validate(&self) -> Result<(), RadrootsSdkError> {
        validate_validation_receipt_limit(self.limit)
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[non_exhaustive]
pub struct TradeValidationReceiptInspectRequest {
    pub receipt_event_id: RadrootsEventId,
}

#[cfg(feature = "runtime")]
impl TradeValidationReceiptInspectRequest {
    pub fn new(receipt_event_id: RadrootsEventId) -> Self {
        Self { receipt_event_id }
    }

    pub fn parse(receipt_event_id: &str) -> Result<Self, RadrootsSdkError> {
        RadrootsEventId::parse(receipt_event_id)
            .map(Self::new)
            .map_err(|error| RadrootsSdkError::InvalidRequest {
                message: format!(
                    "invalid validation receipt event id `{receipt_event_id}`: {error}"
                ),
            })
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[non_exhaustive]
pub struct TradeValidationReceiptVerifyRequest {
    pub receipt_event_id: RadrootsEventId,
}

#[cfg(feature = "runtime")]
impl TradeValidationReceiptVerifyRequest {
    pub fn new(receipt_event_id: RadrootsEventId) -> Self {
        Self { receipt_event_id }
    }

    pub fn parse(receipt_event_id: &str) -> Result<Self, RadrootsSdkError> {
        RadrootsEventId::parse(receipt_event_id)
            .map(Self::new)
            .map_err(|error| RadrootsSdkError::InvalidRequest {
                message: format!(
                    "invalid validation receipt event id `{receipt_event_id}`: {error}"
                ),
            })
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct TradeValidationReceiptListReceipt {
    pub nostr_relay_urls: Vec<String>,
    pub nostr_evidence: TradeValidationReceiptNostrEvidenceReceipt,
    pub order_id: RadrootsOrderId,
    pub receipts: Vec<TradeValidationReceiptEvent>,
    pub invalid_receipts: Vec<TradeValidationReceiptInvalidCandidate>,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct TradeValidationReceiptInspectReceipt {
    pub nostr_relay_urls: Vec<String>,
    pub nostr_evidence: TradeValidationReceiptNostrEvidenceReceipt,
    pub receipt_event_id: RadrootsEventId,
    pub receipt: Option<TradeValidationReceiptEvent>,
    pub invalid_receipt: Option<TradeValidationReceiptInvalidCandidate>,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct TradeValidationReceiptEvent {
    pub event: RadrootsEventEnvelope,
    pub receipt: RadrootsTradeValidationReceipt,
    pub tags: TradeValidationReceiptTags,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct TradeValidationReceiptInvalidCandidate {
    pub event: RadrootsEventEnvelope,
    pub reason_code: String,
    pub reason: String,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct TradeValidationReceiptTags {
    pub order_id: String,
    pub event_set_root: String,
    pub listing_event_id: String,
    pub reducer_output_root: String,
    pub public_values_hash: String,
    pub proof_system: String,
    pub receipt_type: String,
    pub root_event_id: String,
    pub target_event_id: String,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct TradeValidationReceiptNostrEvidenceReceipt {
    pub inserted_count: usize,
    pub duplicate_count: usize,
    pub malformed_count: usize,
    pub out_of_filter_count: usize,
    pub skipped_over_limit_count: usize,
    pub unsupported_count: usize,
    pub eose_count: usize,
    pub closed_count: usize,
    pub notice_count: usize,
    pub events: Vec<TradeResyncEventImportReceipt>,
    pub nostr_relay_outcomes: Vec<TradeValidationReceiptNostrRelayOutcomeReceipt>,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct TradeValidationReceiptNostrRelayOutcomeReceipt {
    pub nostr_relay_url: String,
    pub outcome_kind: TradeValidationReceiptNostrRelayOutcomeKind,
    pub transport_outcome_kind: Option<TradeValidationReceiptNostrRelayTransportOutcomeKind>,
    pub message: Option<String>,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum TradeValidationReceiptNostrRelayOutcomeKind {
    Eose,
    Closed,
    Notice,
}

#[cfg(feature = "runtime")]
impl TradeValidationReceiptNostrRelayOutcomeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Eose => "eose",
            Self::Closed => "closed",
            Self::Notice => "notice",
        }
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum TradeValidationReceiptNostrRelayTransportOutcomeKind {
    Accepted,
    DuplicateAccepted,
    Blocked,
    RateLimited,
    Invalid,
    PowRequired,
    Restricted,
    AuthRequired,
    Muted,
    Unsupported,
    PaymentRequired,
    Error,
    Timeout,
    ConnectionFailed,
    RelayUrlRejected,
    SkippedAlreadyAccepted,
    Unknown,
}

#[cfg(feature = "runtime")]
impl TradeValidationReceiptNostrRelayTransportOutcomeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::DuplicateAccepted => "duplicate_accepted",
            Self::Blocked => "blocked",
            Self::RateLimited => "rate_limited",
            Self::Invalid => "invalid",
            Self::PowRequired => "pow_required",
            Self::Restricted => "restricted",
            Self::AuthRequired => "auth_required",
            Self::Muted => "muted",
            Self::Unsupported => "unsupported",
            Self::PaymentRequired => "payment_required",
            Self::Error => "error",
            Self::Timeout => "timeout",
            Self::ConnectionFailed => "connection_failed",
            Self::RelayUrlRejected => "relay_url_rejected",
            Self::SkippedAlreadyAccepted => "skipped_already_accepted",
            Self::Unknown => "unknown",
        }
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[non_exhaustive]
pub struct TradeStatusRequest {
    pub locator: RadrootsTradeLocator,
    pub limit: u32,
    pub source: SdkTradeStatusSource,
    pub validation_trust_policy: RadrootsTradeValidationTrustPolicy,
}

#[cfg(feature = "runtime")]
impl TradeStatusRequest {
    pub fn new(locator: RadrootsTradeLocator) -> Self {
        Self {
            locator,
            limit: TRADE_STATUS_DEFAULT_LIMIT,
            source: SdkTradeStatusSource::LocalOnly,
            validation_trust_policy: RadrootsTradeValidationTrustPolicy::production(),
        }
    }

    pub fn parse(selector: &str) -> Result<Self, RadrootsSdkError> {
        let (trade_id, root_event_id) = trade_status_selector_parts(selector)?;
        let locator = RadrootsOrderId::parse(trade_id)
            .map(RadrootsTradeLocator::from_order_id)
            .map_err(|error| RadrootsSdkError::invalid_trade_id(trade_id, error.to_string()))?;
        let locator = match root_event_id {
            Some(root_event_id) => {
                locator.with_root_event_id(root_event_id.parse().map_err(|error| {
                    RadrootsSdkError::InvalidRequest {
                        message: format!(
                            "invalid trade status root selector `{selector}`: {error}"
                        ),
                    }
                })?)
            }
            None => locator,
        };
        Ok(Self::new(locator))
    }

    pub fn locator_selector(locator: &RadrootsTradeLocator) -> String {
        match locator.root_event_id.as_ref() {
            Some(root_event_id) => format!(
                "{}{TRADE_STATUS_ROOT_SELECTOR_SEPARATOR}{}",
                locator.trade_id.as_str(),
                root_event_id.as_str()
            ),
            None => locator.trade_id.as_str().to_owned(),
        }
    }

    pub fn with_limit(mut self, limit: u32) -> Self {
        self.limit = limit;
        self
    }

    pub fn with_source(mut self, source: SdkTradeStatusSource) -> Self {
        self.source = source;
        self
    }

    pub fn with_validation_trust_policy(
        mut self,
        policy: RadrootsTradeValidationTrustPolicy,
    ) -> Self {
        self.validation_trust_policy = policy;
        self
    }

    pub fn try_with_trusted_rhi_pubkeys<I, S>(
        mut self,
        pubkeys: I,
    ) -> Result<Self, RadrootsSdkError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.validation_trust_policy.trusted_rhi_pubkeys = parse_worker_pubkeys(pubkeys)?;
        Ok(self)
    }

    fn validate(&self) -> Result<(), RadrootsSdkError> {
        if self.limit == 0 || self.limit > TRADE_STATUS_MAX_LIMIT {
            return Err(RadrootsSdkError::trade_status_limit_invalid(
                self.limit,
                1,
                TRADE_STATUS_MAX_LIMIT,
            ));
        }
        Ok(())
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[non_exhaustive]
pub struct TradeStatusWatchRequest {
    pub status: TradeStatusRequest,
    pub capacity: usize,
    pub refresh_interval_ms: u64,
    pub refresh_limit: Option<u32>,
}

#[cfg(feature = "runtime")]
impl TradeStatusWatchRequest {
    pub fn new(status: TradeStatusRequest) -> Self {
        Self {
            status,
            capacity: TRADE_STATUS_WATCH_DEFAULT_CAPACITY,
            refresh_interval_ms: TRADE_STATUS_WATCH_DEFAULT_REFRESH_INTERVAL_MS,
            refresh_limit: None,
        }
    }

    pub fn parse(selector: &str) -> Result<Self, RadrootsSdkError> {
        TradeStatusRequest::parse(selector).map(Self::new)
    }

    pub fn with_capacity(mut self, capacity: usize) -> Self {
        self.capacity = capacity;
        self
    }

    pub fn with_refresh_interval_ms(mut self, refresh_interval_ms: u64) -> Self {
        self.refresh_interval_ms = refresh_interval_ms;
        self
    }

    pub fn with_refresh_limit(mut self, refresh_limit: u32) -> Self {
        self.refresh_limit = Some(refresh_limit);
        self
    }

    pub fn without_refresh_limit(mut self) -> Self {
        self.refresh_limit = None;
        self
    }

    fn validate(&self) -> Result<(), RadrootsSdkError> {
        self.status.validate()?;
        if self.capacity == 0 || self.capacity > TRADE_STATUS_WATCH_MAX_CAPACITY {
            return Err(RadrootsSdkError::InvalidRequest {
                message: format!(
                    "trade status watch capacity must be between 1 and {TRADE_STATUS_WATCH_MAX_CAPACITY}"
                ),
            });
        }
        if self.refresh_interval_ms == 0
            || self.refresh_interval_ms > TRADE_STATUS_WATCH_MAX_REFRESH_INTERVAL_MS
        {
            return Err(RadrootsSdkError::InvalidRequest {
                message: format!(
                    "trade status watch refresh interval must be between 1 and {TRADE_STATUS_WATCH_MAX_REFRESH_INTERVAL_MS} milliseconds"
                ),
            });
        }
        if self.refresh_limit == Some(0) {
            return Err(RadrootsSdkError::InvalidRequest {
                message: "trade status watch refresh limit must be greater than zero".to_owned(),
            });
        }
        Ok(())
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct TradeStatusWatchUpdate {
    pub sequence: u64,
    pub status: TradeStatusReceipt,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct TradeStatusWatchCancelReceipt {
    pub state: TradeStatusWatchCancelState,
    pub buffered_updates_dropped: usize,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum TradeStatusWatchCancelState {
    Cancelled,
    AlreadyFinished,
}

#[cfg(feature = "runtime")]
pub struct TradeStatusWatch {
    receiver: mpsc::Receiver<Result<TradeStatusWatchUpdate, RadrootsSdkError>>,
    cancel: Option<oneshot::Sender<()>>,
    producer: Option<JoinHandle<()>>,
    capacity: usize,
}

#[cfg(feature = "runtime")]
impl TradeStatusWatch {
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn buffered_len(&self) -> usize {
        self.receiver.len()
    }

    pub async fn next(&mut self) -> Result<Option<TradeStatusWatchUpdate>, RadrootsSdkError> {
        match self.receiver.recv().await {
            Some(Ok(update)) => Ok(Some(update)),
            Some(Err(error)) => Err(error),
            None => Ok(None),
        }
    }

    pub async fn cancel(&mut self) -> TradeStatusWatchCancelReceipt {
        let producer_active = self
            .producer
            .as_ref()
            .map(|producer| !producer.is_finished())
            .unwrap_or(false);
        let buffered_updates_dropped = self.drain_buffered_updates();
        let cancel_sent = self
            .cancel
            .take()
            .map(|sender| sender.send(()).is_ok())
            .unwrap_or(false);
        let producer_finished = match self.producer.take() {
            Some(producer) => producer.await.is_ok(),
            None => true,
        };
        let state = if producer_active || cancel_sent || !producer_finished {
            TradeStatusWatchCancelState::Cancelled
        } else {
            TradeStatusWatchCancelState::AlreadyFinished
        };
        TradeStatusWatchCancelReceipt {
            state,
            buffered_updates_dropped,
        }
    }

    fn drain_buffered_updates(&mut self) -> usize {
        self.receiver.close();
        let mut dropped = 0;
        while self.receiver.try_recv().is_ok() {
            dropped += 1;
        }
        dropped
    }
}

#[cfg(feature = "runtime")]
impl Drop for TradeStatusWatch {
    fn drop(&mut self) {
        self.drain_buffered_updates();
        if let Some(cancel) = self.cancel.take() {
            let _ = cancel.send(());
        }
        if let Some(producer) = self.producer.take() {
            producer.abort();
        }
    }
}

#[cfg(feature = "runtime")]
fn trade_status_selector_parts(selector: &str) -> Result<(&str, Option<&str>), RadrootsSdkError> {
    let selector = selector.trim();
    if selector.is_empty() {
        return Err(RadrootsSdkError::invalid_trade_id(
            selector,
            "empty trade id",
        ));
    }
    let Some((trade_id, root_event_id)) = selector.split_once(TRADE_STATUS_ROOT_SELECTOR_SEPARATOR)
    else {
        return Ok((selector, None));
    };
    if trade_id.trim().is_empty()
        || root_event_id.trim().is_empty()
        || root_event_id.contains(TRADE_STATUS_ROOT_SELECTOR_SEPARATOR)
    {
        return Err(RadrootsSdkError::InvalidRequest {
            message: format!("invalid trade status selector `{selector}`"),
        });
    }
    Ok((trade_id, Some(root_event_id)))
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct TradeStatusReceipt {
    pub locator: RadrootsTradeLocator,
    pub order_id: RadrootsOrderId,
    pub root_event_id: Option<RadrootsEventId>,
    pub ambiguity_candidates: Vec<TradeStatusAmbiguityCandidate>,
    pub source: SdkTradeStatusSource,
    pub found: bool,
    pub event_count: usize,
    pub limit_applied: u32,
    pub status: TradeStatusKind,
    pub lifecycle_terminal: bool,
    pub listing_addr: Option<RadrootsListingAddress>,
    pub buyer_pubkey: Option<RadrootsPublicKey>,
    pub seller_pubkey: Option<RadrootsPublicKey>,
    pub economics: Option<RadrootsOrderEconomics>,
    pub evidence: TradeStatusEvidenceSummary,
    pub validation_trust: Option<TradeValidationTrustDecision>,
    pub online_evidence: Option<TradeResyncEvidenceReceipt>,
    pub eligibility: TradeStatusEligibility,
    pub next_action: TradeStatusNextActionKind,
    pub event_ids: Vec<RadrootsEventId>,
    pub request_event_id: Option<RadrootsEventId>,
    pub decision_event_id: Option<RadrootsEventId>,
    pub agreement_event_id: Option<RadrootsEventId>,
    pub rhi_receipt_event_id: Option<RadrootsEventId>,
    pub cancellation_event_id: Option<RadrootsEventId>,
    pub last_event_id: Option<RadrootsEventId>,
    pub issues: Vec<SdkTradeStatusIssue>,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct TradeStatusAmbiguityCandidate {
    pub locator: RadrootsTradeLocator,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct TradeStatusEvidenceSummary {
    pub event_count: usize,
    pub limit_applied: u32,
    pub has_request: bool,
    pub has_decision: bool,
    pub has_agreement: bool,
    pub has_validation_receipt: bool,
    pub has_cancellation: bool,
    pub has_issues: bool,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct TradeValidationTrustDecision {
    pub state: RadrootsTradeValidationTrustState,
    pub trusted_rhi_pubkey_count: usize,
    pub allow_deterministic_none: bool,
    pub require_cryptographic_proof: bool,
    pub receipt_event_id: Option<RadrootsEventId>,
    pub receipt_author: Option<RadrootsPublicKey>,
    pub result_event_id: Option<RadrootsEventId>,
    pub result_author: Option<RadrootsPublicKey>,
    pub proof_system: Option<String>,
    pub validation_authority: Option<RadrootsTradeValidationAuthority>,
    pub commitment_confidence: Option<RadrootsTradeCommitmentConfidence>,
    pub cryptographic_proof_required: bool,
    pub cryptographic_proof_verified: bool,
    pub production_committed: bool,
    pub reason_code: Option<String>,
    pub reason: Option<String>,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct TradeStatusEligibility {
    pub can_decide: bool,
    pub can_cancel: bool,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum TradeStatusNextActionKind {
    NoLocalOrder,
    InspectEvidenceIssues,
    AwaitSellerDecision,
    AwaitValidation,
    Terminal,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SdkTradeStatusSource {
    LocalOnly,
    ResyncThenLocal,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum TradeStatusKind {
    Missing,
    Ambiguous,
    Requested,
    AgreedPendingValidation,
    Committed,
    Declined,
    Cancelled,
    ValidationExpired,
    Invalid,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SdkTradeStatusIssue {
    pub kind: SdkTradeStatusIssueKind,
    pub event_ids: Vec<RadrootsEventId>,
}

#[cfg(feature = "runtime")]
impl SdkTradeStatusIssue {
    fn new(kind: SdkTradeStatusIssueKind, event_ids: Vec<RadrootsEventId>) -> Self {
        Self { kind, event_ids }
    }

    fn single(kind: SdkTradeStatusIssueKind, event_id: RadrootsEventId) -> Self {
        Self::new(kind, vec![event_id])
    }

    pub fn code(&self) -> String {
        self.kind.code()
    }
}

#[cfg(feature = "runtime")]
impl serde::Serialize for SdkTradeStatusIssue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("SdkTradeStatusIssue", 3)?;
        state.serialize_field("code", &self.code())?;
        state.serialize_field("kind", &self.kind)?;
        state.serialize_field("event_ids", &self.event_ids)?;
        state.end()
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SdkTradeStatusIssueKind {
    MissingRequest,
    MultipleRequests,
    RequestPayloadInvalid,
    RequestOrderIdMismatch,
    RequestAuthorMismatch,
    RequestListingAddressInvalid,
    RequestSellerListingMismatch,
    DecisionPayloadInvalid,
    DecisionOrderIdMismatch,
    DecisionAuthorMismatch,
    DecisionCounterpartyMismatch,
    DecisionBuyerMismatch,
    DecisionSellerMismatch,
    DecisionListingAddressInvalid,
    DecisionListingMismatch,
    DecisionRootMismatch,
    DecisionPreviousMismatch,
    DecisionMissingInventoryCommitments,
    DecisionInventoryCommitmentMismatch,
    DecisionMissingReason,
    ConflictingDecisions,
    CancellationWithoutCancellableOrder,
    CancellationPayloadInvalid,
    CancellationOrderIdMismatch,
    CancellationAuthorMismatch,
    CancellationCounterpartyMismatch,
    CancellationBuyerMismatch,
    CancellationSellerMismatch,
    CancellationListingAddressInvalid,
    CancellationListingMismatch,
    CancellationRootMismatch,
    CancellationPreviousMismatch,
    ForkedLifecycle,
    ValidationReceiptWithoutPendingAgreement,
    ValidationReceiptOrderIdMismatch,
    ValidationReceiptTypeMismatch,
    ValidationReceiptRootMismatch,
    ValidationReceiptTargetMismatch,
    ValidationReceiptListingMismatch,
    ConflictingValidationReceipts,
    DeterministicValidationFailure,
    StaleListingEvent,
}

#[cfg(feature = "runtime")]
impl SdkTradeStatusIssueKind {
    pub fn code(self) -> String {
        camel_to_snake(format!("{self:?}").as_str())
    }
}

#[cfg(feature = "runtime")]
impl<'sdk> TradesClient<'sdk> {
    pub async fn ingest_evidence(
        &self,
        request: TradeEvidenceIngestRequest,
    ) -> Result<TradeEvidenceIngestReceipt, RadrootsSdkError> {
        let evidence = parse_order_evidence(request.event.envelope())?;
        let observed_at = self.resolved_created_at(request.observed_at)?;
        let observed_at_ms = sdk_timestamp_ms(observed_at)?;
        let receipt = self
            .sdk
            ._event_store
            .ingest_event(RadrootsEventIngest::new(request.event, observed_at_ms))
            .await
            .map_err(|error| RadrootsSdkError::EventStore {
                message: error.to_string(),
            })?;
        Ok(TradeEvidenceIngestReceipt {
            order_id: evidence.order_id,
            listing_addr: evidence.listing_addr,
            event_id: evidence.event_id,
            event_kind: evidence.event_kind,
            local_event_seq: receipt.seq,
            inserted: receipt.inserted,
        })
    }

    pub async fn ingest_request_evidence(
        &self,
        request: TradeRequestEvidenceIngestRequest,
    ) -> Result<TradeRequestEvidenceIngestReceipt, RadrootsSdkError> {
        let evidence = parse_order_request_evidence(request.event.envelope())?;
        let observed_at = self.resolved_created_at(request.observed_at)?;
        let observed_at_ms = sdk_timestamp_ms(observed_at)?;
        let receipt = self
            .sdk
            ._event_store
            .ingest_event(RadrootsEventIngest::new(request.event, observed_at_ms))
            .await
            .map_err(|error| RadrootsSdkError::EventStore {
                message: error.to_string(),
            })?;
        Ok(TradeRequestEvidenceIngestReceipt {
            order_id: evidence.order_id,
            listing_addr: evidence.listing_addr,
            buyer_pubkey: evidence.buyer_pubkey,
            seller_pubkey: evidence.seller_pubkey,
            request_event_id: evidence.request_event_id,
            local_event_seq: receipt.seq,
            inserted: receipt.inserted,
        })
    }

    #[cfg(feature = "signer-adapters")]
    pub(crate) fn prepare_submit(
        &self,
        request: TradeSubmitPrepareRequest,
    ) -> Result<TradeSubmitPlan, RadrootsSdkError> {
        let created_at = self.resolved_created_at(request.created_at)?;
        order_submit_plan(
            &request.actor,
            request.listing_event,
            request.order,
            created_at,
        )
    }

    #[cfg(feature = "signer-adapters")]
    pub(crate) async fn enqueue_prepared_submit(
        &self,
        actor: &RadrootsActorContext,
        plan: TradeSubmitPlan,
        target_policy: TargetPolicy,
        publish_mode: PublishMode,
        satisfaction_policy: SatisfactionPolicy,
        idempotency_key: Option<SdkIdempotencyKey>,
    ) -> Result<TradeSubmitReceipt, RadrootsSdkError> {
        validate_trade_enqueue_policy(publish_mode, &satisfaction_policy)?;
        let enqueue = enqueue_configured_signed_workflow(
            self.sdk,
            SdkWorkflowEnqueueRequest {
                operation_kind: TradeWorkflowKind::Submit.operation_kind(),
                actor,
                frozen_draft: &plan.frozen_draft,
                target_policy: target_policy.workflow_target_policy(),
                satisfaction_policy,
                idempotency_key,
            },
        )
        .await?;
        Ok(order_submit_receipt(plan, enqueue))
    }

    #[cfg(feature = "signer-adapters")]
    pub(crate) fn prepare_decision(
        &self,
        request: TradeDecisionPrepareRequest,
    ) -> Result<TradeDecisionPlan, RadrootsSdkError> {
        let created_at = self.resolved_created_at(request.created_at)?;
        order_decision_plan(
            &request.actor,
            request.request_event,
            request.decision,
            created_at,
        )
    }

    #[cfg(feature = "signer-adapters")]
    pub(crate) async fn enqueue_prepared_decision(
        &self,
        actor: &RadrootsActorContext,
        plan: TradeDecisionPlan,
        target_policy: TargetPolicy,
        publish_mode: PublishMode,
        satisfaction_policy: SatisfactionPolicy,
        idempotency_key: Option<SdkIdempotencyKey>,
    ) -> Result<TradeDecisionReceipt, RadrootsSdkError> {
        validate_trade_enqueue_policy(publish_mode, &satisfaction_policy)?;
        if !self
            .prepared_order_event_exists(&plan.expected_event_id)
            .await?
        {
            self.require_decision_preflight(&plan).await?;
        }
        let enqueue = enqueue_configured_signed_workflow(
            self.sdk,
            SdkWorkflowEnqueueRequest {
                operation_kind: TradeWorkflowKind::Decision.operation_kind(),
                actor,
                frozen_draft: &plan.frozen_draft,
                target_policy: target_policy.workflow_target_policy(),
                satisfaction_policy,
                idempotency_key,
            },
        )
        .await?;
        Ok(order_decision_receipt(plan, enqueue))
    }

    #[cfg(feature = "signer-adapters")]
    pub(crate) fn prepare_cancellation(
        &self,
        request: TradeCancellationPrepareRequest,
    ) -> Result<TradeCancellationPlan, RadrootsSdkError> {
        let created_at = self.resolved_created_at(request.created_at)?;
        order_cancellation_plan(
            &request.actor,
            request.root_event,
            request.previous_event,
            request.cancellation,
            created_at,
        )
    }

    #[cfg(feature = "signer-adapters")]
    pub(crate) async fn enqueue_prepared_cancellation(
        &self,
        actor: &RadrootsActorContext,
        plan: TradeCancellationPlan,
        target_policy: TargetPolicy,
        publish_mode: PublishMode,
        satisfaction_policy: SatisfactionPolicy,
        idempotency_key: Option<SdkIdempotencyKey>,
    ) -> Result<TradeCancellationReceipt, RadrootsSdkError> {
        validate_trade_enqueue_policy(publish_mode, &satisfaction_policy)?;
        if !self
            .prepared_order_event_exists(&plan.expected_event_id)
            .await?
        {
            self.require_cancellation_preflight(&plan).await?;
        }
        let enqueue = enqueue_configured_signed_workflow(
            self.sdk,
            SdkWorkflowEnqueueRequest {
                operation_kind: TradeWorkflowKind::Cancellation.operation_kind(),
                actor,
                frozen_draft: &plan.frozen_draft,
                target_policy: target_policy.workflow_target_policy(),
                satisfaction_policy,
                idempotency_key,
            },
        )
        .await?;
        Ok(order_cancellation_receipt(plan, enqueue))
    }

    pub async fn status(
        &self,
        request: TradeStatusRequest,
    ) -> Result<TradeStatusReceipt, RadrootsSdkError> {
        match request.source {
            SdkTradeStatusSource::LocalOnly => self.local_status(request).await,
            SdkTradeStatusSource::ResyncThenLocal => {
                #[cfg(feature = "transport-nostr-runtime")]
                {
                    let adapter = RadrootsNostrClientFetchAdapter;
                    return self.status_with_fetch_adapter(request, &adapter).await;
                }
                #[cfg(not(feature = "transport-nostr-runtime"))]
                {
                    let _ = request;
                    Err(RadrootsSdkError::ProductSyncUnsupported {
                        operation: "trade.status.resync_then_local",
                        required_feature: "transport-nostr-runtime",
                    })
                }
            }
        }
    }

    pub async fn watch(
        &self,
        request: TradeStatusWatchRequest,
    ) -> Result<TradeStatusWatch, RadrootsSdkError> {
        request.validate()?;
        let handle = tokio::runtime::Handle::try_current().map_err(|_| {
            RadrootsSdkError::InvalidRequest {
                message: "trade status watch requires an active Tokio runtime".to_owned(),
            }
        })?;
        let (updates, receiver) = mpsc::channel(request.capacity);
        let (cancel, cancel_receiver) = oneshot::channel();
        let sdk = self.sdk.clone();
        let capacity = request.capacity;
        let producer = handle.spawn(run_trade_status_watch(
            sdk,
            request,
            updates,
            cancel_receiver,
        ));
        Ok(TradeStatusWatch {
            receiver,
            cancel: Some(cancel),
            producer: Some(producer),
            capacity,
        })
    }

    #[cfg(all(feature = "signer-adapters", feature = "transport-nostr-runtime"))]
    pub async fn status_with_fetch_adapter<A>(
        &self,
        request: TradeStatusRequest,
        adapter: &A,
    ) -> Result<TradeStatusReceipt, RadrootsSdkError>
    where
        A: RadrootsRelayFetchAdapter,
    {
        match request.source {
            SdkTradeStatusSource::LocalOnly => self.local_status(request).await,
            SdkTradeStatusSource::ResyncThenLocal => {
                request.validate()?;
                let execution = execute_trade_resync_with_fetch_adapter(
                    self.sdk,
                    request.locator.clone(),
                    request.limit,
                    adapter,
                    "trade.status.resync_then_local",
                )
                .await?;
                let mut status = self
                    .local_status(
                        TradeStatusRequest::new(request.locator)
                            .with_limit(request.limit)
                            .with_validation_trust_policy(request.validation_trust_policy),
                    )
                    .await?;
                status.source = SdkTradeStatusSource::ResyncThenLocal;
                status.online_evidence = Some(execution.evidence);
                Ok(status)
            }
        }
    }

    async fn local_status(
        &self,
        request: TradeStatusRequest,
    ) -> Result<TradeStatusReceipt, RadrootsSdkError> {
        request.validate()?;
        let query_result = order_projection_query_for_trade_locator(
            &self.sdk._event_store,
            &request.locator,
            request.limit,
        )
        .await
        .map_err(projection_error)?;
        let mut receipt =
            TradeStatusReceipt::from_locator_query_result(request.locator, query_result);
        receipt.source = request.source;
        apply_trade_status_validation_trust(
            self.sdk,
            &mut receipt,
            &request.validation_trust_policy,
        )
        .await?;
        Ok(receipt)
    }

    fn resolved_created_at(
        &self,
        created_at: Option<RadrootsSdkTimestamp>,
    ) -> Result<RadrootsSdkTimestamp, RadrootsSdkError> {
        match created_at {
            Some(created_at) => Ok(created_at),
            None => self.sdk.now(),
        }
    }

    #[cfg(feature = "signer-adapters")]
    async fn require_decision_preflight(
        &self,
        plan: &TradeDecisionPlan,
    ) -> Result<(), RadrootsSdkError> {
        let query_result = self.query_order_projection(&plan.order_id).await?;
        require_decision_request_evidence(plan, &query_result.projection)
    }

    #[cfg(feature = "signer-adapters")]
    async fn require_cancellation_preflight(
        &self,
        plan: &TradeCancellationPlan,
    ) -> Result<(), RadrootsSdkError> {
        let query_result = self.query_order_projection(&plan.order_id).await?;
        require_cancellation_state(plan, &query_result.projection)
    }

    #[cfg(feature = "signer-adapters")]
    async fn query_order_projection(
        &self,
        order_id: &RadrootsOrderId,
    ) -> Result<RadrootsOrderProjectionQueryResult, RadrootsSdkError> {
        order_projection_query_for_order_id(
            &self.sdk._event_store,
            order_id,
            TRADE_STATUS_MAX_LIMIT,
        )
        .await
        .map_err(projection_error)
    }

    #[cfg(feature = "signer-adapters")]
    async fn prepared_order_event_exists(
        &self,
        expected_event_id: &RadrootsEventId,
    ) -> Result<bool, RadrootsSdkError> {
        self.sdk
            ._event_store
            .get_event(expected_event_id.as_str())
            .await
            .map(|event| event.is_some())
            .map_err(|error| RadrootsSdkError::EventStore {
                message: error.to_string(),
            })
    }
}

#[cfg(feature = "runtime")]
async fn run_trade_status_watch(
    sdk: crate::RadrootsClient,
    request: TradeStatusWatchRequest,
    updates: mpsc::Sender<Result<TradeStatusWatchUpdate, RadrootsSdkError>>,
    mut cancel: oneshot::Receiver<()>,
) {
    let refresh_interval = Duration::from_millis(request.refresh_interval_ms);
    let mut sequence = 0_u64;
    loop {
        let status_result = sdk.trades().status(request.status.clone()).await;
        let stop_after_update = status_result
            .as_ref()
            .map(|status| status.lifecycle_terminal)
            .unwrap_or(true);
        sequence += 1;
        let update_result = status_result.map(|status| TradeStatusWatchUpdate { sequence, status });
        let send_result = tokio::select! {
            _ = &mut cancel => break,
            send_result = updates.send(update_result) => send_result,
        };
        if send_result.is_err() || stop_after_update {
            break;
        }
        if request
            .refresh_limit
            .map(|refresh_limit| sequence >= u64::from(refresh_limit))
            .unwrap_or(false)
        {
            break;
        }
        tokio::select! {
            _ = &mut cancel => break,
            _ = tokio::time::sleep(refresh_interval) => {}
        }
    }
}

#[cfg(feature = "runtime")]
impl<'sdk> TradeResyncClient<'sdk> {
    pub async fn resync(
        &self,
        request: TradeResyncRequest,
    ) -> Result<TradeResyncReceipt, RadrootsSdkError> {
        #[cfg(feature = "transport-nostr-runtime")]
        {
            let adapter = RadrootsNostrClientFetchAdapter;
            return self.resync_with_fetch_adapter(request, &adapter).await;
        }
        #[cfg(not(feature = "transport-nostr-runtime"))]
        {
            let _ = self.sdk;
            let _ = request;
            Err(RadrootsSdkError::ProductSyncUnsupported {
                operation: "trade.resync",
                required_feature: "transport-nostr-runtime",
            })
        }
    }

    #[cfg(feature = "transport-nostr-runtime")]
    pub async fn resync_with_fetch_adapter<A>(
        &self,
        request: TradeResyncRequest,
        adapter: &A,
    ) -> Result<TradeResyncReceipt, RadrootsSdkError>
    where
        A: RadrootsRelayFetchAdapter,
    {
        request.validate()?;
        let execution = execute_trade_resync_with_fetch_adapter(
            self.sdk,
            request.locator.clone(),
            request.limit,
            adapter,
            "trade.resync",
        )
        .await?;
        let status = trades_client(self.sdk)
            .status(
                TradeStatusRequest::new(request.locator.clone())
                    .with_limit(request.limit)
                    .with_validation_trust_policy(request.validation_trust_policy.clone()),
            )
            .await?;
        if status.status == TradeStatusKind::Ambiguous {
            return Err(RadrootsSdkError::TradeAmbiguous {
                operation: "trade.resync".to_owned(),
                locator: Box::new(request.locator),
                candidates: status
                    .ambiguity_candidates
                    .iter()
                    .map(|candidate| candidate.locator.clone())
                    .collect(),
            });
        }
        Ok(TradeResyncReceipt {
            nostr_relay_urls: execution.nostr_relay_urls,
            evidence: execution.evidence,
            refresh: execution.refresh,
            status,
        })
    }
}

#[cfg(all(feature = "runtime", feature = "transport-nostr-runtime"))]
struct TradeResyncExecution {
    nostr_relay_urls: Vec<String>,
    evidence: TradeResyncEvidenceReceipt,
    refresh: SyncProjectionRefreshReceipt,
}

#[cfg(all(feature = "runtime", feature = "transport-nostr-runtime"))]
async fn execute_trade_resync_with_fetch_adapter<A>(
    sdk: &crate::RadrootsClient,
    locator: RadrootsTradeLocator,
    limit: u32,
    adapter: &A,
    operation: &'static str,
) -> Result<TradeResyncExecution, RadrootsSdkError>
where
    A: RadrootsRelayFetchAdapter,
{
    let nostr_relay_urls = sdk.configured_nostr_relay_urls().to_vec();
    if nostr_relay_urls.is_empty() {
        return Err(RadrootsSdkError::empty_transport_targets(operation));
    }
    let query_plan = trade_evidence_query_plan(locator, limit)?;
    let fetch_request = trade_evidence_fetch_request(sdk, &query_plan, &nostr_relay_urls)?;
    let fetch_receipt =
        fetch_and_ingest_relay_events(adapter, &sdk._event_store, fetch_request).await?;
    if trade_resync_total_relay_failure(&fetch_receipt, nostr_relay_urls.len()) {
        return Err(RadrootsSdkError::ProductSyncTransportSetupFailure {
            message: trade_resync_total_failure_message(operation, &fetch_receipt),
        });
    }
    let evidence = TradeResyncEvidenceReceipt::from_fetch(sdk, query_plan, fetch_receipt).await?;
    let refresh = refresh_product_projections_for_sdk(
        sdk,
        SyncProjectionRefreshRequest::new().with_limit(limit),
    )
    .await?;
    Ok(TradeResyncExecution {
        nostr_relay_urls,
        evidence,
        refresh,
    })
}

#[cfg(all(feature = "runtime", feature = "transport-nostr-runtime"))]
fn trade_evidence_fetch_request(
    sdk: &crate::RadrootsClient,
    query_plan: &TradeEvidenceQueryPlan,
    nostr_relay_urls: &[String],
) -> Result<RadrootsRelayFetchRequest, RadrootsSdkError> {
    let mut filters = Vec::new();
    for branch in query_plan
        .branches
        .iter()
        .filter(|branch| branch.filter.active)
    {
        filters.extend(trade_evidence_branch_filters(branch)?);
    }
    if filters.is_empty() {
        return Err(RadrootsSdkError::InvalidRequest {
            message: "trade evidence query plan has no active relay filters".to_owned(),
        });
    }
    Ok(
        RadrootsRelayFetchRequest::fetch(sdk_now_ms(sdk)?, query_plan.limit as usize, filters)?
            .with_relay_urls(nostr_relay_urls.iter().cloned()),
    )
}

#[cfg(all(feature = "runtime", feature = "transport-nostr-runtime"))]
fn trade_evidence_query_plan(
    locator: RadrootsTradeLocator,
    limit: u32,
) -> Result<TradeEvidenceQueryPlan, RadrootsSdkError> {
    let order_id = locator.order_id().as_str().to_owned();
    let root_event_id = locator
        .root_event_id
        .as_ref()
        .map(|value| value.as_str().to_owned());
    let lifecycle_tag = root_event_id
        .map(|event_id| (TAG_E, vec![event_id]))
        .unwrap_or_else(|| (TAG_D, vec![order_id.clone()]));
    let listing_filter = locator
        .listing_addr
        .as_ref()
        .map(|value| listing_snapshot_filter_parts(value.as_str()))
        .transpose()?;
    let branches = vec![
        trade_evidence_branch(
            TradeEvidenceQueryBranchKind::RequestRoots,
            vec![KIND_ORDER_REQUEST],
            Some((TAG_D, vec![order_id.clone()])),
            None,
            limit,
            true,
        ),
        trade_evidence_branch(
            TradeEvidenceQueryBranchKind::LifecycleChain,
            vec![KIND_ORDER_DECISION, KIND_ORDER_CANCELLATION],
            Some(lifecycle_tag),
            None,
            limit,
            true,
        ),
        trade_evidence_branch(
            TradeEvidenceQueryBranchKind::ValidationReceipts,
            vec![KIND_TRADE_VALIDATION_RECEIPT],
            Some((TAG_D, vec![order_id])),
            None,
            limit,
            true,
        ),
        trade_evidence_branch(
            TradeEvidenceQueryBranchKind::ListingSnapshot,
            vec![KIND_LISTING],
            listing_filter
                .as_ref()
                .map(|parts| (TAG_D, vec![parts.listing_id.clone()])),
            listing_filter
                .as_ref()
                .map(|parts| parts.seller_pubkey.clone()),
            limit,
            locator.listing_addr.is_some(),
        ),
    ];
    Ok(TradeEvidenceQueryPlan {
        locator,
        limit,
        branches,
    })
}

#[cfg(all(feature = "runtime", feature = "transport-nostr-runtime"))]
fn trade_evidence_branch(
    kind: TradeEvidenceQueryBranchKind,
    event_kinds: Vec<u32>,
    tag: Option<(&'static str, Vec<String>)>,
    author_pubkey: Option<String>,
    limit: u32,
    active: bool,
) -> TradeEvidenceQueryBranch {
    TradeEvidenceQueryBranch {
        kind,
        filter: TradeEvidenceNostrRelayFilter {
            active,
            event_kinds,
            author_pubkey,
            tag: tag.map(|(tag_name, values)| TradeEvidenceNostrRelayTagFilter {
                tag_name: tag_name.to_owned(),
                values,
            }),
            limit,
        },
    }
}

#[cfg(all(feature = "runtime", feature = "transport-nostr-runtime"))]
fn trade_evidence_branch_filters(
    branch: &TradeEvidenceQueryBranch,
) -> Result<Vec<RadrootsNostrFilter>, RadrootsSdkError> {
    if branch.filter.event_kinds.is_empty() {
        return Err(RadrootsSdkError::InvalidRequest {
            message: "trade evidence branch has no event kinds".to_owned(),
        });
    }
    branch
        .filter
        .event_kinds
        .iter()
        .map(|kind| trade_evidence_branch_filter(branch, *kind))
        .collect()
}

#[cfg(all(feature = "runtime", feature = "transport-nostr-runtime"))]
fn trade_evidence_branch_filter(
    branch: &TradeEvidenceQueryBranch,
    kind: u32,
) -> Result<RadrootsNostrFilter, RadrootsSdkError> {
    let mut filter = RadrootsNostrFilter::new().limit(branch.filter.limit as usize);
    let nostr_kind = u16::try_from(kind).map_err(|_| RadrootsSdkError::InvalidRequest {
        message: format!("trade evidence event kind {kind} exceeds Nostr filter range"),
    })?;
    filter = filter.kind(RadrootsNostrKind::Custom(nostr_kind));
    if let Some(author_pubkey) = branch.filter.author_pubkey.as_ref() {
        let author = author_pubkey
            .parse::<RadrootsNostrPublicKey>()
            .map_err(|error| RadrootsSdkError::InvalidRequest {
                message: format!("trade evidence filter author invalid: {error}"),
            })?;
        filter = filter.author(author);
    }
    match branch.filter.tag.as_ref() {
        Some(tag) => radroots_nostr_filter_tag(filter, tag.tag_name.as_str(), tag.values.clone())
            .map_err(|error| RadrootsSdkError::InvalidRequest {
                message: format!("trade evidence filter invalid: {error}"),
            }),
        None => Ok(filter),
    }
}

#[cfg(all(feature = "runtime", feature = "transport-nostr-runtime"))]
struct ListingSnapshotFilterParts {
    seller_pubkey: String,
    listing_id: String,
}

#[cfg(all(feature = "runtime", feature = "transport-nostr-runtime"))]
fn listing_snapshot_filter_parts(
    listing_addr: &str,
) -> Result<ListingSnapshotFilterParts, RadrootsSdkError> {
    let parts =
        parse_listing_address(listing_addr).map_err(|error| RadrootsSdkError::InvalidRequest {
            message: format!("trade listing snapshot filter invalid: {error}"),
        })?;
    Ok(ListingSnapshotFilterParts {
        seller_pubkey: parts.seller_pubkey.as_str().to_owned(),
        listing_id: parts.listing_id.as_str().to_owned(),
    })
}

#[cfg(all(feature = "runtime", feature = "transport-nostr-runtime"))]
fn trade_resync_total_relay_failure(
    receipt: &RadrootsRelayFetchReceipt,
    relay_count: usize,
) -> bool {
    relay_count > 0 && receipt.eose_count == 0 && receipt.closed_count >= relay_count
}

#[cfg(all(feature = "runtime", feature = "transport-nostr-runtime"))]
fn trade_resync_total_failure_message(
    operation: &str,
    receipt: &RadrootsRelayFetchReceipt,
) -> String {
    format!(
        "{operation} failed for all configured relays: closed_count={}, notice_count={}, malformed_count={}",
        receipt.closed_count, receipt.notice_count, receipt.malformed_count
    )
}

#[cfg(all(feature = "runtime", feature = "transport-nostr-runtime"))]
impl TradeResyncEvidenceReceipt {
    async fn from_fetch(
        sdk: &crate::RadrootsClient,
        query_plan: TradeEvidenceQueryPlan,
        receipt: RadrootsRelayFetchReceipt,
    ) -> Result<Self, RadrootsSdkError> {
        let events = receipt
            .events
            .into_iter()
            .map(Into::into)
            .collect::<Vec<_>>();
        let nostr_relay_outcomes = receipt
            .relay_outcomes
            .into_iter()
            .map(Into::into)
            .collect::<Vec<_>>();
        let branches = trade_evidence_branch_receipts(
            sdk,
            &query_plan,
            events.as_slice(),
            nostr_relay_outcomes.as_slice(),
        )
        .await?;
        Ok(Self {
            query_plan,
            inserted_count: receipt.inserted_count,
            duplicate_count: receipt.duplicate_count,
            malformed_count: receipt.malformed_count,
            out_of_filter_count: receipt.out_of_filter_count,
            skipped_over_limit_count: receipt.skipped_over_limit_count,
            unsupported_count: receipt.unsupported_count,
            eose_count: receipt.eose_count,
            closed_count: receipt.closed_count,
            notice_count: receipt.notice_count,
            branches,
            events,
            nostr_relay_outcomes,
        })
    }
}

#[cfg(all(feature = "runtime", feature = "transport-nostr-runtime"))]
async fn trade_evidence_branch_receipts(
    sdk: &crate::RadrootsClient,
    query_plan: &TradeEvidenceQueryPlan,
    events: &[TradeResyncEventImportReceipt],
    nostr_relay_outcomes: &[TradeResyncNostrRelayOutcomeReceipt],
) -> Result<Vec<TradeEvidenceBranchReceipt>, RadrootsSdkError> {
    let mut branch_events = query_plan
        .branches
        .iter()
        .filter(|branch| branch.filter.active)
        .map(|branch| (branch.kind, Vec::<TradeResyncEventImportReceipt>::new()))
        .collect::<BTreeMap<_, _>>();
    branch_events
        .entry(TradeEvidenceQueryBranchKind::RejectedEvidence)
        .or_default();
    for event in events {
        let branch = trade_evidence_event_branch(sdk, query_plan, event).await?;
        branch_events.entry(branch).or_default().push(event.clone());
    }
    let nostr_relay_failure_count = nostr_relay_outcomes
        .iter()
        .filter(|relay| relay.outcome_kind != TradeResyncNostrRelayOutcomeKind::Eose)
        .count();
    Ok(branch_events
        .into_iter()
        .map(|(branch, events)| {
            TradeEvidenceBranchReceipt::from_parts(
                branch,
                events,
                nostr_relay_outcomes.to_vec(),
                nostr_relay_failure_count,
            )
        })
        .collect())
}

#[cfg(all(feature = "runtime", feature = "transport-nostr-runtime"))]
async fn trade_evidence_event_branch(
    sdk: &crate::RadrootsClient,
    query_plan: &TradeEvidenceQueryPlan,
    event: &TradeResyncEventImportReceipt,
) -> Result<TradeEvidenceQueryBranchKind, RadrootsSdkError> {
    if event.malformed || event.out_of_filter || event.skipped_over_limit {
        return Ok(TradeEvidenceQueryBranchKind::RejectedEvidence);
    }
    let Some(event_id) = event.event_id.as_deref() else {
        return Ok(TradeEvidenceQueryBranchKind::RejectedEvidence);
    };
    let Some(stored_event) = sdk
        ._event_store
        .get_event(event_id)
        .await
        .map_err(|error| RadrootsSdkError::EventStore {
            message: error.to_string(),
        })?
    else {
        return Ok(TradeEvidenceQueryBranchKind::RejectedEvidence);
    };
    let stored_tags = sdk
        ._event_store
        .tags_for_event(event_id)
        .await
        .map_err(|error| RadrootsSdkError::EventStore {
            message: error.to_string(),
        })?;
    Ok(query_plan
        .branches
        .iter()
        .filter(|branch| branch.filter.active)
        .find(|branch| trade_evidence_branch_matches_event(branch, &stored_event, &stored_tags))
        .map(|branch| branch.kind)
        .unwrap_or(TradeEvidenceQueryBranchKind::RejectedEvidence))
}

#[cfg(all(feature = "runtime", feature = "transport-nostr-runtime"))]
fn trade_evidence_branch_matches_event(
    branch: &TradeEvidenceQueryBranch,
    stored_event: &RadrootsStoredEvent,
    stored_tags: &[RadrootsStoredEventTag],
) -> bool {
    if !branch.filter.event_kinds.contains(&stored_event.kind) {
        return false;
    }
    if let Some(author_pubkey) = branch.filter.author_pubkey.as_ref()
        && stored_event.pubkey != *author_pubkey
    {
        return false;
    }
    match branch.filter.tag.as_ref() {
        Some(tag) => stored_tags.iter().any(|stored_tag| {
            stored_tag.tag_name == tag.tag_name
                && stored_tag
                    .tag_value
                    .as_ref()
                    .is_some_and(|value| tag.values.iter().any(|expected| expected == value))
        }),
        None => true,
    }
}

#[cfg(all(feature = "runtime", feature = "transport-nostr-runtime"))]
impl TradeEvidenceBranchReceipt {
    fn from_parts(
        branch: TradeEvidenceQueryBranchKind,
        events: Vec<TradeResyncEventImportReceipt>,
        nostr_relay_outcomes: Vec<TradeResyncNostrRelayOutcomeReceipt>,
        nostr_relay_failure_count: usize,
    ) -> Self {
        let inserted_count = events.iter().filter(|event| event.inserted).count();
        let duplicate_count = events.iter().filter(|event| event.duplicate).count();
        let malformed_count = events.iter().filter(|event| event.malformed).count();
        let out_of_filter_count = events.iter().filter(|event| event.out_of_filter).count();
        let skipped_over_limit_count = events
            .iter()
            .filter(|event| event.skipped_over_limit)
            .count();
        let unsupported_count = events.iter().filter(|event| event.unsupported).count();
        let accepted_count = events
            .iter()
            .filter(|event| {
                !event.malformed
                    && !event.out_of_filter
                    && !event.skipped_over_limit
                    && !event.unsupported
                    && (event.inserted || event.duplicate)
            })
            .count();
        let empty_result = accepted_count == 0
            && duplicate_count == 0
            && malformed_count == 0
            && out_of_filter_count == 0
            && skipped_over_limit_count == 0
            && unsupported_count == 0;
        Self {
            branch,
            accepted_count,
            inserted_count,
            duplicate_count,
            malformed_count,
            out_of_filter_count,
            skipped_over_limit_count,
            unsupported_count,
            nostr_relay_failure_count,
            empty_result,
            events,
            nostr_relay_outcomes,
        }
    }
}

#[cfg(all(feature = "runtime", feature = "transport-nostr-runtime"))]
impl From<RadrootsRelayFetchEventReceipt> for TradeResyncEventImportReceipt {
    fn from(receipt: RadrootsRelayFetchEventReceipt) -> Self {
        Self {
            nostr_relay_url: receipt.relay_url,
            event_id: receipt.event_id,
            inserted: receipt.inserted,
            duplicate: receipt.duplicate,
            unsupported: receipt.unsupported,
            malformed: receipt.malformed,
            out_of_filter: receipt.out_of_filter,
            skipped_over_limit: receipt.skipped_over_limit,
            projection_eligible: receipt.projection_eligible,
            verification_status: receipt.verification_status,
            message: receipt.message,
        }
    }
}

#[cfg(all(feature = "runtime", feature = "transport-nostr-runtime"))]
impl From<RadrootsRelayFetchRelayOutcome> for TradeResyncNostrRelayOutcomeReceipt {
    fn from(receipt: RadrootsRelayFetchRelayOutcome) -> Self {
        Self {
            nostr_relay_url: receipt.relay_url,
            outcome_kind: receipt.kind.into(),
            transport_outcome_kind: receipt.relay_outcome.map(|outcome| outcome.kind.into()),
            message: receipt.message,
        }
    }
}

#[cfg(all(feature = "runtime", feature = "transport-nostr-runtime"))]
impl From<RadrootsRelayFetchOutcomeKind> for TradeResyncNostrRelayOutcomeKind {
    fn from(kind: RadrootsRelayFetchOutcomeKind) -> Self {
        match kind {
            RadrootsRelayFetchOutcomeKind::Eose => Self::Eose,
            RadrootsRelayFetchOutcomeKind::Closed => Self::Closed,
            RadrootsRelayFetchOutcomeKind::Notice => Self::Notice,
        }
    }
}

#[cfg(all(feature = "runtime", feature = "transport-nostr-runtime"))]
impl From<RadrootsRelayOutcomeKind> for TradeResyncNostrRelayTransportOutcomeKind {
    fn from(kind: RadrootsRelayOutcomeKind) -> Self {
        match kind {
            RadrootsRelayOutcomeKind::Accepted => Self::Accepted,
            RadrootsRelayOutcomeKind::DuplicateAccepted => Self::DuplicateAccepted,
            RadrootsRelayOutcomeKind::Blocked => Self::Blocked,
            RadrootsRelayOutcomeKind::RateLimited => Self::RateLimited,
            RadrootsRelayOutcomeKind::Invalid => Self::Invalid,
            RadrootsRelayOutcomeKind::PowRequired => Self::PowRequired,
            RadrootsRelayOutcomeKind::Restricted => Self::Restricted,
            RadrootsRelayOutcomeKind::AuthRequired => Self::AuthRequired,
            RadrootsRelayOutcomeKind::Muted => Self::Muted,
            RadrootsRelayOutcomeKind::Unsupported => Self::Unsupported,
            RadrootsRelayOutcomeKind::PaymentRequired => Self::PaymentRequired,
            RadrootsRelayOutcomeKind::Error => Self::Error,
            RadrootsRelayOutcomeKind::Timeout => Self::Timeout,
            RadrootsRelayOutcomeKind::ConnectionFailed => Self::ConnectionFailed,
            RadrootsRelayOutcomeKind::RelayUrlRejected => Self::RelayUrlRejected,
            RadrootsRelayOutcomeKind::SkippedAlreadyAccepted => Self::SkippedAlreadyAccepted,
            RadrootsRelayOutcomeKind::Unknown => Self::Unknown,
        }
    }
}

#[cfg(feature = "runtime")]
impl<'sdk> TradeValidationReceiptsClient<'sdk> {
    pub async fn list(
        &self,
        request: TradeValidationReceiptListRequest,
    ) -> Result<TradeValidationReceiptListReceipt, RadrootsSdkError> {
        #[cfg(feature = "transport-nostr-runtime")]
        {
            let adapter = RadrootsNostrClientFetchAdapter;
            return self.list_with_fetch_adapter(request, &adapter).await;
        }
        #[cfg(not(feature = "transport-nostr-runtime"))]
        {
            let _ = self.sdk;
            let _ = request;
            Err(RadrootsSdkError::ProductSyncUnsupported {
                operation: "trade.validation_receipts.list",
                required_feature: "transport-nostr-runtime",
            })
        }
    }

    #[cfg(feature = "transport-nostr-runtime")]
    pub async fn list_with_fetch_adapter<A>(
        &self,
        request: TradeValidationReceiptListRequest,
        adapter: &A,
    ) -> Result<TradeValidationReceiptListReceipt, RadrootsSdkError>
    where
        A: RadrootsRelayFetchAdapter,
    {
        request.validate()?;
        let nostr_relay_urls =
            validation_receipt_nostr_relay_urls(self.sdk, "trade.validation_receipts.list")?;
        let fetch_request =
            validation_receipt_list_fetch_request(self.sdk, &request, &nostr_relay_urls)?;
        let fetch_receipt =
            fetch_and_ingest_relay_events(adapter, &self.sdk._event_store, fetch_request).await?;
        let nostr_evidence = TradeValidationReceiptNostrEvidenceReceipt::from_fetch(fetch_receipt);
        let events = validation_receipt_events_from_fetch(
            self.sdk,
            &nostr_evidence.events,
            KIND_TRADE_VALIDATION_RECEIPT,
        )
        .await?;
        let (mut receipts, mut invalid_receipts) =
            classify_validation_receipts(events, Some(request.order_id.as_str()))?;
        receipts.sort_by(validation_receipt_event_order);
        invalid_receipts.sort_by(validation_receipt_invalid_order);
        Ok(TradeValidationReceiptListReceipt {
            nostr_relay_urls,
            nostr_evidence,
            order_id: request.order_id,
            receipts,
            invalid_receipts,
        })
    }

    pub async fn inspect(
        &self,
        request: TradeValidationReceiptInspectRequest,
    ) -> Result<TradeValidationReceiptInspectReceipt, RadrootsSdkError> {
        #[cfg(feature = "transport-nostr-runtime")]
        {
            let adapter = RadrootsNostrClientFetchAdapter;
            return self.inspect_with_fetch_adapter(request, &adapter).await;
        }
        #[cfg(not(feature = "transport-nostr-runtime"))]
        {
            let _ = self.sdk;
            let _ = request;
            Err(RadrootsSdkError::ProductSyncUnsupported {
                operation: "trade.validation_receipts.inspect",
                required_feature: "transport-nostr-runtime",
            })
        }
    }

    #[cfg(feature = "transport-nostr-runtime")]
    pub async fn inspect_with_fetch_adapter<A>(
        &self,
        request: TradeValidationReceiptInspectRequest,
        adapter: &A,
    ) -> Result<TradeValidationReceiptInspectReceipt, RadrootsSdkError>
    where
        A: RadrootsRelayFetchAdapter,
    {
        validation_receipt_inspect(
            self.sdk,
            request.receipt_event_id,
            adapter,
            "trade.validation_receipts.inspect",
        )
        .await
    }

    pub async fn verify(
        &self,
        request: TradeValidationReceiptVerifyRequest,
    ) -> Result<TradeValidationReceiptInspectReceipt, RadrootsSdkError> {
        #[cfg(feature = "transport-nostr-runtime")]
        {
            let adapter = RadrootsNostrClientFetchAdapter;
            return self.verify_with_fetch_adapter(request, &adapter).await;
        }
        #[cfg(not(feature = "transport-nostr-runtime"))]
        {
            let _ = self.sdk;
            let _ = request;
            Err(RadrootsSdkError::ProductSyncUnsupported {
                operation: "trade.validation_receipts.verify",
                required_feature: "transport-nostr-runtime",
            })
        }
    }

    #[cfg(feature = "transport-nostr-runtime")]
    pub async fn verify_with_fetch_adapter<A>(
        &self,
        request: TradeValidationReceiptVerifyRequest,
        adapter: &A,
    ) -> Result<TradeValidationReceiptInspectReceipt, RadrootsSdkError>
    where
        A: RadrootsRelayFetchAdapter,
    {
        validation_receipt_inspect(
            self.sdk,
            request.receipt_event_id,
            adapter,
            "trade.validation_receipts.verify",
        )
        .await
    }
}

#[cfg(all(feature = "runtime", feature = "transport-nostr-runtime"))]
async fn validation_receipt_inspect<A>(
    sdk: &crate::RadrootsClient,
    receipt_event_id: RadrootsEventId,
    adapter: &A,
    operation: &'static str,
) -> Result<TradeValidationReceiptInspectReceipt, RadrootsSdkError>
where
    A: RadrootsRelayFetchAdapter,
{
    let nostr_relay_urls = validation_receipt_nostr_relay_urls(sdk, operation)?;
    let fetch_request =
        validation_receipt_inspect_fetch_request(sdk, &receipt_event_id, &nostr_relay_urls)?;
    let fetch_receipt =
        fetch_and_ingest_relay_events(adapter, &sdk._event_store, fetch_request).await?;
    let nostr_evidence = TradeValidationReceiptNostrEvidenceReceipt::from_fetch(fetch_receipt);
    let events = validation_receipt_events_from_fetch(
        sdk,
        &nostr_evidence.events,
        KIND_TRADE_VALIDATION_RECEIPT,
    )
    .await?;
    let (mut receipts, mut invalid_receipts) = classify_validation_receipts(events, None)?;
    receipts.retain(|receipt| receipt.event.id_str() == receipt_event_id.as_str());
    invalid_receipts.retain(|receipt| receipt.event.id_str() == receipt_event_id.as_str());
    receipts.sort_by(validation_receipt_event_order);
    invalid_receipts.sort_by(validation_receipt_invalid_order);
    Ok(TradeValidationReceiptInspectReceipt {
        nostr_relay_urls,
        nostr_evidence,
        receipt_event_id,
        receipt: receipts.into_iter().next(),
        invalid_receipt: invalid_receipts.into_iter().next(),
    })
}

#[cfg(all(feature = "runtime", feature = "transport-nostr-runtime"))]
fn validation_receipt_nostr_relay_urls(
    sdk: &crate::RadrootsClient,
    operation: impl Into<String>,
) -> Result<Vec<String>, RadrootsSdkError> {
    let nostr_relay_urls = sdk.configured_nostr_relay_urls().to_vec();
    if nostr_relay_urls.is_empty() {
        return Err(RadrootsSdkError::empty_transport_targets(operation));
    }
    Ok(nostr_relay_urls)
}

#[cfg(all(feature = "runtime", feature = "transport-nostr-runtime"))]
fn validation_receipt_list_fetch_request(
    sdk: &crate::RadrootsClient,
    request: &TradeValidationReceiptListRequest,
    nostr_relay_urls: &[String],
) -> Result<RadrootsRelayFetchRequest, RadrootsSdkError> {
    let filter = RadrootsNostrFilter::new()
        .kind(RadrootsNostrKind::Custom(
            KIND_TRADE_VALIDATION_RECEIPT as u16,
        ))
        .limit(request.limit as usize);
    let filter =
        radroots_nostr_filter_tag(filter, TAG_D, vec![request.order_id.as_str().to_owned()])
            .map_err(|error| RadrootsSdkError::InvalidRequest {
                message: format!("validation receipt list filter invalid: {error}"),
            })?;
    Ok(
        RadrootsRelayFetchRequest::fetch(sdk_now_ms(sdk)?, request.limit as usize, [filter])?
            .with_relay_urls(nostr_relay_urls.iter().cloned()),
    )
}

#[cfg(all(feature = "runtime", feature = "transport-nostr-runtime"))]
fn validation_receipt_inspect_fetch_request(
    sdk: &crate::RadrootsClient,
    receipt_event_id: &RadrootsEventId,
    nostr_relay_urls: &[String],
) -> Result<RadrootsRelayFetchRequest, RadrootsSdkError> {
    let nostr_event_id =
        RadrootsNostrEventId::parse(receipt_event_id.as_str()).map_err(|error| {
            RadrootsSdkError::InvalidRequest {
                message: format!(
                    "validation receipt event id `{}` cannot build relay filter: {error}",
                    receipt_event_id.as_str()
                ),
            }
        })?;
    let filter = RadrootsNostrFilter::new().id(nostr_event_id);
    Ok(
        RadrootsRelayFetchRequest::fetch(sdk_now_ms(sdk)?, 1, [filter])?
            .with_relay_urls(nostr_relay_urls.iter().cloned()),
    )
}

#[cfg(all(feature = "runtime", feature = "transport-nostr-runtime"))]
async fn validation_receipt_events_from_fetch(
    sdk: &crate::RadrootsClient,
    events: &[TradeResyncEventImportReceipt],
    expected_kind: u32,
) -> Result<Vec<RadrootsEventEnvelope>, RadrootsSdkError> {
    let mut event_ids = events
        .iter()
        .filter(|event| !event.malformed)
        .filter(|event| !event.out_of_filter)
        .filter_map(|event| event.event_id.as_deref())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    event_ids.sort();
    let mut fetched = Vec::new();
    for event_id in event_ids {
        let Some(stored_event) = sdk
            ._event_store
            .get_event(event_id)
            .await
            .map_err(|error| RadrootsSdkError::EventStore {
                message: error.to_string(),
            })?
        else {
            continue;
        };
        if stored_event.kind == expected_kind {
            fetched.push(stored_event_to_nostr_event(&stored_event)?);
        }
    }
    Ok(fetched)
}

#[cfg(all(feature = "runtime", feature = "transport-nostr-runtime"))]
fn classify_validation_receipts(
    events: Vec<RadrootsEventEnvelope>,
    expected_order_id: Option<&str>,
) -> Result<
    (
        Vec<TradeValidationReceiptEvent>,
        Vec<TradeValidationReceiptInvalidCandidate>,
    ),
    RadrootsSdkError,
> {
    let mut receipts = Vec::new();
    let mut invalid_receipts = Vec::new();
    for event in events {
        let expected = RadrootsValidationReceiptExpectedBinding {
            order_id: expected_order_id,
            ..RadrootsValidationReceiptExpectedBinding::default()
        };
        match verify_validation_receipt_event(&event, expected) {
            Ok(verified) => receipts.push(TradeValidationReceiptEvent {
                event,
                receipt: verified.receipt,
                tags: TradeValidationReceiptTags::from(verified.tags),
            }),
            Err(error) => invalid_receipts.push(TradeValidationReceiptInvalidCandidate {
                event,
                reason_code: validation_receipt_invalid_reason_code(&error).to_owned(),
                reason: error.to_string(),
            }),
        }
    }
    Ok((receipts, invalid_receipts))
}

#[cfg(feature = "runtime")]
async fn apply_trade_status_validation_trust(
    sdk: &crate::RadrootsClient,
    status: &mut TradeStatusReceipt,
    policy: &RadrootsTradeValidationTrustPolicy,
) -> Result<(), RadrootsSdkError> {
    if !status.found
        || matches!(
            status.status,
            TradeStatusKind::Missing | TradeStatusKind::Ambiguous
        )
    {
        return Ok(());
    }
    let decision = trade_status_validation_trust_decision(sdk, status, policy).await?;
    status.validation_trust = Some(decision);
    apply_validation_trust_decision_to_status(status);
    Ok(())
}

#[cfg(feature = "runtime")]
async fn trade_status_validation_trust_decision(
    sdk: &crate::RadrootsClient,
    status: &TradeStatusReceipt,
    policy: &RadrootsTradeValidationTrustPolicy,
) -> Result<TradeValidationTrustDecision, RadrootsSdkError> {
    let Some(receipt_event_id) = status.rhi_receipt_event_id.clone() else {
        return Ok(trade_validation_trust_decision(
            policy,
            RadrootsTradeValidationTrustState::Pending,
            TradeValidationTrustEvidence::none(),
            false,
            Some(TradeValidationTrustReason::new(
                "validation_receipt_missing",
                "validation receipt is not present in local product evidence",
            )),
        ));
    };
    let Some(stored_event) = sdk
        ._event_store
        .get_event(receipt_event_id.as_str())
        .await
        .map_err(|error| RadrootsSdkError::EventStore {
            message: error.to_string(),
        })?
    else {
        return Ok(trade_validation_trust_decision(
            policy,
            RadrootsTradeValidationTrustState::Pending,
            TradeValidationTrustEvidence::receipt_event(receipt_event_id),
            false,
            Some(TradeValidationTrustReason::new(
                "validation_receipt_event_missing",
                "validation receipt event is referenced by projection but missing from storage",
            )),
        ));
    };
    if stored_event.kind != KIND_TRADE_VALIDATION_RECEIPT {
        return Ok(trade_validation_trust_decision(
            policy,
            RadrootsTradeValidationTrustState::Invalid,
            TradeValidationTrustEvidence::receipt_event(receipt_event_id),
            false,
            Some(TradeValidationTrustReason::new(
                "validation_receipt_event_kind_invalid",
                "validation receipt reference does not point to a validation receipt event",
            )),
        ));
    }
    let event = stored_event_to_nostr_event(&stored_event)?;
    let receipt_author = match RadrootsPublicKey::parse(event.author_str()) {
        Ok(author) => author,
        Err(_) => {
            return Ok(trade_validation_trust_decision(
                policy,
                RadrootsTradeValidationTrustState::Invalid,
                TradeValidationTrustEvidence::receipt_event(receipt_event_id),
                false,
                Some(TradeValidationTrustReason::new(
                    "validation_receipt_author_invalid",
                    "validation receipt author is not a valid public key",
                )),
            ));
        }
    };
    let expected = RadrootsValidationReceiptExpectedBinding {
        order_id: Some(status.order_id.as_str()),
        root_event_id: status
            .request_event_id
            .as_ref()
            .map(RadrootsEventId::as_str),
        target_event_id: status
            .agreement_event_id
            .as_ref()
            .or(status.decision_event_id.as_ref())
            .map(RadrootsEventId::as_str),
        ..RadrootsValidationReceiptExpectedBinding::default()
    };
    let verified = match verify_validation_receipt_event(&event, expected) {
        Ok(verified) => verified,
        Err(error) => {
            let reason = error.to_string();
            return Ok(trade_validation_trust_decision(
                policy,
                RadrootsTradeValidationTrustState::Invalid,
                TradeValidationTrustEvidence::receipt_author(receipt_event_id, receipt_author),
                false,
                Some(TradeValidationTrustReason::new(
                    "validation_receipt_invalid",
                    reason.as_str(),
                )),
            ));
        }
    };
    let receipt = TradeValidationReceiptEvent {
        event,
        receipt: verified.receipt,
        tags: TradeValidationReceiptTags::from(verified.tags),
    };
    if receipt.receipt.result == RadrootsValidationReceiptResult::Invalid {
        return Ok(trade_validation_trust_decision(
            policy,
            RadrootsTradeValidationTrustState::Invalid,
            TradeValidationTrustEvidence::receipt(
                receipt_event_id,
                receipt_author,
                &receipt.receipt,
                None,
            ),
            false,
            Some(TradeValidationTrustReason::new(
                "validation_receipt_result_invalid",
                "validation receipt reports an invalid trade transition",
            )),
        ));
    }
    if policy.trusted_rhi_pubkeys.is_empty() {
        return Ok(trade_validation_trust_decision(
            policy,
            RadrootsTradeValidationTrustState::Untrusted,
            TradeValidationTrustEvidence::receipt(
                receipt_event_id,
                receipt_author,
                &receipt.receipt,
                None,
            ),
            false,
            Some(TradeValidationTrustReason::new(
                "validation_trust_policy_empty",
                "validation trust policy has no trusted RHI public keys",
            )),
        ));
    }
    if !policy.trusts_rhi_pubkey(&receipt_author) {
        return Ok(trade_validation_trust_decision(
            policy,
            RadrootsTradeValidationTrustState::Untrusted,
            TradeValidationTrustEvidence::receipt(
                receipt_event_id,
                receipt_author,
                &receipt.receipt,
                None,
            ),
            false,
            Some(TradeValidationTrustReason::new(
                "validation_receipt_author_untrusted",
                "validation receipt author is not trusted by the active policy",
            )),
        ));
    }
    Ok(evaluate_trade_validation_trust_receipt(
        policy,
        receipt_event_id,
        receipt_author,
        &receipt.receipt,
    ))
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy)]
struct TradeValidationTrustMetadata {
    validation_authority: RadrootsTradeValidationAuthority,
    commitment_confidence: RadrootsTradeCommitmentConfidence,
    cryptographic_proof_verified: bool,
}

#[cfg(feature = "runtime")]
struct TradeValidationTrustEvidence<'a> {
    receipt_event_id: Option<RadrootsEventId>,
    receipt_author: Option<RadrootsPublicKey>,
    receipt: Option<&'a RadrootsTradeValidationReceipt>,
    metadata: Option<TradeValidationTrustMetadata>,
}

#[cfg(feature = "runtime")]
impl<'a> TradeValidationTrustEvidence<'a> {
    fn none() -> Self {
        Self {
            receipt_event_id: None,
            receipt_author: None,
            receipt: None,
            metadata: None,
        }
    }

    fn receipt_event(receipt_event_id: RadrootsEventId) -> Self {
        Self {
            receipt_event_id: Some(receipt_event_id),
            receipt_author: None,
            receipt: None,
            metadata: None,
        }
    }

    fn receipt_author(
        receipt_event_id: RadrootsEventId,
        receipt_author: RadrootsPublicKey,
    ) -> Self {
        Self {
            receipt_event_id: Some(receipt_event_id),
            receipt_author: Some(receipt_author),
            receipt: None,
            metadata: None,
        }
    }

    fn receipt(
        receipt_event_id: RadrootsEventId,
        receipt_author: RadrootsPublicKey,
        receipt: &'a RadrootsTradeValidationReceipt,
        metadata: Option<TradeValidationTrustMetadata>,
    ) -> Self {
        Self {
            receipt_event_id: Some(receipt_event_id),
            receipt_author: Some(receipt_author),
            receipt: Some(receipt),
            metadata,
        }
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy)]
struct TradeValidationTrustReason<'a> {
    code: &'a str,
    message: &'a str,
}

#[cfg(feature = "runtime")]
impl<'a> TradeValidationTrustReason<'a> {
    fn new(code: &'a str, message: &'a str) -> Self {
        Self { code, message }
    }
}

#[cfg(feature = "runtime")]
fn evaluate_trade_validation_trust_receipt(
    policy: &RadrootsTradeValidationTrustPolicy,
    receipt_event_id: RadrootsEventId,
    receipt_author: RadrootsPublicKey,
    receipt: &RadrootsTradeValidationReceipt,
) -> TradeValidationTrustDecision {
    let metadata = validation_receipt_trust_metadata(receipt);
    let authority = metadata.validation_authority;
    let confidence = metadata.commitment_confidence;
    if authority == RadrootsTradeValidationAuthority::DevDeterministicOnly
        || confidence == RadrootsTradeCommitmentConfidence::LocalOnly
    {
        if !policy.allow_deterministic_none {
            return trade_validation_trust_decision(
                policy,
                RadrootsTradeValidationTrustState::Untrusted,
                TradeValidationTrustEvidence::receipt(
                    receipt_event_id,
                    receipt_author,
                    receipt,
                    Some(metadata),
                ),
                false,
                Some(TradeValidationTrustReason::new(
                    "deterministic_none_not_allowed",
                    "deterministic-none validation is not allowed by the active policy",
                )),
            );
        }
        if policy.require_cryptographic_proof {
            return trade_validation_trust_decision(
                policy,
                RadrootsTradeValidationTrustState::Pending,
                TradeValidationTrustEvidence::receipt(
                    receipt_event_id,
                    receipt_author,
                    receipt,
                    Some(metadata),
                ),
                false,
                Some(TradeValidationTrustReason::new(
                    "cryptographic_proof_required",
                    "active policy requires cryptographic proof for committed confidence",
                )),
            );
        }
        return trade_validation_trust_decision(
            policy,
            RadrootsTradeValidationTrustState::TrustedLocal,
            TradeValidationTrustEvidence::receipt(
                receipt_event_id,
                receipt_author,
                receipt,
                Some(metadata),
            ),
            false,
            None,
        );
    }
    let cryptographic_metadata = matches!(
        (authority, confidence),
        (
            RadrootsTradeValidationAuthority::CryptographicProofVerified,
            RadrootsTradeCommitmentConfidence::CommittedByCryptographicProof
        ) | (
            RadrootsTradeValidationAuthority::TrustedServiceAndProofVerified,
            RadrootsTradeCommitmentConfidence::CommittedByTrustedServiceAndProof
        )
    );
    if cryptographic_metadata
        && receipt.proof.system != RadrootsValidationReceiptProofSystem::None
        && metadata.cryptographic_proof_verified
    {
        return trade_validation_trust_decision(
            policy,
            RadrootsTradeValidationTrustState::CryptographicCommitted,
            TradeValidationTrustEvidence::receipt(
                receipt_event_id,
                receipt_author,
                receipt,
                Some(metadata),
            ),
            true,
            None,
        );
    }
    if policy.require_cryptographic_proof {
        return trade_validation_trust_decision(
            policy,
            RadrootsTradeValidationTrustState::Pending,
            TradeValidationTrustEvidence::receipt(
                receipt_event_id,
                receipt_author,
                receipt,
                Some(metadata),
            ),
            false,
            Some(TradeValidationTrustReason::new(
                "cryptographic_proof_required",
                "active policy requires trusted cryptographic proof metadata",
            )),
        );
    }
    if matches!(
        (authority, confidence),
        (
            RadrootsTradeValidationAuthority::TrustedRhiServiceKey,
            RadrootsTradeCommitmentConfidence::CommittedByTrustedService
        ) | (
            RadrootsTradeValidationAuthority::TrustedServiceAndProofVerified,
            RadrootsTradeCommitmentConfidence::CommittedByTrustedServiceAndProof
        ) | (
            RadrootsTradeValidationAuthority::CryptographicProofVerified,
            RadrootsTradeCommitmentConfidence::CommittedByCryptographicProof
        )
    ) {
        return trade_validation_trust_decision(
            policy,
            RadrootsTradeValidationTrustState::TrustedLocal,
            TradeValidationTrustEvidence::receipt(
                receipt_event_id,
                receipt_author,
                receipt,
                Some(metadata),
            ),
            false,
            None,
        );
    }
    trade_validation_trust_decision(
        policy,
        RadrootsTradeValidationTrustState::Pending,
        TradeValidationTrustEvidence::receipt(
            receipt_event_id,
            receipt_author,
            receipt,
            Some(metadata),
        ),
        false,
        Some(TradeValidationTrustReason::new(
            "validation_trust_metadata_insufficient",
            "trusted validation result metadata does not satisfy the active policy",
        )),
    )
}

#[cfg(feature = "runtime")]
fn validation_receipt_trust_metadata(
    receipt: &RadrootsTradeValidationReceipt,
) -> TradeValidationTrustMetadata {
    if receipt.proof.system == RadrootsValidationReceiptProofSystem::None {
        TradeValidationTrustMetadata {
            validation_authority: RadrootsTradeValidationAuthority::DevDeterministicOnly,
            commitment_confidence: RadrootsTradeCommitmentConfidence::LocalOnly,
            cryptographic_proof_verified: false,
        }
    } else {
        TradeValidationTrustMetadata {
            validation_authority: RadrootsTradeValidationAuthority::CryptographicProofVerified,
            commitment_confidence: RadrootsTradeCommitmentConfidence::CommittedByCryptographicProof,
            cryptographic_proof_verified: true,
        }
    }
}

#[cfg(feature = "runtime")]
fn trade_validation_trust_decision(
    policy: &RadrootsTradeValidationTrustPolicy,
    state: RadrootsTradeValidationTrustState,
    evidence: TradeValidationTrustEvidence<'_>,
    production_committed: bool,
    reason: Option<TradeValidationTrustReason<'_>>,
) -> TradeValidationTrustDecision {
    let (reason_code, reason) = match reason {
        Some(reason) => (
            Some(reason.code.to_owned()),
            Some(reason.message.to_owned()),
        ),
        None => (None, None),
    };
    TradeValidationTrustDecision {
        state,
        trusted_rhi_pubkey_count: policy.trusted_rhi_pubkey_count(),
        allow_deterministic_none: policy.allow_deterministic_none,
        require_cryptographic_proof: policy.require_cryptographic_proof,
        receipt_event_id: evidence.receipt_event_id,
        receipt_author: evidence.receipt_author,
        result_event_id: None,
        result_author: None,
        proof_system: evidence
            .receipt
            .map(|receipt| receipt.proof.system.as_str().to_owned()),
        validation_authority: evidence
            .metadata
            .map(|metadata| metadata.validation_authority),
        commitment_confidence: evidence
            .metadata
            .map(|metadata| metadata.commitment_confidence),
        cryptographic_proof_required: policy.require_cryptographic_proof,
        cryptographic_proof_verified: evidence
            .metadata
            .is_some_and(|metadata| metadata.cryptographic_proof_verified),
        production_committed,
        reason_code,
        reason,
    }
}

#[cfg(feature = "runtime")]
fn apply_validation_trust_decision_to_status(status: &mut TradeStatusReceipt) {
    let Some(decision) = status.validation_trust.as_ref() else {
        return;
    };
    match decision.state {
        RadrootsTradeValidationTrustState::Pending
        | RadrootsTradeValidationTrustState::Untrusted => {
            if status.status == TradeStatusKind::Committed {
                status.status = TradeStatusKind::AgreedPendingValidation;
                status.lifecycle_terminal = false;
                status.next_action = TradeStatusNextActionKind::AwaitValidation;
                status.last_event_id = status
                    .agreement_event_id
                    .clone()
                    .or_else(|| status.decision_event_id.clone())
                    .or_else(|| status.request_event_id.clone());
            }
        }
        RadrootsTradeValidationTrustState::Invalid => {
            status.status = TradeStatusKind::Invalid;
            status.lifecycle_terminal = true;
            status.next_action = TradeStatusNextActionKind::InspectEvidenceIssues;
        }
        RadrootsTradeValidationTrustState::TrustedLocal
        | RadrootsTradeValidationTrustState::CryptographicCommitted => {}
    }
}

#[cfg(all(feature = "runtime", feature = "transport-nostr-runtime"))]
impl TradeValidationReceiptNostrEvidenceReceipt {
    fn from_fetch(receipt: RadrootsRelayFetchReceipt) -> Self {
        Self {
            inserted_count: receipt.inserted_count,
            duplicate_count: receipt.duplicate_count,
            malformed_count: receipt.malformed_count,
            out_of_filter_count: receipt.out_of_filter_count,
            skipped_over_limit_count: receipt.skipped_over_limit_count,
            unsupported_count: receipt.unsupported_count,
            eose_count: receipt.eose_count,
            closed_count: receipt.closed_count,
            notice_count: receipt.notice_count,
            events: receipt.events.into_iter().map(Into::into).collect(),
            nostr_relay_outcomes: receipt.relay_outcomes.into_iter().map(Into::into).collect(),
        }
    }
}

#[cfg(all(feature = "runtime", feature = "transport-nostr-runtime"))]
impl From<RadrootsRelayFetchRelayOutcome> for TradeValidationReceiptNostrRelayOutcomeReceipt {
    fn from(receipt: RadrootsRelayFetchRelayOutcome) -> Self {
        Self {
            nostr_relay_url: receipt.relay_url,
            outcome_kind: receipt.kind.into(),
            transport_outcome_kind: receipt.relay_outcome.map(|outcome| outcome.kind.into()),
            message: receipt.message,
        }
    }
}

#[cfg(all(feature = "runtime", feature = "transport-nostr-runtime"))]
impl From<RadrootsRelayFetchOutcomeKind> for TradeValidationReceiptNostrRelayOutcomeKind {
    fn from(kind: RadrootsRelayFetchOutcomeKind) -> Self {
        match kind {
            RadrootsRelayFetchOutcomeKind::Eose => Self::Eose,
            RadrootsRelayFetchOutcomeKind::Closed => Self::Closed,
            RadrootsRelayFetchOutcomeKind::Notice => Self::Notice,
        }
    }
}

#[cfg(all(feature = "runtime", feature = "transport-nostr-runtime"))]
impl From<RadrootsRelayOutcomeKind> for TradeValidationReceiptNostrRelayTransportOutcomeKind {
    fn from(kind: RadrootsRelayOutcomeKind) -> Self {
        match kind {
            RadrootsRelayOutcomeKind::Accepted => Self::Accepted,
            RadrootsRelayOutcomeKind::DuplicateAccepted => Self::DuplicateAccepted,
            RadrootsRelayOutcomeKind::Blocked => Self::Blocked,
            RadrootsRelayOutcomeKind::RateLimited => Self::RateLimited,
            RadrootsRelayOutcomeKind::Invalid => Self::Invalid,
            RadrootsRelayOutcomeKind::PowRequired => Self::PowRequired,
            RadrootsRelayOutcomeKind::Restricted => Self::Restricted,
            RadrootsRelayOutcomeKind::AuthRequired => Self::AuthRequired,
            RadrootsRelayOutcomeKind::Muted => Self::Muted,
            RadrootsRelayOutcomeKind::Unsupported => Self::Unsupported,
            RadrootsRelayOutcomeKind::PaymentRequired => Self::PaymentRequired,
            RadrootsRelayOutcomeKind::Error => Self::Error,
            RadrootsRelayOutcomeKind::Timeout => Self::Timeout,
            RadrootsRelayOutcomeKind::ConnectionFailed => Self::ConnectionFailed,
            RadrootsRelayOutcomeKind::RelayUrlRejected => Self::RelayUrlRejected,
            RadrootsRelayOutcomeKind::SkippedAlreadyAccepted => Self::SkippedAlreadyAccepted,
            RadrootsRelayOutcomeKind::Unknown => Self::Unknown,
        }
    }
}

#[cfg(feature = "runtime")]
impl From<RadrootsValidationReceiptTags> for TradeValidationReceiptTags {
    fn from(tags: RadrootsValidationReceiptTags) -> Self {
        Self {
            order_id: tags.order_id,
            event_set_root: tags.event_set_root,
            listing_event_id: tags.listing_event_id,
            reducer_output_root: tags.reducer_output_root,
            public_values_hash: tags.public_values_hash,
            proof_system: tags.proof_system.as_str().to_owned(),
            receipt_type: tags.receipt_type.as_str().to_owned(),
            root_event_id: tags.root_event_id,
            target_event_id: tags.target_event_id,
        }
    }
}

#[cfg(feature = "runtime")]
fn stored_event_to_nostr_event(
    stored_event: &RadrootsStoredEvent,
) -> Result<RadrootsEventEnvelope, RadrootsSdkError> {
    let tags = serde_json::from_str(&stored_event.tags_json).map_err(|error| {
        RadrootsSdkError::EventStore {
            message: format!(
                "stored event {} contains invalid tags_json: {error}",
                stored_event.event_id
            ),
        }
    })?;
    RadrootsEventEnvelope::new(RadrootsEventEnvelopeParts {
        id: stored_event.event_id.clone(),
        author: stored_event.pubkey.clone(),
        created_at: stored_event.created_at,
        kind: stored_event.kind,
        tags,
        content: stored_event.content.clone(),
        sig: stored_event.sig.clone(),
    })
    .map_err(|error| RadrootsSdkError::EventStore {
        message: format!(
            "stored event {} contains invalid envelope: {error}",
            stored_event.event_id
        ),
    })
}

#[cfg(all(feature = "runtime", feature = "transport-nostr-runtime"))]
fn validation_receipt_event_order(
    left: &TradeValidationReceiptEvent,
    right: &TradeValidationReceiptEvent,
) -> core::cmp::Ordering {
    left.event
        .created_at()
        .cmp(&right.event.created_at())
        .then_with(|| left.event.id().cmp(right.event.id()))
}

#[cfg(all(feature = "runtime", feature = "transport-nostr-runtime"))]
fn validation_receipt_invalid_order(
    left: &TradeValidationReceiptInvalidCandidate,
    right: &TradeValidationReceiptInvalidCandidate,
) -> core::cmp::Ordering {
    left.event
        .created_at()
        .cmp(&right.event.created_at())
        .then_with(|| left.event.id().cmp(right.event.id()))
}

#[cfg(all(feature = "runtime", feature = "transport-nostr-runtime"))]
fn validate_validation_receipt_limit(limit: u32) -> Result<(), RadrootsSdkError> {
    if limit == 0 || limit > TRADE_STATUS_MAX_LIMIT {
        return Err(RadrootsSdkError::trade_status_limit_invalid(
            limit,
            1,
            TRADE_STATUS_MAX_LIMIT,
        ));
    }
    Ok(())
}

#[cfg(feature = "runtime")]
fn parse_worker_pubkeys<I, S>(pubkeys: I) -> Result<Vec<RadrootsPublicKey>, RadrootsSdkError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    pubkeys
        .into_iter()
        .map(|pubkey| {
            let value = pubkey.as_ref();
            RadrootsPublicKey::parse(value).map_err(|error| RadrootsSdkError::InvalidRequest {
                message: format!("invalid trusted worker pubkey `{value}`: {error}"),
            })
        })
        .collect()
}

#[cfg(all(feature = "runtime", feature = "transport-nostr-runtime"))]
fn validation_receipt_invalid_reason_code(error: &RadrootsValidationReceiptError) -> &'static str {
    match error {
        RadrootsValidationReceiptError::InvalidProofMetadata("proof.material")
        | RadrootsValidationReceiptError::InvalidProofMetadata("proof.material_missing") => {
            "sp1_proof_material_missing"
        }
        RadrootsValidationReceiptError::InvalidProofMetadata("proof.material_conflict") => {
            "sp1_proof_material_conflict"
        }
        RadrootsValidationReceiptError::InvalidProofMetadata("proof.inline_proof_base64") => {
            "sp1_inline_proof_invalid"
        }
        RadrootsValidationReceiptError::InvalidProofMetadata("proof.proof_reference") => {
            "sp1_proof_reference_invalid"
        }
        RadrootsValidationReceiptError::TagMismatch("public_values_hash")
        | RadrootsValidationReceiptError::ExpectedBindingMismatch("public_values_hash") => {
            "public_values_hash_mismatch"
        }
        RadrootsValidationReceiptError::ExpectedBindingMismatch("program_hash") => {
            "sp1_program_hash_mismatch"
        }
        RadrootsValidationReceiptError::ExpectedBindingMismatch("verifying_key_hash") => {
            "sp1_verifying_key_hash_mismatch"
        }
        _ => "validation_receipt_invalid",
    }
}

#[cfg(all(feature = "runtime", feature = "signer-adapters"))]
impl<'sdk> TradeBuyerClient<'sdk> {
    pub async fn propose_trade(
        &self,
        request: TradeProposeRequest,
    ) -> Result<TradeMutationOutcome<TradeSubmitPlan, TradeSubmitReceipt>, RadrootsSdkError> {
        validate_trade_product_publish_policy(request.publish_mode, &request.satisfaction_policy)?;
        require_trade_product_privacy_preflight(
            "trade.propose",
            trade_propose_privacy_fields(&request),
            &request.privacy_confirmation,
        )?;
        let TradeProposeRequest {
            actor,
            listing_event,
            order_id,
            listing_addr,
            seller_pubkey,
            items,
            economics,
            public_note: _,
            target_policy,
            publish_mode,
            satisfaction_policy,
            privacy_confirmation: _,
            idempotency_key,
            created_at,
        } = request;
        let order = RadrootsOrderRequest {
            order_id,
            listing_addr,
            buyer_pubkey: actor.pubkey().clone(),
            seller_pubkey,
            items,
            economics,
        };
        let client = trades_client(self.sdk);
        let plan = client.prepare_submit(TradeSubmitPrepareRequest {
            actor: actor.clone(),
            listing_event,
            order,
            created_at,
        })?;
        if publish_mode == PublishMode::DryRun {
            return Ok(TradeMutationOutcome::DryRun { plan });
        }
        let receipt = client
            .enqueue_prepared_submit(
                &actor,
                plan,
                target_policy,
                publish_mode,
                satisfaction_policy.clone(),
                idempotency_key,
            )
            .await?;
        trade_product_post_enqueue_outcome(
            self.sdk,
            publish_mode,
            satisfaction_policy,
            receipt.outbox_event_id,
            receipt,
        )
        .await
    }

    pub async fn cancel_trade(
        &self,
        request: TradeCancelRequest,
    ) -> Result<
        TradeMutationOutcome<TradeCancellationPlan, TradeCancellationReceipt>,
        RadrootsSdkError,
    > {
        validate_trade_product_publish_policy(request.publish_mode, &request.satisfaction_policy)?;
        let TradeCancelRequest {
            actor,
            locator,
            reason,
            target_policy,
            publish_mode,
            satisfaction_policy,
            evidence_mode,
            privacy_confirmation,
            idempotency_key,
            created_at,
        } = request;
        let context =
            trade_mutation_context(self.sdk, locator, evidence_mode, "trade.cancel").await?;
        let cancellation = RadrootsOrderCancellation {
            order_id: context.order_id.clone(),
            listing_addr: context.listing_addr.clone(),
            buyer_pubkey: context.buyer_pubkey.clone(),
            seller_pubkey: context.seller_pubkey.clone(),
            reason,
        };
        require_trade_product_privacy_preflight(
            "trade.cancel",
            trade_reason_privacy_fields(&cancellation.reason),
            &privacy_confirmation,
        )?;
        let client = trades_client(self.sdk);
        let plan = client.prepare_cancellation(TradeCancellationPrepareRequest {
            actor: actor.clone(),
            root_event: event_ptr(&context.root_event_id),
            previous_event: event_ptr(&context.previous_event_id),
            cancellation,
            created_at,
        })?;
        if publish_mode == PublishMode::DryRun {
            return Ok(TradeMutationOutcome::DryRun { plan });
        }
        let receipt = client
            .enqueue_prepared_cancellation(
                &actor,
                plan,
                target_policy,
                publish_mode,
                satisfaction_policy.clone(),
                idempotency_key,
            )
            .await?;
        trade_product_post_enqueue_outcome(
            self.sdk,
            publish_mode,
            satisfaction_policy,
            receipt.outbox_event_id,
            receipt,
        )
        .await
    }
}

#[cfg(feature = "runtime")]
impl<'sdk> TradeSellerClient<'sdk> {
    pub async fn inbox(
        &self,
        request: TradeSellerInboxRequest,
    ) -> Result<TradeSellerInboxReceipt, RadrootsSdkError> {
        require_seller_actor(&request.actor, "trade.inbox")?;
        if request.limit == 0 || request.limit > TRADE_STATUS_MAX_LIMIT {
            return Err(RadrootsSdkError::trade_status_limit_invalid(
                request.limit,
                1,
                TRADE_STATUS_MAX_LIMIT,
            ));
        }
        let seller_pubkey =
            RadrootsPublicKey::parse(request.actor.pubkey().as_str()).map_err(|error| {
                RadrootsSdkError::InvalidRequest {
                    message: format!("seller actor pubkey is invalid: {error}"),
                }
            })?;
        let events = self
            .sdk
            ._event_store
            .events_by_tag(TAG_P, request.actor.pubkey().as_str(), request.limit)
            .await?;
        let mut locators = Vec::new();
        for event in events {
            let tags = serde_json::from_str(&event.tags_json).map_err(|error| {
                RadrootsSdkError::Projection {
                    message: format!(
                        "stored trade inbox event {} contains invalid tags_json: {error}",
                        event.event_id
                    ),
                }
            })?;
            let nostr_event = RadrootsEventEnvelope::new(RadrootsEventEnvelopeParts {
                id: event.event_id,
                author: event.pubkey,
                created_at: event.created_at,
                kind: event.kind,
                tags,
                content: event.content,
                sig: event.sig,
            })
            .map_err(|error| RadrootsSdkError::Projection {
                message: format!("stored trade inbox event envelope is invalid: {error}"),
            })?;
            if let Ok(RadrootsOrderEventRecord::Request(record)) =
                order_event_record_from_event(&nostr_event)
                && record.payload.seller_pubkey == seller_pubkey
            {
                locators.push(
                    RadrootsTradeLocator::from_order_id(record.payload.order_id)
                        .with_root_event_id(record.event_id)
                        .with_listing_addr(record.payload.listing_addr)
                        .with_buyer_pubkey(record.payload.buyer_pubkey)
                        .with_seller_pubkey(record.payload.seller_pubkey),
                );
            }
        }
        locators.sort_by(|left, right| left.root_event_id.cmp(&right.root_event_id));
        locators.dedup_by(|left, right| left.root_event_id == right.root_event_id);
        let mut statuses = Vec::with_capacity(locators.len());
        for locator in locators {
            statuses.push(
                trades_client(self.sdk)
                    .status(TradeStatusRequest::new(locator))
                    .await?,
            );
        }
        Ok(TradeSellerInboxReceipt {
            seller_pubkey,
            statuses,
        })
    }

    #[cfg(feature = "signer-adapters")]
    pub async fn accept_trade(
        &self,
        request: TradeAcceptRequest,
    ) -> Result<TradeMutationOutcome<TradeDecisionPlan, TradeDecisionReceipt>, RadrootsSdkError>
    {
        validate_trade_product_publish_policy(request.publish_mode, &request.satisfaction_policy)?;
        let TradeAcceptRequest {
            actor,
            locator,
            inventory_commitments,
            target_policy,
            publish_mode,
            satisfaction_policy,
            evidence_mode,
            privacy_confirmation,
            idempotency_key,
            created_at,
        } = request;
        let context =
            trade_mutation_context(self.sdk, locator, evidence_mode, "trade.accept").await?;
        let decision = RadrootsOrderDecision {
            order_id: context.order_id.clone(),
            listing_addr: context.listing_addr.clone(),
            buyer_pubkey: context.buyer_pubkey.clone(),
            seller_pubkey: context.seller_pubkey.clone(),
            decision: RadrootsOrderDecisionOutcome::Accepted {
                inventory_commitments,
            },
        };
        require_trade_product_privacy_preflight(
            "trade.accept",
            trade_decision_privacy_fields(&decision),
            &privacy_confirmation,
        )?;
        let client = trades_client(self.sdk);
        let plan = client.prepare_decision(TradeDecisionPrepareRequest {
            actor: actor.clone(),
            request_event: event_ptr(&context.root_event_id),
            decision,
            created_at,
        })?;
        if publish_mode == PublishMode::DryRun {
            return Ok(TradeMutationOutcome::DryRun { plan });
        }
        let receipt = client
            .enqueue_prepared_decision(
                &actor,
                plan,
                target_policy,
                publish_mode,
                satisfaction_policy.clone(),
                idempotency_key,
            )
            .await?;
        trade_product_post_enqueue_outcome(
            self.sdk,
            publish_mode,
            satisfaction_policy,
            receipt.outbox_event_id,
            receipt,
        )
        .await
    }

    #[cfg(feature = "transport-nostr-runtime")]
    pub async fn accept_trade_with_fetch_adapter<A>(
        &self,
        request: TradeAcceptRequest,
        adapter: &A,
    ) -> Result<TradeMutationOutcome<TradeDecisionPlan, TradeDecisionReceipt>, RadrootsSdkError>
    where
        A: RadrootsRelayFetchAdapter,
    {
        validate_trade_product_publish_policy(request.publish_mode, &request.satisfaction_policy)?;
        let TradeAcceptRequest {
            actor,
            locator,
            inventory_commitments,
            target_policy,
            publish_mode,
            satisfaction_policy,
            evidence_mode,
            privacy_confirmation,
            idempotency_key,
            created_at,
        } = request;
        let context = trade_mutation_context_with_fetch_adapter(
            self.sdk,
            locator,
            evidence_mode,
            "trade.accept",
            adapter,
        )
        .await?;
        let decision = RadrootsOrderDecision {
            order_id: context.order_id.clone(),
            listing_addr: context.listing_addr.clone(),
            buyer_pubkey: context.buyer_pubkey.clone(),
            seller_pubkey: context.seller_pubkey.clone(),
            decision: RadrootsOrderDecisionOutcome::Accepted {
                inventory_commitments,
            },
        };
        require_trade_product_privacy_preflight(
            "trade.accept",
            trade_decision_privacy_fields(&decision),
            &privacy_confirmation,
        )?;
        let client = trades_client(self.sdk);
        let plan = client.prepare_decision(TradeDecisionPrepareRequest {
            actor: actor.clone(),
            request_event: event_ptr(&context.root_event_id),
            decision,
            created_at,
        })?;
        if publish_mode == PublishMode::DryRun {
            return Ok(TradeMutationOutcome::DryRun { plan });
        }
        let receipt = client
            .enqueue_prepared_decision(
                &actor,
                plan,
                target_policy,
                publish_mode,
                satisfaction_policy.clone(),
                idempotency_key,
            )
            .await?;
        trade_product_post_enqueue_outcome(
            self.sdk,
            publish_mode,
            satisfaction_policy,
            receipt.outbox_event_id,
            receipt,
        )
        .await
    }

    #[cfg(feature = "signer-adapters")]
    pub async fn decline_trade(
        &self,
        request: TradeDeclineRequest,
    ) -> Result<TradeMutationOutcome<TradeDecisionPlan, TradeDecisionReceipt>, RadrootsSdkError>
    {
        validate_trade_product_publish_policy(request.publish_mode, &request.satisfaction_policy)?;
        let TradeDeclineRequest {
            actor,
            locator,
            reason,
            target_policy,
            publish_mode,
            satisfaction_policy,
            evidence_mode,
            privacy_confirmation,
            idempotency_key,
            created_at,
        } = request;
        let context =
            trade_mutation_context(self.sdk, locator, evidence_mode, "trade.decline").await?;
        let decision = RadrootsOrderDecision {
            order_id: context.order_id.clone(),
            listing_addr: context.listing_addr.clone(),
            buyer_pubkey: context.buyer_pubkey.clone(),
            seller_pubkey: context.seller_pubkey.clone(),
            decision: RadrootsOrderDecisionOutcome::Declined { reason },
        };
        require_trade_product_privacy_preflight(
            "trade.decline",
            trade_decision_privacy_fields(&decision),
            &privacy_confirmation,
        )?;
        let client = trades_client(self.sdk);
        let plan = client.prepare_decision(TradeDecisionPrepareRequest {
            actor: actor.clone(),
            request_event: event_ptr(&context.root_event_id),
            decision,
            created_at,
        })?;
        if publish_mode == PublishMode::DryRun {
            return Ok(TradeMutationOutcome::DryRun { plan });
        }
        let receipt = client
            .enqueue_prepared_decision(
                &actor,
                plan,
                target_policy,
                publish_mode,
                satisfaction_policy.clone(),
                idempotency_key,
            )
            .await?;
        trade_product_post_enqueue_outcome(
            self.sdk,
            publish_mode,
            satisfaction_policy,
            receipt.outbox_event_id,
            receipt,
        )
        .await
    }
}

#[cfg(feature = "runtime")]
fn trades_client(sdk: &crate::RadrootsClient) -> TradesClient<'_> {
    TradesClient { sdk }
}

#[cfg(feature = "signer-adapters")]
struct TradeProductMutationContext {
    order_id: RadrootsOrderId,
    listing_addr: RadrootsListingAddress,
    buyer_pubkey: RadrootsPublicKey,
    seller_pubkey: RadrootsPublicKey,
    root_event_id: RadrootsEventId,
    previous_event_id: RadrootsEventId,
}

#[cfg(feature = "signer-adapters")]
async fn trade_mutation_context(
    sdk: &crate::RadrootsClient,
    locator: RadrootsTradeLocator,
    evidence_mode: TradeEvidenceMode,
    operation: &'static str,
) -> Result<TradeProductMutationContext, RadrootsSdkError> {
    let status = trade_mutation_status(sdk, locator.clone(), &evidence_mode, operation).await?;
    trade_mutation_context_from_status(locator, &evidence_mode, operation, status)
}

#[cfg(feature = "signer-adapters")]
async fn trade_mutation_status(
    sdk: &crate::RadrootsClient,
    locator: RadrootsTradeLocator,
    evidence_mode: &TradeEvidenceMode,
    operation: &'static str,
) -> Result<TradeStatusReceipt, RadrootsSdkError> {
    match evidence_mode {
        TradeEvidenceMode::LocalOnly => {
            trades_client(sdk)
                .status(
                    TradeStatusRequest::new(locator).with_source(SdkTradeStatusSource::LocalOnly),
                )
                .await
        }
        TradeEvidenceMode::RequireExplicitEvidence { evidence } => {
            ingest_explicit_trade_mutation_evidence(sdk, operation, evidence).await?;
            trades_client(sdk)
                .status(
                    TradeStatusRequest::new(locator).with_source(SdkTradeStatusSource::LocalOnly),
                )
                .await
        }
        TradeEvidenceMode::ResyncBeforeMutation => {
            #[cfg(feature = "transport-nostr-runtime")]
            {
                let adapter = RadrootsNostrClientFetchAdapter;
                let status = trades_client(sdk)
                    .status_with_fetch_adapter(
                        TradeStatusRequest::new(locator)
                            .with_source(SdkTradeStatusSource::ResyncThenLocal),
                        &adapter,
                    )
                    .await?;
                require_trade_mutation_online_evidence_clean(operation, &status)?;
                Ok(status)
            }
            #[cfg(not(feature = "transport-nostr-runtime"))]
            {
                let _ = locator;
                Err(RadrootsSdkError::ProductSyncUnsupported {
                    operation,
                    required_feature: "transport-nostr-runtime",
                })
            }
        }
    }
}

#[cfg(all(feature = "signer-adapters", feature = "transport-nostr-runtime"))]
async fn trade_mutation_context_with_fetch_adapter<A>(
    sdk: &crate::RadrootsClient,
    locator: RadrootsTradeLocator,
    evidence_mode: TradeEvidenceMode,
    operation: &'static str,
    adapter: &A,
) -> Result<TradeProductMutationContext, RadrootsSdkError>
where
    A: RadrootsRelayFetchAdapter,
{
    let status = match evidence_mode {
        TradeEvidenceMode::LocalOnly => {
            trades_client(sdk)
                .status(
                    TradeStatusRequest::new(locator.clone())
                        .with_source(SdkTradeStatusSource::LocalOnly),
                )
                .await?
        }
        TradeEvidenceMode::RequireExplicitEvidence { ref evidence } => {
            ingest_explicit_trade_mutation_evidence(sdk, operation, evidence).await?;
            trades_client(sdk)
                .status(
                    TradeStatusRequest::new(locator.clone())
                        .with_source(SdkTradeStatusSource::LocalOnly),
                )
                .await?
        }
        TradeEvidenceMode::ResyncBeforeMutation => {
            let status = trades_client(sdk)
                .status_with_fetch_adapter(
                    TradeStatusRequest::new(locator.clone())
                        .with_source(SdkTradeStatusSource::ResyncThenLocal),
                    adapter,
                )
                .await?;
            require_trade_mutation_online_evidence_clean(operation, &status)?;
            status
        }
    };
    trade_mutation_context_from_status(locator, &evidence_mode, operation, status)
}

#[cfg(feature = "signer-adapters")]
fn trade_mutation_context_from_status(
    locator: RadrootsTradeLocator,
    evidence_mode: &TradeEvidenceMode,
    operation: &'static str,
    status: TradeStatusReceipt,
) -> Result<TradeProductMutationContext, RadrootsSdkError> {
    if status.status == TradeStatusKind::Ambiguous {
        return Err(RadrootsSdkError::TradeAmbiguous {
            operation: operation.to_owned(),
            locator: Box::new(locator),
            candidates: status
                .ambiguity_candidates
                .into_iter()
                .map(|candidate| candidate.locator)
                .collect(),
        });
    }
    if !status.found {
        let evidence_requirement = match evidence_mode {
            TradeEvidenceMode::LocalOnly => "locally projected",
            TradeEvidenceMode::ResyncBeforeMutation => "resynced",
            TradeEvidenceMode::RequireExplicitEvidence { .. } => "explicitly ingested",
        };
        return Err(RadrootsSdkError::InvalidRequest {
            message: format!("{operation} requires a {evidence_requirement} trade"),
        });
    }
    let root_event_id =
        status
            .root_event_id
            .clone()
            .ok_or_else(|| RadrootsSdkError::InvalidRequest {
                message: format!("{operation} requires a trade root event id"),
            })?;
    let previous_event_id =
        status
            .last_event_id
            .clone()
            .ok_or_else(|| RadrootsSdkError::InvalidRequest {
                message: format!("{operation} requires local lifecycle evidence"),
            })?;
    let listing_addr =
        status
            .listing_addr
            .clone()
            .ok_or_else(|| RadrootsSdkError::InvalidRequest {
                message: format!("{operation} requires listing address evidence"),
            })?;
    let buyer_pubkey =
        status
            .buyer_pubkey
            .clone()
            .ok_or_else(|| RadrootsSdkError::InvalidRequest {
                message: format!("{operation} requires buyer pubkey evidence"),
            })?;
    let seller_pubkey =
        status
            .seller_pubkey
            .clone()
            .ok_or_else(|| RadrootsSdkError::InvalidRequest {
                message: format!("{operation} requires seller pubkey evidence"),
            })?;
    Ok(TradeProductMutationContext {
        order_id: status.order_id,
        listing_addr,
        buyer_pubkey,
        seller_pubkey,
        root_event_id,
        previous_event_id,
    })
}

#[cfg(feature = "signer-adapters")]
async fn ingest_explicit_trade_mutation_evidence(
    sdk: &crate::RadrootsClient,
    operation: &'static str,
    evidence: &[TradeEvidenceIngestRequest],
) -> Result<(), RadrootsSdkError> {
    if evidence.is_empty() {
        return Err(RadrootsSdkError::InvalidRequest {
            message: format!("{operation} requires explicit trade evidence"),
        });
    }
    let client = trades_client(sdk);
    for event in evidence {
        client.ingest_evidence(event.clone()).await?;
    }
    Ok(())
}

#[cfg(all(feature = "signer-adapters", feature = "transport-nostr-runtime"))]
fn require_trade_mutation_online_evidence_clean(
    operation: &'static str,
    status: &TradeStatusReceipt,
) -> Result<(), RadrootsSdkError> {
    let evidence =
        status
            .online_evidence
            .as_ref()
            .ok_or_else(|| RadrootsSdkError::InvalidRequest {
                message: format!("{operation} requires online mutation evidence"),
            })?;
    if evidence.malformed_count > 0
        || evidence.out_of_filter_count > 0
        || evidence.skipped_over_limit_count > 0
        || evidence.unsupported_count > 0
    {
        return Err(RadrootsSdkError::InvalidRequest {
            message: format!(
                "{operation} refused online mutation evidence: malformed_count={}, out_of_filter_count={}, skipped_over_limit_count={}, unsupported_count={}",
                evidence.malformed_count,
                evidence.out_of_filter_count,
                evidence.skipped_over_limit_count,
                evidence.unsupported_count
            ),
        });
    }
    Ok(())
}

#[cfg(feature = "signer-adapters")]
fn event_ptr(event_id: &RadrootsEventId) -> RadrootsEventPtr {
    RadrootsEventPtr {
        id: event_id.as_str().to_owned(),
        relays: None,
    }
}

#[cfg(feature = "signer-adapters")]
async fn trade_product_post_enqueue_outcome<Plan, Receipt>(
    sdk: &crate::RadrootsClient,
    publish_mode: PublishMode,
    satisfaction_policy: SatisfactionPolicy,
    outbox_event_id: i64,
    receipt: Receipt,
) -> Result<TradeMutationOutcome<Plan, Receipt>, RadrootsSdkError> {
    match publish_mode {
        PublishMode::DryRun => Err(RadrootsSdkError::InvalidRequest {
            message: "trade product dry-run must return before enqueue".to_owned(),
        }),
        PublishMode::EnqueueOnly => Ok(TradeMutationOutcome::Enqueued { receipt }),
        PublishMode::EnqueueAndPublish => {
            let publish = sdk
                .sync()
                .push_outbox(push_request_for_satisfaction_policy(
                    satisfaction_policy,
                    outbox_event_id,
                )?)
                .await?;
            Ok(TradeMutationOutcome::Published { receipt, publish })
        }
    }
}

#[cfg(feature = "signer-adapters")]
fn push_request_for_satisfaction_policy(
    satisfaction_policy: SatisfactionPolicy,
    outbox_event_id: i64,
) -> Result<PushOutboxRequest, RadrootsSdkError> {
    let request = PushOutboxRequest::new().with_outbox_event_id(outbox_event_id);
    if satisfaction_policy.is_no_wait() {
        Err(RadrootsSdkError::InvalidRequest {
            message: "trade enqueue-and-publish requires a transport satisfaction policy"
                .to_owned(),
        })
    } else {
        Ok(request)
    }
}

#[cfg(feature = "signer-adapters")]
fn validate_trade_product_publish_policy(
    publish_mode: PublishMode,
    satisfaction_policy: &SatisfactionPolicy,
) -> Result<(), RadrootsSdkError> {
    match publish_mode {
        PublishMode::DryRun | PublishMode::EnqueueOnly if !satisfaction_policy.is_no_wait() => {
            Err(RadrootsSdkError::InvalidRequest {
                message: "trade dry-run and enqueue-only modes require no-wait satisfaction"
                    .to_owned(),
            })
        }
        PublishMode::EnqueueAndPublish if satisfaction_policy.is_no_wait() => {
            Err(RadrootsSdkError::InvalidRequest {
                message: "trade enqueue-and-publish requires a transport satisfaction policy"
                    .to_owned(),
            })
        }
        _ => Ok(()),
    }
}

#[cfg(feature = "signer-adapters")]
fn require_trade_product_privacy_preflight(
    operation: &'static str,
    fields: Vec<ProductSensitivityField>,
    confirmation: &PrivacyPreflightConfirmation,
) -> Result<(), RadrootsSdkError> {
    PrivacyPreflightReceipt::evaluate(fields)
        .require_public_publish_allowed(operation, confirmation)
}

#[cfg(feature = "signer-adapters")]
fn trade_propose_privacy_fields(request: &TradeProposeRequest) -> Vec<ProductSensitivityField> {
    let mut fields = Vec::new();
    if !request.items.is_empty() || !request.economics.items.is_empty() {
        fields.push(ProductSensitivityField::ProtocolMinimizedInventoryFields);
    }
    if let Some(public_note) = request.public_note.as_deref()
        && !public_note.trim().is_empty()
    {
        fields.push(ProductSensitivityField::PublicButSensitiveNotes);
        if trade_reason_contains_private_coordination(public_note) {
            fields.push(ProductSensitivityField::SensitiveFulfillmentDetails);
        }
    }
    fields
}

#[cfg(feature = "signer-adapters")]
fn trade_decision_privacy_fields(decision: &RadrootsOrderDecision) -> Vec<ProductSensitivityField> {
    match &decision.decision {
        RadrootsOrderDecisionOutcome::Accepted {
            inventory_commitments,
        } if !inventory_commitments.is_empty() => {
            vec![ProductSensitivityField::ProtocolMinimizedInventoryFields]
        }
        RadrootsOrderDecisionOutcome::Accepted { .. } => Vec::new(),
        RadrootsOrderDecisionOutcome::Declined { reason } => trade_reason_privacy_fields(reason),
    }
}

#[cfg(feature = "signer-adapters")]
fn trade_reason_privacy_fields(reason: &str) -> Vec<ProductSensitivityField> {
    if reason.trim().is_empty() {
        return Vec::new();
    }
    let mut fields = vec![ProductSensitivityField::PublicButSensitiveNotes];
    if trade_reason_contains_private_coordination(reason) {
        fields.push(ProductSensitivityField::SensitiveFulfillmentDetails);
    }
    fields
}

#[cfg(feature = "signer-adapters")]
fn trade_reason_contains_private_coordination(reason: &str) -> bool {
    let reason = reason.to_ascii_lowercase();
    [
        "pickup address",
        "delivery address",
        "street address",
        "exact location",
        "private location",
        "gate code",
        "door code",
        "latitude",
        "longitude",
        " gps",
        "farm gate",
    ]
    .iter()
    .any(|marker| reason.contains(marker))
}

#[cfg(feature = "signer-adapters")]
fn validate_trade_enqueue_policy(
    publish_mode: PublishMode,
    satisfaction_policy: &SatisfactionPolicy,
) -> Result<(), RadrootsSdkError> {
    if publish_mode == PublishMode::DryRun {
        return Err(RadrootsSdkError::InvalidRequest {
            message: "trade dry-run publish mode must use a prepare request".to_owned(),
        });
    }
    if publish_mode == PublishMode::EnqueueOnly && !satisfaction_policy.is_no_wait() {
        return Err(RadrootsSdkError::InvalidRequest {
            message: "trade enqueue-only publish mode only supports no-wait satisfaction"
                .to_owned(),
        });
    }
    if publish_mode == PublishMode::EnqueueAndPublish && satisfaction_policy.is_no_wait() {
        return Err(RadrootsSdkError::InvalidRequest {
            message: "trade enqueue-and-publish requires a transport satisfaction policy"
                .to_owned(),
        });
    }
    Ok(())
}

#[cfg(feature = "runtime")]
struct ParsedOrderEvidence {
    order_id: RadrootsOrderId,
    listing_addr: RadrootsListingAddress,
    event_id: RadrootsEventId,
    event_kind: u32,
}

#[cfg(feature = "runtime")]
#[inline(never)]
fn parse_order_evidence(
    event: &RadrootsEventEnvelope,
) -> Result<ParsedOrderEvidence, RadrootsSdkError> {
    let event_id = RadrootsEventId::parse(event.id_str()).map_err(|error| {
        RadrootsSdkError::InvalidRequest {
            message: format!("order evidence event id is invalid: {error}"),
        }
    })?;
    let (order_id, listing_addr) = match event.kind_u32() {
        KIND_ORDER_REQUEST => {
            let payload = order_request_from_event(event)
                .map_err(order_evidence_parse_error)?
                .payload;
            (payload.order_id, payload.listing_addr)
        }
        KIND_ORDER_DECISION => {
            let payload = order_decision_from_event(event)
                .map_err(order_evidence_parse_error)?
                .payload;
            (payload.order_id, payload.listing_addr)
        }
        KIND_ORDER_CANCELLATION => {
            let payload = order_cancellation_from_event(event)
                .map_err(order_evidence_parse_error)?
                .payload;
            (payload.order_id, payload.listing_addr)
        }
        other => {
            return Err(RadrootsSdkError::InvalidRequest {
                message: format!("order evidence event kind {other} is not supported"),
            });
        }
    };

    Ok(ParsedOrderEvidence {
        order_id,
        listing_addr,
        event_id,
        event_kind: event.kind_u32(),
    })
}

#[cfg(feature = "runtime")]
fn order_evidence_parse_error(
    error: radroots_event_codec::order::RadrootsOrderEnvelopeParseError,
) -> RadrootsSdkError {
    RadrootsSdkError::InvalidRequest {
        message: format!("order evidence event is invalid: {error}"),
    }
}

#[cfg(feature = "runtime")]
impl TradeStatusReceipt {
    fn from_locator_query_result(
        locator: RadrootsTradeLocator,
        query_result: RadrootsTradeLocatorProjectionQueryResult,
    ) -> Self {
        match query_result.resolution {
            RadrootsTradeLocatorProjectionResolution::Projected {
                locator,
                projection,
            } => Self::from_projection(
                locator.clone(),
                locator.root_event_id,
                Vec::new(),
                projection,
                query_result.event_count,
                query_result.limit_applied,
                query_result.event_ids,
            ),
            RadrootsTradeLocatorProjectionResolution::Ambiguous { candidates, .. } => {
                Self::ambiguous(
                    locator,
                    candidates,
                    query_result.event_count,
                    query_result.limit_applied,
                    query_result.event_ids,
                )
            }
            RadrootsTradeLocatorProjectionResolution::Missing { .. } => Self::missing(
                locator,
                query_result.event_count,
                query_result.limit_applied,
                query_result.event_ids,
            ),
        }
    }

    fn from_projection(
        locator: RadrootsTradeLocator,
        root_event_id: Option<RadrootsEventId>,
        ambiguity_candidates: Vec<TradeStatusAmbiguityCandidate>,
        projection: RadrootsOrderProjection,
        event_count: usize,
        limit_applied: u32,
        event_ids: Vec<RadrootsEventId>,
    ) -> Self {
        let found = projection.status != RadrootsTradeWorkflowState::Missing;
        let evidence =
            TradeStatusEvidenceSummary::from_projection(&projection, event_count, limit_applied);
        let eligibility = TradeStatusEligibility::from_projection(&projection);
        let next_action = TradeStatusNextActionKind::from_projection(&projection, &eligibility);
        Self {
            locator,
            order_id: projection.order_id,
            root_event_id,
            ambiguity_candidates,
            source: SdkTradeStatusSource::LocalOnly,
            found,
            event_count,
            limit_applied,
            status: projection.status.into(),
            lifecycle_terminal: projection.lifecycle_terminal,
            listing_addr: projection.listing_addr,
            buyer_pubkey: projection.buyer_pubkey,
            seller_pubkey: projection.seller_pubkey,
            economics: projection.economics,
            evidence,
            validation_trust: None,
            online_evidence: None,
            eligibility,
            next_action,
            event_ids,
            request_event_id: projection.request_event_id,
            decision_event_id: projection.decision_event_id,
            agreement_event_id: projection.agreement_event_id,
            rhi_receipt_event_id: projection.validation_receipt_event_id,
            cancellation_event_id: projection.cancellation_event_id,
            last_event_id: projection.last_event_id,
            issues: projection.issues.into_iter().map(Into::into).collect(),
        }
    }

    fn missing(
        locator: RadrootsTradeLocator,
        event_count: usize,
        limit_applied: u32,
        event_ids: Vec<RadrootsEventId>,
    ) -> Self {
        Self::from_projection(
            locator.clone(),
            locator.root_event_id.clone(),
            Vec::new(),
            RadrootsOrderProjection {
                order_id: locator.order_id().clone(),
                status: RadrootsTradeWorkflowState::Missing,
                request_event_id: None,
                decision_event_id: None,
                cancellation_event_id: None,
                validation_receipt_event_id: None,
                lifecycle_terminal: false,
                economics: None,
                agreement_event_id: None,
                pending_inventory_reservations: Vec::new(),
                committed_inventory_reservations: Vec::new(),
                listing_addr: locator.listing_addr.clone(),
                buyer_pubkey: locator.buyer_pubkey.clone(),
                seller_pubkey: locator.seller_pubkey.clone(),
                last_event_id: None,
                issues: Vec::new(),
            },
            event_count,
            limit_applied,
            event_ids,
        )
    }

    fn ambiguous(
        locator: RadrootsTradeLocator,
        candidates: Vec<RadrootsTradeLocatorCandidate>,
        event_count: usize,
        limit_applied: u32,
        event_ids: Vec<RadrootsEventId>,
    ) -> Self {
        let ambiguity_candidates = candidates
            .into_iter()
            .map(|candidate| TradeStatusAmbiguityCandidate {
                locator: candidate.locator(),
            })
            .collect::<Vec<_>>();
        let mut receipt = Self::missing(locator, event_count, limit_applied, event_ids);
        receipt.status = TradeStatusKind::Ambiguous;
        receipt.next_action = TradeStatusNextActionKind::InspectEvidenceIssues;
        receipt.ambiguity_candidates = ambiguity_candidates;
        receipt
    }
}

#[cfg(feature = "runtime")]
impl TradeStatusEvidenceSummary {
    fn from_projection(
        projection: &RadrootsOrderProjection,
        event_count: usize,
        limit_applied: u32,
    ) -> Self {
        Self {
            event_count,
            limit_applied,
            has_request: projection.request_event_id.is_some(),
            has_decision: projection.decision_event_id.is_some(),
            has_agreement: projection.agreement_event_id.is_some(),
            has_validation_receipt: projection.validation_receipt_event_id.is_some(),
            has_cancellation: projection.cancellation_event_id.is_some(),
            has_issues: !projection.issues.is_empty(),
        }
    }
}

#[cfg(feature = "runtime")]
impl TradeStatusEligibility {
    fn from_projection(projection: &RadrootsOrderProjection) -> Self {
        let clean = projection.issues.is_empty();
        let open = clean && !projection.lifecycle_terminal;
        let requested = projection.status == RadrootsTradeWorkflowState::Requested;

        Self {
            can_decide: open && requested && projection.decision_event_id.is_none(),
            can_cancel: open && requested,
        }
    }
}

#[cfg(feature = "runtime")]
impl TradeStatusNextActionKind {
    fn from_projection(
        projection: &RadrootsOrderProjection,
        eligibility: &TradeStatusEligibility,
    ) -> Self {
        if projection.status == RadrootsTradeWorkflowState::Missing {
            return Self::NoLocalOrder;
        }
        if !projection.issues.is_empty() || projection.status == RadrootsTradeWorkflowState::Invalid
        {
            return Self::InspectEvidenceIssues;
        }
        if projection.status == RadrootsTradeWorkflowState::AgreedPendingValidation {
            return Self::AwaitValidation;
        }
        if projection.lifecycle_terminal {
            return Self::Terminal;
        }
        if eligibility.can_decide {
            return Self::AwaitSellerDecision;
        }
        Self::Terminal
    }
}

#[cfg(feature = "signer-adapters")]
fn order_submit_plan(
    actor: &RadrootsActorContext,
    listing_event: RadrootsEventPtr,
    order_request: RadrootsOrderRequest,
    created_at: RadrootsSdkTimestamp,
) -> Result<TradeSubmitPlan, RadrootsSdkError> {
    require_buyer_actor(actor, "trade.prepare_submit")?;
    let listing_event_id = listing_event_id(&listing_event)?;
    let order_request =
        canonicalize_order_request_for_signer(order_request, actor.pubkey().as_str())
            .map_err(order_canonicalization_error)?;
    let created_at_nostr = created_at.try_into_nostr_created_at()?;
    let order_id = order_request.order_id.clone();
    let listing_addr = order_request.listing_addr.clone();
    let buyer_pubkey = order_request.buyer_pubkey.clone();
    let seller_pubkey = order_request.seller_pubkey.clone();
    let draft =
        order::build_order_request_draft(&listing_event, &order_request).map_err(|error| {
            RadrootsSdkError::InvalidRequest {
                message: format!("order submit draft encode failed: {error}"),
            }
        })?;
    let (frozen_draft, expected_event_id) = freeze_order_workflow_draft(
        draft.into_wire_parts(),
        TRADE_SUBMIT_CONTRACT_ID,
        order_request.buyer_pubkey.as_str(),
        created_at_nostr,
        "trade.prepare_submit",
    );
    Ok(TradeSubmitPlan {
        workflow: order_workflow_plan(
            TradeWorkflowKind::Submit,
            expected_event_id.clone(),
            created_at,
        ),
        order_id,
        listing_addr,
        buyer_pubkey,
        seller_pubkey,
        listing_event_id,
        expected_event_id,
        frozen_draft,
        created_at,
    })
}

#[cfg(feature = "signer-adapters")]
fn order_decision_plan(
    actor: &RadrootsActorContext,
    request_event: RadrootsEventPtr,
    decision: RadrootsOrderDecision,
    created_at: RadrootsSdkTimestamp,
) -> Result<TradeDecisionPlan, RadrootsSdkError> {
    require_seller_actor(actor, "trade.prepare_decision")?;
    let request_event_id = request_event_id(&request_event)?;
    if decision.seller_pubkey.as_str() != actor.pubkey().as_str() {
        return Err(RadrootsSdkError::UnauthorizedActor {
            operation: "trade.prepare_decision".to_owned(),
            reason: "actor pubkey must match order seller_pubkey".to_owned(),
        });
    }
    let decision = canonicalize_order_decision_for_signer(decision, actor.pubkey().as_str())
        .map_err(order_decision_canonicalization_error)?;
    let created_at_nostr = created_at.try_into_nostr_created_at()?;
    let order_id = decision.order_id.clone();
    let listing_addr = decision.listing_addr.clone();
    let buyer_pubkey = decision.buyer_pubkey.clone();
    let seller_pubkey = decision.seller_pubkey.clone();
    validate_order_payload(&decision, "order decision")
        .expect("canonical order decision payload validates");
    let draft = order::build_order_decision_draft(&request_event_id, &request_event_id, &decision)
        .expect("validated order decision draft encodes");
    let (frozen_draft, expected_event_id) = freeze_order_workflow_draft(
        draft.into_wire_parts(),
        TRADE_DECISION_CONTRACT_ID,
        decision.seller_pubkey.as_str(),
        created_at_nostr,
        "trade.prepare_decision",
    );
    Ok(TradeDecisionPlan {
        workflow: order_workflow_plan(
            TradeWorkflowKind::Decision,
            expected_event_id.clone(),
            created_at,
        ),
        order_id,
        listing_addr,
        buyer_pubkey,
        seller_pubkey,
        request_event_id,
        expected_event_id,
        frozen_draft,
        created_at,
    })
}

#[cfg(feature = "signer-adapters")]
fn order_cancellation_plan(
    actor: &RadrootsActorContext,
    root_event: RadrootsEventPtr,
    previous_event: RadrootsEventPtr,
    cancellation: RadrootsOrderCancellation,
    created_at: RadrootsSdkTimestamp,
) -> Result<TradeCancellationPlan, RadrootsSdkError> {
    require_buyer_actor(actor, "trade.prepare_cancellation")?;
    let root_event_id = order_reference_event_id(&root_event, "root")?;
    let previous_event_id = order_reference_event_id(&previous_event, "previous")?;
    if cancellation.buyer_pubkey.as_str() != actor.pubkey().as_str() {
        return Err(RadrootsSdkError::UnauthorizedActor {
            operation: "trade.prepare_cancellation".to_owned(),
            reason: "actor pubkey must match order buyer_pubkey".to_owned(),
        });
    }
    let created_at_nostr = created_at.try_into_nostr_created_at()?;
    let order_id = cancellation.order_id.clone();
    let listing_addr = cancellation.listing_addr.clone();
    let buyer_pubkey = cancellation.buyer_pubkey.clone();
    let seller_pubkey = cancellation.seller_pubkey.clone();
    validate_order_payload(&cancellation, "order cancellation")?;
    let draft =
        order::build_order_cancellation_draft(&root_event_id, &previous_event_id, &cancellation)
            .expect("validated order cancellation draft encodes");
    let (frozen_draft, expected_event_id) = freeze_order_workflow_draft(
        draft.into_wire_parts(),
        TRADE_CANCELLATION_CONTRACT_ID,
        buyer_pubkey.as_str(),
        created_at_nostr,
        "order cancellation",
    );
    Ok(TradeCancellationPlan {
        workflow: order_workflow_plan(
            TradeWorkflowKind::Cancellation,
            expected_event_id.clone(),
            created_at,
        ),
        order_id,
        listing_addr,
        buyer_pubkey,
        seller_pubkey,
        root_event_id,
        previous_event_id,
        expected_event_id,
        frozen_draft,
        created_at,
    })
}

#[cfg(feature = "signer-adapters")]
fn order_workflow_plan(
    kind: TradeWorkflowKind,
    expected_event_id: RadrootsEventId,
    created_at: RadrootsSdkTimestamp,
) -> TradeWorkflowPlan {
    TradeWorkflowPlan {
        kind,
        operation_kind: kind.operation_kind(),
        contract_id: kind.contract_id(),
        expected_event_id,
        created_at,
    }
}

#[cfg(feature = "signer-adapters")]
fn order_submit_receipt(
    plan: TradeSubmitPlan,
    enqueue: crate::workflow_runtime::SdkWorkflowEnqueueReceipt,
) -> TradeSubmitReceipt {
    let locator = RadrootsTradeLocator::from_order_id(plan.order_id.clone())
        .with_root_event_id(enqueue.signed_event_id.clone())
        .with_listing_addr(plan.listing_addr.clone())
        .with_buyer_pubkey(plan.buyer_pubkey.clone())
        .with_seller_pubkey(plan.seller_pubkey.clone());
    TradeSubmitReceipt {
        workflow: order_workflow_enqueue_receipt(
            TradeWorkflowKind::Submit,
            plan.expected_event_id.clone(),
            &enqueue,
        ),
        locator,
        order_id: plan.order_id,
        listing_addr: plan.listing_addr,
        buyer_pubkey: plan.buyer_pubkey,
        seller_pubkey: plan.seller_pubkey,
        listing_event_id: plan.listing_event_id,
        expected_event_id: plan.expected_event_id,
        signed_event_id: enqueue.signed_event_id,
        local_event_seq: enqueue.local_event_seq,
        outbox_operation_id: enqueue.outbox_operation_id,
        outbox_event_id: enqueue.outbox_event_id,
        state: enqueue.state.into(),
        idempotency_digest_prefix: Some(enqueue.idempotency_digest_prefix),
    }
}

#[cfg(feature = "signer-adapters")]
fn order_decision_receipt(
    plan: TradeDecisionPlan,
    enqueue: crate::workflow_runtime::SdkWorkflowEnqueueReceipt,
) -> TradeDecisionReceipt {
    let locator = RadrootsTradeLocator::from_order_id(plan.order_id.clone())
        .with_root_event_id(plan.request_event_id.clone())
        .with_listing_addr(plan.listing_addr.clone())
        .with_buyer_pubkey(plan.buyer_pubkey.clone())
        .with_seller_pubkey(plan.seller_pubkey.clone());
    TradeDecisionReceipt {
        workflow: order_workflow_enqueue_receipt(
            TradeWorkflowKind::Decision,
            plan.expected_event_id.clone(),
            &enqueue,
        ),
        locator,
        order_id: plan.order_id,
        listing_addr: plan.listing_addr,
        buyer_pubkey: plan.buyer_pubkey,
        seller_pubkey: plan.seller_pubkey,
        request_event_id: plan.request_event_id,
        expected_event_id: plan.expected_event_id,
        signed_event_id: enqueue.signed_event_id,
        local_event_seq: enqueue.local_event_seq,
        outbox_operation_id: enqueue.outbox_operation_id,
        outbox_event_id: enqueue.outbox_event_id,
        state: enqueue.state.into(),
        idempotency_digest_prefix: Some(enqueue.idempotency_digest_prefix),
    }
}

#[cfg(feature = "signer-adapters")]
fn order_cancellation_receipt(
    plan: TradeCancellationPlan,
    enqueue: crate::workflow_runtime::SdkWorkflowEnqueueReceipt,
) -> TradeCancellationReceipt {
    let locator = RadrootsTradeLocator::from_order_id(plan.order_id.clone())
        .with_root_event_id(plan.root_event_id.clone())
        .with_listing_addr(plan.listing_addr.clone())
        .with_buyer_pubkey(plan.buyer_pubkey.clone())
        .with_seller_pubkey(plan.seller_pubkey.clone());
    TradeCancellationReceipt {
        workflow: order_workflow_enqueue_receipt(
            TradeWorkflowKind::Cancellation,
            plan.expected_event_id.clone(),
            &enqueue,
        ),
        locator,
        order_id: plan.order_id,
        listing_addr: plan.listing_addr,
        buyer_pubkey: plan.buyer_pubkey,
        seller_pubkey: plan.seller_pubkey,
        root_event_id: plan.root_event_id,
        previous_event_id: plan.previous_event_id,
        expected_event_id: plan.expected_event_id,
        signed_event_id: enqueue.signed_event_id,
        local_event_seq: enqueue.local_event_seq,
        outbox_operation_id: enqueue.outbox_operation_id,
        outbox_event_id: enqueue.outbox_event_id,
        state: enqueue.state.into(),
        idempotency_digest_prefix: Some(enqueue.idempotency_digest_prefix),
    }
}

#[cfg(feature = "signer-adapters")]
fn order_workflow_enqueue_receipt(
    kind: TradeWorkflowKind,
    expected_event_id: RadrootsEventId,
    enqueue: &crate::workflow_runtime::SdkWorkflowEnqueueReceipt,
) -> TradeWorkflowEnqueueReceipt {
    let state = SdkMutationState::from(enqueue.state);
    let digest_prefix = Some(enqueue.idempotency_digest_prefix.clone());
    let safe_retry_same_key = true;
    let replayed_existing_operation = state == SdkMutationState::AlreadyQueued;
    TradeWorkflowEnqueueReceipt {
        kind,
        operation_kind: kind.operation_kind(),
        expected_event_id,
        signed_event_id: enqueue.signed_event_id.clone(),
        local_event_seq: enqueue.local_event_seq,
        outbox_operation_id: enqueue.outbox_operation_id,
        outbox_event_id: enqueue.outbox_event_id,
        state,
        idempotency_digest_prefix: digest_prefix.clone(),
        idempotency: TradeWorkflowIdempotencyReceipt {
            digest_prefix,
            replayed_existing_operation,
            safe_to_retry_with_same_idempotency_key: safe_retry_same_key,
        },
        retry: TradeWorkflowRetryAdvice {
            retryable_after_error: false,
            safe_to_retry_enqueue_with_same_idempotency_key: safe_retry_same_key,
            recovery_actions: Vec::new(),
        },
    }
}

#[cfg(feature = "signer-adapters")]
fn freeze_order_workflow_draft(
    parts: RadrootsNip01EventWireParts,
    contract_id: &str,
    expected_pubkey: &str,
    created_at: u32,
    _operation: &'static str,
) -> (RadrootsEventDraft, RadrootsEventId) {
    let frozen_draft = RadrootsEventDraft::new(
        contract_id,
        parts.kind,
        u64::from(created_at),
        parts.tags,
        parts.content,
        expected_pubkey,
    )
    .expect("validated order workflow draft freezes");
    let expected_event_id = RadrootsEventId::parse(frozen_draft.expected_event_id_str())
        .expect("frozen order workflow draft produces a valid event id");
    (frozen_draft, expected_event_id)
}

#[cfg(feature = "signer-adapters")]
fn validate_order_payload<T>(payload: &T, operation: &'static str) -> Result<(), RadrootsSdkError>
where
    T: OrderPayloadValidate,
{
    payload
        .validate_order_payload()
        .map_err(|error| RadrootsSdkError::InvalidRequest {
            message: format!("{operation} payload is invalid: {error}"),
        })
}

#[cfg(feature = "signer-adapters")]
trait OrderPayloadValidate {
    fn validate_order_payload(
        &self,
    ) -> Result<(), radroots_event::order::RadrootsOrderPayloadError>;
}

#[cfg(feature = "signer-adapters")]
impl OrderPayloadValidate for RadrootsOrderDecision {
    fn validate_order_payload(
        &self,
    ) -> Result<(), radroots_event::order::RadrootsOrderPayloadError> {
        self.validate()
    }
}

#[cfg(feature = "signer-adapters")]
impl OrderPayloadValidate for RadrootsOrderCancellation {
    fn validate_order_payload(
        &self,
    ) -> Result<(), radroots_event::order::RadrootsOrderPayloadError> {
        self.validate()
    }
}

#[cfg(feature = "runtime")]
struct TradeRequestEvidence {
    order_id: RadrootsOrderId,
    listing_addr: RadrootsListingAddress,
    buyer_pubkey: RadrootsPublicKey,
    seller_pubkey: RadrootsPublicKey,
    request_event_id: RadrootsEventId,
}

#[cfg(feature = "runtime")]
fn parse_order_request_evidence(
    event: &RadrootsEventEnvelope,
) -> Result<TradeRequestEvidence, RadrootsSdkError> {
    let request_event_id = RadrootsEventId::parse(event.id_str()).map_err(|error| {
        RadrootsSdkError::InvalidRequest {
            message: format!("order request evidence event id is invalid: {error}"),
        }
    })?;
    let envelope =
        order::parse_order_request(event).map_err(|error| RadrootsSdkError::InvalidRequest {
            message: format!("order request evidence decode failed: {error}"),
        })?;
    let payload = envelope.payload;
    Ok(TradeRequestEvidence {
        order_id: payload.order_id,
        listing_addr: payload.listing_addr,
        buyer_pubkey: payload.buyer_pubkey,
        seller_pubkey: payload.seller_pubkey,
        request_event_id,
    })
}

#[cfg(feature = "runtime")]
fn sdk_timestamp_ms(timestamp: RadrootsSdkTimestamp) -> Result<i64, RadrootsSdkError> {
    let seconds = timestamp.unix_seconds();
    let millis = seconds
        .checked_mul(1_000)
        .ok_or(RadrootsSdkError::TimestampOutOfRange { value: seconds })?;
    i64::try_from(millis).map_err(|_| RadrootsSdkError::TimestampOutOfRange { value: seconds })
}

#[cfg(feature = "signer-adapters")]
fn require_decision_request_evidence(
    plan: &TradeDecisionPlan,
    projection: &RadrootsOrderProjection,
) -> Result<(), RadrootsSdkError> {
    let Some(request_event_id) = &projection.request_event_id else {
        return Err(RadrootsSdkError::InvalidRequest {
            message: format!(
                "order decision requires local request evidence for order {}",
                plan.order_id
            ),
        });
    };
    if request_event_id != &plan.request_event_id {
        return Err(RadrootsSdkError::InvalidRequest {
            message: format!(
                "order decision request evidence {} does not match local request {} for order {}",
                plan.request_event_id, request_event_id, plan.order_id
            ),
        });
    }
    if !matches!(&projection.status, RadrootsTradeWorkflowState::Requested) {
        return Err(RadrootsSdkError::InvalidRequest {
            message: format!(
                "order decision requires requested local state for order {}; current state is {:?}",
                plan.order_id, projection.status
            ),
        });
    }
    if !projection.issues.is_empty() {
        return Err(RadrootsSdkError::InvalidRequest {
            message: format!(
                "order decision request evidence for order {} has {} reducer issue(s)",
                plan.order_id,
                projection.issues.len()
            ),
        });
    }
    require_projection_match(
        "order decision",
        "listing_addr",
        projection.listing_addr.as_ref(),
        &plan.listing_addr,
        &plan.order_id,
    )?;
    require_projection_match(
        "order decision",
        "buyer_pubkey",
        projection.buyer_pubkey.as_ref(),
        &plan.buyer_pubkey,
        &plan.order_id,
    )?;
    require_projection_match(
        "order decision",
        "seller_pubkey",
        projection.seller_pubkey.as_ref(),
        &plan.seller_pubkey,
        &plan.order_id,
    )?;
    Ok(())
}

#[cfg(feature = "signer-adapters")]
#[derive(Clone, Copy)]
struct OrderLifecycleReferences<'a> {
    operation: &'static str,
    order_id: &'a RadrootsOrderId,
    listing_addr: &'a RadrootsListingAddress,
    buyer_pubkey: &'a RadrootsPublicKey,
    seller_pubkey: &'a RadrootsPublicKey,
    root_event_id: &'a RadrootsEventId,
    previous_event_id: &'a RadrootsEventId,
}

#[cfg(feature = "signer-adapters")]
fn require_cancellation_state(
    plan: &TradeCancellationPlan,
    projection: &RadrootsOrderProjection,
) -> Result<(), RadrootsSdkError> {
    let refs = OrderLifecycleReferences {
        operation: "order cancellation",
        order_id: &plan.order_id,
        listing_addr: &plan.listing_addr,
        buyer_pubkey: &plan.buyer_pubkey,
        seller_pubkey: &plan.seller_pubkey,
        root_event_id: &plan.root_event_id,
        previous_event_id: &plan.previous_event_id,
    };
    require_clean_lifecycle_projection(refs, projection)?;
    if projection.status != RadrootsTradeWorkflowState::Requested {
        return Err(lifecycle_invalid(
            refs.operation,
            refs.order_id,
            format!(
                "cancellation requires requested local state; current state is {:?}",
                projection.status
            ),
        ));
    }
    require_no_lifecycle_terminal(&refs, projection)?;
    require_lifecycle_previous_is_current(&refs, projection)
}

#[cfg(feature = "signer-adapters")]
fn require_clean_lifecycle_projection(
    refs: OrderLifecycleReferences<'_>,
    projection: &RadrootsOrderProjection,
) -> Result<(), RadrootsSdkError> {
    let Some(request_event_id) = &projection.request_event_id else {
        return Err(lifecycle_invalid(
            refs.operation,
            refs.order_id,
            "requires local order request evidence",
        ));
    };
    if request_event_id != refs.root_event_id {
        return Err(lifecycle_invalid(
            refs.operation,
            refs.order_id,
            format!(
                "root event {} does not match local request {}",
                refs.root_event_id, request_event_id
            ),
        ));
    }
    if !projection.issues.is_empty() {
        return Err(lifecycle_invalid(
            refs.operation,
            refs.order_id,
            format!(
                "local order evidence has {} reducer issue(s)",
                projection.issues.len()
            ),
        ));
    }
    require_projection_match(
        refs.operation,
        "listing_addr",
        projection.listing_addr.as_ref(),
        refs.listing_addr,
        refs.order_id,
    )?;
    require_projection_match(
        refs.operation,
        "buyer_pubkey",
        projection.buyer_pubkey.as_ref(),
        refs.buyer_pubkey,
        refs.order_id,
    )?;
    require_projection_match(
        refs.operation,
        "seller_pubkey",
        projection.seller_pubkey.as_ref(),
        refs.seller_pubkey,
        refs.order_id,
    )
}

#[cfg(feature = "signer-adapters")]
fn require_no_lifecycle_terminal(
    refs: &OrderLifecycleReferences<'_>,
    projection: &RadrootsOrderProjection,
) -> Result<(), RadrootsSdkError> {
    if projection.lifecycle_terminal {
        Err(lifecycle_invalid(
            refs.operation,
            refs.order_id,
            "requires non-terminal local order state",
        ))
    } else {
        Ok(())
    }
}

#[cfg(feature = "signer-adapters")]
fn require_lifecycle_previous_is_current(
    refs: &OrderLifecycleReferences<'_>,
    projection: &RadrootsOrderProjection,
) -> Result<(), RadrootsSdkError> {
    match projection.last_event_id.as_ref() {
        Some(last_event_id) if last_event_id == refs.previous_event_id => Ok(()),
        Some(last_event_id) => Err(lifecycle_invalid(
            refs.operation,
            refs.order_id,
            format!(
                "previous event {} does not match current lifecycle event {}",
                refs.previous_event_id, last_event_id
            ),
        )),
        None => Err(lifecycle_invalid(
            refs.operation,
            refs.order_id,
            "requires current lifecycle event evidence",
        )),
    }
}

#[cfg(feature = "signer-adapters")]
fn lifecycle_invalid(
    operation: &'static str,
    order_id: &RadrootsOrderId,
    reason: impl Into<String>,
) -> RadrootsSdkError {
    RadrootsSdkError::InvalidRequest {
        message: format!("{operation} for order {order_id} {}", reason.into()),
    }
}

#[cfg(feature = "signer-adapters")]
fn require_projection_match<T>(
    operation: &'static str,
    field: &'static str,
    actual: Option<&T>,
    expected: &T,
    order_id: &RadrootsOrderId,
) -> Result<(), RadrootsSdkError>
where
    T: core::fmt::Display + PartialEq,
{
    match actual {
        Some(actual) if actual == expected => Ok(()),
        Some(actual) => Err(RadrootsSdkError::InvalidRequest {
            message: format!(
                "{operation} {field} {expected} does not match local request {actual} for order {order_id}"
            ),
        }),
        None => Err(RadrootsSdkError::InvalidRequest {
            message: format!(
                "{operation} request evidence is missing {field} for order {order_id}"
            ),
        }),
    }
}

#[cfg(feature = "signer-adapters")]
fn require_buyer_actor(
    actor: &RadrootsActorContext,
    operation: &'static str,
) -> Result<(), RadrootsSdkError> {
    if actor.satisfies(RadrootsActorRole::Buyer) {
        Ok(())
    } else {
        Err(RadrootsSdkError::UnauthorizedActor {
            operation: operation.to_owned(),
            reason: "missing role Buyer".to_owned(),
        })
    }
}

#[cfg(feature = "runtime")]
fn require_seller_actor(
    actor: &RadrootsActorContext,
    operation: &'static str,
) -> Result<(), RadrootsSdkError> {
    if actor.satisfies(RadrootsActorRole::Seller) {
        Ok(())
    } else {
        Err(RadrootsSdkError::UnauthorizedActor {
            operation: operation.to_owned(),
            reason: "missing role Seller".to_owned(),
        })
    }
}

#[cfg(feature = "signer-adapters")]
fn listing_event_id(listing_event: &RadrootsEventPtr) -> Result<RadrootsEventId, RadrootsSdkError> {
    RadrootsEventId::parse(listing_event.id.as_str()).map_err(|error| {
        RadrootsSdkError::InvalidRequest {
            message: format!("listing evidence event id is invalid: {error}"),
        }
    })
}

#[cfg(feature = "signer-adapters")]
fn request_event_id(request_event: &RadrootsEventPtr) -> Result<RadrootsEventId, RadrootsSdkError> {
    RadrootsEventId::parse(request_event.id.as_str()).map_err(|error| {
        RadrootsSdkError::InvalidRequest {
            message: format!("order request evidence event id is invalid: {error}"),
        }
    })
}

#[cfg(feature = "signer-adapters")]
fn order_reference_event_id(
    event: &RadrootsEventPtr,
    label: &'static str,
) -> Result<RadrootsEventId, RadrootsSdkError> {
    RadrootsEventId::parse(event.id.as_str()).map_err(|error| RadrootsSdkError::InvalidRequest {
        message: format!("order {label} evidence event id is invalid: {error}"),
    })
}

#[cfg(feature = "signer-adapters")]
fn order_canonicalization_error(error: RadrootsOrderCanonicalizationError) -> RadrootsSdkError {
    match error {
        RadrootsOrderCanonicalizationError::InvalidBuyerSigner => {
            RadrootsSdkError::UnauthorizedActor {
                operation: "trade.prepare_submit".to_owned(),
                reason: "actor pubkey must match order buyer_pubkey".to_owned(),
            }
        }
        error => RadrootsSdkError::InvalidRequest {
            message: format!("order submit request is invalid: {error}"),
        },
    }
}

#[cfg(feature = "signer-adapters")]
fn order_decision_canonicalization_error(
    error: RadrootsOrderCanonicalizationError,
) -> RadrootsSdkError {
    RadrootsSdkError::InvalidRequest {
        message: format!("order decision request is invalid: {error}"),
    }
}

#[cfg(feature = "runtime")]
impl From<RadrootsTradeWorkflowState> for TradeStatusKind {
    fn from(status: RadrootsTradeWorkflowState) -> Self {
        match status {
            RadrootsTradeWorkflowState::Missing => Self::Missing,
            RadrootsTradeWorkflowState::Requested => Self::Requested,
            RadrootsTradeWorkflowState::AgreedPendingValidation => Self::AgreedPendingValidation,
            RadrootsTradeWorkflowState::Committed => Self::Committed,
            RadrootsTradeWorkflowState::Declined => Self::Declined,
            RadrootsTradeWorkflowState::Cancelled => Self::Cancelled,
            RadrootsTradeWorkflowState::ValidationExpired => Self::ValidationExpired,
            RadrootsTradeWorkflowState::Invalid => Self::Invalid,
        }
    }
}

#[cfg(feature = "runtime")]
impl From<RadrootsOrderIssue> for SdkTradeStatusIssue {
    fn from(issue: RadrootsOrderIssue) -> Self {
        match issue {
            RadrootsOrderIssue::MissingRequest => {
                Self::new(SdkTradeStatusIssueKind::MissingRequest, Vec::new())
            }
            RadrootsOrderIssue::MultipleRequests { event_ids } => {
                Self::new(SdkTradeStatusIssueKind::MultipleRequests, event_ids)
            }
            RadrootsOrderIssue::RequestPayloadInvalid { event_id } => {
                Self::single(SdkTradeStatusIssueKind::RequestPayloadInvalid, event_id)
            }
            RadrootsOrderIssue::RequestOrderIdMismatch { event_id } => {
                Self::single(SdkTradeStatusIssueKind::RequestOrderIdMismatch, event_id)
            }
            RadrootsOrderIssue::RequestAuthorMismatch { event_id } => {
                Self::single(SdkTradeStatusIssueKind::RequestAuthorMismatch, event_id)
            }
            RadrootsOrderIssue::RequestListingAddressInvalid { event_id } => Self::single(
                SdkTradeStatusIssueKind::RequestListingAddressInvalid,
                event_id,
            ),
            RadrootsOrderIssue::RequestSellerListingMismatch { event_id } => Self::single(
                SdkTradeStatusIssueKind::RequestSellerListingMismatch,
                event_id,
            ),
            RadrootsOrderIssue::DecisionPayloadInvalid { event_id } => {
                Self::single(SdkTradeStatusIssueKind::DecisionPayloadInvalid, event_id)
            }
            RadrootsOrderIssue::DecisionOrderIdMismatch { event_id } => {
                Self::single(SdkTradeStatusIssueKind::DecisionOrderIdMismatch, event_id)
            }
            RadrootsOrderIssue::DecisionAuthorMismatch { event_id } => {
                Self::single(SdkTradeStatusIssueKind::DecisionAuthorMismatch, event_id)
            }
            RadrootsOrderIssue::DecisionCounterpartyMismatch { event_id } => Self::single(
                SdkTradeStatusIssueKind::DecisionCounterpartyMismatch,
                event_id,
            ),
            RadrootsOrderIssue::DecisionBuyerMismatch { event_id } => {
                Self::single(SdkTradeStatusIssueKind::DecisionBuyerMismatch, event_id)
            }
            RadrootsOrderIssue::DecisionSellerMismatch { event_id } => {
                Self::single(SdkTradeStatusIssueKind::DecisionSellerMismatch, event_id)
            }
            RadrootsOrderIssue::DecisionListingAddressInvalid { event_id } => Self::single(
                SdkTradeStatusIssueKind::DecisionListingAddressInvalid,
                event_id,
            ),
            RadrootsOrderIssue::DecisionListingMismatch { event_id } => {
                Self::single(SdkTradeStatusIssueKind::DecisionListingMismatch, event_id)
            }
            RadrootsOrderIssue::DecisionRootMismatch { event_id } => {
                Self::single(SdkTradeStatusIssueKind::DecisionRootMismatch, event_id)
            }
            RadrootsOrderIssue::DecisionPreviousMismatch { event_id } => {
                Self::single(SdkTradeStatusIssueKind::DecisionPreviousMismatch, event_id)
            }
            RadrootsOrderIssue::DecisionMissingInventoryCommitments { event_id } => Self::single(
                SdkTradeStatusIssueKind::DecisionMissingInventoryCommitments,
                event_id,
            ),
            RadrootsOrderIssue::DecisionInventoryCommitmentMismatch { event_id } => Self::single(
                SdkTradeStatusIssueKind::DecisionInventoryCommitmentMismatch,
                event_id,
            ),
            RadrootsOrderIssue::DecisionMissingReason { event_id } => {
                Self::single(SdkTradeStatusIssueKind::DecisionMissingReason, event_id)
            }
            RadrootsOrderIssue::ConflictingDecisions { event_ids } => {
                Self::new(SdkTradeStatusIssueKind::ConflictingDecisions, event_ids)
            }
            RadrootsOrderIssue::CancellationWithoutCancellableOrder { event_id } => Self::single(
                SdkTradeStatusIssueKind::CancellationWithoutCancellableOrder,
                event_id,
            ),
            RadrootsOrderIssue::CancellationPayloadInvalid { event_id } => Self::single(
                SdkTradeStatusIssueKind::CancellationPayloadInvalid,
                event_id,
            ),
            RadrootsOrderIssue::CancellationOrderIdMismatch { event_id } => Self::single(
                SdkTradeStatusIssueKind::CancellationOrderIdMismatch,
                event_id,
            ),
            RadrootsOrderIssue::CancellationAuthorMismatch { event_id } => Self::single(
                SdkTradeStatusIssueKind::CancellationAuthorMismatch,
                event_id,
            ),
            RadrootsOrderIssue::CancellationCounterpartyMismatch { event_id } => Self::single(
                SdkTradeStatusIssueKind::CancellationCounterpartyMismatch,
                event_id,
            ),
            RadrootsOrderIssue::CancellationBuyerMismatch { event_id } => {
                Self::single(SdkTradeStatusIssueKind::CancellationBuyerMismatch, event_id)
            }
            RadrootsOrderIssue::CancellationSellerMismatch { event_id } => Self::single(
                SdkTradeStatusIssueKind::CancellationSellerMismatch,
                event_id,
            ),
            RadrootsOrderIssue::CancellationListingAddressInvalid { event_id } => Self::single(
                SdkTradeStatusIssueKind::CancellationListingAddressInvalid,
                event_id,
            ),
            RadrootsOrderIssue::CancellationListingMismatch { event_id } => Self::single(
                SdkTradeStatusIssueKind::CancellationListingMismatch,
                event_id,
            ),
            RadrootsOrderIssue::CancellationRootMismatch { event_id } => {
                Self::single(SdkTradeStatusIssueKind::CancellationRootMismatch, event_id)
            }
            RadrootsOrderIssue::CancellationPreviousMismatch { event_id } => Self::single(
                SdkTradeStatusIssueKind::CancellationPreviousMismatch,
                event_id,
            ),
            RadrootsOrderIssue::ForkedLifecycle { event_ids } => {
                Self::new(SdkTradeStatusIssueKind::ForkedLifecycle, event_ids)
            }
            RadrootsOrderIssue::ValidationReceiptWithoutPendingAgreement { event_id } => {
                Self::single(
                    SdkTradeStatusIssueKind::ValidationReceiptWithoutPendingAgreement,
                    event_id,
                )
            }
            RadrootsOrderIssue::ValidationReceiptOrderIdMismatch { event_id } => Self::single(
                SdkTradeStatusIssueKind::ValidationReceiptOrderIdMismatch,
                event_id,
            ),
            RadrootsOrderIssue::ValidationReceiptTypeMismatch { event_id } => Self::single(
                SdkTradeStatusIssueKind::ValidationReceiptTypeMismatch,
                event_id,
            ),
            RadrootsOrderIssue::ValidationReceiptRootMismatch { event_id } => Self::single(
                SdkTradeStatusIssueKind::ValidationReceiptRootMismatch,
                event_id,
            ),
            RadrootsOrderIssue::ValidationReceiptTargetMismatch { event_id } => Self::single(
                SdkTradeStatusIssueKind::ValidationReceiptTargetMismatch,
                event_id,
            ),
            RadrootsOrderIssue::ValidationReceiptListingMismatch { event_id } => Self::single(
                SdkTradeStatusIssueKind::ValidationReceiptListingMismatch,
                event_id,
            ),
            RadrootsOrderIssue::ConflictingValidationReceipts { event_ids } => Self::new(
                SdkTradeStatusIssueKind::ConflictingValidationReceipts,
                event_ids,
            ),
            RadrootsOrderIssue::DeterministicValidationFailure { event_id, .. } => Self::single(
                SdkTradeStatusIssueKind::DeterministicValidationFailure,
                event_id,
            ),
            RadrootsOrderIssue::StaleListingEvent {
                expected_event_id,
                current_event_id,
            } => Self::new(
                SdkTradeStatusIssueKind::StaleListingEvent,
                vec![expected_event_id, current_event_id],
            ),
        }
    }
}

#[cfg(feature = "runtime")]
fn projection_error(error: RadrootsOrderStoreQueryError) -> RadrootsSdkError {
    let message = match error {
        RadrootsOrderStoreQueryError::Store(_) => "order status store query failed",
        RadrootsOrderStoreQueryError::InvalidStoredTagsJson { .. } => {
            "stored order event tags could not be decoded"
        }
        RadrootsOrderStoreQueryError::Decode { .. } => {
            "stored order event could not decode as order record"
        }
        RadrootsOrderStoreQueryError::InvalidStoredEnvelope { .. } => {
            "stored order event envelope is invalid"
        }
        RadrootsOrderStoreQueryError::Projection(error) => return error.into(),
    };
    RadrootsSdkError::Projection {
        message: message.to_owned(),
    }
}

#[cfg(feature = "runtime")]
fn camel_to_snake(value: &str) -> String {
    let mut output = String::new();
    for (index, character) in value.chars().enumerate() {
        if character.is_ascii_uppercase() {
            if index > 0 {
                output.push('_');
            }
            output.push(character.to_ascii_lowercase());
        } else {
            output.push(character);
        }
    }
    output
}

#[cfg(all(test, feature = "runtime", feature = "signer-adapters"))]
#[path = "../tests/unit/orders_runtime_tests.rs"]
mod tests;
