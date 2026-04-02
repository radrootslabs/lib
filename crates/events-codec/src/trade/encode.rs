#[cfg(all(not(feature = "std"), feature = "serde_json"))]
use alloc::string::String;

#[cfg(feature = "serde_json")]
use radroots_events::{
    RadrootsNostrEventPtr,
    trade::{
        RadrootsTradeEnvelope, RadrootsTradeEnvelopeError, RadrootsTradeMessagePayload,
        RadrootsTradeMessageType,
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
