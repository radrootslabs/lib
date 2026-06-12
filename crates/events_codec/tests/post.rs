use radroots_events::{
    farm::RadrootsFarmRef,
    kinds::{KIND_ARTICLE, KIND_COMMENT, KIND_POST},
    post::RadrootsPost,
    social::{
        RadrootsSocialFarmAnchor, RadrootsSocialLocation, RadrootsSocialMediaMetadata,
        RadrootsSocialTarget,
    },
    tags::{TAG_A, TAG_G, TAG_IMETA, TAG_LOCATION, TAG_Q, TAG_T},
};
use radroots_events_codec::error::{EventEncodeError, EventParseError};
use radroots_events_codec::post::decode::{
    data_from_event, parsed_from_event, post_from_content, post_from_event,
};
use radroots_events_codec::post::encode::{
    post_build_tags, to_wire_parts, to_wire_parts_with_kind,
};

const QUOTE_ID: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const FARM_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAAAA";
const ARTICLE_D_TAG: &str = "BBBBBBBBBBBBBBBBBBBBBA";

#[test]
fn post_to_wire_parts_requires_content() {
    let post = RadrootsPost {
        content: "   ".to_string(),
        farm: None,
        address_refs: None,
        location: None,
        topics: None,
        quote_refs: None,
        media: None,
    };

    let err = to_wire_parts(&post).unwrap_err();
    assert!(matches!(
        err,
        EventEncodeError::EmptyRequiredField("content")
    ));
}

#[test]
fn post_to_wire_parts_sets_kind_and_content() {
    let post = RadrootsPost {
        content: "hello".to_string(),
        farm: None,
        address_refs: None,
        location: None,
        topics: None,
        quote_refs: None,
        media: None,
    };

    let parts = to_wire_parts(&post).unwrap();
    assert_eq!(parts.kind, KIND_POST);
    assert_eq!(parts.content, "hello");
    assert!(parts.tags.is_empty());
}

#[test]
fn post_to_wire_parts_with_kind_rejects_non_post_kind() {
    let post = RadrootsPost {
        content: "hello".to_string(),
        farm: None,
        address_refs: None,
        location: None,
        topics: None,
        quote_refs: None,
        media: None,
    };

    assert!(matches!(
        to_wire_parts_with_kind(&post, KIND_ARTICLE),
        Err(EventEncodeError::InvalidKind(KIND_ARTICLE))
    ));
}

#[test]
fn post_to_wire_parts_roundtrips_optional_social_tags() {
    let post = RadrootsPost {
        content: "field update".to_string(),
        farm: Some(RadrootsSocialFarmAnchor {
            farm: RadrootsFarmRef {
                pubkey: "farm_pubkey".to_string(),
                d_tag: FARM_D_TAG.to_string(),
            },
            relays: Some(vec!["wss://farm-relay.example.test".to_string()]),
        }),
        address_refs: Some(vec![RadrootsSocialTarget::Address {
            address: format!("30023:article_author:{ARTICLE_D_TAG}"),
            author: Some("article_author".to_string()),
            event_kind: Some(30023),
            relays: Some(vec!["wss://article-relay.example.test".to_string()]),
        }]),
        location: Some(RadrootsSocialLocation {
            name: Some("North field".to_string()),
            geohash: Some("c23nb62w20st".to_string()),
        }),
        topics: Some(vec!["soil".to_string(), "cover-crops".to_string()]),
        quote_refs: Some(vec![
            RadrootsSocialTarget::Event {
                id: QUOTE_ID.to_string(),
                author: None,
                event_kind: None,
                relays: Some(vec!["wss://quote-relay.example.test".to_string()]),
            },
            RadrootsSocialTarget::Address {
                address: format!("30023:quote_author:{ARTICLE_D_TAG}"),
                author: Some("quote_author".to_string()),
                event_kind: Some(30023),
                relays: None,
            },
        ]),
        media: Some(vec![RadrootsSocialMediaMetadata {
            imeta: Some(vec![vec![
                "url https://media.example.test/field.jpg".to_string(),
                "m image/jpeg".to_string(),
                format!("x {QUOTE_ID}"),
                "dim 1200x800".to_string(),
                "alt Field rows".to_string(),
                "service https://media.example.test".to_string(),
            ]]),
            ..RadrootsSocialMediaMetadata::default()
        }]),
    };

    let parts = to_wire_parts(&post).unwrap();
    assert_eq!(parts.kind, KIND_POST);
    assert!(parts.tags.iter().any(|tag| {
        tag.first().map(|value| value.as_str()) == Some(TAG_A)
            && tag.get(1).map(|value| value.as_str())
                == Some("30340:farm_pubkey:AAAAAAAAAAAAAAAAAAAAAA")
    }));
    assert!(parts.tags.iter().any(|tag| {
        tag.first().map(|value| value.as_str()) == Some(TAG_A)
            && tag.get(1).map(|value| value.as_str())
                == Some("30023:article_author:BBBBBBBBBBBBBBBBBBBBBA")
    }));
    assert!(parts.tags.iter().any(|tag| {
        tag.first().map(|value| value.as_str()) == Some(TAG_LOCATION)
            && tag.get(1).map(|value| value.as_str()) == Some("North field")
    }));
    assert!(parts.tags.iter().any(|tag| {
        tag.first().map(|value| value.as_str()) == Some(TAG_G)
            && tag.get(1).map(|value| value.as_str()) == Some("c23nb62w20st")
    }));
    assert!(parts.tags.iter().any(|tag| {
        tag.first().map(|value| value.as_str()) == Some(TAG_T)
            && tag.get(1).map(|value| value.as_str()) == Some("soil")
    }));
    assert!(parts.tags.iter().any(|tag| {
        tag.first().map(|value| value.as_str()) == Some(TAG_Q)
            && tag.get(1).map(|value| value.as_str()) == Some(QUOTE_ID)
    }));
    assert!(parts.tags.iter().any(|tag| {
        tag.first().map(|value| value.as_str()) == Some(TAG_IMETA)
            && tag
                .iter()
                .any(|value| value == "url https://media.example.test/field.jpg")
    }));

    let decoded = post_from_event(parts.kind, &parts.tags, &parts.content).unwrap();
    assert_eq!(decoded.content, "field update");
    assert_eq!(
        decoded.farm.as_ref().map(|farm| farm.farm.pubkey.as_str()),
        Some("farm_pubkey")
    );
    assert_eq!(decoded.address_refs.as_ref().map(Vec::len), Some(1));
    assert_eq!(
        decoded
            .location
            .as_ref()
            .and_then(|location| location.name.as_deref()),
        Some("North field")
    );
    assert_eq!(decoded.topics.as_ref().map(Vec::len), Some(2));
    assert_eq!(decoded.quote_refs.as_ref().map(Vec::len), Some(2));
    let media = decoded.media.as_ref().expect("media");
    assert_eq!(
        media[0].url.as_deref(),
        Some("https://media.example.test/field.jpg")
    );
    assert_eq!(media[0].mime_type.as_deref(), Some("image/jpeg"));
    assert_eq!(
        media[0].dimensions.as_ref().map(|value| value.width),
        Some(1200)
    );
    assert_eq!(media[0].alt.as_deref(), Some("Field rows"));
    assert_eq!(media[0].services.as_ref().map(Vec::len), Some(1));
}

#[test]
fn post_social_tags_reject_malformed_supported_structures() {
    let mut post = RadrootsPost {
        content: "field update".to_string(),
        farm: None,
        address_refs: Some(vec![RadrootsSocialTarget::Event {
            id: QUOTE_ID.to_string(),
            author: None,
            event_kind: None,
            relays: None,
        }]),
        location: None,
        topics: None,
        quote_refs: None,
        media: None,
    };
    assert!(matches!(
        post_build_tags(&post),
        Err(EventEncodeError::InvalidField("address_refs"))
    ));

    post.address_refs = None;
    post.quote_refs = Some(vec![RadrootsSocialTarget::Event {
        id: "not-hex".to_string(),
        author: None,
        event_kind: None,
        relays: None,
    }]);
    assert!(matches!(
        post_build_tags(&post),
        Err(EventEncodeError::InvalidField("quote_refs"))
    ));

    let err = post_from_event(
        KIND_POST,
        &[vec![TAG_IMETA.to_string(), "bad-imeta-entry".to_string()]],
        "hello",
    )
    .unwrap_err();
    assert!(matches!(err, EventParseError::InvalidTag(TAG_IMETA)));
}

#[test]
fn post_from_content_requires_kind_and_content() {
    let err = post_from_content(KIND_COMMENT, "hello").unwrap_err();
    assert!(matches!(
        err,
        EventParseError::InvalidKind {
            expected: "1",
            got: KIND_COMMENT
        }
    ));

    let err = post_from_content(KIND_POST, "   ").unwrap_err();
    assert!(matches!(err, EventParseError::InvalidTag("content")));
}

#[test]
fn post_metadata_and_index_from_event_roundtrip() {
    let metadata = data_from_event(
        "id".to_string(),
        "author".to_string(),
        77,
        KIND_POST,
        "hello".to_string(),
        Vec::new(),
    )
    .unwrap();
    assert_eq!(metadata.id, "id");
    assert_eq!(metadata.author, "author");
    assert_eq!(metadata.published_at, 77);
    assert_eq!(metadata.kind, KIND_POST);
    assert_eq!(metadata.data.content, "hello");

    let index = parsed_from_event(
        "id".to_string(),
        "author".to_string(),
        77,
        KIND_POST,
        "hello".to_string(),
        Vec::new(),
        "sig".to_string(),
    )
    .unwrap();
    assert_eq!(index.event.id, "id");
    assert_eq!(index.event.author, "author");
    assert_eq!(index.event.created_at, 77);
    assert_eq!(index.event.kind, KIND_POST);
    assert_eq!(index.event.content, "hello");
    assert_eq!(index.event.sig, "sig");
    assert_eq!(index.data.data.content, "hello");
}

#[test]
fn post_index_from_event_propagates_parse_errors() {
    let err = parsed_from_event(
        "id".to_string(),
        "author".to_string(),
        77,
        KIND_COMMENT,
        "hello".to_string(),
        Vec::new(),
        "sig".to_string(),
    )
    .unwrap_err();
    assert!(matches!(
        err,
        EventParseError::InvalidKind {
            expected: "1",
            got: KIND_COMMENT
        }
    ));
}
