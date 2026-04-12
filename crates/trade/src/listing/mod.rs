mod codec;
pub(crate) mod contract;
pub mod model;
pub mod overlay;
pub mod price_ext;
pub mod projection;
pub mod publish;
pub mod validation;

use radroots_events::{RadrootsNostrEvent, listing::RadrootsListing};

pub(crate) use self::contract as dvm;
#[allow(unused_imports)]
pub(crate) use self::contract as kinds;
pub(crate) use self::contract as order;
pub use radroots_events::trade::RadrootsTradeListingParseError as TradeListingParseError;

pub fn parse_listing_event(
    event: &RadrootsNostrEvent,
) -> Result<RadrootsListing, TradeListingParseError> {
    self::codec::listing_from_event_parts(&event.tags, &event.content)
}
