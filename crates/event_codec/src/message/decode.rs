#[cfg(not(feature = "std"))]
use alloc::{
    string::{String, ToString},
    vec::Vec,
};

use radroots_event::{envelope::kind::KIND_MESSAGE, social::message::Message};

use crate::error::EventParseError;
use crate::message::tags::{parse_recipients, parse_reply_tag, parse_subject_tag};
use crate::parsed::{RadrootsParsedData, RadrootsParsedEvent};

const DEFAULT_KIND: u32 = KIND_MESSAGE;

pub fn message_from_tags(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<Message, EventParseError> {
    if kind != DEFAULT_KIND {
        return Err(EventParseError::InvalidKind {
            expected: "14",
            got: kind,
        });
    }
    if content.trim().is_empty() {
        return Err(EventParseError::InvalidTag("content"));
    }

    let recipients = parse_recipients(tags)?;

    let reply_to = parse_reply_tag(tags)?;

    let subject = parse_subject_tag(tags)?;

    Ok(Message {
        recipients,
        content: content.to_string(),
        reply_to,
        subject,
    })
}

pub fn data_from_event(
    id: String,
    author: String,
    published_at: u64,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
) -> Result<RadrootsParsedData<Message>, EventParseError> {
    let message = message_from_tags(kind, &tags, &content)?;
    Ok(RadrootsParsedData::new(
        id,
        author,
        published_at,
        kind,
        message,
    ))
}

pub fn parsed_from_event(
    id: String,
    author: String,
    published_at: u64,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
    sig: String,
) -> Result<RadrootsParsedEvent<Message>, EventParseError> {
    let data = data_from_event(
        id.clone(),
        author.clone(),
        published_at,
        kind,
        content.clone(),
        tags.clone(),
    )?;
    RadrootsParsedEvent::from_event_parts(id, author, published_at, kind, content, tags, sig, data)
}
