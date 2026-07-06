use core::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsMeshError {
    EmptyCustomScope,
    PayloadTransmissionForbidden,
    InvalidCbor,
    InvalidUtf8,
    UnknownScope,
    UnknownPayloadPolicy,
    UnsupportedVersion,
}

impl fmt::Display for RadrootsMeshError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyCustomScope => f.write_str("mesh custom scope is empty"),
            Self::PayloadTransmissionForbidden => {
                f.write_str("mesh payload transmission is forbidden")
            }
            Self::InvalidCbor => f.write_str("mesh frame CBOR is invalid"),
            Self::InvalidUtf8 => f.write_str("mesh frame text is invalid UTF-8"),
            Self::UnknownScope => f.write_str("mesh scope is unknown"),
            Self::UnknownPayloadPolicy => f.write_str("mesh payload policy is unknown"),
            Self::UnsupportedVersion => f.write_str("mesh frame version is unsupported"),
        }
    }
}
