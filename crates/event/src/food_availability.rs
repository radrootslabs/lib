#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};
#[cfg(feature = "std")]
use std::{string::String, vec::Vec};

use core::{fmt, str::FromStr};
use radroots_blossom::hash::{RadrootsBlossomHashPath, RadrootsBlossomSha256};
use unicode_general_category::{GeneralCategory, get_general_category};
use url_nostd::{Host, Url};

use crate::media::RadrootsAuthoredImage;

pub const RADROOTS_FOOD_CONTENT_MAX_BYTES: usize = 128 * 1024;
pub const RADROOTS_FOOD_IDENTIFIER_MAX_BYTES: usize = 512;
pub const RADROOTS_FOOD_TEXT_MAX_BYTES: usize = 4 * 1024;
pub const RADROOTS_FOOD_DECIMAL_MAX_DIGITS: usize = 28;
pub const RADROOTS_FOOD_IMAGE_MAX_COUNT: usize = 64;
pub const RADROOTS_FOOD_AVAILABILITY_CONTRACT_ID: &str = "radroots.food.availability.v1";

/// Errors raised while constructing strict FoodAvailability details.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsFoodAvailabilityError {
    ContentMissing,
    ContentTooLarge { max: usize, actual: usize },
    IdentifierInvalid,
    IdentifierTooLarge { max: usize, actual: usize },
    TextInvalid,
    TextTooLarge { max: usize, actual: usize },
    PublishedAtInvalid,
    PublishedAtFuture { published_at: u64, created_at: u64 },
    PriceInvalid,
    PriceCurrencyInvalid,
    PriceUnitInvalid,
    QuantityInvalid,
    QuantityZero,
    StatusInvalid,
    ImageDimensionsInvalid,
    ImageCountExceeded { max: usize, actual: usize },
    ImageDuplicateUrl,
    ImageDuplicateDigest,
}

impl RadrootsFoodAvailabilityError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::ContentMissing => "food_content_missing",
            Self::ContentTooLarge { .. } => "food_content_too_large",
            Self::IdentifierInvalid | Self::IdentifierTooLarge { .. } => "food_identifier_invalid",
            Self::TextInvalid | Self::TextTooLarge { .. } => "food_text_invalid",
            Self::PublishedAtInvalid => "food_published_at_invalid",
            Self::PublishedAtFuture { .. } => "food_published_at_future",
            Self::PriceInvalid => "price_invalid",
            Self::PriceCurrencyInvalid => "price_currency_invalid",
            Self::PriceUnitInvalid => "price_unit_invalid",
            Self::QuantityInvalid => "quantity_invalid",
            Self::QuantityZero => "quantity_zero",
            Self::StatusInvalid => "food_status_invalid",
            Self::ImageDimensionsInvalid => "food_image_dimensions_invalid",
            Self::ImageCountExceeded { .. } => "food_image_count_exceeded",
            Self::ImageDuplicateUrl => "food_image_duplicate_url",
            Self::ImageDuplicateDigest => "food_image_duplicate_digest",
        }
    }
}

impl fmt::Display for RadrootsFoodAvailabilityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ContentMissing => {
                formatter.write_str("FoodAvailability content must be non-whitespace")
            }
            Self::ContentTooLarge { max, actual } => write!(
                formatter,
                "FoodAvailability content is {actual} bytes; max is {max}"
            ),
            Self::IdentifierInvalid => formatter.write_str(
                "FoodAvailability identifier must be nonempty and contain no whitespace, control, or format characters",
            ),
            Self::IdentifierTooLarge { max, actual } => write!(
                formatter,
                "FoodAvailability identifier is {actual} bytes; max is {max}"
            ),
            Self::TextInvalid => formatter.write_str(
                "FoodAvailability text must be trimmed, nonempty, and contain no control or format characters",
            ),
            Self::TextTooLarge { max, actual } => write!(
                formatter,
                "FoodAvailability text is {actual} bytes; max is {max}"
            ),
            Self::PublishedAtInvalid => formatter.write_str(
                "FoodAvailability published_at must be a canonical nonzero u64 timestamp",
            ),
            Self::PublishedAtFuture {
                published_at,
                created_at,
            } => write!(
                formatter,
                "FoodAvailability published_at {published_at} exceeds created_at {created_at}"
            ),
            Self::PriceInvalid => formatter.write_str(
                "FoodAvailability price must be a canonical unsigned decimal with at most 28 digits",
            ),
            Self::PriceCurrencyInvalid => formatter.write_str(
                "FoodAvailability price currency must be three uppercase ASCII letters",
            ),
            Self::PriceUnitInvalid => {
                formatter.write_str("FoodAvailability price unit is not governed")
            }
            Self::QuantityInvalid => formatter.write_str(
                "FoodAvailability quantity must be canonical and use the price unit",
            ),
            Self::QuantityZero => {
                formatter.write_str("FoodAvailability quantity must be positive")
            }
            Self::StatusInvalid => {
                formatter.write_str("FoodAvailability status must be active or sold")
            }
            Self::ImageDimensionsInvalid => formatter.write_str(
                "FoodAvailability image dimensions must be canonical nonzero u32 values",
            ),
            Self::ImageCountExceeded { max, actual } => write!(
                formatter,
                "FoodAvailability has {actual} images; max is {max}"
            ),
            Self::ImageDuplicateUrl => {
                formatter.write_str("FoodAvailability image URLs must be unique")
            }
            Self::ImageDuplicateDigest => {
                formatter.write_str("FoodAvailability image digests must be unique")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsFoodAvailabilityError {}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RadrootsFoodContent(String);

impl RadrootsFoodContent {
    pub fn new(value: impl Into<String>) -> Result<Self, RadrootsFoodAvailabilityError> {
        let value = value.into();
        if value.chars().all(is_food_contract_whitespace) {
            return Err(RadrootsFoodAvailabilityError::ContentMissing);
        }
        if value.len() > RADROOTS_FOOD_CONTENT_MAX_BYTES {
            return Err(RadrootsFoodAvailabilityError::ContentTooLarge {
                max: RADROOTS_FOOD_CONTENT_MAX_BYTES,
                actual: value.len(),
            });
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl AsRef<str> for RadrootsFoodContent {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for RadrootsFoodContent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RadrootsFoodIdentifier(String);

impl RadrootsFoodIdentifier {
    pub fn parse(value: impl AsRef<str>) -> Result<Self, RadrootsFoodAvailabilityError> {
        let value = value.as_ref();
        if value.is_empty()
            || value
                .chars()
                .any(|character| character.is_whitespace() || is_control_or_format(character))
        {
            return Err(RadrootsFoodAvailabilityError::IdentifierInvalid);
        }
        if value.len() > RADROOTS_FOOD_IDENTIFIER_MAX_BYTES {
            return Err(RadrootsFoodAvailabilityError::IdentifierTooLarge {
                max: RADROOTS_FOOD_IDENTIFIER_MAX_BYTES,
                actual: value.len(),
            });
        }
        Ok(Self(value.into()))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl AsRef<str> for RadrootsFoodIdentifier {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for RadrootsFoodIdentifier {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for RadrootsFoodIdentifier {
    type Err = RadrootsFoodAvailabilityError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RadrootsFoodText(String);

impl RadrootsFoodText {
    pub fn new(value: impl Into<String>) -> Result<Self, RadrootsFoodAvailabilityError> {
        let value = value.into();
        if value.is_empty() || value.trim() != value || value.chars().any(is_control_or_format) {
            return Err(RadrootsFoodAvailabilityError::TextInvalid);
        }
        if value.len() > RADROOTS_FOOD_TEXT_MAX_BYTES {
            return Err(RadrootsFoodAvailabilityError::TextTooLarge {
                max: RADROOTS_FOOD_TEXT_MAX_BYTES,
                actual: value.len(),
            });
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl AsRef<str> for RadrootsFoodText {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for RadrootsFoodText {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RadrootsFoodPublishedAt(u64);

impl RadrootsFoodPublishedAt {
    pub const fn new(value: u64) -> Result<Self, RadrootsFoodAvailabilityError> {
        if value == 0 {
            return Err(RadrootsFoodAvailabilityError::PublishedAtInvalid);
        }
        Ok(Self(value))
    }

    pub fn parse(value: &str) -> Result<Self, RadrootsFoodAvailabilityError> {
        if !canonical_unsigned_integer(value) {
            return Err(RadrootsFoodAvailabilityError::PublishedAtInvalid);
        }
        value
            .parse::<u64>()
            .ok()
            .and_then(|parsed| Self::new(parsed).ok())
            .ok_or(RadrootsFoodAvailabilityError::PublishedAtInvalid)
    }

    pub const fn as_u64(self) -> u64 {
        self.0
    }

    pub const fn validate_created_at(
        self,
        created_at: u64,
    ) -> Result<(), RadrootsFoodAvailabilityError> {
        if self.0 > created_at {
            return Err(RadrootsFoodAvailabilityError::PublishedAtFuture {
                published_at: self.0,
                created_at,
            });
        }
        Ok(())
    }
}

impl fmt::Display for RadrootsFoodPublishedAt {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

impl FromStr for RadrootsFoodPublishedAt {
    type Err = RadrootsFoodAvailabilityError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RadrootsFoodCurrency(String);

impl RadrootsFoodCurrency {
    pub fn parse(value: impl AsRef<str>) -> Result<Self, RadrootsFoodAvailabilityError> {
        let value = value.as_ref();
        if value.len() != 3 || !value.bytes().all(|byte| byte.is_ascii_uppercase()) {
            return Err(RadrootsFoodAvailabilityError::PriceCurrencyInvalid);
        }
        Ok(Self(value.into()))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl AsRef<str> for RadrootsFoodCurrency {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for RadrootsFoodCurrency {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for RadrootsFoodCurrency {
    type Err = RadrootsFoodAvailabilityError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RadrootsFoodUnit {
    Gram,
    Kilogram,
    Pound,
    Ounce,
    Each,
    Dozen,
    Bunch,
    Punnet,
    Bag,
    Basket,
}

impl RadrootsFoodUnit {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Gram => "g",
            Self::Kilogram => "kg",
            Self::Pound => "lb",
            Self::Ounce => "oz",
            Self::Each => "each",
            Self::Dozen => "dozen",
            Self::Bunch => "bunch",
            Self::Punnet => "punnet",
            Self::Bag => "bag",
            Self::Basket => "basket",
        }
    }

    pub fn parse(value: &str) -> Result<Self, RadrootsFoodAvailabilityError> {
        match value {
            "g" => Ok(Self::Gram),
            "kg" => Ok(Self::Kilogram),
            "lb" => Ok(Self::Pound),
            "oz" => Ok(Self::Ounce),
            "each" => Ok(Self::Each),
            "dozen" => Ok(Self::Dozen),
            "bunch" => Ok(Self::Bunch),
            "punnet" => Ok(Self::Punnet),
            "bag" => Ok(Self::Bag),
            "basket" => Ok(Self::Basket),
            _ => Err(RadrootsFoodAvailabilityError::PriceUnitInvalid),
        }
    }
}

impl fmt::Display for RadrootsFoodUnit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for RadrootsFoodUnit {
    type Err = RadrootsFoodAvailabilityError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsFoodPrice {
    amount: String,
    currency: RadrootsFoodCurrency,
    unit: RadrootsFoodUnit,
}

impl RadrootsFoodPrice {
    pub fn new(
        amount: impl Into<String>,
        currency: RadrootsFoodCurrency,
        unit: RadrootsFoodUnit,
    ) -> Result<Self, RadrootsFoodAvailabilityError> {
        let amount = amount.into();
        validate_canonical_decimal(&amount)
            .then_some(())
            .ok_or(RadrootsFoodAvailabilityError::PriceInvalid)?;
        Ok(Self {
            amount,
            currency,
            unit,
        })
    }

    pub fn amount(&self) -> &str {
        self.amount.as_str()
    }

    pub fn currency(&self) -> &RadrootsFoodCurrency {
        &self.currency
    }

    pub const fn unit(&self) -> RadrootsFoodUnit {
        self.unit
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsFoodQuantity {
    amount: String,
    unit: RadrootsFoodUnit,
}

impl RadrootsFoodQuantity {
    pub fn new(
        amount: impl Into<String>,
        unit: RadrootsFoodUnit,
    ) -> Result<Self, RadrootsFoodAvailabilityError> {
        let amount = amount.into();
        if !validate_canonical_decimal(&amount) {
            return Err(RadrootsFoodAvailabilityError::QuantityInvalid);
        }
        if !amount.bytes().any(|byte| matches!(byte, b'1'..=b'9')) {
            return Err(RadrootsFoodAvailabilityError::QuantityZero);
        }
        Ok(Self { amount, unit })
    }

    pub fn amount(&self) -> &str {
        self.amount.as_str()
    }

    pub const fn unit(&self) -> RadrootsFoodUnit {
        self.unit
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RadrootsFoodAvailabilityStatus {
    Active,
    Sold,
}

impl RadrootsFoodAvailabilityStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Sold => "sold",
        }
    }

    pub fn parse(value: &str) -> Result<Self, RadrootsFoodAvailabilityError> {
        match value {
            "active" => Ok(Self::Active),
            "sold" => Ok(Self::Sold),
            _ => Err(RadrootsFoodAvailabilityError::StatusInvalid),
        }
    }
}

impl fmt::Display for RadrootsFoodAvailabilityStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for RadrootsFoodAvailabilityStatus {
    type Err = RadrootsFoodAvailabilityError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RadrootsFoodImageDimensions {
    width: u32,
    height: u32,
}

impl RadrootsFoodImageDimensions {
    pub const fn new(width: u32, height: u32) -> Result<Self, RadrootsFoodAvailabilityError> {
        if width == 0 || height == 0 {
            return Err(RadrootsFoodAvailabilityError::ImageDimensionsInvalid);
        }
        Ok(Self { width, height })
    }

    pub fn parse(value: &str) -> Result<Self, RadrootsFoodAvailabilityError> {
        let Some((width, height)) = value.split_once('x') else {
            return Err(RadrootsFoodAvailabilityError::ImageDimensionsInvalid);
        };
        if height.contains('x')
            || !canonical_unsigned_integer(width)
            || !canonical_unsigned_integer(height)
        {
            return Err(RadrootsFoodAvailabilityError::ImageDimensionsInvalid);
        }
        let width = width
            .parse::<u32>()
            .map_err(|_| RadrootsFoodAvailabilityError::ImageDimensionsInvalid)?;
        let height = height
            .parse::<u32>()
            .map_err(|_| RadrootsFoodAvailabilityError::ImageDimensionsInvalid)?;
        Self::new(width, height)
    }

    pub const fn width(self) -> u32 {
        self.width
    }

    pub const fn height(self) -> u32 {
        self.height
    }
}

impl fmt::Display for RadrootsFoodImageDimensions {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}x{}", self.width, self.height)
    }
}

impl FromStr for RadrootsFoodImageDimensions {
    type Err = RadrootsFoodAvailabilityError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

/// One byte-verified Blossom image and its declared NIP-58 dimensions.
///
/// This state proves descriptor-to-byte agreement and an `image/*` media type.
/// It does not prove upload completion, raster decoding, or network availability.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsFoodAvailabilityImage {
    image: RadrootsAuthoredImage,
    dimensions: RadrootsFoodImageDimensions,
}

impl RadrootsFoodAvailabilityImage {
    pub const fn new(
        image: RadrootsAuthoredImage,
        dimensions: RadrootsFoodImageDimensions,
    ) -> Self {
        Self { image, dimensions }
    }

    pub fn image(&self) -> &RadrootsAuthoredImage {
        &self.image
    }

    pub const fn dimensions(&self) -> RadrootsFoodImageDimensions {
        self.dimensions
    }

    pub fn url(&self) -> &str {
        self.image.descriptor().url().as_str()
    }
}

/// Validated semantic inputs for a focused FoodAvailability event.
///
/// This is intentionally not a signable event draft. The codec layer must
/// still derive canonical tags, enforce generic tag and compact-wire budgets,
/// and bind these details to a per-revision `created_at` before signing.
///
/// ```compile_fail
/// let _: radroots_event::food_availability::RadrootsFoodAvailabilityDetails =
///     serde_json::from_str(r#"{"content":"carrots"}"#).unwrap();
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsFoodAvailabilityDetails {
    content: RadrootsFoodContent,
    identifier: RadrootsFoodIdentifier,
    title: RadrootsFoodText,
    summary: RadrootsFoodText,
    published_at: RadrootsFoodPublishedAt,
    location: RadrootsFoodText,
    price: RadrootsFoodPrice,
    quantity: Option<RadrootsFoodQuantity>,
    status: RadrootsFoodAvailabilityStatus,
    images: Vec<RadrootsFoodAvailabilityImage>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsFoodAvailabilityDetailsParts {
    pub content: RadrootsFoodContent,
    pub identifier: RadrootsFoodIdentifier,
    pub title: RadrootsFoodText,
    pub summary: RadrootsFoodText,
    pub published_at: RadrootsFoodPublishedAt,
    pub location: RadrootsFoodText,
    pub price: RadrootsFoodPrice,
    pub quantity: Option<RadrootsFoodQuantity>,
    pub status: RadrootsFoodAvailabilityStatus,
    pub images: Vec<RadrootsFoodAvailabilityImage>,
}

impl RadrootsFoodAvailabilityDetails {
    pub fn new(
        parts: RadrootsFoodAvailabilityDetailsParts,
    ) -> Result<Self, RadrootsFoodAvailabilityError> {
        if parts
            .quantity
            .as_ref()
            .is_some_and(|quantity| quantity.unit() != parts.price.unit())
        {
            return Err(RadrootsFoodAvailabilityError::QuantityInvalid);
        }
        validate_images(&parts.images)?;
        Ok(Self {
            content: parts.content,
            identifier: parts.identifier,
            title: parts.title,
            summary: parts.summary,
            published_at: parts.published_at,
            location: parts.location,
            price: parts.price,
            quantity: parts.quantity,
            status: parts.status,
            images: parts.images,
        })
    }

    pub fn validate_created_at(
        &self,
        created_at: u64,
    ) -> Result<(), RadrootsFoodAvailabilityError> {
        self.published_at.validate_created_at(created_at)
    }

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

    pub fn images(&self) -> &[RadrootsFoodAvailabilityImage] {
        &self.images
    }
}

fn validate_images(
    images: &[RadrootsFoodAvailabilityImage],
) -> Result<(), RadrootsFoodAvailabilityError> {
    if images.len() > RADROOTS_FOOD_IMAGE_MAX_COUNT {
        return Err(RadrootsFoodAvailabilityError::ImageCountExceeded {
            max: RADROOTS_FOOD_IMAGE_MAX_COUNT,
            actual: images.len(),
        });
    }
    for (index, image) in images.iter().enumerate() {
        if images[..index]
            .iter()
            .any(|candidate| candidate.url() == image.url())
        {
            return Err(RadrootsFoodAvailabilityError::ImageDuplicateUrl);
        }
        let digest = image.image().descriptor().sha256();
        if images[..index]
            .iter()
            .any(|candidate| candidate.image().descriptor().sha256() == digest)
        {
            return Err(RadrootsFoodAvailabilityError::ImageDuplicateDigest);
        }
    }
    Ok(())
}

/// Returns whether an inbound FoodAvailability image is a structural HTTP(S) URL.
///
/// This is deliberately broader than strict authored Blossom policy. Success
/// makes no byte-verification, upload, reachability, or media-safety claim.
pub fn food_media_http_url_is_valid(value: &str) -> bool {
    if !value.contains("://")
        || value.chars().any(|character| {
            character.is_whitespace()
                || matches!(
                    get_general_category(character),
                    GeneralCategory::Control | GeneralCategory::Format
                )
        })
    {
        return false;
    }

    let Ok(url) = Url::parse(value) else {
        return false;
    };
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return false;
    }

    let Some((raw_host, raw_path)) = raw_food_media_host_and_path(value) else {
        return false;
    };
    if raw_path.is_empty() || !raw_path.starts_with('/') {
        return false;
    }

    match url.host() {
        Some(Host::Domain(_)) => raw_food_dns_host_is_valid(raw_host),
        Some(Host::Ipv4(_)) => raw_host.is_ascii() && !raw_host.is_empty(),
        Some(Host::Ipv6(_)) => raw_host.is_ascii() && !raw_host.is_empty(),
        None => false,
    }
}

/// Extracts a structural Blossom hash from an otherwise valid inbound URL.
///
/// A `None` result does not invalidate a standard inbound NIP-58 image URL; it
/// only means duplicate-digest diagnostics cannot be derived from its path.
pub fn food_media_blossom_digest(value: &str) -> Option<RadrootsBlossomSha256> {
    if !food_media_http_url_is_valid(value) {
        return None;
    }
    let url = Url::parse(value).ok()?;
    RadrootsBlossomHashPath::parse(url.path())
        .ok()
        .map(|path| path.hash())
}

fn raw_food_media_host_and_path(value: &str) -> Option<(&str, &str)> {
    let (_, remainder) = value.split_once("://")?;
    let authority_end = remainder.find(['/', '?', '#']).unwrap_or(remainder.len());
    let authority = &remainder[..authority_end];
    if authority.is_empty() || authority.contains('@') {
        return None;
    }
    let path_and_suffix = &remainder[authority_end..];
    let path_end = path_and_suffix
        .find(['?', '#'])
        .unwrap_or(path_and_suffix.len());
    let raw_path = &path_and_suffix[..path_end];

    let raw_host = if let Some(bracketed) = authority.strip_prefix('[') {
        let (host, suffix) = bracketed.split_once(']')?;
        if !suffix.is_empty() && !suffix.starts_with(':') {
            return None;
        }
        host
    } else if let Some((host, _port)) = authority.rsplit_once(':') {
        if host.contains(':') {
            return None;
        }
        host
    } else {
        authority
    };

    Some((raw_host, raw_path))
}

fn raw_food_dns_host_is_valid(host: &str) -> bool {
    !host.is_empty()
        && host.is_ascii()
        && host.len() <= 253
        && host.split('.').all(|label| {
            let bytes = label.as_bytes();
            !bytes.is_empty()
                && bytes.len() <= 63
                && bytes.first().is_some_and(u8::is_ascii_alphanumeric)
                && bytes.last().is_some_and(u8::is_ascii_alphanumeric)
                && bytes
                    .iter()
                    .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'-')
        })
}

fn validate_canonical_decimal(value: &str) -> bool {
    let mut digits = 0usize;
    let mut seen_dot = false;
    let mut digit_after_dot = false;
    for byte in value.bytes() {
        match byte {
            b'0'..=b'9' => {
                digits += 1;
                digit_after_dot |= seen_dot;
            }
            b'.' if !seen_dot => seen_dot = true,
            _ => return false,
        }
    }
    if digits == 0 || digits > RADROOTS_FOOD_DECIMAL_MAX_DIGITS {
        return false;
    }
    if seen_dot && (!digit_after_dot || value.ends_with('0')) {
        return false;
    }
    let integer = value.split_once('.').map_or(value, |(integer, _)| integer);
    !integer.is_empty() && (integer == "0" || !integer.starts_with('0'))
}

fn canonical_unsigned_integer(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|byte| byte.is_ascii_digit())
        && (value == "0" || !value.starts_with('0'))
}

fn is_food_contract_whitespace(character: char) -> bool {
    character.is_whitespace() || matches!(character, '\u{1c}'..='\u{1f}')
}

fn is_control_or_format(character: char) -> bool {
    matches!(
        get_general_category(character),
        GeneralCategory::Control | GeneralCategory::Format
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use radroots_blossom::{
        RadrootsBlossomBlobDescriptor, RadrootsBlossomBlobUrl, RadrootsBlossomMediaType,
        RadrootsBlossomSha256,
    };

    #[test]
    fn error_codes_and_messages_are_stable_for_every_variant() {
        let cases = [
            (
                RadrootsFoodAvailabilityError::ContentMissing,
                "food_content_missing",
            ),
            (
                RadrootsFoodAvailabilityError::ContentTooLarge { max: 1, actual: 2 },
                "food_content_too_large",
            ),
            (
                RadrootsFoodAvailabilityError::IdentifierInvalid,
                "food_identifier_invalid",
            ),
            (
                RadrootsFoodAvailabilityError::IdentifierTooLarge { max: 1, actual: 2 },
                "food_identifier_invalid",
            ),
            (
                RadrootsFoodAvailabilityError::TextInvalid,
                "food_text_invalid",
            ),
            (
                RadrootsFoodAvailabilityError::TextTooLarge { max: 1, actual: 2 },
                "food_text_invalid",
            ),
            (
                RadrootsFoodAvailabilityError::PublishedAtInvalid,
                "food_published_at_invalid",
            ),
            (
                RadrootsFoodAvailabilityError::PublishedAtFuture {
                    published_at: 2,
                    created_at: 1,
                },
                "food_published_at_future",
            ),
            (RadrootsFoodAvailabilityError::PriceInvalid, "price_invalid"),
            (
                RadrootsFoodAvailabilityError::PriceCurrencyInvalid,
                "price_currency_invalid",
            ),
            (
                RadrootsFoodAvailabilityError::PriceUnitInvalid,
                "price_unit_invalid",
            ),
            (
                RadrootsFoodAvailabilityError::QuantityInvalid,
                "quantity_invalid",
            ),
            (RadrootsFoodAvailabilityError::QuantityZero, "quantity_zero"),
            (
                RadrootsFoodAvailabilityError::StatusInvalid,
                "food_status_invalid",
            ),
            (
                RadrootsFoodAvailabilityError::ImageDimensionsInvalid,
                "food_image_dimensions_invalid",
            ),
            (
                RadrootsFoodAvailabilityError::ImageCountExceeded { max: 1, actual: 2 },
                "food_image_count_exceeded",
            ),
            (
                RadrootsFoodAvailabilityError::ImageDuplicateUrl,
                "food_image_duplicate_url",
            ),
            (
                RadrootsFoodAvailabilityError::ImageDuplicateDigest,
                "food_image_duplicate_digest",
            ),
        ];

        for (error, code) in cases {
            assert_eq!(error.code(), code);
            assert!(!error.to_string().is_empty(), "{code}");
        }
    }

    #[test]
    fn content_enforces_only_non_whitespace_and_utf8_byte_bound() {
        for invalid in [
            "",
            " \t",
            "\u{1c}",
            "\u{1d}",
            "\u{1e}",
            "\u{1f}",
            "\u{1c}\u{2003}\t",
        ] {
            assert_eq!(
                RadrootsFoodContent::new(invalid).unwrap_err(),
                RadrootsFoodAvailabilityError::ContentMissing,
                "{invalid:?}"
            );
        }
        let exact = "é".repeat(RADROOTS_FOOD_CONTENT_MAX_BYTES / 2);
        assert_eq!(
            RadrootsFoodContent::new(exact.clone())
                .unwrap()
                .as_str()
                .len(),
            RADROOTS_FOOD_CONTENT_MAX_BYTES
        );
        assert_eq!(
            RadrootsFoodContent::new(exact + "a").unwrap_err(),
            RadrootsFoodAvailabilityError::ContentTooLarge {
                max: RADROOTS_FOOD_CONTENT_MAX_BYTES,
                actual: RADROOTS_FOOD_CONTENT_MAX_BYTES + 1,
            }
        );
        assert!(RadrootsFoodContent::new(" harvest\nnotes ").is_ok());
        assert!(RadrootsFoodContent::new("carrots\u{1c}").is_ok());
    }

    #[test]
    fn inbound_food_media_urls_are_structural_without_claiming_blossom() {
        let hash = RadrootsBlossomSha256::digest(b"carrots").to_string();
        for valid in [
            format!("https://media.example/{hash}.webp"),
            format!("http://media.example:0/{hash}?download=1"),
            "https://media.example/not-a-blossom-path.jpg".to_string(),
            format!("https://[::1]/{hash}"),
        ] {
            assert!(food_media_http_url_is_valid(&valid), "{valid}");
        }
        for invalid in [
            format!("ftp://media.example/{hash}"),
            format!("https://user@media.example/{hash}"),
            "https://media.example".to_string(),
            format!("https://média.example/{hash}"),
            format!("https://media.example/\u{200b}{hash}"),
        ] {
            assert!(!food_media_http_url_is_valid(&invalid), "{invalid}");
        }

        let blossom = format!("https://media.example/{hash}.webp?download=1");
        assert_eq!(
            food_media_blossom_digest(&blossom),
            Some(RadrootsBlossomSha256::from_hex(&hash).unwrap())
        );
        assert_eq!(
            food_media_blossom_digest("https://media.example/not-a-hash.jpg"),
            None
        );
    }

    #[test]
    fn identifier_enforces_bytes_whitespace_and_unicode_categories() {
        let exact = "a".repeat(RADROOTS_FOOD_IDENTIFIER_MAX_BYTES);
        assert_eq!(
            RadrootsFoodIdentifier::parse(&exact).unwrap().as_str(),
            exact
        );
        assert!(matches!(
            RadrootsFoodIdentifier::parse(exact + "a"),
            Err(RadrootsFoodAvailabilityError::IdentifierTooLarge { .. })
        ));
        for invalid in [
            "",
            "fresh carrots",
            "carrots\0",
            "carrots\u{85}",
            "carrots\u{200b}",
            "carrots\u{2060}",
        ] {
            assert_eq!(
                RadrootsFoodIdentifier::parse(invalid).unwrap_err().code(),
                "food_identifier_invalid"
            );
        }
    }

    #[test]
    fn text_enforces_trim_bytes_and_unicode_categories() {
        let exact = "é".repeat(RADROOTS_FOOD_TEXT_MAX_BYTES / 2);
        assert_eq!(
            RadrootsFoodText::new(exact.clone()).unwrap().as_str(),
            exact
        );
        assert!(matches!(
            RadrootsFoodText::new(exact + "a"),
            Err(RadrootsFoodAvailabilityError::TextTooLarge { .. })
        ));
        for invalid in [
            "",
            " Carrots",
            "Carrots ",
            "Car\nrots",
            "Car\u{85}rots",
            "Car\u{200b}rots",
            "Car\u{2060}rots",
        ] {
            assert_eq!(
                RadrootsFoodText::new(invalid).unwrap_err().code(),
                "food_text_invalid"
            );
        }
    }

    #[test]
    fn published_at_is_nonzero_canonical_u64_and_not_in_the_future() {
        assert_eq!(
            RadrootsFoodPublishedAt::new(0).unwrap_err(),
            RadrootsFoodAvailabilityError::PublishedAtInvalid
        );
        for invalid in ["", "0", "01", "+1", "18446744073709551616"] {
            assert_eq!(
                RadrootsFoodPublishedAt::parse(invalid).unwrap_err(),
                RadrootsFoodAvailabilityError::PublishedAtInvalid
            );
        }
        let timestamp = RadrootsFoodPublishedAt::parse("18446744073709551615").unwrap();
        assert_eq!(timestamp.as_u64(), u64::MAX);
        assert_eq!(timestamp.to_string(), u64::MAX.to_string());
        assert_eq!(
            RadrootsFoodPublishedAt::new(11)
                .unwrap()
                .validate_created_at(10)
                .unwrap_err(),
            RadrootsFoodAvailabilityError::PublishedAtFuture {
                published_at: 11,
                created_at: 10,
            }
        );
    }

    #[test]
    fn food_units_are_exact_and_closed() {
        let cases = [
            ("g", RadrootsFoodUnit::Gram),
            ("kg", RadrootsFoodUnit::Kilogram),
            ("lb", RadrootsFoodUnit::Pound),
            ("oz", RadrootsFoodUnit::Ounce),
            ("each", RadrootsFoodUnit::Each),
            ("dozen", RadrootsFoodUnit::Dozen),
            ("bunch", RadrootsFoodUnit::Bunch),
            ("punnet", RadrootsFoodUnit::Punnet),
            ("bag", RadrootsFoodUnit::Bag),
            ("basket", RadrootsFoodUnit::Basket),
        ];
        for (wire, unit) in cases {
            assert_eq!(RadrootsFoodUnit::parse(wire).unwrap(), unit);
            assert_eq!(unit.as_str(), wire);
            assert_eq!(unit.to_string(), wire);
        }
        for invalid in ["", "G", "lbs", "crate", " each"] {
            assert_eq!(
                RadrootsFoodUnit::parse(invalid).unwrap_err(),
                RadrootsFoodAvailabilityError::PriceUnitInvalid
            );
        }
    }

    #[test]
    fn price_decimal_is_canonical_bounded_and_may_be_zero() {
        let currency = RadrootsFoodCurrency::parse("CAD").unwrap();
        for valid in ["0", "1", "0.1", "10.25", "1234567890123456789012345678"] {
            let price =
                RadrootsFoodPrice::new(valid, currency.clone(), RadrootsFoodUnit::Pound).unwrap();
            assert_eq!(price.amount(), valid);
        }
        for invalid in [
            "",
            ".1",
            "1.",
            "00",
            "01",
            "01.2",
            "1.0",
            "1.20",
            "+1",
            "-1",
            "1e3",
            "١",
            "12345678901234567890123456789",
        ] {
            assert_eq!(
                RadrootsFoodPrice::new(invalid, currency.clone(), RadrootsFoodUnit::Pound)
                    .unwrap_err(),
                RadrootsFoodAvailabilityError::PriceInvalid
            );
        }
    }

    #[test]
    fn currency_is_three_uppercase_ascii_letters_without_registry_semantics() {
        for valid in ["CAD", "USD", "ZZZ"] {
            assert_eq!(RadrootsFoodCurrency::parse(valid).unwrap().as_str(), valid);
        }
        for invalid in ["", "CA", "CADD", "cad", "C1D", "CÁD"] {
            assert_eq!(
                RadrootsFoodCurrency::parse(invalid).unwrap_err(),
                RadrootsFoodAvailabilityError::PriceCurrencyInvalid
            );
        }
    }

    #[test]
    fn quantity_is_positive_canonical_and_retains_its_unit() {
        let quantity = RadrootsFoodQuantity::new("20.5", RadrootsFoodUnit::Pound).unwrap();
        assert_eq!(quantity.amount(), "20.5");
        assert_eq!(quantity.unit(), RadrootsFoodUnit::Pound);
        assert_eq!(
            RadrootsFoodQuantity::new("0", RadrootsFoodUnit::Pound).unwrap_err(),
            RadrootsFoodAvailabilityError::QuantityZero
        );
        assert_eq!(
            RadrootsFoodQuantity::new("01", RadrootsFoodUnit::Pound).unwrap_err(),
            RadrootsFoodAvailabilityError::QuantityInvalid
        );
    }

    #[test]
    fn status_is_exact_and_withdrawal_is_not_a_status() {
        assert_eq!(
            RadrootsFoodAvailabilityStatus::parse("active").unwrap(),
            RadrootsFoodAvailabilityStatus::Active
        );
        assert_eq!(
            RadrootsFoodAvailabilityStatus::parse("sold").unwrap(),
            RadrootsFoodAvailabilityStatus::Sold
        );
        for invalid in ["", "Active", "withdrawn"] {
            assert_eq!(
                RadrootsFoodAvailabilityStatus::parse(invalid).unwrap_err(),
                RadrootsFoodAvailabilityError::StatusInvalid
            );
        }
    }

    #[test]
    fn image_dimensions_are_canonical_nonzero_u32_values() {
        let dimensions = RadrootsFoodImageDimensions::new(u32::MAX, 1).unwrap();
        assert_eq!(dimensions.width(), u32::MAX);
        assert_eq!(dimensions.height(), 1);
        assert_eq!(dimensions.to_string(), "4294967295x1");
        assert_eq!(
            RadrootsFoodImageDimensions::parse("800x600").unwrap(),
            RadrootsFoodImageDimensions::new(800, 600).unwrap()
        );
        for invalid in [
            "",
            "0x1",
            "1x0",
            "01x1",
            "1x01",
            "1X1",
            "1x1x1",
            "4294967296x1",
        ] {
            assert_eq!(
                RadrootsFoodImageDimensions::parse(invalid).unwrap_err(),
                RadrootsFoodAvailabilityError::ImageDimensionsInvalid
            );
        }
    }

    #[test]
    fn details_enforce_quantity_unit_and_created_at() {
        let mut parts = details_parts(Vec::new());
        parts.quantity = Some(RadrootsFoodQuantity::new("12", RadrootsFoodUnit::Kilogram).unwrap());
        assert_eq!(
            RadrootsFoodAvailabilityDetails::new(parts).unwrap_err(),
            RadrootsFoodAvailabilityError::QuantityInvalid
        );

        let details = RadrootsFoodAvailabilityDetails::new(details_parts(Vec::new())).unwrap();
        details.validate_created_at(100).unwrap();
        assert_eq!(details.identifier().as_str(), "nantes-carrots");
        assert_eq!(details.title().as_str(), "Nantes Carrots");
        assert_eq!(details.summary().as_str(), "Fresh bunches");
        assert_eq!(details.location().as_str(), "Central Saanich, BC");
        assert_eq!(
            details.content().as_str(),
            "Nantes carrots available this week."
        );
        assert_eq!(details.price().amount(), "4");
        assert_eq!(details.price().currency().as_str(), "CAD");
        assert_eq!(details.quantity().unwrap().amount(), "24");
        assert_eq!(details.status(), RadrootsFoodAvailabilityStatus::Active);
        assert!(details.images().is_empty());
        assert_eq!(
            details.validate_created_at(99).unwrap_err().code(),
            "food_published_at_future"
        );
    }

    #[test]
    fn details_bound_images_and_apply_url_before_digest_duplicate_precedence() {
        let dimensions = RadrootsFoodImageDimensions::new(800, 600).unwrap();
        let images = (0..RADROOTS_FOOD_IMAGE_MAX_COUNT)
            .map(|index| {
                food_image(
                    "https://media.example",
                    index.to_be_bytes().as_slice(),
                    dimensions,
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            RadrootsFoodAvailabilityDetails::new(details_parts(images))
                .unwrap()
                .images()
                .len(),
            RADROOTS_FOOD_IMAGE_MAX_COUNT
        );
        let too_many = (0..=RADROOTS_FOOD_IMAGE_MAX_COUNT)
            .map(|index| {
                food_image(
                    "https://media.example",
                    index.to_be_bytes().as_slice(),
                    dimensions,
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            RadrootsFoodAvailabilityDetails::new(details_parts(too_many)).unwrap_err(),
            RadrootsFoodAvailabilityError::ImageCountExceeded {
                max: RADROOTS_FOOD_IMAGE_MAX_COUNT,
                actual: RADROOTS_FOOD_IMAGE_MAX_COUNT + 1,
            }
        );

        let image = food_image("https://media.example", b"carrot", dimensions);
        assert_eq!(
            RadrootsFoodAvailabilityDetails::new(details_parts(vec![image.clone(), image]))
                .unwrap_err(),
            RadrootsFoodAvailabilityError::ImageDuplicateUrl
        );
        assert_eq!(
            RadrootsFoodAvailabilityDetails::new(details_parts(vec![
                food_image("https://media.example", b"same", dimensions),
                food_image("https://cache.example", b"same", dimensions),
            ]))
            .unwrap_err(),
            RadrootsFoodAvailabilityError::ImageDuplicateDigest
        );
    }

    fn details_parts(
        images: Vec<RadrootsFoodAvailabilityImage>,
    ) -> RadrootsFoodAvailabilityDetailsParts {
        RadrootsFoodAvailabilityDetailsParts {
            content: RadrootsFoodContent::new("Nantes carrots available this week.").unwrap(),
            identifier: RadrootsFoodIdentifier::parse("nantes-carrots").unwrap(),
            title: RadrootsFoodText::new("Nantes Carrots").unwrap(),
            summary: RadrootsFoodText::new("Fresh bunches").unwrap(),
            published_at: RadrootsFoodPublishedAt::new(100).unwrap(),
            location: RadrootsFoodText::new("Central Saanich, BC").unwrap(),
            price: RadrootsFoodPrice::new(
                "4",
                RadrootsFoodCurrency::parse("CAD").unwrap(),
                RadrootsFoodUnit::Pound,
            )
            .unwrap(),
            quantity: Some(RadrootsFoodQuantity::new("24", RadrootsFoodUnit::Pound).unwrap()),
            status: RadrootsFoodAvailabilityStatus::Active,
            images,
        }
    }

    fn food_image(
        origin: &str,
        bytes: &[u8],
        dimensions: RadrootsFoodImageDimensions,
    ) -> RadrootsFoodAvailabilityImage {
        let hash = RadrootsBlossomSha256::digest(bytes);
        let media_type = RadrootsBlossomMediaType::parse("image/webp").unwrap();
        let descriptor = RadrootsBlossomBlobDescriptor::new(
            RadrootsBlossomBlobUrl::parse(&format!("{origin}/{hash}.webp")).unwrap(),
            hash,
            bytes.len() as u64,
            media_type.clone(),
            1_784_347_200,
        )
        .unwrap()
        .approve_reference()
        .unwrap()
        .verify_bytes(bytes, &media_type)
        .unwrap();
        RadrootsFoodAvailabilityImage::new(
            RadrootsAuthoredImage::try_from(descriptor).unwrap(),
            dimensions,
        )
    }
}
