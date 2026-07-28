use std::{borrow::Cow, fs, path::Path};

use radroots_blossom::{BlobDescriptor, ByteVerifiedDescriptor, Error};
use radroots_event::{
    RadrootsAuthoredImage,
    calendar::{
        RADROOTS_CALENDAR_MAX_COVERED_UTC_DAYS, RadrootsAuthoredCalendar,
        RadrootsAuthoredCalendarDateEvent, RadrootsAuthoredCalendarEventRsvp,
        RadrootsAuthoredCalendarTimeEvent, RadrootsCalendarAdmissionError, RadrootsCalendarDate,
        RadrootsCalendarEventAuthorReference, RadrootsCalendarEventError,
        RadrootsCalendarEventFreeBusy, RadrootsCalendarEventReference,
        RadrootsCalendarEventRevisionReference, RadrootsCalendarEventRsvpStatus,
        RadrootsCalendarParticipant, RadrootsCalendarRequest, RadrootsCalendarUid,
        RadrootsCalendarUri, RadrootsIanaTimeZoneId, RadrootsParsedNip52Calendar,
        RadrootsParsedNip52CalendarCommon, RadrootsParsedNip52CalendarEventRsvp,
    },
    contract::validate_event_contract_parts,
};
use radroots_event_codec::{
    calendar::{
        decode::{
            admit_radroots_calendar, admit_radroots_calendar_date_event,
            admit_radroots_calendar_event_rsvp, admit_radroots_calendar_time_event,
            parse_nip52_calendar, parse_nip52_calendar_date_event, parse_nip52_calendar_event_rsvp,
            parse_nip52_calendar_time_event,
        },
        encode::{
            calendar_collection_build_tags, calendar_date_event_build_tags,
            calendar_time_event_build_tags, calendar_to_wire_parts, date_to_wire_parts,
            rsvp_build_tags, rsvp_to_wire_parts, time_to_wire_parts,
        },
    },
    error::EventParseError,
};
use serde_json::{Map, Value};

const PACKAGED_BASELINE: &str = include_str!("fixtures/calendar_nip52_baseline.v1.json");
const PACKAGED_PROFILE: &str = include_str!("fixtures/calendar_radroots_profile.v1.json");
const WORKSPACE_BASELINE_PATH: &str =
    "../../contracts/conformance/vectors/calendar/nip52_baseline.v1.json";
const WORKSPACE_PROFILE_PATH: &str =
    "../../contracts/conformance/vectors/calendar/radroots_profile.v1.json";
const WORKSPACE_CONTRACT_MARKER_PATH: &str = "../../contracts/manifest.toml";
const BASELINE_VECTOR_KIND_COUNTS: &[(&str, usize)] = &[
    ("calendar.baseline.collection.invalid", 15),
    ("calendar.baseline.collection.valid", 4),
    ("calendar.baseline.date.invalid", 8),
    ("calendar.baseline.date.valid", 3),
    ("calendar.baseline.rsvp.invalid", 23),
    ("calendar.baseline.rsvp.valid", 4),
    ("calendar.baseline.time.invalid", 4),
    ("calendar.baseline.time.valid", 4),
];
const PROFILE_VECTOR_KIND_COUNTS: &[(&str, usize)] = &[
    ("calendar.profile.admit.collection.invalid", 6),
    ("calendar.profile.admit.collection.valid", 2),
    ("calendar.profile.admit.date.invalid", 4),
    ("calendar.profile.admit.date.valid", 1),
    ("calendar.profile.admit.rsvp.invalid", 9),
    ("calendar.profile.admit.rsvp.valid", 3),
    ("calendar.profile.admit.time.invalid", 4),
    ("calendar.profile.admit.time.valid", 1),
    ("calendar.profile.authored.collection.invalid", 6),
    ("calendar.profile.authored.collection.valid", 3),
    ("calendar.profile.authored.date.invalid", 3),
    ("calendar.profile.authored.date.valid", 1),
    ("calendar.profile.authored.rsvp.invalid", 6),
    ("calendar.profile.authored.rsvp.valid", 2),
    ("calendar.profile.authored.time.coverage.valid", 1),
    ("calendar.profile.authored.time.invalid", 2),
    ("calendar.profile.authored.time.valid", 1),
    ("calendar.profile.date.parse.invalid", 2),
    ("calendar.profile.date.parse.valid", 2),
];

#[test]
fn checked_in_baseline_vectors_execute_against_tolerant_nip52_parsers() {
    let suite = conformance_suite(WORKSPACE_BASELINE_PATH, PACKAGED_BASELINE);
    assert_eq!(suite["suite"], "calendar_nip52_baseline");
    assert_eq!(suite["contract_version"], "1.0.0");
    let vectors = suite["vectors"].as_array().expect("baseline vectors");
    assert_vector_kind_inventory(vectors, BASELINE_VECTOR_KIND_COUNTS);
    for vector in vectors {
        execute_baseline(vector);
    }
}

#[test]
fn checked_in_profile_vectors_execute_against_authored_and_admission_apis() {
    let suite = conformance_suite(WORKSPACE_PROFILE_PATH, PACKAGED_PROFILE);
    assert_eq!(suite["suite"], "calendar_radroots_profile");
    assert_eq!(suite["contract_version"], "1.0.0");
    let vectors = suite["vectors"].as_array().expect("profile vectors");
    assert_vector_kind_inventory(vectors, PROFILE_VECTOR_KIND_COUNTS);
    for vector in vectors {
        execute_profile(vector);
    }
}

fn assert_vector_kind_inventory(vectors: &[Value], expected: &[(&str, usize)]) {
    assert_eq!(
        vectors.len(),
        expected.iter().map(|(_, count)| count).sum::<usize>(),
        "Calendar vector inventory count drifted"
    );
    for (kind, expected_count) in expected {
        let actual_count = vectors
            .iter()
            .filter(|vector| vector_kind(vector) == *kind)
            .count();
        assert_eq!(
            actual_count, *expected_count,
            "Calendar vector kind {kind} inventory drifted"
        );
    }
}

fn conformance_suite(workspace_relative_path: &str, packaged: &'static str) -> Value {
    let workspace_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(workspace_relative_path);
    let vectors = match fs::read_to_string(&workspace_path) {
        Ok(canonical) => {
            assert_eq!(
                canonical,
                packaged,
                "packaged Calendar vectors must match {}",
                workspace_path.display()
            );
            Cow::Owned(canonical)
        }
        Err(error)
            if error.kind() == std::io::ErrorKind::NotFound
                && !Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join(WORKSPACE_CONTRACT_MARKER_PATH)
                    .is_file() =>
        {
            Cow::Borrowed(packaged)
        }
        Err(error) => panic!("failed to read {}: {error}", workspace_path.display()),
    };
    serde_json::from_str(&vectors).expect("Calendar vectors must parse")
}

fn execute_baseline(vector: &Value) {
    match vector_kind(vector) {
        "calendar.baseline.date.valid" => baseline_date_valid(vector),
        "calendar.baseline.date.invalid" => baseline_date_invalid(vector),
        "calendar.baseline.time.valid" => baseline_time_valid(vector),
        "calendar.baseline.time.invalid" => baseline_time_invalid(vector),
        "calendar.baseline.collection.valid" => baseline_collection_valid(vector),
        "calendar.baseline.collection.invalid" => baseline_collection_invalid(vector),
        "calendar.baseline.rsvp.valid" => baseline_rsvp_valid(vector),
        "calendar.baseline.rsvp.invalid" => baseline_rsvp_invalid(vector),
        kind => panic!(
            "{} uses unsupported baseline kind {kind}",
            vector_id(vector)
        ),
    }
}

fn execute_profile(vector: &Value) {
    match vector_kind(vector) {
        "calendar.profile.date.parse.valid" => profile_date_parse_valid(vector),
        "calendar.profile.date.parse.invalid" => profile_date_parse_invalid(vector),
        "calendar.profile.authored.date.valid" => profile_authored_date_valid(vector),
        "calendar.profile.authored.date.invalid" => profile_authored_date_invalid(vector),
        "calendar.profile.authored.time.valid" => profile_authored_time_valid(vector),
        "calendar.profile.authored.time.coverage.valid" => {
            profile_authored_time_coverage_valid(vector)
        }
        "calendar.profile.authored.time.invalid" => profile_authored_time_invalid(vector),
        "calendar.profile.authored.collection.valid" => profile_authored_collection_valid(vector),
        "calendar.profile.authored.collection.invalid" => {
            profile_authored_collection_invalid(vector)
        }
        "calendar.profile.authored.rsvp.valid" => profile_authored_rsvp_valid(vector),
        "calendar.profile.authored.rsvp.invalid" => profile_authored_rsvp_invalid(vector),
        "calendar.profile.admit.date.valid" => profile_admit_date_valid(vector),
        "calendar.profile.admit.date.invalid" => profile_admit_date_invalid(vector),
        "calendar.profile.admit.time.valid" => profile_admit_time_valid(vector),
        "calendar.profile.admit.time.invalid" => profile_admit_time_invalid(vector),
        "calendar.profile.admit.collection.valid" => profile_admit_collection_valid(vector),
        "calendar.profile.admit.collection.invalid" => profile_admit_collection_invalid(vector),
        "calendar.profile.admit.rsvp.valid" => profile_admit_rsvp_valid(vector),
        "calendar.profile.admit.rsvp.invalid" => profile_admit_rsvp_invalid(vector),
        kind => panic!("{} uses unsupported profile kind {kind}", vector_id(vector)),
    }
}

fn baseline_date_valid(vector: &Value) {
    let (kind, tags, content) = inbound_parts(vector);
    let parsed = parse_nip52_calendar_date_event(kind, &tags, content)
        .unwrap_or_else(|error| panic!("{} failed: {error}", vector_id(vector)));
    let expected = expected(vector);
    assert_common(parsed.common(), expected, vector);
    assert_eq!(parsed.start().as_str(), value_str(expected, "start"));
    assert_optional_str(
        parsed.end().map(RadrootsCalendarDate::as_str),
        expected,
        "end",
        vector,
    );
    assert_eq!(
        serde_json::to_value(parsed.extension_day_tags()).unwrap(),
        expected["date_day_extensions"],
        "{}",
        vector_id(vector)
    );
}

fn baseline_time_valid(vector: &Value) {
    let (kind, tags, content) = inbound_parts(vector);
    let parsed = parse_nip52_calendar_time_event(kind, &tags, content)
        .unwrap_or_else(|error| panic!("{} failed: {error}", vector_id(vector)));
    let expected = expected(vector);
    assert_common(parsed.common(), expected, vector);
    assert_eq!(parsed.start_wire(), value_str(expected, "start_wire"));
    assert_eq!(parsed.start(), value_u64(expected, "start"));
    assert_optional_str(parsed.end_wire(), expected, "end_wire", vector);
    assert_eq!(
        parsed.end(),
        expected.get("end").and_then(Value::as_u64),
        "{}",
        vector_id(vector)
    );
    let actual_days = parsed
        .observed_day_indices()
        .iter()
        .map(|day| {
            serde_json::json!({
                "wire": day.wire_value(),
                "index": day.index(),
                "canonical": day.is_canonical(),
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(
        Value::Array(actual_days),
        expected["observed_days"],
        "{}",
        vector_id(vector)
    );
    assert_optional_str(
        parsed.start_tzid().map(RadrootsIanaTimeZoneId::as_str),
        expected,
        "start_tzid",
        vector,
    );
    assert_optional_str(
        parsed.end_tzid().map(RadrootsIanaTimeZoneId::as_str),
        expected,
        "end_tzid",
        vector,
    );
    assert_optional_str(
        parsed
            .effective_end_tzid()
            .map(RadrootsIanaTimeZoneId::as_str),
        expected,
        "effective_end_tzid",
        vector,
    );
}

fn baseline_date_invalid(vector: &Value) {
    let (kind, tags, content) = inbound_parts(vector);
    let error = parse_nip52_calendar_date_event(kind, &tags, content)
        .expect_err("invalid baseline date vector must fail");
    assert_parse_error(vector, &error);
}

fn baseline_time_invalid(vector: &Value) {
    let (kind, tags, content) = inbound_parts(vector);
    let error = parse_nip52_calendar_time_event(kind, &tags, content)
        .expect_err("invalid baseline time vector must fail");
    assert_parse_error(vector, &error);
}

fn baseline_collection_valid(vector: &Value) {
    let (kind, tags, content) = inbound_parts(vector);
    let parsed = parse_nip52_calendar(kind, &tags, content)
        .unwrap_or_else(|error| panic!("{} failed: {error}", vector_id(vector)));
    assert_parsed_collection(&parsed, expected(vector), vector);
}

fn baseline_collection_invalid(vector: &Value) {
    let (kind, tags, content) = inbound_parts(vector);
    let error = parse_nip52_calendar(kind, &tags, content)
        .expect_err("invalid baseline collection vector must fail");
    assert_parse_error(vector, &error);
}

fn baseline_rsvp_valid(vector: &Value) {
    let (kind, tags, content) = inbound_parts(vector);
    let parsed = parse_nip52_calendar_event_rsvp(kind, &tags, content)
        .unwrap_or_else(|error| panic!("{} failed: {error}", vector_id(vector)));
    assert_parsed_rsvp(&parsed, expected(vector), vector);
}

fn baseline_rsvp_invalid(vector: &Value) {
    let (kind, tags, content) = inbound_parts(vector);
    let error = parse_nip52_calendar_event_rsvp(kind, &tags, content)
        .expect_err("invalid baseline RSVP vector must fail");
    assert_parse_error(vector, &error);
}

fn assert_parsed_collection(
    parsed: &RadrootsParsedNip52Calendar,
    expected: &Value,
    vector: &Value,
) {
    assert_eq!(
        parsed.d_tag(),
        value_str(expected, "d"),
        "{}",
        vector_id(vector)
    );
    assert_eq!(
        parsed.title(),
        value_str(expected, "title"),
        "{}",
        vector_id(vector)
    );
    assert_eq!(
        parsed.content(),
        value_str(expected, "content"),
        "{}",
        vector_id(vector)
    );
    assert_optional_str(
        parsed.list_description(),
        expected,
        "list_description",
        vector,
    );
    assert_optional_str(
        parsed.image().map(RadrootsCalendarUri::as_str),
        expected,
        "image",
        vector,
    );
    assert_event_references(
        parsed.event_references(),
        expected,
        "event_references",
        vector,
    );
}

fn assert_parsed_rsvp(
    parsed: &RadrootsParsedNip52CalendarEventRsvp,
    expected: &Value,
    vector: &Value,
) {
    assert_eq!(
        parsed.d_tag(),
        value_str(expected, "d"),
        "{}",
        vector_id(vector)
    );
    assert_event_reference(
        parsed.event_reference(),
        &expected["event_reference"],
        vector,
    );
    assert_revision_reference(parsed.revision_reference(), expected, vector);
    assert_eq!(
        rsvp_status_str(parsed.status()),
        value_str(expected, "status"),
        "{}",
        vector_id(vector)
    );
    assert_optional_str(
        parsed.observed_free_busy().map(free_busy_str),
        expected,
        "observed_free_busy",
        vector,
    );
    assert_optional_str(
        parsed.effective_free_busy().map(free_busy_str),
        expected,
        "effective_free_busy",
        vector,
    );
    assert_author_reference(parsed.author_hint(), expected, vector);
    assert_optional_str(parsed.note(), expected, "note", vector);
}

fn assert_common(common: &RadrootsParsedNip52CalendarCommon, expected: &Value, vector: &Value) {
    assert_eq!(
        common.d_tag(),
        value_str(expected, "d"),
        "{}",
        vector_id(vector)
    );
    assert_eq!(
        common.title(),
        value_str(expected, "title"),
        "{}",
        vector_id(vector)
    );
    assert_optional_str(common.description(), expected, "description", vector);
    assert_eq!(
        serde_json::to_value(common.locations()).unwrap(),
        expected["locations"],
        "{}",
        vector_id(vector)
    );
    assert_optional_str(common.geohash(), expected, "geohash", vector);
    assert_optional_str(common.summary(), expected, "summary", vector);
    assert_optional_str(
        common.image().map(RadrootsCalendarUri::as_str),
        expected,
        "image",
        vector,
    );
    assert_participants(common, expected, vector);
    assert_eq!(
        serde_json::to_value(common.categories()).unwrap(),
        expected["categories"],
        "{}",
        vector_id(vector)
    );
    assert_eq!(
        common
            .references()
            .iter()
            .map(RadrootsCalendarUri::as_str)
            .collect::<Vec<_>>(),
        expected["references"]
            .as_array()
            .expect("expected references")
            .iter()
            .map(|value| value.as_str().expect("reference"))
            .collect::<Vec<_>>(),
        "{}",
        vector_id(vector)
    );
    assert_calendar_requests(common, expected, vector);
    assert_optional_str(common.legacy_name(), expected, "legacy_name", vector);
}

fn assert_participants(
    common: &RadrootsParsedNip52CalendarCommon,
    expected: &Value,
    vector: &Value,
) {
    if let Some(count) = expected["participants"].as_u64() {
        assert_eq!(
            common.participants().len() as u64,
            count,
            "{}",
            vector_id(vector)
        );
        return;
    }
    let actual = common
        .participants()
        .iter()
        .map(|participant| {
            serde_json::json!({
                "pubkey": participant.pubkey,
                "relay": participant.relay,
                "role": participant.role,
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(
        Value::Array(actual),
        expected["participants"],
        "{}",
        vector_id(vector)
    );
}

fn assert_calendar_requests(
    common: &RadrootsParsedNip52CalendarCommon,
    expected: &Value,
    vector: &Value,
) {
    if let Some(count) = expected["calendar_requests"].as_u64() {
        assert_eq!(
            common.calendar_requests().len() as u64,
            count,
            "{}",
            vector_id(vector)
        );
        return;
    }
    let actual = common
        .calendar_requests()
        .iter()
        .map(|request| {
            serde_json::json!({
                "calendar": request.calendar().as_str(),
                "relay": request.relay(),
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(
        Value::Array(actual),
        expected["calendar_requests"],
        "{}",
        vector_id(vector)
    );
}

fn assert_event_references(
    actual: &[RadrootsCalendarEventReference],
    expected: &Value,
    key: &str,
    vector: &Value,
) {
    let expected = expected[key]
        .as_array()
        .unwrap_or_else(|| panic!("{} expected.{key} must be an array", vector_id(vector)));
    assert_eq!(actual.len(), expected.len(), "{}", vector_id(vector));
    for (actual, expected) in actual.iter().zip(expected) {
        assert_event_reference(actual, expected, vector);
    }
}

fn assert_event_reference(
    actual: &RadrootsCalendarEventReference,
    expected: &Value,
    vector: &Value,
) {
    let coordinate = value_str(expected, "coordinate");
    let relay = expected.get("relay").and_then(Value::as_str);
    assert_eq!(
        actual.coordinate().as_str(),
        coordinate,
        "{}",
        vector_id(vector)
    );
    assert_eq!(actual.relay(), relay, "{}", vector_id(vector));

    let parts = radroots_event::ids::RadrootsAddressableCoordinateParts::parse(coordinate)
        .expect("expected event reference coordinate");
    let canonical_coordinate = format!("{}:{}:{}", parts.kind, parts.pubkey, parts.d_tag);
    let canonical_relay =
        relay.is_none_or(|relay| radroots_event::ids::RadrootsRelayUrl::parse(relay).is_ok());
    assert_eq!(
        actual.is_canonical(),
        coordinate == canonical_coordinate && canonical_relay,
        "{} reference canonicality",
        vector_id(vector)
    );
}

fn assert_revision_reference(
    actual: Option<&RadrootsCalendarEventRevisionReference>,
    expected: &Value,
    vector: &Value,
) {
    let Some(expected) = expected
        .get("revision_reference")
        .and_then(Value::as_object)
    else {
        assert!(actual.is_none(), "{} revision reference", vector_id(vector));
        return;
    };
    let actual = actual.unwrap_or_else(|| panic!("{} revision reference", vector_id(vector)));
    let event_id = map_str(expected, "id");
    let relay = expected.get("relay").and_then(Value::as_str);
    assert_eq!(actual.raw_event_id(), event_id, "{}", vector_id(vector));
    assert_eq!(
        actual.event_id().to_hex(),
        event_id.to_ascii_lowercase(),
        "{} normalized revision id",
        vector_id(vector)
    );
    assert_eq!(actual.relay(), relay, "{}", vector_id(vector));
    let canonical_relay =
        relay.is_none_or(|relay| radroots_event::ids::RadrootsRelayUrl::parse(relay).is_ok());
    assert_eq!(
        actual.is_canonical(),
        event_id == actual.event_id().to_hex() && canonical_relay,
        "{} revision canonicality",
        vector_id(vector)
    );
}

fn assert_author_reference(
    actual: Option<&RadrootsCalendarEventAuthorReference>,
    expected: &Value,
    vector: &Value,
) {
    let Some(expected) = expected.get("author_hint").and_then(Value::as_object) else {
        assert!(actual.is_none(), "{} author hint", vector_id(vector));
        return;
    };
    let actual = actual.unwrap_or_else(|| panic!("{} author hint", vector_id(vector)));
    let pubkey = map_str(expected, "pubkey");
    let relay = expected.get("relay").and_then(Value::as_str);
    assert_eq!(actual.raw_pubkey(), pubkey, "{}", vector_id(vector));
    assert_eq!(
        actual.pubkey().to_hex(),
        pubkey.to_ascii_lowercase(),
        "{} normalized author key",
        vector_id(vector)
    );
    assert_eq!(actual.relay(), relay, "{}", vector_id(vector));
    let canonical_relay =
        relay.is_none_or(|relay| radroots_event::ids::RadrootsRelayUrl::parse(relay).is_ok());
    assert_eq!(
        actual.is_canonical(),
        pubkey == actual.pubkey().to_hex() && canonical_relay,
        "{} author canonicality",
        vector_id(vector)
    );
}

fn rsvp_status_str(status: &RadrootsCalendarEventRsvpStatus) -> &'static str {
    match status {
        RadrootsCalendarEventRsvpStatus::Accepted => "accepted",
        RadrootsCalendarEventRsvpStatus::Declined => "declined",
        RadrootsCalendarEventRsvpStatus::Tentative => "tentative",
    }
}

fn free_busy_str(free_busy: &RadrootsCalendarEventFreeBusy) -> &'static str {
    match free_busy {
        RadrootsCalendarEventFreeBusy::Free => "free",
        RadrootsCalendarEventFreeBusy::Busy => "busy",
    }
}

fn profile_date_parse_valid(vector: &Value) {
    let parsed = RadrootsCalendarDate::parse(input_str(vector, "value"))
        .unwrap_or_else(|error| panic!("{} failed: {error}", vector_id(vector)));
    assert_eq!(parsed.as_str(), expected_str(vector, "value"));
}

fn profile_date_parse_invalid(vector: &Value) {
    let error = RadrootsCalendarDate::parse(input_str(vector, "value"))
        .expect_err("invalid profile date must fail");
    assert_calendar_event_error(vector, &error);
}

fn profile_authored_date_valid(vector: &Value) {
    let event = authored_date_event(input(vector), vector_id(vector))
        .unwrap_or_else(|error| panic!("{} failed: {error}", vector_id(vector)));
    let wire = date_to_wire_parts(&event)
        .unwrap_or_else(|error| panic!("{} failed: {error}", vector_id(vector)));
    assert_eq!(calendar_date_event_build_tags(&event).unwrap(), wire.tags);
    assert!(
        wire.tags
            .iter()
            .all(|tag| tag.first().map(String::as_str) != Some("D")),
        "{}",
        vector_id(vector)
    );
    assert_wire_parts(&wire, &expected(vector)["wire_parts"], vector_id(vector));
    assert_authored_media_claims(vector, event.image().is_some());
    assert_registry_accepts(
        wire.kind,
        &wire.tags,
        &wire.content,
        "radroots.calendar.date_event.v1",
        vector,
    );

    let parsed = parse_nip52_calendar_date_event(wire.kind, &wire.tags, &wire.content).unwrap();
    admit_radroots_calendar_date_event(parsed)
        .unwrap_or_else(|error| panic!("{} admission failed: {error}", vector_id(vector)));
}

fn profile_authored_date_invalid(vector: &Value) {
    let error = authored_date_event(input(vector), vector_id(vector))
        .expect_err("invalid authored date event must fail");
    assert_calendar_event_error(vector, &error);
}

fn profile_authored_time_valid(vector: &Value) {
    let event = authored_time_event(input(vector), vector_id(vector))
        .unwrap_or_else(|error| panic!("{} failed: {error}", vector_id(vector)));
    assert_optional_str(
        event
            .effective_end_tzid()
            .map(RadrootsIanaTimeZoneId::as_str),
        expected(vector),
        "effective_end_tzid",
        vector,
    );
    let wire = time_to_wire_parts(&event)
        .unwrap_or_else(|error| panic!("{} failed: {error}", vector_id(vector)));
    assert_eq!(calendar_time_event_build_tags(&event).unwrap(), wire.tags);
    assert_wire_parts(&wire, &expected(vector)["wire_parts"], vector_id(vector));
    assert_authored_media_claims(vector, event.image().is_some());
    assert_registry_accepts(
        wire.kind,
        &wire.tags,
        &wire.content,
        "radroots.calendar.time_event.v1",
        vector,
    );

    let parsed = parse_nip52_calendar_time_event(wire.kind, &wire.tags, &wire.content).unwrap();
    admit_radroots_calendar_time_event(parsed)
        .unwrap_or_else(|error| panic!("{} admission failed: {error}", vector_id(vector)));
}

fn profile_authored_time_coverage_valid(vector: &Value) {
    let event = authored_time_event(input(vector), vector_id(vector))
        .unwrap_or_else(|error| panic!("{} failed: {error}", vector_id(vector)));
    let wire = time_to_wire_parts(&event).unwrap();
    assert_registry_accepts(
        wire.kind,
        &wire.tags,
        &wire.content,
        "radroots.calendar.time_event.v1",
        vector,
    );
    let covered = wire
        .tags
        .iter()
        .filter(|tag| tag.first().map(String::as_str) == Some("D"))
        .collect::<Vec<_>>();
    assert_eq!(
        covered.len() as u64,
        expected_u64(vector, "count"),
        "{}",
        vector_id(vector)
    );
    assert_eq!(
        covered.len(),
        RADROOTS_CALENDAR_MAX_COVERED_UTC_DAYS as usize
    );
    assert_eq!(covered.first().unwrap()[1], expected_str(vector, "first"));
    assert_eq!(covered.last().unwrap()[1], expected_str(vector, "last"));
    for (index, tag) in covered.iter().enumerate() {
        assert_eq!(tag[0], "D", "{}", vector_id(vector));
        assert_eq!(tag[1], index.to_string(), "{}", vector_id(vector));
    }
}

fn profile_authored_time_invalid(vector: &Value) {
    let error = authored_time_event(input(vector), vector_id(vector))
        .expect_err("invalid authored time event must fail");
    assert_calendar_event_error(vector, &error);
}

fn profile_authored_collection_valid(vector: &Value) {
    let calendar = authored_collection(input(vector), vector_id(vector))
        .unwrap_or_else(|error| panic!("{} failed: {error}", vector_id(vector)));
    let wire = calendar_to_wire_parts(&calendar)
        .unwrap_or_else(|error| panic!("{} failed: {error}", vector_id(vector)));
    assert_eq!(
        calendar_collection_build_tags(&calendar).unwrap(),
        wire.tags,
        "{}",
        vector_id(vector)
    );
    assert_wire_parts(&wire, &expected(vector)["wire_parts"], vector_id(vector));
    assert_authored_media_claims(vector, calendar.image().is_some());
    assert_registry_accepts(
        wire.kind,
        &wire.tags,
        &wire.content,
        "radroots.calendar.collection.v1",
        vector,
    );

    let parsed = parse_nip52_calendar(wire.kind, &wire.tags, &wire.content).unwrap();
    assert_eq!(
        parsed.d_tag(),
        calendar.uid().as_str(),
        "{}",
        vector_id(vector)
    );
    assert_eq!(parsed.title(), calendar.title(), "{}", vector_id(vector));
    assert_eq!(
        parsed.content(),
        calendar.content(),
        "{}",
        vector_id(vector)
    );
    assert_eq!(
        parsed.event_references(),
        calendar.event_references(),
        "{}",
        vector_id(vector)
    );
    assert_eq!(
        parsed.list_description(),
        calendar.list_description(),
        "{}",
        vector_id(vector)
    );
    assert_eq!(
        parsed.image().map(RadrootsCalendarUri::as_str),
        calendar
            .image()
            .map(|image| image.descriptor().url().as_str()),
        "{}",
        vector_id(vector)
    );
    assert!(
        parsed
            .event_references()
            .iter()
            .all(RadrootsCalendarEventReference::is_canonical),
        "{}",
        vector_id(vector)
    );
    let admitted = admit_radroots_calendar(parsed)
        .unwrap_or_else(|error| panic!("{} admission failed: {error}", vector_id(vector)));
    assert_eq!(admitted.uid(), calendar.uid(), "{}", vector_id(vector));
    assert_eq!(
        admitted.blossom_image().is_some(),
        calendar.image().is_some(),
        "{}",
        vector_id(vector)
    );
}

fn profile_authored_collection_invalid(vector: &Value) {
    if expected_str(vector, "error") == "blob_hash_mismatch" {
        let image = &input(vector)["image"];
        let error = verified_descriptor_result(image).expect_err("invalid image must fail");
        assert_eq!(
            error.code(),
            expected_str(vector, "error"),
            "{}",
            vector_id(vector)
        );
        return;
    }
    let error = authored_collection(input(vector), vector_id(vector))
        .expect_err("invalid authored collection must fail");
    assert_calendar_event_error(vector, &error);
}

fn profile_authored_rsvp_valid(vector: &Value) {
    let rsvp = authored_rsvp(input(vector))
        .unwrap_or_else(|error| panic!("{} failed: {error}", vector_id(vector)));
    let wire = rsvp_to_wire_parts(&rsvp)
        .unwrap_or_else(|error| panic!("{} failed: {error}", vector_id(vector)));
    assert_eq!(
        rsvp_build_tags(&rsvp).unwrap(),
        wire.tags,
        "{}",
        vector_id(vector)
    );
    assert_wire_parts(&wire, &expected(vector)["wire_parts"], vector_id(vector));
    assert_registry_accepts(
        wire.kind,
        &wire.tags,
        &wire.content,
        "radroots.calendar.rsvp.v1",
        vector,
    );
    assert_optional_str(
        rsvp.observed_free_busy().map(free_busy_str),
        expected(vector),
        "observed_free_busy",
        vector,
    );
    assert_optional_str(
        rsvp.effective_free_busy().map(free_busy_str),
        expected(vector),
        "effective_free_busy",
        vector,
    );

    let parsed = parse_nip52_calendar_event_rsvp(wire.kind, &wire.tags, &wire.content).unwrap();
    assert_eq!(parsed.d_tag(), rsvp.uid().as_str(), "{}", vector_id(vector));
    assert_eq!(
        parsed.event_reference(),
        rsvp.event_reference(),
        "{}",
        vector_id(vector)
    );
    assert_eq!(
        parsed.revision_reference(),
        rsvp.revision_reference(),
        "{}",
        vector_id(vector)
    );
    assert_eq!(parsed.status(), rsvp.status(), "{}", vector_id(vector));
    assert_eq!(
        parsed.observed_free_busy(),
        rsvp.observed_free_busy(),
        "{}",
        vector_id(vector)
    );
    assert_eq!(
        parsed.effective_free_busy(),
        rsvp.effective_free_busy(),
        "{}",
        vector_id(vector)
    );
    assert_eq!(
        parsed.author_hint(),
        rsvp.author_hint(),
        "{}",
        vector_id(vector)
    );
    assert_eq!(parsed.note(), rsvp.note(), "{}", vector_id(vector));
    let admitted = admit_radroots_calendar_event_rsvp(parsed)
        .unwrap_or_else(|error| panic!("{} admission failed: {error}", vector_id(vector)));
    assert_eq!(admitted.uid(), rsvp.uid(), "{}", vector_id(vector));
}

fn profile_authored_rsvp_invalid(vector: &Value) {
    let error = authored_rsvp(input(vector)).expect_err("invalid authored RSVP must fail");
    assert_calendar_event_error(vector, &error);
}

fn profile_admit_date_valid(vector: &Value) {
    let (kind, tags, content) = inbound_parts(vector);
    let parsed = parse_nip52_calendar_date_event(kind, &tags, content).unwrap();
    let admitted = admit_radroots_calendar_date_event(parsed)
        .unwrap_or_else(|error| panic!("{} failed: {error}", vector_id(vector)));
    assert_eq!(admitted.d_tag().as_str(), expected_str(vector, "d"));
    assert_eq!(
        admitted.blossom_image().is_some(),
        expected(vector)["blossom_image"].as_bool().unwrap()
    );
    assert_registry_accepts(
        kind,
        &tags,
        content,
        "radroots.calendar.date_event.v1",
        vector,
    );
}

fn profile_admit_date_invalid(vector: &Value) {
    let (kind, tags, content) = inbound_parts(vector);
    let parsed = parse_nip52_calendar_date_event(kind, &tags, content).unwrap();
    let error = admit_radroots_calendar_date_event(parsed)
        .expect_err("invalid date admission vector must fail");
    assert_admission_error(vector, &error);
    assert_registry_rejects(
        kind,
        &tags,
        content,
        "radroots.calendar.date_event.v1",
        vector,
    );
}

fn profile_admit_time_valid(vector: &Value) {
    let (kind, tags, content) = inbound_parts(vector);
    let parsed = parse_nip52_calendar_time_event(kind, &tags, content).unwrap();
    let admitted = admit_radroots_calendar_time_event(parsed)
        .unwrap_or_else(|error| panic!("{} failed: {error}", vector_id(vector)));
    assert_eq!(admitted.d_tag().as_str(), expected_str(vector, "d"));
    assert_eq!(
        serde_json::to_value(admitted.covered_utc_days()).unwrap(),
        expected(vector)["covered_days"]
    );
    assert_eq!(
        admitted.blossom_image().is_some(),
        expected(vector)["blossom_image"].as_bool().unwrap()
    );
    assert_registry_accepts(
        kind,
        &tags,
        content,
        "radroots.calendar.time_event.v1",
        vector,
    );
}

fn assert_registry_accepts(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
    contract_id: &str,
    vector: &Value,
) {
    validate_event_contract_parts(kind, tags, content, contract_id)
        .unwrap_or_else(|error| panic!("{} registry drift: {error:?}", vector_id(vector)));
}

fn assert_registry_rejects(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
    contract_id: &str,
    vector: &Value,
) {
    assert!(
        validate_event_contract_parts(kind, tags, content, contract_id).is_err(),
        "{} registry accepted an admission-invalid event",
        vector_id(vector)
    );
}

fn profile_admit_time_invalid(vector: &Value) {
    let (kind, tags, content) = inbound_parts(vector);
    let parsed = parse_nip52_calendar_time_event(kind, &tags, content).unwrap();
    let error = admit_radroots_calendar_time_event(parsed)
        .expect_err("invalid time admission vector must fail");
    assert_admission_error(vector, &error);
    assert_registry_rejects(
        kind,
        &tags,
        content,
        "radroots.calendar.time_event.v1",
        vector,
    );
}

fn profile_admit_collection_valid(vector: &Value) {
    let (kind, tags, content) = inbound_parts(vector);
    let parsed = parse_nip52_calendar(kind, &tags, content)
        .unwrap_or_else(|error| panic!("{} parse failed: {error}", vector_id(vector)));
    let admitted = admit_radroots_calendar(parsed)
        .unwrap_or_else(|error| panic!("{} admission failed: {error}", vector_id(vector)));
    let expected = expected(vector);
    assert_eq!(
        admitted.uid().as_str(),
        value_str(expected, "d"),
        "{}",
        vector_id(vector)
    );
    let title = tags
        .iter()
        .find(|tag| tag.first().map(String::as_str) == Some("title"))
        .and_then(|tag| tag.get(1))
        .expect("admitted collection title tag");
    assert_eq!(admitted.title(), title, "{}", vector_id(vector));
    assert_eq!(admitted.content(), content, "{}", vector_id(vector));
    assert_optional_str(
        admitted.list_description(),
        expected,
        "list_description",
        vector,
    );
    assert_event_references(
        admitted.event_references(),
        expected,
        "event_references",
        vector,
    );
    assert_eq!(
        admitted.blossom_image().is_some(),
        expected["blossom_image"].as_bool().unwrap(),
        "{}",
        vector_id(vector)
    );
    let image = tags
        .iter()
        .find(|tag| tag.first().map(String::as_str) == Some("image"))
        .and_then(|tag| tag.get(1))
        .map(String::as_str);
    assert_eq!(
        admitted.parsed().image().map(RadrootsCalendarUri::as_str),
        image,
        "{}",
        vector_id(vector)
    );
    if let Some(level) = expected.get("media_verification") {
        assert_eq!(level, "structural_only", "{}", vector_id(vector));
        assert!(admitted.blossom_image().is_some(), "{}", vector_id(vector));
    }
    assert_registry_accepts(
        kind,
        &tags,
        content,
        "radroots.calendar.collection.v1",
        vector,
    );
}

fn profile_admit_collection_invalid(vector: &Value) {
    let (kind, tags, content) = inbound_parts(vector);
    let parsed = parse_nip52_calendar(kind, &tags, content)
        .unwrap_or_else(|error| panic!("{} parse failed: {error}", vector_id(vector)));
    let error =
        admit_radroots_calendar(parsed).expect_err("invalid collection admission vector must fail");
    assert_admission_error(vector, &error);
    assert_registry_rejects(
        kind,
        &tags,
        content,
        "radroots.calendar.collection.v1",
        vector,
    );
}

fn profile_admit_rsvp_valid(vector: &Value) {
    let (kind, tags, content) = inbound_parts(vector);
    let parsed = parse_nip52_calendar_event_rsvp(kind, &tags, content)
        .unwrap_or_else(|error| panic!("{} parse failed: {error}", vector_id(vector)));
    let admitted = admit_radroots_calendar_event_rsvp(parsed)
        .unwrap_or_else(|error| panic!("{} admission failed: {error}", vector_id(vector)));
    let expected = expected(vector);
    assert_eq!(
        admitted.uid().as_str(),
        value_str(expected, "d"),
        "{}",
        vector_id(vector)
    );
    assert_eq!(
        rsvp_status_str(admitted.status()),
        value_str(expected, "status"),
        "{}",
        vector_id(vector)
    );
    assert_optional_str(
        admitted.observed_free_busy().map(free_busy_str),
        expected,
        "observed_free_busy",
        vector,
    );
    assert_optional_str(
        admitted.effective_free_busy().map(free_busy_str),
        expected,
        "effective_free_busy",
        vector,
    );
    assert_admitted_rsvp_references_against_tags(&admitted, &tags, vector);
    assert_eq!(
        admitted.note(),
        (!content.is_empty()).then_some(content),
        "{}",
        vector_id(vector)
    );
    if expected.get("event_reference").is_some() {
        assert_event_reference(
            admitted.event_reference(),
            &expected["event_reference"],
            vector,
        );
    }
    if expected.get("revision_reference").is_some() {
        assert_revision_reference(admitted.revision_reference(), expected, vector);
    }
    if expected.get("author_hint").is_some() {
        assert_author_reference(admitted.author_hint(), expected, vector);
    }
    assert_registry_accepts(kind, &tags, content, "radroots.calendar.rsvp.v1", vector);
}

fn assert_admitted_rsvp_references_against_tags(
    admitted: &radroots_event::calendar::RadrootsAdmittedCalendarEventRsvp,
    tags: &[Vec<String>],
    vector: &Value,
) {
    let event_tag = tags
        .iter()
        .find(|tag| tag.first().map(String::as_str) == Some("a"))
        .expect("admitted RSVP a tag");
    assert_eq!(
        admitted.event_reference().coordinate().as_str(),
        event_tag[1],
        "{}",
        vector_id(vector)
    );
    assert_eq!(
        admitted.event_reference().relay(),
        event_tag.get(2).map(String::as_str),
        "{}",
        vector_id(vector)
    );
    assert!(
        admitted.event_reference().is_canonical(),
        "{}",
        vector_id(vector)
    );

    let revision_tag = tags
        .iter()
        .find(|tag| tag.first().map(String::as_str) == Some("e"));
    match (admitted.revision_reference(), revision_tag) {
        (Some(reference), Some(tag)) => {
            assert_eq!(reference.raw_event_id(), tag[1], "{}", vector_id(vector));
            assert_eq!(reference.relay(), tag.get(2).map(String::as_str));
            assert!(reference.is_canonical(), "{}", vector_id(vector));
        }
        (None, None) => {}
        _ => panic!("{} revision reference drift", vector_id(vector)),
    }

    let author_tag = tags
        .iter()
        .find(|tag| tag.first().map(String::as_str) == Some("p"));
    match (admitted.author_hint(), author_tag) {
        (Some(reference), Some(tag)) => {
            assert_eq!(reference.raw_pubkey(), tag[1], "{}", vector_id(vector));
            assert_eq!(reference.relay(), tag.get(2).map(String::as_str));
            assert!(reference.is_canonical(), "{}", vector_id(vector));
        }
        (None, None) => {}
        _ => panic!("{} author hint drift", vector_id(vector)),
    }
}

fn profile_admit_rsvp_invalid(vector: &Value) {
    let (kind, tags, content) = inbound_parts(vector);
    let parsed = parse_nip52_calendar_event_rsvp(kind, &tags, content)
        .unwrap_or_else(|error| panic!("{} parse failed: {error}", vector_id(vector)));
    let error = admit_radroots_calendar_event_rsvp(parsed)
        .expect_err("invalid RSVP admission vector must fail");
    assert_admission_error(vector, &error);
    assert_registry_rejects(kind, &tags, content, "radroots.calendar.rsvp.v1", vector);
}

fn authored_collection(
    input: &Map<String, Value>,
    vector_id: &str,
) -> Result<RadrootsAuthoredCalendar, RadrootsCalendarEventError> {
    let uid = RadrootsCalendarUid::parse(map_str(input, "d"))?;
    let event_references = input["event_references"]
        .as_array()
        .expect("input.event_references")
        .iter()
        .map(calendar_event_reference)
        .collect::<Result<Vec<_>, _>>()?;
    let mut calendar = RadrootsAuthoredCalendar::new(
        uid,
        map_str(input, "title"),
        map_optional_str(input, "content").unwrap_or_default(),
        event_references,
    )?;
    if let Some(description) = map_optional_str(input, "list_description") {
        calendar = calendar.with_list_description(description)?;
    }
    if let Some(image) = input.get("image") {
        calendar = calendar.with_image(authored_image(image, vector_id))?;
    }
    Ok(calendar)
}

fn authored_rsvp(
    input: &Map<String, Value>,
) -> Result<RadrootsAuthoredCalendarEventRsvp, RadrootsCalendarEventError> {
    let uid = RadrootsCalendarUid::parse(map_str(input, "d"))?;
    let event_reference = calendar_event_reference(&input["event_reference"])?;
    let mut rsvp = RadrootsAuthoredCalendarEventRsvp::new(
        uid,
        event_reference,
        parse_authored_rsvp_status(map_str(input, "status")),
    )?;
    if let Some(reference) = input.get("revision_reference") {
        rsvp = rsvp.with_revision_reference(calendar_revision_reference(reference)?)?;
    }
    if let Some(free_busy) = map_optional_str(input, "free_busy") {
        rsvp = rsvp.with_free_busy(parse_authored_free_busy(free_busy))?;
    }
    if let Some(author_hint) = input.get("author_hint") {
        rsvp = rsvp.with_author_hint(calendar_author_reference(author_hint)?)?;
    }
    if let Some(note) = map_optional_str(input, "note") {
        rsvp = rsvp.with_note(note)?;
    }
    Ok(rsvp)
}

fn calendar_event_reference(
    value: &Value,
) -> Result<RadrootsCalendarEventReference, RadrootsCalendarEventError> {
    RadrootsCalendarEventReference::parse(
        value["coordinate"]
            .as_str()
            .expect("event_reference.coordinate"),
        value.get("relay").and_then(Value::as_str),
    )
}

fn calendar_revision_reference(
    value: &Value,
) -> Result<RadrootsCalendarEventRevisionReference, RadrootsCalendarEventError> {
    RadrootsCalendarEventRevisionReference::parse(
        value["id"].as_str().expect("revision_reference.id"),
        value.get("relay").and_then(Value::as_str),
    )
}

fn calendar_author_reference(
    value: &Value,
) -> Result<RadrootsCalendarEventAuthorReference, RadrootsCalendarEventError> {
    RadrootsCalendarEventAuthorReference::parse(
        value["pubkey"].as_str().expect("author_hint.pubkey"),
        value.get("relay").and_then(Value::as_str),
    )
}

fn parse_authored_rsvp_status(value: &str) -> RadrootsCalendarEventRsvpStatus {
    match value {
        "accepted" => RadrootsCalendarEventRsvpStatus::Accepted,
        "declined" => RadrootsCalendarEventRsvpStatus::Declined,
        "tentative" => RadrootsCalendarEventRsvpStatus::Tentative,
        value => panic!("unsupported authored RSVP status {value}"),
    }
}

fn parse_authored_free_busy(value: &str) -> RadrootsCalendarEventFreeBusy {
    match value {
        "free" => RadrootsCalendarEventFreeBusy::Free,
        "busy" => RadrootsCalendarEventFreeBusy::Busy,
        value => panic!("unsupported authored RSVP free/busy value {value}"),
    }
}

fn authored_date_event(
    input: &Map<String, Value>,
    vector_id: &str,
) -> Result<RadrootsAuthoredCalendarDateEvent, RadrootsCalendarEventError> {
    let start = RadrootsCalendarDate::parse(map_str(input, "start"))?;
    let mut event = RadrootsAuthoredCalendarDateEvent::new(
        map_str(input, "d"),
        map_str(input, "title"),
        start,
    )?;
    if let Some(value) = map_optional_str(input, "end") {
        event = event.with_end(RadrootsCalendarDate::parse(value)?)?;
    }
    event = apply_authored_date_common(event, input, vector_id)?;
    Ok(event)
}

fn apply_authored_date_common(
    mut event: RadrootsAuthoredCalendarDateEvent,
    input: &Map<String, Value>,
    vector_id: &str,
) -> Result<RadrootsAuthoredCalendarDateEvent, RadrootsCalendarEventError> {
    if let Some(value) = map_optional_str(input, "description") {
        event = event.with_description(value)?;
    }
    if let Some(value) = map_optional_string_vec(input, "locations") {
        event = event.with_locations(value)?;
    }
    if let Some(value) = map_optional_str(input, "geohash") {
        event = event.with_geohash(value)?;
    }
    if let Some(value) = map_optional_str(input, "summary") {
        event = event.with_summary(value)?;
    }
    if let Some(value) = input.get("image") {
        event = event.with_image(authored_image(value, vector_id))?;
    }
    if let Some(value) = input.get("participants") {
        event = event.with_participants(participants(value, vector_id))?;
    }
    if let Some(value) = map_optional_string_vec(input, "categories") {
        event = event.with_categories(value)?;
    }
    if let Some(value) = map_optional_string_vec(input, "references") {
        event = event.with_references(
            value
                .into_iter()
                .map(RadrootsCalendarUri::parse)
                .collect::<Result<Vec<_>, _>>()?,
        )?;
    }
    if let Some(value) = input.get("calendar_requests") {
        event = event.with_calendar_requests(calendar_requests(value, vector_id))?;
    }
    Ok(event)
}

fn authored_time_event(
    input: &Map<String, Value>,
    vector_id: &str,
) -> Result<RadrootsAuthoredCalendarTimeEvent, RadrootsCalendarEventError> {
    let mut event = RadrootsAuthoredCalendarTimeEvent::new(
        map_str(input, "d"),
        map_str(input, "title"),
        map_u64(input, "start"),
    )?;
    if let Some(value) = input.get("end").and_then(Value::as_u64) {
        event = event.with_end(value)?;
    }
    if let Some(value) = map_optional_str(input, "description") {
        event = event.with_description(value)?;
    }
    if let Some(value) = map_optional_str(input, "start_tzid") {
        event = event.with_start_tzid(value)?;
    }
    if let Some(value) = map_optional_str(input, "end_tzid") {
        event = event.with_end_tzid(value)?;
    }
    if let Some(value) = map_optional_string_vec(input, "locations") {
        event = event.with_locations(value)?;
    }
    if let Some(value) = map_optional_str(input, "geohash") {
        event = event.with_geohash(value)?;
    }
    if let Some(value) = map_optional_str(input, "summary") {
        event = event.with_summary(value)?;
    }
    if let Some(value) = input.get("image") {
        event = event.with_image(authored_image(value, vector_id))?;
    }
    if let Some(value) = input.get("participants") {
        event = event.with_participants(participants(value, vector_id))?;
    }
    if let Some(value) = map_optional_string_vec(input, "categories") {
        event = event.with_categories(value)?;
    }
    if let Some(value) = map_optional_string_vec(input, "references") {
        event = event.with_references(
            value
                .into_iter()
                .map(RadrootsCalendarUri::parse)
                .collect::<Result<Vec<_>, _>>()?,
        )?;
    }
    if let Some(value) = input.get("calendar_requests") {
        event = event.with_calendar_requests(calendar_requests(value, vector_id))?;
    }
    Ok(event)
}

fn authored_image(input: &Value, vector_id: &str) -> RadrootsAuthoredImage {
    RadrootsAuthoredImage::try_from(verified_descriptor(input, vector_id))
        .unwrap_or_else(|error| panic!("{vector_id} image failed: {error}"))
}

fn verified_descriptor(input: &Value, vector_id: &str) -> ByteVerifiedDescriptor {
    verified_descriptor_result(input)
        .unwrap_or_else(|error| panic!("{vector_id} byte verification failed: {error}"))
}

fn verified_descriptor_result(input: &Value) -> Result<ByteVerifiedDescriptor, Error> {
    let descriptor: BlobDescriptor =
        serde_json::from_value(input["descriptor"].clone()).expect("image descriptor must parse");
    let media_type = descriptor.media_type().clone();
    descriptor.approve_reference()?.verify_bytes(
        input["bytes_utf8"]
            .as_str()
            .expect("image.bytes_utf8")
            .as_bytes(),
        &media_type,
    )
}

fn participants(input: &Value, vector_id: &str) -> Vec<RadrootsCalendarParticipant> {
    input
        .as_array()
        .unwrap_or_else(|| panic!("{vector_id} participants must be an array"))
        .iter()
        .map(|value| RadrootsCalendarParticipant {
            pubkey: value["pubkey"]
                .as_str()
                .expect("participant.pubkey")
                .to_string(),
            relay: value
                .get("relay")
                .and_then(Value::as_str)
                .map(str::to_string),
            role: value
                .get("role")
                .and_then(Value::as_str)
                .map(str::to_string),
        })
        .collect()
}

fn calendar_requests(input: &Value, vector_id: &str) -> Vec<RadrootsCalendarRequest> {
    input
        .as_array()
        .unwrap_or_else(|| panic!("{vector_id} calendar_requests must be an array"))
        .iter()
        .map(|value| {
            RadrootsCalendarRequest::new(
                value["calendar"].as_str().expect("request.calendar"),
                value.get("relay").and_then(Value::as_str),
            )
            .unwrap_or_else(|error| panic!("{vector_id} request failed: {error}"))
        })
        .collect()
}

fn assert_authored_media_claims(vector: &Value, image_expected: bool) {
    let expected = expected(vector);
    let Some(verification) = expected.get("media_verification") else {
        assert!(
            !image_expected,
            "{} omits its media claim",
            vector_id(vector)
        );
        return;
    };
    assert!(image_expected, "{} claims absent media", vector_id(vector));
    assert_eq!(verification, "byte_verified", "{}", vector_id(vector));
    assert_eq!(
        expected_str(vector, "upload_completion"),
        "not_attested_by_codec",
        "{}",
        vector_id(vector)
    );
}

fn assert_calendar_event_error(vector: &Value, error: &RadrootsCalendarEventError) {
    assert_eq!(
        error.code(),
        expected_str(vector, "error"),
        "{}",
        vector_id(vector)
    );
    assert!(!error.to_string().is_empty());
    if let RadrootsCalendarEventError::CoveredDayLimitExceeded { max, actual } = error {
        assert_eq!(*max, expected_u64(vector, "max"));
        assert_eq!(*actual, expected_u64(vector, "actual"));
    }
}

fn assert_admission_error(vector: &Value, error: &RadrootsCalendarAdmissionError) {
    assert_eq!(
        error.code(),
        expected_str(vector, "error"),
        "{}",
        vector_id(vector)
    );
    assert!(!error.to_string().is_empty());
    if let Some(expected_field) = expected(vector).get("field").and_then(Value::as_str) {
        let RadrootsCalendarAdmissionError::NonCanonicalField(actual_field) = error else {
            panic!(
                "{} expected a field-bearing admission error",
                vector_id(vector)
            );
        };
        assert_eq!(*actual_field, expected_field, "{}", vector_id(vector));
    }
    if let RadrootsCalendarAdmissionError::CoveredDayLimitExceeded { max, actual } = error {
        assert_eq!(*max, expected_u64(vector, "max"));
        assert_eq!(*actual, expected_u64(vector, "actual"));
    }
}

fn assert_parse_error(vector: &Value, error: &EventParseError) {
    assert_eq!(
        error.code(),
        expected_str(vector, "error"),
        "{}",
        vector_id(vector)
    );
    assert!(!error.to_string().is_empty());
    if let Some(expected_tag) = expected(vector).get("tag").and_then(Value::as_str) {
        assert_eq!(
            parse_error_tag(error),
            Some(expected_tag),
            "{}",
            vector_id(vector)
        );
    }
}

fn parse_error_tag(error: &EventParseError) -> Option<&'static str> {
    match error {
        EventParseError::MissingTag(tag)
        | EventParseError::InvalidTag(tag)
        | EventParseError::DuplicateTag(tag)
        | EventParseError::InvalidNumber(tag, _)
        | EventParseError::InvalidJson(tag) => Some(tag),
        EventParseError::InvalidEnvelope | EventParseError::InvalidKind { .. } => None,
    }
}

fn assert_wire_parts(
    wire: &radroots_event::wire::RadrootsNip01EventWireParts,
    expected: &Value,
    vector_id: &str,
) {
    assert_eq!(
        u64::from(wire.kind),
        expected["kind"].as_u64().unwrap(),
        "{vector_id}"
    );
    assert_eq!(
        wire.content,
        expected["content"].as_str().unwrap(),
        "{vector_id}"
    );
    assert_eq!(
        serde_json::to_value(&wire.tags).unwrap(),
        expected["tags"],
        "{vector_id}"
    );
}

fn inbound_parts(vector: &Value) -> (u32, Vec<Vec<String>>, &str) {
    let input = input(vector);
    let kind = map_u64(input, "kind") as u32;
    let tags = serde_json::from_value(input["tags"].clone())
        .unwrap_or_else(|error| panic!("{} tags failed: {error}", vector_id(vector)));
    let content = map_str(input, "content");
    (kind, tags, content)
}

fn assert_optional_str(actual: Option<&str>, expected: &Value, key: &str, vector: &Value) {
    assert_eq!(
        actual,
        expected.get(key).and_then(Value::as_str),
        "{} expected.{key}",
        vector_id(vector)
    );
}

fn input(vector: &Value) -> &Map<String, Value> {
    vector["input"].as_object().expect("vector.input")
}

fn expected(vector: &Value) -> &Value {
    &vector["expected"]
}

fn vector_id(vector: &Value) -> &str {
    vector["id"].as_str().expect("vector.id")
}

fn vector_kind(vector: &Value) -> &str {
    vector["kind"].as_str().expect("vector.kind")
}

fn input_str<'a>(vector: &'a Value, field: &str) -> &'a str {
    map_str(input(vector), field)
}

fn expected_str<'a>(vector: &'a Value, field: &str) -> &'a str {
    expected(vector)[field]
        .as_str()
        .unwrap_or_else(|| panic!("{} expected.{field}", vector_id(vector)))
}

fn expected_u64(vector: &Value, field: &str) -> u64 {
    expected(vector)[field]
        .as_u64()
        .unwrap_or_else(|| panic!("{} expected.{field}", vector_id(vector)))
}

fn value_str<'a>(value: &'a Value, field: &str) -> &'a str {
    value[field]
        .as_str()
        .unwrap_or_else(|| panic!("expected.{field} must be a string"))
}

fn value_u64(value: &Value, field: &str) -> u64 {
    value[field]
        .as_u64()
        .unwrap_or_else(|| panic!("expected.{field} must be an integer"))
}

fn map_str<'a>(map: &'a Map<String, Value>, field: &str) -> &'a str {
    map[field]
        .as_str()
        .unwrap_or_else(|| panic!("input.{field} must be a string"))
}

fn map_optional_str<'a>(map: &'a Map<String, Value>, field: &str) -> Option<&'a str> {
    map.get(field).and_then(Value::as_str)
}

fn map_u64(map: &Map<String, Value>, field: &str) -> u64 {
    map[field]
        .as_u64()
        .unwrap_or_else(|| panic!("input.{field} must be an integer"))
}

fn map_optional_string_vec(map: &Map<String, Value>, field: &str) -> Option<Vec<String>> {
    map.get(field).map(|value| {
        value
            .as_array()
            .unwrap_or_else(|| panic!("input.{field} must be an array"))
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .unwrap_or_else(|| panic!("input.{field} values must be strings"))
                    .to_string()
            })
            .collect()
    })
}
