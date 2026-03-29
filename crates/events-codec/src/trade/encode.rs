#[cfg(all(not(feature = "std"), feature = "serde_json"))]
use alloc::string::String;

#[cfg(feature = "serde_json")]
use radroots_events::trade::{RadrootsTradeEnvelope, RadrootsTradeMessageType};

#[cfg(feature = "serde_json")]
use crate::{trade::tags::trade_envelope_tags, wire::WireEventParts};

#[cfg(feature = "serde_json")]
pub fn trade_envelope_event_build<T: serde::Serialize + Clone>(
    recipient_pubkey: impl Into<String>,
    message_type: RadrootsTradeMessageType,
    listing_addr: impl Into<String>,
    order_id: Option<String>,
    payload: &T,
) -> Result<WireEventParts, serde_json::Error> {
    let listing_addr = listing_addr.into();
    let envelope = RadrootsTradeEnvelope::new(
        message_type,
        listing_addr.clone(),
        order_id.clone(),
        payload.clone(),
    );
    let content = serde_json::to_string(&envelope)?;
    let tags = trade_envelope_tags(recipient_pubkey, &listing_addr, order_id.as_deref());
    Ok(WireEventParts {
        kind: message_type.kind(),
        content,
        tags,
    })
}
