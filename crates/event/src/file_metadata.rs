#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use crate::social::{RadrootsSocialMediaDimensions, RadrootsSocialMediaThumbnail};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsFileMetadata {
    pub url: String,
    pub mime_type: String,
    pub sha256: String,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub original_sha256: Option<String>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub size: Option<u64>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub dimensions: Option<RadrootsSocialMediaDimensions>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub blurhash: Option<String>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub thumbnails: Option<Vec<RadrootsSocialMediaThumbnail>>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub summary: Option<String>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub alt: Option<String>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub fallback: Option<String>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub magnet: Option<String>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub content_hashes: Option<Vec<String>>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub services: Option<Vec<String>>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub content: Option<String>,
}

#[cfg(all(test, feature = "std", feature = "serde"))]
mod tests {
    use super::*;

    #[test]
    fn file_metadata_represents_required_nip94_fields() {
        let metadata = RadrootsFileMetadata {
            url: "https://example.test/file.jpg".to_string(),
            mime_type: "image/jpeg".to_string(),
            sha256: "a".repeat(64),
            original_sha256: Some("b".repeat(64)),
            size: Some(1024),
            dimensions: Some(RadrootsSocialMediaDimensions {
                width: 640,
                height: 480,
            }),
            blurhash: None,
            thumbnails: None,
            summary: Some("field image".to_string()),
            alt: Some("rows of lettuce".to_string()),
            fallback: None,
            magnet: Some("magnet:?xt=urn:btih:abc".to_string()),
            content_hashes: Some(vec!["sha256:a".to_string()]),
            services: Some(vec!["https://media.example.test".to_string()]),
            content: Some("caption".to_string()),
        };

        assert_eq!(metadata.mime_type, "image/jpeg");
        assert_eq!(metadata.sha256.len(), 64);
        assert_eq!(metadata.dimensions.expect("dimensions").width, 640);
        assert!(metadata.magnet.expect("magnet").starts_with("magnet:"));
        assert_eq!(metadata.services.expect("services").len(), 1);
    }
}
