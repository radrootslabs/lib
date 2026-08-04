#[path = "../src/test_fixtures.rs"]
mod test_fixtures;

use std::error::Error as _;

use radroots_event_codec::decode::EventParseError;
use radroots_event_codec::decode::job::JobParseError;
use radroots_event_codec::encode::EventEncodeError;
use radroots_event_codec::encode::job::{
    JobEncodeError, assert_no_inputs_when_encrypted, push_provider_tag, push_relay_tag,
    push_status_tag,
};
#[cfg(feature = "json")]
use serde::Serialize;
#[cfg(feature = "json")]
use serde::ser::{Error as _, Serializer};
use test_fixtures::{FIXTURE_ALICE_PUBLIC_KEY_HEX, RELAY_PRIMARY_WSS};

#[test]
fn parse_error_display_and_source_cover_variants() {
    let missing = EventParseError::MissingTag("d");
    assert_eq!(missing.to_string(), "missing tag: d");
    assert_eq!(missing.code(), "missing_tag");
    assert!(missing.source().is_none());

    let invalid = EventParseError::InvalidTag("a");
    assert_eq!(invalid.to_string(), "invalid tag structure for 'a'");
    assert_eq!(invalid.code(), "invalid_tag");
    assert!(invalid.source().is_none());

    let invalid_kind = EventParseError::InvalidKind {
        expected: "30340",
        got: 1,
    };
    assert_eq!(invalid_kind.to_string(), "invalid kind 1 (expected 30340)");
    assert_eq!(invalid_kind.code(), "invalid_kind");
    assert!(invalid_kind.source().is_none());

    let parse_int = "x".parse::<u32>().expect_err("parse int error");
    let invalid_number = EventParseError::InvalidNumber("count", parse_int);
    assert!(
        invalid_number
            .to_string()
            .contains("invalid number in 'count'")
    );
    assert_eq!(invalid_number.code(), "invalid_number");
    assert!(invalid_number.source().is_some());

    let invalid_json = EventParseError::InvalidJson("content");
    assert_eq!(invalid_json.to_string(), "invalid JSON in 'content'");
    assert_eq!(invalid_json.code(), "invalid_json");
    assert!(invalid_json.source().is_none());
}

#[test]
fn encode_error_display_covers_variants() {
    let invalid_kind = EventEncodeError::InvalidKind(30402);
    assert_eq!(invalid_kind.to_string(), "invalid event kind: 30402");
    assert_eq!(invalid_kind.code(), "invalid_kind");

    let empty_required = EventEncodeError::EmptyRequiredField("content");
    assert_eq!(empty_required.to_string(), "empty required field: content");
    assert_eq!(empty_required.code(), "empty_required_field");

    let invalid_field = EventEncodeError::InvalidField("d");
    assert_eq!(invalid_field.to_string(), "invalid field: d");
    assert_eq!(invalid_field.code(), "invalid_field");

    let json = EventEncodeError::Json;
    assert_eq!(json.to_string(), "failed to serialize JSON");
    assert_eq!(json.code(), "json");
}

#[test]
fn job_encode_helpers_cover_status_provider_relay_and_inputs() {
    let mut tags: Vec<Vec<String>> = Vec::new();
    push_status_tag(&mut tags, "ok", None);
    push_status_tag(&mut tags, "warning", Some("detail"));
    push_provider_tag(&mut tags, FIXTURE_ALICE_PUBLIC_KEY_HEX);
    push_relay_tag(&mut tags, RELAY_PRIMARY_WSS);

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
        vec!["p".to_string(), FIXTURE_ALICE_PUBLIC_KEY_HEX.to_string(),]
    );
    assert_eq!(
        tags[3],
        vec!["relays".to_string(), RELAY_PRIMARY_WSS.to_string()]
    );

    assert!(assert_no_inputs_when_encrypted(&tags));
    let tags_with_input = vec![vec!["i".to_string(), "amount".to_string()]];
    assert!(!assert_no_inputs_when_encrypted(&tags_with_input));
}

#[cfg(feature = "json")]
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

    let ok = radroots_event_codec::encode::job::json_content(&vec!["ok".to_string()])
        .expect("json content");
    assert_eq!(ok, "[\"ok\"]");

    let err = radroots_event_codec::encode::job::json_content(&BrokenSerialize)
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
    let kind = JobParseError::KindOutOfRange(u32::from(u16::MAX) + 1);
    assert!(kind.to_string().contains("Nostr event kind"));
    assert!(kind.source().is_none());

    let missing = JobParseError::MissingTag("e");
    assert_eq!(missing.to_string(), "missing tag: e");
    assert!(missing.source().is_none());

    let invalid = JobParseError::InvalidTag("status");
    assert_eq!(invalid.to_string(), "invalid tag structure for 'status'");
    assert!(invalid.source().is_none());

    let invalid_number = JobParseError::InvalidNumber("amount", "x".parse::<u32>().unwrap_err());
    assert!(
        invalid_number
            .to_string()
            .contains("invalid number in 'amount'")
    );
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
