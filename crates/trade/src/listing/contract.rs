#![forbid(unsafe_code)]

#[allow(unused_imports)]
#[cfg(feature = "serde_json")]
use radroots_events::RadrootsNostrEvent;
#[allow(unused_imports)]
pub(crate) use radroots_events::{
    kinds::{
        KIND_TRADE_LISTING_ANSWER_RES, KIND_TRADE_LISTING_CANCEL_REQ,
        KIND_TRADE_LISTING_DISCOUNT_ACCEPT_REQ, KIND_TRADE_LISTING_DISCOUNT_DECLINE_REQ,
        KIND_TRADE_LISTING_DISCOUNT_OFFER_RES, KIND_TRADE_LISTING_DISCOUNT_REQ,
        KIND_TRADE_LISTING_FULFILLMENT_UPDATE_REQ, KIND_TRADE_LISTING_ORDER_REQ,
        KIND_TRADE_LISTING_ORDER_RES, KIND_TRADE_LISTING_ORDER_REVISION_REQ,
        KIND_TRADE_LISTING_ORDER_REVISION_RES, KIND_TRADE_LISTING_QUESTION_REQ,
        KIND_TRADE_LISTING_RECEIPT_REQ, KIND_TRADE_LISTING_VALIDATE_REQ,
        KIND_TRADE_LISTING_VALIDATE_RES, TRADE_LISTING_KINDS, is_trade_listing_kind,
    },
    trade::{
        RADROOTS_TRADE_ENVELOPE_VERSION as TRADE_LISTING_ENVELOPE_VERSION,
        RADROOTS_TRADE_LISTING_DOMAIN as TRADE_LISTING_DOMAIN, RadrootsTradeAnswer as TradeAnswer,
        RadrootsTradeDiscountDecision as TradeDiscountDecision,
        RadrootsTradeDiscountOffer as TradeDiscountOffer,
        RadrootsTradeDiscountRequest as TradeDiscountRequest,
        RadrootsTradeEnvelope as TradeListingEnvelope,
        RadrootsTradeEnvelopeError as TradeListingEnvelopeError,
        RadrootsTradeFulfillmentStatus as TradeFulfillmentStatus,
        RadrootsTradeFulfillmentUpdate as TradeFulfillmentUpdate,
        RadrootsTradeListingCancel as TradeListingCancel,
        RadrootsTradeListingParseError as TradeListingParseError,
        RadrootsTradeListingValidateRequest as TradeListingValidateRequest,
        RadrootsTradeListingValidateResult as TradeListingValidateResult,
        RadrootsTradeListingValidationError as TradeListingValidationError,
        RadrootsTradeMessagePayload as TradeListingMessagePayload,
        RadrootsTradeMessageType as TradeListingMessageType, RadrootsTradeOrder as TradeOrder,
        RadrootsTradeOrderChange as TradeOrderChange, RadrootsTradeOrderItem as TradeOrderItem,
        RadrootsTradeOrderResponse as TradeOrderResponse,
        RadrootsTradeOrderRevision as TradeOrderRevision,
        RadrootsTradeOrderRevisionResponse as TradeOrderRevisionResponse,
        RadrootsTradeOrderStatus as TradeOrderStatus, RadrootsTradeQuestion as TradeQuestion,
        RadrootsTradeReceipt as TradeReceipt,
    },
};
#[allow(unused_imports)]
#[cfg(feature = "serde_json")]
pub(crate) use radroots_events_codec::trade::{
    decode::{
        RadrootsTradeEnvelopeParseError as TradeListingEnvelopeParseError,
        RadrootsTradeListingAddress as TradeListingAddress,
        RadrootsTradeListingAddressError as TradeListingAddressError, trade_envelope_from_event,
    },
    encode::trade_envelope_event_build as trade_listing_envelope_event_build,
};

#[cfg(feature = "serde_json")]
use serde::de::DeserializeOwned;

#[cfg(feature = "serde_json")]
pub(crate) fn trade_listing_envelope_from_event<T: DeserializeOwned>(
    event: &RadrootsNostrEvent,
) -> Result<TradeListingEnvelope<T>, TradeListingEnvelopeParseError> {
    trade_envelope_from_event(event)
}
