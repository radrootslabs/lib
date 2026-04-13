pub use radroots_events::listing::*;
pub use radroots_events::trade::{
    RadrootsTradeListingParseError, RadrootsTradeListingValidationError,
};
pub use radroots_events_codec::error::EventEncodeError;
pub use radroots_trade::listing::validation::RadrootsTradeListing as TradeListingValidateResult;

use crate::{NostrTags, RadrootsNostrEvent, WireEventParts};

#[derive(Debug, Clone)]
pub struct RadrootsListingDraft {
    parts: WireEventParts,
}

impl RadrootsListingDraft {
    pub fn as_wire_parts(&self) -> &WireEventParts {
        &self.parts
    }

    pub fn into_wire_parts(self) -> WireEventParts {
        self.parts
    }
}

pub fn build_tags(listing: &RadrootsListing) -> Result<NostrTags, EventEncodeError> {
    radroots_events_codec::listing::encode::listing_build_tags(listing)
}

#[cfg(feature = "serde_json")]
pub fn build_draft(listing: &RadrootsListing) -> Result<RadrootsListingDraft, EventEncodeError> {
    Ok(RadrootsListingDraft {
        parts: radroots_events_codec::listing::encode::to_wire_parts(listing)?,
    })
}

#[cfg(feature = "serde_json")]
pub fn parse_event(
    event: &RadrootsNostrEvent,
) -> Result<RadrootsListing, RadrootsTradeListingParseError> {
    radroots_trade::listing::parse_listing_event(event)
}
