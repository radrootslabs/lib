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
    fn envelope_requires_order_id_for_order_scoped_messages() {
        let envelope = RadrootsTradeEnvelope::new(
            RadrootsTradeMessageType::OrderRequest,
            "30402:pubkey:AAAAAAAAAAAAAAAAAAAAAg",
            None,
            RadrootsTradeMessagePayload::OrderRequest(RadrootsTradeOrder {
                order_id: "order-1".into(),
                listing_addr: "30402:pubkey:AAAAAAAAAAAAAAAAAAAAAg".into(),
                buyer_pubkey: "buyer".into(),
                seller_pubkey: "seller".into(),
                items: vec![],
                discounts: None,
            }),
        );
        assert_eq!(
            envelope.validate().unwrap_err(),
            RadrootsTradeEnvelopeError::MissingOrderId
        );
    }
}
