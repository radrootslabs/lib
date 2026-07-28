//! Frozen food-availability semantics for event-contract registry v7.

#[cfg(not(feature = "std"))]
use alloc::{boxed::Box, string::String, string::ToString, vec::Vec};

use core::fmt;
use radroots_blossom::{BlobUrl, Sha256};
use radroots_event::{
    RadrootsEventTags,
    classified_listing::RadrootsClassifiedListingPartition,
    food_availability::{
        RADROOTS_FOOD_DECIMAL_MAX_DIGITS, RADROOTS_FOOD_IMAGE_MAX_COUNT,
        RadrootsFoodAvailabilityError, RadrootsFoodAvailabilityStatus, RadrootsFoodContent,
        RadrootsFoodCurrency, RadrootsFoodIdentifier, RadrootsFoodImageDimensions,
        RadrootsFoodPrice, RadrootsFoodPublishedAt, RadrootsFoodQuantity, RadrootsFoodText,
        RadrootsFoodUnit, food_media_blossom_digest, food_media_http_url_is_valid,
    },
    kinds::KIND_CLASSIFIED_LISTING,
    wire::DEFAULT_RAW_JSON_MAX_BYTES,
};

use crate::verification::v1::RadrootsSignatureVerifiedEvent;

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

const PROHIBITED_FOOD_CAPABILITY_TAGS: &[&str] = &[
    "buyer",
    "checkout",
    "delivery",
    "exception",
    "group",
    "invite",
    "order",
    "payment",
    "pickup",
    "proof",
    "provenance",
    "receipt",
    "route",
    "route_stop",
    "task",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsFoodAvailabilityImageDiagnostic {
    ShapeInvalid,
    UrlInvalid,
    DimensionsMissing,
    DimensionsInvalid,
    DuplicateUrl,
    DuplicateDigest,
    CountExceeded,
}

impl RadrootsFoodAvailabilityImageDiagnostic {
    pub const fn code(self) -> &'static str {
        match self {
            Self::ShapeInvalid => "food_image_shape_invalid",
            Self::UrlInvalid => "food_image_url_invalid",
            Self::DimensionsMissing => "food_image_dimensions_missing",
            Self::DimensionsInvalid => "food_image_dimensions_invalid",
            Self::DuplicateUrl => "food_image_duplicate_url",
            Self::DuplicateDigest => "food_image_duplicate_digest",
            Self::CountExceeded => "food_image_count_exceeded",
        }
    }
}

impl fmt::Display for RadrootsFoodAvailabilityImageDiagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsInboundFoodAvailabilityImage {
    raw_tag: Vec<String>,
    url: Option<String>,
    dimensions: Option<RadrootsFoodImageDimensions>,
    diagnostics: Vec<RadrootsFoodAvailabilityImageDiagnostic>,
}

impl RadrootsInboundFoodAvailabilityImage {
    pub fn raw_tag(&self) -> &[String] {
        &self.raw_tag
    }

    pub fn url(&self) -> Option<&str> {
        self.url.as_deref()
    }

    pub const fn dimensions(&self) -> Option<RadrootsFoodImageDimensions> {
        self.dimensions
    }

    pub fn diagnostics(&self) -> &[RadrootsFoodAvailabilityImageDiagnostic] {
        &self.diagnostics
    }

    pub fn qualifies(&self) -> bool {
        self.diagnostics.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsInboundFoodAvailabilityProjection {
    content: RadrootsFoodContent,
    identifier: RadrootsFoodIdentifier,
    title: RadrootsFoodText,
    summary: RadrootsFoodText,
    published_at: RadrootsFoodPublishedAt,
    location: RadrootsFoodText,
    price: RadrootsFoodPrice,
    quantity: Option<RadrootsFoodQuantity>,
    status: RadrootsFoodAvailabilityStatus,
    images: Vec<RadrootsInboundFoodAvailabilityImage>,
    diagnostics: Vec<RadrootsFoodAvailabilityImageDiagnostic>,
}

impl RadrootsInboundFoodAvailabilityProjection {
    pub fn content(&self) -> &RadrootsFoodContent {
        &self.content
    }

    pub fn identifier(&self) -> &RadrootsFoodIdentifier {
        &self.identifier
    }

    pub fn title(&self) -> &RadrootsFoodText {
        &self.title
    }

    pub fn summary(&self) -> &RadrootsFoodText {
        &self.summary
    }

    pub const fn published_at(&self) -> RadrootsFoodPublishedAt {
        self.published_at
    }

    pub fn location(&self) -> &RadrootsFoodText {
        &self.location
    }

    pub fn price(&self) -> &RadrootsFoodPrice {
        &self.price
    }

    pub fn quantity(&self) -> Option<&RadrootsFoodQuantity> {
        self.quantity.as_ref()
    }

    pub const fn status(&self) -> RadrootsFoodAvailabilityStatus {
        self.status
    }

    pub fn images(&self) -> &[RadrootsInboundFoodAvailabilityImage] {
        &self.images
    }

    pub fn diagnostics(&self) -> &[RadrootsFoodAvailabilityImageDiagnostic] {
        &self.diagnostics
    }
}

#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsFoodAvailabilityProjectionOutcome {
    Focused(Box<RadrootsInboundFoodAvailabilityProjection>),
    Excluded(RadrootsClassifiedListingPartition),
}

#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsFoodAvailabilityProjectionError {
    InvalidKind { expected: u32, actual: u32 },
    ProfileAmbiguous,
    ProhibitedCapability { tag: String },
    TagInvalid,
    PriceFrequencyForbidden,
    PriceUnitMissing,
    EventWireTooLarge { max: usize, actual: usize },
    Domain(RadrootsFoodAvailabilityError),
}

impl RadrootsFoodAvailabilityProjectionError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidKind { .. } => "invalid_kind",
            Self::ProfileAmbiguous => "food_profile_ambiguous",
            Self::ProhibitedCapability { .. } => "prohibited_capability",
            Self::TagInvalid => "food_tag_invalid",
            Self::PriceFrequencyForbidden => "price_frequency_forbidden",
            Self::PriceUnitMissing => "price_unit_missing",
            Self::EventWireTooLarge { .. } => "food_event_wire_too_large",
            Self::Domain(error) => error.code(),
        }
    }
}

impl fmt::Display for RadrootsFoodAvailabilityProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidKind { expected, actual } => write!(
                formatter,
                "FoodAvailability event kind must be {expected}, got {actual}"
            ),
            Self::ProfileAmbiguous => {
                formatter.write_str("classified listing mixes focused and operational markers")
            }
            Self::ProhibitedCapability { tag } => {
                write!(
                    formatter,
                    "FoodAvailability tag `{tag}` is a prohibited capability"
                )
            }
            Self::TagInvalid => formatter.write_str("FoodAvailability core tag shape is invalid"),
            Self::PriceFrequencyForbidden => {
                formatter.write_str("FoodAvailability price frequency is forbidden")
            }
            Self::PriceUnitMissing => formatter.write_str("FoodAvailability price unit is missing"),
            Self::EventWireTooLarge { max, actual } => write!(
                formatter,
                "FoodAvailability canonical signed event is {actual} bytes; max is {max}"
            ),
            Self::Domain(error) => write!(formatter, "{error}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsFoodAvailabilityProjectionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Domain(error) => Some(error),
            _ => None,
        }
    }
}

impl From<RadrootsFoodAvailabilityError> for RadrootsFoodAvailabilityProjectionError {
    fn from(value: RadrootsFoodAvailabilityError) -> Self {
        Self::Domain(value)
    }
}

/// Projects a signature-and-id verified kind-30402 event at the focused Food boundary.
///
/// Raw marker partitioning always precedes tag-shape and prohibited-capability
/// validation. Operational and generic NIP-99 inputs are valid exclusions.
pub fn project_verified_food_availability_event(
    verified_event: &RadrootsSignatureVerifiedEvent,
) -> Result<RadrootsFoodAvailabilityProjectionOutcome, RadrootsFoodAvailabilityProjectionError> {
    project_verified_food_availability_event_registry_v7(verified_event)
}

/// Projects a verified listing with contract-registry-v7 semantics.
pub fn project_verified_food_availability_event_registry_v7(
    verified_event: &RadrootsSignatureVerifiedEvent,
) -> Result<RadrootsFoodAvailabilityProjectionOutcome, RadrootsFoodAvailabilityProjectionError> {
    let event = verified_event.event();
    project_inbound_food_availability_parts(
        event.kind_u32(),
        event.created_at_u64(),
        event.tags(),
        event.content(),
    )
}

pub(crate) fn project_inbound_food_availability_parts(
    kind: u32,
    created_at: u64,
    tags: &RadrootsEventTags,
    content: &str,
) -> Result<RadrootsFoodAvailabilityProjectionOutcome, RadrootsFoodAvailabilityProjectionError> {
    if kind != KIND_CLASSIFIED_LISTING {
        return Err(RadrootsFoodAvailabilityProjectionError::InvalidKind {
            expected: KIND_CLASSIFIED_LISTING,
            actual: kind,
        });
    }

    match classify_classified_listing_tags_registry_v7(tags) {
        RadrootsClassifiedListingPartition::Ambiguous => {
            return Err(RadrootsFoodAvailabilityProjectionError::ProfileAmbiguous);
        }
        partition @ (RadrootsClassifiedListingPartition::OperationalListing
        | RadrootsClassifiedListingPartition::GenericNip99) => {
            return Ok(RadrootsFoodAvailabilityProjectionOutcome::Excluded(
                partition,
            ));
        }
        RadrootsClassifiedListingPartition::FocusedFoodAvailability => {}
    }

    let tags = tags.to_vec();
    if let Some(tag) = tags
        .iter()
        .filter_map(|tag| tag.first())
        .find(|name| PROHIBITED_FOOD_CAPABILITY_TAGS.contains(&name.as_str()))
    {
        return Err(
            RadrootsFoodAvailabilityProjectionError::ProhibitedCapability { tag: tag.clone() },
        );
    }

    let identifier_value = singleton_value(&tags, "d")?;
    let title_value = singleton_value(&tags, "title")?;
    let summary_value = singleton_value(&tags, "summary")?;
    let published_at_value = singleton_value(&tags, "published_at")?;
    let location_value = singleton_value(&tags, "location")?;
    let status_value = singleton_value(&tags, "status")?;

    let content = RadrootsFoodContent::new(content.to_string())?;
    let identifier = RadrootsFoodIdentifier::parse(identifier_value)?;
    let title = RadrootsFoodText::new(title_value.to_string())?;
    let summary = RadrootsFoodText::new(summary_value.to_string())?;
    let published_at = RadrootsFoodPublishedAt::parse(published_at_value)?;
    published_at.validate_created_at(created_at)?;
    let location = RadrootsFoodText::new(location_value.to_string())?;

    let price_tags = matching_tags(&tags, "price");
    let price_tag = match price_tags.as_slice() {
        [tag] => *tag,
        _ => return Err(RadrootsFoodAvailabilityError::PriceInvalid.into()),
    };
    if price_tag.len() > 3 {
        return Err(RadrootsFoodAvailabilityProjectionError::PriceFrequencyForbidden);
    }
    if price_tag.len() != 3 {
        return Err(RadrootsFoodAvailabilityError::PriceInvalid.into());
    }
    let amount = normalize_decimal(&price_tag[1], RadrootsFoodAvailabilityError::PriceInvalid)?;
    let currency = normalize_currency(&price_tag[2])?;

    let price_unit_tags = matching_tags(&tags, "radroots:price_unit");
    if price_unit_tags.is_empty() {
        return Err(RadrootsFoodAvailabilityProjectionError::PriceUnitMissing);
    }
    let unit_value = match price_unit_tags.as_slice() {
        [tag] if tag.len() == 2 => tag[1].as_str(),
        _ => return Err(RadrootsFoodAvailabilityError::PriceUnitInvalid.into()),
    };
    let unit = RadrootsFoodUnit::parse(unit_value)?;
    let price = RadrootsFoodPrice::new(amount, currency, unit)?;

    let quantity_tags = matching_tags(&tags, "radroots:quantity");
    let quantity = if quantity_tags.is_empty() {
        None
    } else {
        let tag = match quantity_tags.as_slice() {
            [tag] if tag.len() == 3 => *tag,
            _ => {
                return Err(RadrootsFoodAvailabilityError::QuantityInvalid.into());
            }
        };
        let amount = normalize_decimal(&tag[1], RadrootsFoodAvailabilityError::QuantityInvalid)?;
        let quantity_unit = RadrootsFoodUnit::parse(&tag[2])
            .map_err(|_| RadrootsFoodAvailabilityError::QuantityInvalid)?;
        if quantity_unit != unit {
            return Err(RadrootsFoodAvailabilityError::QuantityInvalid.into());
        }
        Some(RadrootsFoodQuantity::new(amount, quantity_unit)?)
    };

    let status = RadrootsFoodAvailabilityStatus::parse(status_value)?;
    let image_tags = matching_tags(&tags, "image");
    let (images, diagnostics) = project_images(&image_tags);

    Ok(RadrootsFoodAvailabilityProjectionOutcome::Focused(
        Box::new(RadrootsInboundFoodAvailabilityProjection {
            content,
            identifier,
            title,
            summary,
            published_at,
            location,
            price,
            quantity,
            status,
            images,
            diagnostics,
        }),
    ))
}

fn classify_classified_listing_tags_registry_v7(
    tags: &RadrootsEventTags,
) -> RadrootsClassifiedListingPartition {
    let mut has_focused_marker = false;
    let mut has_operational_marker = false;

    for name in tags
        .as_slice()
        .iter()
        .filter_map(|tag| tag.as_slice().first().map(String::as_str))
    {
        match name {
            "radroots:price_unit" | "radroots:quantity" => has_focused_marker = true,
            "radroots:primary_bin" | "radroots:bin" | "radroots:price" => {
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

fn matching_tags<'a>(tags: &'a [Vec<String>], name: &str) -> Vec<&'a Vec<String>> {
    tags.iter()
        .filter(|tag| tag.first().is_some_and(|key| key == name))
        .collect()
}

fn singleton_value<'a>(
    tags: &'a [Vec<String>],
    name: &str,
) -> Result<&'a str, RadrootsFoodAvailabilityProjectionError> {
    exact_single_value(matching_tags(tags, name))
}

fn exact_single_value(
    matches: Vec<&Vec<String>>,
) -> Result<&str, RadrootsFoodAvailabilityProjectionError> {
    let tag = exact_single_tag_from_matches(matches)?;
    if tag.len() != 2 {
        return Err(RadrootsFoodAvailabilityProjectionError::TagInvalid);
    }
    Ok(tag[1].as_str())
}

fn exact_single_tag_from_matches(
    matches: Vec<&Vec<String>>,
) -> Result<&Vec<String>, RadrootsFoodAvailabilityProjectionError> {
    match matches.as_slice() {
        [tag] => Ok(*tag),
        _ => Err(RadrootsFoodAvailabilityProjectionError::TagInvalid),
    }
}

fn normalize_decimal(
    value: &str,
    error: RadrootsFoodAvailabilityError,
) -> Result<String, RadrootsFoodAvailabilityProjectionError> {
    let mut digits = 0usize;
    let mut dot = None;
    for (index, byte) in value.bytes().enumerate() {
        match byte {
            b'0'..=b'9' => digits += 1,
            b'.' if dot.is_none() => dot = Some(index),
            _ => return Err(error.into()),
        }
    }
    if digits == 0
        || digits > RADROOTS_FOOD_DECIMAL_MAX_DIGITS
        || dot.is_some_and(|index| index == 0 || index + 1 == value.len())
    {
        return Err(error.into());
    }

    let (integer, fraction) = dot.map_or((value, None), |index| {
        (&value[..index], Some(&value[index + 1..]))
    });
    let integer = integer.trim_start_matches('0');
    let integer = if integer.is_empty() { "0" } else { integer };
    let fraction = fraction
        .map(|value| value.trim_end_matches('0'))
        .filter(|value| !value.is_empty());
    let mut normalized = String::with_capacity(value.len());
    normalized.push_str(integer);
    if let Some(fraction) = fraction {
        normalized.push('.');
        normalized.push_str(fraction);
    }
    Ok(normalized)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RadrootsStrictFoodAvailabilityProjection {
    identifier: RadrootsFoodIdentifier,
    published_at: RadrootsFoodPublishedAt,
}

impl RadrootsStrictFoodAvailabilityProjection {
    pub(crate) fn identifier(&self) -> &RadrootsFoodIdentifier {
        &self.identifier
    }

    pub(crate) const fn published_at(&self) -> RadrootsFoodPublishedAt {
        self.published_at
    }
}

fn canonical_food_signed_event_size(tags: &[Vec<String>], content: &str, created_at: u64) -> usize {
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

/// Validates a verified event against the exact authored Food wire profile.
///
/// This does not construct signable parts or elevate inbound media to verified
/// media typestate. It exists solely for comparing already signed revisions.
pub(crate) fn project_strict_verified_food_availability_event(
    verified_event: &RadrootsSignatureVerifiedEvent,
) -> Result<RadrootsStrictFoodAvailabilityProjection, RadrootsFoodAvailabilityProjectionError> {
    let event = verified_event.event();
    if event.kind_u32() != KIND_CLASSIFIED_LISTING {
        return Err(RadrootsFoodAvailabilityProjectionError::InvalidKind {
            expected: KIND_CLASSIFIED_LISTING,
            actual: event.kind_u32(),
        });
    }
    if classify_classified_listing_tags_registry_v7(event.tags())
        != RadrootsClassifiedListingPartition::FocusedFoodAvailability
    {
        return Err(RadrootsFoodAvailabilityProjectionError::ProfileAmbiguous);
    }

    let tags = event.tags_as_vec();
    let actual_wire_bytes =
        canonical_food_signed_event_size(&tags, event.content(), event.created_at_u64());
    if actual_wire_bytes > DEFAULT_RAW_JSON_MAX_BYTES {
        return Err(RadrootsFoodAvailabilityProjectionError::EventWireTooLarge {
            max: DEFAULT_RAW_JSON_MAX_BYTES,
            actual: actual_wire_bytes,
        });
    }
    let observed_names = tags
        .iter()
        .map(|tag| tag.first().map(String::as_str))
        .collect::<Vec<_>>();
    let has_quantity = observed_names.contains(&Some("radroots:quantity"));
    let image_count = observed_names
        .iter()
        .filter(|name| **name == Some("image"))
        .count();
    let mut expected_names = Vec::with_capacity(8 + usize::from(has_quantity) + image_count);
    expected_names.extend([
        "d",
        "title",
        "summary",
        "published_at",
        "location",
        "price",
        "radroots:price_unit",
    ]);
    if has_quantity {
        expected_names.push("radroots:quantity");
    }
    expected_names.push("status");
    expected_names.extend(core::iter::repeat_n("image", image_count));
    if observed_names
        .iter()
        .copied()
        .ne(expected_names.iter().copied().map(Some))
    {
        return Err(RadrootsFoodAvailabilityProjectionError::TagInvalid);
    }

    let price_tags = matching_tags(&tags, "price");
    let price = match price_tags.as_slice() {
        [tag] if tag.len() == 3 => *tag,
        [tag] if tag.len() > 3 => {
            return Err(RadrootsFoodAvailabilityProjectionError::PriceFrequencyForbidden);
        }
        _ => return Err(RadrootsFoodAvailabilityError::PriceInvalid.into()),
    };
    let normalized_price =
        normalize_decimal(&price[1], RadrootsFoodAvailabilityError::PriceInvalid)?;
    if normalized_price != price[1] {
        return Err(RadrootsFoodAvailabilityError::PriceInvalid.into());
    }
    if normalize_currency(&price[2])?.as_str() != price[2] {
        return Err(RadrootsFoodAvailabilityError::PriceCurrencyInvalid.into());
    }

    if let Some(quantity) = matching_tags(&tags, "radroots:quantity").first() {
        if quantity.len() != 3 {
            return Err(RadrootsFoodAvailabilityError::QuantityInvalid.into());
        }
        let normalized =
            normalize_decimal(&quantity[1], RadrootsFoodAvailabilityError::QuantityInvalid)?;
        if normalized != quantity[1] {
            return Err(RadrootsFoodAvailabilityError::QuantityInvalid.into());
        }
    }

    if image_count > RADROOTS_FOOD_IMAGE_MAX_COUNT {
        return Err(RadrootsFoodAvailabilityError::ImageCountExceeded {
            max: RADROOTS_FOOD_IMAGE_MAX_COUNT,
            actual: image_count,
        }
        .into());
    }
    let mut seen_urls = Vec::<String>::new();
    let mut seen_digests = Vec::<Sha256>::new();
    for tag in matching_tags(&tags, "image") {
        if tag.len() != 3 {
            return Err(RadrootsFoodAvailabilityProjectionError::TagInvalid);
        }
        let url = BlobUrl::parse(&tag[1])
            .and_then(BlobUrl::approve)
            .map_err(|_| RadrootsFoodAvailabilityProjectionError::TagInvalid)?;
        RadrootsFoodImageDimensions::parse(&tag[2])?;
        if seen_urls.iter().any(|seen| seen == &tag[1]) {
            return Err(RadrootsFoodAvailabilityError::ImageDuplicateUrl.into());
        }
        let digest = url.as_blob_url().hash_path().hash();
        if seen_digests.contains(&digest) {
            return Err(RadrootsFoodAvailabilityError::ImageDuplicateDigest.into());
        }
        seen_urls.push(tag[1].clone());
        seen_digests.push(digest);
    }

    match project_inbound_food_availability_parts(
        event.kind_u32(),
        event.created_at_u64(),
        event.tags(),
        event.content(),
    )? {
        RadrootsFoodAvailabilityProjectionOutcome::Focused(projection)
            if projection.diagnostics().is_empty() =>
        {
            Ok(RadrootsStrictFoodAvailabilityProjection {
                identifier: projection.identifier().clone(),
                published_at: projection.published_at(),
            })
        }
        _ => Err(RadrootsFoodAvailabilityProjectionError::TagInvalid),
    }
}

fn normalize_currency(
    value: &str,
) -> Result<RadrootsFoodCurrency, RadrootsFoodAvailabilityProjectionError> {
    if value.len() != 3 || !value.bytes().all(|byte| byte.is_ascii_alphabetic()) {
        return Err(RadrootsFoodAvailabilityError::PriceCurrencyInvalid.into());
    }
    Ok(RadrootsFoodCurrency::parse(value.to_ascii_uppercase())?)
}

fn project_images(
    image_tags: &[&Vec<String>],
) -> (
    Vec<RadrootsInboundFoodAvailabilityImage>,
    Vec<RadrootsFoodAvailabilityImageDiagnostic>,
) {
    let mut diagnostics = Vec::new();
    if image_tags.len() > RADROOTS_FOOD_IMAGE_MAX_COUNT {
        diagnostics.push(RadrootsFoodAvailabilityImageDiagnostic::CountExceeded);
    }
    let bounded = &image_tags[..image_tags.len().min(RADROOTS_FOOD_IMAGE_MAX_COUNT)];
    let mut images = Vec::with_capacity(bounded.len());
    let mut seen_urls = Vec::<String>::new();
    let mut seen_digests = Vec::<Sha256>::new();

    for tag in bounded {
        let raw_url = tag.get(1).cloned();
        let raw_dimensions = tag.get(2).cloned();
        let mut image_diagnostics = Vec::new();
        if tag.len() != 3 {
            image_diagnostics.push(RadrootsFoodAvailabilityImageDiagnostic::ShapeInvalid);
        }

        let url = match raw_url.as_deref() {
            Some(value) if food_media_http_url_is_valid(value) => Some(value.to_string()),
            Some(_) | None => {
                image_diagnostics.push(RadrootsFoodAvailabilityImageDiagnostic::UrlInvalid);
                None
            }
        };
        let dimensions = match raw_dimensions.as_deref() {
            None => {
                image_diagnostics.push(RadrootsFoodAvailabilityImageDiagnostic::DimensionsMissing);
                None
            }
            Some(value) => match RadrootsFoodImageDimensions::parse(value) {
                Ok(dimensions) => Some(dimensions),
                Err(_) => {
                    image_diagnostics
                        .push(RadrootsFoodAvailabilityImageDiagnostic::DimensionsInvalid);
                    None
                }
            },
        };

        if let Some(raw_url) = raw_url {
            if seen_urls.iter().any(|seen| seen == &raw_url) {
                image_diagnostics.push(RadrootsFoodAvailabilityImageDiagnostic::DuplicateUrl);
            }
            seen_urls.push(raw_url.clone());
            if let Some(digest) = food_media_blossom_digest(&raw_url) {
                if seen_digests.contains(&digest) {
                    image_diagnostics
                        .push(RadrootsFoodAvailabilityImageDiagnostic::DuplicateDigest);
                }
                seen_digests.push(digest);
            }
        }

        diagnostics.extend(image_diagnostics.iter().copied());
        images.push(RadrootsInboundFoodAvailabilityImage {
            raw_tag: (*tag).clone(),
            url,
            dimensions,
            diagnostics: image_diagnostics,
        });
    }

    (images, diagnostics)
}

#[cfg(test)]
mod tests;
