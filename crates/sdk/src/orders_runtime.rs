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
    order::{RadrootsOrderDecision, RadrootsOrderFulfillmentState, RadrootsOrderRequest},
};
#[cfg(feature = "runtime")]
use radroots_events_codec::wire::to_frozen_draft;
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
const ORDER_REQUEST_CONTRACT_ID: &str = "radroots.order.request.v1";
#[cfg(feature = "runtime")]
const ORDER_DECISION_CONTRACT_ID: &str = "radroots.order.decision.v1";

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
    pub event_ids: Vec<RadrootsEventId>,
    pub request_event_id: Option<RadrootsEventId>,
    pub decision_event_id: Option<RadrootsEventId>,
    pub fulfillment_event_id: Option<RadrootsEventId>,
    pub cancellation_event_id: Option<RadrootsEventId>,
    pub receipt_event_id: Option<RadrootsEventId>,
    pub last_event_id: Option<RadrootsEventId>,
    pub issues: Vec<SdkOrderStatusIssue>,
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
                operation_kind: ORDER_SUBMIT_OPERATION_KIND,
                actor,
                frozen_draft: &plan.frozen_draft,
                target_relays,
                idempotency_key,
            },
            signer,
        )
        .await?;
        Ok(OrderSubmitReceipt {
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
        self.require_decision_preflight(&plan).await?;
        let enqueue = enqueue_signed_workflow(
            self.sdk,
            SdkWorkflowEnqueueRequest {
                operation_kind: ORDER_DECISION_OPERATION_KIND,
                actor,
                frozen_draft: &plan.frozen_draft,
                target_relays,
                idempotency_key,
            },
            signer,
        )
        .await?;
        Ok(OrderDecisionReceipt {
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
        let query_result = order_projection_query_for_order_id(
            &self.sdk._event_store,
            &plan.order_id,
            ORDER_STATUS_MAX_LIMIT,
        )
        .await
        .map_err(projection_error)?;
        require_decision_request_evidence(plan, &query_result.projection)
    }
}

#[cfg(feature = "runtime")]
impl OrderStatusReceipt {
    fn from_query_result(query_result: RadrootsOrderProjectionQueryResult) -> Self {
        let projection = query_result.projection;
        let found = projection.status != RadrootsOrderStatus::Missing;
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
            event_ids: query_result.event_ids,
            request_event_id: projection.request_event_id,
            decision_event_id: projection.decision_event_id,
            fulfillment_event_id: projection.fulfillment_event_id,
            cancellation_event_id: projection.cancellation_event_id,
            receipt_event_id: projection.receipt_event_id,
            last_event_id: projection.last_event_id,
            issues: projection.issues.into_iter().map(Into::into).collect(),
        }
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
        "listing_addr",
        projection.listing_addr.as_ref(),
        &plan.listing_addr,
        &plan.order_id,
    )?;
    require_projection_match(
        "buyer_pubkey",
        projection.buyer_pubkey.as_ref(),
        &plan.buyer_pubkey,
        &plan.order_id,
    )?;
    require_projection_match(
        "seller_pubkey",
        projection.seller_pubkey.as_ref(),
        &plan.seller_pubkey,
        &plan.order_id,
    )?;
    Ok(())
}

#[cfg(feature = "runtime")]
fn require_projection_match<T>(
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
                "order decision {field} {expected} does not match local request {actual} for order {order_id}"
            ),
        }),
        None => Err(RadrootsSdkError::InvalidRequest {
            message: format!(
                "order decision request evidence is missing {field} for order {order_id}"
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
