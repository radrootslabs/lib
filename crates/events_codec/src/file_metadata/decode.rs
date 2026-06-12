#[cfg(not(feature = "std"))]
use alloc::{string::ToString, vec::Vec};

use radroots_events::{
    RadrootsNostrEvent,
    file_metadata::RadrootsFileMetadata,
    kinds::KIND_PUBLIC_FILE_METADATA,
    social::RadrootsSocialMediaThumbnail,
    tags::{
        TAG_ALT, TAG_BLURHASH, TAG_DIMENSIONS, TAG_FALLBACK, TAG_MAGNET, TAG_MIME,
        TAG_ORIGINAL_SHA256, TAG_SERVICE, TAG_SHA256, TAG_SIZE, TAG_SUMMARY, TAG_THUMB, TAG_URL,
    },
};

use crate::error::EventParseError;
use crate::field_helpers::{
    optional_tag_value, required_tag_value, tag_values, validate_lowercase_hex_64_tag,
};
use crate::parsed::{RadrootsParsedData, RadrootsParsedEvent};
use crate::social_helpers::{first_tag_value, parse_dimensions_tag};

const EXPECTED_KIND: &str = "1063";
const TAG_RADROOTS_OWNER_DOCUMENT: &str = "radroots:owner_document";

pub fn file_metadata_from_event(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<RadrootsFileMetadata, EventParseError> {
    if kind != KIND_PUBLIC_FILE_METADATA {
        return Err(EventParseError::InvalidKind {
            expected: EXPECTED_KIND,
            got: kind,
        });
    }
    reject_private_farm_file_tags(tags)?;
    let url = required_tag_value(tags, TAG_URL)?;
    let mime_type = required_tag_value(tags, TAG_MIME)?;
    let sha256 = required_tag_value(tags, TAG_SHA256)?;
    validate_lowercase_hex_64_tag(&sha256, TAG_SHA256)?;
    let original_sha256 = optional_hash_tag(tags, TAG_ORIGINAL_SHA256)?;
    let size = optional_tag_value(tags, TAG_SIZE)?
        .map(|value| {
            value
                .parse::<u64>()
                .map_err(|err| EventParseError::InvalidNumber(TAG_SIZE, err))
        })
        .transpose()?;
    let dimensions = optional_tag_value(tags, TAG_DIMENSIONS)?
        .map(|value| parse_dimensions_tag(&value, TAG_DIMENSIONS))
        .transpose()?;

    Ok(RadrootsFileMetadata {
        url,
        mime_type,
        sha256,
        original_sha256,
        size,
        dimensions,
        blurhash: first_tag_value(tags, TAG_BLURHASH),
        thumbnails: parse_thumbnails(tags)?,
        summary: first_tag_value(tags, TAG_SUMMARY),
        alt: first_tag_value(tags, TAG_ALT),
        fallback: first_tag_value(tags, TAG_FALLBACK),
        magnet: first_tag_value(tags, TAG_MAGNET),
        content_hashes: non_empty_vec(tag_values(tags, "i")?),
        services: non_empty_vec(tag_values(tags, TAG_SERVICE)?),
        content: if content.is_empty() {
            None
        } else {
            Some(content.to_string())
        },
    })
}

fn reject_private_farm_file_tags(tags: &[Vec<String>]) -> Result<(), EventParseError> {
    if tags
        .iter()
        .any(|tag| tag.first().map(|value| value.as_str()) == Some(TAG_RADROOTS_OWNER_DOCUMENT))
    {
        Err(EventParseError::InvalidTag(TAG_RADROOTS_OWNER_DOCUMENT))
    } else {
        Ok(())
    }
}

pub fn data_from_event(
    id: String,
    author: String,
    published_at: u32,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
) -> Result<RadrootsParsedData<RadrootsFileMetadata>, EventParseError> {
    let metadata = file_metadata_from_event(kind, &tags, &content)?;
    Ok(RadrootsParsedData::new(
        id,
        author,
        published_at,
        kind,
        metadata,
    ))
}

pub fn parsed_from_event(
    id: String,
    author: String,
    published_at: u32,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
    sig: String,
) -> Result<RadrootsParsedEvent<RadrootsFileMetadata>, EventParseError> {
    let data = data_from_event(
        id.clone(),
        author.clone(),
        published_at,
        kind,
        content.clone(),
        tags.clone(),
    )?;
    Ok(RadrootsParsedEvent {
        event: RadrootsNostrEvent {
            id,
            author,
            created_at: published_at,
            kind,
            content,
            tags,
            sig,
        },
        data,
    })
}

fn optional_hash_tag(
    tags: &[Vec<String>],
    key: &'static str,
) -> Result<Option<String>, EventParseError> {
    let Some(value) = optional_tag_value(tags, key)? else {
        return Ok(None);
    };
    validate_lowercase_hex_64_tag(&value, key)?;
    Ok(Some(value))
}

fn parse_thumbnails(
    tags: &[Vec<String>],
) -> Result<Option<Vec<RadrootsSocialMediaThumbnail>>, EventParseError> {
    let thumbnails = tags
        .iter()
        .filter(|tag| tag.first().map(|value| value.as_str()) == Some(TAG_THUMB))
        .map(|tag| {
            let url = tag
                .get(1)
                .cloned()
                .ok_or(EventParseError::InvalidTag(TAG_THUMB))?;
            let dimensions = tag
                .get(2)
                .filter(|value| !value.trim().is_empty())
                .map(|value| parse_dimensions_tag(value, TAG_THUMB))
                .transpose()?;
            Ok(RadrootsSocialMediaThumbnail { url, dimensions })
        })
        .collect::<Result<Vec<_>, EventParseError>>()?;
    Ok(non_empty_vec(thumbnails))
}

fn non_empty_vec<T>(values: Vec<T>) -> Option<Vec<T>> {
    if values.is_empty() {
        None
    } else {
        Some(values)
    }
}
