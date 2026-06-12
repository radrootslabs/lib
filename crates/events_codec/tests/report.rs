#![cfg(feature = "serde_json")]

use radroots_events::{
    kinds::{KIND_POST, KIND_REPORT},
    report::RadrootsReport,
    social::{RadrootsReportFileTarget, RadrootsReportType, RadrootsSocialTarget},
    tags::{TAG_E, TAG_MAGNET, TAG_P, TAG_SERVER, TAG_SHA256},
};
use radroots_events_codec::{
    error::{EventEncodeError, EventParseError},
    report::{
        decode::{data_from_event, parsed_from_event, report_from_event},
        encode::{report_build_tags, to_wire_parts, to_wire_parts_with_kind},
    },
};

const EVENT_ID: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const REPORTED: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const FILE_HASH: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

fn profile_report() -> RadrootsReport {
    RadrootsReport {
        reported_pubkey: REPORTED.to_string(),
        report_type: RadrootsReportType::Spam,
        event: None,
        file: None,
        content: None,
    }
}

fn event_report() -> RadrootsReport {
    RadrootsReport {
        reported_pubkey: REPORTED.to_string(),
        report_type: RadrootsReportType::Illegal,
        event: Some(RadrootsSocialTarget::Event {
            id: EVENT_ID.to_string(),
            author: Some(REPORTED.to_string()),
            event_kind: Some(KIND_POST),
            relays: Some(vec!["wss://relay.example.test".to_string()]),
        }),
        file: None,
        content: Some("Contains prohibited listing text".to_string()),
    }
}

fn file_report() -> RadrootsReport {
    RadrootsReport {
        reported_pubkey: REPORTED.to_string(),
        report_type: RadrootsReportType::Malware,
        event: None,
        file: Some(RadrootsReportFileTarget {
            sha256: Some(FILE_HASH.to_string()),
            url: Some("https://media.example.test/blob".to_string()),
            magnet: Some("magnet:?xt=urn:btih:example".to_string()),
        }),
        content: None,
    }
}

fn has_tag(tags: &[Vec<String>], key: &str, value: &str) -> bool {
    tags.iter().any(|tag| {
        tag.first().map(|entry| entry.as_str()) == Some(key)
            && tag.get(1).map(|entry| entry.as_str()) == Some(value)
    })
}

#[test]
fn report_to_wire_parts_roundtrips_pubkey_event_and_file_reports() {
    let profile = to_wire_parts(&profile_report()).unwrap();
    assert_eq!(profile.kind, KIND_REPORT);
    assert!(profile.content.is_empty());
    assert!(has_tag(&profile.tags, TAG_P, REPORTED));
    assert_eq!(
        profile.tags[0].get(2).map(|value| value.as_str()),
        Some("spam")
    );
    let decoded = report_from_event(profile.kind, &profile.tags, &profile.content).unwrap();
    assert_eq!(decoded.reported_pubkey, REPORTED);
    assert_eq!(decoded.report_type, RadrootsReportType::Spam);

    let event = to_wire_parts(&event_report()).unwrap();
    assert_eq!(event.content, "Contains prohibited listing text");
    assert!(has_tag(&event.tags, TAG_E, EVENT_ID));
    let decoded = report_from_event(event.kind, &event.tags, &event.content).unwrap();
    assert!(matches!(
        decoded.event,
        Some(RadrootsSocialTarget::Event { .. })
    ));
    assert_eq!(decoded.report_type, RadrootsReportType::Illegal);

    let file = to_wire_parts(&file_report()).unwrap();
    assert!(has_tag(&file.tags, TAG_SHA256, FILE_HASH));
    assert!(has_tag(
        &file.tags,
        TAG_SERVER,
        "https://media.example.test/blob"
    ));
    assert!(has_tag(
        &file.tags,
        TAG_MAGNET,
        "magnet:?xt=urn:btih:example"
    ));
    let decoded = report_from_event(file.kind, &file.tags, &file.content).unwrap();
    assert_eq!(
        decoded.file.and_then(|target| target.sha256).as_deref(),
        Some(FILE_HASH)
    );
}

#[test]
fn report_codec_rejects_missing_pubkey_unknown_type_bad_hash_and_wrong_kind() {
    let mut report = profile_report();
    report.reported_pubkey = " ".to_string();
    assert!(matches!(
        report_build_tags(&report),
        Err(EventEncodeError::EmptyRequiredField("reported_pubkey"))
    ));

    assert!(matches!(
        to_wire_parts_with_kind(&profile_report(), KIND_POST),
        Err(EventEncodeError::InvalidKind(KIND_POST))
    ));

    let mut report = file_report();
    report.file.as_mut().unwrap().sha256 = Some("bad".to_string());
    assert!(matches!(
        to_wire_parts(&report),
        Err(EventEncodeError::InvalidField("file.sha256"))
    ));

    let err = report_from_event(KIND_REPORT, &[], "").unwrap_err();
    assert!(matches!(err, EventParseError::MissingTag(TAG_P)));

    let tags = vec![vec![
        TAG_P.to_string(),
        REPORTED.to_string(),
        "unknown".to_string(),
    ]];
    let err = report_from_event(KIND_REPORT, &tags, "").unwrap_err();
    assert!(matches!(err, EventParseError::InvalidTag(TAG_P)));

    let tags = vec![
        vec![
            TAG_P.to_string(),
            REPORTED.to_string(),
            "malware".to_string(),
        ],
        vec![
            TAG_SHA256.to_string(),
            "bad".to_string(),
            "malware".to_string(),
        ],
    ];
    let err = report_from_event(KIND_REPORT, &tags, "").unwrap_err();
    assert!(matches!(err, EventParseError::InvalidTag(TAG_SHA256)));

    let err = report_from_event(KIND_POST, &tags, "").unwrap_err();
    assert!(matches!(
        err,
        EventParseError::InvalidKind {
            expected: "1984",
            got: KIND_POST
        }
    ));
}

#[test]
fn report_wrappers_preserve_event_metadata() {
    let parts = to_wire_parts(&event_report()).unwrap();
    let data = data_from_event(
        "report_id".to_string(),
        "author".to_string(),
        12,
        parts.kind,
        parts.content.clone(),
        parts.tags.clone(),
    )
    .unwrap();
    assert_eq!(data.kind, KIND_REPORT);
    assert_eq!(data.data.report_type, RadrootsReportType::Illegal);

    let parsed = parsed_from_event(
        "report_id".to_string(),
        "author".to_string(),
        12,
        parts.kind,
        parts.content,
        parts.tags,
        "sig".to_string(),
    )
    .unwrap();
    assert_eq!(parsed.event.sig, "sig");
    assert_eq!(parsed.data.data.reported_pubkey, REPORTED);
}
