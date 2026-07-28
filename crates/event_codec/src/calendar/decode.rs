#[cfg(not(feature = "std"))]
use alloc::{
    string::{String, ToString},
    vec::Vec,
};

use radroots_event::{
    calendar::{
        RADROOTS_CALENDAR_SECONDS_PER_DAY, RadrootsAdmittedCalendar,
        RadrootsAdmittedCalendarDateEvent, RadrootsAdmittedCalendarEventRsvp,
        RadrootsAdmittedCalendarTimeEvent, RadrootsCalendarDate,
        RadrootsCalendarEventAuthorReference, RadrootsCalendarEventFreeBusy,
        RadrootsCalendarEventReference, RadrootsCalendarEventRevisionReference,
        RadrootsCalendarEventRsvpStatus, RadrootsCalendarParticipant, RadrootsCalendarRequest,
        RadrootsCalendarUri, RadrootsIanaTimeZoneId, RadrootsObservedUtcDay,
        RadrootsParsedNip52Calendar, RadrootsParsedNip52CalendarCommon,
        RadrootsParsedNip52CalendarCommonParts, RadrootsParsedNip52CalendarDateEvent,
        RadrootsParsedNip52CalendarEventRsvp, RadrootsParsedNip52CalendarEventRsvpParts,
        RadrootsParsedNip52CalendarParts, RadrootsParsedNip52CalendarTimeEvent,
        calendar_geohash_is_valid, calendar_relay_url_is_valid, calendar_tag_text_is_valid,
    },
    kinds::{
        KIND_CALENDAR, KIND_CALENDAR_DATE_EVENT, KIND_CALENDAR_EVENT_RSVP, KIND_CALENDAR_TIME_EVENT,
    },
    tags::{
        TAG_A, TAG_D, TAG_D_DAY, TAG_DESCRIPTION, TAG_E, TAG_END, TAG_END_TZID, TAG_FREE_BUSY,
        TAG_G, TAG_IMAGE, TAG_LOCATION, TAG_P, TAG_R, TAG_START, TAG_START_TZID, TAG_STATUS,
        TAG_SUMMARY, TAG_T, TAG_TITLE,
    },
    wire::{
        DEFAULT_CONTENT_MAX_BYTES, DEFAULT_TAG_ELEMENT_MAX_BYTES, DEFAULT_TAG_MAX_COUNT,
        DEFAULT_TAG_TOTAL_MAX_BYTES,
    },
};
use radroots_identity::PublicKey;

use crate::error::EventParseError;
use crate::parsed::{RadrootsParsedData, RadrootsParsedEvent};

const EXPECTED_DATE_KIND: &str = "31922";
const EXPECTED_TIME_KIND: &str = "31923";
const EXPECTED_CALENDAR_KIND: &str = "31924";
const EXPECTED_RSVP_KIND: &str = "31925";

pub fn parse_nip52_calendar_date_event(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<RadrootsParsedNip52CalendarDateEvent, EventParseError> {
    validate_bounded_calendar_parts(tags, content)?;
    if kind != KIND_CALENDAR_DATE_EVENT {
        return Err(EventParseError::InvalidKind {
            expected: EXPECTED_DATE_KIND,
            got: kind,
        });
    }
    let common = parse_nip52_calendar_common(tags, content)?;
    let start = parse_calendar_date(&required_exact_tag_value(tags, TAG_START)?, TAG_START)?;
    let end = optional_exact_tag_value(tags, TAG_END)?
        .map(|value| parse_calendar_date(&value, TAG_END))
        .transpose()?;
    let extension_day_tags = tags
        .iter()
        .filter(|tag| tag.first().map(String::as_str) == Some(TAG_D_DAY))
        .cloned()
        .collect();
    RadrootsParsedNip52CalendarDateEvent::try_new(common, start, end, extension_day_tags)
        .map_err(|_| EventParseError::InvalidTag(TAG_END))
}

pub fn parse_nip52_calendar_time_event(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<RadrootsParsedNip52CalendarTimeEvent, EventParseError> {
    validate_bounded_calendar_parts(tags, content)?;
    if kind != KIND_CALENDAR_TIME_EVENT {
        return Err(EventParseError::InvalidKind {
            expected: EXPECTED_TIME_KIND,
            got: kind,
        });
    }
    let common = parse_nip52_calendar_common(tags, content)?;
    let start_wire = required_exact_tag_value(tags, TAG_START)?;
    let start = parse_standard_u64(&start_wire, TAG_START)?;
    let end_wire = optional_exact_tag_value(tags, TAG_END)?;
    let end = end_wire
        .as_deref()
        .map(|value| parse_standard_u64(value, TAG_END))
        .transpose()?;
    let observed_day_indices = repeated_exact_tag_values(tags, TAG_D_DAY)?
        .into_iter()
        .map(|value| {
            RadrootsObservedUtcDay::parse(value).map_err(|_| EventParseError::InvalidTag(TAG_D_DAY))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if end.is_some_and(|end| end <= start) {
        return Err(EventParseError::InvalidTag(TAG_END));
    }
    if observed_day_indices.is_empty() {
        return Err(EventParseError::MissingTag(TAG_D_DAY));
    }
    let first_day = start / RADROOTS_CALENDAR_SECONDS_PER_DAY;
    let last_day = end
        .map(|end| (end - 1) / RADROOTS_CALENDAR_SECONDS_PER_DAY)
        .unwrap_or(first_day);
    if observed_day_indices
        .iter()
        .any(|day| !(first_day..=last_day).contains(&day.index()))
    {
        return Err(EventParseError::InvalidTag(TAG_D_DAY));
    }
    let start_tzid = optional_iana_tzid(tags, TAG_START_TZID)?;
    let end_tzid = optional_iana_tzid(tags, TAG_END_TZID)?;
    RadrootsParsedNip52CalendarTimeEvent::try_new(
        common,
        start_wire,
        start,
        end_wire,
        end,
        observed_day_indices,
        start_tzid,
        end_tzid,
    )
    .map_err(|_| EventParseError::InvalidTag(TAG_END))
}

pub fn admit_radroots_calendar_date_event(
    parsed: RadrootsParsedNip52CalendarDateEvent,
) -> Result<
    RadrootsAdmittedCalendarDateEvent,
    radroots_event::calendar::RadrootsCalendarAdmissionError,
> {
    RadrootsAdmittedCalendarDateEvent::try_from_parsed(parsed)
}

pub fn admit_radroots_calendar_time_event(
    parsed: RadrootsParsedNip52CalendarTimeEvent,
) -> Result<
    RadrootsAdmittedCalendarTimeEvent,
    radroots_event::calendar::RadrootsCalendarAdmissionError,
> {
    RadrootsAdmittedCalendarTimeEvent::try_from_parsed(parsed)
}

pub fn parse_nip52_calendar(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<RadrootsParsedNip52Calendar, EventParseError> {
    validate_bounded_calendar_parts(tags, content)?;
    if kind != KIND_CALENDAR {
        return Err(EventParseError::InvalidKind {
            expected: EXPECTED_CALENDAR_KIND,
            got: kind,
        });
    }
    let d_tag = required_exact_tag_value(tags, TAG_D)?;
    validate_calendar_text_tag(&d_tag, TAG_D)?;
    let title = required_exact_tag_value(tags, TAG_TITLE)?;
    validate_calendar_text_tag(&title, TAG_TITLE)?;
    let event_references = calendar_event_references_from_tags(tags)?;
    let list_description = optional_calendar_text_tag(tags, TAG_DESCRIPTION)?;
    let image = optional_calendar_uri_tag(tags, TAG_IMAGE)?;
    RadrootsParsedNip52Calendar::try_new(RadrootsParsedNip52CalendarParts {
        d_tag,
        title,
        content: content.to_string(),
        event_references,
        list_description,
        image,
    })
    .map_err(|_| EventParseError::InvalidTag("calendar"))
}

pub fn admit_radroots_calendar(
    parsed: RadrootsParsedNip52Calendar,
) -> Result<RadrootsAdmittedCalendar, radroots_event::calendar::RadrootsCalendarAdmissionError> {
    RadrootsAdmittedCalendar::try_from_parsed(parsed)
}

pub fn parse_nip52_calendar_event_rsvp(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<RadrootsParsedNip52CalendarEventRsvp, EventParseError> {
    validate_bounded_calendar_parts(tags, content)?;
    if kind != KIND_CALENDAR_EVENT_RSVP {
        return Err(EventParseError::InvalidKind {
            expected: EXPECTED_RSVP_KIND,
            got: kind,
        });
    }
    let d_tag = required_exact_tag_value(tags, TAG_D)?;
    validate_calendar_text_tag(&d_tag, TAG_D)?;
    let event_reference = calendar_event_reference_from_required_tag(tags)?;
    let revision_reference = optional_reference_tag(tags, TAG_E)?
        .map(|tag| {
            RadrootsCalendarEventRevisionReference::parse(&tag[1], tag.get(2).map(String::as_str))
                .map_err(|_| EventParseError::InvalidTag(TAG_E))
        })
        .transpose()?;
    let status = parse_rsvp_status(&required_exact_tag_value(tags, TAG_STATUS)?)?;
    let observed_free_busy = optional_exact_tag_value(tags, TAG_FREE_BUSY)?
        .map(|value| parse_free_busy(&value))
        .transpose()?;
    let author_hint = optional_reference_tag(tags, TAG_P)?
        .map(|tag| {
            RadrootsCalendarEventAuthorReference::parse(&tag[1], tag.get(2).map(String::as_str))
                .map_err(|_| EventParseError::InvalidTag(TAG_P))
        })
        .transpose()?;
    RadrootsParsedNip52CalendarEventRsvp::try_new(RadrootsParsedNip52CalendarEventRsvpParts {
        d_tag,
        event_reference,
        revision_reference,
        status,
        observed_free_busy,
        author_hint,
        note: optional_content(content),
    })
    .map_err(|_| EventParseError::InvalidTag("calendar_rsvp"))
}

pub fn admit_radroots_calendar_event_rsvp(
    parsed: RadrootsParsedNip52CalendarEventRsvp,
) -> Result<
    RadrootsAdmittedCalendarEventRsvp,
    radroots_event::calendar::RadrootsCalendarAdmissionError,
> {
    RadrootsAdmittedCalendarEventRsvp::try_from_parsed(parsed)
}

pub fn nip52_date_data_from_event(
    id: String,
    author: String,
    published_at: u64,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
) -> Result<RadrootsParsedData<RadrootsParsedNip52CalendarDateEvent>, EventParseError> {
    let event = parse_nip52_calendar_date_event(kind, &tags, &content)?;
    Ok(RadrootsParsedData::new(
        id,
        author,
        published_at,
        kind,
        event,
    ))
}

pub fn nip52_time_data_from_event(
    id: String,
    author: String,
    published_at: u64,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
) -> Result<RadrootsParsedData<RadrootsParsedNip52CalendarTimeEvent>, EventParseError> {
    let event = parse_nip52_calendar_time_event(kind, &tags, &content)?;
    Ok(RadrootsParsedData::new(
        id,
        author,
        published_at,
        kind,
        event,
    ))
}

pub fn nip52_calendar_data_from_event(
    id: String,
    author: String,
    published_at: u64,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
) -> Result<RadrootsParsedData<RadrootsParsedNip52Calendar>, EventParseError> {
    let calendar = parse_nip52_calendar(kind, &tags, &content)?;
    Ok(RadrootsParsedData::new(
        id,
        author,
        published_at,
        kind,
        calendar,
    ))
}

pub fn nip52_calendar_event_rsvp_data_from_event(
    id: String,
    author: String,
    published_at: u64,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
) -> Result<RadrootsParsedData<RadrootsParsedNip52CalendarEventRsvp>, EventParseError> {
    let rsvp = parse_nip52_calendar_event_rsvp(kind, &tags, &content)?;
    Ok(RadrootsParsedData::new(
        id,
        author,
        published_at,
        kind,
        rsvp,
    ))
}

pub fn nip52_date_parsed_from_event(
    id: String,
    author: String,
    published_at: u64,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
    sig: String,
) -> Result<RadrootsParsedEvent<RadrootsParsedNip52CalendarDateEvent>, EventParseError> {
    let data = nip52_date_data_from_event(
        id.clone(),
        author.clone(),
        published_at,
        kind,
        content.clone(),
        tags.clone(),
    )?;
    RadrootsParsedEvent::from_event_parts(id, author, published_at, kind, content, tags, sig, data)
}

pub fn nip52_time_parsed_from_event(
    id: String,
    author: String,
    published_at: u64,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
    sig: String,
) -> Result<RadrootsParsedEvent<RadrootsParsedNip52CalendarTimeEvent>, EventParseError> {
    let data = nip52_time_data_from_event(
        id.clone(),
        author.clone(),
        published_at,
        kind,
        content.clone(),
        tags.clone(),
    )?;
    RadrootsParsedEvent::from_event_parts(id, author, published_at, kind, content, tags, sig, data)
}

pub fn nip52_calendar_parsed_from_event(
    id: String,
    author: String,
    published_at: u64,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
    sig: String,
) -> Result<RadrootsParsedEvent<RadrootsParsedNip52Calendar>, EventParseError> {
    let data = nip52_calendar_data_from_event(
        id.clone(),
        author.clone(),
        published_at,
        kind,
        content.clone(),
        tags.clone(),
    )?;
    RadrootsParsedEvent::from_event_parts(id, author, published_at, kind, content, tags, sig, data)
}

pub fn nip52_calendar_event_rsvp_parsed_from_event(
    id: String,
    author: String,
    published_at: u64,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
    sig: String,
) -> Result<RadrootsParsedEvent<RadrootsParsedNip52CalendarEventRsvp>, EventParseError> {
    let data = nip52_calendar_event_rsvp_data_from_event(
        id.clone(),
        author.clone(),
        published_at,
        kind,
        content.clone(),
        tags.clone(),
    )?;
    RadrootsParsedEvent::from_event_parts(id, author, published_at, kind, content, tags, sig, data)
}

fn parse_nip52_calendar_common(
    tags: &[Vec<String>],
    content: &str,
) -> Result<RadrootsParsedNip52CalendarCommon, EventParseError> {
    let d_tag = required_exact_tag_value(tags, TAG_D)?;
    validate_calendar_text_tag(&d_tag, TAG_D)?;
    let title = required_exact_tag_value(tags, TAG_TITLE)?;
    validate_calendar_text_tag(&title, TAG_TITLE)?;
    let locations = repeated_calendar_text_values(tags, TAG_LOCATION)?;
    let geohash = optional_exact_tag_value(tags, TAG_G)?;
    if geohash
        .as_deref()
        .is_some_and(|value| !calendar_geohash_is_valid(value))
    {
        return Err(EventParseError::InvalidTag(TAG_G));
    }
    let summary = optional_calendar_text_tag(tags, TAG_SUMMARY)?;
    let image = optional_calendar_uri_tag(tags, TAG_IMAGE)?;
    let participants = calendar_participants_from_tags(tags)?;
    let categories = repeated_calendar_text_values(tags, TAG_T)?;
    let references = repeated_exact_tag_values(tags, TAG_R)?
        .into_iter()
        .map(|value| {
            RadrootsCalendarUri::parse(value).map_err(|_| EventParseError::InvalidTag(TAG_R))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let calendar_requests = calendar_requests_from_tags(tags)?;
    let legacy_name = optional_calendar_text_tag(tags, "name")?;
    RadrootsParsedNip52CalendarCommon::try_new(RadrootsParsedNip52CalendarCommonParts {
        d_tag,
        title,
        description: optional_content(content),
        locations,
        geohash,
        summary,
        image,
        participants,
        categories,
        references,
        calendar_requests,
        legacy_name,
    })
    .map_err(|_| EventParseError::InvalidTag("calendar"))
}

fn validate_bounded_calendar_parts(
    tags: &[Vec<String>],
    content: &str,
) -> Result<(), EventParseError> {
    if content.len() > DEFAULT_CONTENT_MAX_BYTES {
        return Err(EventParseError::InvalidEnvelope);
    }
    if tags.len() > DEFAULT_TAG_MAX_COUNT {
        return Err(EventParseError::InvalidEnvelope);
    }
    let mut total_bytes = 0usize;
    for tag in tags {
        let Some(key) = tag.first() else {
            return Err(EventParseError::InvalidEnvelope);
        };
        if key.is_empty() || key.chars().any(char::is_control) {
            return Err(EventParseError::InvalidEnvelope);
        }
        for value in tag {
            if value.len() > DEFAULT_TAG_ELEMENT_MAX_BYTES {
                return Err(EventParseError::InvalidEnvelope);
            }
            total_bytes = total_bytes.saturating_add(value.len());
            if total_bytes > DEFAULT_TAG_TOTAL_MAX_BYTES {
                return Err(EventParseError::InvalidEnvelope);
            }
        }
    }
    Ok(())
}

fn required_exact_tag_value(
    tags: &[Vec<String>],
    key: &'static str,
) -> Result<String, EventParseError> {
    optional_exact_tag_value(tags, key)?.ok_or(EventParseError::MissingTag(key))
}

fn repeated_exact_tag_values(
    tags: &[Vec<String>],
    key: &'static str,
) -> Result<Vec<String>, EventParseError> {
    tags.iter()
        .filter(|tag| tag.first().map(String::as_str) == Some(key))
        .map(|tag| {
            if tag.len() != 2 || tag[1].is_empty() {
                Err(EventParseError::InvalidTag(key))
            } else {
                Ok(tag[1].clone())
            }
        })
        .collect()
}

fn optional_exact_tag_value(
    tags: &[Vec<String>],
    key: &'static str,
) -> Result<Option<String>, EventParseError> {
    let mut matching = tags
        .iter()
        .filter(|tag| tag.first().map(String::as_str) == Some(key));
    let Some(tag) = matching.next() else {
        return Ok(None);
    };
    if matching.next().is_some() {
        return Err(EventParseError::DuplicateTag(key));
    }
    if tag.len() != 2 {
        return Err(EventParseError::InvalidTag(key));
    }
    let value = tag[1].clone();
    if value.is_empty() {
        return Err(EventParseError::InvalidTag(key));
    }
    Ok(Some(value))
}

fn parse_calendar_date(
    value: &str,
    key: &'static str,
) -> Result<RadrootsCalendarDate, EventParseError> {
    RadrootsCalendarDate::parse(value).map_err(|_| EventParseError::InvalidTag(key))
}

fn parse_standard_u64(value: &str, key: &'static str) -> Result<u64, EventParseError> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(EventParseError::InvalidTag(key));
    }
    value
        .parse::<u64>()
        .map_err(|error| EventParseError::InvalidNumber(key, error))
}

fn optional_iana_tzid(
    tags: &[Vec<String>],
    key: &'static str,
) -> Result<Option<RadrootsIanaTimeZoneId>, EventParseError> {
    optional_exact_tag_value(tags, key)?
        .map(|value| {
            RadrootsIanaTimeZoneId::parse(value).map_err(|_| EventParseError::InvalidTag(key))
        })
        .transpose()
}

fn optional_calendar_text_tag(
    tags: &[Vec<String>],
    key: &'static str,
) -> Result<Option<String>, EventParseError> {
    let value = optional_exact_tag_value(tags, key)?;
    if let Some(value) = value.as_deref() {
        validate_calendar_text_tag(value, key)?;
    }
    Ok(value)
}

fn validate_calendar_text_tag(value: &str, key: &'static str) -> Result<(), EventParseError> {
    if !calendar_tag_text_is_valid(value) {
        Err(EventParseError::InvalidTag(key))
    } else {
        Ok(())
    }
}

fn repeated_calendar_text_values(
    tags: &[Vec<String>],
    key: &'static str,
) -> Result<Vec<String>, EventParseError> {
    let values = repeated_exact_tag_values(tags, key)?;
    for value in &values {
        validate_calendar_text_tag(value, key)?;
    }
    Ok(values)
}

fn optional_calendar_uri_tag(
    tags: &[Vec<String>],
    key: &'static str,
) -> Result<Option<RadrootsCalendarUri>, EventParseError> {
    optional_exact_tag_value(tags, key)?
        .map(|value| {
            RadrootsCalendarUri::parse(value).map_err(|_| EventParseError::InvalidTag(key))
        })
        .transpose()
}

fn calendar_participants_from_tags(
    tags: &[Vec<String>],
) -> Result<Vec<RadrootsCalendarParticipant>, EventParseError> {
    let mut participants = Vec::new();
    for tag in tags
        .iter()
        .filter(|tag| tag.first().map(String::as_str) == Some("p"))
    {
        if !(2..=4).contains(&tag.len()) {
            return Err(EventParseError::InvalidTag("p"));
        }
        PublicKey::from_hex(&tag[1]).map_err(|_| EventParseError::InvalidTag(TAG_P))?;
        if tag.len() == 3 && tag[2].is_empty() {
            return Err(EventParseError::InvalidTag(TAG_P));
        }
        let relay = tag.get(2).filter(|value| !value.is_empty()).cloned();
        if relay
            .as_deref()
            .is_some_and(|value| !calendar_relay_url_is_valid(value))
        {
            return Err(EventParseError::InvalidTag(TAG_P));
        }
        let role = tag.get(3).cloned();
        if let Some(role) = role.as_deref() {
            validate_calendar_text_tag(role, TAG_P)?;
        }
        participants.push(RadrootsCalendarParticipant {
            pubkey: tag[1].clone(),
            relay,
            role,
        });
    }
    Ok(participants)
}

fn calendar_requests_from_tags(
    tags: &[Vec<String>],
) -> Result<Vec<RadrootsCalendarRequest>, EventParseError> {
    tags.iter()
        .filter(|tag| tag.first().map(String::as_str) == Some(TAG_A))
        .map(|tag| {
            if !(2..=3).contains(&tag.len())
                || tag[1].is_empty()
                || tag.get(2).is_some_and(String::is_empty)
            {
                return Err(EventParseError::InvalidTag(TAG_A));
            }
            RadrootsCalendarRequest::new(&tag[1], tag.get(2).map(String::as_str))
                .map_err(|_| EventParseError::InvalidTag(TAG_A))
        })
        .collect()
}

fn optional_content(content: &str) -> Option<String> {
    if content.is_empty() {
        None
    } else {
        Some(content.to_string())
    }
}

fn calendar_event_references_from_tags(
    tags: &[Vec<String>],
) -> Result<Vec<RadrootsCalendarEventReference>, EventParseError> {
    tags.iter()
        .filter(|tag| tag.first().map(String::as_str) == Some(TAG_A))
        .map(|tag| parse_calendar_event_reference_tag(tag))
        .collect()
}

fn calendar_event_reference_from_required_tag(
    tags: &[Vec<String>],
) -> Result<RadrootsCalendarEventReference, EventParseError> {
    let tag = optional_reference_tag(tags, TAG_A)?.ok_or(EventParseError::MissingTag(TAG_A))?;
    parse_calendar_event_reference_tag(tag)
}

fn parse_calendar_event_reference_tag(
    tag: &[String],
) -> Result<RadrootsCalendarEventReference, EventParseError> {
    validate_reference_tag_shape(tag, TAG_A)?;
    RadrootsCalendarEventReference::parse(&tag[1], tag.get(2).map(String::as_str))
        .map_err(|_| EventParseError::InvalidTag(TAG_A))
}

fn optional_reference_tag<'a>(
    tags: &'a [Vec<String>],
    key: &'static str,
) -> Result<Option<&'a Vec<String>>, EventParseError> {
    let mut matching = tags
        .iter()
        .filter(|tag| tag.first().map(String::as_str) == Some(key));
    let Some(tag) = matching.next() else {
        return Ok(None);
    };
    if matching.next().is_some() {
        return Err(EventParseError::DuplicateTag(key));
    }
    validate_reference_tag_shape(tag, key)?;
    Ok(Some(tag))
}

fn validate_reference_tag_shape(tag: &[String], key: &'static str) -> Result<(), EventParseError> {
    if !(2..=3).contains(&tag.len())
        || tag[1].is_empty()
        || tag.get(2).is_some_and(String::is_empty)
    {
        return Err(EventParseError::InvalidTag(key));
    }
    Ok(())
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
