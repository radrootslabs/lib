#![cfg(feature = "serde_json")]

use radroots_events::{
    calendar::{RadrootsCalendarDateEvent, RadrootsCalendarTimeEvent},
    kinds::{KIND_CALENDAR_DATE_EVENT, KIND_CALENDAR_TIME_EVENT, KIND_POST},
    social::{RadrootsCalendarDateValue, RadrootsCalendarParticipant, RadrootsSocialLocation},
    tags::{
        TAG_D, TAG_D_DAY, TAG_END, TAG_END_TZID, TAG_G, TAG_IMAGE, TAG_LOCATION, TAG_P, TAG_START,
        TAG_START_TZID, TAG_SUMMARY, TAG_TITLE,
    },
};
use radroots_events_codec::{
    calendar::{
        decode::{
            calendar_date_event_from_event, calendar_time_event_from_event, date_data_from_event,
            date_parsed_from_event, time_data_from_event, time_parsed_from_event,
        },
        encode::{
            calendar_date_event_build_tags, calendar_time_event_build_tags, date_to_wire_parts,
            date_to_wire_parts_with_kind, time_to_wire_parts, time_to_wire_parts_with_kind,
        },
    },
    error::{EventEncodeError, EventParseError},
};

const VALID_D_TAG: &str = "CCCCCCCCCCCCCCCCCCCCCA";

fn sample_date_event() -> RadrootsCalendarDateEvent {
    RadrootsCalendarDateEvent {
        d_tag: VALID_D_TAG.to_string(),
        title: "CSA pickup".to_string(),
        start: "2026-06-20".to_string(),
        end: Some("2026-06-21".to_string()),
        days: Some(vec![RadrootsCalendarDateValue {
            value: "2026-06-20".to_string(),
        }]),
        location: Some(RadrootsSocialLocation {
            name: Some("Farm stand".to_string()),
            geohash: Some("c23nb62w20st".to_string()),
        }),
        summary: Some("Weekly pickup".to_string()),
        image: Some("https://media.example.test/calendar.jpg".to_string()),
        participants: Some(vec![RadrootsCalendarParticipant {
            pubkey: "host_pubkey".to_string(),
            relay: Some("wss://relay.example.test".to_string()),
            role: Some("host".to_string()),
        }]),
    }
}

fn sample_time_event() -> RadrootsCalendarTimeEvent {
    RadrootsCalendarTimeEvent {
        d_tag: VALID_D_TAG.to_string(),
        title: "Wash pack shift".to_string(),
        start: 1_781_895_600,
        end: Some(1_781_899_200),
        start_tzid: Some("America/Vancouver".to_string()),
        end_tzid: Some("America/Vancouver".to_string()),
        location: Some(RadrootsSocialLocation {
            name: Some("Pack shed".to_string()),
            geohash: Some("c23nb62w20st".to_string()),
        }),
        summary: Some("Prepare CSA bins".to_string()),
        image: None,
        participants: Some(vec![RadrootsCalendarParticipant {
            pubkey: "crew_pubkey".to_string(),
            relay: None,
            role: Some("participant".to_string()),
        }]),
    }
}

fn has_tag(tags: &[Vec<String>], key: &str, value: &str) -> bool {
    tags.iter().any(|tag| {
        tag.first().map(|entry| entry.as_str()) == Some(key)
            && tag.get(1).map(|entry| entry.as_str()) == Some(value)
    })
}

#[test]
fn calendar_date_event_to_wire_parts_roundtrips_tags() {
    let event = sample_date_event();
    let parts = date_to_wire_parts(&event).unwrap();

    assert_eq!(parts.kind, KIND_CALENDAR_DATE_EVENT);
    assert!(parts.content.is_empty());
    assert!(has_tag(&parts.tags, TAG_D, VALID_D_TAG));
    assert!(has_tag(&parts.tags, TAG_TITLE, "CSA pickup"));
    assert!(has_tag(&parts.tags, TAG_START, "2026-06-20"));
    assert!(has_tag(&parts.tags, TAG_END, "2026-06-21"));
    assert!(has_tag(&parts.tags, TAG_D_DAY, "2026-06-20"));
    assert!(has_tag(&parts.tags, TAG_LOCATION, "Farm stand"));
    assert!(has_tag(&parts.tags, TAG_G, "c23nb62w20st"));
    assert!(has_tag(&parts.tags, TAG_SUMMARY, "Weekly pickup"));
    assert!(has_tag(
        &parts.tags,
        TAG_IMAGE,
        "https://media.example.test/calendar.jpg"
    ));
    assert!(has_tag(&parts.tags, TAG_P, "host_pubkey"));

    let decoded = calendar_date_event_from_event(parts.kind, &parts.tags, &parts.content).unwrap();
    assert_eq!(decoded.d_tag, VALID_D_TAG);
    assert_eq!(decoded.title, "CSA pickup");
    assert_eq!(decoded.start, "2026-06-20");
    assert_eq!(decoded.end.as_deref(), Some("2026-06-21"));
    assert_eq!(decoded.days.as_ref().map(Vec::len), Some(1));
    assert_eq!(
        decoded
            .location
            .as_ref()
            .and_then(|location| location.name.as_deref()),
        Some("Farm stand")
    );
    assert_eq!(decoded.participants.as_ref().map(Vec::len), Some(1));
}

#[test]
fn calendar_time_event_to_wire_parts_roundtrips_tags() {
    let event = sample_time_event();
    let parts = time_to_wire_parts(&event).unwrap();

    assert_eq!(parts.kind, KIND_CALENDAR_TIME_EVENT);
    assert!(parts.content.is_empty());
    assert!(has_tag(&parts.tags, TAG_D, VALID_D_TAG));
    assert!(has_tag(&parts.tags, TAG_TITLE, "Wash pack shift"));
    assert!(has_tag(&parts.tags, TAG_START, "1781895600"));
    assert!(has_tag(&parts.tags, TAG_END, "1781899200"));
    assert!(has_tag(&parts.tags, TAG_START_TZID, "America/Vancouver"));
    assert!(has_tag(&parts.tags, TAG_END_TZID, "America/Vancouver"));
    assert!(has_tag(&parts.tags, TAG_LOCATION, "Pack shed"));
    assert!(has_tag(&parts.tags, TAG_P, "crew_pubkey"));

    let decoded = calendar_time_event_from_event(parts.kind, &parts.tags, &parts.content).unwrap();
    assert_eq!(decoded.d_tag, VALID_D_TAG);
    assert_eq!(decoded.title, "Wash pack shift");
    assert_eq!(decoded.start, 1_781_895_600);
    assert_eq!(decoded.end, Some(1_781_899_200));
    assert_eq!(decoded.start_tzid.as_deref(), Some("America/Vancouver"));
    assert_eq!(decoded.participants.as_ref().map(Vec::len), Some(1));
}

#[test]
fn calendar_codecs_reject_wrong_kind_invalid_dates_and_nonempty_content() {
    assert!(matches!(
        date_to_wire_parts_with_kind(&sample_date_event(), KIND_POST),
        Err(EventEncodeError::InvalidKind(KIND_POST))
    ));
    assert!(matches!(
        time_to_wire_parts_with_kind(&sample_time_event(), KIND_POST),
        Err(EventEncodeError::InvalidKind(KIND_POST))
    ));

    let mut event = sample_date_event();
    event.start = "2026-6-20".to_string();
    assert!(matches!(
        calendar_date_event_build_tags(&event),
        Err(EventEncodeError::InvalidField("start"))
    ));

    let mut event = sample_time_event();
    event.end = Some(event.start - 1);
    assert!(matches!(
        calendar_time_event_build_tags(&event),
        Err(EventEncodeError::InvalidField("end"))
    ));

    let tags = calendar_date_event_build_tags(&sample_date_event()).unwrap();
    assert!(matches!(
        calendar_date_event_from_event(KIND_CALENDAR_DATE_EVENT, &tags, "body"),
        Err(EventParseError::InvalidJson("content"))
    ));

    let mut tags = calendar_date_event_build_tags(&sample_date_event()).unwrap();
    let start = tags
        .iter_mut()
        .find(|tag| tag.first().map(|value| value.as_str()) == Some(TAG_START))
        .expect("start tag");
    start[1] = "bad".to_string();
    assert!(matches!(
        calendar_date_event_from_event(KIND_CALENDAR_DATE_EVENT, &tags, ""),
        Err(EventParseError::InvalidTag(TAG_START))
    ));

    let err = calendar_time_event_from_event(KIND_POST, &tags, "").unwrap_err();
    assert!(matches!(
        err,
        EventParseError::InvalidKind {
            expected: "31923",
            got: KIND_POST
        }
    ));
}

#[test]
fn calendar_wrappers_preserve_event_metadata() {
    let date = sample_date_event();
    let date_parts = date_to_wire_parts(&date).unwrap();
    let date_data = date_data_from_event(
        "date_id".to_string(),
        "author".to_string(),
        7,
        date_parts.kind,
        date_parts.content.clone(),
        date_parts.tags.clone(),
    )
    .unwrap();
    assert_eq!(date_data.kind, KIND_CALENDAR_DATE_EVENT);
    assert_eq!(date_data.data.title, "CSA pickup");

    let date_parsed = date_parsed_from_event(
        "date_id".to_string(),
        "author".to_string(),
        7,
        date_parts.kind,
        date_parts.content,
        date_parts.tags,
        "sig".to_string(),
    )
    .unwrap();
    assert_eq!(date_parsed.event.sig, "sig");

    let time = sample_time_event();
    let time_parts = time_to_wire_parts(&time).unwrap();
    let time_data = time_data_from_event(
        "time_id".to_string(),
        "author".to_string(),
        8,
        time_parts.kind,
        time_parts.content.clone(),
        time_parts.tags.clone(),
    )
    .unwrap();
    assert_eq!(time_data.kind, KIND_CALENDAR_TIME_EVENT);
    assert_eq!(time_data.data.title, "Wash pack shift");

    let time_parsed = time_parsed_from_event(
        "time_id".to_string(),
        "author".to_string(),
        8,
        time_parts.kind,
        time_parts.content,
        time_parts.tags,
        "sig".to_string(),
    )
    .unwrap();
    assert_eq!(time_parsed.event.created_at, 8);
}
