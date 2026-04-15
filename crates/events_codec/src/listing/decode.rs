#![cfg(feature = "serde_json")]

#[cfg(not(feature = "std"))]
use alloc::{
    string::{String, ToString},
    vec::Vec,
};

use radroots_core::{
    RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreDiscount, RadrootsCoreMoney,
    RadrootsCoreQuantity, RadrootsCoreQuantityPrice, RadrootsCoreUnit,
};
use radroots_events::{
    RadrootsNostrEvent,
    farm::RadrootsFarmRef,
    kinds::{KIND_FARM, KIND_PLOT, KIND_RESOURCE_AREA, is_listing_kind},
    listing::{
        RadrootsListing, RadrootsListingAvailability, RadrootsListingBin,
        RadrootsListingDeliveryMethod, RadrootsListingImage, RadrootsListingImageSize,
        RadrootsListingLocation, RadrootsListingProduct, RadrootsListingStatus,
    },
    plot::RadrootsPlotRef,
    resource_area::RadrootsResourceAreaRef,
    tags::TAG_D,
};

use crate::d_tag::validate_d_tag_tag;
use crate::error::EventParseError;
use crate::parsed::{RadrootsParsedData, RadrootsParsedEvent};

const EXPECTED_LISTING_KINDS: &str = "30402 or 30403";
const TAG_A: &str = "a";
const TAG_P: &str = "p";
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
const TAG_RADROOTS_AVAILABILITY_START: &str = "radroots:availability_start";
const TAG_STATUS: &str = "status";
const TAG_EXPIRES_AT: &str = "expires_at";

fn parse_decimal(value: &str, field: &'static str) -> Result<RadrootsCoreDecimal, EventParseError> {
    value
        .parse::<RadrootsCoreDecimal>()
        .map_err(|_| EventParseError::InvalidTag(field))
}

fn parse_currency(
    value: &str,
    field: &'static str,
) -> Result<RadrootsCoreCurrency, EventParseError> {
    let upper = value.trim().to_ascii_uppercase();
    RadrootsCoreCurrency::from_str_upper(&upper).map_err(|_| EventParseError::InvalidTag(field))
}

fn parse_unit(value: &str, field: &'static str) -> Result<RadrootsCoreUnit, EventParseError> {
    value
        .parse::<RadrootsCoreUnit>()
        .map_err(|_| EventParseError::InvalidTag(field))
}

fn parse_u64_tag_value(
    value: Option<&String>,
    field: &'static str,
) -> Result<u64, EventParseError> {
    value
        .ok_or(EventParseError::InvalidTag(field))?
        .parse::<u64>()
        .map_err(|_| EventParseError::InvalidTag(field))
}

fn parse_d_tag(tags: &[Vec<String>]) -> Result<String, EventParseError> {
    let tag = tags
        .iter()
        .find(|tag| tag.first().map(|value| value.as_str()) == Some(TAG_D))
        .ok_or(EventParseError::MissingTag(TAG_D))?;
    let value = tag
        .get(1)
        .map(|value| value.to_string())
        .ok_or(EventParseError::InvalidTag(TAG_D))?;
    if value.trim().is_empty() {
        return Err(EventParseError::InvalidTag(TAG_D));
    }
    validate_d_tag_tag(&value, TAG_D)?;
    Ok(value)
}

fn parse_farm_ref(tags: &[Vec<String>]) -> Result<RadrootsFarmRef, EventParseError> {
    for tag in tags
        .iter()
        .filter(|tag| tag.first().map(|value| value.as_str()) == Some(TAG_A))
    {
        let value = tag
            .get(1)
            .map(|value| value.to_string())
            .ok_or(EventParseError::InvalidTag(TAG_A))?;
        let mut parts = value.splitn(3, ':');
        let kind = parts
            .next()
            .and_then(|raw| raw.parse::<u32>().ok())
            .ok_or(EventParseError::InvalidTag(TAG_A))?;
        if kind != KIND_FARM {
            continue;
        }
        let pubkey = parts
            .next()
            .ok_or(EventParseError::InvalidTag(TAG_A))?
            .to_string();
        let d_tag = parts
            .next()
            .ok_or(EventParseError::InvalidTag(TAG_A))?
            .to_string();
        if pubkey.trim().is_empty() || d_tag.trim().is_empty() {
            return Err(EventParseError::InvalidTag(TAG_A));
        }
        validate_d_tag_tag(&d_tag, TAG_A)?;
        return Ok(RadrootsFarmRef { pubkey, d_tag });
    }
    Err(EventParseError::MissingTag(TAG_A))
}

fn parse_farm_pubkey(tags: &[Vec<String>]) -> Result<String, EventParseError> {
    let tag = tags
        .iter()
        .find(|tag| tag.first().map(|value| value.as_str()) == Some(TAG_P))
        .ok_or(EventParseError::MissingTag(TAG_P))?;
    let value = tag
        .get(1)
        .map(|value| value.to_string())
        .ok_or(EventParseError::InvalidTag(TAG_P))?;
    if value.trim().is_empty() {
        return Err(EventParseError::InvalidTag(TAG_P));
    }
    Ok(value)
}

fn parse_resource_area(
    tags: &[Vec<String>],
) -> Result<Option<RadrootsResourceAreaRef>, EventParseError> {
    let tag = tags
        .iter()
        .find(|tag| tag.first().map(|value| value.as_str()) == Some(TAG_RADROOTS_RESOURCE_AREA));
    let Some(tag) = tag else {
        return Ok(None);
    };
    let value = tag
        .get(1)
        .map(|value| value.to_string())
        .ok_or(EventParseError::InvalidTag(TAG_RADROOTS_RESOURCE_AREA))?;
    let mut parts = value.splitn(3, ':');
    let kind = parts
        .next()
        .and_then(|raw| raw.parse::<u32>().ok())
        .ok_or(EventParseError::InvalidTag(TAG_RADROOTS_RESOURCE_AREA))?;
    if kind != KIND_RESOURCE_AREA {
        return Err(EventParseError::InvalidTag(TAG_RADROOTS_RESOURCE_AREA));
    }
    let pubkey = parts
        .next()
        .ok_or(EventParseError::InvalidTag(TAG_RADROOTS_RESOURCE_AREA))?
        .to_string();
    let d_tag = parts
        .next()
        .ok_or(EventParseError::InvalidTag(TAG_RADROOTS_RESOURCE_AREA))?
        .to_string();
    if pubkey.trim().is_empty() || d_tag.trim().is_empty() {
        return Err(EventParseError::InvalidTag(TAG_RADROOTS_RESOURCE_AREA));
    }
    validate_d_tag_tag(&d_tag, TAG_RADROOTS_RESOURCE_AREA)?;
    Ok(Some(RadrootsResourceAreaRef { pubkey, d_tag }))
}

fn parse_plot_ref(tags: &[Vec<String>]) -> Result<Option<RadrootsPlotRef>, EventParseError> {
    let tag = tags
        .iter()
        .find(|tag| tag.first().map(|value| value.as_str()) == Some(TAG_RADROOTS_PLOT));
    let Some(tag) = tag else {
        return Ok(None);
    };
    let value = tag
        .get(1)
        .map(|value| value.to_string())
        .ok_or(EventParseError::InvalidTag(TAG_RADROOTS_PLOT))?;
    let mut parts = value.splitn(3, ':');
    let kind = parts
        .next()
        .and_then(|raw| raw.parse::<u32>().ok())
        .ok_or(EventParseError::InvalidTag(TAG_RADROOTS_PLOT))?;
    if kind != KIND_PLOT {
        return Err(EventParseError::InvalidTag(TAG_RADROOTS_PLOT));
    }
    let pubkey = parts
        .next()
        .ok_or(EventParseError::InvalidTag(TAG_RADROOTS_PLOT))?
        .to_string();
    let d_tag = parts
        .next()
        .ok_or(EventParseError::InvalidTag(TAG_RADROOTS_PLOT))?
        .to_string();
    if pubkey.trim().is_empty() || d_tag.trim().is_empty() {
        return Err(EventParseError::InvalidTag(TAG_RADROOTS_PLOT));
    }
    validate_d_tag_tag(&d_tag, TAG_RADROOTS_PLOT)?;
    Ok(Some(RadrootsPlotRef { pubkey, d_tag }))
}

pub fn listing_from_event(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<RadrootsListing, EventParseError> {
    if !is_listing_kind(kind) {
        return Err(EventParseError::InvalidKind {
            expected: EXPECTED_LISTING_KINDS,
            got: kind,
        });
    }
    listing_from_event_parts(tags, content)
}

pub fn listing_from_event_parts(
    tags: &[Vec<String>],
    _content: &str,
) -> Result<RadrootsListing, EventParseError> {
    let d_tag = parse_d_tag(tags)?;
    let farm_ref = parse_farm_ref(tags)?;
    let farm_pubkey = parse_farm_pubkey(tags)?;
    let resource_area = parse_resource_area(tags)?;
    let plot = parse_plot_ref(tags)?;

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
        .any(|tag| tag.first().map(|value| value.as_str()) == Some(TAG_LOCATION) && tag.len() >= 3);

    for tag in tags {
        if tag.is_empty() {
            continue;
        }
        match tag[0].as_str() {
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
                    let primary = tag
                        .get(1)
                        .and_then(|value| clean_value(value))
                        .ok_or(EventParseError::InvalidTag(TAG_LOCATION))?;
                    let mut parsed = RadrootsListingLocation {
                        primary,
                        city: None,
                        region: None,
                        country: None,
                        lat: None,
                        lng: None,
                        geohash: None,
                    };
                    if let Some(city) = tag.get(2).and_then(|value| clean_value(value)) {
                        parsed.city = Some(city);
                    }
                    if let Some(region) = tag.get(3).and_then(|value| clean_value(value)) {
                        parsed.region = Some(region);
                    }
                    if let Some(country) = tag.get(4).and_then(|value| clean_value(value)) {
                        parsed.country = Some(country);
                    }
                    location = Some(parsed);
                } else {
                    set_optional(&mut product.location, tag.get(1));
                }
            }
            "profile" => set_optional(&mut product.profile, tag.get(1)),
            "year" => set_optional(&mut product.year, tag.get(1)),
            TAG_PRICE => {}
            TAG_RADROOTS_PRIMARY_BIN => {
                let value = tag
                    .get(1)
                    .and_then(|value| clean_value(value))
                    .ok_or(EventParseError::InvalidTag(TAG_RADROOTS_PRIMARY_BIN))?;
                if let Some(existing) = primary_bin_id.as_ref() {
                    if existing != &value {
                        return Err(EventParseError::InvalidTag(TAG_RADROOTS_PRIMARY_BIN));
                    }
                } else {
                    primary_bin_id = Some(value);
                }
            }
            TAG_RADROOTS_BIN => {
                if tag.len() < 4 || tag.len() > 7 {
                    return Err(EventParseError::InvalidTag(TAG_RADROOTS_BIN));
                }
                let bin_id = tag
                    .get(1)
                    .and_then(|value| clean_value(value))
                    .ok_or(EventParseError::InvalidTag(TAG_RADROOTS_BIN))?;
                let amount = parse_decimal(&tag[2], TAG_RADROOTS_BIN)?;
                let unit = parse_unit(&tag[3], TAG_RADROOTS_BIN)?;
                if unit != unit.canonical_unit() {
                    return Err(EventParseError::InvalidTag(TAG_RADROOTS_BIN));
                }
                let bin = upsert_bin(&mut bin_drafts, &bin_id, &mut bin_order);
                if bin.quantity.is_some() {
                    return Err(EventParseError::InvalidTag(TAG_RADROOTS_BIN));
                }
                bin.quantity = Some(RadrootsCoreQuantity::new(amount, unit));

                match tag.as_slice() {
                    [_, _, _, _, display_amount_raw, display_unit_raw]
                    | [_, _, _, _, display_amount_raw, display_unit_raw, _] => {
                        let display_amount = parse_decimal(display_amount_raw, TAG_RADROOTS_BIN)?;
                        let display_unit = parse_unit(display_unit_raw, TAG_RADROOTS_BIN)?;
                        bin.display_amount = Some(display_amount);
                        bin.display_unit = Some(display_unit);
                        if let [_, _, _, _, _, _, label] = tag.as_slice() {
                            bin.display_label = clean_value(label);
                        }
                    }
                    [_, _, _, _, _] => return Err(EventParseError::InvalidTag(TAG_RADROOTS_BIN)),
                    _ => {}
                }
            }
            TAG_RADROOTS_PRICE => {
                if tag.len() < 6 || tag.len() > 8 {
                    return Err(EventParseError::InvalidTag(TAG_RADROOTS_PRICE));
                }
                let bin_id = tag
                    .get(1)
                    .and_then(|value| clean_value(value))
                    .ok_or(EventParseError::InvalidTag(TAG_RADROOTS_PRICE))?;
                let amount = parse_decimal(&tag[2], TAG_RADROOTS_PRICE)?;
                let currency = parse_currency(&tag[3], TAG_RADROOTS_PRICE)?;
                let per_amount = parse_decimal(&tag[4], TAG_RADROOTS_PRICE)?;
                let per_unit = parse_unit(&tag[5], TAG_RADROOTS_PRICE)?;
                let price_per_canonical_unit = RadrootsCoreQuantityPrice::new(
                    RadrootsCoreMoney::new(amount, currency),
                    RadrootsCoreQuantity::new(per_amount, per_unit),
                );
                if !price_per_canonical_unit.is_price_per_canonical_unit() {
                    return Err(EventParseError::InvalidTag(TAG_RADROOTS_PRICE));
                }
                let bin = upsert_bin(&mut bin_drafts, &bin_id, &mut bin_order);
                if bin.price_per_canonical_unit.is_some() {
                    return Err(EventParseError::InvalidTag(TAG_RADROOTS_PRICE));
                }
                bin.price_per_canonical_unit = Some(price_per_canonical_unit);

                match tag.as_slice() {
                    [_, _, _, _, _, _, _] => {
                        return Err(EventParseError::InvalidTag(TAG_RADROOTS_PRICE));
                    }
                    [_, _, _, _, _, _, display_price_raw, display_unit_raw] => {
                        let display_price = parse_decimal(display_price_raw, TAG_RADROOTS_PRICE)?;
                        let display_unit = parse_unit(display_unit_raw, TAG_RADROOTS_PRICE)?;
                        bin.display_price = Some(RadrootsCoreMoney::new(display_price, currency));
                        bin.display_price_unit = Some(display_unit);
                    }
                    _ => {}
                }
            }
            TAG_RADROOTS_DISCOUNT => {
                let payload = tag
                    .get(1)
                    .ok_or(EventParseError::InvalidTag(TAG_RADROOTS_DISCOUNT))?;
                discounts.push(parse_discount(payload)?);
            }
            TAG_GEOHASH => {
                if let Some(value) = tag.get(1).and_then(|value| clean_value(value)) {
                    geohash = Some(value);
                }
            }
            TAG_INVENTORY => {
                let value = tag
                    .get(1)
                    .ok_or(EventParseError::InvalidTag(TAG_INVENTORY))?;
                inventory_available = Some(parse_decimal(value, TAG_INVENTORY)?);
            }
            TAG_RADROOTS_AVAILABILITY_START => {
                availability_start = Some(parse_u64_tag_value(
                    tag.get(1),
                    TAG_RADROOTS_AVAILABILITY_START,
                )?);
            }
            TAG_EXPIRES_AT => {
                availability_end = Some(parse_u64_tag_value(tag.get(1), TAG_EXPIRES_AT)?);
            }
            TAG_STATUS => {
                let status = tag
                    .get(1)
                    .and_then(|value| clean_value(value))
                    .unwrap_or_default();
                availability_status = Some(parse_status(&status));
            }
            TAG_DELIVERY => {
                let method = tag
                    .get(1)
                    .and_then(|value| clean_value(value))
                    .unwrap_or_default();
                delivery_method = Some(match method.as_str() {
                    "pickup" => RadrootsListingDeliveryMethod::Pickup,
                    "local_delivery" => RadrootsListingDeliveryMethod::LocalDelivery,
                    "shipping" => RadrootsListingDeliveryMethod::Shipping,
                    "other" => RadrootsListingDeliveryMethod::Other {
                        method: tag
                            .get(2)
                            .and_then(|value| clean_value(value))
                            .unwrap_or_default(),
                    },
                    other => RadrootsListingDeliveryMethod::Other {
                        method: other.to_string(),
                    },
                });
            }
            TAG_IMAGE => {
                let url = tag.get(1).ok_or(EventParseError::InvalidTag(TAG_IMAGE))?;
                if url.trim().is_empty() {
                    continue;
                }
                images.push(RadrootsListingImage {
                    url: url.to_string(),
                    size: tag.get(2).and_then(|value| parse_image_size(value)),
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

    let location = location.map(|mut location| {
        location.geohash = location.geohash.or(geohash);
        location
    });

    if farm_pubkey != farm_ref.pubkey {
        return Err(EventParseError::InvalidTag(TAG_P));
    }

    let primary_bin_id =
        primary_bin_id.ok_or(EventParseError::MissingTag(TAG_RADROOTS_PRIMARY_BIN))?;
    let bins = build_bins(bin_drafts)?;
    if !bins.iter().any(|bin| bin.bin_id == primary_bin_id) {
        return Err(EventParseError::InvalidTag(TAG_RADROOTS_PRIMARY_BIN));
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

pub fn data_from_event(
    id: String,
    author: String,
    published_at: u32,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
) -> Result<RadrootsParsedData<RadrootsListing>, EventParseError> {
    let listing = listing_from_event(kind, &tags, &content)?;
    Ok(RadrootsParsedData::new(
        id,
        author,
        published_at,
        kind,
        listing,
    ))
}

pub fn parsed_from_event(
    id: String,
    author: String,
    published_at: u32,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
    sig: String,
) -> Result<RadrootsParsedEvent<RadrootsListing>, EventParseError> {
    let data = data_from_event(
        id.clone(),
        author.clone(),
        published_at,
        kind,
        content.clone(),
        tags.clone(),
    )?;
    Ok(RadrootsParsedEvent::from_parts(
        id,
        author,
        published_at,
        kind,
        content,
        tags,
        sig,
        data.data,
    ))
}

pub fn data_from_nostr_event(
    event: &RadrootsNostrEvent,
) -> Result<RadrootsParsedData<RadrootsListing>, EventParseError> {
    data_from_event(
        event.id.clone(),
        event.author.clone(),
        event.created_at,
        event.kind,
        event.content.clone(),
        event.tags.clone(),
    )
}

pub fn parsed_from_nostr_event(
    event: &RadrootsNostrEvent,
) -> Result<RadrootsParsedEvent<RadrootsListing>, EventParseError> {
    parsed_from_event(
        event.id.clone(),
        event.author.clone(),
        event.created_at,
        event.kind,
        event.content.clone(),
        event.tags.clone(),
        event.sig.clone(),
    )
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
        if let Some(value) = value.and_then(|value| clean_value(value)) {
            *target = value;
        }
    }
}

fn set_optional(target: &mut Option<String>, value: Option<&String>) {
    if target.is_none() {
        if let Some(value) = value.and_then(|value| clean_value(value)) {
            *target = Some(value);
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
    let (w_raw, h_raw) = value.split_once('x')?;
    let w = w_raw.parse::<u32>().ok()?;
    let h = h_raw.parse::<u32>().ok()?;
    Some(RadrootsListingImageSize { w, h })
}

fn parse_discount(payload: &str) -> Result<RadrootsCoreDiscount, EventParseError> {
    serde_json::from_str(payload).map_err(|_| EventParseError::InvalidTag(TAG_RADROOTS_DISCOUNT))
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
    if let Some(position) = bins.iter().position(|bin| bin.bin_id == bin_id) {
        return &mut bins[position];
    }
    bins.push(BinDraft {
        bin_id: bin_id.to_string(),
        order_index: *order_index,
        quantity: None,
        display_amount: None,
        display_unit: None,
        display_label: None,
        price_per_canonical_unit: None,
        display_price: None,
        display_price_unit: None,
    });
    *order_index += 1;
    let index = bins.len() - 1;
    &mut bins[index]
}

fn build_bins(mut drafts: Vec<BinDraft>) -> Result<Vec<RadrootsListingBin>, EventParseError> {
    drafts.sort_by_key(|draft| draft.order_index);
    let mut bins = Vec::with_capacity(drafts.len());
    for draft in drafts {
        let quantity = draft
            .quantity
            .ok_or(EventParseError::MissingTag(TAG_RADROOTS_BIN))?;
        let price = draft
            .price_per_canonical_unit
            .ok_or(EventParseError::MissingTag(TAG_RADROOTS_PRICE))?;
        if quantity.unit != price.quantity.unit {
            return Err(EventParseError::InvalidTag(TAG_RADROOTS_PRICE));
        }
        bins.push(RadrootsListingBin {
            bin_id: draft.bin_id,
            quantity,
            price_per_canonical_unit: price,
            display_amount: draft.display_amount,
            display_unit: draft.display_unit,
            display_label: draft.display_label,
            display_price: draft.display_price,
            display_price_unit: draft.display_price_unit,
        });
    }
    Ok(bins)
}
