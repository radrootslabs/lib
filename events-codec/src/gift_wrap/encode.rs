#[cfg(not(feature = "std"))]
use alloc::{string::{String, ToString}, vec::Vec};
#[cfg(not(feature = "std"))]
use alloc::vec;

use radroots_events::gift_wrap::{RadrootsGiftWrap, RadrootsGiftWrapRecipient};
use radroots_events::kinds::KIND_GIFT_WRAP;

use crate::error::EventEncodeError;
use crate::wire::WireEventParts;

const DEFAULT_KIND: u32 = KIND_GIFT_WRAP;

fn validate_recipient(
    recipient: &RadrootsGiftWrapRecipient,
) -> Result<Vec<String>, EventEncodeError> {
    if recipient.public_key.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("recipient.public_key"));
    }
    let mut tag = Vec::with_capacity(3);
    tag.push("p".to_string());
    tag.push(recipient.public_key.clone());
    if let Some(relay_url) = &recipient.relay_url {
        if relay_url.trim().is_empty() {
            return Err(EventEncodeError::EmptyRequiredField("recipient.relay_url"));
        }
        tag.push(relay_url.clone());
    }
    Ok(tag)
}

pub fn gift_wrap_build_tags(
    gift_wrap: &RadrootsGiftWrap,
) -> Result<Vec<Vec<String>>, EventEncodeError> {
    let mut tags = Vec::with_capacity(2);
    tags.push(validate_recipient(&gift_wrap.recipient)?);
    if let Some(expiration) = gift_wrap.expiration {
        tags.push(vec!["expiration".to_string(), expiration.to_string()]);
    }
    Ok(tags)
}

pub fn to_wire_parts(gift_wrap: &RadrootsGiftWrap) -> Result<WireEventParts, EventEncodeError> {
    if gift_wrap.content.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("content"));
    }
    let tags = gift_wrap_build_tags(gift_wrap)?;
    Ok(WireEventParts {
        kind: DEFAULT_KIND,
        content: gift_wrap.content.clone(),
        tags,
    })
}

pub fn to_wire_parts_with_kind(
    gift_wrap: &RadrootsGiftWrap,
    kind: u32,
) -> Result<WireEventParts, EventEncodeError> {
    if kind != DEFAULT_KIND {
        return Err(EventEncodeError::InvalidKind(kind));
    }
    to_wire_parts(gift_wrap)
}
