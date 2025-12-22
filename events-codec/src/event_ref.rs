#[cfg(not(feature = "std"))]
use alloc::{string::{String, ToString}, vec::Vec};

use radroots_events::RadrootsNostrEventRef;

use crate::error::EventParseError;

fn looks_like_relay_url(s: &str) -> bool {
    s.starts_with("ws://") || s.starts_with("wss://")
}

pub fn build_event_ref_tag(tag: &str, event: &RadrootsNostrEventRef) -> Vec<String> {
    let relays_len = event.relays.as_ref().map(|r| r.len()).unwrap_or(0);
    let mut out = Vec::with_capacity(5 + relays_len);
    out.push(tag.to_string());
    out.push(event.id.clone());
    out.push(event.author.clone());
    out.push(event.kind.to_string());
    out.push(event.d_tag.clone().unwrap_or_default());
    if let Some(relays) = &event.relays {
        out.extend(relays.iter().cloned());
    }
    out
}

pub fn parse_event_ref_tag(
    tag: &[String],
    tag_name: &'static str,
) -> Result<RadrootsNostrEventRef, EventParseError> {
    if tag.get(0).map(|s| s.as_str()) != Some(tag_name) {
        return Err(EventParseError::InvalidTag(tag_name));
    }
    let id = tag.get(1).ok_or(EventParseError::InvalidTag(tag_name))?;
    let author = tag.get(2).ok_or(EventParseError::InvalidTag(tag_name))?;
    let kind_s = tag.get(3).ok_or(EventParseError::InvalidTag(tag_name))?;
    let kind: u32 = kind_s
        .parse()
        .map_err(|e| EventParseError::InvalidNumber(tag_name, e))?;

    let (d_tag, relays_start) = match tag.get(4) {
        Some(v) if tag.len() == 5 && looks_like_relay_url(v) => (None, 4),
        Some(v) if v.is_empty() => (None, 5),
        Some(v) => (Some(v.clone()), 5),
        None => (None, 4),
    };

    let relays = if tag.len() > relays_start {
        Some(tag[relays_start..].to_vec())
    } else {
        None
    };

    Ok(RadrootsNostrEventRef {
        id: id.clone(),
        author: author.clone(),
        kind,
        d_tag,
        relays,
    })
}

pub fn find_event_ref_tag<'a>(
    tags: &'a [Vec<String>],
    tag_name: &'static str,
) -> Option<&'a Vec<String>> {
    tags.iter()
        .find(|t| t.get(0).map(|s| s.as_str()) == Some(tag_name))
}
