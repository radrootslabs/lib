//! Passive complete listing decoding; no signature, freshness or stock proof.
use super::{LISTING_KIND, decode_profile};
use crate::{command::CommandError, nip01::EventFields};
use radroots_domain::{
    EnvelopeEventId, UnixSeconds,
    listing::{Listing, ListingId, ListingState, ListingText, Price},
};

/// Complete validated values whose event identity remains a caller claim.
#[derive(Debug)]
pub struct DecodedListing(Listing);
impl DecodedListing {
    /// Borrows passive public values; the concrete adapter authenticates them.
    pub const fn listing(&self) -> &Listing {
        &self.0
    }
}
fn tag<'a>(
    tags: &'a [alloc::vec::Vec<alloc::string::String>],
    name: &str,
    arity: usize,
) -> Result<Option<&'a [alloc::string::String]>, CommandError> {
    let mut found = None;
    for tag in tags {
        if tag.first().is_some_and(|key| key == name) {
            if found.is_some() || tag.len() != arity {
                return Err(CommandError::InvalidInput);
            }
            found = Some(tag.as_slice());
        }
    }
    Ok(found)
}
fn required<'a>(fields: &EventFields<'a>, name: &str) -> Result<&'a str, CommandError> {
    tag(fields.tags, name, 2)?
        .map(|tag| tag[1].as_str())
        .ok_or(CommandError::InvalidInput)
}
/// Decode exact incoming projections in any tag order. Unknown generic Nostr
/// tags confer no authority; duplicate known projections are rejected. The
/// default price tag is exact amount/currency only, never a unit/frequency hack.
pub fn decode_observed(
    fields: &EventFields<'_>,
    event_id: [u8; 32],
) -> Result<DecodedListing, CommandError> {
    if fields.kind != LISTING_KIND {
        return Err(CommandError::InvalidInput);
    }
    crate::nip01::signed_json_len(fields, super::LISTING_EVENT_MAX_BYTES).map_err(|error| {
        match error {
            crate::nip01::WireError::LimitExceeded => CommandError::LimitExceeded,
            _ => CommandError::InvalidInput,
        }
    })?;
    let terms = decode_profile(required(fields, "radroots:listing")?.as_bytes())?;
    let id: ListingId = required(fields, "d")?
        .parse()
        .map_err(|_| CommandError::InvalidInput)?;
    let created_at = UnixSeconds::new(fields.created_at).map_err(|_| CommandError::InvalidInput)?;
    let published_at: UnixSeconds = required(fields, "published_at")?
        .parse()
        .map_err(|_| CommandError::InvalidInput)?;
    let text = ListingText::new(
        required(fields, "title")?,
        required(fields, "summary")?,
        fields.content,
    )
    .map_err(|_| CommandError::InvalidInput)?;
    if required(fields, "location")? != terms.fulfillment().area()
        || required(fields, "radroots:price_unit")? != terms.default_option().unit().label()
    {
        return Err(CommandError::InvalidInput);
    }
    let status = tag(fields.tags, "status", 2)?;
    match terms.state() {
        ListingState::Active if status.is_none_or(|tag| tag[1] != "active") => {
            return Err(CommandError::InvalidInput);
        }
        ListingState::Paused | ListingState::Withdrawn if status.is_some() => {
            return Err(CommandError::InvalidInput);
        }
        _ => {}
    }
    let price = tag(fields.tags, "price", 3)?;
    match terms.default_option().price() {
        Price::Fixed(value)
            if price.is_none_or(|tag| {
                !price_matches(&tag[1], value.rate()) || tag[2] != value.rate().currency().code()
            }) =>
        {
            return Err(CommandError::InvalidInput);
        }
        Price::QuoteRequired | Price::VariableWeight(_) if price.is_some() => {
            return Err(CommandError::InvalidInput);
        }
        _ => {}
    }
    Listing::new(
        id,
        fields.author,
        EnvelopeEventId::from_bytes(event_id),
        created_at,
        published_at,
        text,
        terms,
    )
    .map(DecodedListing)
    .map_err(|_| CommandError::InvalidInput)
}

// Standard NIP99 decimal spellings may vary without changing exact minor
// value. This compares checked integer coefficients, never floating point or
// rounded excess precision, and never modifies the signed tag representation.
fn price_matches(amount: &str, expected: radroots_domain::Money) -> bool {
    let (whole, fraction) = match amount.split_once('.') {
        Some((whole, fraction)) if !fraction.is_empty() => (whole, fraction),
        Some(_) => return false,
        None => (amount, ""),
    };
    if whole.is_empty()
        || !whole
            .bytes()
            .chain(fraction.bytes())
            .all(|b| b.is_ascii_digit())
    {
        return false;
    }
    let mut integral = 0_u64;
    for digit in whole.bytes() {
        let Some(value) = integral
            .checked_mul(10)
            .and_then(|v| v.checked_add(u64::from(digit - b'0')))
        else {
            return false;
        };
        integral = value;
    }
    // Currency's pinned registry admits exponents0..4 only.
    let Some(scale) = 10_u64.checked_pow(u32::from(expected.currency().exponent())) else {
        return false;
    };
    let Some(mut minor) = integral.checked_mul(scale) else {
        return false;
    };
    let mut place = scale;
    for digit in fraction.bytes() {
        if place == 1 {
            if digit != b'0' {
                return false;
            }
        } else {
            place /= 10;
            let Some(next) = u64::from(digit - b'0')
                .checked_mul(place)
                .and_then(|part| minor.checked_add(part))
            else {
                return false;
            };
            minor = next;
        }
    }
    minor == expected.minor().get()
}
