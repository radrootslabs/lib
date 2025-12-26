#[cfg(not(feature = "std"))]
use alloc::{string::{String, ToString}, vec::Vec};
#[cfg(not(feature = "std"))]
use alloc::vec;

use radroots_events::{RadrootsNostrEventPtr, message::RadrootsMessageRecipient};

use crate::error::{EventEncodeError, EventParseError};

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

pub(crate) fn build_recipient_tags(
    recipients: &[RadrootsMessageRecipient],
) -> Result<Vec<Vec<String>>, EventEncodeError> {
    if recipients.is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("recipients"));
    }

    let mut tags = Vec::with_capacity(recipients.len());
    for recipient in recipients {
        validate_recipient(recipient)?;
        let mut tag = Vec::with_capacity(3);
        tag.push("p".to_string());
        tag.push(recipient.public_key.clone());
        if let Some(relay_url) = &recipient.relay_url {
            tag.push(relay_url.clone());
        }
        tags.push(tag);
    }
    Ok(tags)
}

pub(crate) fn build_reply_tag(
    reply_to: &Option<RadrootsNostrEventPtr>,
) -> Result<Option<Vec<String>>, EventEncodeError> {
    let reply_to = match reply_to {
        Some(reply_to) => reply_to,
        None => return Ok(None),
    };
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
    Ok(Some(tag))
}

pub(crate) fn build_subject_tag(
    subject: &Option<String>,
) -> Result<Option<Vec<String>>, EventEncodeError> {
    let subject = match subject {
        Some(subject) => subject,
        None => return Ok(None),
    };
    if subject.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("subject"));
    }
    Ok(Some(vec!["subject".to_string(), subject.clone()]))
}

fn parse_recipient_tag(tag: &[String]) -> Result<RadrootsMessageRecipient, EventParseError> {
    if tag.get(0).map(|s| s.as_str()) != Some("p") {
        return Err(EventParseError::InvalidTag("p"));
    }
    let public_key = tag.get(1).ok_or(EventParseError::InvalidTag("p"))?;
    if public_key.trim().is_empty() {
        return Err(EventParseError::InvalidTag("p"));
    }
    let relay_url = match tag.get(2) {
        Some(value) if value.trim().is_empty() => return Err(EventParseError::InvalidTag("p")),
        Some(value) => Some(value.clone()),
        None => None,
    };
    Ok(RadrootsMessageRecipient {
        public_key: public_key.clone(),
        relay_url,
    })
}

pub(crate) fn parse_recipients(
    tags: &[Vec<String>],
) -> Result<Vec<RadrootsMessageRecipient>, EventParseError> {
    let mut recipients = Vec::new();
    for tag in tags.iter().filter(|t| t.get(0).map(|s| s.as_str()) == Some("p")) {
        recipients.push(parse_recipient_tag(tag)?);
    }
    if recipients.is_empty() {
        return Err(EventParseError::MissingTag("p"));
    }
    Ok(recipients)
}

pub(crate) fn parse_reply_tag(
    tags: &[Vec<String>],
) -> Result<Option<RadrootsNostrEventPtr>, EventParseError> {
    let tag = match tags
        .iter()
        .find(|t| t.get(0).map(|s| s.as_str()) == Some("e"))
    {
        Some(tag) => tag,
        None => return Ok(None),
    };
    if tag.get(0).map(|s| s.as_str()) != Some("e") {
        return Err(EventParseError::InvalidTag("e"));
    }
    let id = tag.get(1).ok_or(EventParseError::InvalidTag("e"))?;
    if id.trim().is_empty() {
        return Err(EventParseError::InvalidTag("e"));
    }
    let relay = match tag.get(2) {
        Some(value) if value.trim().is_empty() => return Err(EventParseError::InvalidTag("e")),
        Some(value) => Some(value.clone()),
        None => None,
    };
    Ok(Some(RadrootsNostrEventPtr {
        id: id.clone(),
        relays: relay,
    }))
}

pub(crate) fn parse_subject_tag(
    tags: &[Vec<String>],
) -> Result<Option<String>, EventParseError> {
    let tag = match tags
        .iter()
        .find(|t| t.get(0).map(|s| s.as_str()) == Some("subject"))
    {
        Some(tag) => tag,
        None => return Ok(None),
    };
    if tag.get(0).map(|s| s.as_str()) != Some("subject") {
        return Err(EventParseError::InvalidTag("subject"));
    }
    let subject = tag.get(1).ok_or(EventParseError::InvalidTag("subject"))?;
    if subject.trim().is_empty() {
        return Err(EventParseError::InvalidTag("subject"));
    }
    Ok(Some(subject.clone()))
}
