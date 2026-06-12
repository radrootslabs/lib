#[cfg(not(feature = "std"))]
use alloc::{
    string::{String, ToString},
    vec::Vec,
};

use radroots_events::{
    RadrootsNostrEvent,
    farm_crdt::RadrootsFarmCrdtDocumentKind,
    farm_file::{
        KIND_FARM_FILE_METADATA, RadrootsFarmFileDimensions, RadrootsFarmFileMetadata,
        RadrootsFarmFileSource,
    },
    farm_workspace::KIND_FARM_WORKSPACE_MANIFEST,
    tags::{TAG_A, TAG_D, TAG_H, TAG_MIME, TAG_ORIGINAL_SHA256, TAG_SHA256, TAG_URL},
};

use crate::d_tag::validate_d_tag_tag;
use crate::error::EventParseError;
use crate::farm_file::encode::validate_metadata;
use crate::field_helpers::{
    optional_tag_value, parse_address_tag_with_kind, required_tag_value, tag_values,
    validate_lowercase_hex_64_tag, validate_non_empty_tag_value,
};
use crate::parsed::{RadrootsParsedData, RadrootsParsedEvent};

const EXPECTED_KIND: &str = "1063";
const TAG_ALT: &str = "alt";
const TAG_BLURHASH: &str = "blurhash";
const TAG_DIMENSIONS: &str = "dim";
const TAG_FALLBACK: &str = "fallback";
const TAG_IMAGE: &str = "image";
const TAG_OWNER_DOCUMENT: &str = "radroots:owner_document";
const TAG_SIZE: &str = "size";
const TAG_THUMB: &str = "thumb";

pub fn farm_file_metadata_from_event(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<RadrootsFarmFileMetadata, EventParseError> {
    if kind != KIND_FARM_FILE_METADATA {
        return Err(EventParseError::InvalidKind {
            expected: EXPECTED_KIND,
            got: kind,
        });
    }
    let d_tag = required_single_tag_value(tags, TAG_D)?;
    validate_d_tag_tag(&d_tag, TAG_D)?;
    let farm_group_id = required_tag_value(tags, TAG_H)?;
    let workspace_address = required_tag_value(tags, TAG_A)?;
    let workspace =
        parse_address_tag_with_kind(&workspace_address, KIND_FARM_WORKSPACE_MANIFEST, TAG_A)?;
    let url = required_tag_value(tags, TAG_URL)?;
    let mime_type = required_tag_value(tags, TAG_MIME)?;
    let sha256 = required_tag_value(tags, TAG_SHA256)?;
    validate_lowercase_hex_64_tag(&sha256, TAG_SHA256)?;
    let original_sha256 = optional_hash_tag(tags, TAG_ORIGINAL_SHA256)?;
    let (owner_document_id, owner_document_kind) = parse_owner_document(tags)?;
    let size_bytes = parse_size(tags)?;
    let dimensions = parse_dimensions_tag(tags)?;
    let blurhash = optional_tag_value(tags, TAG_BLURHASH)?;
    let thumb = parse_source_tag(tags, TAG_THUMB)?;
    let image = parse_source_tag(tags, TAG_IMAGE)?;
    let alt = optional_tag_value(tags, TAG_ALT)?;
    let fallbacks = tag_values(tags, TAG_FALLBACK)?;
    let caption = if content.is_empty() {
        None
    } else {
        Some(content.to_string())
    };

    let metadata = RadrootsFarmFileMetadata {
        d_tag,
        workspace: radroots_events::farm_workspace::RadrootsFarmWorkspaceRef {
            pubkey: workspace.pubkey,
            d_tag: workspace.d_tag,
        },
        farm_group_id,
        owner_document_id,
        owner_document_kind,
        caption,
        url,
        mime_type,
        sha256,
        original_sha256,
        size_bytes,
        dimensions,
        blurhash,
        thumb,
        image,
        alt,
        fallbacks,
    };
    validate_metadata(&metadata).map_err(encode_error_to_parse_error)?;
    Ok(metadata)
}

pub fn data_from_event(
    id: String,
    author: String,
    published_at: u32,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
) -> Result<RadrootsParsedData<RadrootsFarmFileMetadata>, EventParseError> {
    let metadata = farm_file_metadata_from_event(kind, &tags, &content)?;
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
) -> Result<RadrootsParsedEvent<RadrootsFarmFileMetadata>, EventParseError> {
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

fn required_single_tag_value(
    tags: &[Vec<String>],
    key: &'static str,
) -> Result<String, EventParseError> {
    let values = tag_values(tags, key)?;
    let Some(first) = values.first() else {
        return Err(EventParseError::MissingTag(key));
    };
    if values.iter().any(|value| value != first) {
        return Err(EventParseError::InvalidTag(key));
    }
    Ok(first.clone())
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

fn parse_owner_document(
    tags: &[Vec<String>],
) -> Result<(String, RadrootsFarmCrdtDocumentKind), EventParseError> {
    let tag = tags
        .iter()
        .find(|tag| tag.first().map(|value| value.as_str()) == Some(TAG_OWNER_DOCUMENT))
        .ok_or(EventParseError::MissingTag(TAG_OWNER_DOCUMENT))?;
    if tag.len() != 3 {
        return Err(EventParseError::InvalidTag(TAG_OWNER_DOCUMENT));
    }
    let document_id = tag[1].clone();
    validate_d_tag_tag(&document_id, TAG_OWNER_DOCUMENT)?;
    let kind = parse_document_kind_tag(&tag[2])?;
    Ok((document_id, kind))
}

fn parse_document_kind_tag(value: &str) -> Result<RadrootsFarmCrdtDocumentKind, EventParseError> {
    match value {
        "FarmTask" => Ok(RadrootsFarmCrdtDocumentKind::FarmTask),
        "FarmWorkSession" => Ok(RadrootsFarmCrdtDocumentKind::FarmWorkSession),
        "FarmHarvestRecord" => Ok(RadrootsFarmCrdtDocumentKind::FarmHarvestRecord),
        "FarmInventoryItem" => Ok(RadrootsFarmCrdtDocumentKind::FarmInventoryItem),
        "FarmMediaAsset" => Ok(RadrootsFarmCrdtDocumentKind::FarmMediaAsset),
        "FarmObservation" => Ok(RadrootsFarmCrdtDocumentKind::FarmObservation),
        _ => Err(EventParseError::InvalidTag(TAG_OWNER_DOCUMENT)),
    }
}

fn parse_size(tags: &[Vec<String>]) -> Result<Option<u64>, EventParseError> {
    let Some(value) = optional_tag_value(tags, TAG_SIZE)? else {
        return Ok(None);
    };
    value
        .parse::<u64>()
        .map(Some)
        .map_err(|err| EventParseError::InvalidNumber(TAG_SIZE, err))
}

fn parse_dimensions_tag(
    tags: &[Vec<String>],
) -> Result<Option<RadrootsFarmFileDimensions>, EventParseError> {
    let Some(value) = optional_tag_value(tags, TAG_DIMENSIONS)? else {
        return Ok(None);
    };
    Ok(Some(parse_dimensions(&value, TAG_DIMENSIONS)?))
}

fn parse_dimensions(
    value: &str,
    tag: &'static str,
) -> Result<RadrootsFarmFileDimensions, EventParseError> {
    let (w, h) = value
        .split_once('x')
        .ok_or(EventParseError::InvalidTag(tag))?;
    let w = w
        .parse::<u32>()
        .map_err(|_| EventParseError::InvalidTag(tag))?;
    let h = h
        .parse::<u32>()
        .map_err(|_| EventParseError::InvalidTag(tag))?;
    if w == 0 || h == 0 {
        return Err(EventParseError::InvalidTag(tag));
    }
    Ok(RadrootsFarmFileDimensions { w, h })
}

fn parse_source_tag(
    tags: &[Vec<String>],
    key: &'static str,
) -> Result<Option<RadrootsFarmFileSource>, EventParseError> {
    let Some(tag) = tags
        .iter()
        .find(|tag| tag.first().map(|value| value.as_str()) == Some(key))
    else {
        return Ok(None);
    };
    if tag.len() < 2 || tag.len() > 4 {
        return Err(EventParseError::InvalidTag(key));
    }
    let url = tag[1].clone();
    validate_non_empty_tag_value(&url, key)?;
    let mut mime_type = None;
    let mut dimensions = None;
    if let Some(value) = tag.get(2) {
        validate_non_empty_tag_value(value, key)?;
        if value.contains('x') {
            dimensions = Some(parse_dimensions(value, key)?);
        } else {
            mime_type = Some(value.clone());
        }
    }
    if let Some(value) = tag.get(3) {
        validate_non_empty_tag_value(value, key)?;
        dimensions = Some(parse_dimensions(value, key)?);
    }
    Ok(Some(RadrootsFarmFileSource {
        url,
        mime_type,
        dimensions,
    }))
}

fn encode_error_to_parse_error(error: crate::error::EventEncodeError) -> EventParseError {
    match error {
        crate::error::EventEncodeError::InvalidKind(kind) => EventParseError::InvalidKind {
            expected: EXPECTED_KIND,
            got: kind,
        },
        crate::error::EventEncodeError::EmptyRequiredField(field)
        | crate::error::EventEncodeError::InvalidField(field) => match field {
            "d_tag" => EventParseError::InvalidTag(TAG_D),
            "farm_group_id" => EventParseError::InvalidTag(TAG_H),
            "workspace.pubkey" | "workspace.d_tag" => EventParseError::InvalidTag(TAG_A),
            "owner_document_id" => EventParseError::InvalidTag(TAG_OWNER_DOCUMENT),
            "url" => EventParseError::InvalidTag(TAG_URL),
            "mime_type" => EventParseError::InvalidTag(TAG_MIME),
            "sha256" => EventParseError::InvalidTag(TAG_SHA256),
            "original_sha256" => EventParseError::InvalidTag(TAG_ORIGINAL_SHA256),
            field => EventParseError::InvalidTag(field),
        },
        crate::error::EventEncodeError::Json => EventParseError::InvalidTag("content"),
    }
}
