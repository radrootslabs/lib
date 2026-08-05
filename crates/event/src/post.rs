//! Authored posts and related public social-content event models.

#[cfg(not(feature = "std"))]
use alloc::{format, string::String, vec::Vec};
use core::fmt;

use radroots_blossom::url::ApprovedBlobUrl;
use url_nostd::Url;

use crate::media::AuthoredImage;
use crate::tag::name::TAG_IMETA;

pub const RADROOTS_POST_CONTENT_MAX_BYTES: usize = crate::wire::v1::DEFAULT_CONTENT_MAX_BYTES;
pub const RADROOTS_POST_IMETA_MAX_COUNT: usize = 64;
pub const RADROOTS_POST_EVENT_WIRE_MAX_BYTES: usize = crate::wire::v1::DEFAULT_RAW_JSON_MAX_BYTES;
pub const RADROOTS_POST_TAG_ELEMENT_MAX_BYTES: usize =
    crate::wire::v1::DEFAULT_TAG_ELEMENT_MAX_BYTES;
pub const RADROOTS_POST_TAG_TOTAL_MAX_BYTES: usize = crate::wire::v1::DEFAULT_TAG_TOTAL_MAX_BYTES;
pub const RADROOTS_POST_ALT_MAX_BYTES: usize = RADROOTS_POST_TAG_ELEMENT_MAX_BYTES - "alt ".len();
pub const RADROOTS_ASK_MARKER_TAG_KEY: &str = "t";
pub const RADROOTS_ASK_MARKER_TAG_VALUE: &str = "radroots-ask";

const RADROOTS_ASK_MARKER_TAG_BYTES: usize =
    RADROOTS_ASK_MARKER_TAG_KEY.len() + RADROOTS_ASK_MARKER_TAG_VALUE.len();
const RADROOTS_POST_SIGNED_EVENT_FIXED_MAX_BYTES: usize = "{\"id\":\"".len()
    + 64
    + "\",\"pubkey\":\"".len()
    + 64
    + "\",\"created_at\":".len()
    + 20
    + ",\"kind\":1,\"tags\":".len()
    + ",\"content\":".len()
    + ",\"sig\":\"".len()
    + 128
    + "\"}".len();

#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthoredPostError {
    ContentMissing,
    ContentTooLarge { max: usize, actual: usize },
    ImageMissing,
    ImageCountExceeded { max: usize, actual: usize },
    ImageUrlOccurrenceCount { expected: usize, actual: usize },
    DuplicateImageUrl,
    ImageMediaTypeInvalid,
    ImageSizeInvalid,
    ImageDimensionsInvalid,
    ImageAltInvalid,
    ImageAltTooLarge { max: usize, actual: usize },
    ImageFallbackHashMismatch,
    TagElementTooLarge { max: usize, actual: usize },
    TagBytesExceeded { max: usize, actual: usize },
    EventWireTooLarge { max: usize, actual: usize },
    ImageUrlOverlap,
}

impl AuthoredPostError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::ContentMissing => "post_content_missing",
            Self::ContentTooLarge { .. } => "post_content_too_large",
            Self::ImageMissing => "photo_imeta_missing",
            Self::ImageCountExceeded { .. } => "imeta_count_exceeded",
            Self::ImageUrlOccurrenceCount { .. } => "imeta_url_occurrence_count",
            Self::ImageUrlOverlap => "imeta_url_overlap",
            Self::DuplicateImageUrl => "duplicate_imeta_url",
            Self::ImageMediaTypeInvalid => "imeta_mime_invalid",
            Self::ImageSizeInvalid => "imeta_size_invalid",
            Self::ImageDimensionsInvalid => "imeta_dimensions_invalid",
            Self::ImageAltInvalid => "imeta_alt_invalid",
            Self::ImageAltTooLarge { .. } => "imeta_alt_too_large",
            Self::ImageFallbackHashMismatch => "imeta_fallback_hash_mismatch",
            Self::TagElementTooLarge { .. } => "post_tag_element_too_large",
            Self::TagBytesExceeded { .. } => "post_tag_bytes_exceeded",
            Self::EventWireTooLarge { .. } => "post_event_wire_too_large",
        }
    }
}

impl fmt::Display for AuthoredPostError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ContentMissing => {
                formatter.write_str("authored post content must be non-whitespace")
            }
            Self::ContentTooLarge { max, actual } => {
                write!(
                    formatter,
                    "authored post content is {actual} bytes; max is {max}"
                )
            }
            Self::ImageMissing => {
                formatter.write_str("authored PhotoUpdate requires at least one image")
            }
            Self::ImageCountExceeded { max, actual } => {
                write!(formatter, "authored post has {actual} images; max is {max}")
            }
            Self::ImageUrlOccurrenceCount { expected, actual } => write!(
                formatter,
                "authored image URL occurrence count is {actual}; expected {expected}"
            ),
            Self::ImageUrlOverlap => formatter.write_str(
                "authored image URL occurrences must not overlap another image URL occurrence",
            ),
            Self::DuplicateImageUrl => {
                formatter.write_str("authored post image URLs must be unique")
            }
            Self::ImageMediaTypeInvalid => formatter.write_str(
                "authored post image media type must be parameter-free canonical lowercase image/*",
            ),
            Self::ImageSizeInvalid => {
                formatter.write_str("authored post image size must be nonzero")
            }
            Self::ImageDimensionsInvalid => {
                formatter.write_str("authored post image dimensions must be nonzero u32 values")
            }
            Self::ImageAltInvalid => {
                formatter.write_str("authored post image alt text must be non-whitespace")
            }
            Self::ImageAltTooLarge { max, actual } => {
                write!(
                    formatter,
                    "authored post image alt text is {actual} bytes; max is {max}"
                )
            }
            Self::ImageFallbackHashMismatch => formatter.write_str(
                "authored post image fallback URL must contain the primary image digest",
            ),
            Self::TagElementTooLarge { max, actual } => {
                write!(
                    formatter,
                    "authored post tag element is {actual} bytes; max is {max}"
                )
            }
            Self::TagBytesExceeded { max, actual } => {
                write!(
                    formatter,
                    "authored post tag bytes are {actual}; max is {max}"
                )
            }
            Self::EventWireTooLarge { max, actual } => write!(
                formatter,
                "authored post canonical signed event is at most {actual} bytes; max is {max}"
            ),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for AuthoredPostError {}

/// Nonzero pixel dimensions for one strict authored NIP-92 image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PostImageDimensions {
    width: u32,
    height: u32,
}

impl PostImageDimensions {
    pub const fn new(width: u32, height: u32) -> Result<Self, AuthoredPostError> {
        if width == 0 || height == 0 {
            return Err(AuthoredPostError::ImageDimensionsInvalid);
        }
        Ok(Self { width, height })
    }

    pub const fn width(self) -> u32 {
        self.width
    }

    pub const fn height(self) -> u32 {
        self.height
    }
}

/// Strict authored NIP-92 image metadata.
///
/// The primary image can only enter through a byte-verified Blossom
/// descriptor. This proves descriptor/byte agreement, not upload completion or
/// network availability. Publication runtimes must separately require a
/// successful BUD-02 upload before signing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthoredPostImage {
    image: AuthoredImage,
    dimensions: PostImageDimensions,
    alt: String,
    fallbacks: Vec<ApprovedBlobUrl>,
    imeta_tag: Vec<String>,
}

impl AuthoredPostImage {
    pub fn new(
        image: AuthoredImage,
        dimensions: PostImageDimensions,
        alt: impl Into<String>,
    ) -> Result<Self, AuthoredPostError> {
        let descriptor = image.descriptor();
        if descriptor.size() == 0 {
            return Err(AuthoredPostError::ImageSizeInvalid);
        }
        if !post_image_media_type_is_valid(descriptor.media_type().as_str()) {
            return Err(AuthoredPostError::ImageMediaTypeInvalid);
        }
        let alt = alt.into();
        if alt.trim().is_empty() {
            return Err(AuthoredPostError::ImageAltInvalid);
        }
        if alt.len() > RADROOTS_POST_ALT_MAX_BYTES {
            return Err(AuthoredPostError::ImageAltTooLarge {
                max: RADROOTS_POST_ALT_MAX_BYTES,
                actual: alt.len(),
            });
        }
        let fallbacks = Vec::new();
        let imeta_tag = derive_imeta_tag(&image, dimensions, &alt, &fallbacks)?;
        Ok(Self {
            image,
            dimensions,
            alt,
            fallbacks,
            imeta_tag,
        })
    }

    pub fn try_with_fallback(
        mut self,
        fallback: ApprovedBlobUrl,
    ) -> Result<Self, AuthoredPostError> {
        if fallback.as_blob_url().hash_path().hash() != self.image.descriptor().sha256() {
            return Err(AuthoredPostError::ImageFallbackHashMismatch);
        }
        let fallback_element = format!("fallback {fallback}");
        validate_tag_element(&fallback_element)?;
        validate_tag_bytes(
            imeta_tag_bytes(&self.imeta_tag).saturating_add(fallback_element.len()),
        )?;
        self.fallbacks.push(fallback);
        self.imeta_tag.push(fallback_element);
        Ok(self)
    }

    pub fn image(&self) -> &AuthoredImage {
        &self.image
    }

    pub const fn dimensions(&self) -> PostImageDimensions {
        self.dimensions
    }

    pub fn alt(&self) -> &str {
        &self.alt
    }

    pub fn fallbacks(&self) -> &[ApprovedBlobUrl] {
        &self.fallbacks
    }

    /// Returns the exact validated NIP-92 `imeta` tag emitted for this image.
    pub fn imeta_tag(&self) -> &[String] {
        &self.imeta_tag
    }

    pub fn url(&self) -> &str {
        self.image.descriptor().url().as_str()
    }
}

/// Strict authored root kind-1 Update without Ask or media tags.
///
/// ```compile_fail
/// let _: radroots_event::post::AuthoredUpdate =
///     serde_json::from_str(r#"{"content":"harvest"}"#).unwrap();
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthoredUpdate {
    content: String,
}

impl AuthoredUpdate {
    pub fn new(content: impl Into<String>) -> Result<Self, AuthoredPostError> {
        let content = content.into();
        validate_authored_root_content(&content)?;
        validate_post_event_wire_size(&content, false, &[])?;
        Ok(Self { content })
    }

    pub fn content(&self) -> &str {
        &self.content
    }
}

/// Strict authored root kind-1 PhotoUpdate with deterministic NIP-92 tags.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthoredPhotoUpdate {
    content: String,
    images: Vec<AuthoredPostImage>,
}

impl AuthoredPhotoUpdate {
    pub fn new(
        content: impl Into<String>,
        images: Vec<AuthoredPostImage>,
    ) -> Result<Self, AuthoredPostError> {
        let content = content.into();
        validate_content_size(&content)?;
        validate_authored_images(&content, &images, 0)?;
        validate_post_event_wire_size(&content, false, &images)?;
        Ok(Self { content, images })
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    pub fn images(&self) -> &[AuthoredPostImage] {
        &self.images
    }
}

/// Strict authored root kind-1 Ask with its exact product marker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthoredAsk {
    content: String,
    images: Vec<AuthoredPostImage>,
}

impl AuthoredAsk {
    pub fn new(
        content: impl Into<String>,
        images: Vec<AuthoredPostImage>,
    ) -> Result<Self, AuthoredPostError> {
        let content = content.into();
        validate_authored_root_content(&content)?;
        if images.len() > RADROOTS_POST_IMETA_MAX_COUNT {
            return Err(AuthoredPostError::ImageCountExceeded {
                max: RADROOTS_POST_IMETA_MAX_COUNT,
                actual: images.len(),
            });
        }
        if !images.is_empty() {
            validate_authored_images(&content, &images, RADROOTS_ASK_MARKER_TAG_BYTES)?;
        }
        validate_post_event_wire_size(&content, true, &images)?;
        Ok(Self { content, images })
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    pub fn images(&self) -> &[AuthoredPostImage] {
        &self.images
    }
}

fn validate_authored_root_content(content: &str) -> Result<(), AuthoredPostError> {
    validate_content_size(content)?;
    if content.trim().is_empty() {
        return Err(AuthoredPostError::ContentMissing);
    }
    Ok(())
}

fn validate_content_size(content: &str) -> Result<(), AuthoredPostError> {
    if content.len() > RADROOTS_POST_CONTENT_MAX_BYTES {
        return Err(AuthoredPostError::ContentTooLarge {
            max: RADROOTS_POST_CONTENT_MAX_BYTES,
            actual: content.len(),
        });
    }
    Ok(())
}

fn validate_authored_images(
    content: &str,
    images: &[AuthoredPostImage],
    initial_tag_bytes: usize,
) -> Result<(), AuthoredPostError> {
    if images.is_empty() {
        return Err(AuthoredPostError::ImageMissing);
    }
    if images.len() > RADROOTS_POST_IMETA_MAX_COUNT {
        return Err(AuthoredPostError::ImageCountExceeded {
            max: RADROOTS_POST_IMETA_MAX_COUNT,
            actual: images.len(),
        });
    }
    let mut occurrences = Vec::with_capacity(images.len());
    for (index, image) in images.iter().enumerate() {
        if images[..index]
            .iter()
            .any(|candidate| candidate.url() == image.url())
        {
            return Err(AuthoredPostError::DuplicateImageUrl);
        }
        let mut matches = content.match_indices(image.url());
        let first = matches.next();
        let actual = usize::from(first.is_some()).saturating_add(matches.count());
        if actual != 1 {
            return Err(AuthoredPostError::ImageUrlOccurrenceCount {
                expected: 1,
                actual,
            });
        }
        let (start, matched) = first.expect("exactly one occurrence was established");
        occurrences.push((start, start.saturating_add(matched.len())));
    }
    occurrences.sort_unstable();
    if occurrences.windows(2).any(|pair| pair[0].1 > pair[1].0) {
        return Err(AuthoredPostError::ImageUrlOverlap);
    }
    let total_tag_bytes = images.iter().fold(initial_tag_bytes, |total, image| {
        total.saturating_add(imeta_tag_bytes(image.imeta_tag()))
    });
    validate_tag_bytes(total_tag_bytes)?;
    Ok(())
}

fn derive_imeta_tag(
    image: &AuthoredImage,
    dimensions: PostImageDimensions,
    alt: &str,
    fallbacks: &[ApprovedBlobUrl],
) -> Result<Vec<String>, AuthoredPostError> {
    let descriptor = image.descriptor();
    let mut tag = Vec::with_capacity(7 + fallbacks.len());
    tag.push(TAG_IMETA.into());
    tag.push(format!("url {}", descriptor.url()));
    tag.push(format!("x {}", descriptor.sha256()));
    tag.push(format!("m {}", descriptor.media_type()));
    tag.push(format!(
        "dim {}x{}",
        dimensions.width(),
        dimensions.height()
    ));
    tag.push(format!("size {}", descriptor.size()));
    tag.push(format!("alt {alt}"));
    tag.extend(
        fallbacks
            .iter()
            .map(|fallback| format!("fallback {fallback}")),
    );
    for element in &tag {
        validate_tag_element(element)?;
    }
    validate_tag_bytes(imeta_tag_bytes(&tag))?;
    Ok(tag)
}

fn validate_tag_element(element: &str) -> Result<(), AuthoredPostError> {
    if element.len() > RADROOTS_POST_TAG_ELEMENT_MAX_BYTES {
        return Err(AuthoredPostError::TagElementTooLarge {
            max: RADROOTS_POST_TAG_ELEMENT_MAX_BYTES,
            actual: element.len(),
        });
    }
    Ok(())
}

fn validate_tag_bytes(actual: usize) -> Result<(), AuthoredPostError> {
    if actual > RADROOTS_POST_TAG_TOTAL_MAX_BYTES {
        return Err(AuthoredPostError::TagBytesExceeded {
            max: RADROOTS_POST_TAG_TOTAL_MAX_BYTES,
            actual,
        });
    }
    Ok(())
}

fn imeta_tag_bytes(tag: &[String]) -> usize {
    tag.iter()
        .fold(0, |total, element| total.saturating_add(element.len()))
}

fn validate_post_event_wire_size(
    content: &str,
    ask_marker: bool,
    images: &[AuthoredPostImage],
) -> Result<(), AuthoredPostError> {
    let mut tags_json_bytes = 2usize;
    let mut tag_count = 0usize;
    if ask_marker {
        add_tag_json_bytes(
            &mut tags_json_bytes,
            &mut tag_count,
            [RADROOTS_ASK_MARKER_TAG_KEY, RADROOTS_ASK_MARKER_TAG_VALUE],
        );
    }
    for image in images {
        add_tag_json_bytes(
            &mut tags_json_bytes,
            &mut tag_count,
            image.imeta_tag().iter().map(String::as_str),
        );
    }
    let actual = RADROOTS_POST_SIGNED_EVENT_FIXED_MAX_BYTES
        .saturating_add(tags_json_bytes)
        .saturating_add(canonical_json_string_bytes(content));
    if actual > RADROOTS_POST_EVENT_WIRE_MAX_BYTES {
        return Err(AuthoredPostError::EventWireTooLarge {
            max: RADROOTS_POST_EVENT_WIRE_MAX_BYTES,
            actual,
        });
    }
    Ok(())
}

fn add_tag_json_bytes<'a>(
    total: &mut usize,
    tag_count: &mut usize,
    elements: impl IntoIterator<Item = &'a str>,
) {
    if *tag_count > 0 {
        *total = total.saturating_add(1);
    }
    *total = total.saturating_add(2);
    let mut element_count = 0usize;
    for element in elements {
        if element_count > 0 {
            *total = total.saturating_add(1);
        }
        *total = total.saturating_add(canonical_json_string_bytes(element));
        element_count = element_count.saturating_add(1);
    }
    *tag_count = tag_count.saturating_add(1);
}

fn canonical_json_string_bytes(value: &str) -> usize {
    value.chars().fold(2usize, |total, character| {
        total.saturating_add(match character {
            '"' | '\\' | '\u{0008}' | '\t' | '\n' | '\u{000c}' | '\r' => 2,
            '\u{0000}'..='\u{001f}' => 6,
            _ => character.len_utf8(),
        })
    })
}

pub fn post_image_media_type_is_valid(value: &str) -> bool {
    let Some(subtype) = value.strip_prefix("image/") else {
        return false;
    };
    let mut bytes = subtype.bytes();
    bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        && bytes.all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'+' | b'-')
        })
}

/// Returns whether an inbound media reference is a structural HTTP(S) URL.
///
/// This is intentionally broader than strict authored Blossom policy and does
/// not make a reachability, byte-verification, or upload claim.
pub fn post_media_http_url_is_valid(value: &str) -> bool {
    if value.is_empty()
        || value
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
    {
        return false;
    }
    let Some((scheme, remainder)) = value.split_once("://") else {
        return false;
    };
    if !(scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https")) {
        return false;
    }
    let authority_end = remainder.find(['/', '?', '#']).unwrap_or(remainder.len());
    let authority = &remainder[..authority_end];
    if authority.is_empty() {
        return false;
    }
    let raw_path = remainder[authority_end..]
        .split(['?', '#'])
        .next()
        .unwrap_or_default();
    let Ok(parsed) = Url::parse(value) else {
        return false;
    };
    matches!(parsed.scheme(), "http" | "https")
        && parsed.host_str().is_some_and(|host| !host.is_empty())
        && parsed.username().is_empty()
        && parsed.password().is_none()
        && !authority.contains('@')
        && raw_path.starts_with('/')
        && !raw_path.is_empty()
}

#[cfg(all(test, feature = "std", feature = "serde"))]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use radroots_blossom::{BlobDescriptor, BlobUrl, ByteVerifiedDescriptor, MediaType, Sha256};

    #[test]
    fn post_image_media_type_uses_exact_product_grammar() {
        for valid in [
            "image/png",
            "image/1",
            "image/a0",
            "image/vnd.radroots+png",
            "image/x-radroots.photo",
        ] {
            assert!(post_image_media_type_is_valid(valid), "{valid}");
        }

        for invalid in [
            "image/",
            "image/PNG",
            "IMAGE/png",
            "image/_png",
            "image/p_ng",
            "image/p!ng",
            "image/p#ng",
            "image/p$ng",
            "image/p&ng",
            "image/p^ng",
            "image/p%ng",
            "image/p*ng",
            "image/p'ng",
            "image/png;quality=90",
            "text/png",
        ] {
            assert!(!post_image_media_type_is_valid(invalid), "{invalid}");
        }
    }

    #[test]
    fn authored_post_models_preserve_validated_content_and_image_metadata() {
        let image = authored_image(b"photo", "image/webp", "webp", "media.example");
        let dimensions = PostImageDimensions::new(640, 480).unwrap();
        let primary_url = image.descriptor().url().as_str().to_owned();
        let post_image = AuthoredPostImage::new(image, dimensions, "market basket").unwrap();
        let fallback = BlobUrl::parse(&format!(
            "https://fallback.example/{}.webp",
            post_image.image().descriptor().sha256()
        ))
        .unwrap()
        .approve()
        .unwrap();
        let post_image = post_image.try_with_fallback(fallback.clone()).unwrap();

        assert_eq!(dimensions.width(), 640);
        assert_eq!(dimensions.height(), 480);
        assert_eq!(post_image.dimensions(), dimensions);
        assert_eq!(post_image.alt(), "market basket");
        assert_eq!(post_image.url(), primary_url);
        assert_eq!(post_image.fallbacks(), &[fallback]);
        assert_eq!(post_image.imeta_tag()[0], TAG_IMETA);
        assert!(
            post_image
                .imeta_tag()
                .iter()
                .any(|value| value == "dim 640x480")
        );

        let update = AuthoredUpdate::new("harvest update").unwrap();
        assert_eq!(update.content(), "harvest update");

        let content = format!("available today {primary_url}");
        let photo = AuthoredPhotoUpdate::new(content.clone(), vec![post_image.clone()]).unwrap();
        assert_eq!(photo.content(), content);
        assert_eq!(photo.images(), std::slice::from_ref(&post_image));

        let ask = AuthoredAsk::new(content.clone(), vec![post_image]).unwrap();
        assert_eq!(ask.content(), content);
        assert_eq!(ask.images().len(), 1);
        assert!(AuthoredAsk::new("where can I buy this?", Vec::new()).is_ok());
    }

    #[test]
    fn authored_post_rejects_invalid_content_and_image_shapes() {
        assert_eq!(
            PostImageDimensions::new(0, 1),
            Err(AuthoredPostError::ImageDimensionsInvalid)
        );
        assert_eq!(
            PostImageDimensions::new(1, 0),
            Err(AuthoredPostError::ImageDimensionsInvalid)
        );
        assert_eq!(
            AuthoredUpdate::new(" \n").unwrap_err(),
            AuthoredPostError::ContentMissing
        );
        let oversized = "x".repeat(RADROOTS_POST_CONTENT_MAX_BYTES + 1);
        assert_eq!(
            AuthoredUpdate::new(oversized).unwrap_err(),
            AuthoredPostError::ContentTooLarge {
                max: RADROOTS_POST_CONTENT_MAX_BYTES,
                actual: RADROOTS_POST_CONTENT_MAX_BYTES + 1,
            }
        );
        assert_eq!(
            AuthoredPhotoUpdate::new("photo", Vec::new()).unwrap_err(),
            AuthoredPostError::ImageMissing
        );

        let image = AuthoredPostImage::new(
            authored_image(b"photo", "image/png", "png", "media.example"),
            PostImageDimensions::new(1, 1).unwrap(),
            "photo",
        )
        .unwrap();
        assert_eq!(
            AuthoredPhotoUpdate::new("missing URL", vec![image.clone()]).unwrap_err(),
            AuthoredPostError::ImageUrlOccurrenceCount {
                expected: 1,
                actual: 0,
            }
        );
        let content = image.url().to_owned();
        assert_eq!(
            AuthoredPhotoUpdate::new(content, vec![image.clone(), image]).unwrap_err(),
            AuthoredPostError::DuplicateImageUrl
        );
    }

    #[test]
    fn authored_post_requires_one_non_overlapping_utf8_url_occurrence() {
        let image = AuthoredPostImage::new(
            authored_image(b"photo", "image/png", "png", "media.example"),
            PostImageDimensions::new(1, 1).unwrap(),
            "photo",
        )
        .unwrap();
        let repeated = format!("{} 🍓 {}", image.url(), image.url());
        assert_eq!(
            AuthoredPhotoUpdate::new(repeated, vec![image.clone()]).unwrap_err(),
            AuthoredPostError::ImageUrlOccurrenceCount {
                expected: 1,
                actual: 2,
            }
        );
        assert!(
            AuthoredPhotoUpdate::new(format!("苗 {} 🍓", image.url()), vec![image]).is_ok(),
            "UTF-8 surrounding text must not disturb byte-boundary occurrence counting"
        );

        let bytes = b"shared-prefix";
        let hash = Sha256::digest(bytes);
        let short_url = format!("https://media.example/{hash}.webp");
        let long_url = format!("{short_url}2");
        let short = AuthoredPostImage::new(
            authored_image_at_url(bytes, "image/webp", &short_url),
            PostImageDimensions::new(1, 1).unwrap(),
            "short",
        )
        .unwrap();
        let long = AuthoredPostImage::new(
            authored_image_at_url(bytes, "image/webp", &long_url),
            PostImageDimensions::new(1, 1).unwrap(),
            "long",
        )
        .unwrap();
        assert_eq!(
            AuthoredPhotoUpdate::new(long_url, vec![short, long]).unwrap_err(),
            AuthoredPostError::ImageUrlOverlap
        );
    }

    #[test]
    fn authored_image_rejects_invalid_descriptor_metadata_and_bounds() {
        let dimensions = PostImageDimensions::new(1, 1).unwrap();
        let empty = authored_image(b"", "image/png", "png", "media.example");
        assert_eq!(
            AuthoredPostImage::new(empty, dimensions, "empty").unwrap_err(),
            AuthoredPostError::ImageSizeInvalid
        );
        let noncanonical_media = authored_image(b"x", "image/p_ng", "png", "media.example");
        assert_eq!(
            AuthoredPostImage::new(noncanonical_media, dimensions, "photo").unwrap_err(),
            AuthoredPostError::ImageMediaTypeInvalid
        );
        let valid = authored_image(b"x", "image/png", "png", "media.example");
        assert_eq!(
            AuthoredPostImage::new(valid.clone(), dimensions, " \t").unwrap_err(),
            AuthoredPostError::ImageAltInvalid
        );
        let long_alt = "a".repeat(RADROOTS_POST_ALT_MAX_BYTES + 1);
        assert_eq!(
            AuthoredPostImage::new(valid, dimensions, long_alt).unwrap_err(),
            AuthoredPostError::ImageAltTooLarge {
                max: RADROOTS_POST_ALT_MAX_BYTES,
                actual: RADROOTS_POST_ALT_MAX_BYTES + 1,
            }
        );

        let primary = AuthoredPostImage::new(
            authored_image(b"primary", "image/png", "png", "media.example"),
            dimensions,
            "primary",
        )
        .unwrap();
        let other_hash = Sha256::digest(b"other");
        let fallback = BlobUrl::parse(&format!("https://fallback.example/{other_hash}.png"))
            .unwrap()
            .approve()
            .unwrap();
        assert_eq!(
            primary.try_with_fallback(fallback).unwrap_err(),
            AuthoredPostError::ImageFallbackHashMismatch
        );
    }

    #[test]
    fn authored_post_enforces_collection_and_wire_accounting_bounds() {
        let image = AuthoredPostImage::new(
            authored_image(b"same", "image/png", "png", "media.example"),
            PostImageDimensions::new(1, 1).unwrap(),
            "a".repeat(RADROOTS_POST_ALT_MAX_BYTES),
        )
        .unwrap();
        let too_many = vec![image.clone(); RADROOTS_POST_IMETA_MAX_COUNT + 1];
        assert_eq!(
            AuthoredPhotoUpdate::new("photo", too_many.clone()).unwrap_err(),
            AuthoredPostError::ImageCountExceeded {
                max: RADROOTS_POST_IMETA_MAX_COUNT,
                actual: RADROOTS_POST_IMETA_MAX_COUNT + 1,
            }
        );
        assert_eq!(
            AuthoredAsk::new("ask", too_many).unwrap_err(),
            AuthoredPostError::ImageCountExceeded {
                max: RADROOTS_POST_IMETA_MAX_COUNT,
                actual: RADROOTS_POST_IMETA_MAX_COUNT + 1,
            }
        );

        let unique_images = (0..RADROOTS_POST_IMETA_MAX_COUNT)
            .map(|index| {
                AuthoredPostImage::new(
                    authored_image(
                        format!("image-{index}").as_bytes(),
                        "image/png",
                        "png",
                        "media.example",
                    ),
                    PostImageDimensions::new(1, 1).unwrap(),
                    "a".repeat(RADROOTS_POST_ALT_MAX_BYTES),
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        let content = unique_images
            .iter()
            .map(|image| image.url())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(matches!(
            AuthoredPhotoUpdate::new(content, unique_images),
            Err(AuthoredPostError::TagBytesExceeded { .. })
        ));

        assert!(matches!(
            validate_tag_element(&"x".repeat(RADROOTS_POST_TAG_ELEMENT_MAX_BYTES + 1)),
            Err(AuthoredPostError::TagElementTooLarge { .. })
        ));
        assert!(matches!(
            validate_post_event_wire_size(
                &"\u{001f}".repeat(RADROOTS_POST_CONTENT_MAX_BYTES),
                true,
                &[]
            ),
            Err(AuthoredPostError::EventWireTooLarge { .. })
        ));
    }

    #[test]
    fn authored_post_errors_expose_stable_codes_and_messages() {
        let errors = [
            AuthoredPostError::ContentMissing,
            AuthoredPostError::ContentTooLarge { max: 1, actual: 2 },
            AuthoredPostError::ImageMissing,
            AuthoredPostError::ImageCountExceeded { max: 1, actual: 2 },
            AuthoredPostError::ImageUrlOccurrenceCount {
                expected: 1,
                actual: 0,
            },
            AuthoredPostError::ImageUrlOverlap,
            AuthoredPostError::DuplicateImageUrl,
            AuthoredPostError::ImageMediaTypeInvalid,
            AuthoredPostError::ImageSizeInvalid,
            AuthoredPostError::ImageDimensionsInvalid,
            AuthoredPostError::ImageAltInvalid,
            AuthoredPostError::ImageAltTooLarge { max: 1, actual: 2 },
            AuthoredPostError::ImageFallbackHashMismatch,
            AuthoredPostError::TagElementTooLarge { max: 1, actual: 2 },
            AuthoredPostError::TagBytesExceeded { max: 1, actual: 2 },
            AuthoredPostError::EventWireTooLarge { max: 1, actual: 2 },
        ];
        for error in errors {
            assert!(!error.code().is_empty());
            assert!(!error.to_string().is_empty());
        }
    }

    #[test]
    fn inbound_media_url_validation_rejects_ambiguous_authorities_and_paths() {
        for valid in [
            "https://media.example/path",
            "HTTP://localhost/path?size=large",
            "https://[::1]/hash#preview",
        ] {
            assert!(post_media_http_url_is_valid(valid), "{valid}");
        }
        for invalid in [
            "",
            " https://media.example/path",
            "\nhttps://media.example/path",
            "media.example/path",
            "ftp://media.example/path",
            "https://user@media.example/path",
            "https://user:password@media.example/path",
            "https://media.example",
            "https:///path",
            "https://[invalid]/path",
            "not a URL://media.example/path",
        ] {
            assert!(!post_media_http_url_is_valid(invalid), "{invalid}");
        }
    }

    #[test]
    fn canonical_json_size_accounts_for_every_escape_class() {
        assert_eq!(canonical_json_string_bytes("plain"), 7);
        for escaped in ['"', '\\', '\u{0008}', '\t', '\n', '\u{000c}', '\r'] {
            assert_eq!(canonical_json_string_bytes(&escaped.to_string()), 4);
        }
        assert_eq!(canonical_json_string_bytes("\u{0001}"), 8);
        assert_eq!(canonical_json_string_bytes("é"), 4);
    }

    fn authored_image(
        bytes: &[u8],
        media_type: &str,
        extension: &str,
        host: &str,
    ) -> AuthoredImage {
        AuthoredImage::try_from(verified_descriptor(bytes, media_type, extension, host)).unwrap()
    }

    fn authored_image_at_url(bytes: &[u8], media_type: &str, url: &str) -> AuthoredImage {
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
        AuthoredImage::try_from(descriptor).unwrap()
    }

    fn verified_descriptor(
        bytes: &[u8],
        media_type: &str,
        extension: &str,
        host: &str,
    ) -> ByteVerifiedDescriptor {
        let hash = Sha256::digest(bytes);
        let media_type = MediaType::parse(media_type).unwrap();
        BlobDescriptor::new(
            BlobUrl::parse(&format!("https://{host}/{hash}.{extension}")).unwrap(),
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
}
#[path = "article.rs"]
pub mod article;
#[path = "comment.rs"]
pub mod comment;
#[path = "deletion.rs"]
pub mod deletion;
#[path = "document.rs"]
pub mod document;
#[path = "reaction.rs"]
pub mod reaction;
#[path = "reply.rs"]
pub mod reply;
#[path = "report.rs"]
pub mod report;
#[path = "repost.rs"]
pub mod repost;
