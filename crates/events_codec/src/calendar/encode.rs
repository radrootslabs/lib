#[cfg(not(feature = "std"))]
use alloc::{string::ToString, vec::Vec};

use radroots_events::{
    calendar::{RadrootsCalendarDateEvent, RadrootsCalendarTimeEvent},
    kinds::{KIND_CALENDAR_DATE_EVENT, KIND_CALENDAR_TIME_EVENT},
    tags::{
        TAG_D, TAG_D_DAY, TAG_END, TAG_END_TZID, TAG_IMAGE, TAG_START, TAG_START_TZID, TAG_SUMMARY,
        TAG_TITLE,
    },
};

use crate::d_tag::validate_d_tag;
use crate::error::EventEncodeError;
use crate::field_helpers::{push_optional_tag, push_tag, validate_non_empty_field};
use crate::social_helpers::{
    push_location_tags, push_participants, validate_date, validate_date_end_after_start,
    validate_end_after_start,
};
use crate::wire::{WireEventParts, empty_content};

pub fn calendar_date_event_build_tags(
    event: &RadrootsCalendarDateEvent,
) -> Result<Vec<Vec<String>>, EventEncodeError> {
    validate_date_event(event)?;
    let mut tags = Vec::new();
    push_tag(&mut tags, TAG_D, event.d_tag.as_str());
    push_tag(&mut tags, TAG_TITLE, event.title.as_str());
    push_tag(&mut tags, TAG_START, event.start.as_str());
    push_optional_tag(&mut tags, TAG_END, event.end.as_deref());
    if let Some(days) = event.days.as_ref() {
        for day in days {
            validate_date(&day.value, "days")?;
            push_tag(&mut tags, TAG_D_DAY, day.value.as_str());
        }
    }
    if let Some(location) = event.location.as_ref() {
        push_location_tags(&mut tags, location);
    }
    push_optional_tag(&mut tags, TAG_SUMMARY, event.summary.as_deref());
    push_optional_tag(&mut tags, TAG_IMAGE, event.image.as_deref());
    push_participants(&mut tags, event.participants.as_ref());
    Ok(tags)
}

pub fn calendar_time_event_build_tags(
    event: &RadrootsCalendarTimeEvent,
) -> Result<Vec<Vec<String>>, EventEncodeError> {
    validate_time_event(event)?;
    let mut tags = Vec::new();
    push_tag(&mut tags, TAG_D, event.d_tag.as_str());
    push_tag(&mut tags, TAG_TITLE, event.title.as_str());
    push_tag(&mut tags, TAG_START, event.start.to_string());
    if let Some(end) = event.end {
        push_tag(&mut tags, TAG_END, end.to_string());
    }
    push_optional_tag(&mut tags, TAG_START_TZID, event.start_tzid.as_deref());
    push_optional_tag(&mut tags, TAG_END_TZID, event.end_tzid.as_deref());
    if let Some(location) = event.location.as_ref() {
        push_location_tags(&mut tags, location);
    }
    push_optional_tag(&mut tags, TAG_SUMMARY, event.summary.as_deref());
    push_optional_tag(&mut tags, TAG_IMAGE, event.image.as_deref());
    push_participants(&mut tags, event.participants.as_ref());
    Ok(tags)
}

pub fn date_to_wire_parts(
    event: &RadrootsCalendarDateEvent,
) -> Result<WireEventParts, EventEncodeError> {
    date_to_wire_parts_with_kind(event, KIND_CALENDAR_DATE_EVENT)
}

pub fn time_to_wire_parts(
    event: &RadrootsCalendarTimeEvent,
) -> Result<WireEventParts, EventEncodeError> {
    time_to_wire_parts_with_kind(event, KIND_CALENDAR_TIME_EVENT)
}

pub fn date_to_wire_parts_with_kind(
    event: &RadrootsCalendarDateEvent,
    kind: u32,
) -> Result<WireEventParts, EventEncodeError> {
    if kind != KIND_CALENDAR_DATE_EVENT {
        return Err(EventEncodeError::InvalidKind(kind));
    }
    Ok(WireEventParts {
        kind,
        content: empty_content(),
        tags: calendar_date_event_build_tags(event)?,
    })
}

pub fn time_to_wire_parts_with_kind(
    event: &RadrootsCalendarTimeEvent,
    kind: u32,
) -> Result<WireEventParts, EventEncodeError> {
    if kind != KIND_CALENDAR_TIME_EVENT {
        return Err(EventEncodeError::InvalidKind(kind));
    }
    Ok(WireEventParts {
        kind,
        content: empty_content(),
        tags: calendar_time_event_build_tags(event)?,
    })
}

fn validate_date_event(event: &RadrootsCalendarDateEvent) -> Result<(), EventEncodeError> {
    validate_d_tag(&event.d_tag, "d_tag")?;
    validate_non_empty_field(&event.title, "title")?;
    validate_date(&event.start, "start")?;
    if let Some(end) = event.end.as_deref() {
        validate_date(end, "end")?;
    }
    validate_date_end_after_start(&event.start, event.end.as_deref(), "end")?;
    Ok(())
}

fn validate_time_event(event: &RadrootsCalendarTimeEvent) -> Result<(), EventEncodeError> {
    validate_d_tag(&event.d_tag, "d_tag")?;
    validate_non_empty_field(&event.title, "title")?;
    validate_end_after_start(event.start, event.end, "end")?;
    Ok(())
}
