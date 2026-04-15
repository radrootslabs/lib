#![forbid(unsafe_code)]

use core::cmp::Ordering;

#[cfg(not(feature = "std"))]
use alloc::{collections::BTreeMap, format, string::String, vec::Vec};
#[cfg(feature = "std")]
use std::collections::BTreeMap;

use radroots_core::{RadrootsCoreDecimal, RadrootsCoreDiscount, RadrootsCoreDiscountValue};
use radroots_events::{
    RadrootsNostrEvent, RadrootsNostrEventPtr,
    farm::RadrootsFarmRef,
    kinds::{KIND_LISTING, is_listing_kind},
    listing::{
        RadrootsListing, RadrootsListingAvailability, RadrootsListingBin,
        RadrootsListingDeliveryMethod, RadrootsListingImage, RadrootsListingLocation,
        RadrootsListingProduct,
    },
    plot::RadrootsPlotRef,
    resource_area::RadrootsResourceAreaRef,
};
#[cfg(feature = "ts-rs")]
use ts_rs::TS;

use crate::listing::{
    codec::{TradeListingParseError, listing_from_event_parts},
    dvm::{
        TradeListingEnvelopeParseError, TradeListingMessagePayload, TradeListingMessageType,
        trade_listing_envelope_from_event,
    },
    model::RadrootsTradeListingTotal,
    order::{
        TradeFulfillmentStatus, TradeOrder, TradeOrderChange, TradeOrderItem, TradeOrderStatus,
    },
    price_ext::BinPricingExt,
};
#[cfg(feature = "serde_json")]
use radroots_events_codec::trade::trade_event_context_from_tags;

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
    #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsFarmRef"))]
    pub farm: RadrootsFarmRef,
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
    #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsTradeOrderItem[]"))]
    pub items: Vec<TradeOrderItem>,
    #[cfg_attr(
        feature = "ts-rs",
        ts(optional, type = "RadrootsCoreDiscountValue[] | null")
    )]
    pub requested_discounts: Option<Vec<RadrootsCoreDiscountValue>>,
    pub status: TradeOrderStatus,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "RadrootsNostrEventPtr | null"))]
    pub listing_snapshot: Option<RadrootsNostrEventPtr>,
    pub root_event_id: String,
    pub last_event_id: String,
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
        ts(optional, type = "RadrootsTradeFulfillmentStatus | null")
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
    pub event_id: String,
    pub actor_pubkey: String,
    pub counterparty_pubkey: String,
    pub listing_addr: String,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub order_id: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "RadrootsNostrEventPtr | null"))]
    pub listing_event: Option<RadrootsNostrEventPtr>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub root_event_id: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub prev_event_id: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsTradeMessagePayload"))]
    pub payload: TradeListingMessagePayload,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsTradeSortDirection {
    Asc,
    Desc,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsTradeListingMarketStatus {
    Unknown,
    Window,
    Active,
    Sold,
    Other { value: String },
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RadrootsTradeListingQuery {
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub seller_pubkey: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub farm_pubkey: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub farm_id: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub product_key: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub product_category: Option<String>,
    #[cfg_attr(
        feature = "ts-rs",
        ts(optional, type = "RadrootsTradeListingMarketStatus | null")
    )]
    pub listing_status: Option<RadrootsTradeListingMarketStatus>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsTradeListingSortField {
    ListingAddr,
    ProductTitle,
    ProductCategory,
    SellerPubkey,
    InventoryAvailable,
    OpenOrderCount,
    TotalOrderCount,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RadrootsTradeListingSort {
    pub field: RadrootsTradeListingSortField,
    pub direction: RadrootsTradeSortDirection,
}

impl Default for RadrootsTradeListingSort {
    fn default() -> Self {
        Self {
            field: RadrootsTradeListingSortField::ListingAddr,
            direction: RadrootsTradeSortDirection::Asc,
        }
    }
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RadrootsTradeOrderQuery {
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub listing_addr: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub buyer_pubkey: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub seller_pubkey: Option<String>,
    #[cfg_attr(
        feature = "ts-rs",
        ts(optional, type = "RadrootsTradeOrderStatus | null")
    )]
    pub status: Option<TradeOrderStatus>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsTradeOrderSortField {
    OrderId,
    ListingAddr,
    BuyerPubkey,
    SellerPubkey,
    Status,
    LastMessageType,
    TotalBinCount,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RadrootsTradeOrderSort {
    pub field: RadrootsTradeOrderSortField,
    pub direction: RadrootsTradeSortDirection,
}

impl Default for RadrootsTradeOrderSort {
    fn default() -> Self {
        Self {
            field: RadrootsTradeOrderSortField::OrderId,
            direction: RadrootsTradeSortDirection::Asc,
        }
    }
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeFacetCount {
    pub key: String,
    pub count: u32,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeListingFacets {
    #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsTradeFacetCount[]"))]
    pub seller_pubkeys: Vec<RadrootsTradeFacetCount>,
    #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsTradeFacetCount[]"))]
    pub farm_pubkeys: Vec<RadrootsTradeFacetCount>,
    #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsTradeFacetCount[]"))]
    pub farm_ids: Vec<RadrootsTradeFacetCount>,
    #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsTradeFacetCount[]"))]
    pub product_keys: Vec<RadrootsTradeFacetCount>,
    #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsTradeFacetCount[]"))]
    pub product_categories: Vec<RadrootsTradeFacetCount>,
    #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsTradeFacetCount[]"))]
    pub listing_statuses: Vec<RadrootsTradeFacetCount>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeOrderFacets {
    #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsTradeFacetCount[]"))]
    pub buyer_pubkeys: Vec<RadrootsTradeFacetCount>,
    #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsTradeFacetCount[]"))]
    pub seller_pubkeys: Vec<RadrootsTradeFacetCount>,
    #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsTradeFacetCount[]"))]
    pub listing_addrs: Vec<RadrootsTradeFacetCount>,
    #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsTradeFacetCount[]"))]
    pub statuses: Vec<RadrootsTradeFacetCount>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeMarketplaceListingSummary {
    pub listing_addr: String,
    pub seller_pubkey: String,
    pub farm_pubkey: String,
    pub farm_id: String,
    pub product_key: String,
    pub product_title: String,
    pub product_category: String,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub product_summary: Option<String>,
    pub listing_status: RadrootsTradeListingMarketStatus,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub location_primary: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "RadrootsCoreDecimal | null"))]
    pub inventory_available: Option<RadrootsCoreDecimal>,
    pub primary_bin_id: String,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub primary_bin_label: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsTradeListingTotal"))]
    pub primary_bin_total: RadrootsTradeListingTotal,
    pub order_count: u32,
    pub open_order_count: u32,
    pub terminal_order_count: u32,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeMarketplaceOrderSummary {
    pub order_id: String,
    pub listing_addr: String,
    pub buyer_pubkey: String,
    pub seller_pubkey: String,
    pub status: TradeOrderStatus,
    pub last_message_type: TradeListingMessageType,
    pub item_count: u32,
    pub total_bin_count: u32,
    pub has_requested_discounts: bool,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub last_reason: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct RadrootsTradeReadIndex {
    listings: BTreeMap<String, RadrootsTradeListingProjection>,
    orders: BTreeMap<String, RadrootsTradeOrderWorkflowProjection>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsTradeProjectionError {
    InvalidListingKind {
        kind: u32,
    },
    InvalidListingContract {
        error: TradeListingParseError,
    },
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
    CounterpartyMismatch,
    MissingListingSnapshot,
    MissingTradeRootEventId,
    MissingTradePrevEventId,
    TradeThreadRootMismatch,
    TradeThreadPrevMismatch,
    #[cfg(feature = "serde_json")]
    InvalidWorkflowEvent {
        error: TradeListingEnvelopeParseError,
    },
}

impl core::fmt::Display for RadrootsTradeProjectionError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            RadrootsTradeProjectionError::InvalidListingKind { kind } => {
                write!(f, "invalid listing event kind: {kind}")
            }
            RadrootsTradeProjectionError::InvalidListingContract { error } => {
                write!(f, "invalid listing contract event: {error}")
            }
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
            RadrootsTradeProjectionError::CounterpartyMismatch => {
                write!(f, "counterparty pubkey mismatch")
            }
            RadrootsTradeProjectionError::MissingListingSnapshot => {
                write!(f, "missing listing snapshot")
            }
            RadrootsTradeProjectionError::MissingTradeRootEventId => {
                write!(f, "missing trade root event id")
            }
            RadrootsTradeProjectionError::MissingTradePrevEventId => {
                write!(f, "missing trade previous event id")
            }
            RadrootsTradeProjectionError::TradeThreadRootMismatch => {
                write!(f, "trade thread root mismatch")
            }
            RadrootsTradeProjectionError::TradeThreadPrevMismatch => {
                write!(f, "trade thread previous event mismatch")
            }
            #[cfg(feature = "serde_json")]
            RadrootsTradeProjectionError::InvalidWorkflowEvent { error } => write!(f, "{error}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsTradeProjectionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            RadrootsTradeProjectionError::InvalidListingContract { error } => Some(error),
            #[cfg(feature = "serde_json")]
            RadrootsTradeProjectionError::InvalidWorkflowEvent { error } => Some(error),
            _ => None,
        }
    }
}

impl RadrootsTradeListingProjection {
    pub fn market_status(&self) -> RadrootsTradeListingMarketStatus {
        match &self.availability {
            Some(RadrootsListingAvailability::Status { status }) => match status {
                radroots_events::listing::RadrootsListingStatus::Active => {
                    RadrootsTradeListingMarketStatus::Active
                }
                radroots_events::listing::RadrootsListingStatus::Sold => {
                    RadrootsTradeListingMarketStatus::Sold
                }
                radroots_events::listing::RadrootsListingStatus::Other { value } => {
                    RadrootsTradeListingMarketStatus::Other {
                        value: value.clone(),
                    }
                }
            },
            Some(RadrootsListingAvailability::Window { .. }) => {
                RadrootsTradeListingMarketStatus::Window
            }
            None => RadrootsTradeListingMarketStatus::Unknown,
        }
    }

    pub fn primary_bin(&self) -> Option<&RadrootsTradeListingBinProjection> {
        self.bins
            .iter()
            .find(|bin| bin.bin.bin_id == self.primary_bin_id)
    }

    pub fn marketplace_summary(&self) -> Option<RadrootsTradeMarketplaceListingSummary> {
        let primary_bin = self.primary_bin()?;
        Some(RadrootsTradeMarketplaceListingSummary {
            listing_addr: self.listing_addr.clone(),
            seller_pubkey: self.seller_pubkey.clone(),
            farm_pubkey: self.farm.pubkey.clone(),
            farm_id: self.farm.d_tag.clone(),
            product_key: self.product.key.clone(),
            product_title: self.product.title.clone(),
            product_category: self.product.category.clone(),
            product_summary: self.product.summary.clone(),
            listing_status: self.market_status(),
            location_primary: self
                .location
                .as_ref()
                .map(|location| location.primary.clone()),
            inventory_available: self.inventory_available.clone(),
            primary_bin_id: self.primary_bin_id.clone(),
            primary_bin_label: primary_bin.bin.display_label.clone(),
            primary_bin_total: primary_bin.one_bin_total.clone(),
            order_count: self.order_count,
            open_order_count: self.open_order_count,
            terminal_order_count: self.terminal_order_count,
        })
    }

    pub fn from_listing_event(
        event: &RadrootsNostrEvent,
    ) -> Result<Self, RadrootsTradeProjectionError> {
        if !is_listing_kind(event.kind) {
            return Err(RadrootsTradeProjectionError::InvalidListingKind { kind: event.kind });
        }
        let listing = listing_from_event_parts(&event.tags, &event.content)
            .map_err(|error| RadrootsTradeProjectionError::InvalidListingContract { error })?;
        let mut projection = Self::from_listing_contract(event.author.clone(), &listing)?;
        projection.listing_addr = format!("{}:{}:{}", event.kind, event.author, listing.d_tag);
        Ok(projection)
    }

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

    pub fn item_count(&self) -> u32 {
        u32::try_from(self.items.len()).unwrap_or(u32::MAX)
    }

    pub fn total_bin_count(&self) -> u32 {
        self.items
            .iter()
            .fold(0u32, |total, item| total.saturating_add(item.bin_count))
    }

    pub fn marketplace_summary(&self) -> RadrootsTradeMarketplaceOrderSummary {
        RadrootsTradeMarketplaceOrderSummary {
            order_id: self.order_id.clone(),
            listing_addr: self.listing_addr.clone(),
            buyer_pubkey: self.buyer_pubkey.clone(),
            seller_pubkey: self.seller_pubkey.clone(),
            status: self.status.clone(),
            last_message_type: self.last_message_type,
            item_count: self.item_count(),
            total_bin_count: self.total_bin_count(),
            has_requested_discounts: self
                .requested_discounts
                .as_ref()
                .is_some_and(|discounts| !discounts.is_empty()),
            last_reason: self.last_reason.clone(),
        }
    }

    fn from_order_request(
        message: &RadrootsTradeOrderWorkflowMessage,
        order: &TradeOrder,
    ) -> Result<Self, RadrootsTradeProjectionError> {
        let listing_snapshot = require_listing_snapshot(message)?;
        Ok(Self {
            order_id: order.order_id.clone(),
            listing_addr: order.listing_addr.clone(),
            buyer_pubkey: order.buyer_pubkey.clone(),
            seller_pubkey: order.seller_pubkey.clone(),
            items: order.items.clone(),
            requested_discounts: order.discounts.clone(),
            status: TradeOrderStatus::Requested,
            listing_snapshot: Some(listing_snapshot),
            root_event_id: message.event_id.clone(),
            last_event_id: message.event_id.clone(),
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
        })
    }
}

impl RadrootsTradeOrderWorkflowMessage {
    #[cfg(feature = "serde_json")]
    pub fn from_event(event: &RadrootsNostrEvent) -> Result<Self, TradeListingEnvelopeParseError> {
        let envelope = trade_listing_envelope_from_event::<TradeListingMessagePayload>(event)?;
        trade_event_context_from_tags(envelope.message_type, &event.tags).map(|context| Self {
            event_id: event.id.clone(),
            actor_pubkey: event.author.clone(),
            counterparty_pubkey: context.counterparty_pubkey,
            listing_addr: envelope.listing_addr,
            order_id: envelope.order_id,
            listing_event: context.listing_event,
            root_event_id: context.root_event_id,
            prev_event_id: context.prev_event_id,
            payload: envelope.payload,
        })
    }

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

    pub fn query_listings<'a>(
        &'a self,
        query: &RadrootsTradeListingQuery,
        sort: RadrootsTradeListingSort,
    ) -> Vec<&'a RadrootsTradeListingProjection> {
        let mut listings = self
            .listings
            .values()
            .filter(|listing| listing_matches_query(listing, query))
            .collect::<Vec<_>>();
        listings.sort_by(|left, right| compare_listings(left, right, sort));
        listings
    }

    pub fn query_orders<'a>(
        &'a self,
        query: &RadrootsTradeOrderQuery,
        sort: RadrootsTradeOrderSort,
    ) -> Vec<&'a RadrootsTradeOrderWorkflowProjection> {
        let mut orders = self
            .orders
            .values()
            .filter(|order| order_matches_query(order, query))
            .collect::<Vec<_>>();
        orders.sort_by(|left, right| compare_orders(left, right, sort));
        orders
    }

    pub fn listing_facets(&self, query: &RadrootsTradeListingQuery) -> RadrootsTradeListingFacets {
        let mut seller_pubkeys = BTreeMap::<String, u32>::new();
        let mut farm_pubkeys = BTreeMap::<String, u32>::new();
        let mut farm_ids = BTreeMap::<String, u32>::new();
        let mut product_keys = BTreeMap::<String, u32>::new();
        let mut product_categories = BTreeMap::<String, u32>::new();
        let mut listing_statuses = BTreeMap::<String, u32>::new();

        for listing in self
            .listings
            .values()
            .filter(|listing| listing_matches_query(listing, query))
        {
            increment_count(&mut seller_pubkeys, listing.seller_pubkey.clone());
            increment_count(&mut farm_pubkeys, listing.farm.pubkey.clone());
            increment_count(&mut farm_ids, listing.farm.d_tag.clone());
            increment_count(&mut product_keys, listing.product.key.clone());
            increment_count(&mut product_categories, listing.product.category.clone());
            increment_count(&mut listing_statuses, listing.market_status().facet_key());
        }

        RadrootsTradeListingFacets {
            seller_pubkeys: facet_counts_from_map(seller_pubkeys),
            farm_pubkeys: facet_counts_from_map(farm_pubkeys),
            farm_ids: facet_counts_from_map(farm_ids),
            product_keys: facet_counts_from_map(product_keys),
            product_categories: facet_counts_from_map(product_categories),
            listing_statuses: facet_counts_from_map(listing_statuses),
        }
    }

    pub fn order_facets(&self, query: &RadrootsTradeOrderQuery) -> RadrootsTradeOrderFacets {
        let mut buyer_pubkeys = BTreeMap::<String, u32>::new();
        let mut seller_pubkeys = BTreeMap::<String, u32>::new();
        let mut listing_addrs = BTreeMap::<String, u32>::new();
        let mut statuses = BTreeMap::<String, u32>::new();

        for order in self
            .orders
            .values()
            .filter(|order| order_matches_query(order, query))
        {
            increment_count(&mut buyer_pubkeys, order.buyer_pubkey.clone());
            increment_count(&mut seller_pubkeys, order.seller_pubkey.clone());
            increment_count(&mut listing_addrs, order.listing_addr.clone());
            increment_count(&mut statuses, order_status_key(&order.status));
        }

        RadrootsTradeOrderFacets {
            buyer_pubkeys: facet_counts_from_map(buyer_pubkeys),
            seller_pubkeys: facet_counts_from_map(seller_pubkeys),
            listing_addrs: facet_counts_from_map(listing_addrs),
            statuses: facet_counts_from_map(statuses),
        }
    }

    pub fn marketplace_listing_summaries(
        &self,
        query: &RadrootsTradeListingQuery,
        sort: RadrootsTradeListingSort,
    ) -> Vec<RadrootsTradeMarketplaceListingSummary> {
        self.query_listings(query, sort)
            .into_iter()
            .filter_map(|listing| listing.marketplace_summary())
            .collect()
    }

    pub fn marketplace_order_summaries(
        &self,
        query: &RadrootsTradeOrderQuery,
        sort: RadrootsTradeOrderSort,
    ) -> Vec<RadrootsTradeMarketplaceOrderSummary> {
        self.query_orders(query, sort)
            .into_iter()
            .map(RadrootsTradeOrderWorkflowProjection::marketplace_summary)
            .collect()
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

    pub fn upsert_listing_event(
        &mut self,
        event: &RadrootsNostrEvent,
    ) -> Result<&RadrootsTradeListingProjection, RadrootsTradeProjectionError> {
        let projection = RadrootsTradeListingProjection::from_listing_event(event)?;
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

    #[cfg(feature = "serde_json")]
    pub fn apply_workflow_event(
        &mut self,
        event: &RadrootsNostrEvent,
    ) -> Result<&RadrootsTradeOrderWorkflowProjection, RadrootsTradeProjectionError> {
        let message = RadrootsTradeOrderWorkflowMessage::from_event(event)
            .map_err(|error| RadrootsTradeProjectionError::InvalidWorkflowEvent { error })?;
        self.apply_workflow_message(&message)
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
                let (order_id, order) = self.order_mut_for_seller_action(message)?;
                let next_status = if response.accepted {
                    TradeOrderStatus::Accepted
                } else {
                    TradeOrderStatus::Declined
                };
                let from_status = order.status.clone();
                radroots_trade_order_status_ensure_transition(from_status, next_status.clone())?;
                order.status = next_status;
                order.last_message_type = TradeListingMessageType::OrderResponse;
                order.last_actor_pubkey = message.actor_pubkey.clone();
                order.last_reason = response.reason.clone();
                order.last_event_id = message.event_id.clone();
                Ok(order_id)
            }
            TradeListingMessagePayload::OrderRevision(revision) => {
                let (order_id, order) = self.order_mut_for_seller_action(message)?;
                let next_status = TradeOrderStatus::Revised;
                let from_status = order.status.clone();
                radroots_trade_order_status_ensure_transition(from_status, next_status.clone())?;
                for change in &revision.changes {
                    apply_order_change(&mut order.items, change)?;
                }
                order.listing_snapshot = Some(require_listing_snapshot(message)?);
                order.status = next_status;
                order.revision_count = order.revision_count.saturating_add(1);
                order.last_message_type = TradeListingMessageType::OrderRevision;
                order.last_actor_pubkey = message.actor_pubkey.clone();
                order.last_reason = None;
                order.last_event_id = message.event_id.clone();
                Ok(order_id)
            }
            TradeListingMessagePayload::OrderRevisionAccept(response) => {
                if !response.accepted {
                    return Err(RadrootsTradeProjectionError::InvalidRevisionResponse);
                }
                let (order_id, order) = self.order_mut_for_buyer_action(message)?;
                let next_status = TradeOrderStatus::Accepted;
                let from_status = order.status.clone();
                radroots_trade_order_status_ensure_transition(from_status, next_status.clone())?;
                order.status = next_status;
                order.last_message_type = TradeListingMessageType::OrderRevisionAccept;
                order.last_actor_pubkey = message.actor_pubkey.clone();
                order.last_reason = response.reason.clone();
                order.last_event_id = message.event_id.clone();
                Ok(order_id)
            }
            TradeListingMessagePayload::OrderRevisionDecline(response) => {
                if response.accepted {
                    return Err(RadrootsTradeProjectionError::InvalidRevisionResponse);
                }
                let (order_id, order) = self.order_mut_for_buyer_action(message)?;
                let next_status = TradeOrderStatus::Declined;
                let from_status = order.status.clone();
                radroots_trade_order_status_ensure_transition(from_status, next_status.clone())?;
                order.status = next_status;
                order.last_message_type = TradeListingMessageType::OrderRevisionDecline;
                order.last_actor_pubkey = message.actor_pubkey.clone();
                order.last_reason = response.reason.clone();
                order.last_event_id = message.event_id.clone();
                Ok(order_id)
            }
            TradeListingMessagePayload::Question(question) => {
                let (order_id, order) = self.order_mut_for_buyer_action(message)?;
                let next_status = TradeOrderStatus::Questioned;
                let from_status = order.status.clone();
                radroots_trade_order_status_ensure_transition(from_status, next_status.clone())?;
                order.status = next_status;
                order.question_count = order.question_count.saturating_add(1);
                order.last_message_type = TradeListingMessageType::Question;
                order.last_actor_pubkey = message.actor_pubkey.clone();
                order.last_reason = Some(question.question_id.clone());
                order.last_event_id = message.event_id.clone();
                Ok(order_id)
            }
            TradeListingMessagePayload::Answer(answer) => {
                let (order_id, order) = self.order_mut_for_seller_action(message)?;
                let next_status = TradeOrderStatus::Requested;
                let from_status = order.status.clone();
                radroots_trade_order_status_ensure_transition(from_status, next_status.clone())?;
                order.status = next_status;
                order.answer_count = order.answer_count.saturating_add(1);
                order.last_message_type = TradeListingMessageType::Answer;
                order.last_actor_pubkey = message.actor_pubkey.clone();
                order.last_reason = Some(answer.question_id.clone());
                order.last_event_id = message.event_id.clone();
                Ok(order_id)
            }
            TradeListingMessagePayload::DiscountRequest(request) => {
                let (order_id, order) = self.order_mut_for_buyer_action(message)?;
                order.discount_request_count = order.discount_request_count.saturating_add(1);
                order.last_discount_request = Some(request.value.clone());
                order.listing_snapshot = Some(require_listing_snapshot(message)?);
                order.last_message_type = TradeListingMessageType::DiscountRequest;
                order.last_actor_pubkey = message.actor_pubkey.clone();
                order.last_reason = None;
                order.last_event_id = message.event_id.clone();
                Ok(order_id)
            }
            TradeListingMessagePayload::DiscountOffer(offer) => {
                let (order_id, order) = self.order_mut_for_seller_action(message)?;
                let next_status = TradeOrderStatus::Revised;
                let from_status = order.status.clone();
                radroots_trade_order_status_ensure_transition(from_status, next_status.clone())?;
                order.status = next_status;
                order.discount_offer_count = order.discount_offer_count.saturating_add(1);
                order.last_discount_offer = Some(offer.value.clone());
                order.listing_snapshot = Some(require_listing_snapshot(message)?);
                order.last_message_type = TradeListingMessageType::DiscountOffer;
                order.last_actor_pubkey = message.actor_pubkey.clone();
                order.last_reason = None;
                order.last_event_id = message.event_id.clone();
                Ok(order_id)
            }
            TradeListingMessagePayload::DiscountAccept(decision) => {
                let (order_id, order) = self.order_mut_for_buyer_action(message)?;
                let TradeDiscountDecisionValue::Accepted(value) =
                    trade_discount_decision_value(decision)
                else {
                    return Err(RadrootsTradeProjectionError::InvalidDiscountDecision);
                };
                let next_status = TradeOrderStatus::Accepted;
                let from_status = order.status.clone();
                radroots_trade_order_status_ensure_transition(from_status, next_status.clone())?;
                order.status = next_status;
                order.discount_accept_count = order.discount_accept_count.saturating_add(1);
                order.accepted_discount = Some(value);
                order.last_discount_decline_reason = None;
                order.last_message_type = TradeListingMessageType::DiscountAccept;
                order.last_actor_pubkey = message.actor_pubkey.clone();
                order.last_reason = None;
                order.last_event_id = message.event_id.clone();
                Ok(order_id)
            }
            TradeListingMessagePayload::DiscountDecline(decision) => {
                let (order_id, order) = self.order_mut_for_buyer_action(message)?;
                let TradeDiscountDecisionValue::Declined(reason) =
                    trade_discount_decision_value(decision)
                else {
                    return Err(RadrootsTradeProjectionError::InvalidDiscountDecision);
                };
                let next_status = TradeOrderStatus::Requested;
                let from_status = order.status.clone();
                radroots_trade_order_status_ensure_transition(from_status, next_status.clone())?;
                order.status = next_status;
                order.discount_decline_count = order.discount_decline_count.saturating_add(1);
                order.last_discount_decline_reason = reason.clone();
                order.last_message_type = TradeListingMessageType::DiscountDecline;
                order.last_actor_pubkey = message.actor_pubkey.clone();
                order.last_reason = reason;
                order.last_event_id = message.event_id.clone();
                Ok(order_id)
            }
            TradeListingMessagePayload::Cancel(cancel) => {
                let (order_id, order) = self.order_mut_for_participant_action(message)?;
                let next_status = TradeOrderStatus::Cancelled;
                let from_status = order.status.clone();
                radroots_trade_order_status_ensure_transition(from_status, next_status.clone())?;
                order.status = next_status;
                order.cancellation_count = order.cancellation_count.saturating_add(1);
                order.last_message_type = TradeListingMessageType::Cancel;
                order.last_actor_pubkey = message.actor_pubkey.clone();
                order.last_reason = cancel.reason.clone();
                order.last_event_id = message.event_id.clone();
                Ok(order_id)
            }
            TradeListingMessagePayload::FulfillmentUpdate(update) => {
                let (order_id, order) = self.order_mut_for_seller_action(message)?;
                if let Some(next_status) =
                    trade_order_status_for_fulfillment_update(&order.status, &update.status)?
                {
                    order.status = next_status;
                }
                order.fulfillment_update_count = order.fulfillment_update_count.saturating_add(1);
                order.last_fulfillment_status = Some(update.status.clone());
                order.last_message_type = TradeListingMessageType::FulfillmentUpdate;
                order.last_actor_pubkey = message.actor_pubkey.clone();
                order.last_reason = None;
                order.last_event_id = message.event_id.clone();
                Ok(order_id)
            }
            TradeListingMessagePayload::Receipt(receipt) => {
                let (order_id, order) = self.order_mut_for_buyer_action(message)?;
                if let Some(next_status) =
                    trade_order_status_for_receipt(&order.status, receipt.acknowledged)?
                {
                    order.status = next_status;
                }
                order.receipt_count = order.receipt_count.saturating_add(1);
                order.receipt_acknowledged = Some(receipt.acknowledged);
                order.receipt_at = Some(receipt.at);
                order.last_message_type = TradeListingMessageType::Receipt;
                order.last_actor_pubkey = message.actor_pubkey.clone();
                order.last_reason = None;
                order.last_event_id = message.event_id.clone();
                Ok(order_id)
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
        ensure_counterparty(&order.seller_pubkey, &message.counterparty_pubkey)?;
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
            RadrootsTradeOrderWorkflowProjection::from_order_request(message, order)?,
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

    fn order_mut_for_buyer_action(
        &mut self,
        message: &RadrootsTradeOrderWorkflowMessage,
    ) -> Result<(String, &mut RadrootsTradeOrderWorkflowProjection), RadrootsTradeProjectionError>
    {
        let order_id = required_order_id(message)?.to_string();
        let order = self.order_mut_checked(&order_id, &message.listing_addr)?;
        ensure_actor(&order.buyer_pubkey, &message.actor_pubkey)?;
        ensure_counterparty(&order.seller_pubkey, &message.counterparty_pubkey)?;
        ensure_trade_chain(order, message)?;
        Ok((order_id, order))
    }

    fn order_mut_for_seller_action(
        &mut self,
        message: &RadrootsTradeOrderWorkflowMessage,
    ) -> Result<(String, &mut RadrootsTradeOrderWorkflowProjection), RadrootsTradeProjectionError>
    {
        let order_id = required_order_id(message)?.to_string();
        let order = self.order_mut_checked(&order_id, &message.listing_addr)?;
        ensure_actor(&order.seller_pubkey, &message.actor_pubkey)?;
        ensure_counterparty(&order.buyer_pubkey, &message.counterparty_pubkey)?;
        ensure_trade_chain(order, message)?;
        Ok((order_id, order))
    }

    fn order_mut_for_participant_action(
        &mut self,
        message: &RadrootsTradeOrderWorkflowMessage,
    ) -> Result<(String, &mut RadrootsTradeOrderWorkflowProjection), RadrootsTradeProjectionError>
    {
        let order_id = required_order_id(message)?.to_string();
        let order = self.order_mut_checked(&order_id, &message.listing_addr)?;
        if order.buyer_pubkey != message.actor_pubkey && order.seller_pubkey != message.actor_pubkey
        {
            return Err(RadrootsTradeProjectionError::UnauthorizedActor);
        }
        let expected_counterparty = if order.buyer_pubkey == message.actor_pubkey {
            &order.seller_pubkey
        } else {
            &order.buyer_pubkey
        };
        ensure_counterparty(expected_counterparty, &message.counterparty_pubkey)?;
        ensure_trade_chain(order, message)?;
        Ok((order_id, order))
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
        TradeOrderStatus::Requested => match to {
            TradeOrderStatus::Accepted
            | TradeOrderStatus::Declined
            | TradeOrderStatus::Questioned
            | TradeOrderStatus::Revised
            | TradeOrderStatus::Cancelled
            | TradeOrderStatus::Requested => true,
            _ => false,
        },
        TradeOrderStatus::Questioned => match to {
            TradeOrderStatus::Requested
            | TradeOrderStatus::Revised
            | TradeOrderStatus::Cancelled => true,
            _ => false,
        },
        TradeOrderStatus::Revised => match to {
            TradeOrderStatus::Accepted
            | TradeOrderStatus::Declined
            | TradeOrderStatus::Cancelled
            | TradeOrderStatus::Requested => true,
            _ => false,
        },
        TradeOrderStatus::Accepted => {
            matches!(
                to,
                TradeOrderStatus::Fulfilled | TradeOrderStatus::Cancelled
            )
        }
        TradeOrderStatus::Declined => false,
        TradeOrderStatus::Cancelled => false,
        TradeOrderStatus::Fulfilled => match to {
            TradeOrderStatus::Completed
            | TradeOrderStatus::Fulfilled
            | TradeOrderStatus::Cancelled => true,
            _ => false,
        },
        TradeOrderStatus::Completed => false,
    }
}

pub fn radroots_trade_order_status_is_terminal(status: &TradeOrderStatus) -> bool {
    matches!(
        status,
        TradeOrderStatus::Declined | TradeOrderStatus::Cancelled | TradeOrderStatus::Completed
    )
}

fn trade_order_status_for_fulfillment_update(
    current: &TradeOrderStatus,
    fulfillment_status: &TradeFulfillmentStatus,
) -> Result<Option<TradeOrderStatus>, RadrootsTradeProjectionError> {
    match fulfillment_status {
        TradeFulfillmentStatus::Preparing
        | TradeFulfillmentStatus::Shipped
        | TradeFulfillmentStatus::ReadyForPickup => {
            if matches!(current, TradeOrderStatus::Accepted) {
                Ok(None)
            } else {
                Err(RadrootsTradeProjectionError::InvalidTransition {
                    from: current.clone(),
                    to: TradeOrderStatus::Accepted,
                })
            }
        }
        TradeFulfillmentStatus::Delivered => {
            let next_status = TradeOrderStatus::Fulfilled;
            radroots_trade_order_status_ensure_transition(current.clone(), next_status.clone())?;
            Ok(Some(next_status))
        }
        TradeFulfillmentStatus::Cancelled => {
            let next_status = TradeOrderStatus::Cancelled;
            radroots_trade_order_status_ensure_transition(current.clone(), next_status.clone())?;
            Ok(Some(next_status))
        }
    }
}

fn trade_order_status_for_receipt(
    current: &TradeOrderStatus,
    acknowledged: bool,
) -> Result<Option<TradeOrderStatus>, RadrootsTradeProjectionError> {
    if acknowledged {
        let next_status = TradeOrderStatus::Completed;
        radroots_trade_order_status_ensure_transition(current.clone(), next_status.clone())?;
        Ok(Some(next_status))
    } else if matches!(current, TradeOrderStatus::Fulfilled) {
        Ok(None)
    } else {
        Err(RadrootsTradeProjectionError::InvalidTransition {
            from: current.clone(),
            to: TradeOrderStatus::Fulfilled,
        })
    }
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

fn require_listing_snapshot(
    message: &RadrootsTradeOrderWorkflowMessage,
) -> Result<RadrootsNostrEventPtr, RadrootsTradeProjectionError> {
    message
        .listing_event
        .clone()
        .ok_or(RadrootsTradeProjectionError::MissingListingSnapshot)
}

fn ensure_actor(expected: &str, actual: &str) -> Result<(), RadrootsTradeProjectionError> {
    if expected == actual {
        Ok(())
    } else {
        Err(RadrootsTradeProjectionError::UnauthorizedActor)
    }
}

fn ensure_counterparty(expected: &str, actual: &str) -> Result<(), RadrootsTradeProjectionError> {
    if expected == actual {
        Ok(())
    } else {
        Err(RadrootsTradeProjectionError::CounterpartyMismatch)
    }
}

fn ensure_trade_chain(
    order: &RadrootsTradeOrderWorkflowProjection,
    message: &RadrootsTradeOrderWorkflowMessage,
) -> Result<(), RadrootsTradeProjectionError> {
    let root_event_id = message
        .root_event_id
        .as_deref()
        .ok_or(RadrootsTradeProjectionError::MissingTradeRootEventId)?;
    if root_event_id != order.root_event_id {
        return Err(RadrootsTradeProjectionError::TradeThreadRootMismatch);
    }
    let prev_event_id = message
        .prev_event_id
        .as_deref()
        .ok_or(RadrootsTradeProjectionError::MissingTradePrevEventId)?;
    if prev_event_id != order.last_event_id {
        return Err(RadrootsTradeProjectionError::TradeThreadPrevMismatch);
    }
    Ok(())
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
            let index = *item_index as usize;
            let item = items
                .get_mut(index)
                .ok_or(RadrootsTradeProjectionError::InvalidItemIndex(*item_index))?;
            item.bin_count = *bin_count;
        }
        TradeOrderChange::ItemAdd { item } => items.push(item.clone()),
        TradeOrderChange::ItemRemove { item_index } => {
            let index = *item_index as usize;
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
) -> TradeDiscountDecisionValue {
    match decision {
        crate::listing::order::TradeDiscountDecision::Accept { value } => {
            TradeDiscountDecisionValue::Accepted(value.clone())
        }
        crate::listing::order::TradeDiscountDecision::Decline { reason } => {
            TradeDiscountDecisionValue::Declined(reason.clone())
        }
    }
}

impl RadrootsTradeListingMarketStatus {
    fn facet_key(&self) -> String {
        match self {
            Self::Unknown => "unknown".into(),
            Self::Window => "window".into(),
            Self::Active => "active".into(),
            Self::Sold => "sold".into(),
            Self::Other { value } => value.clone(),
        }
    }
}

fn order_status_key(status: &TradeOrderStatus) -> String {
    match status {
        TradeOrderStatus::Draft => "draft".into(),
        TradeOrderStatus::Validated => "validated".into(),
        TradeOrderStatus::Requested => "requested".into(),
        TradeOrderStatus::Questioned => "questioned".into(),
        TradeOrderStatus::Revised => "revised".into(),
        TradeOrderStatus::Accepted => "accepted".into(),
        TradeOrderStatus::Declined => "declined".into(),
        TradeOrderStatus::Cancelled => "cancelled".into(),
        TradeOrderStatus::Fulfilled => "fulfilled".into(),
        TradeOrderStatus::Completed => "completed".into(),
    }
}

fn message_type_key(message_type: TradeListingMessageType) -> &'static str {
    match message_type {
        TradeListingMessageType::ListingValidateRequest => "listing_validate_request",
        TradeListingMessageType::ListingValidateResult => "listing_validate_result",
        TradeListingMessageType::OrderRequest => "order_request",
        TradeListingMessageType::OrderResponse => "order_response",
        TradeListingMessageType::OrderRevision => "order_revision",
        TradeListingMessageType::OrderRevisionAccept => "order_revision_accept",
        TradeListingMessageType::OrderRevisionDecline => "order_revision_decline",
        TradeListingMessageType::Question => "question",
        TradeListingMessageType::Answer => "answer",
        TradeListingMessageType::DiscountRequest => "discount_request",
        TradeListingMessageType::DiscountOffer => "discount_offer",
        TradeListingMessageType::DiscountAccept => "discount_accept",
        TradeListingMessageType::DiscountDecline => "discount_decline",
        TradeListingMessageType::Cancel => "cancel",
        TradeListingMessageType::FulfillmentUpdate => "fulfillment_update",
        TradeListingMessageType::Receipt => "receipt",
    }
}

fn compare_direction(ordering: Ordering, direction: RadrootsTradeSortDirection) -> Ordering {
    match direction {
        RadrootsTradeSortDirection::Asc => ordering,
        RadrootsTradeSortDirection::Desc => ordering.reverse(),
    }
}

fn compare_option_decimal(
    left: &Option<RadrootsCoreDecimal>,
    right: &Option<RadrootsCoreDecimal>,
) -> Ordering {
    match (left, right) {
        (Some(left), Some(right)) => left.partial_cmp(right).unwrap_or(Ordering::Equal),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

fn compare_listings(
    left: &RadrootsTradeListingProjection,
    right: &RadrootsTradeListingProjection,
    sort: RadrootsTradeListingSort,
) -> Ordering {
    let ordering = match sort.field {
        RadrootsTradeListingSortField::ListingAddr => left.listing_addr.cmp(&right.listing_addr),
        RadrootsTradeListingSortField::ProductTitle => left
            .product
            .title
            .cmp(&right.product.title)
            .then_with(|| left.listing_addr.cmp(&right.listing_addr)),
        RadrootsTradeListingSortField::ProductCategory => left
            .product
            .category
            .cmp(&right.product.category)
            .then_with(|| left.listing_addr.cmp(&right.listing_addr)),
        RadrootsTradeListingSortField::SellerPubkey => left
            .seller_pubkey
            .cmp(&right.seller_pubkey)
            .then_with(|| left.listing_addr.cmp(&right.listing_addr)),
        RadrootsTradeListingSortField::InventoryAvailable => {
            compare_option_decimal(&left.inventory_available, &right.inventory_available)
                .then_with(|| left.listing_addr.cmp(&right.listing_addr))
        }
        RadrootsTradeListingSortField::OpenOrderCount => left
            .open_order_count
            .cmp(&right.open_order_count)
            .then_with(|| left.listing_addr.cmp(&right.listing_addr)),
        RadrootsTradeListingSortField::TotalOrderCount => left
            .order_count
            .cmp(&right.order_count)
            .then_with(|| left.listing_addr.cmp(&right.listing_addr)),
    };
    compare_direction(ordering, sort.direction)
}

fn compare_orders(
    left: &RadrootsTradeOrderWorkflowProjection,
    right: &RadrootsTradeOrderWorkflowProjection,
    sort: RadrootsTradeOrderSort,
) -> Ordering {
    let ordering = match sort.field {
        RadrootsTradeOrderSortField::OrderId => left.order_id.cmp(&right.order_id),
        RadrootsTradeOrderSortField::ListingAddr => left
            .listing_addr
            .cmp(&right.listing_addr)
            .then_with(|| left.order_id.cmp(&right.order_id)),
        RadrootsTradeOrderSortField::BuyerPubkey => left
            .buyer_pubkey
            .cmp(&right.buyer_pubkey)
            .then_with(|| left.order_id.cmp(&right.order_id)),
        RadrootsTradeOrderSortField::SellerPubkey => left
            .seller_pubkey
            .cmp(&right.seller_pubkey)
            .then_with(|| left.order_id.cmp(&right.order_id)),
        RadrootsTradeOrderSortField::Status => order_status_key(&left.status)
            .cmp(&order_status_key(&right.status))
            .then_with(|| left.order_id.cmp(&right.order_id)),
        RadrootsTradeOrderSortField::LastMessageType => message_type_key(left.last_message_type)
            .cmp(message_type_key(right.last_message_type))
            .then_with(|| left.order_id.cmp(&right.order_id)),
        RadrootsTradeOrderSortField::TotalBinCount => left
            .total_bin_count()
            .cmp(&right.total_bin_count())
            .then_with(|| left.order_id.cmp(&right.order_id)),
    };
    compare_direction(ordering, sort.direction)
}

fn listing_matches_query(
    listing: &RadrootsTradeListingProjection,
    query: &RadrootsTradeListingQuery,
) -> bool {
    if query
        .seller_pubkey
        .as_deref()
        .is_some_and(|value| value != listing.seller_pubkey)
    {
        return false;
    }
    if query
        .farm_pubkey
        .as_deref()
        .is_some_and(|value| value != listing.farm.pubkey)
    {
        return false;
    }
    if query
        .farm_id
        .as_deref()
        .is_some_and(|value| value != listing.farm.d_tag)
    {
        return false;
    }
    if query
        .product_key
        .as_deref()
        .is_some_and(|value| value != listing.product.key)
    {
        return false;
    }
    if query
        .product_category
        .as_deref()
        .is_some_and(|value| value != listing.product.category)
    {
        return false;
    }
    if query
        .listing_status
        .as_ref()
        .is_some_and(|value| value != &listing.market_status())
    {
        return false;
    }
    true
}

fn order_matches_query(
    order: &RadrootsTradeOrderWorkflowProjection,
    query: &RadrootsTradeOrderQuery,
) -> bool {
    if query
        .listing_addr
        .as_deref()
        .is_some_and(|value| value != order.listing_addr)
    {
        return false;
    }
    if query
        .buyer_pubkey
        .as_deref()
        .is_some_and(|value| value != order.buyer_pubkey)
    {
        return false;
    }
    if query
        .seller_pubkey
        .as_deref()
        .is_some_and(|value| value != order.seller_pubkey)
    {
        return false;
    }
    if query
        .status
        .as_ref()
        .is_some_and(|value| value != &order.status)
    {
        return false;
    }
    true
}

fn increment_count(counts: &mut BTreeMap<String, u32>, key: String) {
    let count = counts.entry(key).or_insert(0);
    *count = count.saturating_add(1);
}

fn facet_counts_from_map(counts: BTreeMap<String, u32>) -> Vec<RadrootsTradeFacetCount> {
    let mut values = counts
        .into_iter()
        .map(|(key, count)| RadrootsTradeFacetCount { key, count })
        .collect::<Vec<_>>();
    values.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.key.cmp(&right.key))
    });
    values
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::{
        RadrootsTradeListingMarketStatus, RadrootsTradeListingProjection,
        RadrootsTradeListingQuery, RadrootsTradeListingSort, RadrootsTradeListingSortField,
        RadrootsTradeOrderQuery, RadrootsTradeOrderSort, RadrootsTradeOrderSortField,
        RadrootsTradeOrderWorkflowMessage, RadrootsTradeOrderWorkflowProjection,
        RadrootsTradeProjectionError, RadrootsTradeReadIndex, RadrootsTradeSortDirection,
        radroots_trade_order_status_can_transition, radroots_trade_order_status_is_terminal,
    };
    use crate::listing::{
        codec::{TradeListingParseError, listing_tags_build},
        dvm::{
            TradeListingAddressError, TradeListingCancel, TradeListingEnvelopeParseError,
            TradeListingMessagePayload, TradeListingMessageType, TradeListingValidateRequest,
            TradeListingValidateResult, TradeOrderResponse, TradeOrderRevisionResponse,
            trade_listing_envelope_event_build,
        },
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
    use radroots_events::farm::RadrootsFarmRef;
    use radroots_events::listing::{
        RadrootsListing, RadrootsListingAvailability, RadrootsListingBin,
        RadrootsListingDeliveryMethod, RadrootsListingLocation, RadrootsListingProduct,
        RadrootsListingStatus,
    };
    use radroots_events::{RadrootsNostrEvent, RadrootsNostrEventPtr, kinds::KIND_LISTING};

    #[derive(Clone, Debug)]
    struct TestWorkflowChain {
        buyer_pubkey: String,
        seller_pubkey: String,
        root_event_id: String,
        last_event_id: String,
        next_sequence: u32,
    }

    thread_local! {
        static TEST_WORKFLOW_CHAINS: RefCell<std::collections::BTreeMap<String, TestWorkflowChain>> =
            RefCell::new(std::collections::BTreeMap::new());
    }

    fn listing_snapshot(listing_addr: &str) -> RadrootsNostrEventPtr {
        RadrootsNostrEventPtr {
            id: format!("snapshot:{listing_addr}"),
            relays: None,
        }
    }

    fn seller_pubkey_from_listing_addr(listing_addr: &str) -> String {
        listing_addr
            .split(':')
            .nth(1)
            .unwrap_or_default()
            .to_string()
    }

    fn workflow_refs(
        actor_pubkey: &str,
        listing_addr: &str,
        order_id: Option<&str>,
        payload: &TradeListingMessagePayload,
    ) -> (
        String,
        String,
        Option<RadrootsNostrEventPtr>,
        Option<String>,
        Option<String>,
    ) {
        let message_type = payload.message_type();
        let listing_event = message_type
            .requires_listing_snapshot()
            .then(|| listing_snapshot(listing_addr));
        let default_seller = seller_pubkey_from_listing_addr(listing_addr);

        match (payload, order_id) {
            (_, None) => (
                format!("event:no-order:{}:{actor_pubkey}", message_type.kind()),
                default_seller,
                listing_event,
                None,
                None,
            ),
            (TradeListingMessagePayload::OrderRequest(order), Some(order_id)) => {
                let event_id = format!("{order_id}:request");
                TEST_WORKFLOW_CHAINS.with(|chains| {
                    chains.borrow_mut().insert(
                        order_id.to_string(),
                        TestWorkflowChain {
                            buyer_pubkey: order.buyer_pubkey.clone(),
                            seller_pubkey: order.seller_pubkey.clone(),
                            root_event_id: event_id.clone(),
                            last_event_id: event_id.clone(),
                            next_sequence: 1,
                        },
                    );
                });
                (
                    event_id,
                    order.seller_pubkey.clone(),
                    listing_event,
                    None,
                    None,
                )
            }
            (_, Some(order_id)) => TEST_WORKFLOW_CHAINS.with(|chains| {
                let mut chains = chains.borrow_mut();
                let chain =
                    chains
                        .entry(order_id.to_string())
                        .or_insert_with(|| TestWorkflowChain {
                            buyer_pubkey: String::from("buyer-pubkey"),
                            seller_pubkey: default_seller.clone(),
                            root_event_id: format!("{order_id}:root"),
                            last_event_id: format!("{order_id}:root"),
                            next_sequence: 1,
                        });
                let event_id =
                    format!("{order_id}:{}:{}", message_type.kind(), chain.next_sequence);
                chain.next_sequence += 1;
                let counterparty_pubkey = if actor_pubkey == chain.seller_pubkey {
                    chain.buyer_pubkey.clone()
                } else {
                    chain.seller_pubkey.clone()
                };
                let prev_event_id = chain.last_event_id.clone();
                let root_event_id = chain.root_event_id.clone();
                chain.last_event_id = event_id.clone();
                (
                    event_id,
                    counterparty_pubkey,
                    listing_event,
                    Some(root_event_id),
                    Some(prev_event_id),
                )
            }),
        }
    }

    fn base_listing() -> RadrootsListing {
        RadrootsListing {
            d_tag: "AAAAAAAAAAAAAAAAAAAAAg".into(),
            farm: RadrootsFarmRef {
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
        }
    }

    fn alternate_listing() -> RadrootsListing {
        RadrootsListing {
            d_tag: "AAAAAAAAAAAAAAAAAAAAAw".into(),
            farm: RadrootsFarmRef {
                pubkey: "farm-pubkey-2".into(),
                d_tag: "AAAAAAAAAAAAAAAAAAAABA".into(),
            },
            product: RadrootsListingProduct {
                key: "greens".into(),
                title: "Greens".into(),
                category: "vegetables".into(),
                summary: Some("washed bunches".into()),
                process: None,
                lot: None,
                location: None,
                profile: None,
                year: None,
            },
            primary_bin_id: "bin-1".into(),
            bins: vec![RadrootsListingBin {
                bin_id: "bin-1".into(),
                quantity: RadrootsCoreQuantity::new(
                    RadrootsCoreDecimal::from(500u32),
                    RadrootsCoreUnit::MassG,
                ),
                price_per_canonical_unit: RadrootsCoreQuantityPrice::new(
                    RadrootsCoreMoney::new(
                        RadrootsCoreDecimal::from(4u32),
                        RadrootsCoreCurrency::USD,
                    ),
                    RadrootsCoreQuantity::new(
                        RadrootsCoreDecimal::from(1u32),
                        RadrootsCoreUnit::MassG,
                    ),
                ),
                display_amount: None,
                display_unit: None,
                display_label: Some("500g bunch".into()),
                display_price: Some(RadrootsCoreMoney::new(
                    RadrootsCoreDecimal::from(2000u32),
                    RadrootsCoreCurrency::USD,
                )),
                display_price_unit: Some(RadrootsCoreUnit::Each),
            }],
            resource_area: None,
            plot: None,
            discounts: None,
            inventory_available: Some(RadrootsCoreDecimal::from(4u32)),
            availability: Some(RadrootsListingAvailability::Window {
                start: Some(1_700_000_000),
                end: Some(1_800_000_000),
            }),
            delivery_method: Some(RadrootsListingDeliveryMethod::Shipping),
            location: Some(RadrootsListingLocation {
                primary: "warehouse".into(),
                city: Some("Louisville".into()),
                region: Some("KY".into()),
                country: Some("US".into()),
                lat: None,
                lng: None,
                geohash: None,
            }),
            images: None,
        }
    }

    fn alternate_order() -> TradeOrder {
        TradeOrder {
            order_id: "order-2".into(),
            listing_addr: "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAw".into(),
            buyer_pubkey: "buyer-pubkey-2".into(),
            seller_pubkey: "seller-pubkey".into(),
            items: vec![
                TradeOrderItem {
                    bin_id: "bin-1".into(),
                    bin_count: 3,
                },
                TradeOrderItem {
                    bin_id: "bin-2".into(),
                    bin_count: 1,
                },
            ],
            discounts: None,
        }
    }

    fn message(
        actor_pubkey: &str,
        listing_addr: &str,
        order_id: Option<&str>,
        payload: TradeListingMessagePayload,
    ) -> RadrootsTradeOrderWorkflowMessage {
        let (event_id, counterparty_pubkey, listing_event, root_event_id, prev_event_id) =
            workflow_refs(actor_pubkey, listing_addr, order_id, &payload);
        RadrootsTradeOrderWorkflowMessage {
            event_id,
            actor_pubkey: actor_pubkey.into(),
            counterparty_pubkey,
            listing_addr: listing_addr.into(),
            order_id: order_id.map(str::to_string),
            listing_event,
            root_event_id,
            prev_event_id,
            payload,
        }
    }

    fn listing_event(seller_pubkey: &str, listing: &RadrootsListing) -> RadrootsNostrEvent {
        RadrootsNostrEvent {
            id: "listing-event-id".into(),
            author: seller_pubkey.into(),
            created_at: 1_700_000_000,
            kind: KIND_LISTING,
            tags: listing_tags_build(listing).expect("listing tags"),
            content: serde_json::to_string(listing).expect("listing json"),
            sig: "sig".into(),
        }
    }

    fn workflow_event(
        actor_pubkey: &str,
        recipient_pubkey: &str,
        message_type: crate::listing::dvm::TradeListingMessageType,
        listing_addr: &str,
        order_id: Option<&str>,
        payload: &TradeListingMessagePayload,
    ) -> RadrootsNostrEvent {
        let (_, _, listing_event, root_event_id, prev_event_id) =
            workflow_refs(actor_pubkey, listing_addr, order_id, payload);
        let built = trade_listing_envelope_event_build(
            recipient_pubkey,
            message_type,
            listing_addr.to_string(),
            order_id.map(str::to_string),
            listing_event.as_ref(),
            root_event_id.as_deref(),
            prev_event_id.as_deref(),
            payload,
        )
        .expect("trade workflow event");
        RadrootsNostrEvent {
            id: "workflow-event-id".into(),
            author: actor_pubkey.into(),
            created_at: 1_700_000_000,
            kind: u32::from(built.kind),
            tags: built.tags,
            content: built.content,
            sig: "sig".into(),
        }
    }

    #[test]
    fn projection_defaults_and_helper_errors_cover_paths() {
        let listing_sort = RadrootsTradeListingSort::default();
        assert_eq!(
            listing_sort.field,
            RadrootsTradeListingSortField::ListingAddr
        );
        assert_eq!(listing_sort.direction, RadrootsTradeSortDirection::Asc);

        let order_sort = RadrootsTradeOrderSort::default();
        assert_eq!(order_sort.field, RadrootsTradeOrderSortField::OrderId);
        assert_eq!(order_sort.direction, RadrootsTradeSortDirection::Asc);

        let mut listing = base_listing();
        listing.availability = Some(RadrootsListingAvailability::Status {
            status: RadrootsListingStatus::Other {
                value: "archived".into(),
            },
        });
        let projection =
            RadrootsTradeListingProjection::from_listing_contract("seller-pubkey", &listing)
                .expect("listing projection");
        assert_eq!(
            projection.market_status(),
            RadrootsTradeListingMarketStatus::Other {
                value: "archived".into(),
            }
        );
        listing.availability = None;
        let unknown_projection =
            RadrootsTradeListingProjection::from_listing_contract("seller-pubkey", &listing)
                .expect("unknown listing projection");
        assert_eq!(
            unknown_projection.market_status(),
            RadrootsTradeListingMarketStatus::Unknown
        );
        let mut missing_primary_bin_projection = unknown_projection.clone();
        missing_primary_bin_projection.primary_bin_id = "missing-bin".into();
        assert!(
            missing_primary_bin_projection
                .marketplace_summary()
                .is_none()
        );

        let mut index = RadrootsTradeReadIndex::new();
        assert!(index.listings().is_empty());
        assert!(index.orders().is_empty());
        index
            .upsert_listing("seller-pubkey", &base_listing())
            .expect("listing");
        index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                Some("order-1"),
                TradeListingMessagePayload::OrderRequest(base_order()),
            ))
            .expect("order request");
        assert_eq!(index.listings().len(), 1);
        assert_eq!(index.orders().len(), 1);

        let cases = [
            (
                RadrootsTradeProjectionError::InvalidListingKind { kind: 7 },
                "invalid listing event kind: 7",
                false,
            ),
            (
                RadrootsTradeProjectionError::InvalidListingContract {
                    error: TradeListingParseError::InvalidTag("d".into()),
                },
                "invalid listing contract event: invalid tag: d",
                true,
            ),
            (
                RadrootsTradeProjectionError::MissingPrimaryBin("bin-9".into()),
                "missing primary bin: bin-9",
                false,
            ),
            (
                RadrootsTradeProjectionError::MissingOrderId,
                "missing order id",
                false,
            ),
            (
                RadrootsTradeProjectionError::OrderIdMismatch,
                "order id mismatch",
                false,
            ),
            (
                RadrootsTradeProjectionError::ListingAddrMismatch,
                "listing address mismatch",
                false,
            ),
            (
                RadrootsTradeProjectionError::MissingOrder("order-9".into()),
                "missing order projection: order-9",
                false,
            ),
            (
                RadrootsTradeProjectionError::InvalidTransition {
                    from: TradeOrderStatus::Draft,
                    to: TradeOrderStatus::Accepted,
                },
                "invalid order transition: Draft -> Accepted",
                false,
            ),
            (
                RadrootsTradeProjectionError::InvalidItemIndex(3),
                "invalid order item index: 3",
                false,
            ),
            (
                RadrootsTradeProjectionError::InvalidDiscountDecision,
                "invalid discount decision payload",
                false,
            ),
            (
                RadrootsTradeProjectionError::InvalidRevisionResponse,
                "invalid order revision response payload",
                false,
            ),
            (
                RadrootsTradeProjectionError::NonOrderWorkflowMessage(
                    TradeListingMessageType::ListingValidateRequest,
                ),
                "non-order workflow message: ListingValidateRequest",
                false,
            ),
            (
                RadrootsTradeProjectionError::UnauthorizedActor,
                "unauthorized actor",
                false,
            ),
            (
                RadrootsTradeProjectionError::CounterpartyMismatch,
                "counterparty pubkey mismatch",
                false,
            ),
            (
                RadrootsTradeProjectionError::MissingListingSnapshot,
                "missing listing snapshot",
                false,
            ),
            (
                RadrootsTradeProjectionError::MissingTradeRootEventId,
                "missing trade root event id",
                false,
            ),
            (
                RadrootsTradeProjectionError::MissingTradePrevEventId,
                "missing trade previous event id",
                false,
            ),
            (
                RadrootsTradeProjectionError::TradeThreadRootMismatch,
                "trade thread root mismatch",
                false,
            ),
            (
                RadrootsTradeProjectionError::TradeThreadPrevMismatch,
                "trade thread previous event mismatch",
                false,
            ),
            (
                RadrootsTradeProjectionError::InvalidWorkflowEvent {
                    error: TradeListingEnvelopeParseError::InvalidListingAddr(
                        TradeListingAddressError::InvalidFormat,
                    ),
                },
                "invalid listing address format",
                true,
            ),
        ];
        for (error, expected, has_source) in cases {
            assert_eq!(error.to_string(), expected);
            assert_eq!(std::error::Error::source(&error).is_some(), has_source);
        }
    }

    #[test]
    fn listing_projection_from_event_rejects_invalid_kind_and_invalid_contract() {
        let mut invalid_kind = listing_event("seller-pubkey", &base_listing());
        invalid_kind.kind = 7;
        assert_eq!(
            RadrootsTradeListingProjection::from_listing_event(&invalid_kind)
                .expect_err("invalid kind"),
            RadrootsTradeProjectionError::InvalidListingKind { kind: 7 }
        );

        let invalid_contract = RadrootsNostrEvent {
            id: "bad-listing".into(),
            author: "seller-pubkey".into(),
            created_at: 1_700_000_000,
            kind: KIND_LISTING,
            tags: vec![],
            content: "{}".into(),
            sig: "sig".into(),
        };
        let invalid_contract_error =
            RadrootsTradeListingProjection::from_listing_event(&invalid_contract)
                .expect_err("invalid contract");
        let invalid_contract_source =
            std::error::Error::source(&invalid_contract_error).expect("invalid contract source");
        assert_eq!(
            invalid_contract_error.to_string(),
            format!("invalid listing contract event: {invalid_contract_source}")
        );

        let mut missing_primary_bin = base_listing();
        missing_primary_bin.primary_bin_id = "missing-bin".into();
        let missing_primary_bin_event = listing_event("seller-pubkey", &missing_primary_bin);
        assert_eq!(
            RadrootsTradeListingProjection::from_listing_event(&missing_primary_bin_event)
                .expect_err("missing primary bin"),
            RadrootsTradeProjectionError::MissingPrimaryBin("missing-bin".into())
        );

        let mut index = RadrootsTradeReadIndex::new();
        assert_eq!(
            index
                .upsert_listing_event(&missing_primary_bin_event)
                .expect_err("index missing primary bin"),
            RadrootsTradeProjectionError::MissingPrimaryBin("missing-bin".into())
        );
    }

    #[test]
    fn message_helper_bootstraps_missing_chain_for_non_request_payload() {
        let message = message(
            "seller-pubkey",
            "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
            Some("orphan-order"),
            TradeListingMessagePayload::Cancel(TradeListingCancel {
                reason: Some("cancelled".into()),
            }),
        );

        assert_eq!(message.order_id.as_deref(), Some("orphan-order"));
        assert_eq!(message.counterparty_pubkey, "buyer-pubkey");
        assert_eq!(message.root_event_id.as_deref(), Some("orphan-order:root"));
        assert_eq!(message.prev_event_id.as_deref(), Some("orphan-order:root"));
    }

    #[test]
    fn workflow_message_from_event_rejects_missing_trade_context_tags() {
        let valid_event = workflow_event(
            "seller-pubkey",
            "buyer-pubkey",
            crate::listing::dvm::TradeListingMessageType::OrderResponse,
            "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
            Some("order-valid-tags"),
            &TradeListingMessagePayload::OrderResponse(TradeOrderResponse {
                accepted: true,
                reason: None,
            }),
        );
        let valid_message =
            RadrootsTradeOrderWorkflowMessage::from_event(&valid_event).expect("valid workflow");
        assert_eq!(valid_message.order_id.as_deref(), Some("order-valid-tags"));

        let mut event = workflow_event(
            "seller-pubkey",
            "buyer-pubkey",
            crate::listing::dvm::TradeListingMessageType::OrderResponse,
            "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
            Some("order-missing-tags"),
            &TradeListingMessagePayload::OrderResponse(TradeOrderResponse {
                accepted: true,
                reason: None,
            }),
        );
        event.tags.retain(|tag| {
            !matches!(
                tag.first().map(String::as_str),
                Some("e_root") | Some("e_prev")
            )
        });

        assert_eq!(
            RadrootsTradeOrderWorkflowMessage::from_event(&event),
            Err(TradeListingEnvelopeParseError::MissingTag("e_root"))
        );
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
    fn listing_projection_can_ingest_canonical_nostr_event() {
        let mut index = RadrootsTradeReadIndex::new();
        let event = listing_event("seller-pubkey", &base_listing());

        let projection = index
            .upsert_listing_event(&event)
            .expect("listing event projection");

        assert_eq!(
            projection.listing_addr,
            "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg"
        );
        assert_eq!(projection.bins.len(), 2);
    }

    #[test]
    fn workflow_projection_can_ingest_canonical_trade_event() {
        let mut index = RadrootsTradeReadIndex::new();
        index
            .upsert_listing_event(&listing_event("seller-pubkey", &base_listing()))
            .expect("listing projection");
        let event = workflow_event(
            "buyer-pubkey",
            "seller-pubkey",
            crate::listing::dvm::TradeListingMessageType::OrderRequest,
            "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
            Some("order-1"),
            &TradeListingMessagePayload::OrderRequest(base_order()),
        );

        let order = index
            .apply_workflow_event(&event)
            .expect("workflow event projection");

        assert_eq!(order.order_id, "order-1");
        assert_eq!(order.status, TradeOrderStatus::Requested);
        assert_eq!(order.last_actor_pubkey, "buyer-pubkey");
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
    fn workflow_projection_keeps_in_progress_fulfillment_as_accepted() {
        let mut index = RadrootsTradeReadIndex::new();
        index
            .upsert_listing("seller-pubkey", &base_listing())
            .expect("listing projection");
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
                    status: TradeFulfillmentStatus::Shipped,
                }),
            ))
            .expect("fulfillment update");

        let order = index.order("order-1").expect("order");
        assert_eq!(order.status, TradeOrderStatus::Accepted);
        assert_eq!(
            order.last_fulfillment_status,
            Some(TradeFulfillmentStatus::Shipped)
        );
        let listing = index
            .listing("30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg")
            .expect("listing");
        assert_eq!(listing.open_order_count, 1);
        assert_eq!(listing.terminal_order_count, 0);
    }

    #[test]
    fn workflow_projection_requires_acknowledged_receipt_for_completion() {
        let mut index = RadrootsTradeReadIndex::new();
        index
            .upsert_listing("seller-pubkey", &base_listing())
            .expect("listing projection");
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
                }),
            ))
            .expect("fulfilled");
        index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                Some("order-1"),
                TradeListingMessagePayload::Receipt(TradeReceipt {
                    acknowledged: false,
                    at: 1_700_000_000,
                }),
            ))
            .expect("receipt");

        let order = index.order("order-1").expect("order");
        assert_eq!(order.status, TradeOrderStatus::Fulfilled);
        assert_eq!(order.receipt_acknowledged, Some(false));
        let listing = index
            .listing("30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg")
            .expect("listing");
        assert_eq!(listing.open_order_count, 1);
        assert_eq!(listing.terminal_order_count, 0);
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
                    value: radroots_core::RadrootsCoreDiscountValue::Percent(
                        RadrootsCorePercent::new(RadrootsCoreDecimal::from(15u32)),
                    ),
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
                    value: radroots_core::RadrootsCoreDiscountValue::Percent(
                        RadrootsCorePercent::new(RadrootsCoreDecimal::from(12u32)),
                    ),
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

        let order = index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                Some("order-1"),
                TradeListingMessagePayload::OrderRequest(base_order()),
            ))
            .expect("order request");
        assert_eq!(
            order.status,
            TradeOrderStatus::Requested,
            "canonical helper should still create a requested order"
        );

        let err = index
            .apply_workflow_message(&RadrootsTradeOrderWorkflowMessage {
                event_id: "missing-snapshot".into(),
                actor_pubkey: "buyer-pubkey".into(),
                counterparty_pubkey: "seller-pubkey".into(),
                listing_addr: "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg".into(),
                order_id: Some("order-2".into()),
                listing_event: None,
                root_event_id: None,
                prev_event_id: None,
                payload: TradeListingMessagePayload::OrderRequest(TradeOrder {
                    order_id: "order-2".into(),
                    listing_addr: "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg".into(),
                    buyer_pubkey: "buyer-pubkey".into(),
                    seller_pubkey: "seller-pubkey".into(),
                    items: vec![TradeOrderItem {
                        bin_id: "bin-1".into(),
                        bin_count: 1,
                    }],
                    discounts: None,
                }),
            })
            .expect_err("order request without snapshot should fail");
        assert_eq!(err, RadrootsTradeProjectionError::MissingListingSnapshot);
    }

    #[test]
    fn workflow_projection_rejects_invalid_canonical_trade_event() {
        let mut index = RadrootsTradeReadIndex::new();
        let mut event = workflow_event(
            "buyer-pubkey",
            "seller-pubkey",
            crate::listing::dvm::TradeListingMessageType::OrderRequest,
            "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
            Some("order-1"),
            &TradeListingMessagePayload::OrderRequest(base_order()),
        );
        event.tags[1][1] = "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAw".into();

        let err = index
            .apply_workflow_event(&event)
            .expect_err("invalid workflow event should fail");
        assert_eq!(
            err,
            RadrootsTradeProjectionError::InvalidWorkflowEvent {
                error: TradeListingEnvelopeParseError::ListingAddrTagMismatch,
            }
        );
    }

    #[test]
    fn listing_query_helpers_filter_sort_and_facet_marketplace_views() {
        let mut index = RadrootsTradeReadIndex::new();
        index
            .upsert_listing("seller-pubkey", &base_listing())
            .expect("base listing");
        index
            .upsert_listing("seller-pubkey", &alternate_listing())
            .expect("alternate listing");

        index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                Some("order-1"),
                TradeListingMessagePayload::OrderRequest(base_order()),
            ))
            .expect("open order");
        index
            .apply_workflow_message(&message(
                "buyer-pubkey-2",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAw",
                Some("order-2"),
                TradeListingMessagePayload::OrderRequest(alternate_order()),
            ))
            .expect("second order request");
        index
            .apply_workflow_message(&message(
                "seller-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAw",
                Some("order-2"),
                TradeListingMessagePayload::OrderResponse(TradeOrderResponse {
                    accepted: true,
                    reason: None,
                }),
            ))
            .expect("order accepted");
        index
            .apply_workflow_message(&message(
                "seller-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAw",
                Some("order-2"),
                TradeListingMessagePayload::FulfillmentUpdate(TradeFulfillmentUpdate {
                    status: TradeFulfillmentStatus::Delivered,
                }),
            ))
            .expect("order fulfilled");
        index
            .apply_workflow_message(&message(
                "buyer-pubkey-2",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAw",
                Some("order-2"),
                TradeListingMessagePayload::Receipt(TradeReceipt {
                    acknowledged: true,
                    at: 1_700_000_010,
                }),
            ))
            .expect("order receipt");

        let coffee_query = RadrootsTradeListingQuery {
            product_category: Some("coffee".into()),
            ..Default::default()
        };
        let coffee_results = index.query_listings(
            &coffee_query,
            RadrootsTradeListingSort {
                field: RadrootsTradeListingSortField::ProductTitle,
                direction: RadrootsTradeSortDirection::Asc,
            },
        );
        assert_eq!(coffee_results.len(), 1);
        assert_eq!(coffee_results[0].listing_id, "AAAAAAAAAAAAAAAAAAAAAg");

        let listing_summaries = index.marketplace_listing_summaries(
            &RadrootsTradeListingQuery::default(),
            RadrootsTradeListingSort {
                field: RadrootsTradeListingSortField::OpenOrderCount,
                direction: RadrootsTradeSortDirection::Desc,
            },
        );
        assert_eq!(listing_summaries.len(), 2);
        assert_eq!(listing_summaries[0].listing_addr, base_order().listing_addr);
        assert_eq!(listing_summaries[0].open_order_count, 1);
        assert_eq!(
            listing_summaries[0].primary_bin_label.as_deref(),
            Some("1kg bag")
        );
        assert_eq!(
            listing_summaries[1].listing_status,
            RadrootsTradeListingMarketStatus::Window
        );
        assert_eq!(
            listing_summaries[1].location_primary.as_deref(),
            Some("warehouse")
        );

        let facets = index.listing_facets(&RadrootsTradeListingQuery::default());
        assert_eq!(facets.farm_pubkeys.len(), 2);
        assert_eq!(facets.product_categories.len(), 2);
        assert_eq!(
            facets
                .listing_statuses
                .iter()
                .map(|facet| (facet.key.as_str(), facet.count))
                .collect::<Vec<_>>(),
            vec![("active", 1), ("window", 1)]
        );
    }

    #[test]
    fn projection_helper_comparators_and_queries_cover_remaining_paths() {
        let listing_a =
            RadrootsTradeListingProjection::from_listing_contract("seller-pubkey", &base_listing())
                .expect("listing a");
        let mut listing_b = listing_a.clone();
        listing_b.listing_addr = "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAz".into();
        listing_b.listing_id = "AAAAAAAAAAAAAAAAAAAAAz".into();

        assert_eq!(
            super::compare_option_decimal(
                &Some(RadrootsCoreDecimal::from(10u32)),
                &Some(RadrootsCoreDecimal::from(10u32)),
            ),
            core::cmp::Ordering::Equal
        );
        assert_eq!(
            super::compare_option_decimal(&Some(RadrootsCoreDecimal::from(10u32)), &None),
            core::cmp::Ordering::Less
        );
        assert_eq!(
            super::compare_option_decimal(&None, &Some(RadrootsCoreDecimal::from(10u32))),
            core::cmp::Ordering::Greater
        );
        assert_eq!(
            super::compare_option_decimal(&None, &None),
            core::cmp::Ordering::Equal
        );

        for field in [
            RadrootsTradeListingSortField::ProductTitle,
            RadrootsTradeListingSortField::ProductCategory,
            RadrootsTradeListingSortField::SellerPubkey,
            RadrootsTradeListingSortField::InventoryAvailable,
            RadrootsTradeListingSortField::OpenOrderCount,
            RadrootsTradeListingSortField::TotalOrderCount,
        ] {
            assert_eq!(
                super::compare_listings(
                    &listing_a,
                    &listing_b,
                    RadrootsTradeListingSort {
                        field,
                        direction: RadrootsTradeSortDirection::Asc,
                    },
                ),
                core::cmp::Ordering::Less
            );
        }

        assert!(super::listing_matches_query(
            &listing_a,
            &RadrootsTradeListingQuery::default()
        ));
        assert!(!super::listing_matches_query(
            &listing_a,
            &RadrootsTradeListingQuery {
                seller_pubkey: Some("other-seller".into()),
                ..Default::default()
            }
        ));
        assert!(!super::listing_matches_query(
            &listing_a,
            &RadrootsTradeListingQuery {
                farm_pubkey: Some("other-farm".into()),
                ..Default::default()
            }
        ));
        assert!(!super::listing_matches_query(
            &listing_a,
            &RadrootsTradeListingQuery {
                farm_id: Some("other-farm-id".into()),
                ..Default::default()
            }
        ));
        assert!(!super::listing_matches_query(
            &listing_a,
            &RadrootsTradeListingQuery {
                product_key: Some("other-key".into()),
                ..Default::default()
            }
        ));
        assert!(!super::listing_matches_query(
            &listing_a,
            &RadrootsTradeListingQuery {
                product_category: Some("other-category".into()),
                ..Default::default()
            }
        ));
        assert!(!super::listing_matches_query(
            &listing_a,
            &RadrootsTradeListingQuery {
                listing_status: Some(RadrootsTradeListingMarketStatus::Sold),
                ..Default::default()
            }
        ));

        let request_message = message(
            "buyer-pubkey",
            "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
            Some("order-1"),
            TradeListingMessagePayload::OrderRequest(base_order()),
        );
        let order_a = RadrootsTradeOrderWorkflowProjection::from_order_request(
            &request_message,
            &base_order(),
        )
        .expect("order a");
        let mut order_b = order_a.clone();
        order_b.order_id = "order-2".into();

        for field in [
            RadrootsTradeOrderSortField::ListingAddr,
            RadrootsTradeOrderSortField::BuyerPubkey,
            RadrootsTradeOrderSortField::SellerPubkey,
            RadrootsTradeOrderSortField::Status,
            RadrootsTradeOrderSortField::LastMessageType,
            RadrootsTradeOrderSortField::TotalBinCount,
        ] {
            assert_eq!(
                super::compare_orders(
                    &order_a,
                    &order_b,
                    RadrootsTradeOrderSort {
                        field,
                        direction: RadrootsTradeSortDirection::Asc,
                    },
                ),
                core::cmp::Ordering::Less
            );
        }

        let message_type_expectations = [
            (
                TradeListingMessageType::ListingValidateRequest,
                "listing_validate_request",
            ),
            (
                TradeListingMessageType::ListingValidateResult,
                "listing_validate_result",
            ),
            (TradeListingMessageType::OrderRequest, "order_request"),
            (TradeListingMessageType::OrderResponse, "order_response"),
            (TradeListingMessageType::OrderRevision, "order_revision"),
            (
                TradeListingMessageType::OrderRevisionAccept,
                "order_revision_accept",
            ),
            (
                TradeListingMessageType::OrderRevisionDecline,
                "order_revision_decline",
            ),
            (TradeListingMessageType::Question, "question"),
            (TradeListingMessageType::Answer, "answer"),
            (TradeListingMessageType::DiscountRequest, "discount_request"),
            (TradeListingMessageType::DiscountOffer, "discount_offer"),
            (TradeListingMessageType::DiscountAccept, "discount_accept"),
            (TradeListingMessageType::DiscountDecline, "discount_decline"),
            (TradeListingMessageType::Cancel, "cancel"),
            (
                TradeListingMessageType::FulfillmentUpdate,
                "fulfillment_update",
            ),
            (TradeListingMessageType::Receipt, "receipt"),
        ];
        for (message_type, expected) in message_type_expectations {
            assert_eq!(super::message_type_key(message_type), expected);
        }

        let message_type_cases = [
            (
                message(
                    "seller-pubkey",
                    "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                    None,
                    TradeListingMessagePayload::ListingValidateRequest(
                        TradeListingValidateRequest {
                            listing_event: Some(listing_snapshot(
                                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                            )),
                        },
                    ),
                ),
                TradeListingMessageType::ListingValidateRequest,
            ),
            (
                message(
                    "seller-pubkey",
                    "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                    None,
                    TradeListingMessagePayload::ListingValidateResult(TradeListingValidateResult {
                        valid: true,
                        errors: vec![],
                    }),
                ),
                TradeListingMessageType::ListingValidateResult,
            ),
            (
                message(
                    "buyer-pubkey",
                    "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                    Some("message-type-order-request"),
                    TradeListingMessagePayload::OrderRequest(TradeOrder {
                        order_id: "message-type-order-request".into(),
                        ..base_order()
                    }),
                ),
                TradeListingMessageType::OrderRequest,
            ),
            (
                message(
                    "seller-pubkey",
                    "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                    Some("message-type-order-response"),
                    TradeListingMessagePayload::OrderResponse(TradeOrderResponse {
                        accepted: true,
                        reason: None,
                    }),
                ),
                TradeListingMessageType::OrderResponse,
            ),
            (
                message(
                    "seller-pubkey",
                    "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                    Some("message-type-order-revision"),
                    TradeListingMessagePayload::OrderRevision(TradeOrderRevision {
                        revision_id: "revision-1".into(),
                        changes: vec![TradeOrderChange::BinCount {
                            item_index: 0,
                            bin_count: 3,
                        }],
                    }),
                ),
                TradeListingMessageType::OrderRevision,
            ),
            (
                message(
                    "buyer-pubkey",
                    "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                    Some("message-type-order-revision-accept"),
                    TradeListingMessagePayload::OrderRevisionAccept(TradeOrderRevisionResponse {
                        accepted: true,
                        reason: None,
                    }),
                ),
                TradeListingMessageType::OrderRevisionAccept,
            ),
            (
                message(
                    "buyer-pubkey",
                    "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                    Some("message-type-order-revision-decline"),
                    TradeListingMessagePayload::OrderRevisionDecline(TradeOrderRevisionResponse {
                        accepted: false,
                        reason: Some("no".into()),
                    }),
                ),
                TradeListingMessageType::OrderRevisionDecline,
            ),
            (
                message(
                    "buyer-pubkey",
                    "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                    Some("message-type-question"),
                    TradeListingMessagePayload::Question(TradeQuestion {
                        question_id: "question-1".into(),
                    }),
                ),
                TradeListingMessageType::Question,
            ),
            (
                message(
                    "seller-pubkey",
                    "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                    Some("message-type-answer"),
                    TradeListingMessagePayload::Answer(TradeAnswer {
                        question_id: "question-1".into(),
                    }),
                ),
                TradeListingMessageType::Answer,
            ),
            (
                message(
                    "buyer-pubkey",
                    "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                    Some("message-type-discount-request"),
                    TradeListingMessagePayload::DiscountRequest(TradeDiscountRequest {
                        discount_id: "discount-1".into(),
                        value: radroots_core::RadrootsCoreDiscountValue::Percent(
                            RadrootsCorePercent::new(RadrootsCoreDecimal::from(10u32)),
                        ),
                    }),
                ),
                TradeListingMessageType::DiscountRequest,
            ),
            (
                message(
                    "seller-pubkey",
                    "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                    Some("message-type-discount-offer"),
                    TradeListingMessagePayload::DiscountOffer(TradeDiscountOffer {
                        discount_id: "discount-1".into(),
                        value: radroots_core::RadrootsCoreDiscountValue::Percent(
                            RadrootsCorePercent::new(RadrootsCoreDecimal::from(5u32)),
                        ),
                    }),
                ),
                TradeListingMessageType::DiscountOffer,
            ),
            (
                message(
                    "buyer-pubkey",
                    "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                    Some("message-type-discount-accept"),
                    TradeListingMessagePayload::DiscountAccept(TradeDiscountDecision::Accept {
                        value: radroots_core::RadrootsCoreDiscountValue::Percent(
                            RadrootsCorePercent::new(RadrootsCoreDecimal::from(5u32)),
                        ),
                    }),
                ),
                TradeListingMessageType::DiscountAccept,
            ),
            (
                message(
                    "buyer-pubkey",
                    "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                    Some("message-type-discount-decline"),
                    TradeListingMessagePayload::DiscountDecline(TradeDiscountDecision::Decline {
                        reason: Some("no".into()),
                    }),
                ),
                TradeListingMessageType::DiscountDecline,
            ),
            (
                message(
                    "buyer-pubkey",
                    "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                    Some("message-type-cancel"),
                    TradeListingMessagePayload::Cancel(TradeListingCancel {
                        reason: Some("cancel".into()),
                    }),
                ),
                TradeListingMessageType::Cancel,
            ),
            (
                message(
                    "seller-pubkey",
                    "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                    Some("message-type-fulfillment"),
                    TradeListingMessagePayload::FulfillmentUpdate(TradeFulfillmentUpdate {
                        status: TradeFulfillmentStatus::Preparing,
                    }),
                ),
                TradeListingMessageType::FulfillmentUpdate,
            ),
            (
                message(
                    "buyer-pubkey",
                    "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                    Some("message-type-receipt"),
                    TradeListingMessagePayload::Receipt(TradeReceipt {
                        acknowledged: true,
                        at: 1_700_000_000,
                    }),
                ),
                TradeListingMessageType::Receipt,
            ),
        ];
        for (message, expected) in message_type_cases {
            assert_eq!(message.message_type(), expected);
        }

        assert!(super::order_matches_query(
            &order_a,
            &RadrootsTradeOrderQuery::default()
        ));
        assert!(!super::order_matches_query(
            &order_a,
            &RadrootsTradeOrderQuery {
                listing_addr: Some("other-listing".into()),
                ..Default::default()
            }
        ));
        assert!(!super::order_matches_query(
            &order_a,
            &RadrootsTradeOrderQuery {
                buyer_pubkey: Some("other-buyer".into()),
                ..Default::default()
            }
        ));
        assert!(!super::order_matches_query(
            &order_a,
            &RadrootsTradeOrderQuery {
                seller_pubkey: Some("other-seller".into()),
                ..Default::default()
            }
        ));
        assert!(!super::order_matches_query(
            &order_a,
            &RadrootsTradeOrderQuery {
                status: Some(TradeOrderStatus::Completed),
                ..Default::default()
            }
        ));
    }

    #[test]
    fn order_query_helpers_filter_sort_and_facet_marketplace_views() {
        let mut index = RadrootsTradeReadIndex::new();
        index
            .upsert_listing("seller-pubkey", &base_listing())
            .expect("base listing");
        index
            .upsert_listing("seller-pubkey", &alternate_listing())
            .expect("alternate listing");

        index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                Some("order-1"),
                TradeListingMessagePayload::OrderRequest(base_order()),
            ))
            .expect("first order");
        index
            .apply_workflow_message(&message(
                "buyer-pubkey-2",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAw",
                Some("order-2"),
                TradeListingMessagePayload::OrderRequest(alternate_order()),
            ))
            .expect("second order");
        index
            .apply_workflow_message(&message(
                "seller-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAw",
                Some("order-2"),
                TradeListingMessagePayload::OrderResponse(TradeOrderResponse {
                    accepted: true,
                    reason: Some("approved".into()),
                }),
            ))
            .expect("accepted");
        index
            .apply_workflow_message(&message(
                "seller-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAw",
                Some("order-2"),
                TradeListingMessagePayload::FulfillmentUpdate(TradeFulfillmentUpdate {
                    status: TradeFulfillmentStatus::Delivered,
                }),
            ))
            .expect("fulfilled");
        index
            .apply_workflow_message(&message(
                "buyer-pubkey-2",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAw",
                Some("order-2"),
                TradeListingMessagePayload::Receipt(TradeReceipt {
                    acknowledged: true,
                    at: 1_700_000_020,
                }),
            ))
            .expect("completed");

        let completed_query = RadrootsTradeOrderQuery {
            seller_pubkey: Some("seller-pubkey".into()),
            status: Some(TradeOrderStatus::Completed),
            ..Default::default()
        };
        let completed_orders = index.query_orders(
            &completed_query,
            RadrootsTradeOrderSort {
                field: RadrootsTradeOrderSortField::TotalBinCount,
                direction: RadrootsTradeSortDirection::Desc,
            },
        );
        assert_eq!(completed_orders.len(), 1);
        assert_eq!(completed_orders[0].order_id, "order-2");
        assert_eq!(completed_orders[0].total_bin_count(), 4);

        let summaries = index.marketplace_order_summaries(
            &RadrootsTradeOrderQuery {
                seller_pubkey: Some("seller-pubkey".into()),
                ..Default::default()
            },
            RadrootsTradeOrderSort {
                field: RadrootsTradeOrderSortField::TotalBinCount,
                direction: RadrootsTradeSortDirection::Desc,
            },
        );
        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[0].order_id, "order-2");
        assert_eq!(summaries[0].item_count, 2);
        assert_eq!(summaries[0].total_bin_count, 4);
        assert!(!summaries[0].has_requested_discounts);
        assert_eq!(summaries[0].last_reason, None);
        assert_eq!(summaries[1].order_id, "order-1");
        assert!(summaries[1].has_requested_discounts);

        let facets = index.order_facets(&RadrootsTradeOrderQuery::default());
        assert_eq!(
            facets
                .statuses
                .iter()
                .map(|facet| (facet.key.as_str(), facet.count))
                .collect::<Vec<_>>(),
            vec![("completed", 1), ("requested", 1)]
        );
        assert_eq!(
            facets
                .buyer_pubkeys
                .iter()
                .map(|facet| (facet.key.as_str(), facet.count))
                .collect::<Vec<_>>(),
            vec![("buyer-pubkey", 1), ("buyer-pubkey-2", 1)]
        );
    }

    #[test]
    fn workflow_projection_covers_remaining_error_branches() {
        let mut index = RadrootsTradeReadIndex::new();
        index
            .upsert_listing("seller-pubkey", &base_listing())
            .expect("listing");

        assert_eq!(
            index
                .order_mut_checked("missing", "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",)
                .expect_err("missing order"),
            RadrootsTradeProjectionError::MissingOrder("missing".into())
        );

        index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                Some("order-1"),
                TradeListingMessagePayload::OrderRequest(base_order()),
            ))
            .expect("order request");

        assert_eq!(
            index
                .order_mut_checked("order-1", "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAw",)
                .expect_err("listing mismatch"),
            RadrootsTradeProjectionError::ListingAddrMismatch
        );
        assert_eq!(
            super::ensure_actor("seller-pubkey", "buyer-pubkey"),
            Err(RadrootsTradeProjectionError::UnauthorizedActor)
        );
        assert_eq!(
            super::ensure_counterparty("seller-pubkey", "buyer-pubkey"),
            Err(RadrootsTradeProjectionError::CounterpartyMismatch)
        );
        assert!(radroots_trade_order_status_can_transition(
            &TradeOrderStatus::Requested,
            &TradeOrderStatus::Requested
        ));
        assert_eq!(
            super::radroots_trade_order_status_ensure_transition(
                TradeOrderStatus::Accepted,
                TradeOrderStatus::Requested,
            ),
            Err(RadrootsTradeProjectionError::InvalidTransition {
                from: TradeOrderStatus::Accepted,
                to: TradeOrderStatus::Requested,
            })
        );

        let existing_order = index.order("order-1").expect("existing order").clone();
        let mut bad_root = message(
            "seller-pubkey",
            "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
            Some("order-1"),
            TradeListingMessagePayload::OrderResponse(TradeOrderResponse {
                accepted: true,
                reason: None,
            }),
        );
        bad_root.root_event_id = Some("wrong-root".into());
        assert_eq!(
            super::ensure_trade_chain(&existing_order, &bad_root),
            Err(RadrootsTradeProjectionError::TradeThreadRootMismatch)
        );
        let mut bad_prev = bad_root.clone();
        bad_prev.root_event_id = Some(existing_order.root_event_id.clone());
        bad_prev.prev_event_id = Some("wrong-prev".into());
        assert_eq!(
            super::ensure_trade_chain(&existing_order, &bad_prev),
            Err(RadrootsTradeProjectionError::TradeThreadPrevMismatch)
        );

        let mut items = vec![TradeOrderItem {
            bin_id: "bin-1".into(),
            bin_count: 1,
        }];
        assert_eq!(
            super::apply_order_change(
                &mut items,
                &TradeOrderChange::BinCount {
                    item_index: 7,
                    bin_count: 2,
                },
            ),
            Err(RadrootsTradeProjectionError::InvalidItemIndex(7))
        );
        assert_eq!(
            super::apply_order_change(&mut items, &TradeOrderChange::ItemRemove { item_index: 7 },),
            Err(RadrootsTradeProjectionError::InvalidItemIndex(7))
        );

        let mut decline_index = RadrootsTradeReadIndex::new();
        decline_index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                Some("order-1"),
                TradeListingMessagePayload::OrderRequest(base_order()),
            ))
            .expect("decline order request");
        let declined = decline_index
            .apply_workflow_message(&message(
                "seller-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                Some("order-1"),
                TradeListingMessagePayload::OrderResponse(TradeOrderResponse {
                    accepted: false,
                    reason: Some("declined".into()),
                }),
            ))
            .expect("declined order");
        assert_eq!(declined.order_id, "order-1");
        assert_eq!(
            decline_index
                .order("order-1")
                .expect("declined order")
                .status,
            TradeOrderStatus::Declined
        );

        let mut invalid_accept_index = RadrootsTradeReadIndex::new();
        invalid_accept_index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                Some("order-2"),
                TradeListingMessagePayload::OrderRequest(TradeOrder {
                    order_id: "order-2".into(),
                    ..base_order()
                }),
            ))
            .expect("second order");
        assert_eq!(
            invalid_accept_index
                .apply_workflow_message(&message(
                    "buyer-pubkey",
                    "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                    Some("order-2"),
                    TradeListingMessagePayload::OrderRevisionAccept(TradeOrderRevisionResponse {
                        accepted: false,
                        reason: None,
                    }),
                ))
                .expect_err("invalid revision accept"),
            RadrootsTradeProjectionError::InvalidRevisionResponse
        );

        let mut invalid_decline_index = RadrootsTradeReadIndex::new();
        invalid_decline_index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                Some("order-2"),
                TradeListingMessagePayload::OrderRequest(TradeOrder {
                    order_id: "order-2".into(),
                    ..base_order()
                }),
            ))
            .expect("third order");
        assert_eq!(
            invalid_decline_index
                .apply_workflow_message(&message(
                    "buyer-pubkey",
                    "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                    Some("order-2"),
                    TradeListingMessagePayload::OrderRevisionDecline(TradeOrderRevisionResponse {
                        accepted: true,
                        reason: None,
                    }),
                ))
                .expect_err("invalid revision decline"),
            RadrootsTradeProjectionError::InvalidRevisionResponse
        );

        let mut invalid_discount_accept_index = RadrootsTradeReadIndex::new();
        invalid_discount_accept_index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                Some("order-2"),
                TradeListingMessagePayload::OrderRequest(TradeOrder {
                    order_id: "order-2".into(),
                    ..base_order()
                }),
            ))
            .expect("fourth order");
        assert_eq!(
            invalid_discount_accept_index
                .apply_workflow_message(&message(
                    "buyer-pubkey",
                    "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                    Some("order-2"),
                    TradeListingMessagePayload::DiscountAccept(TradeDiscountDecision::Decline {
                        reason: Some("wrong-shape".into()),
                    }),
                ))
                .expect_err("invalid discount accept"),
            RadrootsTradeProjectionError::InvalidDiscountDecision
        );

        let mut invalid_discount_decline_index = RadrootsTradeReadIndex::new();
        invalid_discount_decline_index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                Some("order-2"),
                TradeListingMessagePayload::OrderRequest(TradeOrder {
                    order_id: "order-2".into(),
                    ..base_order()
                }),
            ))
            .expect("fifth order");
        assert_eq!(
            invalid_discount_decline_index
                .apply_workflow_message(&message(
                    "buyer-pubkey",
                    "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                    Some("order-2"),
                    TradeListingMessagePayload::DiscountDecline(TradeDiscountDecision::Accept {
                        value: radroots_core::RadrootsCoreDiscountValue::Percent(
                            RadrootsCorePercent::new(RadrootsCoreDecimal::from(10u32)),
                        ),
                    }),
                ))
                .expect_err("invalid discount decline"),
            RadrootsTradeProjectionError::InvalidDiscountDecision
        );

        let mut mismatched_order = base_order();
        mismatched_order.order_id = "order-3".into();
        assert_eq!(
            index
                .apply_workflow_message(&message(
                    "buyer-pubkey",
                    "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                    Some("wrong-order-id"),
                    TradeListingMessagePayload::OrderRequest(mismatched_order.clone()),
                ))
                .expect_err("order id mismatch"),
            RadrootsTradeProjectionError::OrderIdMismatch
        );
        mismatched_order.listing_addr = "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAw".into();
        assert_eq!(
            index
                .apply_workflow_message(&message(
                    "buyer-pubkey",
                    "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                    Some("order-3"),
                    TradeListingMessagePayload::OrderRequest(mismatched_order),
                ))
                .expect_err("listing addr mismatch"),
            RadrootsTradeProjectionError::ListingAddrMismatch
        );

        let mut duplicate_order = TradeOrder {
            order_id: "order-1".into(),
            ..base_order()
        };
        duplicate_order.buyer_pubkey = "buyer-pubkey-2".into();
        assert_eq!(
            index
                .apply_workflow_message(&message(
                    "buyer-pubkey-2",
                    "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                    Some("order-1"),
                    TradeListingMessagePayload::OrderRequest(duplicate_order),
                ))
                .expect_err("duplicate order identity mismatch"),
            RadrootsTradeProjectionError::ListingAddrMismatch
        );

        let duplicate_listing_mismatch_order = TradeOrder {
            order_id: "order-1".into(),
            listing_addr: "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAw".into(),
            ..base_order()
        };
        let duplicate_listing_mismatch_message = RadrootsTradeOrderWorkflowMessage {
            event_id: "order-1:duplicate-listing".into(),
            actor_pubkey: "buyer-pubkey".into(),
            counterparty_pubkey: "seller-pubkey".into(),
            listing_addr: duplicate_listing_mismatch_order.listing_addr.clone(),
            order_id: Some("order-1".into()),
            listing_event: Some(listing_snapshot(
                &duplicate_listing_mismatch_order.listing_addr,
            )),
            root_event_id: None,
            prev_event_id: None,
            payload: TradeListingMessagePayload::OrderRequest(duplicate_listing_mismatch_order),
        };
        assert_eq!(
            index
                .apply_workflow_message(&duplicate_listing_mismatch_message)
                .expect_err("duplicate listing mismatch"),
            RadrootsTradeProjectionError::ListingAddrMismatch
        );

        let duplicate_same = index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                Some("order-1"),
                TradeListingMessagePayload::OrderRequest(base_order()),
            ))
            .expect("duplicate same order");
        assert_eq!(duplicate_same.order_id, "order-1");

        let duplicate_seller_mismatch_message = RadrootsTradeOrderWorkflowMessage {
            event_id: "order-1:duplicate-seller".into(),
            actor_pubkey: "buyer-pubkey".into(),
            counterparty_pubkey: "other-seller".into(),
            listing_addr: "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg".into(),
            order_id: Some("order-1".into()),
            listing_event: Some(listing_snapshot(
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
            )),
            root_event_id: None,
            prev_event_id: None,
            payload: TradeListingMessagePayload::OrderRequest(TradeOrder {
                order_id: "order-1".into(),
                seller_pubkey: "other-seller".into(),
                ..base_order()
            }),
        };
        assert_eq!(
            index
                .apply_workflow_message(&duplicate_seller_mismatch_message)
                .expect_err("duplicate seller mismatch"),
            RadrootsTradeProjectionError::ListingAddrMismatch
        );

        let mut cancel_index = RadrootsTradeReadIndex::new();
        cancel_index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                Some("order-4"),
                TradeListingMessagePayload::OrderRequest(TradeOrder {
                    order_id: "order-4".into(),
                    ..base_order()
                }),
            ))
            .expect("cancel order request");
        assert_eq!(
            cancel_index
                .apply_workflow_message(&message(
                    "intruder",
                    "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                    Some("order-4"),
                    TradeListingMessagePayload::Cancel(TradeListingCancel {
                        reason: Some("bad-actor".into()),
                    }),
                ))
                .expect_err("unauthorized cancel"),
            RadrootsTradeProjectionError::UnauthorizedActor
        );

        let mut buyer_cancel_index = RadrootsTradeReadIndex::new();
        buyer_cancel_index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                Some("order-4"),
                TradeListingMessagePayload::OrderRequest(TradeOrder {
                    order_id: "order-4".into(),
                    ..base_order()
                }),
            ))
            .expect("buyer cancel order request");
        buyer_cancel_index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                Some("order-4"),
                TradeListingMessagePayload::Cancel(TradeListingCancel {
                    reason: Some("buyer-cancel".into()),
                }),
            ))
            .expect("buyer cancel");
        assert_eq!(
            buyer_cancel_index
                .order("order-4")
                .expect("cancelled order")
                .status,
            TradeOrderStatus::Cancelled
        );
    }

    #[test]
    fn workflow_projection_rejects_order_request_identity_mismatches() {
        let listing_addr = "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg";
        let mut unauthorized_index = RadrootsTradeReadIndex::new();
        assert_eq!(
            unauthorized_index
                .apply_workflow_message(&message(
                    "intruder",
                    listing_addr,
                    Some("order-request-actor"),
                    TradeListingMessagePayload::OrderRequest(TradeOrder {
                        order_id: "order-request-actor".into(),
                        ..base_order()
                    }),
                ))
                .expect_err("unauthorized order request"),
            RadrootsTradeProjectionError::UnauthorizedActor
        );

        let mut wrong_counterparty = message(
            "buyer-pubkey",
            listing_addr,
            Some("order-request-counterparty"),
            TradeListingMessagePayload::OrderRequest(TradeOrder {
                order_id: "order-request-counterparty".into(),
                ..base_order()
            }),
        );
        wrong_counterparty.counterparty_pubkey = "wrong-seller".into();
        let mut counterparty_index = RadrootsTradeReadIndex::new();
        assert_eq!(
            counterparty_index
                .apply_workflow_message(&wrong_counterparty)
                .expect_err("counterparty mismatch"),
            RadrootsTradeProjectionError::CounterpartyMismatch
        );
    }

    #[test]
    fn workflow_action_helpers_cover_remaining_error_paths() {
        let listing_addr = "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg";
        let mut index = RadrootsTradeReadIndex::new();
        index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                listing_addr,
                Some("order-helper"),
                TradeListingMessagePayload::OrderRequest(TradeOrder {
                    order_id: "order-helper".into(),
                    ..base_order()
                }),
            ))
            .expect("seed order");

        let missing_buyer_order = message(
            "buyer-pubkey",
            listing_addr,
            Some("missing-order"),
            TradeListingMessagePayload::OrderRevisionAccept(TradeOrderRevisionResponse {
                accepted: true,
                reason: None,
            }),
        );
        assert_eq!(
            index
                .order_mut_for_buyer_action(&missing_buyer_order)
                .expect_err("missing buyer order"),
            RadrootsTradeProjectionError::MissingOrder("missing-order".into())
        );

        let wrong_buyer_actor = message(
            "intruder",
            listing_addr,
            Some("order-helper"),
            TradeListingMessagePayload::OrderRevisionAccept(TradeOrderRevisionResponse {
                accepted: true,
                reason: None,
            }),
        );
        assert_eq!(
            index
                .order_mut_for_buyer_action(&wrong_buyer_actor)
                .expect_err("wrong buyer actor"),
            RadrootsTradeProjectionError::UnauthorizedActor
        );

        let mut wrong_buyer_counterparty = message(
            "buyer-pubkey",
            listing_addr,
            Some("order-helper"),
            TradeListingMessagePayload::OrderRevisionAccept(TradeOrderRevisionResponse {
                accepted: true,
                reason: None,
            }),
        );
        wrong_buyer_counterparty.counterparty_pubkey = "wrong-seller".into();
        assert_eq!(
            index
                .order_mut_for_buyer_action(&wrong_buyer_counterparty)
                .expect_err("wrong buyer counterparty"),
            RadrootsTradeProjectionError::CounterpartyMismatch
        );

        let mut missing_buyer_root = message(
            "buyer-pubkey",
            listing_addr,
            Some("order-helper"),
            TradeListingMessagePayload::OrderRevisionAccept(TradeOrderRevisionResponse {
                accepted: true,
                reason: None,
            }),
        );
        missing_buyer_root.root_event_id = None;
        assert_eq!(
            index
                .order_mut_for_buyer_action(&missing_buyer_root)
                .expect_err("missing buyer root"),
            RadrootsTradeProjectionError::MissingTradeRootEventId
        );

        let mut missing_buyer_prev = message(
            "buyer-pubkey",
            listing_addr,
            Some("order-helper"),
            TradeListingMessagePayload::OrderRevisionAccept(TradeOrderRevisionResponse {
                accepted: true,
                reason: None,
            }),
        );
        missing_buyer_prev.prev_event_id = None;
        assert_eq!(
            index
                .order_mut_for_buyer_action(&missing_buyer_prev)
                .expect_err("missing buyer prev"),
            RadrootsTradeProjectionError::MissingTradePrevEventId
        );

        let missing_seller_order = message(
            "seller-pubkey",
            listing_addr,
            Some("missing-order"),
            TradeListingMessagePayload::OrderResponse(TradeOrderResponse {
                accepted: true,
                reason: None,
            }),
        );
        assert_eq!(
            index
                .order_mut_for_seller_action(&missing_seller_order)
                .expect_err("missing seller order"),
            RadrootsTradeProjectionError::MissingOrder("missing-order".into())
        );

        let missing_seller_order_id = RadrootsTradeOrderWorkflowMessage {
            order_id: None,
            ..message(
                "seller-pubkey",
                listing_addr,
                Some("order-helper"),
                TradeListingMessagePayload::OrderResponse(TradeOrderResponse {
                    accepted: true,
                    reason: None,
                }),
            )
        };
        assert_eq!(
            index
                .order_mut_for_seller_action(&missing_seller_order_id)
                .expect_err("missing seller order id"),
            RadrootsTradeProjectionError::MissingOrderId
        );

        let wrong_seller_actor = message(
            "intruder",
            listing_addr,
            Some("order-helper"),
            TradeListingMessagePayload::OrderResponse(TradeOrderResponse {
                accepted: true,
                reason: None,
            }),
        );
        assert_eq!(
            index
                .order_mut_for_seller_action(&wrong_seller_actor)
                .expect_err("wrong seller actor"),
            RadrootsTradeProjectionError::UnauthorizedActor
        );

        let mut wrong_seller_counterparty = message(
            "seller-pubkey",
            listing_addr,
            Some("order-helper"),
            TradeListingMessagePayload::OrderResponse(TradeOrderResponse {
                accepted: true,
                reason: None,
            }),
        );
        wrong_seller_counterparty.counterparty_pubkey = "wrong-buyer".into();
        assert_eq!(
            index
                .order_mut_for_seller_action(&wrong_seller_counterparty)
                .expect_err("wrong seller counterparty"),
            RadrootsTradeProjectionError::CounterpartyMismatch
        );

        let mut missing_seller_root = message(
            "seller-pubkey",
            listing_addr,
            Some("order-helper"),
            TradeListingMessagePayload::OrderResponse(TradeOrderResponse {
                accepted: true,
                reason: None,
            }),
        );
        missing_seller_root.root_event_id = None;
        assert_eq!(
            index
                .order_mut_for_seller_action(&missing_seller_root)
                .expect_err("missing seller root"),
            RadrootsTradeProjectionError::MissingTradeRootEventId
        );

        let mut missing_seller_prev = message(
            "seller-pubkey",
            listing_addr,
            Some("order-helper"),
            TradeListingMessagePayload::OrderResponse(TradeOrderResponse {
                accepted: true,
                reason: None,
            }),
        );
        missing_seller_prev.prev_event_id = None;
        assert_eq!(
            index
                .order_mut_for_seller_action(&missing_seller_prev)
                .expect_err("missing seller prev"),
            RadrootsTradeProjectionError::MissingTradePrevEventId
        );

        let missing_participant_order = message(
            "buyer-pubkey",
            listing_addr,
            Some("missing-order"),
            TradeListingMessagePayload::Cancel(TradeListingCancel { reason: None }),
        );
        assert_eq!(
            index
                .order_mut_for_participant_action(&missing_participant_order)
                .expect_err("missing participant order"),
            RadrootsTradeProjectionError::MissingOrder("missing-order".into())
        );

        let missing_participant_order_id = RadrootsTradeOrderWorkflowMessage {
            order_id: None,
            ..message(
                "buyer-pubkey",
                listing_addr,
                Some("order-helper"),
                TradeListingMessagePayload::Cancel(TradeListingCancel { reason: None }),
            )
        };
        assert_eq!(
            index
                .order_mut_for_participant_action(&missing_participant_order_id)
                .expect_err("missing participant order id"),
            RadrootsTradeProjectionError::MissingOrderId
        );

        let mut wrong_participant_counterparty = message(
            "buyer-pubkey",
            listing_addr,
            Some("order-helper"),
            TradeListingMessagePayload::Cancel(TradeListingCancel { reason: None }),
        );
        wrong_participant_counterparty.counterparty_pubkey = "wrong-seller".into();
        assert_eq!(
            index
                .order_mut_for_participant_action(&wrong_participant_counterparty)
                .expect_err("wrong participant counterparty"),
            RadrootsTradeProjectionError::CounterpartyMismatch
        );

        let mut missing_participant_root = message(
            "buyer-pubkey",
            listing_addr,
            Some("order-helper"),
            TradeListingMessagePayload::Cancel(TradeListingCancel { reason: None }),
        );
        missing_participant_root.root_event_id = None;
        assert_eq!(
            index
                .order_mut_for_participant_action(&missing_participant_root)
                .expect_err("missing participant root"),
            RadrootsTradeProjectionError::MissingTradeRootEventId
        );

        let mut missing_participant_prev = message(
            "buyer-pubkey",
            listing_addr,
            Some("order-helper"),
            TradeListingMessagePayload::Cancel(TradeListingCancel { reason: None }),
        );
        missing_participant_prev.prev_event_id = None;
        assert_eq!(
            index
                .order_mut_for_participant_action(&missing_participant_prev)
                .expect_err("missing participant prev"),
            RadrootsTradeProjectionError::MissingTradePrevEventId
        );

        let existing_order = index.order("order-helper").expect("helper order").clone();
        let mut missing_root = message(
            "seller-pubkey",
            listing_addr,
            Some("order-helper"),
            TradeListingMessagePayload::OrderResponse(TradeOrderResponse {
                accepted: true,
                reason: None,
            }),
        );
        missing_root.root_event_id = None;
        assert_eq!(
            super::ensure_trade_chain(&existing_order, &missing_root),
            Err(RadrootsTradeProjectionError::MissingTradeRootEventId)
        );

        let mut missing_prev = message(
            "seller-pubkey",
            listing_addr,
            Some("order-helper"),
            TradeListingMessagePayload::OrderResponse(TradeOrderResponse {
                accepted: true,
                reason: None,
            }),
        );
        missing_prev.prev_event_id = None;
        assert_eq!(
            super::ensure_trade_chain(&existing_order, &missing_prev),
            Err(RadrootsTradeProjectionError::MissingTradePrevEventId)
        );
    }

    #[test]
    fn workflow_helpers_cover_transition_and_terminal_tables() {
        assert!(radroots_trade_order_status_can_transition(
            &TradeOrderStatus::Draft,
            &TradeOrderStatus::Requested
        ));
        assert!(!radroots_trade_order_status_can_transition(
            &TradeOrderStatus::Draft,
            &TradeOrderStatus::Accepted
        ));
        assert!(radroots_trade_order_status_can_transition(
            &TradeOrderStatus::Validated,
            &TradeOrderStatus::Requested
        ));
        assert!(!radroots_trade_order_status_can_transition(
            &TradeOrderStatus::Validated,
            &TradeOrderStatus::Accepted
        ));
        assert!(radroots_trade_order_status_can_transition(
            &TradeOrderStatus::Requested,
            &TradeOrderStatus::Accepted
        ));
        assert!(radroots_trade_order_status_can_transition(
            &TradeOrderStatus::Requested,
            &TradeOrderStatus::Declined
        ));
        assert!(radroots_trade_order_status_can_transition(
            &TradeOrderStatus::Requested,
            &TradeOrderStatus::Questioned
        ));
        assert!(radroots_trade_order_status_can_transition(
            &TradeOrderStatus::Requested,
            &TradeOrderStatus::Revised
        ));
        assert!(radroots_trade_order_status_can_transition(
            &TradeOrderStatus::Requested,
            &TradeOrderStatus::Cancelled
        ));
        assert!(radroots_trade_order_status_can_transition(
            &TradeOrderStatus::Questioned,
            &TradeOrderStatus::Requested
        ));
        assert!(radroots_trade_order_status_can_transition(
            &TradeOrderStatus::Questioned,
            &TradeOrderStatus::Revised
        ));
        assert!(radroots_trade_order_status_can_transition(
            &TradeOrderStatus::Questioned,
            &TradeOrderStatus::Cancelled
        ));
        assert!(radroots_trade_order_status_can_transition(
            &TradeOrderStatus::Revised,
            &TradeOrderStatus::Accepted
        ));
        assert!(radroots_trade_order_status_can_transition(
            &TradeOrderStatus::Revised,
            &TradeOrderStatus::Declined
        ));
        assert!(radroots_trade_order_status_can_transition(
            &TradeOrderStatus::Revised,
            &TradeOrderStatus::Cancelled
        ));
        assert!(radroots_trade_order_status_can_transition(
            &TradeOrderStatus::Revised,
            &TradeOrderStatus::Requested
        ));
        assert!(radroots_trade_order_status_can_transition(
            &TradeOrderStatus::Accepted,
            &TradeOrderStatus::Fulfilled
        ));
        assert!(radroots_trade_order_status_can_transition(
            &TradeOrderStatus::Accepted,
            &TradeOrderStatus::Cancelled
        ));
        assert!(radroots_trade_order_status_can_transition(
            &TradeOrderStatus::Fulfilled,
            &TradeOrderStatus::Completed
        ));
        assert!(radroots_trade_order_status_can_transition(
            &TradeOrderStatus::Fulfilled,
            &TradeOrderStatus::Fulfilled
        ));
        assert!(radroots_trade_order_status_can_transition(
            &TradeOrderStatus::Fulfilled,
            &TradeOrderStatus::Cancelled
        ));
        assert!(!radroots_trade_order_status_can_transition(
            &TradeOrderStatus::Accepted,
            &TradeOrderStatus::Requested
        ));
        assert!(!radroots_trade_order_status_can_transition(
            &TradeOrderStatus::Requested,
            &TradeOrderStatus::Fulfilled
        ));
        assert!(!radroots_trade_order_status_can_transition(
            &TradeOrderStatus::Questioned,
            &TradeOrderStatus::Accepted
        ));
        assert!(!radroots_trade_order_status_can_transition(
            &TradeOrderStatus::Revised,
            &TradeOrderStatus::Completed
        ));
        assert!(!radroots_trade_order_status_can_transition(
            &TradeOrderStatus::Declined,
            &TradeOrderStatus::Accepted
        ));
        assert!(!radroots_trade_order_status_can_transition(
            &TradeOrderStatus::Cancelled,
            &TradeOrderStatus::Accepted
        ));
        assert!(!radroots_trade_order_status_can_transition(
            &TradeOrderStatus::Completed,
            &TradeOrderStatus::Accepted
        ));
        assert!(!radroots_trade_order_status_can_transition(
            &TradeOrderStatus::Fulfilled,
            &TradeOrderStatus::Accepted
        ));
        assert!(radroots_trade_order_status_is_terminal(
            &TradeOrderStatus::Completed
        ));
        assert!(radroots_trade_order_status_is_terminal(
            &TradeOrderStatus::Declined
        ));
        assert!(radroots_trade_order_status_is_terminal(
            &TradeOrderStatus::Cancelled
        ));
        assert!(!radroots_trade_order_status_is_terminal(
            &TradeOrderStatus::Fulfilled
        ));

        assert_eq!(
            super::trade_order_status_for_fulfillment_update(
                &TradeOrderStatus::Accepted,
                &TradeFulfillmentStatus::Preparing,
            ),
            Ok(None)
        );
        assert_eq!(
            super::trade_order_status_for_fulfillment_update(
                &TradeOrderStatus::Accepted,
                &TradeFulfillmentStatus::Shipped,
            ),
            Ok(None)
        );
        assert_eq!(
            super::trade_order_status_for_fulfillment_update(
                &TradeOrderStatus::Accepted,
                &TradeFulfillmentStatus::ReadyForPickup,
            ),
            Ok(None)
        );
        assert_eq!(
            super::trade_order_status_for_fulfillment_update(
                &TradeOrderStatus::Requested,
                &TradeFulfillmentStatus::Preparing,
            ),
            Err(RadrootsTradeProjectionError::InvalidTransition {
                from: TradeOrderStatus::Requested,
                to: TradeOrderStatus::Accepted,
            })
        );
        assert_eq!(
            super::trade_order_status_for_fulfillment_update(
                &TradeOrderStatus::Accepted,
                &TradeFulfillmentStatus::Delivered,
            ),
            Ok(Some(TradeOrderStatus::Fulfilled))
        );
        assert_eq!(
            super::trade_order_status_for_fulfillment_update(
                &TradeOrderStatus::Requested,
                &TradeFulfillmentStatus::Delivered,
            ),
            Err(RadrootsTradeProjectionError::InvalidTransition {
                from: TradeOrderStatus::Requested,
                to: TradeOrderStatus::Fulfilled,
            })
        );
        assert_eq!(
            super::trade_order_status_for_fulfillment_update(
                &TradeOrderStatus::Accepted,
                &TradeFulfillmentStatus::Cancelled,
            ),
            Ok(Some(TradeOrderStatus::Cancelled))
        );
        assert_eq!(
            super::trade_order_status_for_fulfillment_update(
                &TradeOrderStatus::Completed,
                &TradeFulfillmentStatus::Cancelled,
            ),
            Err(RadrootsTradeProjectionError::InvalidTransition {
                from: TradeOrderStatus::Completed,
                to: TradeOrderStatus::Cancelled,
            })
        );

        assert_eq!(
            super::trade_order_status_for_receipt(&TradeOrderStatus::Fulfilled, true),
            Ok(Some(TradeOrderStatus::Completed))
        );
        assert_eq!(
            super::trade_order_status_for_receipt(&TradeOrderStatus::Accepted, true),
            Err(RadrootsTradeProjectionError::InvalidTransition {
                from: TradeOrderStatus::Accepted,
                to: TradeOrderStatus::Completed,
            })
        );
        assert_eq!(
            super::trade_order_status_for_receipt(&TradeOrderStatus::Fulfilled, false),
            Ok(None)
        );
        assert_eq!(
            super::trade_order_status_for_receipt(&TradeOrderStatus::Accepted, false),
            Err(RadrootsTradeProjectionError::InvalidTransition {
                from: TradeOrderStatus::Accepted,
                to: TradeOrderStatus::Fulfilled,
            })
        );

        let facet_expectations = [
            (RadrootsTradeListingMarketStatus::Unknown, "unknown"),
            (RadrootsTradeListingMarketStatus::Window, "window"),
            (RadrootsTradeListingMarketStatus::Active, "active"),
            (RadrootsTradeListingMarketStatus::Sold, "sold"),
            (
                RadrootsTradeListingMarketStatus::Other {
                    value: "archived".into(),
                },
                "archived",
            ),
        ];
        for (status, expected) in facet_expectations {
            assert_eq!(status.facet_key(), expected);
        }

        let order_status_expectations = [
            (TradeOrderStatus::Draft, "draft"),
            (TradeOrderStatus::Validated, "validated"),
            (TradeOrderStatus::Requested, "requested"),
            (TradeOrderStatus::Questioned, "questioned"),
            (TradeOrderStatus::Revised, "revised"),
            (TradeOrderStatus::Accepted, "accepted"),
            (TradeOrderStatus::Declined, "declined"),
            (TradeOrderStatus::Cancelled, "cancelled"),
            (TradeOrderStatus::Fulfilled, "fulfilled"),
            (TradeOrderStatus::Completed, "completed"),
        ];
        for (status, expected) in order_status_expectations {
            assert_eq!(super::order_status_key(&status), expected);
        }
    }

    #[test]
    fn workflow_projection_rejects_follow_up_messages_with_wrong_actor_or_missing_snapshot() {
        let listing_addr = "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg";

        let mut response_index = RadrootsTradeReadIndex::new();
        response_index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                listing_addr,
                Some("order-response-actor"),
                TradeListingMessagePayload::OrderRequest(TradeOrder {
                    order_id: "order-response-actor".into(),
                    ..base_order()
                }),
            ))
            .expect("seed response order");
        assert_eq!(
            response_index
                .apply_workflow_message(&message(
                    "intruder",
                    listing_addr,
                    Some("order-response-actor"),
                    TradeListingMessagePayload::OrderResponse(TradeOrderResponse {
                        accepted: true,
                        reason: None,
                    }),
                ))
                .expect_err("response wrong actor"),
            RadrootsTradeProjectionError::UnauthorizedActor
        );

        let mut revision_index = RadrootsTradeReadIndex::new();
        revision_index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                listing_addr,
                Some("order-revision-actor"),
                TradeListingMessagePayload::OrderRequest(TradeOrder {
                    order_id: "order-revision-actor".into(),
                    ..base_order()
                }),
            ))
            .expect("seed revision order");
        assert_eq!(
            revision_index
                .apply_workflow_message(&message(
                    "intruder",
                    listing_addr,
                    Some("order-revision-actor"),
                    TradeListingMessagePayload::OrderRevision(TradeOrderRevision {
                        revision_id: "rev-invalid-actor".into(),
                        changes: vec![TradeOrderChange::BinCount {
                            item_index: 0,
                            bin_count: 3,
                        }],
                    }),
                ))
                .expect_err("revision wrong actor"),
            RadrootsTradeProjectionError::UnauthorizedActor
        );
        let seeded_revision_order = revision_index
            .order("order-revision-actor")
            .expect("seeded revision order")
            .clone();
        let empty_revision = RadrootsTradeOrderWorkflowMessage {
            event_id: "order-revision-actor:empty-revision".into(),
            actor_pubkey: "seller-pubkey".into(),
            counterparty_pubkey: "buyer-pubkey".into(),
            listing_addr: listing_addr.into(),
            order_id: Some("order-revision-actor".into()),
            listing_event: Some(listing_snapshot(listing_addr)),
            root_event_id: Some(seeded_revision_order.root_event_id.clone()),
            prev_event_id: Some(seeded_revision_order.last_event_id.clone()),
            payload: TradeListingMessagePayload::OrderRevision(TradeOrderRevision {
                revision_id: "rev-empty".into(),
                changes: vec![],
            }),
        };
        revision_index
            .apply_workflow_message(&empty_revision)
            .expect("empty revision");
        let revised_order = revision_index
            .order("order-revision-actor")
            .expect("revised order")
            .clone();
        let mut missing_revision_snapshot = empty_revision.clone();
        missing_revision_snapshot.event_id = "order-revision-actor:missing-snapshot".into();
        missing_revision_snapshot.listing_event = None;
        missing_revision_snapshot.prev_event_id = Some(revised_order.last_event_id.clone());
        assert_eq!(
            revision_index
                .apply_workflow_message(&missing_revision_snapshot)
                .expect_err("revision missing snapshot"),
            RadrootsTradeProjectionError::MissingListingSnapshot
        );

        let mut revision_accept_index = RadrootsTradeReadIndex::new();
        revision_accept_index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                listing_addr,
                Some("order-revision-accept-actor"),
                TradeListingMessagePayload::OrderRequest(TradeOrder {
                    order_id: "order-revision-accept-actor".into(),
                    ..base_order()
                }),
            ))
            .expect("seed revision accept order");
        assert_eq!(
            revision_accept_index
                .apply_workflow_message(&message(
                    "intruder",
                    listing_addr,
                    Some("order-revision-accept-actor"),
                    TradeListingMessagePayload::OrderRevisionAccept(TradeOrderRevisionResponse {
                        accepted: true,
                        reason: None,
                    },),
                ))
                .expect_err("revision accept wrong actor"),
            RadrootsTradeProjectionError::UnauthorizedActor
        );

        let mut revision_decline_index = RadrootsTradeReadIndex::new();
        revision_decline_index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                listing_addr,
                Some("order-revision-decline-actor"),
                TradeListingMessagePayload::OrderRequest(TradeOrder {
                    order_id: "order-revision-decline-actor".into(),
                    ..base_order()
                }),
            ))
            .expect("seed revision decline order");
        assert_eq!(
            revision_decline_index
                .apply_workflow_message(&message(
                    "intruder",
                    listing_addr,
                    Some("order-revision-decline-actor"),
                    TradeListingMessagePayload::OrderRevisionDecline(TradeOrderRevisionResponse {
                        accepted: false,
                        reason: None,
                    },),
                ))
                .expect_err("revision decline wrong actor"),
            RadrootsTradeProjectionError::UnauthorizedActor
        );

        let mut answer_index = RadrootsTradeReadIndex::new();
        answer_index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                listing_addr,
                Some("order-answer-actor"),
                TradeListingMessagePayload::OrderRequest(TradeOrder {
                    order_id: "order-answer-actor".into(),
                    ..base_order()
                }),
            ))
            .expect("seed answer order");
        answer_index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                listing_addr,
                Some("order-answer-actor"),
                TradeListingMessagePayload::Question(TradeQuestion {
                    question_id: "question-1".into(),
                }),
            ))
            .expect("seed answer question");
        assert_eq!(
            answer_index
                .apply_workflow_message(&message(
                    "intruder",
                    listing_addr,
                    Some("order-answer-actor"),
                    TradeListingMessagePayload::Answer(TradeAnswer {
                        question_id: "question-1".into(),
                    }),
                ))
                .expect_err("answer wrong actor"),
            RadrootsTradeProjectionError::UnauthorizedActor
        );

        let mut discount_request_index = RadrootsTradeReadIndex::new();
        discount_request_index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                listing_addr,
                Some("order-discount-request-actor"),
                TradeListingMessagePayload::OrderRequest(TradeOrder {
                    order_id: "order-discount-request-actor".into(),
                    ..base_order()
                }),
            ))
            .expect("seed discount request order");
        assert_eq!(
            discount_request_index
                .apply_workflow_message(&message(
                    "intruder",
                    listing_addr,
                    Some("order-discount-request-actor"),
                    TradeListingMessagePayload::DiscountRequest(TradeDiscountRequest {
                        discount_id: "discount-request-invalid-actor".into(),
                        value: radroots_core::RadrootsCoreDiscountValue::Percent(
                            RadrootsCorePercent::new(RadrootsCoreDecimal::from(5u32)),
                        ),
                    }),
                ))
                .expect_err("discount request wrong actor"),
            RadrootsTradeProjectionError::UnauthorizedActor
        );
        let discount_request_order = discount_request_index
            .order("order-discount-request-actor")
            .expect("discount request order")
            .clone();
        let missing_discount_request_snapshot = RadrootsTradeOrderWorkflowMessage {
            event_id: "order-discount-request-actor:missing-snapshot".into(),
            actor_pubkey: "buyer-pubkey".into(),
            counterparty_pubkey: "seller-pubkey".into(),
            listing_addr: listing_addr.into(),
            order_id: Some("order-discount-request-actor".into()),
            listing_event: None,
            root_event_id: Some(discount_request_order.root_event_id.clone()),
            prev_event_id: Some(discount_request_order.last_event_id.clone()),
            payload: TradeListingMessagePayload::DiscountRequest(TradeDiscountRequest {
                discount_id: "discount-request-missing-snapshot".into(),
                value: radroots_core::RadrootsCoreDiscountValue::Percent(RadrootsCorePercent::new(
                    RadrootsCoreDecimal::from(5u32),
                )),
            }),
        };
        assert_eq!(
            discount_request_index
                .apply_workflow_message(&missing_discount_request_snapshot)
                .expect_err("discount request missing snapshot"),
            RadrootsTradeProjectionError::MissingListingSnapshot
        );

        let mut discount_offer_index = RadrootsTradeReadIndex::new();
        discount_offer_index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                listing_addr,
                Some("order-discount-offer-actor"),
                TradeListingMessagePayload::OrderRequest(TradeOrder {
                    order_id: "order-discount-offer-actor".into(),
                    ..base_order()
                }),
            ))
            .expect("seed discount offer order");
        assert_eq!(
            discount_offer_index
                .apply_workflow_message(&message(
                    "intruder",
                    listing_addr,
                    Some("order-discount-offer-actor"),
                    TradeListingMessagePayload::DiscountOffer(TradeDiscountOffer {
                        discount_id: "discount-offer-invalid-actor".into(),
                        value: radroots_core::RadrootsCoreDiscountValue::Percent(
                            RadrootsCorePercent::new(RadrootsCoreDecimal::from(5u32)),
                        ),
                    }),
                ))
                .expect_err("discount offer wrong actor"),
            RadrootsTradeProjectionError::UnauthorizedActor
        );
        let discount_offer_order = discount_offer_index
            .order("order-discount-offer-actor")
            .expect("discount offer order")
            .clone();
        let missing_discount_offer_snapshot = RadrootsTradeOrderWorkflowMessage {
            event_id: "order-discount-offer-actor:missing-snapshot".into(),
            actor_pubkey: "seller-pubkey".into(),
            counterparty_pubkey: "buyer-pubkey".into(),
            listing_addr: listing_addr.into(),
            order_id: Some("order-discount-offer-actor".into()),
            listing_event: None,
            root_event_id: Some(discount_offer_order.root_event_id.clone()),
            prev_event_id: Some(discount_offer_order.last_event_id.clone()),
            payload: TradeListingMessagePayload::DiscountOffer(TradeDiscountOffer {
                discount_id: "discount-offer-missing-snapshot".into(),
                value: radroots_core::RadrootsCoreDiscountValue::Percent(RadrootsCorePercent::new(
                    RadrootsCoreDecimal::from(5u32),
                )),
            }),
        };
        assert_eq!(
            discount_offer_index
                .apply_workflow_message(&missing_discount_offer_snapshot)
                .expect_err("discount offer missing snapshot"),
            RadrootsTradeProjectionError::MissingListingSnapshot
        );

        let mut discount_accept_index = RadrootsTradeReadIndex::new();
        discount_accept_index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                listing_addr,
                Some("order-discount-accept-actor"),
                TradeListingMessagePayload::OrderRequest(TradeOrder {
                    order_id: "order-discount-accept-actor".into(),
                    ..base_order()
                }),
            ))
            .expect("seed discount accept order");
        assert_eq!(
            discount_accept_index
                .apply_workflow_message(&message(
                    "intruder",
                    listing_addr,
                    Some("order-discount-accept-actor"),
                    TradeListingMessagePayload::DiscountAccept(TradeDiscountDecision::Accept {
                        value: radroots_core::RadrootsCoreDiscountValue::Percent(
                            RadrootsCorePercent::new(RadrootsCoreDecimal::from(5u32)),
                        ),
                    }),
                ))
                .expect_err("discount accept wrong actor"),
            RadrootsTradeProjectionError::UnauthorizedActor
        );

        let mut discount_decline_index = RadrootsTradeReadIndex::new();
        discount_decline_index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                listing_addr,
                Some("order-discount-decline-actor"),
                TradeListingMessagePayload::OrderRequest(TradeOrder {
                    order_id: "order-discount-decline-actor".into(),
                    ..base_order()
                }),
            ))
            .expect("seed discount decline order");
        assert_eq!(
            discount_decline_index
                .apply_workflow_message(&message(
                    "intruder",
                    listing_addr,
                    Some("order-discount-decline-actor"),
                    TradeListingMessagePayload::DiscountDecline(TradeDiscountDecision::Decline {
                        reason: Some("no".into()),
                    },),
                ))
                .expect_err("discount decline wrong actor"),
            RadrootsTradeProjectionError::UnauthorizedActor
        );

        let mut fulfillment_index = RadrootsTradeReadIndex::new();
        fulfillment_index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                listing_addr,
                Some("order-fulfillment-actor"),
                TradeListingMessagePayload::OrderRequest(TradeOrder {
                    order_id: "order-fulfillment-actor".into(),
                    ..base_order()
                }),
            ))
            .expect("seed fulfillment order");
        assert_eq!(
            fulfillment_index
                .apply_workflow_message(&message(
                    "intruder",
                    listing_addr,
                    Some("order-fulfillment-actor"),
                    TradeListingMessagePayload::FulfillmentUpdate(TradeFulfillmentUpdate {
                        status: TradeFulfillmentStatus::Preparing,
                    }),
                ))
                .expect_err("fulfillment wrong actor"),
            RadrootsTradeProjectionError::UnauthorizedActor
        );

        let mut receipt_index = RadrootsTradeReadIndex::new();
        receipt_index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                listing_addr,
                Some("order-receipt-actor"),
                TradeListingMessagePayload::OrderRequest(TradeOrder {
                    order_id: "order-receipt-actor".into(),
                    ..base_order()
                }),
            ))
            .expect("seed receipt order");
        assert_eq!(
            receipt_index
                .apply_workflow_message(&message(
                    "intruder",
                    listing_addr,
                    Some("order-receipt-actor"),
                    TradeListingMessagePayload::Receipt(TradeReceipt {
                        acknowledged: false,
                        at: 1_700_000_123,
                    }),
                ))
                .expect_err("receipt wrong actor"),
            RadrootsTradeProjectionError::UnauthorizedActor
        );
    }

    #[test]
    fn workflow_projection_rejects_follow_up_messages_with_invalid_transitions() {
        let listing_addr = "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg";

        let mut response_index = RadrootsTradeReadIndex::new();
        response_index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                listing_addr,
                Some("order-response-transition"),
                TradeListingMessagePayload::OrderRequest(TradeOrder {
                    order_id: "order-response-transition".into(),
                    ..base_order()
                }),
            ))
            .expect("seed response transition");
        response_index
            .orders
            .get_mut("order-response-transition")
            .expect("response order")
            .status = TradeOrderStatus::Completed;
        assert_eq!(
            response_index
                .apply_workflow_message(&message(
                    "seller-pubkey",
                    listing_addr,
                    Some("order-response-transition"),
                    TradeListingMessagePayload::OrderResponse(TradeOrderResponse {
                        accepted: true,
                        reason: None,
                    }),
                ))
                .expect_err("response invalid transition"),
            RadrootsTradeProjectionError::InvalidTransition {
                from: TradeOrderStatus::Completed,
                to: TradeOrderStatus::Accepted,
            }
        );

        let mut revision_index = RadrootsTradeReadIndex::new();
        revision_index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                listing_addr,
                Some("order-revision-transition"),
                TradeListingMessagePayload::OrderRequest(TradeOrder {
                    order_id: "order-revision-transition".into(),
                    ..base_order()
                }),
            ))
            .expect("seed revision transition");
        revision_index
            .orders
            .get_mut("order-revision-transition")
            .expect("revision order")
            .status = TradeOrderStatus::Completed;
        assert_eq!(
            revision_index
                .apply_workflow_message(&message(
                    "seller-pubkey",
                    listing_addr,
                    Some("order-revision-transition"),
                    TradeListingMessagePayload::OrderRevision(TradeOrderRevision {
                        revision_id: "rev-invalid-transition".into(),
                        changes: vec![TradeOrderChange::BinCount {
                            item_index: 0,
                            bin_count: 3,
                        }],
                    }),
                ))
                .expect_err("revision invalid transition"),
            RadrootsTradeProjectionError::InvalidTransition {
                from: TradeOrderStatus::Completed,
                to: TradeOrderStatus::Revised,
            }
        );

        let mut revision_accept_index = RadrootsTradeReadIndex::new();
        revision_accept_index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                listing_addr,
                Some("order-revision-accept-transition"),
                TradeListingMessagePayload::OrderRequest(TradeOrder {
                    order_id: "order-revision-accept-transition".into(),
                    ..base_order()
                }),
            ))
            .expect("seed revision accept transition");
        revision_accept_index
            .orders
            .get_mut("order-revision-accept-transition")
            .expect("revision accept order")
            .status = TradeOrderStatus::Completed;
        assert_eq!(
            revision_accept_index
                .apply_workflow_message(&message(
                    "buyer-pubkey",
                    listing_addr,
                    Some("order-revision-accept-transition"),
                    TradeListingMessagePayload::OrderRevisionAccept(TradeOrderRevisionResponse {
                        accepted: true,
                        reason: None,
                    },),
                ))
                .expect_err("revision accept invalid transition"),
            RadrootsTradeProjectionError::InvalidTransition {
                from: TradeOrderStatus::Completed,
                to: TradeOrderStatus::Accepted,
            }
        );

        let mut revision_decline_index = RadrootsTradeReadIndex::new();
        revision_decline_index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                listing_addr,
                Some("order-revision-decline-transition"),
                TradeListingMessagePayload::OrderRequest(TradeOrder {
                    order_id: "order-revision-decline-transition".into(),
                    ..base_order()
                }),
            ))
            .expect("seed revision decline transition");
        revision_decline_index
            .orders
            .get_mut("order-revision-decline-transition")
            .expect("revision decline order")
            .status = TradeOrderStatus::Completed;
        assert_eq!(
            revision_decline_index
                .apply_workflow_message(&message(
                    "buyer-pubkey",
                    listing_addr,
                    Some("order-revision-decline-transition"),
                    TradeListingMessagePayload::OrderRevisionDecline(TradeOrderRevisionResponse {
                        accepted: false,
                        reason: None,
                    },),
                ))
                .expect_err("revision decline invalid transition"),
            RadrootsTradeProjectionError::InvalidTransition {
                from: TradeOrderStatus::Completed,
                to: TradeOrderStatus::Declined,
            }
        );

        let mut question_index = RadrootsTradeReadIndex::new();
        question_index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                listing_addr,
                Some("order-question-transition"),
                TradeListingMessagePayload::OrderRequest(TradeOrder {
                    order_id: "order-question-transition".into(),
                    ..base_order()
                }),
            ))
            .expect("seed question transition");
        question_index
            .orders
            .get_mut("order-question-transition")
            .expect("question order")
            .status = TradeOrderStatus::Completed;
        assert_eq!(
            question_index
                .apply_workflow_message(&message(
                    "buyer-pubkey",
                    listing_addr,
                    Some("order-question-transition"),
                    TradeListingMessagePayload::Question(TradeQuestion {
                        question_id: "question-1".into(),
                    }),
                ))
                .expect_err("question invalid transition"),
            RadrootsTradeProjectionError::InvalidTransition {
                from: TradeOrderStatus::Completed,
                to: TradeOrderStatus::Questioned,
            }
        );

        let mut answer_index = RadrootsTradeReadIndex::new();
        answer_index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                listing_addr,
                Some("order-answer-transition"),
                TradeListingMessagePayload::OrderRequest(TradeOrder {
                    order_id: "order-answer-transition".into(),
                    ..base_order()
                }),
            ))
            .expect("seed answer transition");
        answer_index
            .orders
            .get_mut("order-answer-transition")
            .expect("answer order")
            .status = TradeOrderStatus::Accepted;
        assert_eq!(
            answer_index
                .apply_workflow_message(&message(
                    "seller-pubkey",
                    listing_addr,
                    Some("order-answer-transition"),
                    TradeListingMessagePayload::Answer(TradeAnswer {
                        question_id: "question-1".into(),
                    }),
                ))
                .expect_err("answer invalid transition"),
            RadrootsTradeProjectionError::InvalidTransition {
                from: TradeOrderStatus::Accepted,
                to: TradeOrderStatus::Requested,
            }
        );

        let mut discount_offer_index = RadrootsTradeReadIndex::new();
        discount_offer_index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                listing_addr,
                Some("order-discount-offer-transition"),
                TradeListingMessagePayload::OrderRequest(TradeOrder {
                    order_id: "order-discount-offer-transition".into(),
                    ..base_order()
                }),
            ))
            .expect("seed discount offer transition");
        discount_offer_index
            .orders
            .get_mut("order-discount-offer-transition")
            .expect("discount offer order")
            .status = TradeOrderStatus::Completed;
        assert_eq!(
            discount_offer_index
                .apply_workflow_message(&message(
                    "seller-pubkey",
                    listing_addr,
                    Some("order-discount-offer-transition"),
                    TradeListingMessagePayload::DiscountOffer(TradeDiscountOffer {
                        discount_id: "discount-offer-invalid-transition".into(),
                        value: radroots_core::RadrootsCoreDiscountValue::Percent(
                            RadrootsCorePercent::new(RadrootsCoreDecimal::from(5u32)),
                        ),
                    }),
                ))
                .expect_err("discount offer invalid transition"),
            RadrootsTradeProjectionError::InvalidTransition {
                from: TradeOrderStatus::Completed,
                to: TradeOrderStatus::Revised,
            }
        );

        let mut discount_accept_index = RadrootsTradeReadIndex::new();
        discount_accept_index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                listing_addr,
                Some("order-discount-accept-transition"),
                TradeListingMessagePayload::OrderRequest(TradeOrder {
                    order_id: "order-discount-accept-transition".into(),
                    ..base_order()
                }),
            ))
            .expect("seed discount accept transition");
        discount_accept_index
            .orders
            .get_mut("order-discount-accept-transition")
            .expect("discount accept order")
            .status = TradeOrderStatus::Completed;
        assert_eq!(
            discount_accept_index
                .apply_workflow_message(&message(
                    "buyer-pubkey",
                    listing_addr,
                    Some("order-discount-accept-transition"),
                    TradeListingMessagePayload::DiscountAccept(TradeDiscountDecision::Accept {
                        value: radroots_core::RadrootsCoreDiscountValue::Percent(
                            RadrootsCorePercent::new(RadrootsCoreDecimal::from(5u32)),
                        ),
                    }),
                ))
                .expect_err("discount accept invalid transition"),
            RadrootsTradeProjectionError::InvalidTransition {
                from: TradeOrderStatus::Completed,
                to: TradeOrderStatus::Accepted,
            }
        );

        let mut discount_decline_index = RadrootsTradeReadIndex::new();
        discount_decline_index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                listing_addr,
                Some("order-discount-decline-transition"),
                TradeListingMessagePayload::OrderRequest(TradeOrder {
                    order_id: "order-discount-decline-transition".into(),
                    ..base_order()
                }),
            ))
            .expect("seed discount decline transition");
        discount_decline_index
            .orders
            .get_mut("order-discount-decline-transition")
            .expect("discount decline order")
            .status = TradeOrderStatus::Completed;
        assert_eq!(
            discount_decline_index
                .apply_workflow_message(&message(
                    "buyer-pubkey",
                    listing_addr,
                    Some("order-discount-decline-transition"),
                    TradeListingMessagePayload::DiscountDecline(TradeDiscountDecision::Decline {
                        reason: Some("no".into()),
                    },),
                ))
                .expect_err("discount decline invalid transition"),
            RadrootsTradeProjectionError::InvalidTransition {
                from: TradeOrderStatus::Completed,
                to: TradeOrderStatus::Requested,
            }
        );

        let mut cancel_index = RadrootsTradeReadIndex::new();
        cancel_index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                listing_addr,
                Some("order-cancel-transition"),
                TradeListingMessagePayload::OrderRequest(TradeOrder {
                    order_id: "order-cancel-transition".into(),
                    ..base_order()
                }),
            ))
            .expect("seed cancel transition");
        cancel_index
            .orders
            .get_mut("order-cancel-transition")
            .expect("cancel order")
            .status = TradeOrderStatus::Completed;
        assert_eq!(
            cancel_index
                .apply_workflow_message(&message(
                    "buyer-pubkey",
                    listing_addr,
                    Some("order-cancel-transition"),
                    TradeListingMessagePayload::Cancel(TradeListingCancel {
                        reason: Some("late cancel".into()),
                    }),
                ))
                .expect_err("cancel invalid transition"),
            RadrootsTradeProjectionError::InvalidTransition {
                from: TradeOrderStatus::Completed,
                to: TradeOrderStatus::Cancelled,
            }
        );

        let mut fulfillment_index = RadrootsTradeReadIndex::new();
        fulfillment_index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                listing_addr,
                Some("order-fulfillment-transition"),
                TradeListingMessagePayload::OrderRequest(TradeOrder {
                    order_id: "order-fulfillment-transition".into(),
                    ..base_order()
                }),
            ))
            .expect("seed fulfillment transition");
        assert_eq!(
            fulfillment_index
                .apply_workflow_message(&message(
                    "seller-pubkey",
                    listing_addr,
                    Some("order-fulfillment-transition"),
                    TradeListingMessagePayload::FulfillmentUpdate(TradeFulfillmentUpdate {
                        status: TradeFulfillmentStatus::Delivered,
                    }),
                ))
                .expect_err("fulfillment invalid transition"),
            RadrootsTradeProjectionError::InvalidTransition {
                from: TradeOrderStatus::Requested,
                to: TradeOrderStatus::Fulfilled,
            }
        );

        let mut receipt_index = RadrootsTradeReadIndex::new();
        receipt_index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                listing_addr,
                Some("order-receipt-transition"),
                TradeListingMessagePayload::OrderRequest(TradeOrder {
                    order_id: "order-receipt-transition".into(),
                    ..base_order()
                }),
            ))
            .expect("seed receipt transition");
        receipt_index
            .orders
            .get_mut("order-receipt-transition")
            .expect("receipt order")
            .status = TradeOrderStatus::Accepted;
        assert_eq!(
            receipt_index
                .apply_workflow_message(&message(
                    "buyer-pubkey",
                    listing_addr,
                    Some("order-receipt-transition"),
                    TradeListingMessagePayload::Receipt(TradeReceipt {
                        acknowledged: true,
                        at: 1_700_000_123,
                    }),
                ))
                .expect_err("receipt invalid transition"),
            RadrootsTradeProjectionError::InvalidTransition {
                from: TradeOrderStatus::Accepted,
                to: TradeOrderStatus::Completed,
            }
        );
    }

    #[test]
    fn workflow_projection_rejects_invalid_revision_change_indices() {
        let listing_addr = "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg";
        let mut index = RadrootsTradeReadIndex::new();
        index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                listing_addr,
                Some("order-revision-invalid-change"),
                TradeListingMessagePayload::OrderRequest(TradeOrder {
                    order_id: "order-revision-invalid-change".into(),
                    ..base_order()
                }),
            ))
            .expect("seed invalid revision order");

        assert_eq!(
            index
                .apply_workflow_message(&message(
                    "seller-pubkey",
                    listing_addr,
                    Some("order-revision-invalid-change"),
                    TradeListingMessagePayload::OrderRevision(TradeOrderRevision {
                        revision_id: "rev-invalid-index".into(),
                        changes: vec![TradeOrderChange::BinCount {
                            item_index: 7,
                            bin_count: 3,
                        }],
                    }),
                ))
                .expect_err("invalid revision change"),
            RadrootsTradeProjectionError::InvalidItemIndex(7)
        );
    }

    #[test]
    fn workflow_projection_covers_successful_follow_up_paths() {
        let mut question_index = RadrootsTradeReadIndex::new();
        question_index
            .upsert_listing("seller-pubkey", &base_listing())
            .expect("question listing");
        question_index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                Some("order-question"),
                TradeListingMessagePayload::OrderRequest(TradeOrder {
                    order_id: "order-question".into(),
                    ..base_order()
                }),
            ))
            .expect("question order");
        question_index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                Some("order-question"),
                TradeListingMessagePayload::Question(TradeQuestion {
                    question_id: "question-1".into(),
                }),
            ))
            .expect("question");
        let answered = question_index
            .apply_workflow_message(&message(
                "seller-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                Some("order-question"),
                TradeListingMessagePayload::Answer(TradeAnswer {
                    question_id: "question-1".into(),
                }),
            ))
            .expect("answer");
        assert_eq!(answered.status, TradeOrderStatus::Requested);
        assert_eq!(answered.answer_count, 1);

        let mut revision_accept_index = RadrootsTradeReadIndex::new();
        revision_accept_index
            .upsert_listing("seller-pubkey", &base_listing())
            .expect("revision accept listing");
        revision_accept_index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                Some("order-revision-accept"),
                TradeListingMessagePayload::OrderRequest(TradeOrder {
                    order_id: "order-revision-accept".into(),
                    ..base_order()
                }),
            ))
            .expect("revision accept request");
        revision_accept_index
            .apply_workflow_message(&message(
                "seller-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                Some("order-revision-accept"),
                TradeListingMessagePayload::OrderRevision(TradeOrderRevision {
                    revision_id: "revision-accept".into(),
                    changes: vec![TradeOrderChange::ItemAdd {
                        item: TradeOrderItem {
                            bin_id: "bin-2".into(),
                            bin_count: 1,
                        },
                    }],
                }),
            ))
            .expect("revision");
        let accepted_revision = revision_accept_index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                Some("order-revision-accept"),
                TradeListingMessagePayload::OrderRevisionAccept(TradeOrderRevisionResponse {
                    accepted: true,
                    reason: Some("works".into()),
                }),
            ))
            .expect("revision accept");
        assert_eq!(accepted_revision.status, TradeOrderStatus::Accepted);
        assert_eq!(
            accepted_revision.last_message_type,
            TradeListingMessageType::OrderRevisionAccept
        );

        let mut revision_decline_index = RadrootsTradeReadIndex::new();
        revision_decline_index
            .upsert_listing("seller-pubkey", &base_listing())
            .expect("revision decline listing");
        revision_decline_index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                Some("order-revision-decline"),
                TradeListingMessagePayload::OrderRequest(TradeOrder {
                    order_id: "order-revision-decline".into(),
                    ..base_order()
                }),
            ))
            .expect("revision decline request");
        revision_decline_index
            .apply_workflow_message(&message(
                "seller-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                Some("order-revision-decline"),
                TradeListingMessagePayload::OrderRevision(TradeOrderRevision {
                    revision_id: "revision-decline".into(),
                    changes: vec![TradeOrderChange::ItemRemove { item_index: 0 }],
                }),
            ))
            .expect("revision decline");
        let declined_revision = revision_decline_index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                Some("order-revision-decline"),
                TradeListingMessagePayload::OrderRevisionDecline(TradeOrderRevisionResponse {
                    accepted: false,
                    reason: Some("no thanks".into()),
                }),
            ))
            .expect("revision decline");
        assert_eq!(declined_revision.status, TradeOrderStatus::Declined);
        assert_eq!(
            declined_revision.last_message_type,
            TradeListingMessageType::OrderRevisionDecline
        );

        let mut discount_index = RadrootsTradeReadIndex::new();
        discount_index
            .upsert_listing("seller-pubkey", &base_listing())
            .expect("discount listing");
        discount_index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                Some("order-discount"),
                TradeListingMessagePayload::OrderRequest(TradeOrder {
                    order_id: "order-discount".into(),
                    ..base_order()
                }),
            ))
            .expect("discount request order");
        discount_index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                Some("order-discount"),
                TradeListingMessagePayload::DiscountRequest(TradeDiscountRequest {
                    discount_id: "discount-request".into(),
                    value: radroots_core::RadrootsCoreDiscountValue::Percent(
                        RadrootsCorePercent::new(RadrootsCoreDecimal::from(10u32)),
                    ),
                }),
            ))
            .expect("discount request");
        discount_index
            .apply_workflow_message(&message(
                "seller-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                Some("order-discount"),
                TradeListingMessagePayload::DiscountOffer(TradeDiscountOffer {
                    discount_id: "discount-request".into(),
                    value: radroots_core::RadrootsCoreDiscountValue::Percent(
                        RadrootsCorePercent::new(RadrootsCoreDecimal::from(5u32)),
                    ),
                }),
            ))
            .expect("discount offer");
        let accepted_discount = discount_index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                Some("order-discount"),
                TradeListingMessagePayload::DiscountAccept(TradeDiscountDecision::Accept {
                    value: radroots_core::RadrootsCoreDiscountValue::Percent(
                        RadrootsCorePercent::new(RadrootsCoreDecimal::from(5u32)),
                    ),
                }),
            ))
            .expect("discount accept");
        assert_eq!(accepted_discount.status, TradeOrderStatus::Accepted);
        assert_eq!(accepted_discount.discount_accept_count, 1);

        let mut discount_decline_index = RadrootsTradeReadIndex::new();
        discount_decline_index
            .upsert_listing("seller-pubkey", &base_listing())
            .expect("discount decline listing");
        discount_decline_index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                Some("order-discount-decline"),
                TradeListingMessagePayload::OrderRequest(TradeOrder {
                    order_id: "order-discount-decline".into(),
                    ..base_order()
                }),
            ))
            .expect("discount decline request order");
        discount_decline_index
            .apply_workflow_message(&message(
                "seller-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                Some("order-discount-decline"),
                TradeListingMessagePayload::DiscountOffer(TradeDiscountOffer {
                    discount_id: "discount-decline".into(),
                    value: radroots_core::RadrootsCoreDiscountValue::Percent(
                        RadrootsCorePercent::new(RadrootsCoreDecimal::from(7u32)),
                    ),
                }),
            ))
            .expect("discount decline offer");
        let declined_discount = discount_decline_index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                Some("order-discount-decline"),
                TradeListingMessagePayload::DiscountDecline(TradeDiscountDecision::Decline {
                    reason: Some("still too high".into()),
                }),
            ))
            .expect("discount decline");
        assert_eq!(declined_discount.status, TradeOrderStatus::Requested);
        assert_eq!(declined_discount.discount_decline_count, 1);

        let mut seller_cancel_index = RadrootsTradeReadIndex::new();
        seller_cancel_index
            .upsert_listing("seller-pubkey", &base_listing())
            .expect("seller cancel listing");
        seller_cancel_index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                Some("order-seller-cancel"),
                TradeListingMessagePayload::OrderRequest(TradeOrder {
                    order_id: "order-seller-cancel".into(),
                    ..base_order()
                }),
            ))
            .expect("seller cancel order");
        let seller_cancelled = seller_cancel_index
            .apply_workflow_message(&message(
                "seller-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                Some("order-seller-cancel"),
                TradeListingMessagePayload::Cancel(TradeListingCancel {
                    reason: Some("seller-cancel".into()),
                }),
            ))
            .expect("seller cancel");
        assert_eq!(seller_cancelled.status, TradeOrderStatus::Cancelled);

        let mut preparing_index = RadrootsTradeReadIndex::new();
        preparing_index
            .upsert_listing("seller-pubkey", &base_listing())
            .expect("preparing listing");
        preparing_index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                Some("order-preparing"),
                TradeListingMessagePayload::OrderRequest(TradeOrder {
                    order_id: "order-preparing".into(),
                    ..base_order()
                }),
            ))
            .expect("preparing request");
        preparing_index
            .apply_workflow_message(&message(
                "seller-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                Some("order-preparing"),
                TradeListingMessagePayload::OrderResponse(TradeOrderResponse {
                    accepted: true,
                    reason: None,
                }),
            ))
            .expect("preparing accepted");
        let preparing = preparing_index
            .apply_workflow_message(&message(
                "seller-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                Some("order-preparing"),
                TradeListingMessagePayload::FulfillmentUpdate(TradeFulfillmentUpdate {
                    status: TradeFulfillmentStatus::Preparing,
                }),
            ))
            .expect("preparing update");
        assert_eq!(preparing.status, TradeOrderStatus::Accepted);

        let mut receipt_index = RadrootsTradeReadIndex::new();
        receipt_index
            .upsert_listing("seller-pubkey", &base_listing())
            .expect("receipt listing");
        receipt_index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                Some("order-receipt"),
                TradeListingMessagePayload::OrderRequest(TradeOrder {
                    order_id: "order-receipt".into(),
                    ..base_order()
                }),
            ))
            .expect("receipt request");
        receipt_index
            .apply_workflow_message(&message(
                "seller-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                Some("order-receipt"),
                TradeListingMessagePayload::OrderResponse(TradeOrderResponse {
                    accepted: true,
                    reason: None,
                }),
            ))
            .expect("receipt accepted");
        receipt_index
            .apply_workflow_message(&message(
                "seller-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                Some("order-receipt"),
                TradeListingMessagePayload::FulfillmentUpdate(TradeFulfillmentUpdate {
                    status: TradeFulfillmentStatus::Delivered,
                }),
            ))
            .expect("receipt fulfilled");
        let receipt = receipt_index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                Some("order-receipt"),
                TradeListingMessagePayload::Receipt(TradeReceipt {
                    acknowledged: false,
                    at: 1_700_000_040,
                }),
            ))
            .expect("receipt pending");
        assert_eq!(receipt.status, TradeOrderStatus::Fulfilled);
    }
}
