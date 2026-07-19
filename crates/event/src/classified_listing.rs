use crate::{RadrootsEventTag, RadrootsEventTags};

pub const TAG_RADROOTS_PRICE_UNIT: &str = "radroots:price_unit";
pub const TAG_RADROOTS_QUANTITY: &str = "radroots:quantity";
pub const TAG_RADROOTS_PRIMARY_BIN: &str = "radroots:primary_bin";
pub const TAG_RADROOTS_BIN: &str = "radroots:bin";
pub const TAG_RADROOTS_PRICE: &str = "radroots:price";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// The marker-selected profile partition for a NIP-99 classified listing.
pub enum RadrootsClassifiedListingPartition {
    FocusedFoodAvailability,
    OperationalListing,
    GenericNip99,
    Ambiguous,
}

#[inline]
/// Partitions a classified listing by exact raw marker-name presence.
///
/// The caller owns kind checks and profile validation. Tag values and arity do
/// not affect this partition, so malformed one-element marker tags still count.
pub fn classify_classified_listing_tags(
    tags: &RadrootsEventTags,
) -> RadrootsClassifiedListingPartition {
    classify_classified_listing_tag_slice(tags.as_slice())
}

/// Partitions a borrowed classified-listing tag slice without allocating.
///
/// The caller owns kind checks and profile validation. Tag values and arity do
/// not affect this partition, so malformed one-element marker tags still count.
pub fn classify_classified_listing_tag_slice(
    tags: &[RadrootsEventTag],
) -> RadrootsClassifiedListingPartition {
    let mut has_focused_marker = false;
    let mut has_operational_marker = false;

    for tag in tags {
        let Some(name) = tag.as_slice().first().map(|value| value.as_str()) else {
            continue;
        };

        match name {
            TAG_RADROOTS_PRICE_UNIT | TAG_RADROOTS_QUANTITY => has_focused_marker = true,
            TAG_RADROOTS_PRIMARY_BIN | TAG_RADROOTS_BIN | TAG_RADROOTS_PRICE => {
                has_operational_marker = true;
            }
            _ => {}
        }

        if has_focused_marker && has_operational_marker {
            return RadrootsClassifiedListingPartition::Ambiguous;
        }
    }

    match (has_focused_marker, has_operational_marker) {
        (true, false) => RadrootsClassifiedListingPartition::FocusedFoodAvailability,
        (false, true) => RadrootsClassifiedListingPartition::OperationalListing,
        (false, false) => RadrootsClassifiedListingPartition::GenericNip99,
        (true, true) => RadrootsClassifiedListingPartition::Ambiguous,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FOCUSED_MARKERS: [&str; 2] = [TAG_RADROOTS_PRICE_UNIT, TAG_RADROOTS_QUANTITY];
    const OPERATIONAL_MARKERS: [&str; 3] = [
        TAG_RADROOTS_PRIMARY_BIN,
        TAG_RADROOTS_BIN,
        TAG_RADROOTS_PRICE,
    ];

    fn tags(values: &[&[&str]]) -> RadrootsEventTags {
        RadrootsEventTags::new(
            values
                .iter()
                .map(|tag| tag.iter().map(|value| (*value).to_owned()).collect())
                .collect(),
        )
        .expect("valid test tags")
    }

    fn classify(values: &[&[&str]]) -> RadrootsClassifiedListingPartition {
        classify_classified_listing_tags(&tags(values))
    }

    #[test]
    fn exposes_exact_marker_names() {
        assert_eq!(TAG_RADROOTS_PRICE_UNIT, "radroots:price_unit");
        assert_eq!(TAG_RADROOTS_QUANTITY, "radroots:quantity");
        assert_eq!(TAG_RADROOTS_PRIMARY_BIN, "radroots:primary_bin");
        assert_eq!(TAG_RADROOTS_BIN, "radroots:bin");
        assert_eq!(TAG_RADROOTS_PRICE, "radroots:price");
    }

    #[test]
    fn each_focused_marker_selects_focused_food_availability() {
        for marker in FOCUSED_MARKERS {
            assert_eq!(
                classify(&[&[marker, "value"]]),
                RadrootsClassifiedListingPartition::FocusedFoodAvailability,
                "focused marker {marker}"
            );
        }
    }

    #[test]
    fn each_operational_marker_selects_operational_listing() {
        for marker in OPERATIONAL_MARKERS {
            assert_eq!(
                classify(&[&[marker, "value"]]),
                RadrootsClassifiedListingPartition::OperationalListing,
                "operational marker {marker}"
            );
        }
    }

    #[test]
    fn one_element_malformed_markers_still_partition() {
        for marker in FOCUSED_MARKERS {
            assert_eq!(
                classify(&[&[marker]]),
                RadrootsClassifiedListingPartition::FocusedFoodAvailability,
                "malformed focused marker {marker}"
            );
        }

        for marker in OPERATIONAL_MARKERS {
            assert_eq!(
                classify(&[&[marker]]),
                RadrootsClassifiedListingPartition::OperationalListing,
                "malformed operational marker {marker}"
            );
        }
    }

    #[test]
    fn every_focused_and_operational_marker_pair_is_ambiguous_in_either_order() {
        for focused in FOCUSED_MARKERS {
            for operational in OPERATIONAL_MARKERS {
                assert_eq!(
                    classify(&[&[focused], &[operational]]),
                    RadrootsClassifiedListingPartition::Ambiguous,
                    "focused {focused} before operational {operational}"
                );
                assert_eq!(
                    classify(&[&[operational], &[focused]]),
                    RadrootsClassifiedListingPartition::Ambiguous,
                    "operational {operational} before focused {focused}"
                );
            }
        }
    }

    #[test]
    fn duplicate_markers_do_not_change_the_partition() {
        assert_eq!(
            classify(&[
                &[TAG_RADROOTS_PRICE_UNIT],
                &[TAG_RADROOTS_PRICE_UNIT, "lb"],
                &[TAG_RADROOTS_QUANTITY, "20", "lb"],
            ]),
            RadrootsClassifiedListingPartition::FocusedFoodAvailability
        );
        assert_eq!(
            classify(&[
                &[TAG_RADROOTS_BIN],
                &[TAG_RADROOTS_BIN, "bin-1"],
                &[TAG_RADROOTS_PRICE, "bin-1", "3", "CAD", "1", "lb"],
            ]),
            RadrootsClassifiedListingPartition::OperationalListing
        );
    }

    #[test]
    fn marker_matching_is_exact_and_case_sensitive() {
        for near_match in [
            "RADROOTS:PRICE_UNIT",
            "Radroots:price_unit",
            "radroots:Price_unit",
            "radroots:price-unit",
            "radroots:price_unit ",
            " radroots:price_unit",
            "radroots:price_units",
            "xradroots:price_unit",
            "radroots:primary_bins",
            "radroots:binning",
            "radroots:prices",
        ] {
            assert_eq!(
                classify(&[&[near_match, "value"]]),
                RadrootsClassifiedListingPartition::GenericNip99,
                "near match {near_match}"
            );
        }
    }

    #[test]
    fn marker_names_in_values_do_not_partition() {
        let values = [
            &["summary", TAG_RADROOTS_PRICE_UNIT][..],
            &["description", TAG_RADROOTS_QUANTITY][..],
            &["title", TAG_RADROOTS_PRIMARY_BIN][..],
            &["t", TAG_RADROOTS_BIN][..],
            &["alt", TAG_RADROOTS_PRICE][..],
        ];

        assert_eq!(
            classify(&values),
            RadrootsClassifiedListingPartition::GenericNip99
        );
    }

    #[test]
    fn empty_tags_are_generic_nip99() {
        let tags = RadrootsEventTags::new(Vec::new()).expect("empty tag list is valid");

        assert_eq!(
            classify_classified_listing_tags(&tags),
            RadrootsClassifiedListingPartition::GenericNip99
        );
        assert_eq!(
            classify_classified_listing_tag_slice(tags.as_slice()),
            RadrootsClassifiedListingPartition::GenericNip99
        );
    }
}
