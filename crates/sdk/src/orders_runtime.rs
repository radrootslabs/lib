#[cfg(feature = "runtime")]
use crate::{OrdersClient, RadrootsSdkError};
#[cfg(feature = "runtime")]
use radroots_events::{ids::RadrootsOrderId, order::RadrootsOrderFulfillmentState};
#[cfg(feature = "runtime")]
use radroots_trade::order::{
    RadrootsOrderPaymentState, RadrootsOrderProjection, RadrootsOrderSettlementState,
    RadrootsOrderStatus, RadrootsOrderStoreQueryError, order_projection_for_order_id,
};

#[cfg(feature = "runtime")]
pub const ORDER_STATUS_DEFAULT_LIMIT: u32 = 500;
#[cfg(feature = "runtime")]
pub const ORDER_STATUS_MAX_LIMIT: u32 = 1_000;

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrderStatusRequest {
    pub order_id: String,
    pub limit: u32,
}

#[cfg(feature = "runtime")]
impl OrderStatusRequest {
    pub fn new(order_id: impl Into<String>) -> Self {
        Self {
            order_id: order_id.into(),
            limit: ORDER_STATUS_DEFAULT_LIMIT,
        }
    }

    pub fn with_limit(mut self, limit: u32) -> Self {
        self.limit = limit;
        self
    }

    fn validate(&self) -> Result<RadrootsOrderId, RadrootsSdkError> {
        if self.limit == 0 || self.limit > ORDER_STATUS_MAX_LIMIT {
            return Err(RadrootsSdkError::InvalidRequest {
                message: format!(
                    "order status limit must be between 1 and {ORDER_STATUS_MAX_LIMIT}"
                ),
            });
        }
        RadrootsOrderId::parse(self.order_id.as_str()).map_err(|error| {
            RadrootsSdkError::InvalidRequest {
                message: format!("order_id is invalid: {error}"),
            }
        })
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrderStatusReceipt {
    pub order_id: String,
    pub found: bool,
    pub status: OrderStatusKind,
    pub fulfillment_status: Option<OrderFulfillmentStatusKind>,
    pub payment_state: OrderPaymentStateKind,
    pub settlement_state: OrderSettlementStateKind,
    pub lifecycle_terminal: bool,
    pub request_event_id: Option<String>,
    pub decision_event_id: Option<String>,
    pub fulfillment_event_id: Option<String>,
    pub cancellation_event_id: Option<String>,
    pub receipt_event_id: Option<String>,
    pub last_event_id: Option<String>,
    pub issue_count: usize,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrderFulfillmentStatusKind {
    AcceptedNotFulfilled,
    Preparing,
    ReadyForPickup,
    OutForDelivery,
    Delivered,
    SellerCancelled,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrderPaymentStateKind {
    NotRecorded,
    Recorded,
    Settled,
    Rejected,
    Invalid,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrderSettlementStateKind {
    NotRequired,
    Pending,
    Accepted,
    Rejected,
    Invalid,
}

#[cfg(feature = "runtime")]
impl<'sdk> OrdersClient<'sdk> {
    pub async fn status(
        &self,
        request: OrderStatusRequest,
    ) -> Result<OrderStatusReceipt, RadrootsSdkError> {
        let order_id = request.validate()?;
        let projection =
            order_projection_for_order_id(&self.sdk._event_store, &order_id, request.limit)
                .await
                .map_err(projection_error)?;
        Ok(OrderStatusReceipt::from_projection(projection))
    }
}

#[cfg(feature = "runtime")]
impl OrderStatusReceipt {
    fn from_projection(projection: RadrootsOrderProjection) -> Self {
        let found = projection.status != RadrootsOrderStatus::Missing;
        Self {
            order_id: projection.order_id.into_string(),
            found,
            status: projection.status.into(),
            fulfillment_status: projection.fulfillment_status.map(Into::into),
            payment_state: projection.payment.state.into(),
            settlement_state: projection.payment.settlement_state.into(),
            lifecycle_terminal: projection.lifecycle_terminal,
            request_event_id: projection.request_event_id.map(Into::into),
            decision_event_id: projection.decision_event_id.map(Into::into),
            fulfillment_event_id: projection.fulfillment_event_id.map(Into::into),
            cancellation_event_id: projection.cancellation_event_id.map(Into::into),
            receipt_event_id: projection.receipt_event_id.map(Into::into),
            last_event_id: projection.last_event_id.map(Into::into),
            issue_count: projection.issues.len(),
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
