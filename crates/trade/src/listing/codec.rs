#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use radroots_core::{
    RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreDiscount, RadrootsCoreMoney,
    RadrootsCoreQuantity, RadrootsCoreQuantityPrice, RadrootsCoreUnit,
};
use radroots_events::kinds::{KIND_FARM, KIND_PLOT, KIND_RESOURCE_AREA};
use radroots_events::listing::{
    RadrootsListing, RadrootsListingAvailability, RadrootsListingBin,
    RadrootsListingDeliveryMethod, RadrootsListingFarmRef, RadrootsListingImage,
    RadrootsListingImageSize, RadrootsListingLocation, RadrootsListingProduct,
    RadrootsListingStatus,
};
use radroots_events::plot::RadrootsPlotRef;
use radroots_events::resource_area::RadrootsResourceAreaRef;
use radroots_events::tags::TAG_D;
use radroots_events_codec::d_tag::is_d_tag_base64url;
use radroots_events_codec::error::EventEncodeError;
use radroots_events_codec::listing::tags::{ListingTagOptions, listing_tags_with_options};
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
                            if existing.pubkey != tag_plot.pubkey
                                || existing.d_tag != tag_plot.d_tag
                            {
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

pub fn listing_tags_build(
    listing: &RadrootsListing,
) -> Result<Vec<Vec<String>>, TradeListingParseError> {
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

    let has_structured_location = tags
        .iter()
        .any(|tag| tag.get(0).map(|k| k.as_str()) == Some(TAG_LOCATION) && tag.len() >= 3);

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
                let parse_structured_location = match tag.len() {
                    0 | 1 => false,
                    2 => !has_structured_location && location.is_none(),
                    _ => true,
                };
                if parse_structured_location {
                    let primary = &tag[1];
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
                let value = tag.get(1).and_then(|v| clean_value(v)).ok_or_else(|| {
                    TradeListingParseError::InvalidTag(TAG_RADROOTS_PRIMARY_BIN.to_string())
                })?;
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
                    return Err(TradeListingParseError::InvalidTag(
                        TAG_RADROOTS_BIN.to_string(),
                    ));
                }
                if tag.len() > 7 {
                    return Err(TradeListingParseError::InvalidTag(
                        TAG_RADROOTS_BIN.to_string(),
                    ));
                }
                let bin_id = clean_value(&tag[1]).ok_or_else(|| {
                    TradeListingParseError::InvalidTag(TAG_RADROOTS_BIN.to_string())
                })?;
                let amount = parse_decimal(&tag[2], TAG_RADROOTS_BIN)?;
                let unit = parse_unit(&tag[3])?;
                if unit != unit.canonical_unit() {
                    return Err(TradeListingParseError::InvalidTag(
                        TAG_RADROOTS_BIN.to_string(),
                    ));
                }
                let bin = upsert_bin(&mut bin_drafts, &bin_id, &mut bin_order);
                if bin.quantity.is_some() {
                    return Err(TradeListingParseError::InvalidTag(
                        TAG_RADROOTS_BIN.to_string(),
                    ));
                }
                bin.quantity = Some(RadrootsCoreQuantity::new(amount, unit));

                if tag.len() >= 5 {
                    if tag.len() < 6 {
                        return Err(TradeListingParseError::InvalidTag(
                            TAG_RADROOTS_BIN.to_string(),
                        ));
                    }
                    let display_amount = parse_decimal(&tag[4], TAG_RADROOTS_BIN)?;
                    let display_unit = parse_unit(&tag[5])?;
                    bin.display_amount = Some(display_amount);
                    bin.display_unit = Some(display_unit);
                    if tag.len() == 7 {
                        bin.display_label = clean_value(&tag[6]);
                    }
                }
            }
            TAG_RADROOTS_PRICE => {
                if tag.len() < 6 {
                    return Err(TradeListingParseError::InvalidTag(
                        TAG_RADROOTS_PRICE.to_string(),
                    ));
                }
                if tag.len() > 8 {
                    return Err(TradeListingParseError::InvalidTag(
                        TAG_RADROOTS_PRICE.to_string(),
                    ));
                }
                let bin_id = clean_value(&tag[1]).ok_or_else(|| {
                    TradeListingParseError::InvalidTag(TAG_RADROOTS_PRICE.to_string())
                })?;
                let amount = parse_decimal(&tag[2], TAG_RADROOTS_PRICE)?;
                let currency = parse_currency(&tag[3])?;
                let per_amount = parse_decimal(&tag[4], TAG_RADROOTS_PRICE)?;
                let per_unit = parse_unit(&tag[5])?;
                let price_per_canonical_unit = RadrootsCoreQuantityPrice::new(
                    RadrootsCoreMoney::new(amount, currency),
                    RadrootsCoreQuantity::new(per_amount, per_unit),
                );
                if !price_per_canonical_unit.is_price_per_canonical_unit() {
                    return Err(TradeListingParseError::InvalidTag(
                        TAG_RADROOTS_PRICE.to_string(),
                    ));
                }
                let bin = upsert_bin(&mut bin_drafts, &bin_id, &mut bin_order);
                if bin.price_per_canonical_unit.is_some() {
                    return Err(TradeListingParseError::InvalidTag(
                        TAG_RADROOTS_PRICE.to_string(),
                    ));
                }
                bin.price_per_canonical_unit = Some(price_per_canonical_unit);

                if tag.len() == 7 {
                    return Err(TradeListingParseError::InvalidTag(
                        TAG_RADROOTS_PRICE.to_string(),
                    ));
                }
                if tag.len() == 8 {
                    let display_price = parse_decimal(&tag[6], TAG_RADROOTS_PRICE)?;
                    let display_unit = parse_unit(&tag[7])?;
                    bin.display_price = Some(RadrootsCoreMoney::new(display_price, currency));
                    bin.display_price_unit = Some(display_unit);
                }
            }
            TAG_RADROOTS_DISCOUNT => {
                let payload = tag.get(1).ok_or_else(|| {
                    TradeListingParseError::InvalidTag(TAG_RADROOTS_DISCOUNT.to_string())
                })?;
                let discount = parse_discount(payload)?;
                discounts.push(discount);
            }
            TAG_GEOHASH => {
                if let Some(value) = tag.get(1).and_then(|v| clean_value(v)) {
                    geohash = Some(value);
                }
            }
            TAG_INVENTORY => {
                let value = tag
                    .get(1)
                    .ok_or_else(|| TradeListingParseError::InvalidTag(TAG_INVENTORY.to_string()))?;
                inventory_available = Some(parse_decimal(value, TAG_INVENTORY)?);
            }
            TAG_PUBLISHED_AT => {
                let value = tag.get(1).ok_or_else(|| {
                    TradeListingParseError::InvalidTag(TAG_PUBLISHED_AT.to_string())
                })?;
                availability_start = Some(value.parse::<u64>().map_err(|_| {
                    TradeListingParseError::InvalidNumber(TAG_PUBLISHED_AT.to_string())
                })?);
            }
            TAG_EXPIRES_AT => {
                let value = tag.get(1).ok_or_else(|| {
                    TradeListingParseError::InvalidTag(TAG_EXPIRES_AT.to_string())
                })?;
                availability_end = Some(value.parse::<u64>().map_err(|_| {
                    TradeListingParseError::InvalidNumber(TAG_EXPIRES_AT.to_string())
                })?);
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
                let url = tag
                    .get(1)
                    .ok_or_else(|| TradeListingParseError::InvalidTag(TAG_IMAGE.to_string()))?;
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
        loc.geohash = loc.geohash.or(geohash);
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
        discounts: if discounts.is_empty() {
            None
        } else {
            Some(discounts)
        },
        inventory_available,
        availability,
        delivery_method,
        location,
        images: if images.is_empty() {
            None
        } else {
            Some(images)
        },
    })
}

fn parse_farm_ref(tags: &[Vec<String>]) -> Result<RadrootsListingFarmRef, TradeListingParseError> {
    for tag in tags
        .iter()
        .filter(|t| t.get(0).map(|s| s.as_str()) == Some(TAG_A))
    {
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
    let value = tag.get(1).map(|s| s.to_string()).ok_or_else(|| {
        TradeListingParseError::InvalidTag(TAG_RADROOTS_RESOURCE_AREA.to_string())
    })?;
    let mut parts = value.splitn(3, ':');
    let kind = parts
        .next()
        .and_then(|v| v.parse::<u32>().ok())
        .ok_or_else(|| {
            TradeListingParseError::InvalidTag(TAG_RADROOTS_RESOURCE_AREA.to_string())
        })?;
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
        return Err(TradeListingParseError::InvalidTag(
            TAG_RADROOTS_PLOT.to_string(),
        ));
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
        return Err(TradeListingParseError::InvalidTag(
            TAG_RADROOTS_PLOT.to_string(),
        ));
    }
    if !is_d_tag_base64url(&d_tag) {
        return Err(TradeListingParseError::InvalidTag(
            TAG_RADROOTS_PLOT.to_string(),
        ));
    }
    Ok(Some(RadrootsPlotRef { pubkey, d_tag }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use radroots_core::{
        RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreDiscount, RadrootsCoreDiscountScope,
        RadrootsCoreDiscountThreshold, RadrootsCoreDiscountValue, RadrootsCoreMoney,
        RadrootsCorePercent, RadrootsCoreQuantity, RadrootsCoreQuantityPrice, RadrootsCoreUnit,
    };
    use radroots_events::listing::{
        RadrootsListing, RadrootsListingFarmRef, RadrootsListingStatus,
    };

    fn farm_ref() -> RadrootsListingFarmRef {
        RadrootsListingFarmRef {
            pubkey: "seller".to_string(),
            d_tag: "AAAAAAAAAAAAAAAAAAAAAA".to_string(),
        }
    }

    fn listing_d_tag() -> String {
        "AAAAAAAAAAAAAAAAAAAAAg".to_string()
    }

    fn base_event_tags() -> Vec<Vec<String>> {
        vec![
            vec![TAG_D.into(), listing_d_tag()],
            vec![TAG_P.into(), "seller".into()],
            vec![
                TAG_A.into(),
                format!("{KIND_FARM}:seller:{}", farm_ref().d_tag),
            ],
        ]
    }

    fn base_trade_tags() -> Vec<Vec<String>> {
        vec![
            vec!["key".into(), "coffee".into()],
            vec!["title".into(), "Coffee".into()],
            vec!["category".into(), "coffee".into()],
            vec!["summary".into(), "Single origin".into()],
            vec![TAG_RADROOTS_PRIMARY_BIN.into(), "bin-1".into()],
            vec![
                TAG_RADROOTS_BIN.into(),
                "bin-1".into(),
                "1000".into(),
                "g".into(),
                "1".into(),
                "kg".into(),
                "bag".into(),
            ],
            vec![
                TAG_RADROOTS_PRICE.into(),
                "bin-1".into(),
                "0.01".into(),
                "USD".into(),
                "1".into(),
                "g".into(),
                "10".into(),
                "kg".into(),
            ],
        ]
    }

    fn parse_base_listing_from_tags() -> RadrootsListing {
        listing_from_tags(
            &base_trade_tags(),
            listing_d_tag(),
            farm_ref(),
            "seller".to_string(),
            None,
            None,
        )
        .expect("listing")
    }

    fn parse_error_tag(error: TradeListingParseError) -> String {
        match error {
            TradeListingParseError::MissingTag(tag) => tag,
            TradeListingParseError::InvalidTag(tag) => tag,
            TradeListingParseError::InvalidNumber(field) => field,
            TradeListingParseError::InvalidUnit => "unit".to_string(),
            TradeListingParseError::InvalidCurrency => "currency".to_string(),
            TradeListingParseError::InvalidJson(field) => field,
            TradeListingParseError::InvalidDiscount(kind) => kind,
        }
    }

    #[test]
    fn listing_parses_radroots_bins() {
        let tags = base_trade_tags();

        let listing = listing_from_tags(
            &tags,
            listing_d_tag(),
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
            listing.bins[0].display_unit.expect("display unit").code(),
            "kg"
        );
    }

    #[test]
    fn listing_from_tags_rejects_invalid_d_tag() {
        let tags = base_trade_tags();

        let err = listing_from_tags(
            &tags,
            "invalid:tag".to_string(),
            farm_ref(),
            "seller".to_string(),
            None,
            None,
        )
        .unwrap_err();

        assert_eq!(parse_error_tag(err), TAG_D.to_string());
    }

    #[test]
    fn parse_scalar_helpers_cover_success_and_error_paths() {
        assert_eq!(
            parse_decimal("1.5", "f").unwrap(),
            "1.5".parse::<RadrootsCoreDecimal>().unwrap()
        );
        assert_eq!(
            parse_error_tag(parse_decimal("x", "f").unwrap_err()),
            "f".to_string()
        );
        assert_eq!(parse_currency(" usd ").unwrap(), RadrootsCoreCurrency::USD);
        assert_eq!(
            parse_error_tag(parse_currency("12").unwrap_err()),
            "currency".to_string()
        );
        assert_eq!(parse_unit("g").unwrap(), RadrootsCoreUnit::MassG);
        assert_eq!(
            parse_error_tag(parse_unit("not-unit").unwrap_err()),
            "unit".to_string()
        );
    }

    #[test]
    fn parse_error_display_covers_all_variants() {
        let errors = [
            TradeListingParseError::MissingTag("d".into()),
            TradeListingParseError::InvalidTag("a".into()),
            TradeListingParseError::InvalidNumber("n".into()),
            TradeListingParseError::InvalidUnit,
            TradeListingParseError::InvalidCurrency,
            TradeListingParseError::InvalidJson("j".into()),
            TradeListingParseError::InvalidDiscount("x".into()),
        ];
        for error in errors {
            assert!(!error.to_string().trim().is_empty());
        }
    }

    #[test]
    fn parse_d_tag_covers_all_paths() {
        assert_eq!(
            parse_error_tag(parse_d_tag(&[]).unwrap_err()),
            TAG_D.to_string()
        );
        assert_eq!(
            parse_error_tag(parse_d_tag(&[vec![TAG_D.into()]]).unwrap_err()),
            TAG_D.to_string()
        );
        assert_eq!(
            parse_error_tag(parse_d_tag(&[vec![TAG_D.into(), " ".into()]]).unwrap_err()),
            TAG_D.to_string()
        );
        assert_eq!(
            parse_error_tag(parse_d_tag(&[vec![TAG_D.into(), "invalid".into()]]).unwrap_err()),
            TAG_D.to_string()
        );
        assert_eq!(
            parse_d_tag(&[vec![TAG_D.into(), listing_d_tag()]]).unwrap(),
            listing_d_tag()
        );
    }

    #[test]
    fn listing_from_event_parts_uses_json_content_and_backfills_tags() {
        let mut listing = parse_base_listing_from_tags();
        listing.d_tag = String::new();
        listing.farm.pubkey = String::new();
        listing.farm.d_tag = String::new();
        listing.resource_area = None;
        listing.plot = None;

        let mut tags = base_event_tags();
        tags.push(vec![
            TAG_RADROOTS_RESOURCE_AREA.into(),
            format!("{KIND_RESOURCE_AREA}:seller:AAAAAAAAAAAAAAAAAAAAAQ"),
        ]);
        tags.push(vec![
            TAG_RADROOTS_PLOT.into(),
            format!("{KIND_PLOT}:seller:AAAAAAAAAAAAAAAAAAAAAw"),
        ]);

        let parsed = listing_from_event_parts(&tags, &serde_json::to_string(&listing).unwrap())
            .expect("event listing");
        assert_eq!(parsed.d_tag, listing_d_tag());
        assert_eq!(parsed.farm.pubkey, farm_ref().pubkey);
        assert_eq!(parsed.farm.d_tag, farm_ref().d_tag);
        assert_eq!(
            parsed.resource_area.unwrap().d_tag,
            "AAAAAAAAAAAAAAAAAAAAAQ"
        );
        assert_eq!(parsed.plot.unwrap().d_tag, "AAAAAAAAAAAAAAAAAAAAAw");
    }

    #[test]
    fn listing_from_event_parts_rejects_conflicting_content_values() {
        let tags = base_event_tags();

        let mut listing = parse_base_listing_from_tags();
        listing.d_tag = "AAAAAAAAAAAAAAAAAAAAAw".into();
        let err =
            listing_from_event_parts(&tags, &serde_json::to_string(&listing).unwrap()).unwrap_err();
        assert_eq!(parse_error_tag(err), TAG_D.to_string());

        let mut listing = parse_base_listing_from_tags();
        listing.farm.pubkey = "other".into();
        let err =
            listing_from_event_parts(&tags, &serde_json::to_string(&listing).unwrap()).unwrap_err();
        assert_eq!(parse_error_tag(err), TAG_A.to_string());

        let mut listing = parse_base_listing_from_tags();
        listing.farm.d_tag = "AAAAAAAAAAAAAAAAAAAAAw".into();
        let err =
            listing_from_event_parts(&tags, &serde_json::to_string(&listing).unwrap()).unwrap_err();
        assert_eq!(parse_error_tag(err), TAG_A.to_string());

        let mut listing = parse_base_listing_from_tags();
        listing.farm.d_tag = String::new();
        let parsed = listing_from_event_parts(&tags, &serde_json::to_string(&listing).unwrap())
            .expect("backfill empty farm d_tag");
        assert_eq!(parsed.farm.d_tag, farm_ref().d_tag);

        let listing = parse_base_listing_from_tags();
        let mut mismatched_pubkey_tags = tags.clone();
        mismatched_pubkey_tags[1][1] = "other".into();
        let err = listing_from_event_parts(
            &mismatched_pubkey_tags,
            &serde_json::to_string(&listing).unwrap(),
        )
        .unwrap_err();
        assert_eq!(parse_error_tag(err), TAG_P.to_string());

        let mut listing = parse_base_listing_from_tags();
        listing.resource_area = Some(RadrootsResourceAreaRef {
            pubkey: "seller".into(),
            d_tag: "AAAAAAAAAAAAAAAAAAAAAQ".into(),
        });
        let mut resource_tags = tags.clone();
        resource_tags.push(vec![
            TAG_RADROOTS_RESOURCE_AREA.into(),
            format!("{KIND_RESOURCE_AREA}:seller:AAAAAAAAAAAAAAAAAAAAAw"),
        ]);
        let err =
            listing_from_event_parts(&resource_tags, &serde_json::to_string(&listing).unwrap())
                .unwrap_err();
        assert_eq!(parse_error_tag(err), TAG_RADROOTS_RESOURCE_AREA.to_string());

        let mut listing = parse_base_listing_from_tags();
        listing.resource_area = Some(RadrootsResourceAreaRef {
            pubkey: "other".into(),
            d_tag: "AAAAAAAAAAAAAAAAAAAAAw".into(),
        });
        let mut resource_tags = tags.clone();
        resource_tags.push(vec![
            TAG_RADROOTS_RESOURCE_AREA.into(),
            format!("{KIND_RESOURCE_AREA}:seller:AAAAAAAAAAAAAAAAAAAAAw"),
        ]);
        let err =
            listing_from_event_parts(&resource_tags, &serde_json::to_string(&listing).unwrap())
                .unwrap_err();
        assert_eq!(parse_error_tag(err), TAG_RADROOTS_RESOURCE_AREA.to_string());

        let mut listing = parse_base_listing_from_tags();
        listing.plot = Some(RadrootsPlotRef {
            pubkey: "seller".into(),
            d_tag: "AAAAAAAAAAAAAAAAAAAAAQ".into(),
        });
        let mut plot_tags = tags.clone();
        plot_tags.push(vec![
            TAG_RADROOTS_PLOT.into(),
            format!("{KIND_PLOT}:seller:AAAAAAAAAAAAAAAAAAAAAw"),
        ]);
        let err = listing_from_event_parts(&plot_tags, &serde_json::to_string(&listing).unwrap())
            .unwrap_err();
        assert_eq!(parse_error_tag(err), TAG_RADROOTS_PLOT.to_string());

        let mut listing = parse_base_listing_from_tags();
        listing.plot = Some(RadrootsPlotRef {
            pubkey: "other".into(),
            d_tag: "AAAAAAAAAAAAAAAAAAAAAw".into(),
        });
        let mut plot_tags = tags.clone();
        plot_tags.push(vec![
            TAG_RADROOTS_PLOT.into(),
            format!("{KIND_PLOT}:seller:AAAAAAAAAAAAAAAAAAAAAw"),
        ]);
        let err = listing_from_event_parts(&plot_tags, &serde_json::to_string(&listing).unwrap())
            .unwrap_err();
        assert_eq!(parse_error_tag(err), TAG_RADROOTS_PLOT.to_string());
    }

    #[test]
    fn listing_from_event_parts_accepts_matching_resource_and_plot_refs() {
        let mut listing = parse_base_listing_from_tags();
        listing.resource_area = Some(RadrootsResourceAreaRef {
            pubkey: "seller".into(),
            d_tag: "AAAAAAAAAAAAAAAAAAAAAQ".into(),
        });
        listing.plot = Some(RadrootsPlotRef {
            pubkey: "seller".into(),
            d_tag: "AAAAAAAAAAAAAAAAAAAAAw".into(),
        });
        let mut tags = base_event_tags();
        tags.push(vec![
            TAG_RADROOTS_RESOURCE_AREA.into(),
            format!("{KIND_RESOURCE_AREA}:seller:AAAAAAAAAAAAAAAAAAAAAQ"),
        ]);
        tags.push(vec![
            TAG_RADROOTS_PLOT.into(),
            format!("{KIND_PLOT}:seller:AAAAAAAAAAAAAAAAAAAAAw"),
        ]);
        let parsed = listing_from_event_parts(&tags, &serde_json::to_string(&listing).unwrap())
            .expect("matching refs");
        assert_eq!(
            parsed.resource_area.unwrap().d_tag,
            "AAAAAAAAAAAAAAAAAAAAAQ"
        );
        assert_eq!(parsed.plot.unwrap().d_tag, "AAAAAAAAAAAAAAAAAAAAAw");
    }

    #[test]
    fn listing_from_event_parts_rejects_invalid_plot_tag_shapes() {
        let mut tags = base_event_tags();
        tags.push(vec![TAG_RADROOTS_PLOT.into()]);
        let err = listing_from_event_parts(&tags, "").unwrap_err();
        assert_eq!(parse_error_tag(err), TAG_RADROOTS_PLOT.to_string());

        let mut tags = base_event_tags();
        tags.push(vec![TAG_RADROOTS_PLOT.into(), "bad".into()]);
        let err = listing_from_event_parts(&tags, "").unwrap_err();
        assert_eq!(parse_error_tag(err), TAG_RADROOTS_PLOT.to_string());
    }

    #[test]
    fn listing_from_event_parts_falls_back_to_tag_parser() {
        let mut tags = base_event_tags();
        tags.extend(base_trade_tags());
        let listing =
            listing_from_event_parts(&tags, "{invalid-json").expect("fallback tags parse");
        assert_eq!(listing.primary_bin_id, "bin-1");
        assert_eq!(listing.bins.len(), 1);
    }

    #[test]
    fn listing_tags_build_and_error_mapping_cover_paths() {
        let listing = parse_base_listing_from_tags();
        let built = listing_tags_build(&listing).expect("build tags");
        assert!(
            built
                .iter()
                .any(|tag| tag.get(0).map(|v| v.as_str()) == Some(TAG_RADROOTS_PRIMARY_BIN))
        );

        let mapped = map_listing_tags_error(EventEncodeError::EmptyRequiredField("d"));
        assert_eq!(parse_error_tag(mapped), "d".to_string());
        let mapped = map_listing_tags_error(EventEncodeError::InvalidField("f"));
        assert_eq!(parse_error_tag(mapped), "f".to_string());
        let mapped = map_listing_tags_error(EventEncodeError::Json);
        assert_eq!(parse_error_tag(mapped), "discount".to_string());
        let mapped = map_listing_tags_error(EventEncodeError::InvalidKind(1));
        assert_eq!(parse_error_tag(mapped), "kind".to_string());
    }

    #[test]
    fn listing_from_tags_parses_trade_specific_optional_fields() {
        let mut tags = base_trade_tags();
        tags.push(Vec::new());
        tags.push(vec![TAG_PRICE.into(), "ignored".into()]);
        tags.push(vec![TAG_RADROOTS_PRIMARY_BIN.into(), "bin-1".into()]);
        tags.push(vec![
            TAG_LOCATION.into(),
            "Farm".into(),
            "Town".into(),
            "Region".into(),
            "SE".into(),
        ]);
        tags.push(vec![TAG_GEOHASH.into(), "u6se".into()]);
        tags.push(vec![TAG_INVENTORY.into(), "8".into()]);
        tags.push(vec![TAG_PUBLISHED_AT.into(), "10".into()]);
        tags.push(vec![TAG_EXPIRES_AT.into(), "20".into()]);
        tags.push(vec![TAG_DELIVERY.into(), "other".into(), "drone".into()]);
        tags.push(vec![
            TAG_IMAGE.into(),
            "https://cdn/image.png".into(),
            "100x200".into(),
        ]);
        tags.push(vec![TAG_IMAGE.into(), " ".into()]);
        let discount = RadrootsCoreDiscount {
            scope: RadrootsCoreDiscountScope::Bin,
            threshold: RadrootsCoreDiscountThreshold::BinCount {
                bin_id: "bin-1".into(),
                min: 2,
            },
            value: RadrootsCoreDiscountValue::Percent(RadrootsCorePercent::new(
                "5".parse::<RadrootsCoreDecimal>().unwrap(),
            )),
        };
        tags.push(vec![
            TAG_RADROOTS_DISCOUNT.into(),
            serde_json::to_string(&discount).unwrap(),
        ]);

        let listing = listing_from_tags(
            &tags,
            listing_d_tag(),
            farm_ref(),
            "seller".to_string(),
            None,
            None,
        )
        .expect("listing");

        assert_eq!(
            format!("{:?}", listing.availability),
            "Some(Window { start: Some(10), end: Some(20) })"
        );
        assert_eq!(
            format!("{:?}", listing.delivery_method),
            "Some(Other { method: \"drone\" })"
        );
        assert_eq!(
            listing.location.as_ref().unwrap().geohash.as_deref(),
            Some("u6se")
        );
        assert_eq!(listing.images.as_ref().unwrap().len(), 1);
        assert_eq!(listing.discounts.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn listing_from_tags_uses_unstructured_location_and_custom_delivery() {
        let mut tags = base_trade_tags();
        tags.push(vec![TAG_LOCATION.into(), "Farm".into()]);
        tags.push(vec![TAG_LOCATION.into(), "fallback".into()]);
        tags.push(vec![TAG_DELIVERY.into(), "parcel".into()]);

        let listing = listing_from_tags(
            &tags,
            listing_d_tag(),
            farm_ref(),
            "seller".to_string(),
            None,
            None,
        )
        .expect("listing");

        assert_eq!(listing.product.location.as_deref(), Some("fallback"));
        assert_eq!(
            format!("{:?}", listing.delivery_method),
            "Some(Other { method: \"parcel\" })"
        );
    }

    #[test]
    fn listing_from_tags_rejects_empty_structured_location_primary() {
        let mut tags = base_trade_tags();
        tags.push(vec![
            TAG_LOCATION.into(),
            " ".into(),
            "Town".into(),
            "Region".into(),
        ]);
        let err = listing_from_tags(
            &tags,
            listing_d_tag(),
            farm_ref(),
            "seller".to_string(),
            None,
            None,
        )
        .unwrap_err();
        assert_eq!(parse_error_tag(err), TAG_LOCATION.to_string());
    }

    #[test]
    fn listing_from_tags_handles_short_location_tags_when_structured_present() {
        let mut tags = base_trade_tags();
        tags.push(vec![
            TAG_LOCATION.into(),
            "Farm".into(),
            "Town".into(),
            "Region".into(),
        ]);
        tags.push(vec![TAG_LOCATION.into()]);
        tags.push(vec![TAG_LOCATION.into(), "fallback".into()]);
        let listing = listing_from_tags(
            &tags,
            listing_d_tag(),
            farm_ref(),
            "seller".to_string(),
            None,
            None,
        )
        .expect("listing");
        assert_eq!(listing.location.unwrap().primary, "Farm".to_string());
    }

    #[test]
    fn listing_from_tags_rejects_invalid_tag_forms() {
        let mut tags = base_trade_tags();
        tags.push(vec![TAG_RADROOTS_PRIMARY_BIN.into(), "other".into()]);
        let err = listing_from_tags(
            &tags,
            listing_d_tag(),
            farm_ref(),
            "seller".to_string(),
            None,
            None,
        )
        .unwrap_err();
        assert_eq!(parse_error_tag(err), TAG_RADROOTS_PRIMARY_BIN.to_string());

        let mut tags = base_trade_tags();
        tags.push(vec![TAG_RADROOTS_BIN.into(), "bin-1".into(), "1000".into()]);
        let err = listing_from_tags(
            &tags,
            listing_d_tag(),
            farm_ref(),
            "seller".to_string(),
            None,
            None,
        )
        .unwrap_err();
        assert_eq!(parse_error_tag(err), TAG_RADROOTS_BIN.to_string());

        let mut tags = base_trade_tags();
        tags.push(vec![
            TAG_RADROOTS_PRICE.into(),
            "bin-1".into(),
            "1".into(),
            "USD".into(),
            "1".into(),
        ]);
        let err = listing_from_tags(
            &tags,
            listing_d_tag(),
            farm_ref(),
            "seller".to_string(),
            None,
            None,
        )
        .unwrap_err();
        assert_eq!(parse_error_tag(err), TAG_RADROOTS_PRICE.to_string());

        let mut tags = base_trade_tags();
        tags.push(vec![
            TAG_RADROOTS_BIN.into(),
            "bin-1".into(),
            "1000".into(),
            "g".into(),
            "1".into(),
            "kg".into(),
            "label".into(),
            "extra".into(),
        ]);
        let err = listing_from_tags(
            &tags,
            listing_d_tag(),
            farm_ref(),
            "seller".to_string(),
            None,
            None,
        )
        .unwrap_err();
        assert_eq!(parse_error_tag(err), TAG_RADROOTS_BIN.to_string());

        let mut tags = base_trade_tags();
        tags.push(vec![
            TAG_RADROOTS_BIN.into(),
            " ".into(),
            "1000".into(),
            "g".into(),
        ]);
        let err = listing_from_tags(
            &tags,
            listing_d_tag(),
            farm_ref(),
            "seller".to_string(),
            None,
            None,
        )
        .unwrap_err();
        assert_eq!(parse_error_tag(err), TAG_RADROOTS_BIN.to_string());

        let mut tags = base_trade_tags();
        tags.push(vec![
            TAG_RADROOTS_BIN.into(),
            "bin-1".into(),
            "1000".into(),
            "kg".into(),
        ]);
        let err = listing_from_tags(
            &tags,
            listing_d_tag(),
            farm_ref(),
            "seller".to_string(),
            None,
            None,
        )
        .unwrap_err();
        assert_eq!(parse_error_tag(err), TAG_RADROOTS_BIN.to_string());

        let mut tags = base_trade_tags();
        tags.push(vec![
            TAG_RADROOTS_BIN.into(),
            "bin-1".into(),
            "1000".into(),
            "g".into(),
            "1".into(),
        ]);
        let err = listing_from_tags(
            &tags,
            listing_d_tag(),
            farm_ref(),
            "seller".to_string(),
            None,
            None,
        )
        .unwrap_err();
        assert_eq!(parse_error_tag(err), TAG_RADROOTS_BIN.to_string());

        let mut tags = base_trade_tags();
        tags.push(vec![
            TAG_RADROOTS_PRICE.into(),
            " ".into(),
            "1".into(),
            "USD".into(),
            "1".into(),
            "g".into(),
        ]);
        let err = listing_from_tags(
            &tags,
            listing_d_tag(),
            farm_ref(),
            "seller".to_string(),
            None,
            None,
        )
        .unwrap_err();
        assert_eq!(parse_error_tag(err), TAG_RADROOTS_PRICE.to_string());

        let mut tags = base_trade_tags();
        tags.push(vec![
            TAG_RADROOTS_PRICE.into(),
            "bin-1".into(),
            "1".into(),
            "USD".into(),
            "1".into(),
            "kg".into(),
        ]);
        let err = listing_from_tags(
            &tags,
            listing_d_tag(),
            farm_ref(),
            "seller".to_string(),
            None,
            None,
        )
        .unwrap_err();
        assert_eq!(parse_error_tag(err), TAG_RADROOTS_PRICE.to_string());

        let mut tags = base_trade_tags();
        tags.push(vec![
            TAG_RADROOTS_PRICE.into(),
            "bin-1".into(),
            "1".into(),
            "USD".into(),
            "1".into(),
            "g".into(),
            "9".into(),
        ]);
        let err = listing_from_tags(
            &tags,
            listing_d_tag(),
            farm_ref(),
            "seller".to_string(),
            None,
            None,
        )
        .unwrap_err();
        assert_eq!(parse_error_tag(err), TAG_RADROOTS_PRICE.to_string());

        let mut tags = base_trade_tags();
        tags.push(vec![
            TAG_RADROOTS_PRICE.into(),
            "bin-1".into(),
            "1".into(),
            "USD".into(),
            "1".into(),
            "g".into(),
            "9".into(),
            "bad".into(),
        ]);
        let err = listing_from_tags(
            &tags,
            listing_d_tag(),
            farm_ref(),
            "seller".to_string(),
            None,
            None,
        )
        .unwrap_err();
        assert_eq!(parse_error_tag(err), TAG_RADROOTS_PRICE.to_string());

        let mut tags = base_trade_tags();
        tags.push(vec![
            TAG_RADROOTS_PRICE.into(),
            "bin-1".into(),
            "1".into(),
            "USD".into(),
            "1".into(),
            "g".into(),
            "9".into(),
            "kg".into(),
            "x".into(),
        ]);
        let err = listing_from_tags(
            &tags,
            listing_d_tag(),
            farm_ref(),
            "seller".to_string(),
            None,
            None,
        )
        .unwrap_err();
        assert_eq!(parse_error_tag(err), TAG_RADROOTS_PRICE.to_string());

        let mut tags = base_trade_tags();
        tags.push(vec![TAG_RADROOTS_PRIMARY_BIN.into()]);
        let err = listing_from_tags(
            &tags,
            listing_d_tag(),
            farm_ref(),
            "seller".to_string(),
            None,
            None,
        )
        .unwrap_err();
        assert_eq!(parse_error_tag(err), TAG_RADROOTS_PRIMARY_BIN.to_string());
    }

    #[test]
    fn listing_from_tags_rejects_trade_field_parse_failures() {
        let mut tags = base_trade_tags();
        tags.push(vec![TAG_RADROOTS_DISCOUNT.into(), "{".into()]);
        let err = listing_from_tags(
            &tags,
            listing_d_tag(),
            farm_ref(),
            "seller".to_string(),
            None,
            None,
        )
        .unwrap_err();
        assert_eq!(parse_error_tag(err), TAG_RADROOTS_DISCOUNT.to_string());

        let mut tags = base_trade_tags();
        tags.push(vec![TAG_INVENTORY.into()]);
        let err = listing_from_tags(
            &tags,
            listing_d_tag(),
            farm_ref(),
            "seller".to_string(),
            None,
            None,
        )
        .unwrap_err();
        assert_eq!(parse_error_tag(err), TAG_INVENTORY.to_string());

        let mut tags = base_trade_tags();
        tags.push(vec![TAG_PUBLISHED_AT.into(), "bad".into()]);
        let err = listing_from_tags(
            &tags,
            listing_d_tag(),
            farm_ref(),
            "seller".to_string(),
            None,
            None,
        )
        .unwrap_err();
        assert_eq!(parse_error_tag(err), TAG_PUBLISHED_AT.to_string());

        let mut tags = base_trade_tags();
        tags.push(vec![TAG_EXPIRES_AT.into(), "bad".into()]);
        let err = listing_from_tags(
            &tags,
            listing_d_tag(),
            farm_ref(),
            "seller".to_string(),
            None,
            None,
        )
        .unwrap_err();
        assert_eq!(parse_error_tag(err), TAG_EXPIRES_AT.to_string());

        let mut tags = base_trade_tags();
        tags.push(vec![TAG_RADROOTS_DISCOUNT.into()]);
        let err = listing_from_tags(
            &tags,
            listing_d_tag(),
            farm_ref(),
            "seller".to_string(),
            None,
            None,
        )
        .unwrap_err();
        assert_eq!(parse_error_tag(err), TAG_RADROOTS_DISCOUNT.to_string());

        let mut tags = base_trade_tags();
        tags.push(vec![TAG_PUBLISHED_AT.into()]);
        let err = listing_from_tags(
            &tags,
            listing_d_tag(),
            farm_ref(),
            "seller".to_string(),
            None,
            None,
        )
        .unwrap_err();
        assert_eq!(parse_error_tag(err), TAG_PUBLISHED_AT.to_string());

        let mut tags = base_trade_tags();
        tags.push(vec![TAG_EXPIRES_AT.into()]);
        let err = listing_from_tags(
            &tags,
            listing_d_tag(),
            farm_ref(),
            "seller".to_string(),
            None,
            None,
        )
        .unwrap_err();
        assert_eq!(parse_error_tag(err), TAG_EXPIRES_AT.to_string());

        let mut tags = base_trade_tags();
        tags.push(vec![TAG_IMAGE.into()]);
        let err = listing_from_tags(
            &tags,
            listing_d_tag(),
            farm_ref(),
            "seller".to_string(),
            None,
            None,
        )
        .unwrap_err();
        assert_eq!(parse_error_tag(err), TAG_IMAGE.to_string());
    }

    #[test]
    fn listing_from_tags_covers_bin_display_and_price_shape_edges() {
        let tags = vec![
            vec!["key".into(), "coffee".into()],
            vec!["title".into(), "Coffee".into()],
            vec!["category".into(), "coffee".into()],
            vec![TAG_RADROOTS_PRIMARY_BIN.into(), "bin-2".into()],
            vec![
                TAG_RADROOTS_BIN.into(),
                "bin-2".into(),
                "500".into(),
                "g".into(),
                "1".into(),
            ],
            vec![
                TAG_RADROOTS_PRICE.into(),
                "bin-2".into(),
                "0.02".into(),
                "USD".into(),
                "1".into(),
                "g".into(),
                "10".into(),
            ],
        ];
        let err = listing_from_tags(
            &tags,
            listing_d_tag(),
            farm_ref(),
            "seller".to_string(),
            None,
            None,
        )
        .unwrap_err();
        assert_eq!(parse_error_tag(err), TAG_RADROOTS_BIN.to_string());

        let tags = vec![
            vec!["key".into(), "coffee".into()],
            vec!["title".into(), "Coffee".into()],
            vec!["category".into(), "coffee".into()],
            vec![TAG_RADROOTS_PRIMARY_BIN.into(), "bin-2".into()],
            vec![
                TAG_RADROOTS_BIN.into(),
                "bin-2".into(),
                "500".into(),
                "g".into(),
                "1".into(),
                "kg".into(),
            ],
            vec![
                TAG_RADROOTS_PRICE.into(),
                "bin-2".into(),
                "0.02".into(),
                "USD".into(),
                "1".into(),
                "g".into(),
                "10".into(),
            ],
        ];
        let err = listing_from_tags(
            &tags,
            listing_d_tag(),
            farm_ref(),
            "seller".to_string(),
            None,
            None,
        )
        .unwrap_err();
        assert_eq!(parse_error_tag(err), TAG_RADROOTS_PRICE.to_string());
    }

    #[test]
    fn listing_from_tags_rejects_missing_primary_bin_and_invalid_seller() {
        let mut tags = base_trade_tags();
        tags.retain(|tag| tag[0] != TAG_RADROOTS_PRIMARY_BIN);
        let err = listing_from_tags(
            &tags,
            listing_d_tag(),
            farm_ref(),
            "seller".to_string(),
            None,
            None,
        )
        .unwrap_err();
        assert_eq!(parse_error_tag(err), TAG_RADROOTS_PRIMARY_BIN.to_string());

        let err = listing_from_tags(
            &base_trade_tags(),
            listing_d_tag(),
            farm_ref(),
            "other".to_string(),
            None,
            None,
        )
        .unwrap_err();
        assert_eq!(parse_error_tag(err), TAG_P.to_string());

        let mut tags = base_trade_tags();
        tags[4][1] = "missing".into();
        let err = listing_from_tags(
            &tags,
            listing_d_tag(),
            farm_ref(),
            "seller".to_string(),
            None,
            None,
        )
        .unwrap_err();
        assert_eq!(parse_error_tag(err), TAG_RADROOTS_PRIMARY_BIN.to_string());
    }

    #[test]
    fn parse_farm_and_reference_helpers_cover_all_paths() {
        let valid_farm_tags = vec![vec![
            TAG_A.into(),
            format!("{KIND_FARM}:seller:AAAAAAAAAAAAAAAAAAAAAA"),
        ]];
        let farm = parse_farm_ref(&valid_farm_tags).unwrap();
        assert_eq!(farm.pubkey, farm_ref().pubkey);
        assert_eq!(farm.d_tag, farm_ref().d_tag);
        assert_eq!(
            parse_error_tag(parse_farm_ref(&[]).unwrap_err()),
            TAG_A.to_string()
        );
        assert_eq!(
            parse_error_tag(parse_farm_ref(&[vec![TAG_A.into()]]).unwrap_err()),
            TAG_A.to_string()
        );
        assert_eq!(
            parse_error_tag(parse_farm_ref(&[vec![TAG_A.into(), "bad".into()]]).unwrap_err()),
            TAG_A.to_string()
        );
        assert_eq!(
            parse_error_tag(
                parse_farm_ref(&[vec![TAG_A.into(), format!("1:seller:{}", farm_ref().d_tag)]])
                    .unwrap_err()
            ),
            TAG_A.to_string()
        );
        assert_eq!(
            parse_error_tag(
                parse_farm_ref(&[vec![
                    TAG_A.into(),
                    format!("{KIND_FARM}: :{}", farm_ref().d_tag)
                ]])
                .unwrap_err()
            ),
            TAG_A.to_string()
        );
        assert_eq!(
            parse_error_tag(
                parse_farm_ref(&[vec![TAG_A.into(), format!("{KIND_FARM}:seller:")]]).unwrap_err()
            ),
            TAG_A.to_string()
        );
        assert_eq!(
            parse_error_tag(
                parse_farm_ref(&[vec![TAG_A.into(), format!("{KIND_FARM}")]]).unwrap_err()
            ),
            TAG_A.to_string()
        );
        assert_eq!(
            parse_error_tag(
                parse_farm_ref(&[vec![TAG_A.into(), format!("{KIND_FARM}:seller")]]).unwrap_err()
            ),
            TAG_A.to_string()
        );
        assert_eq!(
            parse_error_tag(
                parse_farm_ref(&[vec![TAG_A.into(), format!("{KIND_FARM}:seller:not-base64")]])
                    .unwrap_err()
            ),
            TAG_A.to_string()
        );

        assert_eq!(
            parse_farm_pubkey(&[vec![TAG_P.into(), "seller".into()]]).unwrap(),
            "seller".to_string()
        );
        assert_eq!(
            parse_error_tag(parse_farm_pubkey(&[]).unwrap_err()),
            TAG_P.to_string()
        );
        assert_eq!(
            parse_error_tag(parse_farm_pubkey(&[vec![TAG_P.into()]]).unwrap_err()),
            TAG_P.to_string()
        );
        assert_eq!(
            parse_error_tag(parse_farm_pubkey(&[vec![TAG_P.into(), " ".into()]]).unwrap_err()),
            TAG_P.to_string()
        );

        assert!(parse_resource_area(&[]).unwrap().is_none());
        let area_tag = vec![vec![
            TAG_RADROOTS_RESOURCE_AREA.into(),
            format!("{KIND_RESOURCE_AREA}:seller:AAAAAAAAAAAAAAAAAAAAAQ"),
        ]];
        assert!(parse_resource_area(&area_tag).unwrap().is_some());
        let missing_area = vec![vec![TAG_RADROOTS_RESOURCE_AREA.into()]];
        assert_eq!(
            parse_error_tag(parse_resource_area(&missing_area).unwrap_err()),
            TAG_RADROOTS_RESOURCE_AREA.to_string()
        );
        let invalid_area_kind = vec![vec![TAG_RADROOTS_RESOURCE_AREA.into(), "bad".into()]];
        assert_eq!(
            parse_error_tag(parse_resource_area(&invalid_area_kind).unwrap_err()),
            TAG_RADROOTS_RESOURCE_AREA.to_string()
        );
        let missing_area_pubkey = vec![vec![
            TAG_RADROOTS_RESOURCE_AREA.into(),
            format!("{KIND_RESOURCE_AREA}"),
        ]];
        assert_eq!(
            parse_error_tag(parse_resource_area(&missing_area_pubkey).unwrap_err()),
            TAG_RADROOTS_RESOURCE_AREA.to_string()
        );
        let missing_area_d = vec![vec![
            TAG_RADROOTS_RESOURCE_AREA.into(),
            format!("{KIND_RESOURCE_AREA}:seller"),
        ]];
        assert_eq!(
            parse_error_tag(parse_resource_area(&missing_area_d).unwrap_err()),
            TAG_RADROOTS_RESOURCE_AREA.to_string()
        );
        let bad_area = vec![vec![
            TAG_RADROOTS_RESOURCE_AREA.into(),
            "1:seller:bad".into(),
        ]];
        assert_eq!(
            parse_error_tag(parse_resource_area(&bad_area).unwrap_err()),
            TAG_RADROOTS_RESOURCE_AREA.to_string()
        );
        let empty_area = vec![vec![
            TAG_RADROOTS_RESOURCE_AREA.into(),
            format!("{KIND_RESOURCE_AREA}: :{}", listing_d_tag()),
        ]];
        assert_eq!(
            parse_error_tag(parse_resource_area(&empty_area).unwrap_err()),
            TAG_RADROOTS_RESOURCE_AREA.to_string()
        );
        let empty_area_d = vec![vec![
            TAG_RADROOTS_RESOURCE_AREA.into(),
            format!("{KIND_RESOURCE_AREA}:seller:"),
        ]];
        assert_eq!(
            parse_error_tag(parse_resource_area(&empty_area_d).unwrap_err()),
            TAG_RADROOTS_RESOURCE_AREA.to_string()
        );
        let invalid_area_d = vec![vec![
            TAG_RADROOTS_RESOURCE_AREA.into(),
            format!("{KIND_RESOURCE_AREA}:seller:not-base64"),
        ]];
        assert_eq!(
            parse_error_tag(parse_resource_area(&invalid_area_d).unwrap_err()),
            TAG_RADROOTS_RESOURCE_AREA.to_string()
        );

        assert!(parse_plot_ref(&[]).unwrap().is_none());
        let plot_tag = vec![vec![
            TAG_RADROOTS_PLOT.into(),
            format!("{KIND_PLOT}:seller:AAAAAAAAAAAAAAAAAAAAAQ"),
        ]];
        assert!(parse_plot_ref(&plot_tag).unwrap().is_some());
        let missing_plot = vec![vec![TAG_RADROOTS_PLOT.into()]];
        assert_eq!(
            parse_error_tag(parse_plot_ref(&missing_plot).unwrap_err()),
            TAG_RADROOTS_PLOT.to_string()
        );
        let missing_plot_pubkey = vec![vec![TAG_RADROOTS_PLOT.into(), format!("{KIND_PLOT}")]];
        assert_eq!(
            parse_error_tag(parse_plot_ref(&missing_plot_pubkey).unwrap_err()),
            TAG_RADROOTS_PLOT.to_string()
        );
        let missing_plot_d = vec![vec![
            TAG_RADROOTS_PLOT.into(),
            format!("{KIND_PLOT}:seller"),
        ]];
        assert_eq!(
            parse_error_tag(parse_plot_ref(&missing_plot_d).unwrap_err()),
            TAG_RADROOTS_PLOT.to_string()
        );
        let bad_plot = vec![vec![TAG_RADROOTS_PLOT.into(), "1:seller:bad".into()]];
        assert_eq!(
            parse_error_tag(parse_plot_ref(&bad_plot).unwrap_err()),
            TAG_RADROOTS_PLOT.to_string()
        );
        let empty_plot = vec![vec![
            TAG_RADROOTS_PLOT.into(),
            format!("{KIND_PLOT}: :{}", listing_d_tag()),
        ]];
        assert_eq!(
            parse_error_tag(parse_plot_ref(&empty_plot).unwrap_err()),
            TAG_RADROOTS_PLOT.to_string()
        );
        let empty_plot_d = vec![vec![
            TAG_RADROOTS_PLOT.into(),
            format!("{KIND_PLOT}:seller:"),
        ]];
        assert_eq!(
            parse_error_tag(parse_plot_ref(&empty_plot_d).unwrap_err()),
            TAG_RADROOTS_PLOT.to_string()
        );
        let invalid_plot_d = vec![vec![
            TAG_RADROOTS_PLOT.into(),
            format!("{KIND_PLOT}:seller:not-base64"),
        ]];
        assert_eq!(
            parse_error_tag(parse_plot_ref(&invalid_plot_d).unwrap_err()),
            TAG_RADROOTS_PLOT.to_string()
        );
    }

    #[test]
    fn helper_functions_cover_assigners_and_classifiers() {
        assert_eq!(clean_value(" value "), Some("value".into()));
        assert_eq!(clean_value(" "), None);
        assert_eq!(clean_value("null"), None);

        let mut s = String::new();
        let val = "one".to_string();
        set_if_empty(&mut s, Some(&val));
        assert_eq!(s, "one");
        let next = "two".to_string();
        set_if_empty(&mut s, Some(&next));
        assert_eq!(s, "one");
        let mut empty = String::new();
        let nullish = "null".to_string();
        set_if_empty(&mut empty, Some(&nullish));
        assert_eq!(empty, "");

        let mut opt = None;
        let v = "set".to_string();
        set_optional(&mut opt, Some(&v));
        assert_eq!(opt.as_deref(), Some("set"));
        let w = "skip".to_string();
        set_optional(&mut opt, Some(&w));
        assert_eq!(opt.as_deref(), Some("set"));
        let mut opt_none = None;
        let blank = " ".to_string();
        set_optional(&mut opt_none, Some(&blank));
        assert_eq!(opt_none, None);

        assert!(matches!(
            parse_status("ACTIVE"),
            RadrootsListingStatus::Active
        ));
        assert!(matches!(parse_status("sold"), RadrootsListingStatus::Sold));
        assert_eq!(
            format!("{:?}", parse_status("queued")),
            "Other { value: \"queued\" }"
        );

        assert_eq!(parse_image_size("100x200").unwrap().w, 100);
        assert!(parse_image_size("invalid").is_none());
        assert!(parse_image_size("100xbad").is_none());
    }

    #[test]
    fn parse_discount_and_bin_helpers_cover_error_paths() {
        let discount = RadrootsCoreDiscount {
            scope: RadrootsCoreDiscountScope::OrderTotal,
            threshold: RadrootsCoreDiscountThreshold::OrderQuantity {
                min: RadrootsCoreQuantity::new("1".parse().unwrap(), RadrootsCoreUnit::MassG),
            },
            value: RadrootsCoreDiscountValue::MoneyPerBin(RadrootsCoreMoney::new(
                "1".parse().unwrap(),
                RadrootsCoreCurrency::USD,
            )),
        };
        let payload = serde_json::to_string(&discount).unwrap();
        assert!(parse_discount(&payload).is_ok());
        assert_eq!(
            parse_error_tag(parse_discount("{").unwrap_err()),
            TAG_RADROOTS_DISCOUNT.to_string()
        );
        assert_eq!(
            parse_error_tag(TradeListingParseError::InvalidJson("x".into())),
            "x".to_string()
        );

        let mut drafts = Vec::new();
        let mut order_index = 0usize;
        let first = upsert_bin(&mut drafts, "a", &mut order_index);
        first.quantity = Some(RadrootsCoreQuantity::new(
            "1".parse().unwrap(),
            RadrootsCoreUnit::MassG,
        ));
        first.price_per_canonical_unit = Some(RadrootsCoreQuantityPrice::new(
            RadrootsCoreMoney::new("1".parse().unwrap(), RadrootsCoreCurrency::USD),
            RadrootsCoreQuantity::new("1".parse().unwrap(), RadrootsCoreUnit::MassG),
        ));

        let second = upsert_bin(&mut drafts, "a", &mut order_index);
        assert_eq!(second.order_index, 0);
        assert_eq!(order_index, 1);
        assert!(build_bins(drafts).is_ok());

        let draft_missing_qty = BinDraft {
            bin_id: "b".into(),
            order_index: 0,
            quantity: None,
            display_amount: None,
            display_unit: None,
            display_label: None,
            price_per_canonical_unit: Some(RadrootsCoreQuantityPrice::new(
                RadrootsCoreMoney::new("1".parse().unwrap(), RadrootsCoreCurrency::USD),
                RadrootsCoreQuantity::new("1".parse().unwrap(), RadrootsCoreUnit::MassG),
            )),
            display_price: None,
            display_price_unit: None,
        };
        assert_eq!(
            parse_error_tag(build_bins(vec![draft_missing_qty]).unwrap_err()),
            TAG_RADROOTS_BIN.to_string()
        );

        let draft_missing_price = BinDraft {
            bin_id: "b".into(),
            order_index: 0,
            quantity: Some(RadrootsCoreQuantity::new(
                "1".parse().unwrap(),
                RadrootsCoreUnit::MassG,
            )),
            display_amount: None,
            display_unit: None,
            display_label: None,
            price_per_canonical_unit: None,
            display_price: None,
            display_price_unit: None,
        };
        assert_eq!(
            parse_error_tag(build_bins(vec![draft_missing_price]).unwrap_err()),
            TAG_RADROOTS_PRICE.to_string()
        );

        let draft_mismatch = BinDraft {
            bin_id: "b".into(),
            order_index: 0,
            quantity: Some(RadrootsCoreQuantity::new(
                "1".parse().unwrap(),
                RadrootsCoreUnit::MassG,
            )),
            display_amount: None,
            display_unit: None,
            display_label: None,
            price_per_canonical_unit: Some(RadrootsCoreQuantityPrice::new(
                RadrootsCoreMoney::new("1".parse().unwrap(), RadrootsCoreCurrency::USD),
                RadrootsCoreQuantity::new("1".parse().unwrap(), RadrootsCoreUnit::Each),
            )),
            display_price: None,
            display_price_unit: None,
        };
        assert_eq!(
            parse_error_tag(build_bins(vec![draft_mismatch]).unwrap_err()),
            TAG_RADROOTS_PRICE.to_string()
        );

        let tags = vec![
            vec!["key".into(), "coffee".into()],
            vec!["title".into(), "Coffee".into()],
            vec!["category".into(), "coffee".into()],
            vec![TAG_RADROOTS_PRIMARY_BIN.into(), "bin-2".into()],
            vec![
                TAG_RADROOTS_BIN.into(),
                "bin-2".into(),
                "500".into(),
                "g".into(),
            ],
            vec![
                TAG_RADROOTS_PRICE.into(),
                "bin-2".into(),
                "0.02".into(),
                "USD".into(),
                "1".into(),
                "g".into(),
            ],
            vec![TAG_GEOHASH.into()],
        ];
        let listing = listing_from_tags(
            &tags,
            listing_d_tag(),
            farm_ref(),
            "seller".to_string(),
            None,
            None,
        )
        .expect("compact listing");
        assert_eq!(listing.primary_bin_id, "bin-2");
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

fn build_bins(
    mut drafts: Vec<BinDraft>,
) -> Result<Vec<RadrootsListingBin>, TradeListingParseError> {
    drafts.sort_by_key(|draft| draft.order_index);
    let mut bins = Vec::with_capacity(drafts.len());
    for draft in drafts {
        let quantity = draft
            .quantity
            .ok_or_else(|| TradeListingParseError::MissingTag(TAG_RADROOTS_BIN.to_string()))?;
        let price = draft
            .price_per_canonical_unit
            .ok_or_else(|| TradeListingParseError::MissingTag(TAG_RADROOTS_PRICE.to_string()))?;
        if quantity.unit != price.quantity.unit {
            return Err(TradeListingParseError::InvalidTag(
                TAG_RADROOTS_PRICE.to_string(),
            ));
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
