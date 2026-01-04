#![cfg(feature = "serde_json")]

use radroots_events::{
    kinds::KIND_POST,
    profile::{
        RadrootsProfileType,
        RADROOTS_PROFILE_TYPE_TAG_ANY,
        RADROOTS_PROFILE_TYPE_TAG_FARM,
        RADROOTS_PROFILE_TYPE_TAG_KEY,
        RADROOTS_PROFILE_TYPE_TAG_RADROOTSD,
    },
};
use radroots_events_codec::error::EventParseError;
use radroots_events_codec::profile::decode::profile_from_content;

#[test]
fn profile_from_content_parses_bot_boolean() {
    let content = r#"{"name":"alice","bot":true}"#;
    let profile = profile_from_content(content).unwrap();

    assert_eq!(profile.name, "alice");
    assert_eq!(profile.bot.as_deref(), Some("true"));
}

#[test]
fn profile_from_content_parses_bot_string() {
    let content = r#"{"name":"alice","bot":"false"}"#;
    let profile = profile_from_content(content).unwrap();

    assert_eq!(profile.name, "alice");
    assert_eq!(profile.bot.as_deref(), Some("false"));
}

#[test]
fn profile_from_content_rejects_missing_name() {
    let content = r#"{"display_name":"alice"}"#;
    let err = profile_from_content(content).unwrap_err();
    assert!(matches!(err, EventParseError::InvalidJson("name")));
}

#[test]
fn profile_from_content_rejects_invalid_json() {
    let err = profile_from_content("{").unwrap_err();
    assert!(matches!(err, EventParseError::InvalidJson("content")));
}

#[test]
fn profile_metadata_rejects_wrong_kind() {
    let err = radroots_events_codec::profile::decode::metadata_from_event(
        "id".to_string(),
        "author".to_string(),
        1,
        1,
        "{\"name\":\"alice\"}".to_string(),
        Vec::new(),
    )
    .unwrap_err();

    assert!(matches!(
        err,
        EventParseError::InvalidKind {
            expected: "0",
            got: KIND_POST
        }
    ));
}

#[test]
fn profile_metadata_reads_profile_type_tag() {
    let metadata = radroots_events_codec::profile::decode::metadata_from_event(
        "id".to_string(),
        "author".to_string(),
        1,
        0,
        "{\"name\":\"alice\"}".to_string(),
        vec![vec![
            RADROOTS_PROFILE_TYPE_TAG_KEY.to_string(),
            RADROOTS_PROFILE_TYPE_TAG_FARM.to_string(),
        ]],
    )
    .expect("metadata");

    assert_eq!(metadata.profile_type, Some(RadrootsProfileType::Farm));
}

#[test]
fn profile_metadata_reads_profile_type_any_tag() {
    let metadata = radroots_events_codec::profile::decode::metadata_from_event(
        "id".to_string(),
        "author".to_string(),
        1,
        0,
        "{\"name\":\"alice\"}".to_string(),
        vec![vec![
            RADROOTS_PROFILE_TYPE_TAG_KEY.to_string(),
            RADROOTS_PROFILE_TYPE_TAG_ANY.to_string(),
        ]],
    )
    .expect("metadata");

    assert_eq!(metadata.profile_type, Some(RadrootsProfileType::Any));
}

#[test]
fn profile_metadata_reads_profile_type_radrootsd_tag() {
    let metadata = radroots_events_codec::profile::decode::metadata_from_event(
        "id".to_string(),
        "author".to_string(),
        1,
        0,
        "{\"name\":\"alice\"}".to_string(),
        vec![vec![
            RADROOTS_PROFILE_TYPE_TAG_KEY.to_string(),
            RADROOTS_PROFILE_TYPE_TAG_RADROOTSD.to_string(),
        ]],
    )
    .expect("metadata");

    assert_eq!(metadata.profile_type, Some(RadrootsProfileType::Radrootsd));
}
