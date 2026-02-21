use std::error::Error as _;

use radroots_events_codec::error::{EventEncodeError, EventParseError};
use radroots_events_codec::job::encode::{
    assert_no_inputs_when_encrypted, push_provider_tag, push_relay_tag, push_status_tag,
    JobEncodeError,
};
use radroots_events_codec::job::error::JobParseError;
use radroots_events_codec::profile::error::ProfileEncodeError;
#[cfg(feature = "serde_json")]
use serde::ser::{Error as _, Serializer};
#[cfg(feature = "serde_json")]
use serde::Serialize;

#[test]
fn parse_error_display_and_source_cover_variants() {
    let missing = EventParseError::MissingTag("d");
    assert_eq!(missing.to_string(), "missing tag: d");
    assert!(missing.source().is_none());

    let invalid = EventParseError::InvalidTag("a");
    assert_eq!(invalid.to_string(), "invalid tag structure for 'a'");
    assert!(invalid.source().is_none());

    let invalid_kind = EventParseError::InvalidKind {
        expected: "30340",
        got: 1,
    };
    assert_eq!(invalid_kind.to_string(), "invalid kind 1 (expected 30340)");
    assert!(invalid_kind.source().is_none());

    let parse_int = "x".parse::<u32>().expect_err("parse int error");
    let invalid_number = EventParseError::InvalidNumber("count", parse_int);
    assert!(invalid_number
        .to_string()
        .contains("invalid number in 'count'"));
    assert!(invalid_number.source().is_some());

    let invalid_json = EventParseError::InvalidJson("content");
    assert_eq!(invalid_json.to_string(), "invalid JSON in 'content'");
    assert!(invalid_json.source().is_none());
}

#[test]
fn encode_error_display_covers_variants() {
    let invalid_kind = EventEncodeError::InvalidKind(30402);
    assert_eq!(invalid_kind.to_string(), "invalid event kind: 30402");

    let empty_required = EventEncodeError::EmptyRequiredField("content");
    assert_eq!(empty_required.to_string(), "empty required field: content");

    let invalid_field = EventEncodeError::InvalidField("d");
    assert_eq!(invalid_field.to_string(), "invalid field: d");

    let json = EventEncodeError::Json;
    assert_eq!(json.to_string(), "failed to serialize JSON");
}

#[test]
fn job_encode_helpers_cover_status_provider_relay_and_inputs() {
    let mut tags: Vec<Vec<String>> = Vec::new();
    push_status_tag(&mut tags, "ok", None);
    push_status_tag(&mut tags, "warning", Some("detail"));
    push_provider_tag(
        &mut tags,
        "58e318557257f2ab58a415d21bb57082b4824cf667a1d64e72bcbc5acc018c62",
    );
    push_relay_tag(&mut tags, "wss://relay.example.com");

    assert_eq!(tags[0], vec!["status".to_string(), "ok".to_string()]);
    assert_eq!(
        tags[1],
        vec![
            "status".to_string(),
            "warning".to_string(),
            "detail".to_string(),
        ]
    );
    assert_eq!(
        tags[2],
        vec![
            "p".to_string(),
            "58e318557257f2ab58a415d21bb57082b4824cf667a1d64e72bcbc5acc018c62".to_string(),
        ]
    );
    assert_eq!(
        tags[3],
        vec!["relays".to_string(), "wss://relay.example.com".to_string()]
    );

    assert!(assert_no_inputs_when_encrypted(&tags));
    let tags_with_input = vec![vec!["i".to_string(), "amount".to_string()]];
    assert!(!assert_no_inputs_when_encrypted(&tags_with_input));
}

#[cfg(feature = "serde_json")]
#[test]
fn job_json_content_covers_success_and_error_paths() {
    #[derive(Clone)]
    struct BrokenSerialize;

    impl Serialize for BrokenSerialize {
        fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            Err(S::Error::custom("forced serialization error"))
        }
    }

    let ok = radroots_events_codec::job::encode::json_content(&vec!["ok".to_string()])
        .expect("json content");
    assert_eq!(ok, "[\"ok\"]");

    let err = radroots_events_codec::job::encode::json_content(&BrokenSerialize)
        .expect_err("json content error");
    assert!(matches!(
        err,
        JobEncodeError::EmptyRequiredField("content-json")
    ));
}

#[test]
fn job_encode_error_display_covers_variants() {
    assert_eq!(
        JobEncodeError::MissingProvidersForEncrypted.to_string(),
        "encrypted=true requires at least one provider ('p') tag"
    );
    assert_eq!(
        JobEncodeError::InvalidKind(7000).to_string(),
        "invalid job event kind: 7000"
    );
    assert_eq!(
        JobEncodeError::EmptyRequiredField("content").to_string(),
        "empty required field: content"
    );
}

#[test]
fn job_parse_error_display_and_source_covers_variants() {
    let missing = JobParseError::MissingTag("e");
    assert_eq!(missing.to_string(), "missing tag: e");
    assert!(missing.source().is_none());

    let invalid = JobParseError::InvalidTag("status");
    assert_eq!(invalid.to_string(), "invalid tag structure for 'status'");
    assert!(invalid.source().is_none());

    let invalid_number = JobParseError::InvalidNumber("amount", "x".parse::<u32>().unwrap_err());
    assert!(invalid_number
        .to_string()
        .contains("invalid number in 'amount'"));
    assert!(invalid_number.source().is_some());

    let non_whole = JobParseError::NonWholeSats("amount");
    assert!(non_whole.to_string().contains("whole number of sats"));
    assert!(non_whole.source().is_none());

    let overflow = JobParseError::AmountOverflow("amount");
    assert!(overflow.to_string().contains("does not fit u32 sat"));
    assert!(overflow.source().is_none());

    let missing_chain = JobParseError::MissingChainTag("e");
    assert_eq!(missing_chain.to_string(), "missing required chain tag: e");
    assert!(missing_chain.source().is_none());
}

#[test]
fn profile_encode_error_display_covers_variants() {
    let invalid = ProfileEncodeError::InvalidUrl("website", "ftp://example.com".to_string());
    assert_eq!(
        invalid.to_string(),
        "invalid URL for website: ftp://example.com"
    );

    let json = ProfileEncodeError::Json;
    assert_eq!(json.to_string(), "failed to serialize metadata JSON");
}
