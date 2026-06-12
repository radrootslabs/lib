#[cfg(not(feature = "std"))]
use alloc::{string::ToString, vec::Vec};

use radroots_events::{
    RadrootsNostrEvent,
    calendar::{RadrootsCalendarDateEvent, RadrootsCalendarTimeEvent},
    kinds::{KIND_CALENDAR_DATE_EVENT, KIND_CALENDAR_TIME_EVENT},
    social::RadrootsCalendarDateValue,
    tags::{
        TAG_D, TAG_D_DAY, TAG_END, TAG_END_TZID, TAG_IMAGE, TAG_START, TAG_START_TZID, TAG_SUMMARY,
        TAG_TITLE,
    },
};

use crate::d_tag::validate_d_tag_tag;
use crate::error::EventParseError;
use crate::field_helpers::{optional_tag_value, required_tag_value, tag_values};
use crate::parsed::{RadrootsParsedData, RadrootsParsedEvent};
use crate::social_helpers::{
    location_from_tags, participants_from_tags, validate_date_tag, validate_end_after_start,
};

const EXPECTED_DATE_KIND: &str = "31922";
const EXPECTED_TIME_KIND: &str = "31923";

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
    if !content.is_empty() {
        return Err(EventParseError::InvalidJson("content"));
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
    if !content.is_empty() {
        return Err(EventParseError::InvalidJson("content"));
    }
    let d_tag = required_tag_value(tags, TAG_D)?;
    validate_d_tag_tag(&d_tag, TAG_D)?;
    let title = required_tag_value(tags, TAG_TITLE)?;
    let start = parse_required_u64(tags, TAG_START)?;
    let end = parse_optional_u64(tags, TAG_END)?;
    validate_end_after_start(start, end, TAG_END)
        .map_err(|_| EventParseError::InvalidTag(TAG_END))?;
    Ok(RadrootsCalendarTimeEvent {
        d_tag,
        title,
        start,
        end,
        start_tzid: optional_tag_value(tags, TAG_START_TZID)?,
        end_tzid: optional_tag_value(tags, TAG_END_TZID)?,
        location: location_from_tags(tags),
        summary: optional_tag_value(tags, TAG_SUMMARY)?,
        image: optional_tag_value(tags, TAG_IMAGE)?,
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
