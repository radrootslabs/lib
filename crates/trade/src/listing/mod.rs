mod codec;
pub mod model;
pub mod price_ext;
pub mod publish;
pub mod validation;

use radroots_events::{RadrootsNostrEvent, kinds::is_listing_kind, listing::RadrootsListing};

pub use radroots_events::order::RadrootsListingParseError as ListingParseError;

pub fn parse_listing_event(
    event: &RadrootsNostrEvent,
) -> Result<RadrootsListing, ListingParseError> {
    if !is_listing_kind(event.kind) {
        return Err(ListingParseError::InvalidKind(event.kind));
    }
    self::codec::listing_from_event_parts(&event.tags, &event.content)
}

#[cfg(test)]
mod tests {
    use super::parse_listing_event;
    use radroots_events::{
        RadrootsNostrEvent, kinds::KIND_PROFILE, order::RadrootsListingParseError,
    };

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
            Err(RadrootsListingParseError::InvalidKind(KIND_PROFILE))
        ));
    }
}
