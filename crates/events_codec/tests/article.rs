#![cfg(feature = "serde_json")]

use radroots_events::{
    article::RadrootsArticle,
    farm::RadrootsFarmRef,
    kinds::{KIND_ARTICLE, KIND_POST},
    social::{RadrootsSocialFarmAnchor, RadrootsSocialLocation},
    tags::{TAG_A, TAG_D, TAG_G, TAG_IMAGE, TAG_LOCATION, TAG_PUBLISHED_AT, TAG_T, TAG_TITLE},
};
use radroots_events_codec::{
    article::{
        decode::{article_from_event, data_from_event, parsed_from_event},
        encode::{article_build_tags, to_wire_parts, to_wire_parts_with_kind},
    },
    error::{EventEncodeError, EventParseError},
};

const VALID_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAAAA";
const FARM_D_TAG: &str = "BBBBBBBBBBBBBBBBBBBBBA";
const FARM_PUBKEY: &str = "farm_pubkey";

fn sample_article() -> RadrootsArticle {
    RadrootsArticle {
        d_tag: VALID_D_TAG.to_string(),
        title: "Spring soil notes".to_string(),
        content: "# Spring soil notes".to_string(),
        summary: Some("Field update".to_string()),
        image: Some("https://media.example.test/soil.jpg".to_string()),
        published_at: Some(1_781_895_600),
        farm: Some(RadrootsSocialFarmAnchor {
            farm: RadrootsFarmRef {
                pubkey: FARM_PUBKEY.to_string(),
                d_tag: FARM_D_TAG.to_string(),
            },
            relays: None,
        }),
        location: Some(RadrootsSocialLocation {
            name: Some("North field".to_string()),
            geohash: Some("c23nb62w20st".to_string()),
        }),
        topics: Some(vec!["soil".to_string(), "cover-crops".to_string()]),
    }
}

fn has_tag(tags: &[Vec<String>], key: &str, value: &str) -> bool {
    tags.iter().any(|tag| {
        tag.first().map(|entry| entry.as_str()) == Some(key)
            && tag.get(1).map(|entry| entry.as_str()) == Some(value)
    })
}

#[test]
fn article_to_wire_parts_roundtrips_social_metadata() {
    let article = sample_article();
    let parts = to_wire_parts(&article).unwrap();

    assert_eq!(parts.kind, KIND_ARTICLE);
    assert_eq!(parts.content, article.content);
    assert!(has_tag(&parts.tags, TAG_D, VALID_D_TAG));
    assert!(has_tag(&parts.tags, TAG_TITLE, "Spring soil notes"));
    assert!(has_tag(
        &parts.tags,
        TAG_IMAGE,
        "https://media.example.test/soil.jpg"
    ));
    assert!(has_tag(&parts.tags, TAG_PUBLISHED_AT, "1781895600"));
    assert!(has_tag(&parts.tags, TAG_LOCATION, "North field"));
    assert!(has_tag(&parts.tags, TAG_G, "c23nb62w20st"));
    assert!(has_tag(
        &parts.tags,
        TAG_A,
        "30340:farm_pubkey:BBBBBBBBBBBBBBBBBBBBBA"
    ));
    assert!(has_tag(&parts.tags, TAG_T, "soil"));
    assert!(has_tag(&parts.tags, TAG_T, "cover-crops"));

    let decoded = article_from_event(parts.kind, &parts.tags, &parts.content).unwrap();
    assert_eq!(decoded.d_tag, VALID_D_TAG);
    assert_eq!(decoded.title, "Spring soil notes");
    assert_eq!(decoded.content, "# Spring soil notes");
    assert_eq!(decoded.summary.as_deref(), Some("Field update"));
    assert_eq!(decoded.published_at, Some(1_781_895_600));
    assert_eq!(
        decoded.farm.as_ref().map(|farm| farm.farm.pubkey.as_str()),
        Some(FARM_PUBKEY)
    );
    assert_eq!(
        decoded
            .location
            .as_ref()
            .and_then(|location| location.name.as_deref()),
        Some("North field")
    );
    assert_eq!(decoded.topics.as_ref().map(Vec::len), Some(2));
}

#[test]
fn article_codec_requires_kind_required_fields_and_valid_d_tag() {
    let mut article = sample_article();
    article.title = " ".to_string();
    assert!(matches!(
        article_build_tags(&article),
        Err(EventEncodeError::EmptyRequiredField("title"))
    ));

    let mut article = sample_article();
    article.d_tag = "bad".to_string();
    assert!(matches!(
        to_wire_parts(&article),
        Err(EventEncodeError::InvalidField("d_tag"))
    ));

    assert!(matches!(
        to_wire_parts_with_kind(&sample_article(), KIND_POST),
        Err(EventEncodeError::InvalidKind(KIND_POST))
    ));

    let mut tags = article_build_tags(&sample_article()).unwrap();
    tags.retain(|tag| tag.first().map(|value| value.as_str()) != Some(TAG_TITLE));
    assert!(matches!(
        article_from_event(KIND_ARTICLE, &tags, "# Spring soil notes"),
        Err(EventParseError::MissingTag(TAG_TITLE))
    ));

    let err = article_from_event(KIND_POST, &tags, "# Spring soil notes").unwrap_err();
    assert!(matches!(
        err,
        EventParseError::InvalidKind {
            expected: "30023",
            got: KIND_POST
        }
    ));
}

#[test]
fn article_wrappers_preserve_event_metadata() {
    let article = sample_article();
    let parts = to_wire_parts(&article).unwrap();
    let data = data_from_event(
        "event_id".to_string(),
        "author".to_string(),
        42,
        parts.kind,
        parts.content.clone(),
        parts.tags.clone(),
    )
    .unwrap();

    assert_eq!(data.id, "event_id");
    assert_eq!(data.author, "author");
    assert_eq!(data.published_at, 42);
    assert_eq!(data.kind, KIND_ARTICLE);
    assert_eq!(data.data.title, "Spring soil notes");

    let parsed = parsed_from_event(
        "event_id".to_string(),
        "author".to_string(),
        42,
        parts.kind,
        parts.content,
        parts.tags,
        "sig".to_string(),
    )
    .unwrap();

    assert_eq!(parsed.event.sig, "sig");
    assert_eq!(parsed.data.data.d_tag, VALID_D_TAG);
}
