#![cfg(feature = "serde_json")]

use radroots_events::{
    file_metadata::RadrootsFileMetadata,
    kinds::{KIND_POST, KIND_PUBLIC_FILE_METADATA},
    social::{RadrootsSocialMediaDimensions, RadrootsSocialMediaThumbnail},
    tags::{
        TAG_ALT, TAG_DIMENSIONS, TAG_FALLBACK, TAG_MAGNET, TAG_MIME, TAG_ORIGINAL_SHA256,
        TAG_SERVICE, TAG_SHA256, TAG_SIZE, TAG_SUMMARY, TAG_THUMB, TAG_URL,
    },
};
use radroots_events_codec::{
    error::{EventEncodeError, EventParseError},
    file_metadata::{
        decode::{data_from_event, file_metadata_from_event, parsed_from_event},
        encode::{file_metadata_build_tags, to_wire_parts, to_wire_parts_with_kind},
    },
};

const VALID_HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const OTHER_HASH: &str = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";

fn sample_metadata() -> RadrootsFileMetadata {
    RadrootsFileMetadata {
        url: "https://media.example.test/field.jpg".to_string(),
        mime_type: "image/jpeg".to_string(),
        sha256: VALID_HASH.to_string(),
        original_sha256: Some(OTHER_HASH.to_string()),
        size: Some(4096),
        dimensions: Some(RadrootsSocialMediaDimensions {
            width: 1200,
            height: 800,
        }),
        blurhash: Some("L6PZfSi_.AyE_3t7t7R**0o#DgR4".to_string()),
        thumbnails: Some(vec![RadrootsSocialMediaThumbnail {
            url: "https://media.example.test/field-thumb.jpg".to_string(),
            dimensions: Some(RadrootsSocialMediaDimensions {
                width: 320,
                height: 200,
            }),
        }]),
        summary: Some("Field image".to_string()),
        alt: Some("Rows of greens after harvest".to_string()),
        fallback: Some("https://backup.example.test/field.jpg".to_string()),
        magnet: Some("magnet:?xt=urn:btih:example".to_string()),
        content_hashes: Some(vec![format!("sha256:{VALID_HASH}")]),
        services: Some(vec!["https://media.example.test".to_string()]),
        content: Some("Harvest block photo".to_string()),
    }
}

fn has_tag(tags: &[Vec<String>], key: &str, value: &str) -> bool {
    tags.iter().any(|tag| {
        tag.first().map(|entry| entry.as_str()) == Some(key)
            && tag.get(1).map(|entry| entry.as_str()) == Some(value)
    })
}

#[test]
fn file_metadata_to_wire_parts_roundtrips_nip94_tags() {
    let metadata = sample_metadata();
    let parts = to_wire_parts(&metadata).unwrap();

    assert_eq!(parts.kind, KIND_PUBLIC_FILE_METADATA);
    assert_eq!(parts.content, "Harvest block photo");
    assert!(has_tag(
        &parts.tags,
        TAG_URL,
        "https://media.example.test/field.jpg"
    ));
    assert!(has_tag(&parts.tags, TAG_MIME, "image/jpeg"));
    assert!(has_tag(&parts.tags, TAG_SHA256, VALID_HASH));
    assert!(has_tag(&parts.tags, TAG_ORIGINAL_SHA256, OTHER_HASH));
    assert!(has_tag(&parts.tags, TAG_SIZE, "4096"));
    assert!(has_tag(&parts.tags, TAG_DIMENSIONS, "1200x800"));
    assert!(has_tag(
        &parts.tags,
        TAG_THUMB,
        "https://media.example.test/field-thumb.jpg"
    ));
    assert!(has_tag(&parts.tags, TAG_SUMMARY, "Field image"));
    assert!(has_tag(
        &parts.tags,
        TAG_ALT,
        "Rows of greens after harvest"
    ));
    assert!(has_tag(
        &parts.tags,
        TAG_FALLBACK,
        "https://backup.example.test/field.jpg"
    ));
    assert!(has_tag(
        &parts.tags,
        TAG_MAGNET,
        "magnet:?xt=urn:btih:example"
    ));
    assert!(has_tag(
        &parts.tags,
        "i",
        format!("sha256:{VALID_HASH}").as_str()
    ));
    assert!(has_tag(
        &parts.tags,
        TAG_SERVICE,
        "https://media.example.test"
    ));

    let decoded = file_metadata_from_event(parts.kind, &parts.tags, &parts.content).unwrap();
    assert_eq!(decoded.url, "https://media.example.test/field.jpg");
    assert_eq!(decoded.mime_type, "image/jpeg");
    assert_eq!(decoded.sha256, VALID_HASH);
    assert_eq!(decoded.original_sha256.as_deref(), Some(OTHER_HASH));
    assert_eq!(decoded.size, Some(4096));
    assert_eq!(decoded.dimensions.as_ref().map(|dim| dim.width), Some(1200));
    assert_eq!(
        decoded
            .thumbnails
            .as_ref()
            .map(|thumbnails| thumbnails[0].url.as_str()),
        Some("https://media.example.test/field-thumb.jpg")
    );
    assert_eq!(decoded.content.as_deref(), Some("Harvest block photo"));
}

#[test]
fn file_metadata_codec_requires_kind_required_tags_and_hash_shape() {
    let mut metadata = sample_metadata();
    metadata.url = "ipfs://field.jpg".to_string();
    assert!(matches!(
        file_metadata_build_tags(&metadata),
        Err(EventEncodeError::InvalidField("url"))
    ));

    let mut metadata = sample_metadata();
    metadata.sha256 = "ABC".to_string();
    assert!(matches!(
        to_wire_parts(&metadata),
        Err(EventEncodeError::InvalidField("sha256"))
    ));

    assert!(matches!(
        to_wire_parts_with_kind(&sample_metadata(), KIND_POST),
        Err(EventEncodeError::InvalidKind(KIND_POST))
    ));

    let mut tags = file_metadata_build_tags(&sample_metadata()).unwrap();
    tags.retain(|tag| tag.first().map(|value| value.as_str()) != Some(TAG_URL));
    assert!(matches!(
        file_metadata_from_event(KIND_PUBLIC_FILE_METADATA, &tags, ""),
        Err(EventParseError::MissingTag(TAG_URL))
    ));

    let mut tags = file_metadata_build_tags(&sample_metadata()).unwrap();
    let hash_tag = tags
        .iter_mut()
        .find(|tag| tag.first().map(|value| value.as_str()) == Some(TAG_SHA256))
        .expect("x tag");
    hash_tag[1] = "not-a-hash".to_string();
    assert!(matches!(
        file_metadata_from_event(KIND_PUBLIC_FILE_METADATA, &tags, ""),
        Err(EventParseError::InvalidTag(TAG_SHA256))
    ));
}

#[test]
fn file_metadata_wrappers_preserve_event_metadata() {
    let metadata = sample_metadata();
    let parts = to_wire_parts(&metadata).unwrap();
    let data = data_from_event(
        "file_id".to_string(),
        "author".to_string(),
        90,
        parts.kind,
        parts.content.clone(),
        parts.tags.clone(),
    )
    .unwrap();

    assert_eq!(data.id, "file_id");
    assert_eq!(data.kind, KIND_PUBLIC_FILE_METADATA);
    assert_eq!(data.data.url, "https://media.example.test/field.jpg");

    let parsed = parsed_from_event(
        "file_id".to_string(),
        "author".to_string(),
        90,
        parts.kind,
        parts.content,
        parts.tags,
        "sig".to_string(),
    )
    .unwrap();

    assert_eq!(parsed.event.created_at, 90);
    assert_eq!(parsed.event.sig, "sig");
    assert_eq!(parsed.data.data.sha256, VALID_HASH);
}
