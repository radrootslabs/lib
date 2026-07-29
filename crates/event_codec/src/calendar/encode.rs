#[cfg(not(feature = "std"))]
use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};

use radroots_event::{
    calendar::{
        RadrootsAuthoredCalendar, RadrootsAuthoredCalendarDateEvent,
        RadrootsAuthoredCalendarEventRsvp, RadrootsAuthoredCalendarTimeEvent,
        RadrootsCalendarEventAuthorReference, RadrootsCalendarEventFreeBusy,
        RadrootsCalendarEventReference, RadrootsCalendarEventRevisionReference,
        RadrootsCalendarEventRsvpStatus, RadrootsCalendarParticipant, RadrootsCalendarRequest,
        RadrootsCalendarUri, covered_utc_days,
    },
    envelope::kind::{
        KIND_CALENDAR, KIND_CALENDAR_DATE_EVENT, KIND_CALENDAR_EVENT_RSVP, KIND_CALENDAR_TIME_EVENT,
    },
    tag::name::{
        TAG_A, TAG_D, TAG_D_DAY, TAG_DESCRIPTION, TAG_E, TAG_END, TAG_END_TZID, TAG_FREE_BUSY,
        TAG_G, TAG_IMAGE, TAG_LOCATION, TAG_P, TAG_R, TAG_START, TAG_START_TZID, TAG_STATUS,
        TAG_SUMMARY, TAG_T, TAG_TITLE,
    },
};

use crate::error::EventEncodeError;
use crate::field_helpers::{push_optional_tag, push_tag};
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
    calendar: &RadrootsAuthoredCalendar,
) -> Result<Vec<Vec<String>>, EventEncodeError> {
    let mut tags = Vec::new();
    push_tag(&mut tags, TAG_D, calendar.uid().as_str());
    push_tag(&mut tags, TAG_TITLE, calendar.title());
    for event in calendar.event_references() {
        push_calendar_event_reference(&mut tags, event);
    }
    push_optional_tag(&mut tags, TAG_DESCRIPTION, calendar.list_description());
    push_optional_tag(
        &mut tags,
        TAG_IMAGE,
        calendar
            .image()
            .map(|image| image.descriptor().url().as_str()),
    );
    Ok(tags)
}

pub fn rsvp_build_tags(
    rsvp: &RadrootsAuthoredCalendarEventRsvp,
) -> Result<Vec<Vec<String>>, EventEncodeError> {
    let mut tags = Vec::new();
    push_tag(&mut tags, TAG_D, rsvp.uid().as_str());
    push_calendar_event_reference(&mut tags, rsvp.event_reference());
    push_tag(&mut tags, TAG_STATUS, rsvp_status_as_str(rsvp.status()));
    if let Some(reference) = rsvp.revision_reference() {
        push_calendar_revision_reference(&mut tags, reference);
    }
    if let Some(free_busy) = rsvp.observed_free_busy() {
        push_tag(&mut tags, TAG_FREE_BUSY, free_busy_as_str(free_busy));
    }
    if let Some(author_hint) = rsvp.author_hint() {
        push_calendar_author_reference(&mut tags, author_hint);
    }
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
    calendar: &RadrootsAuthoredCalendar,
) -> Result<RadrootsNip01EventWireParts, EventEncodeError> {
    calendar_to_wire_parts_with_kind(calendar, KIND_CALENDAR)
}

pub fn rsvp_to_wire_parts(
    rsvp: &RadrootsAuthoredCalendarEventRsvp,
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
    calendar: &RadrootsAuthoredCalendar,
    kind: u32,
) -> Result<RadrootsNip01EventWireParts, EventEncodeError> {
    if kind != KIND_CALENDAR {
        return Err(EventEncodeError::InvalidKind(kind));
    }
    Ok(RadrootsNip01EventWireParts {
        kind,
        content: calendar.content().to_string(),
        tags: calendar_collection_build_tags(calendar)?,
    })
}

pub fn rsvp_to_wire_parts_with_kind(
    rsvp: &RadrootsAuthoredCalendarEventRsvp,
    kind: u32,
) -> Result<RadrootsNip01EventWireParts, EventEncodeError> {
    if kind != KIND_CALENDAR_EVENT_RSVP {
        return Err(EventEncodeError::InvalidKind(kind));
    }
    Ok(RadrootsNip01EventWireParts {
        kind,
        content: rsvp.note().unwrap_or_default().to_string(),
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
    participants: Option<&Vec<RadrootsCalendarParticipant>>,
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

fn push_calendar_event_reference(
    tags: &mut Vec<Vec<String>>,
    reference: &RadrootsCalendarEventReference,
) {
    let mut tag = vec![
        TAG_A.to_string(),
        reference.coordinate().as_str().to_string(),
    ];
    if let Some(relay) = reference.relay() {
        tag.push(relay.to_string());
    }
    tags.push(tag);
}

fn push_calendar_revision_reference(
    tags: &mut Vec<Vec<String>>,
    reference: &RadrootsCalendarEventRevisionReference,
) {
    let mut tag = vec![TAG_E.to_string(), reference.event_id().to_hex()];
    if let Some(relay) = reference.relay() {
        tag.push(relay.to_string());
    }
    tags.push(tag);
}

fn push_calendar_author_reference(
    tags: &mut Vec<Vec<String>>,
    reference: &RadrootsCalendarEventAuthorReference,
) {
    let mut tag = vec![TAG_P.to_string(), reference.pubkey().to_hex()];
    if let Some(relay) = reference.relay() {
        tag.push(relay.to_string());
    }
    tags.push(tag);
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
