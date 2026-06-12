#![cfg(feature = "serde_json")]

use radroots_events::{
    farm_crdt::RadrootsFarmCrdtDocumentKind,
    farm_file::{RadrootsFarmFileDimensions, RadrootsFarmFileMetadata},
    farm_workspace::RadrootsFarmWorkspaceRef,
    file_metadata::RadrootsFileMetadata,
    group::{RadrootsGroupEditableMetadata, RadrootsGroupMetadata},
    kinds::{
        KIND_ARTICLE, KIND_FARM, KIND_FARM_CRDT_CHANGE, KIND_GROUP_METADATA, KIND_LISTING,
        KIND_POST, KIND_PUBLIC_FILE_METADATA, is_public_social_kind,
    },
    social::RadrootsSocialMediaDimensions,
};
use radroots_events_codec::{
    article::decode::article_from_event,
    error::EventParseError,
    farm::decode::farm_from_event,
    farm_file::{
        decode::farm_file_metadata_from_event, encode::to_wire_parts as farm_file_to_wire_parts,
    },
    file_metadata::{
        decode::file_metadata_from_event, encode::to_wire_parts as public_file_to_wire_parts,
    },
    group::{decode::group_metadata_from_event, encode::group_metadata_to_wire_parts},
    listing::decode::listing_from_event,
    post::decode::post_from_event,
};

const SHA256: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const OTHER_SHA256: &str = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";
const D_TAG: &str = "AAAAAAAAAAAAAAAAAAAAAA";

#[test]
fn social_events_keep_public_and_private_file_metadata_apis_separate() {
    let public = public_file_to_wire_parts(&public_file_metadata()).unwrap();
    let decoded_public =
        file_metadata_from_event(public.kind, &public.tags, &public.content).unwrap();
    assert_eq!(decoded_public.url, "https://media.example.test/public.jpg");
    assert!(matches!(
        farm_file_metadata_from_event(public.kind, &public.tags, &public.content),
        Err(EventParseError::MissingTag("d"))
    ));

    let private = farm_file_to_wire_parts(&private_farm_file_metadata()).unwrap();
    let decoded_private =
        farm_file_metadata_from_event(private.kind, &private.tags, &private.content).unwrap();
    assert_eq!(decoded_private.owner_document_id, D_TAG);
    assert!(matches!(
        file_metadata_from_event(private.kind, &private.tags, &private.content),
        Err(EventParseError::InvalidTag("radroots:owner_document"))
    ));
}

#[test]
fn social_events_reject_private_farm_ops_semantics_in_public_codecs() {
    let private_content = serde_json::json!({
        "workspace": {
            "pubkey": "workspace_pubkey",
            "d_tag": D_TAG
        },
        "farm_group_id": "field-group",
        "document_id": D_TAG,
        "document_kind": "FarmTask",
        "encoded_change": "abc-DEF_012"
    })
    .to_string();
    let farm_tags = vec![vec!["d".to_string(), D_TAG.to_string()]];

    assert!(matches!(
        farm_from_event(KIND_FARM, &farm_tags, &private_content),
        Err(EventParseError::InvalidJson("content"))
    ));
    assert!(matches!(
        post_from_event(KIND_FARM_CRDT_CHANGE, &[], "farm task"),
        Err(EventParseError::InvalidKind {
            expected: "1",
            got: KIND_FARM_CRDT_CHANGE
        })
    ));
    assert!(matches!(
        article_from_event(KIND_FARM_CRDT_CHANGE, &[], "farm task"),
        Err(EventParseError::InvalidKind {
            expected: "30023",
            got: KIND_FARM_CRDT_CHANGE
        })
    ));
    assert!(matches!(
        listing_from_event(KIND_FARM_CRDT_CHANGE, &[], "farm task"),
        Err(EventParseError::InvalidKind {
            expected: "30402 or 30403",
            got: KIND_FARM_CRDT_CHANGE
        })
    ));
    assert!(is_public_social_kind(KIND_POST));
    assert!(is_public_social_kind(KIND_ARTICLE));
    assert!(is_public_social_kind(KIND_PUBLIC_FILE_METADATA));
    assert!(!is_public_social_kind(KIND_FARM_CRDT_CHANGE));
    assert!(!is_public_social_kind(KIND_LISTING));
}

#[test]
fn social_events_keep_nip29_groups_out_of_public_social_classification() {
    let group = RadrootsGroupMetadata {
        d_tag: "field-group".to_string(),
        metadata: RadrootsGroupEditableMetadata {
            name: Some("Field Group".to_string()),
            about: Some("Localhost field coordination".to_string()),
            picture: None,
            is_private: false,
            is_restricted: false,
            is_closed: false,
            is_hidden: false,
            supported_kinds: Some(vec![KIND_FARM_CRDT_CHANGE]),
        },
    };
    let parts = group_metadata_to_wire_parts(&group).unwrap();
    assert_eq!(parts.kind, KIND_GROUP_METADATA);
    assert!(!is_public_social_kind(KIND_GROUP_METADATA));
    assert_eq!(
        group_metadata_from_event(parts.kind, &parts.tags, &parts.content)
            .unwrap()
            .metadata
            .supported_kinds,
        Some(vec![KIND_FARM_CRDT_CHANGE])
    );
}

fn public_file_metadata() -> RadrootsFileMetadata {
    RadrootsFileMetadata {
        url: "https://media.example.test/public.jpg".to_string(),
        mime_type: "image/jpeg".to_string(),
        sha256: SHA256.to_string(),
        original_sha256: Some(OTHER_SHA256.to_string()),
        size: Some(4096),
        dimensions: Some(RadrootsSocialMediaDimensions {
            width: 1200,
            height: 800,
        }),
        blurhash: None,
        thumbnails: None,
        summary: Some("Public field photo".to_string()),
        alt: Some("Rows after harvest".to_string()),
        fallback: None,
        magnet: None,
        content_hashes: None,
        services: None,
        content: Some("caption".to_string()),
    }
}

fn private_farm_file_metadata() -> RadrootsFarmFileMetadata {
    RadrootsFarmFileMetadata {
        d_tag: D_TAG.to_string(),
        workspace: RadrootsFarmWorkspaceRef {
            pubkey: "workspace_pubkey".to_string(),
            d_tag: D_TAG.to_string(),
        },
        farm_group_id: "field-group".to_string(),
        owner_document_id: D_TAG.to_string(),
        owner_document_kind: RadrootsFarmCrdtDocumentKind::FarmTask,
        caption: Some("private caption".to_string()),
        url: "https://media.example.test/private.jpg".to_string(),
        mime_type: "image/jpeg".to_string(),
        sha256: SHA256.to_string(),
        original_sha256: Some(OTHER_SHA256.to_string()),
        size_bytes: Some(4096),
        dimensions: Some(RadrootsFarmFileDimensions { w: 1200, h: 800 }),
        blurhash: None,
        thumb: None,
        image: None,
        alt: Some("Private farm task attachment".to_string()),
        fallbacks: Vec::new(),
    }
}
