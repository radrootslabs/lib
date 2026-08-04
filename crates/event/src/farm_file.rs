#![forbid(unsafe_code)]

use crate::envelope::kind::KIND_FARM_FILE_METADATA as KIND_FARM_FILE_METADATA_EVENT;
use crate::farm::crdt::FarmCrdtDocumentKind;
use crate::farm::workspace::FarmWorkspaceRef;

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

pub const KIND_FARM_FILE_METADATA: u32 = KIND_FARM_FILE_METADATA_EVENT;

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FarmFileMetadata {
    pub d_tag: String,
    pub workspace: FarmWorkspaceRef,
    pub farm_group_id: String,
    pub owner_document_id: String,
    pub owner_document_kind: FarmCrdtDocumentKind,
    pub caption: Option<String>,
    pub url: String,
    pub mime_type: String,
    pub sha256: String,
    pub original_sha256: Option<String>,
    pub size_bytes: Option<u64>,
    pub dimensions: Option<FarmFileDimensions>,
    pub blurhash: Option<String>,
    pub thumb: Option<FarmFileSource>,
    pub image: Option<FarmFileSource>,
    pub alt: Option<String>,
    pub fallbacks: Vec<String>,
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FarmFileDimensions {
    pub w: u32,
    pub h: u32,
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FarmFileSource {
    pub url: String,
    pub mime_type: Option<String>,
    pub dimensions: Option<FarmFileDimensions>,
}

#[cfg(all(test, feature = "serde"))]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

    #[test]
    fn file_metadata_kind_uses_nip94_file_metadata_kind() {
        assert_eq!(KIND_FARM_FILE_METADATA, 1063);
    }

    #[test]
    fn file_metadata_remains_separate_from_message_file_model() {
        let metadata = sample_file_metadata();

        assert_eq!(metadata.d_tag, "EFGHIJKLMNOPQRSTUVWXYZ");
        assert_eq!(metadata.owner_document_id, "DEFGHIJKLMNOPQRSTUVWXY");
        assert_eq!(metadata.owner_document_kind, FarmCrdtDocumentKind::FarmTask);
        assert_eq!(
            metadata.caption.as_deref(),
            Some("Tomatoes harvested from Patch Y.")
        );
        assert_eq!(metadata.mime_type, "image/jpeg");
        assert_eq!(
            metadata.dimensions,
            Some(FarmFileDimensions { w: 1600, h: 1200 })
        );
        assert_eq!(metadata.fallbacks.len(), 1);
    }

    #[test]
    fn file_metadata_serializes_stable_content_shape() {
        let value = serde_json::to_value(sample_file_metadata()).unwrap();

        assert_eq!(value["workspace"]["d_tag"], "ABCDEFGHIJKLMNOPQRSTUV");
        assert_eq!(value["farm_group_id"], "BCDEFGHIJKLMNOPQRSTUVW");
        assert_eq!(value["owner_document_kind"], "FarmTask");
        assert_eq!(value["caption"], "Tomatoes harvested from Patch Y.");
        assert_eq!(value["mime_type"], "image/jpeg");
        assert_eq!(
            value["sha256"],
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        );
        assert_eq!(value["dimensions"]["w"], 1600);
        assert_eq!(value["dimensions"]["h"], 1200);
    }

    fn sample_file_metadata() -> FarmFileMetadata {
        FarmFileMetadata {
            d_tag: "EFGHIJKLMNOPQRSTUVWXYZ".to_string(),
            workspace: FarmWorkspaceRef {
                pubkey: "workspace_pubkey".to_string(),
                d_tag: "ABCDEFGHIJKLMNOPQRSTUV".to_string(),
            },
            farm_group_id: "BCDEFGHIJKLMNOPQRSTUVW".to_string(),
            owner_document_id: "DEFGHIJKLMNOPQRSTUVWXY".to_string(),
            owner_document_kind: FarmCrdtDocumentKind::FarmTask,
            caption: Some("Tomatoes harvested from Patch Y.".to_string()),
            url: "https://media.example.invalid/blob/sha256".to_string(),
            mime_type: "image/jpeg".to_string(),
            sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
            original_sha256: Some(
                "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789".to_string(),
            ),
            size_bytes: Some(123_456),
            dimensions: Some(FarmFileDimensions { w: 1600, h: 1200 }),
            blurhash: Some("LEHV6nWB2yk8pyo0adR*.7kCMdnj".to_string()),
            thumb: Some(FarmFileSource {
                url: "https://media.example.invalid/thumb/sha256".to_string(),
                mime_type: Some("image/jpeg".to_string()),
                dimensions: Some(FarmFileDimensions { w: 320, h: 240 }),
            }),
            image: None,
            alt: Some("Harvested tomatoes in a crate".to_string()),
            fallbacks: vec!["https://fallback.example.invalid/blob/sha256".to_string()],
        }
    }
}
