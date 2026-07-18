#[cfg(not(feature = "std"))]
use alloc::{
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};

use radroots_event::{
    calendar::{
        RadrootsAuthoredCalendarDateEvent, RadrootsAuthoredCalendarTimeEvent, RadrootsCalendar,
        RadrootsCalendarEventRsvp, RadrootsCalendarRequest, RadrootsCalendarUri, covered_utc_days,
    },
    kinds::{
        KIND_CALENDAR, KIND_CALENDAR_DATE_EVENT, KIND_CALENDAR_EVENT_RSVP, KIND_CALENDAR_TIME_EVENT,
    },
    social::{
        RadrootsCalendarEventFreeBusy, RadrootsCalendarEventRsvpStatus, RadrootsSocialTarget,
    },
    tags::{
        TAG_A, TAG_D, TAG_D_DAY, TAG_E, TAG_END, TAG_END_TZID, TAG_FREE_BUSY, TAG_G, TAG_IMAGE,
        TAG_LOCATION, TAG_R, TAG_START, TAG_START_TZID, TAG_STATUS, TAG_SUMMARY, TAG_T, TAG_TITLE,
    },
};

use crate::d_tag::validate_d_tag;
use crate::error::EventEncodeError;
use crate::field_helpers::{
    parse_address_tag, push_optional_tag, push_tag, push_tag_values, validate_lowercase_hex_64,
    validate_non_empty_field,
};
use crate::social_helpers::push_participants;
use radroots_event::wire::RadrootsNip01EventWireParts;

pub fn calendar_date_event_build_tags(
    event: &RadrootsAuthoredCalendarDateEvent,
) -> Result<Vec<Vec<String>>, EventEncodeError> {
    validate_authored_date_event(event)?;
    let mut tags = Vec::new();
    push_tag(&mut tags, TAG_D, event.d_tag().as_str());
    push_tag(&mut tags, TAG_TITLE, event.title());
    push_tag(&mut tags, TAG_START, event.start().as_str());
    push_optional_tag(&mut tags, TAG_END, event.end().map(|end| end.as_str()));
    push_authored_calendar_common_tags(
        &mut tags,
        event.locations(),
        event.geohash(),
        event.summary(),
        event.image().map(|image| image.descriptor().url().as_str()),
        event.participants(),
        event.categories(),
        event.references(),
        event.calendar_requests(),
    );
    Ok(tags)
}

pub fn calendar_time_event_build_tags(
    event: &RadrootsAuthoredCalendarTimeEvent,
) -> Result<Vec<Vec<String>>, EventEncodeError> {
    validate_authored_time_event(event)?;
    let mut tags = Vec::new();
    push_tag(&mut tags, TAG_D, event.d_tag().as_str());
    push_tag(&mut tags, TAG_TITLE, event.title());
    push_tag(&mut tags, TAG_START, event.start().to_string());
    if let Some(end) = event.end() {
        push_tag(&mut tags, TAG_END, end.to_string());
    }
    for day in covered_utc_days(event.start(), event.end())
        .map_err(|_| EventEncodeError::InvalidField("end"))?
    {
        push_tag(&mut tags, TAG_D_DAY, day.to_string());
    }
    push_optional_tag(
        &mut tags,
        TAG_START_TZID,
        event.start_tzid().map(|tzid| tzid.as_str()),
    );
    push_optional_tag(
        &mut tags,
        TAG_END_TZID,
        event.end_tzid().map(|tzid| tzid.as_str()),
    );
    push_authored_calendar_common_tags(
        &mut tags,
        event.locations(),
        event.geohash(),
        event.summary(),
        event.image().map(|image| image.descriptor().url().as_str()),
        event.participants(),
        event.categories(),
        event.references(),
        event.calendar_requests(),
    );
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
        if let Some(relays) = calendar_event_relays(&rsvp.event) {
            tag.extend(
                relays
                    .iter()
                    .filter(|relay| !relay.trim().is_empty())
                    .cloned(),
            );
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
    event: &RadrootsAuthoredCalendarDateEvent,
) -> Result<RadrootsNip01EventWireParts, EventEncodeError> {
    date_to_wire_parts_with_kind(event, KIND_CALENDAR_DATE_EVENT)
}

pub fn time_to_wire_parts(
    event: &RadrootsAuthoredCalendarTimeEvent,
) -> Result<RadrootsNip01EventWireParts, EventEncodeError> {
    time_to_wire_parts_with_kind(event, KIND_CALENDAR_TIME_EVENT)
}

pub fn calendar_to_wire_parts(
    calendar: &RadrootsCalendar,
) -> Result<RadrootsNip01EventWireParts, EventEncodeError> {
    calendar_to_wire_parts_with_kind(calendar, KIND_CALENDAR)
}

pub fn rsvp_to_wire_parts(
    rsvp: &RadrootsCalendarEventRsvp,
) -> Result<RadrootsNip01EventWireParts, EventEncodeError> {
    rsvp_to_wire_parts_with_kind(rsvp, KIND_CALENDAR_EVENT_RSVP)
}

pub fn date_to_wire_parts_with_kind(
    event: &RadrootsAuthoredCalendarDateEvent,
    kind: u32,
) -> Result<RadrootsNip01EventWireParts, EventEncodeError> {
    if kind != KIND_CALENDAR_DATE_EVENT {
        return Err(EventEncodeError::InvalidKind(kind));
    }
    Ok(RadrootsNip01EventWireParts {
        kind,
        content: event.description().unwrap_or_default().to_string(),
        tags: calendar_date_event_build_tags(event)?,
    })
}

pub fn time_to_wire_parts_with_kind(
    event: &RadrootsAuthoredCalendarTimeEvent,
    kind: u32,
) -> Result<RadrootsNip01EventWireParts, EventEncodeError> {
    if kind != KIND_CALENDAR_TIME_EVENT {
        return Err(EventEncodeError::InvalidKind(kind));
    }
    Ok(RadrootsNip01EventWireParts {
        kind,
        content: event.description().unwrap_or_default().to_string(),
        tags: calendar_time_event_build_tags(event)?,
    })
}

pub fn calendar_to_wire_parts_with_kind(
    calendar: &RadrootsCalendar,
    kind: u32,
) -> Result<RadrootsNip01EventWireParts, EventEncodeError> {
    if kind != KIND_CALENDAR {
        return Err(EventEncodeError::InvalidKind(kind));
    }
    Ok(RadrootsNip01EventWireParts {
        kind,
        content: calendar.description.clone().unwrap_or_default(),
        tags: calendar_collection_build_tags(calendar)?,
    })
}

pub fn rsvp_to_wire_parts_with_kind(
    rsvp: &RadrootsCalendarEventRsvp,
    kind: u32,
) -> Result<RadrootsNip01EventWireParts, EventEncodeError> {
    if kind != KIND_CALENDAR_EVENT_RSVP {
        return Err(EventEncodeError::InvalidKind(kind));
    }
    Ok(RadrootsNip01EventWireParts {
        kind,
        content: rsvp.note.clone().unwrap_or_default(),
        tags: rsvp_build_tags(rsvp)?,
    })
}

fn validate_authored_date_event(
    _event: &RadrootsAuthoredCalendarDateEvent,
) -> Result<(), EventEncodeError> {
    Ok(())
}

fn validate_authored_time_event(
    event: &RadrootsAuthoredCalendarTimeEvent,
) -> Result<(), EventEncodeError> {
    covered_utc_days(event.start(), event.end())
        .map_err(|_| EventEncodeError::InvalidField("end"))?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn push_authored_calendar_common_tags(
    tags: &mut Vec<Vec<String>>,
    locations: &[String],
    geohash: Option<&str>,
    summary: Option<&str>,
    image: Option<&str>,
    participants: Option<&Vec<radroots_event::social::RadrootsCalendarParticipant>>,
    categories: &[String],
    references: &[RadrootsCalendarUri],
    calendar_requests: &[RadrootsCalendarRequest],
) {
    for location in locations {
        push_tag(tags, TAG_LOCATION, location);
    }
    push_optional_tag(tags, TAG_G, geohash);
    push_optional_tag(tags, TAG_SUMMARY, summary);
    push_optional_tag(tags, TAG_IMAGE, image);
    push_participants(tags, participants);
    for category in categories {
        push_tag(tags, TAG_T, category);
    }
    for reference in references {
        push_tag(tags, TAG_R, reference.as_str());
    }
    for request in calendar_requests {
        let mut tag = vec![TAG_A.to_string(), request.calendar().as_str().to_string()];
        if let Some(relay) = request.relay() {
            tag.push(relay.to_string());
        }
        tags.push(tag);
    }
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
    if let Some(event_kind) = event_kind
        && *event_kind != address.kind
    {
        return Err(EventEncodeError::InvalidField(field));
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

fn calendar_event_relays(target: &RadrootsSocialTarget) -> Option<&Vec<String>> {
    match target {
        RadrootsSocialTarget::Address {
            relays: Some(relays),
            ..
        } => Some(relays),
        _ => None,
    }
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
