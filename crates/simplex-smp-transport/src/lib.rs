#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod error;
pub mod frame;
pub mod handshake;

pub mod prelude {
    pub use crate::error::RadrootsSimplexSmpTransportError;
    pub use crate::frame::{
        RADROOTS_SIMPLEX_SMP_TRANSPORT_BLOCK_SIZE, RADROOTS_SIMPLEX_SMP_TRANSPORT_PAD_BYTE,
        RadrootsSimplexSmpTransportBlock, decode_padded_bytes, encode_padded_bytes,
    };
    pub use crate::handshake::{
        RADROOTS_SIMPLEX_SMP_TLS_ALPN_V1, RADROOTS_SIMPLEX_SMP_TLS_KEY_EXCHANGE_GROUP,
        RADROOTS_SIMPLEX_SMP_TLS_SIGNATURE_ALGORITHM, RADROOTS_SIMPLEX_SMP_TLS_V1_3_CIPHER_SUITE,
        RadrootsSimplexSmpClientHello, RadrootsSimplexSmpServerHello,
        RadrootsSimplexSmpTlsHandshakeEvidence, RadrootsSimplexSmpTlsPolicy,
        RadrootsSimplexSmpTransportServerProof, negotiate_transport_version,
        validate_tls_handshake,
    };
}
