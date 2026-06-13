#[cfg(all(not(feature = "std"), feature = "serde_json"))]
use alloc::string::String;

#[cfg(feature = "serde_json")]
use radroots_events::{
    RadrootsNostrEventPtr,
    ids::RadrootsEventId,
    order::{
        RadrootsOrderCancellation, RadrootsOrderDecision, RadrootsOrderEnvelope,
        RadrootsOrderEnvelopeError, RadrootsOrderEventType, RadrootsOrderFulfillmentUpdate,
        RadrootsOrderPayloadError, RadrootsOrderPaymentRecord, RadrootsOrderReceipt,
        RadrootsOrderRequest, RadrootsOrderRevisionDecision, RadrootsOrderRevisionProposal,
        RadrootsOrderSettlementDecision,
    },
};

#[cfg(feature = "serde_json")]
use crate::{error::EventEncodeError, order::tags::order_envelope_tags, wire::WireEventParts};

#[cfg(feature = "serde_json")]
fn map_order_envelope_error(error: RadrootsOrderEnvelopeError) -> EventEncodeError {
    match error {
        RadrootsOrderEnvelopeError::MissingOrderId => {
            EventEncodeError::EmptyRequiredField("order_id")
        }
        RadrootsOrderEnvelopeError::MissingListingAddr => {
            EventEncodeError::EmptyRequiredField("listing_addr")
        }
        RadrootsOrderEnvelopeError::InvalidVersion { .. } => {
            EventEncodeError::InvalidField("version")
        }
    }
}

#[cfg(feature = "serde_json")]
fn map_order_payload_error(error: RadrootsOrderPayloadError) -> EventEncodeError {
    match error {
        RadrootsOrderPayloadError::EmptyField(field) => EventEncodeError::EmptyRequiredField(field),
        RadrootsOrderPayloadError::MissingItems => EventEncodeError::EmptyRequiredField("items"),
        RadrootsOrderPayloadError::InvalidItemBinCount { .. } => {
            EventEncodeError::InvalidField("items.bin_count")
        }
        RadrootsOrderPayloadError::MissingEconomicItems => {
            EventEncodeError::EmptyRequiredField("economics.items")
        }
        RadrootsOrderPayloadError::InvalidEconomicItemBinCount { .. } => {
            EventEncodeError::InvalidField("economics.items.bin_count")
        }
        RadrootsOrderPayloadError::InvalidEconomicItemQuantity { .. } => {
            EventEncodeError::InvalidField("economics.items.quantity_amount")
        }
        RadrootsOrderPayloadError::InvalidEconomicItemPrice { .. } => {
            EventEncodeError::InvalidField("economics.items.unit_price_amount")
        }
        RadrootsOrderPayloadError::InvalidEconomicItemSubtotal { .. } => {
            EventEncodeError::InvalidField("economics.items.line_subtotal")
        }
        RadrootsOrderPayloadError::InvalidEconomicLineAmount { field, .. }
        | RadrootsOrderPayloadError::InvalidEconomicLineKind { field, .. }
        | RadrootsOrderPayloadError::InvalidEconomicLineEffect { field, .. }
        | RadrootsOrderPayloadError::InvalidEconomicCurrency { field }
        | RadrootsOrderPayloadError::InvalidEconomicOrdering { field }
        | RadrootsOrderPayloadError::InvalidEconomicTotal { field }
        | RadrootsOrderPayloadError::InvalidOrderEconomicsBinding { field } => {
            EventEncodeError::InvalidField(field)
        }
        RadrootsOrderPayloadError::InvalidQuoteVersion => {
            EventEncodeError::InvalidField("economics.quote_version")
        }
        RadrootsOrderPayloadError::MissingInventoryCommitments => {
            EventEncodeError::EmptyRequiredField("inventory_commitments")
        }
        RadrootsOrderPayloadError::InvalidInventoryCommitmentCount { .. } => {
            EventEncodeError::InvalidField("inventory_commitments.bin_count")
        }
        RadrootsOrderPayloadError::InvalidFulfillmentStatus => {
            EventEncodeError::InvalidField("fulfillment.status")
        }
        RadrootsOrderPayloadError::MissingReceiptIssue => {
            EventEncodeError::EmptyRequiredField("receipt.issue")
        }
        RadrootsOrderPayloadError::UnexpectedReceiptIssue => {
            EventEncodeError::InvalidField("receipt.issue")
        }
        RadrootsOrderPayloadError::InvalidPaymentAmount => {
            EventEncodeError::InvalidField("payment.amount")
        }
        RadrootsOrderPayloadError::MissingSettlementReason => {
            EventEncodeError::EmptyRequiredField("settlement.reason")
        }
        RadrootsOrderPayloadError::UnexpectedSettlementReason => {
            EventEncodeError::InvalidField("settlement.reason")
        }
    }
}

#[cfg(feature = "serde_json")]
fn order_envelope_event_build<T: serde::Serialize>(
    recipient_pubkey: &str,
    message_type: RadrootsOrderEventType,
    listing_addr: &str,
    order_id: &str,
    listing_event: Option<&RadrootsNostrEventPtr>,
    root_event_id: Option<&RadrootsEventId>,
    prev_event_id: Option<&RadrootsEventId>,
    payload: &T,
) -> Result<WireEventParts, EventEncodeError> {
    if message_type.requires_listing_snapshot() && listing_event.is_none() {
        return Err(EventEncodeError::EmptyRequiredField("listing_event.id"));
    }
    if message_type.requires_order_chain() {
        if root_event_id.is_none() {
            return Err(EventEncodeError::EmptyRequiredField("root_event_id"));
        }
        if prev_event_id.is_none() {
            return Err(EventEncodeError::EmptyRequiredField("prev_event_id"));
        }
    }

    let envelope = RadrootsOrderEnvelope::new(message_type, listing_addr, order_id, payload);
    envelope.validate().map_err(map_order_envelope_error)?;
    let content = serde_json::to_string(&envelope).map_err(|_| EventEncodeError::Json)?;
    let tags = order_envelope_tags(
        recipient_pubkey,
        listing_addr,
        Some(order_id),
        listing_event,
        root_event_id.map(RadrootsEventId::as_str),
        prev_event_id.map(RadrootsEventId::as_str),
    )?;
    Ok(WireEventParts {
        kind: message_type.kind(),
        content,
        tags,
    })
}

#[cfg(feature = "serde_json")]
pub fn order_request_event_build(
    listing_event: &RadrootsNostrEventPtr,
    payload: &RadrootsOrderRequest,
) -> Result<WireEventParts, EventEncodeError> {
    payload.validate().map_err(map_order_payload_error)?;
    order_envelope_event_build(
        &payload.seller_pubkey,
        RadrootsOrderEventType::OrderRequested,
        &payload.listing_addr,
        &payload.order_id,
        Some(listing_event),
        None,
        None,
        payload,
    )
}

#[cfg(feature = "serde_json")]
pub fn order_decision_event_build(
    root_event_id: &RadrootsEventId,
    prev_event_id: &RadrootsEventId,
    payload: &RadrootsOrderDecision,
) -> Result<WireEventParts, EventEncodeError> {
    payload.validate().map_err(map_order_payload_error)?;
    order_envelope_event_build(
        &payload.buyer_pubkey,
        RadrootsOrderEventType::OrderDecision,
        &payload.listing_addr,
        &payload.order_id,
        None,
        Some(root_event_id),
        Some(prev_event_id),
        payload,
    )
}

#[cfg(feature = "serde_json")]
pub fn order_revision_proposal_event_build(
    root_event_id: &RadrootsEventId,
    prev_event_id: &RadrootsEventId,
    payload: &RadrootsOrderRevisionProposal,
) -> Result<WireEventParts, EventEncodeError> {
    payload.validate().map_err(map_order_payload_error)?;
    if payload.root_event_id.as_str() != root_event_id.as_str() {
        return Err(EventEncodeError::InvalidField("root_event_id"));
    }
    if payload.prev_event_id.as_str() != prev_event_id.as_str() {
        return Err(EventEncodeError::InvalidField("prev_event_id"));
    }
    order_envelope_event_build(
        &payload.buyer_pubkey,
        RadrootsOrderEventType::OrderRevisionProposed,
        &payload.listing_addr,
        &payload.order_id,
        None,
        Some(root_event_id),
        Some(prev_event_id),
        payload,
    )
}

#[cfg(feature = "serde_json")]
pub fn order_revision_decision_event_build(
    root_event_id: &RadrootsEventId,
    prev_event_id: &RadrootsEventId,
    payload: &RadrootsOrderRevisionDecision,
) -> Result<WireEventParts, EventEncodeError> {
    payload.validate().map_err(map_order_payload_error)?;
    if payload.root_event_id.as_str() != root_event_id.as_str() {
        return Err(EventEncodeError::InvalidField("root_event_id"));
    }
    if payload.prev_event_id.as_str() != prev_event_id.as_str() {
        return Err(EventEncodeError::InvalidField("prev_event_id"));
    }
    order_envelope_event_build(
        &payload.seller_pubkey,
        RadrootsOrderEventType::OrderRevisionDecision,
        &payload.listing_addr,
        &payload.order_id,
        None,
        Some(root_event_id),
        Some(prev_event_id),
        payload,
    )
}

#[cfg(feature = "serde_json")]
pub fn order_fulfillment_update_event_build(
    root_event_id: &RadrootsEventId,
    prev_event_id: &RadrootsEventId,
    payload: &RadrootsOrderFulfillmentUpdate,
) -> Result<WireEventParts, EventEncodeError> {
    payload.validate().map_err(map_order_payload_error)?;
    order_envelope_event_build(
        &payload.buyer_pubkey,
        RadrootsOrderEventType::FulfillmentUpdated,
        &payload.listing_addr,
        &payload.order_id,
        None,
        Some(root_event_id),
        Some(prev_event_id),
        payload,
    )
}

#[cfg(feature = "serde_json")]
pub fn order_cancellation_event_build(
    root_event_id: &RadrootsEventId,
    prev_event_id: &RadrootsEventId,
    payload: &RadrootsOrderCancellation,
) -> Result<WireEventParts, EventEncodeError> {
    payload.validate().map_err(map_order_payload_error)?;
    order_envelope_event_build(
        &payload.seller_pubkey,
        RadrootsOrderEventType::OrderCancelled,
        &payload.listing_addr,
        &payload.order_id,
        None,
        Some(root_event_id),
        Some(prev_event_id),
        payload,
    )
}

#[cfg(feature = "serde_json")]
pub fn order_receipt_event_build(
    root_event_id: &RadrootsEventId,
    prev_event_id: &RadrootsEventId,
    payload: &RadrootsOrderReceipt,
) -> Result<WireEventParts, EventEncodeError> {
    payload.validate().map_err(map_order_payload_error)?;
    order_envelope_event_build(
        &payload.seller_pubkey,
        RadrootsOrderEventType::BuyerReceipt,
        &payload.listing_addr,
        &payload.order_id,
        None,
        Some(root_event_id),
        Some(prev_event_id),
        payload,
    )
}

#[cfg(feature = "serde_json")]
pub fn order_payment_record_event_build(
    root_event_id: &RadrootsEventId,
    prev_event_id: &RadrootsEventId,
    payload: &RadrootsOrderPaymentRecord,
) -> Result<WireEventParts, EventEncodeError> {
    payload.validate().map_err(map_order_payload_error)?;
    if payload.root_event_id.as_str() != root_event_id.as_str() {
        return Err(EventEncodeError::InvalidField("root_event_id"));
    }
    if payload.previous_event_id.as_str() != prev_event_id.as_str() {
        return Err(EventEncodeError::InvalidField("previous_event_id"));
    }
    order_envelope_event_build(
        &payload.seller_pubkey,
        RadrootsOrderEventType::PaymentRecorded,
        &payload.listing_addr,
        &payload.order_id,
        None,
        Some(root_event_id),
        Some(prev_event_id),
        payload,
    )
}

#[cfg(feature = "serde_json")]
pub fn order_settlement_decision_event_build(
    root_event_id: &RadrootsEventId,
    prev_event_id: &RadrootsEventId,
    payload: &RadrootsOrderSettlementDecision,
) -> Result<WireEventParts, EventEncodeError> {
    payload.validate().map_err(map_order_payload_error)?;
    if payload.root_event_id.as_str() != root_event_id.as_str() {
        return Err(EventEncodeError::InvalidField("root_event_id"));
    }
    if payload.previous_event_id.as_str() != prev_event_id.as_str() {
        return Err(EventEncodeError::InvalidField("previous_event_id"));
    }
    order_envelope_event_build(
        &payload.buyer_pubkey,
        RadrootsOrderEventType::SettlementDecision,
        &payload.listing_addr,
        &payload.order_id,
        None,
        Some(root_event_id),
        Some(prev_event_id),
        payload,
    )
}
