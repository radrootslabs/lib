pub const RADROOTS_PUBLIC_GEOHASH_PRECISION: usize = 5;
pub const RADROOTS_PUBLIC_GEOHASH_BASE32: &str = "0123456789bcdefghjkmnpqrstuvwxyz";

pub fn is_public_geohash5(value: &str) -> bool {
    let value = value.trim();
    value.len() == RADROOTS_PUBLIC_GEOHASH_PRECISION
        && value.bytes().all(|byte| {
            RADROOTS_PUBLIC_GEOHASH_BASE32
                .as_bytes()
                .contains(&byte.to_ascii_lowercase())
        })
}

pub fn has_textual_locality(
    primary: &str,
    city: Option<&str>,
    region: Option<&str>,
    country: Option<&str>,
) -> bool {
    has_public_location_text(primary)
        && [city, region, country]
            .into_iter()
            .flatten()
            .any(has_public_location_text)
}

fn has_public_location_text(value: &str) -> bool {
    let value = value.trim();
    !value.is_empty() && !value.eq_ignore_ascii_case("null")
}
