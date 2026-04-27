#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use crate::{RadrootsNostrEventPtr, kinds::*};
use radroots_core::RadrootsCoreDiscountValue;
#[cfg(feature = "ts-rs")]
use ts_rs::TS;

pub const RADROOTS_TRADE_LISTING_DOMAIN: &str = "trade:listing";
pub const RADROOTS_TRADE_ENVELOPE_VERSION: u16 = 1;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsTradeListingParseError {
    InvalidKind(u32),
    MissingTag(String),
    InvalidTag(String),
    InvalidNumber(String),
    InvalidUnit,
    InvalidCurrency,
    InvalidJson(String),
    InvalidDiscount(String),
}

impl core::fmt::Display for RadrootsTradeListingParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidKind(kind) => write!(f, "invalid listing kind: {kind}"),
            Self::MissingTag(tag) => write!(f, "missing required tag: {tag}"),
            Self::InvalidTag(tag) => write!(f, "invalid tag: {tag}"),
            Self::InvalidNumber(field) => write!(f, "invalid number: {field}"),
            Self::InvalidUnit => write!(f, "invalid unit"),
            Self::InvalidCurrency => write!(f, "invalid currency"),
            Self::InvalidJson(field) => write!(f, "invalid json: {field}"),
            Self::InvalidDiscount(kind) => write!(f, "invalid discount data for {kind}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsTradeListingParseError {}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(rename_all = "snake_case", tag = "kind", content = "amount")
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsTradeListingValidationError {
    InvalidKind {
        kind: u32,
    },
    MissingListingId,
    ListingEventNotFound {
        listing_addr: String,
    },
    ListingEventFetchFailed {
        listing_addr: String,
    },
    ParseError {
        error: RadrootsTradeListingParseError,
    },
    InvalidSeller,
    MissingFarmProfile,
    MissingFarmRecord,
    MissingTitle,
    MissingDescription,
    MissingProductType,
    MissingBins,
    MissingPrimaryBin,
    InvalidBin,
    MissingPrice,
    InvalidPrice,
    MissingInventory,
    InvalidInventory,
    MissingAvailability,
    MissingLocation,
    MissingDeliveryMethod,
}

impl core::fmt::Display for RadrootsTradeListingValidationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidKind { kind } => write!(f, "invalid listing kind: {kind}"),
            Self::MissingListingId => write!(f, "missing listing id"),
            Self::ListingEventNotFound { listing_addr } => {
                write!(f, "listing event not found: {listing_addr}")
            }
            Self::ListingEventFetchFailed { listing_addr } => {
                write!(f, "listing event fetch failed: {listing_addr}")
            }
            Self::ParseError { error } => write!(f, "invalid listing data: {error}"),
            Self::InvalidSeller => write!(f, "listing author does not match farm pubkey"),
            Self::MissingFarmProfile => write!(f, "missing farm profile"),
            Self::MissingFarmRecord => write!(f, "missing farm record"),
            Self::MissingTitle => write!(f, "missing listing title"),
            Self::MissingDescription => write!(f, "missing listing description"),
            Self::MissingProductType => write!(f, "missing listing product type"),
            Self::MissingBins => write!(f, "missing listing bins"),
            Self::MissingPrimaryBin => write!(f, "missing primary listing bin"),
            Self::InvalidBin => write!(f, "invalid listing bin"),
            Self::MissingPrice => write!(f, "missing listing price"),
            Self::InvalidPrice => write!(f, "invalid listing price"),
            Self::MissingInventory => write!(f, "missing listing inventory"),
            Self::InvalidInventory => write!(f, "invalid listing inventory"),
            Self::MissingAvailability => write!(f, "missing listing availability"),
            Self::MissingLocation => write!(f, "missing listing location"),
            Self::MissingDeliveryMethod => write!(f, "missing listing delivery method"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsTradeListingValidationError {}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeOrderItem {
    pub bin_id: String,
    pub bin_count: u32,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(rename_all = "snake_case", tag = "kind", content = "amount")
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsTradeOrderChange {
    BinCount { item_index: u32, bin_count: u32 },
    ItemAdd { item: RadrootsTradeOrderItem },
    ItemRemove { item_index: u32 },
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeOrderRevision {
    pub revision_id: String,
    pub changes: Vec<RadrootsTradeOrderChange>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsTradeOrderStatus {
    Draft,
    Validated,
    Requested,
    Questioned,
    Revised,
    Accepted,
    Declined,
    Cancelled,
    Fulfilled,
    Completed,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeOrder {
    pub order_id: String,
    pub listing_addr: String,
    pub buyer_pubkey: String,
    pub seller_pubkey: String,
    pub items: Vec<RadrootsTradeOrderItem>,
    #[cfg_attr(
        feature = "ts-rs",
        ts(optional, type = "RadrootsCoreDiscountValue[] | null")
    )]
    pub discounts: Option<Vec<RadrootsCoreDiscountValue>>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeOrderRequested {
    pub order_id: String,
    pub listing_addr: String,
    pub buyer_pubkey: String,
    pub seller_pubkey: String,
    pub items: Vec<RadrootsTradeOrderItem>,
}

impl RadrootsTradeOrderRequested {
    pub fn validate(&self) -> Result<(), RadrootsActiveTradePayloadError> {
        validate_required_field(&self.order_id, "order_id")?;
        validate_required_field(&self.listing_addr, "listing_addr")?;
        validate_required_field(&self.buyer_pubkey, "buyer_pubkey")?;
        validate_required_field(&self.seller_pubkey, "seller_pubkey")?;
        validate_order_items(&self.items)
    }
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeInventoryCommitment {
    pub bin_id: String,
    pub bin_count: u32,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case", tag = "decision"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsTradeOrderDecision {
    Accepted {
        inventory_commitments: Vec<RadrootsTradeInventoryCommitment>,
    },
    Declined {
        reason: String,
    },
}

impl RadrootsTradeOrderDecision {
    pub fn validate(&self) -> Result<(), RadrootsActiveTradePayloadError> {
        match self {
            Self::Accepted {
                inventory_commitments,
            } => validate_inventory_commitments(inventory_commitments),
            Self::Declined { reason } => validate_required_field(reason, "reason"),
        }
    }
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeOrderDecisionEvent {
    pub order_id: String,
    pub listing_addr: String,
    pub buyer_pubkey: String,
    pub seller_pubkey: String,
    pub decision: RadrootsTradeOrderDecision,
}

impl RadrootsTradeOrderDecisionEvent {
    pub fn validate(&self) -> Result<(), RadrootsActiveTradePayloadError> {
        validate_required_field(&self.order_id, "order_id")?;
        validate_required_field(&self.listing_addr, "listing_addr")?;
        validate_required_field(&self.buyer_pubkey, "buyer_pubkey")?;
        validate_required_field(&self.seller_pubkey, "seller_pubkey")?;
        self.decision.validate()
    }
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeQuestion {
    pub question_id: String,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeAnswer {
    pub question_id: String,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeDiscountRequest {
    pub discount_id: String,
    #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsCoreDiscountValue"))]
    pub value: RadrootsCoreDiscountValue,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeDiscountOffer {
    pub discount_id: String,
    #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsCoreDiscountValue"))]
    pub value: RadrootsCoreDiscountValue,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(rename_all = "snake_case", tag = "kind", content = "amount")
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsTradeDiscountDecision {
    Accept {
        #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsCoreDiscountValue"))]
        value: RadrootsCoreDiscountValue,
    },
    Decline {
        #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
        reason: Option<String>,
    },
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(rename_all = "snake_case", tag = "kind", content = "amount")
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsTradeFulfillmentStatus {
    Preparing,
    Shipped,
    ReadyForPickup,
    Delivered,
    Cancelled,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeFulfillmentUpdate {
    pub status: RadrootsTradeFulfillmentStatus,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeReceipt {
    pub acknowledged: bool,
    pub at: u64,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeListingValidateRequest {
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "RadrootsNostrEventPtr | null"))]
    pub listing_event: Option<RadrootsNostrEventPtr>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeListingValidateResult {
    pub valid: bool,
    #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsTradeListingValidationError[]"))]
    pub errors: Vec<RadrootsTradeListingValidationError>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeOrderResponse {
    pub accepted: bool,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub reason: Option<String>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeOrderRevisionResponse {
    pub accepted: bool,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub reason: Option<String>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeListingCancel {
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub reason: Option<String>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsTradeDomain {
    #[cfg_attr(feature = "serde", serde(rename = "trade:listing"))]
    TradeListing,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsTradeTransportLane {
    Service,
    Public,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsActiveTradeMessageType {
    #[cfg_attr(feature = "serde", serde(rename = "TradeOrderRequested"))]
    TradeOrderRequested,
    #[cfg_attr(feature = "serde", serde(rename = "TradeOrderDecision"))]
    TradeOrderDecision,
}

impl RadrootsActiveTradeMessageType {
    #[inline]
    pub const fn from_kind(kind: u32) -> Option<Self> {
        match kind {
            KIND_TRADE_ORDER_REQUEST => Some(Self::TradeOrderRequested),
            KIND_TRADE_ORDER_DECISION => Some(Self::TradeOrderDecision),
            _ => None,
        }
    }

    #[inline]
    pub const fn kind(self) -> u32 {
        match self {
            Self::TradeOrderRequested => KIND_TRADE_ORDER_REQUEST,
            Self::TradeOrderDecision => KIND_TRADE_ORDER_DECISION,
        }
    }

    #[inline]
    pub const fn name(self) -> &'static str {
        match self {
            Self::TradeOrderRequested => "TradeOrderRequested",
            Self::TradeOrderDecision => "TradeOrderDecision",
        }
    }

    #[inline]
    pub const fn requires_listing_snapshot(self) -> bool {
        matches!(self, Self::TradeOrderRequested)
    }

    #[inline]
    pub const fn requires_trade_chain(self) -> bool {
        matches!(self, Self::TradeOrderDecision)
    }
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsTradeMessageType {
    ListingValidateRequest,
    ListingValidateResult,
    OrderRequest,
    OrderResponse,
    OrderRevision,
    OrderRevisionAccept,
    OrderRevisionDecline,
    Question,
    Answer,
    DiscountRequest,
    DiscountOffer,
    DiscountAccept,
    DiscountDecline,
    Cancel,
    FulfillmentUpdate,
    Receipt,
}

impl RadrootsTradeMessageType {
    #[inline]
    pub const fn from_kind(kind: u32) -> Option<Self> {
        match kind {
            KIND_TRADE_LISTING_VALIDATE_REQ => Some(Self::ListingValidateRequest),
            KIND_TRADE_LISTING_VALIDATE_RES => Some(Self::ListingValidateResult),
            KIND_TRADE_ORDER_REQUEST => Some(Self::OrderRequest),
            KIND_TRADE_ORDER_RESPONSE => Some(Self::OrderResponse),
            KIND_TRADE_ORDER_REVISION => Some(Self::OrderRevision),
            KIND_TRADE_ORDER_REVISION_RESPONSE => None,
            KIND_TRADE_QUESTION => Some(Self::Question),
            KIND_TRADE_ANSWER => Some(Self::Answer),
            KIND_TRADE_DISCOUNT_REQUEST => Some(Self::DiscountRequest),
            KIND_TRADE_DISCOUNT_OFFER => Some(Self::DiscountOffer),
            KIND_TRADE_DISCOUNT_ACCEPT => Some(Self::DiscountAccept),
            KIND_TRADE_DISCOUNT_DECLINE => Some(Self::DiscountDecline),
            KIND_TRADE_CANCEL => Some(Self::Cancel),
            KIND_TRADE_FULFILLMENT_UPDATE => Some(Self::FulfillmentUpdate),
            KIND_TRADE_RECEIPT => Some(Self::Receipt),
            _ => None,
        }
    }

    #[inline]
    pub const fn kind(self) -> u32 {
        match self {
            Self::ListingValidateRequest => KIND_TRADE_LISTING_VALIDATE_REQ,
            Self::ListingValidateResult => KIND_TRADE_LISTING_VALIDATE_RES,
            Self::OrderRequest => KIND_TRADE_ORDER_REQUEST,
            Self::OrderResponse => KIND_TRADE_ORDER_RESPONSE,
            Self::OrderRevision => KIND_TRADE_ORDER_REVISION,
            Self::OrderRevisionAccept => KIND_TRADE_ORDER_REVISION_RESPONSE,
            Self::OrderRevisionDecline => KIND_TRADE_ORDER_REVISION_RESPONSE,
            Self::Question => KIND_TRADE_QUESTION,
            Self::Answer => KIND_TRADE_ANSWER,
            Self::DiscountRequest => KIND_TRADE_DISCOUNT_REQUEST,
            Self::DiscountOffer => KIND_TRADE_DISCOUNT_OFFER,
            Self::DiscountAccept => KIND_TRADE_DISCOUNT_ACCEPT,
            Self::DiscountDecline => KIND_TRADE_DISCOUNT_DECLINE,
            Self::Cancel => KIND_TRADE_CANCEL,
            Self::FulfillmentUpdate => KIND_TRADE_FULFILLMENT_UPDATE,
            Self::Receipt => KIND_TRADE_RECEIPT,
        }
    }

    #[inline]
    pub const fn lane(self) -> RadrootsTradeTransportLane {
        match self {
            Self::ListingValidateRequest | Self::ListingValidateResult => {
                RadrootsTradeTransportLane::Service
            }
            Self::OrderRequest
            | Self::OrderResponse
            | Self::OrderRevision
            | Self::OrderRevisionAccept
            | Self::OrderRevisionDecline
            | Self::Question
            | Self::Answer
            | Self::DiscountRequest
            | Self::DiscountOffer
            | Self::DiscountAccept
            | Self::DiscountDecline
            | Self::Cancel
            | Self::FulfillmentUpdate
            | Self::Receipt => RadrootsTradeTransportLane::Public,
        }
    }

    #[inline]
    pub const fn is_service(self) -> bool {
        matches!(self.lane(), RadrootsTradeTransportLane::Service)
    }

    #[inline]
    pub const fn is_public(self) -> bool {
        matches!(self.lane(), RadrootsTradeTransportLane::Public)
    }

    #[inline]
    pub const fn requires_order_id(self) -> bool {
        !matches!(
            self,
            Self::ListingValidateRequest | Self::ListingValidateResult
        )
    }

    #[inline]
    pub const fn requires_listing_snapshot(self) -> bool {
        matches!(
            self,
            Self::OrderRequest | Self::OrderRevision | Self::DiscountRequest | Self::DiscountOffer
        )
    }

    #[inline]
    pub const fn requires_trade_chain(self) -> bool {
        self.is_public() && !matches!(self, Self::OrderRequest)
    }

    #[inline]
    pub const fn is_request(self) -> bool {
        matches!(
            self,
            Self::ListingValidateRequest
                | Self::OrderRequest
                | Self::OrderRevision
                | Self::Question
                | Self::DiscountRequest
                | Self::DiscountAccept
                | Self::DiscountDecline
                | Self::Cancel
                | Self::FulfillmentUpdate
                | Self::Receipt
        )
    }

    #[inline]
    pub const fn is_result(self) -> bool {
        !self.is_request()
    }
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsActiveTradeEnvelope<T> {
    pub version: u16,
    pub domain: RadrootsTradeDomain,
    #[cfg_attr(feature = "serde", serde(rename = "type"))]
    pub message_type: RadrootsActiveTradeMessageType,
    pub order_id: String,
    pub listing_addr: String,
    pub payload: T,
}

impl<T> RadrootsActiveTradeEnvelope<T> {
    #[inline]
    pub fn new(
        message_type: RadrootsActiveTradeMessageType,
        listing_addr: impl Into<String>,
        order_id: impl Into<String>,
        payload: T,
    ) -> Self {
        Self {
            version: RADROOTS_TRADE_ENVELOPE_VERSION,
            domain: RadrootsTradeDomain::TradeListing,
            message_type,
            order_id: order_id.into(),
            listing_addr: listing_addr.into(),
            payload,
        }
    }

    pub fn validate(&self) -> Result<(), RadrootsActiveTradeEnvelopeError> {
        if self.version != RADROOTS_TRADE_ENVELOPE_VERSION {
            return Err(RadrootsActiveTradeEnvelopeError::InvalidVersion {
                expected: RADROOTS_TRADE_ENVELOPE_VERSION,
                got: self.version,
            });
        }
        if self.order_id.trim().is_empty() {
            return Err(RadrootsActiveTradeEnvelopeError::MissingOrderId);
        }
        if self.listing_addr.trim().is_empty() {
            return Err(RadrootsActiveTradeEnvelopeError::MissingListingAddr);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadrootsActiveTradeEnvelopeError {
    InvalidVersion { expected: u16, got: u16 },
    MissingOrderId,
    MissingListingAddr,
}

impl core::fmt::Display for RadrootsActiveTradeEnvelopeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidVersion { expected, got } => {
                write!(
                    f,
                    "invalid active trade envelope version: expected {expected}, got {got}"
                )
            }
            Self::MissingOrderId => write!(f, "missing order_id for active trade message"),
            Self::MissingListingAddr => write!(f, "missing listing_addr"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsActiveTradeEnvelopeError {}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeEnvelope<T> {
    pub version: u16,
    pub domain: RadrootsTradeDomain,
    #[cfg_attr(feature = "serde", serde(rename = "type"))]
    pub message_type: RadrootsTradeMessageType,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub order_id: Option<String>,
    pub listing_addr: String,
    pub payload: T,
}

impl<T> RadrootsTradeEnvelope<T> {
    #[inline]
    pub fn new(
        message_type: RadrootsTradeMessageType,
        listing_addr: impl Into<String>,
        order_id: Option<String>,
        payload: T,
    ) -> Self {
        Self {
            version: RADROOTS_TRADE_ENVELOPE_VERSION,
            domain: RadrootsTradeDomain::TradeListing,
            message_type,
            order_id,
            listing_addr: listing_addr.into(),
            payload,
        }
    }

    pub fn validate(&self) -> Result<(), RadrootsTradeEnvelopeError> {
        if self.version != RADROOTS_TRADE_ENVELOPE_VERSION {
            return Err(RadrootsTradeEnvelopeError::InvalidVersion {
                expected: RADROOTS_TRADE_ENVELOPE_VERSION,
                got: self.version,
            });
        }
        if self.listing_addr.trim().is_empty() {
            return Err(RadrootsTradeEnvelopeError::MissingListingAddr);
        }
        if self.message_type.requires_order_id() {
            match self.order_id.as_deref() {
                Some(id) if !id.trim().is_empty() => {}
                _ => return Err(RadrootsTradeEnvelopeError::MissingOrderId),
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadrootsActiveTradePayloadError {
    EmptyField(&'static str),
    MissingItems,
    InvalidItemBinCount { index: usize },
    MissingInventoryCommitments,
    InvalidInventoryCommitmentCount { index: usize },
}

impl core::fmt::Display for RadrootsActiveTradePayloadError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::EmptyField(field) => write!(f, "{field} cannot be empty"),
            Self::MissingItems => write!(f, "items must contain at least one item"),
            Self::InvalidItemBinCount { index } => {
                write!(f, "items[{index}].bin_count must be greater than zero")
            }
            Self::MissingInventoryCommitments => {
                write!(
                    f,
                    "accepted decisions must contain at least one inventory commitment"
                )
            }
            Self::InvalidInventoryCommitmentCount { index } => write!(
                f,
                "inventory_commitments[{index}].bin_count must be greater than zero"
            ),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsActiveTradePayloadError {}

fn validate_required_field(
    value: &str,
    field: &'static str,
) -> Result<(), RadrootsActiveTradePayloadError> {
    if value.trim().is_empty() {
        Err(RadrootsActiveTradePayloadError::EmptyField(field))
    } else {
        Ok(())
    }
}

fn validate_order_items(
    items: &[RadrootsTradeOrderItem],
) -> Result<(), RadrootsActiveTradePayloadError> {
    if items.is_empty() {
        return Err(RadrootsActiveTradePayloadError::MissingItems);
    }
    for (index, item) in items.iter().enumerate() {
        validate_required_field(&item.bin_id, "bin_id")?;
        if item.bin_count == 0 {
            return Err(RadrootsActiveTradePayloadError::InvalidItemBinCount { index });
        }
    }
    Ok(())
}

fn validate_inventory_commitments(
    commitments: &[RadrootsTradeInventoryCommitment],
) -> Result<(), RadrootsActiveTradePayloadError> {
    if commitments.is_empty() {
        return Err(RadrootsActiveTradePayloadError::MissingInventoryCommitments);
    }
    for (index, commitment) in commitments.iter().enumerate() {
        validate_required_field(&commitment.bin_id, "bin_id")?;
        if commitment.bin_count == 0 {
            return Err(RadrootsActiveTradePayloadError::InvalidInventoryCommitmentCount { index });
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadrootsTradeEnvelopeError {
    InvalidVersion { expected: u16, got: u16 },
    MissingOrderId,
    MissingListingAddr,
}

impl core::fmt::Display for RadrootsTradeEnvelopeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidVersion { expected, got } => {
                write!(
                    f,
                    "invalid envelope version: expected {expected}, got {got}"
                )
            }
            Self::MissingOrderId => write!(f, "missing order_id for order-scoped message"),
            Self::MissingListingAddr => write!(f, "missing listing_addr"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsTradeEnvelopeError {}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(rename_all = "snake_case", tag = "kind", content = "amount")
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsTradeMessagePayload {
    ListingValidateRequest(RadrootsTradeListingValidateRequest),
    ListingValidateResult(RadrootsTradeListingValidateResult),
    OrderRequest(RadrootsTradeOrder),
    OrderResponse(RadrootsTradeOrderResponse),
    OrderRevision(RadrootsTradeOrderRevision),
    OrderRevisionAccept(RadrootsTradeOrderRevisionResponse),
    OrderRevisionDecline(RadrootsTradeOrderRevisionResponse),
    Question(RadrootsTradeQuestion),
    Answer(RadrootsTradeAnswer),
    DiscountRequest(RadrootsTradeDiscountRequest),
    DiscountOffer(RadrootsTradeDiscountOffer),
    DiscountAccept(RadrootsTradeDiscountDecision),
    DiscountDecline(RadrootsTradeDiscountDecision),
    Cancel(RadrootsTradeListingCancel),
    FulfillmentUpdate(RadrootsTradeFulfillmentUpdate),
    Receipt(RadrootsTradeReceipt),
}

impl RadrootsTradeMessagePayload {
    #[inline]
    pub const fn message_type(&self) -> RadrootsTradeMessageType {
        match self {
            Self::ListingValidateRequest(_) => RadrootsTradeMessageType::ListingValidateRequest,
            Self::ListingValidateResult(_) => RadrootsTradeMessageType::ListingValidateResult,
            Self::OrderRequest(_) => RadrootsTradeMessageType::OrderRequest,
            Self::OrderResponse(_) => RadrootsTradeMessageType::OrderResponse,
            Self::OrderRevision(_) => RadrootsTradeMessageType::OrderRevision,
            Self::OrderRevisionAccept(_) => RadrootsTradeMessageType::OrderRevisionAccept,
            Self::OrderRevisionDecline(_) => RadrootsTradeMessageType::OrderRevisionDecline,
            Self::Question(_) => RadrootsTradeMessageType::Question,
            Self::Answer(_) => RadrootsTradeMessageType::Answer,
            Self::DiscountRequest(_) => RadrootsTradeMessageType::DiscountRequest,
            Self::DiscountOffer(_) => RadrootsTradeMessageType::DiscountOffer,
            Self::DiscountAccept(_) => RadrootsTradeMessageType::DiscountAccept,
            Self::DiscountDecline(_) => RadrootsTradeMessageType::DiscountDecline,
            Self::Cancel(_) => RadrootsTradeMessageType::Cancel,
            Self::FulfillmentUpdate(_) => RadrootsTradeMessageType::FulfillmentUpdate,
            Self::Receipt(_) => RadrootsTradeMessageType::Receipt,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use radroots_core::{
        RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreDiscountValue, RadrootsCoreMoney,
        RadrootsCorePercent,
    };

    fn sample_listing_addr() -> String {
        "30402:pubkey:AAAAAAAAAAAAAAAAAAAAAg".into()
    }

    fn sample_discount_value() -> RadrootsCoreDiscountValue {
        RadrootsCoreDiscountValue::MoneyPerBin(RadrootsCoreMoney::from_minor_units_u64(
            250,
            RadrootsCoreCurrency::USD,
        ))
    }

    fn sample_percent_discount() -> RadrootsCoreDiscountValue {
        RadrootsCoreDiscountValue::Percent(RadrootsCorePercent::new(RadrootsCoreDecimal::from(
            10u32,
        )))
    }

    fn sample_order() -> RadrootsTradeOrder {
        RadrootsTradeOrder {
            order_id: "order-1".into(),
            listing_addr: sample_listing_addr(),
            buyer_pubkey: "buyer".into(),
            seller_pubkey: "seller".into(),
            items: vec![RadrootsTradeOrderItem {
                bin_id: "bin-1".into(),
                bin_count: 2,
            }],
            discounts: Some(vec![sample_discount_value()]),
        }
    }

    fn sample_active_order_request() -> RadrootsTradeOrderRequested {
        RadrootsTradeOrderRequested {
            order_id: "order-1".into(),
            listing_addr: sample_listing_addr(),
            buyer_pubkey: "buyer".into(),
            seller_pubkey: "seller".into(),
            items: vec![RadrootsTradeOrderItem {
                bin_id: "bin-1".into(),
                bin_count: 2,
            }],
        }
    }

    fn sample_inventory_commitment() -> RadrootsTradeInventoryCommitment {
        RadrootsTradeInventoryCommitment {
            bin_id: "bin-1".into(),
            bin_count: 2,
        }
    }

    fn sample_active_order_decision() -> RadrootsTradeOrderDecisionEvent {
        RadrootsTradeOrderDecisionEvent {
            order_id: "order-1".into(),
            listing_addr: sample_listing_addr(),
            buyer_pubkey: "buyer".into(),
            seller_pubkey: "seller".into(),
            decision: RadrootsTradeOrderDecision::Accepted {
                inventory_commitments: vec![sample_inventory_commitment()],
            },
        }
    }

    fn sample_order_revision() -> RadrootsTradeOrderRevision {
        RadrootsTradeOrderRevision {
            revision_id: "rev-1".into(),
            changes: vec![
                RadrootsTradeOrderChange::BinCount {
                    item_index: 0,
                    bin_count: 3,
                },
                RadrootsTradeOrderChange::ItemAdd {
                    item: RadrootsTradeOrderItem {
                        bin_id: "bin-2".into(),
                        bin_count: 1,
                    },
                },
                RadrootsTradeOrderChange::ItemRemove { item_index: 1 },
            ],
        }
    }

    fn sample_order_response(accepted: bool) -> RadrootsTradeOrderResponse {
        RadrootsTradeOrderResponse {
            accepted,
            reason: (!accepted).then(|| "not today".into()),
        }
    }

    fn sample_order_revision_response(accepted: bool) -> RadrootsTradeOrderRevisionResponse {
        RadrootsTradeOrderRevisionResponse {
            accepted,
            reason: (!accepted).then(|| "needs changes".into()),
        }
    }

    fn sample_validate_request() -> RadrootsTradeListingValidateRequest {
        RadrootsTradeListingValidateRequest {
            listing_event: Some(RadrootsNostrEventPtr {
                id: "listing-event".into(),
                relays: Some("wss://relay.example.com".into()),
            }),
        }
    }

    fn sample_validate_result() -> RadrootsTradeListingValidateResult {
        RadrootsTradeListingValidateResult {
            valid: false,
            errors: vec![RadrootsTradeListingValidationError::MissingDeliveryMethod],
        }
    }

    #[test]
    fn message_type_classifies_request_and_result_kinds() {
        assert_eq!(
            RadrootsTradeMessageType::from_kind(KIND_TRADE_LISTING_ORDER_REQ),
            Some(RadrootsTradeMessageType::OrderRequest)
        );
        assert_eq!(
            RadrootsTradeMessageType::from_kind(KIND_TRADE_LISTING_ORDER_RES),
            Some(RadrootsTradeMessageType::OrderResponse)
        );
        assert!(RadrootsTradeMessageType::ListingValidateRequest.is_service());
        assert!(RadrootsTradeMessageType::OrderRequest.is_public());
        assert!(RadrootsTradeMessageType::OrderRequest.is_request());
        assert!(RadrootsTradeMessageType::OrderResponse.is_result());
    }

    #[test]
    fn active_message_type_uses_canonical_names_and_kinds() {
        assert_eq!(
            RadrootsActiveTradeMessageType::from_kind(KIND_TRADE_ORDER_REQUEST),
            Some(RadrootsActiveTradeMessageType::TradeOrderRequested)
        );
        assert_eq!(
            RadrootsActiveTradeMessageType::from_kind(KIND_TRADE_ORDER_DECISION),
            Some(RadrootsActiveTradeMessageType::TradeOrderDecision)
        );
        assert_eq!(RadrootsActiveTradeMessageType::from_kind(3431), None);
        assert_eq!(
            RadrootsActiveTradeMessageType::TradeOrderRequested.kind(),
            KIND_TRADE_ORDER_REQUEST
        );
        assert_eq!(
            RadrootsActiveTradeMessageType::TradeOrderDecision.kind(),
            KIND_TRADE_ORDER_DECISION
        );
        assert_eq!(
            RadrootsActiveTradeMessageType::TradeOrderRequested.name(),
            "TradeOrderRequested"
        );
        assert_eq!(
            RadrootsActiveTradeMessageType::TradeOrderDecision.name(),
            "TradeOrderDecision"
        );
        assert!(RadrootsActiveTradeMessageType::TradeOrderRequested.requires_listing_snapshot());
        assert!(RadrootsActiveTradeMessageType::TradeOrderDecision.requires_trade_chain());

        let request_name =
            serde_json::to_value(RadrootsActiveTradeMessageType::TradeOrderRequested).unwrap();
        let decision_name =
            serde_json::to_value(RadrootsActiveTradeMessageType::TradeOrderDecision).unwrap();
        assert_eq!(request_name, serde_json::json!("TradeOrderRequested"));
        assert_eq!(decision_name, serde_json::json!("TradeOrderDecision"));
    }

    #[test]
    fn active_order_request_validation_rejects_invalid_fields() {
        assert_eq!(sample_active_order_request().validate(), Ok(()));

        let mut missing_order_id = sample_active_order_request();
        missing_order_id.order_id = " ".into();
        assert_eq!(
            missing_order_id.validate().unwrap_err(),
            RadrootsActiveTradePayloadError::EmptyField("order_id")
        );

        let mut missing_items = sample_active_order_request();
        missing_items.items.clear();
        assert_eq!(
            missing_items.validate().unwrap_err(),
            RadrootsActiveTradePayloadError::MissingItems
        );

        let mut invalid_count = sample_active_order_request();
        invalid_count.items[0].bin_count = 0;
        assert_eq!(
            invalid_count.validate().unwrap_err(),
            RadrootsActiveTradePayloadError::InvalidItemBinCount { index: 0 }
        );

        let mut missing_bin_id = sample_active_order_request();
        missing_bin_id.items[0].bin_id = " ".into();
        assert_eq!(
            missing_bin_id.validate().unwrap_err(),
            RadrootsActiveTradePayloadError::EmptyField("bin_id")
        );
    }

    #[test]
    fn active_order_decision_validation_enforces_commitment_invariants() {
        assert_eq!(sample_active_order_decision().validate(), Ok(()));

        let declined = RadrootsTradeOrderDecisionEvent {
            decision: RadrootsTradeOrderDecision::Declined {
                reason: "out_of_stock".into(),
            },
            ..sample_active_order_decision()
        };
        assert_eq!(declined.validate(), Ok(()));

        let accepted_without_commitments = RadrootsTradeOrderDecisionEvent {
            decision: RadrootsTradeOrderDecision::Accepted {
                inventory_commitments: Vec::new(),
            },
            ..sample_active_order_decision()
        };
        assert_eq!(
            accepted_without_commitments.validate().unwrap_err(),
            RadrootsActiveTradePayloadError::MissingInventoryCommitments
        );

        let accepted_with_zero_count = RadrootsTradeOrderDecisionEvent {
            decision: RadrootsTradeOrderDecision::Accepted {
                inventory_commitments: vec![RadrootsTradeInventoryCommitment {
                    bin_id: "bin-1".into(),
                    bin_count: 0,
                }],
            },
            ..sample_active_order_decision()
        };
        assert_eq!(
            accepted_with_zero_count.validate().unwrap_err(),
            RadrootsActiveTradePayloadError::InvalidInventoryCommitmentCount { index: 0 }
        );

        let declined_without_reason = RadrootsTradeOrderDecisionEvent {
            decision: RadrootsTradeOrderDecision::Declined { reason: " ".into() },
            ..sample_active_order_decision()
        };
        assert_eq!(
            declined_without_reason.validate().unwrap_err(),
            RadrootsActiveTradePayloadError::EmptyField("reason")
        );
    }

    #[test]
    fn active_envelope_serializes_canonical_type_name() {
        let envelope = RadrootsActiveTradeEnvelope::new(
            RadrootsActiveTradeMessageType::TradeOrderRequested,
            sample_listing_addr(),
            "order-1",
            sample_active_order_request(),
        );
        assert_eq!(envelope.validate(), Ok(()));

        let json = serde_json::to_value(&envelope).unwrap();
        assert_eq!(json["type"], serde_json::json!("TradeOrderRequested"));
        assert_eq!(json["order_id"], serde_json::json!("order-1"));
        assert_eq!(
            json["listing_addr"],
            serde_json::json!("30402:pubkey:AAAAAAAAAAAAAAAAAAAAAg")
        );
        assert_eq!(json["payload"]["items"][0]["bin_id"], "bin-1");
    }

    #[test]
    fn listing_parse_error_display_variants() {
        assert_eq!(
            RadrootsTradeListingParseError::InvalidKind(KIND_PROFILE).to_string(),
            "invalid listing kind: 0"
        );
        assert_eq!(
            RadrootsTradeListingParseError::MissingTag("price".into()).to_string(),
            "missing required tag: price"
        );
        assert_eq!(
            RadrootsTradeListingParseError::InvalidTag("farm".into()).to_string(),
            "invalid tag: farm"
        );
        assert_eq!(
            RadrootsTradeListingParseError::InvalidNumber("inventory".into()).to_string(),
            "invalid number: inventory"
        );
        assert_eq!(
            RadrootsTradeListingParseError::InvalidUnit.to_string(),
            "invalid unit"
        );
        assert_eq!(
            RadrootsTradeListingParseError::InvalidCurrency.to_string(),
            "invalid currency"
        );
        assert_eq!(
            RadrootsTradeListingParseError::InvalidJson("bins".into()).to_string(),
            "invalid json: bins"
        );
        assert_eq!(
            RadrootsTradeListingParseError::InvalidDiscount("offer".into()).to_string(),
            "invalid discount data for offer"
        );
    }

    #[test]
    fn listing_validation_error_display_variants() {
        assert_eq!(
            (RadrootsTradeListingValidationError::InvalidKind { kind: KIND_PROFILE }).to_string(),
            "invalid listing kind: 0"
        );
        assert_eq!(
            RadrootsTradeListingValidationError::MissingListingId.to_string(),
            "missing listing id"
        );
        assert_eq!(
            RadrootsTradeListingValidationError::ListingEventNotFound {
                listing_addr: "listing-1".into(),
            }
            .to_string(),
            "listing event not found: listing-1"
        );
        assert_eq!(
            RadrootsTradeListingValidationError::ListingEventFetchFailed {
                listing_addr: "listing-2".into(),
            }
            .to_string(),
            "listing event fetch failed: listing-2"
        );
        assert_eq!(
            RadrootsTradeListingValidationError::ParseError {
                error: RadrootsTradeListingParseError::InvalidJson("payload".into()),
            }
            .to_string(),
            "invalid listing data: invalid json: payload"
        );
        assert_eq!(
            RadrootsTradeListingValidationError::InvalidSeller.to_string(),
            "listing author does not match farm pubkey"
        );
        assert_eq!(
            RadrootsTradeListingValidationError::MissingFarmProfile.to_string(),
            "missing farm profile"
        );
        assert_eq!(
            RadrootsTradeListingValidationError::MissingFarmRecord.to_string(),
            "missing farm record"
        );
        assert_eq!(
            RadrootsTradeListingValidationError::MissingTitle.to_string(),
            "missing listing title"
        );
        assert_eq!(
            RadrootsTradeListingValidationError::MissingDescription.to_string(),
            "missing listing description"
        );
        assert_eq!(
            RadrootsTradeListingValidationError::MissingProductType.to_string(),
            "missing listing product type"
        );
        assert_eq!(
            RadrootsTradeListingValidationError::MissingBins.to_string(),
            "missing listing bins"
        );
        assert_eq!(
            RadrootsTradeListingValidationError::MissingPrimaryBin.to_string(),
            "missing primary listing bin"
        );
        assert_eq!(
            RadrootsTradeListingValidationError::InvalidBin.to_string(),
            "invalid listing bin"
        );
        assert_eq!(
            RadrootsTradeListingValidationError::MissingPrice.to_string(),
            "missing listing price"
        );
        assert_eq!(
            RadrootsTradeListingValidationError::InvalidPrice.to_string(),
            "invalid listing price"
        );
        assert_eq!(
            RadrootsTradeListingValidationError::MissingInventory.to_string(),
            "missing listing inventory"
        );
        assert_eq!(
            RadrootsTradeListingValidationError::InvalidInventory.to_string(),
            "invalid listing inventory"
        );
        assert_eq!(
            RadrootsTradeListingValidationError::MissingAvailability.to_string(),
            "missing listing availability"
        );
        assert_eq!(
            RadrootsTradeListingValidationError::MissingLocation.to_string(),
            "missing listing location"
        );
        assert_eq!(
            RadrootsTradeListingValidationError::MissingDeliveryMethod.to_string(),
            "missing listing delivery method"
        );
    }

    #[test]
    fn message_type_maps_all_supported_kinds_and_helpers() {
        let cases = [
            (
                RadrootsTradeMessageType::ListingValidateRequest,
                KIND_TRADE_LISTING_VALIDATE_REQ,
                true,
                false,
                false,
                false,
                false,
                true,
            ),
            (
                RadrootsTradeMessageType::ListingValidateResult,
                KIND_TRADE_LISTING_VALIDATE_RES,
                true,
                false,
                false,
                false,
                false,
                false,
            ),
            (
                RadrootsTradeMessageType::OrderRequest,
                KIND_TRADE_ORDER_REQUEST,
                false,
                true,
                true,
                true,
                false,
                true,
            ),
            (
                RadrootsTradeMessageType::OrderResponse,
                KIND_TRADE_ORDER_RESPONSE,
                false,
                true,
                true,
                false,
                true,
                false,
            ),
            (
                RadrootsTradeMessageType::OrderRevision,
                KIND_TRADE_ORDER_REVISION,
                false,
                true,
                true,
                true,
                true,
                true,
            ),
            (
                RadrootsTradeMessageType::OrderRevisionAccept,
                KIND_TRADE_ORDER_REVISION_RESPONSE,
                false,
                true,
                true,
                false,
                true,
                false,
            ),
            (
                RadrootsTradeMessageType::OrderRevisionDecline,
                KIND_TRADE_ORDER_REVISION_RESPONSE,
                false,
                true,
                true,
                false,
                true,
                false,
            ),
            (
                RadrootsTradeMessageType::Question,
                KIND_TRADE_QUESTION,
                false,
                true,
                true,
                false,
                true,
                true,
            ),
            (
                RadrootsTradeMessageType::Answer,
                KIND_TRADE_ANSWER,
                false,
                true,
                true,
                false,
                true,
                false,
            ),
            (
                RadrootsTradeMessageType::DiscountRequest,
                KIND_TRADE_DISCOUNT_REQUEST,
                false,
                true,
                true,
                true,
                true,
                true,
            ),
            (
                RadrootsTradeMessageType::DiscountOffer,
                KIND_TRADE_DISCOUNT_OFFER,
                false,
                true,
                true,
                true,
                true,
                false,
            ),
            (
                RadrootsTradeMessageType::DiscountAccept,
                KIND_TRADE_DISCOUNT_ACCEPT,
                false,
                true,
                true,
                false,
                true,
                true,
            ),
            (
                RadrootsTradeMessageType::DiscountDecline,
                KIND_TRADE_DISCOUNT_DECLINE,
                false,
                true,
                true,
                false,
                true,
                true,
            ),
            (
                RadrootsTradeMessageType::Cancel,
                KIND_TRADE_CANCEL,
                false,
                true,
                true,
                false,
                true,
                true,
            ),
            (
                RadrootsTradeMessageType::FulfillmentUpdate,
                KIND_TRADE_FULFILLMENT_UPDATE,
                false,
                true,
                true,
                false,
                true,
                true,
            ),
            (
                RadrootsTradeMessageType::Receipt,
                KIND_TRADE_RECEIPT,
                false,
                true,
                true,
                false,
                true,
                true,
            ),
        ];

        for (
            message_type,
            kind,
            service,
            public,
            requires_order_id,
            requires_listing_snapshot,
            requires_trade_chain,
            is_request,
        ) in cases
        {
            assert_eq!(message_type.kind(), kind);
            assert_eq!(message_type.is_service(), service);
            assert_eq!(message_type.is_public(), public);
            assert_eq!(message_type.requires_order_id(), requires_order_id);
            assert_eq!(
                message_type.requires_listing_snapshot(),
                requires_listing_snapshot
            );
            assert_eq!(message_type.requires_trade_chain(), requires_trade_chain);
            assert_eq!(message_type.is_request(), is_request);
            assert_eq!(message_type.is_result(), !is_request);
        }

        assert_eq!(
            RadrootsTradeMessageType::from_kind(KIND_TRADE_LISTING_VALIDATE_REQ),
            Some(RadrootsTradeMessageType::ListingValidateRequest)
        );
        assert_eq!(
            RadrootsTradeMessageType::from_kind(KIND_TRADE_LISTING_VALIDATE_RES),
            Some(RadrootsTradeMessageType::ListingValidateResult)
        );
        assert_eq!(
            RadrootsTradeMessageType::from_kind(KIND_TRADE_ORDER_REQUEST),
            Some(RadrootsTradeMessageType::OrderRequest)
        );
        assert_eq!(
            RadrootsTradeMessageType::from_kind(KIND_TRADE_ORDER_RESPONSE),
            Some(RadrootsTradeMessageType::OrderResponse)
        );
        assert_eq!(
            RadrootsTradeMessageType::from_kind(KIND_TRADE_ORDER_REVISION),
            Some(RadrootsTradeMessageType::OrderRevision)
        );
        assert_eq!(
            RadrootsTradeMessageType::from_kind(KIND_TRADE_ORDER_REVISION_RESPONSE),
            None
        );
        assert_eq!(
            RadrootsTradeMessageType::from_kind(KIND_TRADE_QUESTION),
            Some(RadrootsTradeMessageType::Question)
        );
        assert_eq!(
            RadrootsTradeMessageType::from_kind(KIND_TRADE_ANSWER),
            Some(RadrootsTradeMessageType::Answer)
        );
        assert_eq!(
            RadrootsTradeMessageType::from_kind(KIND_TRADE_DISCOUNT_REQUEST),
            Some(RadrootsTradeMessageType::DiscountRequest)
        );
        assert_eq!(
            RadrootsTradeMessageType::from_kind(KIND_TRADE_DISCOUNT_OFFER),
            Some(RadrootsTradeMessageType::DiscountOffer)
        );
        assert_eq!(
            RadrootsTradeMessageType::from_kind(KIND_TRADE_DISCOUNT_ACCEPT),
            Some(RadrootsTradeMessageType::DiscountAccept)
        );
        assert_eq!(
            RadrootsTradeMessageType::from_kind(KIND_TRADE_DISCOUNT_DECLINE),
            Some(RadrootsTradeMessageType::DiscountDecline)
        );
        assert_eq!(
            RadrootsTradeMessageType::from_kind(KIND_TRADE_CANCEL),
            Some(RadrootsTradeMessageType::Cancel)
        );
        assert_eq!(
            RadrootsTradeMessageType::from_kind(KIND_TRADE_FULFILLMENT_UPDATE),
            Some(RadrootsTradeMessageType::FulfillmentUpdate)
        );
        assert_eq!(
            RadrootsTradeMessageType::from_kind(KIND_TRADE_RECEIPT),
            Some(RadrootsTradeMessageType::Receipt)
        );
        assert_eq!(RadrootsTradeMessageType::from_kind(KIND_PROFILE), None);
    }

    #[test]
    fn envelope_requires_order_id_for_order_scoped_messages() {
        let envelope = RadrootsTradeEnvelope::new(
            RadrootsTradeMessageType::OrderRequest,
            sample_listing_addr(),
            None,
            RadrootsTradeMessagePayload::OrderRequest(sample_order()),
        );
        assert_eq!(
            envelope.validate().unwrap_err(),
            RadrootsTradeEnvelopeError::MissingOrderId
        );
    }

    #[test]
    fn envelope_validation_covers_success_and_error_paths() {
        let service_envelope = RadrootsTradeEnvelope::new(
            RadrootsTradeMessageType::ListingValidateRequest,
            sample_listing_addr(),
            None,
            RadrootsTradeMessagePayload::ListingValidateRequest(sample_validate_request()),
        );
        assert_eq!(service_envelope.validate(), Ok(()));

        let public_envelope = RadrootsTradeEnvelope::new(
            RadrootsTradeMessageType::OrderRequest,
            sample_listing_addr(),
            Some("order-1".into()),
            RadrootsTradeMessagePayload::OrderRequest(sample_order()),
        );
        assert_eq!(public_envelope.validate(), Ok(()));

        let invalid_version = RadrootsTradeEnvelope {
            version: RADROOTS_TRADE_ENVELOPE_VERSION + 1,
            domain: RadrootsTradeDomain::TradeListing,
            message_type: RadrootsTradeMessageType::OrderRequest,
            order_id: Some("order-1".into()),
            listing_addr: sample_listing_addr(),
            payload: RadrootsTradeMessagePayload::OrderRequest(sample_order()),
        };
        assert_eq!(
            invalid_version.validate().unwrap_err(),
            RadrootsTradeEnvelopeError::InvalidVersion {
                expected: RADROOTS_TRADE_ENVELOPE_VERSION,
                got: RADROOTS_TRADE_ENVELOPE_VERSION + 1,
            }
        );

        let missing_listing_addr = RadrootsTradeEnvelope::new(
            RadrootsTradeMessageType::ListingValidateRequest,
            "   ",
            None,
            RadrootsTradeMessagePayload::ListingValidateRequest(sample_validate_request()),
        );
        assert_eq!(
            missing_listing_addr.validate().unwrap_err(),
            RadrootsTradeEnvelopeError::MissingListingAddr
        );

        let blank_order_id = RadrootsTradeEnvelope::new(
            RadrootsTradeMessageType::OrderResponse,
            sample_listing_addr(),
            Some("  ".into()),
            RadrootsTradeMessagePayload::OrderResponse(sample_order_response(true)),
        );
        assert_eq!(
            blank_order_id.validate().unwrap_err(),
            RadrootsTradeEnvelopeError::MissingOrderId
        );
    }

    #[test]
    fn envelope_error_display_variants() {
        assert_eq!(
            (RadrootsTradeEnvelopeError::InvalidVersion {
                expected: 1,
                got: 2,
            })
            .to_string(),
            "invalid envelope version: expected 1, got 2"
        );
        assert_eq!(
            RadrootsTradeEnvelopeError::MissingOrderId.to_string(),
            "missing order_id for order-scoped message"
        );
        assert_eq!(
            RadrootsTradeEnvelopeError::MissingListingAddr.to_string(),
            "missing listing_addr"
        );
    }

    #[test]
    fn payload_message_type_covers_all_variants() {
        let payloads = [
            (
                RadrootsTradeMessagePayload::ListingValidateRequest(sample_validate_request()),
                RadrootsTradeMessageType::ListingValidateRequest,
            ),
            (
                RadrootsTradeMessagePayload::ListingValidateResult(sample_validate_result()),
                RadrootsTradeMessageType::ListingValidateResult,
            ),
            (
                RadrootsTradeMessagePayload::OrderRequest(sample_order()),
                RadrootsTradeMessageType::OrderRequest,
            ),
            (
                RadrootsTradeMessagePayload::OrderResponse(sample_order_response(false)),
                RadrootsTradeMessageType::OrderResponse,
            ),
            (
                RadrootsTradeMessagePayload::OrderRevision(sample_order_revision()),
                RadrootsTradeMessageType::OrderRevision,
            ),
            (
                RadrootsTradeMessagePayload::OrderRevisionAccept(sample_order_revision_response(
                    true,
                )),
                RadrootsTradeMessageType::OrderRevisionAccept,
            ),
            (
                RadrootsTradeMessagePayload::OrderRevisionDecline(sample_order_revision_response(
                    false,
                )),
                RadrootsTradeMessageType::OrderRevisionDecline,
            ),
            (
                RadrootsTradeMessagePayload::Question(RadrootsTradeQuestion {
                    question_id: "question-1".into(),
                }),
                RadrootsTradeMessageType::Question,
            ),
            (
                RadrootsTradeMessagePayload::Answer(RadrootsTradeAnswer {
                    question_id: "question-1".into(),
                }),
                RadrootsTradeMessageType::Answer,
            ),
            (
                RadrootsTradeMessagePayload::DiscountRequest(RadrootsTradeDiscountRequest {
                    discount_id: "discount-1".into(),
                    value: sample_discount_value(),
                }),
                RadrootsTradeMessageType::DiscountRequest,
            ),
            (
                RadrootsTradeMessagePayload::DiscountOffer(RadrootsTradeDiscountOffer {
                    discount_id: "discount-1".into(),
                    value: sample_percent_discount(),
                }),
                RadrootsTradeMessageType::DiscountOffer,
            ),
            (
                RadrootsTradeMessagePayload::DiscountAccept(
                    RadrootsTradeDiscountDecision::Accept {
                        value: sample_discount_value(),
                    },
                ),
                RadrootsTradeMessageType::DiscountAccept,
            ),
            (
                RadrootsTradeMessagePayload::DiscountDecline(
                    RadrootsTradeDiscountDecision::Decline {
                        reason: Some("no thanks".into()),
                    },
                ),
                RadrootsTradeMessageType::DiscountDecline,
            ),
            (
                RadrootsTradeMessagePayload::Cancel(RadrootsTradeListingCancel {
                    reason: Some("out of stock".into()),
                }),
                RadrootsTradeMessageType::Cancel,
            ),
            (
                RadrootsTradeMessagePayload::FulfillmentUpdate(RadrootsTradeFulfillmentUpdate {
                    status: RadrootsTradeFulfillmentStatus::Delivered,
                }),
                RadrootsTradeMessageType::FulfillmentUpdate,
            ),
            (
                RadrootsTradeMessagePayload::Receipt(RadrootsTradeReceipt {
                    acknowledged: true,
                    at: 42,
                }),
                RadrootsTradeMessageType::Receipt,
            ),
        ];

        for (payload, message_type) in payloads {
            assert_eq!(payload.message_type(), message_type);
        }
    }
}
