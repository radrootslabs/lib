pub mod decode;
pub mod encode;

#[cfg(test)]
mod tests {
    use radroots_events::{
        farm_crdt::RadrootsFarmCrdtDocumentKind,
        farm_file::{
            KIND_FARM_FILE_METADATA, RadrootsFarmFileDimensions, RadrootsFarmFileMetadata,
            RadrootsFarmFileSource,
        },
        farm_workspace::RadrootsFarmWorkspaceRef,
        kinds::KIND_POST,
    };

    use crate::error::{EventEncodeError, EventParseError};
    use crate::farm_file::decode::{
        data_from_event, farm_file_metadata_from_event, parsed_from_event,
    };
    use crate::farm_file::encode::{
        farm_file_metadata_build_tags, to_wire_parts, to_wire_parts_with_kind,
    };

    const FILE_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAAAQ";
    const WORKSPACE_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAAAA";
    const OWNER_DOCUMENT_ID: &str = "AAAAAAAAAAAAAAAAAAAAAg";
    const GROUP_ID: &str = "field-group";
    const SHA256: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    #[test]
    fn farm_file_metadata_encodes_tags_and_caption_content() {
        let metadata = sample_metadata();
        let parts = to_wire_parts(&metadata).expect("file metadata wire parts");

        assert_eq!(parts.kind, KIND_FARM_FILE_METADATA);
        assert_eq!(parts.content, "Tomatoes harvested from Patch Y.");
        assert!(parts.tags.contains(&tag("d", FILE_D_TAG)));
        assert!(parts.tags.contains(&tag("h", GROUP_ID)));
        assert!(
            parts
                .tags
                .contains(&tag("a", "30078:workspace_pubkey:AAAAAAAAAAAAAAAAAAAAAA"))
        );
        assert!(
            parts
                .tags
                .contains(&tag("url", "https://media.example.invalid/blob/sha256"))
        );
        assert!(parts.tags.contains(&tag("m", "image/jpeg")));
        assert!(parts.tags.contains(&tag("x", SHA256)));

        let decoded = farm_file_metadata_from_event(parts.kind, &parts.tags, &parts.content)
            .expect("file metadata decode");
        assert_eq!(decoded, metadata);
    }

    #[test]
    fn farm_file_metadata_rejects_missing_x_bad_hash_and_missing_url() {
        let parts = to_wire_parts(&sample_metadata()).expect("file metadata wire parts");
        let without_x = parts
            .tags
            .iter()
            .filter(|tag| tag.first().map(|value| value.as_str()) != Some("x"))
            .cloned()
            .collect::<Vec<_>>();
        let missing_x =
            farm_file_metadata_from_event(parts.kind, &without_x, &parts.content).unwrap_err();
        assert!(matches!(missing_x, EventParseError::MissingTag("x")));

        let mut bad_hash = sample_metadata();
        bad_hash.sha256 = "ABC".to_string();
        let hash_err = farm_file_metadata_build_tags(&bad_hash).unwrap_err();
        assert!(matches!(hash_err, EventEncodeError::InvalidField("sha256")));

        let mut missing_url = sample_metadata();
        missing_url.url.clear();
        let url_err = to_wire_parts(&missing_url).unwrap_err();
        assert!(matches!(
            url_err,
            EventEncodeError::EmptyRequiredField("url")
        ));
    }

    #[test]
    fn farm_file_metadata_rejects_d_mismatch_and_kind_mismatch() {
        let parts = to_wire_parts(&sample_metadata()).expect("file metadata wire parts");
        let mut duplicate_d = parts.tags.clone();
        duplicate_d.push(vec!["d".to_string(), "AAAAAAAAAAAAAAAAAAAAAw".to_string()]);
        let mismatch =
            farm_file_metadata_from_event(parts.kind, &duplicate_d, &parts.content).unwrap_err();
        assert!(matches!(mismatch, EventParseError::InvalidTag("d")));

        let wrong_kind = to_wire_parts_with_kind(&sample_metadata(), KIND_POST).unwrap_err();
        assert!(matches!(
            wrong_kind,
            EventEncodeError::InvalidKind(KIND_POST)
        ));

        let decode_wrong_kind =
            farm_file_metadata_from_event(KIND_POST, &parts.tags, &parts.content).unwrap_err();
        assert!(matches!(
            decode_wrong_kind,
            EventParseError::InvalidKind {
                expected: "1063",
                got: KIND_POST
            }
        ));
    }

    #[test]
    fn farm_file_metadata_decodes_empty_content_as_absent_caption() {
        let mut metadata = sample_metadata();
        metadata.caption = None;
        let parts = to_wire_parts(&metadata).expect("file metadata wire parts");

        assert_eq!(parts.content, "");
        let decoded =
            farm_file_metadata_from_event(parts.kind, &parts.tags, "").expect("file decode");
        assert_eq!(decoded.caption, None);
    }

    #[test]
    fn farm_file_metadata_wrappers_roundtrip_minimal_optional_shape() {
        let mut metadata = sample_metadata();
        metadata.caption = None;
        metadata.original_sha256 = None;
        metadata.size_bytes = None;
        metadata.dimensions = None;
        metadata.blurhash = None;
        metadata.thumb = None;
        metadata.image = Some(RadrootsFarmFileSource {
            url: "https://media.example.invalid/image/sha256".to_string(),
            mime_type: None,
            dimensions: Some(RadrootsFarmFileDimensions { w: 640, h: 480 }),
        });
        metadata.alt = None;
        metadata.fallbacks.clear();
        let parts = to_wire_parts(&metadata).expect("file metadata wire parts");

        assert!(
            !parts
                .tags
                .iter()
                .any(|tag| tag.first().map(String::as_str) == Some("size"))
        );
        assert!(
            !parts
                .tags
                .iter()
                .any(|tag| tag.first().map(String::as_str) == Some("dim"))
        );
        assert!(parts.tags.iter().any(|tag| tag
            == &vec![
                "image".to_string(),
                "https://media.example.invalid/image/sha256".to_string(),
                "640x480".to_string()
            ]));

        let data = data_from_event(
            "event-id".to_string(),
            "author-pubkey".to_string(),
            42,
            parts.kind,
            parts.content.clone(),
            parts.tags.clone(),
        )
        .expect("parsed data");
        assert_eq!(data.id, "event-id");
        assert_eq!(data.author, "author-pubkey");
        assert_eq!(data.published_at, 42);
        assert_eq!(data.kind, KIND_FARM_FILE_METADATA);
        assert_eq!(data.data, metadata);

        let err = parsed_from_event(
            "event-id".to_string(),
            "author-pubkey".to_string(),
            42,
            KIND_POST,
            parts.content.clone(),
            parts.tags.clone(),
            "sig".to_string(),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            EventParseError::InvalidKind {
                expected: "1063",
                got: KIND_POST
            }
        ));

        let parsed = parsed_from_event(
            "event-id".to_string(),
            "author-pubkey".to_string(),
            42,
            parts.kind,
            parts.content,
            parts.tags,
            "sig".to_string(),
        )
        .expect("parsed event");
        assert_eq!(parsed.event.sig, "sig");
        assert_eq!(parsed.data.data, metadata);
    }

    #[test]
    fn farm_file_metadata_preserves_expanded_owner_document_kinds() {
        for kind in [
            RadrootsFarmCrdtDocumentKind::FarmMembership,
            RadrootsFarmCrdtDocumentKind::FarmRolePolicy,
            RadrootsFarmCrdtDocumentKind::FarmActivity,
            RadrootsFarmCrdtDocumentKind::FarmLocation,
            RadrootsFarmCrdtDocumentKind::FarmCrop,
            RadrootsFarmCrdtDocumentKind::FarmCropVariety,
            RadrootsFarmCrdtDocumentKind::FarmCropCycle,
            RadrootsFarmCrdtDocumentKind::FarmAttachment,
            RadrootsFarmCrdtDocumentKind::FarmPayPeriod,
            RadrootsFarmCrdtDocumentKind::Other {
                value: "FarmSoilTest".to_string(),
            },
        ] {
            let mut metadata = sample_metadata();
            metadata.owner_document_kind = kind;
            let parts = to_wire_parts(&metadata).expect("file metadata wire parts");
            let decoded = farm_file_metadata_from_event(parts.kind, &parts.tags, &parts.content)
                .expect("file metadata decode");

            assert_eq!(decoded.owner_document_kind, metadata.owner_document_kind);
        }
    }

    #[test]
    fn farm_file_metadata_rejects_malformed_decode_tags() {
        let parts = to_wire_parts(&sample_metadata()).expect("file metadata wire parts");

        let mut missing_owner = parts.tags.clone();
        missing_owner
            .retain(|tag| tag.first().map(String::as_str) != Some("radroots:owner_document"));
        let err =
            farm_file_metadata_from_event(parts.kind, &missing_owner, &parts.content).unwrap_err();
        assert!(matches!(
            err,
            EventParseError::MissingTag("radroots:owner_document")
        ));

        for replacement in [
            vec![
                "radroots:owner_document".to_string(),
                OWNER_DOCUMENT_ID.to_string(),
            ],
            vec![
                "radroots:owner_document".to_string(),
                OWNER_DOCUMENT_ID.to_string(),
                " ".to_string(),
            ],
            vec![
                "radroots:owner_document".to_string(),
                "bad d tag".to_string(),
                "FarmTask".to_string(),
            ],
        ] {
            let mut tags = replace_tag(&parts.tags, "radroots:owner_document", replacement);
            let err = farm_file_metadata_from_event(parts.kind, &tags, &parts.content).unwrap_err();
            assert!(matches!(
                err,
                EventParseError::InvalidTag("radroots:owner_document")
            ));
            tags.clear();
        }

        for (key, value, expected) in [
            ("size", "not-a-number", "size"),
            ("dim", "bad", "dim"),
            ("dim", "0x12", "dim"),
            ("dim", "12x0", "dim"),
            ("thumb", "", "thumb"),
            ("thumb", " ", "thumb"),
        ] {
            let tags = replace_tag(&parts.tags, key, tag(key, value));
            let err = farm_file_metadata_from_event(parts.kind, &tags, &parts.content).unwrap_err();
            match err {
                EventParseError::InvalidTag(found) | EventParseError::InvalidNumber(found, _) => {
                    assert_eq!(found, expected);
                }
                other => panic!("unexpected error: {other:?}"),
            }
        }

        for replacement in [
            vec!["thumb".to_string()],
            vec![
                "thumb".to_string(),
                "https://media.example.invalid/thumb/sha256".to_string(),
                "image/jpeg".to_string(),
                "320x240".to_string(),
                "extra".to_string(),
            ],
            vec![
                "thumb".to_string(),
                "https://media.example.invalid/thumb/sha256".to_string(),
                "image/jpeg".to_string(),
                " ".to_string(),
            ],
        ] {
            let tags = replace_tag(&parts.tags, "thumb", replacement);
            let err = farm_file_metadata_from_event(parts.kind, &tags, &parts.content).unwrap_err();
            assert!(matches!(err, EventParseError::InvalidTag("thumb")));
        }

        let tags = replace_tag(
            &parts.tags,
            "thumb",
            vec![
                "thumb".to_string(),
                "https://media.example.invalid/thumb/sha256".to_string(),
                "320x240".to_string(),
            ],
        );
        let decoded =
            farm_file_metadata_from_event(parts.kind, &tags, &parts.content).expect("metadata");
        assert_eq!(
            decoded
                .thumb
                .as_ref()
                .and_then(|source| source.mime_type.as_deref()),
            None
        );
        assert_eq!(
            decoded.thumb.and_then(|source| source.dimensions),
            Some(RadrootsFarmFileDimensions { w: 320, h: 240 })
        );

        let tags = replace_tag(
            &parts.tags,
            "thumb",
            vec![
                "thumb".to_string(),
                "https://media.example.invalid/thumb/sha256".to_string(),
            ],
        );
        let decoded =
            farm_file_metadata_from_event(parts.kind, &tags, &parts.content).expect("metadata");
        assert_eq!(decoded.thumb.and_then(|source| source.dimensions), None);

        let err = farm_file_metadata_from_event(parts.kind, &parts.tags, " ").unwrap_err();
        assert!(matches!(err, EventParseError::InvalidTag("caption")));
    }

    #[test]
    fn farm_file_metadata_rejects_encoder_validation_edges() {
        for (metadata, expected) in [
            {
                let mut metadata = sample_metadata();
                metadata.workspace.d_tag = "bad d tag".to_string();
                (metadata, EventEncodeError::InvalidField("workspace.d_tag"))
            },
            {
                let mut metadata = sample_metadata();
                metadata.caption = Some("".to_string());
                (metadata, EventEncodeError::EmptyRequiredField("caption"))
            },
            {
                let mut metadata = sample_metadata();
                metadata.dimensions = Some(RadrootsFarmFileDimensions { w: 0, h: 1200 });
                (metadata, EventEncodeError::InvalidField("dimensions"))
            },
            {
                let mut metadata = sample_metadata();
                metadata.blurhash = Some("".to_string());
                (metadata, EventEncodeError::EmptyRequiredField("blurhash"))
            },
            {
                let mut metadata = sample_metadata();
                metadata.thumb = Some(RadrootsFarmFileSource {
                    url: "".to_string(),
                    mime_type: None,
                    dimensions: None,
                });
                (metadata, EventEncodeError::EmptyRequiredField("thumb"))
            },
            {
                let mut metadata = sample_metadata();
                metadata.thumb = Some(RadrootsFarmFileSource {
                    url: "https://media.example.invalid/thumb/sha256".to_string(),
                    mime_type: Some("".to_string()),
                    dimensions: None,
                });
                (metadata, EventEncodeError::EmptyRequiredField("thumb"))
            },
            {
                let mut metadata = sample_metadata();
                metadata.thumb = Some(RadrootsFarmFileSource {
                    url: "https://media.example.invalid/thumb/sha256".to_string(),
                    mime_type: None,
                    dimensions: Some(RadrootsFarmFileDimensions { w: 320, h: 0 }),
                });
                (metadata, EventEncodeError::InvalidField("thumb"))
            },
            {
                let mut metadata = sample_metadata();
                metadata.alt = Some("".to_string());
                (metadata, EventEncodeError::EmptyRequiredField("alt"))
            },
            {
                let mut metadata = sample_metadata();
                metadata.fallbacks = vec!["".to_string()];
                (metadata, EventEncodeError::EmptyRequiredField("fallbacks"))
            },
        ] {
            let err = farm_file_metadata_build_tags(&metadata).unwrap_err();
            assert_same_encode_error(err, expected);
        }
    }

    fn sample_metadata() -> RadrootsFarmFileMetadata {
        RadrootsFarmFileMetadata {
            d_tag: FILE_D_TAG.to_string(),
            workspace: RadrootsFarmWorkspaceRef {
                pubkey: "workspace_pubkey".to_string(),
                d_tag: WORKSPACE_D_TAG.to_string(),
            },
            farm_group_id: GROUP_ID.to_string(),
            owner_document_id: OWNER_DOCUMENT_ID.to_string(),
            owner_document_kind: RadrootsFarmCrdtDocumentKind::FarmTask,
            caption: Some("Tomatoes harvested from Patch Y.".to_string()),
            url: "https://media.example.invalid/blob/sha256".to_string(),
            mime_type: "image/jpeg".to_string(),
            sha256: SHA256.to_string(),
            original_sha256: Some(
                "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789".to_string(),
            ),
            size_bytes: Some(123_456),
            dimensions: Some(RadrootsFarmFileDimensions { w: 1600, h: 1200 }),
            blurhash: Some("LEHV6nWB2yk8pyo0adR*.7kCMdnj".to_string()),
            thumb: Some(RadrootsFarmFileSource {
                url: "https://media.example.invalid/thumb/sha256".to_string(),
                mime_type: Some("image/jpeg".to_string()),
                dimensions: Some(RadrootsFarmFileDimensions { w: 320, h: 240 }),
            }),
            image: None,
            alt: Some("Harvested tomatoes in a crate".to_string()),
            fallbacks: vec!["https://fallback.example.invalid/blob/sha256".to_string()],
        }
    }

    fn tag(key: &str, value: &str) -> Vec<String> {
        vec![key.to_string(), value.to_string()]
    }

    fn replace_tag(tags: &[Vec<String>], key: &str, replacement: Vec<String>) -> Vec<Vec<String>> {
        tags.iter()
            .map(|tag| {
                if tag.first().map(String::as_str) == Some(key) {
                    replacement.clone()
                } else {
                    tag.clone()
                }
            })
            .collect()
    }

    fn assert_same_encode_error(actual: EventEncodeError, expected: EventEncodeError) {
        match (actual, expected) {
            (
                EventEncodeError::EmptyRequiredField(actual),
                EventEncodeError::EmptyRequiredField(expected),
            )
            | (EventEncodeError::InvalidField(actual), EventEncodeError::InvalidField(expected)) => {
                assert_eq!(actual, expected);
            }
            (EventEncodeError::InvalidKind(actual), EventEncodeError::InvalidKind(expected)) => {
                assert_eq!(actual, expected);
            }
            (EventEncodeError::Json, EventEncodeError::Json) => {}
            (actual, expected) => panic!("unexpected error {actual:?}, expected {expected:?}"),
        }
    }
}
