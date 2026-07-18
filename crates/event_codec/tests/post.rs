use radroots_blossom::{
    RadrootsBlossomBlobDescriptor, RadrootsBlossomBlobUrl, RadrootsBlossomByteVerifiedDescriptor,
    RadrootsBlossomMediaType, RadrootsBlossomSha256,
};
use radroots_event::{
    RadrootsAuthoredImage,
    post::{
        RADROOTS_ASK_MARKER_TAG_VALUE, RADROOTS_POST_ALT_MAX_BYTES,
        RADROOTS_POST_CONTENT_MAX_BYTES, RADROOTS_POST_IMETA_MAX_COUNT, RadrootsAuthoredAsk,
        RadrootsAuthoredPhotoUpdate, RadrootsAuthoredPostError, RadrootsAuthoredPostImage,
        RadrootsAuthoredUpdate, RadrootsPostImageDimensions, post_image_media_type_is_valid,
    },
};
use radroots_event_codec::post::authored::{
    authored_ask_to_wire_parts, authored_photo_update_to_wire_parts, authored_update_to_wire_parts,
};

#[test]
fn authored_update_emits_only_bounded_nonblank_content() {
    let update = RadrootsAuthoredUpdate::new("The first strawberries are ready.").unwrap();
    let wire = authored_update_to_wire_parts(&update);

    assert_eq!(wire.kind, 1);
    assert_eq!(wire.content, update.content());
    assert!(wire.tags.is_empty());
    assert_eq!(
        RadrootsAuthoredUpdate::new(" \t").unwrap_err().code(),
        "post_content_missing"
    );
}

#[test]
fn authored_update_enforces_the_utf8_byte_limit() {
    let maximum = "x".repeat(RADROOTS_POST_CONTENT_MAX_BYTES);
    assert!(RadrootsAuthoredUpdate::new(maximum).is_ok());

    let over = "x".repeat(RADROOTS_POST_CONTENT_MAX_BYTES + 1);
    assert_eq!(
        RadrootsAuthoredUpdate::new(over).unwrap_err(),
        RadrootsAuthoredPostError::ContentTooLarge {
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
    let photo = RadrootsAuthoredPhotoUpdate::new(content.clone(), vec![image]).unwrap();
    let wire = authored_photo_update_to_wire_parts(&photo);

    assert_eq!(wire.kind, 1);
    assert_eq!(wire.content, content);
    assert_eq!(wire.tags.len(), 1);
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
    let ask = RadrootsAuthoredAsk::new(
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
fn authored_photo_rejects_missing_and_duplicate_content_urls() {
    let image = authored_image(b"leaf", "image/jpeg", "jpg");
    assert_eq!(
        RadrootsAuthoredPhotoUpdate::new("photo", Vec::new()).unwrap_err(),
        RadrootsAuthoredPostError::ImageMissing
    );
    assert_eq!(
        RadrootsAuthoredPhotoUpdate::new("photo", vec![image.clone()])
            .unwrap_err()
            .code(),
        "imeta_url_missing_from_content"
    );
    let content = image.url().to_string();
    assert_eq!(
        RadrootsAuthoredPhotoUpdate::new(content, vec![image.clone(), image]).unwrap_err(),
        RadrootsAuthoredPostError::DuplicateImageUrl
    );
}

#[test]
fn authored_photo_and_ask_enforce_the_imeta_count_limit() {
    let image = authored_image(b"leaf", "image/jpeg", "jpg");
    let images = vec![image.clone(); RADROOTS_POST_IMETA_MAX_COUNT + 1];
    let expected = RadrootsAuthoredPostError::ImageCountExceeded {
        max: RADROOTS_POST_IMETA_MAX_COUNT,
        actual: RADROOTS_POST_IMETA_MAX_COUNT + 1,
    };

    assert_eq!(
        RadrootsAuthoredPhotoUpdate::new(image.url(), images.clone()).unwrap_err(),
        expected
    );
    assert_eq!(
        RadrootsAuthoredAsk::new("Question", images).unwrap_err(),
        expected
    );
}

#[test]
fn authored_image_rejects_parameterized_mime_zero_dimensions_and_wrong_fallback_hash() {
    let parameterized = RadrootsAuthoredImage::try_from(verified_descriptor(
        b"leaf",
        "image/webp; charset=binary",
        "webp",
    ))
    .unwrap();
    assert_eq!(
        RadrootsAuthoredPostImage::new(
            parameterized,
            RadrootsPostImageDimensions::new(1, 1).unwrap(),
            "Leaf",
        )
        .unwrap_err()
        .code(),
        "imeta_mime_invalid"
    );
    assert_eq!(
        RadrootsPostImageDimensions::new(0, 1).unwrap_err().code(),
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
    let empty =
        RadrootsAuthoredImage::try_from(verified_descriptor(b"", "image/webp", "webp")).unwrap();
    assert_eq!(
        RadrootsAuthoredPostImage::new(
            empty,
            RadrootsPostImageDimensions::new(1, 1).unwrap(),
            "Empty image",
        )
        .unwrap_err(),
        RadrootsAuthoredPostError::ImageSizeInvalid
    );

    let blank_alt =
        RadrootsAuthoredImage::try_from(verified_descriptor(b"leaf", "image/webp", "webp"))
            .unwrap();
    assert_eq!(
        RadrootsAuthoredPostImage::new(
            blank_alt,
            RadrootsPostImageDimensions::new(1, 1).unwrap(),
            " \t",
        )
        .unwrap_err(),
        RadrootsAuthoredPostError::ImageAltInvalid
    );

    let maximum_alt = "a".repeat(RADROOTS_POST_ALT_MAX_BYTES);
    let maximum =
        RadrootsAuthoredImage::try_from(verified_descriptor(b"maximum", "image/webp", "webp"))
            .unwrap();
    assert!(
        RadrootsAuthoredPostImage::new(
            maximum,
            RadrootsPostImageDimensions::new(1, 1).unwrap(),
            maximum_alt,
        )
        .is_ok()
    );

    let oversized_alt = "a".repeat(RADROOTS_POST_ALT_MAX_BYTES + 1);
    let oversized =
        RadrootsAuthoredImage::try_from(verified_descriptor(b"oversized", "image/webp", "webp"))
            .unwrap();
    assert_eq!(
        RadrootsAuthoredPostImage::new(
            oversized,
            RadrootsPostImageDimensions::new(1, 1).unwrap(),
            oversized_alt,
        )
        .unwrap_err(),
        RadrootsAuthoredPostError::ImageAltTooLarge {
            max: RADROOTS_POST_ALT_MAX_BYTES,
            actual: RADROOTS_POST_ALT_MAX_BYTES + 1,
        }
    );
}

fn authored_image(bytes: &[u8], media_type: &str, extension: &str) -> RadrootsAuthoredPostImage {
    RadrootsAuthoredPostImage::new(
        RadrootsAuthoredImage::try_from(verified_descriptor(bytes, media_type, extension)).unwrap(),
        RadrootsPostImageDimensions::new(1200, 900).unwrap(),
        "Harvest",
    )
    .unwrap()
}

fn verified_descriptor(
    bytes: &[u8],
    media_type: &str,
    extension: &str,
) -> RadrootsBlossomByteVerifiedDescriptor {
    let hash = RadrootsBlossomSha256::digest(bytes);
    let media_type = RadrootsBlossomMediaType::parse(media_type).unwrap();
    RadrootsBlossomBlobDescriptor::new(
        RadrootsBlossomBlobUrl::parse(&format!("https://media.example/{hash}.{extension}"))
            .unwrap(),
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

fn fallback_url(
    bytes: &[u8],
    host: &str,
    extension: &str,
) -> radroots_blossom::RadrootsBlossomApprovedBlobUrl {
    let hash = RadrootsBlossomSha256::digest(bytes);
    RadrootsBlossomBlobUrl::parse(&format!("https://{host}/{hash}.{extension}"))
        .unwrap()
        .approve()
        .unwrap()
}
