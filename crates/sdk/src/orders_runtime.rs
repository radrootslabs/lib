#[cfg(feature = "signer-adapters")]
use crate::TradeBuyerClient;
#[cfg(feature = "signer-adapters")]
use crate::workflow_runtime::enqueue_configured_signed_workflow;
#[cfg(any(feature = "signer-adapters", test))]
use crate::{
    AckPolicy, PrivacyPreflightConfirmation, PrivacyPreflightReceipt, ProductSensitivityField,
    PublishMode, PushOutboxReceipt, PushOutboxRequest, RadrootsSdkRecoveryAction,
    RelayResolutionPolicy, SdkIdempotencyKey, SdkMutationState,
    workflow_runtime::SdkWorkflowEnqueueRequest,
};
#[cfg(feature = "runtime")]
use crate::{
    RadrootsSdkError, RadrootsSdkTimestamp, TradeResyncClient, TradeSellerClient, TradesClient,
    order,
};
#[cfg(all(feature = "runtime", test))]
use crate::{SdkRelayUrlPolicy, workflow_runtime::enqueue_signed_workflow};
#[cfg(feature = "runtime")]
use radroots_authority::RadrootsActorContext;
#[cfg(all(feature = "runtime", test))]
use radroots_authority::RadrootsEventSigner;
#[cfg(feature = "runtime")]
use radroots_event_store::RadrootsEventIngest;
#[cfg(feature = "runtime")]
use radroots_events::{
    RadrootsNostrEvent,
    contract::RadrootsActorRole,
    ids::{RadrootsEventId, RadrootsListingAddress, RadrootsOrderId, RadrootsPublicKey},
    kinds::{
        KIND_ORDER_CANCELLATION, KIND_ORDER_DECISION, KIND_ORDER_REQUEST,
        KIND_ORDER_REVISION_DECISION, KIND_ORDER_REVISION_PROPOSAL,
    },
    order::RadrootsOrderEconomics,
    tags::TAG_P,
};
#[cfg(any(feature = "signer-adapters", test))]
use radroots_events::{
    RadrootsNostrEventPtr,
    draft::RadrootsFrozenEventDraft,
    ids::RadrootsOrderRevisionId,
    order::{
        RadrootsOrderCancellation, RadrootsOrderDecision, RadrootsOrderDecisionOutcome,
        RadrootsOrderInventoryCommitment, RadrootsOrderItem, RadrootsOrderRequest,
        RadrootsOrderRevisionDecision, RadrootsOrderRevisionOutcome, RadrootsOrderRevisionProposal,
    },
};
#[cfg(feature = "runtime")]
use radroots_events_codec::order::{
    order_cancellation_from_event, order_decision_from_event, order_request_from_event,
    order_revision_decision_from_event, order_revision_proposal_from_event,
};
#[cfg(any(feature = "signer-adapters", test))]
use radroots_events_codec::wire::{WireEventParts, to_frozen_draft};
#[cfg(feature = "runtime")]
use radroots_trade::identity::{RadrootsTradeLocator, RadrootsTradeLocatorCandidate};
#[cfg(any(feature = "signer-adapters", test))]
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
#[cfg(feature = "runtime")]
use radroots_trade::workflow::RadrootsTradeWorkflowState;
#[cfg(feature = "runtime")]
use serde::ser::SerializeStruct;
#[cfg(feature = "runtime")]
pub const TRADE_STATUS_DEFAULT_LIMIT: u32 = 500;
#[cfg(feature = "runtime")]
pub const TRADE_STATUS_MAX_LIMIT: u32 = 1_000;
#[cfg(any(feature = "signer-adapters", test))]
pub const TRADE_SUBMIT_OPERATION_KIND: &str = "trade.submit.v1";
#[cfg(any(feature = "signer-adapters", test))]
pub const TRADE_DECISION_OPERATION_KIND: &str = "trade.decision.v1";
#[cfg(any(feature = "signer-adapters", test))]
pub const TRADE_REVISION_PROPOSAL_OPERATION_KIND: &str = "trade.revision.proposal.v1";
#[cfg(any(feature = "signer-adapters", test))]
pub const TRADE_REVISION_DECISION_OPERATION_KIND: &str = "trade.revision.decision.v1";
#[cfg(any(feature = "signer-adapters", test))]
pub const TRADE_CANCELLATION_OPERATION_KIND: &str = "trade.cancellation.v1";

#[cfg(any(feature = "signer-adapters", test))]
const TRADE_SUBMIT_CONTRACT_ID: &str = "radroots.order.request.v1";
#[cfg(any(feature = "signer-adapters", test))]
const TRADE_DECISION_CONTRACT_ID: &str = "radroots.order.decision.v1";
#[cfg(any(feature = "signer-adapters", test))]
const TRADE_REVISION_PROPOSAL_CONTRACT_ID: &str = "radroots.order.revision_proposal.v1";
#[cfg(any(feature = "signer-adapters", test))]
const TRADE_REVISION_DECISION_CONTRACT_ID: &str = "radroots.order.revision_decision.v1";
#[cfg(any(feature = "signer-adapters", test))]
const TRADE_CANCELLATION_CONTRACT_ID: &str = "radroots.order.cancellation.v1";

#[cfg(any(feature = "signer-adapters", test))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum TradeWorkflowKind {
    Submit,
    Decision,
    RevisionProposal,
    RevisionDecision,
    Cancellation,
}

#[cfg(any(feature = "signer-adapters", test))]
impl TradeWorkflowKind {
    pub fn operation_kind(self) -> &'static str {
        match self {
            Self::Submit => TRADE_SUBMIT_OPERATION_KIND,
            Self::Decision => TRADE_DECISION_OPERATION_KIND,
            Self::RevisionProposal => TRADE_REVISION_PROPOSAL_OPERATION_KIND,
            Self::RevisionDecision => TRADE_REVISION_DECISION_OPERATION_KIND,
            Self::Cancellation => TRADE_CANCELLATION_OPERATION_KIND,
        }
    }

    pub fn contract_id(self) -> &'static str {
        match self {
            Self::Submit => TRADE_SUBMIT_CONTRACT_ID,
            Self::Decision => TRADE_DECISION_CONTRACT_ID,
            Self::RevisionProposal => TRADE_REVISION_PROPOSAL_CONTRACT_ID,
            Self::RevisionDecision => TRADE_REVISION_DECISION_CONTRACT_ID,
            Self::Cancellation => TRADE_CANCELLATION_CONTRACT_ID,
        }
    }
}

#[cfg(any(feature = "signer-adapters", test))]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct TradeWorkflowPlan {
    pub kind: TradeWorkflowKind,
    pub operation_kind: &'static str,
    pub contract_id: &'static str,
    pub expected_event_id: RadrootsEventId,
    pub created_at: RadrootsSdkTimestamp,
}

#[cfg(any(feature = "signer-adapters", test))]
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

#[cfg(any(feature = "signer-adapters", test))]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct TradeWorkflowIdempotencyReceipt {
    pub digest_prefix: Option<String>,
    pub replayed_existing_operation: bool,
    pub safe_to_retry_with_same_idempotency_key: bool,
}

#[cfg(any(feature = "signer-adapters", test))]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct TradeWorkflowRetryAdvice {
    pub retryable_after_error: bool,
    pub safe_to_retry_enqueue_with_same_idempotency_key: bool,
    pub recovery_actions: Vec<RadrootsSdkRecoveryAction>,
}

#[cfg(any(feature = "signer-adapters", test))]
#[derive(Clone, Debug, serde::Serialize)]
#[non_exhaustive]
pub struct TradeSubmitPrepareRequest {
    #[serde(serialize_with = "crate::actor_json::serialize_actor_context")]
    pub actor: RadrootsActorContext,
    pub listing_event: RadrootsNostrEventPtr,
    pub order: RadrootsOrderRequest,
    pub created_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(any(feature = "signer-adapters", test))]
#[cfg(test)]
impl TradeSubmitPrepareRequest {
    pub fn new(
        actor: RadrootsActorContext,
        listing_event: RadrootsNostrEventPtr,
        order: RadrootsOrderRequest,
    ) -> Self {
        Self {
            actor,
            listing_event,
            order,
            created_at: None,
        }
    }

    pub fn with_created_at(mut self, created_at: RadrootsSdkTimestamp) -> Self {
        self.created_at = Some(created_at);
        self
    }
}

#[cfg(feature = "runtime")]
#[cfg(test)]
#[derive(Clone, Debug, serde::Serialize)]
#[non_exhaustive]
pub struct TradeSubmitEnqueueRequest {
    #[serde(serialize_with = "crate::actor_json::serialize_actor_context")]
    pub actor: RadrootsActorContext,
    pub listing_event: RadrootsNostrEventPtr,
    pub order: RadrootsOrderRequest,
    pub target_relays: RelayResolutionPolicy,
    pub publish_mode: PublishMode,
    pub ack_policy: AckPolicy,
    pub idempotency_key: Option<SdkIdempotencyKey>,
    pub created_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(feature = "runtime")]
#[cfg(test)]
impl TradeSubmitEnqueueRequest {
    pub fn new(
        actor: RadrootsActorContext,
        listing_event: RadrootsNostrEventPtr,
        order: RadrootsOrderRequest,
        target_relays: RelayResolutionPolicy,
        publish_mode: PublishMode,
        ack_policy: AckPolicy,
    ) -> Self {
        Self {
            actor,
            listing_event,
            order,
            target_relays,
            publish_mode,
            ack_policy,
            idempotency_key: None,
            created_at: None,
        }
    }

    pub fn try_with_target_relays<I, S>(
        mut self,
        target_relays: I,
        policy: SdkRelayUrlPolicy,
    ) -> Result<Self, RadrootsSdkError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.target_relays = RelayResolutionPolicy::try_explicit(target_relays, policy)?;
        Ok(self)
    }

    pub fn with_idempotency_key(mut self, idempotency_key: SdkIdempotencyKey) -> Self {
        self.idempotency_key = Some(idempotency_key.into());
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

#[cfg(any(feature = "signer-adapters", test))]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct TradeSubmitPlan {
    pub workflow: TradeWorkflowPlan,
    pub order_id: RadrootsOrderId,
    pub listing_addr: RadrootsListingAddress,
    pub buyer_pubkey: RadrootsPublicKey,
    pub seller_pubkey: RadrootsPublicKey,
    pub listing_event_id: RadrootsEventId,
    pub expected_event_id: RadrootsEventId,
    pub frozen_draft: RadrootsFrozenEventDraft,
    pub created_at: RadrootsSdkTimestamp,
}

#[cfg(any(feature = "signer-adapters", test))]
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
    pub event: RadrootsNostrEvent,
    pub observed_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(feature = "runtime")]
impl TradeRequestEvidenceIngestRequest {
    pub fn new(event: RadrootsNostrEvent) -> Self {
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
    pub event: RadrootsNostrEvent,
    pub observed_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(feature = "runtime")]
impl TradeEvidenceIngestRequest {
    pub fn new(event: RadrootsNostrEvent) -> Self {
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

#[cfg(any(feature = "signer-adapters", test))]
#[derive(Clone, Debug, serde::Serialize)]
#[non_exhaustive]
pub struct TradeDecisionPrepareRequest {
    #[serde(serialize_with = "crate::actor_json::serialize_actor_context")]
    pub actor: RadrootsActorContext,
    pub request_event: RadrootsNostrEventPtr,
    pub decision: RadrootsOrderDecision,
    pub created_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(any(feature = "signer-adapters", test))]
#[cfg(test)]
impl TradeDecisionPrepareRequest {
    pub fn new(
        actor: RadrootsActorContext,
        request_event: RadrootsNostrEventPtr,
        decision: RadrootsOrderDecision,
    ) -> Self {
        Self {
            actor,
            request_event,
            decision,
            created_at: None,
        }
    }

    pub fn with_created_at(mut self, created_at: RadrootsSdkTimestamp) -> Self {
        self.created_at = Some(created_at);
        self
    }
}

#[cfg(feature = "runtime")]
#[cfg(test)]
#[derive(Clone, Debug, serde::Serialize)]
#[non_exhaustive]
pub struct TradeDecisionEnqueueRequest {
    #[serde(serialize_with = "crate::actor_json::serialize_actor_context")]
    pub actor: RadrootsActorContext,
    pub request_event: RadrootsNostrEventPtr,
    pub decision: RadrootsOrderDecision,
    pub target_relays: RelayResolutionPolicy,
    pub publish_mode: PublishMode,
    pub ack_policy: AckPolicy,
    pub idempotency_key: Option<SdkIdempotencyKey>,
    pub created_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(feature = "runtime")]
#[cfg(test)]
impl TradeDecisionEnqueueRequest {
    pub fn new(
        actor: RadrootsActorContext,
        request_event: RadrootsNostrEventPtr,
        decision: RadrootsOrderDecision,
        target_relays: RelayResolutionPolicy,
        publish_mode: PublishMode,
        ack_policy: AckPolicy,
    ) -> Self {
        Self {
            actor,
            request_event,
            decision,
            target_relays,
            publish_mode,
            ack_policy,
            idempotency_key: None,
            created_at: None,
        }
    }

    pub fn try_with_target_relays<I, S>(
        mut self,
        target_relays: I,
        policy: SdkRelayUrlPolicy,
    ) -> Result<Self, RadrootsSdkError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.target_relays = RelayResolutionPolicy::try_explicit(target_relays, policy)?;
        Ok(self)
    }

    pub fn with_idempotency_key(mut self, idempotency_key: SdkIdempotencyKey) -> Self {
        self.idempotency_key = Some(idempotency_key.into());
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

#[cfg(any(feature = "signer-adapters", test))]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct TradeDecisionPlan {
    pub workflow: TradeWorkflowPlan,
    pub order_id: RadrootsOrderId,
    pub listing_addr: RadrootsListingAddress,
    pub buyer_pubkey: RadrootsPublicKey,
    pub seller_pubkey: RadrootsPublicKey,
    pub request_event_id: RadrootsEventId,
    pub expected_event_id: RadrootsEventId,
    pub frozen_draft: RadrootsFrozenEventDraft,
    pub created_at: RadrootsSdkTimestamp,
}

#[cfg(any(feature = "signer-adapters", test))]
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

#[cfg(any(feature = "signer-adapters", test))]
#[derive(Clone, Debug, serde::Serialize)]
#[non_exhaustive]
pub struct TradeRevisionProposalPrepareRequest {
    #[serde(serialize_with = "crate::actor_json::serialize_actor_context")]
    pub actor: RadrootsActorContext,
    pub root_event: RadrootsNostrEventPtr,
    pub previous_event: RadrootsNostrEventPtr,
    pub proposal: RadrootsOrderRevisionProposal,
    pub created_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(any(feature = "signer-adapters", test))]
#[cfg(test)]
impl TradeRevisionProposalPrepareRequest {
    pub fn new(
        actor: RadrootsActorContext,
        root_event: RadrootsNostrEventPtr,
        previous_event: RadrootsNostrEventPtr,
        proposal: RadrootsOrderRevisionProposal,
    ) -> Self {
        Self {
            actor,
            root_event,
            previous_event,
            proposal,
            created_at: None,
        }
    }

    pub fn with_created_at(mut self, created_at: RadrootsSdkTimestamp) -> Self {
        self.created_at = Some(created_at);
        self
    }
}

#[cfg(feature = "runtime")]
#[cfg(test)]
#[derive(Clone, Debug, serde::Serialize)]
#[non_exhaustive]
pub struct TradeRevisionProposalEnqueueRequest {
    #[serde(serialize_with = "crate::actor_json::serialize_actor_context")]
    pub actor: RadrootsActorContext,
    pub root_event: RadrootsNostrEventPtr,
    pub previous_event: RadrootsNostrEventPtr,
    pub proposal: RadrootsOrderRevisionProposal,
    pub target_relays: RelayResolutionPolicy,
    pub publish_mode: PublishMode,
    pub ack_policy: AckPolicy,
    pub idempotency_key: Option<SdkIdempotencyKey>,
    pub created_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(feature = "runtime")]
#[cfg(test)]
impl TradeRevisionProposalEnqueueRequest {
    pub fn new(
        actor: RadrootsActorContext,
        root_event: RadrootsNostrEventPtr,
        previous_event: RadrootsNostrEventPtr,
        proposal: RadrootsOrderRevisionProposal,
        target_relays: RelayResolutionPolicy,
        publish_mode: PublishMode,
        ack_policy: AckPolicy,
    ) -> Self {
        Self {
            actor,
            root_event,
            previous_event,
            proposal,
            target_relays,
            publish_mode,
            ack_policy,
            idempotency_key: None,
            created_at: None,
        }
    }

    pub fn try_with_target_relays<I, S>(
        mut self,
        target_relays: I,
        policy: SdkRelayUrlPolicy,
    ) -> Result<Self, RadrootsSdkError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.target_relays = RelayResolutionPolicy::try_explicit(target_relays, policy)?;
        Ok(self)
    }

    pub fn with_idempotency_key(mut self, idempotency_key: SdkIdempotencyKey) -> Self {
        self.idempotency_key = Some(idempotency_key.into());
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

#[cfg(any(feature = "signer-adapters", test))]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct TradeRevisionProposalPlan {
    pub workflow: TradeWorkflowPlan,
    pub order_id: RadrootsOrderId,
    pub listing_addr: RadrootsListingAddress,
    pub buyer_pubkey: RadrootsPublicKey,
    pub seller_pubkey: RadrootsPublicKey,
    pub root_event_id: RadrootsEventId,
    pub previous_event_id: RadrootsEventId,
    pub expected_event_id: RadrootsEventId,
    pub frozen_draft: RadrootsFrozenEventDraft,
    pub created_at: RadrootsSdkTimestamp,
}

#[cfg(any(feature = "signer-adapters", test))]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct TradeRevisionProposalReceipt {
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

#[cfg(any(feature = "signer-adapters", test))]
#[derive(Clone, Debug, serde::Serialize)]
#[non_exhaustive]
pub struct TradeRevisionDecisionPrepareRequest {
    #[serde(serialize_with = "crate::actor_json::serialize_actor_context")]
    pub actor: RadrootsActorContext,
    pub root_event: RadrootsNostrEventPtr,
    pub previous_event: RadrootsNostrEventPtr,
    pub decision: RadrootsOrderRevisionDecision,
    pub created_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(any(feature = "signer-adapters", test))]
#[cfg(test)]
impl TradeRevisionDecisionPrepareRequest {
    pub fn new(
        actor: RadrootsActorContext,
        root_event: RadrootsNostrEventPtr,
        previous_event: RadrootsNostrEventPtr,
        decision: RadrootsOrderRevisionDecision,
    ) -> Self {
        Self {
            actor,
            root_event,
            previous_event,
            decision,
            created_at: None,
        }
    }

    pub fn with_created_at(mut self, created_at: RadrootsSdkTimestamp) -> Self {
        self.created_at = Some(created_at);
        self
    }
}

#[cfg(feature = "runtime")]
#[cfg(test)]
#[derive(Clone, Debug, serde::Serialize)]
#[non_exhaustive]
pub struct TradeRevisionDecisionEnqueueRequest {
    #[serde(serialize_with = "crate::actor_json::serialize_actor_context")]
    pub actor: RadrootsActorContext,
    pub root_event: RadrootsNostrEventPtr,
    pub previous_event: RadrootsNostrEventPtr,
    pub decision: RadrootsOrderRevisionDecision,
    pub target_relays: RelayResolutionPolicy,
    pub publish_mode: PublishMode,
    pub ack_policy: AckPolicy,
    pub idempotency_key: Option<SdkIdempotencyKey>,
    pub created_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(feature = "runtime")]
#[cfg(test)]
impl TradeRevisionDecisionEnqueueRequest {
    pub fn new(
        actor: RadrootsActorContext,
        root_event: RadrootsNostrEventPtr,
        previous_event: RadrootsNostrEventPtr,
        decision: RadrootsOrderRevisionDecision,
        target_relays: RelayResolutionPolicy,
        publish_mode: PublishMode,
        ack_policy: AckPolicy,
    ) -> Self {
        Self {
            actor,
            root_event,
            previous_event,
            decision,
            target_relays,
            publish_mode,
            ack_policy,
            idempotency_key: None,
            created_at: None,
        }
    }

    pub fn try_with_target_relays<I, S>(
        mut self,
        target_relays: I,
        policy: SdkRelayUrlPolicy,
    ) -> Result<Self, RadrootsSdkError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.target_relays = RelayResolutionPolicy::try_explicit(target_relays, policy)?;
        Ok(self)
    }

    pub fn with_idempotency_key(mut self, idempotency_key: SdkIdempotencyKey) -> Self {
        self.idempotency_key = Some(idempotency_key.into());
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

#[cfg(any(feature = "signer-adapters", test))]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct TradeRevisionDecisionPlan {
    pub workflow: TradeWorkflowPlan,
    pub order_id: RadrootsOrderId,
    pub listing_addr: RadrootsListingAddress,
    pub buyer_pubkey: RadrootsPublicKey,
    pub seller_pubkey: RadrootsPublicKey,
    pub root_event_id: RadrootsEventId,
    pub previous_event_id: RadrootsEventId,
    pub expected_event_id: RadrootsEventId,
    pub frozen_draft: RadrootsFrozenEventDraft,
    pub created_at: RadrootsSdkTimestamp,
}

#[cfg(any(feature = "signer-adapters", test))]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct TradeRevisionDecisionReceipt {
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

#[cfg(any(feature = "signer-adapters", test))]
#[derive(Clone, Debug, serde::Serialize)]
#[non_exhaustive]
pub struct TradeCancellationPrepareRequest {
    #[serde(serialize_with = "crate::actor_json::serialize_actor_context")]
    pub actor: RadrootsActorContext,
    pub root_event: RadrootsNostrEventPtr,
    pub previous_event: RadrootsNostrEventPtr,
    pub cancellation: RadrootsOrderCancellation,
    pub created_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(any(feature = "signer-adapters", test))]
#[cfg(test)]
impl TradeCancellationPrepareRequest {
    pub fn new(
        actor: RadrootsActorContext,
        root_event: RadrootsNostrEventPtr,
        previous_event: RadrootsNostrEventPtr,
        cancellation: RadrootsOrderCancellation,
    ) -> Self {
        Self {
            actor,
            root_event,
            previous_event,
            cancellation,
            created_at: None,
        }
    }

    pub fn with_created_at(mut self, created_at: RadrootsSdkTimestamp) -> Self {
        self.created_at = Some(created_at);
        self
    }
}

#[cfg(feature = "runtime")]
#[cfg(test)]
#[derive(Clone, Debug, serde::Serialize)]
#[non_exhaustive]
pub struct TradeCancellationEnqueueRequest {
    #[serde(serialize_with = "crate::actor_json::serialize_actor_context")]
    pub actor: RadrootsActorContext,
    pub root_event: RadrootsNostrEventPtr,
    pub previous_event: RadrootsNostrEventPtr,
    pub cancellation: RadrootsOrderCancellation,
    pub target_relays: RelayResolutionPolicy,
    pub publish_mode: PublishMode,
    pub ack_policy: AckPolicy,
    pub idempotency_key: Option<SdkIdempotencyKey>,
    pub created_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(feature = "runtime")]
#[cfg(test)]
impl TradeCancellationEnqueueRequest {
    pub fn new(
        actor: RadrootsActorContext,
        root_event: RadrootsNostrEventPtr,
        previous_event: RadrootsNostrEventPtr,
        cancellation: RadrootsOrderCancellation,
        target_relays: RelayResolutionPolicy,
        publish_mode: PublishMode,
        ack_policy: AckPolicy,
    ) -> Self {
        Self {
            actor,
            root_event,
            previous_event,
            cancellation,
            target_relays,
            publish_mode,
            ack_policy,
            idempotency_key: None,
            created_at: None,
        }
    }

    pub fn try_with_target_relays<I, S>(
        mut self,
        target_relays: I,
        policy: SdkRelayUrlPolicy,
    ) -> Result<Self, RadrootsSdkError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.target_relays = RelayResolutionPolicy::try_explicit(target_relays, policy)?;
        Ok(self)
    }

    pub fn with_idempotency_key(mut self, idempotency_key: SdkIdempotencyKey) -> Self {
        self.idempotency_key = Some(idempotency_key.into());
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

#[cfg(any(feature = "signer-adapters", test))]
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
    pub frozen_draft: RadrootsFrozenEventDraft,
    pub created_at: RadrootsSdkTimestamp,
}

#[cfg(any(feature = "signer-adapters", test))]
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

#[cfg(any(feature = "signer-adapters", test))]
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

#[cfg(any(feature = "signer-adapters", test))]
#[derive(Clone, Debug, serde::Serialize)]
#[non_exhaustive]
pub struct TradeProposeRequest {
    #[serde(serialize_with = "crate::actor_json::serialize_actor_context")]
    pub actor: RadrootsActorContext,
    pub listing_event: RadrootsNostrEventPtr,
    pub order: RadrootsOrderRequest,
    pub target_relays: RelayResolutionPolicy,
    pub publish_mode: PublishMode,
    pub ack_policy: AckPolicy,
    pub privacy_confirmation: PrivacyPreflightConfirmation,
    pub idempotency_key: Option<SdkIdempotencyKey>,
    pub created_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(any(feature = "signer-adapters", test))]
impl TradeProposeRequest {
    pub fn new(
        actor: RadrootsActorContext,
        listing_event: RadrootsNostrEventPtr,
        order: RadrootsOrderRequest,
        target_relays: RelayResolutionPolicy,
        publish_mode: PublishMode,
        ack_policy: AckPolicy,
    ) -> Self {
        Self {
            actor,
            listing_event,
            order,
            target_relays,
            publish_mode,
            ack_policy,
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

#[cfg(any(feature = "signer-adapters", test))]
#[derive(Clone, Debug, serde::Serialize)]
#[non_exhaustive]
pub struct TradeAcceptRequest {
    #[serde(serialize_with = "crate::actor_json::serialize_actor_context")]
    pub actor: RadrootsActorContext,
    pub locator: RadrootsTradeLocator,
    pub inventory_commitments: Vec<RadrootsOrderInventoryCommitment>,
    pub target_relays: RelayResolutionPolicy,
    pub publish_mode: PublishMode,
    pub ack_policy: AckPolicy,
    pub privacy_confirmation: PrivacyPreflightConfirmation,
    pub idempotency_key: Option<SdkIdempotencyKey>,
    pub created_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(any(feature = "signer-adapters", test))]
impl TradeAcceptRequest {
    pub fn new(
        actor: RadrootsActorContext,
        locator: RadrootsTradeLocator,
        inventory_commitments: Vec<RadrootsOrderInventoryCommitment>,
        target_relays: RelayResolutionPolicy,
        publish_mode: PublishMode,
        ack_policy: AckPolicy,
    ) -> Self {
        Self {
            actor,
            locator,
            inventory_commitments,
            target_relays,
            publish_mode,
            ack_policy,
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

#[cfg(any(feature = "signer-adapters", test))]
#[derive(Clone, Debug, serde::Serialize)]
#[non_exhaustive]
pub struct TradeDeclineRequest {
    #[serde(serialize_with = "crate::actor_json::serialize_actor_context")]
    pub actor: RadrootsActorContext,
    pub locator: RadrootsTradeLocator,
    pub reason: String,
    pub target_relays: RelayResolutionPolicy,
    pub publish_mode: PublishMode,
    pub ack_policy: AckPolicy,
    pub privacy_confirmation: PrivacyPreflightConfirmation,
    pub idempotency_key: Option<SdkIdempotencyKey>,
    pub created_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(any(feature = "signer-adapters", test))]
impl TradeDeclineRequest {
    pub fn new(
        actor: RadrootsActorContext,
        locator: RadrootsTradeLocator,
        reason: impl Into<String>,
        target_relays: RelayResolutionPolicy,
        publish_mode: PublishMode,
        ack_policy: AckPolicy,
    ) -> Self {
        Self {
            actor,
            locator,
            reason: reason.into(),
            target_relays,
            publish_mode,
            ack_policy,
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

#[cfg(any(feature = "signer-adapters", test))]
#[derive(Clone, Debug, serde::Serialize)]
#[non_exhaustive]
pub struct TradeCancelRequest {
    #[serde(serialize_with = "crate::actor_json::serialize_actor_context")]
    pub actor: RadrootsActorContext,
    pub locator: RadrootsTradeLocator,
    pub reason: String,
    pub target_relays: RelayResolutionPolicy,
    pub publish_mode: PublishMode,
    pub ack_policy: AckPolicy,
    pub privacy_confirmation: PrivacyPreflightConfirmation,
    pub idempotency_key: Option<SdkIdempotencyKey>,
    pub created_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(any(feature = "signer-adapters", test))]
impl TradeCancelRequest {
    pub fn new(
        actor: RadrootsActorContext,
        locator: RadrootsTradeLocator,
        reason: impl Into<String>,
        target_relays: RelayResolutionPolicy,
        publish_mode: PublishMode,
        ack_policy: AckPolicy,
    ) -> Self {
        Self {
            actor,
            locator,
            reason: reason.into(),
            target_relays,
            publish_mode,
            ack_policy,
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

#[cfg(any(feature = "signer-adapters", test))]
#[derive(Clone, Debug, serde::Serialize)]
#[non_exhaustive]
pub struct TradeRevisionProposalRequest {
    #[serde(serialize_with = "crate::actor_json::serialize_actor_context")]
    pub actor: RadrootsActorContext,
    pub locator: RadrootsTradeLocator,
    pub revision_id: RadrootsOrderRevisionId,
    pub items: Vec<RadrootsOrderItem>,
    pub economics: RadrootsOrderEconomics,
    pub reason: String,
    pub target_relays: RelayResolutionPolicy,
    pub publish_mode: PublishMode,
    pub ack_policy: AckPolicy,
    pub privacy_confirmation: PrivacyPreflightConfirmation,
    pub idempotency_key: Option<SdkIdempotencyKey>,
    pub created_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(any(feature = "signer-adapters", test))]
impl TradeRevisionProposalRequest {
    pub fn new(
        actor: RadrootsActorContext,
        locator: RadrootsTradeLocator,
        revision_id: RadrootsOrderRevisionId,
        items: Vec<RadrootsOrderItem>,
        economics: RadrootsOrderEconomics,
        reason: impl Into<String>,
        target_relays: RelayResolutionPolicy,
        publish_mode: PublishMode,
        ack_policy: AckPolicy,
    ) -> Self {
        Self {
            actor,
            locator,
            revision_id,
            items,
            economics,
            reason: reason.into(),
            target_relays,
            publish_mode,
            ack_policy,
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

#[cfg(any(feature = "signer-adapters", test))]
#[derive(Clone, Debug, serde::Serialize)]
#[non_exhaustive]
pub struct TradeRevisionDecisionRequest {
    #[serde(serialize_with = "crate::actor_json::serialize_actor_context")]
    pub actor: RadrootsActorContext,
    pub locator: RadrootsTradeLocator,
    pub revision_id: RadrootsOrderRevisionId,
    pub decision: RadrootsOrderRevisionOutcome,
    pub target_relays: RelayResolutionPolicy,
    pub publish_mode: PublishMode,
    pub ack_policy: AckPolicy,
    pub privacy_confirmation: PrivacyPreflightConfirmation,
    pub idempotency_key: Option<SdkIdempotencyKey>,
    pub created_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(any(feature = "signer-adapters", test))]
impl TradeRevisionDecisionRequest {
    pub fn new(
        actor: RadrootsActorContext,
        locator: RadrootsTradeLocator,
        revision_id: RadrootsOrderRevisionId,
        decision: RadrootsOrderRevisionOutcome,
        target_relays: RelayResolutionPolicy,
        publish_mode: PublishMode,
        ack_policy: AckPolicy,
    ) -> Self {
        Self {
            actor,
            locator,
            revision_id,
            decision,
            target_relays,
            publish_mode,
            ack_policy,
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
}

#[cfg(feature = "runtime")]
impl TradeResyncRequest {
    pub fn new(locator: RadrootsTradeLocator) -> Self {
        Self {
            locator,
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
pub struct TradeResyncReceipt {
    pub status: TradeStatusReceipt,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[non_exhaustive]
pub struct TradeStatusRequest {
    pub locator: RadrootsTradeLocator,
    pub limit: u32,
}

#[cfg(feature = "runtime")]
impl TradeStatusRequest {
    pub fn new(locator: RadrootsTradeLocator) -> Self {
        Self {
            locator,
            limit: TRADE_STATUS_DEFAULT_LIMIT,
        }
    }

    pub fn parse(order_id: &str) -> Result<Self, RadrootsSdkError> {
        RadrootsOrderId::parse(order_id)
            .map(RadrootsTradeLocator::from_order_id)
            .map(Self::new)
            .map_err(|error| RadrootsSdkError::invalid_trade_id(order_id, error.to_string()))
    }

    pub fn with_limit(mut self, limit: u32) -> Self {
        self.limit = limit;
        self
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
    pub eligibility: TradeStatusEligibility,
    pub next_action: TradeStatusNextActionKind,
    pub event_ids: Vec<RadrootsEventId>,
    pub request_event_id: Option<RadrootsEventId>,
    pub decision_event_id: Option<RadrootsEventId>,
    pub agreement_event_id: Option<RadrootsEventId>,
    pub rhi_receipt_event_id: Option<RadrootsEventId>,
    pub pending_revision_event_id: Option<RadrootsEventId>,
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
    pub has_pending_revision: bool,
    pub has_cancellation: bool,
    pub has_issues: bool,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct TradeStatusEligibility {
    pub can_decide: bool,
    pub can_propose_revision: bool,
    pub can_decide_revision: bool,
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
    DecideRevision,
    AwaitRhiValidation,
    Terminal,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SdkTradeStatusSource {
    LocalEventStore,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum TradeStatusKind {
    Missing,
    Ambiguous,
    Requested,
    RevisionProposed,
    AgreedPendingRhi,
    Committed,
    Declined,
    Cancelled,
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
    RevisionProposalPayloadInvalid,
    RevisionProposalOrderIdMismatch,
    RevisionProposalAuthorMismatch,
    RevisionProposalCounterpartyMismatch,
    RevisionProposalBuyerMismatch,
    RevisionProposalSellerMismatch,
    RevisionProposalListingAddressInvalid,
    RevisionProposalListingMismatch,
    RevisionProposalRootMismatch,
    RevisionProposalPreviousMismatch,
    RevisionDecisionWithoutProposal,
    RevisionDecisionPayloadInvalid,
    RevisionDecisionOrderIdMismatch,
    RevisionDecisionAuthorMismatch,
    RevisionDecisionCounterpartyMismatch,
    RevisionDecisionBuyerMismatch,
    RevisionDecisionSellerMismatch,
    RevisionDecisionListingAddressInvalid,
    RevisionDecisionListingMismatch,
    RevisionDecisionRootMismatch,
    RevisionDecisionPreviousMismatch,
    RevisionDecisionRevisionIdMismatch,
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
        let evidence = parse_order_evidence(&request.event)?;
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
        let evidence = parse_order_request_evidence(&request.event)?;
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

    #[cfg(any(feature = "signer-adapters", test))]
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
    #[cfg(test)]
    pub(crate) async fn enqueue_submit(
        &self,
        request: TradeSubmitEnqueueRequest,
    ) -> Result<TradeSubmitReceipt, RadrootsSdkError> {
        let TradeSubmitEnqueueRequest {
            actor,
            listing_event,
            order,
            target_relays,
            publish_mode,
            ack_policy,
            idempotency_key,
            created_at,
        } = request;
        validate_trade_enqueue_policy(publish_mode, ack_policy)?;
        let prepare_request = TradeSubmitPrepareRequest {
            actor: actor.clone(),
            listing_event,
            order,
            created_at,
        };
        let plan = self.prepare_submit(prepare_request)?;
        self.enqueue_prepared_submit(
            &actor,
            plan,
            target_relays,
            publish_mode,
            ack_policy,
            idempotency_key,
        )
        .await
    }

    #[cfg(test)]
    pub(crate) async fn enqueue_submit_with_explicit_signer(
        &self,
        request: TradeSubmitEnqueueRequest,
        signer: &dyn RadrootsEventSigner,
    ) -> Result<TradeSubmitReceipt, RadrootsSdkError> {
        let TradeSubmitEnqueueRequest {
            actor,
            listing_event,
            order,
            target_relays,
            publish_mode,
            ack_policy,
            idempotency_key,
            created_at,
        } = request;
        validate_trade_enqueue_policy(publish_mode, ack_policy)?;
        let prepare_request = TradeSubmitPrepareRequest {
            actor: actor.clone(),
            listing_event,
            order,
            created_at,
        };
        let plan = self.prepare_submit(prepare_request)?;
        self.enqueue_prepared_submit_with_explicit_signer(
            &actor,
            plan,
            target_relays,
            publish_mode,
            ack_policy,
            idempotency_key,
            signer,
        )
        .await
    }

    #[cfg(feature = "signer-adapters")]
    pub(crate) async fn enqueue_prepared_submit(
        &self,
        actor: &RadrootsActorContext,
        plan: TradeSubmitPlan,
        target_relays: RelayResolutionPolicy,
        publish_mode: PublishMode,
        ack_policy: AckPolicy,
        idempotency_key: Option<SdkIdempotencyKey>,
    ) -> Result<TradeSubmitReceipt, RadrootsSdkError> {
        validate_trade_enqueue_policy(publish_mode, ack_policy)?;
        let enqueue = enqueue_configured_signed_workflow(
            self.sdk,
            SdkWorkflowEnqueueRequest {
                operation_kind: TradeWorkflowKind::Submit.operation_kind(),
                actor,
                frozen_draft: &plan.frozen_draft,
                target_relays: target_relays.workflow_target_policy(),
                idempotency_key,
            },
        )
        .await?;
        Ok(order_submit_receipt(plan, enqueue))
    }

    #[cfg(test)]
    pub(crate) async fn enqueue_prepared_submit_with_explicit_signer(
        &self,
        actor: &RadrootsActorContext,
        plan: TradeSubmitPlan,
        target_relays: RelayResolutionPolicy,
        publish_mode: PublishMode,
        ack_policy: AckPolicy,
        idempotency_key: Option<SdkIdempotencyKey>,
        signer: &dyn RadrootsEventSigner,
    ) -> Result<TradeSubmitReceipt, RadrootsSdkError> {
        validate_trade_enqueue_policy(publish_mode, ack_policy)?;
        let enqueue = enqueue_signed_workflow(
            self.sdk,
            SdkWorkflowEnqueueRequest {
                operation_kind: TradeWorkflowKind::Submit.operation_kind(),
                actor,
                frozen_draft: &plan.frozen_draft,
                target_relays: target_relays.workflow_target_policy(),
                idempotency_key,
            },
            signer,
        )
        .await?;
        Ok(order_submit_receipt(plan, enqueue))
    }

    #[cfg(any(feature = "signer-adapters", test))]
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
    #[cfg(test)]
    pub(crate) async fn enqueue_decision(
        &self,
        request: TradeDecisionEnqueueRequest,
    ) -> Result<TradeDecisionReceipt, RadrootsSdkError> {
        let TradeDecisionEnqueueRequest {
            actor,
            request_event,
            decision,
            target_relays,
            publish_mode,
            ack_policy,
            idempotency_key,
            created_at,
        } = request;
        validate_trade_enqueue_policy(publish_mode, ack_policy)?;
        let prepare_request = TradeDecisionPrepareRequest {
            actor: actor.clone(),
            request_event,
            decision,
            created_at,
        };
        let plan = self.prepare_decision(prepare_request)?;
        self.enqueue_prepared_decision(
            &actor,
            plan,
            target_relays,
            publish_mode,
            ack_policy,
            idempotency_key,
        )
        .await
    }

    #[cfg(test)]
    pub(crate) async fn enqueue_decision_with_explicit_signer(
        &self,
        request: TradeDecisionEnqueueRequest,
        signer: &dyn RadrootsEventSigner,
    ) -> Result<TradeDecisionReceipt, RadrootsSdkError> {
        let TradeDecisionEnqueueRequest {
            actor,
            request_event,
            decision,
            target_relays,
            publish_mode,
            ack_policy,
            idempotency_key,
            created_at,
        } = request;
        validate_trade_enqueue_policy(publish_mode, ack_policy)?;
        let prepare_request = TradeDecisionPrepareRequest {
            actor: actor.clone(),
            request_event,
            decision,
            created_at,
        };
        let plan = self.prepare_decision(prepare_request)?;
        self.enqueue_prepared_decision_with_explicit_signer(
            &actor,
            plan,
            target_relays,
            publish_mode,
            ack_policy,
            idempotency_key,
            signer,
        )
        .await
    }

    #[cfg(feature = "signer-adapters")]
    pub(crate) async fn enqueue_prepared_decision(
        &self,
        actor: &RadrootsActorContext,
        plan: TradeDecisionPlan,
        target_relays: RelayResolutionPolicy,
        publish_mode: PublishMode,
        ack_policy: AckPolicy,
        idempotency_key: Option<SdkIdempotencyKey>,
    ) -> Result<TradeDecisionReceipt, RadrootsSdkError> {
        validate_trade_enqueue_policy(publish_mode, ack_policy)?;
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
                target_relays: target_relays.workflow_target_policy(),
                idempotency_key,
            },
        )
        .await?;
        Ok(order_decision_receipt(plan, enqueue))
    }

    #[cfg(test)]
    pub(crate) async fn enqueue_prepared_decision_with_explicit_signer(
        &self,
        actor: &RadrootsActorContext,
        plan: TradeDecisionPlan,
        target_relays: RelayResolutionPolicy,
        publish_mode: PublishMode,
        ack_policy: AckPolicy,
        idempotency_key: Option<SdkIdempotencyKey>,
        signer: &dyn RadrootsEventSigner,
    ) -> Result<TradeDecisionReceipt, RadrootsSdkError> {
        validate_trade_enqueue_policy(publish_mode, ack_policy)?;
        if !self
            .prepared_order_event_exists(&plan.expected_event_id)
            .await?
        {
            self.require_decision_preflight(&plan).await?;
        }
        let enqueue = enqueue_signed_workflow(
            self.sdk,
            SdkWorkflowEnqueueRequest {
                operation_kind: TradeWorkflowKind::Decision.operation_kind(),
                actor,
                frozen_draft: &plan.frozen_draft,
                target_relays: target_relays.workflow_target_policy(),
                idempotency_key,
            },
            signer,
        )
        .await?;
        Ok(order_decision_receipt(plan, enqueue))
    }

    #[cfg(any(feature = "signer-adapters", test))]
    pub(crate) fn prepare_revision_proposal(
        &self,
        request: TradeRevisionProposalPrepareRequest,
    ) -> Result<TradeRevisionProposalPlan, RadrootsSdkError> {
        let created_at = self.resolved_created_at(request.created_at)?;
        order_revision_proposal_plan(
            &request.actor,
            request.root_event,
            request.previous_event,
            request.proposal,
            created_at,
        )
    }

    #[cfg(feature = "signer-adapters")]
    #[cfg(test)]
    pub(crate) async fn enqueue_revision_proposal(
        &self,
        request: TradeRevisionProposalEnqueueRequest,
    ) -> Result<TradeRevisionProposalReceipt, RadrootsSdkError> {
        let TradeRevisionProposalEnqueueRequest {
            actor,
            root_event,
            previous_event,
            proposal,
            target_relays,
            publish_mode,
            ack_policy,
            idempotency_key,
            created_at,
        } = request;
        validate_trade_enqueue_policy(publish_mode, ack_policy)?;
        let prepare_request = TradeRevisionProposalPrepareRequest {
            actor: actor.clone(),
            root_event,
            previous_event,
            proposal,
            created_at,
        };
        let plan = self.prepare_revision_proposal(prepare_request)?;
        self.enqueue_prepared_revision_proposal(
            &actor,
            plan,
            target_relays,
            publish_mode,
            ack_policy,
            idempotency_key,
        )
        .await
    }

    #[cfg(test)]
    pub(crate) async fn enqueue_revision_proposal_with_explicit_signer(
        &self,
        request: TradeRevisionProposalEnqueueRequest,
        signer: &dyn RadrootsEventSigner,
    ) -> Result<TradeRevisionProposalReceipt, RadrootsSdkError> {
        let TradeRevisionProposalEnqueueRequest {
            actor,
            root_event,
            previous_event,
            proposal,
            target_relays,
            publish_mode,
            ack_policy,
            idempotency_key,
            created_at,
        } = request;
        validate_trade_enqueue_policy(publish_mode, ack_policy)?;
        let prepare_request = TradeRevisionProposalPrepareRequest {
            actor: actor.clone(),
            root_event,
            previous_event,
            proposal,
            created_at,
        };
        let plan = self.prepare_revision_proposal(prepare_request)?;
        self.enqueue_prepared_revision_proposal_with_explicit_signer(
            &actor,
            plan,
            target_relays,
            publish_mode,
            ack_policy,
            idempotency_key,
            signer,
        )
        .await
    }

    #[cfg(feature = "signer-adapters")]
    pub(crate) async fn enqueue_prepared_revision_proposal(
        &self,
        actor: &RadrootsActorContext,
        plan: TradeRevisionProposalPlan,
        target_relays: RelayResolutionPolicy,
        publish_mode: PublishMode,
        ack_policy: AckPolicy,
        idempotency_key: Option<SdkIdempotencyKey>,
    ) -> Result<TradeRevisionProposalReceipt, RadrootsSdkError> {
        validate_trade_enqueue_policy(publish_mode, ack_policy)?;
        if !self
            .prepared_order_event_exists(&plan.expected_event_id)
            .await?
        {
            self.require_revision_proposal_preflight(&plan).await?;
        }
        let enqueue = enqueue_configured_signed_workflow(
            self.sdk,
            SdkWorkflowEnqueueRequest {
                operation_kind: TradeWorkflowKind::RevisionProposal.operation_kind(),
                actor,
                frozen_draft: &plan.frozen_draft,
                target_relays: target_relays.workflow_target_policy(),
                idempotency_key,
            },
        )
        .await?;
        Ok(order_revision_proposal_receipt(plan, enqueue))
    }

    #[cfg(test)]
    pub(crate) async fn enqueue_prepared_revision_proposal_with_explicit_signer(
        &self,
        actor: &RadrootsActorContext,
        plan: TradeRevisionProposalPlan,
        target_relays: RelayResolutionPolicy,
        publish_mode: PublishMode,
        ack_policy: AckPolicy,
        idempotency_key: Option<SdkIdempotencyKey>,
        signer: &dyn RadrootsEventSigner,
    ) -> Result<TradeRevisionProposalReceipt, RadrootsSdkError> {
        validate_trade_enqueue_policy(publish_mode, ack_policy)?;
        if !self
            .prepared_order_event_exists(&plan.expected_event_id)
            .await?
        {
            self.require_revision_proposal_preflight(&plan).await?;
        }
        let enqueue = enqueue_signed_workflow(
            self.sdk,
            SdkWorkflowEnqueueRequest {
                operation_kind: TradeWorkflowKind::RevisionProposal.operation_kind(),
                actor,
                frozen_draft: &plan.frozen_draft,
                target_relays: target_relays.workflow_target_policy(),
                idempotency_key,
            },
            signer,
        )
        .await?;
        Ok(order_revision_proposal_receipt(plan, enqueue))
    }

    #[cfg(any(feature = "signer-adapters", test))]
    pub(crate) fn prepare_revision_decision(
        &self,
        request: TradeRevisionDecisionPrepareRequest,
    ) -> Result<TradeRevisionDecisionPlan, RadrootsSdkError> {
        let created_at = self.resolved_created_at(request.created_at)?;
        order_revision_decision_plan(
            &request.actor,
            request.root_event,
            request.previous_event,
            request.decision,
            created_at,
        )
    }

    #[cfg(feature = "signer-adapters")]
    #[cfg(test)]
    pub(crate) async fn enqueue_revision_decision(
        &self,
        request: TradeRevisionDecisionEnqueueRequest,
    ) -> Result<TradeRevisionDecisionReceipt, RadrootsSdkError> {
        let TradeRevisionDecisionEnqueueRequest {
            actor,
            root_event,
            previous_event,
            decision,
            target_relays,
            publish_mode,
            ack_policy,
            idempotency_key,
            created_at,
        } = request;
        validate_trade_enqueue_policy(publish_mode, ack_policy)?;
        let prepare_request = TradeRevisionDecisionPrepareRequest {
            actor: actor.clone(),
            root_event,
            previous_event,
            decision,
            created_at,
        };
        let plan = self.prepare_revision_decision(prepare_request)?;
        self.enqueue_prepared_revision_decision(
            &actor,
            plan,
            target_relays,
            publish_mode,
            ack_policy,
            idempotency_key,
        )
        .await
    }

    #[cfg(test)]
    pub(crate) async fn enqueue_revision_decision_with_explicit_signer(
        &self,
        request: TradeRevisionDecisionEnqueueRequest,
        signer: &dyn RadrootsEventSigner,
    ) -> Result<TradeRevisionDecisionReceipt, RadrootsSdkError> {
        let TradeRevisionDecisionEnqueueRequest {
            actor,
            root_event,
            previous_event,
            decision,
            target_relays,
            publish_mode,
            ack_policy,
            idempotency_key,
            created_at,
        } = request;
        validate_trade_enqueue_policy(publish_mode, ack_policy)?;
        let prepare_request = TradeRevisionDecisionPrepareRequest {
            actor: actor.clone(),
            root_event,
            previous_event,
            decision,
            created_at,
        };
        let plan = self.prepare_revision_decision(prepare_request)?;
        self.enqueue_prepared_revision_decision_with_explicit_signer(
            &actor,
            plan,
            target_relays,
            publish_mode,
            ack_policy,
            idempotency_key,
            signer,
        )
        .await
    }

    #[cfg(feature = "signer-adapters")]
    pub(crate) async fn enqueue_prepared_revision_decision(
        &self,
        actor: &RadrootsActorContext,
        plan: TradeRevisionDecisionPlan,
        target_relays: RelayResolutionPolicy,
        publish_mode: PublishMode,
        ack_policy: AckPolicy,
        idempotency_key: Option<SdkIdempotencyKey>,
    ) -> Result<TradeRevisionDecisionReceipt, RadrootsSdkError> {
        validate_trade_enqueue_policy(publish_mode, ack_policy)?;
        if !self
            .prepared_order_event_exists(&plan.expected_event_id)
            .await?
        {
            self.require_revision_decision_preflight(&plan).await?;
        }
        let enqueue = enqueue_configured_signed_workflow(
            self.sdk,
            SdkWorkflowEnqueueRequest {
                operation_kind: TradeWorkflowKind::RevisionDecision.operation_kind(),
                actor,
                frozen_draft: &plan.frozen_draft,
                target_relays: target_relays.workflow_target_policy(),
                idempotency_key,
            },
        )
        .await?;
        Ok(order_revision_decision_receipt(plan, enqueue))
    }

    #[cfg(test)]
    pub(crate) async fn enqueue_prepared_revision_decision_with_explicit_signer(
        &self,
        actor: &RadrootsActorContext,
        plan: TradeRevisionDecisionPlan,
        target_relays: RelayResolutionPolicy,
        publish_mode: PublishMode,
        ack_policy: AckPolicy,
        idempotency_key: Option<SdkIdempotencyKey>,
        signer: &dyn RadrootsEventSigner,
    ) -> Result<TradeRevisionDecisionReceipt, RadrootsSdkError> {
        validate_trade_enqueue_policy(publish_mode, ack_policy)?;
        if !self
            .prepared_order_event_exists(&plan.expected_event_id)
            .await?
        {
            self.require_revision_decision_preflight(&plan).await?;
        }
        let enqueue = enqueue_signed_workflow(
            self.sdk,
            SdkWorkflowEnqueueRequest {
                operation_kind: TradeWorkflowKind::RevisionDecision.operation_kind(),
                actor,
                frozen_draft: &plan.frozen_draft,
                target_relays: target_relays.workflow_target_policy(),
                idempotency_key,
            },
            signer,
        )
        .await?;
        Ok(order_revision_decision_receipt(plan, enqueue))
    }

    #[cfg(any(feature = "signer-adapters", test))]
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
    #[cfg(test)]
    pub(crate) async fn enqueue_cancellation(
        &self,
        request: TradeCancellationEnqueueRequest,
    ) -> Result<TradeCancellationReceipt, RadrootsSdkError> {
        let TradeCancellationEnqueueRequest {
            actor,
            root_event,
            previous_event,
            cancellation,
            target_relays,
            publish_mode,
            ack_policy,
            idempotency_key,
            created_at,
        } = request;
        validate_trade_enqueue_policy(publish_mode, ack_policy)?;
        let prepare_request = TradeCancellationPrepareRequest {
            actor: actor.clone(),
            root_event,
            previous_event,
            cancellation,
            created_at,
        };
        let plan = self.prepare_cancellation(prepare_request)?;
        self.enqueue_prepared_cancellation(
            &actor,
            plan,
            target_relays,
            publish_mode,
            ack_policy,
            idempotency_key,
        )
        .await
    }

    #[cfg(test)]
    pub(crate) async fn enqueue_cancellation_with_explicit_signer(
        &self,
        request: TradeCancellationEnqueueRequest,
        signer: &dyn RadrootsEventSigner,
    ) -> Result<TradeCancellationReceipt, RadrootsSdkError> {
        let TradeCancellationEnqueueRequest {
            actor,
            root_event,
            previous_event,
            cancellation,
            target_relays,
            publish_mode,
            ack_policy,
            idempotency_key,
            created_at,
        } = request;
        validate_trade_enqueue_policy(publish_mode, ack_policy)?;
        let prepare_request = TradeCancellationPrepareRequest {
            actor: actor.clone(),
            root_event,
            previous_event,
            cancellation,
            created_at,
        };
        let plan = self.prepare_cancellation(prepare_request)?;
        self.enqueue_prepared_cancellation_with_explicit_signer(
            &actor,
            plan,
            target_relays,
            publish_mode,
            ack_policy,
            idempotency_key,
            signer,
        )
        .await
    }

    #[cfg(feature = "signer-adapters")]
    pub(crate) async fn enqueue_prepared_cancellation(
        &self,
        actor: &RadrootsActorContext,
        plan: TradeCancellationPlan,
        target_relays: RelayResolutionPolicy,
        publish_mode: PublishMode,
        ack_policy: AckPolicy,
        idempotency_key: Option<SdkIdempotencyKey>,
    ) -> Result<TradeCancellationReceipt, RadrootsSdkError> {
        validate_trade_enqueue_policy(publish_mode, ack_policy)?;
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
                target_relays: target_relays.workflow_target_policy(),
                idempotency_key,
            },
        )
        .await?;
        Ok(order_cancellation_receipt(plan, enqueue))
    }

    #[cfg(test)]
    pub(crate) async fn enqueue_prepared_cancellation_with_explicit_signer(
        &self,
        actor: &RadrootsActorContext,
        plan: TradeCancellationPlan,
        target_relays: RelayResolutionPolicy,
        publish_mode: PublishMode,
        ack_policy: AckPolicy,
        idempotency_key: Option<SdkIdempotencyKey>,
        signer: &dyn RadrootsEventSigner,
    ) -> Result<TradeCancellationReceipt, RadrootsSdkError> {
        validate_trade_enqueue_policy(publish_mode, ack_policy)?;
        if !self
            .prepared_order_event_exists(&plan.expected_event_id)
            .await?
        {
            self.require_cancellation_preflight(&plan).await?;
        }
        let enqueue = enqueue_signed_workflow(
            self.sdk,
            SdkWorkflowEnqueueRequest {
                operation_kind: TradeWorkflowKind::Cancellation.operation_kind(),
                actor,
                frozen_draft: &plan.frozen_draft,
                target_relays: target_relays.workflow_target_policy(),
                idempotency_key,
            },
            signer,
        )
        .await?;
        Ok(order_cancellation_receipt(plan, enqueue))
    }

    pub async fn status(
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
        Ok(TradeStatusReceipt::from_locator_query_result(
            request.locator,
            query_result,
        ))
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

    #[cfg(any(feature = "signer-adapters", test))]
    async fn require_decision_preflight(
        &self,
        plan: &TradeDecisionPlan,
    ) -> Result<(), RadrootsSdkError> {
        let query_result = self.query_order_projection(&plan.order_id).await?;
        require_decision_request_evidence(plan, &query_result.projection)
    }

    #[cfg(any(feature = "signer-adapters", test))]
    async fn require_revision_proposal_preflight(
        &self,
        plan: &TradeRevisionProposalPlan,
    ) -> Result<(), RadrootsSdkError> {
        let query_result = self.query_order_projection(&plan.order_id).await?;
        require_revision_proposal_state(plan, &query_result.projection)
    }

    #[cfg(any(feature = "signer-adapters", test))]
    async fn require_revision_decision_preflight(
        &self,
        plan: &TradeRevisionDecisionPlan,
    ) -> Result<(), RadrootsSdkError> {
        let query_result = self.query_order_projection(&plan.order_id).await?;
        require_revision_decision_state(plan, &query_result.projection)
    }

    #[cfg(any(feature = "signer-adapters", test))]
    async fn require_cancellation_preflight(
        &self,
        plan: &TradeCancellationPlan,
    ) -> Result<(), RadrootsSdkError> {
        let query_result = self.query_order_projection(&plan.order_id).await?;
        require_cancellation_state(plan, &query_result.projection)
    }

    #[cfg(any(feature = "signer-adapters", test))]
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

    #[cfg(any(feature = "signer-adapters", test))]
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
impl<'sdk> TradeResyncClient<'sdk> {
    pub async fn resync(
        &self,
        request: TradeResyncRequest,
    ) -> Result<TradeResyncReceipt, RadrootsSdkError> {
        let status = trades_client(self.sdk)
            .status(TradeStatusRequest::new(request.locator).with_limit(request.limit))
            .await?;
        Ok(TradeResyncReceipt { status })
    }
}

#[cfg(all(feature = "runtime", feature = "signer-adapters"))]
impl<'sdk> TradeBuyerClient<'sdk> {
    pub async fn propose_trade(
        &self,
        request: TradeProposeRequest,
    ) -> Result<TradeMutationOutcome<TradeSubmitPlan, TradeSubmitReceipt>, RadrootsSdkError> {
        validate_trade_product_publish_policy(request.publish_mode, request.ack_policy)?;
        let TradeProposeRequest {
            actor,
            listing_event,
            order,
            target_relays,
            publish_mode,
            ack_policy,
            privacy_confirmation,
            idempotency_key,
            created_at,
        } = request;
        require_trade_product_privacy_preflight(
            "trade.propose",
            trade_order_request_privacy_fields(&order),
            &privacy_confirmation,
        )?;
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
                target_relays,
                publish_mode,
                ack_policy,
                idempotency_key,
            )
            .await?;
        trade_product_post_enqueue_outcome(
            self.sdk,
            publish_mode,
            ack_policy,
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
        validate_trade_product_publish_policy(request.publish_mode, request.ack_policy)?;
        let TradeCancelRequest {
            actor,
            locator,
            reason,
            target_relays,
            publish_mode,
            ack_policy,
            privacy_confirmation,
            idempotency_key,
            created_at,
        } = request;
        let context = trade_mutation_context(self.sdk, locator, "trade.cancel").await?;
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
                target_relays,
                publish_mode,
                ack_policy,
                idempotency_key,
            )
            .await?;
        trade_product_post_enqueue_outcome(
            self.sdk,
            publish_mode,
            ack_policy,
            receipt.outbox_event_id,
            receipt,
        )
        .await
    }

    pub async fn accept_revision(
        &self,
        mut request: TradeRevisionDecisionRequest,
    ) -> Result<
        TradeMutationOutcome<TradeRevisionDecisionPlan, TradeRevisionDecisionReceipt>,
        RadrootsSdkError,
    > {
        request.decision = RadrootsOrderRevisionOutcome::Accepted;
        self.decide_revision(request).await
    }

    pub async fn decline_revision(
        &self,
        request: TradeRevisionDecisionRequest,
    ) -> Result<
        TradeMutationOutcome<TradeRevisionDecisionPlan, TradeRevisionDecisionReceipt>,
        RadrootsSdkError,
    > {
        self.decide_revision(request).await
    }

    async fn decide_revision(
        &self,
        request: TradeRevisionDecisionRequest,
    ) -> Result<
        TradeMutationOutcome<TradeRevisionDecisionPlan, TradeRevisionDecisionReceipt>,
        RadrootsSdkError,
    > {
        validate_trade_product_publish_policy(request.publish_mode, request.ack_policy)?;
        let TradeRevisionDecisionRequest {
            actor,
            locator,
            revision_id,
            decision,
            target_relays,
            publish_mode,
            ack_policy,
            privacy_confirmation,
            idempotency_key,
            created_at,
        } = request;
        require_trade_product_privacy_preflight(
            "trade.revision_decision",
            trade_revision_decision_privacy_fields(&decision),
            &privacy_confirmation,
        )?;
        let context = trade_mutation_context(self.sdk, locator, "trade.revision_decision").await?;
        let previous_event_id = context.pending_revision_event_id.clone().ok_or_else(|| {
            RadrootsSdkError::InvalidRequest {
                message: "trade revision decision requires a pending revision".to_owned(),
            }
        })?;
        let decision = RadrootsOrderRevisionDecision {
            revision_id,
            order_id: context.order_id.clone(),
            listing_addr: context.listing_addr.clone(),
            buyer_pubkey: context.buyer_pubkey.clone(),
            seller_pubkey: context.seller_pubkey.clone(),
            root_event_id: context.root_event_id.clone(),
            prev_event_id: previous_event_id.clone(),
            decision,
        };
        let client = trades_client(self.sdk);
        let plan = client.prepare_revision_decision(TradeRevisionDecisionPrepareRequest {
            actor: actor.clone(),
            root_event: event_ptr(&context.root_event_id),
            previous_event: event_ptr(&previous_event_id),
            decision,
            created_at,
        })?;
        if publish_mode == PublishMode::DryRun {
            return Ok(TradeMutationOutcome::DryRun { plan });
        }
        let receipt = client
            .enqueue_prepared_revision_decision(
                &actor,
                plan,
                target_relays,
                publish_mode,
                ack_policy,
                idempotency_key,
            )
            .await?;
        trade_product_post_enqueue_outcome(
            self.sdk,
            publish_mode,
            ack_policy,
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
            let nostr_event = RadrootsNostrEvent {
                id: event.event_id,
                author: event.pubkey,
                created_at: event.created_at,
                kind: event.kind,
                tags,
                content: event.content,
                sig: event.sig,
            };
            if let Ok(RadrootsOrderEventRecord::Request(record)) =
                order_event_record_from_event(&nostr_event)
            {
                if record.payload.seller_pubkey == seller_pubkey {
                    locators.push(
                        RadrootsTradeLocator::from_order_id(record.payload.order_id)
                            .with_root_event_id(record.event_id)
                            .with_listing_addr(record.payload.listing_addr)
                            .with_buyer_pubkey(record.payload.buyer_pubkey)
                            .with_seller_pubkey(record.payload.seller_pubkey),
                    );
                }
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
        validate_trade_product_publish_policy(request.publish_mode, request.ack_policy)?;
        let TradeAcceptRequest {
            actor,
            locator,
            inventory_commitments,
            target_relays,
            publish_mode,
            ack_policy,
            privacy_confirmation,
            idempotency_key,
            created_at,
        } = request;
        let context = trade_mutation_context(self.sdk, locator, "trade.accept").await?;
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
                target_relays,
                publish_mode,
                ack_policy,
                idempotency_key,
            )
            .await?;
        trade_product_post_enqueue_outcome(
            self.sdk,
            publish_mode,
            ack_policy,
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
        validate_trade_product_publish_policy(request.publish_mode, request.ack_policy)?;
        let TradeDeclineRequest {
            actor,
            locator,
            reason,
            target_relays,
            publish_mode,
            ack_policy,
            privacy_confirmation,
            idempotency_key,
            created_at,
        } = request;
        let context = trade_mutation_context(self.sdk, locator, "trade.decline").await?;
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
                target_relays,
                publish_mode,
                ack_policy,
                idempotency_key,
            )
            .await?;
        trade_product_post_enqueue_outcome(
            self.sdk,
            publish_mode,
            ack_policy,
            receipt.outbox_event_id,
            receipt,
        )
        .await
    }

    #[cfg(feature = "signer-adapters")]
    pub async fn propose_revision(
        &self,
        request: TradeRevisionProposalRequest,
    ) -> Result<
        TradeMutationOutcome<TradeRevisionProposalPlan, TradeRevisionProposalReceipt>,
        RadrootsSdkError,
    > {
        validate_trade_product_publish_policy(request.publish_mode, request.ack_policy)?;
        let TradeRevisionProposalRequest {
            actor,
            locator,
            revision_id,
            items,
            economics,
            reason,
            target_relays,
            publish_mode,
            ack_policy,
            privacy_confirmation,
            idempotency_key,
            created_at,
        } = request;
        let context = trade_mutation_context(self.sdk, locator, "trade.propose_revision").await?;
        let proposal = RadrootsOrderRevisionProposal {
            revision_id,
            order_id: context.order_id.clone(),
            listing_addr: context.listing_addr.clone(),
            buyer_pubkey: context.buyer_pubkey.clone(),
            seller_pubkey: context.seller_pubkey.clone(),
            root_event_id: context.root_event_id.clone(),
            prev_event_id: context.previous_event_id.clone(),
            items,
            economics,
            reason,
        };
        require_trade_product_privacy_preflight(
            "trade.propose_revision",
            trade_revision_proposal_privacy_fields(&proposal),
            &privacy_confirmation,
        )?;
        let client = trades_client(self.sdk);
        let plan = client.prepare_revision_proposal(TradeRevisionProposalPrepareRequest {
            actor: actor.clone(),
            root_event: event_ptr(&context.root_event_id),
            previous_event: event_ptr(&context.previous_event_id),
            proposal,
            created_at,
        })?;
        if publish_mode == PublishMode::DryRun {
            return Ok(TradeMutationOutcome::DryRun { plan });
        }
        let receipt = client
            .enqueue_prepared_revision_proposal(
                &actor,
                plan,
                target_relays,
                publish_mode,
                ack_policy,
                idempotency_key,
            )
            .await?;
        trade_product_post_enqueue_outcome(
            self.sdk,
            publish_mode,
            ack_policy,
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
    pending_revision_event_id: Option<RadrootsEventId>,
}

#[cfg(feature = "signer-adapters")]
async fn trade_mutation_context(
    sdk: &crate::RadrootsClient,
    locator: RadrootsTradeLocator,
    operation: &'static str,
) -> Result<TradeProductMutationContext, RadrootsSdkError> {
    let status = trades_client(sdk)
        .status(TradeStatusRequest::new(locator.clone()))
        .await?;
    if status.status == TradeStatusKind::Ambiguous {
        return Err(RadrootsSdkError::TradeAmbiguous {
            operation: operation.to_owned(),
            locator,
            candidates: status
                .ambiguity_candidates
                .into_iter()
                .map(|candidate| candidate.locator)
                .collect(),
        });
    }
    if !status.found {
        return Err(RadrootsSdkError::InvalidRequest {
            message: format!("{operation} requires a locally projected trade"),
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
        pending_revision_event_id: status.pending_revision_event_id,
    })
}

#[cfg(feature = "signer-adapters")]
fn event_ptr(event_id: &RadrootsEventId) -> RadrootsNostrEventPtr {
    RadrootsNostrEventPtr {
        id: event_id.as_str().to_owned(),
        relays: None,
    }
}

#[cfg(feature = "signer-adapters")]
async fn trade_product_post_enqueue_outcome<Plan, Receipt>(
    sdk: &crate::RadrootsClient,
    publish_mode: PublishMode,
    ack_policy: AckPolicy,
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
                .push_outbox(push_request_for_ack_policy(ack_policy, outbox_event_id)?)
                .await?;
            Ok(TradeMutationOutcome::Published { receipt, publish })
        }
    }
}

#[cfg(feature = "signer-adapters")]
fn push_request_for_ack_policy(
    ack_policy: AckPolicy,
    outbox_event_id: i64,
) -> Result<PushOutboxRequest, RadrootsSdkError> {
    let request = PushOutboxRequest::new().with_outbox_event_id(outbox_event_id);
    match ack_policy {
        AckPolicy::NoWait => Err(RadrootsSdkError::InvalidRequest {
            message: "trade enqueue-and-publish requires a relay acknowledgement policy".to_owned(),
        }),
        AckPolicy::AtLeastOneRelay => Ok(request.with_accepted_quorum(1)),
        AckPolicy::AllRelays => Ok(request),
        AckPolicy::Quorum { required } => Ok(request.with_accepted_quorum(usize::from(required))),
    }
}

#[cfg(feature = "signer-adapters")]
fn validate_trade_product_publish_policy(
    publish_mode: PublishMode,
    ack_policy: AckPolicy,
) -> Result<(), RadrootsSdkError> {
    match publish_mode {
        PublishMode::DryRun | PublishMode::EnqueueOnly if ack_policy != AckPolicy::NoWait => {
            Err(RadrootsSdkError::InvalidRequest {
                message: "trade dry-run and enqueue-only modes require no-wait acknowledgement"
                    .to_owned(),
            })
        }
        PublishMode::EnqueueAndPublish if ack_policy == AckPolicy::NoWait => {
            Err(RadrootsSdkError::InvalidRequest {
                message: "trade enqueue-and-publish requires a relay acknowledgement policy"
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
fn trade_order_request_privacy_fields(
    order: &RadrootsOrderRequest,
) -> Vec<ProductSensitivityField> {
    if order.items.is_empty() && order.economics.items.is_empty() {
        Vec::new()
    } else {
        vec![ProductSensitivityField::ProtocolMinimizedInventoryFields]
    }
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
fn trade_revision_proposal_privacy_fields(
    proposal: &RadrootsOrderRevisionProposal,
) -> Vec<ProductSensitivityField> {
    let mut fields = trade_reason_privacy_fields(&proposal.reason);
    if !proposal.items.is_empty() || !proposal.economics.items.is_empty() {
        fields.push(ProductSensitivityField::ProtocolMinimizedInventoryFields);
    }
    fields
}

#[cfg(feature = "signer-adapters")]
fn trade_revision_decision_privacy_fields(
    decision: &RadrootsOrderRevisionOutcome,
) -> Vec<ProductSensitivityField> {
    match decision {
        RadrootsOrderRevisionOutcome::Accepted => Vec::new(),
        RadrootsOrderRevisionOutcome::Declined { reason } => trade_reason_privacy_fields(reason),
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

#[cfg(any(feature = "signer-adapters", test))]
fn validate_trade_enqueue_policy(
    publish_mode: PublishMode,
    ack_policy: AckPolicy,
) -> Result<(), RadrootsSdkError> {
    if publish_mode == PublishMode::DryRun {
        return Err(RadrootsSdkError::InvalidRequest {
            message: "trade dry-run publish mode must use a prepare request".to_owned(),
        });
    }
    if publish_mode == PublishMode::EnqueueOnly && ack_policy != AckPolicy::NoWait {
        return Err(RadrootsSdkError::InvalidRequest {
            message: "trade enqueue-only publish mode only supports no-wait acknowledgement"
                .to_owned(),
        });
    }
    if publish_mode == PublishMode::EnqueueAndPublish && ack_policy == AckPolicy::NoWait {
        return Err(RadrootsSdkError::InvalidRequest {
            message: "trade enqueue-and-publish requires a relay acknowledgement policy".to_owned(),
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
    event: &RadrootsNostrEvent,
) -> Result<ParsedOrderEvidence, RadrootsSdkError> {
    let event_id = RadrootsEventId::parse(event.id.as_str()).map_err(|error| {
        RadrootsSdkError::InvalidRequest {
            message: format!("order evidence event id is invalid: {error}"),
        }
    })?;
    let (order_id, listing_addr) = match event.kind {
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
        KIND_ORDER_REVISION_PROPOSAL => {
            let payload = order_revision_proposal_from_event(event)
                .map_err(order_evidence_parse_error)?
                .payload;
            (payload.order_id, payload.listing_addr)
        }
        KIND_ORDER_REVISION_DECISION => {
            let payload = order_revision_decision_from_event(event)
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
        event_kind: event.kind,
    })
}

#[cfg(feature = "runtime")]
fn order_evidence_parse_error(
    error: radroots_events_codec::order::RadrootsOrderEnvelopeParseError,
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
            source: SdkTradeStatusSource::LocalEventStore,
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
            eligibility,
            next_action,
            event_ids,
            request_event_id: projection.request_event_id,
            decision_event_id: projection.decision_event_id,
            agreement_event_id: projection.agreement_event_id,
            rhi_receipt_event_id: projection.validation_receipt_event_id,
            pending_revision_event_id: projection.pending_revision_event_id,
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
                pending_revision_event_id: None,
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
            has_pending_revision: projection.pending_revision_event_id.is_some(),
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
        let revision_proposed = projection.status == RadrootsTradeWorkflowState::RevisionProposed;
        let has_pending_revision = projection.pending_revision_event_id.is_some();

        Self {
            can_decide: open
                && requested
                && projection.decision_event_id.is_none()
                && !has_pending_revision,
            can_propose_revision: open && requested && !has_pending_revision,
            can_decide_revision: open && revision_proposed && has_pending_revision,
            can_cancel: open && requested && !has_pending_revision,
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
        if projection.status == RadrootsTradeWorkflowState::AgreedPendingRhi {
            return Self::AwaitRhiValidation;
        }
        if projection.lifecycle_terminal {
            return Self::Terminal;
        }
        if eligibility.can_decide {
            return Self::AwaitSellerDecision;
        }
        if eligibility.can_decide_revision {
            return Self::DecideRevision;
        }
        Self::Terminal
    }
}

#[cfg(any(feature = "signer-adapters", test))]
fn order_submit_plan(
    actor: &RadrootsActorContext,
    listing_event: RadrootsNostrEventPtr,
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
    let frozen_draft = to_frozen_draft(
        draft.into_wire_parts(),
        TRADE_SUBMIT_CONTRACT_ID,
        order_request.buyer_pubkey.as_str(),
        created_at_nostr,
    )
    .expect("validated order submit draft freezes");
    let expected_event_id = RadrootsEventId::parse(frozen_draft.expected_event_id.as_str())
        .expect("frozen order submit draft produces a valid event id");
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

#[cfg(any(feature = "signer-adapters", test))]
fn order_decision_plan(
    actor: &RadrootsActorContext,
    request_event: RadrootsNostrEventPtr,
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
    let frozen_draft = to_frozen_draft(
        draft.into_wire_parts(),
        TRADE_DECISION_CONTRACT_ID,
        decision.seller_pubkey.as_str(),
        created_at_nostr,
    )
    .expect("validated order decision draft freezes");
    let expected_event_id = RadrootsEventId::parse(frozen_draft.expected_event_id.as_str())
        .expect("frozen order decision draft produces a valid event id");
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

#[cfg(any(feature = "signer-adapters", test))]
fn order_revision_proposal_plan(
    actor: &RadrootsActorContext,
    root_event: RadrootsNostrEventPtr,
    previous_event: RadrootsNostrEventPtr,
    proposal: RadrootsOrderRevisionProposal,
    created_at: RadrootsSdkTimestamp,
) -> Result<TradeRevisionProposalPlan, RadrootsSdkError> {
    require_seller_actor(actor, "trade.prepare_revision_proposal")?;
    let root_event_id = order_reference_event_id(&root_event, "root")?;
    let previous_event_id = order_reference_event_id(&previous_event, "previous")?;
    if proposal.seller_pubkey.as_str() != actor.pubkey().as_str() {
        return Err(RadrootsSdkError::UnauthorizedActor {
            operation: "trade.prepare_revision_proposal".to_owned(),
            reason: "actor pubkey must match order seller_pubkey".to_owned(),
        });
    }
    require_payload_event_refs(
        "order revision proposal",
        &proposal.root_event_id,
        &proposal.prev_event_id,
        &root_event_id,
        &previous_event_id,
    )?;
    let created_at_nostr = created_at.try_into_nostr_created_at()?;
    let order_id = proposal.order_id.clone();
    let listing_addr = proposal.listing_addr.clone();
    let buyer_pubkey = proposal.buyer_pubkey.clone();
    let seller_pubkey = proposal.seller_pubkey.clone();
    validate_order_payload(&proposal, "order revision proposal")?;
    let draft =
        order::build_order_revision_proposal_draft(&root_event_id, &previous_event_id, &proposal)
            .expect("validated order revision proposal draft encodes");
    let (frozen_draft, expected_event_id) = freeze_order_workflow_draft(
        draft.into_wire_parts(),
        TRADE_REVISION_PROPOSAL_CONTRACT_ID,
        seller_pubkey.as_str(),
        created_at_nostr,
        "order revision proposal",
    );
    Ok(TradeRevisionProposalPlan {
        workflow: order_workflow_plan(
            TradeWorkflowKind::RevisionProposal,
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

#[cfg(any(feature = "signer-adapters", test))]
fn order_revision_decision_plan(
    actor: &RadrootsActorContext,
    root_event: RadrootsNostrEventPtr,
    previous_event: RadrootsNostrEventPtr,
    decision: RadrootsOrderRevisionDecision,
    created_at: RadrootsSdkTimestamp,
) -> Result<TradeRevisionDecisionPlan, RadrootsSdkError> {
    require_buyer_actor(actor, "trade.prepare_revision_decision")?;
    let root_event_id = order_reference_event_id(&root_event, "root")?;
    let previous_event_id = order_reference_event_id(&previous_event, "previous")?;
    if decision.buyer_pubkey.as_str() != actor.pubkey().as_str() {
        return Err(RadrootsSdkError::UnauthorizedActor {
            operation: "trade.prepare_revision_decision".to_owned(),
            reason: "actor pubkey must match order buyer_pubkey".to_owned(),
        });
    }
    require_payload_event_refs(
        "order revision decision",
        &decision.root_event_id,
        &decision.prev_event_id,
        &root_event_id,
        &previous_event_id,
    )?;
    let created_at_nostr = created_at.try_into_nostr_created_at()?;
    let order_id = decision.order_id.clone();
    let listing_addr = decision.listing_addr.clone();
    let buyer_pubkey = decision.buyer_pubkey.clone();
    let seller_pubkey = decision.seller_pubkey.clone();
    validate_order_payload(&decision, "order revision decision")?;
    let draft =
        order::build_order_revision_decision_draft(&root_event_id, &previous_event_id, &decision)
            .expect("validated order revision decision draft encodes");
    let (frozen_draft, expected_event_id) = freeze_order_workflow_draft(
        draft.into_wire_parts(),
        TRADE_REVISION_DECISION_CONTRACT_ID,
        buyer_pubkey.as_str(),
        created_at_nostr,
        "order revision decision",
    );
    Ok(TradeRevisionDecisionPlan {
        workflow: order_workflow_plan(
            TradeWorkflowKind::RevisionDecision,
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

#[cfg(any(feature = "signer-adapters", test))]
fn order_cancellation_plan(
    actor: &RadrootsActorContext,
    root_event: RadrootsNostrEventPtr,
    previous_event: RadrootsNostrEventPtr,
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

#[cfg(any(feature = "signer-adapters", test))]
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

#[cfg(any(feature = "signer-adapters", test))]
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

#[cfg(any(feature = "signer-adapters", test))]
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

#[cfg(any(feature = "signer-adapters", test))]
fn order_revision_proposal_receipt(
    plan: TradeRevisionProposalPlan,
    enqueue: crate::workflow_runtime::SdkWorkflowEnqueueReceipt,
) -> TradeRevisionProposalReceipt {
    let locator = RadrootsTradeLocator::from_order_id(plan.order_id.clone())
        .with_root_event_id(plan.root_event_id.clone())
        .with_listing_addr(plan.listing_addr.clone())
        .with_buyer_pubkey(plan.buyer_pubkey.clone())
        .with_seller_pubkey(plan.seller_pubkey.clone());
    TradeRevisionProposalReceipt {
        workflow: order_workflow_enqueue_receipt(
            TradeWorkflowKind::RevisionProposal,
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

#[cfg(any(feature = "signer-adapters", test))]
fn order_revision_decision_receipt(
    plan: TradeRevisionDecisionPlan,
    enqueue: crate::workflow_runtime::SdkWorkflowEnqueueReceipt,
) -> TradeRevisionDecisionReceipt {
    let locator = RadrootsTradeLocator::from_order_id(plan.order_id.clone())
        .with_root_event_id(plan.root_event_id.clone())
        .with_listing_addr(plan.listing_addr.clone())
        .with_buyer_pubkey(plan.buyer_pubkey.clone())
        .with_seller_pubkey(plan.seller_pubkey.clone());
    TradeRevisionDecisionReceipt {
        workflow: order_workflow_enqueue_receipt(
            TradeWorkflowKind::RevisionDecision,
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

#[cfg(any(feature = "signer-adapters", test))]
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

#[cfg(any(feature = "signer-adapters", test))]
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

#[cfg(any(feature = "signer-adapters", test))]
fn freeze_order_workflow_draft(
    parts: WireEventParts,
    contract_id: &str,
    expected_pubkey: &str,
    created_at: u32,
    _operation: &'static str,
) -> (RadrootsFrozenEventDraft, RadrootsEventId) {
    let frozen_draft = to_frozen_draft(parts, contract_id, expected_pubkey, created_at)
        .expect("validated order workflow draft freezes");
    let expected_event_id = RadrootsEventId::parse(frozen_draft.expected_event_id.as_str())
        .expect("frozen order workflow draft produces a valid event id");
    (frozen_draft, expected_event_id)
}

#[cfg(any(feature = "signer-adapters", test))]
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

#[cfg(any(feature = "signer-adapters", test))]
trait OrderPayloadValidate {
    fn validate_order_payload(
        &self,
    ) -> Result<(), radroots_events::order::RadrootsOrderPayloadError>;
}

#[cfg(any(feature = "signer-adapters", test))]
impl OrderPayloadValidate for RadrootsOrderDecision {
    fn validate_order_payload(
        &self,
    ) -> Result<(), radroots_events::order::RadrootsOrderPayloadError> {
        self.validate()
    }
}

#[cfg(any(feature = "signer-adapters", test))]
impl OrderPayloadValidate for RadrootsOrderRevisionProposal {
    fn validate_order_payload(
        &self,
    ) -> Result<(), radroots_events::order::RadrootsOrderPayloadError> {
        self.validate()
    }
}

#[cfg(any(feature = "signer-adapters", test))]
impl OrderPayloadValidate for RadrootsOrderRevisionDecision {
    fn validate_order_payload(
        &self,
    ) -> Result<(), radroots_events::order::RadrootsOrderPayloadError> {
        self.validate()
    }
}

#[cfg(any(feature = "signer-adapters", test))]
impl OrderPayloadValidate for RadrootsOrderCancellation {
    fn validate_order_payload(
        &self,
    ) -> Result<(), radroots_events::order::RadrootsOrderPayloadError> {
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
    event: &RadrootsNostrEvent,
) -> Result<TradeRequestEvidence, RadrootsSdkError> {
    let request_event_id = RadrootsEventId::parse(event.id.as_str()).map_err(|error| {
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

#[cfg(any(feature = "signer-adapters", test))]
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
    if let Some(pending_revision_event_id) = projection.pending_revision_event_id.as_ref() {
        return Err(RadrootsSdkError::InvalidRequest {
            message: format!(
                "order decision for order {} cannot follow pending revision proposal {}",
                plan.order_id, pending_revision_event_id
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

#[cfg(any(feature = "signer-adapters", test))]
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

#[cfg(any(feature = "signer-adapters", test))]
fn require_revision_proposal_state(
    plan: &TradeRevisionProposalPlan,
    projection: &RadrootsOrderProjection,
) -> Result<(), RadrootsSdkError> {
    let refs = OrderLifecycleReferences {
        operation: "order revision proposal",
        order_id: &plan.order_id,
        listing_addr: &plan.listing_addr,
        buyer_pubkey: &plan.buyer_pubkey,
        seller_pubkey: &plan.seller_pubkey,
        root_event_id: &plan.root_event_id,
        previous_event_id: &plan.previous_event_id,
    };
    require_clean_lifecycle_projection(refs, projection)?;
    require_lifecycle_status(&refs, projection, RadrootsTradeWorkflowState::Requested)?;
    require_no_lifecycle_terminal(&refs, projection)?;
    require_no_pending_revision(&refs, projection)?;
    require_lifecycle_previous_is_current(&refs, projection)
}

#[cfg(any(feature = "signer-adapters", test))]
fn require_revision_decision_state(
    plan: &TradeRevisionDecisionPlan,
    projection: &RadrootsOrderProjection,
) -> Result<(), RadrootsSdkError> {
    let refs = OrderLifecycleReferences {
        operation: "order revision decision",
        order_id: &plan.order_id,
        listing_addr: &plan.listing_addr,
        buyer_pubkey: &plan.buyer_pubkey,
        seller_pubkey: &plan.seller_pubkey,
        root_event_id: &plan.root_event_id,
        previous_event_id: &plan.previous_event_id,
    };
    require_clean_lifecycle_projection(refs, projection)?;
    require_lifecycle_status(
        &refs,
        projection,
        RadrootsTradeWorkflowState::RevisionProposed,
    )?;
    require_no_lifecycle_terminal(&refs, projection)?;
    require_pending_revision(&refs, projection)?;
    require_lifecycle_previous_is_current(&refs, projection)
}

#[cfg(any(feature = "signer-adapters", test))]
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
    require_no_pending_revision(&refs, projection)?;
    require_lifecycle_previous_is_current(&refs, projection)
}

#[cfg(any(feature = "signer-adapters", test))]
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

#[cfg(any(feature = "signer-adapters", test))]
fn require_lifecycle_status(
    refs: &OrderLifecycleReferences<'_>,
    projection: &RadrootsOrderProjection,
    expected: RadrootsTradeWorkflowState,
) -> Result<(), RadrootsSdkError> {
    if projection.status == expected {
        Ok(())
    } else {
        Err(lifecycle_invalid(
            refs.operation,
            refs.order_id,
            format!(
                "requires {:?} local state; current state is {:?}",
                expected, projection.status
            ),
        ))
    }
}

#[cfg(any(feature = "signer-adapters", test))]
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

#[cfg(any(feature = "signer-adapters", test))]
fn require_pending_revision(
    refs: &OrderLifecycleReferences<'_>,
    projection: &RadrootsOrderProjection,
) -> Result<(), RadrootsSdkError> {
    match projection.pending_revision_event_id.as_ref() {
        Some(pending_revision_event_id) if pending_revision_event_id == refs.previous_event_id => {
            Ok(())
        }
        Some(pending_revision_event_id) => Err(lifecycle_invalid(
            refs.operation,
            refs.order_id,
            format!(
                "previous event {} does not match pending revision proposal {}",
                refs.previous_event_id, pending_revision_event_id
            ),
        )),
        None => Err(lifecycle_invalid(
            refs.operation,
            refs.order_id,
            "requires pending revision proposal local state",
        )),
    }
}

#[cfg(any(feature = "signer-adapters", test))]
fn require_no_pending_revision(
    refs: &OrderLifecycleReferences<'_>,
    projection: &RadrootsOrderProjection,
) -> Result<(), RadrootsSdkError> {
    if let Some(pending_revision_event_id) = projection.pending_revision_event_id.as_ref() {
        Err(lifecycle_invalid(
            refs.operation,
            refs.order_id,
            format!("cannot follow pending revision proposal {pending_revision_event_id}"),
        ))
    } else {
        Ok(())
    }
}

#[cfg(any(feature = "signer-adapters", test))]
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

#[cfg(any(feature = "signer-adapters", test))]
fn lifecycle_invalid(
    operation: &'static str,
    order_id: &RadrootsOrderId,
    reason: impl Into<String>,
) -> RadrootsSdkError {
    RadrootsSdkError::InvalidRequest {
        message: format!("{operation} for order {order_id} {}", reason.into()),
    }
}

#[cfg(any(feature = "signer-adapters", test))]
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

#[cfg(any(feature = "signer-adapters", test))]
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

#[cfg(any(feature = "signer-adapters", test))]
fn listing_event_id(
    listing_event: &RadrootsNostrEventPtr,
) -> Result<RadrootsEventId, RadrootsSdkError> {
    RadrootsEventId::parse(listing_event.id.as_str()).map_err(|error| {
        RadrootsSdkError::InvalidRequest {
            message: format!("listing evidence event id is invalid: {error}"),
        }
    })
}

#[cfg(any(feature = "signer-adapters", test))]
fn request_event_id(
    request_event: &RadrootsNostrEventPtr,
) -> Result<RadrootsEventId, RadrootsSdkError> {
    RadrootsEventId::parse(request_event.id.as_str()).map_err(|error| {
        RadrootsSdkError::InvalidRequest {
            message: format!("order request evidence event id is invalid: {error}"),
        }
    })
}

#[cfg(any(feature = "signer-adapters", test))]
fn order_reference_event_id(
    event: &RadrootsNostrEventPtr,
    label: &'static str,
) -> Result<RadrootsEventId, RadrootsSdkError> {
    RadrootsEventId::parse(event.id.as_str()).map_err(|error| RadrootsSdkError::InvalidRequest {
        message: format!("order {label} evidence event id is invalid: {error}"),
    })
}

#[cfg(any(feature = "signer-adapters", test))]
fn require_payload_event_refs(
    operation: &'static str,
    payload_root_event_id: &RadrootsEventId,
    payload_previous_event_id: &RadrootsEventId,
    root_event_id: &RadrootsEventId,
    previous_event_id: &RadrootsEventId,
) -> Result<(), RadrootsSdkError> {
    if payload_root_event_id != root_event_id {
        return Err(RadrootsSdkError::InvalidRequest {
            message: format!(
                "{operation} root_event_id {} does not match root evidence {}",
                payload_root_event_id, root_event_id
            ),
        });
    }
    if payload_previous_event_id != previous_event_id {
        return Err(RadrootsSdkError::InvalidRequest {
            message: format!(
                "{operation} prev_event_id {} does not match previous evidence {}",
                payload_previous_event_id, previous_event_id
            ),
        });
    }
    Ok(())
}

#[cfg(any(feature = "signer-adapters", test))]
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

#[cfg(any(feature = "signer-adapters", test))]
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
            RadrootsTradeWorkflowState::RevisionProposed => Self::RevisionProposed,
            RadrootsTradeWorkflowState::AgreedPendingRhi => Self::AgreedPendingRhi,
            RadrootsTradeWorkflowState::Committed => Self::Committed,
            RadrootsTradeWorkflowState::Declined => Self::Declined,
            RadrootsTradeWorkflowState::Cancelled => Self::Cancelled,
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
            RadrootsOrderIssue::RevisionProposalPayloadInvalid { event_id } => Self::single(
                SdkTradeStatusIssueKind::RevisionProposalPayloadInvalid,
                event_id,
            ),
            RadrootsOrderIssue::RevisionProposalOrderIdMismatch { event_id } => Self::single(
                SdkTradeStatusIssueKind::RevisionProposalOrderIdMismatch,
                event_id,
            ),
            RadrootsOrderIssue::RevisionProposalAuthorMismatch { event_id } => Self::single(
                SdkTradeStatusIssueKind::RevisionProposalAuthorMismatch,
                event_id,
            ),
            RadrootsOrderIssue::RevisionProposalCounterpartyMismatch { event_id } => Self::single(
                SdkTradeStatusIssueKind::RevisionProposalCounterpartyMismatch,
                event_id,
            ),
            RadrootsOrderIssue::RevisionProposalBuyerMismatch { event_id } => Self::single(
                SdkTradeStatusIssueKind::RevisionProposalBuyerMismatch,
                event_id,
            ),
            RadrootsOrderIssue::RevisionProposalSellerMismatch { event_id } => Self::single(
                SdkTradeStatusIssueKind::RevisionProposalSellerMismatch,
                event_id,
            ),
            RadrootsOrderIssue::RevisionProposalListingAddressInvalid { event_id } => Self::single(
                SdkTradeStatusIssueKind::RevisionProposalListingAddressInvalid,
                event_id,
            ),
            RadrootsOrderIssue::RevisionProposalListingMismatch { event_id } => Self::single(
                SdkTradeStatusIssueKind::RevisionProposalListingMismatch,
                event_id,
            ),
            RadrootsOrderIssue::RevisionProposalRootMismatch { event_id } => Self::single(
                SdkTradeStatusIssueKind::RevisionProposalRootMismatch,
                event_id,
            ),
            RadrootsOrderIssue::RevisionProposalPreviousMismatch { event_id } => Self::single(
                SdkTradeStatusIssueKind::RevisionProposalPreviousMismatch,
                event_id,
            ),
            RadrootsOrderIssue::RevisionDecisionWithoutProposal { event_id } => Self::single(
                SdkTradeStatusIssueKind::RevisionDecisionWithoutProposal,
                event_id,
            ),
            RadrootsOrderIssue::RevisionDecisionPayloadInvalid { event_id } => Self::single(
                SdkTradeStatusIssueKind::RevisionDecisionPayloadInvalid,
                event_id,
            ),
            RadrootsOrderIssue::RevisionDecisionOrderIdMismatch { event_id } => Self::single(
                SdkTradeStatusIssueKind::RevisionDecisionOrderIdMismatch,
                event_id,
            ),
            RadrootsOrderIssue::RevisionDecisionAuthorMismatch { event_id } => Self::single(
                SdkTradeStatusIssueKind::RevisionDecisionAuthorMismatch,
                event_id,
            ),
            RadrootsOrderIssue::RevisionDecisionCounterpartyMismatch { event_id } => Self::single(
                SdkTradeStatusIssueKind::RevisionDecisionCounterpartyMismatch,
                event_id,
            ),
            RadrootsOrderIssue::RevisionDecisionBuyerMismatch { event_id } => Self::single(
                SdkTradeStatusIssueKind::RevisionDecisionBuyerMismatch,
                event_id,
            ),
            RadrootsOrderIssue::RevisionDecisionSellerMismatch { event_id } => Self::single(
                SdkTradeStatusIssueKind::RevisionDecisionSellerMismatch,
                event_id,
            ),
            RadrootsOrderIssue::RevisionDecisionListingAddressInvalid { event_id } => Self::single(
                SdkTradeStatusIssueKind::RevisionDecisionListingAddressInvalid,
                event_id,
            ),
            RadrootsOrderIssue::RevisionDecisionListingMismatch { event_id } => Self::single(
                SdkTradeStatusIssueKind::RevisionDecisionListingMismatch,
                event_id,
            ),
            RadrootsOrderIssue::RevisionDecisionRootMismatch { event_id } => Self::single(
                SdkTradeStatusIssueKind::RevisionDecisionRootMismatch,
                event_id,
            ),
            RadrootsOrderIssue::RevisionDecisionPreviousMismatch { event_id } => Self::single(
                SdkTradeStatusIssueKind::RevisionDecisionPreviousMismatch,
                event_id,
            ),
            RadrootsOrderIssue::RevisionDecisionRevisionIdMismatch { event_id } => Self::single(
                SdkTradeStatusIssueKind::RevisionDecisionRevisionIdMismatch,
                event_id,
            ),
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

#[cfg(all(test, feature = "runtime"))]
#[path = "../tests/unit/orders_runtime_tests.rs"]
mod tests;
