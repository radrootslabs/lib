#[cfg(not(feature = "std"))]
use alloc::{format, string::{String, ToString}, vec::Vec};
#[cfg(not(feature = "std"))]
use alloc::vec;

use radroots_events::kinds::KIND_MESSAGE_FILE;
use radroots_events::message_file::{RadrootsMessageFile, RadrootsMessageFileDimensions};

use crate::error::EventEncodeError;
use crate::message::tags::{build_recipient_tags, build_reply_tag, build_subject_tag};
use crate::wire::WireEventParts;

const DEFAULT_KIND: u32 = KIND_MESSAGE_FILE;

fn validate_required(value: &str, field: &'static str) -> Result<(), EventEncodeError> {
    if value.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField(field));
    }
    Ok(())
}

fn push_required_tag(
    tags: &mut Vec<Vec<String>>,
    key: &'static str,
    value: &str,
    field: &'static str,
) -> Result<(), EventEncodeError> {
    validate_required(value, field)?;
    tags.push(vec![key.to_string(), value.to_string()]);
    Ok(())
}

fn push_optional_tag(tags: &mut Vec<Vec<String>>, key: &'static str, value: &Option<String>) {
    if let Some(value) = value {
        tags.push(vec![key.to_string(), value.clone()]);
    }
}

fn push_dimensions_tag(
    tags: &mut Vec<Vec<String>>,
    dimensions: &Option<RadrootsMessageFileDimensions>,
) {
    if let Some(dimensions) = dimensions {
        tags.push(vec![
            "dim".to_string(),
            format!("{}x{}", dimensions.w, dimensions.h),
        ]);
    }
}

pub fn message_file_build_tags(
    message: &RadrootsMessageFile,
) -> Result<Vec<Vec<String>>, EventEncodeError> {
    let mut tags = build_recipient_tags(&message.recipients)?;
    if let Some(tag) = build_reply_tag(&message.reply_to)? {
        tags.push(tag);
    }
    if let Some(tag) = build_subject_tag(&message.subject)? {
        tags.push(tag);
    }

    push_required_tag(
        &mut tags,
        "file-type",
        &message.file_type,
        "file_type",
    )?;
    push_required_tag(
        &mut tags,
        "encryption-algorithm",
        &message.encryption_algorithm,
        "encryption_algorithm",
    )?;
    push_required_tag(
        &mut tags,
        "decryption-key",
        &message.decryption_key,
        "decryption_key",
    )?;
    push_required_tag(
        &mut tags,
        "decryption-nonce",
        &message.decryption_nonce,
        "decryption_nonce",
    )?;
    push_required_tag(&mut tags, "x", &message.encrypted_hash, "encrypted_hash")?;

    push_optional_tag(&mut tags, "ox", &message.original_hash);
    if let Some(size) = message.size {
        tags.push(vec!["size".to_string(), size.to_string()]);
    }
    push_dimensions_tag(&mut tags, &message.dimensions);
    push_optional_tag(&mut tags, "blurhash", &message.blurhash);
    push_optional_tag(&mut tags, "thumb", &message.thumb);
    for fallback in &message.fallbacks {
        validate_required(fallback, "fallback")?;
        tags.push(vec!["fallback".to_string(), fallback.clone()]);
    }

    Ok(tags)
}

pub fn to_wire_parts(message: &RadrootsMessageFile) -> Result<WireEventParts, EventEncodeError> {
    validate_required(&message.file_url, "file_url")?;
    let tags = message_file_build_tags(message)?;
    Ok(WireEventParts {
        kind: DEFAULT_KIND,
        content: message.file_url.clone(),
        tags,
    })
}

pub fn to_wire_parts_with_kind(
    message: &RadrootsMessageFile,
    kind: u32,
) -> Result<WireEventParts, EventEncodeError> {
    if kind != DEFAULT_KIND {
        return Err(EventEncodeError::InvalidKind(kind));
    }
    to_wire_parts(message)
}
