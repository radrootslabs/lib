#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use radroots_core::{RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCoreQuantity, RadrootsCoreQuantityPrice, RadrootsCoreUnit};
use radroots_events::listing::{
    RadrootsListing, RadrootsListingAvailability, RadrootsListingDeliveryMethod,
    RadrootsListingDiscount, RadrootsListingImage, RadrootsListingImageSize, RadrootsListingLocation,
    RadrootsListingProduct, RadrootsListingQuantity, RadrootsListingStatus,
};
use radroots_events::tags::TAG_D;
use radroots_events_codec::error::EventEncodeError;
use radroots_events_codec::listing::tags::listing_tags;
#[cfg(feature = "ts-rs")]
use ts_rs::TS;

const TAG_QUANTITY: &str = "quantity";
const TAG_PRICE: &str = "price";
const TAG_PRICE_DISCOUNT_PREFIX: &str = "price-discount-";
const TAG_LOCATION: &str = "location";
const TAG_IMAGE: &str = "image";
const TAG_GEOHASH: &str = "g";
const TAG_INVENTORY: &str = "inventory";
const TAG_DELIVERY: &str = "delivery";
const TAG_PUBLISHED_AT: &str = "published_at";
const TAG_STATUS: &str = "status";
const TAG_EXPIRES_AT: &str = "expires_at";

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TradeListingParseError {
    MissingTag(String),
    InvalidTag(String),
    InvalidNumber(String),
    InvalidUnit,
    InvalidCurrency,
    InvalidJson(String),
    InvalidDiscount(String),
}

impl core::fmt::Display for TradeListingParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            TradeListingParseError::MissingTag(tag) => write!(f, "missing required tag: {tag}"),
            TradeListingParseError::InvalidTag(tag) => write!(f, "invalid tag: {tag}"),
            TradeListingParseError::InvalidNumber(field) => write!(f, "invalid number: {field}"),
            TradeListingParseError::InvalidUnit => write!(f, "invalid unit"),
            TradeListingParseError::InvalidCurrency => write!(f, "invalid currency"),
            TradeListingParseError::InvalidJson(field) => write!(f, "invalid json: {field}"),
            TradeListingParseError::InvalidDiscount(kind) => {
                write!(f, "invalid discount data for {kind}")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for TradeListingParseError {}

fn parse_decimal(s: &str, field: &str) -> Result<RadrootsCoreDecimal, TradeListingParseError> {
    s.parse::<RadrootsCoreDecimal>()
        .map_err(|_| TradeListingParseError::InvalidNumber(field.to_string()))
}

fn parse_currency(s: &str) -> Result<RadrootsCoreCurrency, TradeListingParseError> {
    let upper = s.trim().to_ascii_uppercase();
    RadrootsCoreCurrency::from_str_upper(&upper)
        .map_err(|_| TradeListingParseError::InvalidCurrency)
}

fn parse_unit(s: &str) -> Result<RadrootsCoreUnit, TradeListingParseError> {
    s.parse::<RadrootsCoreUnit>()
        .map_err(|_| TradeListingParseError::InvalidUnit)
}

fn parse_d_tag(tags: &[Vec<String>]) -> Result<String, TradeListingParseError> {
    let tag = tags
        .iter()
        .find(|t| t.get(0).map(|s| s.as_str()) == Some(TAG_D))
        .ok_or_else(|| TradeListingParseError::MissingTag(TAG_D.to_string()))?;
    let value = tag
        .get(1)
        .map(|s| s.to_string())
        .ok_or_else(|| TradeListingParseError::InvalidTag(TAG_D.to_string()))?;
    if value.trim().is_empty() {
        return Err(TradeListingParseError::InvalidTag(TAG_D.to_string()));
    }
    Ok(value)
}

pub fn listing_from_event_parts(
    tags: &[Vec<String>],
    content: &str,
) -> Result<RadrootsListing, TradeListingParseError> {
    let d_tag = parse_d_tag(tags)?;

    if !content.trim().is_empty() {
        #[cfg(feature = "serde_json")]
        {
            if let Ok(mut listing) = serde_json::from_str::<RadrootsListing>(content) {
                if listing.d_tag.trim().is_empty() {
                    listing.d_tag = d_tag;
                } else if listing.d_tag != d_tag {
                    return Err(TradeListingParseError::InvalidTag(TAG_D.to_string()));
                }
                return Ok(listing);
            }
        }
    }

    listing_from_tags(tags, d_tag)
}

pub fn listing_tags_build(listing: &RadrootsListing) -> Result<Vec<Vec<String>>, TradeListingParseError> {
    let mut tags = listing_tags(listing).map_err(map_listing_tags_error)?;
    if let Some(inventory) = &listing.inventory_available {
        tags.push(vec![TAG_INVENTORY.to_string(), inventory.to_string()]);
    }

    if let Some(availability) = &listing.availability {
        match availability {
            RadrootsListingAvailability::Status { status } => {
                tags.push(vec![TAG_STATUS.to_string(), status_as_str(status).to_string()]);
            }
            RadrootsListingAvailability::Window { start, end } => {
                if let Some(start) = start {
                    tags.push(vec![TAG_PUBLISHED_AT.to_string(), start.to_string()]);
                }
                if let Some(end) = end {
                    tags.push(vec![TAG_EXPIRES_AT.to_string(), end.to_string()]);
                }
            }
        }
    }

    if let Some(method) = &listing.delivery_method {
        let mut tag = Vec::with_capacity(3);
        tag.push(TAG_DELIVERY.to_string());
        match method {
            RadrootsListingDeliveryMethod::Pickup => tag.push("pickup".into()),
            RadrootsListingDeliveryMethod::LocalDelivery => tag.push("local_delivery".into()),
            RadrootsListingDeliveryMethod::Shipping => tag.push("shipping".into()),
            RadrootsListingDeliveryMethod::Other { method } => {
                tag.push("other".into());
                tag.push(method.clone());
            }
        }
        tags.push(tag);
    }

    if let Some(location) = &listing.location {
        let mut tag = Vec::with_capacity(5);
        tag.push(TAG_LOCATION.to_string());
        tag.push(location.primary.clone());
        if let Some(city) = location.city.as_ref().and_then(|v| clean_value(v)) {
            tag.push(city);
        }
        if let Some(region) = location.region.as_ref().and_then(|v| clean_value(v)) {
            tag.push(region);
        }
        if let Some(country) = location.country.as_ref().and_then(|v| clean_value(v)) {
            tag.push(country);
        }
        tags.push(tag);
        if let Some(geohash) = location.geohash.as_ref().and_then(|v| clean_value(v)) {
            tags.push(vec![TAG_GEOHASH.to_string(), geohash]);
        }
    }

    if let Some(images) = &listing.images {
        for image in images {
            if image.url.trim().is_empty() {
                continue;
            }
            let mut tag = Vec::with_capacity(3);
            tag.push(TAG_IMAGE.to_string());
            tag.push(image.url.clone());
            if let Some(size) = &image.size {
                tag.push(format!("{}x{}", size.w, size.h));
            }
            tags.push(tag);
        }
    }

    Ok(tags)
}

fn map_listing_tags_error(err: EventEncodeError) -> TradeListingParseError {
    match err {
        EventEncodeError::EmptyRequiredField(field) => {
            TradeListingParseError::MissingTag(field.to_string())
        }
        EventEncodeError::Json => TradeListingParseError::InvalidJson("discount".to_string()),
        EventEncodeError::InvalidKind(_) => TradeListingParseError::InvalidTag("kind".to_string()),
    }
}

fn listing_from_tags(
    tags: &[Vec<String>],
    d_tag: String,
) -> Result<RadrootsListing, TradeListingParseError> {
    let mut product = RadrootsListingProduct {
        key: String::new(),
        title: String::new(),
        category: String::new(),
        summary: None,
        process: None,
        lot: None,
        location: None,
        profile: None,
        year: None,
    };

    let mut quantities: Vec<RadrootsListingQuantity> = Vec::new();
    let mut prices: Vec<RadrootsCoreQuantityPrice> = Vec::new();
    let mut discounts: Vec<RadrootsListingDiscount> = Vec::new();
    let mut location: Option<RadrootsListingLocation> = None;
    let mut inventory_available: Option<RadrootsCoreDecimal> = None;
    let mut availability_status: Option<RadrootsListingStatus> = None;
    let mut availability_start: Option<u64> = None;
    let mut availability_end: Option<u64> = None;
    let mut delivery_method: Option<RadrootsListingDeliveryMethod> = None;
    let mut images: Vec<RadrootsListingImage> = Vec::new();
    let mut geohash: Option<String> = None;

    let has_structured_location = tags.iter().any(|tag| {
        tag.get(0).map(|k| k.as_str()) == Some(TAG_LOCATION) && tag.len() >= 3
    });

    for tag in tags {
        if tag.is_empty() {
            continue;
        }
        let key = tag[0].as_str();
        match key {
            "key" => set_if_empty(&mut product.key, tag.get(1)),
            "title" => set_if_empty(&mut product.title, tag.get(1)),
            "category" => set_if_empty(&mut product.category, tag.get(1)),
            "summary" => set_optional(&mut product.summary, tag.get(1)),
            "process" => set_optional(&mut product.process, tag.get(1)),
            "lot" => set_optional(&mut product.lot, tag.get(1)),
            "location" => {
                if tag.len() >= 3
                    || (!has_structured_location && location.is_none() && tag.len() >= 2)
                {
                    let primary =
                        tag.get(1).ok_or_else(|| TradeListingParseError::InvalidTag(TAG_LOCATION.to_string()))?;
                    if primary.trim().is_empty() {
                        return Err(TradeListingParseError::InvalidTag(TAG_LOCATION.to_string()));
                    }
                    let mut loc = RadrootsListingLocation {
                        primary: primary.to_string(),
                        city: None,
                        region: None,
                        country: None,
                        lat: None,
                        lng: None,
                        geohash: None,
                    };
                    if let Some(city) = tag.get(2).and_then(|v| clean_value(v)) {
                        loc.city = Some(city);
                    }
                    if let Some(region) = tag.get(3).and_then(|v| clean_value(v)) {
                        loc.region = Some(region);
                    }
                    if let Some(country) = tag.get(4).and_then(|v| clean_value(v)) {
                        loc.country = Some(country);
                    }
                    location = Some(loc);
                } else {
                    set_optional(&mut product.location, tag.get(1));
                }
            }
            "profile" => set_optional(&mut product.profile, tag.get(1)),
            "year" => set_optional(&mut product.year, tag.get(1)),
            TAG_QUANTITY => {
                let amount = tag.get(1).ok_or_else(|| TradeListingParseError::InvalidTag(TAG_QUANTITY.to_string()))?;
                let unit = tag.get(2).ok_or_else(|| TradeListingParseError::InvalidTag(TAG_QUANTITY.to_string()))?;
                let amount = parse_decimal(amount, TAG_QUANTITY)?;
                let unit = parse_unit(unit)?;
                let label = tag.get(3).and_then(|v| clean_value(v));
                let count = tag.get(4).and_then(|v| v.parse::<u32>().ok());
                quantities.push(RadrootsListingQuantity {
                    value: RadrootsCoreQuantity::new(amount, unit),
                    label,
                    count,
                });
            }
            TAG_PRICE => {
                let amount = tag.get(1).ok_or_else(|| TradeListingParseError::InvalidTag(TAG_PRICE.to_string()))?;
                let currency = tag.get(2).ok_or_else(|| TradeListingParseError::InvalidTag(TAG_PRICE.to_string()))?;
                if tag.len() >= 5 {
                    let quantity_amount = tag.get(3).ok_or_else(|| TradeListingParseError::InvalidTag(TAG_PRICE.to_string()))?;
                    let unit = tag.get(4).ok_or_else(|| TradeListingParseError::InvalidTag(TAG_PRICE.to_string()))?;
                    let amount = parse_decimal(amount, TAG_PRICE)?;
                    let currency = parse_currency(currency)?;
                    let quantity_amount = parse_decimal(quantity_amount, TAG_PRICE)?;
                    let unit = parse_unit(unit)?;
                    let label = tag.get(5).and_then(|v| clean_value(v));
                    let quantity = RadrootsCoreQuantity::new(quantity_amount, unit).with_optional_label(label);
                    prices.push(RadrootsCoreQuantityPrice {
                        amount: RadrootsCoreMoney::new(amount, currency),
                        quantity,
                    });
                } else {
                    let amount = parse_decimal(amount, TAG_PRICE)?;
                    let currency = parse_currency(currency)?;
                    let quantity = RadrootsCoreQuantity::new(RadrootsCoreDecimal::from(1u32), RadrootsCoreUnit::Each);
                    prices.push(RadrootsCoreQuantityPrice {
                        amount: RadrootsCoreMoney::new(amount, currency),
                        quantity,
                    });
                }
            }
            TAG_GEOHASH => {
                if let Some(value) = tag.get(1).and_then(|v| clean_value(v)) {
                    geohash = Some(value);
                }
            }
            TAG_INVENTORY => {
                let value = tag.get(1).ok_or_else(|| TradeListingParseError::InvalidTag(TAG_INVENTORY.to_string()))?;
                inventory_available = Some(parse_decimal(value, TAG_INVENTORY)?);
            }
            TAG_PUBLISHED_AT => {
                let value = tag.get(1).ok_or_else(|| TradeListingParseError::InvalidTag(TAG_PUBLISHED_AT.to_string()))?;
                availability_start = Some(
                    value
                        .parse::<u64>()
                        .map_err(|_| TradeListingParseError::InvalidNumber(TAG_PUBLISHED_AT.to_string()))?,
                );
            }
            TAG_EXPIRES_AT => {
                let value = tag.get(1).ok_or_else(|| TradeListingParseError::InvalidTag(TAG_EXPIRES_AT.to_string()))?;
                availability_end = Some(
                    value
                        .parse::<u64>()
                        .map_err(|_| TradeListingParseError::InvalidNumber(TAG_EXPIRES_AT.to_string()))?,
                );
            }
            TAG_STATUS => {
                let status = tag.get(1).and_then(|v| clean_value(v)).unwrap_or_default();
                availability_status = Some(parse_status(&status));
            }
            TAG_DELIVERY => {
                let method = tag.get(1).and_then(|v| clean_value(v)).unwrap_or_default();
                delivery_method = Some(match method.as_str() {
                    "pickup" => RadrootsListingDeliveryMethod::Pickup,
                    "local_delivery" => RadrootsListingDeliveryMethod::LocalDelivery,
                    "shipping" => RadrootsListingDeliveryMethod::Shipping,
                    "other" => {
                        let detail = tag.get(2).and_then(|v| clean_value(v)).unwrap_or_default();
                        RadrootsListingDeliveryMethod::Other { method: detail }
                    }
                    other => RadrootsListingDeliveryMethod::Other {
                        method: other.to_string(),
                    },
                });
            }
            TAG_IMAGE => {
                let url = tag.get(1).ok_or_else(|| TradeListingParseError::InvalidTag(TAG_IMAGE.to_string()))?;
                if url.trim().is_empty() {
                    continue;
                }
                let size = tag.get(2).and_then(|s| parse_image_size(s));
                images.push(RadrootsListingImage {
                    url: url.to_string(),
                    size,
                });
            }
            _ if key.starts_with(TAG_PRICE_DISCOUNT_PREFIX) => {
                let kind = key.trim_start_matches(TAG_PRICE_DISCOUNT_PREFIX);
                let payload = tag.get(1).ok_or_else(|| TradeListingParseError::InvalidDiscount(kind.to_string()))?;
                let discount = parse_discount(kind, payload)?;
                discounts.push(discount);
            }
            _ => {}
        }
    }

    let availability = match availability_status {
        Some(status) => Some(RadrootsListingAvailability::Status { status }),
        None => match (availability_start, availability_end) {
            (None, None) => None,
            (start, end) => Some(RadrootsListingAvailability::Window { start, end }),
        },
    };

    let location = location.map(|mut loc| {
        if loc.geohash.is_none() {
            loc.geohash = geohash;
        }
        loc
    });

    Ok(RadrootsListing {
        d_tag,
        product,
        quantities,
        prices,
        discounts: if discounts.is_empty() { None } else { Some(discounts) },
        inventory_available,
        availability,
        delivery_method,
        location,
        images: if images.is_empty() { None } else { Some(images) },
    })
}

fn clean_value(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn set_if_empty(target: &mut String, value: Option<&String>) {
    if target.trim().is_empty() {
        if let Some(v) = value.and_then(|v| clean_value(v)) {
            *target = v;
        }
    }
}

fn set_optional(target: &mut Option<String>, value: Option<&String>) {
    if target.is_none() {
        if let Some(v) = value.and_then(|v| clean_value(v)) {
            *target = Some(v);
        }
    }
}

fn parse_status(value: &str) -> RadrootsListingStatus {
    match value.trim().to_ascii_lowercase().as_str() {
        "active" => RadrootsListingStatus::Active,
        "sold" => RadrootsListingStatus::Sold,
        other => RadrootsListingStatus::Other {
            value: other.to_string(),
        },
    }
}

fn status_as_str(status: &RadrootsListingStatus) -> &str {
    match status {
        RadrootsListingStatus::Active => "active",
        RadrootsListingStatus::Sold => "sold",
        RadrootsListingStatus::Other { value } => value.as_str(),
    }
}

fn parse_image_size(value: &str) -> Option<RadrootsListingImageSize> {
    let mut parts = value.split('x');
    let w = parts.next()?.parse::<u32>().ok()?;
    let h = parts.next()?.parse::<u32>().ok()?;
    Some(RadrootsListingImageSize { w, h })
}

fn parse_discount(
    kind: &str,
    payload: &str,
) -> Result<RadrootsListingDiscount, TradeListingParseError> {
    #[cfg(feature = "serde_json")]
    {
        match kind {
            "quantity" => {
                let data: QuantityDiscountPayload =
                    serde_json::from_str(payload).map_err(|_| TradeListingParseError::InvalidDiscount(kind.to_string()))?;
                Ok(RadrootsListingDiscount::Quantity {
                    ref_quantity: data.ref_quantity,
                    threshold: data.threshold,
                    value: data.value,
                })
            }
            "mass" => {
                let data: MassDiscountPayload =
                    serde_json::from_str(payload).map_err(|_| TradeListingParseError::InvalidDiscount(kind.to_string()))?;
                Ok(RadrootsListingDiscount::Mass {
                    threshold: data.threshold,
                    value: data.value,
                })
            }
            "subtotal" => {
                let data: SubtotalDiscountPayload =
                    serde_json::from_str(payload).map_err(|_| TradeListingParseError::InvalidDiscount(kind.to_string()))?;
                Ok(RadrootsListingDiscount::Subtotal {
                    threshold: data.threshold,
                    value: data.value,
                })
            }
            "total" => {
                let data: TotalDiscountPayload =
                    serde_json::from_str(payload).map_err(|_| TradeListingParseError::InvalidDiscount(kind.to_string()))?;
                Ok(RadrootsListingDiscount::Total {
                    total_min: data.total_min,
                    value: data.value,
                })
            }
            _ => Err(TradeListingParseError::InvalidDiscount(kind.to_string())),
        }
    }
    #[cfg(not(feature = "serde_json"))]
    {
        let _ = (kind, payload);
        Err(TradeListingParseError::InvalidJson("discount".to_string()))
    }
}

#[cfg(feature = "serde_json")]
#[derive(serde::Deserialize, serde::Serialize, Clone)]
struct QuantityDiscountPayload {
    ref_quantity: String,
    threshold: RadrootsCoreQuantity,
    value: RadrootsCoreMoney,
}

#[cfg(feature = "serde_json")]
#[derive(serde::Deserialize, serde::Serialize, Clone)]
struct MassDiscountPayload {
    threshold: RadrootsCoreQuantity,
    value: RadrootsCoreMoney,
}

#[cfg(feature = "serde_json")]
#[derive(serde::Deserialize, serde::Serialize, Clone)]
struct SubtotalDiscountPayload {
    threshold: RadrootsCoreMoney,
    value: radroots_core::RadrootsCoreDiscountValue,
}

#[cfg(feature = "serde_json")]
#[derive(serde::Deserialize, serde::Serialize, Clone)]
struct TotalDiscountPayload {
    total_min: RadrootsCoreMoney,
    value: radroots_core::RadrootsCorePercent,
}
