#[cfg(not(feature = "std"))]
use alloc::string::String;
use core::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadrootsSimplexSmpProtoError {
    UnexpectedEof,

    InvalidTag(String),

    UnsupportedTag(String),

    InvalidUtf8(String),

    InvalidBase64Url { field: &'static str, value: String },

    InvalidVersionRange(String),

    InvalidUri(String),

    InvalidHostList(String),

    InvalidPort(String),

    InvalidShortFieldLength(usize),

    InvalidLargeFieldLength(usize),

    InvalidListLength(usize),

    InvalidCorrelationIdLength(usize),

    InvalidNonceLength(usize),

    InvalidMaybeTag(u8),

    InvalidBoolEncoding(u8),

    MissingField(&'static str),

    TrailingBytes,

    UnsupportedTransportVersion(u16),
}

impl fmt::Display for RadrootsSimplexSmpProtoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEof => write!(f, "unexpected end of SMP input"),
            Self::InvalidTag(tag) => write!(f, "invalid SMP ASCII tag `{tag}`"),
            Self::UnsupportedTag(tag) => write!(f, "unsupported SMP tag `{tag}`"),
            Self::InvalidUtf8(error) => write!(f, "invalid UTF-8 in SMP field: {error}"),
            Self::InvalidBase64Url { field, value } => {
                write!(f, "invalid base64url value for `{field}`: `{value}`")
            }
            Self::InvalidVersionRange(range) => write!(f, "invalid SMP version range `{range}`"),
            Self::InvalidUri(uri) => write!(f, "invalid SMP URI: {uri}"),
            Self::InvalidHostList(hosts) => write!(f, "invalid SMP host list `{hosts}`"),
            Self::InvalidPort(port) => write!(f, "invalid SMP port `{port}`"),
            Self::InvalidShortFieldLength(length) => {
                write!(f, "invalid SMP short field length {length}")
            }
            Self::InvalidLargeFieldLength(length) => {
                write!(f, "invalid SMP large field length {length}")
            }
            Self::InvalidListLength(length) => write!(f, "invalid SMP list length {length}"),
            Self::InvalidCorrelationIdLength(length) => {
                write!(f, "invalid SMP correlation id length {length}")
            }
            Self::InvalidNonceLength(length) => write!(f, "invalid SMP nonce length {length}"),
            Self::InvalidMaybeTag(tag) => write!(f, "invalid SMP maybe tag `{tag}`"),
            Self::InvalidBoolEncoding(value) => write!(f, "invalid SMP bool encoding `{value}`"),
            Self::MissingField(field) => write!(f, "missing required SMP field `{field}`"),
            Self::TrailingBytes => write!(f, "trailing SMP bytes after parse"),
            Self::UnsupportedTransportVersion(version) => {
                write!(f, "unsupported SMP transport version {version}")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsSimplexSmpProtoError {}
