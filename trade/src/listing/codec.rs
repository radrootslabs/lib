#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use radroots_core::{
    RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreDiscount, RadrootsCoreMoney,
    RadrootsCoreQuantity, RadrootsCoreQuantityPrice, RadrootsCoreUnit,
};
use radroots_events::listing::{
    RadrootsListing, RadrootsListingAvailability, RadrootsListingBin,
    RadrootsListingDeliveryMethod, RadrootsListingFarmRef, RadrootsListingImage,
    RadrootsListingImageSize, RadrootsListingLocation, RadrootsListingProduct,
    RadrootsListingStatus,
};
use radroots_events::kinds::{KIND_FARM, KIND_PLOT, KIND_RESOURCE_AREA};
use radroots_events::plot::RadrootsPlotRef;
use radroots_events::resource_area::RadrootsResourceAreaRef;
use radroots_events::tags::TAG_D;
use radroots_events_codec::d_tag::is_d_tag_base64url;
use radroots_events_codec::error::EventEncodeError;
use radroots_events_codec::listing::tags::{listing_tags_with_options, ListingTagOptions};
#[cfg(feature = "ts-rs")]
use ts_rs::TS;

const TAG_PRICE: &str = "price";
const TAG_RADROOTS_BIN: &str = "radroots:bin";
const TAG_RADROOTS_PRICE: &str = "radroots:price";
const TAG_RADROOTS_DISCOUNT: &str = "radroots:discount";
const TAG_RADROOTS_PRIMARY_BIN: &str = "radroots:primary_bin";
const TAG_RADROOTS_RESOURCE_AREA: &str = "radroots:resource_area";
const TAG_RADROOTS_PLOT: &str = "radroots:plot";
const TAG_LOCATION: &str = "location";
const TAG_IMAGE: &str = "image";
const TAG_GEOHASH: &str = "g";
const TAG_INVENTORY: &str = "inventory";
const TAG_DELIVERY: &str = "delivery";
const TAG_PUBLISHED_AT: &str = "published_at";
const TAG_STATUS: &str = "status";
const TAG_EXPIRES_AT: &str = "expires_at";
const TAG_P: &str = "p";
const TAG_A: &str = "a";

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
    if !is_d_tag_base64url(&value) {
        return Err(TradeListingParseError::InvalidTag(TAG_D.to_string()));
    }
    Ok(value)
}

pub fn listing_from_event_parts(
    tags: &[Vec<String>],
    content: &str,
) -> Result<RadrootsListing, TradeListingParseError> {
    let d_tag = parse_d_tag(tags)?;
    let farm_ref = parse_farm_ref(tags)?;
    let farm_pubkey = parse_farm_pubkey(tags)?;
    let resource_area = parse_resource_area(tags)?;
    let plot = parse_plot_ref(tags)?;

    if !content.trim().is_empty() {
        #[cfg(feature = "serde_json")]
        {
            if let Ok(mut listing) = serde_json::from_str::<RadrootsListing>(content) {
                if listing.d_tag.trim().is_empty() {
                    listing.d_tag = d_tag;
                } else if listing.d_tag != d_tag {
                    return Err(TradeListingParseError::InvalidTag(TAG_D.to_string()));
                }
                if listing.farm.pubkey.trim().is_empty() || listing.farm.d_tag.trim().is_empty() {
                    listing.farm = farm_ref;
                } else if listing.farm.pubkey != farm_ref.pubkey
                    || listing.farm.d_tag != farm_ref.d_tag
                {
                    return Err(TradeListingParseError::InvalidTag(TAG_A.to_string()));
                }
                if listing.farm.pubkey != farm_pubkey {
                    return Err(TradeListingParseError::InvalidTag(TAG_P.to_string()));
                }
                if let Some(tag_area) = resource_area {
                    match listing.resource_area.as_ref() {
                        None => listing.resource_area = Some(tag_area),
                        Some(area) => {
                            if area.pubkey != tag_area.pubkey || area.d_tag != tag_area.d_tag {
                                return Err(TradeListingParseError::InvalidTag(
                                    TAG_RADROOTS_RESOURCE_AREA.to_string(),
                                ));
                            }
                        }
                    }
                }
                if let Some(tag_plot) = plot {
                    match listing.plot.as_ref() {
                        None => listing.plot = Some(tag_plot),
                        Some(existing) => {
                            if existing.pubkey != tag_plot.pubkey || existing.d_tag != tag_plot.d_tag {
                                return Err(TradeListingParseError::InvalidTag(
                                    TAG_RADROOTS_PLOT.to_string(),
                                ));
                            }
                        }
                    }
                }
                return Ok(listing);
            }
        }
    }

    listing_from_tags(tags, d_tag, farm_ref, farm_pubkey, resource_area, plot)
}

pub fn listing_tags_build(listing: &RadrootsListing) -> Result<Vec<Vec<String>>, TradeListingParseError> {
    let options = ListingTagOptions::with_trade_fields();
    listing_tags_with_options(listing, options).map_err(map_listing_tags_error)
}

fn map_listing_tags_error(err: EventEncodeError) -> TradeListingParseError {
    match err {
        EventEncodeError::EmptyRequiredField(field) => {
            TradeListingParseError::MissingTag(field.to_string())
        }
        EventEncodeError::InvalidField(field) => {
            TradeListingParseError::InvalidTag(field.to_string())
        }
        EventEncodeError::Json => TradeListingParseError::InvalidJson("discount".to_string()),
        EventEncodeError::InvalidKind(_) => TradeListingParseError::InvalidTag("kind".to_string()),
    }
}

fn listing_from_tags(
    tags: &[Vec<String>],
    d_tag: String,
    farm_ref: RadrootsListingFarmRef,
    farm_pubkey: String,
    resource_area: Option<RadrootsResourceAreaRef>,
    plot: Option<RadrootsPlotRef>,
) -> Result<RadrootsListing, TradeListingParseError> {
    if !is_d_tag_base64url(&d_tag) {
        return Err(TradeListingParseError::InvalidTag(TAG_D.to_string()));
    }
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

    let mut primary_bin_id: Option<String> = None;
    let mut bin_drafts: Vec<BinDraft> = Vec::new();
    let mut bin_order = 0usize;
    let mut discounts: Vec<RadrootsCoreDiscount> = Vec::new();
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
            TAG_PRICE => {
                let _ = tag;
            }
            TAG_RADROOTS_PRIMARY_BIN => {
                let value = tag
                    .get(1)
                    .and_then(|v| clean_value(v))
                    .ok_or_else(|| TradeListingParseError::InvalidTag(TAG_RADROOTS_PRIMARY_BIN.to_string()))?;
                if let Some(existing) = primary_bin_id.as_ref() {
                    if existing != &value {
                        return Err(TradeListingParseError::InvalidTag(
                            TAG_RADROOTS_PRIMARY_BIN.to_string(),
                        ));
                    }
                } else {
                    primary_bin_id = Some(value);
                }
            }
            TAG_RADROOTS_BIN => {
                if tag.len() < 4 {
                    return Err(TradeListingParseError::InvalidTag(TAG_RADROOTS_BIN.to_string()));
                }
                if tag.len() > 7 {
                    return Err(TradeListingParseError::InvalidTag(TAG_RADROOTS_BIN.to_string()));
                }
                let bin_id = tag
                    .get(1)
                    .and_then(|v| clean_value(v))
                    .ok_or_else(|| TradeListingParseError::InvalidTag(TAG_RADROOTS_BIN.to_string()))?;
                let amount = tag
                    .get(2)
                    .ok_or_else(|| TradeListingParseError::InvalidTag(TAG_RADROOTS_BIN.to_string()))?;
                let unit = tag
                    .get(3)
                    .ok_or_else(|| TradeListingParseError::InvalidTag(TAG_RADROOTS_BIN.to_string()))?;
                let amount = parse_decimal(amount, TAG_RADROOTS_BIN)?;
                let unit = parse_unit(unit)?;
                if unit != unit.canonical_unit() {
                    return Err(TradeListingParseError::InvalidTag(TAG_RADROOTS_BIN.to_string()));
                }
                let bin = upsert_bin(&mut bin_drafts, &bin_id, &mut bin_order);
                if bin.quantity.is_some() {
                    return Err(TradeListingParseError::InvalidTag(TAG_RADROOTS_BIN.to_string()));
                }
                bin.quantity = Some(RadrootsCoreQuantity::new(amount, unit));

                if tag.len() >= 5 {
                    let display_amount = tag.get(4).ok_or_else(|| {
                        TradeListingParseError::InvalidTag(TAG_RADROOTS_BIN.to_string())
                    })?;
                    let display_amount = parse_decimal(display_amount, TAG_RADROOTS_BIN)?;
                    let display_unit = tag.get(5).ok_or_else(|| {
                        TradeListingParseError::InvalidTag(TAG_RADROOTS_BIN.to_string())
                    })?;
                    let display_unit = parse_unit(display_unit)?;
                    bin.display_amount = Some(display_amount);
                    bin.display_unit = Some(display_unit);
                    if tag.len() == 7 {
                        bin.display_label = tag.get(6).and_then(|v| clean_value(v));
                    }
                }
            }
            TAG_RADROOTS_PRICE => {
                if tag.len() < 6 {
                    return Err(TradeListingParseError::InvalidTag(TAG_RADROOTS_PRICE.to_string()));
                }
                if tag.len() > 8 {
                    return Err(TradeListingParseError::InvalidTag(TAG_RADROOTS_PRICE.to_string()));
                }
                let bin_id = tag
                    .get(1)
                    .and_then(|v| clean_value(v))
                    .ok_or_else(|| TradeListingParseError::InvalidTag(TAG_RADROOTS_PRICE.to_string()))?;
                let amount = tag
                    .get(2)
                    .ok_or_else(|| TradeListingParseError::InvalidTag(TAG_RADROOTS_PRICE.to_string()))?;
                let currency = tag
                    .get(3)
                    .ok_or_else(|| TradeListingParseError::InvalidTag(TAG_RADROOTS_PRICE.to_string()))?;
                let per_amount = tag
                    .get(4)
                    .ok_or_else(|| TradeListingParseError::InvalidTag(TAG_RADROOTS_PRICE.to_string()))?;
                let per_unit = tag
                    .get(5)
                    .ok_or_else(|| TradeListingParseError::InvalidTag(TAG_RADROOTS_PRICE.to_string()))?;
                let amount = parse_decimal(amount, TAG_RADROOTS_PRICE)?;
                let currency = parse_currency(currency)?;
                let per_amount = parse_decimal(per_amount, TAG_RADROOTS_PRICE)?;
                let per_unit = parse_unit(per_unit)?;
                let price_per_canonical_unit = RadrootsCoreQuantityPrice::new(
                    RadrootsCoreMoney::new(amount, currency),
                    RadrootsCoreQuantity::new(per_amount, per_unit),
                );
                if !price_per_canonical_unit.is_price_per_canonical_unit() {
                    return Err(TradeListingParseError::InvalidTag(TAG_RADROOTS_PRICE.to_string()));
                }
                let bin = upsert_bin(&mut bin_drafts, &bin_id, &mut bin_order);
                if bin.price_per_canonical_unit.is_some() {
                    return Err(TradeListingParseError::InvalidTag(TAG_RADROOTS_PRICE.to_string()));
                }
                bin.price_per_canonical_unit = Some(price_per_canonical_unit);

                if tag.len() == 7 {
                    return Err(TradeListingParseError::InvalidTag(TAG_RADROOTS_PRICE.to_string()));
                }
                if tag.len() == 8 {
                    let display_price = tag.get(6).ok_or_else(|| {
                        TradeListingParseError::InvalidTag(TAG_RADROOTS_PRICE.to_string())
                    })?;
                    let display_unit = tag.get(7).ok_or_else(|| {
                        TradeListingParseError::InvalidTag(TAG_RADROOTS_PRICE.to_string())
                    })?;
                    let display_price = parse_decimal(display_price, TAG_RADROOTS_PRICE)?;
                    let display_unit = parse_unit(display_unit)?;
                    bin.display_price = Some(RadrootsCoreMoney::new(display_price, currency));
                    bin.display_price_unit = Some(display_unit);
                }
            }
            TAG_RADROOTS_DISCOUNT => {
                let payload = tag
                    .get(1)
                    .ok_or_else(|| TradeListingParseError::InvalidTag(TAG_RADROOTS_DISCOUNT.to_string()))?;
                let discount = parse_discount(payload)?;
                discounts.push(discount);
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

    if farm_pubkey != farm_ref.pubkey {
        return Err(TradeListingParseError::InvalidTag(TAG_P.to_string()));
    }

    let primary_bin_id = primary_bin_id
        .and_then(|v| clean_value(&v))
        .ok_or_else(|| TradeListingParseError::MissingTag(TAG_RADROOTS_PRIMARY_BIN.to_string()))?;
    let bins = build_bins(bin_drafts)?;
    if !bins.iter().any(|bin| bin.bin_id == primary_bin_id) {
        return Err(TradeListingParseError::InvalidTag(
            TAG_RADROOTS_PRIMARY_BIN.to_string(),
        ));
    }

    Ok(RadrootsListing {
        d_tag,
        farm: farm_ref,
        product,
        primary_bin_id,
        bins,
        resource_area,
        plot,
        discounts: if discounts.is_empty() { None } else { Some(discounts) },
        inventory_available,
        availability,
        delivery_method,
        location,
        images: if images.is_empty() { None } else { Some(images) },
    })
}

fn parse_farm_ref(tags: &[Vec<String>]) -> Result<RadrootsListingFarmRef, TradeListingParseError> {
    for tag in tags.iter().filter(|t| t.get(0).map(|s| s.as_str()) == Some(TAG_A)) {
        let value = tag
            .get(1)
            .map(|s| s.to_string())
            .ok_or_else(|| TradeListingParseError::InvalidTag(TAG_A.to_string()))?;
        let mut parts = value.splitn(3, ':');
        let kind = parts
            .next()
            .and_then(|v| v.parse::<u32>().ok())
            .ok_or_else(|| TradeListingParseError::InvalidTag(TAG_A.to_string()))?;
        if kind != KIND_FARM {
            continue;
        }
        let pubkey = parts
            .next()
            .ok_or_else(|| TradeListingParseError::InvalidTag(TAG_A.to_string()))?
            .to_string();
        let d_tag = parts
            .next()
            .ok_or_else(|| TradeListingParseError::InvalidTag(TAG_A.to_string()))?
            .to_string();
        if pubkey.trim().is_empty() || d_tag.trim().is_empty() {
            return Err(TradeListingParseError::InvalidTag(TAG_A.to_string()));
        }
        if !is_d_tag_base64url(&d_tag) {
            return Err(TradeListingParseError::InvalidTag(TAG_A.to_string()));
        }
        return Ok(RadrootsListingFarmRef { pubkey, d_tag });
    }
    Err(TradeListingParseError::MissingTag(TAG_A.to_string()))
}

fn parse_farm_pubkey(tags: &[Vec<String>]) -> Result<String, TradeListingParseError> {
    let tag = tags
        .iter()
        .find(|t| t.get(0).map(|s| s.as_str()) == Some(TAG_P))
        .ok_or_else(|| TradeListingParseError::MissingTag(TAG_P.to_string()))?;
    let value = tag
        .get(1)
        .map(|s| s.to_string())
        .ok_or_else(|| TradeListingParseError::InvalidTag(TAG_P.to_string()))?;
    if value.trim().is_empty() {
        return Err(TradeListingParseError::InvalidTag(TAG_P.to_string()));
    }
    Ok(value)
}

fn parse_resource_area(
    tags: &[Vec<String>],
) -> Result<Option<RadrootsResourceAreaRef>, TradeListingParseError> {
    let tag = tags
        .iter()
        .find(|t| t.get(0).map(|s| s.as_str()) == Some(TAG_RADROOTS_RESOURCE_AREA));
    let Some(tag) = tag else {
        return Ok(None);
    };
    let value = tag
        .get(1)
        .map(|s| s.to_string())
        .ok_or_else(|| TradeListingParseError::InvalidTag(TAG_RADROOTS_RESOURCE_AREA.to_string()))?;
    let mut parts = value.splitn(3, ':');
    let kind = parts
        .next()
        .and_then(|v| v.parse::<u32>().ok())
        .ok_or_else(|| TradeListingParseError::InvalidTag(TAG_RADROOTS_RESOURCE_AREA.to_string()))?;
    if kind != KIND_RESOURCE_AREA {
        return Err(TradeListingParseError::InvalidTag(
            TAG_RADROOTS_RESOURCE_AREA.to_string(),
        ));
    }
    let pubkey = parts
        .next()
        .ok_or_else(|| TradeListingParseError::InvalidTag(TAG_RADROOTS_RESOURCE_AREA.to_string()))?
        .to_string();
    let d_tag = parts
        .next()
        .ok_or_else(|| TradeListingParseError::InvalidTag(TAG_RADROOTS_RESOURCE_AREA.to_string()))?
        .to_string();
    if pubkey.trim().is_empty() || d_tag.trim().is_empty() {
        return Err(TradeListingParseError::InvalidTag(
            TAG_RADROOTS_RESOURCE_AREA.to_string(),
        ));
    }
    if !is_d_tag_base64url(&d_tag) {
        return Err(TradeListingParseError::InvalidTag(
            TAG_RADROOTS_RESOURCE_AREA.to_string(),
        ));
    }
    Ok(Some(RadrootsResourceAreaRef { pubkey, d_tag }))
}

fn parse_plot_ref(tags: &[Vec<String>]) -> Result<Option<RadrootsPlotRef>, TradeListingParseError> {
    let tag = tags
        .iter()
        .find(|t| t.get(0).map(|s| s.as_str()) == Some(TAG_RADROOTS_PLOT));
    let Some(tag) = tag else {
        return Ok(None);
    };
    let value = tag
        .get(1)
        .map(|s| s.to_string())
        .ok_or_else(|| TradeListingParseError::InvalidTag(TAG_RADROOTS_PLOT.to_string()))?;
    let mut parts = value.splitn(3, ':');
    let kind = parts
        .next()
        .and_then(|v| v.parse::<u32>().ok())
        .ok_or_else(|| TradeListingParseError::InvalidTag(TAG_RADROOTS_PLOT.to_string()))?;
    if kind != KIND_PLOT {
        return Err(TradeListingParseError::InvalidTag(TAG_RADROOTS_PLOT.to_string()));
    }
    let pubkey = parts
        .next()
        .ok_or_else(|| TradeListingParseError::InvalidTag(TAG_RADROOTS_PLOT.to_string()))?
        .to_string();
    let d_tag = parts
        .next()
        .ok_or_else(|| TradeListingParseError::InvalidTag(TAG_RADROOTS_PLOT.to_string()))?
        .to_string();
    if pubkey.trim().is_empty() || d_tag.trim().is_empty() {
        return Err(TradeListingParseError::InvalidTag(TAG_RADROOTS_PLOT.to_string()));
    }
    if !is_d_tag_base64url(&d_tag) {
        return Err(TradeListingParseError::InvalidTag(TAG_RADROOTS_PLOT.to_string()));
    }
    Ok(Some(RadrootsPlotRef { pubkey, d_tag }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use radroots_core::RadrootsCoreUnit;
    use radroots_events::listing::RadrootsListingFarmRef;

    fn farm_ref() -> RadrootsListingFarmRef {
        RadrootsListingFarmRef {
            pubkey: "seller".to_string(),
            d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_string(),
        }
    }

    #[test]
    fn listing_parses_radroots_bins() {
        let tags = vec![
            vec!["key".into(), "coffee".into()],
            vec!["title".into(), "Coffee".into()],
            vec!["category".into(), "coffee".into()],
            vec!["radroots:primary_bin".into(), "bin-1".into()],
            vec![
                "radroots:bin".into(),
                "bin-1".into(),
                "1000".into(),
                "g".into(),
                "1".into(),
                "kg".into(),
                "bag".into(),
            ],
            vec![
                "radroots:price".into(),
                "bin-1".into(),
                "0.01".into(),
                "USD".into(),
                "1".into(),
                "g".into(),
                "10".into(),
                "kg".into(),
            ],
        ];

        let listing = listing_from_tags(
            &tags,
            "AAAAAAAAAAAAAAAAAAAAAg".to_string(),
            farm_ref(),
            "seller".to_string(),
            None,
            None,
        )
        .expect("listing");

        assert_eq!(listing.primary_bin_id, "bin-1");
        assert_eq!(listing.bins.len(), 1);
        assert_eq!(listing.bins[0].quantity.unit, RadrootsCoreUnit::MassG);
        assert_eq!(
            listing.bins[0].price_per_canonical_unit.quantity.unit,
            RadrootsCoreUnit::MassG
        );
        assert_eq!(
            listing.bins[0]
                .display_unit
                .expect("display unit")
                .code(),
            "kg"
        );
    }

    #[test]
    fn listing_from_tags_rejects_invalid_d_tag() {
        let tags = vec![
            vec!["key".into(), "coffee".into()],
            vec!["title".into(), "Coffee".into()],
            vec!["category".into(), "coffee".into()],
            vec!["radroots:primary_bin".into(), "bin-1".into()],
            vec![
                "radroots:bin".into(),
                "bin-1".into(),
                "1000".into(),
                "g".into(),
                "1".into(),
                "kg".into(),
                "bag".into(),
            ],
            vec![
                "radroots:price".into(),
                "bin-1".into(),
                "0.01".into(),
                "USD".into(),
                "1".into(),
                "g".into(),
                "10".into(),
                "kg".into(),
            ],
        ];

        let err = listing_from_tags(
            &tags,
            "invalid:tag".to_string(),
            farm_ref(),
            "seller".to_string(),
            None,
            None,
        )
        .unwrap_err();

        assert!(matches!(
            err,
            TradeListingParseError::InvalidTag(tag) if tag == TAG_D
        ));
    }
}

fn clean_value(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("null") {
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

fn parse_image_size(value: &str) -> Option<RadrootsListingImageSize> {
    let mut parts = value.split('x');
    let w = parts.next()?.parse::<u32>().ok()?;
    let h = parts.next()?.parse::<u32>().ok()?;
    Some(RadrootsListingImageSize { w, h })
}

fn parse_discount(payload: &str) -> Result<RadrootsCoreDiscount, TradeListingParseError> {
    #[cfg(feature = "serde_json")]
    {
        serde_json::from_str(payload)
            .map_err(|_| TradeListingParseError::InvalidDiscount(TAG_RADROOTS_DISCOUNT.to_string()))
    }
    #[cfg(not(feature = "serde_json"))]
    {
        let _ = payload;
        Err(TradeListingParseError::InvalidJson("discount".to_string()))
    }
}

#[derive(Clone, Debug)]
struct BinDraft {
    bin_id: String,
    order_index: usize,
    quantity: Option<RadrootsCoreQuantity>,
    display_amount: Option<RadrootsCoreDecimal>,
    display_unit: Option<RadrootsCoreUnit>,
    display_label: Option<String>,
    price_per_canonical_unit: Option<RadrootsCoreQuantityPrice>,
    display_price: Option<RadrootsCoreMoney>,
    display_price_unit: Option<RadrootsCoreUnit>,
}

fn upsert_bin<'a>(
    bins: &'a mut Vec<BinDraft>,
    bin_id: &str,
    order_index: &mut usize,
) -> &'a mut BinDraft {
    if let Some(pos) = bins.iter().position(|bin| bin.bin_id == bin_id) {
        return &mut bins[pos];
    }
    let draft = BinDraft {
        bin_id: bin_id.to_string(),
        order_index: *order_index,
        quantity: None,
        display_amount: None,
        display_unit: None,
        display_label: None,
        price_per_canonical_unit: None,
        display_price: None,
        display_price_unit: None,
    };
    bins.push(draft);
    *order_index += 1;
    let idx = bins.len() - 1;
    &mut bins[idx]
}

fn build_bins(mut drafts: Vec<BinDraft>) -> Result<Vec<RadrootsListingBin>, TradeListingParseError> {
    drafts.sort_by_key(|draft| draft.order_index);
    let mut bins = Vec::with_capacity(drafts.len());
    for draft in drafts {
        let quantity = draft
            .quantity
            .ok_or_else(|| TradeListingParseError::MissingTag(TAG_RADROOTS_BIN.to_string()))?;
        let price = draft.price_per_canonical_unit.ok_or_else(|| {
            TradeListingParseError::MissingTag(TAG_RADROOTS_PRICE.to_string())
        })?;
        if quantity.unit != price.quantity.unit {
            return Err(TradeListingParseError::InvalidTag(TAG_RADROOTS_PRICE.to_string()));
        }
        let bin = RadrootsListingBin {
            bin_id: draft.bin_id,
            quantity,
            price_per_canonical_unit: price,
            display_amount: draft.display_amount,
            display_unit: draft.display_unit,
            display_label: draft.display_label,
            display_price: draft.display_price,
            display_price_unit: draft.display_price_unit,
        };
        bins.push(bin);
    }
    Ok(bins)
}
