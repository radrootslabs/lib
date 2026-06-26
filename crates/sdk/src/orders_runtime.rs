#[cfg(feature = "signer-adapters")]
use crate::workflow_runtime::enqueue_configured_signed_workflow;
#[cfg(feature = "runtime")]
use crate::{
    RadrootsSdkError, RadrootsSdkRecoveryAction, RadrootsSdkTimestamp, SdkIdempotencyKey,
    SdkMutationState, SdkRelayTargetPolicy, SdkRelayUrlPolicy, TradesClient, order,
    workflow_runtime::{SdkWorkflowEnqueueRequest, enqueue_signed_workflow},
};
#[cfg(feature = "runtime")]
use radroots_authority::{RadrootsActorContext, RadrootsEventSigner};
#[cfg(feature = "runtime")]
use radroots_event_store::RadrootsEventIngest;
#[cfg(feature = "runtime")]
use radroots_events::{
    RadrootsNostrEvent, RadrootsNostrEventPtr,
    contract::RadrootsActorRole,
    draft::RadrootsFrozenEventDraft,
    ids::{RadrootsEventId, RadrootsListingAddress, RadrootsOrderId, RadrootsPublicKey},
    kinds::{
        KIND_ORDER_CANCELLATION, KIND_ORDER_DECISION, KIND_ORDER_REQUEST,
        KIND_ORDER_REVISION_DECISION, KIND_ORDER_REVISION_PROPOSAL,
    },
    order::{
        RadrootsOrderCancellation, RadrootsOrderDecision, RadrootsOrderEconomics,
        RadrootsOrderRequest, RadrootsOrderRevisionDecision, RadrootsOrderRevisionProposal,
    },
};
#[cfg(feature = "runtime")]
use radroots_events_codec::order::{
    order_cancellation_from_event, order_decision_from_event, order_request_from_event,
    order_revision_decision_from_event, order_revision_proposal_from_event,
};
#[cfg(feature = "runtime")]
use radroots_events_codec::wire::{WireEventParts, to_frozen_draft};
#[cfg(feature = "runtime")]
use radroots_trade::order::{
    RadrootsOrderCanonicalizationError, RadrootsOrderIssue, RadrootsOrderProjection,
    RadrootsOrderProjectionQueryResult, RadrootsOrderStoreQueryError,
    canonicalize_order_decision_for_signer, canonicalize_order_request_for_signer,
    order_projection_query_for_order_id,
};
#[cfg(feature = "runtime")]
use radroots_trade::workflow::RadrootsTradeWorkflowState;
#[cfg(feature = "runtime")]
use serde::ser::SerializeStruct;
#[cfg(feature = "runtime")]
pub const ORDER_STATUS_DEFAULT_LIMIT: u32 = 500;
#[cfg(feature = "runtime")]
pub const ORDER_STATUS_MAX_LIMIT: u32 = 1_000;
#[cfg(feature = "runtime")]
pub const ORDER_SUBMIT_OPERATION_KIND: &str = "order.submit.v1";
#[cfg(feature = "runtime")]
pub const ORDER_DECISION_OPERATION_KIND: &str = "order.decision.v1";
#[cfg(feature = "runtime")]
pub const ORDER_REVISION_PROPOSAL_OPERATION_KIND: &str = "order.revision.proposal.v1";
#[cfg(feature = "runtime")]
pub const ORDER_REVISION_DECISION_OPERATION_KIND: &str = "order.revision.decision.v1";
#[cfg(feature = "runtime")]
pub const ORDER_CANCELLATION_OPERATION_KIND: &str = "order.cancellation.v1";

#[cfg(feature = "runtime")]
const ORDER_REQUEST_CONTRACT_ID: &str = "radroots.order.request.v1";
#[cfg(feature = "runtime")]
const ORDER_DECISION_CONTRACT_ID: &str = "radroots.order.decision.v1";
#[cfg(feature = "runtime")]
const ORDER_REVISION_PROPOSAL_CONTRACT_ID: &str = "radroots.order.revision_proposal.v1";
#[cfg(feature = "runtime")]
const ORDER_REVISION_DECISION_CONTRACT_ID: &str = "radroots.order.revision_decision.v1";
#[cfg(feature = "runtime")]
const ORDER_CANCELLATION_CONTRACT_ID: &str = "radroots.order.cancellation.v1";

#[cfg(feature = "runtime")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum OrderWorkflowKind {
    Submit,
    Decision,
    RevisionProposal,
    RevisionDecision,
    Cancellation,
}

#[cfg(feature = "runtime")]
impl OrderWorkflowKind {
    pub fn operation_kind(self) -> &'static str {
        match self {
            Self::Submit => ORDER_SUBMIT_OPERATION_KIND,
            Self::Decision => ORDER_DECISION_OPERATION_KIND,
            Self::RevisionProposal => ORDER_REVISION_PROPOSAL_OPERATION_KIND,
            Self::RevisionDecision => ORDER_REVISION_DECISION_OPERATION_KIND,
            Self::Cancellation => ORDER_CANCELLATION_OPERATION_KIND,
        }
    }

    pub fn contract_id(self) -> &'static str {
        match self {
            Self::Submit => ORDER_REQUEST_CONTRACT_ID,
            Self::Decision => ORDER_DECISION_CONTRACT_ID,
            Self::RevisionProposal => ORDER_REVISION_PROPOSAL_CONTRACT_ID,
            Self::RevisionDecision => ORDER_REVISION_DECISION_CONTRACT_ID,
            Self::Cancellation => ORDER_CANCELLATION_CONTRACT_ID,
        }
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct OrderWorkflowPlan {
    pub kind: OrderWorkflowKind,
    pub operation_kind: &'static str,
    pub contract_id: &'static str,
    pub expected_event_id: RadrootsEventId,
    pub created_at: RadrootsSdkTimestamp,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct OrderWorkflowEnqueueReceipt {
    pub kind: OrderWorkflowKind,
    pub operation_kind: &'static str,
    pub expected_event_id: RadrootsEventId,
    pub signed_event_id: RadrootsEventId,
    pub local_event_seq: i64,
    pub outbox_operation_id: i64,
    pub outbox_event_id: i64,
    pub state: SdkMutationState,
    pub idempotency_digest_prefix: Option<String>,
    pub idempotency: OrderWorkflowIdempotencyReceipt,
    pub retry: OrderWorkflowRetryAdvice,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct OrderWorkflowIdempotencyReceipt {
    pub digest_prefix: Option<String>,
    pub replayed_existing_operation: bool,
    pub safe_to_retry_with_same_idempotency_key: bool,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct OrderWorkflowRetryAdvice {
    pub retryable_after_error: bool,
    pub safe_to_retry_enqueue_with_same_idempotency_key: bool,
    pub recovery_actions: Vec<RadrootsSdkRecoveryAction>,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, serde::Serialize)]
#[non_exhaustive]
pub struct OrderSubmitPrepareRequest {
    #[serde(serialize_with = "crate::actor_json::serialize_actor_context")]
    pub actor: RadrootsActorContext,
    pub listing_event: RadrootsNostrEventPtr,
    pub order: RadrootsOrderRequest,
    pub created_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(feature = "runtime")]
impl OrderSubmitPrepareRequest {
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
#[derive(Clone, Debug, serde::Serialize)]
#[non_exhaustive]
pub struct OrderSubmitEnqueueRequest {
    #[serde(serialize_with = "crate::actor_json::serialize_actor_context")]
    pub actor: RadrootsActorContext,
    pub listing_event: RadrootsNostrEventPtr,
    pub order: RadrootsOrderRequest,
    pub target_relays: SdkRelayTargetPolicy,
    pub idempotency_key: Option<SdkIdempotencyKey>,
    pub created_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(feature = "runtime")]
impl OrderSubmitEnqueueRequest {
    pub fn new(
        actor: RadrootsActorContext,
        listing_event: RadrootsNostrEventPtr,
        order: RadrootsOrderRequest,
        target_relays: SdkRelayTargetPolicy,
    ) -> Self {
        Self {
            actor,
            listing_event,
            order,
            target_relays,
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
        self.target_relays = SdkRelayTargetPolicy::try_explicit(target_relays, policy)?;
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

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct OrderSubmitPlan {
    pub workflow: OrderWorkflowPlan,
    pub order_id: RadrootsOrderId,
    pub listing_addr: RadrootsListingAddress,
    pub listing_event_id: RadrootsEventId,
    pub expected_event_id: RadrootsEventId,
    pub frozen_draft: RadrootsFrozenEventDraft,
    pub created_at: RadrootsSdkTimestamp,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct OrderSubmitReceipt {
    pub workflow: OrderWorkflowEnqueueReceipt,
    pub order_id: RadrootsOrderId,
    pub listing_addr: RadrootsListingAddress,
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
pub struct OrderRequestEvidenceIngestRequest {
    pub event: RadrootsNostrEvent,
    pub observed_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(feature = "runtime")]
impl OrderRequestEvidenceIngestRequest {
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
pub struct OrderRequestEvidenceIngestReceipt {
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
pub struct OrderEvidenceIngestRequest {
    pub event: RadrootsNostrEvent,
    pub observed_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(feature = "runtime")]
impl OrderEvidenceIngestRequest {
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
pub struct OrderEvidenceIngestReceipt {
    pub order_id: RadrootsOrderId,
    pub listing_addr: RadrootsListingAddress,
    pub event_id: RadrootsEventId,
    pub event_kind: u32,
    pub local_event_seq: i64,
    pub inserted: bool,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, serde::Serialize)]
#[non_exhaustive]
pub struct OrderDecisionPrepareRequest {
    #[serde(serialize_with = "crate::actor_json::serialize_actor_context")]
    pub actor: RadrootsActorContext,
    pub request_event: RadrootsNostrEventPtr,
    pub decision: RadrootsOrderDecision,
    pub created_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(feature = "runtime")]
impl OrderDecisionPrepareRequest {
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
#[derive(Clone, Debug, serde::Serialize)]
#[non_exhaustive]
pub struct OrderDecisionEnqueueRequest {
    #[serde(serialize_with = "crate::actor_json::serialize_actor_context")]
    pub actor: RadrootsActorContext,
    pub request_event: RadrootsNostrEventPtr,
    pub decision: RadrootsOrderDecision,
    pub target_relays: SdkRelayTargetPolicy,
    pub idempotency_key: Option<SdkIdempotencyKey>,
    pub created_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(feature = "runtime")]
impl OrderDecisionEnqueueRequest {
    pub fn new(
        actor: RadrootsActorContext,
        request_event: RadrootsNostrEventPtr,
        decision: RadrootsOrderDecision,
        target_relays: SdkRelayTargetPolicy,
    ) -> Self {
        Self {
            actor,
            request_event,
            decision,
            target_relays,
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
        self.target_relays = SdkRelayTargetPolicy::try_explicit(target_relays, policy)?;
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

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct OrderDecisionPlan {
    pub workflow: OrderWorkflowPlan,
    pub order_id: RadrootsOrderId,
    pub listing_addr: RadrootsListingAddress,
    pub buyer_pubkey: RadrootsPublicKey,
    pub seller_pubkey: RadrootsPublicKey,
    pub request_event_id: RadrootsEventId,
    pub expected_event_id: RadrootsEventId,
    pub frozen_draft: RadrootsFrozenEventDraft,
    pub created_at: RadrootsSdkTimestamp,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct OrderDecisionReceipt {
    pub workflow: OrderWorkflowEnqueueReceipt,
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

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, serde::Serialize)]
#[non_exhaustive]
pub struct OrderRevisionProposalPrepareRequest {
    #[serde(serialize_with = "crate::actor_json::serialize_actor_context")]
    pub actor: RadrootsActorContext,
    pub root_event: RadrootsNostrEventPtr,
    pub previous_event: RadrootsNostrEventPtr,
    pub proposal: RadrootsOrderRevisionProposal,
    pub created_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(feature = "runtime")]
impl OrderRevisionProposalPrepareRequest {
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
#[derive(Clone, Debug, serde::Serialize)]
#[non_exhaustive]
pub struct OrderRevisionProposalEnqueueRequest {
    #[serde(serialize_with = "crate::actor_json::serialize_actor_context")]
    pub actor: RadrootsActorContext,
    pub root_event: RadrootsNostrEventPtr,
    pub previous_event: RadrootsNostrEventPtr,
    pub proposal: RadrootsOrderRevisionProposal,
    pub target_relays: SdkRelayTargetPolicy,
    pub idempotency_key: Option<SdkIdempotencyKey>,
    pub created_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(feature = "runtime")]
impl OrderRevisionProposalEnqueueRequest {
    pub fn new(
        actor: RadrootsActorContext,
        root_event: RadrootsNostrEventPtr,
        previous_event: RadrootsNostrEventPtr,
        proposal: RadrootsOrderRevisionProposal,
        target_relays: SdkRelayTargetPolicy,
    ) -> Self {
        Self {
            actor,
            root_event,
            previous_event,
            proposal,
            target_relays,
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
        self.target_relays = SdkRelayTargetPolicy::try_explicit(target_relays, policy)?;
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

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct OrderRevisionProposalPlan {
    pub workflow: OrderWorkflowPlan,
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

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct OrderRevisionProposalReceipt {
    pub workflow: OrderWorkflowEnqueueReceipt,
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

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, serde::Serialize)]
#[non_exhaustive]
pub struct OrderRevisionDecisionPrepareRequest {
    #[serde(serialize_with = "crate::actor_json::serialize_actor_context")]
    pub actor: RadrootsActorContext,
    pub root_event: RadrootsNostrEventPtr,
    pub previous_event: RadrootsNostrEventPtr,
    pub decision: RadrootsOrderRevisionDecision,
    pub created_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(feature = "runtime")]
impl OrderRevisionDecisionPrepareRequest {
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
#[derive(Clone, Debug, serde::Serialize)]
#[non_exhaustive]
pub struct OrderRevisionDecisionEnqueueRequest {
    #[serde(serialize_with = "crate::actor_json::serialize_actor_context")]
    pub actor: RadrootsActorContext,
    pub root_event: RadrootsNostrEventPtr,
    pub previous_event: RadrootsNostrEventPtr,
    pub decision: RadrootsOrderRevisionDecision,
    pub target_relays: SdkRelayTargetPolicy,
    pub idempotency_key: Option<SdkIdempotencyKey>,
    pub created_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(feature = "runtime")]
impl OrderRevisionDecisionEnqueueRequest {
    pub fn new(
        actor: RadrootsActorContext,
        root_event: RadrootsNostrEventPtr,
        previous_event: RadrootsNostrEventPtr,
        decision: RadrootsOrderRevisionDecision,
        target_relays: SdkRelayTargetPolicy,
    ) -> Self {
        Self {
            actor,
            root_event,
            previous_event,
            decision,
            target_relays,
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
        self.target_relays = SdkRelayTargetPolicy::try_explicit(target_relays, policy)?;
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

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct OrderRevisionDecisionPlan {
    pub workflow: OrderWorkflowPlan,
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

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct OrderRevisionDecisionReceipt {
    pub workflow: OrderWorkflowEnqueueReceipt,
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

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, serde::Serialize)]
#[non_exhaustive]
pub struct OrderCancellationPrepareRequest {
    #[serde(serialize_with = "crate::actor_json::serialize_actor_context")]
    pub actor: RadrootsActorContext,
    pub root_event: RadrootsNostrEventPtr,
    pub previous_event: RadrootsNostrEventPtr,
    pub cancellation: RadrootsOrderCancellation,
    pub created_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(feature = "runtime")]
impl OrderCancellationPrepareRequest {
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
#[derive(Clone, Debug, serde::Serialize)]
#[non_exhaustive]
pub struct OrderCancellationEnqueueRequest {
    #[serde(serialize_with = "crate::actor_json::serialize_actor_context")]
    pub actor: RadrootsActorContext,
    pub root_event: RadrootsNostrEventPtr,
    pub previous_event: RadrootsNostrEventPtr,
    pub cancellation: RadrootsOrderCancellation,
    pub target_relays: SdkRelayTargetPolicy,
    pub idempotency_key: Option<SdkIdempotencyKey>,
    pub created_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(feature = "runtime")]
impl OrderCancellationEnqueueRequest {
    pub fn new(
        actor: RadrootsActorContext,
        root_event: RadrootsNostrEventPtr,
        previous_event: RadrootsNostrEventPtr,
        cancellation: RadrootsOrderCancellation,
        target_relays: SdkRelayTargetPolicy,
    ) -> Self {
        Self {
            actor,
            root_event,
            previous_event,
            cancellation,
            target_relays,
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
        self.target_relays = SdkRelayTargetPolicy::try_explicit(target_relays, policy)?;
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

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct OrderCancellationPlan {
    pub workflow: OrderWorkflowPlan,
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

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct OrderCancellationReceipt {
    pub workflow: OrderWorkflowEnqueueReceipt,
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

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[non_exhaustive]
pub struct OrderStatusRequest {
    pub order_id: RadrootsOrderId,
    pub limit: u32,
}

#[cfg(feature = "runtime")]
impl OrderStatusRequest {
    pub fn new(order_id: RadrootsOrderId) -> Self {
        Self {
            order_id,
            limit: ORDER_STATUS_DEFAULT_LIMIT,
        }
    }

    pub fn parse(order_id: &str) -> Result<Self, RadrootsSdkError> {
        RadrootsOrderId::parse(order_id)
            .map(Self::new)
            .map_err(|error| RadrootsSdkError::invalid_order_id(order_id, error.to_string()))
    }

    pub fn with_limit(mut self, limit: u32) -> Self {
        self.limit = limit;
        self
    }

    fn validate(&self) -> Result<(), RadrootsSdkError> {
        if self.limit == 0 || self.limit > ORDER_STATUS_MAX_LIMIT {
            return Err(RadrootsSdkError::order_status_limit_invalid(
                self.limit,
                1,
                ORDER_STATUS_MAX_LIMIT,
            ));
        }
        Ok(())
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct OrderStatusReceipt {
    pub order_id: RadrootsOrderId,
    pub source: SdkOrderStatusSource,
    pub found: bool,
    pub event_count: usize,
    pub limit_applied: u32,
    pub status: OrderStatusKind,
    pub lifecycle_terminal: bool,
    pub listing_addr: Option<RadrootsListingAddress>,
    pub buyer_pubkey: Option<RadrootsPublicKey>,
    pub seller_pubkey: Option<RadrootsPublicKey>,
    pub economics: Option<RadrootsOrderEconomics>,
    pub evidence: OrderStatusEvidenceSummary,
    pub eligibility: OrderStatusEligibility,
    pub next_action: OrderStatusNextActionKind,
    pub event_ids: Vec<RadrootsEventId>,
    pub request_event_id: Option<RadrootsEventId>,
    pub decision_event_id: Option<RadrootsEventId>,
    pub agreement_event_id: Option<RadrootsEventId>,
    pub pending_revision_event_id: Option<RadrootsEventId>,
    pub cancellation_event_id: Option<RadrootsEventId>,
    pub last_event_id: Option<RadrootsEventId>,
    pub issues: Vec<SdkOrderStatusIssue>,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct OrderStatusEvidenceSummary {
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
pub struct OrderStatusEligibility {
    pub can_decide: bool,
    pub can_propose_revision: bool,
    pub can_decide_revision: bool,
    pub can_cancel: bool,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum OrderStatusNextActionKind {
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
pub enum SdkOrderStatusSource {
    LocalEventStore,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum OrderStatusKind {
    Missing,
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
pub struct SdkOrderStatusIssue {
    pub kind: SdkOrderStatusIssueKind,
    pub event_ids: Vec<RadrootsEventId>,
}

#[cfg(feature = "runtime")]
impl SdkOrderStatusIssue {
    fn new(kind: SdkOrderStatusIssueKind, event_ids: Vec<RadrootsEventId>) -> Self {
        Self { kind, event_ids }
    }

    fn single(kind: SdkOrderStatusIssueKind, event_id: RadrootsEventId) -> Self {
        Self::new(kind, vec![event_id])
    }

    pub fn code(&self) -> String {
        self.kind.code()
    }
}

#[cfg(feature = "runtime")]
impl serde::Serialize for SdkOrderStatusIssue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("SdkOrderStatusIssue", 3)?;
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
pub enum SdkOrderStatusIssueKind {
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
impl SdkOrderStatusIssueKind {
    pub fn code(self) -> String {
        camel_to_snake(format!("{self:?}").as_str())
    }
}

#[cfg(feature = "runtime")]
impl<'sdk> TradesClient<'sdk> {
    pub async fn ingest_evidence(
        &self,
        request: OrderEvidenceIngestRequest,
    ) -> Result<OrderEvidenceIngestReceipt, RadrootsSdkError> {
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
        Ok(OrderEvidenceIngestReceipt {
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
        request: OrderRequestEvidenceIngestRequest,
    ) -> Result<OrderRequestEvidenceIngestReceipt, RadrootsSdkError> {
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
        Ok(OrderRequestEvidenceIngestReceipt {
            order_id: evidence.order_id,
            listing_addr: evidence.listing_addr,
            buyer_pubkey: evidence.buyer_pubkey,
            seller_pubkey: evidence.seller_pubkey,
            request_event_id: evidence.request_event_id,
            local_event_seq: receipt.seq,
            inserted: receipt.inserted,
        })
    }

    pub fn prepare_submit(
        &self,
        request: OrderSubmitPrepareRequest,
    ) -> Result<OrderSubmitPlan, RadrootsSdkError> {
        let created_at = self.resolved_created_at(request.created_at)?;
        order_submit_plan(
            &request.actor,
            request.listing_event,
            request.order,
            created_at,
        )
    }

    #[cfg(feature = "signer-adapters")]
    pub async fn enqueue_submit(
        &self,
        request: OrderSubmitEnqueueRequest,
    ) -> Result<OrderSubmitReceipt, RadrootsSdkError> {
        let OrderSubmitEnqueueRequest {
            actor,
            listing_event,
            order,
            target_relays,
            idempotency_key,
            created_at,
        } = request;
        let prepare_request = OrderSubmitPrepareRequest {
            actor: actor.clone(),
            listing_event,
            order,
            created_at,
        };
        let plan = self.prepare_submit(prepare_request)?;
        self.enqueue_prepared_submit(&actor, plan, target_relays, idempotency_key)
            .await
    }

    pub async fn enqueue_submit_with_explicit_signer(
        &self,
        request: OrderSubmitEnqueueRequest,
        signer: &dyn RadrootsEventSigner,
    ) -> Result<OrderSubmitReceipt, RadrootsSdkError> {
        let OrderSubmitEnqueueRequest {
            actor,
            listing_event,
            order,
            target_relays,
            idempotency_key,
            created_at,
        } = request;
        let prepare_request = OrderSubmitPrepareRequest {
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
            idempotency_key,
            signer,
        )
        .await
    }

    #[cfg(feature = "signer-adapters")]
    pub async fn enqueue_prepared_submit(
        &self,
        actor: &RadrootsActorContext,
        plan: OrderSubmitPlan,
        target_relays: SdkRelayTargetPolicy,
        idempotency_key: Option<SdkIdempotencyKey>,
    ) -> Result<OrderSubmitReceipt, RadrootsSdkError> {
        let enqueue = enqueue_configured_signed_workflow(
            self.sdk,
            SdkWorkflowEnqueueRequest {
                operation_kind: OrderWorkflowKind::Submit.operation_kind(),
                actor,
                frozen_draft: &plan.frozen_draft,
                target_relays,
                idempotency_key,
            },
        )
        .await?;
        Ok(order_submit_receipt(plan, enqueue))
    }

    pub async fn enqueue_prepared_submit_with_explicit_signer(
        &self,
        actor: &RadrootsActorContext,
        plan: OrderSubmitPlan,
        target_relays: SdkRelayTargetPolicy,
        idempotency_key: Option<SdkIdempotencyKey>,
        signer: &dyn RadrootsEventSigner,
    ) -> Result<OrderSubmitReceipt, RadrootsSdkError> {
        let enqueue = enqueue_signed_workflow(
            self.sdk,
            SdkWorkflowEnqueueRequest {
                operation_kind: OrderWorkflowKind::Submit.operation_kind(),
                actor,
                frozen_draft: &plan.frozen_draft,
                target_relays,
                idempotency_key,
            },
            signer,
        )
        .await?;
        Ok(order_submit_receipt(plan, enqueue))
    }

    pub fn prepare_decision(
        &self,
        request: OrderDecisionPrepareRequest,
    ) -> Result<OrderDecisionPlan, RadrootsSdkError> {
        let created_at = self.resolved_created_at(request.created_at)?;
        order_decision_plan(
            &request.actor,
            request.request_event,
            request.decision,
            created_at,
        )
    }

    #[cfg(feature = "signer-adapters")]
    pub async fn enqueue_decision(
        &self,
        request: OrderDecisionEnqueueRequest,
    ) -> Result<OrderDecisionReceipt, RadrootsSdkError> {
        let OrderDecisionEnqueueRequest {
            actor,
            request_event,
            decision,
            target_relays,
            idempotency_key,
            created_at,
        } = request;
        let prepare_request = OrderDecisionPrepareRequest {
            actor: actor.clone(),
            request_event,
            decision,
            created_at,
        };
        let plan = self.prepare_decision(prepare_request)?;
        self.enqueue_prepared_decision(&actor, plan, target_relays, idempotency_key)
            .await
    }

    pub async fn enqueue_decision_with_explicit_signer(
        &self,
        request: OrderDecisionEnqueueRequest,
        signer: &dyn RadrootsEventSigner,
    ) -> Result<OrderDecisionReceipt, RadrootsSdkError> {
        let OrderDecisionEnqueueRequest {
            actor,
            request_event,
            decision,
            target_relays,
            idempotency_key,
            created_at,
        } = request;
        let prepare_request = OrderDecisionPrepareRequest {
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
            idempotency_key,
            signer,
        )
        .await
    }

    #[cfg(feature = "signer-adapters")]
    pub async fn enqueue_prepared_decision(
        &self,
        actor: &RadrootsActorContext,
        plan: OrderDecisionPlan,
        target_relays: SdkRelayTargetPolicy,
        idempotency_key: Option<SdkIdempotencyKey>,
    ) -> Result<OrderDecisionReceipt, RadrootsSdkError> {
        if !self
            .prepared_order_event_exists(&plan.expected_event_id)
            .await?
        {
            self.require_decision_preflight(&plan).await?;
        }
        let enqueue = enqueue_configured_signed_workflow(
            self.sdk,
            SdkWorkflowEnqueueRequest {
                operation_kind: OrderWorkflowKind::Decision.operation_kind(),
                actor,
                frozen_draft: &plan.frozen_draft,
                target_relays,
                idempotency_key,
            },
        )
        .await?;
        Ok(order_decision_receipt(plan, enqueue))
    }

    pub async fn enqueue_prepared_decision_with_explicit_signer(
        &self,
        actor: &RadrootsActorContext,
        plan: OrderDecisionPlan,
        target_relays: SdkRelayTargetPolicy,
        idempotency_key: Option<SdkIdempotencyKey>,
        signer: &dyn RadrootsEventSigner,
    ) -> Result<OrderDecisionReceipt, RadrootsSdkError> {
        if !self
            .prepared_order_event_exists(&plan.expected_event_id)
            .await?
        {
            self.require_decision_preflight(&plan).await?;
        }
        let enqueue = enqueue_signed_workflow(
            self.sdk,
            SdkWorkflowEnqueueRequest {
                operation_kind: OrderWorkflowKind::Decision.operation_kind(),
                actor,
                frozen_draft: &plan.frozen_draft,
                target_relays,
                idempotency_key,
            },
            signer,
        )
        .await?;
        Ok(order_decision_receipt(plan, enqueue))
    }

    pub fn prepare_revision_proposal(
        &self,
        request: OrderRevisionProposalPrepareRequest,
    ) -> Result<OrderRevisionProposalPlan, RadrootsSdkError> {
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
    pub async fn enqueue_revision_proposal(
        &self,
        request: OrderRevisionProposalEnqueueRequest,
    ) -> Result<OrderRevisionProposalReceipt, RadrootsSdkError> {
        let OrderRevisionProposalEnqueueRequest {
            actor,
            root_event,
            previous_event,
            proposal,
            target_relays,
            idempotency_key,
            created_at,
        } = request;
        let prepare_request = OrderRevisionProposalPrepareRequest {
            actor: actor.clone(),
            root_event,
            previous_event,
            proposal,
            created_at,
        };
        let plan = self.prepare_revision_proposal(prepare_request)?;
        self.enqueue_prepared_revision_proposal(&actor, plan, target_relays, idempotency_key)
            .await
    }

    pub async fn enqueue_revision_proposal_with_explicit_signer(
        &self,
        request: OrderRevisionProposalEnqueueRequest,
        signer: &dyn RadrootsEventSigner,
    ) -> Result<OrderRevisionProposalReceipt, RadrootsSdkError> {
        let OrderRevisionProposalEnqueueRequest {
            actor,
            root_event,
            previous_event,
            proposal,
            target_relays,
            idempotency_key,
            created_at,
        } = request;
        let prepare_request = OrderRevisionProposalPrepareRequest {
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
            idempotency_key,
            signer,
        )
        .await
    }

    #[cfg(feature = "signer-adapters")]
    pub async fn enqueue_prepared_revision_proposal(
        &self,
        actor: &RadrootsActorContext,
        plan: OrderRevisionProposalPlan,
        target_relays: SdkRelayTargetPolicy,
        idempotency_key: Option<SdkIdempotencyKey>,
    ) -> Result<OrderRevisionProposalReceipt, RadrootsSdkError> {
        if !self
            .prepared_order_event_exists(&plan.expected_event_id)
            .await?
        {
            self.require_revision_proposal_preflight(&plan).await?;
        }
        let enqueue = enqueue_configured_signed_workflow(
            self.sdk,
            SdkWorkflowEnqueueRequest {
                operation_kind: OrderWorkflowKind::RevisionProposal.operation_kind(),
                actor,
                frozen_draft: &plan.frozen_draft,
                target_relays,
                idempotency_key,
            },
        )
        .await?;
        Ok(order_revision_proposal_receipt(plan, enqueue))
    }

    pub async fn enqueue_prepared_revision_proposal_with_explicit_signer(
        &self,
        actor: &RadrootsActorContext,
        plan: OrderRevisionProposalPlan,
        target_relays: SdkRelayTargetPolicy,
        idempotency_key: Option<SdkIdempotencyKey>,
        signer: &dyn RadrootsEventSigner,
    ) -> Result<OrderRevisionProposalReceipt, RadrootsSdkError> {
        if !self
            .prepared_order_event_exists(&plan.expected_event_id)
            .await?
        {
            self.require_revision_proposal_preflight(&plan).await?;
        }
        let enqueue = enqueue_signed_workflow(
            self.sdk,
            SdkWorkflowEnqueueRequest {
                operation_kind: OrderWorkflowKind::RevisionProposal.operation_kind(),
                actor,
                frozen_draft: &plan.frozen_draft,
                target_relays,
                idempotency_key,
            },
            signer,
        )
        .await?;
        Ok(order_revision_proposal_receipt(plan, enqueue))
    }

    pub fn prepare_revision_decision(
        &self,
        request: OrderRevisionDecisionPrepareRequest,
    ) -> Result<OrderRevisionDecisionPlan, RadrootsSdkError> {
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
    pub async fn enqueue_revision_decision(
        &self,
        request: OrderRevisionDecisionEnqueueRequest,
    ) -> Result<OrderRevisionDecisionReceipt, RadrootsSdkError> {
        let OrderRevisionDecisionEnqueueRequest {
            actor,
            root_event,
            previous_event,
            decision,
            target_relays,
            idempotency_key,
            created_at,
        } = request;
        let prepare_request = OrderRevisionDecisionPrepareRequest {
            actor: actor.clone(),
            root_event,
            previous_event,
            decision,
            created_at,
        };
        let plan = self.prepare_revision_decision(prepare_request)?;
        self.enqueue_prepared_revision_decision(&actor, plan, target_relays, idempotency_key)
            .await
    }

    pub async fn enqueue_revision_decision_with_explicit_signer(
        &self,
        request: OrderRevisionDecisionEnqueueRequest,
        signer: &dyn RadrootsEventSigner,
    ) -> Result<OrderRevisionDecisionReceipt, RadrootsSdkError> {
        let OrderRevisionDecisionEnqueueRequest {
            actor,
            root_event,
            previous_event,
            decision,
            target_relays,
            idempotency_key,
            created_at,
        } = request;
        let prepare_request = OrderRevisionDecisionPrepareRequest {
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
            idempotency_key,
            signer,
        )
        .await
    }

    #[cfg(feature = "signer-adapters")]
    pub async fn enqueue_prepared_revision_decision(
        &self,
        actor: &RadrootsActorContext,
        plan: OrderRevisionDecisionPlan,
        target_relays: SdkRelayTargetPolicy,
        idempotency_key: Option<SdkIdempotencyKey>,
    ) -> Result<OrderRevisionDecisionReceipt, RadrootsSdkError> {
        if !self
            .prepared_order_event_exists(&plan.expected_event_id)
            .await?
        {
            self.require_revision_decision_preflight(&plan).await?;
        }
        let enqueue = enqueue_configured_signed_workflow(
            self.sdk,
            SdkWorkflowEnqueueRequest {
                operation_kind: OrderWorkflowKind::RevisionDecision.operation_kind(),
                actor,
                frozen_draft: &plan.frozen_draft,
                target_relays,
                idempotency_key,
            },
        )
        .await?;
        Ok(order_revision_decision_receipt(plan, enqueue))
    }

    pub async fn enqueue_prepared_revision_decision_with_explicit_signer(
        &self,
        actor: &RadrootsActorContext,
        plan: OrderRevisionDecisionPlan,
        target_relays: SdkRelayTargetPolicy,
        idempotency_key: Option<SdkIdempotencyKey>,
        signer: &dyn RadrootsEventSigner,
    ) -> Result<OrderRevisionDecisionReceipt, RadrootsSdkError> {
        if !self
            .prepared_order_event_exists(&plan.expected_event_id)
            .await?
        {
            self.require_revision_decision_preflight(&plan).await?;
        }
        let enqueue = enqueue_signed_workflow(
            self.sdk,
            SdkWorkflowEnqueueRequest {
                operation_kind: OrderWorkflowKind::RevisionDecision.operation_kind(),
                actor,
                frozen_draft: &plan.frozen_draft,
                target_relays,
                idempotency_key,
            },
            signer,
        )
        .await?;
        Ok(order_revision_decision_receipt(plan, enqueue))
    }

    pub fn prepare_cancellation(
        &self,
        request: OrderCancellationPrepareRequest,
    ) -> Result<OrderCancellationPlan, RadrootsSdkError> {
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
    pub async fn enqueue_cancellation(
        &self,
        request: OrderCancellationEnqueueRequest,
    ) -> Result<OrderCancellationReceipt, RadrootsSdkError> {
        let OrderCancellationEnqueueRequest {
            actor,
            root_event,
            previous_event,
            cancellation,
            target_relays,
            idempotency_key,
            created_at,
        } = request;
        let prepare_request = OrderCancellationPrepareRequest {
            actor: actor.clone(),
            root_event,
            previous_event,
            cancellation,
            created_at,
        };
        let plan = self.prepare_cancellation(prepare_request)?;
        self.enqueue_prepared_cancellation(&actor, plan, target_relays, idempotency_key)
            .await
    }

    pub async fn enqueue_cancellation_with_explicit_signer(
        &self,
        request: OrderCancellationEnqueueRequest,
        signer: &dyn RadrootsEventSigner,
    ) -> Result<OrderCancellationReceipt, RadrootsSdkError> {
        let OrderCancellationEnqueueRequest {
            actor,
            root_event,
            previous_event,
            cancellation,
            target_relays,
            idempotency_key,
            created_at,
        } = request;
        let prepare_request = OrderCancellationPrepareRequest {
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
            idempotency_key,
            signer,
        )
        .await
    }

    #[cfg(feature = "signer-adapters")]
    pub async fn enqueue_prepared_cancellation(
        &self,
        actor: &RadrootsActorContext,
        plan: OrderCancellationPlan,
        target_relays: SdkRelayTargetPolicy,
        idempotency_key: Option<SdkIdempotencyKey>,
    ) -> Result<OrderCancellationReceipt, RadrootsSdkError> {
        if !self
            .prepared_order_event_exists(&plan.expected_event_id)
            .await?
        {
            self.require_cancellation_preflight(&plan).await?;
        }
        let enqueue = enqueue_configured_signed_workflow(
            self.sdk,
            SdkWorkflowEnqueueRequest {
                operation_kind: OrderWorkflowKind::Cancellation.operation_kind(),
                actor,
                frozen_draft: &plan.frozen_draft,
                target_relays,
                idempotency_key,
            },
        )
        .await?;
        Ok(order_cancellation_receipt(plan, enqueue))
    }

    pub async fn enqueue_prepared_cancellation_with_explicit_signer(
        &self,
        actor: &RadrootsActorContext,
        plan: OrderCancellationPlan,
        target_relays: SdkRelayTargetPolicy,
        idempotency_key: Option<SdkIdempotencyKey>,
        signer: &dyn RadrootsEventSigner,
    ) -> Result<OrderCancellationReceipt, RadrootsSdkError> {
        if !self
            .prepared_order_event_exists(&plan.expected_event_id)
            .await?
        {
            self.require_cancellation_preflight(&plan).await?;
        }
        let enqueue = enqueue_signed_workflow(
            self.sdk,
            SdkWorkflowEnqueueRequest {
                operation_kind: OrderWorkflowKind::Cancellation.operation_kind(),
                actor,
                frozen_draft: &plan.frozen_draft,
                target_relays,
                idempotency_key,
            },
            signer,
        )
        .await?;
        Ok(order_cancellation_receipt(plan, enqueue))
    }

    pub async fn status(
        &self,
        request: OrderStatusRequest,
    ) -> Result<OrderStatusReceipt, RadrootsSdkError> {
        request.validate()?;
        let query_result = order_projection_query_for_order_id(
            &self.sdk._event_store,
            &request.order_id,
            request.limit,
        )
        .await
        .map_err(projection_error)?;
        Ok(OrderStatusReceipt::from_query_result(query_result))
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

    async fn require_decision_preflight(
        &self,
        plan: &OrderDecisionPlan,
    ) -> Result<(), RadrootsSdkError> {
        let query_result = self.query_order_projection(&plan.order_id).await?;
        require_decision_request_evidence(plan, &query_result.projection)
    }

    async fn require_revision_proposal_preflight(
        &self,
        plan: &OrderRevisionProposalPlan,
    ) -> Result<(), RadrootsSdkError> {
        let query_result = self.query_order_projection(&plan.order_id).await?;
        require_revision_proposal_state(plan, &query_result.projection)
    }

    async fn require_revision_decision_preflight(
        &self,
        plan: &OrderRevisionDecisionPlan,
    ) -> Result<(), RadrootsSdkError> {
        let query_result = self.query_order_projection(&plan.order_id).await?;
        require_revision_decision_state(plan, &query_result.projection)
    }

    async fn require_cancellation_preflight(
        &self,
        plan: &OrderCancellationPlan,
    ) -> Result<(), RadrootsSdkError> {
        let query_result = self.query_order_projection(&plan.order_id).await?;
        require_cancellation_state(plan, &query_result.projection)
    }

    async fn query_order_projection(
        &self,
        order_id: &RadrootsOrderId,
    ) -> Result<RadrootsOrderProjectionQueryResult, RadrootsSdkError> {
        order_projection_query_for_order_id(
            &self.sdk._event_store,
            order_id,
            ORDER_STATUS_MAX_LIMIT,
        )
        .await
        .map_err(projection_error)
    }

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
impl OrderStatusReceipt {
    fn from_query_result(query_result: RadrootsOrderProjectionQueryResult) -> Self {
        let projection = query_result.projection;
        let found = projection.status != RadrootsTradeWorkflowState::Missing;
        let evidence = OrderStatusEvidenceSummary::from_projection(
            &projection,
            query_result.event_count,
            query_result.limit_applied,
        );
        let eligibility = OrderStatusEligibility::from_projection(&projection);
        let next_action = OrderStatusNextActionKind::from_projection(&projection, &eligibility);
        Self {
            order_id: projection.order_id,
            source: SdkOrderStatusSource::LocalEventStore,
            found,
            event_count: query_result.event_count,
            limit_applied: query_result.limit_applied,
            status: projection.status.into(),
            lifecycle_terminal: projection.lifecycle_terminal,
            listing_addr: projection.listing_addr,
            buyer_pubkey: projection.buyer_pubkey,
            seller_pubkey: projection.seller_pubkey,
            economics: projection.economics,
            evidence,
            eligibility,
            next_action,
            event_ids: query_result.event_ids,
            request_event_id: projection.request_event_id,
            decision_event_id: projection.decision_event_id,
            agreement_event_id: projection.agreement_event_id,
            pending_revision_event_id: projection.pending_revision_event_id,
            cancellation_event_id: projection.cancellation_event_id,
            last_event_id: projection.last_event_id,
            issues: projection.issues.into_iter().map(Into::into).collect(),
        }
    }
}

#[cfg(feature = "runtime")]
impl OrderStatusEvidenceSummary {
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
impl OrderStatusEligibility {
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
impl OrderStatusNextActionKind {
    fn from_projection(
        projection: &RadrootsOrderProjection,
        eligibility: &OrderStatusEligibility,
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

#[cfg(feature = "runtime")]
fn order_submit_plan(
    actor: &RadrootsActorContext,
    listing_event: RadrootsNostrEventPtr,
    order_request: RadrootsOrderRequest,
    created_at: RadrootsSdkTimestamp,
) -> Result<OrderSubmitPlan, RadrootsSdkError> {
    require_buyer_actor(actor, "order.prepare_submit")?;
    let listing_event_id = listing_event_id(&listing_event)?;
    let order_request =
        canonicalize_order_request_for_signer(order_request, actor.pubkey().as_str())
            .map_err(order_canonicalization_error)?;
    let created_at_nostr = created_at.try_into_nostr_created_at()?;
    let order_id = order_request.order_id.clone();
    let listing_addr = order_request.listing_addr.clone();
    let draft =
        order::build_order_request_draft(&listing_event, &order_request).map_err(|error| {
            RadrootsSdkError::InvalidRequest {
                message: format!("order submit draft encode failed: {error}"),
            }
        })?;
    let frozen_draft = to_frozen_draft(
        draft.into_wire_parts(),
        ORDER_REQUEST_CONTRACT_ID,
        order_request.buyer_pubkey.as_str(),
        created_at_nostr,
    )
    .expect("validated order submit draft freezes");
    let expected_event_id = RadrootsEventId::parse(frozen_draft.expected_event_id.as_str())
        .expect("frozen order submit draft produces a valid event id");
    Ok(OrderSubmitPlan {
        workflow: order_workflow_plan(
            OrderWorkflowKind::Submit,
            expected_event_id.clone(),
            created_at,
        ),
        order_id,
        listing_addr,
        listing_event_id,
        expected_event_id,
        frozen_draft,
        created_at,
    })
}

#[cfg(feature = "runtime")]
fn order_decision_plan(
    actor: &RadrootsActorContext,
    request_event: RadrootsNostrEventPtr,
    decision: RadrootsOrderDecision,
    created_at: RadrootsSdkTimestamp,
) -> Result<OrderDecisionPlan, RadrootsSdkError> {
    require_seller_actor(actor, "order.prepare_decision")?;
    let request_event_id = request_event_id(&request_event)?;
    if decision.seller_pubkey.as_str() != actor.pubkey().as_str() {
        return Err(RadrootsSdkError::UnauthorizedActor {
            operation: "order.prepare_decision".to_owned(),
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
        ORDER_DECISION_CONTRACT_ID,
        decision.seller_pubkey.as_str(),
        created_at_nostr,
    )
    .expect("validated order decision draft freezes");
    let expected_event_id = RadrootsEventId::parse(frozen_draft.expected_event_id.as_str())
        .expect("frozen order decision draft produces a valid event id");
    Ok(OrderDecisionPlan {
        workflow: order_workflow_plan(
            OrderWorkflowKind::Decision,
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

#[cfg(feature = "runtime")]
fn order_revision_proposal_plan(
    actor: &RadrootsActorContext,
    root_event: RadrootsNostrEventPtr,
    previous_event: RadrootsNostrEventPtr,
    proposal: RadrootsOrderRevisionProposal,
    created_at: RadrootsSdkTimestamp,
) -> Result<OrderRevisionProposalPlan, RadrootsSdkError> {
    require_seller_actor(actor, "order.prepare_revision_proposal")?;
    let root_event_id = order_reference_event_id(&root_event, "root")?;
    let previous_event_id = order_reference_event_id(&previous_event, "previous")?;
    if proposal.seller_pubkey.as_str() != actor.pubkey().as_str() {
        return Err(RadrootsSdkError::UnauthorizedActor {
            operation: "order.prepare_revision_proposal".to_owned(),
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
        ORDER_REVISION_PROPOSAL_CONTRACT_ID,
        seller_pubkey.as_str(),
        created_at_nostr,
        "order revision proposal",
    );
    Ok(OrderRevisionProposalPlan {
        workflow: order_workflow_plan(
            OrderWorkflowKind::RevisionProposal,
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

#[cfg(feature = "runtime")]
fn order_revision_decision_plan(
    actor: &RadrootsActorContext,
    root_event: RadrootsNostrEventPtr,
    previous_event: RadrootsNostrEventPtr,
    decision: RadrootsOrderRevisionDecision,
    created_at: RadrootsSdkTimestamp,
) -> Result<OrderRevisionDecisionPlan, RadrootsSdkError> {
    require_buyer_actor(actor, "order.prepare_revision_decision")?;
    let root_event_id = order_reference_event_id(&root_event, "root")?;
    let previous_event_id = order_reference_event_id(&previous_event, "previous")?;
    if decision.buyer_pubkey.as_str() != actor.pubkey().as_str() {
        return Err(RadrootsSdkError::UnauthorizedActor {
            operation: "order.prepare_revision_decision".to_owned(),
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
        ORDER_REVISION_DECISION_CONTRACT_ID,
        buyer_pubkey.as_str(),
        created_at_nostr,
        "order revision decision",
    );
    Ok(OrderRevisionDecisionPlan {
        workflow: order_workflow_plan(
            OrderWorkflowKind::RevisionDecision,
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

#[cfg(feature = "runtime")]
fn order_cancellation_plan(
    actor: &RadrootsActorContext,
    root_event: RadrootsNostrEventPtr,
    previous_event: RadrootsNostrEventPtr,
    cancellation: RadrootsOrderCancellation,
    created_at: RadrootsSdkTimestamp,
) -> Result<OrderCancellationPlan, RadrootsSdkError> {
    require_buyer_actor(actor, "order.prepare_cancellation")?;
    let root_event_id = order_reference_event_id(&root_event, "root")?;
    let previous_event_id = order_reference_event_id(&previous_event, "previous")?;
    if cancellation.buyer_pubkey.as_str() != actor.pubkey().as_str() {
        return Err(RadrootsSdkError::UnauthorizedActor {
            operation: "order.prepare_cancellation".to_owned(),
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
        ORDER_CANCELLATION_CONTRACT_ID,
        buyer_pubkey.as_str(),
        created_at_nostr,
        "order cancellation",
    );
    Ok(OrderCancellationPlan {
        workflow: order_workflow_plan(
            OrderWorkflowKind::Cancellation,
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

#[cfg(feature = "runtime")]
fn order_workflow_plan(
    kind: OrderWorkflowKind,
    expected_event_id: RadrootsEventId,
    created_at: RadrootsSdkTimestamp,
) -> OrderWorkflowPlan {
    OrderWorkflowPlan {
        kind,
        operation_kind: kind.operation_kind(),
        contract_id: kind.contract_id(),
        expected_event_id,
        created_at,
    }
}

#[cfg(feature = "runtime")]
fn order_submit_receipt(
    plan: OrderSubmitPlan,
    enqueue: crate::workflow_runtime::SdkWorkflowEnqueueReceipt,
) -> OrderSubmitReceipt {
    OrderSubmitReceipt {
        workflow: order_workflow_enqueue_receipt(
            OrderWorkflowKind::Submit,
            plan.expected_event_id.clone(),
            &enqueue,
        ),
        order_id: plan.order_id,
        listing_addr: plan.listing_addr,
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

#[cfg(feature = "runtime")]
fn order_decision_receipt(
    plan: OrderDecisionPlan,
    enqueue: crate::workflow_runtime::SdkWorkflowEnqueueReceipt,
) -> OrderDecisionReceipt {
    OrderDecisionReceipt {
        workflow: order_workflow_enqueue_receipt(
            OrderWorkflowKind::Decision,
            plan.expected_event_id.clone(),
            &enqueue,
        ),
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

#[cfg(feature = "runtime")]
fn order_revision_proposal_receipt(
    plan: OrderRevisionProposalPlan,
    enqueue: crate::workflow_runtime::SdkWorkflowEnqueueReceipt,
) -> OrderRevisionProposalReceipt {
    OrderRevisionProposalReceipt {
        workflow: order_workflow_enqueue_receipt(
            OrderWorkflowKind::RevisionProposal,
            plan.expected_event_id.clone(),
            &enqueue,
        ),
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

#[cfg(feature = "runtime")]
fn order_revision_decision_receipt(
    plan: OrderRevisionDecisionPlan,
    enqueue: crate::workflow_runtime::SdkWorkflowEnqueueReceipt,
) -> OrderRevisionDecisionReceipt {
    OrderRevisionDecisionReceipt {
        workflow: order_workflow_enqueue_receipt(
            OrderWorkflowKind::RevisionDecision,
            plan.expected_event_id.clone(),
            &enqueue,
        ),
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

#[cfg(feature = "runtime")]
fn order_cancellation_receipt(
    plan: OrderCancellationPlan,
    enqueue: crate::workflow_runtime::SdkWorkflowEnqueueReceipt,
) -> OrderCancellationReceipt {
    OrderCancellationReceipt {
        workflow: order_workflow_enqueue_receipt(
            OrderWorkflowKind::Cancellation,
            plan.expected_event_id.clone(),
            &enqueue,
        ),
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

#[cfg(feature = "runtime")]
fn order_workflow_enqueue_receipt(
    kind: OrderWorkflowKind,
    expected_event_id: RadrootsEventId,
    enqueue: &crate::workflow_runtime::SdkWorkflowEnqueueReceipt,
) -> OrderWorkflowEnqueueReceipt {
    let state = SdkMutationState::from(enqueue.state);
    let digest_prefix = Some(enqueue.idempotency_digest_prefix.clone());
    let safe_retry_same_key = true;
    let replayed_existing_operation = state == SdkMutationState::AlreadyQueued;
    OrderWorkflowEnqueueReceipt {
        kind,
        operation_kind: kind.operation_kind(),
        expected_event_id,
        signed_event_id: enqueue.signed_event_id.clone(),
        local_event_seq: enqueue.local_event_seq,
        outbox_operation_id: enqueue.outbox_operation_id,
        outbox_event_id: enqueue.outbox_event_id,
        state,
        idempotency_digest_prefix: digest_prefix.clone(),
        idempotency: OrderWorkflowIdempotencyReceipt {
            digest_prefix,
            replayed_existing_operation,
            safe_to_retry_with_same_idempotency_key: safe_retry_same_key,
        },
        retry: OrderWorkflowRetryAdvice {
            retryable_after_error: false,
            safe_to_retry_enqueue_with_same_idempotency_key: safe_retry_same_key,
            recovery_actions: Vec::new(),
        },
    }
}

#[cfg(feature = "runtime")]
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

#[cfg(feature = "runtime")]
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

#[cfg(feature = "runtime")]
trait OrderPayloadValidate {
    fn validate_order_payload(
        &self,
    ) -> Result<(), radroots_events::order::RadrootsOrderPayloadError>;
}

#[cfg(feature = "runtime")]
impl OrderPayloadValidate for RadrootsOrderDecision {
    fn validate_order_payload(
        &self,
    ) -> Result<(), radroots_events::order::RadrootsOrderPayloadError> {
        self.validate()
    }
}

#[cfg(feature = "runtime")]
impl OrderPayloadValidate for RadrootsOrderRevisionProposal {
    fn validate_order_payload(
        &self,
    ) -> Result<(), radroots_events::order::RadrootsOrderPayloadError> {
        self.validate()
    }
}

#[cfg(feature = "runtime")]
impl OrderPayloadValidate for RadrootsOrderRevisionDecision {
    fn validate_order_payload(
        &self,
    ) -> Result<(), radroots_events::order::RadrootsOrderPayloadError> {
        self.validate()
    }
}

#[cfg(feature = "runtime")]
impl OrderPayloadValidate for RadrootsOrderCancellation {
    fn validate_order_payload(
        &self,
    ) -> Result<(), radroots_events::order::RadrootsOrderPayloadError> {
        self.validate()
    }
}

#[cfg(feature = "runtime")]
struct OrderRequestEvidence {
    order_id: RadrootsOrderId,
    listing_addr: RadrootsListingAddress,
    buyer_pubkey: RadrootsPublicKey,
    seller_pubkey: RadrootsPublicKey,
    request_event_id: RadrootsEventId,
}

#[cfg(feature = "runtime")]
fn parse_order_request_evidence(
    event: &RadrootsNostrEvent,
) -> Result<OrderRequestEvidence, RadrootsSdkError> {
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
    Ok(OrderRequestEvidence {
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

#[cfg(feature = "runtime")]
fn require_decision_request_evidence(
    plan: &OrderDecisionPlan,
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

#[cfg(feature = "runtime")]
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

#[cfg(feature = "runtime")]
fn require_revision_proposal_state(
    plan: &OrderRevisionProposalPlan,
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

#[cfg(feature = "runtime")]
fn require_revision_decision_state(
    plan: &OrderRevisionDecisionPlan,
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

#[cfg(feature = "runtime")]
fn require_cancellation_state(
    plan: &OrderCancellationPlan,
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

#[cfg(feature = "runtime")]
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

#[cfg(feature = "runtime")]
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

#[cfg(feature = "runtime")]
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

#[cfg(feature = "runtime")]
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

#[cfg(feature = "runtime")]
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

#[cfg(feature = "runtime")]
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

#[cfg(feature = "runtime")]
fn lifecycle_invalid(
    operation: &'static str,
    order_id: &RadrootsOrderId,
    reason: impl Into<String>,
) -> RadrootsSdkError {
    RadrootsSdkError::InvalidRequest {
        message: format!("{operation} for order {order_id} {}", reason.into()),
    }
}

#[cfg(feature = "runtime")]
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

#[cfg(feature = "runtime")]
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

#[cfg(feature = "runtime")]
fn listing_event_id(
    listing_event: &RadrootsNostrEventPtr,
) -> Result<RadrootsEventId, RadrootsSdkError> {
    RadrootsEventId::parse(listing_event.id.as_str()).map_err(|error| {
        RadrootsSdkError::InvalidRequest {
            message: format!("listing evidence event id is invalid: {error}"),
        }
    })
}

#[cfg(feature = "runtime")]
fn request_event_id(
    request_event: &RadrootsNostrEventPtr,
) -> Result<RadrootsEventId, RadrootsSdkError> {
    RadrootsEventId::parse(request_event.id.as_str()).map_err(|error| {
        RadrootsSdkError::InvalidRequest {
            message: format!("order request evidence event id is invalid: {error}"),
        }
    })
}

#[cfg(feature = "runtime")]
fn order_reference_event_id(
    event: &RadrootsNostrEventPtr,
    label: &'static str,
) -> Result<RadrootsEventId, RadrootsSdkError> {
    RadrootsEventId::parse(event.id.as_str()).map_err(|error| RadrootsSdkError::InvalidRequest {
        message: format!("order {label} evidence event id is invalid: {error}"),
    })
}

#[cfg(feature = "runtime")]
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

#[cfg(feature = "runtime")]
fn order_canonicalization_error(error: RadrootsOrderCanonicalizationError) -> RadrootsSdkError {
    match error {
        RadrootsOrderCanonicalizationError::InvalidBuyerSigner => {
            RadrootsSdkError::UnauthorizedActor {
                operation: "order.prepare_submit".to_owned(),
                reason: "actor pubkey must match order buyer_pubkey".to_owned(),
            }
        }
        error => RadrootsSdkError::InvalidRequest {
            message: format!("order submit request is invalid: {error}"),
        },
    }
}

#[cfg(feature = "runtime")]
fn order_decision_canonicalization_error(
    error: RadrootsOrderCanonicalizationError,
) -> RadrootsSdkError {
    RadrootsSdkError::InvalidRequest {
        message: format!("order decision request is invalid: {error}"),
    }
}

#[cfg(feature = "runtime")]
impl From<RadrootsTradeWorkflowState> for OrderStatusKind {
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
impl From<RadrootsOrderIssue> for SdkOrderStatusIssue {
    fn from(issue: RadrootsOrderIssue) -> Self {
        match issue {
            RadrootsOrderIssue::MissingRequest => {
                Self::new(SdkOrderStatusIssueKind::MissingRequest, Vec::new())
            }
            RadrootsOrderIssue::MultipleRequests { event_ids } => {
                Self::new(SdkOrderStatusIssueKind::MultipleRequests, event_ids)
            }
            RadrootsOrderIssue::RequestPayloadInvalid { event_id } => {
                Self::single(SdkOrderStatusIssueKind::RequestPayloadInvalid, event_id)
            }
            RadrootsOrderIssue::RequestOrderIdMismatch { event_id } => {
                Self::single(SdkOrderStatusIssueKind::RequestOrderIdMismatch, event_id)
            }
            RadrootsOrderIssue::RequestAuthorMismatch { event_id } => {
                Self::single(SdkOrderStatusIssueKind::RequestAuthorMismatch, event_id)
            }
            RadrootsOrderIssue::RequestListingAddressInvalid { event_id } => Self::single(
                SdkOrderStatusIssueKind::RequestListingAddressInvalid,
                event_id,
            ),
            RadrootsOrderIssue::RequestSellerListingMismatch { event_id } => Self::single(
                SdkOrderStatusIssueKind::RequestSellerListingMismatch,
                event_id,
            ),
            RadrootsOrderIssue::DecisionPayloadInvalid { event_id } => {
                Self::single(SdkOrderStatusIssueKind::DecisionPayloadInvalid, event_id)
            }
            RadrootsOrderIssue::DecisionOrderIdMismatch { event_id } => {
                Self::single(SdkOrderStatusIssueKind::DecisionOrderIdMismatch, event_id)
            }
            RadrootsOrderIssue::DecisionAuthorMismatch { event_id } => {
                Self::single(SdkOrderStatusIssueKind::DecisionAuthorMismatch, event_id)
            }
            RadrootsOrderIssue::DecisionCounterpartyMismatch { event_id } => Self::single(
                SdkOrderStatusIssueKind::DecisionCounterpartyMismatch,
                event_id,
            ),
            RadrootsOrderIssue::DecisionBuyerMismatch { event_id } => {
                Self::single(SdkOrderStatusIssueKind::DecisionBuyerMismatch, event_id)
            }
            RadrootsOrderIssue::DecisionSellerMismatch { event_id } => {
                Self::single(SdkOrderStatusIssueKind::DecisionSellerMismatch, event_id)
            }
            RadrootsOrderIssue::DecisionListingAddressInvalid { event_id } => Self::single(
                SdkOrderStatusIssueKind::DecisionListingAddressInvalid,
                event_id,
            ),
            RadrootsOrderIssue::DecisionListingMismatch { event_id } => {
                Self::single(SdkOrderStatusIssueKind::DecisionListingMismatch, event_id)
            }
            RadrootsOrderIssue::DecisionRootMismatch { event_id } => {
                Self::single(SdkOrderStatusIssueKind::DecisionRootMismatch, event_id)
            }
            RadrootsOrderIssue::DecisionPreviousMismatch { event_id } => {
                Self::single(SdkOrderStatusIssueKind::DecisionPreviousMismatch, event_id)
            }
            RadrootsOrderIssue::DecisionMissingInventoryCommitments { event_id } => Self::single(
                SdkOrderStatusIssueKind::DecisionMissingInventoryCommitments,
                event_id,
            ),
            RadrootsOrderIssue::DecisionInventoryCommitmentMismatch { event_id } => Self::single(
                SdkOrderStatusIssueKind::DecisionInventoryCommitmentMismatch,
                event_id,
            ),
            RadrootsOrderIssue::DecisionMissingReason { event_id } => {
                Self::single(SdkOrderStatusIssueKind::DecisionMissingReason, event_id)
            }
            RadrootsOrderIssue::ConflictingDecisions { event_ids } => {
                Self::new(SdkOrderStatusIssueKind::ConflictingDecisions, event_ids)
            }
            RadrootsOrderIssue::RevisionProposalPayloadInvalid { event_id } => Self::single(
                SdkOrderStatusIssueKind::RevisionProposalPayloadInvalid,
                event_id,
            ),
            RadrootsOrderIssue::RevisionProposalOrderIdMismatch { event_id } => Self::single(
                SdkOrderStatusIssueKind::RevisionProposalOrderIdMismatch,
                event_id,
            ),
            RadrootsOrderIssue::RevisionProposalAuthorMismatch { event_id } => Self::single(
                SdkOrderStatusIssueKind::RevisionProposalAuthorMismatch,
                event_id,
            ),
            RadrootsOrderIssue::RevisionProposalCounterpartyMismatch { event_id } => Self::single(
                SdkOrderStatusIssueKind::RevisionProposalCounterpartyMismatch,
                event_id,
            ),
            RadrootsOrderIssue::RevisionProposalBuyerMismatch { event_id } => Self::single(
                SdkOrderStatusIssueKind::RevisionProposalBuyerMismatch,
                event_id,
            ),
            RadrootsOrderIssue::RevisionProposalSellerMismatch { event_id } => Self::single(
                SdkOrderStatusIssueKind::RevisionProposalSellerMismatch,
                event_id,
            ),
            RadrootsOrderIssue::RevisionProposalListingAddressInvalid { event_id } => Self::single(
                SdkOrderStatusIssueKind::RevisionProposalListingAddressInvalid,
                event_id,
            ),
            RadrootsOrderIssue::RevisionProposalListingMismatch { event_id } => Self::single(
                SdkOrderStatusIssueKind::RevisionProposalListingMismatch,
                event_id,
            ),
            RadrootsOrderIssue::RevisionProposalRootMismatch { event_id } => Self::single(
                SdkOrderStatusIssueKind::RevisionProposalRootMismatch,
                event_id,
            ),
            RadrootsOrderIssue::RevisionProposalPreviousMismatch { event_id } => Self::single(
                SdkOrderStatusIssueKind::RevisionProposalPreviousMismatch,
                event_id,
            ),
            RadrootsOrderIssue::RevisionDecisionWithoutProposal { event_id } => Self::single(
                SdkOrderStatusIssueKind::RevisionDecisionWithoutProposal,
                event_id,
            ),
            RadrootsOrderIssue::RevisionDecisionPayloadInvalid { event_id } => Self::single(
                SdkOrderStatusIssueKind::RevisionDecisionPayloadInvalid,
                event_id,
            ),
            RadrootsOrderIssue::RevisionDecisionOrderIdMismatch { event_id } => Self::single(
                SdkOrderStatusIssueKind::RevisionDecisionOrderIdMismatch,
                event_id,
            ),
            RadrootsOrderIssue::RevisionDecisionAuthorMismatch { event_id } => Self::single(
                SdkOrderStatusIssueKind::RevisionDecisionAuthorMismatch,
                event_id,
            ),
            RadrootsOrderIssue::RevisionDecisionCounterpartyMismatch { event_id } => Self::single(
                SdkOrderStatusIssueKind::RevisionDecisionCounterpartyMismatch,
                event_id,
            ),
            RadrootsOrderIssue::RevisionDecisionBuyerMismatch { event_id } => Self::single(
                SdkOrderStatusIssueKind::RevisionDecisionBuyerMismatch,
                event_id,
            ),
            RadrootsOrderIssue::RevisionDecisionSellerMismatch { event_id } => Self::single(
                SdkOrderStatusIssueKind::RevisionDecisionSellerMismatch,
                event_id,
            ),
            RadrootsOrderIssue::RevisionDecisionListingAddressInvalid { event_id } => Self::single(
                SdkOrderStatusIssueKind::RevisionDecisionListingAddressInvalid,
                event_id,
            ),
            RadrootsOrderIssue::RevisionDecisionListingMismatch { event_id } => Self::single(
                SdkOrderStatusIssueKind::RevisionDecisionListingMismatch,
                event_id,
            ),
            RadrootsOrderIssue::RevisionDecisionRootMismatch { event_id } => Self::single(
                SdkOrderStatusIssueKind::RevisionDecisionRootMismatch,
                event_id,
            ),
            RadrootsOrderIssue::RevisionDecisionPreviousMismatch { event_id } => Self::single(
                SdkOrderStatusIssueKind::RevisionDecisionPreviousMismatch,
                event_id,
            ),
            RadrootsOrderIssue::RevisionDecisionRevisionIdMismatch { event_id } => Self::single(
                SdkOrderStatusIssueKind::RevisionDecisionRevisionIdMismatch,
                event_id,
            ),
            RadrootsOrderIssue::CancellationWithoutCancellableOrder { event_id } => Self::single(
                SdkOrderStatusIssueKind::CancellationWithoutCancellableOrder,
                event_id,
            ),
            RadrootsOrderIssue::CancellationPayloadInvalid { event_id } => Self::single(
                SdkOrderStatusIssueKind::CancellationPayloadInvalid,
                event_id,
            ),
            RadrootsOrderIssue::CancellationOrderIdMismatch { event_id } => Self::single(
                SdkOrderStatusIssueKind::CancellationOrderIdMismatch,
                event_id,
            ),
            RadrootsOrderIssue::CancellationAuthorMismatch { event_id } => Self::single(
                SdkOrderStatusIssueKind::CancellationAuthorMismatch,
                event_id,
            ),
            RadrootsOrderIssue::CancellationCounterpartyMismatch { event_id } => Self::single(
                SdkOrderStatusIssueKind::CancellationCounterpartyMismatch,
                event_id,
            ),
            RadrootsOrderIssue::CancellationBuyerMismatch { event_id } => {
                Self::single(SdkOrderStatusIssueKind::CancellationBuyerMismatch, event_id)
            }
            RadrootsOrderIssue::CancellationSellerMismatch { event_id } => Self::single(
                SdkOrderStatusIssueKind::CancellationSellerMismatch,
                event_id,
            ),
            RadrootsOrderIssue::CancellationListingAddressInvalid { event_id } => Self::single(
                SdkOrderStatusIssueKind::CancellationListingAddressInvalid,
                event_id,
            ),
            RadrootsOrderIssue::CancellationListingMismatch { event_id } => Self::single(
                SdkOrderStatusIssueKind::CancellationListingMismatch,
                event_id,
            ),
            RadrootsOrderIssue::CancellationRootMismatch { event_id } => {
                Self::single(SdkOrderStatusIssueKind::CancellationRootMismatch, event_id)
            }
            RadrootsOrderIssue::CancellationPreviousMismatch { event_id } => Self::single(
                SdkOrderStatusIssueKind::CancellationPreviousMismatch,
                event_id,
            ),
            RadrootsOrderIssue::ForkedLifecycle { event_ids } => {
                Self::new(SdkOrderStatusIssueKind::ForkedLifecycle, event_ids)
            }
            RadrootsOrderIssue::ValidationReceiptWithoutPendingAgreement { event_id } => {
                Self::single(
                    SdkOrderStatusIssueKind::ValidationReceiptWithoutPendingAgreement,
                    event_id,
                )
            }
            RadrootsOrderIssue::ValidationReceiptOrderIdMismatch { event_id } => Self::single(
                SdkOrderStatusIssueKind::ValidationReceiptOrderIdMismatch,
                event_id,
            ),
            RadrootsOrderIssue::ValidationReceiptTypeMismatch { event_id } => Self::single(
                SdkOrderStatusIssueKind::ValidationReceiptTypeMismatch,
                event_id,
            ),
            RadrootsOrderIssue::ValidationReceiptRootMismatch { event_id } => Self::single(
                SdkOrderStatusIssueKind::ValidationReceiptRootMismatch,
                event_id,
            ),
            RadrootsOrderIssue::ValidationReceiptTargetMismatch { event_id } => Self::single(
                SdkOrderStatusIssueKind::ValidationReceiptTargetMismatch,
                event_id,
            ),
            RadrootsOrderIssue::ValidationReceiptListingMismatch { event_id } => Self::single(
                SdkOrderStatusIssueKind::ValidationReceiptListingMismatch,
                event_id,
            ),
            RadrootsOrderIssue::ConflictingValidationReceipts { event_ids } => Self::new(
                SdkOrderStatusIssueKind::ConflictingValidationReceipts,
                event_ids,
            ),
            RadrootsOrderIssue::DeterministicValidationFailure { event_id, .. } => Self::single(
                SdkOrderStatusIssueKind::DeterministicValidationFailure,
                event_id,
            ),
            RadrootsOrderIssue::StaleListingEvent {
                expected_event_id,
                current_event_id,
            } => Self::new(
                SdkOrderStatusIssueKind::StaleListingEvent,
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
