#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
use alloc::{collections::BTreeMap, format, string::String, vec::Vec};
#[cfg(feature = "std")]
use std::collections::BTreeMap;

use radroots_core::{RadrootsCoreDecimal, RadrootsCoreDiscount, RadrootsCoreDiscountValue};
use radroots_events::{
    kinds::KIND_LISTING,
    listing::{
        RadrootsListing, RadrootsListingAvailability, RadrootsListingBin,
        RadrootsListingDeliveryMethod, RadrootsListingFarmRef, RadrootsListingImage,
        RadrootsListingLocation, RadrootsListingProduct,
    },
    plot::RadrootsPlotRef,
    resource_area::RadrootsResourceAreaRef,
};
#[cfg(feature = "ts-rs")]
use ts_rs::TS;

use crate::listing::{
    dvm::{TradeListingMessagePayload, TradeListingMessageType},
    model::RadrootsTradeListingTotal,
    order::{
        TradeFulfillmentStatus, TradeOrder, TradeOrderChange, TradeOrderItem, TradeOrderStatus,
    },
    price_ext::BinPricingExt,
};

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsTradeListingBinProjection {
    #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsListingBin"))]
    pub bin: RadrootsListingBin,
    #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsTradeListingTotal"))]
    pub one_bin_total: RadrootsTradeListingTotal,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsTradeListingProjection {
    pub listing_addr: String,
    pub seller_pubkey: String,
    pub listing_id: String,
    #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsListingFarmRef"))]
    pub farm: RadrootsListingFarmRef,
    #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsListingProduct"))]
    pub product: RadrootsListingProduct,
    pub primary_bin_id: String,
    #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsTradeListingBinProjection[]"))]
    pub bins: Vec<RadrootsTradeListingBinProjection>,
    #[cfg_attr(
        feature = "ts-rs",
        ts(optional, type = "RadrootsResourceAreaRef | null")
    )]
    pub resource_area: Option<RadrootsResourceAreaRef>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "RadrootsPlotRef | null"))]
    pub plot: Option<RadrootsPlotRef>,
    #[cfg_attr(
        feature = "ts-rs",
        ts(optional, type = "RadrootsCoreDiscount[] | null")
    )]
    pub discounts: Option<Vec<RadrootsCoreDiscount>>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "RadrootsCoreDecimal | null"))]
    pub inventory_available: Option<RadrootsCoreDecimal>,
    #[cfg_attr(
        feature = "ts-rs",
        ts(optional, type = "RadrootsListingAvailability | null")
    )]
    pub availability: Option<RadrootsListingAvailability>,
    #[cfg_attr(
        feature = "ts-rs",
        ts(optional, type = "RadrootsListingDeliveryMethod | null")
    )]
    pub delivery_method: Option<RadrootsListingDeliveryMethod>,
    #[cfg_attr(
        feature = "ts-rs",
        ts(optional, type = "RadrootsListingLocation | null")
    )]
    pub location: Option<RadrootsListingLocation>,
    #[cfg_attr(
        feature = "ts-rs",
        ts(optional, type = "RadrootsListingImage[] | null")
    )]
    pub images: Option<Vec<RadrootsListingImage>>,
    pub order_count: u32,
    pub open_order_count: u32,
    pub terminal_order_count: u32,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeOrderWorkflowProjection {
    pub order_id: String,
    pub listing_addr: String,
    pub buyer_pubkey: String,
    pub seller_pubkey: String,
    #[cfg_attr(feature = "ts-rs", ts(type = "TradeOrderItem[]"))]
    pub items: Vec<TradeOrderItem>,
    #[cfg_attr(
        feature = "ts-rs",
        ts(optional, type = "RadrootsCoreDiscountValue[] | null")
    )]
    pub requested_discounts: Option<Vec<RadrootsCoreDiscountValue>>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub notes: Option<String>,
    pub status: TradeOrderStatus,
    #[cfg_attr(
        feature = "ts-rs",
        ts(optional, type = "RadrootsCoreDiscountValue | null")
    )]
    pub last_discount_request: Option<RadrootsCoreDiscountValue>,
    #[cfg_attr(
        feature = "ts-rs",
        ts(optional, type = "RadrootsCoreDiscountValue | null")
    )]
    pub last_discount_offer: Option<RadrootsCoreDiscountValue>,
    #[cfg_attr(
        feature = "ts-rs",
        ts(optional, type = "RadrootsCoreDiscountValue | null")
    )]
    pub accepted_discount: Option<RadrootsCoreDiscountValue>,
    #[cfg_attr(
        feature = "ts-rs",
        ts(optional, type = "TradeFulfillmentStatus | null")
    )]
    pub last_fulfillment_status: Option<TradeFulfillmentStatus>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "bool | null"))]
    pub receipt_acknowledged: Option<bool>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "number | null"))]
    pub receipt_at: Option<u64>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub last_reason: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub last_discount_decline_reason: Option<String>,
    pub question_count: u32,
    pub answer_count: u32,
    pub revision_count: u32,
    pub discount_request_count: u32,
    pub discount_offer_count: u32,
    pub discount_accept_count: u32,
    pub discount_decline_count: u32,
    pub cancellation_count: u32,
    pub fulfillment_update_count: u32,
    pub receipt_count: u32,
    pub last_message_type: TradeListingMessageType,
    pub last_actor_pubkey: String,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeOrderWorkflowMessage {
    pub actor_pubkey: String,
    pub listing_addr: String,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub order_id: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(type = "TradeListingMessagePayload"))]
    pub payload: TradeListingMessagePayload,
}

#[derive(Clone, Debug, Default)]
pub struct RadrootsTradeReadIndex {
    listings: BTreeMap<String, RadrootsTradeListingProjection>,
    orders: BTreeMap<String, RadrootsTradeOrderWorkflowProjection>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsTradeProjectionError {
    MissingPrimaryBin(String),
    MissingOrderId,
    OrderIdMismatch,
    ListingAddrMismatch,
    MissingOrder(String),
    InvalidTransition {
        from: TradeOrderStatus,
        to: TradeOrderStatus,
    },
    InvalidItemIndex(u32),
    InvalidDiscountDecision,
    InvalidRevisionResponse,
    NonOrderWorkflowMessage(TradeListingMessageType),
    UnauthorizedActor,
}

impl core::fmt::Display for RadrootsTradeProjectionError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            RadrootsTradeProjectionError::MissingPrimaryBin(bin_id) => {
                write!(f, "missing primary bin: {bin_id}")
            }
            RadrootsTradeProjectionError::MissingOrderId => write!(f, "missing order id"),
            RadrootsTradeProjectionError::OrderIdMismatch => write!(f, "order id mismatch"),
            RadrootsTradeProjectionError::ListingAddrMismatch => {
                write!(f, "listing address mismatch")
            }
            RadrootsTradeProjectionError::MissingOrder(order_id) => {
                write!(f, "missing order projection: {order_id}")
            }
            RadrootsTradeProjectionError::InvalidTransition { from, to } => {
                write!(f, "invalid order transition: {from:?} -> {to:?}")
            }
            RadrootsTradeProjectionError::InvalidItemIndex(index) => {
                write!(f, "invalid order item index: {index}")
            }
            RadrootsTradeProjectionError::InvalidDiscountDecision => {
                write!(f, "invalid discount decision payload")
            }
            RadrootsTradeProjectionError::InvalidRevisionResponse => {
                write!(f, "invalid order revision response payload")
            }
            RadrootsTradeProjectionError::NonOrderWorkflowMessage(message_type) => {
                write!(f, "non-order workflow message: {message_type:?}")
            }
            RadrootsTradeProjectionError::UnauthorizedActor => write!(f, "unauthorized actor"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsTradeProjectionError {}

impl RadrootsTradeListingProjection {
    pub fn from_listing_contract(
        seller_pubkey: impl Into<String>,
        listing: &RadrootsListing,
    ) -> Result<Self, RadrootsTradeProjectionError> {
        let seller_pubkey = seller_pubkey.into();
        if !listing
            .bins
            .iter()
            .any(|bin| bin.bin_id == listing.primary_bin_id)
        {
            return Err(RadrootsTradeProjectionError::MissingPrimaryBin(
                listing.primary_bin_id.clone(),
            ));
        }

        let bins = listing
            .bins
            .iter()
            .cloned()
            .map(|bin| RadrootsTradeListingBinProjection {
                one_bin_total: bin.total_for_count(1),
                bin,
            })
            .collect();

        Ok(Self {
            listing_addr: format!("{KIND_LISTING}:{}:{}", seller_pubkey, listing.d_tag),
            seller_pubkey,
            listing_id: listing.d_tag.clone(),
            farm: listing.farm.clone(),
            product: listing.product.clone(),
            primary_bin_id: listing.primary_bin_id.clone(),
            bins,
            resource_area: listing.resource_area.clone(),
            plot: listing.plot.clone(),
            discounts: listing.discounts.clone(),
            inventory_available: listing.inventory_available.clone(),
            availability: listing.availability.clone(),
            delivery_method: listing.delivery_method.clone(),
            location: listing.location.clone(),
            images: listing.images.clone(),
            order_count: 0,
            open_order_count: 0,
            terminal_order_count: 0,
        })
    }
}

impl RadrootsTradeOrderWorkflowProjection {
    pub fn is_terminal(&self) -> bool {
        radroots_trade_order_status_is_terminal(&self.status)
    }

    fn from_order_request(order: &TradeOrder) -> Self {
        Self {
            order_id: order.order_id.clone(),
            listing_addr: order.listing_addr.clone(),
            buyer_pubkey: order.buyer_pubkey.clone(),
            seller_pubkey: order.seller_pubkey.clone(),
            items: order.items.clone(),
            requested_discounts: order.discounts.clone(),
            notes: order.notes.clone(),
            status: TradeOrderStatus::Requested,
            last_discount_request: None,
            last_discount_offer: None,
            accepted_discount: None,
            last_fulfillment_status: None,
            receipt_acknowledged: None,
            receipt_at: None,
            last_reason: None,
            last_discount_decline_reason: None,
            question_count: 0,
            answer_count: 0,
            revision_count: 0,
            discount_request_count: 0,
            discount_offer_count: 0,
            discount_accept_count: 0,
            discount_decline_count: 0,
            cancellation_count: 0,
            fulfillment_update_count: 0,
            receipt_count: 0,
            last_message_type: TradeListingMessageType::OrderRequest,
            last_actor_pubkey: order.buyer_pubkey.clone(),
        }
    }
}

impl RadrootsTradeOrderWorkflowMessage {
    pub fn message_type(&self) -> TradeListingMessageType {
        match &self.payload {
            TradeListingMessagePayload::ListingValidateRequest(_) => {
                TradeListingMessageType::ListingValidateRequest
            }
            TradeListingMessagePayload::ListingValidateResult(_) => {
                TradeListingMessageType::ListingValidateResult
            }
            TradeListingMessagePayload::OrderRequest(_) => TradeListingMessageType::OrderRequest,
            TradeListingMessagePayload::OrderResponse(_) => TradeListingMessageType::OrderResponse,
            TradeListingMessagePayload::OrderRevision(_) => TradeListingMessageType::OrderRevision,
            TradeListingMessagePayload::OrderRevisionAccept(_) => {
                TradeListingMessageType::OrderRevisionAccept
            }
            TradeListingMessagePayload::OrderRevisionDecline(_) => {
                TradeListingMessageType::OrderRevisionDecline
            }
            TradeListingMessagePayload::Question(_) => TradeListingMessageType::Question,
            TradeListingMessagePayload::Answer(_) => TradeListingMessageType::Answer,
            TradeListingMessagePayload::DiscountRequest(_) => {
                TradeListingMessageType::DiscountRequest
            }
            TradeListingMessagePayload::DiscountOffer(_) => TradeListingMessageType::DiscountOffer,
            TradeListingMessagePayload::DiscountAccept(_) => {
                TradeListingMessageType::DiscountAccept
            }
            TradeListingMessagePayload::DiscountDecline(_) => {
                TradeListingMessageType::DiscountDecline
            }
            TradeListingMessagePayload::Cancel(_) => TradeListingMessageType::Cancel,
            TradeListingMessagePayload::FulfillmentUpdate(_) => {
                TradeListingMessageType::FulfillmentUpdate
            }
            TradeListingMessagePayload::Receipt(_) => TradeListingMessageType::Receipt,
        }
    }
}

impl RadrootsTradeReadIndex {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn listings(&self) -> &BTreeMap<String, RadrootsTradeListingProjection> {
        &self.listings
    }

    pub fn orders(&self) -> &BTreeMap<String, RadrootsTradeOrderWorkflowProjection> {
        &self.orders
    }

    pub fn listing(&self, listing_addr: &str) -> Option<&RadrootsTradeListingProjection> {
        self.listings.get(listing_addr)
    }

    pub fn order(&self, order_id: &str) -> Option<&RadrootsTradeOrderWorkflowProjection> {
        self.orders.get(order_id)
    }

    pub fn upsert_listing(
        &mut self,
        seller_pubkey: impl Into<String>,
        listing: &RadrootsListing,
    ) -> Result<&RadrootsTradeListingProjection, RadrootsTradeProjectionError> {
        let projection =
            RadrootsTradeListingProjection::from_listing_contract(seller_pubkey, listing)?;
        let listing_addr = projection.listing_addr.clone();
        self.listings.insert(listing_addr.clone(), projection);
        self.refresh_listing_counts(&listing_addr);
        Ok(self
            .listings
            .get(&listing_addr)
            .expect("listing projection should exist after upsert"))
    }

    pub fn apply_workflow_message(
        &mut self,
        message: &RadrootsTradeOrderWorkflowMessage,
    ) -> Result<&RadrootsTradeOrderWorkflowProjection, RadrootsTradeProjectionError> {
        let order_id = self.apply_workflow_message_inner(message)?;
        let listing_addr = self
            .orders
            .get(&order_id)
            .expect("order projection should exist after workflow apply")
            .listing_addr
            .clone();
        self.refresh_listing_counts(&listing_addr);
        Ok(self
            .orders
            .get(&order_id)
            .expect("order projection should exist after workflow apply"))
    }

    fn apply_workflow_message_inner(
        &mut self,
        message: &RadrootsTradeOrderWorkflowMessage,
    ) -> Result<String, RadrootsTradeProjectionError> {
        match &message.payload {
            TradeListingMessagePayload::ListingValidateRequest(_)
            | TradeListingMessagePayload::ListingValidateResult(_) => Err(
                RadrootsTradeProjectionError::NonOrderWorkflowMessage(message.message_type()),
            ),
            TradeListingMessagePayload::OrderRequest(order) => {
                self.apply_order_request(message, order)
            }
            TradeListingMessagePayload::OrderResponse(response) => {
                let order_id = required_order_id(message)?;
                let order = self.order_mut_checked(order_id, &message.listing_addr)?;
                ensure_actor(&order.seller_pubkey, &message.actor_pubkey)?;
                let next_status = if response.accepted {
                    TradeOrderStatus::Accepted
                } else {
                    TradeOrderStatus::Declined
                };
                radroots_trade_order_status_ensure_transition(
                    order.status.clone(),
                    next_status.clone(),
                )?;
                order.status = next_status;
                order.last_message_type = TradeListingMessageType::OrderResponse;
                order.last_actor_pubkey = message.actor_pubkey.clone();
                order.last_reason = response.reason.clone();
                Ok(order_id.to_string())
            }
            TradeListingMessagePayload::OrderRevision(revision) => {
                let order_id = required_order_id(message)?;
                if revision.order_id != order_id {
                    return Err(RadrootsTradeProjectionError::OrderIdMismatch);
                }
                let order = self.order_mut_checked(order_id, &message.listing_addr)?;
                ensure_actor(&order.seller_pubkey, &message.actor_pubkey)?;
                radroots_trade_order_status_ensure_transition(
                    order.status.clone(),
                    TradeOrderStatus::Revised,
                )?;
                for change in &revision.changes {
                    apply_order_change(&mut order.items, change)?;
                }
                order.status = TradeOrderStatus::Revised;
                order.revision_count = order.revision_count.saturating_add(1);
                order.last_message_type = TradeListingMessageType::OrderRevision;
                order.last_actor_pubkey = message.actor_pubkey.clone();
                order.last_reason = revision.reason.clone();
                Ok(order_id.to_string())
            }
            TradeListingMessagePayload::OrderRevisionAccept(response) => {
                let order_id = required_order_id(message)?;
                if !response.accepted {
                    return Err(RadrootsTradeProjectionError::InvalidRevisionResponse);
                }
                let order = self.order_mut_checked(order_id, &message.listing_addr)?;
                ensure_actor(&order.buyer_pubkey, &message.actor_pubkey)?;
                radroots_trade_order_status_ensure_transition(
                    order.status.clone(),
                    TradeOrderStatus::Accepted,
                )?;
                order.status = TradeOrderStatus::Accepted;
                order.last_message_type = TradeListingMessageType::OrderRevisionAccept;
                order.last_actor_pubkey = message.actor_pubkey.clone();
                order.last_reason = response.reason.clone();
                Ok(order_id.to_string())
            }
            TradeListingMessagePayload::OrderRevisionDecline(response) => {
                let order_id = required_order_id(message)?;
                if response.accepted {
                    return Err(RadrootsTradeProjectionError::InvalidRevisionResponse);
                }
                let order = self.order_mut_checked(order_id, &message.listing_addr)?;
                ensure_actor(&order.buyer_pubkey, &message.actor_pubkey)?;
                radroots_trade_order_status_ensure_transition(
                    order.status.clone(),
                    TradeOrderStatus::Declined,
                )?;
                order.status = TradeOrderStatus::Declined;
                order.last_message_type = TradeListingMessageType::OrderRevisionDecline;
                order.last_actor_pubkey = message.actor_pubkey.clone();
                order.last_reason = response.reason.clone();
                Ok(order_id.to_string())
            }
            TradeListingMessagePayload::Question(question) => {
                let order_id = required_order_id(message)?;
                if question
                    .order_id
                    .as_deref()
                    .is_some_and(|value| value != order_id)
                {
                    return Err(RadrootsTradeProjectionError::OrderIdMismatch);
                }
                let order = self.order_mut_checked(order_id, &message.listing_addr)?;
                ensure_actor(&order.buyer_pubkey, &message.actor_pubkey)?;
                radroots_trade_order_status_ensure_transition(
                    order.status.clone(),
                    TradeOrderStatus::Questioned,
                )?;
                order.status = TradeOrderStatus::Questioned;
                order.question_count = order.question_count.saturating_add(1);
                order.last_message_type = TradeListingMessageType::Question;
                order.last_actor_pubkey = message.actor_pubkey.clone();
                Ok(order_id.to_string())
            }
            TradeListingMessagePayload::Answer(answer) => {
                let order_id = required_order_id(message)?;
                if answer
                    .order_id
                    .as_deref()
                    .is_some_and(|value| value != order_id)
                {
                    return Err(RadrootsTradeProjectionError::OrderIdMismatch);
                }
                let order = self.order_mut_checked(order_id, &message.listing_addr)?;
                ensure_actor(&order.seller_pubkey, &message.actor_pubkey)?;
                radroots_trade_order_status_ensure_transition(
                    order.status.clone(),
                    TradeOrderStatus::Requested,
                )?;
                order.status = TradeOrderStatus::Requested;
                order.answer_count = order.answer_count.saturating_add(1);
                order.last_message_type = TradeListingMessageType::Answer;
                order.last_actor_pubkey = message.actor_pubkey.clone();
                Ok(order_id.to_string())
            }
            TradeListingMessagePayload::DiscountRequest(request) => {
                let order_id = required_order_id(message)?;
                if request.order_id != order_id {
                    return Err(RadrootsTradeProjectionError::OrderIdMismatch);
                }
                let order = self.order_mut_checked(order_id, &message.listing_addr)?;
                ensure_actor(&order.buyer_pubkey, &message.actor_pubkey)?;
                order.discount_request_count = order.discount_request_count.saturating_add(1);
                order.last_discount_request = Some(request.value.clone());
                order.last_message_type = TradeListingMessageType::DiscountRequest;
                order.last_actor_pubkey = message.actor_pubkey.clone();
                order.last_reason = request.conditions.clone();
                Ok(order_id.to_string())
            }
            TradeListingMessagePayload::DiscountOffer(offer) => {
                let order_id = required_order_id(message)?;
                if offer.order_id != order_id {
                    return Err(RadrootsTradeProjectionError::OrderIdMismatch);
                }
                let order = self.order_mut_checked(order_id, &message.listing_addr)?;
                ensure_actor(&order.seller_pubkey, &message.actor_pubkey)?;
                radroots_trade_order_status_ensure_transition(
                    order.status.clone(),
                    TradeOrderStatus::Revised,
                )?;
                order.status = TradeOrderStatus::Revised;
                order.discount_offer_count = order.discount_offer_count.saturating_add(1);
                order.last_discount_offer = Some(offer.value.clone());
                order.last_message_type = TradeListingMessageType::DiscountOffer;
                order.last_actor_pubkey = message.actor_pubkey.clone();
                order.last_reason = offer.conditions.clone();
                Ok(order_id.to_string())
            }
            TradeListingMessagePayload::DiscountAccept(decision) => {
                let order_id = required_order_id(message)?;
                let order = self.order_mut_checked(order_id, &message.listing_addr)?;
                ensure_actor(&order.buyer_pubkey, &message.actor_pubkey)?;
                let TradeDiscountDecisionValue::Accepted(value) =
                    trade_discount_decision_value(decision)?
                else {
                    return Err(RadrootsTradeProjectionError::InvalidDiscountDecision);
                };
                radroots_trade_order_status_ensure_transition(
                    order.status.clone(),
                    TradeOrderStatus::Accepted,
                )?;
                order.status = TradeOrderStatus::Accepted;
                order.discount_accept_count = order.discount_accept_count.saturating_add(1);
                order.accepted_discount = Some(value);
                order.last_discount_decline_reason = None;
                order.last_message_type = TradeListingMessageType::DiscountAccept;
                order.last_actor_pubkey = message.actor_pubkey.clone();
                order.last_reason = None;
                Ok(order_id.to_string())
            }
            TradeListingMessagePayload::DiscountDecline(decision) => {
                let order_id = required_order_id(message)?;
                let order = self.order_mut_checked(order_id, &message.listing_addr)?;
                ensure_actor(&order.buyer_pubkey, &message.actor_pubkey)?;
                let TradeDiscountDecisionValue::Declined(reason) =
                    trade_discount_decision_value(decision)?
                else {
                    return Err(RadrootsTradeProjectionError::InvalidDiscountDecision);
                };
                radroots_trade_order_status_ensure_transition(
                    order.status.clone(),
                    TradeOrderStatus::Requested,
                )?;
                order.status = TradeOrderStatus::Requested;
                order.discount_decline_count = order.discount_decline_count.saturating_add(1);
                order.last_discount_decline_reason = reason.clone();
                order.last_message_type = TradeListingMessageType::DiscountDecline;
                order.last_actor_pubkey = message.actor_pubkey.clone();
                order.last_reason = reason;
                Ok(order_id.to_string())
            }
            TradeListingMessagePayload::Cancel(cancel) => {
                let order_id = required_order_id(message)?;
                let order = self.order_mut_checked(order_id, &message.listing_addr)?;
                if order.buyer_pubkey != message.actor_pubkey
                    && order.seller_pubkey != message.actor_pubkey
                {
                    return Err(RadrootsTradeProjectionError::UnauthorizedActor);
                }
                radroots_trade_order_status_ensure_transition(
                    order.status.clone(),
                    TradeOrderStatus::Cancelled,
                )?;
                order.status = TradeOrderStatus::Cancelled;
                order.cancellation_count = order.cancellation_count.saturating_add(1);
                order.last_message_type = TradeListingMessageType::Cancel;
                order.last_actor_pubkey = message.actor_pubkey.clone();
                order.last_reason = cancel.reason.clone();
                Ok(order_id.to_string())
            }
            TradeListingMessagePayload::FulfillmentUpdate(update) => {
                let order_id = required_order_id(message)?;
                let order = self.order_mut_checked(order_id, &message.listing_addr)?;
                ensure_actor(&order.seller_pubkey, &message.actor_pubkey)?;
                radroots_trade_order_status_ensure_transition(
                    order.status.clone(),
                    TradeOrderStatus::Fulfilled,
                )?;
                order.status = TradeOrderStatus::Fulfilled;
                order.fulfillment_update_count = order.fulfillment_update_count.saturating_add(1);
                order.last_fulfillment_status = Some(update.status.clone());
                order.last_message_type = TradeListingMessageType::FulfillmentUpdate;
                order.last_actor_pubkey = message.actor_pubkey.clone();
                order.last_reason = update.notes.clone();
                Ok(order_id.to_string())
            }
            TradeListingMessagePayload::Receipt(receipt) => {
                let order_id = required_order_id(message)?;
                let order = self.order_mut_checked(order_id, &message.listing_addr)?;
                ensure_actor(&order.buyer_pubkey, &message.actor_pubkey)?;
                radroots_trade_order_status_ensure_transition(
                    order.status.clone(),
                    TradeOrderStatus::Completed,
                )?;
                order.status = TradeOrderStatus::Completed;
                order.receipt_count = order.receipt_count.saturating_add(1);
                order.receipt_acknowledged = Some(receipt.acknowledged);
                order.receipt_at = Some(receipt.at);
                order.last_message_type = TradeListingMessageType::Receipt;
                order.last_actor_pubkey = message.actor_pubkey.clone();
                order.last_reason = receipt.note.clone();
                Ok(order_id.to_string())
            }
        }
    }

    fn apply_order_request(
        &mut self,
        message: &RadrootsTradeOrderWorkflowMessage,
        order: &TradeOrder,
    ) -> Result<String, RadrootsTradeProjectionError> {
        if message
            .order_id
            .as_deref()
            .is_some_and(|value| value != order.order_id)
        {
            return Err(RadrootsTradeProjectionError::OrderIdMismatch);
        }
        if message.listing_addr != order.listing_addr {
            return Err(RadrootsTradeProjectionError::ListingAddrMismatch);
        }
        ensure_actor(&order.buyer_pubkey, &message.actor_pubkey)?;

        if let Some(existing) = self.orders.get(&order.order_id) {
            if existing.listing_addr != order.listing_addr
                || existing.buyer_pubkey != order.buyer_pubkey
                || existing.seller_pubkey != order.seller_pubkey
            {
                return Err(RadrootsTradeProjectionError::ListingAddrMismatch);
            }
            return Ok(order.order_id.clone());
        }

        self.orders.insert(
            order.order_id.clone(),
            RadrootsTradeOrderWorkflowProjection::from_order_request(order),
        );
        Ok(order.order_id.clone())
    }

    fn order_mut_checked(
        &mut self,
        order_id: &str,
        listing_addr: &str,
    ) -> Result<&mut RadrootsTradeOrderWorkflowProjection, RadrootsTradeProjectionError> {
        let order = self
            .orders
            .get_mut(order_id)
            .ok_or_else(|| RadrootsTradeProjectionError::MissingOrder(order_id.to_string()))?;
        if order.listing_addr != listing_addr {
            return Err(RadrootsTradeProjectionError::ListingAddrMismatch);
        }
        Ok(order)
    }

    fn refresh_listing_counts(&mut self, listing_addr: &str) {
        let Some(listing) = self.listings.get_mut(listing_addr) else {
            return;
        };

        let mut order_count = 0u32;
        let mut open_order_count = 0u32;
        let mut terminal_order_count = 0u32;

        for order in self.orders.values() {
            if order.listing_addr != listing_addr {
                continue;
            }
            order_count = order_count.saturating_add(1);
            if order.is_terminal() {
                terminal_order_count = terminal_order_count.saturating_add(1);
            } else {
                open_order_count = open_order_count.saturating_add(1);
            }
        }

        listing.order_count = order_count;
        listing.open_order_count = open_order_count;
        listing.terminal_order_count = terminal_order_count;
    }
}

pub fn radroots_trade_order_status_can_transition(
    from: &TradeOrderStatus,
    to: &TradeOrderStatus,
) -> bool {
    if from == to {
        return true;
    }

    match from {
        TradeOrderStatus::Draft => matches!(to, TradeOrderStatus::Requested),
        TradeOrderStatus::Validated => matches!(to, TradeOrderStatus::Requested),
        TradeOrderStatus::Requested => matches!(
            to,
            TradeOrderStatus::Accepted
                | TradeOrderStatus::Declined
                | TradeOrderStatus::Questioned
                | TradeOrderStatus::Revised
                | TradeOrderStatus::Cancelled
                | TradeOrderStatus::Requested
        ),
        TradeOrderStatus::Questioned => matches!(
            to,
            TradeOrderStatus::Requested | TradeOrderStatus::Revised | TradeOrderStatus::Cancelled
        ),
        TradeOrderStatus::Revised => matches!(
            to,
            TradeOrderStatus::Accepted
                | TradeOrderStatus::Declined
                | TradeOrderStatus::Cancelled
                | TradeOrderStatus::Requested
        ),
        TradeOrderStatus::Accepted => {
            matches!(
                to,
                TradeOrderStatus::Fulfilled | TradeOrderStatus::Cancelled
            )
        }
        TradeOrderStatus::Declined => false,
        TradeOrderStatus::Cancelled => false,
        TradeOrderStatus::Fulfilled => matches!(
            to,
            TradeOrderStatus::Completed | TradeOrderStatus::Fulfilled | TradeOrderStatus::Cancelled
        ),
        TradeOrderStatus::Completed => false,
    }
}

pub fn radroots_trade_order_status_is_terminal(status: &TradeOrderStatus) -> bool {
    matches!(
        status,
        TradeOrderStatus::Declined | TradeOrderStatus::Cancelled | TradeOrderStatus::Completed
    )
}

pub fn radroots_trade_order_status_ensure_transition(
    from: TradeOrderStatus,
    to: TradeOrderStatus,
) -> Result<(), RadrootsTradeProjectionError> {
    if radroots_trade_order_status_can_transition(&from, &to) {
        Ok(())
    } else {
        Err(RadrootsTradeProjectionError::InvalidTransition { from, to })
    }
}

fn required_order_id(
    message: &RadrootsTradeOrderWorkflowMessage,
) -> Result<&str, RadrootsTradeProjectionError> {
    message
        .order_id
        .as_deref()
        .ok_or(RadrootsTradeProjectionError::MissingOrderId)
}

fn ensure_actor(expected: &str, actual: &str) -> Result<(), RadrootsTradeProjectionError> {
    if expected == actual {
        Ok(())
    } else {
        Err(RadrootsTradeProjectionError::UnauthorizedActor)
    }
}

fn apply_order_change(
    items: &mut Vec<TradeOrderItem>,
    change: &TradeOrderChange,
) -> Result<(), RadrootsTradeProjectionError> {
    match change {
        TradeOrderChange::BinCount {
            item_index,
            bin_count,
        } => {
            let index = usize::try_from(*item_index)
                .map_err(|_| RadrootsTradeProjectionError::InvalidItemIndex(*item_index))?;
            let item = items
                .get_mut(index)
                .ok_or(RadrootsTradeProjectionError::InvalidItemIndex(*item_index))?;
            item.bin_count = *bin_count;
        }
        TradeOrderChange::ItemAdd { item } => items.push(item.clone()),
        TradeOrderChange::ItemRemove { item_index } => {
            let index = usize::try_from(*item_index)
                .map_err(|_| RadrootsTradeProjectionError::InvalidItemIndex(*item_index))?;
            if index >= items.len() {
                return Err(RadrootsTradeProjectionError::InvalidItemIndex(*item_index));
            }
            items.remove(index);
        }
    }
    Ok(())
}

enum TradeDiscountDecisionValue {
    Accepted(RadrootsCoreDiscountValue),
    Declined(Option<String>),
}

fn trade_discount_decision_value(
    decision: &crate::listing::order::TradeDiscountDecision,
) -> Result<TradeDiscountDecisionValue, RadrootsTradeProjectionError> {
    match decision {
        crate::listing::order::TradeDiscountDecision::Accept { value } => {
            Ok(TradeDiscountDecisionValue::Accepted(value.clone()))
        }
        crate::listing::order::TradeDiscountDecision::Decline { reason } => {
            Ok(TradeDiscountDecisionValue::Declined(reason.clone()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        RadrootsTradeOrderWorkflowMessage, RadrootsTradeProjectionError, RadrootsTradeReadIndex,
        radroots_trade_order_status_can_transition, radroots_trade_order_status_is_terminal,
    };
    use crate::listing::{
        dvm::{TradeListingCancel, TradeListingMessagePayload, TradeOrderResponse},
        order::{
            TradeAnswer, TradeDiscountDecision, TradeDiscountOffer, TradeDiscountRequest,
            TradeFulfillmentStatus, TradeFulfillmentUpdate, TradeOrder, TradeOrderChange,
            TradeOrderItem, TradeOrderRevision, TradeOrderStatus, TradeQuestion, TradeReceipt,
        },
    };
    use radroots_core::{
        RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCorePercent,
        RadrootsCoreQuantity, RadrootsCoreQuantityPrice, RadrootsCoreUnit,
    };
    use radroots_events::listing::{
        RadrootsListing, RadrootsListingAvailability, RadrootsListingBin,
        RadrootsListingDeliveryMethod, RadrootsListingFarmRef, RadrootsListingLocation,
        RadrootsListingProduct, RadrootsListingStatus,
    };

    fn base_listing() -> RadrootsListing {
        RadrootsListing {
            d_tag: "AAAAAAAAAAAAAAAAAAAAAg".into(),
            farm: RadrootsListingFarmRef {
                pubkey: "farm-pubkey".into(),
                d_tag: "AAAAAAAAAAAAAAAAAAAAAA".into(),
            },
            product: RadrootsListingProduct {
                key: "coffee".into(),
                title: "Coffee".into(),
                category: "coffee".into(),
                summary: Some("single origin".into()),
                process: None,
                lot: None,
                location: None,
                profile: None,
                year: None,
            },
            primary_bin_id: "bin-1".into(),
            bins: vec![
                RadrootsListingBin {
                    bin_id: "bin-1".into(),
                    quantity: RadrootsCoreQuantity::new(
                        RadrootsCoreDecimal::from(1000u32),
                        RadrootsCoreUnit::MassG,
                    ),
                    price_per_canonical_unit: RadrootsCoreQuantityPrice::new(
                        RadrootsCoreMoney::new(
                            RadrootsCoreDecimal::from(2u32),
                            RadrootsCoreCurrency::USD,
                        ),
                        RadrootsCoreQuantity::new(
                            RadrootsCoreDecimal::from(1u32),
                            RadrootsCoreUnit::MassG,
                        ),
                    ),
                    display_amount: None,
                    display_unit: None,
                    display_label: Some("1kg bag".into()),
                    display_price: Some(RadrootsCoreMoney::new(
                        RadrootsCoreDecimal::from(2000u32),
                        RadrootsCoreCurrency::USD,
                    )),
                    display_price_unit: Some(RadrootsCoreUnit::Each),
                },
                RadrootsListingBin {
                    bin_id: "bin-2".into(),
                    quantity: RadrootsCoreQuantity::new(
                        RadrootsCoreDecimal::from(500u32),
                        RadrootsCoreUnit::MassG,
                    ),
                    price_per_canonical_unit: RadrootsCoreQuantityPrice::new(
                        RadrootsCoreMoney::new(
                            RadrootsCoreDecimal::from(3u32),
                            RadrootsCoreCurrency::USD,
                        ),
                        RadrootsCoreQuantity::new(
                            RadrootsCoreDecimal::from(1u32),
                            RadrootsCoreUnit::MassG,
                        ),
                    ),
                    display_amount: None,
                    display_unit: None,
                    display_label: Some("500g bag".into()),
                    display_price: None,
                    display_price_unit: None,
                },
            ],
            resource_area: None,
            plot: None,
            discounts: None,
            inventory_available: Some(RadrootsCoreDecimal::from(10u32)),
            availability: Some(RadrootsListingAvailability::Status {
                status: RadrootsListingStatus::Active,
            }),
            delivery_method: Some(RadrootsListingDeliveryMethod::Pickup),
            location: Some(RadrootsListingLocation {
                primary: "farm".into(),
                city: Some("Nashville".into()),
                region: Some("TN".into()),
                country: Some("US".into()),
                lat: None,
                lng: None,
                geohash: None,
            }),
            images: None,
        }
    }

    fn base_order() -> TradeOrder {
        TradeOrder {
            order_id: "order-1".into(),
            listing_addr: "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg".into(),
            buyer_pubkey: "buyer-pubkey".into(),
            seller_pubkey: "seller-pubkey".into(),
            items: vec![TradeOrderItem {
                bin_id: "bin-1".into(),
                bin_count: 2,
            }],
            discounts: Some(vec![radroots_core::RadrootsCoreDiscountValue::Percent(
                RadrootsCorePercent::new(RadrootsCoreDecimal::from(10u32)),
            )]),
            notes: Some("deliver friday".into()),
            status: TradeOrderStatus::Draft,
        }
    }

    fn message(
        actor_pubkey: &str,
        listing_addr: &str,
        order_id: Option<&str>,
        payload: TradeListingMessagePayload,
    ) -> RadrootsTradeOrderWorkflowMessage {
        RadrootsTradeOrderWorkflowMessage {
            actor_pubkey: actor_pubkey.into(),
            listing_addr: listing_addr.into(),
            order_id: order_id.map(str::to_string),
            payload,
        }
    }

    #[test]
    fn listing_projection_builds_query_friendly_view() {
        let mut index = RadrootsTradeReadIndex::new();
        let listing = base_listing();
        let projection = index
            .upsert_listing("seller-pubkey", &listing)
            .expect("listing projection");

        assert_eq!(
            projection.listing_addr,
            "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg"
        );
        assert_eq!(projection.primary_bin_id, "bin-1");
        assert_eq!(projection.bins.len(), 2);
        assert_eq!(
            projection.bins[0].one_bin_total.price_amount.amount,
            RadrootsCoreDecimal::from(2000u32)
        );
        assert_eq!(projection.order_count, 0);
        assert_eq!(projection.open_order_count, 0);
        assert_eq!(projection.terminal_order_count, 0);
    }

    #[test]
    fn workflow_projection_updates_order_and_listing_views() {
        let mut index = RadrootsTradeReadIndex::new();
        let listing = base_listing();
        index
            .upsert_listing("seller-pubkey", &listing)
            .expect("listing projection");

        index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                Some("order-1"),
                TradeListingMessagePayload::OrderRequest(base_order()),
            ))
            .expect("order request");
        let listing_after_request = index
            .listing("30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg")
            .expect("listing after request");
        assert_eq!(listing_after_request.order_count, 1);
        assert_eq!(listing_after_request.open_order_count, 1);
        assert_eq!(listing_after_request.terminal_order_count, 0);

        index
            .apply_workflow_message(&message(
                "seller-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                Some("order-1"),
                TradeListingMessagePayload::OrderResponse(TradeOrderResponse {
                    accepted: true,
                    reason: None,
                }),
            ))
            .expect("order response");
        index
            .apply_workflow_message(&message(
                "seller-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                Some("order-1"),
                TradeListingMessagePayload::FulfillmentUpdate(TradeFulfillmentUpdate {
                    status: TradeFulfillmentStatus::Delivered,
                    tracking: Some("track-1".into()),
                    eta: None,
                    notes: Some("left at dock".into()),
                }),
            ))
            .expect("fulfillment");
        let order_after_fulfillment = index.order("order-1").expect("order after fulfillment");
        assert_eq!(order_after_fulfillment.status, TradeOrderStatus::Fulfilled);
        assert_eq!(
            order_after_fulfillment.last_fulfillment_status,
            Some(TradeFulfillmentStatus::Delivered)
        );

        index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                Some("order-1"),
                TradeListingMessagePayload::Receipt(TradeReceipt {
                    acknowledged: true,
                    at: 1_700_000_000,
                    note: Some("received".into()),
                }),
            ))
            .expect("receipt");
        let order = index.order("order-1").expect("order");
        assert_eq!(order.status, TradeOrderStatus::Completed);
        assert_eq!(order.receipt_count, 1);
        let listing_after_receipt = index
            .listing("30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg")
            .expect("listing after receipt");
        assert_eq!(listing_after_receipt.open_order_count, 0);
        assert_eq!(listing_after_receipt.terminal_order_count, 1);
    }

    #[test]
    fn workflow_projection_applies_revision_question_and_answer() {
        let mut index = RadrootsTradeReadIndex::new();
        index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                Some("order-1"),
                TradeListingMessagePayload::OrderRequest(base_order()),
            ))
            .expect("order request");

        index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                Some("order-1"),
                TradeListingMessagePayload::Question(TradeQuestion {
                    question_id: "q-1".into(),
                    order_id: Some("order-1".into()),
                    listing_addr: None,
                    question_text: "can you pack separately?".into(),
                }),
            ))
            .expect("question");
        index
            .apply_workflow_message(&message(
                "seller-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                Some("order-1"),
                TradeListingMessagePayload::Answer(TradeAnswer {
                    question_id: "q-1".into(),
                    order_id: Some("order-1".into()),
                    listing_addr: None,
                    answer_text: "yes".into(),
                }),
            ))
            .expect("answer");
        let order_after_answer = index.order("order-1").expect("order after answer");
        assert_eq!(order_after_answer.status, TradeOrderStatus::Requested);
        assert_eq!(order_after_answer.question_count, 1);
        assert_eq!(order_after_answer.answer_count, 1);

        index
            .apply_workflow_message(&message(
                "seller-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                Some("order-1"),
                TradeListingMessagePayload::OrderRevision(TradeOrderRevision {
                    revision_id: "rev-1".into(),
                    order_id: "order-1".into(),
                    changes: vec![
                        TradeOrderChange::BinCount {
                            item_index: 0,
                            bin_count: 3,
                        },
                        TradeOrderChange::ItemAdd {
                            item: TradeOrderItem {
                                bin_id: "bin-2".into(),
                                bin_count: 1,
                            },
                        },
                    ],
                    reason: Some("limited stock".into()),
                }),
            ))
            .expect("order revision");
        let order = index.order("order-1").expect("order");
        assert_eq!(order.status, TradeOrderStatus::Revised);
        assert_eq!(order.revision_count, 1);
        assert_eq!(order.items[0].bin_count, 3);
        assert_eq!(order.items.len(), 2);
    }

    #[test]
    fn workflow_projection_covers_discount_and_cancel_paths() {
        let mut index = RadrootsTradeReadIndex::new();
        index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                Some("order-1"),
                TradeListingMessagePayload::OrderRequest(base_order()),
            ))
            .expect("order request");

        index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                Some("order-1"),
                TradeListingMessagePayload::DiscountRequest(TradeDiscountRequest {
                    discount_id: "disc-1".into(),
                    order_id: "order-1".into(),
                    value: radroots_core::RadrootsCoreDiscountValue::Percent(
                        RadrootsCorePercent::new(RadrootsCoreDecimal::from(15u32)),
                    ),
                    conditions: Some("volume".into()),
                }),
            ))
            .expect("discount request");
        index
            .apply_workflow_message(&message(
                "seller-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                Some("order-1"),
                TradeListingMessagePayload::DiscountOffer(TradeDiscountOffer {
                    discount_id: "disc-1".into(),
                    order_id: "order-1".into(),
                    value: radroots_core::RadrootsCoreDiscountValue::Percent(
                        RadrootsCorePercent::new(RadrootsCoreDecimal::from(12u32)),
                    ),
                    conditions: Some("counter".into()),
                }),
            ))
            .expect("discount offer");
        index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                Some("order-1"),
                TradeListingMessagePayload::DiscountDecline(TradeDiscountDecision::Decline {
                    reason: Some("need full price".into()),
                }),
            ))
            .expect("discount decline");
        index
            .apply_workflow_message(&message(
                "seller-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                Some("order-1"),
                TradeListingMessagePayload::Cancel(TradeListingCancel {
                    reason: Some("inventory issue".into()),
                }),
            ))
            .expect("cancel");
        let order = index.order("order-1").expect("order");
        assert_eq!(order.status, TradeOrderStatus::Cancelled);
        assert_eq!(order.discount_request_count, 1);
        assert_eq!(order.discount_offer_count, 1);
        assert_eq!(order.discount_decline_count, 1);
        assert_eq!(
            order.last_discount_decline_reason.as_deref(),
            Some("need full price")
        );
        assert_eq!(order.cancellation_count, 1);
    }

    #[test]
    fn workflow_projection_backfills_listing_counts_when_listing_arrives_late() {
        let mut index = RadrootsTradeReadIndex::new();
        index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                Some("order-1"),
                TradeListingMessagePayload::OrderRequest(base_order()),
            ))
            .expect("order request");

        index
            .upsert_listing("seller-pubkey", &base_listing())
            .expect("listing projection");
        let listing = index
            .listing("30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg")
            .expect("listing");
        assert_eq!(listing.order_count, 1);
        assert_eq!(listing.open_order_count, 1);
    }

    #[test]
    fn workflow_projection_rejects_invalid_inputs() {
        let mut index = RadrootsTradeReadIndex::new();
        let err = index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                None,
                TradeListingMessagePayload::Question(TradeQuestion {
                    question_id: "q-1".into(),
                    order_id: Some("order-1".into()),
                    listing_addr: None,
                    question_text: "hello".into(),
                }),
            ))
            .expect_err("missing order id should fail");
        assert_eq!(err, RadrootsTradeProjectionError::MissingOrderId);

        let err = index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                None,
                TradeListingMessagePayload::ListingValidateRequest(
                    crate::listing::dvm::TradeListingValidateRequest {
                        listing_event: None,
                    },
                ),
            ))
            .expect_err("non-order message should fail");
        assert_eq!(
            err,
            RadrootsTradeProjectionError::NonOrderWorkflowMessage(
                crate::listing::dvm::TradeListingMessageType::ListingValidateRequest
            )
        );

        let listing = RadrootsListing {
            primary_bin_id: "missing".into(),
            ..base_listing()
        };
        let err = index
            .upsert_listing("seller-pubkey", &listing)
            .expect_err("missing primary bin should fail");
        assert_eq!(
            err,
            RadrootsTradeProjectionError::MissingPrimaryBin("missing".into())
        );
    }

    #[test]
    fn workflow_helpers_cover_transition_and_terminal_tables() {
        assert!(radroots_trade_order_status_can_transition(
            &TradeOrderStatus::Requested,
            &TradeOrderStatus::Accepted
        ));
        assert!(!radroots_trade_order_status_can_transition(
            &TradeOrderStatus::Accepted,
            &TradeOrderStatus::Requested
        ));
        assert!(radroots_trade_order_status_is_terminal(
            &TradeOrderStatus::Completed
        ));
        assert!(!radroots_trade_order_status_is_terminal(
            &TradeOrderStatus::Fulfilled
        ));
    }
}
