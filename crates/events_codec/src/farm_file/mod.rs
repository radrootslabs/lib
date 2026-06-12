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
    use crate::farm_file::decode::farm_file_metadata_from_event;
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
}
