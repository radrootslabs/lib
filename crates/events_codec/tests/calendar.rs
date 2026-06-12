#![cfg(feature = "serde_json")]

use radroots_events::{
    calendar::{
        RadrootsCalendar, RadrootsCalendarDateEvent, RadrootsCalendarEventRsvp,
        RadrootsCalendarTimeEvent,
    },
    kinds::{
        KIND_ARTICLE, KIND_CALENDAR, KIND_CALENDAR_DATE_EVENT, KIND_CALENDAR_EVENT_RSVP,
        KIND_CALENDAR_TIME_EVENT, KIND_POST,
    },
    social::{
        RadrootsCalendarDateValue, RadrootsCalendarEventFreeBusy, RadrootsCalendarEventRsvpStatus,
        RadrootsCalendarParticipant, RadrootsSocialLocation, RadrootsSocialTarget,
    },
    tags::{
        TAG_A, TAG_D, TAG_D_DAY, TAG_E, TAG_END, TAG_END_TZID, TAG_FREE_BUSY, TAG_G, TAG_IMAGE,
        TAG_LOCATION, TAG_P, TAG_START, TAG_START_TZID, TAG_STATUS, TAG_SUMMARY, TAG_TITLE,
    },
};
use radroots_events_codec::{
    calendar::{
        decode::{
            calendar_data_from_event, calendar_date_event_from_event, calendar_from_event,
            calendar_parsed_from_event, calendar_time_event_from_event, date_data_from_event,
            date_parsed_from_event, rsvp_data_from_event, rsvp_from_event, rsvp_parsed_from_event,
            time_data_from_event, time_parsed_from_event,
        },
        encode::{
            calendar_collection_build_tags, calendar_date_event_build_tags,
            calendar_time_event_build_tags, calendar_to_wire_parts,
            calendar_to_wire_parts_with_kind, date_to_wire_parts, date_to_wire_parts_with_kind,
            rsvp_build_tags, rsvp_to_wire_parts, rsvp_to_wire_parts_with_kind, time_to_wire_parts,
            time_to_wire_parts_with_kind,
        },
    },
    error::{EventEncodeError, EventParseError},
};

const VALID_D_TAG: &str = "CCCCCCCCCCCCCCCCCCCCCA";
const EVENT_ID: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const EVENT_AUTHOR: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const EVENT_D_TAG: &str = "EEEEEEEEEEEEEEEEEEEEEA";

fn sample_date_event() -> RadrootsCalendarDateEvent {
    RadrootsCalendarDateEvent {
        d_tag: VALID_D_TAG.to_string(),
        title: "CSA pickup".to_string(),
        start: "2026-06-20".to_string(),
        description: Some("Bring clean bins to the farm stand.".to_string()),
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
        dates: vec![RadrootsCalendarDateValue {
            value: "2026-06-20".to_string(),
        }],
        description: Some("Prepare CSA bins before pickup.".to_string()),
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

fn sample_calendar_collection() -> RadrootsCalendar {
    RadrootsCalendar {
        d_tag: VALID_D_TAG.to_string(),
        title: "Farm calendar".to_string(),
        events: vec![RadrootsSocialTarget::Address {
            address: format!("{KIND_CALENDAR_TIME_EVENT}:{EVENT_AUTHOR}:{EVENT_D_TAG}"),
            author: Some(EVENT_AUTHOR.to_string()),
            event_kind: Some(KIND_CALENDAR_TIME_EVENT),
            relays: Some(vec!["wss://relay.example.test".to_string()]),
        }],
        description: Some("Shared schedule for farm operations.".to_string()),
        summary: Some("CSA and harvest schedule".to_string()),
        image: Some("https://media.example.test/calendar.jpg".to_string()),
    }
}

fn sample_rsvp() -> RadrootsCalendarEventRsvp {
    RadrootsCalendarEventRsvp {
        d_tag: VALID_D_TAG.to_string(),
        event: RadrootsSocialTarget::Address {
            address: format!("{KIND_CALENDAR_TIME_EVENT}:{EVENT_AUTHOR}:{EVENT_D_TAG}"),
            author: Some(EVENT_AUTHOR.to_string()),
            event_kind: Some(KIND_CALENDAR_TIME_EVENT),
            relays: Some(vec!["wss://relay.example.test".to_string()]),
        },
        event_id: Some(EVENT_ID.to_string()),
        status: RadrootsCalendarEventRsvpStatus::Accepted,
        free_busy: Some(RadrootsCalendarEventFreeBusy::Busy),
        note: Some("I can attend after harvest".to_string()),
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
    assert_eq!(parts.content, "Bring clean bins to the farm stand.");
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
    assert_eq!(
        decoded.description.as_deref(),
        Some("Bring clean bins to the farm stand.")
    );
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
    assert_eq!(parts.content, "Prepare CSA bins before pickup.");
    assert!(has_tag(&parts.tags, TAG_D, VALID_D_TAG));
    assert!(has_tag(&parts.tags, TAG_TITLE, "Wash pack shift"));
    assert!(has_tag(&parts.tags, TAG_START, "1781895600"));
    assert!(has_tag(&parts.tags, TAG_D_DAY, "2026-06-20"));
    assert!(has_tag(&parts.tags, TAG_END, "1781899200"));
    assert!(has_tag(&parts.tags, TAG_START_TZID, "America/Vancouver"));
    assert!(has_tag(&parts.tags, TAG_END_TZID, "America/Vancouver"));
    assert!(has_tag(&parts.tags, TAG_LOCATION, "Pack shed"));
    assert!(has_tag(&parts.tags, TAG_P, "crew_pubkey"));

    let decoded = calendar_time_event_from_event(parts.kind, &parts.tags, &parts.content).unwrap();
    assert_eq!(decoded.d_tag, VALID_D_TAG);
    assert_eq!(decoded.title, "Wash pack shift");
    assert_eq!(
        decoded.description.as_deref(),
        Some("Prepare CSA bins before pickup.")
    );
    assert_eq!(decoded.start, 1_781_895_600);
    assert_eq!(decoded.dates.len(), 1);
    assert_eq!(decoded.end, Some(1_781_899_200));
    assert_eq!(decoded.start_tzid.as_deref(), Some("America/Vancouver"));
    assert_eq!(decoded.participants.as_ref().map(Vec::len), Some(1));
}

#[test]
fn calendar_collection_to_wire_parts_roundtrips_event_addresses() {
    let calendar = sample_calendar_collection();
    let parts = calendar_to_wire_parts(&calendar).unwrap();

    assert_eq!(parts.kind, KIND_CALENDAR);
    assert_eq!(parts.content, "Shared schedule for farm operations.");
    assert!(has_tag(&parts.tags, TAG_D, VALID_D_TAG));
    assert!(has_tag(&parts.tags, TAG_TITLE, "Farm calendar"));
    assert!(has_tag(
        &parts.tags,
        TAG_A,
        format!("{KIND_CALENDAR_TIME_EVENT}:{EVENT_AUTHOR}:{EVENT_D_TAG}").as_str()
    ));

    let decoded = calendar_from_event(parts.kind, &parts.tags, &parts.content).unwrap();
    assert_eq!(decoded.d_tag, VALID_D_TAG);
    assert_eq!(decoded.title, "Farm calendar");
    assert_eq!(
        decoded.description.as_deref(),
        Some("Shared schedule for farm operations.")
    );
    assert_eq!(decoded.events.len(), 1);
    assert!(matches!(
        decoded.events[0],
        RadrootsSocialTarget::Address {
            event_kind: Some(KIND_CALENDAR_TIME_EVENT),
            ..
        }
    ));
}

#[test]
fn calendar_rsvp_to_wire_parts_roundtrips_status_event_id_and_participants() {
    let rsvp = sample_rsvp();
    let parts = rsvp_to_wire_parts(&rsvp).unwrap();

    assert_eq!(parts.kind, KIND_CALENDAR_EVENT_RSVP);
    assert_eq!(parts.content, "I can attend after harvest");
    assert!(has_tag(&parts.tags, TAG_D, VALID_D_TAG));
    assert!(has_tag(
        &parts.tags,
        TAG_A,
        format!("{KIND_CALENDAR_TIME_EVENT}:{EVENT_AUTHOR}:{EVENT_D_TAG}").as_str()
    ));
    assert!(has_tag(&parts.tags, TAG_E, EVENT_ID));
    assert!(has_tag(&parts.tags, TAG_STATUS, "accepted"));
    assert!(has_tag(&parts.tags, TAG_FREE_BUSY, "busy"));
    assert!(has_tag(&parts.tags, TAG_P, "crew_pubkey"));

    let decoded = rsvp_from_event(parts.kind, &parts.tags, &parts.content).unwrap();
    assert_eq!(decoded.event_id.as_deref(), Some(EVENT_ID));
    assert_eq!(decoded.status, RadrootsCalendarEventRsvpStatus::Accepted);
    assert_eq!(decoded.free_busy, Some(RadrootsCalendarEventFreeBusy::Busy));
    assert_eq!(decoded.note.as_deref(), Some("I can attend after harvest"));
    assert_eq!(decoded.participants.as_ref().map(Vec::len), Some(1));
}

#[test]
fn calendar_codecs_reject_wrong_kind_invalid_dates_and_missing_time_dates() {
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

    let mut event = sample_time_event();
    event.dates.clear();
    assert!(matches!(
        calendar_time_event_build_tags(&event),
        Err(EventEncodeError::EmptyRequiredField("dates"))
    ));

    let tags = calendar_date_event_build_tags(&sample_date_event()).unwrap();
    let decoded = calendar_date_event_from_event(KIND_CALENDAR_DATE_EVENT, &tags, "body").unwrap();
    assert_eq!(decoded.description.as_deref(), Some("body"));

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

    let mut tags = calendar_time_event_build_tags(&sample_time_event()).unwrap();
    tags.retain(|tag| tag.first().map(|value| value.as_str()) != Some(TAG_D_DAY));
    assert!(matches!(
        calendar_time_event_from_event(KIND_CALENDAR_TIME_EVENT, &tags, ""),
        Err(EventParseError::MissingTag(TAG_D_DAY))
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
fn calendar_collection_and_rsvp_reject_missing_or_invalid_required_tags() {
    assert!(matches!(
        calendar_to_wire_parts_with_kind(&sample_calendar_collection(), KIND_POST),
        Err(EventEncodeError::InvalidKind(KIND_POST))
    ));
    assert!(matches!(
        rsvp_to_wire_parts_with_kind(&sample_rsvp(), KIND_POST),
        Err(EventEncodeError::InvalidKind(KIND_POST))
    ));

    let mut calendar = sample_calendar_collection();
    calendar.events.clear();
    assert!(matches!(
        calendar_collection_build_tags(&calendar),
        Err(EventEncodeError::EmptyRequiredField("events"))
    ));

    let mut rsvp = sample_rsvp();
    if let RadrootsSocialTarget::Address { event_kind, .. } = &mut rsvp.event {
        *event_kind = Some(KIND_ARTICLE);
    }
    assert!(matches!(
        rsvp_build_tags(&rsvp),
        Err(EventEncodeError::InvalidField("event"))
    ));

    let mut tags = calendar_collection_build_tags(&sample_calendar_collection()).unwrap();
    tags.retain(|tag| tag.first().map(|value| value.as_str()) != Some(TAG_A));
    assert!(matches!(
        calendar_from_event(KIND_CALENDAR, &tags, ""),
        Err(EventParseError::MissingTag(TAG_A))
    ));

    let mut tags = rsvp_build_tags(&sample_rsvp()).unwrap();
    let status = tags
        .iter_mut()
        .find(|tag| tag.first().map(|value| value.as_str()) == Some(TAG_STATUS))
        .expect("status tag");
    status[1] = "maybe".to_string();
    assert!(matches!(
        rsvp_from_event(KIND_CALENDAR_EVENT_RSVP, &tags, ""),
        Err(EventParseError::InvalidTag(TAG_STATUS))
    ));

    let mut tags = rsvp_build_tags(&sample_rsvp()).unwrap();
    let free_busy = tags
        .iter_mut()
        .find(|tag| tag.first().map(|value| value.as_str()) == Some(TAG_FREE_BUSY))
        .expect("fb tag");
    free_busy[1] = "unknown".to_string();
    assert!(matches!(
        rsvp_from_event(KIND_CALENDAR_EVENT_RSVP, &tags, ""),
        Err(EventParseError::InvalidTag(TAG_FREE_BUSY))
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

    let calendar = sample_calendar_collection();
    let calendar_parts = calendar_to_wire_parts(&calendar).unwrap();
    let calendar_data = calendar_data_from_event(
        "calendar_id".to_string(),
        "author".to_string(),
        9,
        calendar_parts.kind,
        calendar_parts.content.clone(),
        calendar_parts.tags.clone(),
    )
    .unwrap();
    assert_eq!(calendar_data.kind, KIND_CALENDAR);
    assert_eq!(calendar_data.data.title, "Farm calendar");

    let calendar_parsed = calendar_parsed_from_event(
        "calendar_id".to_string(),
        "author".to_string(),
        9,
        calendar_parts.kind,
        calendar_parts.content,
        calendar_parts.tags,
        "sig".to_string(),
    )
    .unwrap();
    assert_eq!(calendar_parsed.event.sig, "sig");

    let rsvp = sample_rsvp();
    let rsvp_parts = rsvp_to_wire_parts(&rsvp).unwrap();
    let rsvp_data = rsvp_data_from_event(
        "rsvp_id".to_string(),
        "author".to_string(),
        10,
        rsvp_parts.kind,
        rsvp_parts.content.clone(),
        rsvp_parts.tags.clone(),
    )
    .unwrap();
    assert_eq!(rsvp_data.kind, KIND_CALENDAR_EVENT_RSVP);
    assert_eq!(rsvp_data.data.event_id.as_deref(), Some(EVENT_ID));

    let rsvp_parsed = rsvp_parsed_from_event(
        "rsvp_id".to_string(),
        "author".to_string(),
        10,
        rsvp_parts.kind,
        rsvp_parts.content,
        rsvp_parts.tags,
        "sig".to_string(),
    )
    .unwrap();
    assert_eq!(rsvp_parsed.event.created_at, 10);
}
