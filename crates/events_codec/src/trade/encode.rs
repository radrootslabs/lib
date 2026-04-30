#[cfg(all(not(feature = "std"), feature = "serde_json"))]
use alloc::string::String;

#[cfg(feature = "serde_json")]
use radroots_events::{
    RadrootsNostrEventPtr,
    trade::{
        RadrootsActiveTradeEnvelope, RadrootsActiveTradeEnvelopeError,
        RadrootsActiveTradeMessageType, RadrootsActiveTradePayloadError, RadrootsTradeBuyerReceipt,
        RadrootsTradeEnvelope, RadrootsTradeEnvelopeError, RadrootsTradeFulfillmentUpdated,
        RadrootsTradeMessagePayload, RadrootsTradeMessageType, RadrootsTradeOrderCancelled,
        RadrootsTradeOrderDecisionEvent, RadrootsTradeOrderRequested,
        RadrootsTradeOrderRevisionProposed,
    },
};

#[cfg(feature = "serde_json")]
use crate::{error::EventEncodeError, trade::tags::trade_envelope_tags, wire::WireEventParts};

#[cfg(feature = "serde_json")]
fn map_envelope_error(error: RadrootsTradeEnvelopeError) -> EventEncodeError {
    match error {
        RadrootsTradeEnvelopeError::MissingOrderId => {
            EventEncodeError::EmptyRequiredField("order_id")
        }
        RadrootsTradeEnvelopeError::MissingListingAddr => {
            EventEncodeError::EmptyRequiredField("listing_addr")
        }
        RadrootsTradeEnvelopeError::InvalidVersion { .. } => {
            EventEncodeError::InvalidField("version")
        }
    }
}

#[cfg(feature = "serde_json")]
fn map_active_envelope_error(error: RadrootsActiveTradeEnvelopeError) -> EventEncodeError {
    match error {
        RadrootsActiveTradeEnvelopeError::MissingOrderId => {
            EventEncodeError::EmptyRequiredField("order_id")
        }
        RadrootsActiveTradeEnvelopeError::MissingListingAddr => {
            EventEncodeError::EmptyRequiredField("listing_addr")
        }
        RadrootsActiveTradeEnvelopeError::InvalidVersion { .. } => {
            EventEncodeError::InvalidField("version")
        }
    }
}

#[cfg(feature = "serde_json")]
fn map_active_payload_error(error: RadrootsActiveTradePayloadError) -> EventEncodeError {
    match error {
        RadrootsActiveTradePayloadError::EmptyField(field) => {
            EventEncodeError::EmptyRequiredField(field)
        }
        RadrootsActiveTradePayloadError::MissingItems => {
            EventEncodeError::EmptyRequiredField("items")
        }
        RadrootsActiveTradePayloadError::InvalidItemBinCount { .. } => {
            EventEncodeError::InvalidField("items.bin_count")
        }
        RadrootsActiveTradePayloadError::MissingEconomicItems => {
            EventEncodeError::EmptyRequiredField("economics.items")
        }
        RadrootsActiveTradePayloadError::InvalidEconomicItemBinCount { .. } => {
            EventEncodeError::InvalidField("economics.items.bin_count")
        }
        RadrootsActiveTradePayloadError::InvalidEconomicItemQuantity { .. } => {
            EventEncodeError::InvalidField("economics.items.quantity_amount")
        }
        RadrootsActiveTradePayloadError::InvalidEconomicItemPrice { .. } => {
            EventEncodeError::InvalidField("economics.items.unit_price_amount")
        }
        RadrootsActiveTradePayloadError::InvalidEconomicItemSubtotal { .. } => {
            EventEncodeError::InvalidField("economics.items.line_subtotal")
        }
        RadrootsActiveTradePayloadError::InvalidEconomicLineAmount { field, .. }
        | RadrootsActiveTradePayloadError::InvalidEconomicLineKind { field, .. }
        | RadrootsActiveTradePayloadError::InvalidEconomicLineEffect { field, .. }
        | RadrootsActiveTradePayloadError::InvalidEconomicCurrency { field }
        | RadrootsActiveTradePayloadError::InvalidEconomicOrdering { field }
        | RadrootsActiveTradePayloadError::InvalidEconomicTotal { field }
        | RadrootsActiveTradePayloadError::InvalidOrderEconomicsBinding { field } => {
            EventEncodeError::InvalidField(field)
        }
        RadrootsActiveTradePayloadError::InvalidQuoteVersion => {
            EventEncodeError::InvalidField("economics.quote_version")
        }
        RadrootsActiveTradePayloadError::MissingInventoryCommitments => {
            EventEncodeError::EmptyRequiredField("inventory_commitments")
        }
        RadrootsActiveTradePayloadError::InvalidInventoryCommitmentCount { .. } => {
            EventEncodeError::InvalidField("inventory_commitments.bin_count")
        }
        RadrootsActiveTradePayloadError::InvalidFulfillmentStatus => {
            EventEncodeError::InvalidField("fulfillment.status")
        }
        RadrootsActiveTradePayloadError::MissingReceiptIssue => {
            EventEncodeError::EmptyRequiredField("receipt.issue")
        }
        RadrootsActiveTradePayloadError::UnexpectedReceiptIssue => {
            EventEncodeError::InvalidField("receipt.issue")
        }
    }
}

#[cfg(feature = "serde_json")]
pub fn trade_envelope_event_build(
    recipient_pubkey: impl Into<String>,
    message_type: RadrootsTradeMessageType,
    listing_addr: impl Into<String>,
    order_id: Option<String>,
    listing_event: Option<&RadrootsNostrEventPtr>,
    root_event_id: Option<&str>,
    prev_event_id: Option<&str>,
    payload: &RadrootsTradeMessagePayload,
) -> Result<WireEventParts, EventEncodeError> {
    if payload.message_type() != message_type {
        return Err(EventEncodeError::InvalidField("payload"));
    }
    if message_type.requires_listing_snapshot() && listing_event.is_none() {
        return Err(EventEncodeError::EmptyRequiredField("listing_event.id"));
    }
    if message_type.requires_trade_chain() {
        if root_event_id.is_none() {
            return Err(EventEncodeError::EmptyRequiredField("root_event_id"));
        }
        if prev_event_id.is_none() {
            return Err(EventEncodeError::EmptyRequiredField("prev_event_id"));
        }
    }

    let listing_addr = listing_addr.into();
    let envelope = RadrootsTradeEnvelope::new(
        message_type,
        listing_addr.clone(),
        order_id.clone(),
        payload.clone(),
    );
    envelope.validate().map_err(map_envelope_error)?;
    let content = serde_json::to_string(&envelope).map_err(|_| EventEncodeError::Json)?;
    let tags = trade_envelope_tags(
        recipient_pubkey,
        &listing_addr,
        order_id.as_deref(),
        listing_event,
        root_event_id,
        prev_event_id,
    )?;
    Ok(WireEventParts {
        kind: message_type.kind(),
        content,
        tags,
    })
}

#[cfg(feature = "serde_json")]
fn active_trade_envelope_event_build<T: serde::Serialize>(
    recipient_pubkey: &str,
    message_type: RadrootsActiveTradeMessageType,
    listing_addr: &str,
    order_id: &str,
    listing_event: Option<&RadrootsNostrEventPtr>,
    root_event_id: Option<&str>,
    prev_event_id: Option<&str>,
    payload: &T,
) -> Result<WireEventParts, EventEncodeError> {
    if message_type.requires_listing_snapshot() && listing_event.is_none() {
        return Err(EventEncodeError::EmptyRequiredField("listing_event.id"));
    }
    if message_type.requires_trade_chain() {
        if root_event_id.is_none() {
            return Err(EventEncodeError::EmptyRequiredField("root_event_id"));
        }
        if prev_event_id.is_none() {
            return Err(EventEncodeError::EmptyRequiredField("prev_event_id"));
        }
    }

    let envelope = RadrootsActiveTradeEnvelope::new(message_type, listing_addr, order_id, payload);
    envelope.validate().map_err(map_active_envelope_error)?;
    let content = serde_json::to_string(&envelope).map_err(|_| EventEncodeError::Json)?;
    let tags = trade_envelope_tags(
        recipient_pubkey,
        listing_addr,
        Some(order_id),
        listing_event,
        root_event_id,
        prev_event_id,
    )?;
    Ok(WireEventParts {
        kind: message_type.kind(),
        content,
        tags,
    })
}

#[cfg(feature = "serde_json")]
pub fn active_trade_order_request_event_build(
    listing_event: &RadrootsNostrEventPtr,
    payload: &RadrootsTradeOrderRequested,
) -> Result<WireEventParts, EventEncodeError> {
    payload.validate().map_err(map_active_payload_error)?;
    active_trade_envelope_event_build(
        &payload.seller_pubkey,
        RadrootsActiveTradeMessageType::TradeOrderRequested,
        &payload.listing_addr,
        &payload.order_id,
        Some(listing_event),
        None,
        None,
        payload,
    )
}

#[cfg(feature = "serde_json")]
pub fn active_trade_order_decision_event_build(
    root_event_id: &str,
    prev_event_id: &str,
    payload: &RadrootsTradeOrderDecisionEvent,
) -> Result<WireEventParts, EventEncodeError> {
    payload.validate().map_err(map_active_payload_error)?;
    active_trade_envelope_event_build(
        &payload.buyer_pubkey,
        RadrootsActiveTradeMessageType::TradeOrderDecision,
        &payload.listing_addr,
        &payload.order_id,
        None,
        Some(root_event_id),
        Some(prev_event_id),
        payload,
    )
}

#[cfg(feature = "serde_json")]
pub fn active_trade_order_revision_proposal_event_build(
    root_event_id: &str,
    prev_event_id: &str,
    payload: &RadrootsTradeOrderRevisionProposed,
) -> Result<WireEventParts, EventEncodeError> {
    payload.validate().map_err(map_active_payload_error)?;
    if payload.root_event_id != root_event_id {
        return Err(EventEncodeError::InvalidField("root_event_id"));
    }
    if payload.prev_event_id != prev_event_id {
        return Err(EventEncodeError::InvalidField("prev_event_id"));
    }
    active_trade_envelope_event_build(
        &payload.buyer_pubkey,
        RadrootsActiveTradeMessageType::TradeOrderRevisionProposed,
        &payload.listing_addr,
        &payload.order_id,
        None,
        Some(root_event_id),
        Some(prev_event_id),
        payload,
    )
}

#[cfg(feature = "serde_json")]
pub fn active_trade_fulfillment_update_event_build(
    root_event_id: &str,
    prev_event_id: &str,
    payload: &RadrootsTradeFulfillmentUpdated,
) -> Result<WireEventParts, EventEncodeError> {
    payload.validate().map_err(map_active_payload_error)?;
    active_trade_envelope_event_build(
        &payload.buyer_pubkey,
        RadrootsActiveTradeMessageType::TradeFulfillmentUpdated,
        &payload.listing_addr,
        &payload.order_id,
        None,
        Some(root_event_id),
        Some(prev_event_id),
        payload,
    )
}

#[cfg(feature = "serde_json")]
pub fn active_trade_order_cancel_event_build(
    root_event_id: &str,
    prev_event_id: &str,
    payload: &RadrootsTradeOrderCancelled,
) -> Result<WireEventParts, EventEncodeError> {
    payload.validate().map_err(map_active_payload_error)?;
    active_trade_envelope_event_build(
        &payload.seller_pubkey,
        RadrootsActiveTradeMessageType::TradeOrderCancelled,
        &payload.listing_addr,
        &payload.order_id,
        None,
        Some(root_event_id),
        Some(prev_event_id),
        payload,
    )
}

#[cfg(feature = "serde_json")]
pub fn active_trade_buyer_receipt_event_build(
    root_event_id: &str,
    prev_event_id: &str,
    payload: &RadrootsTradeBuyerReceipt,
) -> Result<WireEventParts, EventEncodeError> {
    payload.validate().map_err(map_active_payload_error)?;
    active_trade_envelope_event_build(
        &payload.seller_pubkey,
        RadrootsActiveTradeMessageType::TradeBuyerReceipt,
        &payload.listing_addr,
        &payload.order_id,
        None,
        Some(root_event_id),
        Some(prev_event_id),
        payload,
    )
}
