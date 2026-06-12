#[cfg(not(feature = "std"))]
use alloc::{
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};

use radroots_events::{
    farm_crdt::RadrootsFarmCrdtDocumentKind,
    farm_file::{
        KIND_FARM_FILE_METADATA, RadrootsFarmFileDimensions, RadrootsFarmFileMetadata,
        RadrootsFarmFileSource,
    },
    farm_workspace::KIND_FARM_WORKSPACE_MANIFEST,
    tags::{TAG_A, TAG_D, TAG_H, TAG_MIME, TAG_ORIGINAL_SHA256, TAG_SHA256, TAG_URL},
};

use crate::d_tag::validate_d_tag;
use crate::error::EventEncodeError;
use crate::field_helpers::{
    address_string, push_optional_tag, push_tag, push_tag_values, validate_lowercase_hex_64,
    validate_non_empty_field,
};
use crate::wire::WireEventParts;

const TAG_ALT: &str = "alt";
const TAG_BLURHASH: &str = "blurhash";
const TAG_DIMENSIONS: &str = "dim";
const TAG_FALLBACK: &str = "fallback";
const TAG_IMAGE: &str = "image";
const TAG_OWNER_DOCUMENT: &str = "radroots:owner_document";
const TAG_SIZE: &str = "size";
const TAG_THUMB: &str = "thumb";

pub fn farm_file_metadata_build_tags(
    metadata: &RadrootsFarmFileMetadata,
) -> Result<Vec<Vec<String>>, EventEncodeError> {
    validate_metadata(metadata)?;
    let workspace = address_string(
        KIND_FARM_WORKSPACE_MANIFEST,
        &metadata.workspace.pubkey,
        &metadata.workspace.d_tag,
        "workspace",
    )?;
    let mut tags = Vec::new();
    push_tag(&mut tags, TAG_D, metadata.d_tag.as_str());
    push_tag(&mut tags, TAG_H, metadata.farm_group_id.as_str());
    push_tag(&mut tags, TAG_A, workspace);
    push_tag(&mut tags, TAG_URL, metadata.url.as_str());
    push_tag(&mut tags, TAG_MIME, metadata.mime_type.as_str());
    push_tag(&mut tags, TAG_SHA256, metadata.sha256.as_str());
    push_tag_values(
        &mut tags,
        TAG_OWNER_DOCUMENT,
        [
            metadata.owner_document_id.as_str(),
            document_kind_tag(metadata.owner_document_kind),
        ],
    );
    push_optional_tag(
        &mut tags,
        TAG_ORIGINAL_SHA256,
        metadata.original_sha256.as_deref(),
    );
    if let Some(size) = metadata.size_bytes {
        push_tag(&mut tags, TAG_SIZE, size.to_string());
    }
    if let Some(dimensions) = metadata.dimensions {
        push_tag(&mut tags, TAG_DIMENSIONS, dimensions_tag(dimensions));
    }
    push_optional_tag(&mut tags, TAG_BLURHASH, metadata.blurhash.as_deref());
    push_source_tag(&mut tags, TAG_THUMB, metadata.thumb.as_ref())?;
    push_source_tag(&mut tags, TAG_IMAGE, metadata.image.as_ref())?;
    push_optional_tag(&mut tags, TAG_ALT, metadata.alt.as_deref());
    for fallback in &metadata.fallbacks {
        validate_non_empty_field(fallback, "fallbacks")?;
        push_tag(&mut tags, TAG_FALLBACK, fallback.clone());
    }
    Ok(tags)
}

pub fn to_wire_parts(
    metadata: &RadrootsFarmFileMetadata,
) -> Result<WireEventParts, EventEncodeError> {
    to_wire_parts_with_kind(metadata, KIND_FARM_FILE_METADATA)
}

pub fn to_wire_parts_with_kind(
    metadata: &RadrootsFarmFileMetadata,
    kind: u32,
) -> Result<WireEventParts, EventEncodeError> {
    if kind != KIND_FARM_FILE_METADATA {
        return Err(EventEncodeError::InvalidKind(kind));
    }
    let tags = farm_file_metadata_build_tags(metadata)?;
    Ok(WireEventParts {
        kind,
        content: metadata.caption.clone().unwrap_or_default(),
        tags,
    })
}

pub(crate) fn validate_metadata(
    metadata: &RadrootsFarmFileMetadata,
) -> Result<(), EventEncodeError> {
    validate_d_tag(&metadata.d_tag, "d_tag")?;
    validate_non_empty_field(&metadata.farm_group_id, "farm_group_id")?;
    validate_non_empty_field(&metadata.workspace.pubkey, "workspace.pubkey")?;
    validate_d_tag(&metadata.workspace.d_tag, "workspace.d_tag")?;
    validate_d_tag(&metadata.owner_document_id, "owner_document_id")?;
    validate_non_empty_field(&metadata.url, "url")?;
    validate_non_empty_field(&metadata.mime_type, "mime_type")?;
    validate_lowercase_hex_64(&metadata.sha256, "sha256")?;
    if let Some(hash) = metadata.original_sha256.as_deref() {
        validate_lowercase_hex_64(hash, "original_sha256")?;
    }
    if let Some(caption) = metadata.caption.as_deref() {
        validate_non_empty_field(caption, "caption")?;
    }
    if let Some(dimensions) = metadata.dimensions {
        validate_dimensions(dimensions, "dimensions")?;
    }
    if let Some(blurhash) = metadata.blurhash.as_deref() {
        validate_non_empty_field(blurhash, "blurhash")?;
    }
    validate_source(metadata.thumb.as_ref(), "thumb")?;
    validate_source(metadata.image.as_ref(), "image")?;
    if let Some(alt) = metadata.alt.as_deref() {
        validate_non_empty_field(alt, "alt")?;
    }
    for fallback in &metadata.fallbacks {
        validate_non_empty_field(fallback, "fallbacks")?;
    }
    Ok(())
}

pub(crate) fn document_kind_tag(kind: RadrootsFarmCrdtDocumentKind) -> &'static str {
    match kind {
        RadrootsFarmCrdtDocumentKind::FarmTask => "FarmTask",
        RadrootsFarmCrdtDocumentKind::FarmWorkSession => "FarmWorkSession",
        RadrootsFarmCrdtDocumentKind::FarmHarvestRecord => "FarmHarvestRecord",
        RadrootsFarmCrdtDocumentKind::FarmInventoryItem => "FarmInventoryItem",
        RadrootsFarmCrdtDocumentKind::FarmMediaAsset => "FarmMediaAsset",
        RadrootsFarmCrdtDocumentKind::FarmObservation => "FarmObservation",
    }
}

fn validate_dimensions(
    dimensions: RadrootsFarmFileDimensions,
    field: &'static str,
) -> Result<(), EventEncodeError> {
    if dimensions.w == 0 || dimensions.h == 0 {
        Err(EventEncodeError::InvalidField(field))
    } else {
        Ok(())
    }
}

fn validate_source(
    source: Option<&RadrootsFarmFileSource>,
    field: &'static str,
) -> Result<(), EventEncodeError> {
    let Some(source) = source else {
        return Ok(());
    };
    validate_non_empty_field(&source.url, field)?;
    if let Some(mime_type) = source.mime_type.as_deref() {
        validate_non_empty_field(mime_type, field)?;
    }
    if let Some(dimensions) = source.dimensions {
        validate_dimensions(dimensions, field)?;
    }
    Ok(())
}

fn push_source_tag(
    tags: &mut Vec<Vec<String>>,
    key: &'static str,
    source: Option<&RadrootsFarmFileSource>,
) -> Result<(), EventEncodeError> {
    let Some(source) = source else {
        return Ok(());
    };
    validate_source(Some(source), key)?;
    let mut values = vec![source.url.clone()];
    if let Some(mime_type) = source.mime_type.as_deref() {
        values.push(mime_type.to_string());
    }
    if let Some(dimensions) = source.dimensions {
        values.push(dimensions_tag(dimensions));
    }
    push_tag_values(tags, key, values);
    Ok(())
}

fn dimensions_tag(dimensions: RadrootsFarmFileDimensions) -> String {
    format!("{}x{}", dimensions.w, dimensions.h)
}
