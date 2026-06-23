use alloc::string::String;
use core::fmt;
use radroots_simplex_smp_proto::prelude::RadrootsSimplexSmpProtoError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadrootsSimplexAgentUnsupportedLinkKind {
    FullContactLink,
    ContactAddress,
    Group,
    Channel,
    Relay,
    File,
    Xrcp,
    Bot,
    Unknown(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadrootsSimplexAgentProtoError {
    Proto(RadrootsSimplexSmpProtoError),
    UnexpectedEof,
    InvalidTag(String),
    InvalidUtf8(String),
    InvalidShortFieldLength(usize),
    InvalidLargeFieldLength(usize),
    InvalidBoolEncoding(u8),
    InvalidRatchetHeader(String),
    InvalidE2eParameters(String),
    InvalidBase64Url {
        field: &'static str,
        value: String,
    },
    InvalidLink(String),
    InvalidLinkFieldLength {
        field: &'static str,
        expected: usize,
        actual: usize,
    },
    InvalidLinkParameter {
        key: String,
        reason: String,
    },
    InvalidPort(String),
    UnsupportedLink(RadrootsSimplexAgentUnsupportedLinkKind),
    TrailingBytes,
}

impl From<RadrootsSimplexSmpProtoError> for RadrootsSimplexAgentProtoError {
    fn from(value: RadrootsSimplexSmpProtoError) -> Self {
        Self::Proto(value)
    }
}

impl fmt::Display for RadrootsSimplexAgentProtoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Proto(error) => write!(f, "{error}"),
            Self::UnexpectedEof => write!(f, "unexpected end of SimpleX agent input"),
            Self::InvalidTag(tag) => write!(f, "invalid SimpleX agent tag `{tag}`"),
            Self::InvalidUtf8(error) => write!(f, "invalid UTF-8 in SimpleX agent field: {error}"),
            Self::InvalidShortFieldLength(length) => {
                write!(f, "invalid SimpleX agent short field length {length}")
            }
            Self::InvalidLargeFieldLength(length) => {
                write!(f, "invalid SimpleX agent large field length {length}")
            }
            Self::InvalidBoolEncoding(value) => {
                write!(f, "invalid SimpleX agent bool encoding `{value}`")
            }
            Self::InvalidRatchetHeader(error) => {
                write!(f, "invalid SimpleX agent ratchet header: {error}")
            }
            Self::InvalidE2eParameters(error) => {
                write!(f, "invalid SimpleX agent E2E parameters: {error}")
            }
            Self::InvalidBase64Url { field, value } => {
                write!(
                    f,
                    "invalid SimpleX agent base64url value for `{field}`: `{value}`"
                )
            }
            Self::InvalidLink(link) => write!(f, "invalid SimpleX agent link: {link}"),
            Self::InvalidLinkFieldLength {
                field,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "invalid SimpleX agent link `{field}` length {actual}, expected {expected}"
                )
            }
            Self::InvalidLinkParameter { key, reason } => {
                write!(f, "invalid SimpleX agent link parameter `{key}`: {reason}")
            }
            Self::InvalidPort(port) => write!(f, "invalid SimpleX agent link port `{port}`"),
            Self::UnsupportedLink(kind) => {
                write!(f, "unsupported SimpleX agent link kind `{kind}`")
            }
            Self::TrailingBytes => write!(f, "trailing bytes after SimpleX agent decode"),
        }
    }
}

impl fmt::Display for RadrootsSimplexAgentUnsupportedLinkKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FullContactLink => write!(f, "full-contact-link"),
            Self::ContactAddress => write!(f, "contact-address"),
            Self::Group => write!(f, "group"),
            Self::Channel => write!(f, "channel"),
            Self::Relay => write!(f, "relay"),
            Self::File => write!(f, "file"),
            Self::Xrcp => write!(f, "xrcp"),
            Self::Bot => write!(f, "bot"),
            Self::Unknown(value) => write!(f, "unknown:{value}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsSimplexAgentProtoError {}
