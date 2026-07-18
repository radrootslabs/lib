#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};
use core::fmt;

use radroots_blossom::{RadrootsBlossomApprovedBlobUrl, RadrootsBlossomMediaType};
use url_nostd::Url;

use crate::media::RadrootsAuthoredImage;
use crate::social::{
    RadrootsSocialFarmAnchor, RadrootsSocialLocation, RadrootsSocialMediaMetadata,
    RadrootsSocialTarget,
};

pub const RADROOTS_POST_CONTENT_MAX_BYTES: usize = crate::wire::DEFAULT_CONTENT_MAX_BYTES;
pub const RADROOTS_POST_IMETA_MAX_COUNT: usize = 64;
pub const RADROOTS_POST_ALT_MAX_BYTES: usize = (4 * 1024) - "alt ".len();
pub const RADROOTS_ASK_MARKER_TAG_KEY: &str = "t";
pub const RADROOTS_ASK_MARKER_TAG_VALUE: &str = "radroots-ask";

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
        Ok(Self {
            image,
            dimensions,
            alt,
            fallbacks: Vec::new(),
        })
    }

    pub fn try_with_fallback(
        mut self,
        fallback: RadrootsBlossomApprovedBlobUrl,
    ) -> Result<Self, RadrootsAuthoredPostError> {
        if fallback.as_blob_url().hash_path().hash() != self.image.descriptor().sha256() {
            return Err(RadrootsAuthoredPostError::ImageFallbackHashMismatch);
        }
        self.fallbacks.push(fallback);
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
        validate_authored_images(&content, &images)?;
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
            validate_authored_images(&content, &images)?;
        }
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
    Ok(())
}

pub fn post_image_media_type_is_valid(value: &str) -> bool {
    !value.contains(';')
        && value.starts_with("image/")
        && RadrootsBlossomMediaType::parse(value)
            .is_ok_and(|media_type| media_type.as_str() == value)
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
}
