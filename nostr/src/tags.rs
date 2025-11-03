extern crate alloc;
use alloc::{borrow::Cow, string::String, vec::Vec};

use nostr::{
    event::{Event, Tag, TagKind, TagStandard},
    key::{Keys, PublicKey},
    nips::nip04,
    types::RelayUrl,
};

use crate::error::NostrTagsResolveError;

pub fn nostr_tag_first_value(tag: &Tag, key: &str) -> Option<String> {
    if tag.kind() == TagKind::custom(key) {
        tag.content().map(|v| v.to_string())
    } else {
        None
    }
}

pub fn nostr_tag_at_value(tag: &Tag, index: usize) -> Option<String> {
    tag.as_slice().get(index).cloned()
}

pub fn nostr_tag_slice(tag: &Tag, start: usize) -> Option<Vec<String>> {
    tag.as_slice().get(start..).map(|s| s.to_vec())
}

pub fn nostr_tag_relays_parse(tag: &Tag) -> Option<&Vec<RelayUrl>> {
    match tag.as_standardized()? {
        TagStandard::Relays(urls) => Some(urls),
        _ => None,
    }
}

pub fn nostr_tags_match<'a>(tag: &'a Tag) -> Option<(&'a str, &'a [String])> {
    if let TagKind::Custom(Cow::Borrowed(key)) = tag.kind() {
        Some((key, &tag.as_slice()[1..]))
    } else {
        None
    }
}

pub fn nostr_tag_match_l(tag: &Tag) -> Option<(&str, f64)> {
    let values = tag.as_slice();
    if values.len() >= 3 && values[0].eq_ignore_ascii_case("l") {
        if let Ok(value) = values[1].parse::<f64>() {
            return Some((values[2].as_str(), value));
        }
    }
    None
}

pub fn nostr_tag_match_location(tag: &Tag) -> Option<(&str, &str, &str)> {
    let values = tag.as_slice();
    if values.len() >= 4 && values[0] == "location" {
        Some((values[1].as_str(), values[2].as_str(), values[3].as_str()))
    } else {
        None
    }
}

pub fn nostr_tag_match_geohash(tag: &Tag) -> Option<String> {
    match tag.as_standardized()? {
        TagStandard::Geohash(geohash) => Some(geohash.clone()),
        _ => None,
    }
}

pub fn nostr_tag_match_title(tag: &Tag) -> Option<String> {
    match tag.as_standardized()? {
        TagStandard::Title(title) => Some(title.clone()),
        _ => None,
    }
}

pub fn nostr_tag_match_summary(tag: &Tag) -> Option<String> {
    match tag.as_standardized()? {
        TagStandard::Summary(summary) => Some(summary.clone()),
        _ => None,
    }
}

pub fn nostr_tags_resolve(event: &Event, keys: &Keys) -> Result<Vec<Tag>, NostrTagsResolveError> {
    if !event.tags.iter().any(|t| t.kind() == TagKind::Encrypted) {
        return Ok(event.clone().tags.to_vec());
    }
    let recipient = event
        .tags
        .iter()
        .find_map(|tag| {
            if tag.kind() == TagKind::p() {
                tag.content()?.parse::<PublicKey>().ok()
            } else {
                None
            }
        })
        .ok_or_else(|| NostrTagsResolveError::MissingPTag(event.clone()))?;
    if recipient != keys.public_key() {
        return Err(NostrTagsResolveError::NotRecipient);
    }
    let cleartext = nip04::decrypt(keys.secret_key(), &event.pubkey, &event.content)
        .map_err(|e| NostrTagsResolveError::DecryptionError(e.to_string()))?;
    let decrypted_tags: nostr::event::tag::list::Tags = serde_json::from_str(&cleartext)?;
    Ok(decrypted_tags.to_vec())
}
