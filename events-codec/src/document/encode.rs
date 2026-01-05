#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
use alloc::{string::{String, ToString}, vec::Vec};

use radroots_events::{
    document::RadrootsDocument,
    tags::TAG_D,
};

use crate::d_tag::validate_d_tag;
use crate::error::EventEncodeError;

#[cfg(feature = "serde_json")]
use radroots_events::kinds::KIND_DOCUMENT;

#[cfg(feature = "serde_json")]
use crate::wire::WireEventParts;

const TAG_T: &str = "t";
const TAG_P: &str = "p";
const TAG_A: &str = "a";

fn push_tag(tags: &mut Vec<Vec<String>>, key: &str, value: &str) {
    let mut tag = Vec::with_capacity(2);
    tag.push(key.to_string());
    tag.push(value.to_string());
    tags.push(tag);
}

pub fn document_build_tags(document: &RadrootsDocument) -> Result<Vec<Vec<String>>, EventEncodeError> {
    if document.d_tag.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("d_tag"));
    }
    validate_d_tag(&document.d_tag, "d_tag")?;
    if document.doc_type.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("doc_type"));
    }
    if document.title.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("title"));
    }
    if document.version.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("version"));
    }
    if document.subject.pubkey.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("subject.pubkey"));
    }
    let mut tags = Vec::new();
    push_tag(&mut tags, TAG_D, &document.d_tag);
    push_tag(&mut tags, TAG_P, &document.subject.pubkey);
    if let Some(address) = document.subject.address.as_ref() {
        let address = address.trim();
        if address.is_empty() {
            return Err(EventEncodeError::EmptyRequiredField("subject.address"));
        }
        push_tag(&mut tags, TAG_A, address);
    }
    if let Some(items) = document.tags.as_ref() {
        for item in items.iter().filter(|v| !v.trim().is_empty()) {
            push_tag(&mut tags, TAG_T, item);
        }
    }
    Ok(tags)
}

#[cfg(feature = "serde_json")]
pub fn to_wire_parts(document: &RadrootsDocument) -> Result<WireEventParts, EventEncodeError> {
    to_wire_parts_with_kind(document, KIND_DOCUMENT)
}

#[cfg(feature = "serde_json")]
pub fn to_wire_parts_with_kind(
    document: &RadrootsDocument,
    kind: u32,
) -> Result<WireEventParts, EventEncodeError> {
    if kind != KIND_DOCUMENT {
        return Err(EventEncodeError::InvalidKind(kind));
    }
    let tags = document_build_tags(document)?;
    let content = serde_json::to_string(document).map_err(|_| EventEncodeError::Json)?;
    Ok(WireEventParts { kind, content, tags })
}
