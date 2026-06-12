#[cfg(not(feature = "std"))]
use alloc::{string::ToString, vec::Vec};

use radroots_events::{
    RadrootsNostrEvent,
    calendar::{
        RadrootsCalendar, RadrootsCalendarDateEvent, RadrootsCalendarEventRsvp,
        RadrootsCalendarTimeEvent,
    },
    kinds::{
        KIND_CALENDAR, KIND_CALENDAR_DATE_EVENT, KIND_CALENDAR_EVENT_RSVP, KIND_CALENDAR_TIME_EVENT,
    },
    social::{
        RadrootsCalendarDateValue, RadrootsCalendarEventFreeBusy, RadrootsCalendarEventRsvpStatus,
        RadrootsSocialTarget,
    },
    tags::{
        TAG_A, TAG_D, TAG_D_DAY, TAG_E, TAG_END, TAG_END_TZID, TAG_FREE_BUSY, TAG_IMAGE, TAG_START,
        TAG_START_TZID, TAG_STATUS, TAG_SUMMARY, TAG_TITLE,
    },
};

use crate::d_tag::validate_d_tag_tag;
use crate::error::EventParseError;
use crate::field_helpers::{
    optional_tag_value, parse_address_tag, required_tag_value, tag_values,
    validate_lowercase_hex_64_tag,
};
use crate::parsed::{RadrootsParsedData, RadrootsParsedEvent};
use crate::social_helpers::{
    location_from_tags, participants_from_tags, validate_date_tag, validate_end_after_start,
};

const EXPECTED_DATE_KIND: &str = "31922";
const EXPECTED_TIME_KIND: &str = "31923";
const EXPECTED_CALENDAR_KIND: &str = "31924";
const EXPECTED_RSVP_KIND: &str = "31925";

pub fn calendar_date_event_from_event(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<RadrootsCalendarDateEvent, EventParseError> {
    if kind != KIND_CALENDAR_DATE_EVENT {
        return Err(EventParseError::InvalidKind {
            expected: EXPECTED_DATE_KIND,
            got: kind,
        });
    }
    let d_tag = required_tag_value(tags, TAG_D)?;
    validate_d_tag_tag(&d_tag, TAG_D)?;
    let title = required_tag_value(tags, TAG_TITLE)?;
    let start = required_tag_value(tags, TAG_START)?;
    validate_date_tag(&start, TAG_START)?;
    let end = optional_tag_value(tags, TAG_END)?;
    if let Some(end) = end.as_deref() {
        validate_date_tag(end, TAG_END)?;
        if end < start.as_str() {
            return Err(EventParseError::InvalidTag(TAG_END));
        }
    }
    let days = tag_values(tags, TAG_D_DAY)?
        .into_iter()
        .map(|value| {
            validate_date_tag(&value, TAG_D_DAY)?;
            Ok(RadrootsCalendarDateValue { value })
        })
        .collect::<Result<Vec<_>, EventParseError>>()?;
    Ok(RadrootsCalendarDateEvent {
        d_tag,
        title,
        start,
        description: optional_content(content),
        end,
        days: non_empty_vec(days),
        location: location_from_tags(tags),
        summary: optional_tag_value(tags, TAG_SUMMARY)?,
        image: optional_tag_value(tags, TAG_IMAGE)?,
        participants: participants_from_tags(tags),
    })
}

pub fn calendar_time_event_from_event(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<RadrootsCalendarTimeEvent, EventParseError> {
    if kind != KIND_CALENDAR_TIME_EVENT {
        return Err(EventParseError::InvalidKind {
            expected: EXPECTED_TIME_KIND,
            got: kind,
        });
    }
    let d_tag = required_tag_value(tags, TAG_D)?;
    validate_d_tag_tag(&d_tag, TAG_D)?;
    let title = required_tag_value(tags, TAG_TITLE)?;
    let start = parse_required_u64(tags, TAG_START)?;
    let end = parse_optional_u64(tags, TAG_END)?;
    validate_end_after_start(start, end, TAG_END)
        .map_err(|_| EventParseError::InvalidTag(TAG_END))?;
    let dates = tag_values(tags, TAG_D_DAY)?
        .into_iter()
        .map(|value| {
            validate_date_tag(&value, TAG_D_DAY)?;
            Ok(RadrootsCalendarDateValue { value })
        })
        .collect::<Result<Vec<_>, EventParseError>>()?;
    if dates.is_empty() {
        return Err(EventParseError::MissingTag(TAG_D_DAY));
    }
    Ok(RadrootsCalendarTimeEvent {
        d_tag,
        title,
        start,
        dates,
        description: optional_content(content),
        end,
        start_tzid: optional_tag_value(tags, TAG_START_TZID)?,
        end_tzid: optional_tag_value(tags, TAG_END_TZID)?,
        location: location_from_tags(tags),
        summary: optional_tag_value(tags, TAG_SUMMARY)?,
        image: optional_tag_value(tags, TAG_IMAGE)?,
        participants: participants_from_tags(tags),
    })
}

pub fn calendar_from_event(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<RadrootsCalendar, EventParseError> {
    if kind != KIND_CALENDAR {
        return Err(EventParseError::InvalidKind {
            expected: EXPECTED_CALENDAR_KIND,
            got: kind,
        });
    }
    let d_tag = required_tag_value(tags, TAG_D)?;
    validate_d_tag_tag(&d_tag, TAG_D)?;
    let title = required_tag_value(tags, TAG_TITLE)?;
    let events = calendar_event_targets_from_tags(tags)?;
    if events.is_empty() {
        return Err(EventParseError::MissingTag(TAG_A));
    }
    Ok(RadrootsCalendar {
        d_tag,
        title,
        events,
        description: optional_content(content),
        summary: optional_tag_value(tags, TAG_SUMMARY)?,
        image: optional_tag_value(tags, TAG_IMAGE)?,
    })
}

pub fn rsvp_from_event(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<RadrootsCalendarEventRsvp, EventParseError> {
    if kind != KIND_CALENDAR_EVENT_RSVP {
        return Err(EventParseError::InvalidKind {
            expected: EXPECTED_RSVP_KIND,
            got: kind,
        });
    }
    let d_tag = required_tag_value(tags, TAG_D)?;
    validate_d_tag_tag(&d_tag, TAG_D)?;
    let event = calendar_event_target_from_required_tag(tags)?;
    let event_id = optional_tag_value(tags, TAG_E)?;
    if let Some(event_id) = event_id.as_deref() {
        validate_lowercase_hex_64_tag(event_id, TAG_E)?;
    }
    let status = parse_rsvp_status(&required_tag_value(tags, TAG_STATUS)?)?;
    let free_busy = optional_tag_value(tags, TAG_FREE_BUSY)?
        .map(|value| parse_free_busy(&value))
        .transpose()?;
    Ok(RadrootsCalendarEventRsvp {
        d_tag,
        event,
        event_id,
        status,
        free_busy,
        note: if content.is_empty() {
            None
        } else {
            Some(content.to_string())
        },
        participants: participants_from_tags(tags),
    })
}

pub fn date_data_from_event(
    id: String,
    author: String,
    published_at: u32,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
) -> Result<RadrootsParsedData<RadrootsCalendarDateEvent>, EventParseError> {
    let event = calendar_date_event_from_event(kind, &tags, &content)?;
    Ok(RadrootsParsedData::new(
        id,
        author,
        published_at,
        kind,
        event,
    ))
}

pub fn time_data_from_event(
    id: String,
    author: String,
    published_at: u32,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
) -> Result<RadrootsParsedData<RadrootsCalendarTimeEvent>, EventParseError> {
    let event = calendar_time_event_from_event(kind, &tags, &content)?;
    Ok(RadrootsParsedData::new(
        id,
        author,
        published_at,
        kind,
        event,
    ))
}

pub fn calendar_data_from_event(
    id: String,
    author: String,
    published_at: u32,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
) -> Result<RadrootsParsedData<RadrootsCalendar>, EventParseError> {
    let calendar = calendar_from_event(kind, &tags, &content)?;
    Ok(RadrootsParsedData::new(
        id,
        author,
        published_at,
        kind,
        calendar,
    ))
}

pub fn rsvp_data_from_event(
    id: String,
    author: String,
    published_at: u32,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
) -> Result<RadrootsParsedData<RadrootsCalendarEventRsvp>, EventParseError> {
    let rsvp = rsvp_from_event(kind, &tags, &content)?;
    Ok(RadrootsParsedData::new(
        id,
        author,
        published_at,
        kind,
        rsvp,
    ))
}

pub fn date_parsed_from_event(
    id: String,
    author: String,
    published_at: u32,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
    sig: String,
) -> Result<RadrootsParsedEvent<RadrootsCalendarDateEvent>, EventParseError> {
    let data = date_data_from_event(
        id.clone(),
        author.clone(),
        published_at,
        kind,
        content.clone(),
        tags.clone(),
    )?;
    Ok(RadrootsParsedEvent {
        event: RadrootsNostrEvent {
            id,
            author,
            created_at: published_at,
            kind,
            content,
            tags,
            sig,
        },
        data,
    })
}

pub fn time_parsed_from_event(
    id: String,
    author: String,
    published_at: u32,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
    sig: String,
) -> Result<RadrootsParsedEvent<RadrootsCalendarTimeEvent>, EventParseError> {
    let data = time_data_from_event(
        id.clone(),
        author.clone(),
        published_at,
        kind,
        content.clone(),
        tags.clone(),
    )?;
    Ok(RadrootsParsedEvent {
        event: RadrootsNostrEvent {
            id,
            author,
            created_at: published_at,
            kind,
            content,
            tags,
            sig,
        },
        data,
    })
}

pub fn calendar_parsed_from_event(
    id: String,
    author: String,
    published_at: u32,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
    sig: String,
) -> Result<RadrootsParsedEvent<RadrootsCalendar>, EventParseError> {
    let data = calendar_data_from_event(
        id.clone(),
        author.clone(),
        published_at,
        kind,
        content.clone(),
        tags.clone(),
    )?;
    Ok(RadrootsParsedEvent {
        event: RadrootsNostrEvent {
            id,
            author,
            created_at: published_at,
            kind,
            content,
            tags,
            sig,
        },
        data,
    })
}

pub fn rsvp_parsed_from_event(
    id: String,
    author: String,
    published_at: u32,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
    sig: String,
) -> Result<RadrootsParsedEvent<RadrootsCalendarEventRsvp>, EventParseError> {
    let data = rsvp_data_from_event(
        id.clone(),
        author.clone(),
        published_at,
        kind,
        content.clone(),
        tags.clone(),
    )?;
    Ok(RadrootsParsedEvent {
        event: RadrootsNostrEvent {
            id,
            author,
            created_at: published_at,
            kind,
            content,
            tags,
            sig,
        },
        data,
    })
}

fn parse_required_u64(tags: &[Vec<String>], key: &'static str) -> Result<u64, EventParseError> {
    required_tag_value(tags, key)?
        .parse::<u64>()
        .map_err(|err| EventParseError::InvalidNumber(key, err))
}

fn parse_optional_u64(
    tags: &[Vec<String>],
    key: &'static str,
) -> Result<Option<u64>, EventParseError> {
    optional_tag_value(tags, key)?
        .map(|value| {
            value
                .parse::<u64>()
                .map_err(|err| EventParseError::InvalidNumber(key, err))
        })
        .transpose()
}

fn non_empty_vec<T>(values: Vec<T>) -> Option<Vec<T>> {
    if values.is_empty() {
        None
    } else {
        Some(values)
    }
}

fn optional_content(content: &str) -> Option<String> {
    if content.is_empty() {
        None
    } else {
        Some(content.to_string())
    }
}

fn calendar_event_targets_from_tags(
    tags: &[Vec<String>],
) -> Result<Vec<RadrootsSocialTarget>, EventParseError> {
    tags.iter()
        .filter(|tag| tag.first().map(|value| value.as_str()) == Some(TAG_A))
        .map(|tag| calendar_event_target_from_tag(tag))
        .collect()
}

fn calendar_event_target_from_required_tag(
    tags: &[Vec<String>],
) -> Result<RadrootsSocialTarget, EventParseError> {
    let tag = tags
        .iter()
        .find(|tag| tag.first().map(|value| value.as_str()) == Some(TAG_A))
        .ok_or(EventParseError::MissingTag(TAG_A))?;
    calendar_event_target_from_tag(tag)
}

fn calendar_event_target_from_tag(tag: &[String]) -> Result<RadrootsSocialTarget, EventParseError> {
    let value = tag
        .get(1)
        .cloned()
        .ok_or(EventParseError::InvalidTag(TAG_A))?;
    let address = parse_address_tag(&value, TAG_A)?;
    if !is_calendar_event_kind(address.kind) {
        return Err(EventParseError::InvalidTag(TAG_A));
    }
    Ok(RadrootsSocialTarget::Address {
        address: value,
        author: Some(address.pubkey),
        event_kind: Some(address.kind),
        relays: relays_from_tag(tag, 2),
    })
}

fn relays_from_tag(tag: &[String], start: usize) -> Option<Vec<String>> {
    let relays = tag
        .iter()
        .skip(start)
        .filter(|value| !value.trim().is_empty())
        .cloned()
        .collect::<Vec<_>>();
    if relays.is_empty() {
        None
    } else {
        Some(relays)
    }
}

fn is_calendar_event_kind(kind: u32) -> bool {
    matches!(kind, KIND_CALENDAR_DATE_EVENT | KIND_CALENDAR_TIME_EVENT)
}

fn parse_rsvp_status(value: &str) -> Result<RadrootsCalendarEventRsvpStatus, EventParseError> {
    match value {
        "accepted" => Ok(RadrootsCalendarEventRsvpStatus::Accepted),
        "declined" => Ok(RadrootsCalendarEventRsvpStatus::Declined),
        "tentative" => Ok(RadrootsCalendarEventRsvpStatus::Tentative),
        _ => Err(EventParseError::InvalidTag(TAG_STATUS)),
    }
}

fn parse_free_busy(value: &str) -> Result<RadrootsCalendarEventFreeBusy, EventParseError> {
    match value {
        "free" => Ok(RadrootsCalendarEventFreeBusy::Free),
        "busy" => Ok(RadrootsCalendarEventFreeBusy::Busy),
        _ => Err(EventParseError::InvalidTag(TAG_FREE_BUSY)),
    }
}
