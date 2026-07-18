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
