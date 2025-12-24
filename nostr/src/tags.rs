extern crate alloc;
use alloc::{borrow::Cow, string::String, vec::Vec};

use nostr::nips::nip04;

use crate::error::RadrootsNostrTagsResolveError;
use crate::types::{
    RadrootsNostrEvent,
    RadrootsNostrKeys,
    RadrootsNostrPublicKey,
    RadrootsNostrRelayUrl,
    RadrootsNostrTag,
    RadrootsNostrTagKind,
    RadrootsNostrTagStandard,
};

pub fn radroots_nostr_tag_first_value(tag: &RadrootsNostrTag, key: &str) -> Option<String> {
    if tag.kind() == RadrootsNostrTagKind::custom(key) {
        tag.content().map(|v| v.to_string())
    } else {
        None
    }
}

pub fn radroots_nostr_tag_at_value(tag: &RadrootsNostrTag, index: usize) -> Option<String> {
    tag.as_slice().get(index).cloned()
}

pub fn radroots_nostr_tag_slice(tag: &RadrootsNostrTag, start: usize) -> Option<Vec<String>> {
    tag.as_slice().get(start..).map(|s| s.to_vec())
}

pub fn radroots_nostr_tag_relays_parse(
    tag: &RadrootsNostrTag,
) -> Option<&Vec<RadrootsNostrRelayUrl>> {
    match tag.as_standardized()? {
        RadrootsNostrTagStandard::Relays(urls) => Some(urls),
        _ => None,
    }
}

pub fn radroots_nostr_tags_match<'a>(
    tag: &'a RadrootsNostrTag,
) -> Option<(&'a str, &'a [String])> {
    if let RadrootsNostrTagKind::Custom(Cow::Borrowed(key)) = tag.kind() {
        Some((key, &tag.as_slice()[1..]))
    } else {
        None
    }
}

pub fn radroots_nostr_tag_match_l(tag: &RadrootsNostrTag) -> Option<(&str, f64)> {
    let values = tag.as_slice();
    if values.len() >= 3 && values[0].eq_ignore_ascii_case("l") {
        if let Ok(value) = values[1].parse::<f64>() {
            return Some((values[2].as_str(), value));
        }
    }
    None
}

pub fn radroots_nostr_tag_match_location(
    tag: &RadrootsNostrTag,
) -> Option<(&str, &str, &str)> {
    let values = tag.as_slice();
    if values.len() >= 4 && values[0] == "location" {
        Some((values[1].as_str(), values[2].as_str(), values[3].as_str()))
    } else {
        None
    }
}

pub fn radroots_nostr_tag_match_geohash(tag: &RadrootsNostrTag) -> Option<String> {
    match tag.as_standardized()? {
        RadrootsNostrTagStandard::Geohash(geohash) => Some(geohash.clone()),
        _ => None,
    }
}

pub fn radroots_nostr_tag_match_title(tag: &RadrootsNostrTag) -> Option<String> {
    match tag.as_standardized()? {
        RadrootsNostrTagStandard::Title(title) => Some(title.clone()),
        _ => None,
    }
}

pub fn radroots_nostr_tag_match_summary(tag: &RadrootsNostrTag) -> Option<String> {
    match tag.as_standardized()? {
        RadrootsNostrTagStandard::Summary(summary) => Some(summary.clone()),
        _ => None,
    }
}

pub fn radroots_nostr_tags_resolve(
    event: &RadrootsNostrEvent,
    keys: &RadrootsNostrKeys,
) -> Result<Vec<RadrootsNostrTag>, RadrootsNostrTagsResolveError> {
    if !event
        .tags
        .iter()
        .any(|t| t.kind() == RadrootsNostrTagKind::Encrypted)
    {
        return Ok(event.clone().tags.to_vec());
    }
    let recipient = event
        .tags
        .iter()
        .find_map(|tag| {
            if tag.kind() == RadrootsNostrTagKind::p() {
                tag.content()?.parse::<RadrootsNostrPublicKey>().ok()
            } else {
                None
            }
        })
        .ok_or_else(|| RadrootsNostrTagsResolveError::MissingPTag(event.clone()))?;
    if recipient != keys.public_key() {
        return Err(RadrootsNostrTagsResolveError::NotRecipient);
    }
    let cleartext = nip04::decrypt(keys.secret_key(), &event.pubkey, &event.content)
        .map_err(|e| RadrootsNostrTagsResolveError::DecryptionError(e.to_string()))?;
    let decrypted_tags: nostr::event::tag::list::Tags = serde_json::from_str(&cleartext)?;
    Ok(decrypted_tags.to_vec())
}
