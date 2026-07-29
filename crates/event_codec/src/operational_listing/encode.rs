#[cfg(all(not(feature = "std"), test, feature = "serde_json"))]
use alloc::vec;
#[cfg(all(not(feature = "std"), feature = "serde_json"))]
use alloc::{format, string::ToString};
#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

#[cfg(feature = "serde_json")]
use radroots_event::envelope::kind::{KIND_CLASSIFIED_LISTING, is_classified_listing_kind};
use radroots_event::listing::operational::OperationalListing;

use crate::error::EventEncodeError;
use crate::operational_listing::tags::operational_listing_tags;
#[cfg(feature = "serde_json")]
use crate::operational_listing::tags::operational_listing_tags_full;
#[cfg(feature = "serde_json")]
use radroots_event::wire::Nip01EventWireParts;

#[cfg(feature = "serde_json")]
const DEFAULT_KIND: u32 = KIND_CLASSIFIED_LISTING;

pub fn operational_listing_build_tags(
    listing: &OperationalListing,
) -> Result<Vec<Vec<String>>, EventEncodeError> {
    operational_listing_tags(listing)
}

#[cfg(feature = "serde_json")]
pub fn to_wire_parts(
    listing: &OperationalListing,
) -> Result<Nip01EventWireParts, EventEncodeError> {
    to_wire_parts_with_kind(listing, DEFAULT_KIND)
}

#[cfg(feature = "serde_json")]
pub fn to_wire_parts_with_kind(
    listing: &OperationalListing,
    kind: u32,
) -> Result<Nip01EventWireParts, EventEncodeError> {
    if !is_classified_listing_kind(kind) {
        return Err(EventEncodeError::InvalidKind(kind));
    }
    let tags = operational_listing_tags_full(listing)?;
    let content = operational_listing_markdown_content(listing);
    Ok(Nip01EventWireParts {
        kind,
        content,
        tags,
    })
}

#[cfg(feature = "serde_json")]
fn operational_listing_markdown_content(listing: &OperationalListing) -> String {
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
    use radroots_core::{Currency, Decimal, Money, Quantity, QuantityPrice, Unit};
    use radroots_event::{
        farm::FarmRef,
        id::{DTag, InventoryBinId},
        listing::operational::{OperationalListingBin, OperationalListingProduct},
    };

    fn decimal(value: &str) -> Decimal {
        Decimal::from_str(value).expect("decimal")
    }

    fn listing_with(title: &str, summary: Option<&str>) -> OperationalListing {
        OperationalListing {
            d_tag: DTag::parse("AAAAAAAAAAAAAAAAAAAAAA").expect("d tag"),
            published_at: None,
            farm: FarmRef {
                pubkey: "a".repeat(64),
                d_tag: "AAAAAAAAAAAAAAAAAAAAAQ".to_string(),
            },
            product: OperationalListingProduct {
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
            primary_bin_id: InventoryBinId::parse("bin-1").expect("bin id"),
            bins: vec![OperationalListingBin {
                bin_id: InventoryBinId::parse("bin-1").expect("bin id"),
                quantity: Quantity::try_new(decimal("1"), Unit::MassG).unwrap(),
                price_per_canonical_unit: QuantityPrice::try_new(
                    Money::try_new(decimal("1"), Currency::USD).unwrap(),
                    Quantity::try_new(Decimal::ONE, Unit::MassG).unwrap(),
                )
                .unwrap(),
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
    fn operational_listing_markdown_content_covers_title_summary_combinations() {
        assert_eq!(
            operational_listing_markdown_content(&listing_with("Coffee", Some("Washed"))),
            "# Coffee\n\nWashed"
        );
        assert_eq!(
            operational_listing_markdown_content(&listing_with("Coffee", None)),
            "# Coffee"
        );
        assert_eq!(
            operational_listing_markdown_content(&listing_with(" ", Some("Washed"))),
            "Washed"
        );
        assert_eq!(
            operational_listing_markdown_content(&listing_with(" ", None)),
            ""
        );
    }
}
