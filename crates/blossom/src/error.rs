use core::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum RadrootsBlossomError {
    InvalidSha256,
    InvalidFileExtension,
    InvalidHashPath,
    InvalidBlobUrl,
    UnsupportedBlobUrlScheme,
    BlobUrlCredentialsForbidden,
    BlobUrlQueryForbidden,
    BlobUrlFragmentForbidden,
    InsecureBlobUrl,
    DescriptorExtensionRequired,
    DescriptorHashMismatch,
    InvalidMediaType,
    BlobHashMismatch,
    BlobSizeMismatch { expected: u64, actual: u64 },
    BlobMediaTypeMismatch,
    InvalidAuthorizationContent,
    InvalidAuthorizationAction,
    InvalidAuthorizationServerDomain,
    MissingAuthorizationActionTag,
    DuplicateAuthorizationActionTag,
    MalformedAuthorizationActionTag,
    MissingAuthorizationExpirationTag,
    DuplicateAuthorizationExpirationTag,
    MalformedAuthorizationExpirationTag,
    MalformedAuthorizationServerTag,
    MalformedAuthorizationHashTag,
    InvalidAuthorizationCreatedAge,
    InvalidAuthorizationLifetime,
    AuthorizationTimestampOverflow,
    AuthorizationCreatedInFuture,
    AuthorizationStale,
    AuthorizationExpired,
    AuthorizationActionMismatch,
    AuthorizationServerRequired,
    AuthorizationServerMismatch,
    AuthorizationHashRequired,
    AuthorizationHashMismatch,
    InvalidBud02UploadStatus { actual: u16 },
    InvalidBud01HeadStatus { actual: u16 },
    InvalidBud01GetStatus { actual: u16 },
    PublicationRasterByteLimitExceeded { declared: u64, maximum: u64 },
    PublicationGetBodyAllocationFailed,
    PublicationGetBodyLengthOverflow,
    PublicationGetBodyMissing,
    PublicationGetBodyShort { declared: u64, actual: u64 },
    PublicationGetBodyTrailing { declared: u64, actual: u64 },
    PublicationAuthoredBytesSizeMismatch { expected: u64, actual: u64 },
    PublicationAuthoredBytesHashMismatch,
    PublicationUploadUrlMismatch,
    PublicationUploadHashMismatch,
    PublicationUploadSizeMismatch { expected: u64, actual: u64 },
    PublicationUploadMediaTypeMismatch,
    PublicationHeadUrlMismatch,
    PublicationHeadSizeMismatch { expected: u64, actual: u64 },
    PublicationHeadMediaTypeMismatch,
    PublicationGetUrlMismatch,
    PublicationGetDeclaredSizeMismatch { expected: u64, actual: u64 },
    PublicationRetrievedBytesHashMismatch,
    PublicationRetrievedBytesMismatch,
    UnsupportedPublicationRasterMediaType,
    InvalidPublicationRaster,
    PublicationJpegProcessForbidden,
    PublicationRasterAnimationForbidden,
    PublicationRasterDimensionsOutOfRange { width: u32, height: u32 },
    PublicationRasterPixelLimitExceeded { pixels: u64 },
    PublicationRasterDecodedByteLimitExceeded { decoded: u64, maximum: u64 },
    PublicationRasterDecodeAllocationFailed,
    PublicationRasterDecodeFailed,
    PublicationRasterContainerDimensionMismatch,
    PublicationAuthoredRasterDimensionMismatch,
}

impl RadrootsBlossomError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidSha256 => "invalid_sha256",
            Self::InvalidFileExtension => "invalid_file_extension",
            Self::InvalidHashPath => "invalid_hash_path",
            Self::InvalidBlobUrl => "invalid_blob_url",
            Self::UnsupportedBlobUrlScheme => "unsupported_blob_url_scheme",
            Self::BlobUrlCredentialsForbidden => "blob_url_credentials_forbidden",
            Self::BlobUrlQueryForbidden => "blob_url_query_forbidden",
            Self::BlobUrlFragmentForbidden => "blob_url_fragment_forbidden",
            Self::InsecureBlobUrl => "insecure_blob_url",
            Self::DescriptorExtensionRequired => "descriptor_extension_required",
            Self::DescriptorHashMismatch => "descriptor_hash_mismatch",
            Self::InvalidMediaType => "invalid_media_type",
            Self::BlobHashMismatch => "blob_hash_mismatch",
            Self::BlobSizeMismatch { .. } => "blob_size_mismatch",
            Self::BlobMediaTypeMismatch => "blob_media_type_mismatch",
            Self::InvalidAuthorizationContent => "invalid_authorization_content",
            Self::InvalidAuthorizationAction => "invalid_authorization_action",
            Self::InvalidAuthorizationServerDomain => "invalid_authorization_server_domain",
            Self::MissingAuthorizationActionTag => "missing_authorization_action_tag",
            Self::DuplicateAuthorizationActionTag => "duplicate_authorization_action_tag",
            Self::MalformedAuthorizationActionTag => "malformed_authorization_action_tag",
            Self::MissingAuthorizationExpirationTag => "missing_authorization_expiration_tag",
            Self::DuplicateAuthorizationExpirationTag => "duplicate_authorization_expiration_tag",
            Self::MalformedAuthorizationExpirationTag => "malformed_authorization_expiration_tag",
            Self::MalformedAuthorizationServerTag => "malformed_authorization_server_tag",
            Self::MalformedAuthorizationHashTag => "malformed_authorization_hash_tag",
            Self::InvalidAuthorizationCreatedAge => "invalid_authorization_created_age",
            Self::InvalidAuthorizationLifetime => "invalid_authorization_lifetime",
            Self::AuthorizationTimestampOverflow => "authorization_timestamp_overflow",
            Self::AuthorizationCreatedInFuture => "authorization_created_in_future",
            Self::AuthorizationStale => "authorization_stale",
            Self::AuthorizationExpired => "authorization_expired",
            Self::AuthorizationActionMismatch => "authorization_action_mismatch",
            Self::AuthorizationServerRequired => "authorization_server_required",
            Self::AuthorizationServerMismatch => "authorization_server_mismatch",
            Self::AuthorizationHashRequired => "authorization_hash_required",
            Self::AuthorizationHashMismatch => "authorization_hash_mismatch",
            Self::InvalidBud02UploadStatus { .. } => "invalid_bud02_upload_status",
            Self::InvalidBud01HeadStatus { .. } => "invalid_bud01_head_status",
            Self::InvalidBud01GetStatus { .. } => "invalid_bud01_get_status",
            Self::PublicationRasterByteLimitExceeded { .. } => {
                "publication_raster_byte_limit_exceeded"
            }
            Self::PublicationGetBodyAllocationFailed => "publication_get_body_allocation_failed",
            Self::PublicationGetBodyLengthOverflow => "publication_get_body_length_overflow",
            Self::PublicationGetBodyMissing => "publication_get_body_missing",
            Self::PublicationGetBodyShort { .. } => "publication_get_body_short",
            Self::PublicationGetBodyTrailing { .. } => "publication_get_body_trailing",
            Self::PublicationAuthoredBytesSizeMismatch { .. } => {
                "publication_authored_bytes_size_mismatch"
            }
            Self::PublicationAuthoredBytesHashMismatch => {
                "publication_authored_bytes_hash_mismatch"
            }
            Self::PublicationUploadUrlMismatch => "publication_upload_url_mismatch",
            Self::PublicationUploadHashMismatch => "publication_upload_hash_mismatch",
            Self::PublicationUploadSizeMismatch { .. } => "publication_upload_size_mismatch",
            Self::PublicationUploadMediaTypeMismatch => "publication_upload_media_type_mismatch",
            Self::PublicationHeadUrlMismatch => "publication_head_url_mismatch",
            Self::PublicationHeadSizeMismatch { .. } => "publication_head_size_mismatch",
            Self::PublicationHeadMediaTypeMismatch => "publication_head_media_type_mismatch",
            Self::PublicationGetUrlMismatch => "publication_get_url_mismatch",
            Self::PublicationGetDeclaredSizeMismatch { .. } => {
                "publication_get_declared_size_mismatch"
            }
            Self::PublicationRetrievedBytesHashMismatch => {
                "publication_retrieved_bytes_hash_mismatch"
            }
            Self::PublicationRetrievedBytesMismatch => "publication_retrieved_bytes_mismatch",
            Self::UnsupportedPublicationRasterMediaType => {
                "unsupported_publication_raster_media_type"
            }
            Self::InvalidPublicationRaster => "invalid_publication_raster",
            Self::PublicationJpegProcessForbidden => "publication_jpeg_process_forbidden",
            Self::PublicationRasterAnimationForbidden => "publication_raster_animation_forbidden",
            Self::PublicationRasterDimensionsOutOfRange { .. } => {
                "publication_raster_dimensions_out_of_range"
            }
            Self::PublicationRasterPixelLimitExceeded { .. } => {
                "publication_raster_pixel_limit_exceeded"
            }
            Self::PublicationRasterDecodedByteLimitExceeded { .. } => {
                "publication_raster_decoded_byte_limit_exceeded"
            }
            Self::PublicationRasterDecodeAllocationFailed => {
                "publication_raster_decode_allocation_failed"
            }
            Self::PublicationRasterDecodeFailed => "publication_raster_decode_failed",
            Self::PublicationRasterContainerDimensionMismatch => {
                "publication_raster_container_dimension_mismatch"
            }
            Self::PublicationAuthoredRasterDimensionMismatch => {
                "publication_authored_raster_dimension_mismatch"
            }
        }
    }
}

impl fmt::Display for RadrootsBlossomError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSha256 => {
                f.write_str("sha256 must be 64 lowercase hexadecimal characters")
            }
            Self::InvalidFileExtension => f.write_str("invalid Blossom file extension"),
            Self::InvalidHashPath => f.write_str("invalid Blossom root hash path"),
            Self::InvalidBlobUrl => f.write_str("invalid Blossom blob URL"),
            Self::UnsupportedBlobUrlScheme => {
                f.write_str("Blossom blob URL scheme must be http or https")
            }
            Self::BlobUrlCredentialsForbidden => {
                f.write_str("Blossom blob URL credentials are forbidden")
            }
            Self::BlobUrlQueryForbidden => f.write_str("Blossom blob URL query is forbidden"),
            Self::BlobUrlFragmentForbidden => f.write_str("Blossom blob URL fragment is forbidden"),
            Self::InsecureBlobUrl => {
                f.write_str("Radroots blob references require HTTPS or loopback HTTP")
            }
            Self::DescriptorExtensionRequired => {
                f.write_str("BUD-02 descriptor URL requires a file extension")
            }
            Self::DescriptorHashMismatch => {
                f.write_str("descriptor URL hash does not match descriptor sha256")
            }
            Self::InvalidMediaType => f.write_str("invalid media type"),
            Self::BlobHashMismatch => f.write_str("blob bytes do not match descriptor sha256"),
            Self::BlobSizeMismatch { expected, actual } => {
                write!(f, "blob size mismatch: expected {expected}, got {actual}")
            }
            Self::BlobMediaTypeMismatch => {
                f.write_str("descriptor media type does not match the approved media type")
            }
            Self::InvalidAuthorizationContent => {
                f.write_str("Blossom authorization content must be bounded human-readable text")
            }
            Self::InvalidAuthorizationAction => f.write_str("invalid Blossom authorization action"),
            Self::InvalidAuthorizationServerDomain => {
                f.write_str("invalid Blossom authorization server domain")
            }
            Self::MissingAuthorizationActionTag => {
                f.write_str("Blossom authorization is missing a t action tag")
            }
            Self::DuplicateAuthorizationActionTag => {
                f.write_str("Blossom authorization has more than one t action tag")
            }
            Self::MalformedAuthorizationActionTag => {
                f.write_str("malformed Blossom authorization t action tag")
            }
            Self::MissingAuthorizationExpirationTag => {
                f.write_str("Blossom authorization is missing an expiration tag")
            }
            Self::DuplicateAuthorizationExpirationTag => {
                f.write_str("Blossom authorization has more than one expiration tag")
            }
            Self::MalformedAuthorizationExpirationTag => {
                f.write_str("malformed Blossom authorization expiration tag")
            }
            Self::MalformedAuthorizationServerTag => {
                f.write_str("malformed Blossom authorization server tag")
            }
            Self::MalformedAuthorizationHashTag => {
                f.write_str("malformed Blossom authorization x hash tag")
            }
            Self::InvalidAuthorizationCreatedAge => {
                f.write_str("Blossom authorization maximum created age must not exceed 300 seconds")
            }
            Self::InvalidAuthorizationLifetime => {
                f.write_str("Blossom authorization lifetime must be between 1 and 300 seconds")
            }
            Self::AuthorizationTimestampOverflow => {
                f.write_str("Blossom authorization expiration timestamp overflows u64")
            }
            Self::AuthorizationCreatedInFuture => {
                f.write_str("Blossom authorization must be created in the past")
            }
            Self::AuthorizationStale => {
                f.write_str("Blossom authorization is outside the accepted creation-age window")
            }
            Self::AuthorizationExpired => f.write_str("Blossom authorization is expired"),
            Self::AuthorizationActionMismatch => {
                f.write_str("Blossom authorization action does not match the target endpoint")
            }
            Self::AuthorizationServerRequired => {
                f.write_str("Blossom authorization requires a server scope")
            }
            Self::AuthorizationServerMismatch => {
                f.write_str("Blossom authorization does not include the target server")
            }
            Self::AuthorizationHashRequired => {
                f.write_str("Blossom authorization requires an x hash scope")
            }
            Self::AuthorizationHashMismatch => {
                f.write_str("Blossom authorization does not include the target blob hash")
            }
            Self::InvalidBud02UploadStatus { actual } => {
                write!(f, "BUD-02 upload status must be 200 or 201, got {actual}")
            }
            Self::InvalidBud01HeadStatus { actual } => {
                write!(f, "BUD-01 HEAD status must be 200, got {actual}")
            }
            Self::InvalidBud01GetStatus { actual } => {
                write!(f, "BUD-01 GET status must be 200, got {actual}")
            }
            Self::PublicationRasterByteLimitExceeded { declared, maximum } => write!(
                f,
                "publication raster declares {declared} bytes, exceeding maximum {maximum}"
            ),
            Self::PublicationGetBodyAllocationFailed => {
                f.write_str("publication GET body allocation failed")
            }
            Self::PublicationGetBodyLengthOverflow => {
                f.write_str("publication GET body length overflowed")
            }
            Self::PublicationGetBodyMissing => f.write_str("publication GET body is missing"),
            Self::PublicationGetBodyShort { declared, actual } => write!(
                f,
                "publication GET body is short: declared {declared}, got {actual}"
            ),
            Self::PublicationGetBodyTrailing { declared, actual } => write!(
                f,
                "publication GET body has trailing bytes: declared {declared}, got {actual}"
            ),
            Self::PublicationAuthoredBytesSizeMismatch { expected, actual } => write!(
                f,
                "authored raster byte size mismatch: expected {expected}, got {actual}"
            ),
            Self::PublicationAuthoredBytesHashMismatch => {
                f.write_str("authored raster bytes do not match the sealed descriptor hash")
            }
            Self::PublicationUploadUrlMismatch => {
                f.write_str("BUD-02 upload descriptor URL does not match the authored URL")
            }
            Self::PublicationUploadHashMismatch => {
                f.write_str("BUD-02 upload descriptor hash does not match the authored hash")
            }
            Self::PublicationUploadSizeMismatch { expected, actual } => write!(
                f,
                "BUD-02 upload descriptor size mismatch: expected {expected}, got {actual}"
            ),
            Self::PublicationUploadMediaTypeMismatch => f.write_str(
                "BUD-02 upload descriptor media type does not match the authored media type",
            ),
            Self::PublicationHeadUrlMismatch => {
                f.write_str("BUD-01 HEAD URL does not match the authored URL")
            }
            Self::PublicationHeadSizeMismatch { expected, actual } => write!(
                f,
                "BUD-01 HEAD content length mismatch: expected {expected}, got {actual}"
            ),
            Self::PublicationHeadMediaTypeMismatch => {
                f.write_str("BUD-01 HEAD media type does not match the authored media type")
            }
            Self::PublicationGetUrlMismatch => {
                f.write_str("BUD-01 GET URL does not match the authored URL")
            }
            Self::PublicationGetDeclaredSizeMismatch { expected, actual } => write!(
                f,
                "BUD-01 GET declared size mismatch: expected {expected}, got {actual}"
            ),
            Self::PublicationRetrievedBytesHashMismatch => {
                f.write_str("BUD-01 GET complete-byte hash does not match the authored hash")
            }
            Self::PublicationRetrievedBytesMismatch => {
                f.write_str("BUD-01 GET bytes differ from the exact authored raster bytes")
            }
            Self::UnsupportedPublicationRasterMediaType => f.write_str(
                "publication raster media type must be image/jpeg, image/png, or image/webp",
            ),
            Self::InvalidPublicationRaster => {
                f.write_str("publication raster container is malformed or incomplete")
            }
            Self::PublicationJpegProcessForbidden => {
                f.write_str("publication JPEG must use an 8-bit sequential SOF0 or SOF1 process")
            }
            Self::PublicationRasterAnimationForbidden => {
                f.write_str("publication raster animation is forbidden")
            }
            Self::PublicationRasterDimensionsOutOfRange { width, height } => write!(
                f,
                "publication raster dimensions must be within 1..=16384, got {width}x{height}"
            ),
            Self::PublicationRasterPixelLimitExceeded { pixels } => write!(
                f,
                "publication raster pixel count {pixels} exceeds 20000000"
            ),
            Self::PublicationRasterDecodedByteLimitExceeded { decoded, maximum } => write!(
                f,
                "publication raster requires {decoded} decoded bytes, exceeding maximum {maximum}"
            ),
            Self::PublicationRasterDecodeAllocationFailed => {
                f.write_str("publication raster decoded-pixel buffer allocation failed")
            }
            Self::PublicationRasterDecodeFailed => {
                f.write_str("publication raster bitstream could not be decoded completely")
            }
            Self::PublicationRasterContainerDimensionMismatch => f.write_str(
                "decoded raster dimensions do not match the dimensions encoded by the container",
            ),
            Self::PublicationAuthoredRasterDimensionMismatch => {
                f.write_str("decoded raster dimensions do not match the authored dimensions")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsBlossomError {}

#[cfg(test)]
mod tests {
    use super::RadrootsBlossomError;
    use alloc::format;

    #[test]
    fn error_codes_and_messages_are_stable() {
        let errors = [
            RadrootsBlossomError::InvalidSha256,
            RadrootsBlossomError::InvalidFileExtension,
            RadrootsBlossomError::InvalidHashPath,
            RadrootsBlossomError::InvalidBlobUrl,
            RadrootsBlossomError::UnsupportedBlobUrlScheme,
            RadrootsBlossomError::BlobUrlCredentialsForbidden,
            RadrootsBlossomError::BlobUrlQueryForbidden,
            RadrootsBlossomError::BlobUrlFragmentForbidden,
            RadrootsBlossomError::InsecureBlobUrl,
            RadrootsBlossomError::DescriptorExtensionRequired,
            RadrootsBlossomError::DescriptorHashMismatch,
            RadrootsBlossomError::InvalidMediaType,
            RadrootsBlossomError::BlobHashMismatch,
            RadrootsBlossomError::BlobSizeMismatch {
                expected: 1,
                actual: 2,
            },
            RadrootsBlossomError::BlobMediaTypeMismatch,
            RadrootsBlossomError::InvalidBud02UploadStatus { actual: 202 },
            RadrootsBlossomError::InvalidBud01HeadStatus { actual: 204 },
            RadrootsBlossomError::InvalidBud01GetStatus { actual: 206 },
            RadrootsBlossomError::PublicationRasterByteLimitExceeded {
                declared: 2,
                maximum: 1,
            },
            RadrootsBlossomError::PublicationGetBodyAllocationFailed,
            RadrootsBlossomError::PublicationGetBodyLengthOverflow,
            RadrootsBlossomError::PublicationGetBodyMissing,
            RadrootsBlossomError::PublicationGetBodyShort {
                declared: 2,
                actual: 1,
            },
            RadrootsBlossomError::PublicationGetBodyTrailing {
                declared: 1,
                actual: 2,
            },
            RadrootsBlossomError::PublicationAuthoredBytesSizeMismatch {
                expected: 1,
                actual: 2,
            },
            RadrootsBlossomError::PublicationAuthoredBytesHashMismatch,
            RadrootsBlossomError::PublicationUploadUrlMismatch,
            RadrootsBlossomError::PublicationUploadHashMismatch,
            RadrootsBlossomError::PublicationUploadSizeMismatch {
                expected: 1,
                actual: 2,
            },
            RadrootsBlossomError::PublicationUploadMediaTypeMismatch,
            RadrootsBlossomError::PublicationHeadUrlMismatch,
            RadrootsBlossomError::PublicationHeadSizeMismatch {
                expected: 1,
                actual: 2,
            },
            RadrootsBlossomError::PublicationHeadMediaTypeMismatch,
            RadrootsBlossomError::PublicationGetUrlMismatch,
            RadrootsBlossomError::PublicationGetDeclaredSizeMismatch {
                expected: 1,
                actual: 2,
            },
            RadrootsBlossomError::PublicationRetrievedBytesHashMismatch,
            RadrootsBlossomError::PublicationRetrievedBytesMismatch,
            RadrootsBlossomError::UnsupportedPublicationRasterMediaType,
            RadrootsBlossomError::InvalidPublicationRaster,
            RadrootsBlossomError::PublicationJpegProcessForbidden,
            RadrootsBlossomError::PublicationRasterAnimationForbidden,
            RadrootsBlossomError::PublicationRasterDimensionsOutOfRange {
                width: 0,
                height: 1,
            },
            RadrootsBlossomError::PublicationRasterPixelLimitExceeded { pixels: 20_000_001 },
            RadrootsBlossomError::PublicationRasterDecodedByteLimitExceeded {
                decoded: 2,
                maximum: 1,
            },
            RadrootsBlossomError::PublicationRasterDecodeAllocationFailed,
            RadrootsBlossomError::PublicationRasterDecodeFailed,
            RadrootsBlossomError::PublicationRasterContainerDimensionMismatch,
            RadrootsBlossomError::PublicationAuthoredRasterDimensionMismatch,
            RadrootsBlossomError::InvalidAuthorizationContent,
            RadrootsBlossomError::InvalidAuthorizationAction,
            RadrootsBlossomError::InvalidAuthorizationServerDomain,
            RadrootsBlossomError::MissingAuthorizationActionTag,
            RadrootsBlossomError::DuplicateAuthorizationActionTag,
            RadrootsBlossomError::MalformedAuthorizationActionTag,
            RadrootsBlossomError::MissingAuthorizationExpirationTag,
            RadrootsBlossomError::DuplicateAuthorizationExpirationTag,
            RadrootsBlossomError::MalformedAuthorizationExpirationTag,
            RadrootsBlossomError::MalformedAuthorizationServerTag,
            RadrootsBlossomError::MalformedAuthorizationHashTag,
            RadrootsBlossomError::InvalidAuthorizationCreatedAge,
            RadrootsBlossomError::InvalidAuthorizationLifetime,
            RadrootsBlossomError::AuthorizationTimestampOverflow,
            RadrootsBlossomError::AuthorizationCreatedInFuture,
            RadrootsBlossomError::AuthorizationStale,
            RadrootsBlossomError::AuthorizationExpired,
            RadrootsBlossomError::AuthorizationActionMismatch,
            RadrootsBlossomError::AuthorizationServerRequired,
            RadrootsBlossomError::AuthorizationServerMismatch,
            RadrootsBlossomError::AuthorizationHashRequired,
            RadrootsBlossomError::AuthorizationHashMismatch,
        ];
        for error in errors {
            assert!(!error.code().is_empty());
            assert!(!format!("{error}").is_empty());
        }
    }

    #[test]
    fn authorization_error_codes_and_messages_are_stable() {
        let cases = [
            (
                RadrootsBlossomError::InvalidAuthorizationContent,
                "invalid_authorization_content",
                "Blossom authorization content must be bounded human-readable text",
            ),
            (
                RadrootsBlossomError::InvalidAuthorizationAction,
                "invalid_authorization_action",
                "invalid Blossom authorization action",
            ),
            (
                RadrootsBlossomError::InvalidAuthorizationServerDomain,
                "invalid_authorization_server_domain",
                "invalid Blossom authorization server domain",
            ),
            (
                RadrootsBlossomError::MissingAuthorizationActionTag,
                "missing_authorization_action_tag",
                "Blossom authorization is missing a t action tag",
            ),
            (
                RadrootsBlossomError::DuplicateAuthorizationActionTag,
                "duplicate_authorization_action_tag",
                "Blossom authorization has more than one t action tag",
            ),
            (
                RadrootsBlossomError::MalformedAuthorizationActionTag,
                "malformed_authorization_action_tag",
                "malformed Blossom authorization t action tag",
            ),
            (
                RadrootsBlossomError::MissingAuthorizationExpirationTag,
                "missing_authorization_expiration_tag",
                "Blossom authorization is missing an expiration tag",
            ),
            (
                RadrootsBlossomError::DuplicateAuthorizationExpirationTag,
                "duplicate_authorization_expiration_tag",
                "Blossom authorization has more than one expiration tag",
            ),
            (
                RadrootsBlossomError::MalformedAuthorizationExpirationTag,
                "malformed_authorization_expiration_tag",
                "malformed Blossom authorization expiration tag",
            ),
            (
                RadrootsBlossomError::MalformedAuthorizationServerTag,
                "malformed_authorization_server_tag",
                "malformed Blossom authorization server tag",
            ),
            (
                RadrootsBlossomError::MalformedAuthorizationHashTag,
                "malformed_authorization_hash_tag",
                "malformed Blossom authorization x hash tag",
            ),
            (
                RadrootsBlossomError::InvalidAuthorizationCreatedAge,
                "invalid_authorization_created_age",
                "Blossom authorization maximum created age must not exceed 300 seconds",
            ),
            (
                RadrootsBlossomError::InvalidAuthorizationLifetime,
                "invalid_authorization_lifetime",
                "Blossom authorization lifetime must be between 1 and 300 seconds",
            ),
            (
                RadrootsBlossomError::AuthorizationTimestampOverflow,
                "authorization_timestamp_overflow",
                "Blossom authorization expiration timestamp overflows u64",
            ),
            (
                RadrootsBlossomError::AuthorizationCreatedInFuture,
                "authorization_created_in_future",
                "Blossom authorization must be created in the past",
            ),
            (
                RadrootsBlossomError::AuthorizationStale,
                "authorization_stale",
                "Blossom authorization is outside the accepted creation-age window",
            ),
            (
                RadrootsBlossomError::AuthorizationExpired,
                "authorization_expired",
                "Blossom authorization is expired",
            ),
            (
                RadrootsBlossomError::AuthorizationActionMismatch,
                "authorization_action_mismatch",
                "Blossom authorization action does not match the target endpoint",
            ),
            (
                RadrootsBlossomError::AuthorizationServerRequired,
                "authorization_server_required",
                "Blossom authorization requires a server scope",
            ),
            (
                RadrootsBlossomError::AuthorizationServerMismatch,
                "authorization_server_mismatch",
                "Blossom authorization does not include the target server",
            ),
            (
                RadrootsBlossomError::AuthorizationHashRequired,
                "authorization_hash_required",
                "Blossom authorization requires an x hash scope",
            ),
            (
                RadrootsBlossomError::AuthorizationHashMismatch,
                "authorization_hash_mismatch",
                "Blossom authorization does not include the target blob hash",
            ),
        ];
        for (error, code, message) in cases {
            assert_eq!(error.code(), code);
            assert_eq!(format!("{error}"), message);
        }
    }
}
