#[cfg(not(feature = "std"))]
use alloc::{
    string::{String, ToString},
    vec::Vec,
};

use radroots_event::{envelope::kind::KIND_SEAL, social::seal::Seal};

use crate::error::EventParseError;
use crate::parsed::{RadrootsParsedData, RadrootsParsedEvent};

const DEFAULT_KIND: u32 = KIND_SEAL;

pub fn seal_from_parts(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<Seal, EventParseError> {
    if kind != DEFAULT_KIND {
        return Err(EventParseError::InvalidKind {
            expected: "13",
            got: kind,
        });
    }
    if !tags.is_empty() {
        return Err(EventParseError::InvalidTag("tags"));
    }
    if content.trim().is_empty() {
        return Err(EventParseError::InvalidTag("content"));
    }
    Ok(Seal {
        content: content.to_string(),
    })
}

pub fn data_from_event(
    id: String,
    author: String,
    published_at: u64,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
) -> Result<RadrootsParsedData<Seal>, EventParseError> {
    let seal = seal_from_parts(kind, &tags, &content)?;
    Ok(RadrootsParsedData::new(
        id,
        author,
        published_at,
        kind,
        seal,
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
) -> Result<RadrootsParsedEvent<Seal>, EventParseError> {
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
