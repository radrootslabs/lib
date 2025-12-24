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

pub fn push_nip10_ref_tags(
    tags: &mut Vec<Vec<String>>,
    event: &RadrootsNostrEventRef,
    tag_e: &'static str,
    tag_p: &'static str,
    tag_k: &'static str,
    tag_a: &'static str,
) {
    let relays_len = event.relays.as_ref().map(|r| r.len()).unwrap_or(0);
    let kind_str = event.kind.to_string();

    let mut e_tag = Vec::with_capacity(2 + relays_len);
    e_tag.push(tag_e.to_string());
    e_tag.push(event.id.clone());
    if let Some(relays) = &event.relays {
        e_tag.extend(relays.iter().cloned());
    }
    tags.push(e_tag);

    let mut p_tag = Vec::with_capacity(2);
    p_tag.push(tag_p.to_string());
    p_tag.push(event.author.clone());
    tags.push(p_tag);

    let mut k_tag = Vec::with_capacity(2);
    k_tag.push(tag_k.to_string());
    k_tag.push(kind_str.clone());
    tags.push(k_tag);

    if let Some(d_tag) = event.d_tag.as_deref().filter(|v| !v.is_empty()) {
        let mut addr = String::with_capacity(kind_str.len() + event.author.len() + d_tag.len() + 2);
        addr.push_str(&kind_str);
        addr.push(':');
        addr.push_str(&event.author);
        addr.push(':');
        addr.push_str(d_tag);

        let mut a_tag = Vec::with_capacity(2 + relays_len);
        a_tag.push(tag_a.to_string());
        a_tag.push(addr);
        if let Some(relays) = &event.relays {
            a_tag.extend(relays.iter().cloned());
        }
        tags.push(a_tag);
    }
}

pub fn parse_nip10_ref_tags(
    tags: &[Vec<String>],
    tag_e: &'static str,
    tag_p: &'static str,
    tag_k: &'static str,
    tag_a: &'static str,
) -> Result<RadrootsNostrEventRef, EventParseError> {
    let e_tag = find_event_ref_tag(tags, tag_e).ok_or(EventParseError::MissingTag(tag_e))?;
    let id = e_tag.get(1).ok_or(EventParseError::InvalidTag(tag_e))?;
    if id.trim().is_empty() {
        return Err(EventParseError::InvalidTag(tag_e));
    }
    let relays = if e_tag.len() > 2 {
        Some(e_tag[2..].to_vec())
    } else {
        None
    };

    let p_tag = find_event_ref_tag(tags, tag_p).ok_or(EventParseError::MissingTag(tag_p))?;
    let author = p_tag.get(1).ok_or(EventParseError::InvalidTag(tag_p))?;
    if author.trim().is_empty() {
        return Err(EventParseError::InvalidTag(tag_p));
    }

    let k_tag = find_event_ref_tag(tags, tag_k).ok_or(EventParseError::MissingTag(tag_k))?;
    let kind_key = k_tag.get(1).ok_or(EventParseError::InvalidTag(tag_k))?;
    let kind: u32 = kind_key
        .parse()
        .map_err(|e| EventParseError::InvalidNumber(tag_k, e))?;

    let mut d_tag: Option<String> = None;
    let mut addr_relays: Option<Vec<String>> = None;
    for tag in tags
        .iter()
        .filter(|t| t.get(0).map(|s| s.as_str()) == Some(tag_a))
    {
        let value = match tag.get(1) {
            Some(v) => v,
            None => continue,
        };
        let mut parts = value.splitn(3, ':');
        let kind_part = parts.next();
        let author_part = parts.next();
        let d_part = parts.next();
        if kind_part != Some(kind_key.as_str()) || author_part != Some(author.as_str()) {
            continue;
        }
        if let Some(d) = d_part {
            if !d.is_empty() {
                d_tag = Some(d.to_string());
            }
        }
        if tag.len() > 2 {
            addr_relays = Some(tag[2..].to_vec());
        }
        break;
    }

    let relays = match relays {
        Some(v) if !v.is_empty() => Some(v),
        _ => addr_relays,
    };

    Ok(RadrootsNostrEventRef {
        id: id.clone(),
        author: author.clone(),
        kind,
        d_tag,
        relays,
    })
}
