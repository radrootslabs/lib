#[cfg(not(feature = "std"))]
use alloc::{string::String, string::ToString, vec, vec::Vec};

use core::fmt;
use radroots_event::{
    RadrootsEventEnvelopeError, RadrootsEventTags,
    classified_listing::{TAG_RADROOTS_PRICE_UNIT, TAG_RADROOTS_QUANTITY},
    food_availability::{RadrootsFoodAvailabilityDetails, RadrootsFoodAvailabilityError},
    kinds::KIND_CLASSIFIED_LISTING,
    tags::{
        TAG_D, TAG_IMAGE, TAG_LOCATION, TAG_PRICE, TAG_PUBLISHED_AT, TAG_STATUS, TAG_SUMMARY,
        TAG_TITLE,
    },
    wire::{DEFAULT_RAW_JSON_MAX_BYTES, RadrootsNip01EventWireParts},
};

const FOOD_SIGNED_EVENT_FIXED_BYTES: usize = "{\"id\":\"".len()
    + 64
    + "\",\"pubkey\":\"".len()
    + 64
    + "\",\"created_at\":".len()
    + ",\"kind\":30402,\"tags\":".len()
    + ",\"content\":".len()
    + ",\"sig\":\"".len()
    + 128
    + "\"}".len();

#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsFoodAvailabilityEncodeError {
    Domain(RadrootsFoodAvailabilityError),
    Wire(RadrootsEventEnvelopeError),
    EventWireTooLarge { max: usize, actual: usize },
}

impl RadrootsFoodAvailabilityEncodeError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Domain(error) => error.code(),
            Self::Wire(RadrootsEventEnvelopeError::TagElementTooLarge { .. }) => {
                "food_tag_element_too_large"
            }
            Self::Wire(RadrootsEventEnvelopeError::TagsTooLarge { .. }) => {
                "food_tag_bytes_exceeded"
            }
            Self::Wire(_) => "food_wire_invalid",
            Self::EventWireTooLarge { .. } => "food_event_wire_too_large",
        }
    }
}

impl fmt::Display for RadrootsFoodAvailabilityEncodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Domain(error) => write!(formatter, "{error}"),
            Self::Wire(error) => write!(formatter, "invalid FoodAvailability wire parts: {error}"),
            Self::EventWireTooLarge { max, actual } => write!(
                formatter,
                "FoodAvailability canonical signed event would be {actual} bytes; max is {max}"
            ),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsFoodAvailabilityEncodeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Domain(error) => Some(error),
            Self::Wire(error) => Some(error),
            Self::EventWireTooLarge { .. } => None,
        }
    }
}

impl From<RadrootsFoodAvailabilityError> for RadrootsFoodAvailabilityEncodeError {
    fn from(value: RadrootsFoodAvailabilityError) -> Self {
        Self::Domain(value)
    }
}

impl From<RadrootsEventEnvelopeError> for RadrootsFoodAvailabilityEncodeError {
    fn from(value: RadrootsEventEnvelopeError) -> Self {
        Self::Wire(value)
    }
}

/// Builds exact canonical NIP-99 tags for strict FoodAvailability details.
pub fn authored_food_availability_build_tags(
    details: &RadrootsFoodAvailabilityDetails,
    created_at: u64,
) -> Result<Vec<Vec<String>>, RadrootsFoodAvailabilityEncodeError> {
    details.validate_created_at(created_at)?;
    let mut tags =
        Vec::with_capacity(8 + usize::from(details.quantity().is_some()) + details.images().len());
    tags.push(vec![TAG_D.into(), details.identifier().as_str().into()]);
    tags.push(vec![TAG_TITLE.into(), details.title().as_str().into()]);
    tags.push(vec![TAG_SUMMARY.into(), details.summary().as_str().into()]);
    tags.push(vec![
        TAG_PUBLISHED_AT.into(),
        details.published_at().to_string(),
    ]);
    tags.push(vec![
        TAG_LOCATION.into(),
        details.location().as_str().into(),
    ]);
    tags.push(vec![
        TAG_PRICE.into(),
        details.price().amount().into(),
        details.price().currency().as_str().into(),
    ]);
    tags.push(vec![
        TAG_RADROOTS_PRICE_UNIT.into(),
        details.price().unit().as_str().into(),
    ]);
    if let Some(quantity) = details.quantity() {
        tags.push(vec![
            TAG_RADROOTS_QUANTITY.into(),
            quantity.amount().into(),
            quantity.unit().as_str().into(),
        ]);
    }
    tags.push(vec![TAG_STATUS.into(), details.status().as_str().into()]);
    tags.extend(details.images().iter().map(|image| {
        vec![
            TAG_IMAGE.into(),
            image.url().into(),
            image.dimensions().to_string(),
        ]
    }));

    RadrootsEventTags::new(tags.clone())?;
    Ok(tags)
}

/// Builds deterministic unsigned kind-30402 wire parts for strict FoodAvailability details.
///
/// Every image is already byte-verified by the input typestate. A publication
/// runtime must still prove BUD-02 upload completion before signing.
pub fn authored_food_availability_to_wire_parts(
    details: &RadrootsFoodAvailabilityDetails,
    created_at: u64,
) -> Result<RadrootsNip01EventWireParts, RadrootsFoodAvailabilityEncodeError> {
    let tags = authored_food_availability_build_tags(details, created_at)?;
    validate_compact_signed_wire_size(&tags, details.content().as_str(), created_at)?;
    Ok(RadrootsNip01EventWireParts {
        kind: KIND_CLASSIFIED_LISTING,
        content: details.content().as_str().into(),
        tags,
    })
}

fn validate_compact_signed_wire_size(
    tags: &[Vec<String>],
    content: &str,
    created_at: u64,
) -> Result<(), RadrootsFoodAvailabilityEncodeError> {
    let actual = canonical_food_signed_event_size(tags, content, created_at);
    if actual > DEFAULT_RAW_JSON_MAX_BYTES {
        return Err(RadrootsFoodAvailabilityEncodeError::EventWireTooLarge {
            max: DEFAULT_RAW_JSON_MAX_BYTES,
            actual,
        });
    }
    Ok(())
}

pub(crate) fn canonical_food_signed_event_size(
    tags: &[Vec<String>],
    content: &str,
    created_at: u64,
) -> usize {
    let mut tags_bytes = 2usize;
    for (tag_index, tag) in tags.iter().enumerate() {
        if tag_index > 0 {
            tags_bytes = tags_bytes.saturating_add(1);
        }
        tags_bytes = tags_bytes.saturating_add(2);
        for (element_index, element) in tag.iter().enumerate() {
            if element_index > 0 {
                tags_bytes = tags_bytes.saturating_add(1);
            }
            tags_bytes = tags_bytes.saturating_add(canonical_json_string_bytes(element));
        }
    }
    FOOD_SIGNED_EVENT_FIXED_BYTES
        .saturating_add(decimal_u64_bytes(created_at))
        .saturating_add(tags_bytes)
        .saturating_add(canonical_json_string_bytes(content))
}

fn decimal_u64_bytes(mut value: u64) -> usize {
    let mut bytes = 1usize;
    while value >= 10 {
        value /= 10;
        bytes += 1;
    }
    bytes
}

fn canonical_json_string_bytes(value: &str) -> usize {
    value.chars().fold(2usize, |total, character| {
        total.saturating_add(match character {
            '"' | '\\' | '\u{0008}' | '\t' | '\n' | '\u{000c}' | '\r' => 2,
            '\u{0000}'..='\u{001f}' => 6,
            _ => character.len_utf8(),
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use radroots_event::food_availability::{
        RadrootsFoodAvailabilityDetailsParts, RadrootsFoodAvailabilityStatus, RadrootsFoodContent,
        RadrootsFoodCurrency, RadrootsFoodIdentifier, RadrootsFoodPrice, RadrootsFoodPublishedAt,
        RadrootsFoodQuantity, RadrootsFoodText, RadrootsFoodUnit,
    };

    #[test]
    fn emits_the_exact_canonical_food_tag_order() {
        let details = details("Carrots available this week.");
        let expected: Vec<Vec<String>> = vec![
            vec!["d".into(), "nantes-carrots".into()],
            vec!["title".into(), "Nantes Carrots".into()],
            vec!["summary".into(), "Fresh bunches".into()],
            vec!["published_at".into(), "100".into()],
            vec!["location".into(), "Central Saanich, BC".into()],
            vec!["price".into(), "3".into(), "CAD".into()],
            vec!["radroots:price_unit".into(), "lb".into()],
            vec!["radroots:quantity".into(), "24".into(), "lb".into()],
            vec!["status".into(), "active".into()],
        ];

        assert_eq!(
            authored_food_availability_build_tags(&details, 200).unwrap(),
            expected
        );
        let parts = authored_food_availability_to_wire_parts(&details, 200).unwrap();
        assert_eq!(parts.kind, KIND_CLASSIFIED_LISTING);
        assert_eq!(parts.content, "Carrots available this week.");
    }

    #[test]
    fn rejects_future_publication_and_compact_wire_expansion() {
        assert_eq!(
            authored_food_availability_to_wire_parts(&details("Carrots"), 99)
                .unwrap_err()
                .code(),
            "food_published_at_future"
        );

        let escaped_content = core::iter::repeat_n('\0', 64 * 1024).collect::<String>();
        assert_eq!(
            authored_food_availability_to_wire_parts(&details(&escaped_content), 200)
                .unwrap_err()
                .code(),
            "food_event_wire_too_large"
        );
    }

    fn details(content: &str) -> RadrootsFoodAvailabilityDetails {
        RadrootsFoodAvailabilityDetails::new(RadrootsFoodAvailabilityDetailsParts {
            content: RadrootsFoodContent::new(content).unwrap(),
            identifier: RadrootsFoodIdentifier::parse("nantes-carrots").unwrap(),
            title: RadrootsFoodText::new("Nantes Carrots").unwrap(),
            summary: RadrootsFoodText::new("Fresh bunches").unwrap(),
            published_at: RadrootsFoodPublishedAt::new(100).unwrap(),
            location: RadrootsFoodText::new("Central Saanich, BC").unwrap(),
            price: RadrootsFoodPrice::new(
                "3",
                RadrootsFoodCurrency::parse("CAD").unwrap(),
                RadrootsFoodUnit::Pound,
            )
            .unwrap(),
            quantity: Some(RadrootsFoodQuantity::new("24", RadrootsFoodUnit::Pound).unwrap()),
            status: RadrootsFoodAvailabilityStatus::Active,
            images: Vec::new(),
        })
        .unwrap()
    }
}
