use core::fmt;

use radroots_blossom::descriptor::ByteVerifiedDescriptor;

/// Errors raised while constructing strict authored image media.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsAuthoredImageError {
    MediaTypeNotImage,
}

impl RadrootsAuthoredImageError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::MediaTypeNotImage => "media_type_not_image",
        }
    }
}

impl fmt::Display for RadrootsAuthoredImageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MediaTypeNotImage => {
                f.write_str("authored image descriptor must have an image media type")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsAuthoredImageError {}

/// A byte-verified Blossom descriptor whose declared media type is `image/*`.
///
/// Construction requires the non-forgeable Blossom byte-verification state.
/// This typestate does not prove upload completion, network availability, or
/// image format safety. Owning runtimes remain responsible for those policies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsAuthoredImage(ByteVerifiedDescriptor);

impl RadrootsAuthoredImage {
    pub fn try_from_verified_descriptor(
        descriptor: ByteVerifiedDescriptor,
    ) -> Result<Self, RadrootsAuthoredImageError> {
        if !descriptor.media_type().as_str().starts_with("image/") {
            return Err(RadrootsAuthoredImageError::MediaTypeNotImage);
        }
        Ok(Self(descriptor))
    }

    pub fn descriptor(&self) -> &ByteVerifiedDescriptor {
        &self.0
    }
}

impl TryFrom<ByteVerifiedDescriptor> for RadrootsAuthoredImage {
    type Error = RadrootsAuthoredImageError;

    fn try_from(value: ByteVerifiedDescriptor) -> Result<Self, Self::Error> {
        Self::try_from_verified_descriptor(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use radroots_blossom::{BlobDescriptor, BlobUrl, MediaType, Sha256};

    #[test]
    fn authored_image_requires_a_byte_verified_image_descriptor() {
        let descriptor = verified_descriptor("image/webp", "webp");
        let expected_url = descriptor.url().as_str().to_owned();
        let image = RadrootsAuthoredImage::try_from(descriptor).unwrap();

        assert_eq!(image.descriptor().url().as_str(), expected_url);
    }

    #[test]
    fn authored_image_rejects_non_image_media_types_with_a_stable_error() {
        let error = RadrootsAuthoredImage::try_from_verified_descriptor(verified_descriptor(
            "text/plain",
            "txt",
        ))
        .unwrap_err();

        assert_eq!(error, RadrootsAuthoredImageError::MediaTypeNotImage);
        assert_eq!(error.code(), "media_type_not_image");
        assert_eq!(
            error.to_string(),
            "authored image descriptor must have an image media type"
        );
    }

    fn verified_descriptor(media_type: &str, extension: &str) -> ByteVerifiedDescriptor {
        let bytes = b"hello";
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
}
