#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod auth;
pub mod error;
pub mod message;
pub mod ratchet;

pub mod prelude {
    pub use crate::auth::{
        RadrootsSimplexSmpCommandAuthorization, RadrootsSimplexSmpEd25519Keypair,
        RadrootsSimplexSmpQueueAuthorizationMaterial, RadrootsSimplexSmpQueueAuthorizationScope,
        decode_x25519_public_key_x509, encode_ed25519_public_key_x509,
        encode_x25519_public_key_x509, verify_signature,
    };
    pub use crate::error::RadrootsSimplexSmpCryptoError;
    pub use crate::message::{
        RADROOTS_SIMPLEX_SMP_NONCE_LENGTH, RADROOTS_SIMPLEX_SMP_SHARED_SECRET_LENGTH,
        RadrootsSimplexSmpSecretBoxChainKey, RadrootsSimplexSmpX25519Keypair,
        advance_secretbox_chain, decrypt_no_pad, decrypt_padded, derive_shared_secret,
        encrypt_no_pad, encrypt_padded, init_secretbox_chain, random_nonce,
    };
    pub use crate::ratchet::{
        RadrootsSimplexSmpRatchetHeader, RadrootsSimplexSmpRatchetRole,
        RadrootsSimplexSmpRatchetState,
    };
}
