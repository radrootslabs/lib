#[cfg(not(feature = "std"))]
use alloc::{format, string::String, vec::Vec};
use core::fmt;

use radroots_blossom::url::RadrootsBlossomApprovedBlobUrl;
use url_nostd::Url;

use crate::media::RadrootsAuthoredImage;
use crate::social::{
    RadrootsSocialFarmAnchor, RadrootsSocialLocation, RadrootsSocialMediaMetadata,
    RadrootsSocialTarget,
};
use crate::tags::TAG_IMETA;

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

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[derive(Clone, Debug)]
/// Compatibility projection for the legacy social post decoder.
///
/// This mutable model is not an authored event boundary. New publication code
/// must use `RadrootsAuthoredUpdate`, `RadrootsAuthoredPhotoUpdate`, or
/// `RadrootsAuthoredAsk` so raw `imeta` cannot bypass the strict profile.
pub struct RadrootsPost {
    pub content: String,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub farm: Option<RadrootsSocialFarmAnchor>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub address_refs: Option<Vec<RadrootsSocialTarget>>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub location: Option<RadrootsSocialLocation>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub topics: Option<Vec<String>>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub quote_refs: Option<Vec<RadrootsSocialTarget>>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub media: Option<Vec<RadrootsSocialMediaMetadata>>,
}

#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsAuthoredPostError {
    ContentMissing,
    ContentTooLarge { max: usize, actual: usize },
    ImageMissing,
    ImageCountExceeded { max: usize, actual: usize },
    ImageUrlMissingFromContent,
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
}

impl RadrootsAuthoredPostError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::ContentMissing => "post_content_missing",
            Self::ContentTooLarge { .. } => "post_content_too_large",
            Self::ImageMissing => "photo_imeta_missing",
            Self::ImageCountExceeded { .. } => "imeta_count_exceeded",
            Self::ImageUrlMissingFromContent => "imeta_url_missing_from_content",
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

impl fmt::Display for RadrootsAuthoredPostError {
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
            Self::ImageUrlMissingFromContent => {
                formatter.write_str("each authored image URL must occur exactly in post content")
            }
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
impl std::error::Error for RadrootsAuthoredPostError {}

/// Nonzero pixel dimensions for one strict authored NIP-92 image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RadrootsPostImageDimensions {
    width: u32,
    height: u32,
}

impl RadrootsPostImageDimensions {
    pub const fn new(width: u32, height: u32) -> Result<Self, RadrootsAuthoredPostError> {
        if width == 0 || height == 0 {
            return Err(RadrootsAuthoredPostError::ImageDimensionsInvalid);
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
pub struct RadrootsAuthoredPostImage {
    image: RadrootsAuthoredImage,
    dimensions: RadrootsPostImageDimensions,
    alt: String,
    fallbacks: Vec<RadrootsBlossomApprovedBlobUrl>,
    imeta_tag: Vec<String>,
}

impl RadrootsAuthoredPostImage {
    pub fn new(
        image: RadrootsAuthoredImage,
        dimensions: RadrootsPostImageDimensions,
        alt: impl Into<String>,
    ) -> Result<Self, RadrootsAuthoredPostError> {
        let descriptor = image.descriptor();
        if descriptor.size() == 0 {
            return Err(RadrootsAuthoredPostError::ImageSizeInvalid);
        }
        if !post_image_media_type_is_valid(descriptor.media_type().as_str()) {
            return Err(RadrootsAuthoredPostError::ImageMediaTypeInvalid);
        }
        let alt = alt.into();
        if alt.trim().is_empty() {
            return Err(RadrootsAuthoredPostError::ImageAltInvalid);
        }
        if alt.len() > RADROOTS_POST_ALT_MAX_BYTES {
            return Err(RadrootsAuthoredPostError::ImageAltTooLarge {
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
        fallback: RadrootsBlossomApprovedBlobUrl,
    ) -> Result<Self, RadrootsAuthoredPostError> {
        if fallback.as_blob_url().hash_path().hash() != self.image.descriptor().sha256() {
            return Err(RadrootsAuthoredPostError::ImageFallbackHashMismatch);
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

    pub fn image(&self) -> &RadrootsAuthoredImage {
        &self.image
    }

    pub const fn dimensions(&self) -> RadrootsPostImageDimensions {
        self.dimensions
    }

    pub fn alt(&self) -> &str {
        &self.alt
    }

    pub fn fallbacks(&self) -> &[RadrootsBlossomApprovedBlobUrl] {
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
/// let _: radroots_event::post::RadrootsAuthoredUpdate =
///     serde_json::from_str(r#"{"content":"harvest"}"#).unwrap();
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsAuthoredUpdate {
    content: String,
}

impl RadrootsAuthoredUpdate {
    pub fn new(content: impl Into<String>) -> Result<Self, RadrootsAuthoredPostError> {
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
pub struct RadrootsAuthoredPhotoUpdate {
    content: String,
    images: Vec<RadrootsAuthoredPostImage>,
}

impl RadrootsAuthoredPhotoUpdate {
    pub fn new(
        content: impl Into<String>,
        images: Vec<RadrootsAuthoredPostImage>,
    ) -> Result<Self, RadrootsAuthoredPostError> {
        let content = content.into();
        validate_content_size(&content)?;
        validate_authored_images(&content, &images, 0)?;
        validate_post_event_wire_size(&content, false, &images)?;
        Ok(Self { content, images })
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    pub fn images(&self) -> &[RadrootsAuthoredPostImage] {
        &self.images
    }
}

/// Strict authored root kind-1 Ask with its exact product marker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsAuthoredAsk {
    content: String,
    images: Vec<RadrootsAuthoredPostImage>,
}

impl RadrootsAuthoredAsk {
    pub fn new(
        content: impl Into<String>,
        images: Vec<RadrootsAuthoredPostImage>,
    ) -> Result<Self, RadrootsAuthoredPostError> {
        let content = content.into();
        validate_authored_root_content(&content)?;
        if images.len() > RADROOTS_POST_IMETA_MAX_COUNT {
            return Err(RadrootsAuthoredPostError::ImageCountExceeded {
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

    pub fn images(&self) -> &[RadrootsAuthoredPostImage] {
        &self.images
    }
}

fn validate_authored_root_content(content: &str) -> Result<(), RadrootsAuthoredPostError> {
    validate_content_size(content)?;
    if content.trim().is_empty() {
        return Err(RadrootsAuthoredPostError::ContentMissing);
    }
    Ok(())
}

fn validate_content_size(content: &str) -> Result<(), RadrootsAuthoredPostError> {
    if content.len() > RADROOTS_POST_CONTENT_MAX_BYTES {
        return Err(RadrootsAuthoredPostError::ContentTooLarge {
            max: RADROOTS_POST_CONTENT_MAX_BYTES,
            actual: content.len(),
        });
    }
    Ok(())
}

fn validate_authored_images(
    content: &str,
    images: &[RadrootsAuthoredPostImage],
    initial_tag_bytes: usize,
) -> Result<(), RadrootsAuthoredPostError> {
    if images.is_empty() {
        return Err(RadrootsAuthoredPostError::ImageMissing);
    }
    if images.len() > RADROOTS_POST_IMETA_MAX_COUNT {
        return Err(RadrootsAuthoredPostError::ImageCountExceeded {
            max: RADROOTS_POST_IMETA_MAX_COUNT,
            actual: images.len(),
        });
    }
    for (index, image) in images.iter().enumerate() {
        if !content.contains(image.url()) {
            return Err(RadrootsAuthoredPostError::ImageUrlMissingFromContent);
        }
        if images[..index]
            .iter()
            .any(|candidate| candidate.url() == image.url())
        {
            return Err(RadrootsAuthoredPostError::DuplicateImageUrl);
        }
    }
    let total_tag_bytes = images.iter().fold(initial_tag_bytes, |total, image| {
        total.saturating_add(imeta_tag_bytes(image.imeta_tag()))
    });
    validate_tag_bytes(total_tag_bytes)?;
    Ok(())
}

fn derive_imeta_tag(
    image: &RadrootsAuthoredImage,
    dimensions: RadrootsPostImageDimensions,
    alt: &str,
    fallbacks: &[RadrootsBlossomApprovedBlobUrl],
) -> Result<Vec<String>, RadrootsAuthoredPostError> {
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

fn validate_tag_element(element: &str) -> Result<(), RadrootsAuthoredPostError> {
    if element.len() > RADROOTS_POST_TAG_ELEMENT_MAX_BYTES {
        return Err(RadrootsAuthoredPostError::TagElementTooLarge {
            max: RADROOTS_POST_TAG_ELEMENT_MAX_BYTES,
            actual: element.len(),
        });
    }
    Ok(())
}

fn validate_tag_bytes(actual: usize) -> Result<(), RadrootsAuthoredPostError> {
    if actual > RADROOTS_POST_TAG_TOTAL_MAX_BYTES {
        return Err(RadrootsAuthoredPostError::TagBytesExceeded {
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
    images: &[RadrootsAuthoredPostImage],
) -> Result<(), RadrootsAuthoredPostError> {
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
        return Err(RadrootsAuthoredPostError::EventWireTooLarge {
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
mod tests {
    use super::*;

    #[test]
    fn content_only_post_deserializes_without_social_metadata() {
        let post: RadrootsPost =
            serde_json::from_str(r#"{"content":"farm update"}"#).expect("post");

        assert_eq!(post.content, "farm update");
        assert!(post.farm.is_none());
        assert!(post.address_refs.is_none());
        assert!(post.location.is_none());
        assert!(post.topics.is_none());
        assert!(post.quote_refs.is_none());
        assert!(post.media.is_none());
    }

    #[test]
    fn content_only_post_serializes_without_null_social_metadata() {
        let post = RadrootsPost {
            content: "farm update".to_string(),
            farm: None,
            address_refs: None,
            location: None,
            topics: None,
            quote_refs: None,
            media: None,
        };

        let json = serde_json::to_string(&post).expect("json");
        assert_eq!(json, r#"{"content":"farm update"}"#);
    }

    #[test]
    fn post_image_media_type_uses_exact_product_grammar() {
        for valid in [
            "image/png",
            "image/1",
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
}
