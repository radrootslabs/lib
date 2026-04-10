#[cfg(not(feature = "std"))]
use alloc::{borrow::ToOwned, string::String, vec::Vec};

use radroots_events::{
    RadrootsNostrEventPtr,
    tags::{TAG_D, TAG_E_PREV, TAG_E_ROOT},
};

use crate::{
    error::{EventEncodeError, EventParseError},
    job::error::JobParseError,
};

pub const TAG_LISTING_EVENT: &str = "listing_event";

#[inline]
fn push_tag(tags: &mut Vec<Vec<String>>, name: &'static str, value: impl Into<String>) {
    let mut tag = Vec::with_capacity(2);
    tag.push(name.to_owned());
    tag.push(value.into());
    tags.push(tag);
}

fn build_event_ptr_tag(
    name: &'static str,
    ptr: &RadrootsNostrEventPtr,
    field_prefix: &'static str,
) -> Result<Vec<String>, EventEncodeError> {
    if ptr.id.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField(field_prefix));
    }
    let mut tag = Vec::with_capacity(3);
    tag.push(name.to_owned());
    tag.push(ptr.id.clone());
    if let Some(relay) = &ptr.relays {
        if relay.trim().is_empty() {
            return Err(EventEncodeError::EmptyRequiredField("listing_event.relays"));
        }
        tag.push(relay.clone());
    }
    Ok(tag)
}

fn parse_event_ptr_tag(
    tags: &[Vec<String>],
    name: &'static str,
) -> Result<Option<RadrootsNostrEventPtr>, EventParseError> {
    let Some(tag) = tags
        .iter()
        .find(|tag| tag.first().map(|value| value.as_str()) == Some(name))
    else {
        return Ok(None);
    };
    let id = tag.get(1).ok_or(EventParseError::InvalidTag(name))?;
    if id.trim().is_empty() {
        return Err(EventParseError::InvalidTag(name));
    }
    let relay = match tag.get(2) {
        Some(value) if value.trim().is_empty() => return Err(EventParseError::InvalidTag(name)),
        Some(value) => Some(value.clone()),
        None => None,
    };
    Ok(Some(RadrootsNostrEventPtr {
        id: id.clone(),
        relays: relay,
    }))
}

#[inline]
pub fn trade_envelope_tags<P, A, D>(
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

    let mut capacity = 2 + usize::from(order_id.is_some()) + usize::from(listing_event.is_some());
    capacity += usize::from(root_event_id.is_some()) + usize::from(prev_event_id.is_some());
    let mut tags = Vec::with_capacity(capacity);
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
        tags.push(build_event_ptr_tag(
            TAG_LISTING_EVENT,
            listing_event,
            "listing_event.id",
        )?);
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
pub fn parse_trade_counterparty_tag(tags: &[Vec<String>]) -> Result<String, EventParseError> {
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
    parse_event_ptr_tag(tags, TAG_LISTING_EVENT)
}

#[inline]
pub fn parse_trade_root_tag(tags: &[Vec<String>]) -> Result<Option<String>, EventParseError> {
    let tag = match tags
        .iter()
        .find(|tag| tag.first().map(|value| value.as_str()) == Some(TAG_E_ROOT))
    {
        Some(tag) => tag,
        None => return Ok(None),
    };
    let value = tag.get(1).ok_or(EventParseError::InvalidTag(TAG_E_ROOT))?;
    if value.trim().is_empty() {
        return Err(EventParseError::InvalidTag(TAG_E_ROOT));
    }
    Ok(Some(value.clone()))
}

#[inline]
pub fn parse_trade_prev_tag(tags: &[Vec<String>]) -> Result<Option<String>, EventParseError> {
    let tag = match tags
        .iter()
        .find(|tag| tag.first().map(|value| value.as_str()) == Some(TAG_E_PREV))
    {
        Some(tag) => tag,
        None => return Ok(None),
    };
    let value = tag.get(1).ok_or(EventParseError::InvalidTag(TAG_E_PREV))?;
    if value.trim().is_empty() {
        return Err(EventParseError::InvalidTag(TAG_E_PREV));
    }
    Ok(Some(value.clone()))
}

#[inline]
pub fn push_trade_chain_tags(
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
pub fn validate_trade_chain(tags: &[Vec<String>]) -> Result<(), JobParseError> {
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
            [key] if key == TAG_E_ROOT => return Err(JobParseError::InvalidTag(TAG_E_ROOT)),
            [key, value, ..] if key == TAG_D => {
                if value.trim().is_empty() {
                    return Err(JobParseError::InvalidTag(TAG_D));
                }
                has_d = true;
            }
            [key] if key == TAG_D => return Err(JobParseError::InvalidTag(TAG_D)),
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
        TAG_LISTING_EVENT, parse_trade_counterparty_tag, parse_trade_listing_event_tag,
        parse_trade_prev_tag, parse_trade_root_tag, push_trade_chain_tags, trade_envelope_tags,
        validate_trade_chain,
    };
    use crate::error::EventEncodeError;
    use radroots_events::{
        RadrootsNostrEventPtr,
        kinds::KIND_LISTING,
        tags::{TAG_D, TAG_E_PREV, TAG_E_ROOT},
    };

    #[test]
    fn trade_envelope_tags_build_expected_tags() {
        let listing_addr = format!("{KIND_LISTING}:pubkey:AAAAAAAAAAAAAAAAAAAAAg");
        let tags = trade_envelope_tags("pubkey", &listing_addr, Some("order-1"), None, None, None)
            .expect("trade tags");
        let expected: Vec<Vec<String>> = vec![
            vec![String::from("p"), String::from("pubkey")],
            vec![String::from("a"), listing_addr],
            vec![String::from(TAG_D), String::from("order-1")],
        ];
        assert_eq!(tags, expected);
    }

    #[test]
    fn trade_envelope_tags_include_snapshot_and_chain_refs() {
        let listing_addr = format!("{KIND_LISTING}:pubkey:AAAAAAAAAAAAAAAAAAAAAg");
        let tags = trade_envelope_tags(
            "buyer",
            &listing_addr,
            Some("order-1"),
            Some(&RadrootsNostrEventPtr {
                id: "listing-snapshot".into(),
                relays: Some("wss://relay.example".into()),
            }),
            Some("root-event"),
            Some("prev-event"),
        )
        .expect("trade tags");
        assert!(tags.iter().any(|tag| {
            tag.as_slice()
                == [
                    TAG_LISTING_EVENT.to_string(),
                    "listing-snapshot".to_string(),
                    "wss://relay.example".to_string(),
                ]
        }));
        assert!(
            tags.iter().any(|tag| {
                tag.as_slice() == [TAG_E_ROOT.to_string(), "root-event".to_string()]
            })
        );
        assert!(
            tags.iter().any(|tag| {
                tag.as_slice() == [TAG_E_PREV.to_string(), "prev-event".to_string()]
            })
        );
    }

    #[test]
    fn trade_envelope_tags_support_snapshot_without_relay() {
        let listing_addr = format!("{KIND_LISTING}:pubkey:AAAAAAAAAAAAAAAAAAAAAg");
        let tags = trade_envelope_tags(
            "buyer",
            &listing_addr,
            None::<&str>,
            Some(&RadrootsNostrEventPtr {
                id: "listing-snapshot".into(),
                relays: None,
            }),
            Some("root-event"),
            None::<&str>,
        )
        .expect("trade tags");
        assert_eq!(
            tags,
            vec![
                vec![String::from("p"), String::from("buyer")],
                vec![String::from("a"), listing_addr],
                vec![
                    String::from(TAG_LISTING_EVENT),
                    String::from("listing-snapshot"),
                ],
                vec![String::from(TAG_E_ROOT), String::from("root-event")],
            ]
        );
    }

    #[test]
    fn trade_envelope_tags_reject_empty_required_fields() {
        let listing_addr = format!("{KIND_LISTING}:pubkey:AAAAAAAAAAAAAAAAAAAAAg");

        let err = trade_envelope_tags(" ", &listing_addr, None::<&str>, None, None, None)
            .expect_err("blank recipient");
        assert!(matches!(
            err,
            EventEncodeError::EmptyRequiredField("recipient_pubkey")
        ));

        let err = trade_envelope_tags("buyer", " ", None::<&str>, None, None, None)
            .expect_err("blank listing address");
        assert!(matches!(
            err,
            EventEncodeError::EmptyRequiredField("listing_addr")
        ));

        let err = trade_envelope_tags("buyer", &listing_addr, Some(" "), None, None, None)
            .expect_err("blank order id");
        assert!(matches!(
            err,
            EventEncodeError::EmptyRequiredField("order_id")
        ));

        let err = trade_envelope_tags(
            "buyer",
            &listing_addr,
            None::<&str>,
            Some(&RadrootsNostrEventPtr {
                id: " ".into(),
                relays: None,
            }),
            None,
            None,
        )
        .expect_err("blank listing snapshot id");
        assert!(matches!(
            err,
            EventEncodeError::EmptyRequiredField("listing_event.id")
        ));

        let err = trade_envelope_tags(
            "buyer",
            &listing_addr,
            None::<&str>,
            Some(&RadrootsNostrEventPtr {
                id: "listing-snapshot".into(),
                relays: Some(" ".into()),
            }),
            None,
            None,
        )
        .expect_err("blank listing snapshot relay");
        assert!(matches!(
            err,
            EventEncodeError::EmptyRequiredField("listing_event.relays")
        ));

        let err = trade_envelope_tags("buyer", &listing_addr, None::<&str>, None, Some(" "), None)
            .expect_err("blank root event id");
        assert!(matches!(
            err,
            EventEncodeError::EmptyRequiredField("root_event_id")
        ));

        let err = trade_envelope_tags("buyer", &listing_addr, None::<&str>, None, None, Some(" "))
            .expect_err("blank prev event id");
        assert!(matches!(
            err,
            EventEncodeError::EmptyRequiredField("prev_event_id")
        ));
    }

    #[test]
    fn trade_envelope_tag_parsers_cover_public_context() {
        let tags = vec![
            vec!["p".into(), "counterparty".into()],
            vec![
                TAG_LISTING_EVENT.into(),
                "snapshot".into(),
                "wss://relay".into(),
            ],
            vec![TAG_E_ROOT.into(), "root".into()],
            vec![TAG_E_PREV.into(), "prev".into()],
        ];
        assert_eq!(
            parse_trade_counterparty_tag(&tags).expect("counterparty"),
            "counterparty"
        );
        assert_eq!(
            parse_trade_listing_event_tag(&tags).expect("snapshot"),
            Some(RadrootsNostrEventPtr {
                id: "snapshot".into(),
                relays: Some("wss://relay".into()),
            })
        );
        assert_eq!(
            parse_trade_root_tag(&tags).expect("root"),
            Some("root".into())
        );
        assert_eq!(
            parse_trade_prev_tag(&tags).expect("prev"),
            Some("prev".into())
        );
    }

    #[test]
    fn push_trade_chain_tags_adds_root_prev_and_trade_id() {
        let mut tags = Vec::new();
        push_trade_chain_tags(&mut tags, "root", Some("prev"), Some("trade"));
        assert_eq!(
            tags,
            vec![
                vec![String::from(TAG_E_ROOT), String::from("root")],
                vec![String::from(TAG_E_PREV), String::from("prev")],
                vec![String::from(TAG_D), String::from("trade")],
            ]
        );
    }

    #[test]
    fn validate_trade_chain_requires_root_and_trade_id() {
        let ok = vec![
            vec![String::from(TAG_E_ROOT), String::from("root")],
            vec![String::from(TAG_D), String::from("trade")],
        ];
        assert!(validate_trade_chain(&ok).is_ok());
        let missing = vec![vec![String::from(TAG_D), String::from("trade")]];
        assert!(validate_trade_chain(&missing).is_err());
    }
}
