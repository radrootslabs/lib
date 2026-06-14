#![forbid(unsafe_code)]

use thiserror::Error;

#[cfg(not(feature = "std"))]
use alloc::string::String;
#[cfg(feature = "std")]
use std::string::String;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RadrootsAuthorityError {
    #[error("invalid actor public key")]
    InvalidActorPubkey,

    #[error("invalid signer public key")]
    InvalidSignerPubkey,

    #[error("signer error: {0}")]
    Signer(#[from] RadrootsSignerError),
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RadrootsSignerError {
    #[error("signer unavailable")]
    Unavailable,

    #[error("signer rejected draft")]
    Rejected,

    #[error("signing failed: {message}")]
    SigningFailed { message: String },
}
