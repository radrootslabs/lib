use alloc::string::String;
use core::fmt;
use radroots_simplex_smp_crypto::prelude::RadrootsSimplexSmpCryptoError;
use radroots_simplex_smp_proto::prelude::RadrootsSimplexSmpProtoError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadrootsSimplexSmpTransportError {
    Proto(RadrootsSimplexSmpProtoError),
    Crypto(RadrootsSimplexSmpCryptoError),
    InvalidPaddedBlockLength { expected: usize, actual: usize },
    TransportPayloadTooLarge(usize),
    EmptyTransportBlock,
    TransmissionCountOverflow(usize),
    TransmissionTooLarge(usize),
    InvalidPadding { index: usize, value: u8 },
    UnexpectedTransmissionCount { declared: u8, actual: usize },
    TrailingTransportBytes(usize),
    MissingHandshakeField(&'static str),
    InvalidSessionIdentifierLength(usize),
    MissingServerProof,
    InvalidCertificateChainLength(usize),
    UnsupportedAlpn(String),
    SessionResumptionNotAllowed,
    ServerIdentityMismatch { expected: String, actual: String },
    MissingChannelBinding,
    SessionBindingMismatch,
    NoMutualTransportVersion { offered: String, supported: String },
    MissingCorrelationId,
    InvalidServerAddress(String),
    LiveTransportIo(String),
    MissingPeerCertificates,
    UnexpectedBrokerTransmissionCount(usize),
    CorrelationIdMismatch,
}

impl From<RadrootsSimplexSmpProtoError> for RadrootsSimplexSmpTransportError {
    fn from(value: RadrootsSimplexSmpProtoError) -> Self {
        Self::Proto(value)
    }
}

impl From<RadrootsSimplexSmpCryptoError> for RadrootsSimplexSmpTransportError {
    fn from(value: RadrootsSimplexSmpCryptoError) -> Self {
        Self::Crypto(value)
    }
}

impl fmt::Display for RadrootsSimplexSmpTransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Proto(error) => write!(f, "{error}"),
            Self::Crypto(error) => write!(f, "{error}"),
            Self::InvalidPaddedBlockLength { expected, actual } => {
                write!(
                    f,
                    "invalid SMP padded block length {actual}, expected {expected}"
                )
            }
            Self::TransportPayloadTooLarge(length) => {
                write!(f, "SMP transport payload too large: {length} bytes")
            }
            Self::EmptyTransportBlock => write!(f, "empty SMP transport block"),
            Self::TransmissionCountOverflow(count) => {
                write!(f, "too many SMP transmissions for one block: {count}")
            }
            Self::TransmissionTooLarge(length) => {
                write!(f, "SMP transmission too large for word16 framing: {length}")
            }
            Self::InvalidPadding { index, value } => {
                write!(
                    f,
                    "invalid SMP transport padding byte {value:#04x} at index {index}"
                )
            }
            Self::UnexpectedTransmissionCount { declared, actual } => {
                write!(
                    f,
                    "declared {declared} SMP transmissions but decoded {actual}"
                )
            }
            Self::TrailingTransportBytes(length) => {
                write!(f, "trailing SMP transport bytes after decode: {length}")
            }
            Self::MissingHandshakeField(field) => {
                write!(f, "missing required SMP handshake field `{field}`")
            }
            Self::InvalidSessionIdentifierLength(length) => {
                write!(f, "invalid SMP session identifier length {length}")
            }
            Self::MissingServerProof => write!(f, "missing SMP server proof in handshake"),
            Self::InvalidCertificateChainLength(length) => {
                write!(f, "invalid SMP certificate chain length {length}")
            }
            Self::UnsupportedAlpn(alpn) => write!(f, "unsupported SMP ALPN `{alpn}`"),
            Self::SessionResumptionNotAllowed => {
                write!(f, "SMP TLS session resumption is not allowed")
            }
            Self::ServerIdentityMismatch { expected, actual } => {
                write!(
                    f,
                    "SMP server identity mismatch: expected `{expected}`, got `{actual}`"
                )
            }
            Self::MissingChannelBinding => write!(f, "missing SMP tls-unique channel binding"),
            Self::SessionBindingMismatch => {
                write!(
                    f,
                    "SMP session identifier does not match tls-unique binding"
                )
            }
            Self::NoMutualTransportVersion { offered, supported } => {
                write!(
                    f,
                    "no mutual SMP transport version between `{offered}` and `{supported}`"
                )
            }
            Self::MissingCorrelationId => {
                write!(f, "SMP transport request is missing a correlation id")
            }
            Self::InvalidServerAddress(message) => write!(f, "{message}"),
            Self::LiveTransportIo(message) => write!(f, "{message}"),
            Self::MissingPeerCertificates => {
                write!(f, "SMP TLS peer certificate chain is missing")
            }
            Self::UnexpectedBrokerTransmissionCount(count) => {
                write!(
                    f,
                    "expected exactly one SMP broker transmission, got {count}"
                )
            }
            Self::CorrelationIdMismatch => {
                write!(
                    f,
                    "SMP broker response correlation id did not match the request"
                )
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsSimplexSmpTransportError {}
