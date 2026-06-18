#[cfg(feature = "runtime")]
use crate::{OrdersClient, RadrootsSdkError};
#[cfg(feature = "runtime")]
use radroots_events::{
    ids::{RadrootsEventId, RadrootsOrderId},
    order::RadrootsOrderFulfillmentState,
};
#[cfg(feature = "runtime")]
use radroots_trade::order::{
    RadrootsOrderIssue, RadrootsOrderPaymentState, RadrootsOrderProjectionQueryResult,
    RadrootsOrderSettlementState, RadrootsOrderStatus, RadrootsOrderStoreQueryError,
    order_projection_query_for_order_id,
};
#[cfg(feature = "runtime")]
use serde::ser::SerializeStruct;

#[cfg(feature = "runtime")]
pub const ORDER_STATUS_DEFAULT_LIMIT: u32 = 500;
#[cfg(feature = "runtime")]
pub const ORDER_STATUS_MAX_LIMIT: u32 = 1_000;

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
