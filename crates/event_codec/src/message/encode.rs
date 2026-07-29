#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use radroots_event::envelope::kind::KIND_MESSAGE;
use radroots_event::social::message::RadrootsMessage;

use crate::error::EventEncodeError;
use crate::message::tags::{build_recipient_tags, build_reply_tag, build_subject_tag};
use radroots_event::wire::RadrootsNip01EventWireParts;

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

pub fn to_wire_parts(
    message: &RadrootsMessage,
) -> Result<RadrootsNip01EventWireParts, EventEncodeError> {
    if message.content.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("content"));
    }
    let tags = message_build_tags(message)?;
    Ok(RadrootsNip01EventWireParts {
        kind: DEFAULT_KIND,
        content: message.content.clone(),
        tags,
    })
}
