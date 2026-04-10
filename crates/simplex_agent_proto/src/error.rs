use alloc::string::String;
use core::fmt;
use radroots_simplex_smp_proto::prelude::RadrootsSimplexSmpProtoError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadrootsSimplexAgentProtoError {
    Proto(RadrootsSimplexSmpProtoError),
    UnexpectedEof,
    InvalidTag(String),
    InvalidUtf8(String),
    InvalidShortFieldLength(usize),
    InvalidLargeFieldLength(usize),
    InvalidBoolEncoding(u8),
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
            Self::TrailingBytes => write!(f, "trailing bytes after SimpleX agent decode"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsSimplexAgentProtoError {}
