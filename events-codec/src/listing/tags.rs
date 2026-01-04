#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
use alloc::{format, string::{String, ToString}, vec, vec::Vec};

use core::cmp;

use radroots_core::{RadrootsCoreDiscount, RadrootsCoreMoney};
use radroots_events::listing::{
    RadrootsListing, RadrootsListingAvailability, RadrootsListingDeliveryMethod, RadrootsListingFarmRef,
    RadrootsListingBin, RadrootsListingImage, RadrootsListingLocation,
    RadrootsListingStatus,
};
use radroots_events::plot::RadrootsPlotRef;
use radroots_events::resource_area::RadrootsResourceAreaRef;
use radroots_events::kinds::{KIND_FARM, KIND_PLOT, KIND_RESOURCE_AREA};
use radroots_events::tags::TAG_D;

use crate::error::EventEncodeError;

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
const TAG_LABEL: &str = "l";
const TAG_LABEL_NS: &str = "L";
const TAG_DD: &str = "dd";
const TAG_DD_LAT: &str = "dd.lat";
const TAG_DD_LON: &str = "dd.lon";
const TAG_INVENTORY: &str = "inventory";
const TAG_DELIVERY: &str = "delivery";
const TAG_PUBLISHED_AT: &str = "published_at";
const TAG_STATUS: &str = "status";
const TAG_EXPIRES_AT: &str = "expires_at";
const TAG_P: &str = "p";
const TAG_A: &str = "a";

const GEOHASH_PRECISION_DEFAULT: usize = 9;
const DD_MAX_RESOLUTION_DEFAULT: u32 = 9;

const BASE32_CODES: &[u8; 32] = b"0123456789bcdefghjkmnpqrstuvwxyz";

#[derive(Clone, Copy, Debug)]
pub struct ListingTagOptions {
    pub geohash_precision: usize,
    pub dd_max_resolution: u32,
    pub include_geohash: bool,
    pub include_gps: bool,
    pub include_inventory: bool,
    pub include_availability: bool,
    pub include_delivery: bool,
}

impl Default for ListingTagOptions {
    fn default() -> Self {
        Self {
            geohash_precision: GEOHASH_PRECISION_DEFAULT,
            dd_max_resolution: DD_MAX_RESOLUTION_DEFAULT,
            include_geohash: true,
            include_gps: true,
            include_inventory: false,
            include_availability: false,
            include_delivery: false,
        }
    }
}

impl ListingTagOptions {
    pub fn with_trade_fields() -> Self {
        Self {
            include_inventory: true,
            include_availability: true,
            include_delivery: true,
            ..Self::default()
        }
    }
}

pub fn listing_tags(listing: &RadrootsListing) -> Result<Vec<Vec<String>>, EventEncodeError> {
    listing_tags_with_options(listing, ListingTagOptions::default())
}

pub fn listing_tags_full(listing: &RadrootsListing) -> Result<Vec<Vec<String>>, EventEncodeError> {
    listing_tags_with_options(listing, ListingTagOptions::with_trade_fields())
}

pub fn listing_tags_with_options(
    listing: &RadrootsListing,
    options: ListingTagOptions,
) -> Result<Vec<Vec<String>>, EventEncodeError> {
    let d_tag = listing.d_tag.trim();
    if d_tag.is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("d"));
    }
    if listing.primary_bin_id.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("primary_bin_id"));
    }
    if listing.bins.is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("bins"));
    }

    let mut tags: Vec<Vec<String>> = Vec::new();
    tags.push(vec![TAG_D.to_string(), d_tag.to_string()]);
    push_farm_tags(&mut tags, &listing.farm)?;
    if let Some(resource_area) = listing.resource_area.as_ref() {
        tags.push(tag_listing_resource_area(resource_area)?);
    }
    if let Some(plot) = listing.plot.as_ref() {
        tags.push(tag_listing_plot(plot)?);
    }

    let product = &listing.product;
    push_tag_value(&mut tags, "key", &product.key);
    push_tag_value(&mut tags, "title", &product.title);
    push_tag_value(&mut tags, "category", &product.category);
    if let Some(summary) = product.summary.as_deref() {
        push_tag_value(&mut tags, "summary", summary);
    }
    if let Some(process) = product.process.as_deref() {
        push_tag_value(&mut tags, "process", process);
    }
    if let Some(lot) = product.lot.as_deref() {
        push_tag_value(&mut tags, "lot", lot);
    }
    if let Some(location) = product.location.as_deref() {
        push_tag_value(&mut tags, "location", location);
    }
    if let Some(profile) = product.profile.as_deref() {
        push_tag_value(&mut tags, "profile", profile);
    }
    if let Some(year) = product.year.as_deref() {
        push_tag_value(&mut tags, "year", year);
    }

    tags.push(vec![TAG_RADROOTS_PRIMARY_BIN.to_string(), listing.primary_bin_id.clone()]);

    let mut bins: Vec<&RadrootsListingBin> = listing.bins.iter().collect();
    if let Some(pos) = bins.iter().position(|bin| bin.bin_id == listing.primary_bin_id) {
        let primary = bins.remove(pos);
        bins.insert(0, primary);
    }

    for bin in bins {
        tags.push(tag_listing_bin(bin)?);
        tags.push(tag_listing_price(bin)?);
        let total = bin_total_price(bin)?;
        tags.push(tag_listing_price_generic(&total));
    }

    if let Some(discounts) = &listing.discounts {
        for discount in discounts {
            let payload = discount_tag_payload(discount)?;
            tags.push(vec![TAG_RADROOTS_DISCOUNT.to_string(), payload]);
        }
    }

    if options.include_inventory {
        if let Some(inventory) = &listing.inventory_available {
            tags.push(vec![TAG_INVENTORY.to_string(), inventory.to_string()]);
        }
    }

    if options.include_availability {
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
    }

    if options.include_delivery {
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
    }

    if let Some(location) = &listing.location {
        if let Some(primary) = clean_value(&location.primary) {
            let mut tag = Vec::with_capacity(5);
            tag.push(TAG_LOCATION.to_string());
            tag.push(primary);
            if let Some(city) = location.city.as_deref().and_then(clean_value) {
                tag.push(city);
            }
            if let Some(region) = location.region.as_deref().and_then(clean_value) {
                tag.push(region);
            }
            if let Some(country) = location.country.as_deref().and_then(clean_value) {
                tag.push(country);
            }
            tags.push(tag);
            if options.include_geohash || options.include_gps {
                push_location_geotags(&mut tags, location, options);
            }
        }
    }

    if let Some(images) = &listing.images {
        for image in images {
            if let Some(tag) = tag_listing_image(image) {
                tags.push(tag);
            }
        }
    }

    Ok(tags)
}

fn push_farm_tags(
    tags: &mut Vec<Vec<String>>,
    farm: &RadrootsListingFarmRef,
) -> Result<(), EventEncodeError> {
    if farm.pubkey.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("farm.pubkey"));
    }
    if farm.d_tag.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("farm.d_tag"));
    }
    let mut address = String::new();
    address.push_str(&KIND_FARM.to_string());
    address.push(':');
    address.push_str(&farm.pubkey);
    address.push(':');
    address.push_str(&farm.d_tag);
    tags.push(vec![TAG_P.to_string(), farm.pubkey.clone()]);
    tags.push(vec![TAG_A.to_string(), address]);
    Ok(())
}

fn tag_listing_resource_area(
    area: &RadrootsResourceAreaRef,
) -> Result<Vec<String>, EventEncodeError> {
    if area.pubkey.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("resource_area.pubkey"));
    }
    if area.d_tag.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("resource_area.d_tag"));
    }
    let mut address = String::new();
    address.push_str(&KIND_RESOURCE_AREA.to_string());
    address.push(':');
    address.push_str(&area.pubkey);
    address.push(':');
    address.push_str(&area.d_tag);
    Ok(vec![TAG_RADROOTS_RESOURCE_AREA.to_string(), address])
}

fn tag_listing_plot(plot: &RadrootsPlotRef) -> Result<Vec<String>, EventEncodeError> {
    if plot.pubkey.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("plot.pubkey"));
    }
    if plot.d_tag.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("plot.d_tag"));
    }
    let mut address = String::new();
    address.push_str(&KIND_PLOT.to_string());
    address.push(':');
    address.push_str(&plot.pubkey);
    address.push(':');
    address.push_str(&plot.d_tag);
    Ok(vec![TAG_RADROOTS_PLOT.to_string(), address])
}

fn tag_listing_bin(bin: &RadrootsListingBin) -> Result<Vec<String>, EventEncodeError> {
    if bin.bin_id.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("bin_id"));
    }
    let unit = bin.quantity.unit;
    if unit != unit.canonical_unit() {
        return Err(EventEncodeError::EmptyRequiredField("bin.quantity"));
    }
    let mut tag = Vec::with_capacity(7);
    tag.push(TAG_RADROOTS_BIN.to_string());
    tag.push(bin.bin_id.clone());
    tag.push(bin.quantity.amount.to_string());
    tag.push(unit.code().to_string());
    match (bin.display_amount.as_ref(), bin.display_unit) {
        (Some(amount), Some(unit)) => {
            tag.push(amount.to_string());
            tag.push(unit.code().to_string());
            if let Some(label) = bin
                .display_label
                .as_deref()
                .and_then(clean_value)
                .or_else(|| bin.quantity.label.as_deref().and_then(clean_value))
            {
                tag.push(label);
            }
        }
        (None, None) => {}
        (None, Some(_)) => {
            return Err(EventEncodeError::EmptyRequiredField("bin.display_amount"));
        }
        (Some(_), None) => {
            return Err(EventEncodeError::EmptyRequiredField("bin.display_unit"));
        }
    }
    Ok(tag)
}

fn tag_listing_price(bin: &RadrootsListingBin) -> Result<Vec<String>, EventEncodeError> {
    if bin.bin_id.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("bin_id"));
    }
    let price = &bin.price_per_canonical_unit;
    if !price.is_price_per_canonical_unit() {
        return Err(EventEncodeError::EmptyRequiredField("bin.price_per_canonical_unit"));
    }
    let mut tag = Vec::with_capacity(8);
    tag.push(TAG_RADROOTS_PRICE.to_string());
    tag.push(bin.bin_id.clone());
    tag.push(price.amount.amount.to_string());
    tag.push(price.amount.currency.as_str().to_string());
    tag.push(price.quantity.amount.to_string());
    tag.push(price.quantity.unit.code().to_string());
    match (&bin.display_price, bin.display_price_unit) {
        (Some(price_display), Some(unit)) => {
            if price_display.currency != price.amount.currency {
                return Err(EventEncodeError::EmptyRequiredField("bin.display_price"));
            }
            tag.push(price_display.amount.to_string());
            tag.push(unit.code().to_string());
        }
        (None, None) => {}
        (None, Some(_)) => return Err(EventEncodeError::EmptyRequiredField("bin.display_price")),
        (Some(_), None) => {
            return Err(EventEncodeError::EmptyRequiredField("bin.display_price_unit"));
        }
    }
    Ok(tag)
}

fn bin_total_price(bin: &RadrootsListingBin) -> Result<RadrootsCoreMoney, EventEncodeError> {
    bin.price_per_canonical_unit
        .try_cost_for_quantity_in(&bin.quantity)
        .map_err(|_| EventEncodeError::EmptyRequiredField("bin.price_per_canonical_unit"))
}

fn tag_listing_price_generic(price: &RadrootsCoreMoney) -> Vec<String> {
    let mut tag = Vec::with_capacity(4);
    tag.push(TAG_PRICE.to_string());
    tag.push(price.amount.to_string());
    tag.push(price.currency.as_str().to_string());
    tag
}

fn tag_listing_image(image: &RadrootsListingImage) -> Option<Vec<String>> {
    let url = clean_value(&image.url)?;
    let mut tag = Vec::with_capacity(3);
    tag.push(TAG_IMAGE.to_string());
    tag.push(url);
    if let Some(size) = &image.size {
        tag.push(format!("{}x{}", size.w, size.h));
    }
    Some(tag)
}

fn push_location_geotags(
    tags: &mut Vec<Vec<String>>,
    location: &RadrootsListingLocation,
    options: ListingTagOptions,
) {
    let mut lat = location.lat.filter(|value| value.is_finite());
    let mut lon = location.lng.filter(|value| value.is_finite());
    let location_geohash = location.geohash.as_deref().and_then(clean_value);

    let geohash = if options.include_geohash {
        if let (Some(lat), Some(lon)) = (lat, lon) {
            let precision = options.geohash_precision.max(1);
            Some(geohash_encode(lat, lon, precision))
        } else {
            location_geohash.clone()
        }
    } else {
        None
    };

    if let Some(geohash) = geohash.as_ref() {
        for idx in (1..=geohash.len()).rev() {
            tags.push(vec![TAG_GEOHASH.to_string(), geohash[..idx].to_string()]);
        }
    }

    if options.include_gps {
        if lat.is_none() || lon.is_none() {
            if let Some(geohash) = geohash.as_deref().or(location_geohash.as_deref()) {
                if let Some((decoded_lat, decoded_lon)) = geohash_decode(geohash) {
                    lat = Some(decoded_lat);
                    lon = Some(decoded_lon);
                }
            }
        }
        if let (Some(lat), Some(lon)) = (lat, lon) {
            tags.push(vec![
                TAG_LABEL.to_string(),
                format!("{lat}, {lon}"),
                TAG_DD.to_string(),
            ]);
            let max_resolution = options.dd_max_resolution.max(1);
            let lat_resolution = calculate_resolution(lat, max_resolution);
            let lon_resolution = calculate_resolution(lon, max_resolution);
            tags.push(vec![TAG_LABEL_NS.to_string(), TAG_DD_LAT.to_string()]);
            for idx in (1..=lat_resolution).rev() {
                let truncated = truncate_to_resolution(lat, idx);
                tags.push(vec![
                    TAG_LABEL.to_string(),
                    truncated.to_string(),
                    TAG_DD_LAT.to_string(),
                ]);
            }
            tags.push(vec![TAG_LABEL_NS.to_string(), TAG_DD_LON.to_string()]);
            for idx in (1..=lon_resolution).rev() {
                let truncated = truncate_to_resolution(lon, idx);
                tags.push(vec![
                    TAG_LABEL.to_string(),
                    truncated.to_string(),
                    TAG_DD_LON.to_string(),
                ]);
            }
        }
    }
}

fn calculate_resolution(value: f64, max: u32) -> u32 {
    if value.fract() == 0.0 {
        return 1;
    }
    let s = value.to_string();
    let decimals = s
        .split('.')
        .nth(1)
        .map(|v| v.len() as u32)
        .unwrap_or(0);
    let bounded = cmp::min(decimals, max);
    if bounded == 0 { 1 } else { bounded }
}

fn truncate_to_resolution(value: f64, resolution: u32) -> f64 {
    let multiplier = 10_f64.powi(resolution as i32);
    (value * multiplier).floor() / multiplier
}

fn geohash_encode(latitude: f64, longitude: f64, precision: usize) -> String {
    if precision == 0 {
        return String::new();
    }
    let mut out = String::with_capacity(precision);
    let mut bits: u8 = 0;
    let mut bits_total: u8 = 0;
    let mut hash_value: u8 = 0;
    let mut max_lat = 90.0;
    let mut min_lat = -90.0;
    let mut max_lon = 180.0;
    let mut min_lon = -180.0;

    while out.len() < precision {
        if bits_total % 2 == 0 {
            let mid = (max_lon + min_lon) / 2.0;
            if longitude > mid {
                hash_value = (hash_value << 1) + 1;
                min_lon = mid;
            } else {
                hash_value <<= 1;
                max_lon = mid;
            }
        } else {
            let mid = (max_lat + min_lat) / 2.0;
            if latitude > mid {
                hash_value = (hash_value << 1) + 1;
                min_lat = mid;
            } else {
                hash_value <<= 1;
                max_lat = mid;
            }
        }
        bits += 1;
        bits_total += 1;
        if bits == 5 {
            out.push(BASE32_CODES[hash_value as usize] as char);
            bits = 0;
            hash_value = 0;
        }
    }
    out
}

fn geohash_decode(hash: &str) -> Option<(f64, f64)> {
    let (min_lat, min_lon, max_lat, max_lon) = geohash_decode_bbox(hash)?;
    let lat = (min_lat + max_lat) / 2.0;
    let lon = (min_lon + max_lon) / 2.0;
    Some((lat, lon))
}

fn geohash_decode_bbox(hash: &str) -> Option<(f64, f64, f64, f64)> {
    let mut is_lon = true;
    let mut max_lat = 90.0;
    let mut min_lat = -90.0;
    let mut max_lon = 180.0;
    let mut min_lon = -180.0;

    for b in hash.bytes() {
        let value = base32_value(b)?;
        for bits in (0..5).rev() {
            let bit = (value >> bits) & 1;
            if is_lon {
                let mid = (max_lon + min_lon) / 2.0;
                if bit == 1 {
                    min_lon = mid;
                } else {
                    max_lon = mid;
                }
            } else {
                let mid = (max_lat + min_lat) / 2.0;
                if bit == 1 {
                    min_lat = mid;
                } else {
                    max_lat = mid;
                }
            }
            is_lon = !is_lon;
        }
    }
    Some((min_lat, min_lon, max_lat, max_lon))
}

fn base32_value(c: u8) -> Option<u8> {
    let needle = c.to_ascii_lowercase();
    BASE32_CODES
        .iter()
        .position(|&b| b == needle)
        .map(|idx| idx as u8)
}

fn push_tag_value(tags: &mut Vec<Vec<String>>, key: &str, value: &str) {
    if let Some(cleaned) = clean_value(value) {
        tags.push(vec![key.to_string(), cleaned]);
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

fn discount_tag_payload(discount: &RadrootsCoreDiscount) -> Result<String, EventEncodeError> {
    #[cfg(feature = "serde_json")]
    {
        return serde_json::to_string(discount).map_err(|_| EventEncodeError::Json);
    }
    #[cfg(not(feature = "serde_json"))]
    {
        let _ = discount;
        Err(EventEncodeError::Json)
    }
}

fn status_as_str(status: &RadrootsListingStatus) -> &str {
    match status {
        RadrootsListingStatus::Active => "active",
        RadrootsListingStatus::Sold => "sold",
        RadrootsListingStatus::Other { value } => value.as_str(),
    }
}
