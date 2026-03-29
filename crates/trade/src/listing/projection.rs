#![forbid(unsafe_code)]

use core::cmp::Ordering;

#[cfg(not(feature = "std"))]
use alloc::{collections::BTreeMap, format, string::String, vec::Vec};
#[cfg(feature = "std")]
use std::collections::BTreeMap;

use radroots_core::{RadrootsCoreDecimal, RadrootsCoreDiscount, RadrootsCoreDiscountValue};
use radroots_events::{
    RadrootsNostrEvent,
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
    #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsTradeOrderItem[]"))]
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
    pub actor_pubkey: String,
    pub listing_addr: String,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub order_id: Option<String>,
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
        if event.kind != KIND_LISTING {
            return Err(RadrootsTradeProjectionError::InvalidListingKind { kind: event.kind });
        }
        let listing = listing_from_event_parts(&event.tags, &event.content)
            .map_err(|error| RadrootsTradeProjectionError::InvalidListingContract { error })?;
        Self::from_listing_contract(event.author.clone(), &listing)
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
    #[cfg(feature = "serde_json")]
    pub fn from_event(event: &RadrootsNostrEvent) -> Result<Self, TradeListingEnvelopeParseError> {
        let envelope = trade_listing_envelope_from_event::<TradeListingMessagePayload>(event)?;
        Ok(Self {
            actor_pubkey: event.author.clone(),
            listing_addr: envelope.listing_addr,
            order_id: envelope.order_id,
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
                if let Some(next_status) =
                    trade_order_status_for_fulfillment_update(&order.status, &update.status)?
                {
                    order.status = next_status;
                }
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
        radroots_trade_order_status_ensure_transition(
            order.status.clone(),
            TradeOrderStatus::Requested,
        )?;

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
            radroots_trade_order_status_ensure_transition(
                current.clone(),
                TradeOrderStatus::Fulfilled,
            )?;
            Ok(Some(TradeOrderStatus::Fulfilled))
        }
        TradeFulfillmentStatus::Cancelled => {
            radroots_trade_order_status_ensure_transition(
                current.clone(),
                TradeOrderStatus::Cancelled,
            )?;
            Ok(Some(TradeOrderStatus::Cancelled))
        }
    }
}

fn trade_order_status_for_receipt(
    current: &TradeOrderStatus,
    acknowledged: bool,
) -> Result<Option<TradeOrderStatus>, RadrootsTradeProjectionError> {
    if acknowledged {
        radroots_trade_order_status_ensure_transition(
            current.clone(),
            TradeOrderStatus::Completed,
        )?;
        Ok(Some(TradeOrderStatus::Completed))
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
    use super::{
        RadrootsTradeListingMarketStatus, RadrootsTradeListingQuery, RadrootsTradeListingSort,
        RadrootsTradeListingSortField, RadrootsTradeOrderQuery, RadrootsTradeOrderSort,
        RadrootsTradeOrderSortField, RadrootsTradeOrderWorkflowMessage,
        RadrootsTradeProjectionError, RadrootsTradeReadIndex, RadrootsTradeSortDirection,
        radroots_trade_order_status_can_transition, radroots_trade_order_status_is_terminal,
    };
    use crate::listing::{
        codec::listing_tags_build,
        dvm::{
            TradeListingCancel, TradeListingEnvelopeParseError, TradeListingMessagePayload,
            TradeOrderResponse, trade_listing_envelope_event_build,
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
    use radroots_events::listing::{
        RadrootsListing, RadrootsListingAvailability, RadrootsListingBin,
        RadrootsListingDeliveryMethod, RadrootsListingFarmRef, RadrootsListingLocation,
        RadrootsListingProduct, RadrootsListingStatus,
    };
    use radroots_events::{RadrootsNostrEvent, kinds::KIND_LISTING};

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
            status: TradeOrderStatus::Requested,
        }
    }

    fn alternate_listing() -> RadrootsListing {
        RadrootsListing {
            d_tag: "AAAAAAAAAAAAAAAAAAAAAw".into(),
            farm: RadrootsListingFarmRef {
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
            notes: Some("expedite".into()),
            status: TradeOrderStatus::Requested,
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
        let built = trade_listing_envelope_event_build(
            recipient_pubkey,
            message_type,
            listing_addr.to_string(),
            order_id.map(str::to_string),
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
                    tracking: Some("track-1".into()),
                    eta: None,
                    notes: Some("left warehouse".into()),
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
                    tracking: Some("track-1".into()),
                    eta: None,
                    notes: Some("left at dock".into()),
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
                    note: Some("received pending inspection".into()),
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

        let err = index
            .apply_workflow_message(&message(
                "buyer-pubkey",
                "30402:seller-pubkey:AAAAAAAAAAAAAAAAAAAAAg",
                Some("order-1"),
                TradeListingMessagePayload::OrderRequest(TradeOrder {
                    status: TradeOrderStatus::Accepted,
                    ..base_order()
                }),
            ))
            .expect_err("non-requested order request should fail");
        assert_eq!(
            err,
            RadrootsTradeProjectionError::InvalidTransition {
                from: TradeOrderStatus::Accepted,
                to: TradeOrderStatus::Requested,
            }
        );
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
                    tracking: None,
                    eta: None,
                    notes: Some("shipped".into()),
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
                    note: Some("received".into()),
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
                    tracking: Some("track-2".into()),
                    eta: None,
                    notes: Some("in transit".into()),
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
                    note: Some("all good".into()),
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
        assert_eq!(summaries[0].last_reason.as_deref(), Some("all good"));
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
