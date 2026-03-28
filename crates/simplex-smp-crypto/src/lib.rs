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
        verify_signature,
    };
    pub use crate::error::RadrootsSimplexSmpCryptoError;
    pub use crate::message::{
        RADROOTS_SIMPLEX_SMP_NONCE_LENGTH, RADROOTS_SIMPLEX_SMP_SHARED_SECRET_LENGTH,
        RadrootsSimplexSmpX25519Keypair, decrypt_no_pad, decrypt_padded, derive_shared_secret,
        encrypt_no_pad, encrypt_padded, random_nonce,
    };
    pub use crate::ratchet::{
        RadrootsSimplexSmpRatchetHeader, RadrootsSimplexSmpRatchetRole,
        RadrootsSimplexSmpRatchetState,
    };
}
