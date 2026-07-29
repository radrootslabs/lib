use radroots_blossom::{BlobDescriptor, BlobUrl, MediaType, Sha256};
use radroots_event::{
    RadrootsAuthoredImage, RadrootsEventTags,
    calendar::{
        RADROOTS_CALENDAR_MAX_COVERED_UTC_DAYS, RADROOTS_CALENDAR_SECONDS_PER_DAY,
        RadrootsAuthoredCalendar, RadrootsAuthoredCalendarDateEvent,
        RadrootsAuthoredCalendarEventRsvp, RadrootsAuthoredCalendarTimeEvent,
        RadrootsCalendarAdmissionError, RadrootsCalendarDate, RadrootsCalendarEventError,
        RadrootsCalendarEventReference, RadrootsCalendarEventRsvpStatus,
        RadrootsCalendarParticipant, RadrootsCalendarUid, RadrootsIanaTimeZoneId, covered_utc_days,
    },
    kinds::{
        KIND_CALENDAR, KIND_CALENDAR_DATE_EVENT, KIND_CALENDAR_EVENT_RSVP,
        KIND_CALENDAR_TIME_EVENT, KIND_POST,
    },
    tags::{
        TAG_A, TAG_D, TAG_D_DAY, TAG_END, TAG_END_TZID, TAG_G, TAG_IMAGE, TAG_LOCATION, TAG_P,
        TAG_R, TAG_START, TAG_START_TZID, TAG_SUMMARY, TAG_T, TAG_TITLE,
    },
    wire::{DEFAULT_CONTENT_MAX_BYTES, DEFAULT_TAG_ELEMENT_MAX_BYTES, DEFAULT_TAG_MAX_COUNT},
};
use radroots_event_codec::{
    calendar::{
        decode::{
            admit_radroots_calendar_date_event, admit_radroots_calendar_time_event,
            nip52_calendar_data_from_event, nip52_calendar_event_rsvp_data_from_event,
            nip52_calendar_event_rsvp_parsed_from_event, nip52_calendar_parsed_from_event,
            nip52_date_data_from_event, nip52_date_parsed_from_event,
            parse_nip52_calendar_date_event, parse_nip52_calendar_time_event,
        },
        encode::{
            calendar_date_event_build_tags, calendar_time_event_build_tags, calendar_to_wire_parts,
            calendar_to_wire_parts_with_kind, date_to_wire_parts, date_to_wire_parts_with_kind,
            rsvp_to_wire_parts, rsvp_to_wire_parts_with_kind, time_to_wire_parts,
            time_to_wire_parts_with_kind,
        },
    },
    error::{EventEncodeError, EventParseError},
};

const DATE_D_TAG: &str = "fernwood-csa-pickup";
const TIME_D_TAG: &str = "wash-pack-shift";
const EVENT_ID: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const EVENT_AUTHOR: &str = "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df";
const SECOND_AUTHOR: &str = "e0266e3cfb0d2886f91c73f5f868f3b98273713e5fcd97c081663f5518a4b3af";
const EVENT_SIG: &str = concat!(
    "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
    "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"
);

fn date(value: &str) -> RadrootsCalendarDate {
    RadrootsCalendarDate::parse(value).unwrap()
}

fn authored_image() -> RadrootsAuthoredImage {
    let bytes = b"canonical-calendar-image";
    let hash = Sha256::digest(bytes);
    let media_type = MediaType::parse("image/webp").unwrap();
    let descriptor = BlobDescriptor::new(
        BlobUrl::parse(&format!("https://media.example.test/{hash}.webp")).unwrap(),
        hash,
        bytes.len() as u64,
        media_type.clone(),
        1_784_347_200,
    )
    .unwrap()
    .approve_reference()
    .unwrap()
    .verify_bytes(bytes, &media_type)
    .unwrap();
    RadrootsAuthoredImage::try_from(descriptor).unwrap()
}

fn sample_date_event() -> RadrootsAuthoredCalendarDateEvent {
    RadrootsAuthoredCalendarDateEvent::new(DATE_D_TAG, "CSA pickup", date("2026-06-20"))
        .unwrap()
        .with_end(date("2026-06-21"))
        .unwrap()
        .with_description("Bring clean bins to the farm stand.")
        .unwrap()
        .with_locations(vec!["Farm stand".to_string()])
        .unwrap()
        .with_geohash("c23nb62w20st")
        .unwrap()
        .with_summary("Weekly pickup")
        .unwrap()
        .with_image(authored_image())
        .unwrap()
        .with_participants(vec![RadrootsCalendarParticipant {
            pubkey: EVENT_AUTHOR.to_string(),
            relay: Some("wss://relay.example.test".to_string()),
            role: Some("host".to_string()),
        }])
        .unwrap()
        .with_categories(vec!["csa".to_string(), "vegetables".to_string()])
        .unwrap()
}

fn sample_time_event() -> RadrootsAuthoredCalendarTimeEvent {
    RadrootsAuthoredCalendarTimeEvent::new(TIME_D_TAG, "Wash pack shift", 86_399)
        .unwrap()
        .with_end(172_801)
        .unwrap()
        .with_description("Prepare CSA bins before pickup.")
        .unwrap()
        .with_start_tzid("America/Vancouver")
        .unwrap()
        .with_locations(vec!["Pack shed".to_string()])
        .unwrap()
        .with_geohash("c23nb62w20st")
        .unwrap()
        .with_summary("Prepare CSA bins")
        .unwrap()
        .with_participants(vec![RadrootsCalendarParticipant {
            pubkey: EVENT_AUTHOR.to_string(),
            relay: None,
            role: Some("participant".to_string()),
        }])
        .unwrap()
        .with_categories(vec!["farm-work".to_string()])
        .unwrap()
}

fn baseline_common_tags(image: &str) -> Vec<Vec<String>> {
    vec![
        vec![TAG_D.to_string(), DATE_D_TAG.to_string()],
        vec![TAG_TITLE.to_string(), " CSA pickup ".to_string()],
        vec![TAG_LOCATION.to_string(), "Farm stand".to_string()],
        vec![TAG_LOCATION.to_string(), "Moss Street Market".to_string()],
        vec![TAG_G.to_string(), "C23NB62W20ST".to_string()],
        vec![TAG_SUMMARY.to_string(), " Weekly pickup ".to_string()],
        vec![TAG_IMAGE.to_string(), image.to_string()],
        vec![TAG_P.to_string(), EVENT_AUTHOR.to_string()],
        vec![
            TAG_P.to_string(),
            SECOND_AUTHOR.to_string(),
            "wss://relay.example.test/events".to_string(),
        ],
        vec![
            TAG_P.to_string(),
            EVENT_AUTHOR.to_string(),
            String::new(),
            " host ".to_string(),
        ],
        vec![TAG_T.to_string(), "vegetables".to_string()],
        vec![TAG_T.to_string(), "CSA".to_string()],
        vec![
            TAG_R.to_string(),
            "ipfs://bafybeigdyrzt/reference".to_string(),
        ],
        vec![
            TAG_A.to_string(),
            format!("31924:{EVENT_AUTHOR}:farm-calendar"),
            "wss://relay.example.test".to_string(),
        ],
        vec!["name".to_string(), "Deprecated display name".to_string()],
    ]
}

fn has_tag(tags: &[Vec<String>], key: &str, value: &str) -> bool {
    tags.iter().any(|tag| {
        tag.first().map(String::as_str) == Some(key)
            && tag.get(1).map(String::as_str) == Some(value)
    })
}

fn tag_values<'a>(tags: &'a [Vec<String>], key: &str) -> Vec<&'a str> {
    tags.iter()
        .filter(|tag| tag.first().map(String::as_str) == Some(key))
        .map(|tag| tag[1].as_str())
        .collect()
}

fn replace_tag_value(tags: &mut [Vec<String>], key: &str, value: &str) {
    let tag = tags
        .iter_mut()
        .find(|tag| tag.first().map(String::as_str) == Some(key))
        .expect("tag");
    tag[1] = value.to_string();
}

#[test]
fn authored_date_event_emits_canonical_fields_and_no_uppercase_day_tag() {
    let event = sample_date_event();
    let parts = date_to_wire_parts(&event).unwrap();

    assert_eq!(parts.kind, KIND_CALENDAR_DATE_EVENT);
    assert_eq!(parts.content, "Bring clean bins to the farm stand.");
    assert!(has_tag(&parts.tags, TAG_D, DATE_D_TAG));
    assert!(has_tag(&parts.tags, TAG_TITLE, "CSA pickup"));
    assert!(has_tag(&parts.tags, TAG_START, "2026-06-20"));
    assert!(has_tag(&parts.tags, TAG_END, "2026-06-21"));
    assert!(has_tag(&parts.tags, TAG_LOCATION, "Farm stand"));
    assert!(has_tag(&parts.tags, TAG_G, "c23nb62w20st"));
    assert!(has_tag(&parts.tags, TAG_T, "csa"));
    assert!(has_tag(&parts.tags, TAG_T, "vegetables"));
    assert!(!parts.tags.iter().any(|tag| tag[0] == TAG_D_DAY));
    assert_eq!(calendar_date_event_build_tags(&event).unwrap(), parts.tags);

    let parsed = parse_nip52_calendar_date_event(parts.kind, &parts.tags, &parts.content).unwrap();
    let admitted = admit_radroots_calendar_date_event(parsed).unwrap();
    assert_eq!(admitted.d_tag().as_str(), DATE_D_TAG);
    assert!(admitted.blossom_image().is_some());
}

#[test]
fn authored_time_event_derives_exact_ordered_days_and_iana_fallback() {
    let event = sample_time_event();
    assert_eq!(
        event
            .effective_end_tzid()
            .map(RadrootsIanaTimeZoneId::as_str),
        Some("America/Vancouver")
    );
    assert_eq!(event.end_tzid(), None);

    let parts = time_to_wire_parts(&event).unwrap();
    assert_eq!(parts.kind, KIND_CALENDAR_TIME_EVENT);
    assert_eq!(tag_values(&parts.tags, TAG_D_DAY), ["0", "1", "2"]);
    assert!(has_tag(&parts.tags, TAG_START_TZID, "America/Vancouver"));
    assert!(!parts.tags.iter().any(|tag| tag[0] == TAG_END_TZID));
    assert_eq!(calendar_time_event_build_tags(&event).unwrap(), parts.tags);

    let parsed = parse_nip52_calendar_time_event(parts.kind, &parts.tags, &parts.content).unwrap();
    let admitted = admit_radroots_calendar_time_event(parsed).unwrap();
    assert_eq!(admitted.covered_utc_days(), [0, 1, 2]);
}

#[test]
fn baseline_date_parser_preserves_nip52_extensions_and_common_metadata() {
    let image = "https://ordinary.example.test/images/market.jpg?size=large";
    let mut tags = baseline_common_tags(image);
    tags.push(vec![TAG_START.to_string(), "2026-06-20".to_string()]);
    tags.push(vec![TAG_END.to_string(), "2026-06-22".to_string()]);
    tags.push(vec![
        TAG_D_DAY.to_string(),
        "extension".to_string(),
        "preserved".to_string(),
    ]);

    let parsed =
        parse_nip52_calendar_date_event(KIND_CALENDAR_DATE_EVENT, &tags, "Bring reusable bags.")
            .unwrap();
    let common = parsed.common();
    assert_eq!(common.d_tag(), DATE_D_TAG);
    assert_eq!(common.title(), " CSA pickup ");
    assert_eq!(common.description(), Some("Bring reusable bags."));
    assert_eq!(common.locations(), ["Farm stand", "Moss Street Market"]);
    assert_eq!(common.geohash(), Some("C23NB62W20ST"));
    assert_eq!(common.summary(), Some(" Weekly pickup "));
    assert_eq!(common.image().unwrap().as_str(), image);
    assert_eq!(common.participants().len(), 3);
    assert_eq!(common.participants()[2].relay, None);
    assert_eq!(common.participants()[2].role.as_deref(), Some(" host "));
    assert_eq!(common.categories(), ["vegetables", "CSA"]);
    assert_eq!(
        common.references()[0].as_str(),
        "ipfs://bafybeigdyrzt/reference"
    );
    assert_eq!(
        common.calendar_requests()[0].calendar().as_str(),
        format!("31924:{EVENT_AUTHOR}:farm-calendar")
    );
    assert_eq!(common.legacy_name(), Some("Deprecated display name"));
    assert_eq!(parsed.start().as_str(), "2026-06-20");
    assert_eq!(parsed.end().unwrap().as_str(), "2026-06-22");
    assert_eq!(
        parsed.extension_day_tags(),
        [vec![
            TAG_D_DAY.to_string(),
            "extension".to_string(),
            "preserved".to_string()
        ]]
    );
}

#[test]
fn baseline_time_parser_accepts_nip52_should_level_variance() {
    let mut tags = vec![
        vec![TAG_D.to_string(), TIME_D_TAG.to_string()],
        vec![TAG_TITLE.to_string(), "Harvest window".to_string()],
        vec![TAG_START.to_string(), "086399".to_string()],
        vec![TAG_END.to_string(), "172801".to_string()],
        vec![TAG_D_DAY.to_string(), "000".to_string()],
        vec![TAG_D_DAY.to_string(), "2".to_string()],
        vec![TAG_START_TZID.to_string(), "America/Vancouver".to_string()],
    ];
    tags.extend([
        vec![TAG_T.to_string(), "harvest".to_string()],
        vec![
            TAG_R.to_string(),
            "https://example.test/harvest".to_string(),
        ],
    ]);

    let parsed = parse_nip52_calendar_time_event(KIND_CALENDAR_TIME_EVENT, &tags, "").unwrap();
    assert_eq!(parsed.start_wire(), "086399");
    assert_eq!(parsed.start(), 86_399);
    assert_eq!(parsed.end(), Some(172_801));
    assert_eq!(
        parsed
            .observed_day_indices()
            .iter()
            .map(|day| (day.wire_value(), day.index(), day.is_canonical()))
            .collect::<Vec<_>>(),
        vec![("000", 0, false), ("2", 2, true)]
    );
    assert_eq!(
        parsed
            .effective_end_tzid()
            .map(RadrootsIanaTimeZoneId::as_str),
        Some("America/Vancouver")
    );
}

#[test]
fn strict_admission_rejects_date_extensions_noncanonical_metadata_and_non_blossom_images() {
    let mut extension_tags = vec![
        vec![TAG_D.to_string(), DATE_D_TAG.to_string()],
        vec![TAG_TITLE.to_string(), "CSA pickup".to_string()],
        vec![TAG_START.to_string(), "2026-06-20".to_string()],
        vec![TAG_D_DAY.to_string(), "20620".to_string()],
    ];
    let parsed =
        parse_nip52_calendar_date_event(KIND_CALENDAR_DATE_EVENT, &extension_tags, "").unwrap();
    assert_eq!(
        admit_radroots_calendar_date_event(parsed),
        Err(RadrootsCalendarAdmissionError::ForbiddenDateDayIndex)
    );

    extension_tags.retain(|tag| tag[0] != TAG_D_DAY);
    replace_tag_value(&mut extension_tags, TAG_TITLE, " CSA pickup ");
    let parsed =
        parse_nip52_calendar_date_event(KIND_CALENDAR_DATE_EVENT, &extension_tags, "").unwrap();
    assert_eq!(
        admit_radroots_calendar_date_event(parsed),
        Err(RadrootsCalendarAdmissionError::NonCanonicalField(
            "metadata"
        ))
    );

    replace_tag_value(&mut extension_tags, TAG_TITLE, "CSA pickup");
    extension_tags.push(vec![
        TAG_IMAGE.to_string(),
        "https://ordinary.example.test/market.jpg".to_string(),
    ]);
    let parsed =
        parse_nip52_calendar_date_event(KIND_CALENDAR_DATE_EVENT, &extension_tags, "").unwrap();
    assert_eq!(
        admit_radroots_calendar_date_event(parsed),
        Err(RadrootsCalendarAdmissionError::NonBlossomImage)
    );
}

#[test]
fn strict_time_admission_requires_canonical_exact_coverage() {
    let base = vec![
        vec![TAG_D.to_string(), TIME_D_TAG.to_string()],
        vec![TAG_TITLE.to_string(), "Harvest window".to_string()],
        vec![TAG_START.to_string(), "86399".to_string()],
        vec![TAG_END.to_string(), "172801".to_string()],
    ];

    for days in [
        vec!["0", "2"],
        vec!["0", "2", "1"],
        vec!["0", "1", "1", "2"],
        vec!["00", "1", "2"],
    ] {
        let mut tags = base.clone();
        tags.extend(
            days.into_iter()
                .map(|day| vec![TAG_D_DAY.to_string(), day.to_string()]),
        );
        let parsed = parse_nip52_calendar_time_event(KIND_CALENDAR_TIME_EVENT, &tags, "").unwrap();
        assert_eq!(
            admit_radroots_calendar_time_event(parsed),
            Err(RadrootsCalendarAdmissionError::IncompleteDayCoverage)
        );
    }

    let mut noncanonical_timestamp = base.clone();
    replace_tag_value(&mut noncanonical_timestamp, TAG_START, "086399");
    noncanonical_timestamp
        .extend([0, 1, 2].map(|day| vec![TAG_D_DAY.to_string(), day.to_string()]));
    let parsed =
        parse_nip52_calendar_time_event(KIND_CALENDAR_TIME_EVENT, &noncanonical_timestamp, "")
            .unwrap();
    assert_eq!(
        admit_radroots_calendar_time_event(parsed),
        Err(RadrootsCalendarAdmissionError::NonCanonicalField(
            "timestamp"
        ))
    );
}

#[test]
fn baseline_can_parse_overlong_ranges_that_strict_admission_rejects() {
    let end = (RADROOTS_CALENDAR_MAX_COVERED_UTC_DAYS + 1) * RADROOTS_CALENDAR_SECONDS_PER_DAY;
    let tags = vec![
        vec![TAG_D.to_string(), TIME_D_TAG.to_string()],
        vec![TAG_TITLE.to_string(), "Long range".to_string()],
        vec![TAG_START.to_string(), "0".to_string()],
        vec![TAG_END.to_string(), end.to_string()],
        vec![TAG_D_DAY.to_string(), "0".to_string()],
    ];
    let parsed = parse_nip52_calendar_time_event(KIND_CALENDAR_TIME_EVENT, &tags, "").unwrap();
    assert_eq!(parsed.end(), Some(end));
    assert_eq!(
        admit_radroots_calendar_time_event(parsed),
        Err(RadrootsCalendarAdmissionError::CoveredDayLimitExceeded {
            max: RADROOTS_CALENDAR_MAX_COVERED_UTC_DAYS,
            actual: RADROOTS_CALENDAR_MAX_COVERED_UTC_DAYS + 1,
        })
    );
}

#[test]
fn authored_construction_rejects_invalid_fields_before_encoding() {
    assert_eq!(
        RadrootsAuthoredCalendarDateEvent::new(DATE_D_TAG, " CSA pickup ", date("2026-06-20")),
        Err(RadrootsCalendarEventError::InvalidTitle)
    );
    assert_eq!(
        RadrootsAuthoredCalendarTimeEvent::new(TIME_D_TAG, "Shift", 10)
            .unwrap()
            .with_start_tzid("america/vancouver"),
        Err(RadrootsCalendarEventError::InvalidTimeZone)
    );
    assert!(matches!(
        RadrootsAuthoredCalendarDateEvent::new(DATE_D_TAG, "Pickup", date("2026-06-20"))
            .unwrap()
            .with_summary("x".repeat(DEFAULT_TAG_ELEMENT_MAX_BYTES + 1)),
        Err(RadrootsCalendarEventError::TagElementTooLarge {
            field: "summary",
            ..
        })
    ));
    assert!(matches!(
        RadrootsAuthoredCalendarDateEvent::new(DATE_D_TAG, "Pickup", date("2026-06-20"))
            .unwrap()
            .with_description("x".repeat(DEFAULT_CONTENT_MAX_BYTES + 1)),
        Err(RadrootsCalendarEventError::ContentTooLarge { .. })
    ));

    let invalid_participant = RadrootsCalendarParticipant {
        pubkey: EVENT_AUTHOR.to_ascii_uppercase(),
        relay: None,
        role: None,
    };
    assert!(matches!(
        RadrootsAuthoredCalendarDateEvent::new(DATE_D_TAG, "Pickup", date("2026-06-20"))
            .unwrap()
            .with_participants(vec![invalid_participant]),
        Err(RadrootsCalendarEventError::InvalidParticipant { index: 0 })
    ));
}

#[test]
fn baseline_parser_rejects_malformed_fields_and_bounds_before_projection() {
    let minimum = vec![
        vec![TAG_D.to_string(), DATE_D_TAG.to_string()],
        vec![TAG_TITLE.to_string(), "CSA pickup".to_string()],
        vec![TAG_START.to_string(), "2026-06-20".to_string()],
    ];
    assert!(matches!(
        parse_nip52_calendar_date_event(KIND_POST, &minimum, ""),
        Err(EventParseError::InvalidKind { .. })
    ));

    let mut duplicate = minimum.clone();
    duplicate.push(vec![TAG_TITLE.to_string(), "Duplicate".to_string()]);
    assert!(matches!(
        parse_nip52_calendar_date_event(KIND_CALENDAR_DATE_EVENT, &duplicate, ""),
        Err(EventParseError::DuplicateTag(TAG_TITLE))
    ));

    let mut blank_d_tag = minimum.clone();
    replace_tag_value(&mut blank_d_tag, TAG_D, " \t");
    assert!(matches!(
        parse_nip52_calendar_date_event(KIND_CALENDAR_DATE_EVENT, &blank_d_tag, ""),
        Err(EventParseError::InvalidTag(TAG_D))
    ));

    let mut blank_title = minimum.clone();
    replace_tag_value(&mut blank_title, TAG_TITLE, " \t");
    assert!(matches!(
        parse_nip52_calendar_date_event(KIND_CALENDAR_DATE_EVENT, &blank_title, ""),
        Err(EventParseError::InvalidTag(TAG_TITLE))
    ));

    let mut impossible = minimum.clone();
    replace_tag_value(&mut impossible, TAG_START, "2026-02-30");
    assert!(matches!(
        parse_nip52_calendar_date_event(KIND_CALENDAR_DATE_EVENT, &impossible, ""),
        Err(EventParseError::InvalidTag(TAG_START))
    ));

    let mut relative_image = minimum.clone();
    relative_image.push(vec![TAG_IMAGE.to_string(), "/market.jpg".to_string()]);
    assert!(matches!(
        parse_nip52_calendar_date_event(KIND_CALENDAR_DATE_EVENT, &relative_image, ""),
        Err(EventParseError::InvalidTag(TAG_IMAGE))
    ));

    let mut oversized_element = minimum.clone();
    replace_tag_value(
        &mut oversized_element,
        TAG_TITLE,
        &"x".repeat(DEFAULT_TAG_ELEMENT_MAX_BYTES + 1),
    );
    assert!(matches!(
        parse_nip52_calendar_date_event(KIND_CALENDAR_DATE_EVENT, &oversized_element, ""),
        Err(EventParseError::InvalidEnvelope)
    ));

    let mut invalid_participant = minimum.clone();
    invalid_participant.push(vec![
        TAG_P.to_string(),
        EVENT_AUTHOR.to_string(),
        String::new(),
    ]);
    assert!(matches!(
        parse_nip52_calendar_date_event(KIND_CALENDAR_DATE_EVENT, &invalid_participant, ""),
        Err(EventParseError::InvalidTag(TAG_P))
    ));

    let invalid_tz = vec![
        vec![TAG_D.to_string(), TIME_D_TAG.to_string()],
        vec![TAG_TITLE.to_string(), "Shift".to_string()],
        vec![TAG_START.to_string(), "0".to_string()],
        vec![TAG_D_DAY.to_string(), "0".to_string()],
        vec![TAG_START_TZID.to_string(), "america/vancouver".to_string()],
    ];
    assert!(matches!(
        parse_nip52_calendar_time_event(KIND_CALENDAR_TIME_EVENT, &invalid_tz, ""),
        Err(EventParseError::InvalidTag(TAG_START_TZID))
    ));

    let missing_day = vec![
        vec![TAG_D.to_string(), TIME_D_TAG.to_string()],
        vec![TAG_TITLE.to_string(), "Shift".to_string()],
        vec![TAG_START.to_string(), "0".to_string()],
    ];
    assert!(matches!(
        parse_nip52_calendar_time_event(KIND_CALENDAR_TIME_EVENT, &missing_day, ""),
        Err(EventParseError::MissingTag(TAG_D_DAY))
    ));

    let mut out_of_range_day = missing_day;
    out_of_range_day.push(vec![TAG_D_DAY.to_string(), "1".to_string()]);
    assert!(matches!(
        parse_nip52_calendar_time_event(KIND_CALENDAR_TIME_EVENT, &out_of_range_day, ""),
        Err(EventParseError::InvalidTag(TAG_D_DAY))
    ));

    let oversized_content = "x".repeat(DEFAULT_CONTENT_MAX_BYTES + 1);
    assert!(matches!(
        parse_nip52_calendar_date_event(KIND_CALENDAR_DATE_EVENT, &minimum, &oversized_content),
        Err(EventParseError::InvalidEnvelope)
    ));

    let mut too_many_tags = minimum;
    too_many_tags.extend(
        (too_many_tags.len()..=DEFAULT_TAG_MAX_COUNT)
            .map(|_| vec!["x".to_string(), "y".to_string()]),
    );
    assert!(RadrootsEventTags::new(too_many_tags.clone()).is_err());
    assert!(matches!(
        parse_nip52_calendar_date_event(KIND_CALENDAR_DATE_EVENT, &too_many_tags, ""),
        Err(EventParseError::InvalidEnvelope)
    ));
}

#[test]
fn kind_specific_encoders_and_parsed_wrappers_preserve_envelopes() {
    assert!(matches!(
        date_to_wire_parts_with_kind(&sample_date_event(), KIND_POST),
        Err(EventEncodeError::InvalidKind(KIND_POST))
    ));
    assert!(matches!(
        time_to_wire_parts_with_kind(&sample_time_event(), KIND_POST),
        Err(EventEncodeError::InvalidKind(KIND_POST))
    ));

    let parts = date_to_wire_parts(&sample_date_event()).unwrap();
    let data = nip52_date_data_from_event(
        EVENT_ID.to_string(),
        EVENT_AUTHOR.to_string(),
        7,
        parts.kind,
        parts.content.clone(),
        parts.tags.clone(),
    )
    .unwrap();
    assert_eq!(data.data.common().title(), "CSA pickup");

    let parsed = nip52_date_parsed_from_event(
        EVENT_ID.to_string(),
        EVENT_AUTHOR.to_string(),
        7,
        parts.kind,
        parts.content,
        parts.tags,
        EVENT_SIG.to_string(),
    )
    .unwrap();
    assert_eq!(parsed.event.sig_str(), EVENT_SIG);
    assert_eq!(parsed.data.data.common().d_tag(), DATE_D_TAG);

    let event_reference = RadrootsCalendarEventReference::parse(
        format!("{KIND_CALENDAR_TIME_EVENT}:{EVENT_AUTHOR}:{TIME_D_TAG}"),
        Some("wss://relay.example.test/events"),
    )
    .unwrap();
    let calendar = RadrootsAuthoredCalendar::new(
        RadrootsCalendarUid::parse("AAAAAAAAAAAAAAAAAAAAAA").unwrap(),
        "Farm calendar",
        "Shared farm schedule.",
        vec![event_reference.clone()],
    )
    .unwrap();
    assert!(matches!(
        calendar_to_wire_parts_with_kind(&calendar, KIND_POST),
        Err(EventEncodeError::InvalidKind(KIND_POST))
    ));
    let parts = calendar_to_wire_parts(&calendar).unwrap();
    assert_eq!(parts.kind, KIND_CALENDAR);
    let data = nip52_calendar_data_from_event(
        EVENT_ID.to_string(),
        EVENT_AUTHOR.to_string(),
        8,
        parts.kind,
        parts.content.clone(),
        parts.tags.clone(),
    )
    .unwrap();
    assert_eq!(
        data.data.event_references(),
        core::slice::from_ref(&event_reference)
    );
    let parsed = nip52_calendar_parsed_from_event(
        EVENT_ID.to_string(),
        EVENT_AUTHOR.to_string(),
        8,
        parts.kind,
        parts.content,
        parts.tags,
        EVENT_SIG.to_string(),
    )
    .unwrap();
    assert_eq!(parsed.event.sig_str(), EVENT_SIG);
    assert_eq!(parsed.data.data.title(), "Farm calendar");

    let rsvp = RadrootsAuthoredCalendarEventRsvp::new(
        RadrootsCalendarUid::parse("DDDDDDDDDDDDDDDDDDDDDw").unwrap(),
        event_reference,
        RadrootsCalendarEventRsvpStatus::Accepted,
    )
    .unwrap();
    assert!(matches!(
        rsvp_to_wire_parts_with_kind(&rsvp, KIND_POST),
        Err(EventEncodeError::InvalidKind(KIND_POST))
    ));
    let parts = rsvp_to_wire_parts(&rsvp).unwrap();
    assert_eq!(parts.kind, KIND_CALENDAR_EVENT_RSVP);
    let data = nip52_calendar_event_rsvp_data_from_event(
        EVENT_ID.to_string(),
        EVENT_AUTHOR.to_string(),
        9,
        parts.kind,
        parts.content.clone(),
        parts.tags.clone(),
    )
    .unwrap();
    assert_eq!(
        data.data.status(),
        &RadrootsCalendarEventRsvpStatus::Accepted
    );
    let parsed = nip52_calendar_event_rsvp_parsed_from_event(
        EVENT_ID.to_string(),
        EVENT_AUTHOR.to_string(),
        9,
        parts.kind,
        parts.content,
        parts.tags,
        EVENT_SIG.to_string(),
    )
    .unwrap();
    assert_eq!(parsed.event.sig_str(), EVENT_SIG);
    assert_eq!(parsed.data.data.d_tag(), "DDDDDDDDDDDDDDDDDDDDDw");
}

#[test]
fn exclusive_end_day_math_is_bounded() {
    assert_eq!(
        covered_utc_days(86_399, Some(86_400))
            .unwrap()
            .collect::<Vec<_>>(),
        vec![0]
    );
    assert_eq!(
        covered_utc_days(86_399, Some(86_401))
            .unwrap()
            .collect::<Vec<_>>(),
        vec![0, 1]
    );
    assert_eq!(
        covered_utc_days(10, Some(10)),
        Err(RadrootsCalendarEventError::InvalidRange)
    );
}
