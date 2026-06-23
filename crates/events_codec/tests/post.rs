use radroots_events::{
    farm::RadrootsFarmRef,
    kinds::{KIND_ARTICLE, KIND_COMMENT, KIND_FARM, KIND_POST},
    post::RadrootsPost,
    social::{
        RadrootsSocialFarmAnchor, RadrootsSocialLocation, RadrootsSocialMediaDimensions,
        RadrootsSocialMediaMetadata, RadrootsSocialMediaThumbnail, RadrootsSocialTarget,
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

fn content_post() -> RadrootsPost {
    RadrootsPost {
        content: "field update".to_string(),
        farm: None,
        address_refs: None,
        location: None,
        topics: None,
        quote_refs: None,
        media: None,
    }
}

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
fn post_build_tags_covers_optional_social_encode_branches() {
    let mut post = content_post();
    post.farm = Some(RadrootsSocialFarmAnchor {
        farm: RadrootsFarmRef {
            pubkey: "farm_pubkey".to_string(),
            d_tag: FARM_D_TAG.to_string(),
        },
        relays: Some(vec!["wss://farm-relay.example.test".to_string()]),
    });
    post.address_refs = Some(vec![RadrootsSocialTarget::Address {
        address: format!("30023:article_author:{ARTICLE_D_TAG}"),
        author: None,
        event_kind: None,
        relays: Some(vec!["wss://article-relay.example.test".to_string()]),
    }]);
    post.quote_refs = Some(vec![
        RadrootsSocialTarget::Event {
            id: QUOTE_ID.to_string(),
            author: None,
            event_kind: None,
            relays: Some(vec!["wss://quote-relay.example.test".to_string()]),
        },
        RadrootsSocialTarget::Address {
            address: format!("30023:quote_author:{ARTICLE_D_TAG}"),
            author: None,
            event_kind: None,
            relays: Some(vec!["wss://quote-address-relay.example.test".to_string()]),
        },
    ]);
    post.media = Some(vec![RadrootsSocialMediaMetadata {
        thumbnails: Some(vec![RadrootsSocialMediaThumbnail {
            url: "https://media.example.test/thumb.jpg".to_string(),
            dimensions: Some(RadrootsSocialMediaDimensions {
                width: 120,
                height: 80,
            }),
        }]),
        ..RadrootsSocialMediaMetadata::default()
    }]);

    let tags = post_build_tags(&post).unwrap();
    assert!(tags.iter().any(|tag| {
        tag.first().map(|value| value.as_str()) == Some(TAG_A)
            && tag
                .iter()
                .any(|value| value == "wss://farm-relay.example.test")
    }));
    assert!(tags.iter().any(|tag| {
        tag.first().map(|value| value.as_str()) == Some(TAG_A)
            && tag
                .iter()
                .any(|value| value == "wss://article-relay.example.test")
    }));
    assert!(tags.iter().any(|tag| {
        tag.first().map(|value| value.as_str()) == Some(TAG_Q)
            && tag
                .iter()
                .any(|value| value == "wss://quote-relay.example.test")
    }));
    assert!(tags.iter().any(|tag| {
        tag.first().map(|value| value.as_str()) == Some(TAG_Q)
            && tag
                .iter()
                .any(|value| value == "wss://quote-address-relay.example.test")
    }));
    assert!(tags.iter().any(|tag| {
        tag.first().map(|value| value.as_str()) == Some(TAG_IMETA)
            && tag.iter().any(|value| value == "dim 120x80")
    }));

    let mut no_relay_post = content_post();
    no_relay_post.farm = Some(RadrootsSocialFarmAnchor {
        farm: RadrootsFarmRef {
            pubkey: "farm_pubkey".to_string(),
            d_tag: FARM_D_TAG.to_string(),
        },
        relays: None,
    });
    no_relay_post.address_refs = Some(vec![RadrootsSocialTarget::Address {
        address: format!("30023:article_author:{ARTICLE_D_TAG}"),
        author: None,
        event_kind: None,
        relays: None,
    }]);
    no_relay_post.quote_refs = Some(vec![RadrootsSocialTarget::Event {
        id: QUOTE_ID.to_string(),
        author: None,
        event_kind: None,
        relays: None,
    }]);
    no_relay_post.media = Some(vec![RadrootsSocialMediaMetadata {
        thumbnails: Some(vec![RadrootsSocialMediaThumbnail {
            url: "https://media.example.test/thumb-no-dim.jpg".to_string(),
            dimensions: None,
        }]),
        ..RadrootsSocialMediaMetadata::default()
    }]);

    let tags = post_build_tags(&no_relay_post).unwrap();
    let farm_tag = tags
        .iter()
        .find(|tag| {
            tag.first().map(String::as_str) == Some(TAG_A)
                && tag.get(1).map(String::as_str)
                    == Some("30340:farm_pubkey:AAAAAAAAAAAAAAAAAAAAAA")
        })
        .expect("farm tag");
    assert_eq!(farm_tag.len(), 2);
    let address_tag = tags
        .iter()
        .find(|tag| {
            tag.first().map(String::as_str) == Some(TAG_A)
                && tag.get(1).map(String::as_str)
                    == Some("30023:article_author:BBBBBBBBBBBBBBBBBBBBBA")
        })
        .expect("address tag");
    assert_eq!(address_tag.len(), 2);
    let quote_tag = tags
        .iter()
        .find(|tag| tag.first().map(String::as_str) == Some(TAG_Q))
        .expect("quote tag");
    assert_eq!(quote_tag.len(), 2);
    let imeta = tags
        .iter()
        .find(|tag| tag.first().map(String::as_str) == Some(TAG_IMETA))
        .expect("imeta tag");
    assert!(
        imeta
            .iter()
            .any(|value| value == "thumb https://media.example.test/thumb-no-dim.jpg")
    );
    assert!(!imeta.iter().any(|value| value.starts_with("dim ")));
}

#[test]
fn post_social_tags_reject_malformed_supported_structures() {
    let mut post = content_post();
    post.address_refs = Some(vec![RadrootsSocialTarget::Event {
        id: QUOTE_ID.to_string(),
        author: None,
        event_kind: None,
        relays: None,
    }]);
    assert!(matches!(
        post_build_tags(&post),
        Err(EventEncodeError::InvalidField("address_refs"))
    ));

    post.address_refs = Some(vec![RadrootsSocialTarget::Address {
        address: "not-an-address".to_string(),
        author: None,
        event_kind: None,
        relays: None,
    }]);
    assert!(matches!(
        post_build_tags(&post),
        Err(EventEncodeError::InvalidField("address_refs"))
    ));

    post.address_refs = Some(vec![RadrootsSocialTarget::Address {
        address: format!("30340:farm_pubkey:{FARM_D_TAG}"),
        author: Some("farm_pubkey".to_string()),
        event_kind: Some(30340),
        relays: None,
    }]);
    assert!(matches!(
        post_build_tags(&post),
        Err(EventEncodeError::InvalidField("address_refs"))
    ));

    post.address_refs = Some(vec![RadrootsSocialTarget::Address {
        address: format!("30023:article_author:{ARTICLE_D_TAG}"),
        author: Some("other_author".to_string()),
        event_kind: Some(30023),
        relays: None,
    }]);
    assert!(matches!(
        post_build_tags(&post),
        Err(EventEncodeError::InvalidField("address_refs"))
    ));

    post.address_refs = Some(vec![RadrootsSocialTarget::Address {
        address: format!("30023:article_author:{ARTICLE_D_TAG}"),
        author: Some("article_author".to_string()),
        event_kind: Some(30024),
        relays: None,
    }]);
    assert!(matches!(
        post_build_tags(&post),
        Err(EventEncodeError::InvalidField("address_refs"))
    ));

    post.address_refs = None;
    post.farm = Some(RadrootsSocialFarmAnchor {
        farm: RadrootsFarmRef {
            pubkey: String::new(),
            d_tag: FARM_D_TAG.to_string(),
        },
        relays: None,
    });
    assert!(matches!(
        post_build_tags(&post),
        Err(EventEncodeError::EmptyRequiredField("farm.pubkey"))
    ));

    post.farm = Some(RadrootsSocialFarmAnchor {
        farm: RadrootsFarmRef {
            pubkey: "farm_pubkey".to_string(),
            d_tag: String::new(),
        },
        relays: None,
    });
    assert!(matches!(
        post_build_tags(&post),
        Err(EventEncodeError::EmptyRequiredField("farm.d_tag"))
    ));

    post.farm = Some(RadrootsSocialFarmAnchor {
        farm: RadrootsFarmRef {
            pubkey: "farm_pubkey".to_string(),
            d_tag: "bad d".to_string(),
        },
        relays: None,
    });
    assert!(matches!(
        post_build_tags(&post),
        Err(EventEncodeError::InvalidField("farm"))
    ));

    post.farm = None;
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

    post.quote_refs = Some(vec![RadrootsSocialTarget::Address {
        address: "not-an-address".to_string(),
        author: None,
        event_kind: None,
        relays: None,
    }]);
    assert!(matches!(
        post_build_tags(&post),
        Err(EventEncodeError::InvalidField("quote_refs"))
    ));

    post.quote_refs = Some(vec![RadrootsSocialTarget::Address {
        address: format!("30023:quote_author:{ARTICLE_D_TAG}"),
        author: None,
        event_kind: Some(30024),
        relays: None,
    }]);
    assert!(matches!(
        post_build_tags(&post),
        Err(EventEncodeError::InvalidField("quote_refs"))
    ));

    post.quote_refs = Some(vec![RadrootsSocialTarget::External {
        id: "https://example.test/object".to_string(),
        external_kind: "web".to_string(),
        hint: None,
    }]);
    assert!(matches!(
        post_build_tags(&post),
        Err(EventEncodeError::InvalidField("quote_refs"))
    ));

    post.quote_refs = None;
    post.media = Some(vec![RadrootsSocialMediaMetadata {
        imeta: Some(vec![Vec::new()]),
        ..RadrootsSocialMediaMetadata::default()
    }]);
    assert!(matches!(
        post_build_tags(&post),
        Err(EventEncodeError::InvalidField("imeta"))
    ));

    post.media = Some(vec![RadrootsSocialMediaMetadata {
        imeta: Some(vec![vec![" ".to_string()]]),
        ..RadrootsSocialMediaMetadata::default()
    }]);
    assert!(matches!(
        post_build_tags(&post),
        Err(EventEncodeError::InvalidField("imeta"))
    ));

    post.media = Some(vec![RadrootsSocialMediaMetadata {
        thumbnails: Some(vec![RadrootsSocialMediaThumbnail {
            url: " ".to_string(),
            dimensions: None,
        }]),
        ..RadrootsSocialMediaMetadata::default()
    }]);
    assert!(matches!(
        post_build_tags(&post),
        Err(EventEncodeError::InvalidField("imeta"))
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
fn post_media_structured_fields_encode_and_decode_imeta() {
    let mut post = content_post();
    post.topics = Some(vec![
        "soil".to_string(),
        " ".to_string(),
        "market".to_string(),
    ]);
    post.media = Some(vec![
        RadrootsSocialMediaMetadata::default(),
        RadrootsSocialMediaMetadata {
            url: Some("https://media.example.test/field.jpg".to_string()),
            mime_type: Some("image/jpeg".to_string()),
            sha256: Some(QUOTE_ID.to_string()),
            original_sha256: Some(QUOTE_ID.to_string()),
            size: Some(42),
            dimensions: Some(RadrootsSocialMediaDimensions {
                width: 1200,
                height: 800,
            }),
            blurhash: Some("LEHV6nWB2yk8pyo0adR*.7kCMdnj".to_string()),
            thumbnails: Some(vec![RadrootsSocialMediaThumbnail {
                url: "https://media.example.test/thumb.jpg".to_string(),
                dimensions: Some(RadrootsSocialMediaDimensions {
                    width: 120,
                    height: 80,
                }),
            }]),
            image: Some("https://media.example.test/poster.jpg".to_string()),
            summary: Some("Field row image".to_string()),
            alt: Some("rows in field".to_string()),
            fallback: Some("https://media.example.test/fallback.jpg".to_string()),
            magnet: Some("magnet:?xt=urn:btih:fixture".to_string()),
            content_hashes: Some(vec!["hash-a".to_string(), "hash-b".to_string()]),
            services: Some(vec!["https://media.example.test".to_string()]),
            imeta: None,
        },
    ]);

    let parts = to_wire_parts(&post).unwrap();
    let topic_tags = parts
        .tags
        .iter()
        .filter(|tag| tag.first().map(|value| value.as_str()) == Some(TAG_T))
        .count();
    assert_eq!(topic_tags, 2);

    let imeta = parts
        .tags
        .iter()
        .find(|tag| tag.first().map(|value| value.as_str()) == Some(TAG_IMETA))
        .expect("imeta tag");
    for expected in [
        "url https://media.example.test/field.jpg",
        "m image/jpeg",
        "size 42",
        "dim 1200x800",
        "blurhash LEHV6nWB2yk8pyo0adR*.7kCMdnj",
        "thumb https://media.example.test/thumb.jpg",
        "dim 120x80",
        "image https://media.example.test/poster.jpg",
        "summary Field row image",
        "alt rows in field",
        "fallback https://media.example.test/fallback.jpg",
        "magnet magnet:?xt=urn:btih:fixture",
        "i hash-a",
        "i hash-b",
        "service https://media.example.test",
    ] {
        assert!(imeta.iter().any(|value| value == expected), "{expected}");
    }

    let decoded = post_from_event(parts.kind, &parts.tags, &parts.content).unwrap();
    let media = decoded.media.expect("media");
    assert_eq!(media.len(), 1);
    assert_eq!(media[0].original_sha256.as_deref(), Some(QUOTE_ID));
    assert_eq!(media[0].size, Some(42));
    assert_eq!(
        media[0].blurhash.as_deref(),
        Some("LEHV6nWB2yk8pyo0adR*.7kCMdnj")
    );
    assert_eq!(
        media[0].image.as_deref(),
        Some("https://media.example.test/poster.jpg")
    );
    assert_eq!(media[0].summary.as_deref(), Some("Field row image"));
    assert_eq!(
        media[0].fallback.as_deref(),
        Some("https://media.example.test/fallback.jpg")
    );
    assert_eq!(
        media[0].magnet.as_deref(),
        Some("magnet:?xt=urn:btih:fixture")
    );
    assert_eq!(media[0].content_hashes.as_ref().map(Vec::len), Some(2));
}

#[test]
fn post_decode_rejects_more_invalid_imeta_shapes() {
    for tags in [
        vec![TAG_IMETA.to_string()],
        vec![TAG_IMETA.to_string(), " ".to_string()],
    ] {
        let err = post_from_event(KIND_POST, &[tags], "hello").unwrap_err();
        assert!(matches!(err, EventParseError::InvalidTag(TAG_IMETA)));
    }

    for entry in ["url ", "size not-a-number", "dim bad", "dim 0x10"] {
        let err = post_from_event(
            KIND_POST,
            &[vec![TAG_IMETA.to_string(), entry.to_string()]],
            "hello",
        )
        .unwrap_err();
        assert!(matches!(
            err,
            EventParseError::InvalidTag(TAG_IMETA) | EventParseError::InvalidNumber(TAG_IMETA, _)
        ));
    }
}

#[test]
fn post_decode_handles_non_farm_address_refs_without_relays() {
    let article = format!("30023:article_author:{ARTICLE_D_TAG}");
    let farm = format!("{KIND_FARM}:farm_pubkey:{FARM_D_TAG}");
    let decoded = post_from_event(
        KIND_POST,
        &[
            vec![TAG_A.to_string(), farm.clone()],
            vec![TAG_A.to_string(), article.clone()],
        ],
        "address only",
    )
    .unwrap();

    let anchor = decoded.farm.expect("farm anchor");
    assert_eq!(anchor.farm.d_tag, FARM_D_TAG);
    assert_eq!(anchor.relays, None);
    let refs = decoded.address_refs.expect("address refs");
    assert_eq!(refs.len(), 1);
    match &refs[0] {
        RadrootsSocialTarget::Address {
            address,
            author,
            event_kind,
            relays,
        } => {
            assert_eq!(address, &article);
            assert_eq!(author.as_deref(), Some("article_author"));
            assert_eq!(*event_kind, Some(30023));
            assert_eq!(relays, &None);
        }
        _ => panic!("expected address target"),
    }
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
