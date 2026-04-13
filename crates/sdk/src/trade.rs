pub use radroots_events::trade::*;
pub use radroots_events_codec::error::EventEncodeError;
#[cfg(feature = "serde_json")]
pub use radroots_events_codec::trade::{
    RadrootsTradeEnvelopeParseError, RadrootsTradeEventContext, RadrootsTradeListingAddress,
    RadrootsTradeListingAddressError,
};
pub use radroots_trade::listing::validation::RadrootsTradeListing as TradeListingValidateResult;

use crate::RadrootsTradeEnvelope as SdkTradeEnvelope;
use crate::{RadrootsNostrEvent, RadrootsNostrEventPtr, WireEventParts};

#[cfg(feature = "serde_json")]
pub fn build_envelope_draft(
    recipient_pubkey: impl Into<String>,
    message_type: RadrootsTradeMessageType,
    listing_addr: impl Into<String>,
    order_id: Option<String>,
    listing_event: Option<&RadrootsNostrEventPtr>,
    root_event_id: Option<&str>,
    prev_event_id: Option<&str>,
    payload: &RadrootsTradeMessagePayload,
) -> Result<WireEventParts, EventEncodeError> {
    radroots_events_codec::trade::trade_envelope_event_build(
        recipient_pubkey,
        message_type,
        listing_addr,
        order_id,
        listing_event,
        root_event_id,
        prev_event_id,
        payload,
    )
}

#[cfg(feature = "serde_json")]
pub fn parse_envelope(
    event: &RadrootsNostrEvent,
) -> Result<SdkTradeEnvelope, RadrootsTradeEnvelopeParseError> {
    radroots_events_codec::trade::trade_envelope_from_event::<RadrootsTradeMessagePayload>(event)
}

#[cfg(feature = "serde_json")]
pub fn parse_listing_address(
    listing_addr: &str,
) -> Result<RadrootsTradeListingAddress, RadrootsTradeListingAddressError> {
    RadrootsTradeListingAddress::parse(listing_addr)
}

#[cfg(feature = "serde_json")]
pub fn validate_listing_event(
    event: &RadrootsNostrEvent,
) -> Result<TradeListingValidateResult, RadrootsTradeListingValidationError> {
    radroots_trade::listing::validation::validate_listing_event(event)
}
