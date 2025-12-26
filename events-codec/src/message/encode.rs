#[cfg(not(feature = "std"))]
use alloc::{string::{String, ToString}, vec::Vec};

use radroots_events::message::{RadrootsMessage, RadrootsMessageRecipient};

use crate::error::EventEncodeError;
use crate::wire::WireEventParts;

const DEFAULT_KIND: u32 = 14;

fn validate_recipient(recipient: &RadrootsMessageRecipient) -> Result<(), EventEncodeError> {
    if recipient.public_key.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("recipients.public_key"));
    }
    if let Some(relay_url) = &recipient.relay_url {
        if relay_url.trim().is_empty() {
            return Err(EventEncodeError::EmptyRequiredField("recipients.relay_url"));
        }
    }
    Ok(())
}

pub fn message_build_tags(message: &RadrootsMessage) -> Result<Vec<Vec<String>>, EventEncodeError> {
    if message.recipients.is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("recipients"));
    }

    let mut tags = Vec::with_capacity(
        message.recipients.len()
            + usize::from(message.reply_to.is_some())
            + usize::from(message.subject.is_some()),
    );

    for recipient in &message.recipients {
        validate_recipient(recipient)?;
        let mut tag = Vec::with_capacity(3);
        tag.push("p".to_string());
        tag.push(recipient.public_key.clone());
        if let Some(relay_url) = &recipient.relay_url {
            tag.push(relay_url.clone());
        }
        tags.push(tag);
    }

    if let Some(reply_to) = &message.reply_to {
        if reply_to.id.trim().is_empty() {
            return Err(EventEncodeError::EmptyRequiredField("reply_to.id"));
        }
        let mut tag = Vec::with_capacity(3);
        tag.push("e".to_string());
        tag.push(reply_to.id.clone());
        if let Some(relay) = &reply_to.relays {
            if relay.trim().is_empty() {
                return Err(EventEncodeError::EmptyRequiredField("reply_to.relays"));
            }
            tag.push(relay.clone());
        }
        tags.push(tag);
    }

    if let Some(subject) = &message.subject {
        if subject.trim().is_empty() {
            return Err(EventEncodeError::EmptyRequiredField("subject"));
        }
        let mut tag = Vec::with_capacity(2);
        tag.push("subject".to_string());
        tag.push(subject.clone());
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
