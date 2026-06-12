#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use crate::social::{
    RadrootsCalendarDateValue, RadrootsCalendarParticipant, RadrootsSocialLocation,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsCalendarDateEvent {
    pub d_tag: String,
    pub title: String,
    pub start: String,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub end: Option<String>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub days: Option<Vec<RadrootsCalendarDateValue>>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub location: Option<RadrootsSocialLocation>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub summary: Option<String>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub image: Option<String>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub participants: Option<Vec<RadrootsCalendarParticipant>>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsCalendarTimeEvent {
    pub d_tag: String,
    pub title: String,
    pub start: u64,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub end: Option<u64>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub start_tzid: Option<String>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub end_tzid: Option<String>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub location: Option<RadrootsSocialLocation>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub summary: Option<String>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub image: Option<String>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub participants: Option<Vec<RadrootsCalendarParticipant>>,
}

#[cfg(all(test, feature = "std", feature = "serde"))]
mod tests {
    use super::*;

    #[test]
    fn date_event_represents_all_day_event_fields() {
        let event = RadrootsCalendarDateEvent {
            d_tag: "market-day".to_string(),
            title: "market day".to_string(),
            start: "2026-06-20".to_string(),
            end: None,
            days: Some(vec![RadrootsCalendarDateValue {
                value: "2026-06-20".to_string(),
            }]),
            location: Some(RadrootsSocialLocation {
                name: Some("farm stand".to_string()),
                geohash: Some("c23nb62w20st".to_string()),
            }),
            summary: Some("weekly pickup".to_string()),
            image: None,
            participants: None,
        };

        assert_eq!(event.d_tag, "market-day");
        assert_eq!(event.start, "2026-06-20");
        assert_eq!(event.days.expect("days")[0].value, "2026-06-20");
    }

    #[test]
    fn time_event_represents_timestamped_event_fields() {
        let event = RadrootsCalendarTimeEvent {
            d_tag: "wash-pack".to_string(),
            title: "wash pack shift".to_string(),
            start: 1_781_895_600,
            end: Some(1_781_899_200),
            start_tzid: Some("America/Vancouver".to_string()),
            end_tzid: Some("America/Vancouver".to_string()),
            location: None,
            summary: None,
            image: None,
            participants: Some(vec![RadrootsCalendarParticipant {
                pubkey: "a".repeat(64),
                relay: None,
                role: Some("host".to_string()),
            }]),
        };

        assert_eq!(event.start, 1_781_895_600);
        assert_eq!(event.end, Some(1_781_899_200));
        assert_eq!(event.start_tzid.as_deref(), Some("America/Vancouver"));
        assert_eq!(event.participants.expect("participants").len(), 1);
    }
}
