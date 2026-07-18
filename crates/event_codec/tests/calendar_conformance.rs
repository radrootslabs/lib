use std::{borrow::Cow, fs, path::Path};

use radroots_blossom::{RadrootsBlossomBlobDescriptor, RadrootsBlossomByteVerifiedDescriptor};
use radroots_event::{
    RadrootsAuthoredImage,
    calendar::{
        RADROOTS_CALENDAR_MAX_COVERED_UTC_DAYS, RadrootsAuthoredCalendarDateEvent,
        RadrootsAuthoredCalendarTimeEvent, RadrootsCalendarAdmissionError, RadrootsCalendarDate,
        RadrootsCalendarEventError, RadrootsCalendarRequest, RadrootsCalendarUri,
        RadrootsIanaTimeZoneId, RadrootsParsedNip52CalendarCommon,
    },
    contract::validate_event_contract_parts,
    social::RadrootsCalendarParticipant,
};
use radroots_event_codec::{
    calendar::{
        decode::{
            admit_radroots_calendar_date_event, admit_radroots_calendar_time_event,
            parse_nip52_calendar_date_event, parse_nip52_calendar_time_event,
        },
        encode::{
            calendar_date_event_build_tags, calendar_time_event_build_tags, date_to_wire_parts,
            time_to_wire_parts,
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

#[test]
fn checked_in_baseline_vectors_execute_against_tolerant_nip52_parsers() {
    let suite = conformance_suite(WORKSPACE_BASELINE_PATH, PACKAGED_BASELINE);
    assert_eq!(suite["suite"], "calendar_nip52_baseline");
    assert_eq!(suite["contract_version"], "0.1.0");
    let vectors = suite["vectors"].as_array().expect("baseline vectors");
    assert!(!vectors.is_empty());
    for vector in vectors {
        execute_baseline(vector);
    }
}

#[test]
fn checked_in_profile_vectors_execute_against_authored_and_admission_apis() {
    let suite = conformance_suite(WORKSPACE_PROFILE_PATH, PACKAGED_PROFILE);
    assert_eq!(suite["suite"], "calendar_radroots_profile");
    assert_eq!(suite["contract_version"], "0.1.0");
    let vectors = suite["vectors"].as_array().expect("profile vectors");
    assert!(!vectors.is_empty());
    for vector in vectors {
        execute_profile(vector);
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
        "calendar.profile.admit.date.valid" => profile_admit_date_valid(vector),
        "calendar.profile.admit.date.invalid" => profile_admit_date_invalid(vector),
        "calendar.profile.admit.time.valid" => profile_admit_time_valid(vector),
        "calendar.profile.admit.time.invalid" => profile_admit_time_invalid(vector),
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

fn profile_admit_time_invalid(vector: &Value) {
    let (kind, tags, content) = inbound_parts(vector);
    let parsed = parse_nip52_calendar_time_event(kind, &tags, content).unwrap();
    let error = admit_radroots_calendar_time_event(parsed)
        .expect_err("invalid time admission vector must fail");
    assert_admission_error(vector, &error);
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

fn verified_descriptor(input: &Value, vector_id: &str) -> RadrootsBlossomByteVerifiedDescriptor {
    let descriptor: RadrootsBlossomBlobDescriptor =
        serde_json::from_value(input["descriptor"].clone())
            .unwrap_or_else(|error| panic!("{vector_id} descriptor failed: {error}"));
    let media_type = descriptor.media_type().clone();
    descriptor
        .approve_reference()
        .unwrap_or_else(|error| panic!("{vector_id} descriptor approval failed: {error}"))
        .verify_bytes(
            input["bytes_utf8"]
                .as_str()
                .expect("image.bytes_utf8")
                .as_bytes(),
            &media_type,
        )
        .unwrap_or_else(|error| panic!("{vector_id} byte verification failed: {error}"))
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
