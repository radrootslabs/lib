#[cfg(not(feature = "std"))]
use alloc::string::String;
use core::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadrootsSimplexChatProtoError {
    EmptyInput,
    InvalidUtf8,
    InvalidJson(String),
    InvalidVersionRange(String),
    InvalidBase64Url { field: &'static str, value: String },
    MissingField(&'static str),
    InvalidField(&'static str),
    InvalidCompressedEnvelope(String),
    CompressedMessageTooLarge(usize),
    CompressionUnavailable,
    UnsupportedBinaryMessage,
}

impl fmt::Display for RadrootsSimplexChatProtoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyInput => write!(f, "empty SimpleX chat input"),
            Self::InvalidUtf8 => write!(f, "invalid UTF-8 in SimpleX chat input"),
            Self::InvalidJson(error) => write!(f, "invalid SimpleX chat JSON: {error}"),
            Self::InvalidVersionRange(range) => {
                write!(f, "invalid SimpleX chat version range `{range}`")
            }
            Self::InvalidBase64Url { field, value } => {
                write!(f, "invalid base64url value for `{field}`: `{value}`")
            }
            Self::MissingField(field) => write!(f, "missing required SimpleX chat field `{field}`"),
            Self::InvalidField(field) => write!(f, "invalid SimpleX chat field `{field}`"),
            Self::InvalidCompressedEnvelope(error) => {
                write!(f, "invalid compressed SimpleX chat envelope: {error}")
            }
            Self::CompressedMessageTooLarge(length) => {
                write!(f, "compressed SimpleX chat message exceeds limit: {length}")
            }
            Self::CompressionUnavailable => {
                write!(
                    f,
                    "SimpleX chat compression support requires the `std` feature"
                )
            }
            Self::UnsupportedBinaryMessage => {
                write!(
                    f,
                    "binary SimpleX chat messages are not supported by this crate"
                )
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsSimplexChatProtoError {}
