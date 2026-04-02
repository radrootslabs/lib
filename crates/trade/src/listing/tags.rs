#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use radroots_events::{
    RadrootsNostrEventPtr,
    tags::{TAG_D, TAG_E_PREV, TAG_E_ROOT},
};
use radroots_events_codec::{
    error::{EventEncodeError, EventParseError},
    job::error::JobParseError,
};

pub const TAG_LISTING_EVENT: &str = "listing_event";

#[inline]
fn push_tag(tags: &mut Vec<Vec<String>>, name: &'static str, value: impl Into<String>) {
    let mut tag = Vec::with_capacity(2);
    tag.push(name.to_string());
    tag.push(value.into());
    tags.push(tag);
}

#[inline]
pub fn trade_listing_dvm_tags<P, A, D>(
    recipient_pubkey: P,
    listing_addr: A,
    order_id: Option<D>,
    listing_event: Option<&RadrootsNostrEventPtr>,
    root_event_id: Option<&str>,
    prev_event_id: Option<&str>,
) -> Result<Vec<Vec<String>>, EventEncodeError>
where
    P: Into<String>,
    A: Into<String>,
    D: Into<String>,
{
    let recipient_pubkey = recipient_pubkey.into();
    if recipient_pubkey.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("recipient_pubkey"));
    }
    let listing_addr = listing_addr.into();
    if listing_addr.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("listing_addr"));
    }
    let mut tags = Vec::with_capacity(
        2 + usize::from(order_id.is_some())
            + usize::from(listing_event.is_some())
            + usize::from(root_event_id.is_some())
            + usize::from(prev_event_id.is_some()),
    );
    push_tag(&mut tags, "p", recipient_pubkey);
    push_tag(&mut tags, "a", listing_addr);
    if let Some(order_id) = order_id {
        let order_id = order_id.into();
        if order_id.trim().is_empty() {
            return Err(EventEncodeError::EmptyRequiredField("order_id"));
        }
        push_tag(&mut tags, TAG_D, order_id);
    }
    if let Some(listing_event) = listing_event {
        if listing_event.id.trim().is_empty() {
            return Err(EventEncodeError::EmptyRequiredField("listing_event.id"));
        }
        let mut tag = vec![TAG_LISTING_EVENT.to_string(), listing_event.id.clone()];
        if let Some(relay) = &listing_event.relays {
            if relay.trim().is_empty() {
                return Err(EventEncodeError::EmptyRequiredField("listing_event.relays"));
            }
            tag.push(relay.clone());
        }
        tags.push(tag);
    }
    if let Some(root_event_id) = root_event_id {
        if root_event_id.trim().is_empty() {
            return Err(EventEncodeError::EmptyRequiredField("root_event_id"));
        }
        push_tag(&mut tags, TAG_E_ROOT, root_event_id);
    }
    if let Some(prev_event_id) = prev_event_id {
        if prev_event_id.trim().is_empty() {
            return Err(EventEncodeError::EmptyRequiredField("prev_event_id"));
        }
        push_tag(&mut tags, TAG_E_PREV, prev_event_id);
    }
    Ok(tags)
}

#[inline]
pub fn parse_trade_listing_counterparty_tag(
    tags: &[Vec<String>],
) -> Result<String, EventParseError> {
    let tag = tags
        .iter()
        .find(|tag| tag.first().map(|value| value.as_str()) == Some("p"))
        .ok_or(EventParseError::MissingTag("p"))?;
    let value = tag.get(1).ok_or(EventParseError::InvalidTag("p"))?;
    if value.trim().is_empty() {
        return Err(EventParseError::InvalidTag("p"));
    }
    Ok(value.clone())
}

#[inline]
pub fn parse_trade_listing_event_tag(
    tags: &[Vec<String>],
) -> Result<Option<RadrootsNostrEventPtr>, EventParseError> {
    let Some(tag) = tags
        .iter()
        .find(|tag| tag.first().map(|value| value.as_str()) == Some(TAG_LISTING_EVENT))
    else {
        return Ok(None);
    };
    let id = tag
        .get(1)
        .ok_or(EventParseError::InvalidTag(TAG_LISTING_EVENT))?;
    if id.trim().is_empty() {
        return Err(EventParseError::InvalidTag(TAG_LISTING_EVENT));
    }
    let relays = match tag.get(2) {
        Some(value) if value.trim().is_empty() => {
            return Err(EventParseError::InvalidTag(TAG_LISTING_EVENT));
        }
        Some(value) => Some(value.clone()),
        None => None,
    };
    Ok(Some(RadrootsNostrEventPtr {
        id: id.clone(),
        relays,
    }))
}

#[inline]
pub fn parse_trade_listing_root_tag(
    tags: &[Vec<String>],
) -> Result<Option<String>, EventParseError> {
    let Some(tag) = tags
        .iter()
        .find(|tag| tag.first().map(|value| value.as_str()) == Some(TAG_E_ROOT))
    else {
        return Ok(None);
    };
    let value = tag.get(1).ok_or(EventParseError::InvalidTag(TAG_E_ROOT))?;
    if value.trim().is_empty() {
        return Err(EventParseError::InvalidTag(TAG_E_ROOT));
    }
    Ok(Some(value.clone()))
}

#[inline]
pub fn parse_trade_listing_prev_tag(
    tags: &[Vec<String>],
) -> Result<Option<String>, EventParseError> {
    let Some(tag) = tags
        .iter()
        .find(|tag| tag.first().map(|value| value.as_str()) == Some(TAG_E_PREV))
    else {
        return Ok(None);
    };
    let value = tag.get(1).ok_or(EventParseError::InvalidTag(TAG_E_PREV))?;
    if value.trim().is_empty() {
        return Err(EventParseError::InvalidTag(TAG_E_PREV));
    }
    Ok(Some(value.clone()))
}

#[inline]
pub fn push_trade_listing_chain_tags(
    tags: &mut Vec<Vec<String>>,
    e_root_id: impl Into<String>,
    e_prev_id: Option<impl Into<String>>,
    trade_id: Option<impl Into<String>>,
) {
    let mut reserve = 1;
    if e_prev_id.is_some() {
        reserve += 1;
    }
    if trade_id.is_some() {
        reserve += 1;
    }
    tags.reserve(reserve);

    push_tag(tags, TAG_E_ROOT, e_root_id);
    if let Some(prev) = e_prev_id {
        push_tag(tags, TAG_E_PREV, prev);
    }
    if let Some(d) = trade_id {
        push_tag(tags, TAG_D, d);
    }
}

#[inline]
pub fn validate_trade_listing_chain(tags: &[Vec<String>]) -> Result<(), JobParseError> {
    let mut has_root = false;
    let mut has_d = false;

    for tag in tags {
        match tag.as_slice() {
            [key, value, ..] if key == TAG_E_ROOT => {
                if value.trim().is_empty() {
                    return Err(JobParseError::InvalidTag(TAG_E_ROOT));
                }
                has_root = true;
            }
            [key] if key == TAG_E_ROOT => {
                return Err(JobParseError::InvalidTag(TAG_E_ROOT));
            }
            [key, value, ..] if key == TAG_D => {
                if value.trim().is_empty() {
                    return Err(JobParseError::InvalidTag(TAG_D));
                }
                has_d = true;
            }
            [key] if key == TAG_D => {
                return Err(JobParseError::InvalidTag(TAG_D));
            }
            _ => {}
        }
    }

    if !has_root {
        Err(JobParseError::MissingChainTag(TAG_E_ROOT))
    } else if !has_d {
        Err(JobParseError::MissingChainTag(TAG_D))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        TAG_LISTING_EVENT, parse_trade_listing_counterparty_tag, parse_trade_listing_event_tag,
        parse_trade_listing_prev_tag, parse_trade_listing_root_tag, push_trade_listing_chain_tags,
        trade_listing_dvm_tags, validate_trade_listing_chain,
    };
    use radroots_events::{
        RadrootsNostrEventPtr,
        kinds::KIND_LISTING,
        tags::{TAG_D, TAG_E_PREV, TAG_E_ROOT},
    };

    #[test]
    fn validate_trade_listing_chain_ok() {
        let tags = vec![
            vec![TAG_E_ROOT.into(), "root".into()],
            vec![TAG_D.into(), "trade".into()],
        ];
        assert!(validate_trade_listing_chain(&tags).is_ok());
    }

    #[test]
    fn validate_trade_listing_chain_rejects_missing_root() {
        let tags = vec![vec![TAG_D.into(), "trade".into()]];
        let err = validate_trade_listing_chain(&tags).unwrap_err();
        assert_eq!(
            err.to_string(),
            format!("missing required chain tag: {TAG_E_ROOT}")
        );
    }

    #[test]
    fn trade_listing_dvm_tags_builds_expected_tags() {
        let listing_addr = format!("{KIND_LISTING}:pubkey:AAAAAAAAAAAAAAAAAAAAAg");
        let tags = trade_listing_dvm_tags(
            "pubkey",
            &listing_addr,
            Some("order-1"),
            Some(&RadrootsNostrEventPtr {
                id: "listing-snapshot".into(),
                relays: None,
            }),
            Some("root"),
            Some("prev"),
        )
        .expect("trade listing tags");
        assert_eq!(tags[0], vec!["p".to_string(), "pubkey".to_string()]);
        assert_eq!(tags[1], vec!["a".to_string(), listing_addr]);
        assert!(tags.iter().any(|tag| tag[0] == TAG_LISTING_EVENT));
    }

    #[test]
    fn trade_listing_tag_parsers_extract_context() {
        let tags = vec![
            vec!["p".into(), "counterparty".into()],
            vec![TAG_LISTING_EVENT.into(), "snapshot".into()],
            vec![TAG_E_ROOT.into(), "root".into()],
            vec![TAG_E_PREV.into(), "prev".into()],
        ];
        assert_eq!(
            parse_trade_listing_counterparty_tag(&tags).expect("counterparty"),
            "counterparty"
        );
        assert_eq!(
            parse_trade_listing_event_tag(&tags).expect("snapshot"),
            Some(RadrootsNostrEventPtr {
                id: "snapshot".into(),
                relays: None,
            })
        );
        assert_eq!(
            parse_trade_listing_root_tag(&tags).expect("root"),
            Some("root".into())
        );
        assert_eq!(
            parse_trade_listing_prev_tag(&tags).expect("prev"),
            Some("prev".into())
        );
    }

    #[test]
    fn push_trade_listing_chain_tags_appends_optional_fields() {
        let mut tags = vec![vec![String::from("x"), String::from("seed")]];
        push_trade_listing_chain_tags(
            &mut tags,
            "root-id",
            Some("prev-id".to_string()),
            Some("trade-id".to_string()),
        );

        assert_eq!(
            tags,
            vec![
                vec![String::from("x"), String::from("seed")],
                vec![String::from(TAG_E_ROOT), String::from("root-id")],
                vec![String::from(TAG_E_PREV), String::from("prev-id")],
                vec![String::from(TAG_D), String::from("trade-id")],
            ]
        );
    }
}
