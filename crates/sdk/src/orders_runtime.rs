#[cfg(feature = "runtime")]
use crate::{
    OrdersClient, RadrootsSdkError, RadrootsSdkTimestamp, SdkIdempotencyKey, SdkMutationState,
    SdkRelayTargetPolicy, SdkRelayUrlPolicy,
    actor_json::SdkActorContextJson,
    order,
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
        KIND_ORDER_CANCELLATION, KIND_ORDER_DECISION, KIND_ORDER_FULFILLMENT_UPDATE,
        KIND_ORDER_PAYMENT_RECORD, KIND_ORDER_RECEIPT, KIND_ORDER_REQUEST,
        KIND_ORDER_REVISION_DECISION, KIND_ORDER_REVISION_PROPOSAL, KIND_ORDER_SETTLEMENT_DECISION,
    },
    order::{
        RadrootsOrderCancellation, RadrootsOrderDecision, RadrootsOrderFulfillmentState,
        RadrootsOrderFulfillmentUpdate, RadrootsOrderReceipt, RadrootsOrderRequest,
        RadrootsOrderRevisionDecision, RadrootsOrderRevisionProposal,
    },
};
#[cfg(feature = "runtime")]
use radroots_events_codec::order::{
    order_cancellation_from_event, order_decision_from_event, order_fulfillment_update_from_event,
    order_payment_record_from_event, order_receipt_from_event, order_request_from_event,
    order_revision_decision_from_event, order_revision_proposal_from_event,
    order_settlement_decision_from_event,
};
#[cfg(feature = "runtime")]
use radroots_events_codec::wire::{WireEventParts, to_frozen_draft};
#[cfg(feature = "runtime")]
use radroots_trade::order::{
    RadrootsOrderCanonicalizationError, RadrootsOrderIssue, RadrootsOrderPaymentState,
    RadrootsOrderProjection, RadrootsOrderProjectionQueryResult, RadrootsOrderSettlementState,
    RadrootsOrderStatus, RadrootsOrderStoreQueryError, canonicalize_order_decision_for_signer,
    canonicalize_order_request_for_signer, order_projection_query_for_order_id,
};
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
pub const ORDER_FULFILLMENT_UPDATE_OPERATION_KIND: &str = "order.fulfillment.update.v1";
#[cfg(feature = "runtime")]
pub const ORDER_RECEIPT_RECORD_OPERATION_KIND: &str = "order.receipt.record.v1";

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
const ORDER_FULFILLMENT_UPDATE_CONTRACT_ID: &str = "radroots.order.fulfillment_update.v1";
#[cfg(feature = "runtime")]
const ORDER_RECEIPT_CONTRACT_ID: &str = "radroots.order.receipt.v1";

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
    FulfillmentUpdate,
    ReceiptRecord,
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
            Self::FulfillmentUpdate => ORDER_FULFILLMENT_UPDATE_OPERATION_KIND,
            Self::ReceiptRecord => ORDER_RECEIPT_RECORD_OPERATION_KIND,
        }
    }

    pub fn contract_id(self) -> &'static str {
        match self {
            Self::Submit => ORDER_REQUEST_CONTRACT_ID,
            Self::Decision => ORDER_DECISION_CONTRACT_ID,
            Self::RevisionProposal => ORDER_REVISION_PROPOSAL_CONTRACT_ID,
            Self::RevisionDecision => ORDER_REVISION_DECISION_CONTRACT_ID,
            Self::Cancellation => ORDER_CANCELLATION_CONTRACT_ID,
            Self::FulfillmentUpdate => ORDER_FULFILLMENT_UPDATE_CONTRACT_ID,
            Self::ReceiptRecord => ORDER_RECEIPT_CONTRACT_ID,
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
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct OrderSubmitPrepareRequest {
    pub actor: RadrootsActorContext,
    pub listing_event: RadrootsNostrEventPtr,
    pub order: RadrootsOrderRequest,
    pub created_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(feature = "runtime")]
impl serde::Serialize for OrderSubmitPrepareRequest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("OrderSubmitPrepareRequest", 4)?;
        state.serialize_field("actor", &SdkActorContextJson(&self.actor))?;
        state.serialize_field("listing_event", &self.listing_event)?;
        state.serialize_field("order", &self.order)?;
        state.serialize_field("created_at", &self.created_at)?;
        state.end()
    }
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
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct OrderSubmitEnqueueRequest {
    pub actor: RadrootsActorContext,
    pub listing_event: RadrootsNostrEventPtr,
    pub order: RadrootsOrderRequest,
    pub target_relays: SdkRelayTargetPolicy,
    pub idempotency_key: Option<SdkIdempotencyKey>,
    pub created_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(feature = "runtime")]
impl serde::Serialize for OrderSubmitEnqueueRequest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("OrderSubmitEnqueueRequest", 6)?;
        state.serialize_field("actor", &SdkActorContextJson(&self.actor))?;
        state.serialize_field("listing_event", &self.listing_event)?;
        state.serialize_field("order", &self.order)?;
        state.serialize_field("target_relays", &self.target_relays)?;
        state.serialize_field("idempotency_key", &self.idempotency_key)?;
        state.serialize_field("created_at", &self.created_at)?;
        state.end()
    }
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
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct OrderRequestEvidenceIngestRequest {
    pub event: RadrootsNostrEvent,
    pub observed_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(feature = "runtime")]
impl serde::Serialize for OrderRequestEvidenceIngestRequest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("OrderRequestEvidenceIngestRequest", 2)?;
        state.serialize_field("event", &self.event)?;
        state.serialize_field("observed_at", &self.observed_at)?;
        state.end()
    }
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
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct OrderEvidenceIngestRequest {
    pub event: RadrootsNostrEvent,
    pub observed_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(feature = "runtime")]
impl serde::Serialize for OrderEvidenceIngestRequest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("OrderEvidenceIngestRequest", 2)?;
        state.serialize_field("event", &self.event)?;
        state.serialize_field("observed_at", &self.observed_at)?;
        state.end()
    }
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
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct OrderDecisionPrepareRequest {
    pub actor: RadrootsActorContext,
    pub request_event: RadrootsNostrEventPtr,
    pub decision: RadrootsOrderDecision,
    pub created_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(feature = "runtime")]
impl serde::Serialize for OrderDecisionPrepareRequest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("OrderDecisionPrepareRequest", 4)?;
        state.serialize_field("actor", &SdkActorContextJson(&self.actor))?;
        state.serialize_field("request_event", &self.request_event)?;
        state.serialize_field("decision", &self.decision)?;
        state.serialize_field("created_at", &self.created_at)?;
        state.end()
    }
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
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct OrderDecisionEnqueueRequest {
    pub actor: RadrootsActorContext,
    pub request_event: RadrootsNostrEventPtr,
    pub decision: RadrootsOrderDecision,
    pub target_relays: SdkRelayTargetPolicy,
    pub idempotency_key: Option<SdkIdempotencyKey>,
    pub created_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(feature = "runtime")]
impl serde::Serialize for OrderDecisionEnqueueRequest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("OrderDecisionEnqueueRequest", 6)?;
        state.serialize_field("actor", &SdkActorContextJson(&self.actor))?;
        state.serialize_field("request_event", &self.request_event)?;
        state.serialize_field("decision", &self.decision)?;
        state.serialize_field("target_relays", &self.target_relays)?;
        state.serialize_field("idempotency_key", &self.idempotency_key)?;
        state.serialize_field("created_at", &self.created_at)?;
        state.end()
    }
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
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct OrderRevisionProposalPrepareRequest {
    pub actor: RadrootsActorContext,
    pub root_event: RadrootsNostrEventPtr,
    pub previous_event: RadrootsNostrEventPtr,
    pub proposal: RadrootsOrderRevisionProposal,
    pub created_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(feature = "runtime")]
impl serde::Serialize for OrderRevisionProposalPrepareRequest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("OrderRevisionProposalPrepareRequest", 5)?;
        state.serialize_field("actor", &SdkActorContextJson(&self.actor))?;
        state.serialize_field("root_event", &self.root_event)?;
        state.serialize_field("previous_event", &self.previous_event)?;
        state.serialize_field("proposal", &self.proposal)?;
        state.serialize_field("created_at", &self.created_at)?;
        state.end()
    }
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
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct OrderRevisionProposalEnqueueRequest {
    pub actor: RadrootsActorContext,
    pub root_event: RadrootsNostrEventPtr,
    pub previous_event: RadrootsNostrEventPtr,
    pub proposal: RadrootsOrderRevisionProposal,
    pub target_relays: SdkRelayTargetPolicy,
    pub idempotency_key: Option<SdkIdempotencyKey>,
    pub created_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(feature = "runtime")]
impl serde::Serialize for OrderRevisionProposalEnqueueRequest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("OrderRevisionProposalEnqueueRequest", 7)?;
        state.serialize_field("actor", &SdkActorContextJson(&self.actor))?;
        state.serialize_field("root_event", &self.root_event)?;
        state.serialize_field("previous_event", &self.previous_event)?;
        state.serialize_field("proposal", &self.proposal)?;
        state.serialize_field("target_relays", &self.target_relays)?;
        state.serialize_field("idempotency_key", &self.idempotency_key)?;
        state.serialize_field("created_at", &self.created_at)?;
        state.end()
    }
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
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct OrderRevisionDecisionPrepareRequest {
    pub actor: RadrootsActorContext,
    pub root_event: RadrootsNostrEventPtr,
    pub previous_event: RadrootsNostrEventPtr,
    pub decision: RadrootsOrderRevisionDecision,
    pub created_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(feature = "runtime")]
impl serde::Serialize for OrderRevisionDecisionPrepareRequest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("OrderRevisionDecisionPrepareRequest", 5)?;
        state.serialize_field("actor", &SdkActorContextJson(&self.actor))?;
        state.serialize_field("root_event", &self.root_event)?;
        state.serialize_field("previous_event", &self.previous_event)?;
        state.serialize_field("decision", &self.decision)?;
        state.serialize_field("created_at", &self.created_at)?;
        state.end()
    }
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
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct OrderRevisionDecisionEnqueueRequest {
    pub actor: RadrootsActorContext,
    pub root_event: RadrootsNostrEventPtr,
    pub previous_event: RadrootsNostrEventPtr,
    pub decision: RadrootsOrderRevisionDecision,
    pub target_relays: SdkRelayTargetPolicy,
    pub idempotency_key: Option<SdkIdempotencyKey>,
    pub created_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(feature = "runtime")]
impl serde::Serialize for OrderRevisionDecisionEnqueueRequest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("OrderRevisionDecisionEnqueueRequest", 7)?;
        state.serialize_field("actor", &SdkActorContextJson(&self.actor))?;
        state.serialize_field("root_event", &self.root_event)?;
        state.serialize_field("previous_event", &self.previous_event)?;
        state.serialize_field("decision", &self.decision)?;
        state.serialize_field("target_relays", &self.target_relays)?;
        state.serialize_field("idempotency_key", &self.idempotency_key)?;
        state.serialize_field("created_at", &self.created_at)?;
        state.end()
    }
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
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct OrderCancellationPrepareRequest {
    pub actor: RadrootsActorContext,
    pub root_event: RadrootsNostrEventPtr,
    pub previous_event: RadrootsNostrEventPtr,
    pub cancellation: RadrootsOrderCancellation,
    pub created_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(feature = "runtime")]
impl serde::Serialize for OrderCancellationPrepareRequest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("OrderCancellationPrepareRequest", 5)?;
        state.serialize_field("actor", &SdkActorContextJson(&self.actor))?;
        state.serialize_field("root_event", &self.root_event)?;
        state.serialize_field("previous_event", &self.previous_event)?;
        state.serialize_field("cancellation", &self.cancellation)?;
        state.serialize_field("created_at", &self.created_at)?;
        state.end()
    }
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
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct OrderCancellationEnqueueRequest {
    pub actor: RadrootsActorContext,
    pub root_event: RadrootsNostrEventPtr,
    pub previous_event: RadrootsNostrEventPtr,
    pub cancellation: RadrootsOrderCancellation,
    pub target_relays: SdkRelayTargetPolicy,
    pub idempotency_key: Option<SdkIdempotencyKey>,
    pub created_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(feature = "runtime")]
impl serde::Serialize for OrderCancellationEnqueueRequest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("OrderCancellationEnqueueRequest", 7)?;
        state.serialize_field("actor", &SdkActorContextJson(&self.actor))?;
        state.serialize_field("root_event", &self.root_event)?;
        state.serialize_field("previous_event", &self.previous_event)?;
        state.serialize_field("cancellation", &self.cancellation)?;
        state.serialize_field("target_relays", &self.target_relays)?;
        state.serialize_field("idempotency_key", &self.idempotency_key)?;
        state.serialize_field("created_at", &self.created_at)?;
        state.end()
    }
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
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct OrderFulfillmentUpdatePrepareRequest {
    pub actor: RadrootsActorContext,
    pub root_event: RadrootsNostrEventPtr,
    pub previous_event: RadrootsNostrEventPtr,
    pub fulfillment: RadrootsOrderFulfillmentUpdate,
    pub created_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(feature = "runtime")]
impl serde::Serialize for OrderFulfillmentUpdatePrepareRequest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("OrderFulfillmentUpdatePrepareRequest", 5)?;
        state.serialize_field("actor", &SdkActorContextJson(&self.actor))?;
        state.serialize_field("root_event", &self.root_event)?;
        state.serialize_field("previous_event", &self.previous_event)?;
        state.serialize_field("fulfillment", &self.fulfillment)?;
        state.serialize_field("created_at", &self.created_at)?;
        state.end()
    }
}

#[cfg(feature = "runtime")]
impl OrderFulfillmentUpdatePrepareRequest {
    pub fn new(
        actor: RadrootsActorContext,
        root_event: RadrootsNostrEventPtr,
        previous_event: RadrootsNostrEventPtr,
        fulfillment: RadrootsOrderFulfillmentUpdate,
    ) -> Self {
        Self {
            actor,
            root_event,
            previous_event,
            fulfillment,
            created_at: None,
        }
    }

    pub fn with_created_at(mut self, created_at: RadrootsSdkTimestamp) -> Self {
        self.created_at = Some(created_at);
        self
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct OrderFulfillmentUpdateEnqueueRequest {
    pub actor: RadrootsActorContext,
    pub root_event: RadrootsNostrEventPtr,
    pub previous_event: RadrootsNostrEventPtr,
    pub fulfillment: RadrootsOrderFulfillmentUpdate,
    pub target_relays: SdkRelayTargetPolicy,
    pub idempotency_key: Option<SdkIdempotencyKey>,
    pub created_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(feature = "runtime")]
impl serde::Serialize for OrderFulfillmentUpdateEnqueueRequest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("OrderFulfillmentUpdateEnqueueRequest", 7)?;
        state.serialize_field("actor", &SdkActorContextJson(&self.actor))?;
        state.serialize_field("root_event", &self.root_event)?;
        state.serialize_field("previous_event", &self.previous_event)?;
        state.serialize_field("fulfillment", &self.fulfillment)?;
        state.serialize_field("target_relays", &self.target_relays)?;
        state.serialize_field("idempotency_key", &self.idempotency_key)?;
        state.serialize_field("created_at", &self.created_at)?;
        state.end()
    }
}

#[cfg(feature = "runtime")]
impl OrderFulfillmentUpdateEnqueueRequest {
    pub fn new(
        actor: RadrootsActorContext,
        root_event: RadrootsNostrEventPtr,
        previous_event: RadrootsNostrEventPtr,
        fulfillment: RadrootsOrderFulfillmentUpdate,
        target_relays: SdkRelayTargetPolicy,
    ) -> Self {
        Self {
            actor,
            root_event,
            previous_event,
            fulfillment,
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
pub struct OrderFulfillmentUpdatePlan {
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
pub struct OrderFulfillmentUpdateReceipt {
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
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct OrderReceiptRecordPrepareRequest {
    pub actor: RadrootsActorContext,
    pub root_event: RadrootsNostrEventPtr,
    pub previous_event: RadrootsNostrEventPtr,
    pub receipt: RadrootsOrderReceipt,
    pub created_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(feature = "runtime")]
impl serde::Serialize for OrderReceiptRecordPrepareRequest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("OrderReceiptRecordPrepareRequest", 5)?;
        state.serialize_field("actor", &SdkActorContextJson(&self.actor))?;
        state.serialize_field("root_event", &self.root_event)?;
        state.serialize_field("previous_event", &self.previous_event)?;
        state.serialize_field("receipt", &self.receipt)?;
        state.serialize_field("created_at", &self.created_at)?;
        state.end()
    }
}

#[cfg(feature = "runtime")]
impl OrderReceiptRecordPrepareRequest {
    pub fn new(
        actor: RadrootsActorContext,
        root_event: RadrootsNostrEventPtr,
        previous_event: RadrootsNostrEventPtr,
        receipt: RadrootsOrderReceipt,
    ) -> Self {
        Self {
            actor,
            root_event,
            previous_event,
            receipt,
            created_at: None,
        }
    }

    pub fn with_created_at(mut self, created_at: RadrootsSdkTimestamp) -> Self {
        self.created_at = Some(created_at);
        self
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct OrderReceiptRecordEnqueueRequest {
    pub actor: RadrootsActorContext,
    pub root_event: RadrootsNostrEventPtr,
    pub previous_event: RadrootsNostrEventPtr,
    pub receipt: RadrootsOrderReceipt,
    pub target_relays: SdkRelayTargetPolicy,
    pub idempotency_key: Option<SdkIdempotencyKey>,
    pub created_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(feature = "runtime")]
impl serde::Serialize for OrderReceiptRecordEnqueueRequest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("OrderReceiptRecordEnqueueRequest", 7)?;
        state.serialize_field("actor", &SdkActorContextJson(&self.actor))?;
        state.serialize_field("root_event", &self.root_event)?;
        state.serialize_field("previous_event", &self.previous_event)?;
        state.serialize_field("receipt", &self.receipt)?;
        state.serialize_field("target_relays", &self.target_relays)?;
        state.serialize_field("idempotency_key", &self.idempotency_key)?;
        state.serialize_field("created_at", &self.created_at)?;
        state.end()
    }
}

#[cfg(feature = "runtime")]
impl OrderReceiptRecordEnqueueRequest {
    pub fn new(
        actor: RadrootsActorContext,
        root_event: RadrootsNostrEventPtr,
        previous_event: RadrootsNostrEventPtr,
        receipt: RadrootsOrderReceipt,
        target_relays: SdkRelayTargetPolicy,
    ) -> Self {
        Self {
            actor,
            root_event,
            previous_event,
            receipt,
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
pub struct OrderReceiptRecordPlan {
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
pub struct OrderReceiptRecordReceipt {
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
    pub fulfillment_status: Option<OrderFulfillmentStatusKind>,
    pub payment_state: OrderPaymentStateKind,
    pub settlement_state: OrderSettlementStateKind,
    pub lifecycle_terminal: bool,
    pub evidence: OrderStatusEvidenceSummary,
    pub eligibility: OrderStatusEligibility,
    pub payment_handoff: OrderPaymentHandoffKind,
    pub next_action: OrderStatusNextActionKind,
    pub event_ids: Vec<RadrootsEventId>,
    pub request_event_id: Option<RadrootsEventId>,
    pub decision_event_id: Option<RadrootsEventId>,
    pub agreement_event_id: Option<RadrootsEventId>,
    pub pending_revision_event_id: Option<RadrootsEventId>,
    pub fulfillment_event_id: Option<RadrootsEventId>,
    pub cancellation_event_id: Option<RadrootsEventId>,
    pub receipt_event_id: Option<RadrootsEventId>,
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
    pub has_fulfillment: bool,
    pub has_cancellation: bool,
    pub has_receipt: bool,
    pub has_issues: bool,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct OrderStatusEligibility {
    pub can_decide: bool,
    pub can_propose_revision: bool,
    pub can_decide_revision: bool,
    pub can_cancel: bool,
    pub can_update_fulfillment: bool,
    pub can_record_receipt: bool,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum OrderPaymentHandoffKind {
    NotReady,
    NotRequired,
    InPersonOrOffPlatformPending,
    InPersonOrOffPlatformRecorded,
    InPersonOrOffPlatformSettled,
    Rejected,
    Invalid,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum OrderStatusNextActionKind {
    NoLocalOrder,
    InspectEvidenceIssues,
    AwaitSellerDecision,
    ArrangeInPersonOrOffPlatformPayment,
    DecideRevision,
    FulfillOrder,
    RecordReceipt,
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
    Accepted,
    Declined,
    Cancelled,
    Completed,
    Disputed,
    Invalid,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum OrderFulfillmentStatusKind {
    AcceptedNotFulfilled,
    Preparing,
    ReadyForPickup,
    OutForDelivery,
    Delivered,
    SellerCancelled,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum OrderPaymentStateKind {
    NotRecorded,
    Recorded,
    Settled,
    Rejected,
    Invalid,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum OrderSettlementStateKind {
    NotRequired,
    Pending,
    Accepted,
    Rejected,
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
    RevisionProposalWithoutAcceptedDecision,
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
    FulfillmentWithoutAcceptedDecision,
    FulfillmentPayloadInvalid,
    FulfillmentOrderIdMismatch,
    FulfillmentAuthorMismatch,
    FulfillmentCounterpartyMismatch,
    FulfillmentBuyerMismatch,
    FulfillmentSellerMismatch,
    FulfillmentListingAddressInvalid,
    FulfillmentListingMismatch,
    FulfillmentRootMismatch,
    FulfillmentPreviousMismatch,
    FulfillmentStatusNotPublishable,
    FulfillmentUnsupportedTransition,
    ForkedFulfillments,
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
    CancellationAfterFulfillment,
    ReceiptWithoutEligibleFulfillment,
    ReceiptPayloadInvalid,
    ReceiptOrderIdMismatch,
    ReceiptAuthorMismatch,
    ReceiptCounterpartyMismatch,
    ReceiptBuyerMismatch,
    ReceiptSellerMismatch,
    ReceiptListingAddressInvalid,
    ReceiptListingMismatch,
    ReceiptRootMismatch,
    ReceiptPreviousMismatch,
    PaymentWithoutAcceptedAgreement,
    PaymentPayloadInvalid,
    PaymentOrderIdMismatch,
    PaymentAuthorMismatch,
    PaymentCounterpartyMismatch,
    PaymentBuyerMismatch,
    PaymentSellerMismatch,
    PaymentListingAddressInvalid,
    PaymentListingMismatch,
    PaymentRootMismatch,
    PaymentPreviousMismatch,
    PaymentAgreementMismatch,
    PaymentQuoteMismatch,
    PaymentQuoteVersionMismatch,
    PaymentEconomicsDigestMismatch,
    PaymentAmountMismatch,
    PaymentCurrencyMismatch,
    PaymentAfterCancellation,
    RevisionAfterPayment,
    DuplicatePayments,
    SettlementWithoutValidPayment,
    SettlementPayloadInvalid,
    SettlementOrderIdMismatch,
    SettlementAuthorMismatch,
    SettlementCounterpartyMismatch,
    SettlementBuyerMismatch,
    SettlementSellerMismatch,
    SettlementListingAddressInvalid,
    SettlementListingMismatch,
    SettlementRootMismatch,
    SettlementPreviousMismatch,
    SettlementPaymentEventMismatch,
    SettlementAgreementMismatch,
    SettlementQuoteMismatch,
    SettlementQuoteVersionMismatch,
    SettlementEconomicsDigestMismatch,
    SettlementAmountMismatch,
    SettlementCurrencyMismatch,
    DuplicateSettlements,
    ForkedLifecycle,
}

#[cfg(feature = "runtime")]
impl SdkOrderStatusIssueKind {
    pub fn code(self) -> String {
        camel_to_snake(format!("{self:?}").as_str())
    }
}

#[cfg(feature = "runtime")]
impl<'sdk> OrdersClient<'sdk> {
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

    pub async fn enqueue_submit<S>(
        &self,
        request: OrderSubmitEnqueueRequest,
        signer: &S,
    ) -> Result<OrderSubmitReceipt, RadrootsSdkError>
    where
        S: RadrootsEventSigner + ?Sized,
    {
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
        self.enqueue_prepared_submit(&actor, plan, target_relays, idempotency_key, signer)
            .await
    }

    pub async fn enqueue_prepared_submit<S>(
        &self,
        actor: &RadrootsActorContext,
        plan: OrderSubmitPlan,
        target_relays: SdkRelayTargetPolicy,
        idempotency_key: Option<SdkIdempotencyKey>,
        signer: &S,
    ) -> Result<OrderSubmitReceipt, RadrootsSdkError>
    where
        S: RadrootsEventSigner + ?Sized,
    {
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
        Ok(OrderSubmitReceipt {
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
        })
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

    pub async fn enqueue_decision<S>(
        &self,
        request: OrderDecisionEnqueueRequest,
        signer: &S,
    ) -> Result<OrderDecisionReceipt, RadrootsSdkError>
    where
        S: RadrootsEventSigner + ?Sized,
    {
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
        self.enqueue_prepared_decision(&actor, plan, target_relays, idempotency_key, signer)
            .await
    }

    pub async fn enqueue_prepared_decision<S>(
        &self,
        actor: &RadrootsActorContext,
        plan: OrderDecisionPlan,
        target_relays: SdkRelayTargetPolicy,
        idempotency_key: Option<SdkIdempotencyKey>,
        signer: &S,
    ) -> Result<OrderDecisionReceipt, RadrootsSdkError>
    where
        S: RadrootsEventSigner + ?Sized,
    {
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
        Ok(OrderDecisionReceipt {
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
        })
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

    pub async fn enqueue_revision_proposal<S>(
        &self,
        request: OrderRevisionProposalEnqueueRequest,
        signer: &S,
    ) -> Result<OrderRevisionProposalReceipt, RadrootsSdkError>
    where
        S: RadrootsEventSigner + ?Sized,
    {
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
        self.enqueue_prepared_revision_proposal(
            &actor,
            plan,
            target_relays,
            idempotency_key,
            signer,
        )
        .await
    }

    pub async fn enqueue_prepared_revision_proposal<S>(
        &self,
        actor: &RadrootsActorContext,
        plan: OrderRevisionProposalPlan,
        target_relays: SdkRelayTargetPolicy,
        idempotency_key: Option<SdkIdempotencyKey>,
        signer: &S,
    ) -> Result<OrderRevisionProposalReceipt, RadrootsSdkError>
    where
        S: RadrootsEventSigner + ?Sized,
    {
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
        Ok(OrderRevisionProposalReceipt {
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
        })
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

    pub async fn enqueue_revision_decision<S>(
        &self,
        request: OrderRevisionDecisionEnqueueRequest,
        signer: &S,
    ) -> Result<OrderRevisionDecisionReceipt, RadrootsSdkError>
    where
        S: RadrootsEventSigner + ?Sized,
    {
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
        self.enqueue_prepared_revision_decision(
            &actor,
            plan,
            target_relays,
            idempotency_key,
            signer,
        )
        .await
    }

    pub async fn enqueue_prepared_revision_decision<S>(
        &self,
        actor: &RadrootsActorContext,
        plan: OrderRevisionDecisionPlan,
        target_relays: SdkRelayTargetPolicy,
        idempotency_key: Option<SdkIdempotencyKey>,
        signer: &S,
    ) -> Result<OrderRevisionDecisionReceipt, RadrootsSdkError>
    where
        S: RadrootsEventSigner + ?Sized,
    {
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
        Ok(OrderRevisionDecisionReceipt {
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
        })
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

    pub async fn enqueue_cancellation<S>(
        &self,
        request: OrderCancellationEnqueueRequest,
        signer: &S,
    ) -> Result<OrderCancellationReceipt, RadrootsSdkError>
    where
        S: RadrootsEventSigner + ?Sized,
    {
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
        self.enqueue_prepared_cancellation(&actor, plan, target_relays, idempotency_key, signer)
            .await
    }

    pub async fn enqueue_prepared_cancellation<S>(
        &self,
        actor: &RadrootsActorContext,
        plan: OrderCancellationPlan,
        target_relays: SdkRelayTargetPolicy,
        idempotency_key: Option<SdkIdempotencyKey>,
        signer: &S,
    ) -> Result<OrderCancellationReceipt, RadrootsSdkError>
    where
        S: RadrootsEventSigner + ?Sized,
    {
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
        Ok(OrderCancellationReceipt {
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
        })
    }

    pub fn prepare_fulfillment_update(
        &self,
        request: OrderFulfillmentUpdatePrepareRequest,
    ) -> Result<OrderFulfillmentUpdatePlan, RadrootsSdkError> {
        let created_at = self.resolved_created_at(request.created_at)?;
        order_fulfillment_update_plan(
            &request.actor,
            request.root_event,
            request.previous_event,
            request.fulfillment,
            created_at,
        )
    }

    pub async fn enqueue_fulfillment_update<S>(
        &self,
        request: OrderFulfillmentUpdateEnqueueRequest,
        signer: &S,
    ) -> Result<OrderFulfillmentUpdateReceipt, RadrootsSdkError>
    where
        S: RadrootsEventSigner + ?Sized,
    {
        let OrderFulfillmentUpdateEnqueueRequest {
            actor,
            root_event,
            previous_event,
            fulfillment,
            target_relays,
            idempotency_key,
            created_at,
        } = request;
        let prepare_request = OrderFulfillmentUpdatePrepareRequest {
            actor: actor.clone(),
            root_event,
            previous_event,
            fulfillment,
            created_at,
        };
        let plan = self.prepare_fulfillment_update(prepare_request)?;
        self.enqueue_prepared_fulfillment_update(
            &actor,
            plan,
            target_relays,
            idempotency_key,
            signer,
        )
        .await
    }

    pub async fn enqueue_prepared_fulfillment_update<S>(
        &self,
        actor: &RadrootsActorContext,
        plan: OrderFulfillmentUpdatePlan,
        target_relays: SdkRelayTargetPolicy,
        idempotency_key: Option<SdkIdempotencyKey>,
        signer: &S,
    ) -> Result<OrderFulfillmentUpdateReceipt, RadrootsSdkError>
    where
        S: RadrootsEventSigner + ?Sized,
    {
        if !self
            .prepared_order_event_exists(&plan.expected_event_id)
            .await?
        {
            self.require_fulfillment_update_preflight(&plan).await?;
        }
        let enqueue = enqueue_signed_workflow(
            self.sdk,
            SdkWorkflowEnqueueRequest {
                operation_kind: OrderWorkflowKind::FulfillmentUpdate.operation_kind(),
                actor,
                frozen_draft: &plan.frozen_draft,
                target_relays,
                idempotency_key,
            },
            signer,
        )
        .await?;
        Ok(OrderFulfillmentUpdateReceipt {
            workflow: order_workflow_enqueue_receipt(
                OrderWorkflowKind::FulfillmentUpdate,
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
        })
    }

    pub fn prepare_receipt_record(
        &self,
        request: OrderReceiptRecordPrepareRequest,
    ) -> Result<OrderReceiptRecordPlan, RadrootsSdkError> {
        let created_at = self.resolved_created_at(request.created_at)?;
        order_receipt_record_plan(
            &request.actor,
            request.root_event,
            request.previous_event,
            request.receipt,
            created_at,
        )
    }

    pub async fn enqueue_receipt_record<S>(
        &self,
        request: OrderReceiptRecordEnqueueRequest,
        signer: &S,
    ) -> Result<OrderReceiptRecordReceipt, RadrootsSdkError>
    where
        S: RadrootsEventSigner + ?Sized,
    {
        let OrderReceiptRecordEnqueueRequest {
            actor,
            root_event,
            previous_event,
            receipt,
            target_relays,
            idempotency_key,
            created_at,
        } = request;
        let prepare_request = OrderReceiptRecordPrepareRequest {
            actor: actor.clone(),
            root_event,
            previous_event,
            receipt,
            created_at,
        };
        let plan = self.prepare_receipt_record(prepare_request)?;
        self.enqueue_prepared_receipt_record(&actor, plan, target_relays, idempotency_key, signer)
            .await
    }

    pub async fn enqueue_prepared_receipt_record<S>(
        &self,
        actor: &RadrootsActorContext,
        plan: OrderReceiptRecordPlan,
        target_relays: SdkRelayTargetPolicy,
        idempotency_key: Option<SdkIdempotencyKey>,
        signer: &S,
    ) -> Result<OrderReceiptRecordReceipt, RadrootsSdkError>
    where
        S: RadrootsEventSigner + ?Sized,
    {
        if !self
            .prepared_order_event_exists(&plan.expected_event_id)
            .await?
        {
            self.require_receipt_record_preflight(&plan).await?;
        }
        let enqueue = enqueue_signed_workflow(
            self.sdk,
            SdkWorkflowEnqueueRequest {
                operation_kind: OrderWorkflowKind::ReceiptRecord.operation_kind(),
                actor,
                frozen_draft: &plan.frozen_draft,
                target_relays,
                idempotency_key,
            },
            signer,
        )
        .await?;
        Ok(OrderReceiptRecordReceipt {
            workflow: order_workflow_enqueue_receipt(
                OrderWorkflowKind::ReceiptRecord,
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
        })
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

    async fn require_fulfillment_update_preflight(
        &self,
        plan: &OrderFulfillmentUpdatePlan,
    ) -> Result<(), RadrootsSdkError> {
        let query_result = self.query_order_projection(&plan.order_id).await?;
        require_fulfillment_update_state(plan, &query_result.projection)
    }

    async fn require_receipt_record_preflight(
        &self,
        plan: &OrderReceiptRecordPlan,
    ) -> Result<(), RadrootsSdkError> {
        let query_result = self.query_order_projection(&plan.order_id).await?;
        require_receipt_record_state(plan, &query_result.projection)
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
        KIND_ORDER_FULFILLMENT_UPDATE => {
            let payload = order_fulfillment_update_from_event(event)
                .map_err(order_evidence_parse_error)?
                .payload;
            (payload.order_id, payload.listing_addr)
        }
        KIND_ORDER_RECEIPT => {
            let payload = order_receipt_from_event(event)
                .map_err(order_evidence_parse_error)?
                .payload;
            (payload.order_id, payload.listing_addr)
        }
        KIND_ORDER_PAYMENT_RECORD => {
            let payload = order_payment_record_from_event(event)
                .map_err(order_evidence_parse_error)?
                .payload;
            (payload.order_id, payload.listing_addr)
        }
        KIND_ORDER_SETTLEMENT_DECISION => {
            let payload = order_settlement_decision_from_event(event)
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
        let found = projection.status != RadrootsOrderStatus::Missing;
        let evidence = OrderStatusEvidenceSummary::from_projection(
            &projection,
            query_result.event_count,
            query_result.limit_applied,
        );
        let eligibility = OrderStatusEligibility::from_projection(&projection);
        let payment_handoff = OrderPaymentHandoffKind::from_projection(&projection);
        let next_action =
            OrderStatusNextActionKind::from_projection(&projection, &eligibility, payment_handoff);
        Self {
            order_id: projection.order_id,
            source: SdkOrderStatusSource::LocalEventStore,
            found,
            event_count: query_result.event_count,
            limit_applied: query_result.limit_applied,
            status: projection.status.into(),
            fulfillment_status: projection.fulfillment_status.map(Into::into),
            payment_state: projection.payment.state.into(),
            settlement_state: projection.payment.settlement_state.into(),
            lifecycle_terminal: projection.lifecycle_terminal,
            evidence,
            eligibility,
            payment_handoff,
            next_action,
            event_ids: query_result.event_ids,
            request_event_id: projection.request_event_id,
            decision_event_id: projection.decision_event_id,
            agreement_event_id: projection.agreement_event_id,
            pending_revision_event_id: projection.pending_revision_event_id,
            fulfillment_event_id: projection.fulfillment_event_id,
            cancellation_event_id: projection.cancellation_event_id,
            receipt_event_id: projection.receipt_event_id,
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
            has_fulfillment: projection.fulfillment_event_id.is_some(),
            has_cancellation: projection.cancellation_event_id.is_some(),
            has_receipt: projection.receipt_event_id.is_some(),
            has_issues: !projection.issues.is_empty(),
        }
    }
}

#[cfg(feature = "runtime")]
impl OrderStatusEligibility {
    fn from_projection(projection: &RadrootsOrderProjection) -> Self {
        let clean = projection.issues.is_empty();
        let open = clean && !projection.lifecycle_terminal;
        let requested = projection.status == RadrootsOrderStatus::Requested;
        let accepted = projection.status == RadrootsOrderStatus::Accepted;
        let has_pending_revision = projection.pending_revision_event_id.is_some();
        let has_fulfillment = projection.fulfillment_event_id.is_some();
        let fulfillment_terminal = matches!(
            projection.fulfillment_status,
            Some(
                RadrootsOrderFulfillmentState::Delivered
                    | RadrootsOrderFulfillmentState::SellerCancelled
            )
        );
        let receipt_ready = matches!(
            projection.fulfillment_status,
            Some(
                RadrootsOrderFulfillmentState::ReadyForPickup
                    | RadrootsOrderFulfillmentState::Delivered
            )
        );
        let revision_payment_open =
            projection.payment.state == RadrootsOrderPaymentState::NotRecorded;

        Self {
            can_decide: open && requested && projection.decision_event_id.is_none(),
            can_propose_revision: open
                && accepted
                && !has_pending_revision
                && !has_fulfillment
                && revision_payment_open,
            can_decide_revision: open && accepted && has_pending_revision,
            can_cancel: open
                && matches!(
                    projection.status,
                    RadrootsOrderStatus::Requested | RadrootsOrderStatus::Accepted
                )
                && !has_pending_revision
                && !has_fulfillment,
            can_update_fulfillment: open
                && accepted
                && !has_pending_revision
                && !fulfillment_terminal,
            can_record_receipt: open
                && accepted
                && receipt_ready
                && projection.receipt_event_id.is_none(),
        }
    }
}

#[cfg(feature = "runtime")]
impl OrderPaymentHandoffKind {
    fn from_projection(projection: &RadrootsOrderProjection) -> Self {
        if !projection.issues.is_empty() || projection.status == RadrootsOrderStatus::Invalid {
            return Self::Invalid;
        }
        match projection.status {
            RadrootsOrderStatus::Missing | RadrootsOrderStatus::Requested => Self::NotReady,
            RadrootsOrderStatus::Declined | RadrootsOrderStatus::Cancelled => Self::NotRequired,
            RadrootsOrderStatus::Accepted
            | RadrootsOrderStatus::Completed
            | RadrootsOrderStatus::Disputed => match projection.payment.state {
                RadrootsOrderPaymentState::NotRecorded => Self::InPersonOrOffPlatformPending,
                RadrootsOrderPaymentState::Recorded => Self::InPersonOrOffPlatformRecorded,
                RadrootsOrderPaymentState::Settled => Self::InPersonOrOffPlatformSettled,
                RadrootsOrderPaymentState::Rejected => Self::Rejected,
                RadrootsOrderPaymentState::Invalid => Self::Invalid,
            },
            RadrootsOrderStatus::Invalid => Self::Invalid,
        }
    }
}

#[cfg(feature = "runtime")]
impl OrderStatusNextActionKind {
    fn from_projection(
        projection: &RadrootsOrderProjection,
        eligibility: &OrderStatusEligibility,
        payment_handoff: OrderPaymentHandoffKind,
    ) -> Self {
        if projection.status == RadrootsOrderStatus::Missing {
            return Self::NoLocalOrder;
        }
        if !projection.issues.is_empty() || projection.status == RadrootsOrderStatus::Invalid {
            return Self::InspectEvidenceIssues;
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
        if eligibility.can_record_receipt {
            return Self::RecordReceipt;
        }
        if matches!(
            payment_handoff,
            OrderPaymentHandoffKind::InPersonOrOffPlatformPending
        ) {
            return Self::ArrangeInPersonOrOffPlatformPayment;
        }
        if eligibility.can_update_fulfillment {
            return Self::FulfillOrder;
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
    .map_err(|error| RadrootsSdkError::InvalidRequest {
        message: format!("order submit draft freeze failed: {error}"),
    })?;
    let expected_event_id = RadrootsEventId::parse(frozen_draft.expected_event_id.as_str())
        .map_err(|error| RadrootsSdkError::InvalidRequest {
            message: format!("order submit draft produced invalid event id: {error}"),
        })?;
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
    let draft = order::build_order_decision_draft(&request_event_id, &request_event_id, &decision)
        .map_err(|error| RadrootsSdkError::InvalidRequest {
            message: format!("order decision draft encode failed: {error}"),
        })?;
    let frozen_draft = to_frozen_draft(
        draft.into_wire_parts(),
        ORDER_DECISION_CONTRACT_ID,
        decision.seller_pubkey.as_str(),
        created_at_nostr,
    )
    .map_err(|error| RadrootsSdkError::InvalidRequest {
        message: format!("order decision draft freeze failed: {error}"),
    })?;
    let expected_event_id = RadrootsEventId::parse(frozen_draft.expected_event_id.as_str())
        .map_err(|error| RadrootsSdkError::InvalidRequest {
            message: format!("order decision draft produced invalid event id: {error}"),
        })?;
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
    let draft =
        order::build_order_revision_proposal_draft(&root_event_id, &previous_event_id, &proposal)
            .map_err(|error| RadrootsSdkError::InvalidRequest {
            message: format!("order revision proposal draft encode failed: {error}"),
        })?;
    let (frozen_draft, expected_event_id) = freeze_order_workflow_draft(
        draft.into_wire_parts(),
        ORDER_REVISION_PROPOSAL_CONTRACT_ID,
        seller_pubkey.as_str(),
        created_at_nostr,
        "order revision proposal",
    )?;
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
    let draft =
        order::build_order_revision_decision_draft(&root_event_id, &previous_event_id, &decision)
            .map_err(|error| RadrootsSdkError::InvalidRequest {
            message: format!("order revision decision draft encode failed: {error}"),
        })?;
    let (frozen_draft, expected_event_id) = freeze_order_workflow_draft(
        draft.into_wire_parts(),
        ORDER_REVISION_DECISION_CONTRACT_ID,
        buyer_pubkey.as_str(),
        created_at_nostr,
        "order revision decision",
    )?;
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
    let draft =
        order::build_order_cancellation_draft(&root_event_id, &previous_event_id, &cancellation)
            .map_err(|error| RadrootsSdkError::InvalidRequest {
                message: format!("order cancellation draft encode failed: {error}"),
            })?;
    let (frozen_draft, expected_event_id) = freeze_order_workflow_draft(
        draft.into_wire_parts(),
        ORDER_CANCELLATION_CONTRACT_ID,
        buyer_pubkey.as_str(),
        created_at_nostr,
        "order cancellation",
    )?;
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
fn order_fulfillment_update_plan(
    actor: &RadrootsActorContext,
    root_event: RadrootsNostrEventPtr,
    previous_event: RadrootsNostrEventPtr,
    fulfillment: RadrootsOrderFulfillmentUpdate,
    created_at: RadrootsSdkTimestamp,
) -> Result<OrderFulfillmentUpdatePlan, RadrootsSdkError> {
    require_seller_actor(actor, "order.prepare_fulfillment_update")?;
    let root_event_id = order_reference_event_id(&root_event, "root")?;
    let previous_event_id = order_reference_event_id(&previous_event, "previous")?;
    if fulfillment.seller_pubkey.as_str() != actor.pubkey().as_str() {
        return Err(RadrootsSdkError::UnauthorizedActor {
            operation: "order.prepare_fulfillment_update".to_owned(),
            reason: "actor pubkey must match order seller_pubkey".to_owned(),
        });
    }
    let created_at_nostr = created_at.try_into_nostr_created_at()?;
    let order_id = fulfillment.order_id.clone();
    let listing_addr = fulfillment.listing_addr.clone();
    let buyer_pubkey = fulfillment.buyer_pubkey.clone();
    let seller_pubkey = fulfillment.seller_pubkey.clone();
    let draft =
        order::build_fulfillment_update_draft(&root_event_id, &previous_event_id, &fulfillment)
            .map_err(|error| RadrootsSdkError::InvalidRequest {
                message: format!("order fulfillment update draft encode failed: {error}"),
            })?;
    let (frozen_draft, expected_event_id) = freeze_order_workflow_draft(
        draft.into_wire_parts(),
        ORDER_FULFILLMENT_UPDATE_CONTRACT_ID,
        seller_pubkey.as_str(),
        created_at_nostr,
        "order fulfillment update",
    )?;
    Ok(OrderFulfillmentUpdatePlan {
        workflow: order_workflow_plan(
            OrderWorkflowKind::FulfillmentUpdate,
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
fn order_receipt_record_plan(
    actor: &RadrootsActorContext,
    root_event: RadrootsNostrEventPtr,
    previous_event: RadrootsNostrEventPtr,
    receipt: RadrootsOrderReceipt,
    created_at: RadrootsSdkTimestamp,
) -> Result<OrderReceiptRecordPlan, RadrootsSdkError> {
    require_buyer_actor(actor, "order.prepare_receipt_record")?;
    let root_event_id = order_reference_event_id(&root_event, "root")?;
    let previous_event_id = order_reference_event_id(&previous_event, "previous")?;
    if receipt.buyer_pubkey.as_str() != actor.pubkey().as_str() {
        return Err(RadrootsSdkError::UnauthorizedActor {
            operation: "order.prepare_receipt_record".to_owned(),
            reason: "actor pubkey must match order buyer_pubkey".to_owned(),
        });
    }
    let created_at_nostr = created_at.try_into_nostr_created_at()?;
    let order_id = receipt.order_id.clone();
    let listing_addr = receipt.listing_addr.clone();
    let buyer_pubkey = receipt.buyer_pubkey.clone();
    let seller_pubkey = receipt.seller_pubkey.clone();
    let draft = order::build_buyer_receipt_draft(&root_event_id, &previous_event_id, &receipt)
        .map_err(|error| RadrootsSdkError::InvalidRequest {
            message: format!("order receipt record draft encode failed: {error}"),
        })?;
    let (frozen_draft, expected_event_id) = freeze_order_workflow_draft(
        draft.into_wire_parts(),
        ORDER_RECEIPT_CONTRACT_ID,
        buyer_pubkey.as_str(),
        created_at_nostr,
        "order receipt record",
    )?;
    Ok(OrderReceiptRecordPlan {
        workflow: order_workflow_plan(
            OrderWorkflowKind::ReceiptRecord,
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
fn order_workflow_enqueue_receipt(
    kind: OrderWorkflowKind,
    expected_event_id: RadrootsEventId,
    enqueue: &crate::workflow_runtime::SdkWorkflowEnqueueReceipt,
) -> OrderWorkflowEnqueueReceipt {
    OrderWorkflowEnqueueReceipt {
        kind,
        operation_kind: kind.operation_kind(),
        expected_event_id,
        signed_event_id: enqueue.signed_event_id.clone(),
        local_event_seq: enqueue.local_event_seq,
        outbox_operation_id: enqueue.outbox_operation_id,
        outbox_event_id: enqueue.outbox_event_id,
        state: enqueue.state.into(),
        idempotency_digest_prefix: Some(enqueue.idempotency_digest_prefix.clone()),
    }
}

#[cfg(feature = "runtime")]
fn freeze_order_workflow_draft(
    parts: WireEventParts,
    contract_id: &str,
    expected_pubkey: &str,
    created_at: u32,
    operation: &'static str,
) -> Result<(RadrootsFrozenEventDraft, RadrootsEventId), RadrootsSdkError> {
    let frozen_draft =
        to_frozen_draft(parts, contract_id, expected_pubkey, created_at).map_err(|error| {
            RadrootsSdkError::InvalidRequest {
                message: format!("{operation} draft freeze failed: {error}"),
            }
        })?;
    let expected_event_id = RadrootsEventId::parse(frozen_draft.expected_event_id.as_str())
        .map_err(|error| RadrootsSdkError::InvalidRequest {
            message: format!("{operation} draft produced invalid event id: {error}"),
        })?;
    Ok((frozen_draft, expected_event_id))
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
    let author_pubkey = RadrootsPublicKey::parse(event.author.as_str()).map_err(|error| {
        RadrootsSdkError::InvalidRequest {
            message: format!("order request evidence author is invalid: {error}"),
        }
    })?;
    let envelope =
        order::parse_order_request(event).map_err(|error| RadrootsSdkError::InvalidRequest {
            message: format!("order request evidence decode failed: {error}"),
        })?;
    let payload = envelope.payload;
    if payload.buyer_pubkey != author_pubkey {
        return Err(RadrootsSdkError::InvalidRequest {
            message: "order request evidence author must match buyer_pubkey".to_owned(),
        });
    }
    if envelope.order_id != payload.order_id.as_str() {
        return Err(RadrootsSdkError::InvalidRequest {
            message: "order request evidence order_id envelope mismatch".to_owned(),
        });
    }
    if envelope.listing_addr != payload.listing_addr.as_str() {
        return Err(RadrootsSdkError::InvalidRequest {
            message: "order request evidence listing_addr envelope mismatch".to_owned(),
        });
    }
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
    if !matches!(&projection.status, RadrootsOrderStatus::Requested) {
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
    require_lifecycle_status(&refs, projection, RadrootsOrderStatus::Accepted)?;
    require_no_lifecycle_terminal(&refs, projection)?;
    require_no_payment_for_revision(&refs, projection)?;
    require_no_pending_revision(&refs, projection)?;
    if projection.fulfillment_event_id.is_some() {
        return Err(lifecycle_invalid(
            refs.operation,
            refs.order_id,
            "revision proposal requires order before fulfillment",
        ));
    }
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
    require_lifecycle_status(&refs, projection, RadrootsOrderStatus::Accepted)?;
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
    if !matches!(
        projection.status,
        RadrootsOrderStatus::Requested | RadrootsOrderStatus::Accepted
    ) {
        return Err(lifecycle_invalid(
            refs.operation,
            refs.order_id,
            format!(
                "cancellation requires requested or accepted local state; current state is {:?}",
                projection.status
            ),
        ));
    }
    require_no_lifecycle_terminal(&refs, projection)?;
    require_no_pending_revision(&refs, projection)?;
    if projection.fulfillment_event_id.is_some() {
        return Err(lifecycle_invalid(
            refs.operation,
            refs.order_id,
            "cancellation requires order before fulfillment",
        ));
    }
    require_lifecycle_previous_is_current(&refs, projection)
}

#[cfg(feature = "runtime")]
fn require_fulfillment_update_state(
    plan: &OrderFulfillmentUpdatePlan,
    projection: &RadrootsOrderProjection,
) -> Result<(), RadrootsSdkError> {
    let refs = OrderLifecycleReferences {
        operation: "order fulfillment update",
        order_id: &plan.order_id,
        listing_addr: &plan.listing_addr,
        buyer_pubkey: &plan.buyer_pubkey,
        seller_pubkey: &plan.seller_pubkey,
        root_event_id: &plan.root_event_id,
        previous_event_id: &plan.previous_event_id,
    };
    require_clean_lifecycle_projection(refs, projection)?;
    require_lifecycle_status(&refs, projection, RadrootsOrderStatus::Accepted)?;
    require_no_lifecycle_terminal(&refs, projection)?;
    require_no_pending_revision(&refs, projection)?;
    if matches!(
        projection.fulfillment_status,
        Some(
            RadrootsOrderFulfillmentState::Delivered
                | RadrootsOrderFulfillmentState::SellerCancelled
        )
    ) {
        return Err(lifecycle_invalid(
            refs.operation,
            refs.order_id,
            "fulfillment update cannot follow terminal fulfillment status",
        ));
    }
    require_lifecycle_previous_is_current(&refs, projection)
}

#[cfg(feature = "runtime")]
fn require_receipt_record_state(
    plan: &OrderReceiptRecordPlan,
    projection: &RadrootsOrderProjection,
) -> Result<(), RadrootsSdkError> {
    let refs = OrderLifecycleReferences {
        operation: "order receipt record",
        order_id: &plan.order_id,
        listing_addr: &plan.listing_addr,
        buyer_pubkey: &plan.buyer_pubkey,
        seller_pubkey: &plan.seller_pubkey,
        root_event_id: &plan.root_event_id,
        previous_event_id: &plan.previous_event_id,
    };
    require_clean_lifecycle_projection(refs, projection)?;
    require_lifecycle_status(&refs, projection, RadrootsOrderStatus::Accepted)?;
    require_no_lifecycle_terminal(&refs, projection)?;
    if projection.fulfillment_event_id.as_ref() != Some(refs.previous_event_id) {
        return Err(lifecycle_invalid(
            refs.operation,
            refs.order_id,
            "receipt record requires previous event to be the current fulfillment event",
        ));
    }
    if !matches!(
        projection.fulfillment_status,
        Some(
            RadrootsOrderFulfillmentState::ReadyForPickup
                | RadrootsOrderFulfillmentState::Delivered
        )
    ) {
        return Err(lifecycle_invalid(
            refs.operation,
            refs.order_id,
            "receipt record requires ready-for-pickup or delivered fulfillment state",
        ));
    }
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
    expected: RadrootsOrderStatus,
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
fn require_no_payment_for_revision(
    refs: &OrderLifecycleReferences<'_>,
    projection: &RadrootsOrderProjection,
) -> Result<(), RadrootsSdkError> {
    if projection.payment.state == RadrootsOrderPaymentState::NotRecorded {
        Ok(())
    } else {
        Err(lifecycle_invalid(
            refs.operation,
            refs.order_id,
            "revision proposal cannot follow recorded payment state",
        ))
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
impl From<RadrootsOrderStatus> for OrderStatusKind {
    fn from(status: RadrootsOrderStatus) -> Self {
        match status {
            RadrootsOrderStatus::Missing => Self::Missing,
            RadrootsOrderStatus::Requested => Self::Requested,
            RadrootsOrderStatus::Accepted => Self::Accepted,
            RadrootsOrderStatus::Declined => Self::Declined,
            RadrootsOrderStatus::Cancelled => Self::Cancelled,
            RadrootsOrderStatus::Completed => Self::Completed,
            RadrootsOrderStatus::Disputed => Self::Disputed,
            RadrootsOrderStatus::Invalid => Self::Invalid,
        }
    }
}

#[cfg(feature = "runtime")]
impl From<RadrootsOrderFulfillmentState> for OrderFulfillmentStatusKind {
    fn from(status: RadrootsOrderFulfillmentState) -> Self {
        match status {
            RadrootsOrderFulfillmentState::AcceptedNotFulfilled => Self::AcceptedNotFulfilled,
            RadrootsOrderFulfillmentState::Preparing => Self::Preparing,
            RadrootsOrderFulfillmentState::ReadyForPickup => Self::ReadyForPickup,
            RadrootsOrderFulfillmentState::OutForDelivery => Self::OutForDelivery,
            RadrootsOrderFulfillmentState::Delivered => Self::Delivered,
            RadrootsOrderFulfillmentState::SellerCancelled => Self::SellerCancelled,
        }
    }
}

#[cfg(feature = "runtime")]
impl From<RadrootsOrderPaymentState> for OrderPaymentStateKind {
    fn from(state: RadrootsOrderPaymentState) -> Self {
        match state {
            RadrootsOrderPaymentState::NotRecorded => Self::NotRecorded,
            RadrootsOrderPaymentState::Recorded => Self::Recorded,
            RadrootsOrderPaymentState::Settled => Self::Settled,
            RadrootsOrderPaymentState::Rejected => Self::Rejected,
            RadrootsOrderPaymentState::Invalid => Self::Invalid,
        }
    }
}

#[cfg(feature = "runtime")]
impl From<RadrootsOrderSettlementState> for OrderSettlementStateKind {
    fn from(state: RadrootsOrderSettlementState) -> Self {
        match state {
            RadrootsOrderSettlementState::NotRequired => Self::NotRequired,
            RadrootsOrderSettlementState::Pending => Self::Pending,
            RadrootsOrderSettlementState::Accepted => Self::Accepted,
            RadrootsOrderSettlementState::Rejected => Self::Rejected,
            RadrootsOrderSettlementState::Invalid => Self::Invalid,
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
            RadrootsOrderIssue::RevisionProposalWithoutAcceptedDecision { event_id } => {
                Self::single(
                    SdkOrderStatusIssueKind::RevisionProposalWithoutAcceptedDecision,
                    event_id,
                )
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
            RadrootsOrderIssue::FulfillmentWithoutAcceptedDecision { event_id } => Self::single(
                SdkOrderStatusIssueKind::FulfillmentWithoutAcceptedDecision,
                event_id,
            ),
            RadrootsOrderIssue::FulfillmentPayloadInvalid { event_id } => {
                Self::single(SdkOrderStatusIssueKind::FulfillmentPayloadInvalid, event_id)
            }
            RadrootsOrderIssue::FulfillmentOrderIdMismatch { event_id } => Self::single(
                SdkOrderStatusIssueKind::FulfillmentOrderIdMismatch,
                event_id,
            ),
            RadrootsOrderIssue::FulfillmentAuthorMismatch { event_id } => {
                Self::single(SdkOrderStatusIssueKind::FulfillmentAuthorMismatch, event_id)
            }
            RadrootsOrderIssue::FulfillmentCounterpartyMismatch { event_id } => Self::single(
                SdkOrderStatusIssueKind::FulfillmentCounterpartyMismatch,
                event_id,
            ),
            RadrootsOrderIssue::FulfillmentBuyerMismatch { event_id } => {
                Self::single(SdkOrderStatusIssueKind::FulfillmentBuyerMismatch, event_id)
            }
            RadrootsOrderIssue::FulfillmentSellerMismatch { event_id } => {
                Self::single(SdkOrderStatusIssueKind::FulfillmentSellerMismatch, event_id)
            }
            RadrootsOrderIssue::FulfillmentListingAddressInvalid { event_id } => Self::single(
                SdkOrderStatusIssueKind::FulfillmentListingAddressInvalid,
                event_id,
            ),
            RadrootsOrderIssue::FulfillmentListingMismatch { event_id } => Self::single(
                SdkOrderStatusIssueKind::FulfillmentListingMismatch,
                event_id,
            ),
            RadrootsOrderIssue::FulfillmentRootMismatch { event_id } => {
                Self::single(SdkOrderStatusIssueKind::FulfillmentRootMismatch, event_id)
            }
            RadrootsOrderIssue::FulfillmentPreviousMismatch { event_id } => Self::single(
                SdkOrderStatusIssueKind::FulfillmentPreviousMismatch,
                event_id,
            ),
            RadrootsOrderIssue::FulfillmentStatusNotPublishable { event_id } => Self::single(
                SdkOrderStatusIssueKind::FulfillmentStatusNotPublishable,
                event_id,
            ),
            RadrootsOrderIssue::FulfillmentUnsupportedTransition { event_id } => Self::single(
                SdkOrderStatusIssueKind::FulfillmentUnsupportedTransition,
                event_id,
            ),
            RadrootsOrderIssue::ForkedFulfillments { event_ids } => {
                Self::new(SdkOrderStatusIssueKind::ForkedFulfillments, event_ids)
            }
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
            RadrootsOrderIssue::CancellationAfterFulfillment { event_id } => Self::single(
                SdkOrderStatusIssueKind::CancellationAfterFulfillment,
                event_id,
            ),
            RadrootsOrderIssue::ReceiptWithoutEligibleFulfillment { event_id } => Self::single(
                SdkOrderStatusIssueKind::ReceiptWithoutEligibleFulfillment,
                event_id,
            ),
            RadrootsOrderIssue::ReceiptPayloadInvalid { event_id } => {
                Self::single(SdkOrderStatusIssueKind::ReceiptPayloadInvalid, event_id)
            }
            RadrootsOrderIssue::ReceiptOrderIdMismatch { event_id } => {
                Self::single(SdkOrderStatusIssueKind::ReceiptOrderIdMismatch, event_id)
            }
            RadrootsOrderIssue::ReceiptAuthorMismatch { event_id } => {
                Self::single(SdkOrderStatusIssueKind::ReceiptAuthorMismatch, event_id)
            }
            RadrootsOrderIssue::ReceiptCounterpartyMismatch { event_id } => Self::single(
                SdkOrderStatusIssueKind::ReceiptCounterpartyMismatch,
                event_id,
            ),
            RadrootsOrderIssue::ReceiptBuyerMismatch { event_id } => {
                Self::single(SdkOrderStatusIssueKind::ReceiptBuyerMismatch, event_id)
            }
            RadrootsOrderIssue::ReceiptSellerMismatch { event_id } => {
                Self::single(SdkOrderStatusIssueKind::ReceiptSellerMismatch, event_id)
            }
            RadrootsOrderIssue::ReceiptListingAddressInvalid { event_id } => Self::single(
                SdkOrderStatusIssueKind::ReceiptListingAddressInvalid,
                event_id,
            ),
            RadrootsOrderIssue::ReceiptListingMismatch { event_id } => {
                Self::single(SdkOrderStatusIssueKind::ReceiptListingMismatch, event_id)
            }
            RadrootsOrderIssue::ReceiptRootMismatch { event_id } => {
                Self::single(SdkOrderStatusIssueKind::ReceiptRootMismatch, event_id)
            }
            RadrootsOrderIssue::ReceiptPreviousMismatch { event_id } => {
                Self::single(SdkOrderStatusIssueKind::ReceiptPreviousMismatch, event_id)
            }
            RadrootsOrderIssue::PaymentWithoutAcceptedAgreement { event_id } => Self::single(
                SdkOrderStatusIssueKind::PaymentWithoutAcceptedAgreement,
                event_id,
            ),
            RadrootsOrderIssue::PaymentPayloadInvalid { event_id } => {
                Self::single(SdkOrderStatusIssueKind::PaymentPayloadInvalid, event_id)
            }
            RadrootsOrderIssue::PaymentOrderIdMismatch { event_id } => {
                Self::single(SdkOrderStatusIssueKind::PaymentOrderIdMismatch, event_id)
            }
            RadrootsOrderIssue::PaymentAuthorMismatch { event_id } => {
                Self::single(SdkOrderStatusIssueKind::PaymentAuthorMismatch, event_id)
            }
            RadrootsOrderIssue::PaymentCounterpartyMismatch { event_id } => Self::single(
                SdkOrderStatusIssueKind::PaymentCounterpartyMismatch,
                event_id,
            ),
            RadrootsOrderIssue::PaymentBuyerMismatch { event_id } => {
                Self::single(SdkOrderStatusIssueKind::PaymentBuyerMismatch, event_id)
            }
            RadrootsOrderIssue::PaymentSellerMismatch { event_id } => {
                Self::single(SdkOrderStatusIssueKind::PaymentSellerMismatch, event_id)
            }
            RadrootsOrderIssue::PaymentListingAddressInvalid { event_id } => Self::single(
                SdkOrderStatusIssueKind::PaymentListingAddressInvalid,
                event_id,
            ),
            RadrootsOrderIssue::PaymentListingMismatch { event_id } => {
                Self::single(SdkOrderStatusIssueKind::PaymentListingMismatch, event_id)
            }
            RadrootsOrderIssue::PaymentRootMismatch { event_id } => {
                Self::single(SdkOrderStatusIssueKind::PaymentRootMismatch, event_id)
            }
            RadrootsOrderIssue::PaymentPreviousMismatch { event_id } => {
                Self::single(SdkOrderStatusIssueKind::PaymentPreviousMismatch, event_id)
            }
            RadrootsOrderIssue::PaymentAgreementMismatch { event_id } => {
                Self::single(SdkOrderStatusIssueKind::PaymentAgreementMismatch, event_id)
            }
            RadrootsOrderIssue::PaymentQuoteMismatch { event_id } => {
                Self::single(SdkOrderStatusIssueKind::PaymentQuoteMismatch, event_id)
            }
            RadrootsOrderIssue::PaymentQuoteVersionMismatch { event_id } => Self::single(
                SdkOrderStatusIssueKind::PaymentQuoteVersionMismatch,
                event_id,
            ),
            RadrootsOrderIssue::PaymentEconomicsDigestMismatch { event_id } => Self::single(
                SdkOrderStatusIssueKind::PaymentEconomicsDigestMismatch,
                event_id,
            ),
            RadrootsOrderIssue::PaymentAmountMismatch { event_id } => {
                Self::single(SdkOrderStatusIssueKind::PaymentAmountMismatch, event_id)
            }
            RadrootsOrderIssue::PaymentCurrencyMismatch { event_id } => {
                Self::single(SdkOrderStatusIssueKind::PaymentCurrencyMismatch, event_id)
            }
            RadrootsOrderIssue::PaymentAfterCancellation { event_id } => {
                Self::single(SdkOrderStatusIssueKind::PaymentAfterCancellation, event_id)
            }
            RadrootsOrderIssue::RevisionAfterPayment { event_id } => {
                Self::single(SdkOrderStatusIssueKind::RevisionAfterPayment, event_id)
            }
            RadrootsOrderIssue::DuplicatePayments { event_ids } => {
                Self::new(SdkOrderStatusIssueKind::DuplicatePayments, event_ids)
            }
            RadrootsOrderIssue::SettlementWithoutValidPayment { event_id } => Self::single(
                SdkOrderStatusIssueKind::SettlementWithoutValidPayment,
                event_id,
            ),
            RadrootsOrderIssue::SettlementPayloadInvalid { event_id } => {
                Self::single(SdkOrderStatusIssueKind::SettlementPayloadInvalid, event_id)
            }
            RadrootsOrderIssue::SettlementOrderIdMismatch { event_id } => {
                Self::single(SdkOrderStatusIssueKind::SettlementOrderIdMismatch, event_id)
            }
            RadrootsOrderIssue::SettlementAuthorMismatch { event_id } => {
                Self::single(SdkOrderStatusIssueKind::SettlementAuthorMismatch, event_id)
            }
            RadrootsOrderIssue::SettlementCounterpartyMismatch { event_id } => Self::single(
                SdkOrderStatusIssueKind::SettlementCounterpartyMismatch,
                event_id,
            ),
            RadrootsOrderIssue::SettlementBuyerMismatch { event_id } => {
                Self::single(SdkOrderStatusIssueKind::SettlementBuyerMismatch, event_id)
            }
            RadrootsOrderIssue::SettlementSellerMismatch { event_id } => {
                Self::single(SdkOrderStatusIssueKind::SettlementSellerMismatch, event_id)
            }
            RadrootsOrderIssue::SettlementListingAddressInvalid { event_id } => Self::single(
                SdkOrderStatusIssueKind::SettlementListingAddressInvalid,
                event_id,
            ),
            RadrootsOrderIssue::SettlementListingMismatch { event_id } => {
                Self::single(SdkOrderStatusIssueKind::SettlementListingMismatch, event_id)
            }
            RadrootsOrderIssue::SettlementRootMismatch { event_id } => {
                Self::single(SdkOrderStatusIssueKind::SettlementRootMismatch, event_id)
            }
            RadrootsOrderIssue::SettlementPreviousMismatch { event_id } => Self::single(
                SdkOrderStatusIssueKind::SettlementPreviousMismatch,
                event_id,
            ),
            RadrootsOrderIssue::SettlementPaymentEventMismatch { event_id } => Self::single(
                SdkOrderStatusIssueKind::SettlementPaymentEventMismatch,
                event_id,
            ),
            RadrootsOrderIssue::SettlementAgreementMismatch { event_id } => Self::single(
                SdkOrderStatusIssueKind::SettlementAgreementMismatch,
                event_id,
            ),
            RadrootsOrderIssue::SettlementQuoteMismatch { event_id } => {
                Self::single(SdkOrderStatusIssueKind::SettlementQuoteMismatch, event_id)
            }
            RadrootsOrderIssue::SettlementQuoteVersionMismatch { event_id } => Self::single(
                SdkOrderStatusIssueKind::SettlementQuoteVersionMismatch,
                event_id,
            ),
            RadrootsOrderIssue::SettlementEconomicsDigestMismatch { event_id } => Self::single(
                SdkOrderStatusIssueKind::SettlementEconomicsDigestMismatch,
                event_id,
            ),
            RadrootsOrderIssue::SettlementAmountMismatch { event_id } => {
                Self::single(SdkOrderStatusIssueKind::SettlementAmountMismatch, event_id)
            }
            RadrootsOrderIssue::SettlementCurrencyMismatch { event_id } => Self::single(
                SdkOrderStatusIssueKind::SettlementCurrencyMismatch,
                event_id,
            ),
            RadrootsOrderIssue::DuplicateSettlements { event_ids } => {
                Self::new(SdkOrderStatusIssueKind::DuplicateSettlements, event_ids)
            }
            RadrootsOrderIssue::ForkedLifecycle { event_ids } => {
                Self::new(SdkOrderStatusIssueKind::ForkedLifecycle, event_ids)
            }
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
