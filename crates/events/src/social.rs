use crate::farm::RadrootsFarmRef;

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case", tag = "kind"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsSocialTarget {
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

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct RadrootsSocialFarmAnchor {
    pub farm: RadrootsFarmRef,
    pub relays: Option<Vec<String>>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RadrootsSocialLocation {
    pub name: Option<String>,
    pub geohash: Option<String>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RadrootsSocialMediaDimensions {
    pub width: u32,
    pub height: u32,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RadrootsSocialMediaThumbnail {
    pub url: String,
    pub dimensions: Option<RadrootsSocialMediaDimensions>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RadrootsSocialMediaMetadata {
    pub url: Option<String>,
    pub mime_type: Option<String>,
    pub sha256: Option<String>,
    pub original_sha256: Option<String>,
    pub size: Option<u64>,
    pub dimensions: Option<RadrootsSocialMediaDimensions>,
    pub blurhash: Option<String>,
    pub thumbnails: Option<Vec<RadrootsSocialMediaThumbnail>>,
    pub image: Option<String>,
    pub summary: Option<String>,
    pub alt: Option<String>,
    pub fallback: Option<String>,
    pub magnet: Option<String>,
    pub content_hashes: Option<Vec<String>>,
    pub services: Option<Vec<String>>,
    pub imeta: Option<Vec<Vec<String>>>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RadrootsCalendarParticipant {
    pub pubkey: String,
    pub relay: Option<String>,
    pub role: Option<String>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RadrootsCalendarDateValue {
    pub value: String,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsCalendarEventRsvpStatus {
    Accepted,
    Declined,
    Tentative,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsCalendarEventFreeBusy {
    Free,
    Busy,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsReportType {
    Nudity,
    Malware,
    Profanity,
    Illegal,
    Spam,
    Impersonation,
    Other,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RadrootsReportFileTarget {
    pub sha256: Option<String>,
    pub url: Option<String>,
    pub magnet: Option<String>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsReportTarget {
    pub reported_pubkey: String,
    pub event: Option<RadrootsSocialTarget>,
    pub file: Option<RadrootsReportFileTarget>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supports_nip22_target_shapes() {
        let event = RadrootsSocialTarget::Event {
            id: "a".repeat(64),
            author: Some("b".repeat(64)),
            event_kind: Some(30023),
            relays: Some(vec!["wss://relay.example".to_string()]),
        };
        let address = RadrootsSocialTarget::Address {
            address: "30023:pubkey:d-tag".to_string(),
            author: None,
            event_kind: Some(30023),
            relays: None,
        };
        let external = RadrootsSocialTarget::External {
            id: "https://example.test/object".to_string(),
            external_kind: "web".to_string(),
            hint: None,
        };

        assert!(matches!(event, RadrootsSocialTarget::Event { .. }));
        assert!(matches!(address, RadrootsSocialTarget::Address { .. }));
        assert!(matches!(external, RadrootsSocialTarget::External { .. }));
    }

    #[test]
    fn defaults_media_and_farm_anchor_primitives() {
        let media = RadrootsSocialMediaMetadata::default();
        assert!(media.url.is_none());
        assert!(media.content_hashes.is_none());
        assert!(media.services.is_none());

        let anchor = RadrootsSocialFarmAnchor::default();
        assert!(anchor.farm.pubkey.is_empty());
        assert!(anchor.farm.d_tag.is_empty());
        assert!(anchor.relays.is_none());
    }

    #[test]
    fn exposes_calendar_and_report_enums() {
        assert_eq!(
            RadrootsCalendarEventRsvpStatus::Accepted,
            RadrootsCalendarEventRsvpStatus::Accepted
        );
        assert_eq!(
            RadrootsCalendarEventFreeBusy::Busy,
            RadrootsCalendarEventFreeBusy::Busy
        );
        assert_eq!(RadrootsReportType::Spam, RadrootsReportType::Spam);
    }
}
