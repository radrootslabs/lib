#[cfg(not(feature = "std"))]
use alloc::{format, string::ToString, vec, vec::Vec};

use radroots_events::{
    calendar::{
        RadrootsCalendar, RadrootsCalendarDateEvent, RadrootsCalendarEventRsvp,
        RadrootsCalendarTimeEvent,
    },
    kinds::{
        KIND_CALENDAR, KIND_CALENDAR_DATE_EVENT, KIND_CALENDAR_EVENT_RSVP, KIND_CALENDAR_TIME_EVENT,
    },
    social::{
        RadrootsCalendarEventFreeBusy, RadrootsCalendarEventRsvpStatus, RadrootsSocialTarget,
    },
    tags::{
        TAG_A, TAG_D, TAG_D_DAY, TAG_E, TAG_END, TAG_END_TZID, TAG_FREE_BUSY, TAG_IMAGE, TAG_START,
        TAG_START_TZID, TAG_STATUS, TAG_SUMMARY, TAG_TITLE,
    },
};

use crate::d_tag::validate_d_tag;
use crate::error::EventEncodeError;
use crate::field_helpers::{
    parse_address_tag, push_optional_tag, push_tag, push_tag_values, validate_lowercase_hex_64,
    validate_non_empty_field,
};
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

pub fn calendar_collection_build_tags(
    calendar: &RadrootsCalendar,
) -> Result<Vec<Vec<String>>, EventEncodeError> {
    validate_calendar_collection(calendar)?;
    let mut tags = Vec::new();
    push_tag(&mut tags, TAG_D, calendar.d_tag.as_str());
    push_tag(&mut tags, TAG_TITLE, calendar.title.as_str());
    push_optional_tag(&mut tags, TAG_SUMMARY, calendar.summary.as_deref());
    push_optional_tag(&mut tags, TAG_IMAGE, calendar.image.as_deref());
    for event in &calendar.events {
        push_calendar_event_address(&mut tags, event, "events")?;
    }
    Ok(tags)
}

pub fn rsvp_build_tags(
    rsvp: &RadrootsCalendarEventRsvp,
) -> Result<Vec<Vec<String>>, EventEncodeError> {
    validate_rsvp(rsvp)?;
    let mut tags = Vec::new();
    push_tag(&mut tags, TAG_D, rsvp.d_tag.as_str());
    push_calendar_event_address(&mut tags, &rsvp.event, "event")?;
    if let Some(event_id) = rsvp.event_id.as_deref() {
        let mut tag = vec![TAG_E.to_string(), event_id.to_string()];
        if let RadrootsSocialTarget::Address { relays, .. } = &rsvp.event {
            if let Some(relays) = relays.as_ref() {
                tag.extend(
                    relays
                        .iter()
                        .filter(|relay| !relay.trim().is_empty())
                        .cloned(),
                );
            }
        }
        tags.push(tag);
    }
    push_tag(&mut tags, TAG_STATUS, rsvp_status_as_str(&rsvp.status));
    if let Some(free_busy) = rsvp.free_busy.as_ref() {
        push_tag(&mut tags, TAG_FREE_BUSY, free_busy_as_str(free_busy));
    }
    push_participants(&mut tags, rsvp.participants.as_ref());
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

pub fn calendar_to_wire_parts(
    calendar: &RadrootsCalendar,
) -> Result<WireEventParts, EventEncodeError> {
    calendar_to_wire_parts_with_kind(calendar, KIND_CALENDAR)
}

pub fn rsvp_to_wire_parts(
    rsvp: &RadrootsCalendarEventRsvp,
) -> Result<WireEventParts, EventEncodeError> {
    rsvp_to_wire_parts_with_kind(rsvp, KIND_CALENDAR_EVENT_RSVP)
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

pub fn calendar_to_wire_parts_with_kind(
    calendar: &RadrootsCalendar,
    kind: u32,
) -> Result<WireEventParts, EventEncodeError> {
    if kind != KIND_CALENDAR {
        return Err(EventEncodeError::InvalidKind(kind));
    }
    Ok(WireEventParts {
        kind,
        content: empty_content(),
        tags: calendar_collection_build_tags(calendar)?,
    })
}

pub fn rsvp_to_wire_parts_with_kind(
    rsvp: &RadrootsCalendarEventRsvp,
    kind: u32,
) -> Result<WireEventParts, EventEncodeError> {
    if kind != KIND_CALENDAR_EVENT_RSVP {
        return Err(EventEncodeError::InvalidKind(kind));
    }
    Ok(WireEventParts {
        kind,
        content: rsvp.note.clone().unwrap_or_default(),
        tags: rsvp_build_tags(rsvp)?,
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

fn validate_calendar_collection(calendar: &RadrootsCalendar) -> Result<(), EventEncodeError> {
    validate_d_tag(&calendar.d_tag, "d_tag")?;
    validate_non_empty_field(&calendar.title, "title")?;
    if calendar.events.is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("events"));
    }
    Ok(())
}

fn validate_rsvp(rsvp: &RadrootsCalendarEventRsvp) -> Result<(), EventEncodeError> {
    validate_d_tag(&rsvp.d_tag, "d_tag")?;
    validate_calendar_event_address(&rsvp.event, "event")?;
    if let Some(event_id) = rsvp.event_id.as_deref() {
        validate_lowercase_hex_64(event_id, "event_id")?;
    }
    Ok(())
}

fn push_calendar_event_address(
    tags: &mut Vec<Vec<String>>,
    target: &RadrootsSocialTarget,
    field: &'static str,
) -> Result<(), EventEncodeError> {
    let RadrootsSocialTarget::Address {
        address,
        event_kind,
        relays,
        ..
    } = target
    else {
        return Err(EventEncodeError::InvalidField(field));
    };
    let address =
        parse_address_tag(address, field).map_err(|_| EventEncodeError::InvalidField(field))?;
    if !is_calendar_event_kind(address.kind) {
        return Err(EventEncodeError::InvalidField(field));
    }
    if let Some(event_kind) = event_kind {
        if *event_kind != address.kind {
            return Err(EventEncodeError::InvalidField(field));
        }
    }
    let value = format!("{}:{}:{}", address.kind, address.pubkey, address.d_tag);
    if let Some(relays) = relays.as_ref() {
        let mut values = Vec::with_capacity(1 + relays.len());
        values.push(value);
        values.extend(
            relays
                .iter()
                .filter(|relay| !relay.trim().is_empty())
                .cloned(),
        );
        push_tag_values(tags, TAG_A, values);
    } else {
        push_tag(tags, TAG_A, value);
    }
    Ok(())
}

fn validate_calendar_event_address(
    target: &RadrootsSocialTarget,
    field: &'static str,
) -> Result<(), EventEncodeError> {
    let mut tags = Vec::new();
    push_calendar_event_address(&mut tags, target, field)
}

fn is_calendar_event_kind(kind: u32) -> bool {
    matches!(kind, KIND_CALENDAR_DATE_EVENT | KIND_CALENDAR_TIME_EVENT)
}

fn rsvp_status_as_str(status: &RadrootsCalendarEventRsvpStatus) -> &'static str {
    match status {
        RadrootsCalendarEventRsvpStatus::Accepted => "accepted",
        RadrootsCalendarEventRsvpStatus::Declined => "declined",
        RadrootsCalendarEventRsvpStatus::Tentative => "tentative",
    }
}

fn free_busy_as_str(free_busy: &RadrootsCalendarEventFreeBusy) -> &'static str {
    match free_busy {
        RadrootsCalendarEventFreeBusy::Free => "free",
        RadrootsCalendarEventFreeBusy::Busy => "busy",
    }
}
