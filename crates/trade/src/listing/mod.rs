mod codec;
pub(crate) mod contract;
pub mod model;
pub mod overlay;
pub mod price_ext;
pub mod projection;
pub mod publish;
pub mod validation;

use radroots_events::{RadrootsNostrEvent, kinds::is_listing_kind, listing::RadrootsListing};

pub(crate) use self::contract as dvm;
#[allow(unused_imports)]
pub(crate) use self::contract as kinds;
pub(crate) use self::contract as order;
pub use radroots_events::trade::RadrootsTradeListingParseError as TradeListingParseError;

pub fn parse_listing_event(
    event: &RadrootsNostrEvent,
) -> Result<RadrootsListing, TradeListingParseError> {
    if !is_listing_kind(event.kind) {
        return Err(TradeListingParseError::InvalidKind(event.kind));
    }
    self::codec::listing_from_event_parts(&event.tags, &event.content)
}

#[cfg(test)]
mod tests {
    use super::parse_listing_event;
    use radroots_events::{RadrootsNostrEvent, kinds::KIND_PROFILE, trade::RadrootsTradeListingParseError};

    #[test]
    fn parse_listing_event_rejects_non_listing_kind() {
        let event = RadrootsNostrEvent {
            id: "event-1".into(),
            author: "seller".into(),
            created_at: 1,
            kind: KIND_PROFILE,
            tags: vec![],
            content: String::new(),
            sig: String::new(),
        };

        assert!(matches!(
            parse_listing_event(&event),
            Err(RadrootsTradeListingParseError::InvalidKind(KIND_PROFILE))
        ));
    }
}
