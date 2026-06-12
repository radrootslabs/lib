#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use crate::social::{
    RadrootsCalendarDateValue, RadrootsCalendarEventFreeBusy, RadrootsCalendarEventRsvpStatus,
    RadrootsCalendarParticipant, RadrootsSocialLocation, RadrootsSocialTarget,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsCalendar {
    pub d_tag: String,
    pub title: String,
    pub events: Vec<RadrootsSocialTarget>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub description: Option<String>,
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
}

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
    pub description: Option<String>,
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
        serde(default, skip_serializing_if = "Vec::is_empty")
    )]
    pub dates: Vec<RadrootsCalendarDateValue>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub description: Option<String>,
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

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsCalendarEventRsvp {
    pub d_tag: String,
    pub event: RadrootsSocialTarget,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub event_id: Option<String>,
    pub status: RadrootsCalendarEventRsvpStatus,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub free_busy: Option<RadrootsCalendarEventFreeBusy>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub note: Option<String>,
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
            description: Some("Farm stand pickup window.".to_string()),
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
            dates: vec![RadrootsCalendarDateValue {
                value: "2026-06-20".to_string(),
            }],
            description: Some("Pack CSA shares before pickup.".to_string()),
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

    #[test]
    fn calendar_collection_represents_event_address_refs() {
        let calendar = RadrootsCalendar {
            d_tag: "farm-calendar".to_string(),
            title: "farm calendar".to_string(),
            events: vec![RadrootsSocialTarget::Address {
                address: "31923:pubkey:wash-pack".to_string(),
                author: None,
                event_kind: Some(31923),
                relays: None,
            }],
            description: Some("Shared farm operations schedule.".to_string()),
            summary: None,
            image: None,
        };

        assert_eq!(calendar.d_tag, "farm-calendar");
        assert_eq!(calendar.events.len(), 1);
        assert!(matches!(
            calendar.events[0],
            RadrootsSocialTarget::Address { .. }
        ));
    }

    #[test]
    fn rsvp_represents_status_and_free_busy_state() {
        let rsvp = RadrootsCalendarEventRsvp {
            d_tag: "rsvp-1".to_string(),
            event: RadrootsSocialTarget::Address {
                address: "31923:pubkey:wash-pack".to_string(),
                author: Some("a".repeat(64)),
                event_kind: Some(31923),
                relays: None,
            },
            event_id: Some("b".repeat(64)),
            status: RadrootsCalendarEventRsvpStatus::Tentative,
            free_busy: Some(RadrootsCalendarEventFreeBusy::Busy),
            note: Some("depends on harvest".to_string()),
            participants: None,
        };

        assert_eq!(rsvp.status, RadrootsCalendarEventRsvpStatus::Tentative);
        assert_eq!(
            rsvp.event_id.as_deref(),
            Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
        );
        assert_eq!(rsvp.free_busy, Some(RadrootsCalendarEventFreeBusy::Busy));
    }
}
