use radroots_blossom::{BlobDescriptor, BlobUrl, ByteVerifiedDescriptor, MediaType, Sha256};
use radroots_event::{
    media::AuthoredImage,
    post::{
        AuthoredAsk, AuthoredPhotoUpdate, AuthoredPostError, AuthoredPostImage, AuthoredUpdate,
        PostImageDimensions, RADROOTS_ASK_MARKER_TAG_VALUE, RADROOTS_POST_ALT_MAX_BYTES,
        RADROOTS_POST_CONTENT_MAX_BYTES, RADROOTS_POST_EVENT_WIRE_MAX_BYTES,
        RADROOTS_POST_IMETA_MAX_COUNT, RADROOTS_POST_TAG_ELEMENT_MAX_BYTES,
        RADROOTS_POST_TAG_TOTAL_MAX_BYTES, post_image_media_type_is_valid,
    },
};
use radroots_event_codec::encode::post::{
    authored_ask_to_wire_parts, authored_photo_update_to_wire_parts, authored_update_to_wire_parts,
};

#[test]
fn authored_update_emits_only_bounded_nonblank_content() {
    let update = AuthoredUpdate::new("The first strawberries are ready.").unwrap();
    let wire = authored_update_to_wire_parts(&update);

    assert_eq!(wire.kind, 1);
    assert_eq!(wire.content, update.content());
    assert!(wire.tags.is_empty());
    assert_eq!(
        AuthoredUpdate::new(" \t").unwrap_err().code(),
        "post_content_missing"
    );
}

#[test]
fn authored_update_enforces_the_utf8_byte_limit() {
    let maximum = "x".repeat(RADROOTS_POST_CONTENT_MAX_BYTES);
    assert!(AuthoredUpdate::new(maximum).is_ok());

    let over = "x".repeat(RADROOTS_POST_CONTENT_MAX_BYTES + 1);
    assert_eq!(
        AuthoredUpdate::new(over).unwrap_err(),
        AuthoredPostError::ContentTooLarge {
            max: RADROOTS_POST_CONTENT_MAX_BYTES,
            actual: RADROOTS_POST_CONTENT_MAX_BYTES + 1,
        }
    );
}

#[test]
fn authored_photo_emits_exact_nip92_order_and_repeatable_fallbacks() {
    let image = authored_image(b"strawberries", "image/webp", "webp")
        .try_with_fallback(fallback_url(b"strawberries", "cache-one.example", "webp"))
        .unwrap()
        .try_with_fallback(fallback_url(b"strawberries", "cache-two.example", "webp"))
        .unwrap();
    let content = format!("Today's harvest {}", image.url());
    let photo = AuthoredPhotoUpdate::new(content.clone(), vec![image]).unwrap();
    let wire = authored_photo_update_to_wire_parts(&photo);

    assert_eq!(wire.kind, 1);
    assert_eq!(wire.content, content);
    assert_eq!(wire.tags.len(), 1);
    assert_eq!(wire.tags[0].as_slice(), photo.images()[0].imeta_tag());
    assert_eq!(
        wire.tags[0]
            .iter()
            .map(|field| field.split_once(' ').map_or(field.as_str(), |part| part.0))
            .collect::<Vec<_>>(),
        [
            "imeta", "url", "x", "m", "dim", "size", "alt", "fallback", "fallback"
        ]
    );
}

#[test]
fn authored_ask_precedes_optional_media_with_one_exact_marker() {
    let image = authored_image(b"leaf", "image/jpeg", "jpg");
    let ask = AuthoredAsk::new(
        format!("Is this leaf healthy? {}", image.url()),
        vec![image],
    )
    .unwrap();
    let wire = authored_ask_to_wire_parts(&ask);

    assert_eq!(
        wire.tags[0],
        ["t".to_string(), RADROOTS_ASK_MARKER_TAG_VALUE.to_string()]
    );
    assert_eq!(wire.tags[1][0], "imeta");
}

#[test]
fn authored_photo_requires_exact_disjoint_content_url_occurrences() {
    let image = authored_image(b"leaf", "image/jpeg", "jpg");
    assert_eq!(
        AuthoredPhotoUpdate::new("photo", Vec::new()).unwrap_err(),
        AuthoredPostError::ImageMissing
    );
    assert_eq!(
        AuthoredPhotoUpdate::new("photo", vec![image.clone()])
            .unwrap_err()
            .code(),
        "imeta_url_occurrence_count"
    );
    let content = image.url().to_string();
    assert_eq!(
        AuthoredPhotoUpdate::new(content, vec![image.clone(), image]).unwrap_err(),
        AuthoredPostError::DuplicateImageUrl
    );

    let repeated = authored_image(b"repeated", "image/jpeg", "jpg");
    assert_eq!(
        AuthoredPhotoUpdate::new(
            format!("{} then {}", repeated.url(), repeated.url()),
            vec![repeated],
        )
        .unwrap_err(),
        AuthoredPostError::ImageUrlOccurrenceCount {
            expected: 1,
            actual: 2,
        }
    );

    let unicode = authored_image(b"unicode", "image/webp", "webp");
    assert!(AuthoredPhotoUpdate::new(format!("収穫 {} 🍓", unicode.url()), vec![unicode]).is_ok());

    let bytes = b"prefix";
    let hash = Sha256::digest(bytes);
    let short_url = format!("https://media.example/{hash}.webp");
    let long_url = format!("{short_url}2");
    let short = authored_image_with_url(bytes, "image/webp", &short_url);
    let long = authored_image_with_url(bytes, "image/webp", &long_url);
    assert_eq!(
        AuthoredPhotoUpdate::new(long_url, vec![short, long]).unwrap_err(),
        AuthoredPostError::ImageUrlOverlap
    );
}

#[test]
fn authored_photo_and_ask_enforce_the_imeta_count_limit() {
    let image = authored_image(b"leaf", "image/jpeg", "jpg");
    let images = vec![image.clone(); RADROOTS_POST_IMETA_MAX_COUNT + 1];
    let expected = AuthoredPostError::ImageCountExceeded {
        max: RADROOTS_POST_IMETA_MAX_COUNT,
        actual: RADROOTS_POST_IMETA_MAX_COUNT + 1,
    };

    assert_eq!(
        AuthoredPhotoUpdate::new(image.url(), images.clone()).unwrap_err(),
        expected
    );
    assert_eq!(AuthoredAsk::new("Question", images).unwrap_err(), expected);
}

#[test]
fn authored_image_rejects_parameterized_mime_zero_dimensions_and_wrong_fallback_hash() {
    let parameterized = AuthoredImage::try_from(verified_descriptor(
        b"leaf",
        "image/webp; charset=binary",
        "webp",
    ))
    .unwrap();
    assert_eq!(
        AuthoredPostImage::new(
            parameterized,
            PostImageDimensions::new(1, 1).unwrap(),
            "Leaf",
        )
        .unwrap_err()
        .code(),
        "imeta_mime_invalid"
    );
    assert_eq!(
        PostImageDimensions::new(0, 1).unwrap_err().code(),
        "imeta_dimensions_invalid"
    );

    let image = authored_image(b"leaf", "image/webp", "webp");
    assert_eq!(
        image
            .try_with_fallback(fallback_url(b"other", "cache.example", "webp"))
            .unwrap_err()
            .code(),
        "imeta_fallback_hash_mismatch"
    );
}

#[test]
fn post_image_mime_profile_uses_canonical_parameter_free_media_types() {
    assert!(post_image_media_type_is_valid("image/webp"));
    assert!(post_image_media_type_is_valid("image/svg+xml"));
    assert!(post_image_media_type_is_valid("image/vnd.microsoft.icon"));
    assert!(!post_image_media_type_is_valid("IMAGE/WEBP"));
    assert!(!post_image_media_type_is_valid("image/webp;quality=90"));
    assert!(!post_image_media_type_is_valid("text/plain"));
}

#[test]
fn authored_image_rejects_zero_size_and_invalid_alt_text() {
    let empty = AuthoredImage::try_from(verified_descriptor(b"", "image/webp", "webp")).unwrap();
    assert_eq!(
        AuthoredPostImage::new(
            empty,
            PostImageDimensions::new(1, 1).unwrap(),
            "Empty image",
        )
        .unwrap_err(),
        AuthoredPostError::ImageSizeInvalid
    );

    let blank_alt =
        AuthoredImage::try_from(verified_descriptor(b"leaf", "image/webp", "webp")).unwrap();
    assert_eq!(
        AuthoredPostImage::new(blank_alt, PostImageDimensions::new(1, 1).unwrap(), " \t",)
            .unwrap_err(),
        AuthoredPostError::ImageAltInvalid
    );

    let maximum_alt = "a".repeat(RADROOTS_POST_ALT_MAX_BYTES);
    let maximum =
        AuthoredImage::try_from(verified_descriptor(b"maximum", "image/webp", "webp")).unwrap();
    assert!(
        AuthoredPostImage::new(
            maximum,
            PostImageDimensions::new(1, 1).unwrap(),
            maximum_alt,
        )
        .is_ok()
    );

    let oversized_alt = "a".repeat(RADROOTS_POST_ALT_MAX_BYTES + 1);
    let oversized =
        AuthoredImage::try_from(verified_descriptor(b"oversized", "image/webp", "webp")).unwrap();
    assert_eq!(
        AuthoredPostImage::new(
            oversized,
            PostImageDimensions::new(1, 1).unwrap(),
            oversized_alt,
        )
        .unwrap_err(),
        AuthoredPostError::ImageAltTooLarge {
            max: RADROOTS_POST_ALT_MAX_BYTES,
            actual: RADROOTS_POST_ALT_MAX_BYTES + 1,
        }
    );
}

#[test]
fn authored_image_enforces_the_exact_tag_element_limit() {
    let maximum = AuthoredImage::try_from(verified_descriptor_with_url_element_bytes(
        b"maximum-url",
        RADROOTS_POST_TAG_ELEMENT_MAX_BYTES,
    ))
    .unwrap();
    let maximum = AuthoredPostImage::new(
        maximum,
        PostImageDimensions::new(1, 1).unwrap(),
        "Maximum URL",
    )
    .unwrap();
    assert_eq!(
        maximum.imeta_tag()[1].len(),
        RADROOTS_POST_TAG_ELEMENT_MAX_BYTES
    );

    let oversized = AuthoredImage::try_from(verified_descriptor_with_url_element_bytes(
        b"oversized-url",
        RADROOTS_POST_TAG_ELEMENT_MAX_BYTES + 1,
    ))
    .unwrap();
    assert_eq!(
        AuthoredPostImage::new(
            oversized,
            PostImageDimensions::new(1, 1).unwrap(),
            "Oversized URL",
        )
        .unwrap_err(),
        AuthoredPostError::TagElementTooLarge {
            max: RADROOTS_POST_TAG_ELEMENT_MAX_BYTES,
            actual: RADROOTS_POST_TAG_ELEMENT_MAX_BYTES + 1,
        }
    );

    let maximum_fallback = authored_image(b"maximum-fallback", "image/webp", "webp")
        .try_with_fallback(fallback_url_with_element_bytes(
            b"maximum-fallback",
            RADROOTS_POST_TAG_ELEMENT_MAX_BYTES,
        ))
        .unwrap();
    assert_eq!(
        maximum_fallback.imeta_tag().last().unwrap().len(),
        RADROOTS_POST_TAG_ELEMENT_MAX_BYTES
    );
    assert_eq!(
        authored_image(b"oversized-fallback", "image/webp", "webp")
            .try_with_fallback(fallback_url_with_element_bytes(
                b"oversized-fallback",
                RADROOTS_POST_TAG_ELEMENT_MAX_BYTES + 1,
            ))
            .unwrap_err(),
        AuthoredPostError::TagElementTooLarge {
            max: RADROOTS_POST_TAG_ELEMENT_MAX_BYTES,
            actual: RADROOTS_POST_TAG_ELEMENT_MAX_BYTES + 1,
        }
    );
}

#[test]
fn authored_posts_enforce_the_exact_aggregate_tag_byte_limit() {
    let second = authored_image(b"second-image", "image/webp", "webp");
    let second_tag_bytes = tag_bytes(second.imeta_tag());
    let first_target = RADROOTS_POST_TAG_TOTAL_MAX_BYTES - second_tag_bytes;
    let first = fill_image_tag_bytes(
        authored_image(b"first-image", "image/webp", "webp"),
        b"first-image",
        first_target,
    );
    let content = format!("{} {}", first.url(), second.url());
    let photo =
        AuthoredPhotoUpdate::new(content.clone(), vec![first.clone(), second.clone()]).unwrap();
    let wire = authored_photo_update_to_wire_parts(&photo);
    assert_eq!(
        wire.tags.iter().map(|tag| tag_bytes(tag)).sum::<usize>(),
        RADROOTS_POST_TAG_TOTAL_MAX_BYTES
    );

    let first_over = fill_image_tag_bytes(
        authored_image(b"first-over", "image/webp", "webp"),
        b"first-over",
        first_target + 1,
    );
    let over_content = format!("{} {}", first_over.url(), second.url());
    assert_eq!(
        AuthoredPhotoUpdate::new(over_content, vec![first_over, second]).unwrap_err(),
        AuthoredPostError::TagBytesExceeded {
            max: RADROOTS_POST_TAG_TOTAL_MAX_BYTES,
            actual: RADROOTS_POST_TAG_TOTAL_MAX_BYTES + 1,
        }
    );

    let full = fill_image_tag_bytes(
        authored_image(b"full-image", "image/webp", "webp"),
        b"full-image",
        RADROOTS_POST_TAG_TOTAL_MAX_BYTES,
    );
    assert!(
        AuthoredPhotoUpdate::new(full.url(), vec![full.clone()]).is_ok(),
        "PhotoUpdate must accept the exact aggregate maximum"
    );
    assert_eq!(
        AuthoredAsk::new(full.url(), vec![full.clone()]).unwrap_err(),
        AuthoredPostError::TagBytesExceeded {
            max: RADROOTS_POST_TAG_TOTAL_MAX_BYTES,
            actual: RADROOTS_POST_TAG_TOTAL_MAX_BYTES
                + "t".len()
                + RADROOTS_ASK_MARKER_TAG_VALUE.len(),
        }
    );

    let minimum_fallback_bytes = fallback_element_prefix(b"full-image").len() + 1;
    assert_eq!(
        full.try_with_fallback(fallback_url_with_element_bytes(
            b"full-image",
            minimum_fallback_bytes,
        ))
        .unwrap_err(),
        AuthoredPostError::TagBytesExceeded {
            max: RADROOTS_POST_TAG_TOTAL_MAX_BYTES,
            actual: RADROOTS_POST_TAG_TOTAL_MAX_BYTES + minimum_fallback_bytes,
        }
    );
}

#[test]
fn authored_posts_enforce_the_exact_canonical_signed_event_budget() {
    let image = fill_image_tag_bytes(
        authored_image(b"wire-budget", "image/webp", "webp"),
        b"wire-budget",
        RADROOTS_POST_TAG_TOTAL_MAX_BYTES,
    );
    let tags = vec![image.imeta_tag().to_vec()];
    let minimum_content = image.url();
    let minimum_wire_bytes = canonical_signed_post_json_bytes(minimum_content, &tags);
    let padding_bytes = RADROOTS_POST_EVENT_WIRE_MAX_BYTES - minimum_wire_bytes;
    let maximum_content = format!("{minimum_content}{}", "x".repeat(padding_bytes));

    let maximum = AuthoredPhotoUpdate::new(maximum_content.clone(), vec![image.clone()]).unwrap();
    let maximum_wire = authored_photo_update_to_wire_parts(&maximum);
    assert_eq!(
        canonical_signed_post_json_bytes(&maximum_wire.content, &maximum_wire.tags),
        RADROOTS_POST_EVENT_WIRE_MAX_BYTES
    );

    let oversized_content = format!("{maximum_content}x");
    assert_eq!(
        AuthoredPhotoUpdate::new(oversized_content, vec![image.clone()]).unwrap_err(),
        AuthoredPostError::EventWireTooLarge {
            max: RADROOTS_POST_EVENT_WIRE_MAX_BYTES,
            actual: RADROOTS_POST_EVENT_WIRE_MAX_BYTES + 1,
        }
    );

    let mut escaped_content = maximum_content;
    escaped_content.pop();
    escaped_content.push('"');
    assert_eq!(
        AuthoredPhotoUpdate::new(escaped_content, vec![image]).unwrap_err(),
        AuthoredPostError::EventWireTooLarge {
            max: RADROOTS_POST_EVENT_WIRE_MAX_BYTES,
            actual: RADROOTS_POST_EVENT_WIRE_MAX_BYTES + 1,
        }
    );
}

#[test]
fn authored_update_counts_json_control_character_expansion() {
    let content = format!(
        "a{}",
        "\u{0001}".repeat(RADROOTS_POST_CONTENT_MAX_BYTES - 1)
    );
    let actual = canonical_signed_post_json_bytes(&content, &[]);

    assert_eq!(
        AuthoredUpdate::new(content).unwrap_err(),
        AuthoredPostError::EventWireTooLarge {
            max: RADROOTS_POST_EVENT_WIRE_MAX_BYTES,
            actual,
        }
    );
    assert!(actual > RADROOTS_POST_EVENT_WIRE_MAX_BYTES);
}

fn authored_image(bytes: &[u8], media_type: &str, extension: &str) -> AuthoredPostImage {
    AuthoredPostImage::new(
        AuthoredImage::try_from(verified_descriptor(bytes, media_type, extension)).unwrap(),
        PostImageDimensions::new(1200, 900).unwrap(),
        "Harvest",
    )
    .unwrap()
}

fn authored_image_with_url(bytes: &[u8], media_type: &str, url: &str) -> AuthoredPostImage {
    let hash = Sha256::digest(bytes);
    let media_type = MediaType::parse(media_type).unwrap();
    let descriptor = BlobDescriptor::new(
        BlobUrl::parse(url).unwrap(),
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
    AuthoredPostImage::new(
        AuthoredImage::try_from(descriptor).unwrap(),
        PostImageDimensions::new(1200, 900).unwrap(),
        "Harvest",
    )
    .unwrap()
}

fn verified_descriptor(bytes: &[u8], media_type: &str, extension: &str) -> ByteVerifiedDescriptor {
    let hash = Sha256::digest(bytes);
    let media_type = MediaType::parse(media_type).unwrap();
    BlobDescriptor::new(
        BlobUrl::parse(&format!("https://media.example/{hash}.{extension}")).unwrap(),
        hash,
        bytes.len() as u64,
        media_type.clone(),
        1_784_347_200,
    )
    .unwrap()
    .approve_reference()
    .unwrap()
    .verify_bytes(bytes, &media_type)
    .unwrap()
}

fn verified_descriptor_with_url_element_bytes(
    bytes: &[u8],
    element_bytes: usize,
) -> ByteVerifiedDescriptor {
    let hash = Sha256::digest(bytes);
    let prefix = format!("https://media.example/{hash}.");
    let extension_bytes = element_bytes
        .checked_sub("url ".len() + prefix.len())
        .expect("requested URL tag element is large enough");
    assert!(extension_bytes > 0);
    let media_type = MediaType::parse("image/webp").unwrap();
    let descriptor = BlobDescriptor::new(
        BlobUrl::parse(&format!("{prefix}{}", "x".repeat(extension_bytes))).unwrap(),
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
    assert_eq!(
        "url ".len() + descriptor.url().as_str().len(),
        element_bytes
    );
    descriptor
}

fn fallback_url(
    bytes: &[u8],
    host: &str,
    extension: &str,
) -> radroots_blossom::url::ApprovedBlobUrl {
    let hash = Sha256::digest(bytes);
    BlobUrl::parse(&format!("https://{host}/{hash}.{extension}"))
        .unwrap()
        .approve()
        .unwrap()
}

fn fallback_url_with_element_bytes(
    bytes: &[u8],
    element_bytes: usize,
) -> radroots_blossom::url::ApprovedBlobUrl {
    let prefix = fallback_element_prefix(bytes);
    let extension_bytes = element_bytes
        .checked_sub(prefix.len())
        .expect("requested fallback tag element is large enough");
    assert!(extension_bytes > 0);
    let fallback = BlobUrl::parse(&format!(
        "{}{}",
        prefix.strip_prefix("fallback ").unwrap(),
        "x".repeat(extension_bytes)
    ))
    .unwrap()
    .approve()
    .unwrap();
    assert_eq!(format!("fallback {fallback}").len(), element_bytes);
    fallback
}

fn fallback_element_prefix(bytes: &[u8]) -> String {
    let hash = Sha256::digest(bytes);
    format!("fallback https://cache.example/{hash}.")
}

fn tag_bytes(tag: &[String]) -> usize {
    tag.iter().map(String::len).sum()
}

fn canonical_signed_post_json_bytes(content: &str, tags: &[Vec<String>]) -> usize {
    serde_json::json!({
        "id": "0".repeat(64),
        "pubkey": "0".repeat(64),
        "created_at": u64::MAX,
        "kind": 1,
        "tags": tags,
        "content": content,
        "sig": "0".repeat(128),
    })
    .to_string()
    .len()
}

fn fill_image_tag_bytes(
    mut image: AuthoredPostImage,
    bytes: &[u8],
    target: usize,
) -> AuthoredPostImage {
    let minimum_fallback_bytes = fallback_element_prefix(bytes).len() + 1;
    while tag_bytes(image.imeta_tag()) < target {
        let remaining = target - tag_bytes(image.imeta_tag());
        assert!(remaining >= minimum_fallback_bytes);
        let mut next = remaining.min(RADROOTS_POST_TAG_ELEMENT_MAX_BYTES);
        if remaining > RADROOTS_POST_TAG_ELEMENT_MAX_BYTES
            && remaining - RADROOTS_POST_TAG_ELEMENT_MAX_BYTES < minimum_fallback_bytes
        {
            next = remaining - minimum_fallback_bytes;
        }
        assert!((minimum_fallback_bytes..=RADROOTS_POST_TAG_ELEMENT_MAX_BYTES).contains(&next));
        image = image
            .try_with_fallback(fallback_url_with_element_bytes(bytes, next))
            .unwrap();
    }
    assert_eq!(tag_bytes(image.imeta_tag()), target);
    image
}
