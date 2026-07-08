use core::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsMeshError {
    EmptyCustomScope,
    EmptyMessageId,
    InvalidTtl,
    PayloadTransmissionForbidden,
    InvalidCbor,
    InvalidUtf8,
    UnknownFrameType,
    UnknownScope,
    UnsupportedVersion,
}

impl fmt::Display for RadrootsMeshError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyCustomScope => f.write_str("mesh custom scope is empty"),
            Self::EmptyMessageId => f.write_str("mesh message id is empty"),
            Self::InvalidTtl => f.write_str("mesh frame TTL is invalid"),
            Self::PayloadTransmissionForbidden => {
                f.write_str("mesh payload transmission is forbidden")
            }
            Self::InvalidCbor => f.write_str("mesh frame CBOR is invalid"),
            Self::InvalidUtf8 => f.write_str("mesh frame text is invalid UTF-8"),
            Self::UnknownFrameType => f.write_str("mesh frame type is unknown"),
            Self::UnknownScope => f.write_str("mesh scope is unknown"),
            Self::UnsupportedVersion => f.write_str("mesh frame version is unsupported"),
        }
    }
}
