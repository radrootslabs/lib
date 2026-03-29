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
    InvalidKind { kind: u32 },
    MissingListingId,
    ListingEventNotFound { listing_addr: String },
    ListingEventFetchFailed { listing_addr: String },
    ParseError { error: RadrootsTradeListingParseError },
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
    pub order_id: String,
    pub changes: Vec<RadrootsTradeOrderChange>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub reason: Option<String>,
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
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub notes: Option<String>,
    pub status: RadrootsTradeOrderStatus,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeQuestion {
    pub question_id: String,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub order_id: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub listing_addr: Option<String>,
    pub question_text: String,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeAnswer {
    pub question_id: String,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub order_id: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub listing_addr: Option<String>,
    pub answer_text: String,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeDiscountRequest {
    pub discount_id: String,
    pub order_id: String,
    #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsCoreDiscountValue"))]
    pub value: RadrootsCoreDiscountValue,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub conditions: Option<String>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeDiscountOffer {
    pub discount_id: String,
    pub order_id: String,
    #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsCoreDiscountValue"))]
    pub value: RadrootsCoreDiscountValue,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub conditions: Option<String>,
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
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub tracking: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub eta: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub notes: Option<String>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeReceipt {
    pub acknowledged: bool,
    pub at: u64,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub note: Option<String>,
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
    #[cfg_attr(
        feature = "ts-rs",
        ts(type = "RadrootsTradeListingValidationError[]")
    )]
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
            KIND_TRADE_LISTING_ORDER_REQ => Some(Self::OrderRequest),
            KIND_TRADE_LISTING_ORDER_RES => Some(Self::OrderResponse),
            KIND_TRADE_LISTING_ORDER_REVISION_REQ => Some(Self::OrderRevision),
            KIND_TRADE_LISTING_ORDER_REVISION_RES => None,
            KIND_TRADE_LISTING_QUESTION_REQ => Some(Self::Question),
            KIND_TRADE_LISTING_ANSWER_RES => Some(Self::Answer),
            KIND_TRADE_LISTING_DISCOUNT_REQ => Some(Self::DiscountRequest),
            KIND_TRADE_LISTING_DISCOUNT_OFFER_RES => Some(Self::DiscountOffer),
            KIND_TRADE_LISTING_DISCOUNT_ACCEPT_REQ => Some(Self::DiscountAccept),
            KIND_TRADE_LISTING_DISCOUNT_DECLINE_REQ => Some(Self::DiscountDecline),
            KIND_TRADE_LISTING_CANCEL_REQ => Some(Self::Cancel),
            KIND_TRADE_LISTING_FULFILLMENT_UPDATE_REQ => Some(Self::FulfillmentUpdate),
            KIND_TRADE_LISTING_RECEIPT_REQ => Some(Self::Receipt),
            _ => None,
        }
    }

    #[inline]
    pub const fn kind(self) -> u32 {
        match self {
            Self::ListingValidateRequest => KIND_TRADE_LISTING_VALIDATE_REQ,
            Self::ListingValidateResult => KIND_TRADE_LISTING_VALIDATE_RES,
            Self::OrderRequest => KIND_TRADE_LISTING_ORDER_REQ,
            Self::OrderResponse => KIND_TRADE_LISTING_ORDER_RES,
            Self::OrderRevision => KIND_TRADE_LISTING_ORDER_REVISION_REQ,
            Self::OrderRevisionAccept => KIND_TRADE_LISTING_ORDER_REVISION_RES,
            Self::OrderRevisionDecline => KIND_TRADE_LISTING_ORDER_REVISION_RES,
            Self::Question => KIND_TRADE_LISTING_QUESTION_REQ,
            Self::Answer => KIND_TRADE_LISTING_ANSWER_RES,
            Self::DiscountRequest => KIND_TRADE_LISTING_DISCOUNT_REQ,
            Self::DiscountOffer => KIND_TRADE_LISTING_DISCOUNT_OFFER_RES,
            Self::DiscountAccept => KIND_TRADE_LISTING_DISCOUNT_ACCEPT_REQ,
            Self::DiscountDecline => KIND_TRADE_LISTING_DISCOUNT_DECLINE_REQ,
            Self::Cancel => KIND_TRADE_LISTING_CANCEL_REQ,
            Self::FulfillmentUpdate => KIND_TRADE_LISTING_FULFILLMENT_UPDATE_REQ,
            Self::Receipt => KIND_TRADE_LISTING_RECEIPT_REQ,
        }
    }

    #[inline]
    pub const fn requires_order_id(self) -> bool {
        !matches!(self, Self::ListingValidateRequest | Self::ListingValidateResult)
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
                write!(f, "invalid envelope version: expected {expected}, got {got}")
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
                notes: None,
                status: RadrootsTradeOrderStatus::Requested,
            }),
        );
        assert_eq!(
            envelope.validate().unwrap_err(),
            RadrootsTradeEnvelopeError::MissingOrderId
        );
    }
}
