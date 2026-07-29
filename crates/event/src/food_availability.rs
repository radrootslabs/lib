#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};
#[cfg(feature = "std")]
use std::{string::String, vec::Vec};

use core::{fmt, str::FromStr};
use radroots_blossom::hash::{HashPath, Sha256};
use unicode_general_category::{GeneralCategory, get_general_category};
use url_nostd::{Host, Url};

use crate::media::AuthoredImage;

pub const RADROOTS_FOOD_CONTENT_MAX_BYTES: usize = 128 * 1024;
pub const RADROOTS_FOOD_IDENTIFIER_MAX_BYTES: usize = 512;
pub const RADROOTS_FOOD_TEXT_MAX_BYTES: usize = 4 * 1024;
pub const RADROOTS_FOOD_DECIMAL_MAX_DIGITS: usize = 28;
pub const RADROOTS_FOOD_IMAGE_MAX_COUNT: usize = 64;
pub const RADROOTS_FOOD_AVAILABILITY_CONTRACT_ID: &str = "radroots.food.availability.v1";

/// Errors raised while constructing strict FoodAvailability details.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FoodAvailabilityError {
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

impl FoodAvailabilityError {
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

impl fmt::Display for FoodAvailabilityError {
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
impl std::error::Error for FoodAvailabilityError {}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FoodContent(String);

impl FoodContent {
    pub fn new(value: impl Into<String>) -> Result<Self, FoodAvailabilityError> {
        let value = value.into();
        if value.chars().all(is_food_contract_whitespace) {
            return Err(FoodAvailabilityError::ContentMissing);
        }
        if value.len() > RADROOTS_FOOD_CONTENT_MAX_BYTES {
            return Err(FoodAvailabilityError::ContentTooLarge {
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

impl AsRef<str> for FoodContent {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for FoodContent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FoodIdentifier(String);

impl FoodIdentifier {
    pub fn parse(value: impl AsRef<str>) -> Result<Self, FoodAvailabilityError> {
        let value = value.as_ref();
        if value.is_empty()
            || value
                .chars()
                .any(|character| character.is_whitespace() || is_control_or_format(character))
        {
            return Err(FoodAvailabilityError::IdentifierInvalid);
        }
        if value.len() > RADROOTS_FOOD_IDENTIFIER_MAX_BYTES {
            return Err(FoodAvailabilityError::IdentifierTooLarge {
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

impl AsRef<str> for FoodIdentifier {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for FoodIdentifier {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for FoodIdentifier {
    type Err = FoodAvailabilityError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FoodText(String);

impl FoodText {
    pub fn new(value: impl Into<String>) -> Result<Self, FoodAvailabilityError> {
        let value = value.into();
        if value.is_empty() || value.trim() != value || value.chars().any(is_control_or_format) {
            return Err(FoodAvailabilityError::TextInvalid);
        }
        if value.len() > RADROOTS_FOOD_TEXT_MAX_BYTES {
            return Err(FoodAvailabilityError::TextTooLarge {
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

impl AsRef<str> for FoodText {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for FoodText {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FoodPublishedAt(u64);

impl FoodPublishedAt {
    pub const fn new(value: u64) -> Result<Self, FoodAvailabilityError> {
        if value == 0 {
            return Err(FoodAvailabilityError::PublishedAtInvalid);
        }
        Ok(Self(value))
    }

    pub fn parse(value: &str) -> Result<Self, FoodAvailabilityError> {
        if !canonical_unsigned_integer(value) {
            return Err(FoodAvailabilityError::PublishedAtInvalid);
        }
        value
            .parse::<u64>()
            .ok()
            .and_then(|parsed| Self::new(parsed).ok())
            .ok_or(FoodAvailabilityError::PublishedAtInvalid)
    }

    pub const fn as_u64(self) -> u64 {
        self.0
    }

    pub const fn validate_created_at(self, created_at: u64) -> Result<(), FoodAvailabilityError> {
        if self.0 > created_at {
            return Err(FoodAvailabilityError::PublishedAtFuture {
                published_at: self.0,
                created_at,
            });
        }
        Ok(())
    }
}

impl fmt::Display for FoodPublishedAt {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

impl FromStr for FoodPublishedAt {
    type Err = FoodAvailabilityError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FoodCurrency(String);

impl FoodCurrency {
    pub fn parse(value: impl AsRef<str>) -> Result<Self, FoodAvailabilityError> {
        let value = value.as_ref();
        if value.len() != 3 || !value.bytes().all(|byte| byte.is_ascii_uppercase()) {
            return Err(FoodAvailabilityError::PriceCurrencyInvalid);
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

impl AsRef<str> for FoodCurrency {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for FoodCurrency {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for FoodCurrency {
    type Err = FoodAvailabilityError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FoodUnit {
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

impl FoodUnit {
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

    pub fn parse(value: &str) -> Result<Self, FoodAvailabilityError> {
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
            _ => Err(FoodAvailabilityError::PriceUnitInvalid),
        }
    }
}

impl fmt::Display for FoodUnit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for FoodUnit {
    type Err = FoodAvailabilityError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FoodPrice {
    amount: String,
    currency: FoodCurrency,
    unit: FoodUnit,
}

impl FoodPrice {
    pub fn new(
        amount: impl Into<String>,
        currency: FoodCurrency,
        unit: FoodUnit,
    ) -> Result<Self, FoodAvailabilityError> {
        let amount = amount.into();
        validate_canonical_decimal(&amount)
            .then_some(())
            .ok_or(FoodAvailabilityError::PriceInvalid)?;
        Ok(Self {
            amount,
            currency,
            unit,
        })
    }

    pub fn amount(&self) -> &str {
        self.amount.as_str()
    }

    pub fn currency(&self) -> &FoodCurrency {
        &self.currency
    }

    pub const fn unit(&self) -> FoodUnit {
        self.unit
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FoodQuantity {
    amount: String,
    unit: FoodUnit,
}

impl FoodQuantity {
    pub fn new(amount: impl Into<String>, unit: FoodUnit) -> Result<Self, FoodAvailabilityError> {
        let amount = amount.into();
        if !validate_canonical_decimal(&amount) {
            return Err(FoodAvailabilityError::QuantityInvalid);
        }
        if !amount.bytes().any(|byte| matches!(byte, b'1'..=b'9')) {
            return Err(FoodAvailabilityError::QuantityZero);
        }
        Ok(Self { amount, unit })
    }

    pub fn amount(&self) -> &str {
        self.amount.as_str()
    }

    pub const fn unit(&self) -> FoodUnit {
        self.unit
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FoodAvailabilityStatus {
    Active,
    Sold,
}

impl FoodAvailabilityStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Sold => "sold",
        }
    }

    pub fn parse(value: &str) -> Result<Self, FoodAvailabilityError> {
        match value {
            "active" => Ok(Self::Active),
            "sold" => Ok(Self::Sold),
            _ => Err(FoodAvailabilityError::StatusInvalid),
        }
    }
}

impl fmt::Display for FoodAvailabilityStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for FoodAvailabilityStatus {
    type Err = FoodAvailabilityError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FoodImageDimensions {
    width: u32,
    height: u32,
}

impl FoodImageDimensions {
    pub const fn new(width: u32, height: u32) -> Result<Self, FoodAvailabilityError> {
        if width == 0 || height == 0 {
            return Err(FoodAvailabilityError::ImageDimensionsInvalid);
        }
        Ok(Self { width, height })
    }

    pub fn parse(value: &str) -> Result<Self, FoodAvailabilityError> {
        let Some((width, height)) = value.split_once('x') else {
            return Err(FoodAvailabilityError::ImageDimensionsInvalid);
        };
        if height.contains('x')
            || !canonical_unsigned_integer(width)
            || !canonical_unsigned_integer(height)
        {
            return Err(FoodAvailabilityError::ImageDimensionsInvalid);
        }
        let width = width
            .parse::<u32>()
            .map_err(|_| FoodAvailabilityError::ImageDimensionsInvalid)?;
        let height = height
            .parse::<u32>()
            .map_err(|_| FoodAvailabilityError::ImageDimensionsInvalid)?;
        Self::new(width, height)
    }

    pub const fn width(self) -> u32 {
        self.width
    }

    pub const fn height(self) -> u32 {
        self.height
    }
}

impl fmt::Display for FoodImageDimensions {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}x{}", self.width, self.height)
    }
}

impl FromStr for FoodImageDimensions {
    type Err = FoodAvailabilityError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

/// One byte-verified Blossom image and its declared NIP-58 dimensions.
///
/// This state proves descriptor-to-byte agreement and an `image/*` media type.
/// It does not prove upload completion, raster decoding, or network availability.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FoodAvailabilityImage {
    image: AuthoredImage,
    dimensions: FoodImageDimensions,
}

impl FoodAvailabilityImage {
    pub const fn new(image: AuthoredImage, dimensions: FoodImageDimensions) -> Self {
        Self { image, dimensions }
    }

    pub fn image(&self) -> &AuthoredImage {
        &self.image
    }

    pub const fn dimensions(&self) -> FoodImageDimensions {
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
/// let _: radroots_event::food::availability::FoodAvailabilityDetails =
///     serde_json::from_str(r#"{"content":"carrots"}"#).unwrap();
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FoodAvailabilityDetails {
    content: FoodContent,
    identifier: FoodIdentifier,
    title: FoodText,
    summary: FoodText,
    published_at: FoodPublishedAt,
    location: FoodText,
    price: FoodPrice,
    quantity: Option<FoodQuantity>,
    status: FoodAvailabilityStatus,
    images: Vec<FoodAvailabilityImage>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FoodAvailabilityDetailsParts {
    pub content: FoodContent,
    pub identifier: FoodIdentifier,
    pub title: FoodText,
    pub summary: FoodText,
    pub published_at: FoodPublishedAt,
    pub location: FoodText,
    pub price: FoodPrice,
    pub quantity: Option<FoodQuantity>,
    pub status: FoodAvailabilityStatus,
    pub images: Vec<FoodAvailabilityImage>,
}

impl FoodAvailabilityDetails {
    pub fn new(parts: FoodAvailabilityDetailsParts) -> Result<Self, FoodAvailabilityError> {
        if parts
            .quantity
            .as_ref()
            .is_some_and(|quantity| quantity.unit() != parts.price.unit())
        {
            return Err(FoodAvailabilityError::QuantityInvalid);
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

    pub fn validate_created_at(&self, created_at: u64) -> Result<(), FoodAvailabilityError> {
        self.published_at.validate_created_at(created_at)
    }

    pub fn content(&self) -> &FoodContent {
        &self.content
    }

    pub fn identifier(&self) -> &FoodIdentifier {
        &self.identifier
    }

    pub fn title(&self) -> &FoodText {
        &self.title
    }

    pub fn summary(&self) -> &FoodText {
        &self.summary
    }

    pub const fn published_at(&self) -> FoodPublishedAt {
        self.published_at
    }

    pub fn location(&self) -> &FoodText {
        &self.location
    }

    pub fn price(&self) -> &FoodPrice {
        &self.price
    }

    pub fn quantity(&self) -> Option<&FoodQuantity> {
        self.quantity.as_ref()
    }

    pub const fn status(&self) -> FoodAvailabilityStatus {
        self.status
    }

    pub fn images(&self) -> &[FoodAvailabilityImage] {
        &self.images
    }
}

fn validate_images(images: &[FoodAvailabilityImage]) -> Result<(), FoodAvailabilityError> {
    if images.len() > RADROOTS_FOOD_IMAGE_MAX_COUNT {
        return Err(FoodAvailabilityError::ImageCountExceeded {
            max: RADROOTS_FOOD_IMAGE_MAX_COUNT,
            actual: images.len(),
        });
    }
    for (index, image) in images.iter().enumerate() {
        if images[..index]
            .iter()
            .any(|candidate| candidate.url() == image.url())
        {
            return Err(FoodAvailabilityError::ImageDuplicateUrl);
        }
        let digest = image.image().descriptor().sha256();
        if images[..index]
            .iter()
            .any(|candidate| candidate.image().descriptor().sha256() == digest)
        {
            return Err(FoodAvailabilityError::ImageDuplicateDigest);
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
pub fn food_media_blossom_digest(value: &str) -> Option<Sha256> {
    if !food_media_http_url_is_valid(value) {
        return None;
    }
    let url = Url::parse(value).ok()?;
    HashPath::parse(url.path()).ok().map(|path| path.hash())
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
    use radroots_blossom::{BlobDescriptor, BlobUrl, MediaType, Sha256};

    #[test]
    fn error_codes_and_messages_are_stable_for_every_variant() {
        let cases = [
            (
                FoodAvailabilityError::ContentMissing,
                "food_content_missing",
            ),
            (
                FoodAvailabilityError::ContentTooLarge { max: 1, actual: 2 },
                "food_content_too_large",
            ),
            (
                FoodAvailabilityError::IdentifierInvalid,
                "food_identifier_invalid",
            ),
            (
                FoodAvailabilityError::IdentifierTooLarge { max: 1, actual: 2 },
                "food_identifier_invalid",
            ),
            (FoodAvailabilityError::TextInvalid, "food_text_invalid"),
            (
                FoodAvailabilityError::TextTooLarge { max: 1, actual: 2 },
                "food_text_invalid",
            ),
            (
                FoodAvailabilityError::PublishedAtInvalid,
                "food_published_at_invalid",
            ),
            (
                FoodAvailabilityError::PublishedAtFuture {
                    published_at: 2,
                    created_at: 1,
                },
                "food_published_at_future",
            ),
            (FoodAvailabilityError::PriceInvalid, "price_invalid"),
            (
                FoodAvailabilityError::PriceCurrencyInvalid,
                "price_currency_invalid",
            ),
            (
                FoodAvailabilityError::PriceUnitInvalid,
                "price_unit_invalid",
            ),
            (FoodAvailabilityError::QuantityInvalid, "quantity_invalid"),
            (FoodAvailabilityError::QuantityZero, "quantity_zero"),
            (FoodAvailabilityError::StatusInvalid, "food_status_invalid"),
            (
                FoodAvailabilityError::ImageDimensionsInvalid,
                "food_image_dimensions_invalid",
            ),
            (
                FoodAvailabilityError::ImageCountExceeded { max: 1, actual: 2 },
                "food_image_count_exceeded",
            ),
            (
                FoodAvailabilityError::ImageDuplicateUrl,
                "food_image_duplicate_url",
            ),
            (
                FoodAvailabilityError::ImageDuplicateDigest,
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
                FoodContent::new(invalid).unwrap_err(),
                FoodAvailabilityError::ContentMissing,
                "{invalid:?}"
            );
        }
        let exact = "é".repeat(RADROOTS_FOOD_CONTENT_MAX_BYTES / 2);
        assert_eq!(
            FoodContent::new(exact.clone()).unwrap().as_str().len(),
            RADROOTS_FOOD_CONTENT_MAX_BYTES
        );
        assert_eq!(
            FoodContent::new(exact + "a").unwrap_err(),
            FoodAvailabilityError::ContentTooLarge {
                max: RADROOTS_FOOD_CONTENT_MAX_BYTES,
                actual: RADROOTS_FOOD_CONTENT_MAX_BYTES + 1,
            }
        );
        assert!(FoodContent::new(" harvest\nnotes ").is_ok());
        assert!(FoodContent::new("carrots\u{1c}").is_ok());
    }

    #[test]
    fn inbound_food_media_urls_are_structural_without_claiming_blossom() {
        let hash = Sha256::digest(b"carrots").to_string();
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
            Some(Sha256::from_hex(&hash).unwrap())
        );
        assert_eq!(
            food_media_blossom_digest("https://media.example/not-a-hash.jpg"),
            None
        );
    }

    #[test]
    fn identifier_enforces_bytes_whitespace_and_unicode_categories() {
        let exact = "a".repeat(RADROOTS_FOOD_IDENTIFIER_MAX_BYTES);
        assert_eq!(FoodIdentifier::parse(&exact).unwrap().as_str(), exact);
        assert!(matches!(
            FoodIdentifier::parse(exact + "a"),
            Err(FoodAvailabilityError::IdentifierTooLarge { .. })
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
                FoodIdentifier::parse(invalid).unwrap_err().code(),
                "food_identifier_invalid"
            );
        }
    }

    #[test]
    fn text_enforces_trim_bytes_and_unicode_categories() {
        let exact = "é".repeat(RADROOTS_FOOD_TEXT_MAX_BYTES / 2);
        assert_eq!(FoodText::new(exact.clone()).unwrap().as_str(), exact);
        assert!(matches!(
            FoodText::new(exact + "a"),
            Err(FoodAvailabilityError::TextTooLarge { .. })
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
                FoodText::new(invalid).unwrap_err().code(),
                "food_text_invalid"
            );
        }
    }

    #[test]
    fn published_at_is_nonzero_canonical_u64_and_not_in_the_future() {
        assert_eq!(
            FoodPublishedAt::new(0).unwrap_err(),
            FoodAvailabilityError::PublishedAtInvalid
        );
        for invalid in ["", "0", "01", "+1", "18446744073709551616"] {
            assert_eq!(
                FoodPublishedAt::parse(invalid).unwrap_err(),
                FoodAvailabilityError::PublishedAtInvalid
            );
        }
        let timestamp = FoodPublishedAt::parse("18446744073709551615").unwrap();
        assert_eq!(timestamp.as_u64(), u64::MAX);
        assert_eq!(timestamp.to_string(), u64::MAX.to_string());
        assert_eq!(
            FoodPublishedAt::new(11)
                .unwrap()
                .validate_created_at(10)
                .unwrap_err(),
            FoodAvailabilityError::PublishedAtFuture {
                published_at: 11,
                created_at: 10,
            }
        );
    }

    #[test]
    fn food_units_are_exact_and_closed() {
        let cases = [
            ("g", FoodUnit::Gram),
            ("kg", FoodUnit::Kilogram),
            ("lb", FoodUnit::Pound),
            ("oz", FoodUnit::Ounce),
            ("each", FoodUnit::Each),
            ("dozen", FoodUnit::Dozen),
            ("bunch", FoodUnit::Bunch),
            ("punnet", FoodUnit::Punnet),
            ("bag", FoodUnit::Bag),
            ("basket", FoodUnit::Basket),
        ];
        for (wire, unit) in cases {
            assert_eq!(FoodUnit::parse(wire).unwrap(), unit);
            assert_eq!(unit.as_str(), wire);
            assert_eq!(unit.to_string(), wire);
        }
        for invalid in ["", "G", "lbs", "crate", " each"] {
            assert_eq!(
                FoodUnit::parse(invalid).unwrap_err(),
                FoodAvailabilityError::PriceUnitInvalid
            );
        }
    }

    #[test]
    fn price_decimal_is_canonical_bounded_and_may_be_zero() {
        let currency = FoodCurrency::parse("CAD").unwrap();
        for valid in ["0", "1", "0.1", "10.25", "1234567890123456789012345678"] {
            let price = FoodPrice::new(valid, currency.clone(), FoodUnit::Pound).unwrap();
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
                FoodPrice::new(invalid, currency.clone(), FoodUnit::Pound).unwrap_err(),
                FoodAvailabilityError::PriceInvalid
            );
        }
    }

    #[test]
    fn currency_is_three_uppercase_ascii_letters_without_registry_semantics() {
        for valid in ["CAD", "USD", "ZZZ"] {
            assert_eq!(FoodCurrency::parse(valid).unwrap().as_str(), valid);
        }
        for invalid in ["", "CA", "CADD", "cad", "C1D", "CÁD"] {
            assert_eq!(
                FoodCurrency::parse(invalid).unwrap_err(),
                FoodAvailabilityError::PriceCurrencyInvalid
            );
        }
    }

    #[test]
    fn quantity_is_positive_canonical_and_retains_its_unit() {
        let quantity = FoodQuantity::new("20.5", FoodUnit::Pound).unwrap();
        assert_eq!(quantity.amount(), "20.5");
        assert_eq!(quantity.unit(), FoodUnit::Pound);
        assert_eq!(
            FoodQuantity::new("0", FoodUnit::Pound).unwrap_err(),
            FoodAvailabilityError::QuantityZero
        );
        assert_eq!(
            FoodQuantity::new("01", FoodUnit::Pound).unwrap_err(),
            FoodAvailabilityError::QuantityInvalid
        );
    }

    #[test]
    fn status_is_exact_and_withdrawal_is_not_a_status() {
        assert_eq!(
            FoodAvailabilityStatus::parse("active").unwrap(),
            FoodAvailabilityStatus::Active
        );
        assert_eq!(
            FoodAvailabilityStatus::parse("sold").unwrap(),
            FoodAvailabilityStatus::Sold
        );
        for invalid in ["", "Active", "withdrawn"] {
            assert_eq!(
                FoodAvailabilityStatus::parse(invalid).unwrap_err(),
                FoodAvailabilityError::StatusInvalid
            );
        }
    }

    #[test]
    fn image_dimensions_are_canonical_nonzero_u32_values() {
        let dimensions = FoodImageDimensions::new(u32::MAX, 1).unwrap();
        assert_eq!(dimensions.width(), u32::MAX);
        assert_eq!(dimensions.height(), 1);
        assert_eq!(dimensions.to_string(), "4294967295x1");
        assert_eq!(
            FoodImageDimensions::parse("800x600").unwrap(),
            FoodImageDimensions::new(800, 600).unwrap()
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
                FoodImageDimensions::parse(invalid).unwrap_err(),
                FoodAvailabilityError::ImageDimensionsInvalid
            );
        }
    }

    #[test]
    fn details_enforce_quantity_unit_and_created_at() {
        let mut parts = details_parts(Vec::new());
        parts.quantity = Some(FoodQuantity::new("12", FoodUnit::Kilogram).unwrap());
        assert_eq!(
            FoodAvailabilityDetails::new(parts).unwrap_err(),
            FoodAvailabilityError::QuantityInvalid
        );

        let details = FoodAvailabilityDetails::new(details_parts(Vec::new())).unwrap();
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
        assert_eq!(details.status(), FoodAvailabilityStatus::Active);
        assert!(details.images().is_empty());
        assert_eq!(
            details.validate_created_at(99).unwrap_err().code(),
            "food_published_at_future"
        );
    }

    #[test]
    fn details_bound_images_and_apply_url_before_digest_duplicate_precedence() {
        let dimensions = FoodImageDimensions::new(800, 600).unwrap();
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
            FoodAvailabilityDetails::new(details_parts(images))
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
            FoodAvailabilityDetails::new(details_parts(too_many)).unwrap_err(),
            FoodAvailabilityError::ImageCountExceeded {
                max: RADROOTS_FOOD_IMAGE_MAX_COUNT,
                actual: RADROOTS_FOOD_IMAGE_MAX_COUNT + 1,
            }
        );

        let image = food_image("https://media.example", b"carrot", dimensions);
        assert_eq!(
            FoodAvailabilityDetails::new(details_parts(vec![image.clone(), image])).unwrap_err(),
            FoodAvailabilityError::ImageDuplicateUrl
        );
        assert_eq!(
            FoodAvailabilityDetails::new(details_parts(vec![
                food_image("https://media.example", b"same", dimensions),
                food_image("https://cache.example", b"same", dimensions),
            ]))
            .unwrap_err(),
            FoodAvailabilityError::ImageDuplicateDigest
        );
    }

    fn details_parts(images: Vec<FoodAvailabilityImage>) -> FoodAvailabilityDetailsParts {
        FoodAvailabilityDetailsParts {
            content: FoodContent::new("Nantes carrots available this week.").unwrap(),
            identifier: FoodIdentifier::parse("nantes-carrots").unwrap(),
            title: FoodText::new("Nantes Carrots").unwrap(),
            summary: FoodText::new("Fresh bunches").unwrap(),
            published_at: FoodPublishedAt::new(100).unwrap(),
            location: FoodText::new("Central Saanich, BC").unwrap(),
            price: FoodPrice::new("4", FoodCurrency::parse("CAD").unwrap(), FoodUnit::Pound)
                .unwrap(),
            quantity: Some(FoodQuantity::new("24", FoodUnit::Pound).unwrap()),
            status: FoodAvailabilityStatus::Active,
            images,
        }
    }

    fn food_image(
        origin: &str,
        bytes: &[u8],
        dimensions: FoodImageDimensions,
    ) -> FoodAvailabilityImage {
        let hash = Sha256::digest(bytes);
        let media_type = MediaType::parse("image/webp").unwrap();
        let descriptor = BlobDescriptor::new(
            BlobUrl::parse(&format!("{origin}/{hash}.webp")).unwrap(),
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
        FoodAvailabilityImage::new(AuthoredImage::try_from(descriptor).unwrap(), dimensions)
    }
}
