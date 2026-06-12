#[cfg(not(feature = "std"))]
use alloc::{string::ToString, vec::Vec};

use radroots_events::{
    file_metadata::RadrootsFileMetadata,
    kinds::KIND_PUBLIC_FILE_METADATA,
    tags::{
        TAG_ALT, TAG_BLURHASH, TAG_DIMENSIONS, TAG_FALLBACK, TAG_MAGNET, TAG_MIME,
        TAG_ORIGINAL_SHA256, TAG_SERVICE, TAG_SHA256, TAG_SIZE, TAG_SUMMARY, TAG_URL,
    },
};

use crate::error::EventEncodeError;
use crate::field_helpers::{
    push_optional_tag, push_tag, validate_lowercase_hex_64, validate_non_empty_field,
};
use crate::social_helpers::{dimensions_tag, push_thumbnail, validate_http_url};
use crate::wire::WireEventParts;

const DEFAULT_KIND: u32 = KIND_PUBLIC_FILE_METADATA;

pub fn file_metadata_build_tags(
    metadata: &RadrootsFileMetadata,
) -> Result<Vec<Vec<String>>, EventEncodeError> {
    validate_metadata(metadata)?;
    let mut tags = Vec::new();
    push_tag(&mut tags, TAG_URL, metadata.url.as_str());
    push_tag(&mut tags, TAG_MIME, metadata.mime_type.as_str());
    push_tag(&mut tags, TAG_SHA256, metadata.sha256.as_str());
    push_optional_tag(
        &mut tags,
        TAG_ORIGINAL_SHA256,
        metadata.original_sha256.as_deref(),
    );
    if let Some(size) = metadata.size {
        push_tag(&mut tags, TAG_SIZE, size.to_string());
    }
    if let Some(dimensions) = metadata.dimensions.as_ref() {
        push_tag(&mut tags, TAG_DIMENSIONS, dimensions_tag(dimensions));
    }
    push_optional_tag(&mut tags, TAG_BLURHASH, metadata.blurhash.as_deref());
    if let Some(thumbnails) = metadata.thumbnails.as_ref() {
        for thumbnail in thumbnails {
            push_thumbnail(&mut tags, thumbnail);
        }
    }
    push_optional_tag(&mut tags, TAG_SUMMARY, metadata.summary.as_deref());
    push_optional_tag(&mut tags, TAG_ALT, metadata.alt.as_deref());
    push_optional_tag(&mut tags, TAG_FALLBACK, metadata.fallback.as_deref());
    push_optional_tag(&mut tags, TAG_MAGNET, metadata.magnet.as_deref());
    if let Some(content_hashes) = metadata.content_hashes.as_ref() {
        for hash in content_hashes {
            push_optional_tag(&mut tags, "i", Some(hash.as_str()));
        }
    }
    if let Some(services) = metadata.services.as_ref() {
        for service in services {
            push_optional_tag(&mut tags, TAG_SERVICE, Some(service.as_str()));
        }
    }
    Ok(tags)
}

pub fn to_wire_parts(metadata: &RadrootsFileMetadata) -> Result<WireEventParts, EventEncodeError> {
    to_wire_parts_with_kind(metadata, DEFAULT_KIND)
}

pub fn to_wire_parts_with_kind(
    metadata: &RadrootsFileMetadata,
    kind: u32,
) -> Result<WireEventParts, EventEncodeError> {
    if kind != DEFAULT_KIND {
        return Err(EventEncodeError::InvalidKind(kind));
    }
    Ok(WireEventParts {
        kind,
        content: metadata.content.clone().unwrap_or_default(),
        tags: file_metadata_build_tags(metadata)?,
    })
}

fn validate_metadata(metadata: &RadrootsFileMetadata) -> Result<(), EventEncodeError> {
    validate_http_url(&metadata.url, "url")?;
    validate_non_empty_field(&metadata.mime_type, "mime_type")?;
    validate_lowercase_hex_64(&metadata.sha256, "sha256")?;
    if let Some(hash) = metadata.original_sha256.as_deref() {
        validate_lowercase_hex_64(hash, "original_sha256")?;
    }
    if let Some(dimensions) = metadata.dimensions.as_ref() {
        if dimensions.width == 0 || dimensions.height == 0 {
            return Err(EventEncodeError::InvalidField("dimensions"));
        }
    }
    if let Some(thumbnails) = metadata.thumbnails.as_ref() {
        for thumbnail in thumbnails {
            validate_http_url(&thumbnail.url, "thumb")?;
        }
    }
    Ok(())
}
