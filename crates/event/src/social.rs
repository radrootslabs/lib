//! Social graph, messaging, access, and job event models.

use crate::farm::FarmRef;

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(all(test, feature = "std"), derive(dto_bindgen::Dto))]
#[cfg_attr(
    any(feature = "serde", test),
    serde(rename_all = "snake_case", tag = "kind")
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SocialTarget {
    Event {
        id: String,
        author: Option<String>,
        event_kind: Option<u32>,
        relays: Option<Vec<String>>,
    },
    Address {
        address: String,
        author: Option<String>,
        event_kind: Option<u32>,
        relays: Option<Vec<String>>,
    },
    External {
        id: String,
        external_kind: String,
        hint: Option<String>,
    },
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(all(test, feature = "std"), derive(dto_bindgen::Dto))]
#[derive(Clone, Debug, Default)]
pub struct SocialFarmAnchor {
    pub farm: FarmRef,
    pub relays: Option<Vec<String>>,
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(all(test, feature = "std"), derive(dto_bindgen::Dto))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SocialLocation {
    pub name: Option<String>,
    pub geohash: Option<String>,
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(all(test, feature = "std"), derive(dto_bindgen::Dto))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SocialMediaDimensions {
    pub width: u32,
    pub height: u32,
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(all(test, feature = "std"), derive(dto_bindgen::Dto))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SocialMediaThumbnail {
    pub url: String,
    pub dimensions: Option<SocialMediaDimensions>,
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(all(test, feature = "std"), derive(dto_bindgen::Dto))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SocialMediaMetadata {
    pub url: Option<String>,
    pub mime_type: Option<String>,
    pub sha256: Option<String>,
    pub original_sha256: Option<String>,
    #[cfg_attr(all(test, feature = "std"), dto(int = "json_string"))]
    pub size: Option<u64>,
    pub dimensions: Option<SocialMediaDimensions>,
    pub blurhash: Option<String>,
    pub thumbnails: Option<Vec<SocialMediaThumbnail>>,
    pub image: Option<String>,
    pub summary: Option<String>,
    pub alt: Option<String>,
    pub fallback: Option<String>,
    pub magnet: Option<String>,
    pub content_hashes: Option<Vec<String>>,
    pub services: Option<Vec<String>>,
    pub imeta: Option<Vec<Vec<String>>>,
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(any(feature = "serde", test), serde(rename_all = "snake_case"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReportType {
    Nudity,
    Malware,
    Profanity,
    Illegal,
    Spam,
    Impersonation,
    Other,
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReportFileTarget {
    pub sha256: Option<String>,
    pub url: Option<String>,
    pub magnet: Option<String>,
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReportTarget {
    pub reported_pubkey: String,
    pub event: Option<SocialTarget>,
    pub file: Option<ReportFileTarget>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supports_nip22_target_shapes() {
        let event = SocialTarget::Event {
            id: "a".repeat(64),
            author: Some(crate::test_valid_hex_64('b')),
            event_kind: Some(30023),
            relays: Some(vec!["wss://relay.example".to_string()]),
        };
        let address = SocialTarget::Address {
            address: "30023:pubkey:d-tag".to_string(),
            author: None,
            event_kind: Some(30023),
            relays: None,
        };
        let external = SocialTarget::External {
            id: "https://example.test/object".to_string(),
            external_kind: "web".to_string(),
            hint: None,
        };

        assert!(matches!(event, SocialTarget::Event { .. }));
        assert!(matches!(address, SocialTarget::Address { .. }));
        assert!(matches!(external, SocialTarget::External { .. }));
    }

    #[test]
    fn defaults_media_and_farm_anchor_primitives() {
        let media = SocialMediaMetadata::default();
        assert!(media.url.is_none());
        assert!(media.content_hashes.is_none());
        assert!(media.services.is_none());

        let anchor = SocialFarmAnchor::default();
        assert!(anchor.farm.pubkey.is_empty());
        assert!(anchor.farm.d_tag.is_empty());
        assert!(anchor.relays.is_none());
    }

    #[test]
    fn exposes_report_enums() {
        assert_eq!(ReportType::Spam, ReportType::Spam);
    }
}
#[path = "app_data.rs"]
pub mod app_data;
#[path = "follow.rs"]
pub mod follow;
#[path = "geochat.rs"]
pub mod geochat;
#[path = "gift_wrap.rs"]
pub mod gift_wrap;
#[path = "group.rs"]
pub mod group;
#[path = "http_auth.rs"]
pub mod http_auth;
#[path = "job.rs"]
pub mod job;
#[path = "job_feedback.rs"]
pub mod job_feedback;
#[path = "job_request.rs"]
pub mod job_request;
#[path = "job_result.rs"]
pub mod job_result;
#[path = "list.rs"]
pub mod list;
#[path = "list_set.rs"]
pub mod list_set;
#[path = "message.rs"]
pub mod message;
#[path = "message_file.rs"]
pub mod message_file;
#[path = "relay_auth.rs"]
pub mod relay_auth;
#[path = "relay_document.rs"]
pub mod relay_document;
#[path = "seal.rs"]
pub mod seal;
