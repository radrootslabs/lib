#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use radroots_events::message::RadrootsMessage;
use radroots_events::kinds::KIND_MESSAGE;

use crate::error::EventEncodeError;
use crate::message::tags::{build_recipient_tags, build_reply_tag, build_subject_tag};
use crate::wire::WireEventParts;

const DEFAULT_KIND: u32 = KIND_MESSAGE;

pub fn message_build_tags(message: &RadrootsMessage) -> Result<Vec<Vec<String>>, EventEncodeError> {
    let mut tags = build_recipient_tags(&message.recipients)?;
    if let Some(tag) = build_reply_tag(&message.reply_to)? {
        tags.push(tag);
    }
    if let Some(tag) = build_subject_tag(&message.subject)? {
        tags.push(tag);
    }
    Ok(tags)
}

pub fn to_wire_parts(message: &RadrootsMessage) -> Result<WireEventParts, EventEncodeError> {
    if message.content.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("content"));
    }
    let tags = message_build_tags(message)?;
    Ok(WireEventParts {
        kind: DEFAULT_KIND,
        content: message.content.clone(),
        tags,
    })
}
