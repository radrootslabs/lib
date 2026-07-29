use core::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
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
    PublicationReadinessUrlTooLarge { max: usize, actual: usize },
    PublicationRasterEmpty,
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
    PublicationRasterProcessForbidden,
    PublicationRasterAnimationForbidden,
    PublicationRasterDimensionsOutOfRange { width: u32, height: u32 },
    PublicationRasterPixelLimitExceeded { pixels: u64 },
    PublicationRasterDecodedByteLimitExceeded { decoded: u64, maximum: u64 },
    PublicationRasterDecodeAllocationFailed,
    PublicationRasterDecodeFailed,
    PublicationRasterContainerDimensionMismatch,
    PublicationAuthoredRasterDimensionMismatch,
    PublicationReadinessEvidenceTooLarge { max: usize, actual: usize },
    PublicationReadinessEvidenceInvalidJson,
    PublicationReadinessEvidenceUnsupportedSchemaVersion { expected: u32, actual: u32 },
    PublicationReadinessEvidenceUnsupportedPolicyVersion { expected: u16, actual: u16 },
    PublicationReadinessEvidenceInvalidField { field: &'static str },
    PublicationReadinessEvidenceDigestMismatch,
    PublicationReadinessEvidenceNonCanonicalJson,
    PublicationReadinessEvidenceSerialization,
}

impl Error {
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
            Self::PublicationReadinessUrlTooLarge { .. } => "publication_readiness_url_too_large",
            Self::PublicationRasterEmpty => "publication_raster_empty",
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
            Self::PublicationRasterProcessForbidden => "publication_raster_process_forbidden",
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
            Self::PublicationReadinessEvidenceTooLarge { .. } => {
                "publication_readiness_evidence_too_large"
            }
            Self::PublicationReadinessEvidenceInvalidJson => {
                "publication_readiness_evidence_invalid_json"
            }
            Self::PublicationReadinessEvidenceUnsupportedSchemaVersion { .. } => {
                "publication_readiness_evidence_schema_version_unsupported"
            }
            Self::PublicationReadinessEvidenceUnsupportedPolicyVersion { .. } => {
                "publication_readiness_evidence_policy_version_unsupported"
            }
            Self::PublicationReadinessEvidenceInvalidField { .. } => {
                "publication_readiness_evidence_field_invalid"
            }
            Self::PublicationReadinessEvidenceDigestMismatch => {
                "publication_readiness_evidence_digest_mismatch"
            }
            Self::PublicationReadinessEvidenceNonCanonicalJson => {
                "publication_readiness_evidence_json_non_canonical"
            }
            Self::PublicationReadinessEvidenceSerialization => {
                "publication_readiness_evidence_serialization_failed"
            }
        }
    }
}

impl fmt::Display for Error {
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
            Self::PublicationReadinessUrlTooLarge { max, actual } => write!(
                f,
                "publication-readiness URL contains {actual} bytes, exceeding maximum {max}"
            ),
            Self::PublicationRasterEmpty => {
                f.write_str("publication raster must contain at least one byte")
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
            Self::PublicationRasterProcessForbidden => f.write_str(
                "publication PNG and WebP rasters must use an approved static 8-bit process",
            ),
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
            Self::PublicationReadinessEvidenceTooLarge { max, actual } => write!(
                f,
                "publication-readiness evidence is {actual} bytes, exceeding maximum {max}"
            ),
            Self::PublicationReadinessEvidenceInvalidJson => {
                f.write_str("publication-readiness evidence is not valid strict JSON")
            }
            Self::PublicationReadinessEvidenceUnsupportedSchemaVersion { expected, actual } => {
                write!(
                    f,
                    "publication-readiness evidence schema version mismatch: expected {expected}, got {actual}"
                )
            }
            Self::PublicationReadinessEvidenceUnsupportedPolicyVersion { expected, actual } => {
                write!(
                    f,
                    "publication-readiness evidence policy version mismatch: expected {expected}, got {actual}"
                )
            }
            Self::PublicationReadinessEvidenceInvalidField { field } => {
                write!(
                    f,
                    "publication-readiness evidence field `{field}` is invalid"
                )
            }
            Self::PublicationReadinessEvidenceDigestMismatch => {
                f.write_str("publication-readiness evidence digest does not match its facts")
            }
            Self::PublicationReadinessEvidenceNonCanonicalJson => {
                f.write_str("publication-readiness evidence JSON is not canonical")
            }
            Self::PublicationReadinessEvidenceSerialization => {
                f.write_str("publication-readiness evidence serialization failed")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}

#[cfg(test)]
mod tests {
    use super::Error;
    use alloc::format;

    #[test]
    fn error_codes_and_messages_are_stable() {
        let errors = [
            Error::InvalidSha256,
            Error::InvalidFileExtension,
            Error::InvalidHashPath,
            Error::InvalidBlobUrl,
            Error::UnsupportedBlobUrlScheme,
            Error::BlobUrlCredentialsForbidden,
            Error::BlobUrlQueryForbidden,
            Error::BlobUrlFragmentForbidden,
            Error::InsecureBlobUrl,
            Error::DescriptorExtensionRequired,
            Error::DescriptorHashMismatch,
            Error::InvalidMediaType,
            Error::BlobHashMismatch,
            Error::BlobSizeMismatch {
                expected: 1,
                actual: 2,
            },
            Error::BlobMediaTypeMismatch,
            Error::InvalidBud02UploadStatus { actual: 202 },
            Error::InvalidBud01HeadStatus { actual: 204 },
            Error::InvalidBud01GetStatus { actual: 206 },
            Error::PublicationReadinessUrlTooLarge { max: 1, actual: 2 },
            Error::PublicationRasterEmpty,
            Error::PublicationRasterByteLimitExceeded {
                declared: 2,
                maximum: 1,
            },
            Error::PublicationGetBodyAllocationFailed,
            Error::PublicationGetBodyLengthOverflow,
            Error::PublicationGetBodyMissing,
            Error::PublicationGetBodyShort {
                declared: 2,
                actual: 1,
            },
            Error::PublicationGetBodyTrailing {
                declared: 1,
                actual: 2,
            },
            Error::PublicationAuthoredBytesSizeMismatch {
                expected: 1,
                actual: 2,
            },
            Error::PublicationAuthoredBytesHashMismatch,
            Error::PublicationUploadUrlMismatch,
            Error::PublicationUploadHashMismatch,
            Error::PublicationUploadSizeMismatch {
                expected: 1,
                actual: 2,
            },
            Error::PublicationUploadMediaTypeMismatch,
            Error::PublicationHeadUrlMismatch,
            Error::PublicationHeadSizeMismatch {
                expected: 1,
                actual: 2,
            },
            Error::PublicationHeadMediaTypeMismatch,
            Error::PublicationGetUrlMismatch,
            Error::PublicationGetDeclaredSizeMismatch {
                expected: 1,
                actual: 2,
            },
            Error::PublicationRetrievedBytesHashMismatch,
            Error::PublicationRetrievedBytesMismatch,
            Error::UnsupportedPublicationRasterMediaType,
            Error::InvalidPublicationRaster,
            Error::PublicationJpegProcessForbidden,
            Error::PublicationRasterProcessForbidden,
            Error::PublicationRasterAnimationForbidden,
            Error::PublicationRasterDimensionsOutOfRange {
                width: 0,
                height: 1,
            },
            Error::PublicationRasterPixelLimitExceeded { pixels: 20_000_001 },
            Error::PublicationRasterDecodedByteLimitExceeded {
                decoded: 2,
                maximum: 1,
            },
            Error::PublicationRasterDecodeAllocationFailed,
            Error::PublicationRasterDecodeFailed,
            Error::PublicationRasterContainerDimensionMismatch,
            Error::PublicationAuthoredRasterDimensionMismatch,
            Error::PublicationReadinessEvidenceTooLarge { max: 1, actual: 2 },
            Error::PublicationReadinessEvidenceInvalidJson,
            Error::PublicationReadinessEvidenceUnsupportedSchemaVersion {
                expected: 1,
                actual: 2,
            },
            Error::PublicationReadinessEvidenceUnsupportedPolicyVersion {
                expected: 1,
                actual: 2,
            },
            Error::PublicationReadinessEvidenceInvalidField { field: "url" },
            Error::PublicationReadinessEvidenceDigestMismatch,
            Error::PublicationReadinessEvidenceNonCanonicalJson,
            Error::PublicationReadinessEvidenceSerialization,
            Error::InvalidAuthorizationContent,
            Error::InvalidAuthorizationAction,
            Error::InvalidAuthorizationServerDomain,
            Error::MissingAuthorizationActionTag,
            Error::DuplicateAuthorizationActionTag,
            Error::MalformedAuthorizationActionTag,
            Error::MissingAuthorizationExpirationTag,
            Error::DuplicateAuthorizationExpirationTag,
            Error::MalformedAuthorizationExpirationTag,
            Error::MalformedAuthorizationServerTag,
            Error::MalformedAuthorizationHashTag,
            Error::InvalidAuthorizationCreatedAge,
            Error::InvalidAuthorizationLifetime,
            Error::AuthorizationTimestampOverflow,
            Error::AuthorizationCreatedInFuture,
            Error::AuthorizationStale,
            Error::AuthorizationExpired,
            Error::AuthorizationActionMismatch,
            Error::AuthorizationServerRequired,
            Error::AuthorizationServerMismatch,
            Error::AuthorizationHashRequired,
            Error::AuthorizationHashMismatch,
        ];
        for error in errors {
            assert!(!error.code().is_empty());
            assert!(!format!("{error}").is_empty());
        }
        let cases = [
            (
                Error::InvalidSha256,
                "invalid_sha256",
                "sha256 must be 64 lowercase hexadecimal characters",
            ),
            (
                Error::InvalidFileExtension,
                "invalid_file_extension",
                "invalid Blossom file extension",
            ),
            (
                Error::InvalidHashPath,
                "invalid_hash_path",
                "invalid Blossom root hash path",
            ),
            (
                Error::InvalidBlobUrl,
                "invalid_blob_url",
                "invalid Blossom blob URL",
            ),
            (
                Error::UnsupportedBlobUrlScheme,
                "unsupported_blob_url_scheme",
                "Blossom blob URL scheme must be http or https",
            ),
            (
                Error::BlobUrlCredentialsForbidden,
                "blob_url_credentials_forbidden",
                "Blossom blob URL credentials are forbidden",
            ),
            (
                Error::BlobUrlQueryForbidden,
                "blob_url_query_forbidden",
                "Blossom blob URL query is forbidden",
            ),
            (
                Error::BlobUrlFragmentForbidden,
                "blob_url_fragment_forbidden",
                "Blossom blob URL fragment is forbidden",
            ),
            (
                Error::InsecureBlobUrl,
                "insecure_blob_url",
                "Radroots blob references require HTTPS or loopback HTTP",
            ),
            (
                Error::DescriptorExtensionRequired,
                "descriptor_extension_required",
                "BUD-02 descriptor URL requires a file extension",
            ),
            (
                Error::DescriptorHashMismatch,
                "descriptor_hash_mismatch",
                "descriptor URL hash does not match descriptor sha256",
            ),
            (
                Error::InvalidMediaType,
                "invalid_media_type",
                "invalid media type",
            ),
            (
                Error::BlobHashMismatch,
                "blob_hash_mismatch",
                "blob bytes do not match descriptor sha256",
            ),
            (
                Error::BlobSizeMismatch {
                    expected: 1,
                    actual: 2,
                },
                "blob_size_mismatch",
                "blob size mismatch: expected 1, got 2",
            ),
            (
                Error::BlobMediaTypeMismatch,
                "blob_media_type_mismatch",
                "descriptor media type does not match the approved media type",
            ),
        ];
        for (error, code, message) in cases {
            assert_eq!(error.code(), code);
            assert_eq!(format!("{error}"), message);
        }
    }

    #[test]
    fn authorization_error_codes_and_messages_are_stable() {
        let cases = [
            (
                Error::InvalidAuthorizationContent,
                "invalid_authorization_content",
                "Blossom authorization content must be bounded human-readable text",
            ),
            (
                Error::InvalidAuthorizationAction,
                "invalid_authorization_action",
                "invalid Blossom authorization action",
            ),
            (
                Error::InvalidAuthorizationServerDomain,
                "invalid_authorization_server_domain",
                "invalid Blossom authorization server domain",
            ),
            (
                Error::MissingAuthorizationActionTag,
                "missing_authorization_action_tag",
                "Blossom authorization is missing a t action tag",
            ),
            (
                Error::DuplicateAuthorizationActionTag,
                "duplicate_authorization_action_tag",
                "Blossom authorization has more than one t action tag",
            ),
            (
                Error::MalformedAuthorizationActionTag,
                "malformed_authorization_action_tag",
                "malformed Blossom authorization t action tag",
            ),
            (
                Error::MissingAuthorizationExpirationTag,
                "missing_authorization_expiration_tag",
                "Blossom authorization is missing an expiration tag",
            ),
            (
                Error::DuplicateAuthorizationExpirationTag,
                "duplicate_authorization_expiration_tag",
                "Blossom authorization has more than one expiration tag",
            ),
            (
                Error::MalformedAuthorizationExpirationTag,
                "malformed_authorization_expiration_tag",
                "malformed Blossom authorization expiration tag",
            ),
            (
                Error::MalformedAuthorizationServerTag,
                "malformed_authorization_server_tag",
                "malformed Blossom authorization server tag",
            ),
            (
                Error::MalformedAuthorizationHashTag,
                "malformed_authorization_hash_tag",
                "malformed Blossom authorization x hash tag",
            ),
            (
                Error::InvalidAuthorizationCreatedAge,
                "invalid_authorization_created_age",
                "Blossom authorization maximum created age must not exceed 300 seconds",
            ),
            (
                Error::InvalidAuthorizationLifetime,
                "invalid_authorization_lifetime",
                "Blossom authorization lifetime must be between 1 and 300 seconds",
            ),
            (
                Error::AuthorizationTimestampOverflow,
                "authorization_timestamp_overflow",
                "Blossom authorization expiration timestamp overflows u64",
            ),
            (
                Error::AuthorizationCreatedInFuture,
                "authorization_created_in_future",
                "Blossom authorization must be created in the past",
            ),
            (
                Error::AuthorizationStale,
                "authorization_stale",
                "Blossom authorization is outside the accepted creation-age window",
            ),
            (
                Error::AuthorizationExpired,
                "authorization_expired",
                "Blossom authorization is expired",
            ),
            (
                Error::AuthorizationActionMismatch,
                "authorization_action_mismatch",
                "Blossom authorization action does not match the target endpoint",
            ),
            (
                Error::AuthorizationServerRequired,
                "authorization_server_required",
                "Blossom authorization requires a server scope",
            ),
            (
                Error::AuthorizationServerMismatch,
                "authorization_server_mismatch",
                "Blossom authorization does not include the target server",
            ),
            (
                Error::AuthorizationHashRequired,
                "authorization_hash_required",
                "Blossom authorization requires an x hash scope",
            ),
            (
                Error::AuthorizationHashMismatch,
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
