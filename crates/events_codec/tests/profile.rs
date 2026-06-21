#![cfg(feature = "serde_json")]

use radroots_events::{
    kinds::{KIND_POST, KIND_PROFILE},
    profile::{
        RADROOTS_PROFILE_TYPE_TAG_ANY, RADROOTS_PROFILE_TYPE_TAG_COOP,
        RADROOTS_PROFILE_TYPE_TAG_FARM, RADROOTS_PROFILE_TYPE_TAG_KEY,
        RADROOTS_PROFILE_TYPE_TAG_RADROOTSD, RadrootsProfileType,
    },
};
use radroots_events_codec::error::EventParseError;
use radroots_events_codec::profile::decode::{
    data_from_event, parsed_from_event, profile_from_content,
};

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
fn profile_from_content_parses_optional_metadata_and_ignores_invalid_scalars() {
    let content = r#"{"name":"alice","display_name":"Alice","nip05":"alice@example.test","about":"farm account","website":"https://farm.example.test","picture":"https://farm.example.test/pic.png","banner":"https://farm.example.test/banner.png","lud06":"lnurl1farm","lud16":"alice@example.test","bot":12}"#;
    let profile = profile_from_content(content).unwrap();

    assert_eq!(profile.name, "alice");
    assert_eq!(profile.display_name.as_deref(), Some("Alice"));
    assert_eq!(profile.nip05.as_deref(), Some("alice@example.test"));
    assert_eq!(profile.about.as_deref(), Some("farm account"));
    assert_eq!(
        profile.website.as_deref(),
        Some("https://farm.example.test")
    );
    assert_eq!(
        profile.picture.as_deref(),
        Some("https://farm.example.test/pic.png")
    );
    assert_eq!(
        profile.banner.as_deref(),
        Some("https://farm.example.test/banner.png")
    );
    assert_eq!(profile.lud06.as_deref(), Some("lnurl1farm"));
    assert_eq!(profile.lud16.as_deref(), Some("alice@example.test"));
    assert_eq!(profile.bot, None);
}

#[test]
fn profile_from_content_rejects_missing_name() {
    let content = r#"{"display_name":"alice"}"#;
    let err = profile_from_content(content).unwrap_err();
    assert!(matches!(err, EventParseError::InvalidJson("name")));
}

#[test]
fn profile_from_content_rejects_non_object_json() {
    let err = profile_from_content("[]").unwrap_err();
    assert!(matches!(err, EventParseError::InvalidJson("content")));
}

#[test]
fn profile_from_content_rejects_invalid_json() {
    let err = profile_from_content("{").unwrap_err();
    assert!(matches!(err, EventParseError::InvalidJson("content")));
}

#[test]
fn profile_metadata_rejects_wrong_kind() {
    let err = data_from_event(
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
    let metadata = data_from_event(
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

    assert_eq!(metadata.data.profile_type, Some(RadrootsProfileType::Farm));
}

#[test]
fn profile_metadata_reads_profile_type_any_tag() {
    let metadata = data_from_event(
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

    assert_eq!(metadata.data.profile_type, Some(RadrootsProfileType::Any));
}

#[test]
fn profile_metadata_reads_profile_type_radrootsd_tag() {
    let metadata = data_from_event(
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

    assert_eq!(
        metadata.data.profile_type,
        Some(RadrootsProfileType::Radrootsd)
    );
}

#[test]
fn profile_metadata_ignores_short_unknown_and_unrelated_profile_type_tags() {
    let metadata = data_from_event(
        "id".to_string(),
        "author".to_string(),
        1,
        KIND_PROFILE,
        "{\"name\":\"alice\"}".to_string(),
        vec![
            vec![RADROOTS_PROFILE_TYPE_TAG_KEY.to_string()],
            vec![
                RADROOTS_PROFILE_TYPE_TAG_KEY.to_string(),
                "radroots:type:unknown".to_string(),
            ],
            vec!["x".to_string(), RADROOTS_PROFILE_TYPE_TAG_COOP.to_string()],
            vec![
                RADROOTS_PROFILE_TYPE_TAG_KEY.to_string(),
                RADROOTS_PROFILE_TYPE_TAG_COOP.to_string(),
            ],
        ],
    )
    .expect("metadata");

    assert_eq!(metadata.data.profile_type, Some(RadrootsProfileType::Coop));
}

#[test]
fn profile_parsed_event_preserves_wire_event_and_decoded_data() {
    let parsed = parsed_from_event(
        "event-id".to_string(),
        "author-pubkey".to_string(),
        42,
        KIND_PROFILE,
        "{\"name\":\"alice\"}".to_string(),
        vec![vec![
            RADROOTS_PROFILE_TYPE_TAG_KEY.to_string(),
            RADROOTS_PROFILE_TYPE_TAG_FARM.to_string(),
        ]],
        "event-sig".to_string(),
    )
    .expect("parsed profile");

    assert_eq!(parsed.event.id, "event-id");
    assert_eq!(parsed.event.author, "author-pubkey");
    assert_eq!(parsed.event.created_at, 42);
    assert_eq!(parsed.event.kind, KIND_PROFILE);
    assert_eq!(parsed.event.content, "{\"name\":\"alice\"}");
    assert_eq!(parsed.event.sig, "event-sig");
    assert_eq!(parsed.data.data.profile.name, "alice");
    assert_eq!(
        parsed.data.data.profile_type,
        Some(RadrootsProfileType::Farm)
    );
}
