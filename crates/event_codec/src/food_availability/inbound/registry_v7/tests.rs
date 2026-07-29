use super::*;
use radroots_event::envelope::RadrootsEventTags;

const HASH: &str = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";

#[test]
fn normalizes_tolerant_decimal_and_currency_values() {
    let tags = focused_tags(vec![
        tag(&["price", "0003.5000", "cad"]),
        tag(&["radroots:price_unit", "lb"]),
        tag(&["radroots:quantity", "0010.00", "lb"]),
    ]);

    let projection = focused(project(&tags).unwrap());
    assert_eq!(projection.price().amount(), "3.5");
    assert_eq!(projection.price().currency().as_str(), "CAD");
    assert_eq!(projection.quantity().unwrap().amount(), "10");
}

#[test]
fn partitions_raw_markers_before_shape_or_capability_validation() {
    let operational = tags(vec![tag(&["radroots:bin"]), tag(&["delivery"])]);
    assert_eq!(
        project(&operational).unwrap(),
        RadrootsFoodAvailabilityProjectionOutcome::Excluded(
            RadrootsClassifiedListingPartition::OperationalListing
        )
    );

    let mixed = tags(vec![
        tag(&["radroots:price_unit"]),
        tag(&["radroots:price"]),
    ]);
    assert_eq!(
        project(&mixed).unwrap_err().code(),
        "food_profile_ambiguous"
    );

    let malformed_focused = tags(vec![tag(&["radroots:price_unit"])]);
    assert_eq!(
        project(&malformed_focused).unwrap_err().code(),
        "food_tag_invalid"
    );
    assert_eq!(
        project_inbound_food_availability_parts(
            KIND_CLASSIFIED_LISTING,
            200,
            &malformed_focused,
            "",
        )
        .unwrap_err()
        .code(),
        "food_tag_invalid"
    );
}

#[test]
fn focused_marker_errors_and_prohibited_capabilities_are_stable() {
    let mut malformed_unit = base_tags();
    malformed_unit.push(tag(&["price", "3", "CAD"]));
    malformed_unit.push(tag(&["radroots:price_unit"]));
    malformed_unit.push(tag(&["status", "active"]));
    assert_eq!(
        project(&tags(malformed_unit)).unwrap_err().code(),
        "price_unit_invalid"
    );

    let mut prohibited = base_tags();
    prohibited.push(tag(&["radroots:price_unit", "lb"]));
    prohibited.push(tag(&["status", "active"]));
    prohibited.push(tag(&["delivery", "tomorrow"]));
    assert_eq!(
        project(&tags(prohibited)).unwrap_err().code(),
        "prohibited_capability"
    );
}

#[test]
fn retains_ordered_bounded_image_diagnostics() {
    let url = format!("https://media.example/{HASH}.webp");
    let mut extra = focused_tags(Vec::new()).to_vec();
    extra.extend([
        tag(&["image", &url]),
        tag(&["image", &url, "0x600", "extra"]),
    ]);
    let projection = focused(project(&tags(extra)).unwrap());
    assert_eq!(
        projection.diagnostics(),
        &[
            RadrootsFoodAvailabilityImageDiagnostic::ShapeInvalid,
            RadrootsFoodAvailabilityImageDiagnostic::DimensionsMissing,
            RadrootsFoodAvailabilityImageDiagnostic::ShapeInvalid,
            RadrootsFoodAvailabilityImageDiagnostic::DimensionsInvalid,
            RadrootsFoodAvailabilityImageDiagnostic::DuplicateUrl,
            RadrootsFoodAvailabilityImageDiagnostic::DuplicateDigest,
        ]
    );

    let mut bounded = focused_tags(Vec::new()).to_vec();
    bounded.extend((0..=RADROOTS_FOOD_IMAGE_MAX_COUNT).map(|index| {
        tag(&[
            "image",
            &format!("https://media.example/{index:064x}.webp"),
            "800x600",
        ])
    }));
    let projection = focused(project(&tags(bounded)).unwrap());
    assert_eq!(projection.images().len(), RADROOTS_FOOD_IMAGE_MAX_COUNT);
    assert_eq!(
        projection.diagnostics().first(),
        Some(&RadrootsFoodAvailabilityImageDiagnostic::CountExceeded)
    );
}

#[test]
fn accepts_optional_standard_tags_but_bounds_raw_decimal_digits() {
    let mut optional = focused_tags(Vec::new()).to_vec();
    optional.extend([tag(&["t", "vegetables"]), tag(&["g", "c28"])]);
    assert!(matches!(
        project(&tags(optional)).unwrap(),
        RadrootsFoodAvailabilityProjectionOutcome::Focused(_)
    ));

    let mut oversized = base_tags();
    oversized.push(tag(&["price", "00000000000000000000000000003", "CAD"]));
    oversized.push(tag(&["radroots:price_unit", "lb"]));
    oversized.push(tag(&["status", "active"]));
    assert_eq!(
        project(&tags(oversized)).unwrap_err().code(),
        "price_invalid"
    );
}

fn project(
    tags: &RadrootsEventTags,
) -> Result<RadrootsFoodAvailabilityProjectionOutcome, RadrootsFoodAvailabilityProjectionError> {
    project_inbound_food_availability_parts(
        KIND_CLASSIFIED_LISTING,
        200,
        tags,
        "Carrots available this week.",
    )
}

fn focused(
    outcome: RadrootsFoodAvailabilityProjectionOutcome,
) -> RadrootsInboundFoodAvailabilityProjection {
    match outcome {
        RadrootsFoodAvailabilityProjectionOutcome::Focused(projection) => *projection,
        RadrootsFoodAvailabilityProjectionOutcome::Excluded(partition) => {
            panic!("unexpected exclusion: {partition:?}")
        }
    }
}

fn focused_tags(mut additional: Vec<Vec<String>>) -> RadrootsEventTags {
    let mut values = base_tags();
    if !additional
        .iter()
        .any(|value| value.first().is_some_and(|name| name == "price"))
    {
        values.push(tag(&["price", "3", "CAD"]));
    }
    values.append(&mut additional);
    if !values.iter().any(|value| {
        value
            .first()
            .is_some_and(|name| name == "radroots:price_unit")
    }) {
        values.push(tag(&["radroots:price_unit", "lb"]));
    }
    values.push(tag(&["status", "active"]));
    tags(values)
}

fn base_tags() -> Vec<Vec<String>> {
    vec![
        tag(&["d", "nantes-carrots"]),
        tag(&["title", "Nantes Carrots"]),
        tag(&["summary", "Fresh bunches"]),
        tag(&["published_at", "100"]),
        tag(&["location", "Central Saanich, BC"]),
    ]
}

fn tags(values: Vec<Vec<String>>) -> RadrootsEventTags {
    RadrootsEventTags::new(values).unwrap()
}

fn tag(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}
