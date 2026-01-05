#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use radroots_events::{RadrootsNostrEventPtr, kinds::KIND_PROFILE};
#[cfg(feature = "ts-rs")]
use ts_rs::TS;

use crate::listing::dvm_kinds::{
    KIND_TRADE_LISTING_ANSWER_RES, KIND_TRADE_LISTING_CANCEL_REQ,
    KIND_TRADE_LISTING_DISCOUNT_ACCEPT_REQ, KIND_TRADE_LISTING_DISCOUNT_DECLINE_REQ,
    KIND_TRADE_LISTING_DISCOUNT_OFFER_RES, KIND_TRADE_LISTING_DISCOUNT_REQ,
    KIND_TRADE_LISTING_FULFILLMENT_UPDATE_REQ, KIND_TRADE_LISTING_ORDER_REQ,
    KIND_TRADE_LISTING_ORDER_RES, KIND_TRADE_LISTING_ORDER_REVISION_REQ,
    KIND_TRADE_LISTING_ORDER_REVISION_RES, KIND_TRADE_LISTING_QUESTION_REQ,
    KIND_TRADE_LISTING_RECEIPT_REQ, KIND_TRADE_LISTING_VALIDATE_REQ,
    KIND_TRADE_LISTING_VALIDATE_RES,
};
use crate::listing::order::{
    TradeAnswer, TradeDiscountDecision, TradeDiscountOffer, TradeDiscountRequest, TradeFulfillmentUpdate,
    TradeOrder, TradeOrderRevision, TradeQuestion, TradeReceipt,
};
use crate::listing::validation::TradeListingValidationError;

pub const TRADE_LISTING_DOMAIN: &str = "trade:listing";
pub const TRADE_LISTING_ENVELOPE_VERSION: u16 = 1;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TradeListingDomain {
    #[cfg_attr(feature = "serde", serde(rename = "trade:listing"))]
    TradeListing,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TradeListingMessageType {
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

impl TradeListingMessageType {
    #[inline]
    pub const fn kind(self) -> u16 {
        match self {
            TradeListingMessageType::ListingValidateRequest => KIND_TRADE_LISTING_VALIDATE_REQ,
            TradeListingMessageType::ListingValidateResult => KIND_TRADE_LISTING_VALIDATE_RES,
            TradeListingMessageType::OrderRequest => KIND_TRADE_LISTING_ORDER_REQ,
            TradeListingMessageType::OrderResponse => KIND_TRADE_LISTING_ORDER_RES,
            TradeListingMessageType::OrderRevision => KIND_TRADE_LISTING_ORDER_REVISION_REQ,
            TradeListingMessageType::OrderRevisionAccept => KIND_TRADE_LISTING_ORDER_REVISION_RES,
            TradeListingMessageType::OrderRevisionDecline => KIND_TRADE_LISTING_ORDER_REVISION_RES,
            TradeListingMessageType::Question => KIND_TRADE_LISTING_QUESTION_REQ,
            TradeListingMessageType::Answer => KIND_TRADE_LISTING_ANSWER_RES,
            TradeListingMessageType::DiscountRequest => KIND_TRADE_LISTING_DISCOUNT_REQ,
            TradeListingMessageType::DiscountOffer => KIND_TRADE_LISTING_DISCOUNT_OFFER_RES,
            TradeListingMessageType::DiscountAccept => KIND_TRADE_LISTING_DISCOUNT_ACCEPT_REQ,
            TradeListingMessageType::DiscountDecline => KIND_TRADE_LISTING_DISCOUNT_DECLINE_REQ,
            TradeListingMessageType::Cancel => KIND_TRADE_LISTING_CANCEL_REQ,
            TradeListingMessageType::FulfillmentUpdate => KIND_TRADE_LISTING_FULFILLMENT_UPDATE_REQ,
            TradeListingMessageType::Receipt => KIND_TRADE_LISTING_RECEIPT_REQ,
        }
    }

    #[inline]
    pub const fn requires_order_id(self) -> bool {
        !matches!(
            self,
            TradeListingMessageType::ListingValidateRequest
                | TradeListingMessageType::ListingValidateResult
        )
    }

    #[inline]
    pub const fn is_request(self) -> bool {
        matches!(
            self,
            TradeListingMessageType::ListingValidateRequest
                | TradeListingMessageType::OrderRequest
                | TradeListingMessageType::OrderRevision
                | TradeListingMessageType::Question
                | TradeListingMessageType::DiscountRequest
                | TradeListingMessageType::DiscountAccept
                | TradeListingMessageType::DiscountDecline
                | TradeListingMessageType::Cancel
                | TradeListingMessageType::FulfillmentUpdate
                | TradeListingMessageType::Receipt
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
pub struct TradeListingEnvelope<T> {
    pub version: u16,
    pub domain: TradeListingDomain,
    #[cfg_attr(feature = "serde", serde(rename = "type"))]
    pub message_type: TradeListingMessageType,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub order_id: Option<String>,
    pub listing_addr: String,
    pub payload: T,
}

impl<T> TradeListingEnvelope<T> {
    #[inline]
    pub fn new(
        message_type: TradeListingMessageType,
        listing_addr: impl Into<String>,
        order_id: Option<String>,
        payload: T,
    ) -> Self {
        Self {
            version: TRADE_LISTING_ENVELOPE_VERSION,
            domain: TradeListingDomain::TradeListing,
            message_type,
            order_id,
            listing_addr: listing_addr.into(),
            payload,
        }
    }

    pub fn validate(&self) -> Result<(), TradeListingEnvelopeError> {
        if self.version != TRADE_LISTING_ENVELOPE_VERSION {
            return Err(TradeListingEnvelopeError::InvalidVersion {
                expected: TRADE_LISTING_ENVELOPE_VERSION,
                got: self.version,
            });
        }
        if self.listing_addr.trim().is_empty() {
            return Err(TradeListingEnvelopeError::MissingListingAddr);
        }
        if self.message_type.requires_order_id() {
            match self.order_id.as_deref() {
                Some(id) if !id.trim().is_empty() => {}
                _ => return Err(TradeListingEnvelopeError::MissingOrderId),
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TradeListingEnvelopeError {
    InvalidVersion { expected: u16, got: u16 },
    MissingOrderId,
    MissingListingAddr,
}

impl core::fmt::Display for TradeListingEnvelopeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            TradeListingEnvelopeError::InvalidVersion { expected, got } => {
                write!(f, "invalid envelope version: expected {expected}, got {got}")
            }
            TradeListingEnvelopeError::MissingOrderId => {
                write!(f, "missing order_id for order-scoped message")
            }
            TradeListingEnvelopeError::MissingListingAddr => {
                write!(f, "missing listing_addr")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for TradeListingEnvelopeError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TradeListingAddress {
    pub kind: u16,
    pub seller_pubkey: String,
    pub listing_id: String,
}

impl TradeListingAddress {
    pub fn parse(addr: &str) -> Result<Self, TradeListingAddressError> {
        let mut parts = addr.split(':');
        let kind = parts
            .next()
            .ok_or(TradeListingAddressError::InvalidFormat)?
            .parse::<u16>()
            .map_err(|_| TradeListingAddressError::InvalidFormat)?;
        let seller_pubkey = parts
            .next()
            .ok_or(TradeListingAddressError::InvalidFormat)?
            .to_string();
        let listing_id = parts
            .next()
            .ok_or(TradeListingAddressError::InvalidFormat)?
            .to_string();
        if parts.next().is_some() {
            return Err(TradeListingAddressError::InvalidFormat);
        }
        if kind == KIND_PROFILE as u16
            || seller_pubkey.trim().is_empty()
            || listing_id.trim().is_empty()
        {
            return Err(TradeListingAddressError::InvalidFormat);
        }
        Ok(Self {
            kind,
            seller_pubkey,
            listing_id,
        })
    }

    #[inline]
    pub fn as_str(&self) -> String {
        format!("{}:{}:{}", self.kind, self.seller_pubkey, self.listing_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TradeListingAddressError {
    InvalidFormat,
}

impl core::fmt::Display for TradeListingAddressError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            TradeListingAddressError::InvalidFormat => {
                write!(f, "invalid listing address format")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for TradeListingAddressError {}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TradeListingValidateRequest {
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "RadrootsNostrEventPtr | null"))]
    pub listing_event: Option<RadrootsNostrEventPtr>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TradeListingValidateResult {
    pub valid: bool,
    #[cfg_attr(feature = "ts-rs", ts(type = "TradeListingValidationError[]"))]
    pub errors: Vec<TradeListingValidationError>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TradeOrderResponse {
    pub accepted: bool,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub reason: Option<String>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TradeOrderRevisionResponse {
    pub accepted: bool,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub reason: Option<String>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TradeListingCancel {
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub reason: Option<String>,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case", tag = "kind", content = "amount"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TradeListingMessagePayload {
    ListingValidateRequest(TradeListingValidateRequest),
    ListingValidateResult(TradeListingValidateResult),
    OrderRequest(TradeOrder),
    OrderResponse(TradeOrderResponse),
    OrderRevision(TradeOrderRevision),
    OrderRevisionAccept(TradeOrderRevisionResponse),
    OrderRevisionDecline(TradeOrderRevisionResponse),
    Question(TradeQuestion),
    Answer(TradeAnswer),
    DiscountRequest(TradeDiscountRequest),
    DiscountOffer(TradeDiscountOffer),
    DiscountAccept(TradeDiscountDecision),
    DiscountDecline(TradeDiscountDecision),
    Cancel(TradeListingCancel),
    FulfillmentUpdate(TradeFulfillmentUpdate),
    Receipt(TradeReceipt),
}

#[cfg(test)]
mod tests {
    use super::{
        TradeListingEnvelope, TradeListingEnvelopeError, TradeListingMessageType,
        TradeListingValidateRequest,
    };
    use radroots_events::kinds::KIND_LISTING;

    #[test]
    fn envelope_requires_listing_addr() {
        let env = TradeListingEnvelope::new(
            TradeListingMessageType::ListingValidateRequest,
            "",
            None,
            TradeListingValidateRequest { listing_event: None },
        );
        assert_eq!(
            env.validate().unwrap_err(),
            TradeListingEnvelopeError::MissingListingAddr
        );
    }

    #[test]
    fn envelope_requires_order_id_for_order_scoped() {
        let env = TradeListingEnvelope::new(
            TradeListingMessageType::OrderRequest,
            format!("{KIND_LISTING}:pubkey:AAAAAAAAAAAAAAAAAAAAAg"),
            None,
            TradeListingValidateRequest { listing_event: None },
        );
        assert_eq!(
            env.validate().unwrap_err(),
            TradeListingEnvelopeError::MissingOrderId
        );
    }
}
