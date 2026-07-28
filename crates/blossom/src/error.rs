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
