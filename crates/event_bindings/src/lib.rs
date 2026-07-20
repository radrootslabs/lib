pub use radroots_event as upstream;

mod model;

pub use model::{constants_module, kinds_module};

#[cfg(test)]
mod tests {
    use radroots_event::{
        kinds, operational_listing::RADROOTS_OPERATIONAL_LISTING_PRODUCT_TAG_KEYS,
    };

    use super::{constants_module, kinds_module};

    #[test]
    fn preserves_event_constant_exports() {
        let constants = constants_module().render_source();
        let kinds_ts = kinds_module().render_source();
        assert!(constants.contains("RADROOTS_OPERATIONAL_LISTING_PRODUCT_TAG_KEYS"));
        assert!(constants.contains(RADROOTS_OPERATIONAL_LISTING_PRODUCT_TAG_KEYS[0]));
        assert!(kinds_ts.contains("KIND_CLASSIFIED_LISTING"));
        assert!(kinds_ts.contains(&kinds::KIND_CLASSIFIED_LISTING.to_string()));
        assert!(kinds_ts.contains("KIND_TRADE_PROPOSAL"));
        assert!(kinds_ts.contains(&kinds::KIND_TRADE_PROPOSAL.to_string()));
        assert!(!kinds_ts.contains("KIND_ORDER_REQUEST"));
    }
}
