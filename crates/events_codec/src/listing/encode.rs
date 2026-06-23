#[cfg(not(feature = "std"))]
use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};

#[cfg(feature = "serde_json")]
use radroots_events::kinds::{KIND_LISTING, is_listing_kind};
use radroots_events::listing::RadrootsListing;

use crate::error::EventEncodeError;
use crate::listing::tags::listing_tags;
#[cfg(feature = "serde_json")]
use crate::listing::tags::listing_tags_full;
#[cfg(feature = "serde_json")]
use crate::wire::WireEventParts;

#[cfg(feature = "serde_json")]
const DEFAULT_KIND: u32 = KIND_LISTING;

pub fn listing_build_tags(listing: &RadrootsListing) -> Result<Vec<Vec<String>>, EventEncodeError> {
    listing_tags(listing)
}

#[cfg(feature = "serde_json")]
pub fn to_wire_parts(listing: &RadrootsListing) -> Result<WireEventParts, EventEncodeError> {
    to_wire_parts_with_kind(listing, DEFAULT_KIND)
}

#[cfg(feature = "serde_json")]
pub fn to_wire_parts_with_kind(
    listing: &RadrootsListing,
    kind: u32,
) -> Result<WireEventParts, EventEncodeError> {
    if !is_listing_kind(kind) {
        return Err(EventEncodeError::InvalidKind(kind));
    }
    let tags = listing_tags_full(listing)?;
    let content = listing_markdown_content(listing);
    Ok(WireEventParts {
        kind,
        content,
        tags,
    })
}

#[cfg(feature = "serde_json")]
fn listing_markdown_content(listing: &RadrootsListing) -> String {
    let title = listing.product.title.trim();
    let summary = listing
        .product
        .summary
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    match (title.is_empty(), summary) {
        (false, Some(summary)) => format!("# {title}\n\n{summary}"),
        (false, None) => format!("# {title}"),
        (true, Some(summary)) => summary.to_string(),
        (true, None) => String::new(),
    }
}

#[cfg(all(test, feature = "serde_json"))]
mod tests {
    use super::*;
    use core::str::FromStr;
    use radroots_core::{
        RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCoreQuantity,
        RadrootsCoreQuantityPrice, RadrootsCoreUnit,
    };
    use radroots_events::{
        farm::RadrootsFarmRef,
        ids::{RadrootsDTag, RadrootsInventoryBinId},
        listing::{RadrootsListingBin, RadrootsListingProduct},
    };

    fn decimal(value: &str) -> RadrootsCoreDecimal {
        RadrootsCoreDecimal::from_str(value).expect("decimal")
    }

    fn listing_with(title: &str, summary: Option<&str>) -> RadrootsListing {
        RadrootsListing {
            d_tag: RadrootsDTag::parse("AAAAAAAAAAAAAAAAAAAAAA").expect("d tag"),
            published_at: None,
            farm: RadrootsFarmRef {
                pubkey: "a".repeat(64),
                d_tag: "AAAAAAAAAAAAAAAAAAAAAQ".to_string(),
            },
            product: RadrootsListingProduct {
                key: "coffee".to_string(),
                title: title.to_string(),
                category: "produce".to_string(),
                summary: summary.map(ToOwned::to_owned),
                process: None,
                lot: None,
                location: None,
                profile: None,
                year: None,
            },
            primary_bin_id: RadrootsInventoryBinId::parse("bin-1").expect("bin id"),
            bins: vec![RadrootsListingBin {
                bin_id: RadrootsInventoryBinId::parse("bin-1").expect("bin id"),
                quantity: RadrootsCoreQuantity::new(decimal("1"), RadrootsCoreUnit::MassG),
                price_per_canonical_unit: RadrootsCoreQuantityPrice::new(
                    RadrootsCoreMoney::new(decimal("1"), RadrootsCoreCurrency::USD),
                    RadrootsCoreQuantity::new(RadrootsCoreDecimal::ONE, RadrootsCoreUnit::MassG),
                ),
                display_amount: None,
                display_unit: None,
                display_label: None,
                display_price: None,
                display_price_unit: None,
            }],
            resource_area: None,
            plot: None,
            discounts: None,
            inventory_available: None,
            availability: None,
            delivery_method: None,
            location: None,
            images: None,
        }
    }

    #[test]
    fn listing_markdown_content_covers_title_summary_combinations() {
        assert_eq!(
            listing_markdown_content(&listing_with("Coffee", Some("Washed"))),
            "# Coffee\n\nWashed"
        );
        assert_eq!(
            listing_markdown_content(&listing_with("Coffee", None)),
            "# Coffee"
        );
        assert_eq!(
            listing_markdown_content(&listing_with(" ", Some("Washed"))),
            "Washed"
        );
        assert_eq!(listing_markdown_content(&listing_with(" ", None)), "");
    }
}
